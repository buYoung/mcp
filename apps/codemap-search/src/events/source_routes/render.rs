use super::model::Location;
use super::snapshot::{range, Snapshot};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Freshness {
    Current,
    Changed,
    Hidden,
    Budget,
}
#[derive(Default)]
pub(crate) struct ProofBudget {
    checked: HashMap<String, Freshness>,
    bytes: usize,
    files: usize,
    pub budget_omissions: usize,
}
impl ProofBudget {
    pub fn check(
        &mut self,
        path: &str,
        sources: &HashMap<String, String>,
        root: &Path,
    ) -> Freshness {
        if let Some(status) = self.checked.get(path) {
            return *status;
        }
        let status = if let Some(expected) = sources.get(path) {
            if self.files >= super::super::SOURCE_FILES_PER_QUERY
                || self.bytes + expected.len() > super::super::SOURCE_BYTES_PER_QUERY
            {
                self.budget_omissions += 1;
                Freshness::Budget
            } else {
                self.files += 1;
                self.bytes += expected.len();
                let requested = root.join(path);
                let canonical_root = root.canonicalize();
                let canonical = requested.canonicalize();
                if !canonical_root
                    .as_ref()
                    .ok()
                    .zip(canonical.as_ref().ok())
                    .is_some_and(|(root, path)| path.starts_with(root))
                    || !crate::workspace::walk_root_is_visible(&requested, true)
                {
                    Freshness::Hidden
                } else if std::fs::metadata(&requested)
                    .is_ok_and(|m| m.is_file() && m.len() == expected.len() as u64)
                    && std::fs::read(&requested).is_ok_and(|bytes| bytes == expected.as_bytes())
                {
                    Freshness::Current
                } else {
                    Freshness::Changed
                }
            }
        } else {
            Freshness::Changed
        };
        self.checked.insert(path.into(), status);
        status
    }
}
#[derive(Default)]
pub(crate) struct QueryState {
    pub proof: ProofBudget,
    pub output_endpoints: usize,
    candidates: BTreeSet<(u8, usize)>,
    shown: BTreeMap<usize, String>,
}
impl QueryState {
    pub fn allow_candidate(&mut self, kind: u8, index: usize) -> bool {
        if self.candidates.contains(&(kind, index)) {
            return true;
        }
        if self.candidates.len() >= super::super::CANDIDATES_PER_QUERY {
            return false;
        }
        self.candidates.insert((kind, index));
        true
    }
}
fn overlaps(location: &Location, anchors: &[(String, usize, usize)]) -> bool {
    anchors.iter().any(|(path, start, end)| {
        path == &location.path && *start <= location.line && location.line <= *end
    })
}
fn location(location: &Location, current: Option<&str>) -> String {
    crate::locations::display(&location.path, location.line, current)
}
fn label(kind: &str) -> &str {
    match kind {
        "storage_to_invocation" => "callback invocation candidate",
        "stored_object_method_candidate" => "stored object method candidate",
        "stored_value_argument" => "argument transfer (consumer unproven)",
        "stored_value_return" => "stored value return",
        "stored_object_write" => "stored object write",
        "stored_object_read" => "stored object read",
        "stored_key_lookup" => "stored lookup key",
        _ => "source candidate",
    }
}
fn bounded(text: &str, cap: usize) -> String {
    let mut end = text.len().min(cap);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].into()
}

pub(crate) struct RenderContext<'a> {
    pub scope: Option<&'a str>,
    pub root: &'a Path,
    pub current_file: Option<&'a str>,
    pub sources: &'a HashMap<String, String>,
}

