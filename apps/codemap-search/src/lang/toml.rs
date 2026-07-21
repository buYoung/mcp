//! TOML language spec.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::format_support::{clean, first_named, is_recoverable, named_children, nearest_ancestor};
use super::{LanguageSpec, NameDecision};

const QUERY_SOURCE: &str = include_str!("../../queries/toml/symbols.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_toml_ng::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile TOML query")
    })
}

fn table_name(table: Node<'_>, source: &[u8]) -> String {
    named_children(table)
        .into_iter()
        .take_while(|node| node.kind() != "pair")
        .filter(|node| matches!(node.kind(), "bare_key" | "quoted_key" | "dotted_key"))
        .map(|node| clean(node, source))
        .collect::<Vec<_>>()
        .join(".")
}

pub(crate) struct TomlSpec;

impl LanguageSpec for TomlSpec {
    fn language_name(&self) -> &'static str {
        "toml"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_toml_ng::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["toml"]
    }
    fn caller_scan_enabled(&self) -> bool {
        false
    }
    fn indexes_format_text(&self) -> bool {
        true
    }

    fn capture_is_valid(&self, _capture: &str, node: Node<'_>, _source: &[u8]) -> bool {
        is_recoverable(nearest_ancestor(
            node,
            &["pair", "table", "table_array_element"],
        ))
    }

    fn refine_kind(&self, capture: &str, _node: Node<'_>, _kind: &'static str) -> &'static str {
        if capture == "symbol.type" {
            "table"
        } else {
            "key"
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
        if capture == "symbol.type" {
            return Some(NameDecision::Name(table_name(node, source)));
        }
        let key = clean(first_named(node).unwrap_or(node), source);
        let current_pair = node.parent();
        let mut parts = Vec::new();
        let mut ancestor = current_pair.and_then(|pair| pair.parent());
        while let Some(candidate) = ancestor {
            if candidate.kind() == "pair" && Some(candidate) != current_pair {
                if let Some(parent_key) = first_named(candidate) {
                    parts.push(clean(parent_key, source));
                }
            } else if matches!(candidate.kind(), "table" | "table_array_element") {
                let name = table_name(candidate, source);
                if !name.is_empty() {
                    parts.push(name);
                }
            }
            ancestor = candidate.parent();
        }
        parts.reverse();
        let prefix = parts.join(".");
        Some(NameDecision::Name(if prefix.is_empty() {
            key
        } else {
            format!("{prefix}.{key}")
        }))
    }
}
