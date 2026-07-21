//! HCL and Terraform language spec.

use super::format_support::{
    clean, descendants, import, is_recoverable, named_children, nearest_ancestor, reference, text,
};
use super::{LanguageSpec, NameDecision};
use crate::parser::{ImportEntry, ReferenceSite};
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
const QUERY_SOURCE: &str = include_str!("../../queries/hcl/symbols.scm");
fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_hcl::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile HCL query")
    })
}
fn block_parts<'tree>(block: Node<'tree>, source: &[u8]) -> (String, Vec<(Node<'tree>, String)>) {
    let labels: Vec<_> = named_children(block)
        .into_iter()
        .filter(|node| matches!(node.kind(), "identifier" | "string_lit"))
        .collect();
    let kind = labels
        .first()
        .map(|node| text(*node, source))
        .unwrap_or_default();
    let names = labels
        .into_iter()
        .skip(1)
        .map(|node| (node, clean(node, source)))
        .collect();
    (kind, names)
}
fn module_source<'tree>(block: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let (kind, _) = block_parts(block, source);
    if kind != "module" {
        return None;
    }
    descendants(block, "attribute")
        .into_iter()
        .find_map(|attribute| {
            let children = named_children(attribute);
            (children
                .first()
                .is_some_and(|node| text(*node, source) == "source"))
            .then(|| descendants(attribute, "string_lit").into_iter().next())
            .flatten()
        })
}
pub(crate) struct HclSpec;
impl LanguageSpec for HclSpec {
    fn language_name(&self) -> &'static str {
        "hcl"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_hcl::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["hcl", "tf", "tfvars"]
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
        let boundary = if capture == "symbol.property" {
            nearest_ancestor(node, &["attribute"])
        } else {
            nearest_ancestor(node, &["block"])
        };
        if !is_recoverable(boundary) {
            return false;
        }
        match capture {
            "symbol.type" => {
                let (kind, _) = block_parts(node, source);
                !kind.is_empty()
            }
            "symbol.property" => !named_children(node).is_empty(),
            "nav.import" => module_source(node, source).is_some(),
            "local.reference" => {
                let name = text(node, source);
                named_children(node)
                    .iter()
                    .any(|child| child.kind() == "variable_expr")
                    && ["var.", "local.", "module.", "data."]
                        .iter()
                        .any(|prefix| name.starts_with(prefix))
            }
            _ => true,
        }
    }
    fn symbol_kind_for_capture(
        &self,
        capture: &str,
        node: Node<'_>,
        source: &[u8],
        _default_kind: &'static str,
    ) -> String {
        if capture == "symbol.property" {
            "attribute".to_string()
        } else {
            block_parts(node, source).0
        }
    }
    fn refine_kind(&self, _capture: &str, node: Node<'_>, _kind: &'static str) -> &'static str {
        match named_children(node).first().map(|node| node.kind()) {
            Some("identifier") => "block",
            _ => "block",
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
        if capture == "symbol.property" {
            return named_children(node)
                .first()
                .map(|name| NameDecision::Name(clean(*name, source)));
        }
        let (kind, names) = block_parts(node, source);
        Some(NameDecision::Name(if names.is_empty() {
            kind
        } else {
            names
                .into_iter()
                .map(|(_, name)| name)
                .collect::<Vec<_>>()
                .join(".")
        }))
    }
    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        Some(
            module_source(node, source)
                .map(|value| import(value, source))
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
