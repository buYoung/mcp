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

use crate::parser::{ExtractedFile, ExtractedSymbol};
use grep::regex::RegexMatcherBuilder;
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkMatch};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Tunable caps for one annotation pass. Sourced from `config.rs` so a repo can retune
/// them; defaults: scan_cap 500, list caps 5, sub-budget 8192, common threshold 2.
#[derive(Debug, Clone, Copy)]
pub struct CallerConfig {
    /// Overall hit-collection budget for one combined-regex scan, distributed across the
    /// scanned names (per-name cap = `scan_cap / names`, floored at
    /// [`MIN_PER_NAME_SCAN_HITS`]) so a hot name cannot starve the others. Per-name
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

/// A single call-site / reference hit produced by the combined-regex scan.
#[derive(Debug, Clone)]
struct ScanHit {
    /// The matched symbol name this hit belongs to.
    name: String,
    /// Workspace-relative (display) path of the file the hit was found in.
    file_path: String,
    /// 1-based line number of the hit.
    line_number: usize,
    /// True when the first non-whitespace char after the name was `(` (a call site);
    /// false marks a non-call reference (callback / handler registration / passing).
    is_call: bool,
    /// The raw matched line text (for non-call-reference rendering).
    line_text: String,
}

/// Sink that records, per matched line, every name-occurrence and whether it is a call
/// site (`name(`) or a non-call reference. One sink per file (the `grep.rs` pattern).
///
/// Budgets are PER NAME (parallel to `names`): a hot name exhausting its own budget can
/// no longer starve every other name of the walk's remaining files — which used to leave
/// a later-in-walk symbol with zero collected hits and a misleading "no direct caller
/// observed" despite real call sites.
struct ClassifySink<'a> {
    names: &'a [String],
    file_path: String,
    /// Remaining hit budget per name (same index as `names`). Decremented on push.
    budgets: &'a mut [usize],
    hits: Vec<ScanHit>,
    /// Per-name flag (same index as `names`): set the moment a hit for that name is
    /// dropped because its budget was already exhausted.
    truncated: &'a mut [bool],
}

impl<'a> Sink for ClassifySink<'a> {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, std::io::Error> {
        let start = mat.line_number().unwrap_or(0) as usize;
        let cow = String::from_utf8_lossy(mat.bytes());
        let block: &str = cow.as_ref();
        for (offset, raw) in block.split('\n').enumerate() {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            let line_number = start + offset;
            // Find every occurrence of every scanned name on this line, classifying each
            // by the first non-whitespace char after it. The combined regex guarantees at
            // least one name is present; we re-locate to read the trailing char and to
            // attribute the hit to a specific name.
            for (name_idx, name) in self.names.iter().enumerate() {
                let mut from = 0usize;
                while let Some(rel) = line[from..].find(name.as_str()) {
                    let at = from + rel;
                    from = at + name.len();
                    // Word-boundary guard: the char immediately before/after the match must
                    // not be an identifier char, so `new` does not match inside `renew`.
                    let before_ok = at == 0
                        || !is_ident_char(line[..at].chars().next_back().unwrap_or(' '));
                    let after_idx = at + name.len();
                    let after_char = line[after_idx..].chars().next();
                    let after_ok = after_char.map(|c| !is_ident_char(c)).unwrap_or(true);
                    if !before_ok || !after_ok {
                        continue;
                    }
                    // First non-whitespace char after the name decides call vs reference.
                    let trailing = line[after_idx..].trim_start().chars().next();
                    let is_call = trailing == Some('(');
                    if self.budgets[name_idx] == 0 {
                        self.truncated[name_idx] = true;
                        continue;
                    }
                    self.budgets[name_idx] -= 1;
                    self.hits.push(ScanHit {
                        name: name.clone(),
                        file_path: self.file_path.clone(),
                        line_number,
                        is_call,
                        line_text: line.to_string(),
                    });
                }
            }
        }
        Ok(true)
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Outcome of the single workspace scan: every classified hit plus the names whose
/// per-name hit budget was exhausted (their lists may be incomplete).
struct ScanResult {
    hits: Vec<ScanHit>,
    truncated_names: HashSet<String>,
}

/// Floor of the per-name scan budget. `scan_cap` divided across many names could leave
/// a name too few hits to survive downstream filtering (definition headers and
/// recursion hits are collected, then excluded from caller lists), so each name is
/// guaranteed at least this many.
const MIN_PER_NAME_SCAN_HITS: usize = 25;

/// Regex-escape one identifier so a name carrying regex metacharacters (JS/TS `$`, or a
/// stray `.`) is matched literally inside the combined alternation.
fn escape_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// One combined-regex `\b(?:n1|n2|…)\b` walk of the workspace, classifying every hit.
/// Returns `None` on any setup failure (failure isolation). The walk reuses
/// `build_walker` + the source-extension + size filters so coverage matches the indexer.
fn scan_workspace(names: &[String], cfg: &CallerConfig, root: &Path) -> Option<ScanResult> {
    if names.is_empty() {
        return Some(ScanResult {
            hits: Vec::new(),
            truncated_names: HashSet::new(),
        });
    }
    let alternation = names
        .iter()
        .map(|n| escape_name(n))
        .collect::<Vec<_>>()
        .join("|");
    let pattern = format!(r"\b(?:{alternation})\b");

    let mut matcher_builder = RegexMatcherBuilder::new();
    matcher_builder.line_terminator(Some(b'\n'));
    let matcher = matcher_builder.build(&pattern).ok()?;

    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .binary_detection(BinaryDetection::quit(0))
        .build();

    // Produce workspace-relative display paths that match the codemap snapshot's
    // `file_path` keys via the shared `crate::workspace::workspace_display_path` helper
    // (canonicalize → strip canonical root, falling back to the raw root → backslash→slash).
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let raw_root = root.to_path_buf();

    let mut all_hits: Vec<ScanHit> = Vec::new();
    // `scan_cap` is distributed across the scanned names (with a floor) so one hot name
    // cannot consume the whole budget during the early walk and starve every other name
    // of its later-in-walk call sites.
    let per_name_cap = cfg
        .scan_cap
        .div_ceil(names.len())
        .max(MIN_PER_NAME_SCAN_HITS);
    let mut budgets = vec![per_name_cap; names.len()];
    let mut truncated = vec![false; names.len()];

    for result in crate::workspace::build_walker(root, false).build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();
        // Same coverage as the indexer: source extensions only, under the size filter.
        let ext = match path.extension().and_then(|s| s.to_str()) {
            Some(e) => e,
            None => continue,
        };
        if !crate::workspace::is_source_extension(ext) {
            continue;
        }
        match std::fs::metadata(path) {
            Ok(meta) if meta.len() > cfg.max_file_size => continue,
            Ok(_) => {}
            Err(_) => continue,
        }
        let display = crate::workspace::workspace_display_path(path, &canonical_root, &raw_root);
        let mut sink = ClassifySink {
            names,
            file_path: display,
            budgets: &mut budgets,
            hits: Vec::new(),
            truncated: &mut truncated,
        };
        // A per-file searcher error is isolated: skip the file, keep scanning.
        if searcher.search_path(&matcher, path, &mut sink).is_err() {
            continue;
        }
        all_hits.append(&mut sink.hits);
        // Every name's budget exhausted → nothing further can be collected. Unscanned
        // files may still hold hits, so every name is conservatively marked truncated.
        if budgets.iter().all(|&b| b == 0) {
            truncated.iter_mut().for_each(|t| *t = true);
            break;
        }
    }

