//! Nix language spec for statically decidable symbols and navigation relationships.

use super::format_support::{clean, is_recoverable, named_children, range, text};
use super::{LanguageSpec, NameDecision};
use crate::parser::{CallSite, ImportEntry, ImportKind, LocalBinding, ReferenceSite};
use std::collections::HashSet;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

const QUERY_SOURCE: &str = concat!(
    include_str!("../../queries/nix/symbols.scm"),
    "\n",
    include_str!("../../queries/nix/navigation.scm")
);
const TAGS_QUERY_SOURCE: &str = include_str!("../../queries/nix/tags.scm");

fn query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_nix::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile Nix query")
    })
}

fn tags_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_nix::LANGUAGE.into(), TAGS_QUERY_SOURCE)
            .expect("Failed to compile Nix tags query")
    })
}

fn static_attr(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" => Some(text(node, source)),
        "string_expression"
            if !named_children(node)
                .iter()
                .any(|child| child.kind() == "interpolation") =>
        {
            Some(clean(node, source))
        }
        _ => None,
    }
    .filter(|name| !name.is_empty())
}

fn static_attrpath(node: Node<'_>, source: &[u8]) -> Option<Vec<String>> {
    let attrs = named_children(node);
    let path = attrs
        .into_iter()
        .map(|attr| static_attr(attr, source))
        .collect::<Option<Vec<_>>>()?;
    (!path.is_empty()).then_some(path)
}

fn binding_attrpath(binding: Node<'_>, source: &[u8]) -> Option<Vec<String>> {
    binding
        .child_by_field_name("attrpath")
        .and_then(|attrpath| static_attrpath(attrpath, source))
}

fn parent_binding_for_attrset(attrset: Node<'_>) -> Option<Node<'_>> {
    let binding = attrset.parent()?;
    if binding.kind() != "binding"
        || binding
            .child_by_field_name("expression")
            .is_none_or(|expression| expression.id() != attrset.id())
    {
        return None;
    }
    Some(binding)
}

fn binding_name(binding: Node<'_>, source: &[u8]) -> Option<String> {
    let mut segments = binding_attrpath(binding, source)?;
    let mut current = binding.parent();
    while let Some(node) = current {
        if matches!(
            node.kind(),
            "attrset_expression" | "rec_attrset_expression" | "let_attrset_expression"
        ) {
            if let Some(parent_binding) = parent_binding_for_attrset(node) {
                let mut prefix = binding_attrpath(parent_binding, source)?;
                prefix.extend(segments);
                segments = prefix;
                current = parent_binding.parent();
                continue;
            }
        }
        current = node.parent();
    }
    Some(segments.join("."))
}

fn inherited_names(node: Node<'_>, source: &[u8]) -> Vec<String> {
    node.child_by_field_name("attrs")
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|attr| static_attr(attr, source))
        .collect()
}

fn inherited_symbol_names(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let names = inherited_names(node, source);
    let Some(binding_set) = node
        .parent()
        .filter(|parent| parent.kind() == "binding_set")
    else {
        return names;
    };
    let Some(attrset) = binding_set.parent().filter(|parent| {
        matches!(
            parent.kind(),
            "attrset_expression" | "rec_attrset_expression" | "let_attrset_expression"
        )
    }) else {
        return names;
    };
    let Some(parent_binding) = parent_binding_for_attrset(attrset) else {
        return names;
    };
    let Some(prefix) = binding_name(parent_binding, source) else {
        return names;
    };
    names
        .into_iter()
        .map(|name| format!("{prefix}.{name}"))
        .collect()
}

fn static_selection(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "variable_expression" => node
            .child_by_field_name("name")
            .map(|name| text(name, source))
            .filter(|name| !name.is_empty()),
        "select_expression" => {
            let base = node
                .child_by_field_name("expression")
                .and_then(|expression| static_selection(expression, source))?;
            let attrs = node
                .child_by_field_name("attrpath")
                .and_then(|attrpath| static_attrpath(attrpath, source))?;
            Some(format!("{base}.{}", attrs.join(".")))
        }
        "parenthesized_expression" => node
            .child_by_field_name("expression")
            .and_then(|expression| static_selection(expression, source)),
        _ => None,
    }
}

fn static_callee(node: Node<'_>, source: &[u8]) -> Option<(String, Option<String>)> {
    let function = node.child_by_field_name("function")?;
    let selection = static_selection(function, source)?;
    if let Some((receiver, name)) = selection.rsplit_once('.') {
        Some((name.to_string(), Some(receiver.to_string())))
    } else {
        Some((selection, None))
    }
}

fn literal_path(node: Node<'_>, source: &[u8]) -> Option<String> {
    let is_literal = match node.kind() {
        "path_expression"
        | "hpath_expression"
        | "string_expression"
        | "indented_string_expression" => !named_children(node)
            .iter()
            .any(|child| child.kind() == "interpolation"),
        "spath_expression" => true,
        _ => false,
    };
    is_literal
        .then(|| clean(node, source))
        .filter(|path| !path.is_empty())
}

fn import_path<'tree>(node: Node<'tree>, source: &[u8]) -> Option<(Node<'tree>, String)> {
    let (callee, receiver) = static_callee(node, source)?;
    let is_import = matches!(
        (receiver.as_deref(), callee.as_str()),
        (None, "import") | (Some("builtins"), "import") | (None, "callPackage")
    );
    if !is_import {
        return None;
    }
    let argument = node.child_by_field_name("argument")?;
    literal_path(argument, source).map(|path| (argument, path))
}

