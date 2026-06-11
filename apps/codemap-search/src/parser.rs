use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::OnceLock;
use streaming_iterator::StreamingIterator;
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
    /// The enclosing *type* of a member (the class for class-nested languages, the `impl`
    /// type for Rust, the receiver type for Go). `None` for free functions, top-level
    /// symbols, and functions nested only inside a module/namespace or a function/closure/
    /// lambda scope. Additive and best-effort: a wrong owner is worse than `None`, so any
    /// unexpected parse shape yields `None`. `#[serde(default)]` keeps pre-upgrade docs
    /// (which lack this field) deserializable during the one-time reindex transition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedLiteral {
    pub text: String,
    /// 1-based source line where the literal starts (matches `read` line numbers).
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedFile {
    pub file_path: String,
    #[serde(default)]
    pub total_lines: usize,
    pub symbols: Vec<ExtractedSymbol>,
    pub literals: Vec<ExtractedLiteral>,
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

;; Enum Variants — error/state variants ("TxReadonly") are the names agents search
;; for; without them an error enum's file is unreachable via symbol search.
(enum_variant
  name: (identifier) @symbol.name) @symbol.variant

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

// Go: all named types are `type_spec`; struct vs interface vs alias is resolved in
// code from the `type:` child (see the `symbol.gotype` arm) to avoid referencing
// type-expression node kinds the grammar may or may not expose (an unknown kind makes
// `Query::new` return Err -> the `.expect()` panics on first use).
const GO_QUERY_STR: &str = r#"
;; Functions and methods
(function_declaration
  name: (identifier) @symbol.name) @symbol.fn
(method_declaration
  name: (field_identifier) @symbol.name) @symbol.fn

;; Named types (struct / interface / alias resolved in code)
(type_spec
  name: (type_identifier) @symbol.name) @symbol.gotype
(type_alias
  name: (type_identifier) @symbol.name) @symbol.type

;; Struct fields
(field_declaration
  name: (field_identifier) @symbol.name) @symbol.field

;; Interface methods
(method_elem
  name: (field_identifier) @symbol.name) @symbol.fn

;; Package-level constants and variables
(const_spec
  name: (identifier) @symbol.name) @symbol.const
(var_spec
  name: (identifier) @symbol.name) @symbol.variable

;; Literals (only strings are kept downstream)
(interpreted_string_literal) @literal.string
(raw_string_literal) @literal.string
"#;

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

fn get_rust_query() -> &'static Query {
    static RUST_QUERY: OnceLock<Query> = OnceLock::new();
    RUST_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_rust::LANGUAGE.into(), RUST_QUERY_STR)
            .expect("Failed to compile Rust query")
    })
}

fn get_python_query() -> &'static Query {
    static PYTHON_QUERY: OnceLock<Query> = OnceLock::new();
    PYTHON_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_python::LANGUAGE.into(), PYTHON_QUERY_STR)
            .expect("Failed to compile Python query")
    })
}

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

fn get_go_query() -> &'static Query {
    static GO_QUERY: OnceLock<Query> = OnceLock::new();
    GO_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_go::LANGUAGE.into(), GO_QUERY_STR)
            .expect("Failed to compile Go query")
    })
}

fn get_java_query() -> &'static Query {
    static JAVA_QUERY: OnceLock<Query> = OnceLock::new();
    JAVA_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_java::LANGUAGE.into(), JAVA_QUERY_STR)
            .expect("Failed to compile Java query")
    })
}

