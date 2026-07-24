use std::collections::HashSet;
use std::sync::OnceLock;

use tree_sitter::{Language, Node, Query};

use super::{generic_find_owner, path_indicates_test, LanguageSpec};
use crate::parser::{CodeRange, ImportEntry, ImportKind};

const QUERY_STR: &str = concat!(
    include_str!("../../queries/powershell/symbols.scm"),
    "\n",
    include_str!("../../queries/powershell/navigation.scm")
);
const TAGS_QUERY_STR: &str = include_str!("../../queries/powershell/tags.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_powershell::LANGUAGE.into(), QUERY_STR)
            .expect("Failed to compile PowerShell query")
    })
}

fn tags_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_powershell::LANGUAGE.into(), TAGS_QUERY_STR)
            .expect("Failed to compile PowerShell tags query")
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

fn literal_arguments(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let quote = bytes[index];
        if quote != b'\'' && quote != b'"' {
            index += 1;
            continue;
        }
        let start = index + 1;
        index = start;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index < bytes.len() {
            let value = &text[start..index];
            if !value.contains('$') {
                values.push(value.to_string());
            }
        }
        index += 1;
    }
    values
}

fn is_dynamic_command(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("& ")
        || trimmed.starts_with("Invoke-Expression ")
        || trimmed.starts_with("iex ")
        || trimmed.starts_with(". $")
}

fn command_import(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.starts_with("Import-Module ") {
        return literal_arguments(trimmed).into_iter().next().or_else(|| {
            trimmed
                .split_whitespace()
                .nth(1)
                .filter(|value| {
                    !value.starts_with('$') && !value.starts_with('(') && !value.starts_with('-')
                })
                .map(str::to_string)
        });
    }
    if trimmed.starts_with(". ") {
        return literal_arguments(trimmed).into_iter().next().or_else(|| {
            trimmed
                .split_whitespace()
                .nth(1)
                .filter(|value| !value.starts_with('$') && !value.starts_with('('))
                .map(str::to_string)
        });
    }
    None
}

fn previous_nonempty_line_contains(node: Node<'_>, source: &[u8], needle: &str) -> bool {
    let prefix = std::str::from_utf8(&source[..node.start_byte()]).unwrap_or_default();
    prefix
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| line.contains(needle))
}

pub(crate) struct PowerShellSpec;

impl LanguageSpec for PowerShellSpec {
    fn language_name(&self) -> &'static str {
        "powershell"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["pwsh"]
    }

    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_powershell::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(tags_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ps1", "psm1"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn capture_is_valid(&self, capture_name: &str, node: Node<'_>, source: &[u8]) -> bool {
        if capture_name == "nav.call" {
            return node
                .utf8_text(source)
                .is_ok_and(|text| !is_dynamic_command(text));
        }
        if capture_name == "symbol.variable" {
            let mut ancestor = node.parent();
            while let Some(current) = ancestor {
                if matches!(
                    current.kind(),
                    "function_statement" | "class_method_definition" | "script_block_expression"
                ) {
                    return false;
                }
                ancestor = current.parent();
            }
        }
        true
    }

    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        let text = node.utf8_text(source).ok()?;
        let Some(path) = command_import(text) else {
            return Some(Vec::new());
        };
        let local_name = path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&path)
            .trim_end_matches(".psm1")
            .trim_end_matches(".ps1")
            .to_string();
        Some(vec![ImportEntry {
            local_name,
            imported_name: None,
            source: Some(path),
            kind: ImportKind::Namespace,
            range: range_for_node(node),
        }])
    }

    fn collect_exported_names(&self, root: Node<'_>, source: &[u8], out: &mut HashSet<String>) {
        let Ok(text) = root.utf8_text(source) else {
            return;
        };
        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("Export-ModuleMember ") || !trimmed.contains("-Function") {
                continue;
            }
            for name in literal_arguments(trimmed) {
                if name != "*" {
                    out.insert(name);
                }
            }
            if let Some((_, names)) = trimmed.split_once("-Function") {
                for name in names
                    .split([',', ' ', '\t'])
                    .map(|name| name.trim_matches(['\'', '"']))
                    .take_while(|name| !name.starts_with('-'))
                    .filter(|name| {
                        !name.is_empty()
                            && *name != "*"
                            && !name.contains('$')
                            && name
                                .chars()
                                .all(|character| character.is_alphanumeric() || character == '-')
                    })
                {
                    out.insert(name.to_string());
                }
            }
        }
    }

    fn is_import_line(&self, line: &str) -> bool {
        let line = line.trim_start();
        line.starts_with("Import-Module ") || line.starts_with(". ")
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
            || file_path.ends_with(".Tests.ps1")
            || name.starts_with("Test-")
    }

    fn is_exported(
        &self,
        _node: Node<'_>,
        name: &str,
        _kind: &str,
        _source: &[u8],
        exported_names: &HashSet<String>,
    ) -> bool {
        exported_names.is_empty() || exported_names.contains(name)
    }

    fn is_deprecated(
        &self,
        node: Node<'_>,
        source: &[u8],
        docstring: &Option<String>,
        comments_text: &str,
    ) -> bool {
        node.utf8_text(source)
            .is_ok_and(|text| text.contains("[Obsolete"))
            || previous_nonempty_line_contains(node, source, "[Obsolete")
            || previous_nonempty_line_contains(node, source, "Deprecated")
            || comments_text.to_ascii_lowercase().contains("deprecated")
            || docstring.as_ref().is_some_and(|docstring| {
                let text = docstring.to_ascii_lowercase();
                text.contains("deprecated") || text.contains("obsolete")
            })
    }

    fn find_owner(&self, node: Node<'_>, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["function_statement", "script_block_expression"]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["class_statement", "enum_statement"]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["statement_block", "script_block"]
    }

    fn owner_for_container<'a>(&self, current: Node<'a>, source: &[u8]) -> Option<Option<String>> {
        if !matches!(current.kind(), "class_statement" | "enum_statement") {
            return None;
        }
        let mut cursor = current.walk();
        let owner = current
            .named_children(&mut cursor)
            .find(|child| child.kind() == "simple_name")
            .and_then(|name| name.utf8_text(source).ok())
            .map(str::to_string);
        Some(owner)
    }
}