    let truncated_names: HashSet<String> = names
        .iter()
        .zip(&truncated)
        .filter(|(_, &t)| t)
        .map(|(n, _)| n.clone())
        .collect();
    Some(ScanResult {
        hits: all_hits,
        truncated_names,
    })
}

/// Whether a hit falls inside the line range of ANY `fn` definition carrying `name` in the
/// hit's own file. A definition header (`fn name(`) classifies as a call site, and a call
/// inside a same-named body is (self-)recursion — both must be filtered from caller lists.
/// For a unique name this is exactly the old own-range exclusion; for a common name it also
/// covers the sibling definitions.
fn is_within_same_named_fn(hit: &ScanHit, name: &str, index: &SymbolIndex<'_>) -> bool {
    index.by_name.get(name).is_some_and(|defs| {
        defs.iter().any(|(file, def)| {
            def.kind == "fn"
                && file.file_path == hit.file_path
                && def.range.start_line <= hit.line_number
                && hit.line_number <= def.range.end_line
        })
    })
}

/// The innermost `fn`-scope symbol whose inclusive line range contains `line` in `file`.
/// Smallest span wins (innermost nesting), tie-broken by `range_strictly_contains`. The
/// inclusive test (`start <= line <= end`) keeps single-line callables attributable.
fn enclosing_fn<'a>(file: &'a ExtractedFile, line: usize) -> Option<&'a ExtractedSymbol> {
    let mut best: Option<&ExtractedSymbol> = None;
    for sym in &file.symbols {
        if sym.kind != "fn" {
            continue;
        }
        let (start, end) = (sym.range.start_line, sym.range.end_line);
        if start <= line && line <= end {
            best = match best {
                None => Some(sym),
                Some(current) => {
                    // Prefer the strictly-inner one; on equal spans keep the first found.
                    if crate::parser::range_strictly_contains(&current.range, &sym.range) {
                        Some(sym)
                    } else {
                        Some(current)
                    }
                }
            };
        }
    }
    best
}

