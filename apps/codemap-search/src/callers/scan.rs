//! Workspace scan stage: one combined-regex `\b(?:n1|n2|…)\b` walk over the workspace
//! classifying every hit as a call site or a non-call reference. Produces the
//! [`ScanResult`] consumed by the annotate stage.

use grep::matcher::Matcher;
use grep::regex::RegexMatcherBuilder;
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkMatch};
use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

use crate::parser::ExtractedFile;

use super::source::SourceSyntax;
use super::symbols::{definition_is_compatible, language_family, SymbolIndex};
use super::CallerConfig;

/// A single call-site / reference hit produced by the combined-regex scan.
#[derive(Debug, Clone)]
pub(super) struct ScanHit {
    /// The matched symbol name this hit belongs to.
    pub(super) name: String,
    /// Workspace-relative (display) path of the file the hit was found in.
    pub(super) file_path: String,
    /// 1-based line number of the hit.
    pub(super) line_number: usize,
    /// True when the first non-whitespace char after the name was `(` (a call site);
    /// false marks a non-call reference (callback / handler registration / passing).
    pub(super) is_call: bool,
    pub(super) is_member_access: bool,
    /// The raw matched line text (for non-call-reference rendering).
    pub(super) line_text: String,
}

/// Sink that records, per matched line, every name-occurrence and whether it is a call
/// site (`name(`) or a non-call reference. One sink per file (the `grep.rs` pattern).
/// [`Budgets`] then keeps the hits their names' budgets allow, in walk order.
struct ClassifySink<'a> {
    names: &'a [String],
    file_path: String,
    source: &'a [u8],
    syntax: OnceCell<Option<SourceSyntax>>,
    index: &'a SymbolIndex<'a>,
    /// Hits in file order, each with the index of its name in `names`.
    hits: Vec<(usize, ScanHit)>,
}

/// The hits of one searched file, before budgets are applied.
struct FileHits {
    hits: Vec<(usize, ScanHit)>,
    /// The searcher failed on the file; its hits only use up budget.
    is_failed: bool,
}

/// Hit budgets PER NAME (parallel to `names`): a hot name exhausting its own budget can
/// no longer starve every other name of the walk's remaining files — which used to leave
/// a later-in-walk symbol with zero collected hits and a misleading "no direct caller
/// observed" despite real call sites.
struct Budgets {
    /// Remaining hit budget per name. Decremented for each hit taken.
    remaining: Vec<usize>,
    /// Per-name flag: set the moment a hit for that name is dropped because its budget was
    /// already exhausted.
    truncated: Vec<bool>,
}

impl Budgets {
    /// Takes the hits of the next searched file in walk order into `hits`; false once nothing
    /// further can be collected.
    fn take(&mut self, file: FileHits, hits: &mut Vec<ScanHit>) -> bool {
        for (name, hit) in file.hits {
            if self.remaining[name] == 0 {
                self.truncated[name] = true;
                continue;
            }
            self.remaining[name] -= 1;
            if !file.is_failed {
                hits.push(hit);
            }
        }
        // A per-file searcher error is isolated: skip the file, keep scanning.
        if file.is_failed {
            return true;
        }
        // Every name's budget exhausted → nothing further can be collected. Unscanned
        // files may still hold hits, so every name is conservatively marked truncated.
        if self.remaining.iter().all(|&b| b == 0) {
            self.truncated.iter_mut().for_each(|t| *t = true);
            return false;
        }
        true
    }
}

/// Files a scan without a deadline searches together before taking their hits in walk order.
const SCAN_BATCH_FILES: usize = 256;

