//! Body filtering over the exact live buffers and producer-written source spans.
//! Parsing is bounded and never rereads a file; rendering finishes before inference, so
//! omissions cannot refill context budgets or mix a later filesystem snapshot into a call.
use super::callable::{self, CallableBounds};
use crate::analyze::FileObservation;
use crate::parser::ExtractedFile;
use crate::tools::search::jev::BlockSymbol;
use crate::tools::search::jev::{
    self as policy, EvidenceStatus, FilterEntity, FilterInput, FilterResult, Unit,
    MAX_COMPLETE_BODY_BYTES,
};
use crate::tools::search::jev::{is_callable_kind, simple_name};
use crate::tools::task::RegisteredTask;
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;

const EVIDENCE_VERSION: &str = "live-task-evidence/6";

pub(crate) struct CapturedFile {
    path: String,
    file: ExtractedFile,
    bounds: Vec<CallableBounds>,
    // Masked full buffer, the source of a grep match view's judged body.
    source: String,
}

impl CapturedFile {
    pub fn new(path: &str, source: &str) -> Option<Self> {
        let captured = callable::capture(Path::new(path), source).ok()?;
        Some(Self {
            path: path.into(),
            file: captured.file,
            bounds: captured.bounds,
            source: crate::redact::source(source).into_owned(),
        })
    }
}

pub(crate) struct SourceRow {
    pub path: String,
    pub line: usize,
    pub range: Range<usize>,
    pub is_complete: bool,
    /// Masked source without formatting/path prefixes, used only for masking availability.
    pub content: String,
}

#[derive(Default)]
pub(crate) struct Capture {
    pub files: Vec<CapturedFile>,
    pub rows: Vec<SourceRow>,
    /// The rows are grep matches: a callable around them is judged from its whole captured
    /// body, and an omission replaces only its displayed rows.
    pub judges_enclosing_bodies: bool,
}

/// An exact copy made by the live renderer, excluding removed grep path prefixes.
pub(crate) struct SourceCopy {
    pub original: Range<usize>,
    pub rendered_start: usize,
}

pub(crate) struct Plan {
    pub input: FilterInput,
    spans: Vec<Option<Range<usize>>>,
    rows: Vec<SourceRow>,
}

/// A line window a read named, with whether it is exactly a location omitted earlier.
pub(crate) struct RequestedWindow {
    pub path: String,
    pub start: usize,
    pub end: usize,
    pub is_omitted_location: bool,
}

/// A location an applied plan replaced with one note.
pub(crate) struct OmittedLocation {
    pub path: String,
    pub start: usize,
    pub end: usize,
}

impl Capture {
    pub fn add_source(&mut self, path: &str, source: &str) {
        if !self.files.iter().any(|file| file.path == path) {
            if let Some(file) = CapturedFile::new(path, source) {
                self.files.push(file);
            }
        }
    }

