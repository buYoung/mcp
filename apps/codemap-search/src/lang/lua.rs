//! Lua language spec.

use std::collections::HashSet;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{path_indicates_test, LanguageSpec, NameDecision};
use crate::parser::{CodeRange, ImportEntry, ImportKind};

const QUERY_STR: &str = concat!(
    include_str!("../../queries/lua/symbols.scm"),
    "\n",
    include_str!("../../queries/lua/navigation.scm")
);
const TAGS_QUERY_STR: &str = include_str!("../../queries/lua/tags.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_lua::LANGUAGE.into(), QUERY_STR)
            .expect("Failed to compile Lua query")
    })
}

fn tags_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_lua::LANGUAGE.into(), TAGS_QUERY_STR)
            .expect("Failed to compile Lua tags query")
    })
}

fn range_for_node(node: Node<'_>) -> CodeRange {
    let start = node.start_position();
    let end = node.end_position();
    CodeRange {
        start_line: start.row + 1,
        start_col: start.column + 1,
        end_line: end.row + 1,
        end_col: end.column + 1,
    }
}

fn literal_string(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).ok()?.trim();
    if text.starts_with("[[") && text.ends_with("]]") {
        return Some(text[2..text.len() - 2].to_string());
    }
    let quote = text.chars().next()?;
    if !matches!(quote, '\'' | '"') || !text.ends_with(quote) || text.len() < 2 {
        return None;
    }
    Some(text[1..text.len() - 1].to_string())
}

fn contains_function_ancestor(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "function_declaration" | "function_definition"
        ) {
            return true;
        }
        node = parent;
    }
    false
}

fn contains_descendant_kind(node: Node<'_>, target: &str) -> bool {
    (0..node.named_child_count())
        .filter_map(|index| node.named_child(index as u32))
        .any(|child| child.kind() == target || contains_descendant_kind(child, target))
}

fn receiver_from_function_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() == "field" {
        let mut current = node.parent();
        while let Some(parent) = current {
            if parent.kind() == "assignment_statement" {
                let variable_list = parent.named_child(0)?;
                let name = variable_list.named_child(0)?;
                return name.utf8_text(source).ok().map(str::to_string);
            }
            current = parent.parent();
        }
        return None;
    }
    let name = if node.kind() == "function_definition" {
        assigned_function_target(node)?
    } else {
        node.child_by_field_name("name")?
    };
    if !matches!(
        name.kind(),
        "dot_index_expression" | "method_index_expression"
    ) {
        return None;
    }
    let receiver = name
        .child_by_field_name("table")
        .or_else(|| name.child_by_field_name("prefix"))
        .or_else(|| name.child_by_field_name("object"))?;
    receiver.utf8_text(source).ok().map(str::to_string)
}

