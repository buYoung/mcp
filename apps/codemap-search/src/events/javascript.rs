//! Bounded ESM binding evaluation over the immutable indexed source generation.
use super::EventLocation;
use crate::parser::CodeRange;
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};
use tree_sitter::{Node, Tree};

#[derive(Clone, Debug)]
pub(super) struct ApiIdentity {
    pub module: String,
    pub symbol: String,
    pub location: Option<EventLocation>,
    pub is_callable: bool,
}

#[derive(Clone, Debug)]
pub(super) enum ValueKind {
    String(String),
    Definition(ApiIdentity),
    Bus(ApiIdentity, EventLocation),
    Opaque(Option<ApiIdentity>, String),
    Namespace(String),
}

#[derive(Clone, Debug)]
pub(super) struct Value {
    pub kind: ValueKind,
    pub evidence: Vec<EventLocation>,
}
impl Value {
    pub fn unknown(reason: &str) -> Self {
        Self {
            kind: ValueKind::Opaque(None, reason.into()),
            evidence: Vec::new(),
        }
    }
    pub fn api(&self) -> Option<&ApiIdentity> {
        match &self.kind {
            ValueKind::Definition(api)
            | ValueKind::Bus(api, _)
            | ValueKind::Opaque(Some(api), _) => Some(api),
            _ => None,
        }
    }
}

#[derive(Clone)]
enum BindingKind {
    Expression(Range<usize>),
    Definition(bool),
    Import(String, String, bool),
    Opaque,
}
struct Binding {
    name: String,
    range: Range<usize>,
    scope: Range<usize>,
    kind: BindingKind,
    annotation: Option<String>,
    is_immutable: bool,
    is_exported: bool,
    is_mutated: bool,
}
enum Export {
    Local(String),
    Foreign(String, String),
    Expression(Range<usize>),
    Ambiguous,
}

fn add_export(exports: &mut HashMap<String, Export>, name: String, value: Export) {
    use std::collections::hash_map::Entry;
    match exports.entry(name) {
        Entry::Vacant(entry) => {
            entry.insert(value);
        }
        Entry::Occupied(mut entry) => {
            entry.insert(Export::Ambiguous);
        }
    }
}

pub(super) struct JsFile<'a> {
    pub path: &'a str,
    pub source: &'a str,
    pub tree: Tree,
    bindings: Vec<Binding>,
    exports: HashMap<String, Export>,
    star_exports: Vec<String>,
}

pub(super) fn range(node: Node<'_>) -> CodeRange {
    CodeRange {
        start_line: node.start_position().row + 1,
        start_col: node.start_position().column + 1,
        end_line: node.end_position().row + 1,
        end_col: node.end_position().column + 1,
    }
}
pub(super) fn text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}
pub(super) fn location(path: &str, node: Node<'_>, name: Option<String>) -> EventLocation {
    EventLocation {
        file_path: path.into(),
        range: range(node),
        name,
    }
}
pub(super) fn static_string(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(
        node.kind(),
        "string" | "string_literal" | "raw_string_literal" | "template_string"
    ) {
        return None;
    }
    let value = text(node, source);
    if value.starts_with('r') {
        let quote = value.find('"')?;
        let end = value.rfind('"')?;
        return (end > quote).then(|| value[quote + 1..end].to_string());
    }
    let first = value.chars().next()?;
    if !['\'', '"', '`'].contains(&first) || !value.ends_with(first) || value.len() < 2 {
        return None;
    }
    let inner = &value[first.len_utf8()..value.len() - first.len_utf8()];
    if inner.contains('\\') || first == '`' && inner.contains("${") {
        return None;
    }
    Some(inner.to_string())
}
fn callable(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "generator_function_declaration"
            | "function_expression"
            | "generator_function"
            | "arrow_function"
            | "method_definition"
    )
}
fn scope(mut node: Node<'_>) -> Range<usize> {
    let is_function_scoped = node.kind() == "variable_declarator"
        && node
            .parent()
            .is_some_and(|parent| parent.kind() == "variable_declaration");
    while let Some(parent) = node.parent() {
        node = parent;
        if (!is_function_scoped
            && matches!(
                node.kind(),
                "program"
                    | "statement_block"
                    | "for_statement"
                    | "for_in_statement"
                    | "catch_clause"
            ))
            || node.kind() == "program"
            || callable(node.kind())
        {
            break;
        }
    }
    node.byte_range()
}
fn is_module_allocation(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if callable(parent.kind())
            || matches!(
                parent.kind(),
                "for_statement"
                    | "for_in_statement"
                    | "while_statement"
                    | "do_statement"
                    | "class_body"
            )
        {
            return false;
        }
        node = parent;
    }
    true
}
fn is_exported(node: Node<'_>) -> bool {
    node.parent().is_some_and(|p| {
        p.kind() == "export_statement" || p.parent().is_some_and(|p| p.kind() == "export_statement")
    })
}