/// Render a symbol's display name, prefixed by its `owner` when present:
/// Rust/Go → `Owner::name`, class-nested languages → `Owner.name`. The separator is
/// chosen by extension so the rendered form matches each language's convention.
fn qualified_name(sym: &ExtractedSymbol, file_path: &str) -> String {
    match &sym.owner {
        Some(owner) => {
            let sep = match extension_of(file_path) {
                "rs" => "::",
                _ => ".",
            };
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
/// so a name appearing there is excluded from non-call-reference reporting. Conservative:
/// an unclassifiable line stays in (degrades to the hedged "possible callback" wording).
fn is_import_line(line: &str, ext: &str) -> bool {
    let trimmed = line.trim_start();
    match ext {
        "rs" => trimmed.starts_with("use "),
        "py" => trimmed.starts_with("import ") || trimmed.starts_with("from "),
        "ts" | "tsx" | "js" | "jsx" => {
            trimmed.starts_with("import ") || trimmed.starts_with("require(")
                || trimmed.contains("require(")
        }
        "go" => trimmed.starts_with("import ") || trimmed.starts_with("import("),
        "java" | "kt" | "kts" => trimmed.starts_with("import "),
        // C/C++: `#include` is the only import-equivalent construct.
        "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => {
            trimmed.starts_with("#include")
        }
        // Assembly: `.include` embeds another assembly file.
        "s" | "S" | "asm" => trimmed.starts_with(".include"),
        _ => false,
    }
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

/// Discover depth-1 callees of `sym`: names invoked as `identifier(` inside the symbol's
/// full source range that are in the snapshot's global `fn`-name set, excluding the
/// symbol's own name. Reads the symbol's full range from disk (not the display snippet).
fn discover_callees(
    sym: &ExtractedSymbol,
    file_path: &str,
    fn_names: &HashSet<String>,
    root: &Path,
) -> Vec<String> {
    let content = match read_workspace_file(file_path, root) {
        Some(c) => c,
        None => return Vec::new(),
    };
    let lines: Vec<&str> = content.lines().collect();
    let start = sym.range.start_line.saturating_sub(1);
    let end = sym.range.end_line.min(lines.len());
    if start >= end {
        return Vec::new();
    }
    let body = lines[start..end].join("\n");
    let mut found: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let bytes: Vec<char> = body.chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        if is_ident_start(bytes[i]) {
            let begin = i;
            while i < bytes.len() && is_ident_char(bytes[i]) {
                i += 1;
            }
            let ident: String = bytes[begin..i].iter().collect();
            // Skip whitespace, then require `(` for a call.
            let mut j = i;
            while j < bytes.len() && (bytes[j] == ' ' || bytes[j] == '\t') {
                j += 1;
            }
            let is_call = j < bytes.len() && bytes[j] == '(';
            if is_call
                && ident != sym.name
                && fn_names.contains(&ident)
                && seen.insert(ident.clone())
            {
                found.push(ident);
            }
        } else {
            i += 1;
        }
    }
    found
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

/// A per-symbol view of where every snapshot symbol of a given name lives, used to
/// resolve a bare callee name to its qualified form and to count definitions.
struct SymbolIndex<'a> {
    /// name → all snapshot symbols (any kind) carrying that name.
    by_name: HashMap<&'a str, Vec<(&'a ExtractedFile, &'a ExtractedSymbol)>>,
    /// Global set of `fn` names (callee intersection target).
    fn_names: HashSet<String>,
    /// name → count of `fn` definitions (common-name threshold input).
    fn_def_counts: HashMap<String, usize>,
}

fn build_symbol_index(snapshot: &[ExtractedFile]) -> SymbolIndex<'_> {
    let mut by_name: HashMap<&str, Vec<(&ExtractedFile, &ExtractedSymbol)>> = HashMap::new();
    let mut fn_names: HashSet<String> = HashSet::new();
    let mut fn_def_counts: HashMap<String, usize> = HashMap::new();
    for file in snapshot {
        for sym in &file.symbols {
            by_name.entry(sym.name.as_str()).or_default().push((file, sym));
            if sym.kind == "fn" {
                fn_names.insert(sym.name.clone());
                *fn_def_counts.entry(sym.name.clone()).or_insert(0) += 1;
            }
        }
    }
    SymbolIndex {
        by_name,
        fn_names,
        fn_def_counts,
    }
}

/// Render the qualified form of a callee name when exactly one `fn` of that name exists in
/// the snapshot (unambiguous owner); otherwise the bare name.
fn callee_display(name: &str, index: &SymbolIndex<'_>) -> String {
    let defs: Vec<_> = index
        .by_name
        .get(name)
        .map(|v| v.iter().filter(|(_, s)| s.kind == "fn").collect::<Vec<_>>())
        .unwrap_or_default();
    if defs.len() == 1 {
        let (file, sym) = defs[0];
        qualified_name(sym, &file.file_path)
    } else {
        name.to_string()
    }
}

/// One-line stand-in emitted when a symbol's full annotation would overflow the remaining
/// byte budget, so an omission is visible instead of silent. Also used by `mcp.rs` at the
/// note-attach point when snippets have since consumed the overall-cap headroom.
pub const ANNOTATION_OMITTED_MARKER: &str =
    "  - _call-context annotation omitted (byte budget exhausted)_\n";

/// The observation-scope caveat appended whenever a `fn` shows no discoverable callers —
/// so a zero is never read as "dead code".
const OBSERVATION_SCOPE_CAVEAT: &str =
    "_(no direct caller observed — scope: indexed source only (rs/py/ts/tsx/js/jsx/go/java/kt/kts), \
direct `name(` calls; callbacks, higher-order/method-reference passing, macro-wrapped, and \
event/dispatch calls are not counted; approximate)_";

/// One matched `fn` symbol's annotation, kept in THREE separable parts so the P2 caller-block
/// dedup can be applied at the actual emission point in render order (not here, in scan order).
///
/// Rendering layout is always `prefix` + (caller block) + `suffix`. The caller block is held
/// apart from the fixed parts because only it is deduped: a later same-named symbol whose
/// `caller_block` is byte-identical to one ALREADY EMITTED collapses to a "same as `name`
/// above" back-reference. `prefix` (decorator/entry-point label) and `suffix` (callees) are
/// never deduped. Holding the parts separate is what guarantees a "same as above" line is
/// only ever emitted AFTER its referenced original block was actually printed — the original
/// is chosen in render order at emission time, so a scan-order original that the renderer
/// later drops (summary container, symbol cap, byte budget) can no longer leave a dangling
/// back-reference (the live A/B "dangling `same as __iter__ above`" defect).
#[derive(Debug, Clone)]
struct SymbolAnnotation {
    /// The matched symbol's name (dedup key for the caller block, per file).
    name: String,
    /// Decorator/entry-point label rendered before the caller block (may be empty).
    prefix: String,
    /// The caller list / omission note / observation caveat block. The only deduped part.
    caller_block: String,
    /// The depth-1 callee block rendered after the caller block (may be empty).
    suffix: String,
}

impl SymbolAnnotation {
    /// Total rendered length (the three parts emitted back-to-back). Used for the byte-budget
    /// accounting in `annotate_results`, which reserves against the FULL (un-deduped) length —
    /// the render-order dedup only ever emits this or LESS, so the reserved cap never overflows.
    fn full_len(&self) -> usize {
        self.prefix.len() + self.caller_block.len() + self.suffix.len()
    }

    /// Render in render order, deduping the caller block against `seen_caller_blocks` (a
    /// per-file `name → already-emitted caller block` map owned by the renderer). When this
    /// symbol's caller block byte-matches one already emitted for the same name, the caller
    /// block collapses to a one-line back-reference; otherwise it renders in full. The fixed
    /// `prefix` / `suffix` always render verbatim.
    ///
    /// The map is NOT mutated here: a full caller block is recorded as the back-reference
    /// target only once the renderer confirms it actually emitted the text (the renderer may
    /// still drop it for byte budget). The second tuple element is that record intent — the
    /// `(name, caller block)` to insert on successful emission, or `None` when this render is
    /// already a back-reference / has no caller block to record.
    fn render(&self, seen_caller_blocks: &HashMap<String, String>) -> (String, Option<(String, String)>) {
        let mut out = String::with_capacity(self.full_len());
        out.push_str(&self.prefix);
        let record = match seen_caller_blocks.get(&self.name) {
            Some(prev) if *prev == self.caller_block && !self.caller_block.is_empty() => {
                out.push_str(&format!("  - _callers: same as `{}` above_\n", self.name));
                None
            }
            _ => {
                out.push_str(&self.caller_block);
                if self.caller_block.is_empty() {
                    None
                } else {
                    Some((self.name.clone(), self.caller_block.clone()))
                }
            }
        };
        out.push_str(&self.suffix);
        (out, record)
    }
}

/// Build the full annotation block for ONE matched `fn` symbol. Returns `None` to omit the
/// annotation entirely (e.g. byte budget already exhausted) — never an error.
///
/// `byte_budget` is the bytes still available for THIS symbol's annotation: the lesser of
/// the remaining `annotation_sub_budget` and the remaining `search_detail_byte_cap` space,
/// computed by the caller (the two-counter). The full rendered length never exceeds it.
///
/// The caller block is returned UN-deduped (separated from the fixed prefix/suffix). The P2
/// "same as above" collapse is deferred to [`SymbolAnnotation::render`] so it runs in the
/// renderer's emission order over only the symbols actually emitted — see the type doc.
fn render_symbol_annotation(
    sym: &ExtractedSymbol,
    file_path: &str,
    scan: &ScanResult,
    snapshot: &[ExtractedFile],
    index: &SymbolIndex<'_>,
    cfg: &CallerConfig,
    byte_budget: usize,
    root: &Path,
) -> Option<SymbolAnnotation> {
    if byte_budget == 0 {
        return None;
    }
    let mut prefix = String::new();

    // --- Decorator / attribute entry-point label (on-demand source re-read). ---
    let decorators = decorator_lines_above(file_path, sym.range.start_line, root);
    if !decorators.is_empty() {
        prefix.push_str(&format!(
            "  - _framework entry-point candidate (verbatim, approximate):_ `{}`\n",
            decorators.join(" ")
        ));
    }

    // --- Callers (built into its own block so identical repeats can be deduped per file). ---
    let own_def_count = *index.fn_def_counts.get(&sym.name).unwrap_or(&0);
    let mut caller_block = String::new();
    // Too-many-definitions short-circuit: with this many same-named `fn`s, a name-match
    // scan cannot attribute any call site to THIS definition, so even a labeled list would
    // mislead. Suppress the caller list and point at `grep` for the real enumeration — never
    // a false "no callers" (guard ④): the note states the omission, the cause, and the
    // alternative. The callee section below still renders; the scan itself ran unchanged.
    if own_def_count >= cfg.caller_omit_def_threshold {
        caller_block.push_str(&format!(
            "  - _callers omitted: `{}` has {} definitions — attribution ambiguous; use grep \"{}(\" to enumerate call sites_\n",
            sym.name, own_def_count, sym.name
        ));
    } else {
    let is_common = own_def_count >= cfg.common_name_threshold;
    // Map this name's call-site hits to their enclosing fn.
    let mut caller_entries: Vec<String> = Vec::new();
    let mut seen_callers: HashSet<String> = HashSet::new();
    for hit in scan.hits.iter().filter(|h| h.name == sym.name && h.is_call) {
        // Exclude hits inside ANY same-named `fn` definition's range: a definition header
        // (`fn name(`) classifies as a call, and a call within a same-named body is
        // (self-)recursion — neither is a caller. Covers the symbol's own range and, for
        // common names, every sibling definition.
        if is_within_same_named_fn(hit, &sym.name, index) {
            continue;
        }
        let file = snapshot.iter().find(|f| f.file_path == hit.file_path);
        let entry = match file.and_then(|f| enclosing_fn(f, hit.line_number)) {
            Some(encl) => {
                let qn = qualified_name(encl, &file.unwrap().file_path);
                format!("{} ({}:{})", qn, hit.file_path, hit.line_number)
            }
            None => {
                // File absent from snapshot, or line in no symbol range → never drop.
                format!(
                    "{}:{} (top-level/unindexed)",
                    hit.file_path, hit.line_number
                )
            }
        };
        if seen_callers.insert(entry.clone()) {
            caller_entries.push(entry);
        }
    }
    let scan_truncated = scan.truncated_names.contains(&sym.name);
    if caller_entries.is_empty() {
        // No direct callers: surface non-call references (the dead-code antidote), then
        // always the observation-scope caveat — never a bare "0 callers".
        let mut refs: Vec<String> = Vec::new();
        let mut seen_refs: HashSet<String> = HashSet::new();
        for hit in scan.hits.iter().filter(|h| h.name == sym.name && !h.is_call) {
            // Exclude references inside same-named definition ranges and import/use lines.
            if is_within_same_named_fn(hit, &sym.name, index) {
                continue;
            }
            let hit_ext = extension_of(&hit.file_path);
            if is_import_line(&hit.line_text, hit_ext) {
                continue;
            }
            let entry = format!(
                "{}:{}: `{}`",
                hit.file_path,
                hit.line_number,
                hit.line_text.trim()
            );
            if seen_refs.insert(entry.clone()) {
                refs.push(entry);
            }
            if refs.len() >= cfg.caller_list_cap {
                break;
            }
        }
        if refs.is_empty() {
            caller_block.push_str(&format!("  - {OBSERVATION_SCOPE_CAVEAT}\n"));
        } else {
            caller_block.push_str(
                "  - _referenced in a non-call position (possible callback / handler registration, approximate):_\n",
            );
            for r in refs {
                caller_block.push_str(&format!("    - {r}\n"));
            }
        }
        // A truncated scan must never read as a confident zero.
        if scan_truncated {
            caller_block.push_str(
                "    - _(caller scan hit its per-name hit cap — sites may have been missed)_\n",
            );
        }
    } else {
        let shown = caller_entries.len().min(cfg.caller_list_cap);
        if is_common {
            // Common matched name: a name-match scan cannot tell which definition each
            // site targets — render the list anyway, labeled, instead of suppressing it.
            caller_block.push_str(&format!(
                "  - _callers (file:line positions exact; name-match attribution approximate — `{}` has {} definitions, call sites may target any of them):_\n",
                sym.name, own_def_count
            ));
        } else {
            caller_block.push_str(
                "  - _callers (file:line positions exact; name-match attribution approximate):_\n",
            );
        }
        for entry in caller_entries.iter().take(shown) {
            caller_block.push_str(&format!("    - {entry}\n"));
        }
        if caller_entries.len() > shown {
            caller_block.push_str(&format!(
                "    - _… {} more not shown._\n",
                caller_entries.len() - shown
            ));
        }
        if scan_truncated {
            caller_block.push_str(
                "    - _(caller scan hit its per-name hit cap — list may be incomplete)_\n",
            );
        }
    }
    } // end caller-list branch (skipped when callers are omitted for too-many-defs)

    // --- Callees (always shown; labeled target-ambiguous at ≥ threshold defs). Held in the
    // fixed `suffix` part — never deduped, always rendered after the (possibly back-referenced)
    // caller block. ---
    let mut suffix = String::new();
    let callees = discover_callees(sym, file_path, &index.fn_names, root);
    if !callees.is_empty() {
        suffix.push_str("  - _calls (depth 1, approximate, name-match only):_\n");
        let shown = callees.len().min(cfg.callee_list_cap);
        for name in callees.iter().take(shown) {
            let def_count = *index.fn_def_counts.get(name).unwrap_or(&0);
            if def_count >= cfg.common_name_threshold {
                suffix.push_str(&format!(
                    "    - {name} ({def_count} defs, target ambiguous)\n"
                ));
            } else {
                suffix.push_str(&format!("    - {}\n", callee_display(name, index)));
            }
        }
        if callees.len() > shown {
            suffix.push_str(&format!(
                "    - _… {} more not shown._\n",
                callees.len() - shown
            ));
        }
    }

    let annotation = SymbolAnnotation {
        name: sym.name.clone(),
        prefix,
        caller_block,
        suffix,
    };
    // Budget check against the FULL (un-deduped) length: the render-order dedup only ever
    // emits this length or LESS, so reserving the full length keeps the cap safe. Over budget
    // → degrade to the one-line marker when even that fits (a visible omission, never silent),
    // else drop entirely. Snippets keep priority; never a partial line. The marker is carried
    // as a self-contained `prefix` (no caller block, so it is never deduped or back-referenced).
    if annotation.full_len() > byte_budget {
        if ANNOTATION_OMITTED_MARKER.len() <= byte_budget {
            return Some(SymbolAnnotation {
                name: sym.name.clone(),
                prefix: ANNOTATION_OMITTED_MARKER.to_string(),
                caller_block: String::new(),
                suffix: String::new(),
            });
        }
        return None;
    }
    Some(annotation)
}

/// One matched-file's identity for annotation lookup: its workspace-relative path plus the
/// list of its non-fallback matched `fn` symbols.
pub struct AnnotationRequest<'a> {
    pub file_path: &'a str,
    pub symbols: &'a [ExtractedSymbol],
    /// `symbol_fallback` results are not annotated (ranked in via path/docstring).
    pub is_fallback: bool,
}

