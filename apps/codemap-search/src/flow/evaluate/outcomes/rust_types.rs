use crate::parser::{CodeRange, ImportEntry, ImportKind};
use std::collections::BTreeMap;
use tree_sitter::{Node, Point, Tree};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Type {
    Json(String),
    Reference(Box<Type>),
    Result(Box<Type>),
    Tuple(Vec<Type>),
    Primitive(String),
    String,
    Unknown,
}

pub(super) struct Facts {
    pub crate_name: String,
    pub result: Type,
    pub input_range: CodeRange,
    pub import_ranges: Vec<CodeRange>,
    pub is_known_object: bool,
    pub has_identity_error: bool,
}

pub(super) fn range(node: Node<'_>) -> CodeRange {
    CodeRange {
        start_line: node.start_position().row + 1,
        start_col: node.start_position().column + 1,
        end_line: node.end_position().row + 1,
        end_col: node.end_position().column + 1,
    }
}
fn text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}
fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}
fn node_at<'a>(tree: &'a Tree, source: &str, range: &CodeRange) -> Option<Node<'a>> {
    let mut node = tree.root_node().descendant_for_point_range(
        Point::new(
            range.start_line.checked_sub(1)?,
            range.start_col.checked_sub(1)?,
        ),
        Point::new(
            range.end_line.checked_sub(1)?,
            range.end_col.checked_sub(1)?,
        ),
    )?;
    while !crate::parser::node_matches_source_range(node, source.as_bytes(), range) {
        node = node.parent()?;
    }
    Some(node)
}
fn scope(mut node: Node<'_>) -> Node<'_> {
    while let Some(parent) = node.parent() {
        if matches!(parent.kind(), "source_file" | "block" | "declaration_list") {
            return parent;
        }
        node = parent;
    }
    node
}

fn has_scope_macro(node: Node<'_>) -> bool {
    node.kind() == "macro_invocation"
        || node.kind() == "expression_statement"
            && node
                .named_child(0)
                .is_some_and(|child| child.kind() == "macro_invocation")
}

struct Resolver<'a> {
    source: &'a str,
    tree: &'a Tree,
    imports: &'a [ImportEntry],
    used_imports: Vec<CodeRange>,
}
impl<'a> Resolver<'a> {
    /// Check the closure's source types before treating `?` as an identity error
    /// conversion. In particular, a bare Rust string literal is `&str`, not String.
    fn constructs(&mut self, node: Node<'a>, expected: &Type, depth: usize) -> bool {
        if depth > 8 || node.has_error() {
            return false;
        }
        match node.kind() {
            "block" => {
                let statements = children(node)
                    .into_iter()
                    .filter(|node| !node.kind().contains("comment"))
                    .collect::<Vec<_>>();
                statements.len() == 1 && self.constructs(statements[0], expected, depth + 1)
            }
            "expression_statement" | "return_expression" | "parenthesized_expression" => node
                .named_child(0)
                .is_some_and(|node| self.constructs(node, expected, depth + 1)),
            "tuple_expression" => {
                let Type::Tuple(types) = expected else {
                    return false;
                };
                let values = children(node);
                values.len() == types.len()
                    && values
                        .iter()
                        .zip(types)
                        .all(|(&node, ty)| self.constructs(node, ty, depth + 1))
            }
            "if_expression" => {
                let Some(consequence) = node.child_by_field_name("consequence") else {
                    return false;
                };
                let Some(alternative) = node.child_by_field_name("alternative") else {
                    return false;
                };
                let alternative = if alternative.kind() == "else_clause" {
                    alternative.named_child(0).unwrap_or(alternative)
                } else {
                    alternative
                };
                self.constructs(consequence, expected, depth + 1)
                    && self.constructs(alternative, expected, depth + 1)
            }
            "integer_literal" | "unary_expression" => {
                let value = text(node, self.source);
                let Type::Primitive(name) = expected else {
                    return false;
                };
                match name.as_str() {
                    "i8" => value.parse::<i8>().is_ok(),
                    "i16" => value.parse::<i16>().is_ok(),
                    "i32" => value.parse::<i32>().is_ok(),
                    "i64" => value.parse::<i64>().is_ok(),
                    "i128" => value.parse::<i128>().is_ok(),
                    "u8" => value.parse::<u8>().is_ok(),
                    "u16" => value.parse::<u16>().is_ok(),
                    "u32" => value.parse::<u32>().is_ok(),
                    "u64" => value.parse::<u64>().is_ok(),
                    "u128" => value.parse::<u128>().is_ok(),
                    _ => false,
                }
            }
            "call_expression" if expected == &Type::String => {
                let Some(function) = node.child_by_field_name("function") else {
                    return false;
                };
                let Some(arguments) = node.child_by_field_name("arguments") else {
                    return false;
                };
                if function.kind() == "field_expression" && arguments.named_child_count() == 0 {
                    let Some(receiver) = function.child_by_field_name("value") else {
                        return false;
                    };
                    let Some(method) = function.child_by_field_name("field") else {
                        return false;
                    };
                    return matches!(receiver.kind(), "string_literal" | "raw_string_literal")
                        && text(method, self.source) == "into";
                }
                false
            }
            _ => false,
        }
    }
    fn scopes(&self, mut at: Node<'a>) -> Vec<Node<'a>> {
        let mut scopes = Vec::new();
        loop {
            if matches!(
                at.kind(),
                "source_file"
                    | "block"
                    | "declaration_list"
                    | "function_item"
                    | "impl_item"
                    | "trait_item"
            ) {
                scopes.push(at);
            }
            if at.kind() == "source_file"
                || at.kind() == "declaration_list"
                    && at.parent().is_some_and(|p| p.kind() == "mod_item")
            {
                break;
            }
            let Some(parent) = at.parent() else { break };
            at = parent;
        }
        scopes
    }
    fn visible_imports(&self, at: Node<'a>) -> Vec<&'a ImportEntry> {
        let scopes = self.scopes(at);
        self.imports
            .iter()
            .filter(|entry| {
                node_at(self.tree, self.source, &entry.range).is_some_and(|node| {
                    scopes
                        .iter()
                        .any(|candidate| candidate.id() == scope(node).id())
                })
            })
            .collect()
    }
    fn declaration(&self, name: &str, at: Node<'a>) -> Option<Node<'a>> {
        for scope in self.scopes(at) {
            if let Some(parameters) = scope.child_by_field_name("type_parameters") {
                if let Some(parameter) = children(parameters).into_iter().find(|parameter| {
                    parameter
                        .child_by_field_name("name")
                        .is_some_and(|node| text(node, self.source) == name)
                }) {
                    return Some(parameter);
                }
            }
            if matches!(scope.kind(), "function_item" | "impl_item" | "trait_item") {
                continue;
            }
            if let Some(item) = children(scope).into_iter().find(|item| {
                matches!(
                    item.kind(),
                    "type_item"
                        | "struct_item"
                        | "enum_item"
                        | "trait_item"
                        | "mod_item"
                        | "extern_crate_declaration"
                ) && item
                    .child_by_field_name("alias")
                    .or_else(|| item.child_by_field_name("name"))
                    .is_some_and(|node| text(node, self.source) == name)
            }) {
                return Some(item);
            }
        }
        None
    }
    fn resolve(
        &mut self,
        node: Node<'a>,
        substitutions: &BTreeMap<String, Type>,
        depth: usize,
    ) -> Type {
        if depth > 8 || node.has_error() {
            return Type::Unknown;
        }
        match node.kind() {
            "reference_type" => node
                .child_by_field_name("type")
                .map(|node| Type::Reference(Box::new(self.resolve(node, substitutions, depth + 1))))
                .unwrap_or(Type::Unknown),
            "unit_type" => Type::Tuple(Vec::new()),
            "tuple_type" => {
                let items = children(node);
                if items.len() > 32 {
                    return Type::Unknown;
                }
                Type::Tuple(
                    items
                        .into_iter()
                        .map(|node| self.resolve(node, substitutions, depth + 1))
                        .collect(),
                )
            }
            "generic_type" => {
                let Some(base) = node.child_by_field_name("type") else {
                    return Type::Unknown;
                };
                let Some(arguments) = node.child_by_field_name("type_arguments") else {
                    return Type::Unknown;
                };
                let arguments = children(arguments);
                if arguments.len() > 8 {
                    return Type::Unknown;
                }
                let arguments = arguments
                    .into_iter()
                    .map(|arg| self.resolve(arg, substitutions, depth + 1))
                    .collect();
                self.path(
                    text(base, self.source),
                    node,
                    arguments,
                    substitutions,
                    depth + 1,
                )
            }
            "type_identifier"
            | "primitive_type"
            | "scoped_type_identifier"
            | "scoped_identifier"
            | "identifier" => self.path(
                text(node, self.source),
                node,
                Vec::new(),
                substitutions,
                depth + 1,
            ),
            _ => Type::Unknown,
        }
    }
    fn path(
        &mut self,
        path: &str,
        at: Node<'a>,
        arguments: Vec<Type>,
        substitutions: &BTreeMap<String, Type>,
        depth: usize,
    ) -> Type {
        if depth > 8 {
            return Type::Unknown;
        }
        if let Some(ty) = substitutions.get(path) {
            return ty.clone();
        }
        let path = path.trim_start_matches("::");
        let parts = path.split("::").collect::<Vec<_>>();
        if let Some(declaration) = self.declaration(parts[0], at) {
            if declaration.kind() == "type_item" && parts.len() == 1 {
                self.used_imports.push(range(declaration));
                let parameters = declaration
                    .child_by_field_name("type_parameters")
                    .map(children)
                    .unwrap_or_default();
                if parameters.len() != arguments.len() {
                    return Type::Unknown;
                }
                let mut substituted = BTreeMap::new();
                for (parameter, argument) in parameters.into_iter().zip(arguments) {
                    let Some(name) = parameter.child_by_field_name("name") else {
                        return Type::Unknown;
                    };
                    substituted.insert(text(name, self.source).into(), argument);
                }
                return declaration
                    .child_by_field_name("type")
                    .map(|node| self.resolve(node, &substituted, depth + 1))
                    .unwrap_or(Type::Unknown);
            }
            return Type::Unknown;
        }
        let imports = self
            .visible_imports(at)
            .into_iter()
            .filter(|entry| entry.local_name == parts[0])
            .collect::<Vec<_>>();
        if imports.len() > 1 {
            return Type::Unknown;
        }
        if let Some(import) = imports.first() {
            let Some(imported) = import.source.as_deref() else {
                return Type::Unknown;
            };
            self.used_imports.push(import.range.clone());
            let expanded = if parts.len() == 1 {
                imported.into()
            } else {
                format!("{imported}::{}", parts[1..].join("::"))
            };
            // Do not expand a same-name import indefinitely (`use serde_json;`).
            if expanded != path {
                return self.path(&expanded, at, arguments, substitutions, depth + 1);
            }
        }
        match parts.as_slice() {
            ["Result"] | ["std" | "core", "result", "Result"] if arguments.len() == 2 => {
                Type::Result(Box::new(arguments[1].clone()))
            }
            ["String"] | ["std" | "alloc", "string", "String"] if arguments.is_empty() => {
                Type::String
            }
            [name]
                if matches!(
                    *name,
                    "i8" | "i16"
                        | "i32"
                        | "i64"
                        | "i128"
                        | "isize"
                        | "u8"
                        | "u16"
                        | "u32"
                        | "u64"
                        | "u128"
                        | "usize"
                        | "bool"
                        | "str"
                ) && arguments.is_empty() =>
            {
                Type::Primitive((*name).into())
            }
            [dependency, "Value"] | [dependency, "value", "Value"]
                if !matches!(*dependency, "self" | "super" | "crate") && arguments.is_empty() =>
            {
                Type::Json((*dependency).into())
            }
            _ => Type::Unknown,
        }
    }
    fn has_unknown_method_scope(&self, at: Node<'a>, dependency: &str) -> bool {
        if self.source.contains("#![no_implicit_prelude]") || self.source.contains("#![feature(") {
            return true;
        }
        for scope in self.scopes(at) {
            if !matches!(scope.kind(), "block" | "source_file" | "declaration_list") {
                continue;
            }
            for item in children(scope) {
                if has_scope_macro(item) {
                    return true;
                }
                if item.kind() == "trait_item"
                    && item.child_by_field_name("body").is_some_and(|body| {
                        children(body).iter().any(|method| {
                            method.child_by_field_name("name").is_some_and(|name| {
                                matches!(text(name, self.source), "as_object" | "into" | "from")
                            })
                        })
                    })
                {
                    return true;
                }
            }
        }
        self.visible_imports(at).iter().any(|entry| {
            if entry.kind == ImportKind::Glob {
                return true;
            }
            let source = entry.source.as_deref().unwrap_or("");
            let root = source
                .trim_start_matches("::")
                .split("::")
                .next()
                .unwrap_or("");
            if self.declaration(root, at).is_some() {
                return true;
            }
            // Unknown imported traits may alter method lookup. Known standard
            // types and serde_json type/module imports do not add such methods.
            !(self.used_imports.contains(&entry.range)
                || matches!(
                    source,
                    "std::result::Result"
                        | "core::result::Result"
                        | "std::string::String"
                        | "alloc::string::String"
                )
                || root == dependency
                    && matches!(
                        source.rsplit("::").next(),
                        Some("Value" | "Map" | "Number" | "Error" | "json")
                    ))
        })
    }
}

pub(super) fn inspect(
    source: &str,
    tree: &Tree,
    imports: &[ImportEntry],
    binding: Option<&CodeRange>,
    receiver: &CodeRange,
    function: &CodeRange,
    callback: &CodeRange,
) -> Option<Facts> {
    let receiver_node = node_at(tree, source, receiver)?;
    let function_node = node_at(tree, source, function)?;
    let mut resolver = Resolver {
        source,
        tree,
        imports,
        used_imports: Vec::new(),
    };
    let input_node = binding.and_then(|binding| node_at(tree, source, binding));
    let candidate = input_node
        .and_then(|node| node.child_by_field_name("value"))
        .unwrap_or(receiver_node);
    let constructor = (candidate.kind() == "call_expression")
        .then(|| candidate.child_by_field_name("function"))
        .flatten()
        .filter(|callee| callee.kind() == "scoped_identifier");
    let constructor_type = constructor.and_then(|callee| {
        callee
            .child_by_field_name("path")
            .map(|base| resolver.path(text(base, source), callee, Vec::new(), &BTreeMap::new(), 0))
    });
    let ty = if let Some(ty) = input_node.and_then(|node| node.child_by_field_name("type")) {
        resolver.resolve(ty, &BTreeMap::new(), 0)
    } else {
        constructor_type.clone()?
    };
    let mut ty = ty;
    while let Type::Reference(inner) = ty {
        ty = *inner;
    }
    let Type::Json(crate_name) = ty else {
        return None;
    };
    let is_known_object = constructor_type == Some(Type::Json(crate_name.clone()))
        && input_node.is_none_or(|node| {
            !children(node)
                .iter()
                .any(|node| node.kind() == "mutable_specifier")
        })
        && constructor
            .and_then(|callee| callee.child_by_field_name("name"))
            .is_some_and(|name| text(name, source) == "Object");
    if resolver.has_unknown_method_scope(receiver_node, &crate_name) {
        return None;
    }
    let result = function_node
        .child_by_field_name("return_type")
        .map(|node| resolver.resolve(node, &BTreeMap::new(), 0))
        .unwrap_or(Type::Unknown);
    let has_identity_error = if let Type::Result(error) = &result {
        node_at(tree, source, callback)
            .and_then(|node| node.child_by_field_name("body"))
            .is_some_and(|body| resolver.constructs(body, error, 0))
    } else {
        false
    };
    Some(Facts {
        crate_name,
        result,
        has_identity_error,
        input_range: input_node.map(range).unwrap_or_else(|| receiver.clone()),
        import_ranges: resolver.used_imports,
        is_known_object,
    })
}

pub(super) fn root_shadows(source: &str, tree: &Tree, name: &str) -> bool {
    source.contains("#![no_implicit_prelude]")
        || source.contains("#![feature(")
        || children(tree.root_node()).iter().any(|node| {
            has_scope_macro(*node)
                || matches!(node.kind(), "mod_item" | "extern_crate_declaration")
                    && node
                        .child_by_field_name("alias")
                        .or_else(|| node.child_by_field_name("name"))
                        .is_some_and(|node| {
                            text(node, source) == name
                                || matches!(text(node, source), "std" | "core" | "alloc")
                        })
        })
}

pub(super) fn has_identity_tuple_error(ty: &Type, value: &super::ReturnedValue) -> bool {
    let (Type::Result(error), super::ReturnedValue::Tuple(values)) = (ty, value) else {
        return false;
    };
    let Type::Tuple(types) = error.as_ref() else {
        return false;
    };
    if types.len() != values.len() {
        return false;
    }
    // A tuple of standard types has no project-defined From<Tuple> conversion.
    // Keep only values compatible with this identity conversion; unknown tuple
    // elements stay unknown (including unproven `.into()` results).
    types
        .iter()
        .zip(values)
        .all(|(ty, value)| match (ty, value) {
            (
                Type::String,
                super::ReturnedValue::Unknown
                | super::ReturnedValue::Constant(super::Constant::String(_)),
            ) => true,
            (Type::Primitive(name), super::ReturnedValue::Unknown) => matches!(
                name.as_str(),
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "bool"
            ),
            (
                Type::Primitive(name),
                super::ReturnedValue::Constant(super::Constant::Number(value)),
            ) => match name.as_str() {
                "i8" => value.parse::<i8>().is_ok(),
                "i16" => value.parse::<i16>().is_ok(),
                "i32" => value.parse::<i32>().is_ok(),
                "i64" => value.parse::<i64>().is_ok(),
                "i128" => value.parse::<i128>().is_ok(),
                "u8" => value.parse::<u8>().is_ok(),
                "u16" => value.parse::<u16>().is_ok(),
                "u32" => value.parse::<u32>().is_ok(),
                "u64" => value.parse::<u64>().is_ok(),
                "u128" => value.parse::<u128>().is_ok(),
                _ => false,
            },
            _ => false,
        })
}
