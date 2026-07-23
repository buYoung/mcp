//! Vue, Astro, and Svelte markup specs.

use std::sync::OnceLock;

use tree_sitter::{Language, Node, Query};

use super::format_support::{
    clean, first_descendant, is_recoverable, named_children, nearest_ancestor,
};
use super::{LanguageSpec, NameDecision};

const QUERY_SOURCE: &str = include_str!("../../queries/component/symbols.scm");

fn vue_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_vue_next::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile Vue query")
    })
}

fn astro_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_astro_next::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile Astro query")
    })
}

fn svelte_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(&tree_sitter_svelte_next::LANGUAGE.into(), QUERY_SOURCE)
            .expect("Failed to compile Svelte query")
    })
}

fn attribute_parts(node: Node<'_>, source: &[u8]) -> Option<(String, Vec<String>)> {
    let name = named_children(node)
        .into_iter()
        .find(|child| child.kind() == "attribute_name")
        .map(|child| clean(child, source))?;
    let value = first_descendant(node, "attribute_value").map(|child| clean(child, source))?;
    Some((name, value.split_whitespace().map(str::to_string).collect()))
}

fn capture_is_valid(capture: &str, node: Node<'_>, source: &[u8]) -> bool {
    is_recoverable(nearest_ancestor(
        node,
        &[
            "element",
            "template_element",
            "script_element",
            "style_element",
        ],
    )) && (capture != "symbol.property"
        || attribute_parts(node, source).is_some_and(|(name, values)| {
            matches!(name.as_str(), "id" | "class") && !values.is_empty()
        }))
}

fn name_for_capture(capture: &str, node: Node<'_>, source: &[u8]) -> Option<NameDecision> {
    let name = if capture == "symbol.type" {
        first_descendant(node, "tag_name").map(|tag| clean(tag, source))?
    } else {
        attribute_parts(node, source)?.1.into_iter().next()?
    };
    Some(NameDecision::Name(name))
}

fn additional_names(capture: &str, node: Node<'_>, source: &[u8]) -> Vec<String> {
    if capture != "symbol.property" {
        return Vec::new();
    }
    attribute_parts(node, source)
        .map(|(_, values)| values.into_iter().skip(1).collect())
        .unwrap_or_default()
}

macro_rules! component_spec {
    ($name:ident, $language_name:literal, $extension:literal, $language:expr, $query:ident) => {
        pub(crate) struct $name;

        impl LanguageSpec for $name {
            fn language_name(&self) -> &'static str {
                $language_name
            }

            fn grammar(&self, _ext: &str) -> Language {
                $language.into()
            }

            fn query(&self, _ext: &str) -> &'static Query {
                $query()
            }

            fn extensions(&self) -> &'static [&'static str] {
                &[$extension]
            }

            fn caller_scan_enabled(&self) -> bool {
                false
            }

            fn capture_is_valid(&self, capture: &str, node: Node<'_>, source: &[u8]) -> bool {
                capture_is_valid(capture, node, source)
            }

            fn refine_kind(
                &self,
                capture: &str,
                _node: Node<'_>,
                _kind: &'static str,
            ) -> &'static str {
                if capture == "symbol.type" {
                    "tag"
                } else {
                    "attribute"
                }
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
                name_for_capture(capture, node, source)
            }

            fn additional_symbol_names_for_capture(
                &self,
                capture: &str,
                node: Node<'_>,
                source: &[u8],
                _primary_name: &str,
            ) -> Vec<String> {
                additional_names(capture, node, source)
            }
        }
    };
}

component_spec!(
    VueSpec,
    "vue",
    "vue",
    tree_sitter_vue_next::LANGUAGE,
    vue_query
);
component_spec!(
    AstroSpec,
    "astro",
    "astro",
    tree_sitter_astro_next::LANGUAGE,
    astro_query
);
component_spec!(
    SvelteSpec,
    "svelte",
    "svelte",
    tree_sitter_svelte_next::LANGUAGE,
    svelte_query
);
