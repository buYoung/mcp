//! JSON and JSONC language spec.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::format_support::{clean, is_recoverable, nearest_ancestor};
use super::{LanguageSpec, NameDecision};

const QUERY_SOURCE: &str = include_str!("../../queries/json/symbols.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_json::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile JSON query")
    })
}

fn qualified_key(node: Node<'_>, source: &[u8]) -> String {
    let mut keys = vec![clean(node, source)];
    let current_pair = node.parent();
    let mut ancestor = current_pair.and_then(|pair| pair.parent());
    while let Some(candidate) = ancestor {
        if candidate.kind() == "pair" && Some(candidate) != current_pair {
            if let Some(key) = candidate.child_by_field_name("key") {
                keys.push(clean(key, source));
            }
        }
        ancestor = candidate.parent();
    }
    keys.reverse();
    keys.join(".")
}

pub(crate) struct JsonSpec;

impl LanguageSpec for JsonSpec {
    fn language_name(&self) -> &'static str {
        "json"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_json::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["json", "jsonc"]
    }
    fn caller_scan_enabled(&self) -> bool {
        false
    }
    fn indexes_format_text(&self) -> bool {
        true
    }

    fn capture_is_valid(&self, _capture: &str, node: Node<'_>, _source: &[u8]) -> bool {
        is_recoverable(nearest_ancestor(node, &["pair"]))
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