/// The opt-in scan/annotation result for a whole detail view. `annotations` maps
/// `(file_path, symbol_start_line)` to its (un-deduped) [`SymbolAnnotation`]. Performs EXACTLY
/// ONE workspace walk across all matched `fn` names of all detail files. Returns `None` on any
/// failure (the caller then renders the un-annotated detail view).
///
/// The P2 caller-block dedup is NOT applied here — it is applied by [`Self::render`] at the
/// renderer's emission point, in render order, over only the symbols actually emitted (the
/// renderer skips summary containers / cap overflow). This is what prevents a "same as `name`
/// above" back-reference whose original block was never emitted (the live A/B dangling defect).
pub struct DetailAnnotations {
    annotations: HashMap<(String, usize), SymbolAnnotation>,
}

/// Per-file caller-block dedup state owned by the renderer across one file's emitted symbols:
/// `name → already-emitted caller block`. Construct one per detail file (the "same as above"
/// back-reference only points within the same file), thread it through every
/// [`DetailAnnotations::render`] / [`PreparedAnnotation::commit`] call for that file in
/// emission order, then drop it.
pub type CallerBlockDedup = HashMap<String, String>;

/// A rendered-but-not-yet-committed annotation: the text to emit plus the dedup record intent.
/// The renderer emits [`Self::text`] only if it fits the byte cap, then calls [`Self::commit`]
/// to record the back-reference target — so a full caller block becomes a back-reference target
/// for later same-named symbols ONLY when it was actually emitted (never when the renderer drops
/// it for budget). This is what keeps every "same as `name` above" pointing at a printed block.
pub struct PreparedAnnotation {
    text: String,
    record: Option<(String, String)>,
}

