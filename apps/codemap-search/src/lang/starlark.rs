//! Starlark and Bazel language spec.

use super::format_support::{
    clean, descendants, first_descendant, import, is_recoverable, reference, text,
};
use super::{LanguageSpec, NameDecision};
use crate::parser::{ImportEntry, ReferenceSite};
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
const QUERY_SOURCE: &str = include_str!("../../queries/starlark/symbols.scm");
fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_starlark::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile Starlark query")
    })
}
fn function_name(call: Node<'_>, source: &[u8]) -> Option<String> {
    call.child_by_field_name("function")
        .and_then(|node| first_descendant(node, "identifier"))
        .map(|node| text(node, source))
}
fn keyword<'tree>(call: Node<'tree>, expected: &str, source: &[u8]) -> Option<Node<'tree>> {
    let arguments = call.child_by_field_name("arguments")?;
    descendants(arguments, "keyword_argument")
        .into_iter()
        .find(|argument| {
            argument
                .child_by_field_name("name")
                .is_some_and(|name| text(name, source) == expected)
        })
}
pub(crate) struct StarlarkSpec;
impl LanguageSpec for StarlarkSpec {
    fn language_name(&self) -> &'static str {
        "starlark"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_starlark::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["bzl"]
    }
    fn exact_names(&self) -> &'static [&'static str] {
        &["BUILD", "BUILD.bazel"]
    }
    fn caller_scan_enabled(&self) -> bool {
        false
    }
    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }
    fn always_store_references(&self) -> bool {
        true
    }
    fn indexes_format_text(&self) -> bool {
        true
    }
    fn capture_is_valid(&self, capture: &str, node: Node<'_>, source: &[u8]) -> bool {
        if !is_recoverable(node) {
            return false;
        }
        match capture {
            "symbol.type" => keyword(node, "name", source).is_some(),
            "nav.import" => function_name(node, source).as_deref() == Some("load"),
            "local.reference" => ["deps", "srcs", "data", "tools", "runtime_deps", "exports"]
                .iter()
                .any(|name| keyword(node, name, source).is_some()),
            _ => true,
        }
    }
    fn refine_kind(&self, capture: &str, _node: Node<'_>, _kind: &'static str) -> &'static str {
        match capture {
            "symbol.fn" => "function",
            "symbol.variable" => "variable",
            _ => "rule",
        }
    }
    fn name_for_capture(
        &self,
        capture: &str,
        node: Node<'_>,
        _kind: &str,
        _ext: &str,
        source: &[u8],
        _meta: &Option<String>,
    ) -> Option<NameDecision> {
        let name = match capture {
            "symbol.fn" => node.child_by_field_name("name"),
            "symbol.variable" => node
                .child_by_field_name("left")
                .and_then(|left| first_descendant(left, "identifier")),
            _ => keyword(node, "name", source)
                .and_then(|argument| argument.child_by_field_name("value"))
                .and_then(|value| first_descendant(value, "string")),
        }?;
        Some(NameDecision::Name(clean(name, source)))
    }
    fn additional_symbol_names_for_capture(
        &self,
        capture: &str,
        node: Node<'_>,
        source: &[u8],
        primary_name: &str,
    ) -> Vec<String> {
        if capture != "symbol.variable" {
            return Vec::new();
        }
        node.child_by_field_name("left")
            .map(|left| {
                descendants(left, "identifier")
                    .into_iter()
                    .map(|identifier| clean(identifier, source))
                    .filter(|name| name != primary_name)
                    .collect()
            })
            .unwrap_or_default()
    }
    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        Some(
            node.child_by_field_name("arguments")
                .and_then(|arguments| first_descendant(arguments, "string"))
                .map(|path| import(path, source))
                .into_iter()
                .collect(),
        )
    }
    fn reference_sites_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ReferenceSite>> {
        let mut sites = Vec::new();
        for name in ["deps", "srcs", "data", "tools", "runtime_deps", "exports"] {
            if let Some(value) = keyword(node, name, source)
                .and_then(|argument| argument.child_by_field_name("value"))
            {
                sites.extend(
                    descendants(value, "string")
                        .into_iter()
                        .map(|dependency| reference(dependency, source)),
                );
            }
        }
        Some(sites)
    }
}
