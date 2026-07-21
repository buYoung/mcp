//! SCSS language spec.

use super::format_support::{is_recoverable, nearest_ancestor, text};
use super::{LanguageSpec, NameDecision};
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
const QUERY_SOURCE: &str = include_str!("../../queries/scss/symbols.scm");
fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_scss::language(), QUERY_SOURCE)
            .expect("Failed to compile SCSS query")
    })
}
pub(crate) struct ScssSpec;
impl LanguageSpec for ScssSpec {
    fn language_name(&self) -> &'static str {
        "scss"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_scss::language()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["scss"]
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
    fn refine_kind(&self, capture: &str, node: Node<'_>, _kind: &'static str) -> &'static str {
        match capture {
            "symbol.type" => "keyframes",
            "symbol.const" => "custom_property",
            "symbol.variable" => "variable",
            "symbol.fn"
                if node
                    .parent()
                    .is_some_and(|parent| parent.kind() == "mixin_statement") =>
            {
                "mixin"
            }
            "symbol.fn" => "function",
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
