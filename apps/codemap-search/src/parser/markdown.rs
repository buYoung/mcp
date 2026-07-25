//! Markdown-specific extraction over tree-sitter-md's combined block and inline trees.

use tree_sitter::Node;
use tree_sitter_md::{MarkdownCursor, MarkdownParser};

use super::{CodeRange, ExtractedFile, ExtractedSymbol, IndexAuxiliary, SymbolFlags};

pub(super) fn extract(
    file_content: &str,
    file_path: &str,
    collect_auxiliary: bool,
) -> Result<(ExtractedFile, IndexAuxiliary), String> {
    let source = file_content.as_bytes();
    let mut parser = MarkdownParser::default();
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| format!("Failed to parse Markdown file content: {file_path}"))?;
    let mut symbols = Vec::new();
    let mut cursor = tree.walk();
    walk(&mut cursor, source, &mut symbols);

    let mut auxiliary = IndexAuxiliary::default();
    if collect_auxiliary {
        auxiliary.format_text.push(file_content.to_string());
    }

    Ok((
        ExtractedFile {
            file_path: file_path.to_string(),
            total_lines: file_content.lines().count(),
            symbols,
            literals: Vec::new(),
            docstrings: Vec::new(),
            navigation: None,
        },
        auxiliary,
    ))
}

fn walk(cursor: &mut MarkdownCursor<'_>, source: &[u8], symbols: &mut Vec<ExtractedSymbol>) {
    let node = cursor.node();
    let symbol = if cursor.is_inline() {
        link_symbol(node, source)
    } else {
        block_symbol(node, source)
    };
    if let Some(symbol) = symbol {
        symbols.push(symbol);
    }

    if cursor.goto_first_child() {
        loop {
            walk(cursor, source, symbols);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn block_symbol(node: Node<'_>, source: &[u8]) -> Option<ExtractedSymbol> {
    match node.kind() {
        "atx_heading" | "setext_heading" => {
            let name = heading_name(node, source);
            (!name.is_empty()).then(|| symbol(name, heading_kind(node, source).to_string(), node))
        }
        "link_reference_definition" => link_from_parts(node, source),
        "fenced_code_block" => {
            let name = named_descendant(node, "info_string")
                .and_then(|info| node_text(info, source))
                .and_then(|text| text.split_whitespace().next().map(str::to_string))
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| "code block".to_string());
            Some(symbol(name, "code".to_string(), node))
        }
        "indented_code_block" => Some(symbol("code block".to_string(), "code".to_string(), node)),
        _ => None,
    }
}

fn link_symbol(node: Node<'_>, source: &[u8]) -> Option<ExtractedSymbol> {
    if !matches!(
        node.kind(),
        "inline_link"
            | "full_reference_link"
            | "collapsed_reference_link"
            | "shortcut_link"
            | "uri_autolink"
            | "email_autolink"
    ) {
        return None;
    }
    link_from_parts(node, source)
}

fn link_from_parts(node: Node<'_>, source: &[u8]) -> Option<ExtractedSymbol> {
    let name = named_descendant(node, "link_text")
        .or_else(|| named_descendant(node, "link_label"))
        .or_else(|| named_descendant(node, "link_destination"))
        .and_then(|part| node_text(part, source))
        .map(|text| trim_link_delimiters(&text).to_string())
        .filter(|text| !text.is_empty())
        .or_else(|| {
            node_text(node, source)
                .map(|text| trim_link_delimiters(&text).to_string())
                .filter(|text| !text.is_empty())
        })?;
    Some(symbol(name, "link".to_string(), node))
}

fn heading_name(node: Node<'_>, source: &[u8]) -> String {
    if let Some(content) = node.child_by_field_name("heading_content") {
        return display_text(node_text(content, source).as_deref().unwrap_or(""));
    }
    let raw = node_text(node, source).unwrap_or_default();
    let content = if node.kind() == "atx_heading" {
        raw.trim()
            .trim_start_matches('#')
            .trim()
            .trim_end_matches('#')
            .trim()
            .to_string()
    } else {
        raw.lines()
            .filter(|line| {
                let trimmed = line.trim();
                !(trimmed.chars().all(|ch| ch == '=') || trimmed.chars().all(|ch| ch == '-'))
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    display_text(&content)
}

fn display_text(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                if let Some(next) = chars.next() {
                    output.push(next);
                }
            }
            '[' => {
                let mut label = String::new();
                for next in chars.by_ref() {
                    if next == ']' {
                        break;
                    }
                    label.push(next);
                }
                output.push_str(&label);
                if chars.peek() == Some(&'(') {
                    let mut depth = 0usize;
                    for next in chars.by_ref() {
                        if next == '(' {
                            depth += 1;
                        } else if next == ')' {
                            if depth == 1 {
                                break;
                            }
                            depth = depth.saturating_sub(1);
                        }
                    }
                }
            }
            '*' | '_' | '`' | '~' => {}
            '<' => {
                let mut target = String::new();
                for next in chars.by_ref() {
                    if next == '>' {
                        break;
                    }
                    target.push(next);
                }
                output.push_str(&target);
            }
            _ => output.push(ch),
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn heading_kind(node: Node<'_>, source: &[u8]) -> &'static str {
    if node.kind() == "setext_heading" {
        let raw = node_text(node, source).unwrap_or_default();
        return if raw
            .lines()
            .last()
            .is_some_and(|line| line.trim().starts_with('='))
        {
            "heading1"
        } else {
            "heading2"
        };
    }
    let marker = (0..node.named_child_count() as u32)
        .filter_map(|index| node.named_child(index))
        .map(|child| child.kind())
        .find(|kind| kind.starts_with("atx_h"));
    match marker {
        Some("atx_h1_marker") => "heading1",
        Some("atx_h2_marker") => "heading2",
        Some("atx_h3_marker") => "heading3",
        Some("atx_h4_marker") => "heading4",
        Some("atx_h5_marker") => "heading5",
        Some("atx_h6_marker") => "heading6",
        _ => "heading1",
    }
}

fn named_descendant<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .find_map(|child| named_descendant(child, kind));
    found
}

fn node_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.utf8_text(source)
        .ok()
        .map(|text| text.trim().to_string())
}

fn trim_link_delimiters(text: &str) -> &str {
    text.trim()
        .trim_start_matches(['[', '<'])
        .trim_end_matches([']', '>'])
        .trim()
}

fn symbol(name: String, kind: String, node: Node<'_>) -> ExtractedSymbol {
    let start = node.start_position();
    let end = node.end_position();
    ExtractedSymbol {
        name,
        kind,
        range: CodeRange {
            start_line: start.row + 1,
            start_col: start.column + 1,
            end_line: end.row + 1,
            end_col: end.column + 1,
        },
        docstring: None,
        flags: SymbolFlags {
            has_todo: false,
            has_fixme: false,
            is_test: false,
            is_exported: false,
            is_deprecated: false,
        },
        owner: None,
    }
}
