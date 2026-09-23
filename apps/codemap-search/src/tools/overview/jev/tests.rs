use super::*;
use crate::codemap::CodemapView;
use crate::jev::mock::{answers, Gate, MockEvaluator};
use crate::jev::{EvaluationRequest, JevError};
use crate::parser::{CodeExtractor, TreeSitterExtractor};
use std::time::Duration;

const TASK: &str = "where is the output budget applied?";
fn file(path: &str, source: &str) -> ExtractedFile {
    TreeSitterExtractor::new().extract(source, path).unwrap()
}
fn catalog() -> Vec<ExtractedFile> {
    vec![
        file(
            "apps/a/src/budget.ts",
            "export function applyBudget() { return 1; }",
        ),
        file(
            "apps/b/src/string.ts",
            "export function trimText() { return 2; }",
        ),
    ]
}
fn judge(probabilities: &'static [f64]) -> MockEvaluator {
    MockEvaluator::new(move |request: &EvaluationRequest| {
        Ok(request
            .questions()
            .iter()
            .map(|q| (q.id().clone(), answers::score(probabilities)))
            .collect())
    })
}
fn input() -> RootInput {
    RootInput::capture(TASK, 42, &catalog()).unwrap()
}

#[test]
fn activation_and_explicit_intent_are_preserved() {
    assert!(root_activation(true, None, false, false, 1).is_ok());
    assert_eq!(
        root_activation(false, None, false, false, 1),
        Err("not_root_scope")
    );
    assert_eq!(
        root_activation(true, Some("llms-txt"), false, false, 1),
        Err("unsupported_format")
    );
    assert_eq!(
        root_activation(true, None, true, false, 1),
        Err("index_warming")
    );
    assert_eq!(
        root_activation(true, None, false, true, 1),
        Err("indexer_dead")
    );
    assert_eq!(
        root_activation(true, None, false, false, 0),
        Err("empty_index")
    );
    assert_eq!(
        RootInput::capture(" ", 1, &catalog()).unwrap_err(),
        "missing_task_query"
    );
}

#[test]
fn common_overview_rows_have_no_file_or_symbol_presentation_caps() {
    let source: String = (0..12)
        .map(|i| format!("export function budget_{i:02}() {{ return {i}; }}\n"))
        .collect();
    let files: Vec<_> = (0..70)
        .map(|i| file(&format!("apps/w{i:02}/src/budget.ts"), &source))
        .collect();
    let ordinary = crate::codemap::CodemapGenerator::generate_root_view(&files);
    let bounded = ordinary.to_markdown();
    assert!(!bounded.contains("apps/w69/src/budget.ts"));
    assert!(!bounded.contains("budget_11"));
    let input = RootInput::capture(TASK, 1, &files).unwrap();
    assert_eq!(input.files.len(), 70);
    for (candidate, summary) in input.files.iter().zip(&ordinary.files) {
        assert_eq!(
            candidate.overview_text,
            crate::codemap::render_file_summary(summary, None)
        );
        assert!(candidate.overview_text.contains("budget_11"));
        assert!(!candidate.overview_text.contains("more"));
    }
}

#[test]
fn evidence_is_only_the_overview_not_private_docs_calls_or_bodies() {
    let files = [file("src/budget.ts", "/** PRIVATE_DOCUMENTATION */\nexport function applyBudget() { hiddenInvocation(); return 'PRIVATE_BODY'; }")];
    let input = RootInput::capture(TASK, 1, &files).unwrap();
    let encoded = serde_json::to_string(&input.fragments().unwrap()[0].evidence).unwrap();
    assert!(encoded.contains("applyBudget"));
    for excluded in [
        "PRIVATE_DOCUMENTATION",
        "hiddenInvocation",
        "PRIVATE_BODY",
        "outgoing_calls",
        "docstring",
    ] {
        assert!(!encoded.contains(excluded), "{encoded}");
    }
}

