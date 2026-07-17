//! Rust language spec: query, compiled query, and the Rust-specific extraction hooks.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{contains_case_insensitive, generic_find_owner, LanguageSpec};

const RUST_QUERY_STR: &str = concat!(
    include_str!("../../queries/rust/symbols.scm"),
    "\n",
    include_str!("../../queries/rust/navigation.scm")
);

const RUST_TAGS_QUERY_STR: &str = include_str!("../../queries/rust/tags.scm");
const RUST_STATIC_COLLECTION_QUERY_STR: &str =
    include_str!("../../queries/rust/static_collection_edges.scm");

fn get_rust_query() -> &'static Query {
    static RUST_QUERY: OnceLock<Query> = OnceLock::new();
    RUST_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_rust::LANGUAGE.into(), RUST_QUERY_STR)
            .expect("Failed to compile Rust query")
    })
}

fn get_rust_tags_query() -> &'static Query {
    static RUST_TAGS_QUERY: OnceLock<Query> = OnceLock::new();
    RUST_TAGS_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_rust::LANGUAGE.into(), RUST_TAGS_QUERY_STR)
            .expect("Failed to compile Rust tags query")
    })
}

fn get_rust_static_collection_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(
            &tree_sitter_rust::LANGUAGE.into(),
            RUST_STATIC_COLLECTION_QUERY_STR,
        )
        .expect("Failed to compile Rust static collection query")
    })
}

fn has_rust_attribute_containing(node: Node, sub: &str, source: &[u8]) -> bool {
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == "attribute_item" {
            if let Ok(text) = child.utf8_text(source) {
                if text.contains(sub) {
                    return true;
                }
            }
        }
    }
    let mut curr = node.prev_sibling();
    while let Some(sibling) = curr {
        if sibling.kind() == "attribute_item" {
            if let Ok(text) = sibling.utf8_text(source) {
                if text.contains(sub) {
                    return true;
                }
            }
            curr = sibling.prev_sibling();
        } else if sibling.kind() == "comment"
            || sibling.kind() == "line_comment"
            || sibling.kind() == "block_comment"
        {
            curr = sibling.prev_sibling();
        } else {
            break;
        }
    }
    false
}

/// Rust: reduce a type node from `impl_item`'s `type` field to its base identifier —
/// peel `&`/`mut` (`reference_type`), generic args (`generic_type`), and module paths
/// (`scoped_type_identifier` → rightmost `name`). Returns `None` on any unexpected shape.
fn rust_base_type_name(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "type_identifier" => node.utf8_text(source).ok().map(|t| t.to_string()),
        "reference_type" => {
            // `&T` / `&mut T` — the inner type is the `type` field.
            rust_base_type_name(node.child_by_field_name("type")?, source)
        }
        "generic_type" => {
            // `Foo<T>` — the base is the `type` field.
            rust_base_type_name(node.child_by_field_name("type")?, source)
        }
        "scoped_type_identifier" => {
            // `a::b::Foo` — the rightmost segment is the `name` field.
            let name = node.child_by_field_name("name")?;
            name.utf8_text(source).ok().map(|t| t.to_string())
        }
        _ => None,
    }
}

pub(crate) struct RustSpec;

impl LanguageSpec for RustSpec {
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        get_rust_query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(get_rust_tags_query())
    }

    fn static_collection_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(get_rust_static_collection_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn qualified_name_separator(&self) -> &'static str {
        "::"
    }

    fn is_import_line(&self, line: &str) -> bool {
        line.trim_start().starts_with("use ")
    }

    fn is_test(
        &self,
        node: Node,
        name: &str,
        _kind: &str,
        file_path: &str,
        source: &[u8],
        _comments_text: &str,
    ) -> bool {
        let name_contains_test = name.starts_with("test_") || name.ends_with("_test");
        let has_test_attr = has_rust_attribute_containing(node, "test", source);
        super::path_indicates_test(file_path) || name_contains_test || has_test_attr
    }

    fn is_exported(
        &self,
        node: Node,
        _name: &str,
        _kind: &str,
        _source: &[u8],
        _exported_names: &std::collections::HashSet<String>,
    ) -> bool {
        let mut found = false;
        for i in 0..node.child_count() {
            if node.child(i as u32).unwrap().kind() == "visibility_modifier" {
                found = true;
                break;
            }
        }
        found
    }

    fn is_deprecated(
        &self,
        node: Node,
        source: &[u8],
        docstring: &Option<String>,
        _comments_text: &str,
    ) -> bool {
        let has_deprecated_attr = has_rust_attribute_containing(node, "deprecated", source);
        let docstring_contains_deprecated = docstring
            .as_ref()
            .is_some_and(|d| contains_case_insensitive(d, "deprecated"));
        has_deprecated_attr || docstring_contains_deprecated
    }

    fn find_owner(&self, node: Node, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["function_item", "closure_expression"]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["mod_item", "declaration_list", "enum_variant_list"]
    }

    fn owner_for_container<'a>(&self, current: Node<'a>, source: &[u8]) -> Option<Option<String>> {
        // Rust: a method lives in an `impl_item` (read its `type`; for `impl Trait for Type`
        // the `type` field is the implementing `Type`) or a `trait_item` default method; an
        // enum variant is owned by its `enum_item` (reached via the `enum_variant_list`
        // passthrough).
        let kind = current.kind();
        if kind == "impl_item" {
            let Some(type_node) = current.child_by_field_name("type") else {
                return Some(None);
            };
            return Some(rust_base_type_name(type_node, source));
        }
        if kind == "trait_item" || kind == "enum_item" {
            let Some(name) = current.child_by_field_name("name") else {
                return Some(None);
            };
            return Some(name.utf8_text(source).ok().map(|t| t.to_string()));
        }
        None
    }
}
