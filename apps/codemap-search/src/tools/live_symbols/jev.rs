//! Body filtering over the exact live buffers and producer-written source spans.
//! Parsing is bounded and never rereads a file; rendering finishes before inference, so
//! omissions cannot refill context budgets or mix a later filesystem snapshot into a call.
use super::callable::{self, CallableBounds};
use crate::analyze::FileObservation;
use crate::parser::ExtractedFile;
use crate::tools::overview::jev::{is_callable_kind, simple_name};
use crate::tools::search::jev::BlockSymbol;
use crate::tools::search::jev::{
    self as policy, EvidenceStatus, FilterEntity, FilterInput, FilterResult,
    MAX_COMPLETE_BODY_BYTES,
};
use serde_json::Value;
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;

pub(crate) struct CapturedFile {
    path: String,
    file: ExtractedFile,
    bounds: Vec<CallableBounds>,
}

impl CapturedFile {
    pub fn new(path: &str, source: &str) -> Option<Self> {
        let (file, bounds) = callable::capture(Path::new(path), source).ok()?;
        Some(Self {
            path: path.into(),
            file,
            bounds,
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

fn masked_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(crate::redact::source(text).into_owned()),
        Value::Array(values) => Value::Array(values.iter().map(masked_value).collect()),
        Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| {
                    let masked = match value {
                        Value::String(text) => {
                            Value::String(crate::redact::named_value(key, text).into_owned())
                        }
                        _ => masked_value(value),
                    };
                    (crate::redact::source(key).into_owned(), masked)
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

impl Capture {
    pub fn add_source(&mut self, path: &str, source: &str) {
        if !self.files.iter().any(|file| file.path == path) {
            if let Some(file) = CapturedFile::new(path, source) {
                self.files.push(file);
            }
        }
    }

    pub fn prepare(
        self,
        text: &str,
        copies: &[SourceCopy],
        task_query: &str,
        arguments: &Value,
    ) -> Plan {
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
            let significant: Vec<_> =
                crate::codemap::significant_symbols(&captured.file.symbols).collect();
            for (symbol_index, symbol) in captured.file.symbols.iter().enumerate() {
                let is_callable = is_callable_kind(&symbol.kind);
                if !is_callable && !significant.iter().any(|s| std::ptr::eq(*s, symbol)) {
                    continue;
                }
                let bounds = captured
                    .bounds
                    .iter()
                    .find(|b| b.symbol_index == symbol_index);
                let start = bounds.map_or(symbol.range.start_line, |b| b.start);
                let end = symbol.range.end_line_inclusive();
                let selected: Vec<_> = rows
                    .iter()
                    .filter(|row| row.path == captured.path && start <= row.line && row.line <= end)
                    .collect();
                let (Some(first), Some(last)) = (selected.first(), selected.last()) else {
                    continue;
                };
                let is_complete = selected.len() == end.saturating_sub(start) + 1
                    && selected
                        .iter()
                        .enumerate()
                        .all(|(i, row)| row.is_complete && row.line == start + i)
                    && selected.windows(2).all(|pair| {
                        pair[0].range.end <= pair[1].range.start
                            && text[pair[0].range.end..pair[1].range.start]
                                .trim()
                                .is_empty()
                    });
                let span = first.range.start..last.range.end;
                let body = &text[span.clone()];
                let masked_lines = selected
                    .iter()
                    .map(|row| format!("{}→{}", row.line, row.content))
                    .collect::<Vec<_>>()
                    .join("\n");
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
                let evidence = if !is_complete {
                    EvidenceStatus::PartialSource
                } else if body.len() > MAX_COMPLETE_BODY_BYTES {
                    EvidenceStatus::Oversized
                } else if is_callable
                    && (bounds.is_none() || !policy::identity_verified(&masked_lines, &identity))
                {
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
                    displayed: Some((first.line, last.line)),
                    evidence,
                    is_callable,
                    is_masked: body.contains(crate::redact::MARKER),
                    body_bytes: body.len(),
                    body: (evidence == EvidenceStatus::Complete).then(|| body.to_string()),
                    outgoing: Vec::new(),
                    incoming: Vec::new(),
                    parent: None,
                });
                spans.push(is_complete.then_some(span));
            }
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
                task_query: crate::redact::source(task_query).into_owned(),
                search_arguments: masked_value(arguments),
                file_count: self.files.len(),
                entities,
            },
            spans,
            rows,
        }
    }
}

impl Plan {
    pub fn apply(
        self,
        text: &mut String,
        observations: &mut Vec<FileObservation>,
        result: &mut FilterResult,
    ) {
        let mut omitted: Vec<_> = result
            .decisions
            .iter()
            .filter(|d| !d.is_retained)
            .filter_map(|decision| {
                let span = self.spans[decision.entity].clone()?;
                let note = policy::omission_note(
                    &self.input.entities[decision.entity],
                    decision.unrelated_probability,
                );
                (note.len() < span.len()).then_some((span, note))
            })
            .collect();
        omitted.sort_by_key(|(span, _)| (span.start, std::cmp::Reverse(span.end)));
        let mut disjoint: Vec<(Range<usize>, String)> = Vec::new();
        for (span, note) in omitted {
            if disjoint
                .last()
                .is_some_and(|(previous, _)| span.start < previous.end)
            {
                continue;
            }
            disjoint.push((span, note));
        }
        if disjoint.is_empty() {
            return;
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
    }
}
