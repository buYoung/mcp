mod syntax;
use super::*;
use crate::parser::{CodeRange, ExtractedSymbol, NavigationFile};
use std::collections::HashMap;
use syntax::*;
use tree_sitter::{Node, Tree};

struct Collector<'a> {
    source: &'a [u8],
    unit: FlowUnit,
    issues: Vec<FlowIssue>,
    functions: Vec<Node<'a>>,
    function_ids: HashMap<usize, usize>,
    classes: Vec<Node<'a>>,
    class_ids: HashMap<usize, usize>,
    declarations: HashMap<usize, Vec<usize>>,
    active_function: usize,
    statements_visited: usize,
}

pub(crate) fn collect(
    language: &str,
    tree: &Tree,
    source: &[u8],
    symbols: &[ExtractedSymbol],
    navigation: &NavigationFile,
) -> Option<FlowFile> {
    if !super::supports(language) {
        return None;
    }
    let root = tree.root_node();
    let mut collector = Collector {
        source,
        unit: FlowUnit {
            language: language.into(),
            imports: navigation
                .imports
                .iter()
                .take(NODES_PER_FILE)
                .cloned()
                .collect(),
            ..Default::default()
        },
        issues: Vec::new(),
        functions: vec![root],
        function_ids: HashMap::new(),
        classes: Vec::new(),
        class_ids: HashMap::new(),
        declarations: HashMap::new(),
        active_function: 0,
        statements_visited: 0,
    };
    collector.unit.functions.push(FunctionSummary {
        name: "<module>".into(),
        range: range(root),
        parameters: Vec::new(),
        body: Vec::new(),
        owner: None,
        is_available: true,
        is_method: false,
        is_static: false,
        has_lexical_receiver: false,
    });
    collector.unit.nodes.push(Expression {
        range: range(root),
        function: 0,
        kind: ExpressionKind::Unknown {
            reason: "analysis extraction budget exceeded".into(),
            inputs: Vec::new(),
        },
    });
    let mut pending = vec![root];
    let mut visited = 0;
    while let Some(node) = pending.pop() {
        visited += 1;
        if visited > 65_536 {
            collector.issue(node, "syntax traversal budget exceeded");
            break;
        }
        if is_class(node) && collector.classes.len() < FUNCTIONS_PER_FILE {
            if let Some(name) = name(node, source) {
                let id = collector.unit.classes.len();
                collector.class_ids.insert(node.id(), id);
                collector.classes.push(node);
                collector.unit.classes.push(Class {
                    name,
                    range: range(node),
                    constructor: None,
                    methods: Default::default(),
                    fields: Vec::new(),
                    is_available: !matches!(
                        node.kind(),
                        "struct_declaration" | "struct_specifier" | "struct_item" | "type_spec"
                    ) && !node.has_error()
                        && !children(node).iter().any(|child| {
                            matches!(
                                child.kind(),
                                "class_heritage"
                                    | "superclass"
                                    | "superclasses"
                                    | "argument_list"
                                    | "decorator"
                            )
                        })
                        && !node
                            .parent()
                            .is_some_and(|parent| parent.kind() == "decorated_definition"),
                });
            }
        }
        if is_function(node) {
            if collector.functions.len() >= FUNCTIONS_PER_FILE {
                collector.issue(node, "function summary budget exceeded");
                continue;
            }
            let id = collector.functions.len();
            collector.function_ids.insert(node.id(), id);
            collector.functions.push(node);
            let symbol = symbols
                .iter()
                .find(|symbol| symbol.kind == "fn" && symbol.range == range(node));
            let name = name(node, source)
                .or_else(|| symbol.map(|symbol| symbol.name.clone()))
                .unwrap_or_else(|| "<closure>".into());
            collector.unit.functions.push(FunctionSummary {
                name,
                range: range(node),
                parameters: Vec::new(),
                body: Vec::new(),
                owner: None,
                is_available: body(node).is_some()
                    && !node.has_error()
                    && !has_opaque_modifiers(node, source)
                    && !matches!(language, "bash" | "zsh" | "powershell")
                    && (!has_conditional_parent(node)
                        || matches!(
                            node.kind(),
                            "arrow_function"
                                | "lambda"
                                | "lambda_expression"
                                | "function_expression"
                                | "anonymous_function"
                                | "closure_expression"
                        )),
                is_method: is_method(node),
                is_static: text(node, source)
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .any(|word| word == "static"),
                has_lexical_receiver: matches!(
                    node.kind(),
                    "arrow_function" | "lambda" | "lambda_expression" | "closure_expression"
                ),
            });
        }
        let mut children = children(node);
        children.reverse();
        pending.extend(children);
    }
    collector.bind_declarations(root, navigation);
    collector.bind_class_fields();
    for id in 0..collector.functions.len() {
        collector.active_function = id;
        let node = collector.functions[id];
        let body = if id == 0 { Some(node) } else { body(node) };
        if let Some(body) = body {
            collector.unit.functions[id].body = collector.statements(body, id);
        }
    }
    collector.exports(root);
    if collector.unit.bindings.len() >= NODES_PER_FILE {
        collector.issue(
            root,
            "binding budget exceeded; incomplete name resolution withheld",
        );
        for function in &mut collector.unit.functions {
            function.is_available = false;
        }
        for class in &mut collector.unit.classes {
            class.is_available = false;
        }
    }
    Some(FlowFile {
        digest: crate::implementations::digest(source),
        units: vec![collector.unit],
        omissions: collector.issues,
    })
}

