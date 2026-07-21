//! Dockerfile/Containerfile language spec.

use super::format_support::{clean, first_descendant, import, is_recoverable};
use super::{LanguageSpec, NameDecision};
use crate::parser::ImportEntry;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
const QUERY_SOURCE: &str = include_str!("../../queries/containerfile/symbols.scm");
fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_containerfile::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile Containerfile query")
    })
}
fn symbol_node(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "arg_pair" | "env_pair" => node.child_by_field_name("name"),
        "label_pair" => node.child_by_field_name("key"),
        "from_instruction" => node.child_by_field_name("as"),
        _ => None,
    }
}
pub(crate) struct ContainerfileSpec;
impl LanguageSpec for ContainerfileSpec {
    fn language_name(&self) -> &'static str {
        "dockerfile"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_containerfile::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }
    fn exact_names(&self) -> &'static [&'static str] {
        &["Dockerfile"]
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
        is_recoverable(node)
            && (capture != "symbol.type" || symbol_node(node).is_some())
            && (capture != "nav.import" || first_descendant(node, "image_spec").is_some())
    }
    fn refine_kind(&self, _capture: &str, node: Node<'_>, _kind: &'static str) -> &'static str {
        match node.kind() {
            "arg_pair" => "argument",
            "env_pair" => "environment",
            "label_pair" => "label",
            "from_instruction" => "stage",
            _ => "unknown",
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
        symbol_node(node).map(|name| NameDecision::Name(clean(name, source)))
    }
    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        Some(
            first_descendant(node, "image_spec")
                .map(|image| import(image, source))
                .into_iter()
                .collect(),
        )
    }
}
