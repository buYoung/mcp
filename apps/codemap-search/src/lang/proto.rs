//! Protocol Buffers language spec.

use super::format_support::{import, is_recoverable, named_children, reference, text};
use super::{LanguageSpec, NameDecision};
use crate::parser::{ImportEntry, ReferenceSite};
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
const QUERY_SOURCE: &str = include_str!("../../queries/proto/symbols.scm");
fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_proto::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile Proto query")
    })
}
fn declaration_name(node: Node<'_>) -> Option<Node<'_>> {
    let expected = match node.kind() {
        "message" => Some("message_name"),
        "enum" => Some("enum_name"),
        "service" => Some("service_name"),
        "rpc" => Some("rpc_name"),
        "package" | "extend" => Some("full_ident"),
        "oneof" => Some("identifier"),
        "field" | "map_field" | "oneof_field" | "enum_field" => None,
        _ => return None,
    };
    let children = named_children(node);
    match expected {
        Some(kind) => children.into_iter().find(|child| child.kind() == kind),
        None => children
            .into_iter()
            .rfind(|child| child.kind() == "identifier"),
    }
}
pub(crate) struct ProtoSpec;
impl LanguageSpec for ProtoSpec {
    fn language_name(&self) -> &'static str {
        "proto"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_proto::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["proto"]
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
        is_recoverable(node) && (capture != "symbol.type" || declaration_name(node).is_some())
    }
    fn refine_kind(&self, _capture: &str, node: Node<'_>, _kind: &'static str) -> &'static str {
        match node.kind() {
            "message" => "message",
            "enum" => "enum",
            "service" => "service",
            "rpc" => "rpc",
            "package" => "package",
            "extend" => "extend",
            "oneof" => "oneof",
            "enum_field" => "variant",
            "field" | "map_field" | "oneof_field" => "field",
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
        declaration_name(node).map(|name| NameDecision::Name(text(name, source)))
    }
    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        Some(
            named_children(node)
                .into_iter()
                .find(|child| child.kind() == "string")
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
        Some(vec![reference(node, source)])
    }
}
