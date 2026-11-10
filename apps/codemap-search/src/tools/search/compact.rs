//! Compact ranked page: one short entry per listed detail file plus one-hop connected
//! locations as rows. Used only when the full page exceeds the compact threshold; ranking,
//! selection, file order and the tail stay exactly as planned for the full page.
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

/// Declarations listed per file before a `+N more` count.
pub(super) const SYMBOL_LIMIT: usize = 4;
/// Matching source lines previewed per file.
pub(super) const EVIDENCE_ROWS: usize = 2;
/// Characters kept per preview row or name.
pub(super) const ROW_MAX_CHARS: usize = 160;
/// Connected-location rows after the details, tail and phrase matches.
const CONNECTED_ROW_LIMIT: usize = 24;
const CONNECTED_BYTE_CAP: usize = 3 * 1024;
const LEGEND: &str = "\nBody deferred by delivery budget, not a relevance judgment; listed reads restore unshown source.\n";
const CONNECTED_HEADING: &str = "\n## Connected locations\n";

static PATHLESS_LOCATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"— L([0-9]+)").unwrap());
static LOCATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([^\s()\[\]`]+\.[A-Za-z0-9]+):([0-9]+)").unwrap());
static DISPLAY_LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\[[A-Z][0-9]+(?: applies here)?\]").unwrap());

pub(super) struct CompactEntry {
    pub text: String,
    pub path: String,
    pub listed: Vec<super::grouped::BlockSymbol>,
    pub shown_lines: Vec<usize>,
    pub evidence_bytes: usize,
}

pub(super) fn truncate(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
    }
}

/// A compact page under construction: notices, title, legend and one entry per detail
/// file. The caller appends what follows the files (tail or omission note, candidates,
/// phrase matches) and then [`CompactPage::finish`] adds the connected locations.
pub(super) struct CompactPage {
    pub text: String,
    entries: Vec<CompactEntry>,
    printed: HashSet<(String, usize)>,
    source_files: Vec<crate::analyze::FileObservation>,
}

impl CompactPage {
    /// `prefix` holds the readiness/scope notices and the title of the full page.
    pub(super) fn new(prefix: String, entries: Vec<CompactEntry>) -> Self {
        let mut text = prefix;
        text.push_str(LEGEND);
        let mut printed = HashSet::new();
        let mut source_files = Vec::new();
        for entry in &entries {
            text.push_str(&entry.text);
            printed.extend(
                entry
                    .shown_lines
                    .iter()
                    .map(|line| (entry.path.clone(), *line)),
            );
            if entry.evidence_bytes > 0 {
                source_files.push(crate::analyze::FileObservation {
                    path: entry.path.clone(),
                    result_bytes: entry.evidence_bytes as u64,
                });
            }
        }
        Self {
            text,
            entries,
            printed,
            source_files,
        }
    }

    /// Append the connected locations when they fit `byte_cap`; they never shorten what
    /// the page already holds.
    pub(super) fn finish(
        self,
        relations_by_file: &[Vec<String>],
        caller_annotations: Option<&crate::callers::DetailAnnotations>,
        byte_cap: usize,
    ) -> super::SearchOutput {
        let Self {
            mut text,
            entries,
            printed,
            source_files,
        } = self;
        debug_assert_eq!(entries.len(), relations_by_file.len());
        let connected =
            connected_locations(&entries, relations_by_file, caller_annotations, printed);
        if !connected.is_empty() && text.len() + connected.len() <= byte_cap {
            text.push_str(&connected);
        }
        super::SearchOutput { text, source_files }
    }
}

/// Rows for what the full page would have attached to the listed declarations within its
/// budget order: caller sites when caller context is on, then the planned static,
/// event and implementation relations. Rows whose location is already printed are skipped.
fn connected_locations(
    entries: &[CompactEntry],
    relations_by_file: &[Vec<String>],
    caller_annotations: Option<&crate::callers::DetailAnnotations>,
    mut printed: HashSet<(String, usize)>,
) -> String {
    let mut rows = String::new();
    let mut count = 0;
    let mut push = |key: (String, usize), row: String, rows: &mut String| {
        if count >= CONNECTED_ROW_LIMIT
            || rows.len() + row.len() > CONNECTED_BYTE_CAP
            || !printed.insert(key)
        {
            return;
        }
        count += 1;
        rows.push_str(&row);
    };
    for (entry, relations) in entries.iter().zip(relations_by_file) {
        if let Some(annotations) = caller_annotations {
            for symbol in &entry.listed {
                let Some((callers, unconfirmed)) =
                    annotations.callers_at(&entry.path, symbol.start_line)
                else {
                    continue;
                };
                let sites = callers
                    .iter()
                    .map(|site| (site, ""))
                    .chain(unconfirmed.iter().map(|site| (site, " (unconfirmed)")));
                for (site, label) in sites {
                    let caller = site.caller.as_deref().unwrap_or("top-level/unindexed");
                    let row = format!(
                        "- {}:{} {caller} → {}{label}\n",
                        site.path, site.line, symbol.name
                    );
                    push((site.path.clone(), site.line), row, &mut rows);
                }
            }
        }
        for relation in relations {
            for (key, row) in relation_rows(relation, &entry.path) {
                push(key, row, &mut rows);
            }
        }
    }
    if rows.is_empty() {
        String::new()
    } else {
        format!("{CONNECTED_HEADING}{rows}")
    }
}

/// Relation lines that carry a location, as `- …` rows with the file's path made explicit.
fn relation_rows(relations: &str, path: &str) -> Vec<((String, usize), String)> {
    let mut out = Vec::new();
    for line in relations.lines() {
        let line = line.trim_start_matches('#').trim();
        let line = line.strip_prefix("- ").unwrap_or(line);
        if line.starts_with('_') {
            continue;
        }
        let line = DISPLAY_LABEL.replace_all(line, "");
        let line = PATHLESS_LOCATION.replace_all(&line, |captures: &regex::Captures<'_>| {
            format!("— {path}:{}", &captures[1])
        });
        let Some(location) = LOCATION.captures(&line) else {
            continue;
        };
        let Ok(number) = location[2].parse::<usize>() else {
            continue;
        };
        let key = (location[1].to_string(), number);
        out.push((
            key,
            format!("- {}\n", truncate(line.trim(), 2 * ROW_MAX_CHARS)),
        ));
    }
    out
}