impl PreparedAnnotation {
    /// The rendered annotation text (full caller block, or a back-reference, plus prefix/suffix).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Commit this annotation's caller block as the back-reference target for later same-named
    /// symbols in the same file. Call ONLY after the text was actually emitted. A no-op when the
    /// render was itself a back-reference or carried no caller block.
    pub fn commit(self, seen: &mut CallerBlockDedup) {
        if let Some((name, block)) = self.record {
            seen.insert(name, block);
        }
    }
}

impl DetailAnnotations {
    /// Prepare the annotation for a specific symbol AT ITS EMISSION POINT, deduping its caller
    /// block against the symbols already emitted for this file (`seen`). Returns the prepared
    /// text + record intent, or `None` when the symbol has no annotation. Callers MUST invoke
    /// this exactly once per emitted symbol, in emission order, threading the SAME per-file
    /// `seen` map and calling [`PreparedAnnotation::commit`] after emitting — so the first
    /// EMITTED symbol of a name prints the full caller block and later same-named ones collapse
    /// to a back-reference that always has a real original above it.
    pub fn render(
        &self,
        file_path: &str,
        start_line: usize,
        seen: &CallerBlockDedup,
    ) -> Option<PreparedAnnotation> {
        self.annotations
            .get(&(file_path.to_string(), start_line))
            .map(|ann| {
                let (text, record) = ann.render(seen);
                PreparedAnnotation { text, record }
            })
    }
}