impl<'a> Collector<'a> {
    fn issue(&mut self, node: Node<'_>, reason: &str) {
        if self.issues.len() < 16
            && !self
                .issues
                .iter()
                .any(|issue| issue.range == range(node) && issue.reason == reason)
        {
            self.issues.push(FlowIssue {
                range: range(node),
                reason: reason.into(),
            });
        }
    }
    fn binding(
        &mut self,
        name: String,
        node: Node<'_>,
        scope: CodeRange,
        kind: BindingKind,
    ) -> usize {
        let id = self.unit.bindings.len();
        self.unit.bindings.push(Binding {
            name,
            range: range(node),
            scope,
            kind,
            initializer: None,
            is_mutated: false,
        });
        id
    }
    fn bind_declarations(&mut self, root: Node<'a>, navigation: &NavigationFile) {
        for id in 0..self.classes.len() {
            let node = self.classes[id];
            self.binding(
                self.unit.classes[id].name.clone(),
                node,
                range(scope(node)),
                BindingKind::Class(id),
            );
        }
        for id in 1..self.functions.len() {
            if self.unit.bindings.len() >= NODES_PER_FILE {
                break;
            }
            let node = self.functions[id];
            let mut parent = node.parent();
            let mut owner = None;
            while let Some(candidate) = parent {
                if let Some(&class) = self.class_ids.get(&candidate.id()) {
                    owner = Some(class);
                    break;
                }
                if is_function(candidate) {
                    break;
                }
                parent = candidate.parent();
            }
            self.unit.functions[id].owner = owner;
            if let Some(owner) = owner {
                let name = self.unit.functions[id].name.clone();
                if (self.unit.language == "python"
                    && matches!(
                        name.as_str(),
                        "__new__" | "__getattribute__" | "__getattr__" | "__setattr__"
                    ))
                    || has_accessor_modifiers(node, self.source)
                {
                    self.unit.classes[owner].is_available = false;
                }
                if matches!(name.as_str(), "constructor" | "__init__" | "initialize")
                    || name == self.unit.classes[owner].name
                    || node.kind() == "constructor_declaration"
                {
                    if self.unit.classes[owner].constructor.replace(id).is_some() {
                        self.unit.classes[owner].is_available = false;
                    }
                } else {
                    if let Some(previous) = self.unit.classes[owner].methods.insert(name, id) {
                        self.unit.functions[previous].is_available = false;
                        self.unit.functions[id].is_available = false;
                        self.issue(
                            node,
                            "overloaded/duplicate method requires type or runtime resolution",
                        );
                    }
                }
            } else if (!is_method(node) || matches!(self.unit.language.as_str(), "ruby" | "groovy"))
                && !matches!(
                    node.kind(),
                    "arrow_function"
                        | "lambda"
                        | "lambda_expression"
                        | "function_expression"
                        | "anonymous_function"
                        | "closure_expression"
                )
            {
                if let Some(name) = name(node, self.source) {
                    self.binding(name, node, range(scope(node)), BindingKind::Function(id));
                }
            }
            let parameters = parameters(node);
            if parameters.len() > 32 {
                self.unit.functions[id].is_available = false;
                self.issue(node, "parameter budget exceeded");
            }
            for parameter in parameters.into_iter().take(32) {
                if self.unit.bindings.len() >= NODES_PER_FILE {
                    break;
                }
                if self.unit.language == "c" && text(parameter, self.source).trim() == "void" {
                    continue;
                }
                let Some(name) = parameter_name(parameter, self.source) else {
                    self.unit.functions[id].is_available = false;
                    self.issue(
                        parameter,
                        "destructured/variadic parameter is not summarized",
                    );
                    continue;
                };
                let position = self.unit.functions[id].parameters.len();
                let binding = self.binding(
                    name,
                    parameter,
                    range(node),
                    BindingKind::Parameter {
                        function: id,
                        index: position,
                    },
                );
                self.unit.functions[id].parameters.push(binding);
            }
        }
        for (id, import) in navigation.imports.iter().enumerate() {
            if self.unit.bindings.len() >= NODES_PER_FILE {
                break;
            }
            let node = node_at(root, &import.range).unwrap_or(root);
            self.binding(
                import.local_name.clone(),
                node,
                range(scope(node)),
                BindingKind::Import(id),
            );
        }
        let mut pending = vec![root];
        let mut visited = 0;
        while let Some(node) = pending.pop() {
            visited += 1;
            if visited > 65_536 {
                break;
            }
            if is_binding(node) {
                if self.unit.bindings.len() >= NODES_PER_FILE {
                    self.issue(node, "binding budget exceeded");
                    break;
                }
                if let Some((pattern, _)) = binding_parts(node) {
                    let names = pattern_names(pattern, self.source);
                    let scope = range(scope(node));
                    for name in names {
                        if name == "_" && matches!(self.unit.language.as_str(), "go" | "rust") {
                            continue;
                        }
                        let id = self.binding(name, node, scope.clone(), BindingKind::Local);
                        self.declarations.entry(node.id()).or_default().push(id);
                    }
                }
            }
            let mut children = children(node);
            children.reverse();
            pending.extend(children);
        }
        let mut pending = vec![root];
        let mut visited = 0;
        while let Some(node) = pending.pop() {
            visited += 1;
            if visited > 65_536 {
                break;
            }
            let target = assignment(node).map(|(target, _)| target).or_else(|| {
                matches!(
                    node.kind(),
                    "update_expression" | "prefix_unary_expression" | "postfix_unary_expression"
                )
                .then(|| {
                    node.child_by_field_name("argument")
                        .or_else(|| node.child_by_field_name("operand"))
                })
                .flatten()
            });
            if let Some(target) = target {
                let target_text = text(target, self.source);
                self.unit.has_modified_map_builtin |= target_text == "Map"
                    || target_text.starts_with("Map.")
                    || target_text.starts_with("Map[")
                    || target_text.starts_with("globalThis.Map");
                for name in pattern_names(target, self.source) {
                    for binding in &mut self.unit.bindings {
                        if binding.name == name
                            && binding.range != range(node)
                            && encloses(&binding.scope, &range(node))
                        {
                            binding.is_mutated = true;
                        }
                    }
                }
            }
            let mut children = children(node);
            children.reverse();
            pending.extend(children);
        }
    }
    fn lookup(&self, name: &str, location: &CodeRange) -> Option<usize> {
        self.unit
            .bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| binding.name == name && encloses(&binding.scope, location))
            .min_by_key(|(_, binding)| {
                (
                    binding.scope.end_line - binding.scope.start_line,
                    binding
                        .scope
                        .end_col
                        .saturating_sub(binding.scope.start_col),
                    std::cmp::Reverse((binding.range.start_line, binding.range.start_col)),
                )
            })
            .map(|(id, _)| id)
    }
    fn bind_class_fields(&mut self) {
        // Only explicit JS/TS instance field initializers have the allocation semantics
        // used here. Constructor assignments in other languages use the same field IR.
        if !matches!(self.unit.language.as_str(), "javascript" | "typescript") {
            return;
        }
        for id in 0..self.classes.len() {
            let node = self.classes[id];
            let Some(body) = node.child_by_field_name("body") else {
                continue;
            };
            for member in children(body).into_iter().take(64) {
                if !matches!(
                    member.kind(),
                    "public_field_definition" | "field_definition"
                ) || has_opaque_modifiers(member, self.source)
                    || text(member, self.source)
                        .split_whitespace()
                        .take(3)
                        .any(|word| word == "static")
                {
                    continue;
                }
                let Some(name) = member
                    .child_by_field_name("name")
                    .or_else(|| member.child_by_field_name("property"))
                else {
                    continue;
                };
                if !is_identifier(name) {
                    continue;
                }
                if let Some(value) = member.child_by_field_name("value") {
                    let value = self.expression(value, 0);
                    self.unit.classes[id]
                        .fields
                        .push((text(name, self.source).into(), value));
                }
            }
        }
    }
    fn push(&mut self, node: Node<'_>, kind: ExpressionKind) -> usize {
        if self.unit.nodes.len() >= NODES_PER_FILE {
            self.issue(node, "expression budget exceeded");
            return 0;
        }
        let id = self.unit.nodes.len();
        self.unit.nodes.push(Expression {
            range: range(node),
            function: self.active_function,
            kind,
        });
        id
    }
    fn expression(&mut self, node: Node<'a>, depth: usize) -> usize {
        if depth > 32 || self.unit.nodes.len() >= NODES_PER_FILE {
            self.issue(node, "expression depth/node budget exceeded");
            return 0;
        }
        if let Some(&function) = self.function_ids.get(&node.id()) {
            return self.push(node, ExpressionKind::Function(function));
        }
        if let Some(value) = literal(node, self.source) {
            return self.push(node, ExpressionKind::Literal(value));
        }
        if matches!(
            node.kind(),
            "tuple_expression" | "tuple" | "unit_expression" | "expression_list"
        ) {
            let items = children(node);
            if items.len() > 32 {
                self.issue(node, "tuple item budget exceeded");
                return 0;
            }
            let items = items
                .into_iter()
                .map(|item| self.expression(item, depth + 1))
                .collect();
            return self.push(node, ExpressionKind::Tuple(items));
        }
        if let Some((condition, consequence, alternative)) = conditional_parts(node) {
            let condition = self.expression(condition, depth + 1);
            let consequence = self.branch_expression(consequence, depth + 1);
            let alternative = alternative
                .map(|node| self.branch_expression(node, depth + 1))
                .unwrap_or_else(|| self.push(node, ExpressionKind::Tuple(Vec::new())));
            return self.push(
                node,
                ExpressionKind::Conditional {
                    condition,
                    consequence,
                    alternative,
                },
            );
        }
        if matches!(
            node.kind(),
            "unary_expression" | "unary_operator" | "not_operator"
        ) {
            if let Some(operand) = node
                .child_by_field_name("argument")
                .or_else(|| node.child_by_field_name("operand"))
                .or_else(|| node.named_child(0))
            {
                let operator =
                    std::str::from_utf8(&self.source[node.start_byte()..operand.start_byte()])
                        .unwrap_or("")
                        .trim();
                if matches!(operator, "!" | "not") {
                    let operand = self.expression(operand, depth + 1);
                    return self.push(node, ExpressionKind::Not(operand));
                }
            }
        }
        if matches!(
            node.kind(),
            "binary_expression" | "binary_operator" | "comparison_operator" | "boolean_operator"
        ) {
            if let Some((left, right)) = node
                .child_by_field_name("left")
                .zip(node.child_by_field_name("right"))
            {
                let operator =
                    std::str::from_utf8(&self.source[left.end_byte()..right.start_byte()])
                        .unwrap_or("")
                        .trim();
                if matches!(operator, "&&" | "||" | "and" | "or") {
                    let left = self.expression(left, depth + 1);
                    let right = self.expression(right, depth + 1);
                    return self.push(
                        node,
                        ExpressionKind::Logical {
                            left,
                            right,
                            is_and: matches!(operator, "&&" | "and"),
                        },
                    );
                }
                if matches!(operator, "==" | "===" | "!=" | "!==") {
                    let left = self.expression(left, depth + 1);
                    let right = self.expression(right, depth + 1);
                    return self.push(
                        node,
                        ExpressionKind::Equal {
                            left,
                            right,
                            is_negated: operator.starts_with('!'),
                        },
                    );
                }
            }
        }
        if is_identifier(node) {
            let name = identifier(node, self.source);
            let binding = self.lookup(&name, &range(node));
            return self.push(node, ExpressionKind::Read { name, binding });
        }
        if let Some((object, key, is_static)) = field_parts(node) {
            if self.unit.language == "python" && node.kind() == "attribute" {
                self.issue(
                    node,
                    "Python attribute/descriptor lookup is distinct from dictionary indexing",
                );
                return self.push(node, ExpressionKind::Unknown { reason: "Python attribute/descriptor lookup is distinct from dictionary indexing".into(), inputs: Vec::new() });
            }
            let object = self.expression(object, depth + 1);
            let key = if is_static {
                self.push(
                    key,
                    ExpressionKind::Literal(Constant::String(text(key, self.source).to_string())),
                )
            } else {
                self.expression(key, depth + 1)
            };
            return self.push(node, ExpressionKind::Field { object, key });
        }
        if let Some((callee, receiver, arguments, is_constructor)) = call_parts(node) {
            if arguments.len() > 32
                || arguments
                    .iter()
                    .any(|argument| !is_positional_argument(*argument))
            {
                return self.push(node, ExpressionKind::Unknown {
                    reason: "named/spread/variadic argument mapping or argument budget is unresolved".into(),
                    inputs: Vec::new(),
                });
            }
            let callee = if let Some(receiver) = receiver {
                let object = self.expression(receiver, depth + 1);
                let key = self.push(
                    callee,
                    ExpressionKind::Literal(Constant::String(
                        text(callee, self.source).to_string(),
                    )),
                );
                self.push(node, ExpressionKind::Field { object, key })
            } else {
                self.expression(callee, depth + 1)
            };
            let arguments = arguments
                .into_iter()
                .take(32)
                .map(|arg| self.expression(arg, depth + 1))
                .collect();
            return self.push(
                node,
                if is_constructor {
                    ExpressionKind::Construct { callee, arguments }
                } else {
                    ExpressionKind::Call { callee, arguments }
                },
            );
        }
        if is_object(node) {
            if self.unit.language == "go"
                && node
                    .child_by_field_name("type")
                    .is_some_and(|ty| ty.kind() == "map_type")
            {
                return self.push(
                    node,
                    ExpressionKind::Unknown {
                        reason: "Go map key/value semantics are outside the bounded object summary"
                            .into(),
                        inputs: Vec::new(),
                    },
                );
            }
            let mut fields = Vec::new();
            let members = node
                .child_by_field_name("body")
                .or_else(|| child(node, &["literal_value", "field_initializer_list"]))
                .unwrap_or(node);
            if members.named_child_count() > 64 {
                return self.push(
                    node,
                    ExpressionKind::Unknown {
                        reason: "object member budget exceeded".into(),
                        inputs: Vec::new(),
                    },
                );
            }
            for member in children(members).into_iter().take(64) {
                if is_comment(member) {
                    continue;
                }
                if self.unit.language == "lua"
                    && text(member, self.source).trim_start().starts_with('[')
                    && member
                        .child_by_field_name("key")
                        .or_else(|| member.child_by_field_name("name"))
                        .is_none_or(|key| {
                            !matches!(literal(key, self.source), Some(Constant::String(_)))
                        })
                {
                    return self.push(
                        node,
                        ExpressionKind::Unknown {
                            reason: "computed Lua table key is not a static field name".into(),
                            inputs: Vec::new(),
                        },
                    );
                }
                if self.unit.language == "python"
                    && member.child_by_field_name("key").is_some_and(|key| {
                        !matches!(literal(key, self.source), Some(Constant::String(_)))
                    })
                {
                    return self.push(
                        node,
                        ExpressionKind::Unknown {
                            reason: "dynamic/non-string dictionary key is unresolved".into(),
                            inputs: Vec::new(),
                        },
                    );
                }
                if let Some(&function) = self.function_ids.get(&member.id()) {
                    if has_opaque_modifiers(member, self.source) {
                        return self.push(
                            node,
                            ExpressionKind::Unknown {
                                reason: "object accessor semantics are unresolved".into(),
                                inputs: Vec::new(),
                            },
                        );
                    }
                    fields.push((
                        self.unit.functions[function].name.clone(),
                        self.push(member, ExpressionKind::Function(function)),
                    ));
                } else if let Some((name, value)) = object_field(member, self.source) {
                    let value = self.expression(value, depth + 1);
                    fields.push((name, value));
                } else {
                    return self.push(
                        node,
                        ExpressionKind::Unknown {
                            reason: "dynamic/spread object member can override known fields".into(),
                            inputs: Vec::new(),
                        },
                    );
                }
            }
            let class = node
                .child_by_field_name("type")
                .map(|node| text(node, self.source).to_string());
            return self.push(node, ExpressionKind::Object { class, fields });
        }
        if let Some(inner) = transparent(node) {
            return self.expression(inner, depth + 1);
        }
        let reason = format!(
            "{} expression is not a value-preserving summary",
            node.kind()
        );
        let inputs = children(node)
            .into_iter()
            .filter(|child| is_call(*child))
            .take(8)
            .map(|child| self.expression(child, depth + 1))
            .collect();
        self.push(node, ExpressionKind::Unknown { reason, inputs })
    }
    fn branch_statements(&mut self, node: Node<'a>, function: usize) -> Vec<Statement> {
        if is_statement_container(node) {
            let mut result = Vec::new();
            for child in children(node) {
                self.statement(child, function, &mut result);
            }
            result
        } else {
            let mut result = Vec::new();
            self.statement(node, function, &mut result);
            result
        }
    }
    fn branch_expression(&mut self, node: Node<'a>, depth: usize) -> NodeId {
        if is_statement_container(node) {
            let mut pending = children(node);
            while let Some(child) = pending.pop() {
                if is_return(child) {
                    self.issue(
                        child,
                        "nonlocal return inside a value expression is not summarized",
                    );
                    self.unit.functions[self.active_function].is_available = false;
                    return 0;
                }
                if !self.function_ids.contains_key(&child.id()) {
                    pending.extend(children(child));
                }
            }
            let statements = self.statements(node, self.active_function);
            self.push(node, ExpressionKind::Block(statements))
        } else {
            self.expression(node, depth + 1)
        }
    }
    fn statements(&mut self, node: Node<'a>, function: usize) -> Vec<Statement> {
        let is_expression_body = function != 0 && !is_statement_container(node);
        if is_expression_body {
            let value = self.expression(node, 0);
            return vec![Statement {
                range: range(node),
                kind: StatementKind::Return(value),
            }];
        }
        let mut result = Vec::new();
        let body = children(node)
            .into_iter()
            .filter(|child| !is_comment(*child))
            .collect::<Vec<_>>();
        for (position, statement) in body.iter().copied().enumerate() {
            if result.len() >= NODES_PER_FILE {
                self.issue(statement, "statement budget exceeded");
                break;
            }
            let tail = if statement.kind() == "expression_statement" {
                statement.named_child(0).unwrap_or(statement)
            } else {
                statement
            };
            if self.unit.language == "rust"
                && function != 0
                && position + 1 == body.len()
                && tail.kind() == "if_expression"
                && !text(statement, self.source).trim_end().ends_with(';')
            {
                let value = self.expression(tail, 0);
                result.push(Statement {
                    range: range(statement),
                    kind: StatementKind::Return(value),
                });
                continue;
            }
            self.statement(statement, function, &mut result);
        }
        if function != 0 && matches!(self.unit.language.as_str(), "rust" | "ruby" | "scala") {
            if let Some(last) = result.last_mut() {
                if let StatementKind::Evaluate(value) = last.kind {
                    if !text(node, self.source)
                        .trim_end_matches('}')
                        .trim_end()
                        .ends_with(';')
                    {
                        last.kind = StatementKind::Return(value);
                    }
                }
            }
        }
        result
    }
    fn statement(&mut self, node: Node<'a>, function: usize, result: &mut Vec<Statement>) {
        self.statements_visited += 1;
        if self.statements_visited > NODES_PER_FILE {
            self.issue(node, "statement budget exceeded");
            self.unit.functions[function].is_available = false;
            return;
        }
        if self.function_ids.contains_key(&node.id())
            || self.class_ids.contains_key(&node.id())
            || is_comment(node)
            || is_import(node)
        {
            return;
        }
        if let Some(ids) = self.declarations.get(&node.id()).cloned() {
            let initializer = binding_parts(node).and_then(|(_, value)| value);
            let value = initializer
                .map(|node| self.expression(node, 0))
                .unwrap_or_else(|| {
                    self.push(
                        node,
                        ExpressionKind::Unknown {
                            reason: "binding has no summarized initializer".into(),
                            inputs: Vec::new(),
                        },
                    )
                });
            let pattern = binding_parts(node).map(|(pattern, _)| pattern);
            let projection = pattern
                .and_then(|pattern| tuple_bindings(pattern, self.source))
                .map(|mut items| {
                    if matches!(self.unit.language.as_str(), "go" | "rust") {
                        items.retain(|(name, _)| name != "_");
                    }
                    items
                });
            if let Some(projection) =
                projection.filter(|items| items.iter().any(|(_, path)| !path.is_empty()))
            {
                if projection.len() == ids.len() {
                    result.push(Statement {
                        range: range(node),
                        kind: StatementKind::Destructure {
                            bindings: ids
                                .into_iter()
                                .zip(projection)
                                .map(|(id, (_, path))| (id, path))
                                .collect(),
                            value,
                        },
                    });
                    return;
                }
            }
            let value = if ids.len() > 1
                || pattern.is_some_and(|node| {
                    matches!(
                        node.kind(),
                        "tuple_pattern" | "pattern_list" | "tuple" | "expression_list"
                    )
                }) {
                self.issue(node, "destructuring pattern is outside the tuple summary");
                self.push(
                    node,
                    ExpressionKind::Unknown {
                        reason: "destructuring pattern is unresolved".into(),
                        inputs: Vec::new(),
                    },
                )
            } else {
                value
            };
            for id in ids {
                self.unit.bindings[id].initializer = Some(value);
                result.push(Statement {
                    range: range(node),
                    kind: StatementKind::Bind { binding: id, value },
                });
            }
            return;
        }
        if is_return(node) {
            let values = return_values(node);
            let value = if values.len() == 1 {
                self.expression(values[0], 0)
            } else if values.len() <= 32 {
                let items = values
                    .into_iter()
                    .map(|value| self.expression(value, 0))
                    .collect();
                self.push(node, ExpressionKind::Tuple(items))
            } else {
                self.issue(node, "tuple return budget exceeded");
                0
            };
            result.push(Statement {
                range: range(node),
                kind: StatementKind::Return(value),
            });
            return;
        }
        if let Some((target, value)) = assignment(node) {
            if self
                .source
                .get(target.end_byte()..value.start_byte())
                .is_some_and(|operator| {
                    let operator = std::str::from_utf8(operator).unwrap_or("").trim();
                    !matches!(operator, "=" | ":=")
                })
            {
                let reason = "compound assignment is not value-preserving".to_string();
                result.push(Statement {
                    range: range(node),
                    kind: StatementKind::Barrier {
                        reason,
                        expressions: Vec::new(),
                    },
                });
                return;
            }
            let target = self.expression(target, 0);
            let value = self.expression(value, 0);
            result.push(Statement {
                range: range(node),
                kind: StatementKind::Assign { target, value },
            });
            return;
        }
        if let Some((condition, consequence, alternative)) = conditional_parts(node) {
            let condition = self.expression(condition, 0);
            let consequence = self.branch_statements(consequence, function);
            let alternative = alternative
                .map(|node| self.branch_statements(node, function))
                .unwrap_or_default();
            result.push(Statement {
                range: range(node),
                kind: StatementKind::Branch {
                    condition,
                    consequence,
                    alternative,
                },
            });
            return;
        }
        if is_control(node) {
            let reason = format!(
                "{} control flow is outside the bounded summary",
                node.kind()
            );
            self.issue(node, &reason);
            result.push(Statement {
                range: range(node),
                kind: StatementKind::Barrier {
                    reason,
                    expressions: Vec::new(),
                },
            });
            return;
        }
        if is_call(node) || is_expression(node) {
            let value = self.expression(node, 0);
            result.push(Statement {
                range: range(node),
                kind: StatementKind::Evaluate(value),
            });
            return;
        }
        if is_statement_container(node)
            || matches!(
                node.kind(),
                "lexical_declaration"
                    | "variable_declaration"
                    | "var_declaration"
                    | "const_declaration"
                    | "expression_statement"
                    | "export_statement"
                    | "declaration"
                    | "short_var_declaration"
                    | "local_variable_declaration"
                    | "assignment_statement"
            )
        {
            for child in children(node) {
                self.statement(child, function, result);
            }
        } else if !matches!(
            node.kind(),
            "empty_statement"
                | "package_clause"
                | "package_declaration"
                | "attribute_item"
                | "annotation"
                | "php_tag"
        ) {
            let reason = format!("{} statement is outside the bounded summary", node.kind());
            self.issue(node, &reason);
            result.push(Statement {
                range: range(node),
                kind: StatementKind::Barrier {
                    reason,
                    expressions: Vec::new(),
                },
            });
        }
    }
    fn exports(&mut self, root: Node<'a>) {
        for node in children(root) {
            if matches!(
                node.kind(),
                "package_clause" | "package_declaration" | "package_header"
            ) {
                if let Some(name) = name(node, self.source).or_else(|| {
                    children(node)
                        .first()
                        .map(|node| text(*node, self.source).to_string())
                }) {
                    self.unit.namespace = name;
                }
            }
            if node.kind() != "export_statement" {
                continue;
            }
            let source = node
                .child_by_field_name("source")
                .and_then(|node| literal(node, self.source))
                .and_then(|value| {
                    if let Constant::String(value) = value {
                        Some(value)
                    } else {
                        None
                    }
                });
            if let Some(declaration) = node.child_by_field_name("declaration") {
                if let Some(name) = name(declaration, self.source) {
                    self.unit.exports.insert(name.clone(), Export::Local(name));
                }
                for child in children(declaration) {
                    if let Some((pattern, _)) = binding_parts(child) {
                        for name in pattern_names(pattern, self.source) {
                            self.unit.exports.insert(name.clone(), Export::Local(name));
                        }
                    }
                }
            }
            if let Some(clause) = child(node, &["export_clause"]) {
                for specifier in children(clause) {
                    let Some(original) = specifier.child_by_field_name("name") else {
                        continue;
                    };
                    let alias = specifier.child_by_field_name("alias").unwrap_or(original);
                    let name = text(original, self.source).to_string();
                    let value = source.as_ref().map_or_else(
                        || Export::Local(name.clone()),
                        |source| Export::Foreign {
                            source: source.clone(),
                            name: name.clone(),
                        },
                    );
                    self.unit
                        .exports
                        .insert(text(alias, self.source).to_string(), value);
                }
            }
        }
    }
}
