//! TypeScript language spec: TypeScript and TSX grammars serving ts/mts/cts and tsx.

use std::collections::HashSet;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{LanguageSpec, NameDecision};

const TS_QUERY_STR: &str = concat!(
    include_str!("../../queries/typescript/symbols.scm"),
    "\n",
    include_str!("../../queries/typescript/navigation.scm")
);

const TS_TAGS_QUERY_STR: &str = include_str!("../../queries/typescript/tags.scm");
const TS_STATIC_COLLECTION_QUERY_STR: &str =
    include_str!("../../queries/typescript/static_collection_edges.scm");

fn get_ts_query() -> &'static Query {
    static TS_QUERY: OnceLock<Query> = OnceLock::new();
    TS_QUERY.get_or_init(|| {
        Query::new(
            &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            TS_QUERY_STR,
        )
        .expect("Failed to compile TS query")
    })
}

fn get_tsx_query() -> &'static Query {
    static TSX_QUERY: OnceLock<Query> = OnceLock::new();
    TSX_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_typescript::LANGUAGE_TSX.into(), TS_QUERY_STR)
            .expect("Failed to compile TSX query")
    })
}

fn get_ts_tags_query() -> &'static Query {
    static TS_TAGS_QUERY: OnceLock<Query> = OnceLock::new();
    TS_TAGS_QUERY.get_or_init(|| {
        Query::new(
            &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            TS_TAGS_QUERY_STR,
        )
        .expect("Failed to compile TS tags query")
    })
}

fn get_tsx_tags_query() -> &'static Query {
    static TSX_TAGS_QUERY: OnceLock<Query> = OnceLock::new();
    TSX_TAGS_QUERY.get_or_init(|| {
        Query::new(
            &tree_sitter_typescript::LANGUAGE_TSX.into(),
            TS_TAGS_QUERY_STR,
        )
        .expect("Failed to compile TSX tags query")
    })
}

fn get_ts_static_collection_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(
            &tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            TS_STATIC_COLLECTION_QUERY_STR,
        )
        .expect("Failed to compile TypeScript static collection query")
    })
}

fn get_tsx_static_collection_query() -> &'static Query {
    static QUERY: OnceLock<Query> = OnceLock::new();
    QUERY.get_or_init(|| {
        Query::new(
            &tree_sitter_typescript::LANGUAGE_TSX.into(),
            TS_STATIC_COLLECTION_QUERY_STR,
        )
        .expect("Failed to compile TSX static collection query")
    })
}

pub(super) fn collect_ecmascript_exported_names(
    node: Node,
    source: &[u8],
    exported_names: &mut HashSet<String>,
) {
    let kind = node.kind();
    if kind == "export_specifier" {
        if let Some(name_node) = node.child_by_field_name("name") {
            if let Ok(name_str) = name_node.utf8_text(source) {
                exported_names.insert(name_str.to_string());
            }
        } else {
            for i in 0..node.child_count() {
                let child = node.child(i as u32).unwrap();
                if child.kind() == "identifier" {
                    if let Ok(name_str) = child.utf8_text(source) {
                        exported_names.insert(name_str.to_string());
                        break;
                    }
                }
            }
        }
    } else if kind == "export_statement" {
        let mut has_default = false;
        for i in 0..node.child_count() {
            let child = node.child(i as u32).unwrap();
            if child.kind() == "default" {
                has_default = true;
            } else if has_default && child.kind() == "identifier" {
                if let Ok(name_str) = child.utf8_text(source) {
                    exported_names.insert(name_str.to_string());
                }
            }
        }
        for i in 0..node.child_count() {
            collect_ecmascript_exported_names(
                node.child(i as u32).unwrap(),
                source,
                exported_names,
            );
        }
    } else {
        for i in 0..node.child_count() {
            collect_ecmascript_exported_names(
                node.child(i as u32).unwrap(),
                source,
                exported_names,
            );
        }
    }
}

/// True only for variables declared directly in a file or TypeScript namespace/module.
/// Block- and function-local declarations remain navigation bindings instead of polluting
/// the repository-wide symbol field.
pub(super) fn is_ecmascript_module_variable(node: Node) -> bool {
    let mut ancestor = node.parent();
    while let Some(current) = ancestor {
        match current.kind() {
            "program" => return true,
            "lexical_declaration"
            | "variable_declaration"
            | "export_statement"
            | "ambient_declaration"
            | "internal_module"
            | "module"
            | "namespace"
            | "expression_statement" => {}
            "statement_block" => {
                let is_module_body = current.parent().is_some_and(|parent| {
                    matches!(parent.kind(), "internal_module" | "module" | "namespace")
                });
                if !is_module_body {
                    return false;
                }
            }
            _ => return false,
        }
        ancestor = current.parent();
    }
    false
}

pub(crate) struct TypeScriptSpec;

impl LanguageSpec for TypeScriptSpec {
    fn language_name(&self) -> &'static str {
        "typescript"
    }

    fn grammar(&self, ext: &str) -> Language {
        match ext {
            "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
            // "ts" | "mts" | "cts"
            _ => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }
    }

    fn query(&self, ext: &str) -> &'static Query {
        match ext {
            "tsx" => get_tsx_query(),
            // "ts" | "mts" | "cts"
            _ => get_ts_query(),
        }
    }

    fn tags_query(&self, ext: &str) -> Option<&'static Query> {
        Some(match ext {
            "tsx" => get_tsx_tags_query(),
            _ => get_ts_tags_query(),
        })
    }

    fn static_collection_query(&self, ext: &str) -> Option<&'static Query> {
        Some(match ext {
            "tsx" => get_tsx_static_collection_query(),
            _ => get_ts_static_collection_query(),
        })
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx", "mts", "cts"]
    }

    fn navigation_enabled(&self, _ext: &str) -> bool {
        true
    }

    // `is_import_line` and `qualified_name_separator` use the trait defaults (the
    // TypeScript-family `import` / `require(` rule and the `.` separator) verbatim.

    fn collect_exported_names(&self, root: Node, source: &[u8], out: &mut HashSet<String>) {
        collect_ecmascript_exported_names(root, source, out);
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
        if capture_name == "symbol.variable" && !is_ecmascript_module_variable(node) {
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

    // is_test / is_exported / is_deprecated all use the trait defaults (the TypeScript-family
    // behavior the defaults were copied from). `find_owner` uses the trait default, which
    // delegates to the generic ancestor-walk over the owner-kind tables below.

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            // function/closure/lambda scopes
            "function_declaration",
            "function_expression",
            "arrow_function",
            "method_definition",
            "generator_function",
            "generator_function_declaration",
            "class_static_block",
            // anonymous-type / object-value bodies (a value, not a named type)
            "class",  // class expression: `const X = class { ... }`
            "object", // object literal: `{ handler() {} }`
        ]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["class_declaration", "abstract_class_declaration"]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "class_body",
            "statement_block",
            "internal_module",
            "module",
            "namespace",
        ]
    }
}