fn pattern_names(node: Node<'_>, source: &str, names: &mut Vec<String>) {
    match node.kind() {
        "identifier" | "shorthand_property_identifier_pattern" => {
            names.push(text(node, source).into())
        }
        "pair_pattern" => {
            if let Some(value) = node.child_by_field_name("value") {
                pattern_names(value, source, names);
            }
        }
        "assignment_pattern" | "object_assignment_pattern" => {
            if let Some(left) = node.child_by_field_name("left") {
                pattern_names(left, source, names);
            }
        }
        "object_pattern" | "array_pattern" | "rest_pattern" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                pattern_names(child, source, names);
            }
        }
        _ => {}
    }
}

fn add_opaque_patterns(
    bindings: &mut Vec<Binding>,
    node: Node<'_>,
    pattern: Node<'_>,
    source: &str,
) {
    let mut names = Vec::new();
    pattern_names(pattern, source, &mut names);
    for name in names {
        bindings.push(Binding {
            name,
            range: node.byte_range(),
            scope: scope(node),
            kind: BindingKind::Opaque,
            annotation: None,
            is_immutable: false,
            is_exported: false,
            is_mutated: false,
        });
    }
}

impl<'a> JsFile<'a> {
    pub fn parse(path: &'a str, source: &'a str) -> Option<Self> {
        let ext = Path::new(path).extension()?.to_str()?;
        if !matches!(
            ext,
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts"
        ) {
            return None;
        }
        let spec = crate::lang::spec_for_path(Path::new(path))?;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&spec.grammar(ext)).ok()?;
        let tree = crate::parser::parse_source(&mut parser, source.as_bytes()).ok()?;
        let mut bindings = Vec::new();
        let mut exports = HashMap::new();
        let mut star_exports = Vec::new();
        let mut writes = Vec::new();
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            if node.is_error() || node.is_missing() {
                continue;
            }
            let mut cursor = node.walk();
            let children: Vec<_> = node.named_children(&mut cursor).collect();
            stack.extend(children.iter().rev().copied());
            // A truncated symbol table must never fall through an omitted shadow to an outer bus.
            if bindings.len() >= 4096 {
                return None;
            }
            match node.kind() {
                "variable_declarator" => {
                    if let Some(name) = node.child_by_field_name("name") {
                        if name.kind() != "identifier" {
                            add_opaque_patterns(&mut bindings, node, name, source);
                            continue;
                        }
                        let kind = node
                            .child_by_field_name("value")
                            .map_or(BindingKind::Opaque, |v| {
                                BindingKind::Expression(v.byte_range())
                            });
                        let is_immutable = node.parent().is_some_and(|p| {
                            text(p, source).split_whitespace().next() == Some("const")
                        });
                        bindings.push(Binding {
                            name: text(name, source).into(),
                            range: node.byte_range(),
                            scope: scope(node),
                            kind,
                            annotation: node.child_by_field_name("type").map(|n| {
                                text(n, source).trim_start_matches(':').trim().to_string()
                            }),
                            is_immutable,
                            is_exported: is_exported(node),
                            is_mutated: false,
                        });
                    }
                }
                "function_declaration"
                | "generator_function_declaration"
                | "class_declaration"
                | "enum_declaration" => {
                    if let Some(name) = node.child_by_field_name("name") {
                        bindings.push(Binding {
                            name: text(name, source).into(),
                            range: node.byte_range(),
                            scope: scope(node),
                            kind: BindingKind::Definition(callable(node.kind())),
                            annotation: None,
                            is_immutable: true,
                            is_exported: is_exported(node),
                            is_mutated: false,
                        });
                    }
                }
                "required_parameter" | "optional_parameter" => {
                    if let Some(name) = node.child_by_field_name("pattern") {
                        if name.kind() != "identifier" {
                            add_opaque_patterns(&mut bindings, node, name, source);
                            continue;
                        }
                        bindings.push(Binding {
                            name: text(name, source).into(),
                            range: node.byte_range(),
                            scope: scope(node),
                            kind: BindingKind::Opaque,
                            annotation: node.child_by_field_name("type").map(|n| {
                                text(n, source).trim_start_matches(':').trim().to_string()
                            }),
                            is_immutable: false,
                            is_exported: false,
                            is_mutated: false,
                        });
                    }
                }
                "identifier"
                    if node.parent().is_some_and(|p| {
                        p.kind() == "formal_parameters"
                            || p.kind() == "arrow_function"
                                && p.child_by_field_name("parameter") == Some(node)
                    }) =>
                {
                    bindings.push(Binding {
                        name: text(node, source).into(),
                        range: node.byte_range(),
                        scope: scope(node),
                        kind: BindingKind::Opaque,
                        annotation: None,
                        is_immutable: false,
                        is_exported: false,
                        is_mutated: false,
                    });
                }
                "object_pattern" | "array_pattern" | "rest_pattern" | "assignment_pattern"
                    if node
                        .parent()
                        .is_some_and(|p| p.kind() == "formal_parameters") =>
                {
                    add_opaque_patterns(&mut bindings, node, node, source)
                }
                "catch_clause" => {
                    if let Some(parameter) = node.child_by_field_name("parameter") {
                        add_opaque_patterns(&mut bindings, parameter, parameter, source);
                    }
                }
                "import_statement" => {
                    let Some(module) = node
                        .child_by_field_name("source")
                        .and_then(|n| static_string(n, source))
                    else {
                        continue;
                    };
                    let is_type_only = text(node, source).trim_start().starts_with("import type ");
                    let mut parts = children.clone();
                    while let Some(part) = parts.pop() {
                        let binding = match part.kind() {
                            "import_specifier" => part.child_by_field_name("name").map(|name| {
                                let alias = part.child_by_field_name("alias").unwrap_or(name);
                                (
                                    text(alias, source).to_string(),
                                    text(name, source).to_string(),
                                    is_type_only
                                        || text(part, source).trim_start().starts_with("type "),
                                )
                            }),
                            "namespace_import" => part.named_child(0).map(|name| {
                                (text(name, source).to_string(), "*".into(), is_type_only)
                            }),
                            "identifier"
                                if part.parent().is_some_and(|p| p.kind() == "import_clause") =>
                            {
                                Some((text(part, source).into(), "default".into(), is_type_only))
                            }
                            _ => None,
                        };
                        if let Some((name, imported, only_type)) = binding {
                            bindings.push(Binding {
                                name,
                                range: part.byte_range(),
                                scope: tree.root_node().byte_range(),
                                kind: BindingKind::Import(module.clone(), imported, only_type),
                                annotation: None,
                                is_immutable: true,
                                is_exported: false,
                                is_mutated: false,
                            });
                        } else {
                            let mut c = part.walk();
                            parts.extend(part.named_children(&mut c));
                        }
                    }
                }
                "export_statement" => {
                    let module = node
                        .child_by_field_name("source")
                        .and_then(|n| static_string(n, source));
                    if let Some(clause) = children.iter().find(|n| n.kind() == "export_clause") {
                        let mut c = clause.walk();
                        for spec in clause.named_children(&mut c) {
                            if let Some(name) = spec.child_by_field_name("name") {
                                let alias = spec.child_by_field_name("alias").unwrap_or(name);
                                let value = match &module {
                                    Some(module) => {
                                        Export::Foreign(module.clone(), text(name, source).into())
                                    }
                                    None => Export::Local(text(name, source).into()),
                                };
                                add_export(&mut exports, text(alias, source).into(), value);
                            }
                        }
                    } else if let Some(module) = module {
                        star_exports.push(module);
                    } else if text(node, source)
                        .trim_start()
                        .starts_with("export default")
                    {
                        if let Some(value) = node
                            .child_by_field_name("value")
                            .or_else(|| node.child_by_field_name("declaration"))
                        {
                            add_export(
                                &mut exports,
                                "default".into(),
                                Export::Expression(value.byte_range()),
                            );
                        }
                    }
                }
                "assignment_expression"
                | "augmented_assignment_expression"
                | "update_expression" => {
                    if let Some(left) = node
                        .child_by_field_name("left")
                        .or_else(|| node.child_by_field_name("argument"))
                    {
                        let base = if left.kind() == "member_expression" {
                            left.child_by_field_name("object").unwrap_or(left)
                        } else {
                            left
                        };
                        if base.kind() == "identifier" {
                            writes.push((text(base, source).to_string(), node.start_byte()));
                        }
                    }
                }
                _ => {}
            }
        }
        for (name, at) in writes {
            if let Some(index) = bindings
                .iter()
                .enumerate()
                .filter(|(_, b)| b.name == name && b.scope.contains(&at))
                .min_by_key(|(_, b)| b.scope.len())
                .map(|(i, _)| i)
            {
                bindings[index].is_mutated = true;
            }
        }
        Some(Self {
            path,
            source,
            tree,
            bindings,
            exports,
            star_exports,
        })
    }
    pub fn node(&self, span: &Range<usize>) -> Option<Node<'_>> {
        let mut node = self
            .tree
            .root_node()
            .descendant_for_byte_range(span.start, span.end)?;
        while node.byte_range() != *span {
            node = node.parent()?;
        }
        Some(node)
    }
    fn binding(&self, name: &str, at: usize) -> Option<&Binding> {
        let length = self
            .bindings
            .iter()
            .filter(|b| b.name == name && b.scope.contains(&at))
            .map(|b| b.scope.len())
            .min()?;
        let mut candidates = self
            .bindings
            .iter()
            .filter(|b| b.name == name && b.scope.contains(&at) && b.scope.len() == length);
        let first = candidates.next()?;
        candidates.next().is_none().then_some(first)
    }
}

