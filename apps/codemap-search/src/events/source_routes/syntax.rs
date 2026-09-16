//! Syntax adapters supply source facts; they do not identify event APIs.
use super::model::{Location, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

pub(crate) type NodeId = usize;

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub kind: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub fields: BTreeMap<String, Vec<NodeId>>,
    pub tokens: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub path: String,
    pub language: String,
    pub data: Arc<String>,
    pub parsed_data: Arc<String>,
    pub line_starts: Vec<usize>,
    pub segments: Vec<super::rust_macros::Segment>,
    pub expansions: Vec<super::rust_macros::Expansion>,
    pub generated_imports: BTreeMap<String, String>,
    pub nodes: Vec<Node>,
    pub module_bindings: BTreeSet<String>,
    pub has_parse_error: bool,
    pub has_node_limit: bool,
    pub has_type_projection: bool,
    pub has_conditional_projection: bool,
}

pub(crate) fn supported(language: &str) -> bool {
    matches!(
        language,
        "javascript"
            | "typescript"
            | "go"
            | "rust"
            | "java"
            | "kotlin"
            | "groovy"
            | "scala"
            | "csharp"
            | "c"
            | "cpp"
            | "swift"
            | "dart"
            | "php"
            | "python"
            | "ruby"
            | "lua"
            | "assembly"
            | "asm"
    )
}

