use crate::related_terms::evidence::EvidenceSnapshot;
use crate::related_terms::lookup::{Topic, UnsortedDirect, WordGroups};
use crate::related_terms::normalize::word_key;
use crate::related_terms::overlay::{SenseEvidence, SenseSupport, MAX_SENSE_TERMS};
use crate::related_terms::LookupResult;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fmt::Write;

const MAX_SENSES: usize = 6;
const MAX_DIRECT: usize = 20;
const MAX_NEAR: usize = 10;
const MAX_SAME_TOPIC: usize = 10;
const MAX_LOOKED_UP_INPUTS: usize = 8;

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct DisplayGroup {
    field: String,
    field_label: String,
    topics: Vec<Topic>,
    spans: Vec<String>,
    words: WordGroups,
    has_evidence: bool,
    support: SenseSupport,
    terms: Vec<String>,
    has_omitted_words: bool,
}

impl DisplayGroup {
    fn has_words(&self) -> bool {
        !self.words.direct.is_empty()
            || !self.words.near.is_empty()
            || !self.words.same_topic.is_empty()
    }
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn grouped_senses(
    answer: &LookupResult,
    supported_sense_count: usize,
    sense_evidence: &[SenseEvidence],
) -> Vec<DisplayGroup> {
    let mut groups: Vec<DisplayGroup> = Vec::new();
    let mut positions = BTreeMap::new();
    for (index, sense) in answer.senses.iter().enumerate() {
        let has_evidence = index < supported_sense_count;
        // Do not combine supported and unsupported memberships: that would promote
        // unrelated words across the evidence boundary established by the overlay.
        let key = (
            has_evidence,
            sense.field.clone(),
            sense
                .topics
                .iter()
                .map(|topic| topic.topic.clone())
                .collect::<Vec<_>>(),
        );
        let position = *positions.entry(key).or_insert_with(|| {
            groups.push(DisplayGroup {
                field: sense.field.clone(),
                field_label: sense.field_label.clone(),
                topics: sense.topics.clone(),
                spans: Vec::new(),
                words: WordGroups::default(),
                has_evidence,
                support: SenseSupport::None,
                terms: Vec::new(),
                has_omitted_words: false,
            });
            groups.len() - 1
        });
        let group = &mut groups[position];
        if !group.spans.contains(&sense.span) {
            group.spans.push(sense.span.clone());
        }
        if let Some(judged) = sense_evidence.get(index) {
            group.support = group.support.max(judged.support);
            for term in &judged.terms {
                if group.terms.len() < MAX_SENSE_TERMS && !group.terms.contains(term) {
                    group.terms.push(term.clone());
                }
            }
        }
        group
            .words
            .direct
            .extend(sense.full_groups.direct.iter().cloned());
        group
            .words
            .near
            .extend(sense.full_groups.near.iter().cloned());
        group
            .words
            .same_topic
            .extend(sense.full_groups.same_topic.iter().cloned());
    }
    groups.retain(DisplayGroup::has_words);
    groups
}

fn balanced_order(groups: &[DisplayGroup]) -> Vec<usize> {
    let mut order = Vec::new();
    let mut placed = HashSet::new();
    for has_evidence in [true, false] {
        let mut buckets: Vec<VecDeque<usize>> = Vec::new();
        let mut positions = BTreeMap::new();
        for (index, group) in groups
            .iter()
            .enumerate()
            .filter(|(_, group)| group.has_evidence == has_evidence)
        {
            for span in &group.spans {
                let position = *positions.entry(span).or_insert_with(|| {
                    buckets.push(VecDeque::new());
                    buckets.len() - 1
                });
                buckets[position].push_back(index);
            }
        }
        loop {
            let mut has_added = false;
            for bucket in &mut buckets {
                while let Some(index) = bucket.pop_front() {
                    if placed.insert(index) {
                        order.push(index);
                        has_added = true;
                        break;
                    }
                }
            }
            if !has_added {
                break;
            }
        }
    }
    order
}

fn place_words(
    words: &mut Vec<String>,
    evidence: Option<&EvidenceSnapshot>,
    placed: &mut HashSet<String>,
    stronger: &HashSet<String>,
    cap: usize,
) -> bool {
    if let Some(evidence) = evidence {
        words.sort_by_key(|word| !evidence.words.contains_key(&word_key(word)));
    }
    let mut seen = HashSet::new();
    let mut has_omitted_words = false;
    words.retain(|word| {
        let key = word_key(word);
        if stronger.contains(&key) {
            has_omitted_words |= !placed.contains(&key);
            false
        } else {
            !placed.contains(&key) && seen.insert(key)
        }
    });
    // Keep the full count for truncation notes, but claim only the emitted prefix.
    placed.extend(words.iter().take(cap).map(|word| word_key(word)));
    has_omitted_words
}

fn display_words(
    groups: &mut [DisplayGroup],
    direct: &mut [UnsortedDirect],
    evidence: Option<&EvidenceSnapshot>,
) {
    let mut placed = HashSet::new();
    let mut stronger = HashSet::new();
    let mut current = HashSet::new();
    // A capped word can appear in a later group of the same relation tier. Still
    // reserve stronger memberships so a hidden direct word is not relabeled as near.
    // Only selected groups participate; hidden groups must not claim words.
    for group in groups.iter_mut() {
        current.extend(group.words.direct.iter().map(|word| word_key(word)));
        group.has_omitted_words = place_words(
            &mut group.words.direct,
            evidence,
            &mut placed,
            &stronger,
            MAX_DIRECT,
        );
    }
    stronger.extend(current.drain());
    for group in direct {
        current.extend(group.direct.iter().map(|word| word_key(word)));
        place_words(
            &mut group.direct,
            evidence,
            &mut placed,
            &stronger,
            MAX_DIRECT,
        );
    }
    stronger.extend(current.drain());
    for group in groups.iter_mut() {
        current.extend(group.words.near.iter().map(|word| word_key(word)));
        group.has_omitted_words |= place_words(
            &mut group.words.near,
            evidence,
            &mut placed,
            &stronger,
            MAX_NEAR,
        );
    }
    stronger.extend(current.drain());
    for group in groups {
        group.has_omitted_words |= place_words(
            &mut group.words.same_topic,
            evidence,
            &mut placed,
            &stronger,
            MAX_SAME_TOPIC,
        );
    }
}

fn select_groups(
    groups: &[DisplayGroup],
    answer: &LookupResult,
    evidence: Option<&EvidenceSnapshot>,
) -> (Vec<DisplayGroup>, Vec<UnsortedDirect>) {
    let mut selected = Vec::new();
    for index in balanced_order(groups) {
        selected.push(index);
        let mut display = selected
            .iter()
            .map(|index| groups[*index].clone())
            .collect::<Vec<_>>();
        let mut direct = answer.full_unsorted_direct.clone();
        display_words(&mut display, &mut direct, evidence);
        selected = selected
            .into_iter()
            .zip(display)
            .filter_map(|(index, group)| (group.has_words() || group.has_evidence).then_some(index))
            .collect();
        if selected.len() == MAX_SENSES {
            break;
        }
    }
    let mut display = selected
        .iter()
        .map(|index| groups[*index].clone())
        .collect::<Vec<_>>();
    let mut direct = answer.full_unsorted_direct.clone();
    display_words(&mut display, &mut direct, evidence);
    (display, direct)
}

fn render_words(
    text: &mut String,
    label: &str,
    words: &[String],
    cap: usize,
    evidence: Option<&EvidenceSnapshot>,
) {
    if words.is_empty() {
        return;
    }
    let shown = words.len().min(cap);
    let values = words
        .iter()
        .take(shown)
        .map(|word| quoted(word))
        .collect::<Vec<_>>()
        .join(", ");
    // `place_words` orders words found in code first, so the count names the shown prefix.
    let presence = evidence.map_or_else(String::new, |evidence| {
        let present = words
            .iter()
            .take(shown)
            .filter(|word| evidence.words.contains_key(&word_key(word)))
            .count();
        format!("; {present} in code")
    });
    let _ = writeln!(
        text,
        "{label} (showing {shown} of {}{presence}): {values}",
        words.len()
    );
}

/// Most supported senses only have related words in code, so that common level stays
/// unlabeled to keep responses small; the description documents the convention.
fn support_note(group: &DisplayGroup) -> String {
    match group.support {
        SenseSupport::SenseTerm => format!(
            " (code uses {})",
            group
                .terms
                .iter()
                .map(|term| quoted(term))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        SenseSupport::RelatedWords => String::new(),
        SenseSupport::None => " (no related words in code)".into(),
    }
}

/// `evidence` orders and counts words found in code whenever codebase evidence is ready;
/// `sense_evidence` is non-empty only when the overlay judged and reordered senses.
pub(super) fn render(
    answer: &LookupResult,
    supported_sense_count: usize,
    evidence: Option<&EvidenceSnapshot>,
    sense_evidence: &[SenseEvidence],
    readiness: &str,
    evidence_note: Option<&str>,
) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "## {}", quoted(&answer.input));
    // Preserve the established ready-response prefix for existing MCP consumers.
    text.push_str(readiness);
    if let Some(note) = evidence_note {
        text.push_str(note);
    }
    let whole = answer
        .input
        .split_whitespace()
        .flat_map(crate::parser::split_identifier)
        .collect::<Vec<_>>()
        .join(" ");
    if whole != answer.input {
        let _ = writeln!(text, "Identifier words: {}", quoted(&whole));
    }
    // Lookup diagnostics that only repeat the input carry no information.
    let is_single_attempt =
        answer.looked_up_inputs.len() == 1 && answer.looked_up_inputs[0] == answer.input;
    if !is_single_attempt {
        let looked_up = answer
            .looked_up_inputs
            .iter()
            .take(MAX_LOOKED_UP_INPUTS)
            .map(|word| quoted(word))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(text, "Looked up: {looked_up}");
        if answer.looked_up_inputs.len() > MAX_LOOKED_UP_INPUTS {
            let _ = writeln!(
                text,
                "Lookup attempts omitted: {} (diagnostic list only).",
                answer.looked_up_inputs.len() - MAX_LOOKED_UP_INPUTS
            );
        }
    }
    let is_identity_resolution = answer.resolution.mode == "whole"
        && matches!(answer.resolution.seeds.as_slice(),
            [seed] if seed.span == answer.input && seed.terms.len() == 1 && seed.terms[0] == answer.input);
    if !is_identity_resolution {
        let resolved = answer
            .resolution
            .seeds
            .iter()
            .map(|seed| {
                format!(
                    "{} => {}",
                    quoted(&seed.span),
                    seed.terms
                        .iter()
                        .map(|word| quoted(word))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let _ = writeln!(
            text,
            "Resolved inputs ({}): {resolved}",
            answer.resolution.mode
        );
    }
    if answer.status == "no_match" {
        let _ = writeln!(text, "Not in the dictionary.");
        // The dictionary holds English code words, so a non-English word never matches.
        if answer
            .input
            .chars()
            .any(|c| c.is_alphabetic() && !c.is_ascii())
        {
            let _ = writeln!(
                text,
                "Dictionary terms are English code words; retry with the English term used in code."
            );
        }
        text.push('\n');
        return text;
    }
    let groups = grouped_senses(answer, supported_sense_count, sense_evidence);
    let (display, direct) = select_groups(&groups, answer, evidence);
    let _ = writeln!(
        text,
        "Senses (showing {} of {}):",
        display.len(),
        groups.len()
    );
    if groups.len() != answer.senses.len() {
        let _ = writeln!(text, "Display groups combine identical field/topics; {} source senses. Empty unsupported groups omitted.", answer.senses.len());
    }
    for group in &display {
        let topics = group
            .topics
            .iter()
            .map(|topic| quoted(&topic.label))
            .collect::<Vec<_>>()
            .join(", ");
        let support = if sense_evidence.is_empty() {
            String::new()
        } else {
            support_note(group)
        };
        let _ = writeln!(
            text,
            "### {} [{}] — {topics}{support}",
            quoted(&group.field_label),
            group.field
        );
        if answer.resolution.seeds.len() > 1 {
            let _ = writeln!(
                text,
                "Input parts: {}",
                group
                    .spans
                    .iter()
                    .map(|span| quoted(span))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !group.has_words() {
            if group.has_omitted_words {
                text.push_str("Some related words were omitted by word limits in a stronger relation group.\n");
            } else {
                text.push_str("Related words are already shown in another group.\n");
            }
        }
        render_words(
            &mut text,
            "Direct",
            &group.words.direct,
            MAX_DIRECT,
            evidence,
        );
        render_words(&mut text, "Near", &group.words.near, MAX_NEAR, evidence);
        render_words(
            &mut text,
            "Same topic",
            &group.words.same_topic,
            MAX_SAME_TOPIC,
            evidence,
        );
    }
    for group in &direct {
        render_words(
            &mut text,
            &format!("Direct without a sense ({})", quoted(&group.span)),
            &group.direct,
            MAX_DIRECT,
            evidence,
        );
    }
    if display.is_empty() && direct.iter().all(|group| group.direct.is_empty()) {
        let _ = writeln!(
            text,
            "No additional related words; empty display groups omitted."
        );
    }
    text.push('\n');
    text
}