pub(super) struct JsGraph<'a> {
    pub files: HashMap<String, JsFile<'a>>,
}
impl<'a> JsGraph<'a> {
    pub fn new(sources: &'a HashMap<String, String>) -> Self {
        Self {
            files: sources
                .iter()
                .filter_map(|(path, source)| Some((path.clone(), JsFile::parse(path, source)?)))
                .collect(),
        }
    }
    fn module(&self, path: &str, module: &str) -> String {
        if !module.starts_with('.') {
            return format!("external:{module}");
        }
        let mut normalized = PathBuf::new();
        for part in Path::new(path)
            .parent()
            .unwrap_or(Path::new(""))
            .join(module)
            .components()
        {
            match part {
                Component::ParentDir => {
                    if !normalized.pop() {
                        return "unavailable:outside-workspace".into();
                    }
                }
                Component::Normal(p) => normalized.push(p),
                Component::CurDir => {}
                _ => return "unavailable:absolute-module".into(),
            }
        }
        let mut paths = vec![normalized.clone()];
        let extension = normalized
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if matches!(extension, "js" | "mjs" | "cjs") {
            for ext in ["ts", "tsx", "mts", "cts"] {
                paths.push(normalized.with_extension(ext));
            }
        }
        if extension.is_empty() {
            for ext in ["ts", "tsx", "js", "jsx", "mts", "mjs", "cts", "cjs"] {
                paths.push(normalized.with_extension(ext));
                paths.push(normalized.join(format!("index.{ext}")));
            }
        }
        let matches: Vec<_> = paths
            .into_iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .filter(|p| self.files.contains_key(p))
            .collect();
        if matches.len() == 1 {
            matches[0].clone()
        } else {
            "unavailable:module missing or ambiguous".into()
        }
    }
    fn exported(&self, module: &str, name: &str, depth: usize) -> Value {
        if depth > 32 {
            return Value::unknown("binding/import resolution depth exceeded");
        }
        if let Some(module) = module.strip_prefix("external:") {
            return Value {
                kind: ValueKind::Definition(ApiIdentity {
                    module: module.into(),
                    symbol: name.into(),
                    location: None,
                    is_callable: false,
                }),
                evidence: Vec::new(),
            };
        }
        let Some(file) = self.files.get(module) else {
            return Value::unknown("module source unavailable");
        };
        if let Some(export) = file.exports.get(name) {
            let mut value = match export {
                Export::Ambiguous => Value::unknown("conflicting exports"),
                Export::Local(local) => self.name(
                    module,
                    local,
                    file.source.len().saturating_sub(1),
                    depth + 1,
                ),
                Export::Foreign(source, foreign) => {
                    self.exported(&self.module(module, source), foreign, depth + 1)
                }
                Export::Expression(span) => file.node(span).map_or_else(
                    || Value::unknown("export source unavailable"),
                    |node| self.eval(module, node, depth + 1),
                ),
            };
            value.evidence.push(EventLocation {
                file_path: module.into(),
                range: CodeRange {
                    start_line: 1,
                    start_col: 1,
                    end_line: 1,
                    end_col: 1,
                },
                name: Some("export resolution".into()),
            });
            return value;
        }
        if let Some(binding) = file
            .binding(name, file.source.len().saturating_sub(1))
            .filter(|b| b.is_exported)
        {
            return self.binding_value(file, binding, depth + 1);
        }
        if file.star_exports.len() == 1 {
            let mut value =
                self.exported(&self.module(module, &file.star_exports[0]), name, depth + 1);
            value.evidence.push(EventLocation {
                file_path: module.into(),
                range: CodeRange {
                    start_line: 1,
                    start_col: 1,
                    end_line: 1,
                    end_col: 1,
                },
                name: Some("reexport resolution".into()),
            });
            return value;
        }
        Value::unknown("export missing, ambiguous or unsupported")
    }
    pub fn name(&self, path: &str, name: &str, at: usize, depth: usize) -> Value {
        if depth > 32 {
            return Value::unknown("binding/import resolution depth exceeded");
        }
        let Some(file) = self.files.get(path) else {
            return Value::unknown("source unavailable");
        };
        let Some(binding) = file.binding(name, at) else {
            return Value::unknown("receiver/key/handler binding is not proven");
        };
        self.binding_value(file, binding, depth + 1)
    }
    fn binding_value(&self, file: &JsFile<'_>, binding: &Binding, depth: usize) -> Value {
        let Some(node) = file.node(&binding.range) else {
            return Value::unknown("binding source unavailable");
        };
        if node.has_error() {
            return Value::unknown("binding contains a parse error");
        }
        let mut value = match &binding.kind {
            BindingKind::Definition(is_callable) => Value {
                kind: ValueKind::Definition(ApiIdentity {
                    module: file.path.into(),
                    symbol: binding.name.clone(),
                    location: Some(location(file.path, node, Some(binding.name.clone()))),
                    is_callable: *is_callable,
                }),
                evidence: Vec::new(),
            },
            BindingKind::Expression(span) => file.node(span).map_or_else(
                || Value::unknown("initializer unavailable"),
                |node| self.eval(file.path, node, depth + 1),
            ),
            BindingKind::Import(module, name, is_type_only) => {
                if *is_type_only {
                    Value::unknown("type-only import has no runtime binding")
                } else {
                    let module = self.module(file.path, module);
                    if name == "*" {
                        Value {
                            kind: ValueKind::Namespace(module),
                            evidence: Vec::new(),
                        }
                    } else {
                        self.exported(&module, name, depth + 1)
                    }
                }
            }
            BindingKind::Opaque => {
                Value::unknown("parameter or opaque binding has no proven instance")
            }
        };
        if (!binding.is_immutable || binding.is_mutated)
            || matches!(value.kind, ValueKind::Opaque(None, _))
        {
            let api = value.api().cloned().or_else(|| {
                binding.annotation.as_ref().and_then(|name| {
                    self.name(file.path, name, binding.range.start, depth + 1)
                        .api()
                        .cloned()
                })
            });
            let reason = if binding.is_mutated {
                "binding or its members are reassigned"
            } else {
                "mutable, parameter or factory instance is unresolved"
            };
            value.kind = ValueKind::Opaque(api, reason.into());
        }
        value
            .evidence
            .push(location(file.path, node, Some(binding.name.clone())));
        value
    }
    pub fn eval(&self, path: &str, node: Node<'_>, depth: usize) -> Value {
        if depth > 32 {
            return Value::unknown("binding/import resolution depth exceeded");
        }
        let Some(file) = self.files.get(path) else {
            return Value::unknown("source unavailable");
        };
        if node.has_error() || node.is_missing() {
            return Value::unknown("expression contains a parse error");
        }
        if let Some(value) = static_string(node, file.source) {
            return Value {
                kind: ValueKind::String(value),
                evidence: vec![location(path, node, None)],
            };
        }
        match node.kind() {
            "identifier" => self.name(path, text(node, file.source), node.start_byte(), depth + 1),
            "parenthesized_expression"
            | "as_expression"
            | "satisfies_expression"
            | "non_null_expression" => node.named_child(0).map_or_else(
                || Value::unknown("wrapped expression unavailable"),
                |n| self.eval(path, n, depth + 1),
            ),
            "member_expression" => {
                let Some(object) = node.child_by_field_name("object") else {
                    return Value::unknown("member receiver unavailable");
                };
                let Some(property) = node.child_by_field_name("property") else {
                    return Value::unknown("dynamic member name");
                };
                let value = self.eval(path, object, depth + 1);
                match value.kind {
                    ValueKind::Namespace(module) => {
                        let mut out =
                            self.exported(&module, text(property, file.source), depth + 1);
                        out.evidence.extend(value.evidence);
                        out
                    }
                    ValueKind::Definition(api) => {
                        let Some(target) = self.files.get(&api.module) else {
                            return Value::unknown("enum source unavailable");
                        };
                        let Some(binding) = target.bindings.iter().find(|b| b.name == api.symbol)
                        else {
                            return Value::unknown("enum binding unavailable");
                        };
                        let Some(enum_node) = target
                            .node(&binding.range)
                            .filter(|n| n.kind() == "enum_declaration")
                        else {
                            return Value::unknown("only string enum members are static values");
                        };
                        let Some(body) = enum_node.child_by_field_name("body") else {
                            return Value::unknown("enum body unavailable");
                        };
                        let mut cursor = body.walk();
                        for member in body.named_children(&mut cursor) {
                            if member.child_by_field_name("name").is_some_and(|n| {
                                text(n, target.source) == text(property, file.source)
                            }) {
                                if let Some(initializer) = member.child_by_field_name("value") {
                                    let mut out = self.eval(&api.module, initializer, depth + 1);
                                    out.evidence.extend(value.evidence);
                                    out.evidence.push(location(&api.module, member, None));
                                    return out;
                                }
                            }
                        }
                        Value::unknown("enum member needs an explicit static string initializer")
                    }
                    _ => Value::unknown("instance member values are not inferred"),
                }
            }
            "new_expression" => {
                let Some(constructor) = node.child_by_field_name("constructor") else {
                    return Value::unknown("constructor unavailable");
                };
                let mut value = self.eval(path, constructor, depth + 1);
                let Some(api) = value.api().cloned() else {
                    return Value::unknown("constructor identity unavailable");
                };
                value.kind = if is_module_allocation(node) {
                    ValueKind::Bus(api, location(path, node, None))
                } else {
                    ValueKind::Opaque(
                        Some(api),
                        "allocation inside a callable can create multiple instances".into(),
                    )
                };
                value
            }
            "arrow_function"
            | "function_expression"
            | "generator_function"
            | "function_declaration" => Value {
                kind: ValueKind::Definition(ApiIdentity {
                    module: path.into(),
                    symbol: "<inline handler>".into(),
                    location: Some(location(path, node, Some("<inline handler>".into()))),
                    is_callable: true,
                }),
                evidence: Vec::new(),
            },
            _ => Value::unknown("dynamic or unsupported expression"),
        }
    }
}