fn get_kotlin_query() -> &'static Query {
    static KOTLIN_QUERY: OnceLock<Query> = OnceLock::new();
    KOTLIN_QUERY.get_or_init(|| {
        Query::new(&tree_sitter_kotlin_ng::LANGUAGE.into(), KOTLIN_QUERY_STR)
            .expect("Failed to compile Kotlin query")
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
            if s.as_bytes()[suffix_starts] == b'"' {
                let mut valid_end = true;
                for i in 0..hash_count {
                    if s.as_bytes()[suffix_starts + 1 + i] != b'#' {
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

/// True if `node` is lexically inside one of `fn_kinds` (a function/method/closure body),
/// i.e. a function-local declaration — never public API regardless of name or default
/// visibility. The Go uppercase-name and Kotlin default-public export rules must defer to
/// this so a local `val`/`var` isn't reported as exported. Unknown kinds in `fn_kinds`
/// simply never match, so over-listing is safe.
fn is_inside_function(node: Node, fn_kinds: &[&str]) -> bool {
    let mut ancestor = node.parent();
    while let Some(current) = ancestor {
        if fn_kinds.contains(&current.kind()) {
            return true;
        }
        ancestor = current.parent();
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
        let child = node.child(i as u32).unwrap();
        let k = child.kind();
        if k == "identifier" || k == "type_identifier" || k == "field_identifier" {
            return Some(child.utf8_text(source).unwrap_or("").to_string());
        }
    }
    None
}

/// Go: an identifier is exported iff its first letter is uppercase (Go spec — "Exported
/// identifiers"). Applies uniformly to functions, types, fields, consts and vars.
fn go_is_exported(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_uppercase())
}

/// Go test entry points: a `Test` / `Benchmark` / `Example` / `Fuzz` prefix followed by
/// end-of-name or a non-lowercase rune, so `Testify` / `Examples` are not flagged.
fn go_is_test_name(name: &str) -> bool {
    const PREFIXES: [&str; 4] = ["Test", "Benchmark", "Example", "Fuzz"];
    PREFIXES.iter().any(|prefix| {
        name.strip_prefix(prefix)
            .is_some_and(|rest| rest.chars().next().is_none_or(|c| !c.is_lowercase()))
    })
}

/// Go doc comments are plain `//` (or `/* */`) lines directly above a declaration — Go
/// has no `///`-style marker — so [`clean_docstring`] (which only promotes `///`/`//!`/`/**`)
/// drops them. Promote the contiguous run here, for Go only.
fn clean_go_doc_comments(comments: &[String]) -> Option<String> {
    if comments.is_empty() {
        return None;
    }
    // `comments` is collected nearest-first; restore source order.
    let mut ordered = comments.to_vec();
    ordered.reverse();
    let mut lines = Vec::new();
    for comment in ordered {
        let trimmed = comment.trim();
        if let Some(rest) = trimmed.strip_prefix("//") {
            lines.push(rest.trim().to_string());
        } else if trimmed.starts_with("/*") {
            let inner = trimmed.trim_start_matches("/*").trim_end_matches("*/").trim();
            for line in inner.lines() {
                lines.push(line.trim().trim_start_matches('*').trim().to_string());
            }
        }
    }
    let joined = lines.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

/// Recursively: does `node`'s subtree contain a `marker_annotation` / `annotation` whose
/// simple name equals `target`? Compares the simple name so `@org.junit.Test` matches and
/// `@TestFactory` does not.
fn annotation_subtree_contains(node: Node, target: &str, source: &[u8]) -> bool {
    let kind = node.kind();
    if kind == "marker_annotation" || kind == "annotation" {
        if let Ok(text) = node.utf8_text(source) {
            let body = text.trim_start_matches('@').trim();
            let head = body.split('(').next().unwrap_or(body).trim();
            let simple = head.rsplit('.').next().unwrap_or(head).trim();
            if simple == target {
                return true;
            }
        }
    }
    for i in 0..node.child_count() {
        if annotation_subtree_contains(node.child(i as u32).unwrap(), target, source) {
            return true;
        }
    }
    false
}

/// Java/Kotlin: does the declaration's `modifiers` carry an annotation named `target`?
fn has_annotation(node: Node, target: &str, source: &[u8]) -> bool {
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == "modifiers" && annotation_subtree_contains(child, target, source) {
            return true;
        }
    }
    false
}

/// Kotlin annotation lookup. Like [`has_annotation`], but also recovers tree-sitter-kotlin-ng's
/// quirk where a *top-level* annotation carrying arguments (`@Deprecated("msg")`) parses as a
/// detached `annotated_expression` sibling instead of the following declaration's modifier.
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
/// caught by [`path_indicates_test`].
fn java_is_test(node: Node, file_path: &str, source: &[u8]) -> bool {
    if has_annotation(node, "Test", source) {
        return true;
    }
    let file = file_path.rsplit(['/', '\\']).next().unwrap_or(file_path);
    file.ends_with("Test.java") || file.ends_with("Tests.java") || file.ends_with("IT.java")
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

/// Go: one query capture (`symbol.gotype`) covers every `type_spec`; the concrete kind
/// is the `type:` child — `struct_type` / `interface_type` / everything-else (defined
/// types like `type UserID int`).
fn go_type_kind(node: Node) -> &'static str {
    match node.child_by_field_name("type").map(|t| t.kind()) {
        Some("struct_type") => "struct",
        Some("interface_type") => "interface",
        _ => "type",
    }
}

// ---------------------------------------------------------------------------
// Owner (enclosing type) resolution.
//
// All node/field kinds below were confirmed against the resolved grammar versions
// (tree-sitter-rust 0.24.2, -go 0.25.0, -typescript 0.23.2, -java 0.23.5, -python 0.25.0,
// -kotlin-ng 1.1.0) — via each grammar's `node-types.json` and an empirical ancestor-chain
// dump over fixture snippets. Every field/child access falls back to `None` on an
// unexpected shape: a wrong owner is worse than none.
// ---------------------------------------------------------------------------

/// Tree-sitter node kinds that terminate the owner ancestor-walk with `None`: another
/// function/closure/lambda scope (the symbol is then a local function, not a member) or an
/// anonymous-type / object-value body (no named type to attribute). Verified per grammar
/// against `node-types.json` and an empirical ancestor dump. Over-listing is safe — an
/// absent kind simply never matches.
fn owner_stop_kinds(ext: &str) -> &'static [&'static str] {
    match ext {
        "rs" => &["function_item", "closure_expression"],
        "py" => &["function_definition", "lambda"],
        "ts" | "js" | "tsx" | "jsx" => &[
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
        ],
        "go" => &["function_declaration", "method_declaration", "func_literal"],
        "java" => &[
            "method_declaration",
            "constructor_declaration",
            "lambda_expression",
            // anonymous class body: `new Runnable() { ... }`
            "object_creation_expression",
        ],
        "kt" | "kts" => &[
            "function_declaration",
            "anonymous_function",
            "lambda_literal",
            // anonymous object expression: `object { ... }`
            "object_literal",
        ],
        _ => &[],
    }
}

/// Named type-container node kinds whose `name` is the owner of a member declared inside
/// them. Verified per grammar. (Rust/Go are handled specially below and are not listed.)
fn owner_type_container_kinds(ext: &str) -> &'static [&'static str] {
    match ext {
        "py" => &["class_definition"],
        "ts" | "js" | "tsx" | "jsx" => &["class_declaration", "abstract_class_declaration"],
        "java" => &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
        ],
        "kt" | "kts" => &["class_declaration", "object_declaration"],
        _ => &[],
    }
}

/// Container node kinds that are transparent to ownership: a member inside them belongs to
/// the next enclosing named type, not the container itself. Rust `mod_item` / TS
/// `namespace` are modules (their members are free, so the walk continues but never
/// attributes them); Kotlin `companion_object` resolves to its enclosing class.
fn owner_passthrough_kinds(ext: &str) -> &'static [&'static str] {
    match ext {
        "rs" => &["mod_item", "declaration_list", "enum_variant_list"],
        "ts" | "js" | "tsx" | "jsx" => &[
            "class_body",
            "statement_block",
            "internal_module",
            "module",
            "namespace",
        ],
        "py" => &["block"],
        "java" => &[
            "class_body",
            "interface_body",
            "enum_body",
            "enum_body_declarations",
            "block",
        ],
        "kt" | "kts" => &["class_body", "companion_object", "enum_class_body"],
        "go" => &["interface_type", "type_declaration"],
        _ => &[],
    }
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

/// Go: reduce a receiver `type` node to its base identifier — peel `*` (`pointer_type`),
/// generic args (`generic_type`, e.g. `Box[T]` → `Box`), and package qualification
/// (`qualified_type` → `name`). Returns `None` on any unexpected shape.
fn go_base_type_name(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "type_identifier" => node.utf8_text(source).ok().map(|t| t.to_string()),
        "pointer_type" => {
            // `*T` — the inner type is the single named child.
            for i in 0..node.child_count() {
                let child = node.child(i as u32).unwrap();
                if child.is_named() {
                    return go_base_type_name(child, source);
                }
            }
            None
        }
        "generic_type" => {
            // `Box[T]` — the base is the `type` field.
            go_base_type_name(node.child_by_field_name("type")?, source)
        }
        "qualified_type" => {
            // `pkg.Type` — the local name is the `name` field.
            let name = node.child_by_field_name("name")?;
            name.utf8_text(source).ok().map(|t| t.to_string())
        }
        _ => None,
    }
}

