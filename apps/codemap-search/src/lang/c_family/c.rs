//! C language spec: query, compiled query, and the C-specific extraction hooks. Shared
//! C/C++ helpers live in the parent [`super`] module.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::super::{generic_find_owner, is_inside_function, LanguageSpec, NameDecision};
use super::{c_has_static_storage, name_for_cfn};

// C: node kinds verified against tree-sitter-c 0.24.2 node-types.json and an empirical
// parse-tree dump. Key shapes confirmed:
//   - function_definition → declarator field is function_declarator (identifier child) OR
//     pointer_declarator → function_declarator (identifier child).
//   - declaration with function_declarator child → function prototype in a header.
//   - struct_specifier / union_specifier / enum_specifier carry a `name` field (type_identifier).
//   - enumerator carries a `name` field (identifier).
//   - type_definition carries a `declarator` field (type_identifier for simple typedef).
//   - preproc_def / preproc_function_def carry a `name` field (identifier).
//   - storage_class_specifier is a direct child of function_definition / declaration with text "static".
//   - preceding comment nodes have kind "comment" (covers both `//` and `/* */`).
const C_QUERY_STR: &str = r#"
;; Function definitions — name is the innermost identifier inside the declarator chain.
;; The `@symbol.cfn` capture signals the C-specific extract arm to do the name walk.
(function_definition) @symbol.cfn

;; Function prototypes in headers: `declaration` whose declarator contains a
;; function_declarator. Same `@symbol.cfn` arm handles the name extraction.
(declaration
  declarator: (function_declarator)) @symbol.cfn

;; Structs, unions, enums with a name (skip anonymous: no `name` field).
(struct_specifier
  name: (type_identifier) @symbol.name) @symbol.struct

(union_specifier
  name: (type_identifier) @symbol.name) @symbol.struct

(enum_specifier
  name: (type_identifier) @symbol.name) @symbol.enum

;; Enum constants (enumerators).
(enumerator
  name: (identifier) @symbol.name) @symbol.variant

;; typedef — simple alias: `typedef struct {...} Point` has type_identifier as declarator.
(type_definition
  declarator: (type_identifier) @symbol.name) @symbol.type

;; typedef function-pointer: `typedef int (*cb)(void)` has function_declarator as declarator.
;; The @symbol.cfn arm walks the declarator chain to dig out the type_identifier name.
(type_definition
  declarator: (function_declarator)) @symbol.cfn

;; Object-like macros (#define NAME value) → const.
(preproc_def
  name: (identifier) @symbol.name) @symbol.const

;; Function-like macros (#define NAME(args) body) → fn.
(preproc_function_def
  name: (identifier) @symbol.name) @symbol.fn

;; String literals for BM25 index.
(string_literal) @literal.string
"#;

fn get_c_query() -> &'static Query {
    static C_QUERY: OnceLock<Query> = OnceLock::new();
    C_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_c::LANGUAGE.into(), C_QUERY_STR).expect("Failed to compile C query")
    })
}

pub(crate) struct CSpec;

impl LanguageSpec for CSpec {
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_c::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        get_c_query()
    }

    fn extensions(&self) -> &'static [&'static str] {
        // `.h` is served by the C++ grammar (tolerant of plain C), so it lives on CppSpec.
        &["c"]
    }

    fn is_import_line(&self, line: &str) -> bool {
        // C/C++: `#include` is the only import-equivalent construct.
        line.trim_start().starts_with("#include")
    }

    fn name_for_capture(
        &self,
        capture_name: &str,
        node: Node,
        kind: &str,
        _ext: &str,
        source: &[u8],
        _asm_meta_kind_text: &Option<String>,
    ) -> Option<NameDecision> {
        name_for_cfn(capture_name, node, kind, source)
    }

    fn is_test(
        &self,
        _node: Node,
        _name: &str,
        _kind: &str,
        file_path: &str,
        _source: &[u8],
        _comments_text: &str,
    ) -> bool {
        // C/C++/ASM: use path-only test detection (no framework-specific attribute parsing).
        super::super::path_indicates_test(file_path)
    }

    fn is_exported(
        &self,
        node: Node,
        _name: &str,
        kind: &str,
        source: &[u8],
        _exported_names: &std::collections::HashSet<String>,
    ) -> bool {
        // C: a function/declaration carrying `static` is file-local.
        // Macros (#define) are always exported (no static equivalent).
        // Everything else at file scope is exported by default.
        if kind == "fn" && !is_inside_function(node, &["function_definition"]) {
            !c_has_static_storage(node, source)
        } else {
            !is_inside_function(node, &["function_definition"])
        }
    }

    fn find_owner(&self, node: Node, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        // C: function_definition is the only lexical scope that nests declarations.
        &["function_definition"]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        // C: struct/union/enum specifiers are the only named type containers.
        &["struct_specifier", "union_specifier", "enum_specifier"]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        // C: field_declaration_list is the body of struct/union (transparent to ownership);
        // enumerator_list is the body of an enum (transparent — enumerators owned by enum).
        &["field_declaration_list", "enumerator_list"]
    }

    // is_deprecated uses the trait default (docstring `@deprecated` marker).
}
