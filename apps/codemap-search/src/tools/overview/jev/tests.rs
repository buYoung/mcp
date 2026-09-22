use super::*;
use crate::jev::{DecisionFuture, Usage, MODEL};
use crate::parser::{CodeRange, ExtractedFile, ExtractedSymbol, SymbolFlags};
use std::sync::Mutex;

fn snapshot(count: usize) -> Arc<PublishedIndexSnapshot> {
    Arc::new(PublishedIndexSnapshot::from_files_and_edges(
        (0..count)
            .map(|index| {
                let symbol = ExtractedSymbol {
                    name: format!("entry{index}"),
                    kind: "function".into(),
                    range: CodeRange {
                        start_line: 2,
                        start_col: 1,
                        end_line: 5,
                        end_col: 1,
                    },
                    docstring: Some("Support cancellation".into()),
                    owner: None,
                    flags: SymbolFlags {
                        has_todo: false,
                        has_fixme: false,
                        is_test: false,
                        is_exported: true,
                        is_deprecated: false,
                    },
                };
                (
                    ExtractedFile {
                        file_path: format!("src/f{index:03}.rs"),
                        total_lines: 5,
                        symbols: vec![symbol],
                        literals: vec![],
                        docstrings: vec![],
                        navigation: None,
                    },
                    vec![],
                )
            })
            .collect(),
    ))
}

struct Fake {
    probabilities: [f64; 4],
    last_only: bool,
    calls: Mutex<Vec<EvaluationRequest>>,
    role: String,
    should_fail_roles: bool,
}
impl Fake {
    fn new(probabilities: [f64; 4]) -> Self {
        Self {
            probabilities,
            last_only: false,
            calls: Mutex::new(Vec::new()),
            role: "implementation".into(),
            should_fail_roles: false,
        }
    }
}
impl Evaluator for Fake {
    fn evaluate(
        &self,
        request: EvaluationRequest,
        _options: EvaluationOptions,
    ) -> DecisionFuture<'_, Result<Evaluation, Failure>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(request.clone());
            tokio::task::yield_now().await;
            if self.should_fail_roles && request.request_id == "overview-roles" {
                return Err(Failure {
                    kind: FailureKind::Deadline,
                    metrics: Metrics::default(),
                });
            }
            let answers = request
                .questions
                .iter()
                .map(|(id, question)| {
                    let answer = match question {
                        Question::Score { .. } => {
                            let p = if self.last_only && !id.starts_with("f00000029_") {
                                [0.49, 0.02, 0.0, 0.49]
                            } else {
                                self.probabilities
                            };
                            Answer::Score {
                                score: p.iter().enumerate().map(|(i, p)| i as f64 * p).sum(),
                                probabilities: p
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, p)| (i.to_string(), p))
                                    .collect(),
                                confidence: 0.9,
                                legend: BTreeMap::new(),
                            }
                        }
                        Question::Choice { criteria, .. } => Answer::Choice {
                            choice: self.role.clone(),
                            probabilities: criteria
                                .keys()
                                .map(|key| (key.clone(), f64::from(key == &self.role)))
                                .collect(),
                            confidence: 1.0,
                        },
                        _ => unreachable!(),
                    };
                    (id.clone(), answer)
                })
                .collect();
            Ok(Evaluation {
                request_id: request.request_id,
                model: MODEL.into(),
                answers,
                metrics: Metrics {
                    usage: Usage {
                        input_tokens: 20,
                        output_tokens: 4,
                    },
                    requests_started: 1,
                    requests_completed: 1,
                    ..Default::default()
                },
            })
        })
    }
}

#[tokio::test]
async fn test_no_match_and_tied_evidence_skip_role_stage() {
    for (probabilities, status) in [
        ([1.0, 0.0, 0.0, 0.0], RecommendationStatus::NoMatch),
        ([0.0, 1.0, 0.0, 0.0], RecommendationStatus::NoMatch),
        ([0.25; 4], RecommendationStatus::InsufficientEvidence),
    ] {
        let fake = Fake::new(probabilities);
        let result = evaluate(
            snapshot(30),
            "Find cancellation",
            &fake,
            EvaluationOptions::default(),
        )
        .await
        .unwrap();
        assert_eq!(result.candidates.len(), 30);
        assert!(result.recommendations.is_empty());
        assert_eq!(result.status, status);
        assert_eq!(fake.calls.lock().unwrap().len(), 1);
        assert!(render(&result, 10_000)
            .unwrap()
            .contains("does not establish absence"));
    }
}

