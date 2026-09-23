use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Map, Value};
use tokio::time::Instant;

use super::*;
use crate::jev::{
    BoxFuture, Evaluator, JevTransport, TransportError, TransportPolicy, TransportResponse,
    DEFAULT_MODEL,
};
use crate::parser::{CodeRange, NavigationFile, SymbolFlags};

type ScoreDistribution = [f64; 4];

const UNRELATED: ScoreDistribution = [0.7, 0.3, 0.0, 0.0];
const TANGENTIAL: ScoreDistribution = [0.2, 0.6, 0.2, 0.0];
const SUPPORTING: ScoreDistribution = [0.05, 0.15, 0.6, 0.2];
const DIRECT: ScoreDistribution = [0.0, 0.0, 0.1, 0.9];
const TIED: ScoreDistribution = [0.25, 0.25, 0.25, 0.25];

/// Answers by file path (and fragment part) for Score questions and by declaration name for
/// role questions; unknown files are unrelated and unknown declarations are `unrelated`.
#[derive(Default)]
struct Script {
    scores: HashMap<String, ScoreDistribution>,
    fragment_scores: HashMap<(String, u64), ScoreDistribution>,
    roles: HashMap<String, &'static str>,
    failing_status_for_roles: Option<u16>,
}

struct FakeTransport {
    script: Script,
    delay: Duration,
    calls: AtomicUsize,
    bodies: Mutex<Vec<Value>>,
}

impl FakeTransport {
    fn new(script: Script) -> Self {
        Self {
            script,
            delay: Duration::ZERO,
            calls: AtomicUsize::new(0),
            bodies: Mutex::new(Vec::new()),
        }
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    fn answer(&self, question: &Value) -> Value {
        let instructions = &question["instructions"];
        if question["type"] == "score" {
            let evidence = &instructions["indexed_evidence"];
            let path = evidence["path"].as_str().unwrap().to_string();
            let part = evidence["part"].as_u64().unwrap();
            let distribution = self
                .script
                .fragment_scores
                .get(&(path.clone(), part))
                .or_else(|| self.script.scores.get(&path))
                .copied()
                .unwrap_or(UNRELATED);
            score_answer(distribution)
        } else {
            let name = instructions["indexed_declaration"]["name"]
                .as_str()
                .unwrap();
            role_answer(
                self.script
                    .roles
                    .get(name)
                    .copied()
                    .unwrap_or(UNRELATED_ROLE),
            )
        }
    }

    fn bodies(&self) -> Vec<Value> {
        self.bodies.lock().unwrap().clone()
    }

    fn question_count(&self, question_type: &str) -> usize {
        self.bodies()
            .iter()
            .flat_map(|body| {
                body["questions"]
                    .as_object()
                    .unwrap()
                    .values()
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .filter(|question| question["type"] == question_type)
            .count()
    }
}

impl JevTransport for FakeTransport {
    fn post(&self, body: Vec<u8>) -> BoxFuture<'_, Result<TransportResponse, TransportError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            let request: Value = serde_json::from_slice(&body).unwrap();
            self.bodies.lock().unwrap().push(request.clone());
            let questions = request["questions"].as_object().unwrap();
            let is_role_stage = questions
                .values()
                .any(|question| question["type"] == "choice");
            if let (true, Some(status)) = (is_role_stage, self.script.failing_status_for_roles) {
                return Ok(TransportResponse {
                    status,
                    body: Vec::new(),
                });
            }
            let answers: Map<String, Value> = questions
                .iter()
                .map(|(id, question)| (id.clone(), self.answer(question)))
                .collect();
            let response = json!({"model": DEFAULT_MODEL, "answers": answers,
                                  "usage": {"input_tokens": 1_000, "output_tokens": 10}});
            Ok(TransportResponse {
                status: 200,
                body: serde_json::to_vec(&response).unwrap(),
            })
        })
    }
}

fn score_answer(distribution: ScoreDistribution) -> Value {
    let score: f64 = distribution
        .iter()
        .enumerate()
        .map(|(level, p)| level as f64 * p)
        .sum();
    json!({"type": "score", "score": score, "confidence": 0.5,
           "probabilities": {"0": distribution[0], "1": distribution[1], "2": distribution[2], "3": distribution[3]}})
}

