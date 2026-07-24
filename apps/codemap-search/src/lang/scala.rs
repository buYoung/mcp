use std::collections::HashSet;
use std::sync::OnceLock;

use tree_sitter::{Language, Node, Query};

use super::{generic_find_owner, has_annotation, path_indicates_test, LanguageSpec, NameDecision};
use crate::parser::{CodeRange, ImportEntry, ImportKind};

const QUERY_STR: &str = concat!(
    include_str!("../../queries/scala/symbols.scm"),
    "\n",
    include_str!("../../queries/scala/navigation.scm")
);
const TAGS_QUERY_STR: &str = include_str!("../../queries/scala/tags.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_scala::LANGUAGE.into(), QUERY_STR)
            .expect("Failed to compile Scala query")
    })
}

fn tags_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_scala::LANGUAGE.into(), TAGS_QUERY_STR)
            .expect("Failed to compile Scala tags query")
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

fn imported_path(text: &str) -> Option<String> {
    let body = text
        .trim()
        .strip_prefix("import ")
        .or_else(|| text.trim().strip_prefix("export "))?
        .trim();
    if body.is_empty() || body.contains('$') {
        return None;
    }
    Some(
        body.split(['{', '*'])
            .next()
            .unwrap_or(body)
            .trim()
            .trim_end_matches('.')
            .to_string(),
    )
}

pub(crate) struct ScalaSpec;

impl LanguageSpec for ScalaSpec {
    fn language_name(&self) -> &'static str {
        "scala"
    }

    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_scala::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(tags_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["scala", "sc"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn name_for_capture(
        &self,
        capture_name: &str,
        _node: Node<'_>,
        _kind: &str,
        _ext: &str,
        _source: &[u8],
        _asm_meta_kind_text: &Option<String>,
    ) -> Option<NameDecision> {
        (capture_name == "symbol.scala_extension")
            .then(|| NameDecision::Name("extension".to_string()))
    }

    fn symbol_kind_for_capture(
        &self,
        capture_name: &str,
        _node: Node<'_>,
        _source: &[u8],
        default_kind: &'static str,
    ) -> String {
        if capture_name == "symbol.scala_extension" {
            "type".to_string()
        } else {
            default_kind.to_string()
        }
    }

    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        let text = node.utf8_text(source).ok()?;
        let Some(path) = imported_path(text) else {
            return Some(Vec::new());
        };
        let local_name = path.rsplit('.').next().unwrap_or(&path).to_string();
        Some(vec![ImportEntry {
            local_name,
            imported_name: None,
            source: Some(path),
            kind: ImportKind::Namespace,
            range: range_for_node(node),
        }])
    }

    fn is_import_line(&self, line: &str) -> bool {
        let line = line.trim_start();
        line.starts_with("import ") || line.starts_with("export ")
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
            || file_path.contains("/test/")
            || ["Test", "Suite", "RunWith"]
                .iter()
                .any(|annotation| has_annotation(node, annotation, source))
    }

    fn is_exported(
        &self,
        node: Node<'_>,
        _name: &str,
        _kind: &str,
        source: &[u8],
        _exported_names: &HashSet<String>,
    ) -> bool {
        let text = node.utf8_text(source).unwrap_or_default();
        !text
            .split(|character: char| character.is_whitespace() || character == '[')
            .any(|token| matches!(token, "private" | "protected"))
    }

    fn is_deprecated(
        &self,
        node: Node<'_>,
        source: &[u8],
        docstring: &Option<String>,
        _comments_text: &str,
    ) -> bool {
        has_annotation(node, "deprecated", source)
            || node
                .utf8_text(source)
                .is_ok_and(|text| text.contains("@deprecated"))
            || docstring
                .as_ref()
                .is_some_and(|docstring| docstring.contains("@deprecated"))
    }

    fn find_owner(&self, node: Node<'_>, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "function_definition",
            "function_declaration",
            "lambda_expression",
        ]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "class_definition",
            "trait_definition",
            "object_definition",
            "enum_definition",
            "extension_definition",
        ]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["template_body", "enum_body"]
    }
}