impl<'a> Sink for ClassifySink<'a> {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, std::io::Error> {
        let start = mat.line_number().unwrap_or(0) as usize;
        let Ok(block) = std::str::from_utf8(mat.bytes()) else {
            return Ok(true);
        };
        // Parse only files with matching names, and only once per file. Classification
        // uses the whole file so multiline comments and raw strings stay intact.
        let Some(syntax) = self
            .syntax
            .get_or_init(|| SourceSyntax::parse(&self.file_path, self.source))
        else {
            return Ok(true);
        };
        let mut line_start = mat.absolute_byte_offset() as usize;
        for (offset, raw) in block.split_inclusive('\n').enumerate() {
            let current_start = line_start;
            line_start += raw.len();
            let raw = raw.strip_suffix('\n').unwrap_or(raw);
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            let line_number = start + offset;
            if !self.index.is_active_line(&self.file_path, line_number) {
                continue;
            }
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
                    let before_ok =
                        at == 0 || !is_ident_char(line[..at].chars().next_back().unwrap_or(' '));
                    let after_idx = at + name.len();
                    let after_char = line[after_idx..].chars().next();
                    let after_ok = after_char.map(|c| !is_ident_char(c)).unwrap_or(true);
                    if !before_ok || !after_ok {
                        continue;
                    }
                    let name_start = current_start + at;
                    if !syntax.is_code(name_start..name_start + name.len()) {
                        continue;
                    }
                    let is_member_access =
                        syntax.is_member_access(name_start..name_start + name.len());
                    if !self.index.definitions(name).iter().any(|(file, symbol)| {
                        definition_is_compatible(
                            &self.file_path,
                            is_member_access,
                            &file.file_path,
                            symbol,
                        )
                    }) {
                        continue;
                    }
                    // First non-whitespace char after the name decides call vs reference.
                    let trailing = line[after_idx..].trim_start().chars().next();
                    let is_call = trailing == Some('(');
                    self.hits.push((
                        name_idx,
                        ScanHit {
                            name: name.clone(),
                            file_path: self.file_path.clone(),
                            line_number,
                            is_call,
                            is_member_access,
                            line_text: line.to_string(),
                        },
                    ));
                }
            }
        }
        Ok(true)
    }
}

pub(super) fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Outcome of the single workspace scan: every classified hit plus the names whose
/// per-name hit budget was exhausted (their lists may be incomplete).
pub(super) struct ScanResult {
    pub(super) hits: Vec<ScanHit>,
    pub(super) truncated_names: HashSet<String>,
    /// The walk stopped at the scan deadline; every name is also truncated.
    pub(super) is_past_deadline: bool,
}

/// Indexed files by path and the nanosecond modification stamp each had when extracted.
pub(super) struct IndexedFiles<'a> {
    pub(super) files: &'a HashMap<&'a str, &'a ExtractedFile>,
    pub(super) mtime: &'a dyn Fn(&str) -> Option<u64>,
}

/// Shortcuts a scan whose caller only needs call sites may take.
#[derive(Default)]
pub(super) struct ScanOptions<'a> {
    /// An unchanged indexed file whose recorded calls hold none of the scanned names is not
    /// read: caller resolution confirms only calls that live extraction of that same source
    /// finds, and non-call references are not wanted.
    pub(super) unchanged_calls: Option<IndexedFiles<'a>>,
    /// No further file is scanned after this instant.
    pub(super) deadline: Option<Instant>,
    /// Indexed files the filesystem watcher keeps current, with their lookup by recorded call
    /// names. They are scanned in place of a workspace walk, and one whose recorded calls hold
    /// none of the scanned names is skipped before its metadata is read.
    pub(super) indexed_walk: Option<(&'a [std::sync::Arc<ExtractedFile>], &'a super::CallLookup)>,
}

/// Whether `display` is an indexed file that has not changed since extraction and records no
/// call of any scanned name.
fn records_no_scanned_call(
    display: &str,
    metadata: &std::fs::Metadata,
    names: &HashSet<&str>,
    options: &ScanOptions<'_>,
) -> bool {
    let Some(indexed) = &options.unchanged_calls else {
        return false;
    };
    // Files outside source resolution become callers by name, so their text is needed.
    if !super::resolution::supports(display) {
        return false;
    }
    let Some(navigation) = indexed
        .files
        .get(display)
        .and_then(|file| file.navigation.as_ref())
    else {
        return false;
    };
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_nanos() as u64);
    mtime.is_some()
        && (indexed.mtime)(display) == mtime
        && !navigation
            .calls
            .iter()
            .any(|call| names.contains(call.name.as_str()))
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