impl Snapshot {
    pub fn has_paths(&self, anchors: &[(String, usize, usize)]) -> bool {
        anchors
            .iter()
            .any(|(path, _, _)| self.by_path.contains_key(path))
    }
    pub fn render(
        &self,
        anchors: &[(String, usize, usize)],
        cap: usize,
        state: &mut QueryState,
        context: RenderContext<'_>,
    ) -> String {
        let RenderContext {
            scope,
            root,
            current_file,
            sources,
        } = context;
        if cap < 128 || anchors.is_empty() {
            return String::new();
        }
        let filter = crate::callers::test_code::TestCodeFilter::from_config(root);
        let mut selected = BTreeSet::new();
        let mut skipped = 0;
        let mut hidden = 0;
        let mut stale = 0;
        let mut proof_limited = 0;
        for path in anchors.iter().map(|(p, _, _)| p).collect::<BTreeSet<_>>() {
            for &id in self.by_path.get(path).into_iter().flatten() {
                if !state.allow_candidate(1, id) {
                    skipped += 1;
                    continue;
                }
                if self.routes[id]
                    .locations()
                    .any(|point| overlaps(point, anchors))
                {
                    selected.insert(id);
                }
            }
        }
        let header="## Source routes\n\n_Conditional source candidates; instance identity and execution are unproven. Event classification is not inferred._\n";
        let mut output = String::new();
        let mut rendered = 0usize;
        for id in selected {
            let route = &self.routes[id];
            if route.locations().any(|point| {
                filter.is_excluded(&point.path, &range(point))
                    || scope.is_some_and(|scope| {
                        point.path != scope
                            && !point
                                .path
                                .strip_prefix(scope)
                                .is_some_and(|rest| rest.starts_with('/'))
                    })
            }) {
                hidden += 1;
                continue;
            }
            if !route.has_complete_dependencies {
                proof_limited += 1;
                continue;
            }
            let status = route
                .proof_paths
                .iter()
                .map(|path| state.proof.check(path, sources, root))
                .find(|status| *status != Freshness::Current);
            match status {
                Some(Freshness::Hidden) => {
                    hidden += 1;
                    continue;
                }
                Some(Freshness::Budget) => {
                    proof_limited += 1;
                    continue;
                }
                Some(_) => {
                    stale += 1;
                    continue;
                }
                None => {}
            }
            let row = if let Some(previous) = state.shown.get(&id) {
                if current_file == Some(previous.as_str()) {
                    continue;
                }
                format!("- shared source route: see `{previous}` above.\n")
            } else {
                let conditions: BTreeSet<_> = route
                    .conditions
                    .iter()
                    .map(|c| c.split(':').next().unwrap_or(c))
                    .collect();
                let mut row = format!(
                    "- {}: {} → {}\n  - conditions: {}\n",
                    label(&route.kind),
                    location(&route.storage, current_file),
                    location(&route.invocation, current_file),
                    conditions.into_iter().collect::<Vec<_>>().join(", ")
                );
                let via: BTreeSet<_> = route.via.iter().collect();
                if !via.is_empty() {
                    row.push_str(&format!(
                        "  - via: {}\n",
                        via.iter()
                            .take(6)
                            .map(|point| location(point, current_file))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    if via.len() > 6 {
                        row.push_str("  - additional source steps omitted by display limit\n");
                    }
                }
                row
            };
            if !state.shown.contains_key(&id)
                && state.output_endpoints + 2 > super::super::ENDPOINTS_PER_QUERY
                || rendered >= super::super::ENDPOINTS_PER_QUERY / 2
                || output.len() + usize::from(output.is_empty()) * header.len() + row.len() + 128
                    > cap
            {
                skipped += 1;
                continue;
            }
            if output.is_empty() {
                output.push_str(header);
            }
            output.push_str(&row);
            if !state.shown.contains_key(&id) {
                state.output_endpoints += 2;
            }
            state.shown.insert(
                id,
                current_file
                    .unwrap_or(&route.storage.path)
                    .into(),
            );
            rendered += 1;
        }
        let mut diagnostics = BTreeSet::new();
        for (path, _, _) in anchors {
            if filter.is_file_excluded(path) {
                continue;
            }
            if let Some(notices) = self.notices.get(path) {
                diagnostics.extend(notices.iter().cloned());
            }
        }
        if !output.is_empty() {
            diagnostics.extend(self.notices.get("").into_iter().flatten().cloned());
        } else {
            diagnostics.extend(
                self.notices
                    .get("")
                    .into_iter()
                    .flatten()
                    .filter(|kind| kind.ends_with("_cap"))
                    .cloned(),
            );
        }
        let notice = if skipped + hidden + stale + proof_limited > 0 {
            format!("[Source routes omitted: {skipped} candidate/output limit, {hidden} excluded, {stale} changed/unavailable evidence, {proof_limited} incomplete evidence budget (128 files / 4194304 bytes).]\n")
        } else {
            String::new()
        };
        if output.len() + notice.len() <= cap {
            output.push_str(&notice);
        } else if !notice.is_empty() {
            return bounded(
                "[Source route output budget reached; narrow the source request.]\n",
                cap,
            );
        }
        if !diagnostics.is_empty() {
            let mut note = format!(
                "[Partial source analysis: {}",
                diagnostics
                    .iter()
                    .take(6)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            if diagnostics.len() > 6 {
                note.push_str(", additional limits omitted");
            }
            note.push_str(".]\n");
            if output.len() + note.len() <= cap {
                output.push_str(&note);
            }
        }
        output
    }
}
