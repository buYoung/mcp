//! Parser-side language knowledge: one [`LanguageSpec`] per supported language, plus a
//! static registry resolving a source extension to its spec.
//!
//! ## Why this module exists
//! The tree-sitter extraction in [`crate::parser`] is a single generic tree-walk. Every
//! place where the walk needs language-specific behavior — grammar/query selection, the
//! `is_test`/`is_exported`/`is_deprecated` flags, docstring promotion, owner (enclosing
//! type) resolution, and the C-family/ASM name-extraction cluster — is expressed as a hook
//! on the [`LanguageSpec`] trait. Each language file states only its differences; the trait
//! default implementations encode the common (TypeScript-family) behavior.
//!
//! ## Add-a-language recipe
//! To support a new tree-sitter language:
//! 1. Add the grammar crate to `apps/codemap-search/Cargo.toml` (one Cargo dependency).
//! 2. Add one file `lang/<name>.rs` with a `struct <Name>Spec;` implementing
//!    [`LanguageSpec`] — its query string, compiled-query getter (lazy `OnceLock`), its
//!    [`LanguageSpec::extensions`] list, and the hooks whose behavior differs from the
//!    defaults (e.g. [`LanguageSpec::qualified_name_separator`],
//!    [`LanguageSpec::is_import_line`]).
//! 3. Add one registry entry in [`spec_for_ext`] mapping the file extension(s) to a
//!    `&'static <Name>Spec`, and one entry in [`ALL_SPECS`] so the new extensions join the
//!    [`source_extensions`] allowlist.
//!
//! No edits to [`crate::parser`], `crate::callers`, or `crate::workspace` are needed: the
//! generic walk resolves the spec from the registry and calls the hooks, the extension
//! allowlist is derived from [`ALL_SPECS`], and `callers` delegates separators / import-line
//! detection to the spec.

mod asm;
pub(crate) mod c_family;
mod go;
mod java;
mod javascript;
mod kotlin;
mod python;
mod rust;
mod sql;
pub(crate) mod structured_formats;
mod typescript;

use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

/// Outcome of the C-family/ASM accept-and-name cluster: either skip this query match
/// entirely (preserving the original `continue` semantics) or accept it with a name.
pub(crate) enum NameDecision {
    /// Discard this match (the walk's `continue`).
    Skip,
    /// Accept this match with the resolved symbol name.
    Name(String),
}

/// Per-language behavior for the generic extraction walk in [`crate::parser`]. Default
/// implementations encode the common (TypeScript-family) behavior; each language file
/// overrides only the hooks where it differs.
pub(crate) trait LanguageSpec: Sync {
    /// Stable language label used by ranking and language hints.
    fn language_name(&self) -> &'static str;

    /// The tree-sitter grammar for `ext`. One spec may serve multiple dialect extensions
    /// backed by the same grammar (TypeScript: ts/mts/cts; JavaScript: js/jsx/mjs/cjs).
    fn grammar(&self, ext: &str) -> Language;

    /// The compiled query for `ext` (lazy, compiled once per grammar via `OnceLock`).
    fn query(&self, ext: &str) -> &'static Query;

