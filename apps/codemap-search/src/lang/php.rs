//! PHP language spec using the mixed HTML/PHP grammar.

use std::collections::HashSet;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{generic_find_owner, path_indicates_test, LanguageSpec};
use crate::parser::{CodeRange, ImportEntry, ImportKind};

const QUERY_STR: &str = concat!(
    include_str!("../../queries/php/symbols.scm"),
    "\n",
    include_str!("../../queries/php/navigation.scm")
);
const TAGS_QUERY_STR: &str = include_str!("../../queries/php/tags.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_php::LANGUAGE_PHP.into(), QUERY_STR)
            .expect("Failed to compile PHP query")
    })
}

fn tags_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_php::LANGUAGE_PHP.into(), TAGS_QUERY_STR)
            .expect("Failed to compile PHP tags query")
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

fn unquote_literal(text: &str) -> Option<String> {
    let text = text.trim();
    let quote = text.chars().next()?;
    if !matches!(quote, '\'' | '"') || !text.ends_with(quote) || text.len() < 2 {
        return None;
    }
    let value = &text[1..text.len() - 1];
    (!value.contains("${") && !value.contains("{$")).then(|| value.to_string())
}

fn simple_name(path: &str) -> Option<String> {
    path.trim()
        .trim_matches('\\')
        .rsplit('\\')
        .find(|part| !part.is_empty())
        .map(str::to_string)
}

fn namespace_use_entries(text: &str, range: CodeRange) -> Vec<ImportEntry> {
    let body = text
        .trim()
        .strip_prefix("use ")
        .unwrap_or("")
        .trim_end_matches(';')
        .trim();
    if body.is_empty() {
        return Vec::new();
    }
    let body = body
        .strip_prefix("function ")
        .or_else(|| body.strip_prefix("const "))
        .unwrap_or(body);
    let (prefix, clauses) = match body.split_once('{') {
        Some((prefix, grouped)) => (
            prefix.trim_end_matches('\\'),
            grouped.trim_end_matches('}').trim(),
        ),
        None => ("", body),
    };
    clauses
        .split(',')
        .filter_map(|clause| {
            let clause = clause.trim();
            let clause = clause
                .strip_prefix("function ")
                .or_else(|| clause.strip_prefix("const "))
                .unwrap_or(clause);
            let (path, alias) = clause
                .rsplit_once(" as ")
                .map(|(path, alias)| (path.trim(), Some(alias.trim())))
                .unwrap_or((clause, None));
            let full_path = if prefix.is_empty() {
                path.to_string()
            } else {
                format!("{prefix}\\{path}")
            };
            let imported_name = simple_name(&full_path)?;
            let local_name = alias
                .filter(|alias| !alias.is_empty())
                .unwrap_or(&imported_name)
                .to_string();
            Some(ImportEntry {
                local_name,
                imported_name: Some(imported_name),
                source: Some(full_path),
                kind: ImportKind::Named,
                range: range.clone(),
            })
        })
        .collect()
}

fn has_visibility(node: Node<'_>, visibility: &str, source: &[u8]) -> bool {
    node.utf8_text(source)
        .is_ok_and(|text| text.split_whitespace().any(|part| part == visibility))
}

fn is_top_level(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "class_declaration"
                | "interface_declaration"
                | "trait_declaration"
                | "enum_declaration"
        ) {
            return false;
        }
        node = parent;
    }
    true
}

pub(crate) struct PhpSpec;

impl LanguageSpec for PhpSpec {
    fn language_name(&self) -> &'static str {
        "php"
    }

    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_php::LANGUAGE_PHP.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(tags_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["php"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn capture_is_valid(&self, _capture_name: &str, node: Node<'_>, _source: &[u8]) -> bool {
        !node.has_error()
    }

    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        let text = node.utf8_text(source).ok()?.trim();
        let range = range_for_node(node);
        if text.starts_with("use ") {
            return Some(namespace_use_entries(text, range));
        }
        let literal = text.find(['\'', '"']).and_then(|start| {
            let quoted = &text[start..];
            let quote = quoted.chars().next()?;
            let end = quoted[1..].find(quote)? + 2;
            unquote_literal(&quoted[..end])
        });
        let Some(source_path) = literal else {
            return Some(Vec::new());
        };
        let local_name = source_path
            .rsplit(['/', '\\'])
            .next()
            .and_then(|file| file.rsplit_once('.').map(|(stem, _)| stem).or(Some(file)))
            .unwrap_or(&source_path)
            .to_string();
        Some(vec![ImportEntry {
            local_name: local_name.clone(),
            imported_name: Some(local_name),
            source: Some(source_path),
            kind: ImportKind::Named,
            range,
        }])
    }

    fn is_import_line(&self, line: &str) -> bool {
        let line = line.trim_start();
        line.starts_with("use ") || line.starts_with("include") || line.starts_with("require")
    }

    fn is_test(
        &self,
        node: Node<'_>,
        name: &str,
        _kind: &str,
        file_path: &str,
        source: &[u8],
        _comments_text: &str,
    ) -> bool {
        path_indicates_test(file_path)
            || name.starts_with("test")
            || node
                .utf8_text(source)
                .is_ok_and(|text| text.contains("#[Test") || text.contains("@test"))
    }

    fn is_exported(
        &self,
        node: Node<'_>,
        _name: &str,
        _kind: &str,
        source: &[u8],
        _exported_names: &HashSet<String>,
    ) -> bool {
        is_top_level(node)
            || (!has_visibility(node, "private", source)
                && !has_visibility(node, "protected", source))
    }

    fn is_deprecated(
        &self,
        node: Node<'_>,
        source: &[u8],
        docstring: &Option<String>,
        comments_text: &str,
    ) -> bool {
        node.utf8_text(source)
            .is_ok_and(|text| text.contains("#[Deprecated"))
            || comments_text.to_ascii_lowercase().contains("@deprecated")
            || docstring
                .as_ref()
                .is_some_and(|docstring| docstring.to_ascii_lowercase().contains("@deprecated"))
    }

    fn find_owner(&self, node: Node<'_>, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "function_definition",
            "method_declaration",
            "anonymous_function_creation_expression",
            "arrow_function",
            "anonymous_class",
        ]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "class_declaration",
            "interface_declaration",
            "trait_declaration",
            "enum_declaration",
        ]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["declaration_list", "enum_declaration_list"]
    }
}