/// Build annotations for every matched `fn` symbol across ALL detail-view result files in a
/// single workspace scan. `available_bytes` is the bytes still free under
/// `search_detail_byte_cap` when annotation begins; this enforces both that overall cap AND
/// the `annotation_sub_budget` (the two-counter, whichever binds first). Snippets keep
/// priority — an annotation that would overflow is dropped, never truncated mid-line.
///
/// Returns `None` on any setup/scan failure so the caller degrades to the un-annotated view.
pub fn annotate_results(
    requests: &[AnnotationRequest<'_>],
    snapshot: &[ExtractedFile],
    cfg: &CallerConfig,
    available_bytes: usize,
    root: &Path,
) -> Option<DetailAnnotations> {
    let index = build_symbol_index(snapshot);

    // Union of every non-fallback matched `fn` name across all detail files → one scan.
    let mut names: Vec<String> = Vec::new();
    for req in requests {
        if req.is_fallback {
            continue;
        }
        for sym in req.symbols.iter().filter(|s| s.kind == "fn") {
            names.push(sym.name.clone());
        }
    }
    names.sort();
    names.dedup();
    let scan = scan_workspace(&names, cfg, root)?;

    let mut annotations: HashMap<(String, usize), SymbolAnnotation> = HashMap::new();
    // Two-counter: the annotation budget is the smaller of the sub-budget and the
    // remaining overall-cap space; both deplete as annotations are reserved. Reservation is
    // against the FULL (un-deduped) length — the render-order dedup only ever emits that or
    // less, so the reserved cap is never exceeded at emission time.
    let mut sub_remaining = cfg.annotation_sub_budget;
    let mut overall_remaining = available_bytes;
    for req in requests {
        if req.is_fallback {
            continue;
        }
        for sym in req.symbols.iter().filter(|s| s.kind == "fn") {
            let budget = sub_remaining.min(overall_remaining);
            if budget == 0 {
                break;
            }
            if let Some(annotation) = render_symbol_annotation(
                sym,
                req.file_path,
                &scan,
                snapshot,
                &index,
                cfg,
                budget,
                root,
            ) {
                let reserved = annotation.full_len();
                sub_remaining = sub_remaining.saturating_sub(reserved);
                overall_remaining = overall_remaining.saturating_sub(reserved);
                annotations.insert((req.file_path.to_string(), sym.range.start_line), annotation);
            }
        }
    }
    Some(DetailAnnotations { annotations })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{CodeRange, SymbolFlags};

    fn flags() -> SymbolFlags {
        SymbolFlags {
            has_todo: false,
            has_fixme: false,
            is_test: false,
            is_exported: true,
            is_deprecated: false,
        }
    }

    fn sym(name: &str, kind: &str, start: usize, end: usize, owner: Option<&str>) -> ExtractedSymbol {
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

    fn file(path: &str, symbols: Vec<ExtractedSymbol>) -> ExtractedFile {
        ExtractedFile {
            file_path: path.to_string(),
            total_lines: 100,
            symbols,
            literals: vec![],
            docstrings: vec![],
        }
    }

    fn cfg() -> CallerConfig {
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

    #[test]
    fn test_escape_name_handles_dollar_and_dot() {
        assert_eq!(escape_name("$watch"), r"\$watch");
        assert_eq!(escape_name("a.b"), r"a\.b");
        assert_eq!(escape_name("plain"), "plain");
    }

    #[test]
    fn test_enclosing_fn_inclusive_single_line() {
        // A one-line arrow function: start == end. The inclusive test must attribute a call
        // on that exact line to it (the strict-contains test would drop it).
        let f = file(
            "a.ts",
            vec![sym("handler", "fn", 10, 10, None), sym("outer", "fn", 1, 50, None)],
        );
        let encl = enclosing_fn(&f, 10).unwrap();
        // The innermost (smallest span) wins: handler (10-10), not outer (1-50).
        assert_eq!(encl.name, "handler");
    }

    #[test]
    fn test_enclosing_fn_innermost_wins() {
        let f = file(
            "a.rs",
            vec![sym("outer", "fn", 1, 100, None), sym("inner", "fn", 40, 60, None)],
        );
        assert_eq!(enclosing_fn(&f, 50).unwrap().name, "inner");
        assert_eq!(enclosing_fn(&f, 5).unwrap().name, "outer");
        assert!(enclosing_fn(&f, 200).is_none());
    }

    #[test]
    fn test_qualified_name_from_owner_rust_uses_colons() {
        let s = sym("new", "fn", 5, 8, Some("TantivySearchEngine"));
        assert_eq!(qualified_name(&s, "src/index.rs"), "TantivySearchEngine::new");
    }

    #[test]
    fn test_qualified_name_from_owner_class_uses_dot() {
        let s = sym("render", "fn", 5, 8, Some("Widget"));
        assert_eq!(qualified_name(&s, "src/widget.ts"), "Widget.render");
        let s2 = sym("draw", "fn", 5, 8, Some("Shape"));
        assert_eq!(qualified_name(&s2, "src/shape.py"), "Shape.draw");
    }

    #[test]
    fn test_qualified_name_bare_when_owner_none() {
        let s = sym("free_fn", "fn", 5, 8, None);
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

    #[test]
    fn test_callee_display_unambiguous_qualifies_ambiguous_bare() {
        let snapshot = vec![
            file("a.rs", vec![sym("alpha", "fn", 1, 3, Some("Engine"))]),
            file("b.rs", vec![sym("beta", "fn", 1, 3, None), sym("beta", "fn", 5, 7, None)]),
        ];
        let index = build_symbol_index(&snapshot);
        // alpha: exactly one fn def → qualified via owner.
        assert_eq!(callee_display("alpha", &index), "Engine::alpha");
        // beta: two defs → bare.
        assert_eq!(callee_display("beta", &index), "beta");
    }

    // --- Fixture-based pipeline tests (real on-disk scan, no stubs). ---

    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Write fixture files into a temp dir and return its handle + path.
    fn write_repo(files: &[(&str, &str)]) -> (TempDir, PathBuf) {
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
    fn note(ann: &DetailAnnotations, file_path: &str, start: usize) -> String {
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
    fn has_note(ann: &DetailAnnotations, file_path: &str, start: usize) -> bool {
        let seen = CallerBlockDedup::new();
        ann.render(file_path, start, &seen).is_some()
    }

    #[test]
    fn test_caller_line_shows_enclosing_symbol_and_file_line() {
        // `target_fn` is defined in def.rs and called from inside `caller_fn` in use.rs.
        let (_dir, root) = write_repo(&[
            ("def.rs", "pub fn target_fn() {\n    let x = 1;\n}\n"),
            (
                "use.rs",
                "pub fn caller_fn() {\n    target_fn();\n}\n",
            ),
        ]);
        let snapshot = vec![
            file("def.rs", vec![sym("target_fn", "fn", 1, 3, None)]),
            file("use.rs", vec![sym("caller_fn", "fn", 1, 3, None)]),
        ];
        let requests = vec![AnnotationRequest {
            file_path: "def.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "def.rs", 1);
        assert!(text.contains("callers"), "should have a callers section: {text}");
        assert!(text.contains("caller_fn"), "enclosing symbol name: {text}");
        assert!(text.contains("use.rs:2"), "file:line of the call site: {text}");
        assert!(text.contains("approximate"), "approximate label: {text}");
    }

    #[test]
    fn test_callee_depth_one_d_calls_c() {
        // The requester's example: `d` calls `c`. Annotating `d` must list `c` at depth 1.
        let (_dir, root) = write_repo(&[(
            "chain.rs",
            "pub fn c() {}\npub fn d() {\n    c();\n}\n",
        )]);
        let snapshot = vec![file(
            "chain.rs",
            vec![sym("c", "fn", 1, 1, None), sym("d", "fn", 2, 4, None)],
        )];
        let requests = vec![AnnotationRequest {
            file_path: "chain.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "chain.rs", 2);
        assert!(text.contains("calls (depth 1"), "callee section: {text}");
        assert!(text.contains("- c"), "callee c listed: {text}");
        assert!(text.contains("approximate"), "approximate label: {text}");
    }

    #[test]
    fn test_qualified_caller_method_from_owner() {
        // The caller is a Rust method `Engine::run` calling free `helper`. The owner field
        // (Phase A) must render the caller as `Engine::run`, exercising the owner path.
        let (_dir, root) = write_repo(&[(
            "engine.rs",
            "pub fn helper() {}\nimpl Engine {\n    pub fn run(&self) {\n        helper();\n    }\n}\n",
        )]);
        let snapshot = vec![file(
            "engine.rs",
            vec![
                sym("helper", "fn", 1, 1, None),
                sym("run", "fn", 3, 5, Some("Engine")),
            ],
        )];
        let requests = vec![AnnotationRequest {
            file_path: "engine.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "engine.rs", 1); // helper's annotation lists its callers.
        assert!(
            text.contains("Engine::run"),
            "caller rendered with owner-qualified name: {text}"
        );
    }

    #[test]
    fn test_callee_and_caller_ambiguity_labels_for_common_name() {
        // `make` has two fn defs → as a callee it is labeled target-ambiguous (still shown);
        // as a MATCHED name its callers are rendered with an attribution-ambiguity label —
        // never suppressed. The sibling definition's own header line must not appear as a
        // caller (it classifies as `make(` but sits inside a same-named def range).
        let (_dir, root) = write_repo(&[(
            "amb.rs",
            "pub fn make() {}\npub fn make() {}\npub fn user() {\n    make();\n}\n",
        )]);
        let snapshot = vec![file(
            "amb.rs",
            vec![
                sym("make", "fn", 1, 1, None),
                sym("make", "fn", 2, 2, None),
                sym("user", "fn", 3, 5, None),
            ],
        )];
        // Callee side: annotate `user`, which calls `make` (2 defs).
        let req_user = vec![AnnotationRequest {
            file_path: "amb.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&req_user, &snapshot, &cfg(), 100_000, &root).unwrap();
        let user_text = note(&ann, "amb.rs", 3);
        assert!(
            user_text.contains("make (2 defs, target ambiguous)"),
            "callee target-ambiguity label: {user_text}"
        );
        // Caller side: annotating `make` itself → callers listed with an ambiguity label.
        let make_text = note(&ann, "amb.rs", 1);
        assert!(
            make_text.contains("has 2 definitions"),
            "common matched name → attribution-ambiguity label: {make_text}"
        );
        assert!(
            make_text.contains("user (amb.rs:4)"),
            "the real call site is still listed: {make_text}"
        );
        assert!(
            !make_text.contains("amb.rs:2"),
            "sibling definition header must not be listed as a caller: {make_text}"
        );
    }

    #[test]
    fn test_non_call_reference_label_for_zero_caller_fn() {
        // `handler` is never called as `handler(`, only registered via a callback pass.
        let (_dir, root) = write_repo(&[
            ("h.rs", "pub fn handler() {}\n"),
            (
                "reg.rs",
                "pub fn setup() {\n    register(\"x\", handler);\n}\n",
            ),
        ]);
        let snapshot = vec![
            file("h.rs", vec![sym("handler", "fn", 1, 1, None)]),
            file("reg.rs", vec![sym("setup", "fn", 1, 3, None)]),
        ];
        let requests = vec![AnnotationRequest {
            file_path: "h.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "h.rs", 1);
        assert!(
            text.contains("non-call position"),
            "non-call reference label instead of bare 0 callers: {text}"
        );
        assert!(text.contains("reg.rs:2"), "the raw reference line:line: {text}");
        assert!(!text.contains("0 callers"), "never a bare 0 callers: {text}");
    }

    #[test]
    fn test_decorator_entry_point_label() {
        // A Python `@app.route(...)` decorator directly above the matched fn is surfaced
        // verbatim as a framework entry-point candidate.
        let (_dir, root) = write_repo(&[(
            "app.py",
            "@app.route(\"/health\")\ndef health():\n    return ok\n",
        )]);
        let snapshot = vec![file("app.py", vec![sym("health", "fn", 2, 3, None)])];
        let requests = vec![AnnotationRequest {
            file_path: "app.py",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "app.py", 2);
        assert!(
            text.contains("framework entry-point candidate"),
            "entry-point label: {text}"
        );
        assert!(
            text.contains("@app.route(\"/health\")"),
            "verbatim decorator text: {text}"
        );
    }

    #[test]
    fn test_zero_caller_shows_observation_scope_caveat_never_bare_zero() {
        // `lonely` has no callers and no references anywhere → observation-scope caveat.
        let (_dir, root) = write_repo(&[("lone.rs", "pub fn lonely() {}\n")]);
        let snapshot = vec![file("lone.rs", vec![sym("lonely", "fn", 1, 1, None)])];
        let requests = vec![AnnotationRequest {
            file_path: "lone.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "lone.rs", 1);
        assert!(
            text.contains("no direct caller observed"),
            "observation-scope caveat: {text}"
        );
        assert!(!text.contains("0 callers"), "never a bare 0 callers: {text}");
    }

    #[test]
    fn test_annotation_respects_byte_budget() {
        // With a tiny available budget, no annotation should be emitted (snippets keep
        // priority; an over-budget annotation is dropped, not truncated mid-line).
        let (_dir, root) = write_repo(&[
            ("def.rs", "pub fn target_fn() {}\n"),
            ("use.rs", "pub fn caller_fn() {\n    target_fn();\n}\n"),
        ]);
        let snapshot = vec![
            file("def.rs", vec![sym("target_fn", "fn", 1, 1, None)]),
            file("use.rs", vec![sym("caller_fn", "fn", 1, 3, None)]),
        ];
        let requests = vec![AnnotationRequest {
            file_path: "def.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        // available_bytes = 10 → far below any annotation length → dropped entirely.
        let ann = annotate_results(&requests, &snapshot, &cfg(), 10, &root).unwrap();
        assert!(
            !has_note(&ann, "def.rs", 1),
            "annotation dropped when over budget"
        );
    }

    #[test]
    fn test_scan_failure_isolation_returns_none() {
        // A non-existent root makes the walk yield nothing; the scan itself still succeeds
        // (degrades to empty), so annotation is produced from snapshot-only data with the
        // observation-scope caveat — never a panic, never an error. This proves the
        // never-exit contract: the pipeline degrades rather than failing the response.
        let bogus = PathBuf::from("/nonexistent/path/for/codemap/test");
        let snapshot = vec![file("x.rs", vec![sym("foo", "fn", 1, 1, None)])];
        let requests = vec![AnnotationRequest {
            file_path: "x.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &bogus);
        // Did not panic / error out; produced a degraded annotation.
        let ann = ann.unwrap();
        let text = note(&ann, "x.rs", 1);
        assert!(
            text.contains("no direct caller observed"),
            "degraded to observation-scope caveat: {text}"
        );
    }

    #[test]
    fn test_fallback_results_not_annotated() {
        // A `symbol_fallback` result (ranked via path/docstring) is never annotated.
        let (_dir, root) = write_repo(&[("def.rs", "pub fn target_fn() {}\n")]);
        let snapshot = vec![file("def.rs", vec![sym("target_fn", "fn", 1, 1, None)])];
        let requests = vec![AnnotationRequest {
            file_path: "def.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: true, // fallback → skip
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        assert!(!has_note(&ann, "def.rs", 1), "fallback not annotated");
    }

    #[test]
    fn test_self_recursion_not_counted_as_caller() {
        // A fn calling itself: the call inside its own body must not be reported as a caller.
        let (_dir, root) = write_repo(&[(
            "rec.rs",
            "pub fn recurse(n: u32) {\n    if n > 0 { recurse(n - 1); }\n}\n",
        )]);
        let snapshot = vec![file("rec.rs", vec![sym("recurse", "fn", 1, 3, None)])];
        let requests = vec![AnnotationRequest {
            file_path: "rec.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "rec.rs", 1);
        // The only `recurse(` call is on its own definition line range → excluded → caveat.
        assert!(
            text.contains("no direct caller observed"),
            "self-recursion excluded from callers: {text}"
        );
    }

    /// Render a sequence of `(file_path, start_line)` symbols through the SAME per-file dedup
    /// map, in the given order — the exact emission contract the renderer (`mcp.rs`) follows:
    /// `render` to get the prepared text, emit it, then `commit`. Returns the emitted text per
    /// symbol (empty string when a symbol has no annotation). This is the test harness for the
    /// P2 render-order dedup: it lets a test choose the emission order (and skip symbols that
    /// the renderer would suppress) and assert the back-reference integrity.
    fn render_in_order(
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

    #[test]
    fn test_p2_dedup_full_block_before_back_reference_in_render_order() {
        // Three same-named (< omit-threshold, so the list renders) `tick` fns sharing one caller
        // `driver`. The FIRST emitted in render order must carry the full caller block; the next
        // two collapse to "same as `tick` above". A back-reference must never appear before its
        // original — the live A/B dangling defect.
        let (_dir, root) = write_repo(&[(
            "t.rs",
            "pub fn tick() {}\npub fn tick() {}\npub fn tick() {}\npub fn driver() {\n    tick();\n}\n",
        )]);
        let snapshot = vec![file(
            "t.rs",
            vec![
                sym("tick", "fn", 1, 1, None),
                sym("tick", "fn", 2, 2, None),
                sym("tick", "fn", 3, 3, None),
                sym("driver", "fn", 4, 6, None),
            ],
        )];
        let requests = vec![AnnotationRequest {
            file_path: "t.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        // Emit in line order 1,2,3 (the renderer's outermost-first order).
        let rendered = render_in_order(&ann, "t.rs", &[1, 2, 3]);
        assert!(
            rendered[0].contains("driver (t.rs:5)") && !rendered[0].contains("same as"),
            "first emitted carries the full caller block: {:?}",
            rendered[0]
        );
        assert!(
            rendered[1].contains("same as `tick` above")
                && rendered[2].contains("same as `tick` above"),
            "later same-named symbols collapse to a back-reference: {rendered:?}"
        );
        // Integrity: no "same as above" is emitted before the full block (defect 1).
        let first_back_ref = rendered.iter().position(|t| t.contains("same as"));
        let first_full = rendered
            .iter()
            .position(|t| t.contains("driver (t.rs:5)") && !t.contains("same as"));
        assert!(
            first_full.is_some() && first_full < first_back_ref,
            "the original full block precedes every back-reference: {rendered:?}"
        );
    }

    #[test]
    fn test_p2_dedup_promotes_full_block_when_first_symbol_is_skipped() {
        // The renderer suppresses some symbols (summary containers, cap overflow). When the
        // FIRST same-named symbol in line order is skipped, the next EMITTED one must render the
        // full block — never a dangling back-reference (defects 1 & 3). Emulated by simply not
        // emitting the L1 symbol: render only L2 then L3.
        let (_dir, root) = write_repo(&[(
            "t.rs",
            "pub fn tick() {}\npub fn tick() {}\npub fn tick() {}\npub fn driver() {\n    tick();\n}\n",
        )]);
        let snapshot = vec![file(
            "t.rs",
            vec![
                sym("tick", "fn", 1, 1, None),
                sym("tick", "fn", 2, 2, None),
                sym("tick", "fn", 3, 3, None),
                sym("driver", "fn", 4, 6, None),
            ],
        )];
        let requests = vec![AnnotationRequest {
            file_path: "t.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        // L1 is "skipped" by the renderer (e.g. a summary container) → emit only L2, L3.
        let rendered = render_in_order(&ann, "t.rs", &[2, 3]);
        assert!(
            rendered[0].contains("driver (t.rs:5)") && !rendered[0].contains("same as"),
            "first EMITTED symbol (L2) carries the full block, not a dangling back-ref: {:?}",
            rendered[0]
        );
        // And the common-name label is restored on that promoted full block (defect 3): with 3
        // defs (≥ common_name_threshold 2) the label must carry the def count.
        assert!(
            rendered[0].contains("has 3 definitions"),
            "common-name label present on the promoted full block: {:?}",
            rendered[0]
        );
        assert!(
            rendered[1].contains("same as `tick` above"),
            "the later one back-references the now-emitted original: {rendered:?}"
        );
    }

    #[test]
    fn test_p3_omit_line_renders_in_render_order() {
        // A name with ≥ caller_omit_def_threshold (5) defs: every symbol's caller block is the
        // omission note. The FIRST emitted must show the full omit line (defect 2); the rest
        // back-reference it (they are byte-identical). Even if the renderer skips the first
        // line-order symbol, the omit line must still appear on the first EMITTED one.
        let body = (0..6)
            .map(|_| "pub fn poll() {}\n")
            .collect::<String>();
        let (_dir, root) = write_repo(&[("p.rs", &body)]);
        let symbols: Vec<_> = (0..6)
            .map(|i| sym("poll", "fn", i + 1, i + 1, None))
            .collect();
        let snapshot = vec![file("p.rs", symbols)];
        let requests = vec![AnnotationRequest {
            file_path: "p.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        // Skip the L1 symbol (renderer suppressed it); first emitted is L2.
        let rendered = render_in_order(&ann, "p.rs", &[2, 3, 4]);
        assert!(
            rendered[0].contains("callers omitted: `poll` has 6 definitions"),
            "P3 omit line on the first emitted symbol even when L1 is skipped: {:?}",
            rendered[0]
        );
        assert!(
            rendered[0].contains("use grep \"poll(\""),
            "omit line keeps the grep pointer form: {:?}",
            rendered[0]
        );
        assert!(
            rendered[1].contains("same as `poll` above"),
            "subsequent omit blocks (byte-identical) collapse to a back-reference: {rendered:?}"
        );
    }
}
