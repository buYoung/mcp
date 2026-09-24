//! Search-only evaluation of registered task questions over bounded source evidence.
//! Each candidate body appears once in its group state. Code composes separate Noul
//! answers as all/any/uncertain decisions; it never reports a joint probability.
//! Any required evaluation failure preserves the entire ordinary search response.

pub(crate) use super::grouped::BlockSymbol;
use super::grouped::FileOutput;
use crate::jev::{
    CancelToken, EvaluationRequest, Evaluator, JevError, NoulAnswer, NoulCriteria, Question,
    QuestionId, RequestIdentity, RequestPolicy, Timing, Usage,
};
pub(crate) fn is_callable_kind(kind: &str) -> bool {
    matches!(kind, "fn" | "function" | "method")
}

pub(crate) fn simple_name(name: &str) -> &str {
    name.rsplit("::")
        .next()
        .and_then(|tail| tail.rsplit('.').next())
        .unwrap_or(name)
}

use serde_json::{json, Map, Value};
use std::collections::HashMap;
use tokio::time::Instant;

mod evidence;
mod followups;
mod questions;
use crate::tools::task::{MatchMode, RegisteredTask};
pub(super) use followups::append_call_candidates;
use questions::question_id;

#[cfg(test)]
mod tests;

/// Version of the evidence capture (statuses, identity check, masking rule, links).
pub const EVIDENCE_VERSION: &str = "search-task-evidence/7";
pub const QUESTION_VERSION: &str = "search-task-questions/5";
pub const POLICY_VERSION: &str = "search-selection-policy/4-experimental";
/// Provisional default for `search_filter_min_unrelated_probability`. It is a starting
/// policy value, not a calibrated one: omission needs a decisive composed no-match; a Noul
/// answer assigns at least this much mass to a criterion being false.
pub const DEFAULT_MIN_UNRELATED_PROBABILITY: f64 = 0.70;
/// A body alone cannot exceed the entire state-plus-question operating budget.
/// Smaller bodies are preflighted together with their goal, questions and evidence.
pub const MAX_COMPLETE_BODY_BYTES: usize = crate::jev::DEFAULT_STATE_QUESTION_TOKEN_BUDGET as usize
    * crate::jev::ESTIMATED_BYTES_PER_TOKEN as usize;
/// Call links named per direction in one question.
pub const MAX_LINKS_PER_QUESTION: usize = 8;
/// Displayed lines inspected for the declaration name when verifying identity.
pub const IDENTITY_CHECK_LINES: usize = 3;

/// The threshold must be a finite probability in `(0.5, 1.0]`: at or below one half the
/// policy would omit a body while the answer assigns at least as much mass to "related".
pub fn validate_threshold(value: f64) -> Result<f64, String> {
    if value.is_finite() && value > 0.5 && value <= 1.0 {
        Ok(value)
    } else {
        Err(format!(
            "search_filter_min_unrelated_probability must be a finite number in (0.5, 1.0], got {value}"
        ))
    }
}

/// Caller-owned limits for one filter run. `deadline_at` is the caller's absolute deadline
/// (the evaluator may bring it forward, never extend it); `cancel` stops an in-flight
/// evaluation.
#[derive(Clone, Debug)]
pub struct FilterPolicy {
    pub min_unrelated_probability: f64,
    pub max_group_bytes: usize,
    pub model: String,
    pub deadline_at: Option<Instant>,
    pub cancel: Option<CancelToken>,
}

impl Default for FilterPolicy {
    fn default() -> Self {
        Self {
            min_unrelated_probability: DEFAULT_MIN_UNRELATED_PROBABILITY,
            max_group_bytes: questions::MAX_GROUP_BYTES,
            model: crate::jev::DEFAULT_MODEL.into(),
            deadline_at: None,
            cancel: None,
        }
    }
}

/// How much of a declaration the renderer actually displayed, and whether it can be judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceStatus {
    /// Every line of the declaration is displayed uncut and the displayed buffer names the
    /// declaration where the index says it starts.
    Complete,
    /// A window, a container summary or a byte-clipped body.
    PartialSource,
    /// Only the declaration row is shown; there is no body block.
    NoSource,
    /// Complete but above [`MAX_COMPLETE_BODY_BYTES`].
    Oversized,
    /// The displayed buffer does not show the indexed declaration name where the index
    /// places it (the file changed since indexing, or the check cannot see the name).
    IdentityUnverified,
    /// Redaction left no usable text beyond the signature line.
    MaskedUnavailable,
}

impl EvidenceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::PartialSource => "partial_source",
            Self::NoSource => "no_source",
            Self::Oversized => "oversized",
            Self::IdentityUnverified => "identity_unverified",
            Self::MaskedUnavailable => "masked_unavailable",
        }
    }
}

