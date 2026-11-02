//! Bash and POSIX shell language spec.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::format_support::{
    clean, descendants, import, is_recoverable, named_children, nearest_ancestor, text,
};
use super::{literal_path_tail, LanguageSpec, NameDecision};
use crate::parser::ImportEntry;

const QUERY_SOURCE: &str = include_str!("../../queries/bash/symbols.scm");
fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_bash::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile Bash query")
    })
}

/// The path a `source` or `.` command reads, when at most a leading part of it is computed, as
/// in `"$(dirname "$0")/lib.sh"`: the literal rest still names the file.
fn sourced_path<'tree>(command: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let children = named_children(command);
    let name = children.iter().find(|node| node.kind() == "command_name")?;
    if !matches!(text(*name, source).as_str(), "source" | ".") {
        return None;
    }
    let path = children
        .iter()
        .find(|node| matches!(node.kind(), "word" | "string" | "concatenation"))
        .copied()?;
    let is_dynamic = [
        "simple_expansion",
        "expansion",
        "command_substitution",
        "process_substitution",
    ]
    .iter()
    .any(|kind| !descendants(path, kind).is_empty());
    (!is_dynamic || literal_path_tail(&clean(path, source)).is_some()).then_some(path)
}

pub(crate) struct BashSpec;
impl LanguageSpec for BashSpec {
    fn language_name(&self) -> &'static str {
        "bash"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_bash::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["sh", "bash"]
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
        is_recoverable(nearest_ancestor(
            node,
            &["function_definition", "variable_assignment", "command"],
        )) && (capture != "nav.import" || sourced_path(node, source).is_some())
    }
    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        Some(
            sourced_path(node, source)
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
            // A function spans its body; the query's name capture names it.
            return None;
        }
        Some(NameDecision::Name(clean(node, source)))
    }
}
