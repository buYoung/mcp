use super::evidence::{Dictionary, EvidenceSnapshot};
use super::normalize::word_key;
use super::LookupResult;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Serialize)]
pub(crate) struct RemovedSense {
    pub field: String,
    pub topics: Vec<String>,
    pub full_word_keys: Vec<String>,
    pub reason: &'static str,
}

#[derive(Default, Serialize)]
pub(crate) struct OverlayDecision {
    pub is_input_present: bool,
    pub input_file_count: u64,
    pub input_occurrence_count: u64,
    pub is_applied: bool,
    /// The supported prefix after overlay ordering; presentation must keep it first.
    #[serde(skip)]
    pub(crate) supported_sense_count: usize,
    pub moved_sense_count: usize,
    pub removed_sense_count: usize,
    pub removed_senses: Vec<RemovedSense>,
    pub reason: &'static str,
}

impl OverlayDecision {
    pub fn note(&self, evidence: &EvidenceSnapshot) -> String {
        let source = format!(
            "input files: {}, occurrences: {}, generation {}",
            self.input_file_count, self.input_occurrence_count, evidence.generation
        );
        if self.is_applied {
            format!(
                "Codebase evidence: moved back {} senses; removed {} senses ({source}).\n",
                self.moved_sense_count, self.removed_sense_count
            )
        } else {
            format!(
                "Codebase evidence: {} (unfiltered; {source}).\n",
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

pub(crate) fn apply(
    answer: &mut LookupResult,
    evidence: &EvidenceSnapshot,
    dictionary: &Dictionary,
    is_overlay_enabled: bool,
    should_remove: bool,
) -> OverlayDecision {
    let input_evidence = evidence.words.get(&word_key(&answer.input));
    let mut decision = OverlayDecision {
        is_input_present: input_evidence.is_some(),
        input_file_count: input_evidence.map_or(0, |counts| counts.file_count),
        input_occurrence_count: input_evidence.map_or(0, |counts| counts.occurrence_count),
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
    let supported = answer
        .senses
        .iter()
        .map(|sense| {
            field_supported(&sense.field)
                && sense
                    .full_word_keys
                    .iter()
                    .any(|key| evidence.words.contains_key(key))
        })
        .collect::<Vec<_>>();
    if !supported.iter().any(|value| *value) {
        decision.reason = "no supporting sense; keeping all senses";
        return decision;
    }
    decision.is_applied = true;
    decision.supported_sense_count = supported
        .iter()
        .filter(|is_supported| **is_supported)
        .count();
    decision.reason = "applied";
    let mut ranked = std::mem::take(&mut answer.senses)
        .into_iter()
        .enumerate()
        .map(|(index, sense)| (index, supported[index], sense))
        .collect::<Vec<_>>();
    if should_remove {
        ranked.retain(|(_, is_supported, sense)| {
            if !*is_supported {
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
            *is_supported
        });
        decision.removed_sense_count = decision.removed_senses.len();
    } else {
        ranked.sort_by_key(|(_, is_supported, _)| !*is_supported);
        decision.moved_sense_count = ranked
            .iter()
            .enumerate()
            .filter(|(new_index, (old_index, is_supported, _))| {
                !*is_supported && new_index > old_index
            })
            .count();
    }
    answer.senses = ranked.into_iter().map(|(_, _, sense)| sense).collect();
    redisplay(answer, evidence);
    decision
}