#[test]
fn fragments_rejoin_exactly_and_bound_json_escaped_unicode_bytes() {
    let mut files = catalog();
    let symbol = files[0].symbols[0].clone();
    files[0].symbols = (0..900)
        .map(|i| {
            let mut symbol = symbol.clone();
            symbol.name = format!("기능_{i:04}_{}", "한글\\\"\n".repeat(6));
            symbol
        })
        .collect();
    let input = RootInput::capture(TASK, 1, &files).unwrap();
    let fragments = input.fragments().unwrap();
    let parts: Vec<_> = fragments.iter().filter(|f| f.file_index == 0).collect();
    assert!(parts.len() > 2);
    let joined: String = parts
        .iter()
        .map(|f| f.evidence["overview_text"].as_str().unwrap())
        .collect();
    assert_eq!(joined, input.files[0].overview_text);
    for fragment in &fragments {
        assert!(serde_json::to_vec(&fragment.evidence).unwrap().len() <= FRAGMENT_BYTE_LIMIT);
        assert_eq!(
            fragment.evidence["file_path"],
            input.files[fragment.file_index].path
        );
    }
}

#[test]
fn task_path_and_overview_names_are_masked_before_transmission() {
    let _scope = crate::redact::begin_request();
    let secret = format!("AKIA{}", "IOSFODNN7EXAMPLE");
    let files = [file(
        &format!("src/{secret}.ts"),
        &format!("export function {secret}() {{}}"),
    )];
    let input = RootInput::capture(&format!("locate {secret}"), 1, &files).unwrap();
    assert!(!input.task_query.contains(&secret));
    for fragment in input.fragments().unwrap() {
        assert!(!serde_json::to_string(&fragment.evidence)
            .unwrap()
            .contains(&secret));
    }
}

#[tokio::test]
async fn evaluates_every_file_before_selecting_24_and_never_requests_roles() {
    let files: Vec<_> = (0..70)
        .map(|i| {
            file(
                &format!("apps/w{i:02}/budget.ts"),
                "export function budget() {}",
            )
        })
        .collect();
    let input = RootInput::capture(TASK, 1, &files).unwrap();
    let evaluator = judge(&[0.0, 0.0, 0.0, 1.0]);
    let deadline = Instant::now() + Duration::from_secs(5);
    let result = recommend(
        &input,
        &evaluator,
        &RecommendationPolicy {
            deadline_at: Some(deadline),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(result.status, RecommendationStatus::Matched);
    assert_eq!(result.coverage.judged_fragments, 70);
    assert_eq!(result.qualified_file_count, 70);
    assert_eq!(result.ranking.len(), 24);
    assert_eq!(result.ranking[0].path, "apps/w00/budget.ts");
    let requests = evaluator.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].deadline_at, Some(deadline));
    assert_eq!(requests[0].task_query(), Some(TASK));
    assert!(requests[0].questions.values().all(|q| q["type"] == "score"));
    assert!(!result.rendered.unwrap().contains("role:"));
}

#[tokio::test]
async fn useful_evidence_in_the_last_fragment_can_qualify_a_large_file() {
    let mut source: String = (0..800)
        .map(|i| format!("export function routine_{i:04}_with_a_long_descriptive_name() {{}}\n"))
        .collect();
    source.push_str("export function zzBudgetTail() {}\n");
    let input = RootInput::capture(TASK, 1, &[file("src/large.ts", &source)]).unwrap();
    assert!(input.fragments().unwrap().len() > 1);
    let evaluator = MockEvaluator::new(|request: &EvaluationRequest| {
        Ok(request
            .questions()
            .iter()
            .map(|q| {
                let is_tail = q.instructions()["candidate"]["overview_text"]
                    .as_str()
                    .unwrap()
                    .contains("zzBudgetTail");
                (
                    q.id().clone(),
                    answers::score(if is_tail {
                        &[0.0, 0.0, 0.0, 1.0]
                    } else {
                        &[1.0, 0.0, 0.0, 0.0]
                    }),
                )
            })
            .collect())
    });
    let result = recommend(&input, &evaluator, &Default::default()).await;
    assert_eq!(result.coverage.judged_fragments, result.coverage.fragments);
    assert_eq!(result.ranking.len(), 1);
    assert_eq!(result.ranking[0].max_score, 3.0);
    assert_eq!(evaluator.request_count(), 1);
}

