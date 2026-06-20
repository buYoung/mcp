//! Kotlin language spec: query, compiled query, and the Kotlin-specific extraction hooks.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{
    annotation_subtree_contains, generic_find_owner, has_annotation, is_inside_function,
    LanguageSpec,
};

// Kotlin: `class` and `interface` share the `class_declaration` node; the concrete
// kind is resolved in code from the presence of an `interface` keyword child (see the
// `symbol.ktclass` arm).
const KOTLIN_QUERY_STR: &str = r#"
;; Classes / interfaces (disambiguated in code) and objects
(class_declaration
  name: (identifier) @symbol.name) @symbol.ktclass
(object_declaration
  name: (identifier) @symbol.name) @symbol.object

;; Enum entries
(enum_entry
  (identifier) @symbol.name) @symbol.variant

;; Functions
(function_declaration
  name: (identifier) @symbol.name) @symbol.fn

;; Properties
(property_declaration
  (variable_declaration
    (identifier) @symbol.name)) @symbol.property

;; Type aliases
(type_alias
  (identifier) @symbol.name) @symbol.type

;; Literals
(string_literal) @literal.string
"#;

fn get_kotlin_query() -> &'static Query {
    static KOTLIN_QUERY: OnceLock<Query> = OnceLock::new();
    KOTLIN_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_kotlin_ng::LANGUAGE.into(), KOTLIN_QUERY_STR)
            .expect("Failed to compile Kotlin query")
    })
}

/// Kotlin annotation lookup. Like [`super::has_annotation`], but also recovers
/// tree-sitter-kotlin-ng's quirk where a *top-level* annotation carrying arguments
/// (`@Deprecated("msg")`) parses as a detached `annotated_expression` sibling instead of the
/// following declaration's modifier.
fn kotlin_has_annotation(node: Node, target: &str, source: &[u8]) -> bool {
    if has_annotation(node, target, source) {
        return true;
    }
    let mut sibling = node.prev_sibling();
    while let Some(current) = sibling {
        match current.kind() {
            "annotated_expression" | "annotation" => {
                if annotation_subtree_contains(current, target, source) {
                    return true;
                }
                sibling = current.prev_sibling();
            }
            "comment" | "line_comment" | "block_comment" => sibling = current.prev_sibling(),
            _ => break,
        }
    }
    false
}

/// Kotlin: members are public by default, so a declaration is exported unless it carries
/// a `private` / `internal` / `protected` visibility modifier — or is a function-local
/// declaration (local `val`/`var`/`fun`), which is never public API.
fn kotlin_is_exported(node: Node, source: &[u8]) -> bool {
    if is_inside_function(node, &["function_declaration", "anonymous_function"]) {
        return false;
    }
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == "modifiers" {
            for j in 0..child.child_count() {
                let modifier = child.child(j as u32).unwrap();
                if modifier.kind() == "visibility_modifier" {
                    if let Ok(text) = modifier.utf8_text(source) {
                        return text.trim() == "public";
                    }
                }
            }
        }
    }
    true
}

/// Kotlin: `class` and `interface` are both `class_declaration`; disambiguate via the
/// presence of an `interface` keyword child token.
fn kotlin_class_kind(node: Node) -> &'static str {
    for i in 0..node.child_count() {
        if node.child(i as u32).unwrap().kind() == "interface" {
            return "interface";
        }
    }
    "class"
}

pub(crate) struct KotlinSpec;

impl LanguageSpec for KotlinSpec {
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_kotlin_ng::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        get_kotlin_query()
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["kt", "kts"]
    }

    fn is_import_line(&self, line: &str) -> bool {
        line.trim_start().starts_with("import ")
    }

    fn refine_kind(&self, capture_name: &str, node: Node, kind: &'static str) -> &'static str {
        if capture_name == "symbol.ktclass" {
            kotlin_class_kind(node)
        } else {
            kind
        }
    }

    fn is_test(
        &self,
        node: Node,
        _name: &str,
        _kind: &str,
        file_path: &str,
        source: &[u8],
        _comments_text: &str,
    ) -> bool {
        super::path_indicates_test(file_path) || kotlin_has_annotation(node, "Test", source)
    }

    fn is_exported(
        &self,
        node: Node,
        _name: &str,
        _kind: &str,
        source: &[u8],
        _exported_names: &std::collections::HashSet<String>,
    ) -> bool {
        kotlin_is_exported(node, source)
    }

    fn is_deprecated(
        &self,
        node: Node,
        source: &[u8],
        _docstring: &Option<String>,
        _comments_text: &str,
    ) -> bool {
        kotlin_has_annotation(node, "Deprecated", source)
    }

    fn find_owner(&self, node: Node, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "function_declaration",
            "anonymous_function",
            "lambda_literal",
            // anonymous object expression: `object { ... }`
            "object_literal",
        ]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["class_declaration", "object_declaration"]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["class_body", "companion_object", "enum_class_body"]
    }
}
