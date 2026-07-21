//! JavaScript language spec backed by the official JavaScript grammar.

use std::collections::HashSet;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{LanguageSpec, NameDecision};

const JAVASCRIPT_QUERY_STR: &str = concat!(
    include_str!("../../queries/javascript/symbols.scm"),
    "\n",
    include_str!("../../queries/javascript/navigation.scm")
);
const JAVASCRIPT_TAGS_QUERY_STR: &str = include_str!("../../queries/javascript/tags.scm");
const JAVASCRIPT_STATIC_COLLECTION_QUERY_STR: &str =
    include_str!("../../queries/javascript/static_collection_edges.scm");

fn get_javascript_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(
            &tree_sitter_javascript::LANGUAGE.into(),
            JAVASCRIPT_QUERY_STR,
        )
        .expect("Failed to compile JavaScript query")
    })
}

fn get_javascript_tags_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(
            &tree_sitter_javascript::LANGUAGE.into(),
            JAVASCRIPT_TAGS_QUERY_STR,
        )
        .expect("Failed to compile JavaScript tags query")
    })
}

fn get_javascript_static_collection_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(
            &tree_sitter_javascript::LANGUAGE.into(),
            JAVASCRIPT_STATIC_COLLECTION_QUERY_STR,
        )
        .expect("Failed to compile JavaScript static collection query")
    })
}

pub(crate) struct JavaScriptSpec;

impl LanguageSpec for JavaScriptSpec {
    fn language_name(&self) -> &'static str {
        "javascript"
    }

    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_javascript::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        get_javascript_query()
    }

    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(get_javascript_tags_query())
    }

    fn static_collection_query(&self, _ext: &str) -> Option<&'static Query> {
        Some(get_javascript_static_collection_query())
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["js", "jsx", "mjs", "cjs"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    fn collect_exported_names(&self, root: Node, source: &[u8], out: &mut HashSet<String>) {
        super::typescript::collect_ecmascript_exported_names(root, source, out);
    }

    fn name_for_capture(
        &self,
        capture_name: &str,
        node: Node,
        _kind: &str,
        _ext: &str,
        _source: &[u8],
        _asm_meta_kind_text: &Option<String>,
    ) -> Option<NameDecision> {
        if capture_name == "symbol.variable"
            && !super::typescript::is_ecmascript_module_variable(node)
        {
            Some(NameDecision::Skip)
        } else {
            None
        }
    }

    fn refine_kind(&self, capture_name: &str, node: Node, kind: &'static str) -> &'static str {
        if capture_name != "symbol.variable" {
            return kind;
        }
        match node.child_by_field_name("value").map(|value| value.kind()) {
            Some("arrow_function" | "function_expression" | "generator_function") => "fn",
            Some("class") => "class",
            _ => kind,
        }
    }

    fn docstring_anchor<'a>(&self, node: Node<'a>) -> Node<'a> {
        if let Some(parent) = node.parent() {
            if parent.kind() == "export_statement" {
                return parent;
            }
        }
        node
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "function_declaration",
            "function_expression",
            "arrow_function",
            "method_definition",
            "generator_function",
            "generator_function_declaration",
            "class_static_block",
            "class",
            "object",
        ]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["class_body", "statement_block"]
    }
}