    /// `requested` are the windows a read named itself. A unit containing one is the
    /// requested source and stays whole; so does every unit inside a window that is exactly
    /// a location this connection omitted before.
    pub fn prepare(
        self,
        text: &str,
        copies: &[SourceCopy],
        task: &RegisteredTask,
        requested: &[RequestedWindow],
    ) -> Plan {
        let judges_enclosing_bodies = self.judges_enclosing_bodies;
        let rows: Vec<_> = self
            .rows
            .into_iter()
            .filter_map(|mut row| {
                let mut mapped: Option<Range<usize>> = None;
                for copy in copies {
                    let start = row.range.start.max(copy.original.start);
                    let end = row.range.end.min(copy.original.end);
                    if start >= end {
                        continue;
                    }
                    let start = copy.rendered_start + start - copy.original.start;
                    let end = copy.rendered_start + end - copy.original.start;
                    if let Some(range) = &mut mapped {
                        range.end = end;
                    } else {
                        mapped = Some(start..end);
                    }
                }
                row.range = mapped?;
                Some(row)
            })
            .collect();
        let mut entities = Vec::new();
        let mut spans = Vec::new();
        for (file_index, captured) in self.files.iter().enumerate() {
            for (symbol_index, symbol) in captured.file.symbols.iter().enumerate() {
                let is_callable = is_callable_kind(&symbol.kind);
                // A callable without safe body bounds, such as an interface method, is
                // judged with the lines around it.
                let Some(bounds) = captured
                    .bounds
                    .iter()
                    .find(|b| b.symbol_index == symbol_index)
                    .filter(|_| is_callable)
                else {
                    continue;
                };
                let start = bounds.start;
                let end = symbol.range.end_line_inclusive();
                let selected: Vec<_> = rows
                    .iter()
                    .filter(|row| row.path == captured.path && start <= row.line && row.line <= end)
                    .collect();
                let (Some(first), Some(last)) = (selected.first(), selected.last()) else {
                    continue;
                };
                let is_contiguous = selected.windows(2).all(|pair| {
                    pair[0].range.end <= pair[1].range.start
                        && text[pair[0].range.end..pair[1].range.start]
                            .trim()
                            .is_empty()
                });
                let is_complete = selected.len() == end.saturating_sub(start) + 1
                    && selected
                        .iter()
                        .enumerate()
                        .all(|(i, row)| row.is_complete && row.line == start + i)
                    && is_contiguous;
                let is_match_view = !is_complete && judges_enclosing_bodies && is_contiguous;
                // A window cutting the body: its displayed rows are judged as they read.
                let is_part = !is_complete
                    && !is_match_view
                    && is_contiguous
                    && selected.iter().all(|row| row.is_complete);
                let span = first.range.start..last.range.end;
                let masked_lines = if is_match_view {
                    numbered_lines(&captured.source, start, end)
                } else {
                    selected
                        .iter()
                        .map(|row| format!("{}→{}", row.line, row.content))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                let body = if is_match_view {
                    masked_lines.as_str()
                } else {
                    &text[span.clone()]
                };
                let identity = BlockSymbol {
                    name: crate::redact::source(&symbol.name).into_owned(),
                    kind: symbol.kind.clone(),
                    owner: symbol
                        .owner
                        .as_deref()
                        .map(|s| crate::redact::source(s).into_owned()),
                    start_line: start,
                    end_line: end,
                };
                let evidence = if !is_complete && !is_match_view && !is_part {
                    EvidenceStatus::PartialSource
                } else if body.len() > MAX_COMPLETE_BODY_BYTES {
                    EvidenceStatus::Oversized
                } else if !is_part && !policy::identity_verified(&masked_lines, &identity) {
                    EvidenceStatus::IdentityUnverified
                } else if policy::is_masked_unavailable(&masked_lines) {
                    EvidenceStatus::MaskedUnavailable
                } else {
                    EvidenceStatus::Complete
                };
                entities.push(FilterEntity {
                    file_index,
                    path: crate::redact::source(&captured.path).into_owned(),
                    symbol: identity,
                    block_index: Some(spans.len()),
                    // The whole judged body stands for its rows in nesting and retention.
                    displayed: Some(if is_match_view {
                        (start, end)
                    } else {
                        (first.line, last.line)
                    }),
                    evidence,
                    is_callable,
                    body_bytes: span.len(),
                    body: (evidence == EvidenceStatus::Complete).then(|| body.to_string()),
                    is_match_view,
                    unit: if is_part {
                        Unit::Part
                    } else {
                        Unit::Declaration
                    },
                    outgoing: Vec::new(),
                    incoming: Vec::new(),
                    parent: None,
                    is_requested: false,
                });
                spans.push((is_complete || is_match_view || is_part).then_some(span));
            }
            line_units(text, file_index, captured, &rows, &mut entities, &mut spans);
        }
        for entity in &mut entities {
            let path = &entity.path;
            let (first, last) = entity
                .displayed
                .unwrap_or((entity.symbol.start_line, entity.symbol.end_line));
            entity.is_requested = requested.iter().any(|window| {
                crate::redact::source(&window.path) == path.as_str()
                    && ((entity.symbol.start_line <= window.start
                        && window.end <= entity.symbol.end_line)
                        || (window.is_omitted_location
                            && window.start <= first
                            && last <= window.end))
            });
        }
        // Use the same conservative relationship resolver and retention closure as search.
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, entity) in entities.iter().enumerate() {
            if entity.is_callable {
                by_name
                    .entry(simple_name(&entity.symbol.name).into())
                    .or_default()
                    .push(index);
            }
        }
        for index in 0..entities.len() {
            entities[index].parent = (0..entities.len())
                .filter(|other| {
                    *other != index
                        && entities[*other].file_index == entities[index].file_index
                        && entities[*other].symbol.contains(&entities[index].symbol)
                })
                .min_by_key(|other| entities[*other].line_count());
        }
        let mut links = Vec::new();
        for (file_index, file) in self.files.iter().enumerate() {
            let Some(navigation) = &file.file.navigation else {
                continue;
            };
            for call in &navigation.calls {
                let owner = entities
                    .iter()
                    .enumerate()
                    .filter(|(_, entity)| {
                        entity.file_index == file_index
                            && entity.is_callable
                            && entity.displayed.is_some_and(|(start, end)| {
                                start <= call.range.start_line && call.range.start_line <= end
                            })
                            && rows.iter().any(|row| {
                                row.path == file.path
                                    && row.line == call.range.start_line
                                    && row.is_complete
                            })
                    })
                    .min_by_key(|(_, entity)| entity.line_count())
                    .map(|(i, _)| i);
                let Some(owner) = owner else {
                    continue;
                };
                if let Some(target) = policy::resolve_call(&entities, &by_name, call, owner) {
                    if target != owner && !links.contains(&(owner, target)) {
                        links.push((owner, target));
                    }
                }
            }
        }
        for (owner, target) in links {
            entities[owner].outgoing.push(target);
            entities[target].incoming.push(owner);
        }
        Plan {
            input: FilterInput {
                evidence_version: EVIDENCE_VERSION,
                task: task.masked(),
                is_snapshot_fresh: true,
                merges_omissions: true,
                entities,
            },
            spans,
            rows,
        }
    }
}