#[tokio::test]
async fn qualification_precedes_score_order_and_path_only_never_qualifies() {
    let mut files = catalog();
    files.push(file("src/empty.ts", "// no declarations\n"));
    let input = RootInput::capture(TASK, 1, &files).unwrap();
    let evaluator = MockEvaluator::new(|request: &EvaluationRequest| {
        Ok(request
            .questions()
            .iter()
            .map(|q| {
                let path = q.instructions()["candidate"]["file_path"].as_str().unwrap();
                let distribution = if path.contains("apps/a") {
                    [0.45, 0.0, 0.55, 0.0]
                } else if path.contains("empty") {
                    [0.0, 0.0, 0.0, 1.0]
                } else {
                    [0.55, 0.0, 0.0, 0.45]
                };
                (q.id().clone(), answers::score(&distribution))
            })
            .collect())
    });
    let result = recommend(&input, &evaluator, &Default::default()).await;
    assert_eq!(result.ranking.len(), 1);
    assert_eq!(result.ranking[0].path, "apps/a/src/budget.ts");
}

#[tokio::test]
async fn no_match_and_tied_evidence_are_not_absence_claims() {
    for (distribution, expected) in [
        (&[1.0, 0.0, 0.0, 0.0][..], RecommendationStatus::NoMatch),
        (
            &[0.5, 0.0, 0.0, 0.5][..],
            RecommendationStatus::InsufficientEvidence,
        ),
    ] {
        let evaluator = judge(distribution);
        let result = recommend(&input(), &evaluator, &Default::default()).await;
        assert_eq!(result.status, expected);
        assert!(result.ranking.is_empty());
        assert!(result
            .rendered
            .unwrap()
            .contains("does not show that the implementation is absent"));
        assert_eq!(evaluator.request_count(), 1);
    }
}

#[tokio::test]
async fn empty_or_all_path_only_catalogs_do_not_send_requests() {
    let evaluator = judge(&[0.0, 0.0, 0.0, 1.0]);
    let empty = RootInput::capture(TASK, 1, &[]).unwrap();
    assert_eq!(
        recommend(&empty, &evaluator, &Default::default())
            .await
            .status,
        RecommendationStatus::Bypassed("empty_index".into())
    );
    let unavailable = RootInput::capture(TASK, 1, &[file("src/a.ts", "// only a path")]).unwrap();
    let result = recommend(&unavailable, &evaluator, &Default::default()).await;
    assert_eq!(result.status, RecommendationStatus::InsufficientEvidence);
    assert_eq!(evaluator.request_count(), 0);
}

#[tokio::test]
async fn output_caps_bypass_before_sending_or_drop_whole_entries() {
    let input = input();
    let evaluator = judge(&[0.0, 0.0, 0.0, 1.0]);
    let tiny = RecommendationPolicy {
        output_budget_bytes: Some(minimum_section_bytes(&input) - 1),
        ..Default::default()
    };
    assert!(matches!(
        recommend(&input, &evaluator, &tiny).await.status,
        RecommendationStatus::Bypassed(_)
    ));
    assert_eq!(evaluator.request_count(), 0);
    let full = recommend(&input, &evaluator, &Default::default())
        .await
        .rendered
        .unwrap();
    let policy = RecommendationPolicy {
        output_budget_bytes: Some(full.len() - 1),
        ..Default::default()
    };
    let result = recommend(&input, &evaluator, &policy).await;
    let text = result.rendered.unwrap();
    assert!(text.len() < full.len());
    assert!(text.contains("further recommended file(s) omitted"));
    assert!(!text.contains("### 2."));
}

#[tokio::test]
async fn failure_without_a_response_has_unknown_usage_and_no_partial_ranking() {
    let evaluator = MockEvaluator::failing(JevError::RateLimited { status: 429 });
    let result = recommend(&input(), &evaluator, &Default::default()).await;
    assert_eq!(
        result.status,
        RecommendationStatus::Fallback("rate_limited".into())
    );
    assert_eq!(result.usage.input_tokens, None);
    assert!(result.rendered.is_none() && result.ranking.is_empty());
    assert_eq!(evaluator.request_count(), 1);
}

