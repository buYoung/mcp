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

/// A row of the connected locations and the location it names.
type LocationRow = ((String, usize), String);

/// The connected-location rows collected so far, within [`CONNECTED_ROW_LIMIT`] and
/// [`CONNECTED_BYTE_CAP`]. A location the page already shows is not listed again.
struct ConnectedRows {
    text: String,
    count: usize,
    printed: HashSet<(String, usize)>,
}

impl ConnectedRows {
    fn has_room(&self, rows: usize, bytes: usize) -> bool {
        self.count + rows <= CONNECTED_ROW_LIMIT && self.text.len() + bytes <= CONNECTED_BYTE_CAP
    }

    fn push(&mut self, (key, row): LocationRow) -> bool {
        if !self.has_room(1, row.len()) || !self.printed.insert(key) {
            return false;
        }
        self.count += 1;
        self.text.push_str(&row);
        true
    }

    /// Adds `row` below the heading of its section. The heading is listed even when the page
    /// already shows its location, because the row does not say what it belongs to.
    fn push_under(&mut self, heading: &LocationRow, (key, row): LocationRow) -> bool {
        if !self.has_room(2, heading.1.len() + row.len()) || !self.printed.insert(key) {
            return false;
        }
        self.printed.insert(heading.0.clone());
        self.count += 2;
        self.text.push_str(&heading.1);
        self.text.push_str(&row);
        true
    }
}

/// Rows for what the full page would have attached to the listed declarations within its
/// budget order: caller sites when caller context is on, then the planned static,
/// event and implementation relations. Rows whose location is already printed are skipped.
/// A section heading is listed with the first row of its section, so that no row comes
/// without what it belongs to and no heading comes without a relation; the heading of an
/// abstract declaration is a row in its own right.
fn connected_locations(
    entries: &[CompactEntry],
    relations_by_file: &[Vec<String>],
    caller_annotations: Option<&crate::callers::DetailAnnotations>,
    printed: HashSet<(String, usize)>,
) -> String {
    let mut rows = ConnectedRows {
        text: String::new(),
        count: 0,
        printed,
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
                    rows.push(((site.path.clone(), site.line), row));
                }
            }
        }
        for relation in relations {
            // The heading of the current section while it is not listed yet.
            let mut heading: Option<LocationRow> = None;
            for line in relation_lines(relation, &entry.path) {
                match line {
                    RelationLine::Heading(row) => {
                        heading = row.filter(|row| {
                            !(row
                                .1
                                .contains(crate::implementations::ABSTRACT_DECLARATION_LABEL)
                                && rows.push(row.clone()))
                        });
                    }
                    RelationLine::Row(row) => match heading.take() {
                        Some(pending) => {
                            if !rows.push_under(&pending, row) {
                                heading = Some(pending);
                            }
                        }
                        None => {
                            rows.push(row);
                        }
                    },
                }
            }
        }
    }
    if rows.text.is_empty() {
        String::new()
    } else {
        format!("{CONNECTED_HEADING}{}", rows.text)
    }
}

enum RelationLine {
    /// The heading of a section, with its row when it names a location.
    Heading(Option<LocationRow>),
    /// A line of the current section that names a location.
    Row(LocationRow),
}

/// The section headings of a relation text and the lines that carry a location, as `- …`
/// rows with the file's path made explicit.
fn relation_lines(relations: &str, path: &str) -> Vec<RelationLine> {
    let mut out = Vec::new();
    for line in relations.lines() {
        let is_heading = line.starts_with('#');
        let line = line.trim_start_matches('#').trim();
        let line = line.strip_prefix("- ").unwrap_or(line);
        if line.starts_with('_') {
            continue;
        }
        let line = DISPLAY_LABEL.replace_all(line, "");
        let line = PATHLESS_LOCATION.replace_all(&line, |captures: &regex::Captures<'_>| {
            format!("— {path}:{}", &captures[1])
        });
        let row = LOCATION.captures(&line).and_then(|location| {
            let number = location[2].parse::<usize>().ok()?;
            Some((
                (location[1].to_string(), number),
                format!("- {}\n", truncate(line.trim(), 2 * ROW_MAX_CHARS)),
            ))
        });
        match (is_heading, row) {
            (true, row) => out.push(RelationLine::Heading(row)),
            (false, Some(row)) => out.push(RelationLine::Row(row)),
            (false, None) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> CompactEntry {
        CompactEntry {
            text: String::new(),
            path: path.to_string(),
            listed: Vec::new(),
            shown_lines: Vec::new(),
            evidence_bytes: 0,
        }
    }

    fn connected(relations: &str, printed: &[(&str, usize)]) -> String {
        let printed = printed
            .iter()
            .map(|(path, line)| (path.to_string(), *line))
            .collect();
        connected_locations(
            &[entry("src/scene.ts")],
            &[vec![relations.to_string()]],
            None,
            printed,
        )
    }

    #[test]
    fn test_heading_without_relation_row_is_not_listed() {
        // The byte cap of the relation text cut the section after its heading.
        let relations = "#### Implementations\n\n##### Panel.title — L12\n- implements/overrides declaration: Titled.title — src/titled.ts:3\n\n##### Scene.switchLayout — L40\n\n[Implementation context truncated; narrow the source window or workspace.]\n";
        assert_eq!(
            connected(relations, &[]),
            "\n## Connected locations\n- Panel.title — src/scene.ts:12\n- implements/overrides declaration: Titled.title — src/titled.ts:3\n"
        );
    }

    #[test]
    fn test_relation_row_keeps_heading_already_on_page() {
        let relations = "#### Implementations\n\n##### Queues.GetQueue — L30 [abstract declaration]\n- implementation candidate: Builder.GetQueue — src/builder.go:77\n";
        assert_eq!(
            connected(relations, &[("src/scene.ts", 30)]),
            "\n## Connected locations\n- Queues.GetQueue — src/scene.ts:30 [abstract declaration]\n- implementation candidate: Builder.GetQueue — src/builder.go:77\n"
        );
    }

    #[test]
    fn test_abstract_heading_is_listed_alone() {
        let relations = "#### Implementations\n\n##### Queues.GetQueue — L30 [abstract declaration]\n- implementation: unresolved (no eligible source-proven candidate in this snapshot).\n";
        assert_eq!(
            connected(relations, &[]),
            "\n## Connected locations\n- Queues.GetQueue — src/scene.ts:30 [abstract declaration]\n"
        );
        assert_eq!(connected(relations, &[("src/scene.ts", 30)]), "");
    }

    #[test]
    fn test_heading_is_listed_once_for_several_rows() {
        let relations = "#### Implementations\n\n##### Store.Insert — L9 [abstract declaration]\n- implementation candidate: Memory.Insert — src/memory.go:5\n- implementation candidate: Disk.Insert — src/disk.go:8\n";
        assert_eq!(
            connected(relations, &[("src/memory.go", 5)]),
            "\n## Connected locations\n- Store.Insert — src/scene.ts:9 [abstract declaration]\n- implementation candidate: Disk.Insert — src/disk.go:8\n"
        );
    }

    #[test]
    fn test_row_without_located_heading_is_listed_alone() {
        let relations =
            "## Event relationships\n\n### Event \"saved\"\n- emit: Scene.save — src/scene.ts:21\n";
        assert_eq!(
            connected(relations, &[]),
            "\n## Connected locations\n- emit: Scene.save — src/scene.ts:21\n"
        );
    }
}