    /// Optional tags-only auxiliary query for `ext`, compiled against the same grammar as
    /// [`Self::query`]. `None` keeps indexing auxiliary text empty for languages that do
    /// not ship a `queries/<lang>/tags.scm` hook.
    fn tags_query(&self, _ext: &str) -> Option<&'static Query> {
        None
    }

    /// Optional query for statically identifiable collection write/read endpoints.
    fn static_collection_query(&self, _ext: &str) -> Option<&'static Query> {
        None
    }

    /// The source-file extensions this spec serves (no leading dot). Unioned across all
    /// specs in [`ALL_SPECS`] to derive the source-extension allowlist
    /// ([`source_extensions`]) consumed by `workspace::is_source_extension`.
    fn extensions(&self) -> &'static [&'static str];

    /// Exact basenames served by this spec when extension matching is insufficient.
    fn exact_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether approximate caller scanning is meaningful for this language.
    fn caller_scan_enabled(&self) -> bool {
        true
    }

    /// Optional AST extraction policy for structured formats whose symbol/dependency model does
    /// not fit the programming-language query captures. The default keeps the generic query walk.
    fn extract_override(
        &self,
        _root: Node<'_>,
        _source: &str,
    ) -> Option<crate::parser::SpecExtraction> {
        None
    }

    /// Whether this spec's runtime query includes `@nav.*` / `@local.*` captures. `false`
    /// means `ExtractedFile.navigation` stays `None`; `true` means extraction ran and may
    /// produce `Some(NavigationFile { calls: vec![], .. })`.
    fn navigation_enabled(&self, _ext: &str) -> bool {
        false
    }

    /// The separator between an owning type and a member in a qualified symbol name
    /// (`callers` annotation display). Default `"."` matches every class-nested language;
    /// only Rust overrides to `"::"`.
    fn qualified_name_separator(&self) -> &'static str {
        "."
    }

    /// Whether `line` (already trimmed by the caller via `trim_start`) is an import/use
    /// statement in this language, so a name appearing there is excluded from
    /// non-call-reference reporting. Default encodes the TypeScript-family rule
    /// (`import` / `require(`); each language file overrides with its own construct.
    fn is_import_line(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with("import ")
            || trimmed.starts_with("require(")
            || trimmed.contains("require(")
    }

    /// Pre-pass collecting file-wide exported symbol names before the match loop (TS
    /// `export { ... }` specifiers; ASM `.globl`/`.global` directives). Default: no-op.
    fn collect_exported_names(&self, _root: Node, _source: &[u8], _out: &mut HashSet<String>) {}

    /// Refine a single-capture symbol kind that depends on the node itself (Go `type_spec`,
    /// Kotlin `class`/`interface`). `capture_name` is the query capture; `kind` is the
    /// already-resolved static kind. Default: return `kind` unchanged.
    fn refine_kind(&self, _capture_name: &str, _node: Node, kind: &'static str) -> &'static str {
        kind
    }

    /// The C-family/ASM accept-and-name cluster: filter out non-symbol matches and extract
    /// the symbol name from the declarator chain / label / macro. Returns `None` when this
    /// language has no special handling for `capture_name` (the walk then uses the generic
    /// `symbol_name`/`find_name` path). Default: `None`.
    fn name_for_capture(
        &self,
        _capture_name: &str,
        _node: Node,
        _kind: &str,
        _ext: &str,
        _source: &[u8],
        _asm_meta_kind_text: &Option<String>,
    ) -> Option<NameDecision> {
        None
    }

    /// Adjust the start node for the preceding-comment walk (Python `decorated_definition`,
    /// TS `export_statement`, Go outer declaration). Default: the node itself.
    fn docstring_anchor<'a>(&self, node: Node<'a>) -> Node<'a> {
        node
    }

    /// Language-specific docstring fallback when the comment-promotion path yields `None`
    /// (Python inline `"""` docstrings, Go plain `//` doc comments). Default: `None`.
    fn docstring_fallback(
        &self,
        _node: Node,
        _source: &[u8],
        _comments: &[String],
    ) -> Option<String> {
        None
    }

    /// Whether the symbol at `node` is a test. Default encodes the TypeScript-family rule:
    /// a path-indicated test file or a `test`-kind call expression.
    fn is_test(
        &self,
        node: Node,
        _name: &str,
        kind: &str,
        file_path: &str,
        _source: &[u8],
        _comments_text: &str,
    ) -> bool {
        let _ = node;
        let is_test_call = kind == "test";
        path_indicates_test(file_path) || is_test_call
    }

    /// Whether the symbol at `node` is part of the public API. Default encodes the
    /// TypeScript-family rule: an `export` ancestor or a name in the exported-names set.
    fn is_exported(
        &self,
        node: Node,
        name: &str,
        _kind: &str,
        _source: &[u8],
        exported_names: &HashSet<String>,
    ) -> bool {
        has_ancestor_export(node) || exported_names.contains(name)
    }

    /// Whether the symbol at `node` is deprecated. Default: a `@deprecated` marker in the
    /// docstring (no attribute parsing).
    fn is_deprecated(
        &self,
        _node: Node,
        _source: &[u8],
        docstring: &Option<String>,
        _comments_text: &str,
    ) -> bool {
        docstring
            .as_ref()
            .is_some_and(|d| d.contains("@deprecated"))
    }

    /// Resolve the enclosing *type* (owner) of the symbol at `node`, or `None`. Default
    /// implementation performs the generic ancestor walk using the owner-kind tables.
    fn find_owner(&self, node: Node, ext: &str, source: &[u8]) -> Option<String> {
        generic_find_owner(self, node, ext, source)
    }

    /// Node kinds that terminate the owner ancestor-walk with `None`.
    fn owner_stop_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[]
    }

    /// Named type-container node kinds whose `name` is the owner of a member inside them.
    fn owner_type_container_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[]
    }

    /// Container node kinds transparent to ownership (a member inside belongs to the next
    /// enclosing named type).
    fn owner_passthrough_kinds(&self, _ext: &str) -> &'static [&'static str] {
        &[]
    }

    /// Owner resolution for an in-loop named type container, given the language can refine
    /// it (Rust `impl_item`/`trait_item`/`enum_item`, Go `type_spec`). Returns `Some(result)`
    /// to short-circuit the walk with that result, or `None` to fall through to the generic
    /// type-container/passthrough handling. Default: `None`.
    fn owner_for_container<'a>(
        &self,
        _current: Node<'a>,
        _source: &[u8],
    ) -> Option<Option<String>> {
        None
    }
}