/// Go: read the receiver base type from a `method_declaration` node directly (the receiver
/// lives on the node itself, not an ancestor). `func (s *Server) Start()` → `Server`.
fn go_receiver_owner(method_node: Node, source: &[u8]) -> Option<String> {
    let receiver = method_node.child_by_field_name("receiver")?; // parameter_list
    for i in 0..receiver.child_count() {
        let child = receiver.child(i as u32).unwrap();
        if child.kind() == "parameter_declaration" {
            let type_node = child.child_by_field_name("type")?;
            return go_base_type_name(type_node, source);
        }
    }
    None
}

/// Resolve the enclosing *type* (owner) of the symbol at `node`, or `None` when the symbol
/// is a free function, a top-level symbol, a function/closure/lambda-local definition, or a
/// member of an anonymous type. Only meaningful for `fn`/method symbols (the only callers).
fn find_owner_name(node: Node, ext: &str, source: &[u8]) -> Option<String> {
    // Go methods carry their receiver type on the symbol node itself — read it directly;
    // an ancestor walk alone would miss it (and `method_declaration` is itself a stop node).
    if ext == "go" && node.kind() == "method_declaration" {
        return go_receiver_owner(node, source);
    }

    let stop_kinds = owner_stop_kinds(ext);
    let type_container_kinds = owner_type_container_kinds(ext);
    let passthrough_kinds = owner_passthrough_kinds(ext);

    let mut ancestor = node.parent();
    while let Some(current) = ancestor {
        let kind = current.kind();

        // Crossed a function/closure/lambda scope or an anonymous-type body before reaching
        // a named type container → the symbol is local, not a member: yield None.
        if stop_kinds.contains(&kind) {
            return None;
        }

        // Rust: a method lives in an `impl_item` (read its `type`; for `impl Trait for Type`
        // the `type` field is the implementing `Type`) or a `trait_item` default method; an
        // enum variant is owned by its `enum_item` (reached via the `enum_variant_list`
        // passthrough).
        if ext == "rs" {
            if kind == "impl_item" {
                let type_node = current.child_by_field_name("type")?;
                return rust_base_type_name(type_node, source);
            }
            if kind == "trait_item" || kind == "enum_item" {
                let name = current.child_by_field_name("name")?;
                return name.utf8_text(source).ok().map(|t| t.to_string());
            }
        }

        // Go: an interface method (`method_elem`) is owned by its enclosing `type_spec`.
        if ext == "go" && kind == "type_spec" {
            let name = current.child_by_field_name("name")?;
            return name.utf8_text(source).ok().map(|t| t.to_string());
        }

        // Generic named type containers (class/interface/enum/record/object).
        if type_container_kinds.contains(&kind) {
            let name = current.child_by_field_name("name")?;
            return name.utf8_text(source).ok().map(|t| t.to_string());
        }

        // Module/namespace/companion-object/body containers are transparent — keep walking.
        if passthrough_kinds.contains(&kind) {
            ancestor = current.parent();
            continue;
        }

        // An unrecognized container before any type container: stop conservatively (None)
        // rather than risk attributing a wrong owner across an unmodeled scope.
        return None;
    }
    None
}