impl Source {
    pub fn parse(path: &str, data: &str) -> Option<Self> {
        let path_obj = Path::new(path);
        let spec = crate::lang::spec_for_path(path_obj)?;
        let language = spec.language_name();
        if !supported(language) {
            return None;
        }
        let mut source = Self {
            path: path.into(),
            language: language.into(),
            data: Arc::new(data.into()),
            parsed_data: Arc::new(data.into()),
            line_starts: std::iter::once(0)
                .chain(
                    data.bytes()
                        .enumerate()
                        .filter(|(_, b)| *b == b'\n')
                        .map(|(i, _)| i + 1),
                )
                .collect(),
            segments: Vec::new(),
            expansions: Vec::new(),
            generated_imports: BTreeMap::new(),
            nodes: Vec::new(),
            module_bindings: BTreeSet::new(),
            has_parse_error: false,
            has_node_limit: false,
            has_type_projection: false,
            has_conditional_projection: false,
        };
        if matches!(language, "asm" | "assembly") {
            source.language = "assembly".into();
            return Some(source);
        }
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(
                &spec.grammar(
                    path_obj
                        .extension()
                        .and_then(|p| p.to_str())
                        .unwrap_or_default(),
                ),
            )
            .ok()?;
        let mut tree = parser.parse(data, None)?;
        if tree.root_node().has_error() && matches!(language, "typescript" | "javascript") {
            for bodies in [false, true] {
                let projected = super::typescript::project_types(data, bodies);
                if projected != data {
                    if let Some(alternative) = parser.parse(&projected, None) {
                        if !alternative.root_node().has_error()
                            && alternative.root_node().named_child_count() > 0
                        {
                            tree = alternative;
                            source.has_type_projection = true;
                            break;
                        }
                    }
                }
            }
        }
        if tree.root_node().has_error() && matches!(language, "swift" | "c" | "cpp") {
            let projected = super::conditional_syntax::project(data, language);
            if projected != data {
                if let Some(alternative) = parser.parse(&projected, None) {
                    if !alternative.root_node().has_error()
                        && alternative.root_node().named_child_count() > 0
                    {
                        tree = alternative;
                        source.has_conditional_projection = true;
                    }
                }
            }
        }
        source.has_parse_error = tree.root_node().has_error();
        source.collect_node(tree.root_node(), None, 0);
        source.module_bindings = source.lexical_bindings(0);
        Some(source)
    }
    pub(super) fn collect_node(
        &mut self,
        node: tree_sitter::Node<'_>,
        parent: Option<NodeId>,
        depth: usize,
    ) -> NodeId {
        let id = self.nodes.len();
        let location = self.offset_location(self.original_offset(node.start_byte()));
        self.nodes.push(Node {
            kind: node.kind().into(),
            start: node.start_byte(),
            end: node.end_byte(),
            line: location.line,
            column: location.column,
            parent,
            children: Vec::new(),
            fields: BTreeMap::new(),
            tokens: BTreeSet::new(),
        });
        for index in 0..node.child_count() {
            let Some(child) = node.child(index as u32) else {
                continue;
            };
            if child.is_named() {
                if self.nodes.len() >= 131_072 || depth >= 256 {
                    self.has_node_limit = true;
                    break;
                }
                let nested = self.collect_node(child, Some(id), depth + 1);
                self.nodes[id].children.push(nested);
                if let Some(name) = node.field_name_for_child(index as u32) {
                    self.nodes[id]
                        .fields
                        .entry(name.into())
                        .or_default()
                        .push(nested);
                }
            } else {
                self.nodes[id].tokens.insert(child.kind().into());
                if child.is_missing() {
                    self.has_parse_error = true;
                }
                if let Some(name) = node.field_name_for_child(index as u32) {
                    if self.nodes.len() < 131_072 && depth < 256 {
                        let nested = self.collect_node(child, Some(id), depth + 1);
                        self.nodes[id]
                            .fields
                            .entry(name.into())
                            .or_default()
                            .push(nested);
                    } else {
                        self.has_node_limit = true;
                    }
                }
            }
        }
        id
    }
    pub fn text(&self, id: Option<NodeId>) -> &str {
        id.and_then(|id| self.nodes.get(id))
            .and_then(|n| self.parsed_data.get(n.start..n.end))
            .unwrap_or_default()
    }
    pub fn child(&self, id: NodeId, names: &[&str]) -> Option<NodeId> {
        let node = &self.nodes[id];
        names
            .iter()
            .find_map(|name| node.fields.get(*name).and_then(|v| v.first()).copied())
    }
    pub fn first(&self, id: NodeId, kinds: &[&str]) -> Option<NodeId> {
        self.nodes[id]
            .children
            .iter()
            .copied()
            .find(|id| kinds.contains(&self.nodes[*id].kind.as_str()))
    }
    pub fn location(&self, id: NodeId) -> Location {
        Location {
            path: self.path.clone(),
            line: self.nodes[id].line,
            column: self.nodes[id].column,
        }
    }
    pub fn walk(&self, id: NodeId, stop_functions: bool) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut pending = vec![id];
        while let Some(next) = pending.pop() {
            result.push(next);
            pending.extend(
                self.nodes[next]
                    .children
                    .iter()
                    .rev()
                    .copied()
                    .filter(|id| !stop_functions || !is_function(&self.nodes[*id].kind)),
            );
        }
        result
    }
    pub fn identifier(&self, id: Option<NodeId>) -> Option<NodeId> {
        let id = id?;
        if is_identifier(&self.nodes[id].kind) {
            return Some(id);
        }
        if let Some(next) = self.child(id, &["declarator", "bound_identifier", "pattern", "name"]) {
            if next != id {
                if let Some(found) = self.identifier(Some(next)) {
                    return Some(found);
                }
            }
        }
        self.walk(id, false)
            .into_iter()
            .find(|id| is_identifier(&self.nodes[*id].kind))
    }
    pub fn rust_pattern_bindings(&self, id: NodeId) -> BTreeSet<String> {
        let node = &self.nodes[id];
        if matches!(
            node.kind.as_str(),
            "identifier" | "shorthand_field_identifier"
        ) {
            return BTreeSet::from([self.text(Some(id)).to_owned()]);
        }
        if matches!(
            node.kind.as_str(),
            "type_identifier" | "scoped_identifier" | "scoped_type_identifier"
        ) {
            return BTreeSet::new();
        }
        if node.kind == "field_pattern" {
            return self
                .child(id, &["pattern", "name"])
                .map(|n| self.rust_pattern_bindings(n))
                .unwrap_or_default();
        }
        let excluded = if matches!(
            node.kind.as_str(),
            "tuple_struct_pattern" | "struct_pattern"
        ) {
            self.child(id, &["type"])
        } else {
            None
        };
        node.children
            .iter()
            .filter(|n| Some(**n) != excluded)
            .flat_map(|n| self.rust_pattern_bindings(*n))
            .collect()
    }
    pub fn lexical_bindings(&self, id: NodeId) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        let mut pending = self.nodes[id].children.clone();
        while let Some(next) = pending.pop() {
            let kind = self.nodes[next].kind.as_str();
            if is_function(kind)
                || is_class(kind)
                || matches!(
                    kind,
                    "struct_item" | "enum_item" | "trait_item" | "mod_item" | "type_item"
                )
            {
                let name = self.text(self.child(next, &["name"]));
                if !name.is_empty() {
                    names.insert(name.into());
                }
                continue;
            }
            if kind.contains("import") || kind == "use_declaration" {
                names.extend(words(self.text(Some(next))));
                continue;
            }
            if is_assignment(kind)
                || is_declaration(kind)
                || matches!(
                    kind,
                    "let_declaration" | "var_spec" | "const_spec" | "short_var_declaration"
                )
            {
                if let Some(target) = self.child(next, &["name", "pattern", "left", "target"]) {
                    if !is_member(&self.nodes[target].kind) {
                        if self.language == "rust" {
                            names.extend(self.rust_pattern_bindings(target));
                        } else {
                            for part in self.walk(target, false) {
                                if is_identifier(&self.nodes[part].kind) {
                                    names.insert(self.text(Some(part)).into());
                                }
                            }
                        }
                    }
                }
            }
            pending.extend(self.nodes[next].children.iter().copied());
        }
        names
    }
}

