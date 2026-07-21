//! Make language spec.

use super::format_support::{clean, is_recoverable, named_children, reference};
use super::{LanguageSpec, NameDecision};
use crate::parser::ReferenceSite;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
const QUERY_SOURCE: &str = include_str!("../../queries/make/symbols.scm");
fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_make::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile Make query")
    })
}
fn first_word(node: Node<'_>) -> Option<Node<'_>> {
    named_children(node)
        .into_iter()
        .find(|child| matches!(child.kind(), "word" | "string"))
}
fn words(node: Node<'_>, source: &[u8]) -> Vec<String> {
    named_children(node)
        .into_iter()
        .filter(|child| matches!(child.kind(), "word" | "string"))
        .map(|child| clean(child, source))
        .collect()
}
pub(crate) struct MakeSpec;
impl LanguageSpec for MakeSpec {
    fn language_name(&self) -> &'static str {
        "make"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_make::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["mk"]
    }
    fn exact_names(&self) -> &'static [&'static str] {
        &["Makefile"]
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
    fn capture_is_valid(&self, capture: &str, node: Node<'_>, _source: &[u8]) -> bool {
        is_recoverable(node) && (capture != "symbol.type" || first_word(node).is_some())
    }
    fn refine_kind(&self, capture: &str, _node: Node<'_>, _kind: &'static str) -> &'static str {
        if capture == "symbol.type" {
            "target"
        } else {
            "variable"
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
        let name = if capture == "symbol.type" {
            first_word(node).map(|name| clean(name, source))
        } else {
            node.child_by_field_name("name")
                .map(|name| clean(name, source))
                .or_else(|| match node.kind() {
                    "VPATH_assignment" => Some("VPATH".to_string()),
                    "RECIPEPREFIX_assignment" => Some(".RECIPEPREFIX".to_string()),
                    _ => None,
                })
        }?;
        Some(NameDecision::Name(name))
    }
    fn additional_symbol_names_for_capture(
        &self,
        capture: &str,
        node: Node<'_>,
        source: &[u8],
        _primary_name: &str,
    ) -> Vec<String> {
        if capture == "symbol.type" {
            words(node, source).into_iter().skip(1).collect()
        } else {
            Vec::new()
        }
    }
    fn reference_sites_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ReferenceSite>> {
        Some(
            named_children(node)
                .into_iter()
                .filter(|child| matches!(child.kind(), "word" | "string"))
                .map(|child| reference(child, source))
                .collect(),
        )
    }
}
