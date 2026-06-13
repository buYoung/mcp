//! `callers` — caller/callee context for the `search` detail view (on by default,
//! per-call `caller_context=false` or config `caller_context_default` to disable).
//!
//! Given the matched `fn` symbols of one file, this module performs a single
//! combined-regex workspace scan (reusing [`crate::workspace::build_walker`] + the
//! `grep.rs` searcher pattern), classifies each hit post-hoc (trailing `(` → call
//! site, else → non-call reference), attributes call sites to their innermost
//! enclosing definition symbol from the codemap snapshot, discovers depth-1 callees
//! by intersecting the matched symbol's own body with the snapshot's global `fn`-name
//! set, and renders the result as a markdown annotation block.
//!
//! Everything here is **approximate by construction** — a name-match scan with no type
//! resolution. Every rendered line says so. Qualified names (`Type::method` /
//! `Class.method`) are read DIRECTLY off the Phase-A `ExtractedSymbol::owner` field; no
//! on-demand owner source scan is performed. The decorator/attribute entry-point label
//! is the one remaining on-demand source re-read (the lines above a symbol's range fall
//! outside its recorded span).
//!
//! Failure isolation: any IO/regex/scan error makes the whole annotation degrade to
//! `None`, so the caller emits the un-annotated search result. The feature never fails
//! the response (mirrors `index.rs::parse_query_catching_panic`).
//!
//! Module layout follows the pipeline stages:
//! - [`scan`] — combined-regex workspace walk → [`scan::ScanResult`].
//! - [`symbols`] — snapshot symbol index + call-site attribution.
//! - [`callees`] — depth-1 callee discovery from a symbol's own body.
//! - [`annotate`] — render / byte-budget / dedup protocol (the public API).
//!
//! This root keeps [`CallerConfig`], the language helpers (`qualified_name`,
//! `extension_of`, `is_import_line`) and the annotation I/O helpers (`read_workspace_file`,
//! `decorator_lines_above`); it re-exports the annotate stage's public surface as the single
//! canonical path.

mod annotate;
mod callees;
mod scan;
mod symbols;

pub use annotate::{
    annotate_results, AnnotationRequest, CallerBlockDedup, DetailAnnotations, PreparedAnnotation,
    ANNOTATION_OMITTED_MARKER,
};

use crate::lang::spec_for_ext;
use crate::parser::ExtractedSymbol;
use std::path::Path;

/// Tunable caps for one annotation pass. Sourced from `config.rs` so a repo can retune
/// them; defaults: scan_cap 500, list caps 5, sub-budget 8192, common threshold 2.
#[derive(Debug, Clone, Copy)]
pub struct CallerConfig {
    /// Overall hit-collection budget for one combined-regex scan, distributed across the
    /// scanned names (per-name cap = `scan_cap / names`, floored at
    /// `MIN_PER_NAME_SCAN_HITS`) so a hot name cannot starve the others. Per-name
    /// truncation is signalled in the rendered list.
    pub scan_cap: usize,
    /// Per-symbol caller-list cap.
    pub caller_list_cap: usize,
    /// Per-symbol callee-list cap.
    pub callee_list_cap: usize,
    /// Annotation byte sub-budget WITHIN `search_detail_byte_cap` (the two-counter limit).
    pub annotation_sub_budget: usize,
    /// A name defined in ≥ this many snapshot symbols is "common": its caller list and
    /// callee occurrences are labeled attribution-ambiguous (rendered, never suppressed).
    pub common_name_threshold: usize,
    /// A matched `fn` name defined in ≥ this many snapshot `fn`s has its caller list
    /// SUPPRESSED (not merely labeled): a name-match scan cannot attribute call sites among
    /// that many same-named definitions, so a labeled-but-confident list is noise. The
    /// render emits a one-line omission note with the def count and a `grep` pointer instead.
    /// Callees are unaffected. Stricter than `common_name_threshold`.
    pub caller_omit_def_threshold: usize,
    /// Files larger than this (bytes) are skipped by the scan, matching the indexer's
    /// `collect_index_entry` size filter (config `max_file_size`).
    pub max_file_size: u64,
}

