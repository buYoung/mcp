//! Go language spec: query, compiled query, and the Go-specific extraction hooks.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{contains_case_insensitive, generic_find_owner, is_inside_function, LanguageSpec};

// Go: all named types are `type_spec`; struct vs interface vs alias is resolved in
// code from the `type:` child (see the `symbol.gotype` arm) to avoid referencing
// type-expression node kinds the grammar may or may not expose (an unknown kind makes
// `Query::new` return Err -> the `.expect()` panics on first use).
const GO_QUERY_STR: &str = concat!(
    include_str!("../../queries/go/symbols.scm"),
    "\n",
    include_str!("../../queries/go/navigation.scm")
);

const GO_TAGS_QUERY_STR: &str = include_str!("../../queries/go/tags.scm");

fn get_go_query() -> &'static Query {
    static GO_QUERY: OnceLock<Query> = OnceLock::new();
    GO_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_go::LANGUAGE.into(), GO_QUERY_STR)
            .expect("Failed to compile Go query")
    })
}

fn get_go_tags_query() -> &'static Query {
    static GO_TAGS_QUERY: OnceLock<Query> = OnceLock::new();
    GO_TAGS_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_go::LANGUAGE.into(), GO_TAGS_QUERY_STR)
            .expect("Failed to compile Go tags query")
    })
}

/// Go: an identifier is exported iff its first letter is uppercase (Go spec — "Exported
/// identifiers"). Applies uniformly to functions, types, fields, consts and vars.
fn go_is_exported(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_uppercase())
}

/// Go test entry points: a `Test` / `Benchmark` / `Example` / `Fuzz` prefix followed by
/// end-of-name or a non-lowercase rune, so `Testify` / `Examples` are not flagged.
fn go_is_test_name(name: &str) -> bool {
    const PREFIXES: [&str; 4] = ["Test", "Benchmark", "Example", "Fuzz"];
    PREFIXES.iter().any(|prefix| {
        name.strip_prefix(prefix)
            .is_some_and(|rest| rest.chars().next().is_none_or(|c| !c.is_lowercase()))
    })
}

/// Go doc comments are plain `//` (or `/* */`) lines directly above a declaration — Go
/// has no `///`-style marker — so [`super::clean_docstring`] (which only promotes
/// `///`/`//!`/`/**`) drops them. Promote the contiguous run here, for Go only.
fn clean_go_doc_comments(comments: &[String]) -> Option<String> {
    if comments.is_empty() {
        return None;
    }
    // `comments` is collected nearest-first; restore source order.
    let mut ordered = comments.to_vec();
    ordered.reverse();
    let mut lines = Vec::new();
    for comment in ordered {
        let trimmed = comment.trim();
        if let Some(rest) = trimmed.strip_prefix("//") {
            lines.push(rest.trim().to_string());
        } else if trimmed.starts_with("/*") {
            let inner = trimmed
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim();
            for line in inner.lines() {
                lines.push(line.trim().trim_start_matches('*').trim().to_string());
            }
        }
    }
    let joined = lines.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

/// Go: one query capture (`symbol.gotype`) covers every `type_spec`; the concrete kind
/// is the `type:` child — `struct_type` / `interface_type` / everything-else (defined
/// types like `type UserID int`).
fn go_type_kind(node: Node) -> &'static str {
    match node.child_by_field_name("type").map(|t| t.kind()) {
        Some("struct_type") => "struct",
        Some("interface_type") => "interface",
        _ => "type",
    }
}

/// Go: reduce a receiver `type` node to its base identifier — peel `*` (`pointer_type`),
/// generic args (`generic_type`, e.g. `Box[T]` → `Box`), and package qualification
/// (`qualified_type` → `name`). Returns `None` on any unexpected shape.
fn go_base_type_name(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "type_identifier" => node.utf8_text(source).ok().map(|t| t.to_string()),
        "pointer_type" => {
            // `*T` — the inner type is the single named child.
            for i in 0..node.child_count() {
                let child = node.child(i as u32).unwrap();
                if child.is_named() {
                    return go_base_type_name(child, source);
                }
            }
            None
        }
        "generic_type" => {
            // `Box[T]` — the base is the `type` field.
            go_base_type_name(node.child_by_field_name("type")?, source)
        }
        "qualified_type" => {
            // `pkg.Type` — the local name is the `name` field.
            let name = node.child_by_field_name("name")?;
            name.utf8_text(source).ok().map(|t| t.to_string())
        }
        _ => None,
    }
}