fn nix_scope_id(node: Node<'_>) -> Option<usize> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if candidate.kind() == "function_expression" {
            let range = range(candidate);
            return Some(range.start_line.saturating_mul(100_000) + range.end_line);
        }
        current = candidate.parent();
    }
    None
}

fn symbol_value(node: Node<'_>) -> Option<Node<'_>> {
    (node.kind() == "binding")
        .then(|| node.child_by_field_name("expression"))
        .flatten()
}

fn is_direct_derivation(value: Node<'_>, source: &[u8]) -> bool {
    value.kind() == "apply_expression"
        && static_callee(value, source)
            .is_some_and(|(name, _)| matches!(name.as_str(), "derivation" | "mkDerivation"))
}

fn reference(name: String, node: Node<'_>) -> ReferenceSite {
    ReferenceSite {
        name,
        range: range(node),
        scope_id: nix_scope_id(node),
    }
}

fn is_let_binding(node: Node<'_>) -> bool {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if candidate.kind() == "binding_set"
            && candidate
                .parent()
                .is_some_and(|parent| parent.kind() == "let_expression")
        {
            return true;
        }
        current = candidate.parent();
    }
    false
}

pub(crate) struct NixSpec;

impl LanguageSpec for NixSpec {
    fn language_name(&self) -> &'static str {
        "nix"
    }

    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_nix::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(tags_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["nix"]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["nix"]
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
        if !is_recoverable(node) {
            return false;
        }
        match capture {
            name if name.starts_with("symbol.") => match node.kind() {
                "binding" => binding_name(node, source).is_some(),
                "inherit" | "inherit_from" => !inherited_symbol_names(node, source).is_empty(),
                _ => false,
            },
            "nav.call" => static_callee(node, source).is_some(),
            "nav.import" => import_path(node, source).is_some(),
            "local.scope" => matches!(node.kind(), "formal" | "identifier"),
            "local.reference" => self
                .reference_sites_for_capture(node, source)
                .is_some_and(|references| !references.is_empty()),
            _ => true,
        }
    }

    fn symbol_kind_for_capture(
        &self,
        _capture: &str,
        node: Node<'_>,
        source: &[u8],
        _default_kind: &'static str,
    ) -> String {
        match symbol_value(node) {
            Some(value) if value.kind() == "function_expression" => "fn",
            Some(value) if is_direct_derivation(value, source) => "target",
            _ => "variable",
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
        let name = match node.kind() {
            "binding" => binding_name(node, source),
            "inherit" | "inherit_from" => inherited_symbol_names(node, source).into_iter().next(),
            _ => None,
        }?;
        Some(NameDecision::Name(name))
    }

    fn additional_symbol_names_for_capture(
        &self,
        _capture: &str,
        node: Node<'_>,
        source: &[u8],
        primary_name: &str,
    ) -> Vec<String> {
        if matches!(node.kind(), "inherit" | "inherit_from") {
            inherited_symbol_names(node, source)
                .into_iter()
                .filter(|name| name != primary_name)
                .collect()
        } else {
            Vec::new()
        }
    }

    fn is_exported(
        &self,
        node: Node<'_>,
        _name: &str,
        _kind: &str,
        _source: &[u8],
        _exported_names: &HashSet<String>,
    ) -> bool {
        !is_let_binding(node)
    }

    fn call_sites_for_capture(&self, node: Node<'_>, source: &[u8]) -> Option<Vec<CallSite>> {
        Some(
            static_callee(node, source)
                .map(|(name, receiver)| CallSite {
                    name,
                    receiver,
                    range: range(node),
                    scope_id: nix_scope_id(node),
                })
                .into_iter()
                .collect(),
        )
    }

    fn import_entries_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ImportEntry>> {
        Some(
            import_path(node, source)
                .map(|(path_node, path)| ImportEntry {
                    local_name: path.clone(),
                    imported_name: None,
                    source: Some(path),
                    kind: ImportKind::Default,
                    range: range(path_node),
                })
                .into_iter()
                .collect(),
        )
    }

    fn local_bindings_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<LocalBinding>> {
        let name_node = if node.kind() == "formal" {
            node.child_by_field_name("name")
        } else {
            Some(node)
        };
        Some(
            name_node
                .map(|name| LocalBinding {
                    name: text(name, source),
                    type_name: None,
                    value_type: None,
                    range: range(node),
                    scope_id: nix_scope_id(node),
                })
                .into_iter()
                .collect(),
        )
    }

    fn reference_sites_for_capture(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<Vec<ReferenceSite>> {
        let references = match node.kind() {
            "variable_expression" => {
                let is_selection_base = node.parent().is_some_and(|parent| {
                    parent.kind() == "select_expression"
                        && parent
                            .child_by_field_name("expression")
                            .is_some_and(|expression| expression.id() == node.id())
                });
                if is_selection_base {
                    Vec::new()
                } else {
                    static_selection(node, source)
                        .map(|name| reference(name, node))
                        .into_iter()
                        .collect()
                }
            }
            "select_expression" => static_selection(node, source)
                .map(|name| reference(name, node))
                .into_iter()
                .collect(),
            "inherit" => inherited_names(node, source)
                .into_iter()
                .map(|name| reference(name, node))
                .collect(),
            "inherit_from" => {
                let Some(receiver) = node
                    .child_by_field_name("expression")
                    .and_then(|expression| static_selection(expression, source))
                else {
                    return Some(Vec::new());
                };
                inherited_names(node, source)
                    .into_iter()
                    .map(|name| reference(format!("{receiver}.{name}"), node))
                    .collect()
            }
            _ => Vec::new(),
        };
        Some(references)
    }
}