/// Render a symbol's display name, prefixed by its `owner` when present:
/// Rust → `Owner::name`, class-nested languages → `Owner.name`. The separator comes from
/// the file's [`crate::lang::LanguageSpec`] so the rendered form matches each language's
/// convention; an unregistered extension falls back to `.` (the historic default arm).
fn qualified_name(sym: &ExtractedSymbol, file_path: &str) -> String {
    match &sym.owner {
        Some(owner) => {
            let sep = spec_for_ext(extension_of(file_path))
                .map(|spec| spec.qualified_name_separator())
                .unwrap_or(".");
            format!("{owner}{sep}{}", sym.name)
        }
        None => sym.name.clone(),
    }
}

fn extension_of(path: &str) -> &str {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
}

/// Whether `line` (already trimmed) is an import/use statement in the language of `ext`,
/// so a name appearing there is excluded from non-call-reference reporting. Delegates to
/// the file's [`crate::lang::LanguageSpec`]. Conservative: an unclassifiable line — including
/// any unregistered extension (the historic default arm) — stays in (degrades to the hedged
/// "possible callback" wording).
fn is_import_line(line: &str, ext: &str) -> bool {
    spec_for_ext(ext).is_some_and(|spec| spec.is_import_line(line))
}

/// Read a workspace-relative file's contents, resolving it against `root`. Falls back to
/// the path as-given (covers absolute paths and a cwd that already equals the root).
/// Returns `None` on any IO error (failure isolation — the annotation degrades, never fails).
fn read_workspace_file(file_path: &str, root: &Path) -> Option<String> {
    let joined = root.join(file_path);
    std::fs::read_to_string(&joined)
        .or_else(|_| std::fs::read_to_string(file_path))
        .ok()
}

/// Read the contiguous decorator/attribute lines directly above `start_line` (1-based) of
/// `file_path`, returning them top-to-bottom. Scans upward across `@…` (Python/TS/Java/
/// Kotlin) and `#[…]` (Rust) lines plus blank lines, stopping at the first line that is
/// neither. Returns an empty vec on any IO error (failure isolation).
fn decorator_lines_above(file_path: &str, start_line: usize, root: &Path) -> Vec<String> {
    let content = match read_workspace_file(file_path, root) {
        Some(c) => c,
        None => return Vec::new(),
    };
    let lines: Vec<&str> = content.lines().collect();
    if start_line == 0 || start_line > lines.len() {
        return Vec::new();
    }
    let mut collected: Vec<String> = Vec::new();
    // `start_line` is 1-based; the line directly above is index `start_line - 2`.
    let mut idx = start_line as isize - 2;
    while idx >= 0 {
        let raw = lines[idx as usize];
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            idx -= 1;
            continue;
        }
        if trimmed.starts_with('@') || trimmed.starts_with("#[") {
            collected.push(trimmed.to_string());
            idx -= 1;
            continue;
        }
        break;
    }
    collected.reverse();
    collected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qualified_name_from_owner_rust_uses_colons() {
        let s = fixtures::sym("new", "fn", 5, 8, Some("TantivySearchEngine"));
        assert_eq!(qualified_name(&s, "src/index.rs"), "TantivySearchEngine::new");
    }

    #[test]
    fn test_qualified_name_from_owner_class_uses_dot() {
        let s = fixtures::sym("render", "fn", 5, 8, Some("Widget"));
        assert_eq!(qualified_name(&s, "src/widget.ts"), "Widget.render");
        let s2 = fixtures::sym("draw", "fn", 5, 8, Some("Shape"));
        assert_eq!(qualified_name(&s2, "src/shape.py"), "Shape.draw");
    }

    #[test]
    fn test_qualified_name_bare_when_owner_none() {
        let s = fixtures::sym("free_fn", "fn", 5, 8, None);
        assert_eq!(qualified_name(&s, "src/lib.rs"), "free_fn");
    }

    #[test]
    fn test_is_import_line_per_language() {
        assert!(is_import_line("use crate::foo;", "rs"));
        assert!(is_import_line("import os", "py"));
        assert!(is_import_line("from x import y", "py"));
        assert!(is_import_line("import { a } from 'b'", "ts"));
        assert!(is_import_line("import \"fmt\"", "go"));
        assert!(is_import_line("import java.util.List;", "java"));
        assert!(!is_import_line("handler(x)", "rs"));
        assert!(!is_import_line("let x = useState();", "ts"));
        // C/C++ include directives.
        assert!(is_import_line("#include <stdio.h>", "c"));
        assert!(is_import_line("#include \"myheader.h\"", "cpp"));
        assert!(!is_import_line("int foo();", "h"));
        // Assembly include directive.
        assert!(is_import_line(".include \"defs.s\"", "s"));
        assert!(!is_import_line("movq %rsp, %rbp", "S"));
    }
}

