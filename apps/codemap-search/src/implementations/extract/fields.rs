//! Declared types of the instance fields of a type: field and property declarations,
//! constructor parameters that become fields, and typed self attributes.

use super::*;
use std::collections::HashMap;

/// Every typed field one type declaration declares, read once per type.
pub(super) struct Fields {
    by_name: HashMap<String, Vec<TypeReference>>,
}

impl Fields {
    pub(super) fn of(owner: Node<'_>, language: &str, source: &[u8]) -> Self {
        let mut by_name: HashMap<String, Vec<TypeReference>> = HashMap::new();
        let mut pending = children(owner);
        while let Some(node) = pending.pop() {
            // Nested types declare their own fields, and method bodies declare locals; Python
            // and Ruby assign instance attributes inside methods.
            if is_type(node) || !matches!(language, "python" | "ruby") && is_method_body(node) {
                continue;
            }
            for (name, typ) in declared_fields(node, language, source) {
                if let Some(typ) = typ.and_then(|typ| declared(typ, source)) {
                    by_name.entry(name).or_default().push(typ);
                }
            }
            pending.extend(children(node));
        }
        Self { by_name }
    }

    /// The one declared type of field `name`; `None` when it is undeclared, untyped or
    /// declared with different types.
    pub(super) fn get(&self, name: &str) -> Option<TypeReference> {
        let found = self.by_name.get(name)?;
        let first = found.first()?;
        found
            .iter()
            .all(|typ| typ.name == first.name && typ.arguments == first.arguments)
            .then(|| first.clone())
    }
}

