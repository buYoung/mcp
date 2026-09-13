//! Zsh language spec.

use super::format_support::{
    clean, descendants, import, is_recoverable, named_children, nearest_ancestor, text,
};
use super::{LanguageSpec, NameDecision};
use crate::parser::ImportEntry;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
const QUERY_SOURCE: &str = include_str!("../../queries/zsh/symbols.scm");
fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&super::bundled_grammars::ZSH.into(), QUERY_SOURCE)
            .expect("Failed to compile Zsh query")
    })
}
fn static_source<'tree>(command: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let children = named_children(command);
    let name = children.iter().find(|node| node.kind() == "command_name")?;
    if !matches!(text(*name, source).as_str(), "source" | ".") {
        return None;
    }
    let path = children
        .iter()
        .find(|node| matches!(node.kind(), "word" | "string"))
        .copied()?;
    let is_dynamic = [
        "simple_expansion",
        "expansion",
        "command_substitution",
        "process_substitution",
    ]
    .iter()
    .any(|kind| !descendants(path, kind).is_empty());
    (!is_dynamic).then_some(path)
}
pub(crate) struct ZshSpec;
impl LanguageSpec for ZshSpec {
    fn language_name(&self) -> &'static str {
        "zsh"
    }
    fn grammar(&self, _ext: &str) -> Language {
        super::bundled_grammars::ZSH.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["zsh"]
    }
    fn caller_scan_enabled(&self) -> bool {
        false
    }
    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }
    fn always_store_references(&self, _ext: &str) -> bool {
        true
    }
    fn indexes_format_text(&self) -> bool {
        true
    }
    fn capture_is_valid(&self, capture: &str, node: Node<'_>, source: &[u8]) -> bool {
        if capture == "symbol.fn" {
            let Some(name) = node.child_by_field_name("name") else {
                return false;
            };
            let Some(body) = node.child_by_field_name("body") else {
                return false;
            };
            // An unsupported expression inside a complete body must not erase
            // its independently parsed declaration header. Error ancestors and
            // a missing body delimiter still make the boundary unreliable.
            let mut last = body;
            while let Some(child) = last.child(last.child_count().saturating_sub(1) as u32) {
                last = child;
            }
            return is_recoverable(name) && !last.is_missing() && !last.is_error();
        }
        is_recoverable(nearest_ancestor(
            node,
            &["function_definition", "variable_assignment", "command"],
        )) && (capture != "nav.import" || static_source(node, source).is_some())
    }
    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        Some(
            static_source(node, source)
                .map(|path| import(path, source))
                .into_iter()
                .collect(),
        )
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
        if capture == "symbol.fn" {
            // The query's name capture also distinguishes `function first second`.
            return None;
        }
        Some(NameDecision::Name(clean(node, source)))
    }
}