fn role_answer(role: &str) -> Value {
    let probabilities: Map<String, Value> = ROLES
        .iter()
        .map(|(key, _)| {
            (
                key.to_string(),
                json!(if *key == role { 0.88 } else { 0.02 }),
            )
        })
        .collect();
    json!({"type": "choice", "choice": role, "probabilities": probabilities, "confidence": 0.8})
}

fn symbol(name: &str, kind: &str, start_line: usize, end_line: usize) -> ExtractedSymbol {
    ExtractedSymbol {
        name: name.to_string(),
        kind: kind.to_string(),
        range: CodeRange {
            start_line,
            start_col: 1,
            end_line,
            end_col: 2,
        },
        docstring: None,
        flags: SymbolFlags {
            has_todo: false,
            has_fixme: false,
            is_test: false,
            is_exported: true,
            is_deprecated: false,
        },
        owner: None,
    }
}

fn call(name: &str, receiver: Option<&str>, line: usize) -> CallSite {
    CallSite {
        name: name.to_string(),
        receiver: receiver.map(str::to_string),
        range: CodeRange {
            start_line: line,
            start_col: 5,
            end_line: line,
            end_col: 20,
        },
        scope_id: None,
    }
}

fn file(path: &str, symbols: Vec<ExtractedSymbol>, calls: Vec<CallSite>) -> ExtractedFile {
    let total_lines = symbols
        .iter()
        .map(|symbol| symbol.range.end_line)
        .max()
        .unwrap_or(1)
        + 5;
    ExtractedFile {
        file_path: path.to_string(),
        total_lines,
        symbols,
        literals: Vec::new(),
        docstrings: Vec::new(),
        navigation: Some(NavigationFile {
            calls,
            ..NavigationFile::default()
        }),
    }
}

/// A file with one function named after its path.
fn simple_file(path: &str) -> ExtractedFile {
    let stem = path.rsplit('/').next().unwrap().split('.').next().unwrap();
    file(
        path,
        vec![symbol(&format!("{stem}_entry"), "fn", 3, 20)],
        Vec::new(),
    )
}

fn snapshot(files: Vec<ExtractedFile>) -> Arc<PublishedIndexSnapshot> {
    Arc::new(PublishedIndexSnapshot::from_files_and_edges(
        files.into_iter().map(|file| (file, Vec::new())).collect(),
    ))
}

fn evaluator(transport: FakeTransport) -> Evaluator<FakeTransport> {
    let policy = TransportPolicy {
        request_spacing: Duration::ZERO,
        ..TransportPolicy::default()
    };
    Evaluator::new(transport, policy).unwrap()
}

fn options(output_room_bytes: Option<usize>) -> RecommendOptions {
    RecommendOptions {
        task_query: "Where are failed uploads retried?".to_string(),
        deadline: Instant::now() + Duration::from_secs(45),
        cancellation: None,
        output_room_bytes,
    }
}

async fn run(
    candidates: &RootCandidates,
    transport: FakeTransport,
    output_room_bytes: Option<usize>,
) -> (RecommendationOutcome, Evaluator<FakeTransport>) {
    let evaluator = evaluator(transport);
    let outcome = recommend(
        candidates,
        "# Root Codemap Overview\n",
        &evaluator,
        options(output_room_bytes),
    )
    .await;
    (outcome, evaluator)
}

fn applied(outcome: RecommendationOutcome) -> Recommendation {
    match outcome {
        RecommendationOutcome::Applied(recommendation) => *recommendation,
        other => panic!("expected an applied recommendation, got {other:?}"),
    }
}