/// Go: read the receiver base type from a `method_declaration` node directly (the receiver
/// lives on the node itself, not an ancestor). `func (s *Server) Start()` → `Server`.
fn go_receiver_owner(method_node: Node, source: &[u8]) -> Option<String> {
    let receiver = method_node.child_by_field_name("receiver")?; // parameter_list
    for i in 0..receiver.child_count() {
        let child = receiver.child(i as u32).unwrap();
        if child.kind() == "parameter_declaration" {
            let type_node = child.child_by_field_name("type")?;
            return go_base_type_name(type_node, source);
        }
    }
    None
}

pub(crate) struct GoSpec;

impl LanguageSpec for GoSpec {
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        get_go_query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(get_go_tags_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn is_import_line(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with("import ") || trimmed.starts_with("import(")
    }

    fn refine_kind(&self, capture_name: &str, node: Node, kind: &'static str) -> &'static str {
        if capture_name == "symbol.gotype" {
            go_type_kind(node)
        } else {
            kind
        }
    }

    fn docstring_anchor<'a>(&self, node: Node<'a>) -> Node<'a> {
        // Go doc comments sit above the outer declaration, not the inner `*_spec` the
        // symbol is captured on.
        if let Some(parent) = node.parent() {
            let pk = parent.kind();
            if pk == "type_declaration" || pk == "const_declaration" || pk == "var_declaration" {
                return parent;
            }
        }
        node
    }

    fn docstring_fallback(
        &self,
        _node: Node,
        _source: &[u8],
        comments: &[String],
    ) -> Option<String> {
        clean_go_doc_comments(comments)
    }

    fn is_test(
        &self,
        _node: Node,
        name: &str,
        _kind: &str,
        file_path: &str,
        _source: &[u8],
        _comments_text: &str,
    ) -> bool {
        super::path_indicates_test(file_path) || go_is_test_name(name)
    }

    fn is_exported(
        &self,
        node: Node,
        name: &str,
        _kind: &str,
        _source: &[u8],
        _exported_names: &std::collections::HashSet<String>,
    ) -> bool {
        go_is_exported(name)
            && !is_inside_function(
                node,
                &["function_declaration", "method_declaration", "func_literal"],
            )
    }

    fn is_deprecated(
        &self,
        _node: Node,
        _source: &[u8],
        docstring: &Option<String>,
        comments_text: &str,
    ) -> bool {
        // Go marks deprecation with a `// Deprecated:` paragraph in the doc comment
        // (gopls/staticcheck convention).
        contains_case_insensitive(comments_text, "deprecated:")
            || docstring
                .as_ref()
                .is_some_and(|d| contains_case_insensitive(d, "deprecated:"))
    }

    fn find_owner(&self, node: Node, ext: &str, source: &[u8]) -> Option<String> {
        // Go methods carry their receiver type on the symbol node itself — read it directly;
        // an ancestor walk alone would miss it (and `method_declaration` is itself a stop node).
        if node.kind() == "method_declaration" {
            return go_receiver_owner(node, source);
        }
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["function_declaration", "method_declaration", "func_literal"]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["interface_type", "type_declaration"]
    }

    fn owner_for_container<'a>(&self, current: Node<'a>, source: &[u8]) -> Option<Option<String>> {
        // Go: an interface method (`method_elem`) is owned by its enclosing `type_spec`.
        if current.kind() == "type_spec" {
            let Some(name) = current.child_by_field_name("name") else {
                return Some(None);
            };
            return Some(name.utf8_text(source).ok().map(|t| t.to_string()));
        }
        None
    }
}