/// Units for displayed rows outside every callable with safe bounds: runs of adjacent rows
/// held by the same outermost declaration, or by none.
fn line_units(
    text: &str,
    file_index: usize,
    captured: &CapturedFile,
    rows: &[SourceRow],
    entities: &mut Vec<FilterEntity>,
    spans: &mut Vec<Option<Range<usize>>>,
) {
    let symbols = &captured.file.symbols;
    let callables: Vec<(usize, usize)> = captured
        .bounds
        .iter()
        .filter(|bounds| is_callable_kind(&symbols[bounds.symbol_index].kind))
        .map(|bounds| {
            (
                bounds.start,
                symbols[bounds.symbol_index].range.end_line_inclusive(),
            )
        })
        .collect();
    let holder = |line: usize| {
        symbols
            .iter()
            .filter(|symbol| {
                symbol.range.start_line <= line && line <= symbol.range.end_line_inclusive()
            })
            .max_by_key(|symbol| symbol.range.end_line_inclusive() - symbol.range.start_line)
    };
    let outside: Vec<&SourceRow> = rows
        .iter()
        .filter(|row| {
            row.path == captured.path
                && !callables
                    .iter()
                    .any(|(start, end)| *start <= row.line && row.line <= *end)
        })
        .collect();
    let mut start = 0;
    while start < outside.len() {
        let owner = holder(outside[start].line);
        let mut end = start + 1;
        while end < outside.len() {
            let (previous, next) = (outside[end - 1], outside[end]);
            let is_adjacent = next.line == previous.line + 1
                && previous.range.end <= next.range.start
                && text[previous.range.end..next.range.start].trim().is_empty();
            if !is_adjacent || holder(next.line).map(|s| &s.range) != owner.map(|s| &s.range) {
                break;
            }
            end += 1;
        }
        let mut run = &outside[start..end];
        start = end;
        // Blank rows carry nothing to judge; they only join neighboring omissions.
        while run.first().is_some_and(|row| row.content.trim().is_empty()) {
            run = &run[1..];
        }
        while run.last().is_some_and(|row| row.content.trim().is_empty()) {
            run = &run[..run.len() - 1];
        }
        let (Some(&first), Some(&last)) = (run.first(), run.last()) else {
            continue;
        };
        let span = first.range.start..last.range.end;
        let is_complete = run.iter().all(|row| row.is_complete);
        let masked_lines = run
            .iter()
            .map(|row| format!("{}→{}", row.line, row.content))
            .collect::<Vec<_>>()
            .join("\n");
        let body = &text[span.clone()];
        let evidence = if !is_complete {
            EvidenceStatus::PartialSource
        } else if body.len() > MAX_COMPLETE_BODY_BYTES {
            EvidenceStatus::Oversized
        } else if policy::is_masked_unavailable(&masked_lines) {
            EvidenceStatus::MaskedUnavailable
        } else {
            EvidenceStatus::Complete
        };
        let (kind, name, owner_name) = match owner {
            Some(symbol) => (
                symbol.kind.clone(),
                crate::redact::source(&symbol.name).into_owned(),
                symbol
                    .owner
                    .as_deref()
                    .map(|s| crate::redact::source(s).into_owned()),
            ),
            None => ("lines".into(), String::new(), None),
        };
        entities.push(FilterEntity {
            file_index,
            path: crate::redact::source(&captured.path).into_owned(),
            symbol: BlockSymbol {
                name,
                kind,
                owner: owner_name,
                start_line: first.line,
                end_line: last.line,
            },
            block_index: Some(spans.len()),
            displayed: Some((first.line, last.line)),
            evidence,
            is_callable: false,
            body_bytes: span.len(),
            body: (evidence == EvidenceStatus::Complete).then(|| body.to_string()),
            is_match_view: false,
            unit: Unit::Lines,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            parent: None,
            is_requested: false,
        });
        spans.push(is_complete.then_some(span));
    }
}