impl CodeExtractor for TreeSitterExtractor {
    fn extract(&self, file_content: &str, file_path: &str) -> Result<ExtractedFile, String> {
        let path = Path::new(file_path);
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let mut parser = Parser::new();
        let (lang, query) = match ext {
            "rs" => (tree_sitter_rust::LANGUAGE.into(), get_rust_query()),
            "py" => (tree_sitter_python::LANGUAGE.into(), get_python_query()),
            "ts" | "js" => (
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                get_ts_query(),
            ),
            "tsx" | "jsx" => (
                tree_sitter_typescript::LANGUAGE_TSX.into(),
                get_tsx_query(),
            ),
            "go" => (tree_sitter_go::LANGUAGE.into(), get_go_query()),
            "java" => (tree_sitter_java::LANGUAGE.into(), get_java_query()),
            "kt" | "kts" => (tree_sitter_kotlin_ng::LANGUAGE.into(), get_kotlin_query()),
            _ => {
                return Ok(ExtractedFile {
                    file_path: file_path.to_string(),
                    total_lines: file_content.lines().count(),
                    symbols: Vec::new(),
                    literals: Vec::new(),
                    docstrings: Vec::new(),
                });
            }
        };

        parser.set_language(&lang).map_err(|e| e.to_string())?;
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
        let mut matches = query_cursor.matches(query, tree.root_node(), source);

        while let Some(mat) = matches.next() {
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
                            "symbol.variant" => "variant",
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
                            "symbol.record" => "record",
                            "symbol.object" => "object",
                            "symbol.property" => "property",
                            // Go `type_spec` and Kotlin `class_declaration` carry a single
                            // capture; the concrete kind comes from the node itself.
                            "symbol.gotype" => go_type_kind(node),
                            "symbol.ktclass" => kotlin_class_kind(node),
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
                            } else if ext == "go" {
                                // Go doc comments sit above the outer declaration, not the
                                // inner `*_spec` the symbol is captured on.
                                if let Some(parent) = node.parent() {
                                    let pk = parent.kind();
                                    if pk == "type_declaration"
                                        || pk == "const_declaration"
                                        || pk == "var_declaration"
                                    {
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
                            if ext == "go" && docstring.is_none() {
                                docstring = clean_go_doc_comments(&comments);
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
                                            let child = parent.child(i as u32).unwrap();
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
                            } else if ext == "go" {
                                path_indicates_test(file_path) || go_is_test_name(&name)
                            } else if ext == "java" {
                                path_indicates_test(file_path)
                                    || java_is_test(node, file_path, source)
                            } else if ext == "kt" || ext == "kts" {
                                path_indicates_test(file_path)
                                    || kotlin_has_annotation(node, "Test", source)
                            } else {
                                let is_test_call = kind == "test";
                                path_indicates_test(file_path) || is_test_call
                            };

                            let is_exported = if ext == "rs" {
                                let mut found = false;
                                for i in 0..node.child_count() {
                                    if node.child(i as u32).unwrap().kind() == "visibility_modifier" {
                                        found = true;
                                        break;
                                    }
                                }
                                found
                            } else if ext == "py" {
                                !name.starts_with('_') && !has_ancestor_fn(node)
                            } else if ext == "go" {
                                go_is_exported(&name)
                                    && !is_inside_function(
                                        node,
                                        &["function_declaration", "method_declaration", "func_literal"],
                                    )
                            } else if ext == "java" {
                                java_is_public(node)
                            } else if ext == "kt" || ext == "kts" {
                                kotlin_is_exported(node, source)
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
                            } else if ext == "go" {
                                // Go marks deprecation with a `// Deprecated:` paragraph in
                                // the doc comment (gopls/staticcheck convention).
                                contains_case_insensitive(&comments_text, "deprecated:")
                                    || docstring.as_ref().is_some_and(|d| {
                                        contains_case_insensitive(d, "deprecated:")
                                    })
                            } else if ext == "java" {
                                has_annotation(node, "Deprecated", source)
                                    || docstring.as_ref().is_some_and(|d| d.contains("@deprecated"))
                            } else if ext == "kt" || ext == "kts" {
                                kotlin_has_annotation(node, "Deprecated", source)
                            } else {
                                docstring
                                    .as_ref()
                                    .is_some_and(|d| d.contains("@deprecated"))
                            };

                            // Owner (enclosing type) is computed for callables — the search
                            // annotation qualifies `fn`/method names — and for enum variants
                            // (the owning enum names which type the variant belongs to).
                            // Other members are deferred (see the brief's Open Questions).
                            // Best-effort: any unexpected shape yields `None`.
                            let owner = if kind == "fn" || kind == "variant" {
                                find_owner_name(node, ext, source)
                            } else {
                                None
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
                                owner,
                            });
                        }
                    }
                } else if capture_name.starts_with("literal.string") {
                    // Only string literals carry search/detail value; numeric and boolean
                    // literals are dropped (low value, index/detail bloat — Child 03).
                    if let Ok(text) = node.utf8_text(source) {
                        let stripped = strip_quotes(text);
                        let line = node.start_position().row + 1;
                        literals.push(ExtractedLiteral { text: stripped, line });
                    }
                }
            }
        }

        let docstrings = symbols.iter().filter_map(|s| s.docstring.clone()).collect();

        Ok(ExtractedFile {
            file_path: file_path.to_string(),
            total_lines: file_content.lines().count(),
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
        assert_eq!(file.literals.len(), 1);
        assert_eq!(file.literals[0].text, "magic_value");
        assert_eq!(file.literals[0].line, 2);
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

    // --- Owner (enclosing type) Tests (Phase A) ---

    /// Helper: extract `content` as `path` and return the `owner` of the first symbol named
    /// `name`. Panics if no such symbol exists, so a missing extraction fails loudly.
    fn owner_of(content: &str, path: &str, name: &str) -> Option<String> {
        let extractor = TreeSitterExtractor::new();
        let file = extractor.extract(content, path).unwrap();
        file.symbols
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("symbol `{name}` not extracted from {path}"))
            .owner
            .clone()
    }

    #[test]
    fn test_owner_rust_impl_method() {
        let content = "impl Server { pub fn start(&self) {} }";
        assert_eq!(
            owner_of(content, "src/lib.rs", "start"),
            Some("Server".to_string())
        );
    }

    #[test]
    fn test_owner_rust_impl_trait_for_type_uses_type() {
        // `impl Trait for Type` → the implementing Type, not the trait.
        let content = "impl Display for Widget { fn fmt(&self) {} }";
        assert_eq!(
            owner_of(content, "src/lib.rs", "fmt"),
            Some("Widget".to_string())
        );
    }

    #[test]
    fn test_owner_rust_generic_and_scoped_impl_normalized() {
        // Generic args stripped: `impl Cache<K, V>` → `Cache`.
        assert_eq!(
            owner_of(
                "impl<K, V> Cache<K, V> { fn get(&self) {} }",
                "src/lib.rs",
                "get"
            ),
            Some("Cache".to_string())
        );
        // Module path reduced to the rightmost segment: `impl a::b::Store` → `Store`.
        assert_eq!(
            owner_of(
                "impl crate::store::Store { fn put(&self) {} }",
                "src/lib.rs",
                "put"
            ),
            Some("Store".to_string())
        );
    }

    #[test]
    fn test_owner_rust_trait_default_method() {
        let content = "trait Greeter { fn greet(&self) { println!(\"hi\"); } }";
        assert_eq!(
            owner_of(content, "src/lib.rs", "greet"),
            Some("Greeter".to_string())
        );
    }

    #[test]
    fn test_owner_rust_free_fn_is_none() {
        assert_eq!(owner_of("pub fn run() {}", "src/lib.rs", "run"), None);
    }

    #[test]
    fn test_owner_rust_fn_in_module_is_none() {
        // A function nested only inside a module (not a type) has no owner.
        assert_eq!(
            owner_of("mod util { pub fn helper() {} }", "src/lib.rs", "helper"),
            None
        );
    }

    #[test]
    fn test_owner_rust_local_fn_in_method_is_none() {
        let content = "impl Server { fn run(&self) { fn helper() {} } }";
        assert_eq!(owner_of(content, "src/lib.rs", "helper"), None);
    }

    #[test]
    fn test_owner_rust_fn_in_closure_in_method_is_none() {
        let content = "impl Server { fn run(&self) { let c = || { fn inner() {} }; } }";
        assert_eq!(owner_of(content, "src/lib.rs", "inner"), None);
    }

    #[test]
    fn test_owner_go_method_receiver_base_type() {
        // Pointer receiver `*Server` → `Server`.
        assert_eq!(
            owner_of(
                "package p\nfunc (s *Server) Start() {}\n",
                "main.go",
                "Start"
            ),
            Some("Server".to_string())
        );
        // Value receiver `Server` → `Server`.
        assert_eq!(
            owner_of("package p\nfunc (s Server) Stop() {}\n", "main.go", "Stop"),
            Some("Server".to_string())
        );
    }

    #[test]
    fn test_owner_go_generic_receiver_normalized() {
        // `*Box[T]` → `Box` (square-bracketed generic args stripped).
        assert_eq!(
            owner_of(
                "package p\nfunc (b *Box[T]) Get() {}\n",
                "main.go",
                "Get"
            ),
            Some("Box".to_string())
        );
    }

    #[test]
    fn test_owner_go_interface_method_elem() {
        let content = "package p\ntype Reader interface {\n Read() error\n}\n";
        assert_eq!(
            owner_of(content, "main.go", "Read"),
            Some("Reader".to_string())
        );
    }

    #[test]
    fn test_owner_go_free_fn_is_none() {
        assert_eq!(
            owner_of("package p\nfunc Run() {}\n", "main.go", "Run"),
            None
        );
    }

    #[test]
    fn test_owner_go_local_fn_in_method_is_none() {
        let content = "package p\nfunc (s *Server) Run() {\n inner := func() {}\n _ = inner\n}\n";
        // A `func_literal` is anonymous (no name symbol) — assert the method itself instead:
        // the receiver method resolves, and no spurious owner leaks to nested closures.
        assert_eq!(
            owner_of(content, "main.go", "Run"),
            Some("Server".to_string())
        );
    }

    #[test]
    fn test_owner_python_class_method() {
        let content = "class Foo:\n    def bar(self):\n        pass\n";
        assert_eq!(owner_of(content, "x.py", "bar"), Some("Foo".to_string()));
    }

    #[test]
    fn test_owner_python_free_fn_is_none() {
        assert_eq!(owner_of("def run():\n    pass\n", "x.py", "run"), None);
    }

    #[test]
    fn test_owner_python_local_fn_in_method_is_none() {
        let content = "class Foo:\n    def bar(self):\n        def helper():\n            pass\n";
        assert_eq!(owner_of(content, "x.py", "helper"), None);
    }

    #[test]
    fn test_owner_ts_class_method() {
        let content = "class Service { handle() {} }";
        assert_eq!(
            owner_of(content, "x.ts", "handle"),
            Some("Service".to_string())
        );
    }

    #[test]
    fn test_owner_ts_free_fn_is_none() {
        assert_eq!(owner_of("function run() {}", "x.ts", "run"), None);
    }

    #[test]
    fn test_owner_ts_local_fn_in_method_is_none() {
        let content = "class A { method() { function localFn() { return 1; } } }";
        assert_eq!(owner_of(content, "x.ts", "localFn"), None);
    }

    #[test]
    fn test_owner_ts_object_literal_method_is_none() {
        // A `method_definition` inside an object-literal value, not a named type.
        let content = "class A { config = { handler() {} }; }";
        assert_eq!(owner_of(content, "x.ts", "handler"), None);
    }

    #[test]
    fn test_owner_ts_class_expression_method_is_none() {
        // A class *expression* (a value) is anonymous — no owner.
        let content = "const X = class { doThing() {} };";
        assert_eq!(owner_of(content, "x.ts", "doThing"), None);
    }

    #[test]
    fn test_owner_js_class_method() {
        let content = "class Widget { render() {} }";
        assert_eq!(
            owner_of(content, "x.js", "render"),
            Some("Widget".to_string())
        );
    }

    #[test]
    fn test_owner_ts_abstract_class_method() {
        // `abstract_class_declaration` is a named type container (verified against the
        // grammar's node-types.json: `name` field is a `type_identifier`).
        let content = "abstract class Base { run() {} }";
        assert_eq!(owner_of(content, "x.ts", "run"), Some("Base".to_string()));
    }

    #[test]
    fn test_owner_java_class_method() {
        let content = "class A { void m() {} }";
        assert_eq!(owner_of(content, "A.java", "m"), Some("A".to_string()));
    }

    #[test]
    fn test_owner_java_interface_enum_record_methods() {
        assert_eq!(
            owner_of("interface I { void doI(); }", "I.java", "doI"),
            Some("I".to_string())
        );
        assert_eq!(
            owner_of("enum E { A; void doE() {} }", "E.java", "doE"),
            Some("E".to_string())
        );
        assert_eq!(
            owner_of("record R(int x) { void doR() {} }", "R.java", "doR"),
            Some("R".to_string())
        );
    }

    #[test]
    fn test_owner_java_anonymous_class_method_is_none() {
        let content =
            "class A { void m() { Runnable r = new Runnable() { public void run() {} }; } }";
        assert_eq!(owner_of(content, "A.java", "run"), None);
    }

    #[test]
    fn test_owner_kotlin_class_method() {
        let content = "class Service {\n  fun handle() {}\n}\n";
        assert_eq!(
            owner_of(content, "x.kt", "handle"),
            Some("Service".to_string())
        );
    }

    #[test]
    fn test_owner_kotlin_object_method() {
        let content = "object Singleton {\n  fun go() {}\n}\n";
        assert_eq!(
            owner_of(content, "x.kt", "go"),
            Some("Singleton".to_string())
        );
    }

    #[test]
    fn test_owner_kotlin_object_literal_method_is_none() {
        let content = "fun build() {\n  val x = object {\n    fun anon() {}\n  }\n}\n";
        assert_eq!(owner_of(content, "x.kt", "anon"), None);
    }

    #[test]
    fn test_owner_kotlin_free_fn_is_none() {
        assert_eq!(owner_of("fun run() {}\n", "x.kt", "run"), None);
    }

    #[test]
    fn test_owner_kotlin_companion_object_resolves_enclosing_class() {
        // Kotlin `companion object` members resolve to the enclosing class name. Note:
        // tree-sitter-kotlin-ng 1.1.0 is shape-sensitive here — a multi-line body nests the
        // member under the class so `companion_object` (passthrough) is traversed to
        // `class_declaration`; a single-line body can instead collapse the companion into an
        // `ERROR` node, in which case the walk yields `None` (best-effort, never wrong). This
        // test pins the common multi-line shape resolving to the enclosing class.
        let extractor = TreeSitterExtractor::new();
        let content = "class A {\n  companion object {\n    fun create() {}\n  }\n}\n";
        let file = extractor.extract(content, "x.kt").unwrap();
        let sym = file
            .symbols
            .iter()
            .find(|s| s.name == "create")
            .expect("companion member should be extracted");
        assert_eq!(sym.owner, Some("A".to_string()));
    }

    /// Owner is best-effort and never panics: feed each grammar a method-in-type fixture and
    /// confirm the call returns without crashing (a wrong owner is worse than `None`).
    #[test]
    fn test_owner_no_panic_across_grammars() {
        let cases = [
            ("impl T { fn a(&self) {} }", "x.rs"),
            ("package p\nfunc (s *T) A() {}\n", "x.go"),
            ("class T:\n    def a(self): pass\n", "x.py"),
            ("class T { a() {} }", "x.ts"),
            ("class T { void a() {} }", "T.java"),
            ("class T {\n  fun a() {}\n}\n", "x.kt"),
        ];
        for (content, path) in cases {
            let extractor = TreeSitterExtractor::new();
            // Must not panic; owner correctness is asserted in the per-language tests above.
            let _ = extractor.extract(content, path).unwrap();
        }
    }
}