/// Resolve a source extension to its [`LanguageSpec`], or `None` for an unsupported
/// extension (the walk then returns an empty `ExtractedFile`).
pub(crate) fn spec_for_ext(ext: &str) -> Option<&'static dyn LanguageSpec> {
    match ext {
        "rs" => Some(&rust::RustSpec),
        "py" => Some(&python::PythonSpec),
        "ts" | "tsx" | "mts" | "cts" => Some(&typescript::TypeScriptSpec),
        "js" | "jsx" | "mjs" | "cjs" => Some(&javascript::JavaScriptSpec),
        "go" => Some(&go::GoSpec),
        "java" => Some(&java::JavaSpec),
        "kt" | "kts" => Some(&kotlin::KotlinSpec),
        "c" => Some(&c_family::c::CSpec),
        "h" | "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Some(&c_family::cpp::CppSpec),
        "s" | "S" | "asm" => Some(&asm::AsmSpec),
        "sql" => Some(&sql::SqlSpec),
        "json" | "jsonc" => Some(&structured_formats::JsonSpec),
        "toml" => Some(&structured_formats::TomlSpec),
        "yaml" | "yml" => Some(&structured_formats::YamlSpec),
        "html" | "htm" => Some(&structured_formats::HtmlSpec),
        "xml" | "xsd" | "xsl" | "xslt" | "plist" | "csproj" | "props" | "targets" => {
            Some(&structured_formats::XmlSpec)
        }
        "css" => Some(&structured_formats::CssSpec),
        "scss" => Some(&structured_formats::ScssSpec),
        "less" => Some(&structured_formats::LessSpec),
        "sh" | "bash" => Some(&structured_formats::BashSpec),
        "zsh" => Some(&structured_formats::ZshSpec),
        "hcl" | "tf" | "tfvars" => Some(&structured_formats::HclSpec),
        "proto" => Some(&structured_formats::ProtoSpec),
        "graphql" | "gql" => Some(&structured_formats::GraphqlSpec),
        "mk" => Some(&structured_formats::MakeSpec),
        "cmake" => Some(&structured_formats::CmakeSpec),
        "bzl" => Some(&structured_formats::StarlarkSpec),
        _ => None,
    }
}

