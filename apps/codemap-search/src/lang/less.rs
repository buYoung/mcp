//! Less language spec.

use super::format_support::{is_recoverable, named_children, nearest_ancestor, text};
use super::{LanguageSpec, NameDecision};
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
const QUERY_SOURCE: &str = include_str!("../../queries/less/symbols.scm");
fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_less::language(), QUERY_SOURCE)
            .expect("Failed to compile Less query")
    })
}
pub(crate) struct LessSpec;
impl LanguageSpec for LessSpec {
    fn language_name(&self) -> &'static str {
        "less"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_less::language()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["less"]
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
        )) && (capture != "symbol.const"
            || matches!(text(node, source).as_bytes().first(), Some(b'@'))
            || text(node, source).starts_with("--"))
    }
    fn refine_kind(&self, capture: &str, _node: Node<'_>, _kind: &'static str) -> &'static str {
        match capture {
            "symbol.type" => "keyframes",
            "symbol.const" => "custom_property",
            "symbol.variable" => "variable",
            "symbol.fn" => "mixin",
            _ => "selector",
        }
    }
    fn symbol_kind_for_capture(
        &self,
        capture: &str,
        node: Node<'_>,
        source: &[u8],
        kind: &'static str,
    ) -> String {
        if capture == "symbol.const" && text(node, source).starts_with('@') {
            "variable".to_string()
        } else {
            self.refine_kind(capture, node, kind).to_string()
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
        let name = if capture == "symbol.fn" {
            named_children(node)
                .into_iter()
                .find(|child| matches!(child.kind(), "class_name" | "id_name"))
                .map(|child| {
                    let name = text(child, source);
                    match child.kind() {
                        "class_name" if !name.starts_with('.') => format!(".{name}"),
                        "id_name" if !name.starts_with('#') => format!("#{name}"),
                        _ => name,
                    }
                })
                .unwrap_or_default()
        } else {
            text(node, source)
        };
        Some(NameDecision::Name(name))
    }
}
