//! CMake language spec.

use super::format_support::{
    clean, descendants, first_descendant, import, is_recoverable, reference, text,
};
use super::{LanguageSpec, NameDecision};
use crate::parser::{ImportEntry, ReferenceSite};
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
const QUERY_SOURCE: &str = include_str!("../../queries/cmake/symbols.scm");
fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_cmake::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile CMake query")
    })
}
fn command_parts<'tree>(node: Node<'tree>, source: &[u8]) -> Option<(String, Vec<Node<'tree>>)> {
    let command = if node.kind() == "normal_command" {
        node
    } else {
        first_descendant(
            node,
            if node.kind() == "function_def" {
                "function_command"
            } else {
                "macro_command"
            },
        )?
    };
    let name = first_descendant(command, "identifier")
        .map(|identifier| text(identifier, source).to_ascii_lowercase())
        .unwrap_or_else(|| {
            if node.kind() == "function_def" {
                "function".into()
            } else {
                "macro".into()
            }
        });
    let arguments = first_descendant(command, "argument_list")?;
    let values = descendants(arguments, "argument")
        .into_iter()
        .filter_map(|argument| super::format_support::first_named(argument))
        .collect();
    Some((name, values))
}
pub(crate) struct CmakeSpec;
impl LanguageSpec for CmakeSpec {
    fn language_name(&self) -> &'static str {
        "cmake"
    }
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_cmake::LANGUAGE.into()
    }
    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["cmake"]
    }
    fn exact_names(&self) -> &'static [&'static str] {
        &["CMakeLists.txt"]
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
        if !is_recoverable(node) {
            return false;
        }
        let Some((command, values)) = command_parts(node, source) else {
            return false;
        };
        match capture {
            "symbol.type" => {
                matches!(node.kind(), "function_def" | "macro_def")
                    || matches!(
                        command.as_str(),
                        "add_executable"
                            | "add_library"
                            | "add_custom_target"
                            | "set"
                            | "option"
                            | "project"
                            | "add_test"
                            | "find_package"
                            | "fetchcontent_declare"
                    ) && !values.is_empty()
            }
            "nav.import" => {
                matches!(
                    command.as_str(),
                    "include" | "add_subdirectory" | "find_package" | "fetchcontent_declare"
                ) && !values.is_empty()
            }
            "local.reference" => {
                matches!(
                    command.as_str(),
                    "add_dependencies" | "target_link_libraries"
                ) && values.len() > 1
            }
            _ => true,
        }
    }
    fn refine_kind(&self, _capture: &str, node: Node<'_>, _kind: &'static str) -> &'static str {
        match node.kind() {
            "function_def" => "function",
            "macro_def" => "macro",
            _ => "target",
        }
    }
    fn symbol_kind_for_capture(
        &self,
        _capture: &str,
        node: Node<'_>,
        source: &[u8],
        _default_kind: &'static str,
    ) -> String {
        if node.kind() == "function_def" {
            return "function".to_string();
        }
        if node.kind() == "macro_def" {
            return "macro".to_string();
        }
        match command_parts(node, source).map(|(command, _)| command) {
            Some(command) if matches!(command.as_str(), "set" | "option") => "variable",
            Some(command) if command == "project" => "project",
            Some(command) if command == "add_test" => "test",
            Some(command)
                if matches!(command.as_str(), "find_package" | "fetchcontent_declare") =>
            {
                "dependency"
            }
            _ => "target",
        }
        .to_string()
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
        command_parts(node, source)
            .and_then(|(command, values)| {
                if command == "add_test"
                    && values
                        .first()
                        .is_some_and(|value| text(*value, source).eq_ignore_ascii_case("NAME"))
                {
                    values.get(1).copied()
                } else {
                    values.first().copied()
                }
            })
            .map(|name| NameDecision::Name(clean(name, source)))
    }
    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        Some(
            command_parts(node, source)
                .and_then(|(_, values)| values.first().copied())
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
        Some(
            command_parts(node, source)
                .map(|(_, values)| {
                    values
                        .into_iter()
                        .skip(1)
                        .map(|value| reference(value, source))
                        .collect()
                })
                .unwrap_or_default(),
        )
    }
}