pub(crate) fn language_name_for_extension(ext: &str) -> Option<&'static str> {
    let normalized = ext.to_ascii_lowercase();
    spec_for_ext(&normalized)
        .map(LanguageSpec::language_name)
        .or(match normalized.as_str() {
            "vue" => Some("vue"),
            "astro" => Some("astro"),
            "svelte" => Some("svelte"),
            _ => None,
        })
}

/// Resolve ordinary extensions and exact basenames through the shared language registry.
pub(crate) fn spec_for_path(path: &Path) -> Option<&'static dyn LanguageSpec> {
    let exact = path.file_name().and_then(|name| name.to_str());
    ALL_SPECS
        .iter()
        .copied()
        .find(|spec| exact.is_some_and(|name| spec.exact_names().contains(&name)))
        .or_else(|| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .and_then(spec_for_ext)
        })
}

pub(crate) fn language_name_for_path(path: &Path) -> Option<&'static str> {
    spec_for_path(path)
        .map(LanguageSpec::language_name)
        .or_else(|| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .and_then(language_name_for_extension)
        })
}

pub(crate) fn is_supported_source_path(path: &Path) -> bool {
    spec_for_path(path).is_some()
        || path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(is_composite_extension)
}

/// Only tree-sitter languages/composite components participate in approximate caller scans.
pub(crate) fn supports_caller_scan(path: &Path) -> bool {
    spec_for_path(path).is_some_and(LanguageSpec::caller_scan_enabled)
        || path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(is_composite_extension)
}

pub(crate) fn normalize_language_hint(hint: &str) -> Option<&'static str> {
    let normalized = hint.trim().trim_start_matches('.').to_ascii_lowercase();
    ALL_SPECS
        .iter()
        .copied()
        .find(|spec| {
            spec.language_name() == normalized || spec.extensions().contains(&normalized.as_str())
        })
        .map(LanguageSpec::language_name)
        .or(match normalized.as_str() {
            "vue" => Some("vue"),
            "astro" => Some("astro"),
            "svelte" => Some("svelte"),
            _ => None,
        })
}

