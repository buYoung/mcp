//! Ruby language spec.

use std::collections::HashSet;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{generic_find_owner, path_indicates_test, LanguageSpec, NameDecision};
use crate::parser::{CodeRange, ImportEntry, ImportKind};

const QUERY_STR: &str = concat!(
    include_str!("../../queries/ruby/symbols.scm"),
    "\n",
    include_str!("../../queries/ruby/navigation.scm")
);
const TAGS_QUERY_STR: &str = include_str!("../../queries/ruby/tags.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_ruby::LANGUAGE.into(), QUERY_STR)
            .expect("Failed to compile Ruby query")
    })
}

fn tags_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_ruby::LANGUAGE.into(), TAGS_QUERY_STR)
            .expect("Failed to compile Ruby tags query")
    })
}

fn range_for_node(node: Node<'_>) -> CodeRange {
    let start = node.start_position();
    let end = node.end_position();
    CodeRange {
        start_line: start.row + 1,
        start_col: start.column + 1,
        end_line: end.row + 1,
        end_col: end.column + 1,
    }
}

fn literal_string(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).ok()?.trim();
    let quote = text.chars().next()?;
    if !matches!(quote, '\'' | '"') || !text.ends_with(quote) || text.len() < 2 {
        return None;
    }
    let value = &text[1..text.len() - 1];
    (!value.contains("#{")).then(|| value.to_string())
}

fn call_method(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.child_by_field_name("method")?
        .utf8_text(source)
        .ok()
        .map(str::to_string)
}

fn first_simple_symbol(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() == "simple_symbol" {
        return node
            .utf8_text(source)
            .ok()
            .map(|name| name.trim_start_matches(':').to_string());
    }
    (0..node.named_child_count())
        .filter_map(|index| node.named_child(index as u32))
        .find_map(|child| first_simple_symbol(child, source))
}

fn visibility_before(node: Node<'_>, source: &[u8]) -> &'static str {
    let mut sibling = node.prev_named_sibling();
    while let Some(previous) = sibling {
        let visibility = if previous.kind() == "call" {
            call_method(previous, source)
        } else if previous.kind() == "identifier" {
            previous.utf8_text(source).ok().map(str::to_string)
        } else {
            None
        };
        if let Some(method) = visibility {
            if matches!(method.as_str(), "public" | "private" | "protected") {
                return match method.as_str() {
                    "private" => "private",
                    "protected" => "protected",
                    _ => "public",
                };
            }
        }
        sibling = previous.prev_named_sibling();
    }
    "public"
}

fn containing_body_statement(mut node: Node<'_>) -> Option<Node<'_>> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "body_statement" {
            return Some(node);
        }
        node = parent;
    }
    None
}

fn is_file_top_level(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if matches!(parent.kind(), "class" | "module" | "singleton_class") {
            return false;
        }
        node = parent;
    }
    true
}

fn preceding_comment_contains(node: Node<'_>, source: &[u8], marker: &str) -> bool {
    let Ok(source) = std::str::from_utf8(source) else {
        return false;
    };
    source
        .lines()
        .collect::<Vec<_>>()
        .into_iter()
        .take(node.start_position().row)
        .rev()
        .take_while(|line| {
            let line = line.trim();
            line.is_empty() || line.starts_with('#')
        })
        .any(|line| line.to_ascii_lowercase().contains(marker))
}

pub(crate) struct RubySpec;

impl LanguageSpec for RubySpec {
    fn language_name(&self) -> &'static str {
        "ruby"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["rb"]
    }

    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_ruby::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(tags_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rb"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn capture_is_valid(&self, _capture_name: &str, node: Node<'_>, _source: &[u8]) -> bool {
        !node.has_error()
    }

    fn name_for_capture(
        &self,
        capture_name: &str,
        node: Node<'_>,
        _kind: &str,
        _ext: &str,
        source: &[u8],
        _asm_meta_kind_text: &Option<String>,
    ) -> Option<NameDecision> {
        (capture_name == "symbol.property").then(|| {
            first_simple_symbol(node, source)
                .map(NameDecision::Name)
                .unwrap_or(NameDecision::Skip)
        })
    }

    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        let method = call_method(node, source)?;
        let arguments = node.child_by_field_name("arguments")?;
        let string_node = (0..arguments.named_child_count())
            .filter_map(|index| arguments.named_child(index as u32))
            .find(|child| child.kind() == "string")?;
        let Some(mut source_path) = literal_string(string_node, source) else {
            return Some(Vec::new());
        };
        if method == "require_relative"
            && !source_path.starts_with("./")
            && !source_path.starts_with("../")
        {
            source_path = format!("./{source_path}");
        }
        let local_name = source_path
            .rsplit('/')
            .next()
            .and_then(|part| part.rsplit_once('.').map(|(stem, _)| stem).or(Some(part)))
            .unwrap_or(&source_path)
            .to_string();
        Some(vec![ImportEntry {
            local_name: local_name.clone(),
            imported_name: Some(local_name),
            source: Some(source_path),
            kind: ImportKind::Named,
            range: range_for_node(node),
        }])
    }

    fn additional_symbol_names_for_capture(
        &self,
        capture_name: &str,
        node: Node<'_>,
        source: &[u8],
        _primary_name: &str,
    ) -> Vec<String> {
        if capture_name != "symbol.property" {
            return Vec::new();
        }
        let Some(arguments) = node.child_by_field_name("arguments") else {
            return Vec::new();
        };
        (0..arguments.named_child_count())
            .filter_map(|index| arguments.named_child(index as u32))
            .filter(|child| child.kind() == "simple_symbol")
            .filter_map(|child| child.utf8_text(source).ok())
            .map(|name| name.trim_start_matches(':').to_string())
            .skip(1)
            .collect()
    }

    fn is_import_line(&self, line: &str) -> bool {
        let line = line.trim_start();
        line.starts_with("require ")
            || line.starts_with("require(")
            || line.starts_with("require_relative ")
            || line.starts_with("require_relative(")
            || line.starts_with("load ")
            || line.starts_with("load(")
    }

    fn is_test(
        &self,
        _node: Node<'_>,
        name: &str,
        _kind: &str,
        file_path: &str,
        _source: &[u8],
        _comments_text: &str,
    ) -> bool {
        path_indicates_test(file_path)
            || file_path.ends_with("_spec.rb")
            || file_path.ends_with("_test.rb")
            || name.starts_with("test_")
    }

    fn is_exported(
        &self,
        node: Node<'_>,
        _name: &str,
        _kind: &str,
        source: &[u8],
        _exported_names: &HashSet<String>,
    ) -> bool {
        if is_file_top_level(node) {
            return true;
        }
        containing_body_statement(node)
            .is_some_and(|statement| visibility_before(statement, source) == "public")
    }

    fn is_deprecated(
        &self,
        node: Node<'_>,
        source: &[u8],
        docstring: &Option<String>,
        comments_text: &str,
    ) -> bool {
        preceding_comment_contains(node, source, "@deprecated")
            || comments_text.to_ascii_lowercase().contains("@deprecated")
            || docstring
                .as_ref()
                .is_some_and(|docstring| docstring.to_ascii_lowercase().contains("@deprecated"))
    }

    fn find_owner(&self, node: Node<'_>, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "method",
            "singleton_method",
            "singleton_class",
            "block",
            "do_block",
        ]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["class", "module"]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["body_statement"]
    }
}