#[tokio::test]
async fn test_full_catalog_before_limit_and_deterministic_path_ties() {
    let fake = Fake::new([0.0, 0.0, 1.0, 0.0]);
    let result = evaluate(
        snapshot(31),
        "Find cancellation",
        &fake,
        EvaluationOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(result.candidates.len(), 31);
    assert_eq!(result.recommendations.len(), 24);
    assert_eq!(result.recommendations[0].file_index, 0);
    assert_eq!(result.recommendations[23].file_index, 23);
    assert_eq!(result.metrics.usage.input_tokens, 40);
    assert!(result
        .recommendations
        .iter()
        .all(|r| r.declarations.len() == 1));
    let calls = fake.calls.lock().unwrap();
    assert_eq!(calls[0].questions.len(), 31);
    assert_eq!(calls[1].questions.len(), 24);
}

#[tokio::test]
async fn test_qualification_precedes_max_score_limit() {
    let mut fake = Fake::new([0.0, 0.49, 0.51, 0.0]);
    fake.last_only = true;
    let result = evaluate(
        snapshot(30),
        "Find cancellation",
        &fake,
        EvaluationOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(result.recommendations.len(), 1);
    assert_eq!(result.recommendations[0].file_index, 29);
    assert_eq!(result.status, RecommendationStatus::Matched);
}

#[tokio::test]
async fn test_snapshot_identity_survives_new_publication_and_missing_role() {
    let old = snapshot(3);
    let id = Arc::as_ptr(&old).addr();
    let mut fake = Fake::new([0.0, 0.0, 0.0, 1.0]);
    fake.role = "unrelated".into();
    let pending = evaluate(
        old,
        "Find cancellation",
        &fake,
        EvaluationOptions::default(),
    );
    let publish = async {
        tokio::task::yield_now().await;
        snapshot(6)
    };
    let (result, new) = tokio::join!(pending, publish);
    let result = result.unwrap();
    assert_eq!(result.snapshot_id, id);
    assert_ne!(id, Arc::as_ptr(&new).addr());
    assert_eq!(result.snapshot.codemap().len(), 3);
    assert_eq!(result.recommendations.len(), 3);
    assert!(result
        .recommendations
        .iter()
        .all(|r| r.declarations.is_empty()));
    assert!(render(&result, 10_000)
        .unwrap()
        .contains("No representative declaration"));
    assert_eq!(render(&result, 30).err(), Some("insufficient_output_room"));
}

#[tokio::test]
async fn test_fragmentation_covers_all_evidence_and_raw_scores_replay() {
    let base = snapshot(1);
    let mut file = base.codemap()[0].clone();
    file.docstrings = vec!["많은 설명 ".repeat(2000)];
    let snapshot = Arc::new(PublishedIndexSnapshot::from_files_and_edges(vec![(
        file,
        vec![],
    )]));
    let fake = Fake::new([0.0, 0.0, 1.0, 0.0]);
    let result = evaluate(
        snapshot,
        "Find cancellation",
        &fake,
        EvaluationOptions::default(),
    )
    .await
    .unwrap();
    assert!(result.candidates.len() > 2);
    let calls = fake.calls.lock().unwrap();
    let encoded: String = calls[0]
        .questions
        .values()
        .map(|question| match question {
            Question::Score { instructions, .. } => instructions["candidate"]
                ["indexed_metadata_fragment"]
                .as_str()
                .unwrap(),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(encoded, metadata(&result.snapshot.codemap()[0]).to_string());
    drop(calls);
    assert_eq!(
        select(&result.snapshot, &result.candidates, &result.scores.answers)
            .unwrap()
            .0
            .len(),
        1
    );
    assert_eq!(fake.calls.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn test_role_failure_preserves_first_stage_usage() {
    let mut fake = Fake::new([0.0, 0.0, 0.0, 1.0]);
    fake.should_fail_roles = true;
    let failure = evaluate(
        snapshot(3),
        "Find cancellation",
        &fake,
        EvaluationOptions::default(),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(failure.kind, FailureKind::Deadline);
    assert_eq!(failure.metrics.usage.input_tokens, 20);
}

#[tokio::test]
async fn test_two_roles_max_exact_inclusive_read_window() {
    let mut file = snapshot(1).codemap()[0].clone();
    let mut extra = file.symbols[0].clone();
    extra.range.end_line = 400;
    file.symbols.extend([extra.clone(), extra]);
    let input = Arc::new(PublishedIndexSnapshot::from_files_and_edges(vec![(
        file,
        vec![],
    )]));
    let fake = Fake::new([0.0, 0.0, 0.0, 1.0]);
    let result = evaluate(
        input,
        "Find cancellation",
        &fake,
        EvaluationOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(result.recommendations[0].declarations.len(), 2);
    let text = render(&result, 10_000).unwrap();
    assert!(text.contains("[L2-4]"));
    assert!(text.contains("\"limit\":180"));
}
