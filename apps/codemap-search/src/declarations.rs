//! Source-backed declaration presentation shared by folder and live views.
use crate::parser::{CodeRange, ExtractedFile, ExtractedSymbol};
use tree_sitter::{Node, Point, Tree};

pub(crate) fn kind_label(s: &ExtractedSymbol) -> &str {
    if callable(s) {
        "fn"
    } else {
        &s.kind
    }
}

pub(crate) fn visibility(
    s: &ExtractedSymbol,
    tree: Option<&Tree>,
    source: &str,
    extension: &str,
    is_local: bool,
) -> Option<String> {
    if s.kind == "impl" {
        return None;
    }
    if is_local && s.kind != "field" {
        return Some("local".into());
    }
    if extension == "rs" {
        if let Some(node) = tree.and_then(|tree| symbol_node(tree, s, source)) {
            let mut cursor = node.walk();
            if let Some(value) = node
                .named_children(&mut cursor)
                .find(|child| child.kind() == "visibility_modifier")
                .and_then(|child| text(child, source))
            {
                return Some(value);
            }
            // Trait members inherit the trait contract, not the type's visibility.
            let mut parent = node.parent();
            while let Some(ancestor) = parent {
                if ancestor.kind() == "trait_item"
                    || ancestor.kind() == "impl_item"
                        && ancestor.child_by_field_name("trait").is_some()
                {
                    return Some("trait member".into());
                }
                if matches!(ancestor.kind(), "impl_item" | "function_item") {
                    break;
                }
                parent = ancestor.parent();
            }
            return Some("private".into());
        }
    }
    Some(
        if s.flags.is_exported {
            "exported"
        } else {
            "not exported"
        }
        .into(),
    )
}

/// Preserve language syntax when type-only parameters are unavailable (e.g. Python).
pub(crate) fn folder_signature(s: &ExtractedSymbol, tree: &Tree, source: &str) -> String {
    let fallback = || format!("{} {}", kind_label(s), s.name);
    let Some(mut node) = symbol_node(tree, s, source) else {
        return fallback();
    };
    if callable(s) {
        if node.child_by_field_name("declarator").is_some() {
            if let Some(body) = node.child_by_field_name("body") {
                // C/C++ pointer and qualifier syntax belongs to the declarator,
                // not just the base `type` node. Preserve the complete header.
                if let Some(header) = source.get(node.start_byte()..body.start_byte()) {
                    return header.split_whitespace().collect::<Vec<_>>().join(" ");
                }
            }
        }
        // C-family parameter lists live in nested declarators.
        while node.child_by_field_name("parameters").is_none() {
            let Some(next) = node.child_by_field_name("declarator") else {
                break;
            };
            node = next;
        }
        let parameters = node.child_by_field_name("parameters");
        if let Some(parameters) = parameters {
            let mut cursor = parameters.walk();
            let types = if matches!(node.kind(), "function_item" | "function_signature_item") {
                parameters
                    .named_children(&mut cursor)
                    .filter(|p| !p.kind().contains("comment"))
                    .filter_map(|p| text(p.child_by_field_name("type").unwrap_or(p), source))
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                // Retain named/default/grouped parameters where removing names
                // could change arity or erase pointer/type annotations.
                let value = text(parameters, source).unwrap_or_default();
                value
                    .strip_prefix('(')
                    .and_then(|value| value.strip_suffix(')'))
                    .unwrap_or(&value)
                    .to_string()
            };
            let generics = node
                .child_by_field_name("type_parameters")
                .and_then(|n| text(n, source))
                .unwrap_or_default();
            let result = ["return_type", "result", "type"]
                .iter()
                .find_map(|field| {
                    node.child_by_field_name(field)
                        .and_then(|n| text(n, source))
                })
                .map(|value| format!(" -> {}", value.trim_start_matches(':').trim()))
                .unwrap_or_default();
            return format!("fn {}{generics}({types}){result}", s.name);
        }
    }
    if s.kind == "field" {
        if let Some(typ) = node
            .child_by_field_name("type")
            .and_then(|n| text(n, source))
        {
            return format!("{} {typ}", s.name);
        }
    }
    fallback()
}

pub(crate) fn callable(s: &ExtractedSymbol) -> bool {
    matches!(s.kind.as_str(), "fn" | "function" | "method")
}

pub(crate) fn container(s: &ExtractedSymbol) -> bool {
    matches!(
        s.kind.as_str(),
        "class" | "impl" | "struct" | "interface" | "trait" | "type" | "enum"
    )
}

pub(crate) fn bounds(r: &CodeRange) -> ((usize, usize), (usize, usize)) {
    ((r.start_line, r.start_col), (r.end_line, r.end_col))
}

pub(crate) fn contains(outer: &ExtractedSymbol, inner: &ExtractedSymbol) -> bool {
    let (a, b) = bounds(&outer.range);
    let (c, d) = bounds(&inner.range);
    a <= c && d <= b && (a != c || b != d)
}