/// The fields `node` declares, by name, with their written type node.
fn declared_fields<'a>(
    node: Node<'a>,
    language: &str,
    source: &'a [u8],
) -> Vec<(String, Option<Node<'a>>)> {
    let named = |name: &str, typ: Option<Node<'a>>| vec![(name.to_string(), typ)];
    match node.kind() {
        // TS class fields, C# properties and Swift stored properties.
        "public_field_definition" | "property_declaration"
            if language != "kotlin" && language != "php" =>
        {
            node.child_by_field_name("name")
                .map(|field| {
                    named(
                        member_name(field, source),
                        node.child_by_field_name("type")
                            .or_else(|| child(node, &["type_annotation"])),
                    )
                })
                .unwrap_or_default()
        }
        // Java/Groovy/C++ fields and C# field declarations.
        "field_declaration" | "variable_declaration" => {
            let typ = node.child_by_field_name("type");
            declarators(node)
                .into_iter()
                .map(|field| (member_name(field, source).to_string(), typ))
                .collect()
        }
        // Kotlin `val x: T` members.
        "property_declaration" if language == "kotlin" => child(node, &["variable_declaration"])
            .and_then(|variable| {
                child(variable, &["identifier", "simple_identifier"]).map(|field| {
                    named(
                        text(field, source),
                        child(variable, &["user_type", "nullable_type", "type"]),
                    )
                })
            })
            .unwrap_or_default(),
        // Scala class parameters; Kotlin ones only with `val`/`var`.
        "class_parameter"
            if language != "kotlin"
                || children_with_tokens(node)
                    .iter()
                    .any(|token| matches!(text(*token, source), "val" | "var")) =>
        {
            child(node, &["identifier", "simple_identifier"])
                .or_else(|| node.child_by_field_name("name"))
                .map(|field| {
                    named(
                        text(field, source),
                        node.child_by_field_name("type")
                            .or_else(|| child(node, &["user_type", "nullable_type"])),
                    )
                })
                .unwrap_or_default()
        }
        // Scala `val`/`var` members.
        "val_definition" | "var_definition" => node
            .child_by_field_name("pattern")
            .map(|field| named(text(field, source), node.child_by_field_name("type")))
            .unwrap_or_default(),
        // Dart members: `final T x;`.
        "declaration" if language == "dart" => {
            let typ = child(node, &["type"]);
            child(node, &["initialized_identifier_list"])
                .map(children)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|field| field.child_by_field_name("name"))
                .map(|field| (text(field, source).to_string(), typ))
                .collect()
        }
        // PHP typed properties and promoted constructor parameters.
        "property_declaration" | "property_promotion_parameter" if language == "php" => {
            let typ = node.child_by_field_name("type");
            let names = if node.kind() == "property_declaration" {
                children(node)
                    .into_iter()
                    .filter(|element| element.kind() == "property_element")
                    .filter_map(|element| element.child_by_field_name("name"))
                    .collect::<Vec<_>>()
            } else {
                node.child_by_field_name("name").into_iter().collect()
            };
            names
                .into_iter()
                .map(|field| (text(field, source).trim_start_matches('$').to_string(), typ))
                .collect()
        }
        // TS parameter properties: an accessibility modifier or `readonly` makes a field.
        "required_parameter" | "optional_parameter"
            if child(node, &["accessibility_modifier"]).is_some()
                || children_with_tokens(node)
                    .iter()
                    .any(|token| token.kind() == "readonly") =>
        {
            node.child_by_field_name("pattern")
                .map(|field| named(text(field, source), node.child_by_field_name("type")))
                .unwrap_or_default()
        }
        // Python class annotations and `self.x` assignments.
        "assignment" if language == "python" => {
            let Some(left) = node.child_by_field_name("left") else {
                return Vec::new();
            };
            let field = match left.kind() {
                "identifier"
                    if node
                        .parent()
                        .and_then(|parent| parent.parent())
                        .is_some_and(|block| block.parent().is_some_and(is_type)) =>
                {
                    Some(text(left, source))
                }
                "attribute"
                    if left
                        .child_by_field_name("object")
                        .is_some_and(|object| text(object, source) == "self") =>
                {
                    left.child_by_field_name("attribute")
                        .map(|field| text(field, source))
                }
                _ => None,
            };
            field
                .map(|field| {
                    named(
                        field,
                        node.child_by_field_name("type")
                            .or_else(|| python_value_type(node, source)),
                    )
                })
                .unwrap_or_default()
        }
        // Ruby `@x = Type.new`.
        "assignment" if language == "ruby" => {
            let Some(left) = node
                .child_by_field_name("left")
                .filter(|left| left.kind() == "instance_variable")
            else {
                return Vec::new();
            };
            let typ = node.child_by_field_name("right").and_then(|right| {
                (right.kind() == "call"
                    && right
                        .child_by_field_name("method")
                        .is_some_and(|method| text(method, source) == "new"))
                .then(|| right.child_by_field_name("receiver"))
                .flatten()
                .filter(|receiver| receiver.kind() == "constant")
            });
            named(text(left, source).trim_start_matches('@'), typ)
        }
        // PowerShell `[T] $x` class properties.
        "class_property_definition" => child(node, &["variable"])
            .map(|field| {
                named(
                    text(field, source).trim_start_matches('$'),
                    child(node, &["type_literal"]),
                )
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Declarators of a field declaration: Java/Groovy `variable_declarator`, C# `variable_declarator`
/// inside its declaration, and C++ field identifiers behind pointer or reference declarators.
fn declarators(node: Node<'_>) -> Vec<Node<'_>> {
    let mut result = Vec::new();
    for item in children(node) {
        match item.kind() {
            "variable_declarator" => result.extend(item.child_by_field_name("name").or(Some(item))),
            "field_identifier" | "pointer_declarator" | "reference_declarator" => result.push(item),
            _ => {}
        }
    }
    result
}

/// The identifier a field declarator names, through C++ pointer and reference declarators.
fn member_name<'a>(node: Node<'_>, source: &'a [u8]) -> &'a str {
    let mut current = node;
    while !matches!(
        current.kind(),
        "identifier" | "field_identifier" | "property_identifier" | "simple_identifier"
    ) {
        let Some(next) = current
            .child_by_field_name("declarator")
            .or_else(|| child(current, &["field_identifier", "identifier"]))
        else {
            break;
        };
        current = next;
    }
    text(current, source)
}

/// The type a Python `self.x = value` assignment proves: an annotated parameter of the
/// enclosing function or a call of a class-like name.
fn python_value_type<'a>(assignment: Node<'a>, source: &[u8]) -> Option<Node<'a>> {
    let right = assignment.child_by_field_name("right")?;
    match right.kind() {
        "identifier" => {
            let parameter = text(right, source);
            let mut current = assignment.parent();
            while let Some(node) = current {
                if node.kind() == "function_definition" {
                    return children(node.child_by_field_name("parameters")?)
                        .into_iter()
                        .find(|candidate| {
                            candidate.kind() == "typed_parameter"
                                && child(*candidate, &["identifier"])
                                    .is_some_and(|name| text(name, source) == parameter)
                        })
                        .and_then(|candidate| candidate.child_by_field_name("type"));
                }
                current = node.parent();
            }
            None
        }
        "call" => right.child_by_field_name("function").filter(|function| {
            function.kind() == "identifier"
                && text(*function, source)
                    .chars()
                    .next()
                    .is_some_and(char::is_uppercase)
        }),
        _ => None,
    }
}

/// A named type behind annotations, optional markers and wrappers.
fn declared(node: Node<'_>, source: &[u8]) -> Option<TypeReference> {
    let mut current = node;
    loop {
        let next = match current.kind() {
            "type_annotation" | "type" | "type_literal" | "type_spec" | "named_type"
                if current.named_child_count() == 1 =>
            {
                current.named_child(0)
            }
            "nullable_type" | "optional_type" => current
                .child_by_field_name("wrapped")
                .or_else(|| current.child_by_field_name("type"))
                .or_else(|| current.named_child(0)),
            _ => None,
        };
        match next {
            Some(next) => current = next,
            None => break,
        }
    }
    if matches!(current.kind(), "nullable_type" | "optional_type") {
        return None;
    }
    reference(current, source)
}

fn is_method_body(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        is_method(parent) && body(parent).is_some_and(|body| body.id() == node.id())
    })
}

fn children_with_tokens(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}
