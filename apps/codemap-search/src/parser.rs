use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::{Node, Parser, Query, QueryCursor};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeRange {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolFlags {
    pub has_todo: bool,
    pub has_fixme: bool,
    pub is_test: bool,
    pub is_exported: bool,
    pub is_deprecated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedSymbol {
    pub name: String,
    pub kind: String,
    pub range: CodeRange,
    pub docstring: Option<String>,
    pub flags: SymbolFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedFile {
    pub file_path: String,
    pub symbols: Vec<ExtractedSymbol>,
    pub literals: Vec<String>,
    pub docstrings: Vec<String>,
}

pub trait CodeExtractor {
    fn extract(&self, file_content: &str, file_path: &str) -> Result<ExtractedFile, String>;
}

pub struct TreeSitterExtractor;

impl Default for TreeSitterExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeSitterExtractor {
    pub fn new() -> Self {
        Self
    }
}

const RUST_QUERY_STR: &str = r#"
;; Structs
(struct_item
  name: (type_identifier) @symbol.name) @symbol.struct

;; Enums
(enum_item
  name: (type_identifier) @symbol.name) @symbol.enum

;; Traits
(trait_item
  name: (type_identifier) @symbol.name) @symbol.trait

;; Modules
(mod_item
  name: (identifier) @symbol.name) @symbol.mod

;; Functions and Methods
(function_item
  name: (identifier) @symbol.name) @symbol.fn

;; Type Aliases
(type_item
  name: (type_identifier) @symbol.name) @symbol.type

;; Constants
(const_item
  name: (identifier) @symbol.name) @symbol.const

;; Statics
(static_item
  name: (identifier) @symbol.name) @symbol.static

;; Struct Fields
(field_declaration
  name: (field_identifier) @symbol.name) @symbol.field

;; Literals
(string_literal) @literal.string
(raw_string_literal) @literal.string
(integer_literal) @literal.number
(float_literal) @literal.number
(boolean_literal) @literal.boolean
"#;

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

fn get_rust_query() -> &'static Query {
    static RUST_QUERY: OnceLock<Query> = OnceLock::new();
    RUST_QUERY.get_or_init(|| {
        Query::new(tree_sitter_rust::language(), RUST_QUERY_STR)
            .expect("Failed to compile Rust query")
    })
}

fn get_python_query() -> &'static Query {
    static PYTHON_QUERY: OnceLock<Query> = OnceLock::new();
    PYTHON_QUERY.get_or_init(|| {
        Query::new(tree_sitter_python::language(), PYTHON_QUERY_STR)
            .expect("Failed to compile Python query")
    })
}

fn get_ts_query() -> &'static Query {
    static TS_QUERY: OnceLock<Query> = OnceLock::new();
    TS_QUERY.get_or_init(|| {
        Query::new(tree_sitter_typescript::language_typescript(), TS_QUERY_STR)
            .expect("Failed to compile TS query")
    })
}

fn get_tsx_query() -> &'static Query {
    static TSX_QUERY: OnceLock<Query> = OnceLock::new();
    TSX_QUERY.get_or_init(|| {
        Query::new(tree_sitter_typescript::language_tsx(), TS_QUERY_STR)
            .expect("Failed to compile TSX query")
    })
}

fn contains_case_insensitive(text: &str, pattern: &str) -> bool {
    text.to_ascii_lowercase()
        .contains(&pattern.to_ascii_lowercase())
}

fn strip_rust_raw_string(s: &str) -> Option<String> {
    if !s.starts_with('r') {
        return None;
    }
    let mut hash_count = 0;
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 2 {
        return None;
    }
    let mut idx = 1;
    while idx < chars.len() && chars[idx] == '#' {
        hash_count += 1;
        idx += 1;
    }
    if idx < chars.len() && chars[idx] == '"' {
        let start_len = 1 + hash_count + 1;
        if s.len() >= start_len * 2 - 1 {
            let suffix_starts = s.len() - (1 + hash_count);
            if chars[suffix_starts] == '"' {
                let mut valid_end = true;
                for i in 0..hash_count {
                    if chars[suffix_starts + 1 + i] != '#' {
                        valid_end = false;
                        break;
                    }
                }
                if valid_end {
                    return Some(s[start_len..suffix_starts].to_string());
                }
            }
        }
    }
    None
}