pub(crate) fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
        .filter(|s| !s.is_empty() && !s.as_bytes()[0].is_ascii_digit())
        .map(str::to_owned)
        .collect()
}
pub(crate) fn is_identifier(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "simple_identifier"
            | "property_identifier"
            | "private_property_identifier"
            | "field_identifier"
            | "type_identifier"
            | "shorthand_property_identifier"
            | "shorthand_property_identifier_pattern"
            | "name"
            | "variable_name"
            | "constant"
            | "shorthand_field_identifier"
    )
}
pub(crate) fn is_class(kind: &str) -> bool {
    matches!(
        kind,
        "class_declaration"
            | "class_definition"
            | "class"
            | "module"
            | "impl_item"
            | "struct_specifier"
            | "class_specifier"
            | "object_definition"
            | "trait_definition"
            | "interface_declaration"
            | "enum_declaration"
            | "package_object"
            | "struct_item"
            | "type_spec"
    )
}
pub(crate) fn is_lambda(kind: &str) -> bool {
    matches!(
        kind,
        "lambda"
            | "lambda_expression"
            | "lambda_literal"
            | "anonymous_function"
            | "anonymous_function_creation_expression"
            | "arrow_function"
            | "function_expression"
            | "lambda_function"
            | "closure"
            | "anonymous_method_expression"
            | "func_literal"
            | "closure_expression"
            | "generator_function"
    )
}
pub(crate) fn is_function(kind: &str) -> bool {
    is_lambda(kind)
        || matches!(
            kind,
            "function_declaration"
                | "generator_function_declaration"
                | "method_definition"
                | "function_item"
                | "method_declaration"
                | "function_definition"
                | "method"
                | "constructor_declaration"
                | "singleton_method"
                | "init_declaration"
        )
}
pub(crate) fn is_call(kind: &str) -> bool {
    matches!(
        kind,
        "call"
            | "call_expression"
            | "function_call"
            | "function_call_expression"
            | "method_invocation"
            | "member_call_expression"
            | "nullsafe_member_call_expression"
            | "invocation_expression"
            | "scoped_call_expression"
    )
}
pub(crate) fn is_member(kind: &str) -> bool {
    matches!(
        kind,
        "member_expression"
            | "null_aware_member_expression"
            | "selector_expression"
            | "attribute"
            | "field_access"
            | "field_expression"
            | "member_access_expression"
            | "nullsafe_member_access_expression"
            | "navigation_expression"
            | "dot_index_expression"
            | "method_index_expression"
            | "conditional_access_expression"
            | "member_binding_expression"
    )
}
pub(crate) fn is_assignment(kind: &str) -> bool {
    matches!(
        kind,
        "assignment"
            | "assignment_expression"
            | "assignment_statement"
            | "augmented_assignment"
            | "augmented_assignment_expression"
            | "operator_assignment"
            | "reference_assignment_expression"
    )
}
pub(crate) fn is_declaration(kind: &str) -> bool {
    matches!(
        kind,
        "variable_declarator"
            | "init_declarator"
            | "var_definition"
            | "val_definition"
            | "property_declaration"
            | "initialized_identifier"
            | "local_variable_declaration"
            | "initialized_variable_definition"
            | "pattern_variable_declaration"
    )
}
pub(crate) fn is_loop(kind: &str) -> bool {
    matches!(
        kind,
        "for_statement"
            | "for_in_statement"
            | "for_of_statement"
            | "foreach_statement"
            | "enhanced_for_statement"
            | "for_generic_clause"
            | "for_expression"
    )
}
pub(crate) fn is_block(kind: &str) -> bool {
    matches!(
        kind,
        "block"
            | "compound_statement"
            | "statement_block"
            | "body_statement"
            | "function_body"
            | "statements"
            | "program"
            | "module"
            | "source_file"
            | "compilation_unit"
            | "translation_unit"
            | "chunk"
            | "indented_block"
            | "declaration_list"
    )
}