/// One displayed declaration of the prepared search output.
#[derive(Clone, Debug)]
pub struct FilterEntity {
    /// Index into the prepared file list (output order).
    pub file_index: usize,
    /// Masked display path.
    pub path: String,
    pub symbol: BlockSymbol,
    /// Index of the body block inside the file's block list, when a body is displayed.
    pub block_index: Option<usize>,
    /// First and last displayed line of the body block.
    pub displayed: Option<(usize, usize)>,
    pub evidence: EvidenceStatus,
    pub is_callable: bool,
    /// The displayed body contains at least one redaction marker.
    pub is_masked: bool,
    /// Rendered bytes of the body block (fence included); 0 without a body.
    pub body_bytes: usize,
    /// Numbered source lines of a complete body, exactly as displayed (already redacted).
    pub body: Option<String>,
    /// Displayed callables this body visibly calls (entity indices).
    pub outgoing: Vec<usize>,
    /// Displayed callables whose bodies visibly call this one (entity indices).
    pub incoming: Vec<usize>,
    /// Indexed callers outside the rendered source windows: (entity, call-site line).
    /// These are bounded name-resolution candidates, not verified runtime targets.
    pub indexed_callers: Vec<(usize, usize)>,
    pub omitted_indexed_callers: usize,
    /// Smallest strictly enclosing displayed declaration in the same file.
    pub parent: Option<usize>,
    pub unresolved_calls: usize,
    pub supporting_context: String,
    pub is_context_clipped: bool,
}

impl FilterEntity {
    /// A body is judged only when it is a callable displayed completely with a verified
    /// identity and usable text.
    pub fn is_judgeable(&self) -> bool {
        self.is_callable && self.evidence == EvidenceStatus::Complete && !self.is_context_clipped
    }

    pub fn qualified_name(&self) -> String {
        match &self.symbol.owner {
            Some(owner) => format!("{owner}::{}", self.symbol.name),
            None => self.symbol.name.clone(),
        }
    }

    pub fn line_count(&self) -> usize {
        self.symbol.end_line.saturating_sub(self.symbol.start_line) + 1
    }
}

/// Counts of displayed bodies per evidence status, for diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvidenceSummary {
    pub complete: usize,
    pub partial: usize,
    pub no_source: usize,
    pub oversized: usize,
    pub identity_unverified: usize,
    pub masked_unavailable: usize,
    pub non_callable: usize,
}

/// Presentation copy of everything the filter judges: the explicit (masked) intent, the
/// masked search arguments named in the shared state, and one entity per displayed
/// declaration.
#[derive(Clone, Debug)]
pub struct FilterInput {
    pub task: RegisteredTask,
    pub search_arguments: Value,
    pub entities: Vec<FilterEntity>,
    pub file_count: usize,
    pub is_snapshot_fresh: bool,
    /// Masked direct caller/callee source shared by candidate groups.
    pub supporting_sources: HashMap<usize, String>,
}

fn symbol_key(symbol: &BlockSymbol) -> (String, usize, usize) {
    (symbol.name.clone(), symbol.start_line, symbol.end_line)
}

fn entity_index(
    entities: &mut Vec<FilterEntity>,
    by_symbol: &mut HashMap<(String, usize, usize), usize>,
    file_index: usize,
    path: &str,
    symbol: &BlockSymbol,
) -> usize {
    *by_symbol.entry(symbol_key(symbol)).or_insert_with(|| {
        entities.push(FilterEntity {
            file_index,
            path: path.into(),
            is_callable: is_callable_kind(&symbol.kind),
            symbol: symbol.clone(),
            block_index: None,
            displayed: None,
            evidence: EvidenceStatus::NoSource,
            is_masked: false,
            body_bytes: 0,
            body: None,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            indexed_callers: Vec::new(),
            omitted_indexed_callers: 0,
            parent: None,
            unresolved_calls: 0,
            supporting_context: String::new(),
            is_context_clipped: false,
        });
        entities.len() - 1
    })
}

