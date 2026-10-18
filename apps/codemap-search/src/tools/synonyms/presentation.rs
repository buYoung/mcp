use crate::related_terms::evidence::EvidenceSnapshot;
use crate::related_terms::lookup::{Topic, UnsortedDirect, WordGroups};
use crate::related_terms::normalize::word_key;
use crate::related_terms::LookupResult;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fmt::Write;

const MAX_SENSES: usize = 6;
const MAX_DIRECT: usize = 20;
const MAX_NEAR: usize = 10;
const MAX_SAME_TOPIC: usize = 10;
const MAX_LOOKED_UP_INPUTS: usize = 8;

#[derive(Clone)]
struct DisplayGroup {
    field: String,
    field_label: String,
    topics: Vec<Topic>,
    spans: Vec<String>,
    words: WordGroups,
    has_evidence: bool,
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

fn grouped_senses(answer: &LookupResult, supported_sense_count: usize) -> Vec<DisplayGroup> {
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
            });
            groups.len() - 1
        });
        let group = &mut groups[position];
        if !group.spans.contains(&sense.span) {
            group.spans.push(sense.span.clone());
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
) {
    if let Some(evidence) = evidence {
        words.sort_by_key(|word| !evidence.words.contains_key(&word_key(word)));
    }
    words.retain(|word| placed.insert(word_key(word)));
}

fn display_words(
    groups: &mut [DisplayGroup],
    direct: &mut [UnsortedDirect],
    evidence: Option<&EvidenceSnapshot>,
) {
    let mut placed = HashSet::new();
    // Preserve direct > unclassified direct > near > same-topic precedence, but
    // deduplicate only the selected groups. Hidden groups must not claim words.
    for group in groups.iter_mut() {
        place_words(&mut group.words.direct, evidence, &mut placed);
    }
    for group in direct {
        place_words(&mut group.direct, evidence, &mut placed);
    }
    for group in groups.iter_mut() {
        place_words(&mut group.words.near, evidence, &mut placed);
    }
    for group in groups {
        place_words(&mut group.words.same_topic, evidence, &mut placed);
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

fn render_words(text: &mut String, label: &str, words: &[String], cap: usize) {
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
    let _ = writeln!(
        text,
        "{label} (showing {shown} of {}): {values}",
        words.len()
    );
}

pub(super) fn render(
    answer: &LookupResult,
    supported_sense_count: usize,
    evidence: Option<&EvidenceSnapshot>,
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
    if answer.status == "no_match" {
        let _ = writeln!(text, "Not in the dictionary.\n");
        return text;
    }
    let groups = grouped_senses(answer, supported_sense_count);
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
        let _ = writeln!(
            text,
            "### {} [{}] — {topics}",
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
            text.push_str("Related words are already shown in another group.\n");
        }
        render_words(&mut text, "Direct", &group.words.direct, MAX_DIRECT);
        render_words(&mut text, "Near", &group.words.near, MAX_NEAR);
        render_words(
            &mut text,
            "Same topic",
            &group.words.same_topic,
            MAX_SAME_TOPIC,
        );
    }
    for group in &direct {
        render_words(
            &mut text,
            &format!("Direct without a sense ({})", quoted(&group.span)),
            &group.direct,
            MAX_DIRECT,
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
