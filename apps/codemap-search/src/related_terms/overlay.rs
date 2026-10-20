use super::evidence::{Dictionary, EvidenceSnapshot};
use super::normalize::word_key;
use super::{LookupResult, SenseResult};
use crate::parser::split_identifier;
use serde::Serialize;
use std::collections::HashSet;

/// Sense terms named per sense; the display names the evidence instead of every match.
pub(crate) const MAX_SENSE_TERMS: usize = 2;

#[derive(Serialize)]
pub(crate) struct RemovedSense {
    pub field: String,
    pub topics: Vec<String>,
    pub full_word_keys: Vec<String>,
    pub reason: &'static str,
}

/// Code evidence for one sense, weakest first. Same-topic words alone are too generic to
/// support a sense: on 12 pinned repositories every field had evidence and no sense moved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SenseSupport {
    #[default]
    None,
    /// A direct or near relation other than the input's own forms appears in code.
    RelatedWords,
    /// A direct sense term that contains the input appears in code (`abort signal` for `signal`).
    SenseTerm,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct SenseEvidence {
    pub support: SenseSupport,
    /// Present sense terms that contain the input, at most `MAX_SENSE_TERMS`.
    pub terms: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct InputPart {
    pub span: String,
    pub file_count: u64,
    pub occurrence_count: u64,
}

#[derive(Default, Serialize)]
pub(crate) struct OverlayDecision {
    pub is_input_present: bool,
    pub input_file_count: u64,
    pub input_occurrence_count: u64,
    /// Present resolved parts of a phrase input that has no dictionary key of its own.
    pub input_parts: Vec<InputPart>,
    pub is_applied: bool,
    /// The supported prefix after overlay ordering; presentation must keep it first.
    #[serde(skip)]
    pub(crate) supported_sense_count: usize,
    /// Evidence per sense in the final sense order; empty unless applied.
    pub sense_evidence: Vec<SenseEvidence>,
    pub moved_sense_count: usize,
    pub removed_sense_count: usize,
    pub removed_senses: Vec<RemovedSense>,
    pub reason: &'static str,
}

fn senses(count: usize) -> String {
    format!("{count} sense{}", if count == 1 { "" } else { "s" })
}

impl OverlayDecision {
    pub fn note(&self) -> String {
        let input = if self.input_parts.is_empty() {
            format!(
                "input files: {}, occurrences: {}",
                self.input_file_count, self.input_occurrence_count
            )
        } else {
            let parts = self
                .input_parts
                .iter()
                .map(|part| {
                    format!(
                        "{} (files: {}, occurrences: {})",
                        serde_json::to_string(&part.span).unwrap_or_default(),
                        part.file_count,
                        part.occurrence_count
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("input parts: {parts}")
        };
        if self.is_applied {
            // Removal only runs in evaluation builds; production responses never carry a count.
            let removed = if self.removed_sense_count > 0 {
                format!("; removed {}", senses(self.removed_sense_count))
            } else {
                String::new()
            };
            format!(
                "Codebase evidence: moved back {}{removed} ({input}).\n",
                senses(self.moved_sense_count)
            )
        } else {
            format!(
                "Codebase evidence: {} (unfiltered; {input}).\n",
                self.reason
            )
        }
    }
}

fn present_first(words: &[String], evidence: &EvidenceSnapshot) -> Vec<String> {
    let (mut present, absent): (Vec<_>, Vec<_>) = words
        .iter()
        .cloned()
        .partition(|word| evidence.words.contains_key(&word_key(word)));
    present.extend(absent);
    present
}

fn place(
    words: &[String],
    evidence: &EvidenceSnapshot,
    placed: &mut HashSet<String>,
) -> Vec<String> {
    present_first(words, evidence)
        .into_iter()
        .filter(|word| placed.insert(word_key(word)))
        .collect()
}

/// Rebuild the display only after decisions; full groups were never cross-sense deduplicated.
fn redisplay(answer: &mut LookupResult, evidence: &EvidenceSnapshot) {
    let mut placed = HashSet::new();
    for sense in &mut answer.senses {
        sense.direct = place(&sense.full_groups.direct, evidence, &mut placed);
    }
    answer.unsorted_direct = answer.full_unsorted_direct.clone();
    for group in &mut answer.unsorted_direct {
        group.direct = place(&group.direct, evidence, &mut placed);
    }
    for sense in &mut answer.senses {
        sense.near = place(&sense.full_groups.near, evidence, &mut placed);
    }
    for sense in &mut answer.senses {
        sense.same_topic = place(&sense.full_groups.same_topic, evidence, &mut placed);
    }
    answer.candidate_count = placed.len();
}

/// Phrase inputs resolved by parts (`draft autosave`) have no dictionary key, so their own
/// evidence is never counted; they are present when every resolved part is present.
fn present_parts(answer: &LookupResult, evidence: &EvidenceSnapshot) -> Vec<InputPart> {
    if answer.resolution.mode != "parts" {
        return Vec::new();
    }
    answer
        .resolution
        .seeds
        .iter()
        .map(|seed| {
            evidence
                .words
                .get(&word_key(&seed.span))
                .map(|counts| InputPart {
                    span: seed.span.clone(),
                    file_count: counts.file_count,
                    occurrence_count: counts.occurrence_count,
                })
        })
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

fn tokens(text: &str) -> Vec<String> {
    text.split_whitespace().flat_map(split_identifier).collect()
}

/// Inflections and derivations (`drafts`, `saved`) name the input itself, not one sense.
fn is_input_form(token: &str, input: &str) -> bool {
    let is_plural = |longer: &str, shorter: &str| {
        longer
            .strip_prefix(shorter)
            .is_some_and(|ending| ending == "s" || ending == "es")
    };
    token == input
        || is_plural(token, input)
        || is_plural(input, token)
        || (input.len() >= 4
            && token.len() >= 4
            && (token.starts_with(input) || input.starts_with(token)))
}

fn contains_input(word: &[String], input: &[String]) -> bool {
    !input.is_empty()
        && word.windows(input.len()).any(|window| {
            window
                .iter()
                .zip(input)
                .all(|(token, part)| is_input_form(token, part))
        })
}

fn sense_evidence(sense: &SenseResult, evidence: &EvidenceSnapshot) -> SenseEvidence {
    let input = tokens(&sense.span);
    let mut result = SenseEvidence::default();
    for (words, is_direct) in [
        (&sense.full_groups.direct, true),
        (&sense.full_groups.near, false),
    ] {
        for word in words {
            if !evidence.words.contains_key(&word_key(word)) {
                continue;
            }
            let word_tokens = tokens(word);
            if !contains_input(&word_tokens, &input) {
                result.support = result.support.max(SenseSupport::RelatedWords);
            } else if word_tokens.len() > input.len() {
                if is_direct {
                    result.support = SenseSupport::SenseTerm;
                    if result.terms.len() < MAX_SENSE_TERMS {
                        result.terms.push(word.clone());
                    }
                } else {
                    result.support = result.support.max(SenseSupport::RelatedWords);
                }
            }
        }
    }
    result
}

pub(crate) fn apply(
    answer: &mut LookupResult,
    evidence: &EvidenceSnapshot,
    dictionary: &Dictionary,
    is_overlay_enabled: bool,
    should_remove: bool,
) -> OverlayDecision {
    let input_evidence = evidence.words.get(&word_key(&answer.input));
    let input_parts = if input_evidence.is_some() {
        Vec::new()
    } else {
        present_parts(answer, evidence)
    };
    let mut decision = OverlayDecision {
        is_input_present: input_evidence.is_some() || !input_parts.is_empty(),
        input_file_count: input_evidence.map_or(0, |counts| counts.file_count),
        input_occurrence_count: input_evidence.map_or(0, |counts| counts.occurrence_count),
        input_parts,
        reason: "input absent from indexed code",
        ..OverlayDecision::default()
    };
    if !is_overlay_enabled {
        decision.reason = "overlay disabled for evaluation";
        return decision;
    }
    if !decision.is_input_present {
        return decision;
    }
    let mut excluded = HashSet::from([word_key(&answer.input)]);
    for seed in &answer.resolution.seeds {
        excluded.insert(word_key(&seed.span));
        excluded.extend(seed.terms.iter().map(|term| word_key(term)));
    }
    let field_supported = |field: &str| {
        let input_members = excluded
            .iter()
            .filter(|key| evidence.words.contains_key(*key))
            .filter(|key| {
                dictionary
                    .fields_by_word
                    .get(*key)
                    .is_some_and(|fields| fields.iter().any(|candidate| candidate == field))
            })
            .count();
        evidence.field_word_counts.get(field).copied().unwrap_or(0) > input_members
    };
    // Removal keeps its measured rule: only a sense with no word in code, or whose field has
    // none, is removable. Evidence levels below only order and label the kept senses.
    let has_code_word = |sense: &SenseResult| {
        field_supported(&sense.field)
            && sense
                .full_word_keys
                .iter()
                .any(|key| evidence.words.contains_key(key))
    };
    if !answer.senses.iter().any(has_code_word) {
        decision.reason = "no supporting sense; keeping all senses";
        return decision;
    }
    let judged = answer
        .senses
        .iter()
        .map(|sense| {
            if field_supported(&sense.field) {
                sense_evidence(sense, evidence)
            } else {
                SenseEvidence::default()
            }
        })
        .collect::<Vec<_>>();
    decision.is_applied = true;
    decision.reason = "applied";
    let mut ranked = std::mem::take(&mut answer.senses)
        .into_iter()
        .zip(judged)
        .collect::<Vec<_>>();
    if should_remove {
        ranked.retain(|(sense, _)| {
            let is_removable = !has_code_word(sense);
            if is_removable {
                decision.removed_senses.push(RemovedSense {
                    field: sense.field.clone(),
                    topics: sense
                        .topics
                        .iter()
                        .map(|topic| topic.label.clone())
                        .collect(),
                    full_word_keys: sense.full_word_keys.clone(),
                    reason: if field_supported(&sense.field) {
                        "sense has no evidence"
                    } else {
                        "field has no evidence"
                    },
                });
            }
            !is_removable
        });
        decision.removed_sense_count = decision.removed_senses.len();
    }
    // A stable sort keeps the dictionary order within each evidence level.
    let mut ranked = ranked.into_iter().enumerate().collect::<Vec<_>>();
    ranked.sort_by_key(|(_, (_, judged))| std::cmp::Reverse(judged.support));
    decision.moved_sense_count = ranked
        .iter()
        .enumerate()
        .filter(|(new_index, (old_index, _))| new_index > old_index)
        .count();
    decision.supported_sense_count = ranked
        .iter()
        .filter(|(_, (_, judged))| judged.support != SenseSupport::None)
        .count();
    let (senses, judged): (Vec<_>, Vec<_>) = ranked.into_iter().map(|(_, pair)| pair).unzip();
    answer.senses = senses;
    decision.sense_evidence = judged;
    redisplay(answer, evidence);
    decision
}