pub(crate) fn normalize_extension_hint(hint: &str) -> Option<String> {
    let normalized = hint.trim().trim_start_matches('.').to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

/// Every registered [`LanguageSpec`], one per supported language. The source-extension
/// allowlist ([`source_extensions`]) is the union of each spec's [`LanguageSpec::extensions`].
/// Adding a language appends one entry here (alongside the one [`spec_for_ext`] arm).
static ALL_SPECS: &[&dyn LanguageSpec] = &[
    &rust::RustSpec,
    &python::PythonSpec,
    &typescript::TypeScriptSpec,
    &javascript::JavaScriptSpec,
    &go::GoSpec,
    &java::JavaSpec,
    &kotlin::KotlinSpec,
    &c_family::c::CSpec,
    &c_family::cpp::CppSpec,
    &asm::AsmSpec,
    &sql::SqlSpec,
    &structured_formats::JsonSpec,
    &structured_formats::TomlSpec,
    &structured_formats::YamlSpec,
    &structured_formats::HtmlSpec,
    &structured_formats::XmlSpec,
    &structured_formats::CssSpec,
    &structured_formats::ScssSpec,
    &structured_formats::LessSpec,
    &structured_formats::BashSpec,
    &structured_formats::ZshSpec,
    &structured_formats::HclSpec,
    &structured_formats::ContainerfileSpec,
    &structured_formats::ProtoSpec,
    &structured_formats::GraphqlSpec,
    &structured_formats::MakeSpec,
    &structured_formats::CmakeSpec,
    &structured_formats::StarlarkSpec,
];

/// Composite component formats use embedded JavaScript/TypeScript grammars rather than a
/// standalone tree-sitter grammar. Keep their eligibility in this same registry module so
/// workspace walking and ranking remain centralized; MDX is intentionally absent.
const COMPOSITE_SOURCE_EXTENSIONS: &[&str] = &["vue", "astro", "svelte"];

pub(crate) fn is_composite_extension(ext: &str) -> bool {
    COMPOSITE_SOURCE_EXTENSIONS.contains(&ext)
}

/// The source-file extensions codemap-search understands, derived once as the union of
/// every spec's [`LanguageSpec::extensions`] in [`ALL_SPECS`]. Replaces the former
/// hand-maintained `workspace::SOURCE_EXTENSIONS` literal so adding a language never edits
/// `workspace.rs`. Membership-only (a `HashSet`); no caller depends on ordering.
pub fn source_extensions() -> &'static HashSet<&'static str> {
    static SOURCE_EXTENSIONS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SOURCE_EXTENSIONS.get_or_init(|| {
        ALL_SPECS
            .iter()
            .flat_map(|spec| spec.extensions().iter().copied())
            .chain(COMPOSITE_SOURCE_EXTENSIONS.iter().copied())
            .collect()
    })
}

// ---------------------------------------------------------------------------
// Shared helpers used by default trait impls and/or by more than one language file.
// Moved verbatim from parser/mod.rs.
// ---------------------------------------------------------------------------

pub(crate) fn contains_case_insensitive(text: &str, pattern: &str) -> bool {
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

pub(crate) fn strip_quotes(s: &str) -> String {
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

pub(crate) fn clean_docstring(comments: &[String]) -> Option<String> {
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
pub(crate) fn path_indicates_test(file_path: &str) -> bool {
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

/// True if `node` is lexically inside one of `fn_kinds` (a function/method/closure body),
/// i.e. a function-local declaration — never public API regardless of name or default
/// visibility. The Go uppercase-name and Kotlin default-public export rules must defer to
/// this so a local `val`/`var` isn't reported as exported. Unknown kinds in `fn_kinds`
/// simply never match, so over-listing is safe.
pub(crate) fn is_inside_function(node: Node, fn_kinds: &[&str]) -> bool {
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

pub(crate) fn find_name(node: Node, source: &[u8]) -> Option<String> {
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

/// Java/Kotlin: does the declaration's `modifiers` carry an annotation named `target`?
/// Shared by Java and Kotlin specs.
pub(crate) fn has_annotation(node: Node, target: &str, source: &[u8]) -> bool {
    for i in 0..node.child_count() {
        let child = node.child(i as u32).unwrap();
        if child.kind() == "modifiers" && annotation_subtree_contains(child, target, source) {
            return true;
        }
    }
    false
}

/// Recursively: does `node`'s subtree contain a `marker_annotation` / `annotation` whose
/// simple name equals `target`? Compares the simple name so `@org.junit.Test` matches and
/// `@TestFactory` does not. Shared by Java and Kotlin specs.
pub(crate) fn annotation_subtree_contains(node: Node, target: &str, source: &[u8]) -> bool {
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

/// The generic owner ancestor-walk shared by the default [`LanguageSpec::find_owner`].
/// Languages with pre-walk specials (Go receiver, C++ out-of-line) override `find_owner`
/// and call this for the generic tail; in-loop named-container specials are supplied via
/// [`LanguageSpec::owner_for_container`].
pub(crate) fn generic_find_owner<S: LanguageSpec + ?Sized>(
    spec: &S,
    node: Node,
    ext: &str,
    source: &[u8],
) -> Option<String> {
    let stop_kinds = spec.owner_stop_kinds(ext);
    let type_container_kinds = spec.owner_type_container_kinds(ext);
    let passthrough_kinds = spec.owner_passthrough_kinds(ext);

    let mut ancestor = node.parent();
    while let Some(current) = ancestor {
        let kind = current.kind();

        // Crossed a function/closure/lambda scope or an anonymous-type body before reaching
        // a named type container → the symbol is local, not a member: yield None.
        if stop_kinds.contains(&kind) {
            return None;
        }

        // Language-specific in-loop named containers (Rust impl/trait/enum, Go type_spec).
        if let Some(result) = spec.owner_for_container(current, source) {
            return result;
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
