//! C# language spec.

use std::collections::HashSet;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{generic_find_owner, path_indicates_test, LanguageSpec};
use crate::parser::{CodeRange, ImportEntry, ImportKind};

const QUERY_STR: &str = concat!(
    include_str!("../../queries/csharp/symbols.scm"),
    "\n",
    include_str!("../../queries/csharp/navigation.scm")
);
const TAGS_QUERY_STR: &str = include_str!("../../queries/csharp/tags.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_c_sharp::LANGUAGE.into(), QUERY_STR)
            .expect("Failed to compile C# query")
    })
}

fn tags_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_c_sharp::LANGUAGE.into(), TAGS_QUERY_STR)
            .expect("Failed to compile C# tags query")
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

fn simple_name(value: &str) -> Option<String> {
    value
        .trim()
        .trim_end_matches(';')
        .rsplit(['.', ':'])
        .find(|part| !part.is_empty())
        .map(str::to_string)
}

fn has_modifier(node: Node<'_>, modifier: &str, source: &[u8]) -> bool {
    (0..node.child_count()).any(|index| {
        let child = node.child(index as u32).expect("child index is in bounds");
        child.kind() == "modifier" && child.utf8_text(source).is_ok_and(|text| text == modifier)
    })
}

fn is_interface_member(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == "interface_declaration" {
            return true;
        }
        if matches!(
            parent.kind(),
            "class_declaration" | "record_declaration" | "struct_declaration"
        ) {
            return false;
        }
        node = parent;
    }
    false
}

fn has_attribute(node: Node<'_>, target: &str, source: &[u8]) -> bool {
    (0..node.child_count()).any(|index| {
        let child = node.child(index as u32).expect("child index is in bounds");
        child.kind() == "attribute_list"
            && child.utf8_text(source).is_ok_and(|text| {
                text.split(['[', ']', ',', '(', ')'])
                    .map(str::trim)
                    .any(|part| {
                        let name = part.rsplit('.').next().unwrap_or(part);
                        name == target || name.strip_suffix("Attribute") == Some(target)
                    })
            })
    })
}

pub(crate) struct CsharpSpec;

impl LanguageSpec for CsharpSpec {
    fn language_name(&self) -> &'static str {
        "csharp"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["cs", "c#", "c-sharp"]
    }

    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_c_sharp::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(tags_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cs"]
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
        let body = text
            .strip_prefix("global ")
            .unwrap_or(text)
            .strip_prefix("using ")?
            .trim_end_matches(';')
            .trim();
        let range = range_for_node(node);
        if let Some(target) = body.strip_prefix("static ") {
            let target = target.trim();
            let name = simple_name(target)?;
            return Some(vec![ImportEntry {
                local_name: name.clone(),
                imported_name: Some(name),
                source: Some(target.to_string()),
                kind: ImportKind::Named,
                range,
            }]);
        }
        if let Some((alias, target)) = body.split_once('=') {
            let alias = alias.trim();
            let target = target.trim();
            if alias.is_empty() || target.is_empty() {
                return Some(Vec::new());
            }
            return Some(vec![ImportEntry {
                local_name: alias.to_string(),
                imported_name: simple_name(target),
                source: Some(target.to_string()),
                kind: ImportKind::Named,
                range,
            }]);
        }
        let name = simple_name(body)?;
        Some(vec![ImportEntry {
            local_name: name,
            imported_name: None,
            source: Some(body.to_string()),
            kind: ImportKind::Namespace,
            range,
        }])
    }

    fn is_import_line(&self, line: &str) -> bool {
        let line = line.trim_start();
        line.starts_with("using ") || line.starts_with("global using ")
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
            || has_attribute(node, "Test", source)
            || has_attribute(node, "TestMethod", source)
            || has_attribute(node, "Fact", source)
            || has_attribute(node, "Theory", source)
    }

    fn is_exported(
        &self,
        node: Node<'_>,
        _name: &str,
        _kind: &str,
        source: &[u8],
        _exported_names: &HashSet<String>,
    ) -> bool {
        has_modifier(node, "public", source) || is_interface_member(node)
    }

    fn is_deprecated(
        &self,
        node: Node<'_>,
        source: &[u8],
        docstring: &Option<String>,
        _comments_text: &str,
    ) -> bool {
        has_attribute(node, "Obsolete", source)
            || docstring
                .as_ref()
                .is_some_and(|docstring| docstring.contains("@deprecated"))
    }

    fn find_owner(&self, node: Node<'_>, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "method_declaration",
            "constructor_declaration",
            "local_function_statement",
            "anonymous_method_expression",
            "lambda_expression",
        ]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "class_declaration",
            "record_declaration",
            "struct_declaration",
            "interface_declaration",
            "enum_declaration",
        ]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "declaration_list",
            "enum_member_declaration_list",
            "namespace_declaration",
            "file_scoped_namespace_declaration",
        ]
    }
}