#[tokio::test]
async fn invalid_provider_answers_keep_reported_usage_without_partial_ranking() {
    use crate::jev::mock::{MockTransport, ScriptedResponse};
    use crate::jev::{EvaluatorConfig, JevEvaluator};
    let transport = std::sync::Arc::new(MockTransport::with_handler(|request| {
        let answers: serde_json::Map<_, _> = request["questions"].as_object().unwrap().keys()
            .map(|id| (id.clone(), json!({"type":"score","score":99.0,"probabilities":{"0":0.0,"1":0.0,"2":0.0,"3":1.0}}))).collect();
        ScriptedResponse::ok(json!({"model":"jev-1.13.0","answers":answers,"usage":{"input_tokens":11,"output_tokens":2}}).to_string())
    }));
    let evaluator = JevEvaluator::new(transport, EvaluatorConfig::default()).unwrap();
    let result = recommend(&input(), &evaluator, &Default::default()).await;
    assert_eq!(
        result.status,
        RecommendationStatus::Fallback("invalid_answer".into())
    );
    assert_eq!(result.usage.input_tokens, Some(11));
    assert_eq!(result.usage.output_tokens, Some(2));
    assert_eq!(result.requests.len(), 1);
    assert!(result.rendered.is_none() && result.ranking.is_empty());
}

#[tokio::test]
async fn unrepresentable_file_identity_falls_back_without_dropping_it() {
    let mut files = catalog();
    files[1].file_path = "x".repeat(FRAGMENT_BYTE_LIMIT + 1);
    let input = RootInput::capture(TASK, 1, &files).unwrap();
    let evaluator = judge(&[0.0, 0.0, 0.0, 1.0]);
    let result = recommend(&input, &evaluator, &Default::default()).await;
    assert_eq!(
        result.status,
        RecommendationStatus::Fallback("projection_incomplete".into())
    );
    assert_eq!(result.coverage.eligible_files, 2);
    assert_eq!(evaluator.request_count(), 0);
}

#[tokio::test]
async fn replay_rejects_changed_query_snapshot_source_missing_and_duplicate_answers() {
    let files = catalog();
    let input = RootInput::capture(TASK, 1, &files).unwrap();
    let evaluator = judge(&[0.0, 0.0, 0.0, 1.0]);
    let result = recommend(&input, &evaluator, &Default::default()).await;
    let fragments = input.fragments().unwrap();
    assert_eq!(
        rank_files(&input, &fragments, &result.fragment_judgments)
            .unwrap()
            .files,
        result.ranking
    );
    for changed in [
        RootInput::capture("another task", 1, &files).unwrap(),
        RootInput::capture(TASK, 2, &files).unwrap(),
        RootInput::capture(TASK, 1, &[file("src/new.ts", "function another() {}")]).unwrap(),
    ] {
        assert!(rank_files(&changed, &fragments, &result.fragment_judgments).is_err());
    }
    assert!(rank_files(&input, &fragments, &result.fragment_judgments[..1]).is_err());
    let mut duplicate = result.fragment_judgments.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(rank_files(&input, &fragments, &duplicate).is_err());
    assert_eq!(evaluator.request_count(), 1);
}

#[tokio::test]
async fn a_new_snapshot_while_waiting_cannot_replace_the_captured_overview() {
    let input = input();
    let gate = Gate::new();
    let evaluator = judge(&[0.0, 0.0, 0.0, 1.0]).with_gate(gate.clone());
    let policy = RecommendationPolicy::default();
    let (result, ()) = tokio::join!(recommend(&input, &evaluator, &policy), async {
        gate.entered().await;
        let changed =
            RootInput::capture(TASK, 43, &[file("src/NEW.ts", "function newBudget() {}")]).unwrap();
        assert_ne!(input.fingerprint(), changed.fingerprint());
        gate.release();
    });
    assert_eq!(result.snapshot_id, 42);
    assert!(!result.rendered.unwrap().contains("NEW"));
}