/// One combined-regex `\b(?:n1|n2|…)\b` walk of the workspace, or of the indexed files the
/// watcher keeps current, classifying every hit. Returns `None` on any setup failure (failure
/// isolation). The walk reuses `build_index_walker` + the source-extension + size filters so
/// coverage matches the indexer.
pub(super) fn scan_workspace(
    names: &[String],
    cfg: &CallerConfig,
    root: &Path,
    index: &SymbolIndex<'_>,
    options: &ScanOptions<'_>,
) -> Option<ScanResult> {
    if names.is_empty() {
        return Some(ScanResult {
            hits: Vec::new(),
            truncated_names: HashSet::new(),
            is_past_deadline: false,
        });
    }
    let name_set: HashSet<&str> = names.iter().map(String::as_str).collect();
    let mut is_past_deadline = false;
    let alternation = names
        .iter()
        .map(|n| escape_name(n))
        .collect::<Vec<_>>()
        .join("|");
    let pattern = format!(r"\b(?:{alternation})\b");

    let mut matcher_builder = RegexMatcherBuilder::new();
    matcher_builder.line_terminator(Some(b'\n'));
    let matcher = matcher_builder.build(&pattern).ok()?;

    let new_searcher = || {
        SearcherBuilder::new()
            .line_number(true)
            .binary_detection(BinaryDetection::quit(0))
            .build()
    };

    // Produce workspace-relative display paths that match the codemap snapshot's
    // `file_path` keys via the shared `crate::workspace::workspace_display_path` helper
    // (canonicalize → strip canonical root, falling back to the raw root → backslash→slash).
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let raw_root = root.to_path_buf();
    let test_filter = super::test_code::TestCodeFilter::from_config(root);

    let mut all_hits: Vec<ScanHit> = Vec::new();
    // `scan_cap` is distributed across the scanned names (with a floor) so one hot name
    // cannot consume the whole budget during the early walk and starve every other name
    // of its later-in-walk call sites.
    let per_name_cap = cfg
        .scan_cap
        .div_ceil(names.len())
        .max(MIN_PER_NAME_SCAN_HITS);
    let mut budgets = Budgets {
        remaining: vec![per_name_cap; names.len()],
        truncated: vec![false; names.len()],
    };
    // The classifier already rejects unrelated runtimes. Apply that same boundary
    // before opening files, especially in generated repositories with many languages.
    let families: HashSet<_> = names
        .iter()
        .flat_map(|name| index.definitions(name))
        .filter_map(|(file, _)| language_family(Path::new(&file.file_path)))
        .collect();

    // Same coverage as the indexer: source extensions only, under the size filter.
    let is_scanned_source = |path: &Path| {
        !crate::workspace::is_explicitly_excluded_file(path)
            && crate::lang::supports_caller_scan(path)
            && language_family(path).is_some_and(|family| families.contains(family))
    };
    let metadata_under_cap = |path: &Path| {
        std::fs::metadata(path)
            .ok()
            .filter(|meta| meta.len() <= cfg.max_file_size)
    };
    let is_past = || {
        options
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    };
    // Reads and searches one file that is not skipped; `None` when it holds no match.
    let search_file = |searcher: &mut Searcher, path: &Path, display: String| {
        let mut source = std::fs::read(path).ok()?;
        // Only matching source can contribute a caller; parse test regions on demand.
        if !matcher.is_match(&source).unwrap_or(false) {
            return None;
        }
        test_filter.mask_source(&display, &mut source);
        let mut sink = ClassifySink {
            names,
            file_path: display,
            source: &source,
            syntax: OnceCell::new(),
            index,
            hits: Vec::new(),
        };
        let is_failed = searcher.search_slice(&matcher, &source, &mut sink).is_err();
        Some(FileHits {
            hits: sink.hits,
            is_failed,
        })
    };
    let mut searcher = new_searcher();
    // Scans one file; false once nothing further can be collected.
    let scan_file = |searcher: &mut Searcher,
                     budgets: &mut Budgets,
                     all_hits: &mut Vec<ScanHit>,
                     path: &Path,
                     display: String,
                     metadata: std::fs::Metadata| {
        if test_filter.is_file_excluded(&display)
            || records_no_scanned_call(&display, &metadata, &name_set, options)
        {
            return true;
        }
        search_file(searcher, path, display).is_none_or(|file| budgets.take(file, all_hits))
    };
    match options.indexed_walk {
        // Without a deadline or skipped files the outcome does not depend on how long a file
        // takes, so a batch of files is searched on all cores and its hits are taken in walk
        // order.
        None if options.deadline.is_none() && options.unchanged_calls.is_none() => {
            let threads = crate::parallel::threads();
            let mut walk = crate::workspace::build_index_walker(root).build();
            let mut batch = Vec::with_capacity(SCAN_BATCH_FILES);
            loop {
                let result = walk.next();
                let is_walked = result.is_none();
                if let Some(entry) = result.and_then(Result::ok) {
                    let path = entry.path();
                    if entry.file_type().is_some_and(|ft| ft.is_file()) && is_scanned_source(path) {
                        let display = crate::workspace::shown_path(
                            crate::workspace::walk_entry_display_path(
                                &entry,
                                &canonical_root,
                                &raw_root,
                            ),
                        );
                        batch.push((entry.into_path(), display));
                    }
                }
                if batch.len() < SCAN_BATCH_FILES && !is_walked {
                    continue;
                }
                let found = crate::parallel::map_in_order(
                    &batch,
                    threads,
                    new_searcher,
                    |searcher, (path, display)| {
                        metadata_under_cap(path)?;
                        if test_filter.is_file_excluded(display) {
                            return None;
                        }
                        search_file(searcher, path, display.clone())
                    },
                );
                batch.clear();
                let is_exhausted = !found
                    .into_iter()
                    .flatten()
                    .all(|file| budgets.take(file, &mut all_hits));
                if is_exhausted || is_walked {
                    break;
                }
            }
        }
        Some((files, lookup)) => {
            for position in lookup.files(names.iter().map(String::as_str), [], true) {
                let file = &files[position];
                if is_past() {
                    is_past_deadline = true;
                    break;
                }
                let display = file.file_path.as_str();
                let path = root.join(display);
                if !is_scanned_source(&path) {
                    continue;
                }
                let Some(metadata) = metadata_under_cap(&path) else {
                    continue;
                };
                if !scan_file(
                    &mut searcher,
                    &mut budgets,
                    &mut all_hits,
                    &path,
                    display.to_string(),
                    metadata,
                ) {
                    break;
                }
            }
        }
        None => {
            for result in crate::workspace::build_index_walker(root).build() {
                if is_past() {
                    is_past_deadline = true;
                    break;
                }
                let Ok(entry) = result else {
                    continue;
                };
                if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                    continue;
                }
                let path = entry.path();
                if !is_scanned_source(path) {
                    continue;
                }
                let Some(metadata) = metadata_under_cap(path) else {
                    continue;
                };
                let display = crate::workspace::shown_path(
                    crate::workspace::walk_entry_display_path(&entry, &canonical_root, &raw_root),
                );
                if !scan_file(
                    &mut searcher,
                    &mut budgets,
                    &mut all_hits,
                    path,
                    display,
                    metadata,
                ) {
                    break;
                }
            }
        }
    }
    if is_past_deadline {
        budgets.truncated.iter_mut().for_each(|t| *t = true);
    }

    let truncated_names: HashSet<String> = names
        .iter()
        .zip(&budgets.truncated)
        .filter(|(_, &t)| t)
        .map(|(n, _)| n.clone())
        .collect();
    Some(ScanResult {
        hits: all_hits,
        truncated_names,
        is_past_deadline,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_name_handles_dollar_and_dot() {
        assert_eq!(escape_name("$watch"), r"\$watch");
        assert_eq!(escape_name("a.b"), r"a\.b");
        assert_eq!(escape_name("plain"), "plain");
    }
}
