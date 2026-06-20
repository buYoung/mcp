//! Java language spec: query, compiled query, and the Java-specific extraction hooks.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{generic_find_owner, has_annotation, LanguageSpec};

const JAVA_QUERY_STR: &str = r#"
;; Type declarations
(class_declaration
  name: (identifier) @symbol.name) @symbol.class
(interface_declaration
  name: (identifier) @symbol.name) @symbol.interface
(enum_declaration
  name: (identifier) @symbol.name) @symbol.enum
(record_declaration
  name: (identifier) @symbol.name) @symbol.record

;; Enum constants
(enum_constant
  name: (identifier) @symbol.name) @symbol.variant

;; Methods and constructors
(method_declaration
  name: (identifier) @symbol.name) @symbol.method
(constructor_declaration
  name: (identifier) @symbol.name) @symbol.method

;; Fields
(field_declaration
  declarator: (variable_declarator
    name: (identifier) @symbol.name)) @symbol.field

;; Literals
(string_literal) @literal.string
"#;

fn get_java_query() -> &'static Query {
    static JAVA_QUERY: OnceLock<Query> = OnceLock::new();
    JAVA_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_java::LANGUAGE.into(), JAVA_QUERY_STR)
            .expect("Failed to compile Java query")
    })
}

/// Java: a declaration is part of the public API iff its `modifiers` include `public`.
/// (Java defaults to package-private, so visibility must be explicit.)
fn java_is_public(node: Node) -> bool {
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == "modifiers" {
            for j in 0..child.child_count() {
                if child.child(j as u32).unwrap().kind() == "public" {
                    return true;
                }
            }
        }
    }
    false
}

/// Java test detection: a `@Test` annotation, or a conventional test-class filename
/// (`*Test.java` / `*Tests.java` / `*IT.java`). The `src/test/...` directory is already
/// caught by [`super::path_indicates_test`].
fn java_is_test(node: Node, file_path: &str, source: &[u8]) -> bool {
    if has_annotation(node, "Test", source) {
        return true;
    }
    let file = file_path.rsplit(['/', '\\']).next().unwrap_or(file_path);
    file.ends_with("Test.java") || file.ends_with("Tests.java") || file.ends_with("IT.java")
}

pub(crate) struct JavaSpec;

impl LanguageSpec for JavaSpec {
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_java::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        get_java_query()
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn is_import_line(&self, line: &str) -> bool {
        line.trim_start().starts_with("import ")
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
        super::path_indicates_test(file_path) || java_is_test(node, file_path, source)
    }

    fn is_exported(
        &self,
        node: Node,
        _name: &str,
        _kind: &str,
        _source: &[u8],
        _exported_names: &std::collections::HashSet<String>,
    ) -> bool {
        java_is_public(node)
    }

    fn is_deprecated(
        &self,
        node: Node,
        source: &[u8],
        docstring: &Option<String>,
        _comments_text: &str,
    ) -> bool {
        has_annotation(node, "Deprecated", source)
            || docstring
                .as_ref()
                .is_some_and(|d| d.contains("@deprecated"))
    }

    fn find_owner(&self, node: Node, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "method_declaration",
            "constructor_declaration",
            "lambda_expression",
            // anonymous class body: `new Runnable() { ... }`
            "object_creation_expression",
        ]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
        ]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[
            "class_body",
            "interface_body",
            "enum_body",
            "enum_body_declarations",
            "block",
        ]
    }
}