pub(crate) fn split_arguments(text: &str) -> Vec<String> {
    split_top_level(text, ',')
}
pub(crate) fn split_top_level(text: &str, separator: char) -> Vec<String> {
    let mut depth = 0usize;
    let mut start = 0;
    let mut result = Vec::new();
    for (index, c) in text.char_indices() {
        if matches!(c, '<' | '(' | '[' | '{') {
            depth += 1;
        } else if matches!(c, '>' | ')' | ']' | '}') {
            depth = depth.saturating_sub(1);
        } else if c == separator && depth == 0 {
            result.push(text[start..index].trim().into());
            start = index + c.len_utf8();
        }
    }
    result.push(text[start..].trim().into());
    result
}
pub(crate) fn type_kind(text: &str) -> String {
    let text = text.trim().trim_start_matches(':').trim_start_matches('&');
    let text = if text.starts_with('\'') {
        text.split_once(' ').map(|(_, rest)| rest).unwrap_or(text)
    } else {
        text
    };
    let text = text.trim_start_matches("mut ").trim_start_matches("mut");
    let clean = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    if clean.trim_end_matches('?').starts_with('[') && clean.trim_end_matches('?').ends_with(']') {
        return if clean.contains(':') { "map" } else { "array" }.into();
    }
    let tokens = words(&clean);
    if clean.starts_with("map[")
        || tokens.iter().any(|t| {
            matches!(
                t.as_str(),
                "Map"
                    | "ReadonlyMap"
                    | "HashMap"
                    | "BTreeMap"
                    | "Dictionary"
                    | "dict"
                    | "Hash"
                    | "TreeMap"
                    | "ConcurrentHashMap"
                    | "MutableMap"
            )
        })
    {
        return "map".into();
    }
    if tokens
        .iter()
        .any(|t| matches!(t.as_str(), "Set" | "HashSet" | "BTreeSet" | "MutableSet"))
    {
        return "set".into();
    }
    if clean.contains("[]")
        || tokens.iter().any(|t| {
            matches!(
                t.as_str(),
                "List"
                    | "Array"
                    | "ArrayList"
                    | "MutableList"
                    | "vector"
                    | "Vec"
                    | "VecDeque"
                    | "CopyOnWriteArrayList"
                    | "array"
                    | "ReadonlyArray"
            )
        })
    {
        return "array".into();
    }
    String::new()
}
pub(crate) fn element_type(text: &str) -> String {
    let value = text.trim().trim_start_matches([':', ' ']);
    if value.starts_with('{') && value.ends_with('}') {
        if let Some((key, rest)) = value[1..value.len() - 1].trim().split_once(']') {
            if key.starts_with('[') {
                if let Some(ty) = rest.trim().strip_prefix(':') {
                    return ty.trim().into();
                }
            }
        }
    }
    if let Some(value) = value.strip_prefix("[]") {
        return value.into();
    }
    if let Some(value) = value.strip_suffix("[]") {
        return value.into();
    }
    if value.starts_with("map[") {
        if let Some((_, rest)) = value.split_once(']') {
            return rest.into();
        }
    }
    if let Some((_, arguments)) = value.split_once('<') {
        if let Some(arguments) = arguments.strip_suffix('>') {
            let parts = split_arguments(arguments);
            let index = usize::from(type_kind(value) == "map");
            return parts.get(index).cloned().unwrap_or_default();
        }
    }
    String::new()
}