fn unique(mut matches: impl Iterator<Item = usize>) -> Option<usize> {
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

/// Resolve one indexed call site to a displayed callable by simple name (the PoC
/// heuristic): a unique owner matching an explicit receiver, else a unique same-file
/// same-owner match, else a unique match across the displayed files.
pub(crate) fn resolve_call(
    entities: &[FilterEntity],
    by_name: &HashMap<String, Vec<usize>>,
    call: &crate::parser::CallSite,
    owner: usize,
) -> Option<usize> {
    let candidates = by_name.get(simple_name(&call.name))?;
    let receiver = call.receiver.as_deref().unwrap_or("");
    if !receiver.is_empty() && !matches!(receiver, "self" | "this" | "Self") {
        let wanted = simple_name(receiver);
        let by_owner = candidates.iter().copied().filter(|index| {
            entities[*index]
                .symbol
                .owner
                .as_deref()
                .is_some_and(|owner| simple_name(owner) == wanted)
        });
        if let Some(target) = unique(by_owner) {
            return Some(target);
        }
    }
    let same_scope = candidates.iter().copied().filter(|index| {
        entities[*index].file_index == entities[owner].file_index
            && entities[*index].symbol.owner == entities[owner].symbol.owner
    });
    if let Some(target) = unique(same_scope) {
        return Some(target);
    }
    unique(candidates.iter().copied())
}

/// The text after the line-number arrow of one displayed line.
fn line_text(line: &str) -> &str {
    line.split_once('\u{2192}').map_or(line, |(_, text)| text)
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

/// Whether `text` contains `name` as a whole identifier (not as a substring of a longer
/// identifier).
fn contains_identifier(text: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(position) = text[from..].find(name) {
        let start = from + position;
        let end = start + name.len();
        let before_ok = start == 0 || !is_identifier_byte(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_identifier_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        // A failed occurrence may start with a non-ASCII identifier character.
        // Advance on a character boundary before slicing the remaining live source.
        from = start + text[start..].chars().next().map_or(1, char::len_utf8);
        if from >= text.len() {
            break;
        }
    }
    false
}

/// Bounded syntactic identity check on the displayed buffer: the indexed declaration name
/// must appear as a whole identifier within the first [`IDENTITY_CHECK_LINES`] displayed
/// lines of a body that starts on the indexed start line. Masked names cannot be checked
/// and are reported as unverified.
pub(crate) fn identity_verified(body: &str, symbol: &BlockSymbol) -> bool {
    let name = simple_name(&symbol.name);
    if name.is_empty() || name.contains(crate::redact::MARKER) {
        return false;
    }
    body.lines()
        .take(IDENTITY_CHECK_LINES)
        .any(|line| contains_identifier(line_text(line), name))
}

/// Mechanical rule for `masked_unavailable`: the body carries at least one redaction
/// marker and, outside the signature line, no alphanumeric character survives outside the
/// markers. A body with some unmasked text is judged with its markers visible.
pub(crate) fn is_masked_unavailable(body: &str) -> bool {
    if !body.contains(crate::redact::MARKER) {
        return false;
    }
    !body.lines().skip(1).map(line_text).any(|text| {
        text.replace(crate::redact::MARKER, "")
            .chars()
            .any(char::is_alphanumeric)
    })
}

impl FilterInput {
    /// Capture the displayed declarations of `files` (prepared output order). Bodies come
    /// from the renderer's block list, nesting from declaration ranges, and call links from
    /// the indexed call sites whose line is inside a displayed callable body.
    pub(crate) fn capture(
        task: &RegisteredTask,
        search_arguments: Value,
        files: &[&FileOutput],
    ) -> Self {
        let mut entities: Vec<FilterEntity> = Vec::new();
        let mut file_spans: Vec<std::ops::Range<usize>> = Vec::with_capacity(files.len());
        for (file_index, file) in files.iter().enumerate() {
            let first_entity = entities.len();
            let path = crate::redact::source(&file.path).into_owned();
            let mut by_symbol: HashMap<(String, usize, usize), usize> = HashMap::new();
            for symbol in file.shown_symbols() {
                entity_index(&mut entities, &mut by_symbol, file_index, &path, &symbol);
            }
            for (block_index, block) in file.blocks().iter().enumerate() {
                let Some(symbol) = &block.symbol else {
                    continue;
                };
                let index = entity_index(&mut entities, &mut by_symbol, file_index, &path, symbol);
                let entity = &mut entities[index];
                if entity.block_index.is_some() {
                    // The first block of a declaration is its body; later blocks for the
                    // same declaration cannot occur, but never re-judge if they did.
                    continue;
                }
                entity.block_index = Some(block_index);
                entity.displayed = block.displayed;
                entity.body_bytes = block.rendered_len();
                entity.is_masked = block.text.contains(crate::redact::MARKER);
                entity.evidence = if !block.is_complete_body() {
                    EvidenceStatus::PartialSource
                } else if block.rendered_len() > MAX_COMPLETE_BODY_BYTES {
                    EvidenceStatus::Oversized
                } else {
                    let body = block.displayed_source();
                    if !identity_verified(&body, symbol) {
                        EvidenceStatus::IdentityUnverified
                    } else if is_masked_unavailable(&body) {
                        EvidenceStatus::MaskedUnavailable
                    } else {
                        entity.body = Some(body);
                        EvidenceStatus::Complete
                    }
                };
            }
            file_spans.push(first_entity..entities.len());
        }

        for span in &file_spans {
            for index in span.clone() {
                let child = entities[index].symbol.clone();
                let parent = span
                    .clone()
                    .filter(|other| *other != index && entities[*other].symbol.contains(&child))
                    .min_by_key(|other| {
                        entities[*other].symbol.end_line - entities[*other].symbol.start_line
                    });
                entities[index].parent = parent;
            }
        }

        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, entity) in entities.iter().enumerate() {
            if entity.is_callable {
                by_name
                    .entry(simple_name(&entity.symbol.name).to_string())
                    .or_default()
                    .push(index);
            }
        }
        let mut links: Vec<(usize, usize)> = Vec::new();
        for (file_index, file) in files.iter().enumerate() {
            let Some(navigation) = file
                .indexed()
                .and_then(|indexed| indexed.navigation.as_ref())
            else {
                continue;
            };
            let span = &file_spans[file_index];
            for call in &navigation.calls {
                let line = call.range.start_line;
                let indexed_owner = span
                    .clone()
                    .filter(|index| {
                        let entity = &entities[*index];
                        entity.is_callable
                            && entity.symbol.start_line <= line
                            && line <= entity.symbol.end_line
                    })
                    .min_by_key(|index| {
                        entities[*index].symbol.end_line - entities[*index].symbol.start_line
                    });
                let displayed_owner = span
                    .clone()
                    .filter(|index| {
                        let entity = &entities[*index];
                        entity.is_callable
                            && entity
                                .displayed
                                .is_some_and(|(first, last)| first <= line && line <= last)
                    })
                    .min_by_key(|index| {
                        entities[*index].symbol.end_line - entities[*index].symbol.start_line
                    });
                let Some(owner) = displayed_owner.or(indexed_owner) else {
                    continue;
                };
                let is_displayed = entities[owner]
                    .displayed
                    .is_some_and(|(first, last)| first <= line && line <= last);
                let Some(target) = resolve_call(&entities, &by_name, call, owner) else {
                    if is_displayed {
                        entities[owner].unresolved_calls += 1;
                    }
                    continue;
                };
                if target == owner {
                    continue;
                }
                if !is_displayed {
                    let callers = &mut entities[target].indexed_callers;
                    if !callers.contains(&(owner, line)) {
                        if callers.len() < MAX_LINKS_PER_QUESTION {
                            callers.push((owner, line));
                        } else {
                            entities[target].omitted_indexed_callers += 1;
                        }
                    }
                } else if !links.contains(&(owner, target)) {
                    links.push((owner, target));
                }
            }
        }
        for (owner, target) in links {
            entities[owner].outgoing.push(target);
            entities[target].incoming.push(owner);
        }

        Self {
            task: task.masked(),
            search_arguments,
            entities,
            file_count: files.len(),
            is_snapshot_fresh: true,
            supporting_sources: HashMap::new(),
        }
    }

    /// Retention known before inference. A judgment cannot shrink these bodies.
    fn deterministic_retention(&self, index: usize) -> Option<RetentionReason> {
        let entity = &self.entities[index];
        if !entity.is_callable {
            return Some(RetentionReason::NotCallable);
        }
        if entity.evidence != EvidenceStatus::Complete {
            return Some(RetentionReason::IncompleteEvidence(entity.evidence));
        }
        if omission_note(entity).len() >= entity.body_bytes {
            return Some(RetentionReason::TooSmallToOmit);
        }
        let mut ancestor = entity.parent;
        while let Some(parent) = ancestor {
            let container = &self.entities[parent];
            if (!container.is_callable
                || container.evidence != EvidenceStatus::Complete
                || omission_note(container).len() >= container.body_bytes)
                && container.displayed.is_some_and(|(first, last)| {
                    first <= entity.symbol.start_line && entity.symbol.end_line <= last
                })
            {
                return Some(RetentionReason::NestedInRetained(parent));
            }
            ancestor = container.parent;
        }
        None
    }

    /// Entity indices that can shrink and therefore receive questions, in output order.
    pub fn judgeable(&self) -> Vec<usize> {
        self.entities
            .iter()
            .enumerate()
            .filter(|(index, entity)| {
                entity.is_judgeable()
                    && (self.deterministic_retention(*index).is_none()
                    // A small/covered function may still decide whether a neighboring
                    // shrinkable body supplies direct support. Do not skip that judgment.
                    || entity.outgoing.iter().chain(&entity.incoming)
                        .any(|neighbor| self.deterministic_retention(*neighbor).is_none()))
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Entities with a displayed body block.
    pub fn body_count(&self) -> usize {
        self.entities
            .iter()
            .filter(|entity| entity.block_index.is_some())
            .count()
    }

    pub fn evidence_summary(&self) -> EvidenceSummary {
        let mut summary = EvidenceSummary::default();
        for entity in self
            .entities
            .iter()
            .filter(|entity| entity.block_index.is_some())
        {
            if !entity.is_callable {
                summary.non_callable += 1;
                continue;
            }
            match entity.evidence {
                EvidenceStatus::Complete => summary.complete += 1,
                EvidenceStatus::PartialSource => summary.partial += 1,
                EvidenceStatus::NoSource => summary.no_source += 1,
                EvidenceStatus::Oversized => summary.oversized += 1,
                EvidenceStatus::IdentityUnverified => summary.identity_unverified += 1,
                EvidenceStatus::MaskedUnavailable => summary.masked_unavailable += 1,
            }
        }
        summary
    }
}

/// One typed answer, kept with its entity for replay.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyJudgment {
    pub question_id: QuestionId,
    pub entity: usize,
    pub criterion: usize,
    pub group: usize,
    pub noul: NoulAnswer,
}

/// Why a body stayed in the output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetentionReason {
    /// Judged below the threshold: probably related.
    JudgedRelated,
    Uncertain,
    /// Structs, enums, classes and other non-callable declarations are never judged.
    NotCallable,
    /// The body is not displayed completely, is oversized, could not be identified in the
    /// displayed buffer, or is masked beyond use, so no judgment is fair.
    IncompleteEvidence(EvidenceStatus),
    /// Judgeable, but the evaluation returned no answer for it.
    NoJudgment,
    /// Judged unrelated, but its rendered body is no larger than the omission note that
    /// would replace it: an omission must shrink the output, never grow it.
    TooSmallToOmit,
    /// Visibly calls, or is visibly called by, the retained callable at this index.
    ConnectedToRetained(usize),
    /// Its lines are displayed inside the retained enclosing block at this index.
    NestedInRetained(usize),
    /// The retained declaration at this index is displayed inside this body: omitting the
    /// body would lose that declaration's lines.
    ContainsRetained(usize),
}

impl RetentionReason {
    pub fn label(&self) -> &'static str {
        match self {
            Self::JudgedRelated => "judged_related",
            Self::Uncertain => "uncertain",
            Self::NotCallable => "not_callable",
            Self::IncompleteEvidence(_) => "incomplete_evidence",
            Self::NoJudgment => "no_judgment",
            Self::TooSmallToOmit => "too_small_to_omit",
            Self::ConnectedToRetained(_) => "connected_to_retained",
            Self::NestedInRetained(_) => "nested_in_retained",
            Self::ContainsRetained(_) => "contains_retained",
        }
    }

    /// Retained for evidence, budget or uncertainty reasons.
    pub fn is_protection(&self) -> bool {
        matches!(
            self,
            Self::NotCallable
                | Self::IncompleteEvidence(_)
                | Self::Uncertain
                | Self::NoJudgment
                | Self::TooSmallToOmit
        )
    }

    pub fn is_link(&self) -> bool {
        matches!(
            self,
            Self::ConnectedToRetained(_) | Self::NestedInRetained(_) | Self::ContainsRetained(_)
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RetentionDecision {
    pub entity: usize,
    pub is_retained: bool,
    /// `None` exactly when the body is omitted.
    pub reason: Option<RetentionReason>,
    /// Composition of leaf decisions, never a calibrated joint probability.
    pub match_state: MatchState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchState {
    Matched,
    NoMatch,
    Uncertain,
}

fn compose(input: &FilterInput, judgments: &[BodyJudgment], threshold: f64) -> Vec<MatchState> {
    (0..input.entities.len())
        .map(|entity| {
            let leaves: Vec<_> = (0..input.task.questions.len())
                .map(|criterion| {
                    match judgments
                        .iter()
                        .find(|answer| answer.entity == entity && answer.criterion == criterion)
                    {
                        Some(answer)
                            if answer.noul.noul.is_finite()
                                && (0.0..=1.0).contains(&answer.noul.noul) =>
                        {
                            if answer.noul.noul >= threshold {
                                MatchState::Matched
                            } else if 1.0 - answer.noul.noul + 1e-9 >= threshold {
                                MatchState::NoMatch
                            } else {
                                MatchState::Uncertain
                            }
                        }
                        _ => MatchState::Uncertain,
                    }
                })
                .collect();
            if leaves.is_empty() {
                return MatchState::Uncertain;
            }
            match input.task.match_mode {
                MatchMode::All if leaves.contains(&MatchState::NoMatch) => MatchState::NoMatch,
                MatchMode::All if leaves.iter().all(|leaf| *leaf == MatchState::Matched) => {
                    MatchState::Matched
                }
                MatchMode::Any if leaves.contains(&MatchState::Matched) => MatchState::Matched,
                MatchMode::Any if leaves.iter().all(|leaf| *leaf == MatchState::NoMatch) => {
                    MatchState::NoMatch
                }
                _ => MatchState::Uncertain,
            }
        })
        .collect()
}

/// Pure retention policy over raw judgments; the same answers replay to the same
/// decisions for a given threshold, without inference.
pub fn apply_policy(
    input: &FilterInput,
    judgments: &[BodyJudgment],
    threshold: f64,
) -> Vec<RetentionDecision> {
    let count = input.entities.len();
    let states = compose(input, judgments, threshold);
    let mut reasons: Vec<Option<RetentionReason>> = input
        .entities
        .iter()
        .enumerate()
        .map(|(index, _)| {
            if let Some(reason) = input.deterministic_retention(index) {
                Some(reason)
            } else {
                match states[index] {
                    MatchState::Matched => Some(RetentionReason::JudgedRelated),
                    MatchState::Uncertain
                        if !judgments.iter().any(|answer| answer.entity == index) =>
                    {
                        Some(RetentionReason::NoJudgment)
                    }
                    MatchState::Uncertain => Some(RetentionReason::Uncertain),
                    MatchState::NoMatch => None,
                }
            }
        })
        .collect();
    loop {
        let mut changed = false;
        for index in 0..count {
            if reasons[index].is_some() {
                continue;
            }
            let entity = &input.entities[index];
            let nested = entity.parent.filter(|parent| {
                reasons[*parent].is_some()
                    && input.entities[*parent]
                        .displayed
                        .is_some_and(|(first, last)| {
                            first <= entity.symbol.start_line && entity.symbol.end_line <= last
                        })
            });
            let contains = entity.displayed.and_then(|(first, last)| {
                (0..count).find(|other| {
                    *other != index
                        && reasons[*other].is_some()
                        && input.entities[*other].file_index == entity.file_index
                        && input.entities[*other].block_index.is_some()
                        && first <= input.entities[*other].symbol.start_line
                        && input.entities[*other].symbol.end_line <= last
                })
            });
            let reason = match (nested, contains) {
                (Some(parent), _) => Some(RetentionReason::NestedInRetained(parent)),
                (None, Some(child)) => Some(RetentionReason::ContainsRetained(child)),
                (None, None) => entity
                    .outgoing
                    .iter()
                    .chain(entity.incoming.iter())
                    .copied()
                    // Protect direct concrete support for a positively matched function.
                    // A small, partial, uncertain or transitively protected neighbor must
                    // not retain an entire unrelated connected component.
                    .find(|link| states[*link] == MatchState::Matched)
                    .map(RetentionReason::ConnectedToRetained),
            };
            if reason.is_some() {
                reasons[index] = reason;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    (0..count)
        .map(|index| RetentionDecision {
            entity: index,
            is_retained: reasons[index].is_some(),
            reason: reasons[index].clone(),
            match_state: states[index],
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterStatus {
    Applied {
        /// Entities with a displayed body block.
        bodies: usize,
        /// Questions answered.
        judged: usize,
        /// Bodies the policy omits (the mask); `FilterResult::rendered_omissions` reports
        /// how many the renderer actually replaced.
        omitted: usize,
        /// Bodies retained by a protection rule (never judged, or judged without answer).
        protected: usize,
        /// Bodies retained through a call link or nesting after being judged unrelated.
        linked: usize,
    },
    Bypassed(String),
    Fallback(String),
}

impl FilterStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Applied { .. } => "applied",
            Self::Bypassed(_) => "bypassed",
            Self::Fallback(_) => "fallback",
        }
    }

    pub fn is_applied(&self) -> bool {
        matches!(self, Self::Applied { .. })
    }
}

#[derive(Clone, Debug)]
pub struct FilterResult {
    pub status: FilterStatus,
    /// One decision per entity when applied; empty when bypassed or fallen back, which
    /// means every body stays untouched.
    pub decisions: Vec<RetentionDecision>,
    pub judgments: Vec<BodyJudgment>,
    pub effective_threshold: f64,
    pub entity_count: usize,
    pub judgeable_count: usize,
    pub evidence: EvidenceSummary,
    /// Bodies the renderer actually replaced with an omission note (set after rendering).
    pub rendered_omissions: usize,
    /// Whether the applied status line was written inline (only when omissions freed the
    /// room for it); the canonical record is the host's diagnostic line.
    pub is_note_inline: bool,
    pub usage: Usage,
    pub timing: Timing,
    pub requests: Vec<RequestIdentity>,
    pub evidence_version: &'static str,
    pub question_version: &'static str,
    pub policy_version: &'static str,
}

impl FilterResult {
    fn untouched(
        input: &FilterInput,
        status: FilterStatus,
        threshold: f64,
        started: Instant,
        usage: Usage,
        timing: Timing,
        requests: Vec<RequestIdentity>,
    ) -> Self {
        Self {
            status,
            decisions: Vec::new(),
            judgments: Vec::new(),
            effective_threshold: threshold,
            entity_count: input.entities.len(),
            judgeable_count: input.judgeable().len(),
            evidence: input.evidence_summary(),
            rendered_omissions: 0,
            is_note_inline: false,
            usage,
            timing: Timing {
                elapsed: started.elapsed(),
                ..timing
            },
            requests,
            evidence_version: EVIDENCE_VERSION,
            question_version: QUESTION_VERSION,
            policy_version: POLICY_VERSION,
        }
    }

    pub fn omitted_entities(&self) -> impl Iterator<Item = usize> + '_ {
        self.decisions
            .iter()
            .filter(|decision| !decision.is_retained)
            .map(|decision| decision.entity)
    }

    /// The bounded stderr-style summary of this result (never contains evidence).
    pub fn diagnostic(&self) -> String {
        match &self.status {
            FilterStatus::Applied {
                bodies,
                judged,
                omitted,
                protected,
                linked,
            } => format!(
                "bodies={bodies} judged={judged} omitted={omitted} rendered_omissions={} protected={protected} linked={linked} unverified={} masked_unavailable={} threshold={:.2} criteria_answers={} uncertain={}",
                self.rendered_omissions,
                self.evidence.identity_unverified,
                self.evidence.masked_unavailable,
                self.effective_threshold,
                self.judgments.len(),
                self.decisions.iter().filter(|decision| decision.reason == Some(RetentionReason::Uncertain)).count()
            ),
            FilterStatus::Bypassed(reason) | FilterStatus::Fallback(reason) => reason.clone(),
        }
    }
}

/// Evaluate bounded groups using the same evaluator, cancellation signal and absolute deadline.
pub async fn evaluate(
    input: &FilterInput,
    evaluator: &dyn Evaluator,
    policy: &FilterPolicy,
) -> FilterResult {
    let started = Instant::now();
    let requested_threshold = policy.min_unrelated_probability;
    let bypass = |reason: &str, threshold: f64| {
        FilterResult::untouched(
            input,
            FilterStatus::Bypassed(reason.into()),
            threshold,
            started,
            Usage::default(),
            Timing::default(),
            Vec::new(),
        )
    };
    let threshold = match validate_threshold(requested_threshold) {
        Ok(threshold) => threshold,
        Err(_) => return bypass("invalid_threshold", requested_threshold),
    };
    if input.task.task_query.trim().is_empty() || input.task.questions.is_empty() {
        return bypass("missing_task_query", threshold);
    }
    if !input.is_snapshot_fresh {
        return bypass("stale_snapshot", threshold);
    }
    let judgeable = input.judgeable();
    if judgeable.is_empty() {
        return bypass(
            if input.evidence_summary().complete == 0 {
                "no_complete_bodies"
            } else {
                "no_shrinkable_bodies"
            },
            threshold,
        );
    }
    let fallback = |reason: &str, usage, timing, requests| {
        FilterResult::untouched(
            input,
            FilterStatus::Fallback(reason.into()),
            threshold,
            started,
            usage,
            timing,
            requests,
        )
    };
    let groups = match questions::groups(input, &judgeable, policy) {
        Ok(groups) => groups,
        Err(error) => {
            return fallback(
                error.kind(),
                Usage::default(),
                Timing::default(),
                Vec::new(),
            )
        }
    };
    if groups.is_empty() {
        return bypass("candidate_context_budget", threshold);
    }
    let deadline_at = policy
        .deadline_at
        .unwrap_or_else(|| started + crate::jev::EvaluatorConfig::default().deadline);
    let request_policy = RequestPolicy {
        deadline_at: Some(deadline_at),
        cancel: policy.cancel.clone(),
    };
    let mut groups = groups.into_iter();
    let mut usage = Usage::default();
    let mut timing = Timing::default();
    let mut requests = Vec::new();
    let mut judgments = Vec::new();
    let evaluate_group = |group: Option<questions::Group>| {
        let policy = request_policy.clone();
        async move {
            match group {
                Some(group) => Some((
                    group.index,
                    group.entities,
                    evaluator.evaluate(group.request.with_policy(policy)).await,
                )),
                None => None,
            }
        }
    };
    while let Some(first) = groups.next() {
        // At most three borrowed evaluation futures; transport permits and spacing remain
        // owned by the shared evaluator. No detached work can outlive this search.
        let (a, b, c) = tokio::join!(
            evaluate_group(Some(first)),
            evaluate_group(groups.next()),
            evaluate_group(groups.next())
        );
        let mut failure = None;
        for (group, entities, outcome) in [a, b, c].into_iter().flatten() {
            let (group_usage, group_timing, group_requests) = match &outcome {
                Ok(result) => (result.usage, result.timing, &result.requests),
                Err(result) => (result.usage, result.timing, &result.requests),
            };
            usage = usage + group_usage;
            timing.http += group_timing.http;
            timing.queue_wait += group_timing.queue_wait;
            timing.request_count += group_timing.request_count;
            for request in group_requests {
                tracing::info!(group_index=group, batch_index=request.batch_index,
                    request_sha256=%request.request_sha256, request_bytes=request.request_bytes,
                    "jev question group");
            }
            requests.extend(group_requests.iter().cloned());
            match outcome {
                Err(error) => {
                    failure = Some(error.error.kind().to_string());
                }
                Ok(outcome) => {
                    for entity in entities {
                        for criterion in 0..input.task.questions.len() {
                            let id = question_id(entity, criterion);
                            match outcome.noul(&id) {
                                Some(noul) => judgments.push(BodyJudgment {
                                    question_id: id,
                                    entity,
                                    criterion,
                                    group,
                                    noul: *noul,
                                }),
                                None => {
                                    failure = Some("incomplete_answers".into());
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(reason) = failure {
            return fallback(&reason, usage, timing, requests);
        }
    }
    let decisions = apply_policy(input, &judgments, threshold);
    let with_body =
        |decision: &&RetentionDecision| input.entities[decision.entity].block_index.is_some();
    let status = FilterStatus::Applied {
        bodies: input.body_count(),
        judged: judgeable.len(),
        omitted: decisions
            .iter()
            .filter(with_body)
            .filter(|decision| !decision.is_retained)
            .count(),
        protected: decisions
            .iter()
            .filter(with_body)
            .filter(|decision| {
                decision
                    .reason
                    .as_ref()
                    .is_some_and(RetentionReason::is_protection)
            })
            .count(),
        linked: decisions
            .iter()
            .filter(with_body)
            .filter(|decision| {
                decision
                    .reason
                    .as_ref()
                    .is_some_and(RetentionReason::is_link)
            })
            .count(),
    };
    FilterResult {
        status,
        decisions,
        judgments,
        effective_threshold: threshold,
        entity_count: input.entities.len(),
        judgeable_count: judgeable.len(),
        evidence: input.evidence_summary(),
        rendered_omissions: 0,
        is_note_inline: false,
        usage,
        timing: Timing {
            elapsed: started.elapsed(),
            ..timing
        },
        requests,
        evidence_version: EVIDENCE_VERSION,
        question_version: QUESTION_VERSION,
        policy_version: POLICY_VERSION,
    }
}

/// The inline note that replaces an omitted body. It keeps the declaration identity and
/// the exact `read` arguments, so the reader can restore the evidence in one call.
pub fn omission_note(entity: &FilterEntity) -> String {
    format!(
        "- _omitted body: L{start}-{end} ({kind} {name}) did not match the task questions; read {path} offset {start} limit {lines} to restore._\n",
        start = entity.symbol.start_line,
        end = entity.symbol.end_line,
        kind = entity.symbol.kind,
        name = entity.qualified_name(),
        path = entity.path,
        lines = entity.line_count(),
    )
}

/// The status line written after the rendered files when the filter omitted bodies and the
/// omissions freed at least this much room. Every count names what it counts.
pub fn summary_note(result: &FilterResult) -> String {
    match &result.status {
        FilterStatus::Applied {
            bodies,
            judged,
            omitted,
            protected,
            linked,
        } => format!(
            "\n_Jev body filter applied ({policy}, threshold {threshold:.2}): {omitted} of {bodies} planned bodies omitted as unrelated ({judged} judged, {protected} protected, {uncertain} uncertain retained, {linked} kept through direct support or nesting). Omitted ranges are noted inline; use read to restore them._\n",
            policy = POLICY_VERSION,
            threshold = result.effective_threshold,
            uncertain = result.decisions.iter().filter(|decision| decision.reason == Some(RetentionReason::Uncertain)).count(),
        ),
        FilterStatus::Bypassed(reason) => {
            format!("\n_Jev body filter bypassed ({reason}); full output preserved._\n")
        }
        FilterStatus::Fallback(kind) => {
            format!("\n_Jev body filter fallback ({kind}); full output preserved._\n")
        }
    }
}

/// What `finish_detail` needs: the note per omitted block and, when the omissions freed
/// the room for it, the inline status line.
pub(crate) struct FilterOutcome {
    replacements: HashMap<(usize, usize), String>,
    delivery_protected: std::collections::HashSet<(usize, usize)>,
    pub(crate) inline_note: Option<String>,
}

impl FilterOutcome {
    pub(crate) fn from_result(input: &FilterInput, result: &FilterResult) -> Self {
        let mut replacements = HashMap::new();
        let mut delivery_protected = std::collections::HashSet::new();
        let mut freed_bytes: usize = 0;
        for decision in &result.decisions {
            let entity = &input.entities[decision.entity];
            if matches!(
                decision.reason,
                Some(RetentionReason::JudgedRelated | RetentionReason::Uncertain)
            ) {
                if let Some(block_index) = entity.block_index {
                    delivery_protected.insert((entity.file_index, block_index));
                }
            }
            if decision.is_retained {
                continue;
            }
            let Some(block_index) = entity.block_index else {
                continue;
            };
            let note = omission_note(entity);
            freed_bytes += entity.body_bytes.saturating_sub(note.len());
            replacements.insert((entity.file_index, block_index), note);
        }
        let summary = summary_note(result);
        let inline_note = (result.status.is_applied()
            && !replacements.is_empty()
            && freed_bytes >= summary.len())
        .then_some(summary);
        Self {
            replacements,
            delivery_protected,
            inline_note,
        }
    }

    pub(crate) fn replacement_for(&self, file_index: usize, block_index: usize) -> Option<String> {
        self.replacements.get(&(file_index, block_index)).cloned()
    }

    pub(crate) fn is_delivery_protected(&self, file_index: usize, block_index: usize) -> bool {
        self.delivery_protected.contains(&(file_index, block_index))
    }

    #[cfg(test)]
    pub(crate) fn replacement_count(&self) -> usize {
        self.replacements.len()
    }
}
