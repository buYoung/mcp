//! CSS language spec.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::format_support::{is_recoverable, nearest_ancestor, text};
use super::{LanguageSpec, NameDecision};

const QUERY_SOURCE: &str = include_str!("../../queries/css/symbols.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_css::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile CSS query")
    })
}

pub(crate) struct CssSpec;
impl LanguageSpec for CssSpec {
    fn language_name(&self) -> &'static str {
        "css"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_css::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["css"]
    }
    fn caller_scan_enabled(&self) -> bool {
        false
    }
    fn indexes_format_text(&self) -> bool {
        true
    }
    fn capture_is_valid(&self, capture: &str, node: Node<'_>, source: &[u8]) -> bool {
        is_recoverable(nearest_ancestor(
            node,
            &[
                "rule_set",
                "keyframes_statement",
                "declaration",
                "variable_assignment",
            ],
        )) && (capture != "symbol.const" || text(node, source).starts_with("--"))
    }
    fn refine_kind(&self, capture: &str, _node: Node<'_>, _kind: &'static str) -> &'static str {
        match capture {
            "symbol.type" => "keyframes",
            "symbol.const" => "custom_property",
            "symbol.variable" => "variable",
            _ => "selector",
        }
    }
    fn name_for_capture(
        &self,
        _capture: &str,
        node: Node<'_>,
        _kind: &str,
        _ext: &str,
        source: &[u8],
        _meta: &Option<String>,
    ) -> Option<NameDecision> {
        Some(NameDecision::Name(text(node, source)))
    }
}