#[derive(Clone, Debug)]
pub(crate) struct Function {
    pub identifier: String,
    pub name: String,
    pub owner: String,
    pub source: usize,
    pub node: NodeId,
    pub body: Option<NodeId>,
    pub parameters: Vec<(String, String)>,
    pub receiver_name: String,
    pub parent: Option<usize>,
    pub is_static: bool,
}
impl Function {
    pub fn receiver(&self) -> Value {
        if self.owner.is_empty() {
            Value::unknown()
        } else if self.is_static {
            Value::new("global", format!("{}::static", self.owner))
        } else {
            Value::new("receiver", &self.owner)
        }
    }
}

#[derive(Default)]
pub(crate) struct Program {
    pub sources: Vec<Arc<Source>>,
    pub module_bindings: BTreeMap<String, String>,
    pub functions: Vec<Function>,
    pub function_nodes: BTreeMap<(usize, NodeId), usize>,
    pub methods: BTreeMap<(String, String), Vec<usize>>,
    pub getters: BTreeMap<(String, String), Vec<usize>>,
    pub fields: BTreeMap<(String, String), String>,
    pub types: BTreeMap<(String, String), String>,
    pub aliases: BTreeMap<(String, String), String>,
    pub imports: BTreeMap<(String, String), String>,
    pub import_paths: BTreeMap<(String, String), String>,
    pub namespaces: BTreeMap<String, String>,
    pub namespace_imports: BTreeMap<String, BTreeSet<String>>,
    pub owner_sources: BTreeMap<String, usize>,
    pub initializers: Vec<(usize, String, String, NodeId, NodeId)>,
    pub events: BTreeSet<(String, String)>,
    pub embedded_fields: BTreeMap<String, Vec<String>>,
    pub dereferences: BTreeMap<(String, String), Vec<usize>>,
    pub interface_methods: BTreeMap<String, BTreeSet<String>>,
    pub tuple_fields: BTreeMap<String, Vec<String>>,
    pub generic_types: BTreeSet<String>,
    pub exports: BTreeMap<(String, String), String>,
    pub global_values: BTreeMap<(String, String), Value>,
    pub global_initializers: Vec<(usize, Value, NodeId, NodeId)>,
    pub sam_methods: BTreeMap<String, String>,
    pub jvm: super::jvm::Jvm,
    pub scala: super::scala::Scala,
    pub rust_scopes: BTreeMap<usize, Vec<super::rust_names::RustScope>>,
}
