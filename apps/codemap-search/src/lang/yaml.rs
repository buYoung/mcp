//! YAML language spec.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::format_support::{clean, first_named, is_recoverable, nearest_ancestor};
use super::{LanguageSpec, NameDecision};

const QUERY_SOURCE: &str = include_str!("../../queries/yaml/symbols.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_yaml::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile YAML query")
    })
}

fn qualified_key(pair: Node<'_>, source: &[u8]) -> String {
    let mut keys = Vec::new();
    let mut current = Some(pair);
    while let Some(candidate) = current {
        if matches!(candidate.kind(), "block_mapping_pair" | "flow_pair") {
            if let Some(key) = candidate.child_by_field_name("key") {
                keys.push(clean(first_named(key).unwrap_or(key), source));
            }
        }
        current = candidate.parent();
    }
    keys.reverse();
    keys.join(".")
}

pub(crate) struct YamlSpec;

impl LanguageSpec for YamlSpec {
    fn language_name(&self) -> &'static str {
        "yaml"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_yaml::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["yaml", "yml"]
    }
    fn caller_scan_enabled(&self) -> bool {
        false
    }
    fn indexes_format_text(&self) -> bool {
        true
    }

    fn capture_is_valid(&self, _capture: &str, node: Node<'_>, _source: &[u8]) -> bool {
        is_recoverable(nearest_ancestor(node, &["block_mapping_pair", "flow_pair"]))
    }

    fn refine_kind(&self, _capture: &str, _node: Node<'_>, _kind: &'static str) -> &'static str {
        "key"
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
        Some(NameDecision::Name(qualified_key(node, source)))
    }
}