/// Lines `start..=end` of a captured buffer, numbered as the live tools display them.
fn numbered_lines(source: &str, start: usize, end: usize) -> String {
    source
        .lines()
        .enumerate()
        .skip(start.saturating_sub(1))
        .take(end.saturating_sub(start) + 1)
        .map(|(index, line)| format!("{}→{line}", index + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

impl Plan {
    /// Displayed line numbers of the first and last source row inside `span`.
    fn row_lines(&self, span: &Range<usize>) -> Option<(usize, usize)> {
        let mut lines = self
            .rows
            .iter()
            .filter(|row| span.start <= row.range.start && row.range.end <= span.end)
            .map(|row| row.line);
        let first = lines.next()?;
        Some((first, lines.next_back().unwrap_or(first)))
    }

    /// Whether only blank source rows and whitespace lie in `gap`, and how many rows.
    fn blank_rows_in(&self, text: &str, gap: Range<usize>) -> Option<usize> {
        let mut cursor = gap.start;
        let mut count = 0;
        for row in self
            .rows
            .iter()
            .filter(|row| gap.start <= row.range.start && row.range.end <= gap.end)
        {
            if cursor > row.range.start
                || !text[cursor..row.range.start].trim().is_empty()
                || !row.content.trim().is_empty()
            {
                return None;
            }
            cursor = row.range.end;
            count += 1;
        }
        text[cursor..gap.end].trim().is_empty().then_some(count)
    }

    /// Replace omitted units with notes. Adjacent omitted units of one file, with only blank
    /// rows between them, share one note; a note never grows the output.
    pub fn apply(
        self,
        text: &mut String,
        observations: &mut Vec<FileObservation>,
        result: &mut FilterResult,
    ) -> Vec<OmittedLocation> {
        let mut omitted: Vec<(Range<usize>, usize)> = result
            .decisions
            .iter()
            .filter(|d| !d.is_retained)
            .filter_map(|decision| Some((self.spans[decision.entity].clone()?, decision.entity)))
            .collect();
        omitted.sort_by_key(|(span, _)| (span.start, std::cmp::Reverse(span.end)));
        let mut outer: Vec<(Range<usize>, usize)> = Vec::new();
        for (span, entity) in omitted {
            if outer
                .last()
                .is_some_and(|(previous, _)| span.start < previous.end)
            {
                continue;
            }
            outer.push((span, entity));
        }
        let mut groups: Vec<(Range<usize>, Vec<usize>)> = Vec::new();
        for (span, entity) in outer {
            if let Some((previous, members)) = groups.last_mut() {
                let is_adjacent = self.input.entities[members[0]].path
                    == self.input.entities[entity].path
                    && self
                        .blank_rows_in(text, previous.end..span.start)
                        .zip(self.row_lines(previous).zip(self.row_lines(&span)))
                        .is_some_and(|(blank, ((_, last), (first, _)))| last + 1 + blank == first);
                if is_adjacent {
                    previous.end = span.end;
                    members.push(entity);
                    continue;
                }
            }
            groups.push((span, vec![entity]));
        }
        let mut disjoint: Vec<(Range<usize>, String)> = Vec::new();
        let mut locations = Vec::new();
        for (span, members) in groups {
            let units: Vec<&FilterEntity> = members
                .iter()
                .map(|index| &self.input.entities[*index])
                .collect();
            let (note, (start, end)) = if let [unit] = units.as_slice() {
                let location = match unit.unit {
                    Unit::Declaration => (unit.symbol.start_line, unit.symbol.end_line),
                    Unit::Part | Unit::Lines => unit
                        .displayed
                        .unwrap_or((unit.symbol.start_line, unit.symbol.end_line)),
                };
                (policy::omission_note(unit), location)
            } else {
                let Some((first, last)) = self.row_lines(&span) else {
                    continue;
                };
                (
                    policy::merged_omission_note(&units, first, last),
                    (first, last),
                )
            };
            if note.len() >= span.len() {
                continue;
            }
            locations.push(OmittedLocation {
                path: units[0].path.clone(),
                start,
                end,
            });
            disjoint.push((span, note));
        }
        if disjoint.is_empty() {
            return locations;
        }
        observations.retain_mut(|observation| {
            let retained = self.rows.iter().any(|row| {
                row.path == observation.path
                    && row.is_complete
                    && !disjoint
                        .iter()
                        .any(|(span, _)| span.start <= row.range.start && row.range.end <= span.end)
            });
            if !retained {
                return false;
            }
            let removed_bytes: usize = disjoint
                .iter()
                .filter(|(span, _)| {
                    self.rows.iter().any(|row| {
                        row.path == observation.path
                            && span.start <= row.range.start
                            && row.range.end <= span.end
                    })
                })
                .map(|(span, _)| span.len())
                .sum();
            observation.result_bytes = observation
                .result_bytes
                .saturating_sub(removed_bytes as u64);
            true
        });
        result.rendered_omissions = disjoint.len();
        // Work back to front; unaffected source, file headings, context and footers are
        // retained exactly, without rebuilding anything from the filesystem after await.
        for (span, note) in disjoint.into_iter().rev() {
            text.replace_range(span, &note);
        }
        locations
    }
}
