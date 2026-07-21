//! GraphQL language spec.

use super::format_support::{first_named, is_recoverable, named_children, reference, text};
use super::{LanguageSpec, NameDecision};
use crate::parser::ReferenceSite;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
const QUERY_SOURCE: &str = include_str!("../../queries/graphql/symbols.scm");
fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_graphql::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile GraphQL query")
    })
}
fn declaration_name(node: Node<'_>) -> Option<Node<'_>> {
    if matches!(node.kind(), "schema_definition" | "schema_extension") {
        return Some(node);
    }
    let name = named_children(node)
        .into_iter()
        .find(|child| matches!(child.kind(), "name" | "fragment_name"));
    if node.kind() == "operation_definition" {
        return name.or_else(|| {
            named_children(node)
                .into_iter()
                .find(|child| child.kind() == "operation_type")
        });
    }
    let name = name?;
    Some(if name.kind() == "fragment_name" {
        first_named(name).unwrap_or(name)
    } else {
        name
    })
}
pub(crate) struct GraphqlSpec;
impl LanguageSpec for GraphqlSpec {
    fn language_name(&self) -> &'static str {
        "graphql"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_graphql::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["graphql", "gql"]
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
            "object_type_definition" => "type",
            "interface_type_definition" => "interface",
            "input_object_type_definition" => "input",
            "enum_type_definition" => "enum",
            "scalar_type_definition" => "scalar",
            "union_type_definition" => "union",
            "schema_definition" => "schema",
            "directive_definition" => "directive",
            "fragment_definition" => "fragment",
            "operation_definition" => "operation",
            "field_definition" | "input_value_definition" => "field",
            "enum_value" => "variant",
            "object_type_extension" => "type_extension",
            "interface_type_extension" => "interface_extension",
            "input_object_type_extension" => "input_extension",
            "enum_type_extension" => "enum_extension",
            "scalar_type_extension" => "scalar_extension",
            "union_type_extension" => "union_extension",
            "schema_extension" => "schema_extension",
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
        if matches!(node.kind(), "schema_definition" | "schema_extension") {
            Some(NameDecision::Name("schema".into()))
        } else {
            declaration_name(node).map(|name| NameDecision::Name(text(name, source)))
        }
    }
    fn reference_sites_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ReferenceSite>> {
        let capture = first_named(node).unwrap_or(node);
        Some(vec![reference(capture, source)])
    }
}
