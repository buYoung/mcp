//! Immutable bindings verified from each language's declaration syntax.

use crate::parser::ExtractedSymbol;
use tree_sitter::Node;

fn child<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == kind);
    found
}

fn has_token(node: Node<'_>, token: &str) -> bool {
    let mut cursor = node.walk();
    let found = node
        .children(&mut cursor)
        .any(|child| child.kind() == token);
    found
}

fn name_matches(node: Option<Node<'_>>, name: &str, source: &str) -> bool {
    node.and_then(|node| node.utf8_text(source.as_bytes()).ok()) == Some(name)
}

fn next_non_comment_sibling(node: Node<'_>) -> Option<Node<'_>> {
    std::iter::successors(node.next_named_sibling(), |node| node.next_named_sibling())
        .find(|node| !matches!(node.kind(), "comment" | "line_comment" | "block_comment"))
}

/// Returns the binding node and its initializer. This never searches initializer
/// text for a name: multiple declarators are paired with their own value.
pub(super) fn binding<'a>(
    node: Node<'a>,
    symbol: &ExtractedSymbol,
    language: &str,
    source: &str,
) -> Option<(Node<'a>, Option<Node<'a>>)> {
    let name = symbol.name.as_str();
    match language {
        "java" | "groovy" if node.kind() == "field_declaration" => {
            if !child(node, "modifiers").is_some_and(|modifiers| has_token(modifiers, "final")) {
                return None;
            }
            let mut cursor = node.walk();
            let declaration = node.named_children(&mut cursor).find(|child| {
                child.kind() == "variable_declarator"
                    && name_matches(child.child_by_field_name("name"), name, source)
            })?;
            Some((declaration, declaration.child_by_field_name("value")))
        }
        "csharp" if node.kind() == "field_declaration" => {
            let mut cursor = node.walk();
            if !node
                .named_children(&mut cursor)
                .any(|child| child.kind() == "modifier" && has_token(child, "const"))
            {
                return None;
            }
            let variables = child(node, "variable_declaration")?;
            let mut cursor = variables.walk();
            let declaration = variables.named_children(&mut cursor).find(|child| {
                child.kind() == "variable_declarator"
                    && name_matches(child.child_by_field_name("name"), name, source)
            })?;
            let value = next_non_comment_sibling(declaration.child_by_field_name("name")?);
            Some((declaration, value))
        }
        "php" if node.kind() == "const_declaration" => {
            // Bare PHP names never refer to a class constant. Unbraced namespaces
            // also do not enclose their following declarations in the syntax tree;
            // leave that module resolution to a future qualified-name resolver.
            let mut ancestor = node.parent();
            while let Some(parent) = ancestor {
                if matches!(
                    parent.kind(),
                    "class_declaration"
                        | "anonymous_class"
                        | "interface_declaration"
                        | "trait_declaration"
                        | "enum_declaration"
                ) {
                    return None;
                }
                if parent.kind() == "program" {
                    let mut cursor = parent.walk();
                    if parent.named_children(&mut cursor).any(|child| {
                        child.kind() == "namespace_definition"
                            && child.child_by_field_name("body").is_none()
                    }) {
                        return None;
                    }
                }
                ancestor = parent.parent();
            }
            let mut cursor = node.walk();
            let declaration = node.named_children(&mut cursor).find(|child| {
                child.kind() == "const_element" && name_matches(child.named_child(0), name, source)
            })?;
            Some((declaration, declaration.named_child(1)))
        }
        "ruby" if node.kind() == "assignment" => {
            let left = node.child_by_field_name("left")?;
            (left.kind() == "constant" && name_matches(Some(left), name, source))
                .then_some((node, node.child_by_field_name("right")))
        }
        "kotlin" if node.kind() == "property_declaration" && has_token(node, "val") => {
            let declaration = child(node, "variable_declaration")?;
            if !name_matches(declaration.named_child(0), name, source) {
                return None;
            }
            Some((
                node,
                has_token(node, "=")
                    .then(|| next_non_comment_sibling(declaration))
                    .flatten(),
            ))
        }
        "swift" if node.kind() == "property_declaration" => {
            if !child(node, "value_binding_pattern")
                .is_some_and(|binding| has_token(binding, "let"))
            {
                return None;
            }
            let pattern = node.child_by_field_name("name")?;
            name_matches(
                pattern.child_by_field_name("bound_identifier"),
                name,
                source,
            )
            .then_some((node, node.child_by_field_name("value")))
        }
        "scala" if node.kind() == "val_definition" => {
            name_matches(node.child_by_field_name("pattern"), name, source)
                .then_some((node, node.child_by_field_name("value")))
        }
        "dart" if node.kind() == "static_final_declaration" => (node
            .parent()
            .and_then(|parent| parent.parent())
            .is_some_and(|parent| has_token(parent, "const"))
            && name_matches(node.child_by_field_name("name"), name, source))
        .then_some((node, node.child_by_field_name("value"))),
        "c" | "cpp" if node.kind() == "init_declarator" => {
            let parent = node.parent()?;
            let mut cursor = parent.walk();
            let is_const = parent.named_children(&mut cursor).any(|child| {
                child.kind() == "type_qualifier"
                    && child
                        .utf8_text(source.as_bytes())
                        .is_ok_and(|text| matches!(text, "const" | "constexpr"))
            });
            (is_const && name_matches(node.child_by_field_name("declarator"), name, source))
                .then_some((node, node.child_by_field_name("value")))
        }
        _ => {
            let is_const = symbol.kind == "const"
                || (symbol.kind == "variable"
                    && node.parent().is_some_and(|parent| {
                        parent.kind() == "lexical_declaration" && has_token(parent, "const")
                    }));
            (is_const && name_matches(node.child_by_field_name("name"), name, source))
                .then_some((node, node.child_by_field_name("value")))
        }
    }
}