fn scored_paths(transport: &FakeTransport) -> BTreeSet<String> {
    transport
        .bodies()
        .iter()
        .flat_map(|body| {
            body["questions"]
                .as_object()
                .unwrap()
                .values()
                .cloned()
                .collect::<Vec<_>>()
        })
        .filter(|question| question["type"] == "score")
        .map(|question| {
            question["instructions"]["indexed_evidence"]["path"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}

#[tokio::test]
async fn test_mixed_fit_scores_every_file_and_ranks_qualified_files_by_max_score() {
    let files = vec![
        file(
            "src/upload/retry.rs",
            vec![
                symbol("RetryPolicy", "struct", 1, 8),
                symbol("retry_upload", "fn", 10, 40),
                symbol("backoff_delay", "fn", 42, 50),
            ],
            vec![
                call("backoff_delay", None, 20),
                call("send", Some("client"), 25),
            ],
        ),
        file(
            "src/upload/client.rs",
            vec![symbol("send", "fn", 5, 30)],
            Vec::new(),
        ),
        simple_file("src/logging.rs"),
        simple_file("src/unrelated.rs"),
        simple_file("src/split.rs"),
    ];
    let candidates = RootCandidates::project(&snapshot(files));
    let mut script = Script::default();
    script.scores.insert("src/upload/retry.rs".into(), DIRECT);
    script
        .scores
        .insert("src/upload/client.rs".into(), SUPPORTING);
    script.scores.insert("src/logging.rs".into(), TANGENTIAL);
    script.scores.insert("src/split.rs".into(), TIED);
    script.roles.insert("retry_upload".into(), "entry");
    script.roles.insert("backoff_delay".into(), "support");
    script.roles.insert("send".into(), "producer");
    let (outcome, evaluator) = run(&candidates, FakeTransport::new(script), None).await;
    let recommendation = applied(outcome);
    let transport = evaluator.transport();

    assert_eq!(recommendation.status, RecommendationStatus::Matched);
    assert_eq!(recommendation.evaluated_files, 5);
    assert_eq!(
        scored_paths(transport).len(),
        5,
        "every indexed file is scored"
    );
    assert_eq!(
        transport.question_count("score"),
        candidates.fragment_count()
    );
    let ranked: Vec<&str> = recommendation
        .recommended
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(ranked, ["src/upload/retry.rs", "src/upload/client.rs"]);
    // Roles are asked only for leaf declarations of the two selected files.
    assert_eq!(transport.question_count("choice"), 3);

    let retry = &recommendation.recommended[0];
    let roles: Vec<(&str, &str)> = retry
        .declarations
        .iter()
        .map(|recommended| {
            (
                recommended.declaration.name.as_str(),
                recommended.answer.choice.as_str(),
            )
        })
        .collect();
    assert_eq!(roles.len(), 2);
    assert!(
        roles.contains(&("retry_upload", "entry")) && roles.contains(&("backoff_delay", "support"))
    );
    let retry_upload = &retry
        .declarations
        .iter()
        .find(|recommended| recommended.declaration.name == "retry_upload")
        .unwrap()
        .declaration;
    assert_eq!((retry_upload.start_line, retry_upload.end_line), (10, 40));
    assert!(retry_upload
        .outgoing_calls
        .iter()
        .any(|call| call.call == "backoff_delay"
            && call.target.as_deref() == Some("src/upload/retry.rs:backoff_delay")));

    let section = &recommendation.section;
    assert!(section.contains("### 1. src/upload/retry.rs · relevance 2.90/3"));
    assert!(
        section.contains(
            r#"read {"file_path":"src/upload/retry.rs","offset":10,"limit":31,"view":"source"}"#
        ),
        "{section}"
    );
    assert!(section.contains("[jev overview: applied · status=matched · requests=2"));
    assert!(!section.contains("src/logging.rs") && !section.contains("src/split.rs"));

    for judgment in &recommendation.judgments {
        assert!(candidates.files()[judgment.candidate_index].path == judgment.path);
    }
    assert_eq!(
        recommendation.snapshot_identity,
        candidates.snapshot_identity()
    );
    assert_eq!(recommendation.usage.input_tokens, 2_000);
}

#[tokio::test]
async fn test_all_unrelated_or_tangential_returns_no_match_without_role_stage() {
    let files = vec![
        simple_file("src/a.rs"),
        simple_file("src/b.rs"),
        simple_file("src/c.rs"),
    ];
    let candidates = RootCandidates::project(&snapshot(files));
    let mut script = Script::default();
    script.scores.insert("src/b.rs".into(), TANGENTIAL);
    let (outcome, evaluator) = run(&candidates, FakeTransport::new(script), None).await;
    let recommendation = applied(outcome);
    assert_eq!(recommendation.status, RecommendationStatus::NoMatch);
    assert!(recommendation.recommended.is_empty());
    assert!(recommendation.declaration_roles.is_none());
    assert_eq!(evaluator.transport().question_count("choice"), 0);
    assert_eq!(evaluator.transport().calls.load(Ordering::SeqCst), 1);
    assert!(recommendation.section.contains("Status: no_match"));
    assert!(recommendation
        .section
        .contains("does not show that the implementation is absent"));
}

#[tokio::test]
async fn test_tied_fragment_without_qualification_is_insufficient_evidence() {
    let files = vec![simple_file("src/a.rs"), simple_file("src/b.rs")];
    let candidates = RootCandidates::project(&snapshot(files));
    let mut script = Script::default();
    script.scores.insert("src/b.rs".into(), TIED);
    let (outcome, evaluator) = run(&candidates, FakeTransport::new(script), None).await;
    let recommendation = applied(outcome);
    assert_eq!(
        recommendation.status,
        RecommendationStatus::InsufficientEvidence
    );
    assert!(recommendation.recommended.is_empty());
    assert_eq!(evaluator.transport().question_count("choice"), 0);
    assert!(recommendation
        .section
        .contains("Status: insufficient_evidence"));
}

#[tokio::test]
async fn test_empty_snapshot_is_insufficient_evidence_without_requests() {
    let candidates = RootCandidates::project(&snapshot(Vec::new()));
    let (outcome, evaluator) = run(&candidates, FakeTransport::new(Script::default()), None).await;
    let recommendation = applied(outcome);
    assert_eq!(
        recommendation.status,
        RecommendationStatus::InsufficientEvidence
    );
    assert_eq!(evaluator.transport().calls.load(Ordering::SeqCst), 0);
    assert!(recommendation
        .section
        .contains("[jev overview: applied · status=insufficient_evidence · requests=0"));
}

#[tokio::test]
async fn test_qualification_precedes_the_24_file_limit() {
    let mut files = Vec::new();
    let mut script = Script::default();
    // High raw scores that still favour the no-useful/tangential group.
    for index in 0..30 {
        let path = format!("src/noise/n{index:02}.rs");
        script.scores.insert(path.clone(), [0.0, 0.51, 0.0, 0.49]);
        files.push(simple_file(&path));
    }
    // Lower raw scores that qualify.
    for index in 0..26 {
        let path = format!("src/flow/q{index:02}.rs");
        let supporting = 0.51 + index as f64 * 0.01;
        script
            .scores
            .insert(path.clone(), [1.0 - supporting, 0.0, supporting, 0.0]);
        files.push(simple_file(&path));
    }
    let candidates = RootCandidates::project(&snapshot(files));
    let (outcome, _) = run(&candidates, FakeTransport::new(script), None).await;
    let recommendation = applied(outcome);
    assert_eq!(recommendation.status, RecommendationStatus::Matched);
    assert_eq!(recommendation.recommended.len(), MAX_RECOMMENDED_FILES);
    assert!(recommendation
        .recommended
        .iter()
        .all(|file| file.path.starts_with("src/flow/")));
    assert_eq!(recommendation.recommended[0].path, "src/flow/q25.rs");
    let scores: Vec<f64> = recommendation
        .recommended
        .iter()
        .map(|file| file.max_score)
        .collect();
    assert!(scores.windows(2).all(|pair| pair[0] >= pair[1]));
    assert_eq!(
        recommendation
            .judgments
            .iter()
            .filter(|judgment| judgment.is_qualified)
            .count(),
        26
    );
}

#[tokio::test]
async fn test_fragmented_file_is_scored_by_every_fragment_and_its_best_one() {
    let symbols: Vec<ExtractedSymbol> = (0..1_200)
        .map(|index| {
            symbol(
                &format!("handler_with_a_descriptive_name_{index:04}"),
                "fn",
                index * 3 + 1,
                index * 3 + 2,
            )
        })
        .collect();
    let candidates = RootCandidates::project(&snapshot(vec![file(
        "src/handlers.rs",
        symbols,
        Vec::new(),
    )]));
    let file = &candidates.files()[0];
    assert!(file.fragment_count() >= 2);
    for fragment in &file.fragments {
        assert!(serde_json::to_vec(fragment).unwrap().len() <= FRAGMENT_BYTES);
        assert_eq!(fragment["path"], "src/handlers.rs");
        assert_eq!(fragment["parts"], json!(file.fragment_count()));
        assert_eq!(fragment["omitted_declarations"], json!(800));
    }
    let listed: usize = file
        .fragments
        .iter()
        .map(|fragment| fragment["declarations"].as_array().unwrap().len())
        .sum();
    assert_eq!(listed, MAX_EVIDENCE_DECLARATIONS);

    let mut script = Script::default();
    script
        .fragment_scores
        .insert(("src/handlers.rs".into(), 2), DIRECT);
    let second_fragment_first = file
        .declarations
        .iter()
        .find(|declaration| declaration.fragment_index == Some(1))
        .unwrap()
        .name
        .clone();
    script
        .roles
        .insert(second_fragment_first.clone(), "consumer");
    let (outcome, evaluator) = run(&candidates, FakeTransport::new(script), None).await;
    let recommendation = applied(outcome);
    assert_eq!(
        evaluator.transport().question_count("score"),
        file.fragment_count()
    );
    assert_eq!(recommendation.recommended.len(), 1);
    assert!((recommendation.recommended[0].max_score - 2.9).abs() < 1e-9);
    // Role candidates come from the best fragment first and are capped per file.
    assert_eq!(
        evaluator.transport().question_count("choice"),
        MAX_ROLE_CANDIDATES_PER_FILE
    );
    assert_eq!(
        recommendation.recommended[0].declarations[0]
            .declaration
            .name,
        second_fragment_first
    );
}

#[tokio::test]
async fn test_qualified_file_without_supported_role_stays_visible() {
    let candidates = RootCandidates::project(&snapshot(vec![simple_file("src/upload.rs")]));
    let mut script = Script::default();
    script.scores.insert("src/upload.rs".into(), SUPPORTING);
    let (outcome, _) = run(&candidates, FakeTransport::new(script), None).await;
    let recommendation = applied(outcome);
    assert_eq!(recommendation.recommended.len(), 1);
    assert!(recommendation.recommended[0].declarations.is_empty());
    assert!(recommendation
        .section
        .contains("No declaration role was supported by the indexed evidence"));
}

#[tokio::test(start_paused = true)]
async fn test_snapshot_refresh_during_evaluation_does_not_mix_generations() {
    let published = Arc::new(Mutex::new(snapshot(vec![
        simple_file("apps/api/upload.rs"),
        simple_file("packages/core/retry.rs"),
    ])));
    let held = Arc::clone(&published.lock().unwrap());
    let candidates = RootCandidates::project(&held);
    let mut script = Script::default();
    script
        .scores
        .insert("packages/core/retry.rs".into(), DIRECT);
    let evaluator = evaluator(FakeTransport::new(script).with_delay(Duration::from_secs(2)));
    let refresh = async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        *published.lock().unwrap() = snapshot(vec![simple_file("apps/api/replacement.rs")]);
    };
    let (outcome, ()) = tokio::join!(
        recommend(&candidates, "overview", &evaluator, options(None)),
        refresh
    );
    let recommendation = applied(outcome);
    assert_eq!(recommendation.snapshot_identity, Arc::as_ptr(&held).addr());
    assert_ne!(
        recommendation.snapshot_identity,
        Arc::as_ptr(&published.lock().unwrap()).addr()
    );
    let paths: BTreeSet<String> = recommendation
        .judgments
        .iter()
        .map(|judgment| judgment.path.clone())
        .collect();
    assert_eq!(
        paths,
        BTreeSet::from([
            "apps/api/upload.rs".to_string(),
            "packages/core/retry.rs".to_string()
        ])
    );
    assert!(!scored_paths(evaluator.transport()).contains("apps/api/replacement.rs"));
    assert_eq!(recommendation.recommended[0].path, "packages/core/retry.rs");
}

#[tokio::test]
async fn test_small_output_room_bypasses_before_any_request() {
    let candidates = RootCandidates::project(&snapshot(vec![simple_file("src/a.rs")]));
    let (outcome, evaluator) = run(
        &candidates,
        FakeTransport::new(Script::default()),
        Some(900),
    )
    .await;
    assert!(matches!(outcome, RecommendationOutcome::OutputBudgetBypass));
    assert_eq!(evaluator.transport().calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_rendered_section_respects_the_output_room() {
    let mut files = Vec::new();
    let mut script = Script::default();
    for index in 0..10 {
        let path = format!("src/flow/step_{index:02}.rs");
        script.scores.insert(path.clone(), DIRECT);
        script
            .roles
            .insert(format!("step_{index:02}_entry"), "consumer");
        files.push(simple_file(&path));
    }
    let candidates = RootCandidates::project(&snapshot(files));
    let room = 2_000;
    let (outcome, _) = run(&candidates, FakeTransport::new(script), Some(room)).await;
    let recommendation = applied(outcome);
    assert_eq!(recommendation.recommended.len(), 10);
    assert!(recommendation.section.len() <= room - REDACTION_MARGIN_BYTES);
    assert!(recommendation
        .section
        .contains("more recommended files omitted by the overview output cap"));
    assert!(recommendation.section.contains("[jev overview: applied"));
}

#[tokio::test]
async fn test_role_stage_failure_falls_back_with_both_stages_accounted() {
    let candidates = RootCandidates::project(&snapshot(vec![simple_file("src/upload.rs")]));
    let script = Script {
        failing_status_for_roles: Some(529),
        scores: HashMap::from([("src/upload.rs".to_string(), DIRECT)]),
        ..Script::default()
    };
    let (outcome, _) = run(&candidates, FakeTransport::new(script), None).await;
    let RecommendationOutcome::Fallback(fallback) = outcome else {
        panic!("expected a fallback");
    };
    assert_eq!(fallback.stage, EvaluationStage::DeclarationRoles);
    assert_eq!(fallback.request_count, 2);
    assert_eq!(fallback.usage.input_tokens, 1_000);
    assert!(fallback
        .note()
        .contains("[jev overview: fallback (http_status) · original output preserved"));
}

#[tokio::test(start_paused = true)]
async fn test_deadline_during_file_scores_is_a_fallback_not_a_ranking() {
    let candidates = RootCandidates::project(&snapshot(vec![simple_file("src/upload.rs")]));
    let evaluator =
        evaluator(FakeTransport::new(Script::default()).with_delay(Duration::from_secs(60)));
    let mut request_options = options(None);
    request_options.deadline = Instant::now() + Duration::from_secs(5);
    let outcome = recommend(&candidates, "overview", &evaluator, request_options).await;
    let RecommendationOutcome::Fallback(fallback) = outcome else {
        panic!("expected a fallback");
    };
    assert_eq!(fallback.stage, EvaluationStage::FileScores);
    assert_eq!(fallback.error.label(), "deadline_exceeded");
    assert_eq!(fallback.request_count, 0);
}

#[tokio::test]
async fn test_selection_replays_from_raw_answers_without_inference() {
    let files = vec![
        simple_file("src/a.rs"),
        simple_file("src/b.rs"),
        simple_file("src/c.rs"),
    ];
    let candidates = RootCandidates::project(&snapshot(files));
    let mut script = Script::default();
    script.scores.insert("src/a.rs".into(), SUPPORTING);
    script.scores.insert("src/c.rs".into(), DIRECT);
    let (outcome, evaluator) = run(&candidates, FakeTransport::new(script), None).await;
    let recommendation = applied(outcome);
    let calls_before = evaluator.transport().calls.load(Ordering::SeqCst);
    let answers = &recommendation.file_scores.as_ref().unwrap().answers;
    let judgments = judge_files(&candidates, answers).unwrap();
    assert_eq!(judgments, recommendation.judgments);
    let (status, selected) = select_files(&judgments);
    assert_eq!(status, recommendation.status);
    let replayed: Vec<&str> = selected
        .iter()
        .map(|judgment| judgment.path.as_str())
        .collect();
    assert_eq!(replayed, ["src/c.rs", "src/a.rs"]);
    assert_eq!(
        evaluator.transport().calls.load(Ordering::SeqCst),
        calls_before
    );
    assert_eq!(
        recommendation.versions(),
        [
            FILE_SCORE_QUESTION_VERSION,
            DECLARATION_ROLE_QUESTION_VERSION,
            SELECTION_POLICY_VERSION
        ]
    );
}

#[tokio::test]
async fn test_model_bound_state_carries_the_explicit_task_query() {
    let candidates = RootCandidates::project(&snapshot(vec![simple_file("src/a.rs")]));
    let (_, evaluator) = run(&candidates, FakeTransport::new(Script::default()), None).await;
    let bodies = evaluator.transport().bodies();
    assert_eq!(
        bodies[0]["state"]["task_query"],
        "Where are failed uploads retried?"
    );
    assert_eq!(
        bodies[0]["state"]["repository_overview"],
        "# Root Codemap Overview\n"
    );
    let question = bodies[0]["questions"]["f00000p000"].clone();
    assert!(question["instructions"]["question"]
        .as_str()
        .unwrap()
        .contains("`task_query`"));
    assert_eq!(question["criteria"].as_array().unwrap().len(), 4);
}

#[test]
fn test_projection_applies_declaration_rules_and_links_calls() {
    let mut local_helper = symbol("local_helper", "fn", 12, 14);
    local_helper.flags.is_exported = false;
    let mut test_case = symbol("test_upload", "fn", 60, 70);
    test_case.flags.is_test = true;
    let mut method = symbol("send", "method", 30, 40);
    method.owner = Some("UploadClient".to_string());
    method.docstring = Some("Sends one chunk.\nRetries are handled by the caller.".to_string());
    let files = vec![
        file(
            "src/upload.rs",
            vec![
                symbol("upload", "mod", 1, 80),
                symbol("run_upload", "fn", 10, 20),
                local_helper,
                method,
                test_case,
            ],
            vec![
                call("send", Some("self.client"), 15),
                call("local_helper", None, 16),
            ],
        ),
        file(
            "tests/upload_test.rs",
            vec![symbol("helper", "fn", 1, 3)],
            Vec::new(),
        ),
    ];
    let candidates = RootCandidates::project(&snapshot(files));
    let upload = candidates
        .files()
        .iter()
        .find(|file| file.path == "src/upload.rs")
        .unwrap();
    let names: Vec<&str> = upload
        .declarations
        .iter()
        .map(|declaration| declaration.name.as_str())
        .collect();
    assert_eq!(names, ["run_upload", "send"]);
    assert!(!upload.is_test_file);
    assert!(candidates
        .files()
        .iter()
        .any(|file| file.path == "tests/upload_test.rs" && file.is_test_file));

    let run_upload = &upload.declarations[0];
    assert_eq!(run_upload.outgoing_calls.len(), 2);
    assert_eq!(
        run_upload.outgoing_calls[0],
        CallEvidence {
            call: "self.client.send".to_string(),
            line: 15,
            target: Some("src/upload.rs:UploadClient.send".to_string()),
        }
    );
    let send = &upload.declarations[1];
    assert_eq!(send.qualified_name(), "UploadClient.send");
    assert_eq!(
        send.possible_callers,
        vec![CallerEvidence {
            path: "src/upload.rs".to_string(),
            caller: "run_upload".to_string(),
            line: 15
        }]
    );
    let fragment = &upload.fragments[0];
    assert_eq!(
        fragment["declarations"],
        json!([
            "run_upload (fn) L10-20",
            "UploadClient.send (method) L30-40"
        ])
    );
    assert_eq!(
        fragment["docs"],
        json!(["UploadClient.send: Sends one chunk."])
    );
    assert_eq!(
        fragment["calls"],
        json!(["self.client.send", "local_helper"])
    );
    assert_eq!(
        (fragment["part"].clone(), fragment["parts"].clone()),
        (json!(1), json!(1))
    );
}
