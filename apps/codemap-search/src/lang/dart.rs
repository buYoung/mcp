use std::collections::HashSet;
use std::sync::OnceLock;

use tree_sitter::{Language, Node, Query};

use super::{generic_find_owner, has_annotation, path_indicates_test, LanguageSpec};
use crate::parser::{CodeRange, ImportEntry, ImportKind};

const QUERY_STR: &str = concat!(
    include_str!("../../queries/dart/symbols.scm"),
    "\n",
    include_str!("../../queries/dart/navigation.scm")
);
const TAGS_QUERY_STR: &str = include_str!("../../queries/dart/tags.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_dart::LANGUAGE.into(), QUERY_STR)
            .expect("Failed to compile Dart query")
    })
}

fn tags_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_dart::LANGUAGE.into(), TAGS_QUERY_STR)
            .expect("Failed to compile Dart tags query")
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

fn literal_uri(text: &str) -> Option<String> {
    let start = text.find(['\'', '"'])?;
    let quote = text.as_bytes()[start] as char;
    let tail = &text[start + 1..];
    let end = tail.find(quote)?;
    let value = &tail[..end];
    (!value.contains('$')).then(|| value.to_string())
}

fn declaration_or_previous_line_contains(node: Node<'_>, source: &[u8], needle: &str) -> bool {
    if node
        .utf8_text(source)
        .is_ok_and(|text| text.contains(needle))
    {
        return true;
    }
    let prefix = std::str::from_utf8(&source[..node.start_byte()]).unwrap_or_default();
    prefix
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| line.contains(needle))
}

pub(crate) struct DartSpec;

impl LanguageSpec for DartSpec {
    fn language_name(&self) -> &'static str {
        "dart"
    }

    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_dart::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(tags_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["dart"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        let text = node.utf8_text(source).ok()?;
        let Some(uri) = literal_uri(text) else {
            return Some(Vec::new());
        };
        let local_name = uri
            .rsplit('/')
            .next()
            .unwrap_or(&uri)
            .trim_end_matches(".dart")
            .to_string();
        Some(vec![ImportEntry {
            local_name: local_name.clone(),
            imported_name: Some(local_name),
            source: Some(uri),
            kind: ImportKind::Named,
            range: range_for_node(node),
        }])
    }

    fn is_import_line(&self, line: &str) -> bool {
        let line = line.trim_start();
        line.starts_with("import ")
            || line.starts_with("export ")
            || line.starts_with("part ")
            || line.starts_with("part of ")
    }

    fn is_test(
        &self,
        node: Node<'_>,
        _name: &str,
        _kind: &str,
        file_path: &str,
        source: &[u8],
        _comments_text: &str,
    ) -> bool {
        path_indicates_test(file_path)
            || file_path.ends_with("_test.dart")
            || has_annotation(node, "Test", source)
            || declaration_or_previous_line_contains(node, source, "@Test")
    }

    fn is_exported(
        &self,
        _node: Node<'_>,
        name: &str,
        _kind: &str,
        _source: &[u8],
        _exported_names: &HashSet<String>,
    ) -> bool {
        !name.starts_with('_')
    }

    fn is_deprecated(
        &self,
        node: Node<'_>,
        source: &[u8],
        docstring: &Option<String>,
        _comments_text: &str,
    ) -> bool {
        has_annotation(node, "Deprecated", source)
            || declaration_or_previous_line_contains(node, source, "@Deprecated")
            || docstring
                .as_ref()
                .is_some_and(|docstring| docstring.contains("@deprecated"))
    }

    fn find_owner(&self, node: Node<'_>, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "function_declaration",
            "method_declaration",
            "local_function_declaration",
            "function_expression",
        ]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "class_declaration",
            "mixin_declaration",
            "extension_declaration",
            "extension_type_declaration",
            "enum_declaration",
        ]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "method_declaration",
            "class_member",
            "class_body",
            "extension_body",
            "enum_body",
        ]
    }
}
