use super::{render, MAX_DIRECT, MAX_NEAR, MAX_SAME_TOPIC};
use crate::related_terms::evidence::{EvidenceScope, EvidenceSnapshot, WordEvidence};
use crate::related_terms::lookup::{Resolution, Topic, UnsortedDirect, WordGroups};
use crate::related_terms::{LookupResult, ResolvedInput, SenseResult};
use std::collections::BTreeMap;

fn answer(groups: Vec<WordGroups>) -> LookupResult {
    LookupResult {
        input: "token".into(),
        status: "ok".into(),
        resolution: Resolution {
            mode: "whole".into(),
            seeds: vec![ResolvedInput {
                span: "token".into(),
                terms: vec!["token".into()],
            }],
        },
        senses: groups
            .into_iter()
            .enumerate()
            .map(|(index, groups)| SenseResult {
                span: "token".into(),
                field: format!("field{index}"),
                field_label: format!("Field {index}"),
                topics: vec![Topic {
                    topic: format!("topic{index}"),
                    label: format!("Topic {index}"),
                }],
                concepts: Vec::new(),
                direct: groups.direct.clone(),
                near: groups.near.clone(),
                same_topic: groups.same_topic.clone(),
                full_word_keys: groups
                    .direct
                    .iter()
                    .chain(&groups.near)
                    .chain(&groups.same_topic)
                    .cloned()
                    .collect(),
                full_groups: groups,
            })
            .collect(),
        unsorted_direct: Vec::new(),
        full_unsorted_direct: Vec::new(),
        candidate_count: 0,
        looked_up_inputs: vec!["token".into()],
    }
}

fn render_supported(answer: &LookupResult) -> String {
    let mut words: BTreeMap<_, _> = answer
        .senses
        .iter()
        .flat_map(|sense| &sense.full_word_keys)
        .map(|word| {
            (
                word.clone(),
                WordEvidence {
                    file_count: 1,
                    occurrence_count: 1,
                },
            )
        })
        .collect();
    words.insert(
        "token".into(),
        WordEvidence {
            file_count: 1,
            occurrence_count: 1,
        },
    );
    let evidence = EvidenceSnapshot {
        generation: 1,
        scope: EvidenceScope::Symbols,
        file_count: 1,
        changed_file_count: 1,
        removed_file_count: 0,
        elapsed_ms: 0.0,
        index_pass_ms: 0.0,
        words,
        field_word_counts: BTreeMap::new(),
    };
    render(answer, answer.senses.len(), Some(&evidence), &[], "", None)
}

fn overflowing_words(cap: usize) -> Vec<String> {
    (0..cap)
        .map(|index| format!("word{index:02}"))
        .chain(["shared".into()])
        .collect()
}

#[test]
fn test_related_terms_display_caps_place_only_visible_words() {
    for (label, cap) in [
        ("Direct", MAX_DIRECT),
        ("Near", MAX_NEAR),
        ("Same topic", MAX_SAME_TOPIC),
    ] {
        let mut first = WordGroups::default();
        let mut second = WordGroups::default();
        match label {
            "Direct" => {
                first.direct = overflowing_words(cap);
                second.direct = vec!["shared".into()];
            }
            "Near" => {
                first.near = overflowing_words(cap);
                second.near = vec!["shared".into()];
            }
            _ => {
                first.same_topic = overflowing_words(cap);
                second.same_topic = vec!["shared".into()];
            }
        }
        let text = render_supported(&answer(vec![first, second]));
        assert!(
            text.contains(&format!(
                "{label} (showing {cap} of {}; {cap} in code):",
                cap + 1
            )),
            "{text}"
        );
        let second = text.split("### ").nth(2).unwrap();
        assert!(
            second.contains(&format!("{label} (showing 1 of 1; 1 in code): \"shared\"")),
            "{text}"
        );
        assert_eq!(text.matches("\"shared\"").count(), 1, "{text}");
        assert!(!text.contains("already shown"), "{text}");
    }

    let mut direct = answer(Vec::new());
    direct.full_unsorted_direct = vec![
        UnsortedDirect {
            span: "first".into(),
            direct: overflowing_words(MAX_DIRECT),
        },
        UnsortedDirect {
            span: "second".into(),
            direct: vec!["shared".into()],
        },
    ];
    let text = render(&direct, 0, None, &[], "", None);
    assert!(
        text.contains("Direct without a sense (\"second\") (showing 1 of 1): \"shared\""),
        "{text}"
    );
    assert_eq!(text.matches("\"shared\"").count(), 1, "{text}");
}

#[test]
fn test_related_terms_display_distinguishes_hidden_stronger_words() {
    for is_hidden in [true, false] {
        let first = WordGroups {
            direct: if is_hidden {
                overflowing_words(MAX_DIRECT)
            } else {
                vec!["shared".into()]
            },
            ..WordGroups::default()
        };
        let second = WordGroups {
            near: vec!["shared".into()],
            ..WordGroups::default()
        };
        let text = render_supported(&answer(vec![first, second]));
        let second = text.split("### ").nth(2).unwrap();
        assert!(
            !second.contains("Near ("),
            "a stronger relation must not be relabeled: {text}"
        );
        if is_hidden {
            assert!(second.contains("omitted by word limits"), "{text}");
            assert!(!second.contains("already shown"), "{text}");
            assert!(!text.contains("\"shared\""), "{text}");
        } else {
            assert!(second.contains("already shown"), "{text}");
            assert_eq!(text.matches("\"shared\"").count(), 1, "{text}");
        }
    }
}
