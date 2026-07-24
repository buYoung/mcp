use std::collections::HashSet;
use std::sync::OnceLock;

use tree_sitter::{Language, Node, Query};

use super::{generic_find_owner, has_annotation, path_indicates_test, LanguageSpec};
use crate::parser::{CodeRange, ImportEntry, ImportKind};

const QUERY_STR: &str = concat!(
    include_str!("../../queries/swift/symbols.scm"),
    "\n",
    include_str!("../../queries/swift/navigation.scm")
);
const TAGS_QUERY_STR: &str = include_str!("../../queries/swift/tags.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_swift::LANGUAGE.into(), QUERY_STR)
            .expect("Failed to compile Swift query")
    })
}

fn tags_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_swift::LANGUAGE.into(), TAGS_QUERY_STR)
            .expect("Failed to compile Swift tags query")
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

fn declaration_text<'a>(node: Node<'_>, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or_default()
}

pub(crate) struct SwiftSpec;

impl LanguageSpec for SwiftSpec {
    fn language_name(&self) -> &'static str {
        "swift"
    }

    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_swift::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(tags_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["swift"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn refine_kind(&self, capture_name: &str, node: Node<'_>, kind: &'static str) -> &'static str {
        if capture_name != "symbol.swift_type" {
            return kind;
        }
        match node
            .child_by_field_name("declaration_kind")
            .map(|child| child.kind())
        {
            Some("struct") => "struct",
            Some("enum") => "enum",
            Some("actor") => "class",
            Some("extension") => "type",
            _ => "class",
        }
    }

    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        let text = declaration_text(node, source).trim();
        let body = text.strip_prefix("import ")?.trim();
        let body = [
            "typealias ",
            "struct ",
            "class ",
            "enum ",
            "protocol ",
            "let ",
            "var ",
            "func ",
        ]
        .iter()
        .find_map(|prefix| body.strip_prefix(prefix))
        .unwrap_or(body);
        if body.is_empty() {
            return Some(Vec::new());
        }
        let module = body.split('.').next()?.trim();
        Some(vec![ImportEntry {
            local_name: module.to_string(),
            imported_name: None,
            source: Some(body.to_string()),
            kind: ImportKind::Namespace,
            range: range_for_node(node),
        }])
    }

    fn is_import_line(&self, line: &str) -> bool {
        line.trim_start().starts_with("import ")
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
            || file_path.contains("Tests/")
            || name.starts_with("test")
            || has_annotation(node, "Test", source)
    }

    fn is_exported(
        &self,
        node: Node<'_>,
        _name: &str,
        _kind: &str,
        source: &[u8],
        _exported_names: &HashSet<String>,
    ) -> bool {
        let text = declaration_text(node, source);
        text.split(|character: char| character.is_whitespace() || character == '(')
            .any(|token| matches!(token, "public" | "open"))
    }

    fn is_deprecated(
        &self,
        node: Node<'_>,
        source: &[u8],
        docstring: &Option<String>,
        _comments_text: &str,
    ) -> bool {
        let text = declaration_text(node, source).to_ascii_lowercase();
        (text.contains("@available") && text.contains("deprecated"))
            || docstring
                .as_ref()
                .is_some_and(|docstring| docstring.contains("@deprecated"))
    }

    fn find_owner(&self, node: Node<'_>, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["function_declaration", "lambda_literal"]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["class_declaration", "protocol_declaration"]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["class_body", "enum_class_body", "protocol_body"]
    }
}