/// Shared test fixtures for the `callers/` submodules. Kept inline (not a separate file) so
/// `callers/` contains exactly the five pipeline modules; the per-stage `tests` modules reach
/// these via `use crate::callers::fixtures::*`. `#[cfg(test)]`, so never compiled into the binary.
#[cfg(test)]
pub(super) mod fixtures {
    use super::CallerConfig;
    use crate::callers::{CallerBlockDedup, DetailAnnotations};
    use crate::parser::{CodeRange, ExtractedFile, ExtractedSymbol, SymbolFlags};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    pub(in crate::callers) fn flags() -> SymbolFlags {
        SymbolFlags {
            has_todo: false,
            has_fixme: false,
            is_test: false,
            is_exported: true,
            is_deprecated: false,
        }
    }

    pub(in crate::callers) fn sym(
        name: &str,
        kind: &str,
        start: usize,
        end: usize,
        owner: Option<&str>,
    ) -> ExtractedSymbol {
        ExtractedSymbol {
            name: name.to_string(),
            kind: kind.to_string(),
            range: CodeRange {
                start_line: start,
                start_col: 0,
                end_line: end,
                end_col: 0,
            },
            docstring: None,
            flags: flags(),
            owner: owner.map(|o| o.to_string()),
        }
    }

    pub(in crate::callers) fn file(path: &str, symbols: Vec<ExtractedSymbol>) -> ExtractedFile {
        ExtractedFile {
            file_path: path.to_string(),
            total_lines: 100,
            symbols,
            literals: vec![],
            docstrings: vec![],
        }
    }

    pub(in crate::callers) fn cfg() -> CallerConfig {
        CallerConfig {
            scan_cap: 500,
            caller_list_cap: 5,
            callee_list_cap: 5,
            annotation_sub_budget: 4096,
            common_name_threshold: 2,
            caller_omit_def_threshold: 5,
            max_file_size: 1_048_576,
        }
    }

    /// Write fixture files into a temp dir and return its handle + path.
    pub(in crate::callers) fn write_repo(files: &[(&str, &str)]) -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        for (rel, content) in files {
            let path = dir.path().join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, content).unwrap();
        }
        let root = dir.path().to_path_buf();
        (dir, root)
    }

    /// Render one symbol's annotation from a `DetailAnnotations` with a FRESH per-file dedup
    /// map (so a single-symbol lookup always yields the full block, never a back-reference).
    pub(in crate::callers) fn note(ann: &DetailAnnotations, file_path: &str, start: usize) -> String {
        let mut seen = CallerBlockDedup::new();
        ann.render(file_path, start, &seen)
            .map(|prepared| {
                let text = prepared.text().to_string();
                prepared.commit(&mut seen);
                text
            })
            .unwrap_or_default()
    }

    /// Whether a symbol has an annotation at all (the render-time analog of the old `get`).
    pub(in crate::callers) fn has_note(ann: &DetailAnnotations, file_path: &str, start: usize) -> bool {
        let seen = CallerBlockDedup::new();
        ann.render(file_path, start, &seen).is_some()
    }

    /// Render a sequence of `(file_path, start_line)` symbols through the SAME per-file dedup
    /// map, in the given order — the exact emission contract the renderer (`mcp.rs`) follows:
    /// `render` to get the prepared text, emit it, then `commit`. Returns the emitted text per
    /// symbol (empty string when a symbol has no annotation). This is the test harness for the
    /// P2 render-order dedup: it lets a test choose the emission order (and skip symbols that
    /// the renderer would suppress) and assert the back-reference integrity.
    pub(in crate::callers) fn render_in_order(
        ann: &DetailAnnotations,
        file_path: &str,
        starts: &[usize],
    ) -> Vec<String> {
        let mut seen = CallerBlockDedup::new();
        let mut out = Vec::new();
        for &start in starts {
            match ann.render(file_path, start, &seen) {
                Some(prepared) => {
                    let text = prepared.text().to_string();
                    prepared.commit(&mut seen);
                    out.push(text);
                }
                None => out.push(String::new()),
            }
        }
        out
    }
}
