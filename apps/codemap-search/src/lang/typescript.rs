//! TypeScript/JavaScript language spec: one spec serving four extensions (ts/tsx/js/jsx)
//! across two grammars (`LANGUAGE_TYPESCRIPT` for ts/js, `LANGUAGE_TSX` for tsx/jsx).

use std::collections::HashSet;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::LanguageSpec;

const TS_QUERY_STR: &str = r#"
;; Classes
(class_declaration
  name: (type_identifier) @symbol.name) @symbol.class

;; Functions
(function_declaration
  name: (identifier) @symbol.name) @symbol.fn

;; Methods & Constructor
(method_definition
  name: [
    (property_identifier)
    (private_property_identifier)
  ] @symbol.name) @symbol.method

;; Interfaces
(interface_declaration
  name: (type_identifier) @symbol.name) @symbol.interface

;; Type Aliases
(type_alias_declaration
  name: (type_identifier) @symbol.name) @symbol.type

;; Enums
(enum_declaration
  name: (identifier) @symbol.name) @symbol.enum

;; Variables
(variable_declarator
  name: (identifier) @symbol.name) @symbol.variable

;; Test Call Expressions
(call_expression
  function: [
    (identifier) @fn_name
    (member_expression
      object: (identifier) @fn_name)
  ]
  arguments: (arguments
    [
      (string) @symbol.name
      (template_string) @symbol.name
    ]
  )
) @symbol.test

;; Literals
(string) @literal.string
(template_string) @literal.string
(number) @literal.number
[(true) (false)] @literal.boolean
(null) @literal.null
(undefined) @literal.undefined
"#;

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

fn collect_ts_exported_names(node: Node, source: &[u8], exported_names: &mut HashSet<String>) {
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
            collect_ts_exported_names(node.child(i as u32).unwrap(), source, exported_names);
        }
    } else {
        for i in 0..node.child_count() {
            collect_ts_exported_names(node.child(i as u32).unwrap(), source, exported_names);
        }
    }
}

pub(crate) struct TypeScriptSpec;

impl LanguageSpec for TypeScriptSpec {
    fn grammar(&self, ext: &str) -> Language {
        match ext {
            "tsx" | "jsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
            // "ts" | "js"
            _ => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }
    }

    fn query(&self, ext: &str) -> &'static Query {
        match ext {
            "tsx" | "jsx" => get_tsx_query(),
            // "ts" | "js"
            _ => get_ts_query(),
        }
    }

    fn collect_exported_names(&self, root: Node, source: &[u8], out: &mut HashSet<String>) {
        collect_ts_exported_names(root, source, out);
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
            "class", // class expression: `const X = class { ... }`
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