fn strip_quotes(s: &str) -> String {
    let trimmed = s.trim();
    if let Some(stripped) = strip_rust_raw_string(trimmed) {
        return stripped;
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let mut quote_idx = 0;
    while quote_idx < chars.len() {
        let c = chars[quote_idx];
        if c == '"' || c == '\'' || c == '`' {
            break;
        }
        let c_lower = c.to_ascii_lowercase();
        if c_lower == 'r' || c_lower == 'f' || c_lower == 'b' || c_lower == 'u' {
            quote_idx += 1;
        } else {
            break;
        }
    }
    let mut s_stripped = if quote_idx > 0 && quote_idx < chars.len() {
        &trimmed[quote_idx..]
    } else {
        trimmed
    };
    // The `"""` and `'''` branches strip 3 chars identically by design; merging them is
    // extraction semantics frozen by the packaging brief (Child 03 owns this), so suppress
    // rather than refactor.
    #[allow(clippy::if_same_then_else)]
    if s_stripped.starts_with("\"\"\"") && s_stripped.ends_with("\"\"\"") && s_stripped.len() >= 6 {
        s_stripped = &s_stripped[3..s_stripped.len() - 3];
    } else if s_stripped.starts_with("'''") && s_stripped.ends_with("'''") && s_stripped.len() >= 6
    {
        s_stripped = &s_stripped[3..s_stripped.len() - 3];
    } else if ((s_stripped.starts_with('"') && s_stripped.ends_with('"'))
        || (s_stripped.starts_with('\'') && s_stripped.ends_with('\''))
        || (s_stripped.starts_with('`') && s_stripped.ends_with('`')))
        && s_stripped.len() >= 2
    {
        s_stripped = &s_stripped[1..s_stripped.len() - 1];
    }
    s_stripped.to_string()
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

fn clean_docstring(comments: &[String]) -> Option<String> {
    if comments.is_empty() {
        return None;
    }
    let mut cleaned_lines = Vec::new();
    let mut comments_ordered = comments.to_vec();
    comments_ordered.reverse();

    // Only doc-comments become docstrings. Plain `//` / `#` line comments and
    // non-doc `/* */` blocks are NOT promoted (Child 03 — they leaked before).
    // Python `"""` docstrings are handled separately by get_python_inline_docstring.
    for comment in comments_ordered {
        let trimmed = comment.trim();
        if trimmed.starts_with("///") {
            cleaned_lines.push(trimmed.trim_start_matches("///").trim().to_string());
        } else if trimmed.starts_with("//!") {
            cleaned_lines.push(trimmed.trim_start_matches("//!").trim().to_string());
        } else if trimmed.starts_with("/**") {
            // rustdoc / JSDoc block comment.
            let content = trimmed
                .trim_start_matches("/**")
                .trim_end_matches("*/")
                .trim();
            for line in content.lines() {
                let line_trimmed = line
                    .trim()
                    .trim_start_matches('*')
                    .trim()
                    .trim_end_matches('*')
                    .trim();
                cleaned_lines.push(line_trimmed.to_string());
            }
        }
        // Anything else (plain `//`, `#`, non-doc `/* */`) is intentionally skipped.
    }

    let joined = cleaned_lines.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

/// Does the path indicate a test file, using segment/suffix boundaries rather than a
/// raw `contains("test")` (which false-matches `attestation.rs`, `latest.rs`, `contest.rs`).
fn path_indicates_test(file_path: &str) -> bool {
    let path = file_path.to_lowercase();
    let file_name = path.rsplit(['/', '\\']).next().unwrap_or(&path);
    path.contains("/tests/")
        || path.starts_with("tests/")
        || path.contains("/test/")
        || path.starts_with("test/")
        || file_name.starts_with("test_")
        || file_name.contains("_test.")
        || file_name.contains(".test.")
        || file_name.contains("_spec.")
        || file_name.contains(".spec.")
}

fn has_rust_attribute_containing(node: Node, sub: &str, source: &[u8]) -> bool {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
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

fn has_ancestor_export(node: Node) -> bool {
    let mut curr = node.parent();
    for _ in 0..3 {
        if let Some(n) = curr {
            let k = n.kind();
            if k == "export_statement" {
                return true;
            }
            curr = n.parent();
        } else {
            break;
        }
    }
    false
}

fn collect_ts_exported_names(
    node: Node,
    source: &[u8],
    exported_names: &mut std::collections::HashSet<String>,
) {
    let kind = node.kind();
    if kind == "export_specifier" {
        if let Some(name_node) = node.child_by_field_name("name") {
            if let Ok(name_str) = name_node.utf8_text(source) {
                exported_names.insert(name_str.to_string());
            }
        } else {
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
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
            let child = node.child(i).unwrap();
            if child.kind() == "default" {
                has_default = true;
            } else if has_default && child.kind() == "identifier" {
                if let Ok(name_str) = child.utf8_text(source) {
                    exported_names.insert(name_str.to_string());
                }
            }
        }
        for i in 0..node.child_count() {
            collect_ts_exported_names(node.child(i).unwrap(), source, exported_names);
        }
    } else {
        for i in 0..node.child_count() {
            collect_ts_exported_names(node.child(i).unwrap(), source, exported_names);
        }
    }
}

fn find_name(node: Node, source: &[u8]) -> Option<String> {
    if node.kind() == "identifier"
        || node.kind() == "type_identifier"
        || node.kind() == "field_identifier"
    {
        return Some(node.utf8_text(source).unwrap_or("").to_string());
    }
    if let Some(name_node) = node.child_by_field_name("name") {
        return Some(name_node.utf8_text(source).unwrap_or("").to_string());
    }
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        let k = child.kind();
        if k == "identifier" || k == "type_identifier" || k == "field_identifier" {
            return Some(child.utf8_text(source).unwrap_or("").to_string());
        }
    }
    None
}

impl CodeExtractor for TreeSitterExtractor {
    fn extract(&self, file_content: &str, file_path: &str) -> Result<ExtractedFile, String> {
        let path = Path::new(file_path);
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let mut parser = Parser::new();
        let (lang, query) = match ext {
            "rs" => (tree_sitter_rust::language(), get_rust_query()),
            "py" => (tree_sitter_python::language(), get_python_query()),
            "ts" | "js" => (
                tree_sitter_typescript::language_typescript(),
                get_ts_query(),
            ),
            "tsx" | "jsx" => (tree_sitter_typescript::language_tsx(), get_tsx_query()),
            _ => {
                return Ok(ExtractedFile {
                    file_path: file_path.to_string(),
                    symbols: Vec::new(),
                    literals: Vec::new(),
                    docstrings: Vec::new(),
                });
            }
        };

        parser.set_language(lang).map_err(|e| e.to_string())?;
        let tree = parser
            .parse(file_content, None)
            .ok_or("Failed to parse file content")?;

        let mut symbols = Vec::new();
        let mut literals = Vec::new();
        let source = file_content.as_bytes();

        let mut exported_names = std::collections::HashSet::new();
        if ext == "ts" || ext == "js" || ext == "tsx" || ext == "jsx" {
            collect_ts_exported_names(tree.root_node(), source, &mut exported_names);
        }

        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor.matches(query, tree.root_node(), source);

        for mat in matches {
            let mut main_node: Option<(Node, &str)> = None;
            let mut symbol_name: Option<String> = None;
            let mut is_valid_test_call = true;

            for capture in mat.captures {
                let name_idx = capture.index as usize;
                let name = &*query.capture_names()[name_idx];

                // `symbol.*` and `literal.*` both record `main_node`; the distinguishing
                // capture name is carried in the tuple and branched on below. This is
                // frozen Child 03 extraction routing — suppress, don't merge.
                #[allow(clippy::if_same_then_else)]
                if name.starts_with("symbol.") && name != "symbol.name" {
                    main_node = Some((capture.node, name));
                } else if name.starts_with("literal.") {
                    main_node = Some((capture.node, name));
                } else if name == "symbol.name" {
                    if let Ok(text) = capture.node.utf8_text(source) {
                        symbol_name = Some(text.to_string());
                    }
                } else if name == "fn_name" {
                    if let Ok(text) = capture.node.utf8_text(source) {
                        let t = text.trim();
                        if t != "describe" && t != "it" && t != "test" {
                            is_valid_test_call = false;
                        }
                    }
                }
            }

            if let Some((node, capture_name)) = main_node {
                if capture_name.starts_with("symbol.") {
                    if is_valid_test_call {
                        let kind = match capture_name {
                            "symbol.struct" => "struct",
                            "symbol.enum" => "enum",
                            "symbol.trait" => "trait",
                            "symbol.mod" => "mod",
                            "symbol.fn" | "symbol.method" => "fn",
                            "symbol.type" => "type",
                            "symbol.const" => "const",
                            "symbol.static" => "static",
                            "symbol.field" => "field",
                            "symbol.class" => "class",
                            "symbol.variable" => "variable",
                            "symbol.interface" => "interface",
                            "symbol.test" => "test",
                            _ => "unknown",
                        };

                        let mut name = symbol_name
                            .unwrap_or_else(|| find_name(node, source).unwrap_or_default());

                        if kind == "test" {
                            name = strip_quotes(&name);
                        }

                        if !name.is_empty() {
                            let start = node.start_position();
                            let end = node.end_position();
                            let range = CodeRange {
                                start_line: start.row + 1,
                                start_col: start.column + 1,
                                end_line: end.row + 1,
                                end_col: end.column + 1,
                            };

                            // Associated comments proximity search
                            let mut walk_start_node = node;
                            if ext == "py" {
                                if let Some(parent) = node.parent() {
                                    if parent.kind() == "decorated_definition" {
                                        walk_start_node = parent;
                                    }
                                }
                            } else if ext == "ts" || ext == "js" || ext == "tsx" || ext == "jsx" {
                                if let Some(parent) = node.parent() {
                                    if parent.kind() == "export_statement" {
                                        walk_start_node = parent;
                                    }
                                }
                            }

                            let mut current_sibling = walk_start_node.prev_sibling();
                            let mut comments = Vec::new();
                            let mut last_row = walk_start_node.start_position().row;

                            while let Some(sibling) = current_sibling {
                                let sk = sibling.kind();
                                if sk == "comment" || sk == "line_comment" || sk == "block_comment"
                                {
                                    let end_row = sibling.end_position().row;
                                    if end_row >= last_row - 1 {
                                        if let Ok(text) = sibling.utf8_text(source) {
                                            comments.push(text.to_string());
                                        }
                                        last_row = sibling.start_position().row;
                                        current_sibling = sibling.prev_sibling();
                                    } else {
                                        break;
                                    }
                                } else if sk == "attribute_item" || sk == "decorator" {
                                    last_row = sibling.start_position().row;
                                    current_sibling = sibling.prev_sibling();
                                } else {
                                    break;
                                }
                            }

                            let mut docstring = clean_docstring(&comments);
                            if ext == "py" && docstring.is_none() {
                                docstring = get_python_inline_docstring(node, source);
                            }

                            let node_text = node.utf8_text(source).unwrap_or("");

                            // Preceding comments are no longer promoted into docstrings,
                            // so scan them directly for TODO/FIXME (covers `// TODO` above a
                            // symbol); node_text covers in-body and python `"""` docstrings.
                            let comments_text = comments.join("\n");
                            let has_todo = contains_case_insensitive(node_text, "todo")
                                || contains_case_insensitive(&comments_text, "todo");
                            let has_fixme = contains_case_insensitive(node_text, "fixme")
                                || contains_case_insensitive(&comments_text, "fixme");

                            let is_test = if ext == "rs" {
                                let name_contains_test =
                                    name.starts_with("test_") || name.ends_with("_test");
                                let has_test_attr =
                                    has_rust_attribute_containing(node, "test", source);
                                path_indicates_test(file_path)
                                    || name_contains_test
                                    || has_test_attr
                            } else if ext == "py" {
                                let name_starts_test_normalized =
                                    name.to_lowercase().starts_with("test");
                                let has_test_decorator = if let Some(parent) = node.parent() {
                                    if parent.kind() == "decorated_definition" {
                                        let mut found = false;
                                        for i in 0..parent.child_count() {
                                            let child = parent.child(i).unwrap();
                                            if child.kind() == "decorator" {
                                                if let Ok(text) = child.utf8_text(source) {
                                                    if text.contains("test")
                                                        || text.contains("pytest")
                                                    {
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
                                path_indicates_test(file_path)
                                    || name_starts_test_normalized
                                    || has_test_decorator
                            } else {
                                let is_test_call = kind == "test";
                                path_indicates_test(file_path) || is_test_call
                            };

                            let is_exported = if ext == "rs" {
                                let mut found = false;
                                for i in 0..node.child_count() {
                                    if node.child(i).unwrap().kind() == "visibility_modifier" {
                                        found = true;
                                        break;
                                    }
                                }
                                found
                            } else if ext == "py" {
                                !name.starts_with('_') && !has_ancestor_fn(node)
                            } else {
                                has_ancestor_export(node) || exported_names.contains(&name)
                            };

                            let is_deprecated = if ext == "rs" {
                                let has_deprecated_attr =
                                    has_rust_attribute_containing(node, "deprecated", source);
                                let docstring_contains_deprecated = docstring
                                    .as_ref()
                                    .is_some_and(|d| contains_case_insensitive(d, "deprecated"));
                                has_deprecated_attr || docstring_contains_deprecated
                            } else if ext == "py" {
                                let has_deprecated_decorator = if let Some(parent) = node.parent() {
                                    if parent.kind() == "decorated_definition" {
                                        let mut found = false;
                                        for i in 0..parent.child_count() {
                                            let child = parent.child(i).unwrap();
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
                            } else {
                                docstring
                                    .as_ref()
                                    .is_some_and(|d| d.contains("@deprecated"))
                            };

                            symbols.push(ExtractedSymbol {
                                name,
                                kind: kind.to_string(),
                                range,
                                docstring,
                                flags: SymbolFlags {
                                    has_todo,
                                    has_fixme,
                                    is_test,
                                    is_exported,
                                    is_deprecated,
                                },
                            });
                        }
                    }
                } else if capture_name.starts_with("literal.string") {
                    // Only string literals carry search/detail value; numeric and boolean
                    // literals are dropped (low value, index/detail bloat — Child 03).
                    if let Ok(text) = node.utf8_text(source) {
                        let stripped = strip_quotes(text);
                        literals.push(stripped);
                    }
                }
            }
        }

        let docstrings = symbols.iter().filter_map(|s| s.docstring.clone()).collect();

        Ok(ExtractedFile {
            file_path: file_path.to_string(),
            symbols,
            literals,
            docstrings,
        })
    }
}

pub fn split_identifier(ident: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = ident.chars().collect();
    let len = chars.len();

    for i in 0..len {
        let c = chars[i];

        if c == '_' || c == '-' {
            if !current.is_empty() {
                tokens.push(current.to_lowercase());
                current.clear();
            }
            continue;
        }

        let prev_is_lowercase = i > 0 && chars[i - 1].is_lowercase();
        let prev_is_digit = i > 0 && chars[i - 1].is_ascii_digit();
        let prev_is_uppercase = i > 0 && chars[i - 1].is_uppercase();

        let current_is_uppercase = c.is_uppercase();

        let next_is_lowercase = i + 1 < len && chars[i + 1].is_lowercase();

        let is_camel_boundary = (prev_is_lowercase || prev_is_digit) && current_is_uppercase;
        let is_acronym_boundary = prev_is_uppercase && current_is_uppercase && next_is_lowercase;
        let is_digit_boundary = prev_is_digit && c.is_alphabetic();

        let prev_is_uncased = i > 0
            && chars[i - 1].is_alphabetic()
            && !chars[i - 1].is_lowercase()
            && !chars[i - 1].is_uppercase();
        let current_is_uncased = c.is_alphabetic() && !c.is_lowercase() && !c.is_uppercase();
        let is_uncased_boundary = i > 0
            && ((prev_is_uncased
                && !current_is_uncased
                && (c.is_alphabetic() || c.is_ascii_digit()))
                || (!prev_is_uncased
                    && (chars[i - 1].is_alphabetic() || chars[i - 1].is_ascii_digit())
                    && current_is_uncased));

        if (is_camel_boundary || is_acronym_boundary || is_digit_boundary || is_uncased_boundary)
            && !current.is_empty()
        {
            tokens.push(current.to_lowercase());
            current.clear();
        }

        current.push(c);
    }

    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Sub-tokenization Tests ---
    #[test]
    fn test_split_identifier_cases() {
        assert_eq!(
            split_identifier("handleLoginError"),
            vec!["handle", "login", "error"]
        );
        assert_eq!(
            split_identifier("handle_login_error"),
            vec!["handle", "login", "error"]
        );
        assert_eq!(
            split_identifier("handle-login-error"),
            vec!["handle", "login", "error"]
        );
        assert_eq!(split_identifier("HTTPClient"), vec!["http", "client"]);
        assert_eq!(split_identifier("v2Engine"), vec!["v2", "engine"]);
        assert_eq!(
            split_identifier("API2026version"),
            vec!["api2026", "version"]
        );
        assert_eq!(
            split_identifier("XMLHttpRequest"),
            vec!["xml", "http", "request"]
        );
        assert_eq!(split_identifier("HTMLElement"), vec!["html", "element"]);
        assert_eq!(split_identifier(""), Vec::<String>::new());
        assert_eq!(split_identifier("a"), vec!["a"]);
    }

    // --- Rust Parser Tests ---
    #[test]
    fn test_rust_parser_struct_and_fields() {
        let content = r#"
            /// Config struct description
            pub struct Config {
                pub port: u16,
            }
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/config.rs").unwrap();

        // Assert struct symbol
        let struct_sym = file.symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(struct_sym.kind, "struct");
        assert!(struct_sym.flags.is_exported);
        assert_eq!(
            struct_sym.docstring.as_deref(),
            Some("Config struct description")
        );

        // Assert field variable symbol (verifies e2e target "port")
        let field_sym = file.symbols.iter().find(|s| s.name == "port").unwrap();
        assert_eq!(field_sym.kind, "field");
        assert!(field_sym.flags.is_exported);
    }

    #[test]
    fn test_rust_parser_flags_deprecated_and_todo() {
        let content = r#"
            // TODO: refactor this
            #[deprecated(since = "1.0.0")]
            fn deprecated_function() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/lib.rs").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "deprecated_function")
            .unwrap();

        assert!(sym.flags.has_todo);
        assert!(sym.flags.is_deprecated);
        assert!(!sym.flags.is_test);
    }

    #[test]
    fn test_rust_parser_test_detection() {
        let content = r#"
            #[test]
            fn my_test_case() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/tests.rs").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "my_test_case")
            .unwrap();

        assert!(sym.flags.is_test);
    }

    // --- Python Parser Tests ---
    #[test]
    fn test_python_parser_class_and_methods() {
        let content = r#"
class Database:
    """Manages db connection."""
    
    def __init__(self, url):
        self.url = url
        
    def query(self, sql):
        # FIXME: handle sql injection
        pass
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "db.py").unwrap();

        let class_sym = file.symbols.iter().find(|s| s.name == "Database").unwrap();
        assert_eq!(class_sym.kind, "class");
        assert_eq!(
            class_sym.docstring.as_deref(),
            Some("Manages db connection.")
        );
        assert!(class_sym.flags.is_exported); // No leading underscore

        let method_sym = file.symbols.iter().find(|s| s.name == "query").unwrap();
        assert_eq!(method_sym.kind, "fn");
        assert!(method_sym.flags.has_fixme);
    }

    #[test]
    fn test_python_private_and_deprecated() {
        let content = r#"
@deprecated
def _private_func():
    pass
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "utils.py").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "_private_func")
            .unwrap();

        assert!(!sym.flags.is_exported); // Starts with underscore
        assert!(sym.flags.is_deprecated);
    }

    // --- TypeScript / JavaScript Parser Tests ---
    #[test]
    fn test_ts_parser_interface_and_exports() {
        let content = r#"
            /** User info interface */
            export interface User {
                id: string;
                name: string;
            }
        "#;
        let extractor = TreeSitterExtractor::new();
        // TS extension: parsed using typescript grammar
        let file = extractor.extract(content, "types.ts").unwrap();

        let sym = file.symbols.iter().find(|s| s.name == "User").unwrap();
        assert_eq!(sym.kind, "interface");
        assert!(sym.flags.is_exported);
        assert_eq!(sym.docstring.as_deref(), Some("User info interface"));
    }

    #[test]
    fn test_tsx_jsx_grammar_selection() {
        let content = r#"
            export function Component() {
                return <div>Hello</div>;
            }
        "#;
        let extractor = TreeSitterExtractor::new();
        // TSX extension: verifies TSX grammar parses JSX syntax successfully without erroring
        let file = extractor.extract(content, "component.tsx").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "Component").unwrap();
        assert_eq!(sym.kind, "fn");
        assert!(sym.flags.is_exported);
    }

    #[test]
    fn test_js_test_suite_hook_detection() {
        let content = r#"
            describe("auth service", () => {
                it("should validate credentials", () => {
                    // TODO: add boundary testing
                });
            });
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "auth.test.js").unwrap();

        let describe_sym = file
            .symbols
            .iter()
            .find(|s| s.name == "auth service")
            .unwrap();
        assert_eq!(describe_sym.kind, "test");
        assert!(describe_sym.flags.is_test);

        let it_sym = file
            .symbols
            .iter()
            .find(|s| s.name == "should validate credentials")
            .unwrap();
        assert_eq!(it_sym.kind, "test");
        assert!(it_sym.flags.is_test);
        assert!(it_sym.flags.has_todo);
    }

    #[test]
    fn test_split_identifier_unicode_boundaries() {
        assert_eq!(split_identifier("한글MyName"), vec!["한글", "my", "name"]);
        assert_eq!(split_identifier("My한글Name"), vec!["my", "한글", "name"]);
        assert_eq!(split_identifier("한글_my_name"), vec!["한글", "my", "name"]);
    }

    #[test]
    fn test_raw_string_literal_quote_stripping() {
        let content = r##"
            pub const VAL: &str = r#"magic_value"#;
        "##;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/lib.rs").unwrap();
        assert_eq!(file.literals, vec!["magic_value"]);
    }

    #[test]
    fn test_ts_named_exports_at_bottom() {
        let content = r#"
            function myFunc() {}
            export { myFunc };
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "index.ts").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "myFunc").unwrap();
        assert!(sym.flags.is_exported);
    }

    #[test]
    fn test_python_class_methods_export_status() {
        let content = r#"
class Calculator:
    def add(self, x, y):
        pass
    def _private_helper(self):
        pass
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "calc.py").unwrap();

        let add_sym = file.symbols.iter().find(|s| s.name == "add").unwrap();
        assert!(add_sym.flags.is_exported);

        let helper_sym = file
            .symbols
            .iter()
            .find(|s| s.name == "_private_helper")
            .unwrap();
        assert!(!helper_sym.flags.is_exported);
    }

    #[test]
    fn test_block_comment_trailing_asterisks() {
        let content = r#"
            /** check function **/
            pub fn check() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "src/lib.rs").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "check").unwrap();
        assert_eq!(sym.docstring.as_deref(), Some("check function"));
    }

    #[test]
    fn test_ts_test_file_pattern_matching() {
        let content = r#"
            export function helper() {}
        "#;
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, "tests/parser_test.ts").unwrap();
        let sym = file.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert!(sym.flags.is_test);
    }
}
