//! Python language spec: query, compiled query, and the Python-specific extraction hooks.

use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

use super::{contains_case_insensitive, generic_find_owner, strip_quotes, LanguageSpec};

const PYTHON_QUERY_STR: &str = r#"
;; Class Definitions
(class_definition
  name: (identifier) @symbol.name) @symbol.class

;; Function and Method Definitions
(function_definition
  name: (identifier) @symbol.name) @symbol.fn

;; Assignments (Variables)
(assignment
  left: (identifier) @symbol.name) @symbol.variable

;; Literals
(string) @literal.string
(integer) @literal.number
(float) @literal.number
[(true) (false)] @literal.boolean
"#;

fn get_python_query() -> &'static Query {
    static PYTHON_QUERY: OnceLock<Query> = OnceLock::new();
    PYTHON_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_python::LANGUAGE.into(), PYTHON_QUERY_STR)
            .expect("Failed to compile Python query")
    })
}

fn clean_python_string(text: &str) -> String {
    strip_quotes(text)
}

fn get_python_inline_docstring(node: Node, source: &[u8]) -> Option<String> {
    if let Some(body) = node.child_by_field_name("body") {
        if body.kind() == "block" && body.child_count() > 0 {
            let first_stmt = body.child(0).unwrap();
            if first_stmt.kind() == "expression_statement" && first_stmt.child_count() > 0 {
                let expr = first_stmt.child(0).unwrap();
                if expr.kind() == "string" {
                    if let Ok(text) = expr.utf8_text(source) {
                        return Some(clean_python_string(text));
                    }
                }
            } else if first_stmt.kind() == "string" {
                if let Ok(text) = first_stmt.utf8_text(source) {
                    return Some(clean_python_string(text));
                }
            }
        }
    }
    None
}

fn has_ancestor_fn(node: Node) -> bool {
    let mut curr = node.parent();
    while let Some(n) = curr {
        if n.kind() == "function_definition" {
            return true;
        }
        curr = n.parent();
    }
    false
}

pub(crate) struct PythonSpec;

impl LanguageSpec for PythonSpec {
    fn grammar(&self, _ext: &str) -> Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn query(&self, _ext: &str) -> &'static Query {
        get_python_query()
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn is_import_line(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with("import ") || trimmed.starts_with("from ")
    }

    fn docstring_anchor<'a>(&self, node: Node<'a>) -> Node<'a> {
        if let Some(parent) = node.parent() {
            if parent.kind() == "decorated_definition" {
                return parent;
            }
        }
        node
    }

    fn docstring_fallback(
        &self,
        node: Node,
        source: &[u8],
        _comments: &[String],
    ) -> Option<String> {
        get_python_inline_docstring(node, source)
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
        let name_starts_test_normalized = name.to_lowercase().starts_with("test");
        let has_test_decorator = if let Some(parent) = node.parent() {
            if parent.kind() == "decorated_definition" {
                let mut found = false;
                for i in 0..parent.child_count() {
                    let child = parent.child(i as u32).unwrap();
                    if child.kind() == "decorator" {
                        if let Ok(text) = child.utf8_text(source) {
                            if text.contains("test") || text.contains("pytest") {
                                found = true;
                                break;
                            }
                        }
                    }
                }
                found
            } else {
                false
            }
        } else {
            false
        };
        super::path_indicates_test(file_path) || name_starts_test_normalized || has_test_decorator
    }

    fn is_exported(
        &self,
        node: Node,
        name: &str,
        _kind: &str,
        _source: &[u8],
        _exported_names: &std::collections::HashSet<String>,
    ) -> bool {
        !name.starts_with('_') && !has_ancestor_fn(node)
    }

    fn is_deprecated(
        &self,
        node: Node,
        source: &[u8],
        docstring: &Option<String>,
        _comments_text: &str,
    ) -> bool {
        let has_deprecated_decorator = if let Some(parent) = node.parent() {
            if parent.kind() == "decorated_definition" {
                let mut found = false;
                for i in 0..parent.child_count() {
                    let child = parent.child(i as u32).unwrap();
                    if child.kind() == "decorator" {
                        if let Ok(text) = child.utf8_text(source) {
                            if text.contains("deprecated") {
                                found = true;
                                break;
                            }
                        }
                    }
                }
                found
            } else {
                false
            }
        } else {
            false
        };
        let docstring_contains_deprecated = docstring
            .as_ref()
            .is_some_and(|d| contains_case_insensitive(d, "deprecated"));
        has_deprecated_decorator || docstring_contains_deprecated
    }

    fn find_owner(&self, node: Node, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["function_definition", "lambda"]
    }

    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["class_definition"]
    }

    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &["block"]
    }
}