pub(crate) fn symbol_node<'a>(
    tree: &'a Tree,
    s: &ExtractedSymbol,
    source: &str,
) -> Option<Node<'a>> {
    let r = &s.range;
    let start = Point::new(
        r.start_line.saturating_sub(1),
        r.start_col.saturating_sub(1),
    );
    let end = Point::new(r.end_line.saturating_sub(1), r.end_col.saturating_sub(1));
    let mut node = tree.root_node().descendant_for_point_range(start, end)?;
    while !crate::parser::node_matches_source_range(node, source.as_bytes(), r) {
        node = node.parent()?;
    }
    // A value-less Go const spec has exactly the same range as its identifier.
    // Prefer the declaration carrying that name over the deepest matching token.
    let original = node;
    while node.child_by_field_name("name").is_none() {
        let Some(parent) = node.parent().filter(|parent| {
            crate::parser::node_matches_source_range(*parent, source.as_bytes(), r)
        }) else {
            return Some(original);
        };
        node = parent;
    }
    Some(node)
}

fn text(node: Node<'_>, source: &str) -> Option<String> {
    Some(
        node.utf8_text(source.as_bytes())
            .ok()?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Add actual Rust impl scopes without changing indexed symbol identity.
/// Fields stay with their type; methods belong to their lexical impl block.
pub(crate) fn add_impl_containers(file: &mut ExtractedFile, tree: &Tree, source: &str) {
    let original_len = file.symbols.len();
    let mut pending = vec![tree.root_node()];
    while let Some(node) = pending.pop() {
        if node.kind() == "impl_item" {
            let start = node.start_position();
            let end = node.end_position();
            let range = CodeRange {
                start_line: start.row + 1,
                start_col: start.column + 1,
                end_line: end.row + 1,
                end_col: end.column + 1,
            };
            let original = &file.symbols[..original_len];
            let owner = original.iter().find_map(|symbol| {
                let (a, b) = bounds(&range);
                let (c, d) = bounds(&symbol.range);
                (a <= c && d <= b && callable(symbol))
                    .then_some(symbol.owner.as_deref())
                    .flatten()
            });
            if original.iter().any(|symbol| {
                let (a, b) = bounds(&range);
                let (c, d) = bounds(&symbol.range);
                a <= c && d <= b
            }) && !original
                .iter()
                .any(|symbol| symbol.kind == "impl" && symbol.range == range)
            {
                let name = node
                    .child_by_field_name("type")
                    .and_then(|node| text(node, source))
                    .or_else(|| owner.map(str::to_string));
                let Some(name) = name else { continue };
                let name = node
                    .child_by_field_name("trait")
                    .and_then(|node| text(node, source))
                    .map_or_else(|| name.clone(), |contract| format!("{contract} for {name}"));
                file.symbols.push(ExtractedSymbol {
                    name,
                    kind: "impl".into(),
                    range,
                    docstring: None,
                    flags: crate::parser::SymbolFlags {
                        has_todo: false,
                        has_fixme: false,
                        is_test: false,
                        is_exported: false,
                        is_deprecated: false,
                    },
                    owner: None,
                });
            }
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
}

pub(crate) fn live_signature(
    s: &ExtractedSymbol,
    tree: Option<&Tree>,
    source: &str,
    is_go: bool,
) -> String {
    let Some(node) = tree.and_then(|t| symbol_node(t, s, source)) else {
        return s.name.clone();
    };
    if is_go && callable(s) {
        let parameters = node
            .child_by_field_name("parameters")
            .and_then(|n| text(n, source));
        if let Some(parameters) = parameters {
            let receiver = node.child_by_field_name("receiver").and_then(|receiver| {
                let mut cursor = receiver.walk();
                let value = receiver
                    .named_children(&mut cursor)
                    .find_map(|p| p.child_by_field_name("type").and_then(|n| text(n, source)));
                value
            });
            let result = node
                .child_by_field_name("result")
                .and_then(|n| text(n, source))
                .map(|x| format!(" {x}"))
                .unwrap_or_default();
            return format!(
                "{}{}{parameters}{result}",
                receiver.map(|x| format!("({x}).")).unwrap_or_default(),
                s.name
            );
        }
    }
    if matches!(s.kind.as_str(), "field" | "variable" | "const") {
        if let Some(typ) = node
            .child_by_field_name("type")
            .and_then(|n| text(n, source))
        {
            return format!("{} {typ}", s.name);
        }
    }
    s.name.clone()
}

pub(crate) fn parents(symbols: &[ExtractedSymbol]) -> Vec<Option<usize>> {
    symbols
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let lexical = symbols
                .iter()
                .enumerate()
                .filter(|(j, c)| *j != i && contains(c, s))
                .min_by_key(|(_, c)| {
                    (
                        c.range.end_line - c.range.start_line,
                        c.range.end_col.saturating_sub(c.range.start_col),
                    )
                })
                .map(|(j, _)| j);
            if lexical.is_some() {
                return lexical;
            }
            s.owner.as_deref().and_then(|owner| {
                let matches: Vec<_> = symbols
                    .iter()
                    .enumerate()
                    .filter(|(j, c)| {
                        *j != i
                            && container(c)
                            && c.name == owner
                            && !symbols
                                .iter()
                                .any(|outer| callable(outer) && contains(outer, c))
                    })
                    .map(|(j, _)| j)
                    .collect();
                (matches.len() == 1).then(|| matches[0])
            })
        })
        .collect()
}