fn assigned_function_target(node: Node<'_>) -> Option<Node<'_>> {
    let expressions = node
        .parent()
        .filter(|parent| parent.kind() == "expression_list")?;
    let assignment = expressions
        .parent()
        .filter(|parent| parent.kind() == "assignment_statement")?;
    let mut cursor = expressions.walk();
    let position = expressions
        .named_children(&mut cursor)
        .filter(|child| child.kind() != "comment")
        .position(|child| child == node)?;
    let variables = assignment.named_child(0)?;
    let mut cursor = variables.walk();
    let target = variables
        .named_children(&mut cursor)
        .filter(|child| child.kind() != "comment")
        .nth(position);
    target
}

fn preceding_comment_contains(node: Node<'_>, source: &[u8], marker: &str) -> bool {
    let Ok(source) = std::str::from_utf8(source) else {
        return false;
    };
    source
        .lines()
        .collect::<Vec<_>>()
        .into_iter()
        .take(node.start_position().row)
        .rev()
        .take_while(|line| {
            let line = line.trim();
            line.is_empty() || line.starts_with("--")
        })
        .any(|line| line.to_ascii_lowercase().contains(marker))
}

pub(crate) struct LuaSpec;

impl LanguageSpec for LuaSpec {
    fn language_name(&self) -> &'static str {
        "lua"
    }

    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_lua::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(tags_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["lua"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn capture_is_valid(&self, capture_name: &str, node: Node<'_>, _source: &[u8]) -> bool {
        if node.has_error() {
            return false;
        }
        if capture_name == "symbol.variable"
            && contains_descendant_kind(node, "function_definition")
        {
            return false;
        }
        if matches!(capture_name, "symbol.variable" | "symbol.field") {
            return !contains_function_ancestor(node);
        }
        true
    }

    fn name_for_capture(
        &self,
        capture_name: &str,
        node: Node<'_>,
        _kind: &str,
        _ext: &str,
        source: &[u8],
        _asm_meta_kind_text: &Option<String>,
    ) -> Option<NameDecision> {
        if capture_name != "symbol.fn" || node.kind() != "function_definition" {
            return None;
        }
        let name = assigned_function_target(node).and_then(|target| {
            let name = match target.kind() {
                "identifier" => target,
                "dot_index_expression" => target.child_by_field_name("field")?,
                _ => return None,
            };
            name.utf8_text(source).ok().map(str::to_string)
        });
        Some(name.map(NameDecision::Name).unwrap_or(NameDecision::Skip))
    }

    fn symbol_kind_for_capture(
        &self,
        capture_name: &str,
        node: Node<'_>,
        _source: &[u8],
        default_kind: &'static str,
    ) -> String {
        if capture_name == "symbol.field" && contains_descendant_kind(node, "function_definition") {
            return "fn".to_string();
        }
        default_kind.to_string()
    }

    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        let name = node.child_by_field_name("name")?.utf8_text(source).ok()?;
        let arguments = node.child_by_field_name("arguments")?;
        let string_node = (0..arguments.named_child_count())
            .filter_map(|index| arguments.named_child(index as u32))
            .find(|child| child.kind() == "string")?;
        let Some(source_path) = literal_string(string_node, source) else {
            return Some(Vec::new());
        };
        let tail = source_path
            .rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or(&source_path);
        let local_name = tail
            .rsplit_once('.')
            .map(
                |(stem, extension)| {
                    if extension == "lua" {
                        stem
                    } else {
                        extension
                    }
                },
            )
            .unwrap_or(tail)
            .to_string();
        let source_path = if name == "dofile"
            && !source_path.starts_with("./")
            && !source_path.starts_with("../")
        {
            format!("./{source_path}")
        } else {
            source_path
        };
        Some(vec![ImportEntry {
            local_name: local_name.clone(),
            imported_name: Some(local_name),
            source: Some(source_path),
            kind: ImportKind::Named,
            range: range_for_node(node),
        }])
    }

    fn is_import_line(&self, line: &str) -> bool {
        let line = line.trim_start();
        line.starts_with("require(")
            || line.starts_with("require ")
            || line.starts_with("dofile(")
            || line.starts_with("dofile ")
            || line.contains("require(")
            || line.contains("dofile(")
    }

    fn is_test(
        &self,
        _node: Node<'_>,
        name: &str,
        _kind: &str,
        file_path: &str,
        _source: &[u8],
        _comments_text: &str,
    ) -> bool {
        path_indicates_test(file_path)
            || file_path.ends_with("_spec.lua")
            || file_path.ends_with("_test.lua")
            || name.starts_with("test_")
    }

    fn is_exported(
        &self,
        node: Node<'_>,
        _name: &str,
        _kind: &str,
        _source: &[u8],
        _exported_names: &HashSet<String>,
    ) -> bool {
        !contains_function_ancestor(node)
    }

    fn is_deprecated(
        &self,
        node: Node<'_>,
        source: &[u8],
        docstring: &Option<String>,
        comments_text: &str,
    ) -> bool {
        preceding_comment_contains(node, source, "@deprecated")
            || comments_text.to_ascii_lowercase().contains("@deprecated")
            || docstring
                .as_ref()
                .is_some_and(|docstring| docstring.to_ascii_lowercase().contains("@deprecated"))
    }

    fn find_owner(&self, node: Node<'_>, _ext: &str, source: &[u8]) -> Option<String> {
        receiver_from_function_name(node, source)
    }
}
