//! Workspace scan stage: one combined-regex `\b(?:n1|n2|…)\b` walk over the workspace
//! classifying every hit as a call site or a non-call reference. Produces the
//! [`ScanResult`] consumed by the annotate stage.

use grep::regex::RegexMatcherBuilder;
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkMatch};
use std::collections::HashSet;
use std::path::Path;

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
    /// The raw matched line text (for non-call-reference rendering).
    pub(super) line_text: String,
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
                    let before_ok =
                        at == 0 || !is_ident_char(line[..at].chars().next_back().unwrap_or(' '));
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

pub(super) fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Outcome of the single workspace scan: every classified hit plus the names whose
/// per-name hit budget was exhausted (their lists may be incomplete).
pub(super) struct ScanResult {
    pub(super) hits: Vec<ScanHit>,
    pub(super) truncated_names: HashSet<String>,
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
pub(super) fn scan_workspace(
    names: &[String],
    cfg: &CallerConfig,
    root: &Path,
) -> Option<ScanResult> {
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
        if crate::workspace::is_explicitly_excluded_file(path) {
            continue;
        }
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
