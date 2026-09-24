//! Offline regression tests for the Jev runtime. No test contacts the provider: every
//! evaluation runs through `mock::MockTransport`, `mock::MockEvaluator` (mostly on a paused
//! Tokio clock) or, for the transport policy checks, a plain-HTTP listener on 127.0.0.1 that
//! only this test module can point the client at.

use super::answer::parse_answer;
use super::batch::pack;
use super::mock::{answers, raw_answer, Gate, MockEvaluator, MockTransport, ScriptedResponse};
use super::*;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::Instant;

const TASK_QUERY: &str = "find where search result bodies are kept or omitted";
const EVIDENCE_PATH: &str = "src/tools/search/mod.rs";
const LEVELS: [&str; 4] = [
    "no evidence",
    "weak evidence",
    "strong evidence",
    "direct match",
];

fn id(name: &str) -> QuestionId {
    QuestionId::new(name).expect("valid question id")
}

fn levels() -> Vec<Value> {
    LEVELS.iter().map(|level| json!(level)).collect()
}

fn score_question(name: &str, instructions: impl Into<String>) -> Question {
    Question::score(id(name), Value::String(instructions.into()), levels())
        .expect("valid score question")
}

fn keep_omit() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("keep".to_string(), json!("Keep the body")),
        ("omit".to_string(), json!("Omit the body")),
    ])
}

fn choice_question(name: &str) -> Question {
    Question::choice(
        id(name),
        json!("Decide whether `candidate` should keep its body for `task_query`."),
        keep_omit(),
    )
    .expect("valid choice question")
}

fn noul_question(name: &str) -> Question {
    Question::noul(
        id(name),
        json!("Is `candidate` unrelated to `task_query`?"),
        Some(NoulCriteria {
            when_true: json!("unrelated"),
            when_false: json!("related"),
        }),
    )
    .expect("valid noul question")
}

/// The adapters' state convention: the caller's intent under `task_query` plus named
/// evidence. The runtime treats it as opaque JSON.
fn state_fields() -> Map<String, Value> {
    let mut state = Map::new();
    state.insert(TASK_QUERY_FIELD.into(), json!(TASK_QUERY));
    state.insert(
        "candidate".into(),
        json!({ "path": EVIDENCE_PATH, "summary": "groups BM25 hits per file" }),
    );
    state
}

fn state() -> Value {
    Value::Object(state_fields())
}

fn request(questions: Vec<Question>) -> EvaluationRequest {
    EvaluationRequest::new(state(), questions).expect("valid request")
}

fn large_questions(count: usize, instruction_bytes: usize) -> Vec<Question> {
    (0..count)
        .map(|index| score_question(&format!("q{index}"), "x".repeat(instruction_bytes)))
        .collect()
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn response_bytes(value: Value) -> Vec<u8> {
    serde_json::to_vec(&value).expect("response serializes")
}

/// A provider-shaped 200 response answering every question in a decoded request body.
fn answer_body(body: &Value) -> ScriptedResponse {
    ScriptedResponse::ok(answer_bytes(body))
}

fn answer_bytes(body: &Value) -> Vec<u8> {
    let questions = body["questions"]
        .as_object()
        .expect("request carries a questions object");
    let mut answers = Map::new();
    for (name, question) in questions {
        let answer = match question["type"].as_str() {
            Some("score") => {
                let level_count = question["criteria"].as_array().map_or(0, Vec::len);
                let chosen = 2.min(level_count.saturating_sub(1));
                let probabilities: Map<String, Value> = (0..level_count)
                    .map(|level| {
                        (
                            level.to_string(),
                            json!(if level == chosen { 1.0 } else { 0.0 }),
                        )
                    })
                    .collect();
                json!({
                    "type": "score",
                    "score": chosen as f64,
                    "confidence": 1.0,
                    "legend": {},
                    "probabilities": probabilities,
                })
            }
            Some("choice") => {
                let options = question["criteria"].as_object().expect("choice criteria");
                let chosen = options.keys().next().expect("at least one option").clone();
                let probabilities: Map<String, Value> = options
                    .keys()
                    .map(|option| {
                        (
                            option.clone(),
                            json!(if *option == chosen { 1.0 } else { 0.0 }),
                        )
                    })
                    .collect();
                json!({
                    "type": "choice",
                    "choice": chosen,
                    "confidence": 1.0,
                    "probabilities": probabilities,
                })
            }
            _ => json!({ "type": "noul", "noul": 0.9 }),
        };
        answers.insert(name.clone(), answer);
    }
    response_bytes(json!({
        "model": DEFAULT_MODEL,
        "answers": answers,
        "usage": { "input_tokens": 100, "output_tokens": 10 },
    }))
}

fn handler_evaluator(
    config: EvaluatorConfig,
    delay: Duration,
) -> (JevEvaluator, Arc<MockTransport>) {
    let transport = Arc::new(MockTransport::with_handler(move |body| {
        answer_body(body).after(delay)
    }));
    let evaluator = JevEvaluator::new(transport.clone(), config).expect("valid config");
    (evaluator, transport)
}

fn scripted_evaluator(responses: Vec<ScriptedResponse>) -> (JevEvaluator, Arc<MockTransport>) {
    let transport = Arc::new(MockTransport::scripted(responses));
    let evaluator =
        JevEvaluator::new(transport.clone(), EvaluatorConfig::default()).expect("valid config");
    (evaluator, transport)
}

/// Every 600-byte question needs its own batch under this ceiling.
fn single_question_batches() -> EvaluatorConfig {
    EvaluatorConfig {
        max_batch_bytes: 1024,
        ..EvaluatorConfig::default()
    }
}

// ---------------------------------------------------------------------------------------
// Questions and requests
// ---------------------------------------------------------------------------------------

#[test]
fn question_id_accepts_only_the_documented_charset_and_length() {
    assert!(QuestionId::new("file.0:decl-1_x").is_ok());
    assert!(QuestionId::new("a".repeat(QuestionId::MAX_LEN)).is_ok());
    let too_long = "a".repeat(QuestionId::MAX_LEN + 1);
    for invalid in ["", " ", "has space", "slash/", "한글", too_long.as_str()] {
        assert!(
            matches!(
                QuestionId::new(invalid),
                Err(JevError::InvalidQuestionId(_))
            ),
            "{invalid:?}"
        );
    }
}

#[test]
fn score_question_enforces_the_published_level_limits() {
    let levels = |count: usize| {
        (0..count)
            .map(|level| json!(format!("level {level}")))
            .collect::<Vec<_>>()
    };
    assert!(matches!(
        Question::score(id("s"), json!("judge"), levels(1)),
        Err(JevError::InvalidCriteria { .. })
    ));
    assert!(Question::score(id("s"), json!("judge"), levels(Question::MIN_SCORE_LEVELS)).is_ok());
    assert!(Question::score(id("s"), json!("judge"), levels(Question::MAX_SCORE_LEVELS)).is_ok());
    assert!(matches!(
        Question::score(
            id("s"),
            json!("judge"),
            levels(Question::MAX_SCORE_LEVELS + 1)
        ),
        Err(JevError::InvalidCriteria { .. })
    ));
    assert!(matches!(
        Question::score(id("s"), json!("judge"), vec![json!("a"), json!("  ")]),
        Err(JevError::InvalidCriteria { .. })
    ));
    let question = Question::score(id("s"), json!("judge"), levels(4)).unwrap();
    assert_eq!(question.kind().level_count(), Some(4));
    let wire = question.to_wire();
    assert_eq!(wire["type"], "score");
    assert_eq!(wire["criteria"].as_array().map(Vec::len), Some(4));
}

#[test]
fn choice_question_enforces_option_limits_and_keys() {
    let options = |count: usize| {
        (0..count)
            .map(|index| (format!("option{index}"), json!(null)))
            .collect::<BTreeMap<_, _>>()
    };
    assert!(matches!(
        Question::choice(id("c"), json!("pick"), options(1)),
        Err(JevError::InvalidCriteria { .. })
    ));
    assert!(Question::choice(
        id("c"),
        json!("pick"),
        options(Question::MIN_CHOICE_OPTIONS)
    )
    .is_ok());
    assert!(Question::choice(
        id("c"),
        json!("pick"),
        options(Question::MAX_CHOICE_OPTIONS)
    )
    .is_ok());
    assert!(matches!(
        Question::choice(
            id("c"),
            json!("pick"),
            options(Question::MAX_CHOICE_OPTIONS + 1)
        ),
        Err(JevError::InvalidCriteria { .. })
    ));
    let blank_key = BTreeMap::from([(" ".to_string(), json!("a")), ("b".to_string(), json!("b"))]);
    assert!(matches!(
        Question::choice(id("c"), json!("pick"), blank_key),
        Err(JevError::InvalidCriteria { .. })
    ));
    let question = Question::choice(id("c"), json!("pick"), keep_omit()).unwrap();
    assert_eq!(question.to_wire()["criteria"]["keep"], "Keep the body");
}

#[test]
fn instructions_must_carry_content() {
    for blank in [
        json!(""),
        json!("   "),
        json!({}),
        json!([]),
        json!(null),
        json!(3),
    ] {
        assert!(
            matches!(
                Question::noul(id("n"), blank.clone(), None),
                Err(JevError::InvalidInstructions { .. })
            ),
            "{blank}"
        );
    }
    assert!(Question::noul(id("n"), json!({ "judge": "is it related?" }), None).is_ok());
    assert!(Question::noul(id("n"), json!(["step one", "step two"]), None).is_ok());
}

#[test]
fn noul_wire_shape_omits_absent_criteria() {
    let bare = Question::noul(id("n"), json!("related?"), None)
        .unwrap()
        .to_wire();
    assert_eq!(bare, json!({ "type": "noul", "instructions": "related?" }));
    let with_criteria = noul_question("n").to_wire();
    assert_eq!(
        with_criteria["criteria"],
        json!({ "true": "unrelated", "false": "related" })
    );
    assert!(matches!(
        Question::noul(
            id("n"),
            json!("related?"),
            Some(NoulCriteria {
                when_true: json!(""),
                when_false: json!("x"),
            })
        ),
        Err(JevError::InvalidCriteria { .. })
    ));
}

#[test]
fn request_validates_state_shape_unique_ids_and_question_count() {
    for unsupported in [json!(null), json!(true), json!(3), json!(""), json!("   ")] {
        assert!(
            matches!(
                EvaluationRequest::new(unsupported.clone(), vec![noul_question("n")]),
                Err(JevError::InvalidState(_))
            ),
            "{unsupported}"
        );
    }
    for supported in [
        json!("plain text state"),
        json!({}),
        json!([]),
        json!(["a", "b"]),
        state(),
    ] {
        assert!(
            EvaluationRequest::new(supported.clone(), vec![noul_question("n")]).is_ok(),
            "{supported}"
        );
    }
    assert!(matches!(
        EvaluationRequest::new(state(), vec![]),
        Err(JevError::EmptyQuestions)
    ));
    assert!(matches!(
        EvaluationRequest::new(state(), vec![noul_question("n"), choice_question("n")]),
        Err(JevError::DuplicateQuestionId(_))
    ));
    let too_many: Vec<Question> = (0..=MAX_QUESTIONS_PER_REQUEST)
        .map(|index| noul_question(&format!("q{index}")))
        .collect();
    assert!(matches!(
        EvaluationRequest::new(state(), too_many),
        Err(JevError::TooManyQuestions { count, limit })
            if count == MAX_QUESTIONS_PER_REQUEST + 1 && limit == MAX_QUESTIONS_PER_REQUEST
    ));

    // The runtime neither adds nor reads state fields: the intent is whatever the caller
    // named, and a caller that names nothing sends nothing.
    let request = request(vec![noul_question("n")]);
    assert_eq!(request.state()[TASK_QUERY_FIELD], TASK_QUERY);
    assert_eq!(
        request.state_field("candidate").unwrap()["path"],
        EVIDENCE_PATH
    );
    let without_intent =
        EvaluationRequest::new(json!({ "candidate": "x" }), vec![noul_question("n")]).unwrap();
    assert!(without_intent.state_field(TASK_QUERY_FIELD).is_none());
}

// ---------------------------------------------------------------------------------------
// Batching and token estimates
// ---------------------------------------------------------------------------------------

#[test]
fn pack_splits_greedily_on_the_exact_byte_ceiling() {
    let limits = BatchLimits {
        max_batch_bytes: 2_000,
        ..BatchLimits::default()
    };
    let questions = large_questions(7, 300);
    let request = request(questions.clone());
    let batches =
        pack(DEFAULT_MODEL, request.state(), request.questions(), &limits).expect("packs");
    assert!(
        batches.len() >= 2,
        "expected a split, got {} batch",
        batches.len()
    );

    let mut seen = Vec::new();
    for (position, batch) in batches.iter().enumerate() {
        assert_eq!(batch.index, position);
        assert!(
            batch.body.len() <= limits.max_batch_bytes,
            "batch {position} is {} bytes",
            batch.body.len()
        );
        let decoded: Value = serde_json::from_slice(&batch.body).expect("batch body is JSON");
        assert_eq!(decoded["model"], DEFAULT_MODEL);
        assert_eq!(&decoded["state"], request.state());
        let mut keys: Vec<&str> = decoded["questions"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        let mut ids: Vec<&str> = batch.question_ids.iter().map(QuestionId::as_str).collect();
        ids.sort_unstable();
        assert_eq!(keys, ids);
        seen.extend(batch.question_ids.iter().cloned());
        if let Some(next) = batches.get(position + 1) {
            let first_next = questions
                .iter()
                .find(|question| question.id() == &next.question_ids[0])
                .unwrap();
            let entry = serde_json::to_vec(&first_next.to_wire()).unwrap().len()
                + first_next.id().as_str().len()
                + 3
                + 1;
            assert!(
                batch.body.len() + entry > limits.max_batch_bytes,
                "batch {position} left room for the next question"
            );
        }
    }
    assert_eq!(
        seen,
        questions
            .iter()
            .map(|question| question.id().clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn pack_rejects_a_single_question_over_the_ceiling() {
    let limits = BatchLimits {
        max_batch_bytes: 1_024,
        ..BatchLimits::default()
    };
    let request = request(vec![
        score_question("small", "fits"),
        score_question("huge", "x".repeat(1_100)),
    ]);
    let error = pack(DEFAULT_MODEL, request.state(), request.questions(), &limits)
        .expect_err("oversized question");
    assert!(matches!(
        error,
        JevError::QuestionTooLarge { ref id, request_bytes, max_batch_bytes: 1_024 }
            if id.as_str() == "huge" && request_bytes > 1_100
    ));
    assert_eq!(error.kind(), "question_too_large");
}

#[test]
fn pack_splits_on_the_total_token_limit() {
    let limits = BatchLimits {
        max_estimated_tokens: 2_500,
        ..BatchLimits::default()
    };
    let request = request(large_questions(3, 3_000));
    let batches = pack(DEFAULT_MODEL, request.state(), request.questions(), &limits).unwrap();
    assert_eq!(batches.len(), 2);
    assert_eq!(
        batches
            .iter()
            .map(|batch| batch.question_ids.len())
            .sum::<usize>(),
        3
    );
    for batch in batches {
        let body: Value = serde_json::from_slice(&batch.body).unwrap();
        let state_tokens = estimate_tokens(serde_json::to_vec(&body["state"]).unwrap().len());
        let question_tokens: u64 = body["questions"]
            .as_object()
            .unwrap()
            .values()
            .map(|question| estimate_tokens(serde_json::to_vec(question).unwrap().len()))
            .sum();
        assert!(state_tokens + question_tokens <= limits.max_estimated_tokens);
    }
}

#[test]
fn pack_reports_the_state_plus_longest_question_limit() {
    let limits = BatchLimits {
        max_estimated_state_plus_longest_question_tokens: 1_500,
        ..BatchLimits::default()
    };
    let mut state = state_fields();
    state.insert("shared".into(), json!("s".repeat(3_000)));
    let request = EvaluationRequest::new(
        Value::Object(state),
        vec![
            score_question("long", "x".repeat(3_000)),
            score_question("small", "tiny"),
        ],
    )
    .unwrap();
    let error = pack(DEFAULT_MODEL, request.state(), request.questions(), &limits)
        .expect_err("over the state-plus-longest estimate");
    assert!(matches!(
        error,
        JevError::EstimatedTokenLimit {
            kind: TokenLimitKind::StateWithLongestQuestion,
            batch_index: 0,
            limit_tokens: 1_500,
            ..
        }
    ));
    assert_eq!(error.kind(), "estimated_token_limit");
}

#[test]
fn pack_bounds_the_number_of_batches() {
    let limits = BatchLimits {
        max_batch_bytes: 1_024,
        max_batches: 2,
        ..BatchLimits::default()
    };
    let request = request(large_questions(3, 600));
    let error = pack(DEFAULT_MODEL, request.state(), request.questions(), &limits)
        .expect_err("three single-question batches over a two-batch bound");
    assert_eq!(error, JevError::TooManyBatches { count: 3, limit: 2 });
    assert_eq!(error.kind(), "too_many_batches");
    assert_eq!(BatchLimits::default().max_batches, MAX_BATCHES_PER_REQUEST);
    assert_eq!(BatchLimits::default().max_batch_bytes, MAX_BATCH_BYTES);
}

#[test]
fn token_estimate_rounds_bytes_up() {
    assert_eq!(ESTIMATED_BYTES_PER_TOKEN, 3);
    assert_eq!(estimate_tokens(0), 0);
    assert_eq!(estimate_tokens(1), 1);
    assert_eq!(estimate_tokens(3), 1);
    assert_eq!(estimate_tokens(4), 2);
}

// ---------------------------------------------------------------------------------------
// Answer validation
// ---------------------------------------------------------------------------------------

#[test]
fn parse_answer_rejects_malformed_answers_with_a_reason() {
    let score = score_question("s", "judge");
    let choice = choice_question("c");
    let noul = noul_question("n");
    let cases: Vec<(&Question, Value, &str)> = vec![
        (&score, json!("2"), "answer must be an object"),
        (
            &score,
            json!({ "type": "choice", "choice": "keep" }),
            "does not match question type",
        ),
        (
            &score,
            json!({ "type": "score", "score": 2.0, "probabilities": { "0": 0, "1": 0, "2": 1 } }),
            "missing level 3",
        ),
        (
            &score,
            json!({ "type": "score", "score": 2.0, "probabilities": { "0": 0, "1": 0, "2": 1, "3": 0, "4": 0 } }),
            "outside the 4 declared levels",
        ),
        (
            &score,
            json!({ "type": "score", "score": 2.0, "probabilities": { "0": 0, "1": 0, "2": 0.5, "3": 0 } }),
            "sums to",
        ),
        (
            &score,
            json!({ "type": "score", "score": 3.5, "probabilities": { "0": 0, "1": 0, "2": 0, "3": 1 } }),
            "outside [0, 3]",
        ),
        (
            &score,
            json!({ "type": "score", "score": "2", "probabilities": { "0": 0, "1": 0, "2": 1, "3": 0 } }),
            "'score' must be a finite number",
        ),
        (
            &score,
            json!({ "type": "score", "score": 2.0, "probabilities": { "0": 0, "1": 0, "2": 1.2, "3": -0.2 } }),
            "must lie in [0, 1]",
        ),
        // The score must be the probability-weighted level: 3.0 with all mass on level 0.
        (
            &score,
            json!({ "type": "score", "score": 3.0, "probabilities": { "0": 1, "1": 0, "2": 0, "3": 0 } }),
            "disagrees with the probability-weighted level",
        ),
        (
            &choice,
            json!({ "type": "choice", "choice": "drop", "probabilities": { "keep": 0.5, "omit": 0.5 } }),
            "unknown option 'drop'",
        ),
        (
            &choice,
            json!({ "type": "choice", "choice": "keep", "probabilities": { "keep": 1.0 } }),
            "missing option 'omit'",
        ),
        (
            &choice,
            json!({ "type": "choice", "choice": "keep", "probabilities": { "keep": 1.0, "omit": 0.0, "drop": 0.0 } }),
            "unknown option 'drop'",
        ),
        // The chosen option must be one of the most probable ones.
        (
            &choice,
            json!({ "type": "choice", "choice": "omit", "probabilities": { "keep": 0.9, "omit": 0.1 } }),
            "while another option reaches",
        ),
        (
            &noul,
            json!({ "type": "noul", "noul": 1.5 }),
            "must lie in [0, 1]",
        ),
        (
            &noul,
            json!({ "type": "noul" }),
            "'noul' must be a finite number",
        ),
    ];
    for (question, raw, expected) in cases {
        let reason = parse_answer(question, &raw).expect_err(&format!("{raw} should fail"));
        assert!(reason.contains(expected), "{raw}: {reason}");
    }
}

#[test]
fn parse_answer_accepts_values_within_the_recorded_tolerances() {
    let score = score_question("s", "judge");
    let answer = parse_answer(
        &score,
        &json!({
            "type": "score",
            "score": 3.01,
            "probabilities": { "0": 0.0, "1": 0.0, "2": 0.01, "3": 1.0 },
            "confidence": null,
            "legend": { "3": "direct match" },
        }),
    )
    .expect("within tolerance");
    let score_answer = answer.as_score().unwrap();
    assert_eq!(score_answer.score, 3.0);
    assert_eq!(score_answer.confidence, None);
    assert_eq!(score_answer.level_count(), 4);
    assert!((score_answer.mass_at_or_above(3) - 1.0).abs() < 1e-9);
    assert!((score_answer.mass_below(3) - 0.01).abs() < 1e-9);

    // A two-decimal answer whose weighted level differs from the score by rounding only.
    let rounded = parse_answer(
        &score,
        &json!({
            "type": "score",
            "score": 2.11,
            "probabilities": { "0": 0.03, "1": 0.13, "2": 0.54, "3": 0.30 },
            "confidence": 0.53,
        }),
    )
    .expect("recorded provider precision is accepted");
    assert_eq!(rounded.as_score().unwrap().score, 2.11);
    assert!((score_weighted_sum_tolerance(4) - 0.035).abs() < 1e-6);
    assert!((score_weighted_sum_tolerance(10) - 0.23).abs() < 1e-6);
    assert!((distribution_sum_tolerance(4) - 0.02).abs() < 1e-6);
    assert!((distribution_sum_tolerance(8) - 0.04).abs() < 1e-6);
    assert!((distribution_sum_tolerance(2) - 0.02).abs() < 1e-6);

    // Rounded Choice ties: a 0.50/0.50 split may name either option, 0.49 vs 0.50 too.
    let choice = choice_question("c");
    for (chosen, keep, omit) in [("keep", 0.5, 0.5), ("omit", 0.5, 0.5), ("omit", 0.5, 0.49)] {
        let raw = json!({ "type": "choice", "choice": chosen, "probabilities": { "keep": keep, "omit": omit } });
        assert!(parse_answer(&choice, &raw).is_ok(), "{raw}");
    }
    // Eight rounded options summing to 1.04 stay within the per-entry tolerance.
    let eight: BTreeMap<String, Value> = (0..8)
        .map(|index| (format!("o{index}"), json!(null)))
        .collect();
    let wide = Question::choice(id("w"), json!("pick"), eight).unwrap();
    let mut probabilities = Map::new();
    for index in 0..8 {
        probabilities.insert(format!("o{index}"), json!(0.13));
    }
    let raw = json!({ "type": "choice", "choice": "o0", "probabilities": probabilities });
    assert!(parse_answer(&wide, &raw).is_ok(), "{raw}");
}

#[test]
fn noul_answer_ignores_confidence_and_never_requires_it() {
    let noul = noul_question("n");
    let with = parse_answer(
        &noul,
        &json!({ "type": "noul", "noul": 0.4, "confidence": 0.99 }),
    )
    .unwrap();
    let without = parse_answer(&noul, &json!({ "type": "noul", "noul": 0.4 })).unwrap();
    assert_eq!(with, without);
    assert_eq!(with.as_noul().unwrap().noul, 0.4);
    assert!(parse_answer(&noul, &json!({ "type": "noul", "noul": -0.1 })).is_err());
}

#[test]
fn mock_raw_answers_round_trip_through_the_validator() {
    let score = answers::score(&[0.0, 0.0, 1.0, 0.0]);
    assert_eq!(
        parse_answer(&score_question("s", "judge"), &raw_answer(&score)).unwrap(),
        score
    );
    let choice = answers::choice("keep", &[("keep", 0.8), ("omit", 0.2)]);
    assert_eq!(
        parse_answer(&choice_question("c"), &raw_answer(&choice)).unwrap(),
        choice
    );
    let noul = answers::noul(0.25);
    assert_eq!(
        parse_answer(&noul_question("n"), &raw_answer(&noul)).unwrap(),
        noul
    );
}

// ---------------------------------------------------------------------------------------
// Evaluation round trips
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn mixed_round_trip_returns_typed_answers_usage_and_request_identity() {
    let response = json!({
        "model": DEFAULT_MODEL,
        "answers": {
            "relevance": {
                "type": "score",
                "score": 2.0,
                "confidence": 1.0,
                "legend": { "2": "strong evidence" },
                "probabilities": { "0": 0.0, "1": 0.0, "2": 1.0, "3": 0.0 },
            },
            "body": {
                "type": "choice",
                "choice": "keep",
                "confidence": 0.9,
                "probabilities": { "keep": 0.9, "omit": 0.1 },
            },
            "unrelated": { "type": "noul", "noul": 0.9 },
        },
        "usage": { "input_tokens": 296, "output_tokens": 20 },
    });
    let (evaluator, transport) =
        scripted_evaluator(vec![ScriptedResponse::ok(response_bytes(response))]);
    let outcome = evaluator
        .evaluate(request(vec![
            score_question("relevance", "Rate `candidate` for `task_query`."),
            choice_question("body"),
            noul_question("unrelated"),
        ]))
        .await
        .expect("mixed batch succeeds");

    assert_eq!(outcome.model, DEFAULT_MODEL);
    let score = outcome.score(&id("relevance")).expect("score answer");
    assert_eq!(score.score, 2.0);
    assert_eq!(score.probabilities, vec![0.0, 0.0, 1.0, 0.0]);
    assert_eq!(score.confidence, Some(1.0));
    assert!((score.mass_at_or_above(2) - 1.0).abs() < 1e-9);
    assert!(score.mass_below(2).abs() < 1e-9);
    let choice = outcome.choice(&id("body")).expect("choice answer");
    assert_eq!(choice.choice, "keep");
    assert!((choice.probability("omit") - 0.1).abs() < 1e-9);
    assert_eq!(choice.confidence, Some(0.9));
    assert_eq!(
        outcome.noul(&id("unrelated")).expect("noul answer").noul,
        0.9
    );
    assert_eq!(
        outcome.raw_answers[&id("unrelated")],
        json!({ "type": "noul", "noul": 0.9 })
    );
    assert_eq!(outcome.usage, Usage::reported(296, 20));
    assert_eq!(outcome.timing.request_count, 1);

    let posts = transport.posts();
    assert_eq!(posts.len(), 1);
    assert_eq!(outcome.requests.len(), 1);
    let identity = &outcome.requests[0];
    assert_eq!(identity.batch_index, 0);
    assert_eq!(identity.http_status, Some(200));
    assert_eq!(
        identity.question_ids,
        vec![id("relevance"), id("body"), id("unrelated")]
    );
    assert_eq!(identity.request_bytes, posts[0].bytes.len());
    assert_eq!(
        identity.request_sha256,
        hex(Sha256::digest(&posts[0].bytes))
    );
    let wire = &posts[0].body;
    assert_eq!(wire["model"], DEFAULT_MODEL);
    assert_eq!(wire["state"][TASK_QUERY_FIELD], TASK_QUERY);
    assert_eq!(wire["state"]["candidate"]["path"], EVIDENCE_PATH);
    assert_eq!(wire["questions"]["relevance"]["type"], "score");
    assert_eq!(
        wire["questions"]["body"]["criteria"]["omit"],
        "Omit the body"
    );
    assert_eq!(
        wire["questions"]["unrelated"]["criteria"]["true"],
        "unrelated"
    );
    assert!(posts[0].timeout <= Duration::from_millis(45_000));
    assert!(posts[0].timeout > Duration::from_millis(44_000));
}

#[tokio::test]
async fn invalid_answer_fails_the_batch_but_keeps_usage_and_identity() {
    let response = json!({
        "model": DEFAULT_MODEL,
        "answers": { "n": { "type": "noul", "noul": 7 } },
        "usage": { "input_tokens": 50, "output_tokens": 5 },
    });
    let (evaluator, _) = scripted_evaluator(vec![ScriptedResponse::ok(response_bytes(response))]);
    let failure = evaluator
        .evaluate(request(vec![noul_question("n")]))
        .await
        .expect_err("invalid noul value");
    assert!(matches!(
        failure.error,
        JevError::InvalidAnswer { ref id, .. } if id.as_str() == "n"
    ));
    assert_eq!(failure.error.kind(), "invalid_answer");
    assert_eq!(failure.usage, Usage::reported(50, 5));
    assert_eq!(failure.requests.len(), 1);
    assert_eq!(failure.requests[0].http_status, Some(200));
    assert_eq!(failure.timing.request_count, 1);
    assert_eq!(evaluator.available_permits(), 3);
}

#[tokio::test]
async fn model_mismatch_incomplete_sets_and_non_json_bodies_are_explicit() {
    let mismatch = json!({
        "model": "jev-latest",
        "answers": { "n": { "type": "noul", "noul": 0.5 } },
        "usage": { "input_tokens": 1, "output_tokens": 1 },
    });
    let (evaluator, _) = scripted_evaluator(vec![ScriptedResponse::ok(response_bytes(mismatch))]);
    let failure = evaluator
        .evaluate(request(vec![noul_question("n")]))
        .await
        .expect_err("model mismatch");
    assert_eq!(
        failure.error,
        JevError::ModelMismatch {
            expected: DEFAULT_MODEL.into(),
            actual: "jev-latest".into(),
        }
    );
    assert_eq!(failure.usage, Usage::reported(1, 1));

    let incomplete = json!({
        "model": DEFAULT_MODEL,
        "answers": {
            "relevance": {
                "type": "score",
                "score": 1.0,
                "probabilities": { "0": 0, "1": 1, "2": 0, "3": 0 },
            },
            "extra": { "type": "noul", "noul": 0.1 },
        },
    });
    let (evaluator, _) = scripted_evaluator(vec![ScriptedResponse::ok(response_bytes(incomplete))]);
    let failure = evaluator
        .evaluate(request(vec![
            score_question("relevance", "judge"),
            choice_question("body"),
        ]))
        .await
        .expect_err("answer set mismatch");
    assert_eq!(
        failure.error,
        JevError::IncompleteAnswers {
            batch_index: 0,
            missing: vec![id("body")],
            unexpected: vec!["extra".into()],
        }
    );
    assert_eq!(failure.usage, Usage::unreported());

    let (evaluator, _) = scripted_evaluator(vec![ScriptedResponse::ok(b"<html>".to_vec())]);
    let failure = evaluator
        .evaluate(request(vec![noul_question("n")]))
        .await
        .expect_err("not JSON");
    assert!(matches!(
        failure.error,
        JevError::InvalidResponse { batch_index: 0, .. }
    ));
    assert_eq!(evaluator.available_permits(), 3);
}

#[tokio::test]
async fn duplicate_answer_keys_are_rejected_before_any_answer_is_read() {
    // `serde_json::Value` would keep the second "n" silently; the runtime must not.
    let body = format!(
        r#"{{"model":"{DEFAULT_MODEL}","answers":{{"n":{{"type":"noul","noul":0.1}},"n":{{"type":"noul","noul":0.9}}}},"usage":{{"input_tokens":3,"output_tokens":1}}}}"#
    );
    let (evaluator, _) = scripted_evaluator(vec![ScriptedResponse::ok(body.into_bytes())]);
    let failure = evaluator
        .evaluate(request(vec![noul_question("n")]))
        .await
        .expect_err("duplicate answer key");
    match &failure.error {
        JevError::InvalidResponse {
            batch_index,
            reason,
        } => {
            assert_eq!(*batch_index, 0);
            assert!(reason.contains("duplicate object key 'n'"), "{reason}");
        }
        other => panic!("unexpected error {other:?}"),
    }
    // Usage cannot be trusted from a body that was rejected as malformed.
    assert_eq!(failure.usage, Usage::unreported());
    assert_eq!(failure.requests.len(), 1);
    assert_eq!(failure.requests[0].http_status, Some(200));

    // Nested duplicates (inside one answer object) are caught as well; well-formed nested
    // objects with repeated keys in *different* objects are fine.
    let nested = format!(
        r#"{{"model":"{DEFAULT_MODEL}","answers":{{"n":{{"type":"noul","noul":0.1,"noul":0.2}}}}}}"#
    );
    let (evaluator, _) = scripted_evaluator(vec![ScriptedResponse::ok(nested.into_bytes())]);
    assert!(matches!(
        evaluator
            .evaluate(request(vec![noul_question("n")]))
            .await
            .unwrap_err()
            .error,
        JevError::InvalidResponse { .. }
    ));
    let fine = format!(
        r#"{{"model":"{DEFAULT_MODEL}","answers":{{"a":{{"type":"noul","noul":0.1}},"b":{{"type":"noul","noul":0.2}}}}}}"#
    );
    let (evaluator, _) = scripted_evaluator(vec![ScriptedResponse::ok(fine.into_bytes())]);
    assert!(evaluator
        .evaluate(request(vec![noul_question("a"), noul_question("b")]))
        .await
        .is_ok());
}

#[tokio::test]
async fn response_bodies_above_the_limit_fail_without_parsing() {
    let body = vec![b' '; MAX_RESPONSE_BODY_BYTES + 1];
    let (evaluator, _) = scripted_evaluator(vec![ScriptedResponse::ok(body)]);
    let failure = evaluator
        .evaluate(request(vec![noul_question("n")]))
        .await
        .expect_err("oversized body");
    match &failure.error {
        JevError::InvalidResponse { reason, .. } => {
            assert!(reason.contains("exceeds the"), "{reason}");
        }
        other => panic!("unexpected error {other:?}"),
    }
    assert_eq!(failure.usage, Usage::unreported());
    assert_eq!(evaluator.available_permits(), 3);
}

#[tokio::test]
async fn http_statuses_and_transport_errors_map_to_bounded_errors() {
    let cases: Vec<(u16, &str, JevError)> = vec![
        (401, "", JevError::Unauthorized { status: 401 }),
        (403, "", JevError::Unauthorized { status: 403 }),
        (
            422,
            r#"{"error":"context window exceeded"}"#,
            JevError::Rejected {
                status: 422,
                excerpt: r#"{"error":"context window exceeded"}"#.into(),
            },
        ),
        (429, "", JevError::RateLimited { status: 429 }),
        (529, "", JevError::Overloaded { status: 529 }),
        (
            500,
            "boom",
            JevError::Http {
                status: 500,
                excerpt: "boom".into(),
            },
        ),
    ];
    for (status, body, expected) in cases {
        let (evaluator, _) = scripted_evaluator(vec![ScriptedResponse::status(status, body)]);
        let failure = evaluator
            .evaluate(request(vec![noul_question("n")]))
            .await
            .expect_err("status failure");
        assert_eq!(failure.error, expected);
        assert_eq!(failure.requests[0].http_status, Some(status));
        assert!(failure.usage.is_empty());
        assert_eq!(evaluator.available_permits(), 3);
    }

    for (error, kind) in [
        (TransportError::Connect("refused".into()), "transport"),
        (
            TransportError::ResponseTooLarge {
                limit: MAX_RESPONSE_BODY_BYTES,
            },
            "transport",
        ),
    ] {
        let (evaluator, _) = scripted_evaluator(vec![ScriptedResponse::error(error.clone())]);
        let failure = evaluator
            .evaluate(request(vec![noul_question("n")]))
            .await
            .expect_err("transport failure");
        assert_eq!(failure.error, JevError::Transport(error));
        assert_eq!(failure.error.kind(), kind);
        assert_eq!(failure.requests[0].http_status, None);
        assert_eq!(failure.timing.request_count, 1);
        assert_eq!(evaluator.available_permits(), 3);
    }
}

// ---------------------------------------------------------------------------------------
// Deadlines, cancellation, spacing and concurrency (paused clock)
// ---------------------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn deadline_counts_queue_time_and_reports_completed_work() {
    let config = EvaluatorConfig {
        deadline: Duration::from_millis(500),
        ..single_question_batches()
    };
    let (evaluator, transport) = handler_evaluator(config, Duration::ZERO);
    let failure = evaluator
        .evaluate(request(large_questions(3, 600)))
        .await
        .expect_err("the third batch would start at 600ms");
    assert_eq!(
        failure.error,
        JevError::DeadlineExceeded {
            deadline: Duration::from_millis(500),
        }
    );
    assert_eq!(transport.post_count(), 2);
    assert_eq!(failure.requests.len(), 2);
    assert!(failure
        .requests
        .iter()
        .all(|identity| identity.http_status == Some(200)));
    assert_eq!(
        failure.usage,
        Usage {
            input_tokens: Some(200),
            output_tokens: Some(20),
            reported_responses: 2,
            unreported_responses: 0,
        }
    );
    assert_eq!(failure.timing.elapsed, Duration::from_millis(500));
    assert_eq!(failure.timing.queue_wait, Duration::from_millis(300));
    assert_eq!(failure.timing.http, Duration::ZERO);
    assert_eq!(failure.timing.request_count, 2);
    assert_eq!(evaluator.available_permits(), 3);
}

#[tokio::test(start_paused = true)]
async fn caller_deadline_shorter_than_the_configured_one_wins() {
    let (evaluator, transport) =
        handler_evaluator(EvaluatorConfig::default(), Duration::from_millis(200));
    let failure = evaluator
        .evaluate(request(vec![noul_question("n")]).with_deadline(Duration::from_millis(100)))
        .await
        .expect_err("caller deadline");
    assert_eq!(
        failure.error,
        JevError::DeadlineExceeded {
            deadline: Duration::from_millis(100),
        }
    );
    assert_eq!(failure.timing.elapsed, Duration::from_millis(100));
    // The in-flight request stays on record as attempted, without a status or usage.
    assert_eq!(failure.requests.len(), 1);
    assert_eq!(failure.requests[0].http_status, None);
    assert!(failure.usage.is_empty());
    assert_eq!(transport.posts()[0].timeout, Duration::from_millis(100));
    assert_eq!(evaluator.available_permits(), 3);
}

#[tokio::test(start_paused = true)]
async fn caller_deadline_longer_than_the_configured_one_is_clamped() {
    let config = EvaluatorConfig {
        deadline: Duration::from_millis(1_000),
        ..EvaluatorConfig::default()
    };
    let (evaluator, transport) = handler_evaluator(config, Duration::from_secs(2));
    let failure = evaluator
        .evaluate(request(vec![noul_question("n")]).with_deadline(Duration::from_secs(60)))
        .await
        .expect_err("clamped deadline");
    assert_eq!(
        failure.error,
        JevError::DeadlineExceeded {
            deadline: Duration::from_millis(1_000),
        }
    );
    assert_eq!(transport.posts()[0].timeout, Duration::from_millis(1_000));
    assert_eq!(evaluator.available_permits(), 3);
}

#[tokio::test(start_paused = true)]
async fn one_absolute_deadline_is_shared_by_consecutive_requests() {
    // An adapter's two stages pass the same instant: the second stage only gets what the
    // first left, and nothing re-adds a duration.
    let (evaluator, transport) =
        handler_evaluator(EvaluatorConfig::default(), Duration::from_millis(200));
    let deadline_at = Instant::now() + Duration::from_millis(300);
    let first = evaluator
        .evaluate(request(vec![noul_question("a")]).with_deadline_at(deadline_at))
        .await
        .expect("200ms of work inside a 300ms budget");
    assert_eq!(first.timing.elapsed, Duration::from_millis(200));
    assert_eq!(transport.posts()[0].timeout, Duration::from_millis(300));

    let failure = evaluator
        .evaluate(request(vec![noul_question("b")]).with_deadline_at(deadline_at))
        .await
        .expect_err("only 100ms remain and the response needs 200ms");
    assert_eq!(
        failure.error,
        JevError::DeadlineExceeded {
            deadline: Duration::from_millis(100),
        }
    );
    // Request spacing (300ms after the first start) already exhausted the remainder, so the
    // second request never reached the transport.
    assert_eq!(transport.post_count(), 1);
    assert!(failure.requests.is_empty());

    // A deadline that already passed fails before packing, with nothing attempted.
    let spent = evaluator
        .evaluate(
            request(vec![noul_question("c")])
                .with_deadline_at(Instant::now() - Duration::from_millis(1)),
        )
        .await
        .expect_err("spent deadline");
    assert_eq!(
        spent.error,
        JevError::DeadlineExceeded {
            deadline: Duration::ZERO,
        }
    );
    assert!(spent.requests.is_empty());
    assert_eq!(transport.post_count(), 1);
    assert_eq!(evaluator.available_permits(), 3);
}

#[tokio::test(start_paused = true)]
async fn cancellation_stops_the_call_and_the_evaluator_stays_usable() {
    let slow = Arc::new(AtomicBool::new(true));
    let transport = Arc::new(MockTransport::with_handler({
        let slow = Arc::clone(&slow);
        move |body| {
            let response = answer_body(body);
            if slow.load(Ordering::SeqCst) {
                response.after(Duration::from_secs(10))
            } else {
                response
            }
        }
    }));
    let evaluator = JevEvaluator::new(transport.clone(), EvaluatorConfig::default()).unwrap();

    let cancel = CancelToken::new();
    let cancelling = {
        let cancel = cancel.clone();
        async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel.cancel();
        }
    };
    let (result, ()) = tokio::join!(
        evaluator.evaluate(request(vec![noul_question("n")]).with_cancel(cancel.clone())),
        cancelling
    );
    let failure = result.expect_err("cancelled");
    assert_eq!(failure.error, JevError::Cancelled);
    assert_eq!(failure.timing.elapsed, Duration::from_millis(50));
    assert_eq!(failure.requests.len(), 1);
    assert_eq!(failure.requests[0].http_status, None);
    assert!(cancel.is_cancelled());
    assert_eq!(evaluator.available_permits(), 3);

    slow.store(false, Ordering::SeqCst);
    let outcome = evaluator
        .evaluate(request(vec![noul_question("n")]))
        .await
        .expect("the evaluator still serves requests");
    assert_eq!(outcome.noul(&id("n")).unwrap().noul, 0.9);
    assert_eq!(transport.post_count(), 2);

    // A token cancelled before the call fails before any dispatch.
    let pre_cancelled = CancelToken::new();
    pre_cancelled.cancel();
    let failure = evaluator
        .evaluate(request(vec![noul_question("n")]).with_cancel(pre_cancelled))
        .await
        .expect_err("pre-cancelled");
    assert_eq!(failure.error, JevError::Cancelled);
    assert!(failure.requests.is_empty());
    assert_eq!(transport.post_count(), 2);
}

#[tokio::test]
async fn cancel_token_resolves_for_early_and_late_cancels() {
    let token = CancelToken::new();
    token.cancel();
    token.cancelled().await;

    let token = CancelToken::new();
    let waiter = {
        let token = token.clone();
        async move {
            token.cancelled().await;
            true
        }
    };
    let ((), woke) = tokio::join!(
        async {
            tokio::task::yield_now().await;
            token.cancel();
        },
        waiter
    );
    assert!(woke);
}

#[tokio::test(start_paused = true)]
async fn request_spacing_and_in_flight_limits_bound_dispatch() {
    let config = EvaluatorConfig {
        request_spacing: Duration::from_millis(300),
        max_in_flight_requests: 3,
        ..single_question_batches()
    };
    let (evaluator, transport) = handler_evaluator(config, Duration::from_secs(1));
    let outcome = evaluator
        .evaluate(request(large_questions(5, 600)))
        .await
        .expect("five single-question batches");
    assert_eq!(outcome.answers.len(), 5);
    assert_eq!(
        outcome
            .requests
            .iter()
            .map(|identity| identity.batch_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    assert_eq!(
        outcome.usage,
        Usage {
            input_tokens: Some(500),
            output_tokens: Some(50),
            reported_responses: 5,
            unreported_responses: 0,
        }
    );

    let posts = transport.posts();
    let first = posts.iter().map(|post| post.started_at).min().unwrap();
    let mut starts: Vec<u64> = posts
        .iter()
        .map(|post| (post.started_at - first).as_millis() as u64)
        .collect();
    starts.sort_unstable();
    assert_eq!(starts, vec![0, 300, 600, 1_000, 1_300]);
    assert_eq!(
        posts.iter().map(|post| post.concurrent_posts).max(),
        Some(3)
    );
    assert_eq!(outcome.timing.elapsed, Duration::from_millis(2_300));
    assert_eq!(outcome.timing.http, Duration::from_secs(5));
    assert_eq!(outcome.timing.queue_wait, Duration::from_millis(3_200));
    assert_eq!(outcome.timing.request_count, 5);
}

#[tokio::test(start_paused = true)]
async fn request_spacing_is_shared_across_sequential_calls() {
    let (evaluator, transport) = handler_evaluator(EvaluatorConfig::default(), Duration::ZERO);
    evaluator
        .evaluate(request(vec![noul_question("a")]))
        .await
        .expect("first call");
    evaluator
        .evaluate(request(vec![noul_question("b")]))
        .await
        .expect("second call");
    let posts = transport.posts();
    assert_eq!(
        posts[1].started_at - posts[0].started_at,
        Duration::from_millis(300)
    );
}

#[tokio::test(start_paused = true)]
async fn a_failed_batch_stops_new_dispatch_but_keeps_completed_accounting() {
    // Batch 0 succeeds, batch 1 is rate limited, batch 2 must not be posted.
    let counter = Arc::new(AtomicUsize::new(0));
    let transport = Arc::new(MockTransport::with_handler({
        let counter = Arc::clone(&counter);
        move |body| {
            let position = counter.fetch_add(1, Ordering::SeqCst);
            if position == 1 {
                ScriptedResponse::status(429, "slow down")
            } else {
                answer_body(body)
            }
        }
    }));
    let config = EvaluatorConfig {
        max_in_flight_requests: 1,
        ..single_question_batches()
    };
    let evaluator = JevEvaluator::new(transport.clone(), config).unwrap();
    let failure = evaluator
        .evaluate(request(large_questions(3, 600)))
        .await
        .expect_err("the second batch fails");
    assert_eq!(failure.error, JevError::RateLimited { status: 429 });
    assert_eq!(transport.post_count(), 2);
    assert_eq!(failure.requests.len(), 2);
    assert_eq!(failure.requests[0].http_status, Some(200));
    assert_eq!(failure.requests[1].http_status, Some(429));
    assert_eq!(failure.usage, Usage::reported(100, 10));
    assert_eq!(evaluator.available_permits(), 1);
}

// ---------------------------------------------------------------------------------------
// Configuration, diagnostics and accounting
// ---------------------------------------------------------------------------------------

#[test]
fn evaluator_config_defaults_carry_the_initial_safety_policy() {
    let config = EvaluatorConfig::default();
    assert_eq!(config.model, "jev-1.13.0");
    assert_eq!(config.max_in_flight_requests, 3);
    assert_eq!(config.request_spacing, Duration::from_millis(300));
    assert_eq!(config.max_batch_bytes, 168_000);
    assert_eq!(config.deadline, Duration::from_millis(45_000));
    assert_eq!(config.max_estimated_tokens, 56_000);
    assert_eq!(
        config.max_estimated_state_plus_longest_question_tokens,
        28_000
    );
    assert!(config.validate().is_ok());
    assert_eq!(MAX_IN_FLIGHT_REQUESTS, 3);
    assert_eq!(MIN_REQUEST_SPACING, Duration::from_millis(300));
    assert_eq!(MAX_BATCH_BYTES, 168_000);
    assert_eq!(MAX_QUESTIONS_PER_REQUEST, 8_192);
    assert_eq!(MAX_BATCHES_PER_REQUEST, 128);
    assert_eq!(MAX_RESPONSE_BODY_BYTES, 4 * 1024 * 1024);

    let settings = HttpsSettings::default();
    assert_eq!(settings.pool_idle_timeout, Duration::from_millis(30_000));
    assert_eq!(settings.max_idle_connections, 3);
    assert_eq!(settings.connect_timeout, Duration::from_millis(10_000));
    assert!(settings.validate().is_ok());
    assert_eq!(ENDPOINT, "https://api.typesafe.ai/v1/systemone");
}

#[test]
fn evaluator_config_rejects_values_outside_the_initial_policy() {
    let base = EvaluatorConfig::default();
    let invalid = [
        EvaluatorConfig {
            model: " ".into(),
            ..base.clone()
        },
        EvaluatorConfig {
            max_in_flight_requests: 0,
            ..base.clone()
        },
        EvaluatorConfig {
            max_in_flight_requests: MAX_IN_FLIGHT_REQUESTS + 1,
            ..base.clone()
        },
        EvaluatorConfig {
            request_spacing: MIN_REQUEST_SPACING - Duration::from_millis(1),
            ..base.clone()
        },
        EvaluatorConfig {
            max_batch_bytes: 0,
            ..base.clone()
        },
        EvaluatorConfig {
            max_batch_bytes: MAX_BATCH_BYTES + 1,
            ..base.clone()
        },
        EvaluatorConfig {
            deadline: Duration::ZERO,
            ..base.clone()
        },
        EvaluatorConfig {
            deadline: Duration::MAX,
            ..base.clone()
        },
        EvaluatorConfig {
            deadline: MAX_SAFE_DURATION + Duration::from_millis(1),
            ..base.clone()
        },
        EvaluatorConfig {
            max_estimated_tokens: 0,
            ..base.clone()
        },
        EvaluatorConfig {
            max_estimated_state_plus_longest_question_tokens: 0,
            ..base.clone()
        },
    ];
    for config in invalid {
        let error = JevEvaluator::new(Arc::new(MockTransport::scripted(vec![])), config.clone())
            .err()
            .map(|error| error.kind());
        assert_eq!(error, Some("invalid_config"), "{config:?}");
    }
    // Tightening is allowed: one request in flight, a second between starts, one small batch.
    let tightened = EvaluatorConfig {
        max_in_flight_requests: 1,
        request_spacing: Duration::from_secs(1),
        max_batch_bytes: 1,
        ..base.clone()
    };
    assert!(tightened.validate().is_ok());
    for settings in [
        HttpsSettings {
            pool_idle_timeout: Duration::ZERO,
            ..HttpsSettings::default()
        },
        HttpsSettings {
            connect_timeout: Duration::MAX,
            ..HttpsSettings::default()
        },
        HttpsSettings {
            max_idle_connections: 0,
            ..HttpsSettings::default()
        },
    ] {
        assert!(matches!(
            HttpsTransport::new(SecretString::new("k"), settings.clone()),
            Err(JevError::InvalidConfig(_))
        ));
    }
}

#[tokio::test]
async fn debug_output_hides_evidence_and_credentials() {
    let request = request(vec![score_question("relevance", "SECRET EVIDENCE BODY")]);
    let debug = format!("{request:?}");
    assert!(debug.contains("relevance"));
    for hidden in ["SECRET EVIDENCE BODY", EVIDENCE_PATH, TASK_QUERY] {
        assert!(!debug.contains(hidden), "{debug}");
    }
    assert!(!format!("{:?}", request.questions()[0]).contains("SECRET"));

    let secret = SecretString::new("sk-test-not-a-real-key");
    assert_eq!(format!("{secret:?}"), "SecretString([REDACTED])");
    let transport = HttpsTransport::new(secret.clone(), HttpsSettings::default())
        .expect("the client builds without network access");
    assert!(!format!("{transport:?}").contains("sk-test"));
    assert_eq!(transport.endpoint(), ENDPOINT);
    let evaluator =
        JevEvaluator::https(secret, EvaluatorConfig::default(), HttpsSettings::default())
            .expect("the evaluator builds without network access");
    assert!(!format!("{evaluator:?}").contains("sk-test"));
    assert!(matches!(
        HttpsTransport::new(SecretString::new("  "), HttpsSettings::default()),
        Err(JevError::InvalidConfig(_))
    ));
}

#[test]
fn usage_sums_keep_unknown_totals_unknown() {
    assert_eq!(
        Usage::reported(1, 1) + Usage::reported(2, 2),
        Usage {
            input_tokens: Some(3),
            output_tokens: Some(3),
            reported_responses: 2,
            unreported_responses: 0,
        }
    );
    assert_eq!(
        Usage::reported(10, 2) + Usage::unreported(),
        Usage {
            input_tokens: None,
            output_tokens: None,
            reported_responses: 1,
            unreported_responses: 1,
        }
    );
    assert_eq!(
        Usage::default() + Usage::reported(5, 1),
        Usage::reported(5, 1)
    );
    assert_eq!(
        Usage::reported(5, 1) + Usage::default(),
        Usage::reported(5, 1)
    );
    assert!(Usage::default().is_empty());
}

#[test]
fn error_kinds_are_stable_secret_free_labels() {
    assert_eq!(JevError::InvalidState("x".into()).kind(), "invalid_state");
    assert_eq!(
        JevError::TooManyQuestions { count: 1, limit: 0 }.kind(),
        "too_many_questions"
    );
    assert_eq!(
        JevError::DeadlineExceeded {
            deadline: Duration::from_secs(1)
        }
        .kind(),
        "deadline_exceeded"
    );
    assert_eq!(
        JevError::Unauthorized { status: 401 }.kind(),
        "unauthorized"
    );
    assert_eq!(
        JevError::Unauthorized { status: 401 }.to_string(),
        "provider rejected the credentials (HTTP 401)"
    );
    assert_eq!(
        EvaluationFailure::from(JevError::Cancelled).to_string(),
        "evaluation cancelled by the caller"
    );
}

// ---------------------------------------------------------------------------------------
// Mock evaluator
// ---------------------------------------------------------------------------------------

#[tokio::test]
async fn mock_evaluator_records_requests_and_reports_incomplete_answers() {
    let evaluator = MockEvaluator::new(|request| {
        let first = request.questions()[0].id().clone();
        Ok(BTreeMap::from([(first, answers::noul(0.25))]))
    })
    .with_usage(Usage::reported(7, 1));
    let outcome = evaluator
        .evaluate(request(vec![noul_question("only")]))
        .await
        .expect("single answer");
    assert_eq!(outcome.noul(&id("only")).unwrap().noul, 0.25);
    assert_eq!(outcome.usage, Usage::reported(7, 1));
    assert_eq!(
        outcome.raw_answers[&id("only")],
        json!({ "type": "noul", "noul": 0.25 })
    );
    let recorded = evaluator.requests();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].task_query(), Some(TASK_QUERY));
    assert_eq!(recorded[0].state[TASK_QUERY_FIELD], TASK_QUERY);
    assert_eq!(recorded[0].questions[&id("only")]["type"], "noul");
    assert!(recorded[0].deadline_at.is_none());

    let failure = evaluator
        .evaluate(request(vec![
            noul_question("first"),
            noul_question("second"),
        ]))
        .await
        .expect_err("second question unanswered");
    assert_eq!(
        failure.error,
        JevError::IncompleteAnswers {
            batch_index: 0,
            missing: vec![id("second")],
            unexpected: vec![],
        }
    );
    assert_eq!(evaluator.request_count(), 2);

    let failing = MockEvaluator::failing(JevError::RateLimited { status: 429 });
    assert_eq!(
        failing
            .evaluate(request(vec![noul_question("n")]))
            .await
            .unwrap_err()
            .error
            .kind(),
        "rate_limited"
    );
    let cancelled = CancelToken::new();
    cancelled.cancel();
    assert_eq!(
        failing
            .evaluate(request(vec![noul_question("n")]).with_cancel(cancelled))
            .await
            .unwrap_err()
            .error,
        JevError::Cancelled
    );
}

#[tokio::test]
async fn mock_gate_holds_an_evaluation_until_released() {
    let gate = Gate::new();
    let evaluator = Arc::new(
        MockEvaluator::new(|request| {
            Ok(request
                .questions()
                .iter()
                .map(|question| {
                    (
                        question.id().clone(),
                        answers::choice("keep", &[("keep", 0.8), ("omit", 0.2)]),
                    )
                })
                .collect())
        })
        .with_gate(Arc::clone(&gate)),
    );
    let running = tokio::spawn({
        let evaluator = Arc::clone(&evaluator);
        async move {
            evaluator
                .evaluate(request(vec![choice_question("body")]))
                .await
        }
    });
    gate.entered().await;
    assert_eq!(evaluator.request_count(), 1);
    assert!(!running.is_finished());
    gate.release();
    let outcome = running
        .await
        .expect("task joins")
        .expect("answers after release");
    assert_eq!(outcome.choice(&id("body")).unwrap().choice, "keep");
    assert!((outcome.choice(&id("body")).unwrap().probability("omit") - 0.2).abs() < 1e-9);
}

// ---------------------------------------------------------------------------------------
// Real transport policy against a local listener (no external endpoint, no TLS)
// ---------------------------------------------------------------------------------------

/// One request the local listener saw: the raw header block, the body bytes and which
/// accepted connection carried it.
#[derive(Clone, Debug)]
struct SeenRequest {
    headers: String,
    body: Vec<u8>,
    connection: usize,
}

/// What the listener answers for one request.
#[derive(Clone)]
struct LocalReply {
    status: u16,
    body: Vec<u8>,
    close_after: bool,
    extra_headers: Vec<String>,
}

fn ok_reply(body: Vec<u8>) -> LocalReply {
    LocalReply {
        status: 200,
        body,
        close_after: false,
        extra_headers: Vec::new(),
    }
}

struct LocalServer {
    url: String,
    seen: Arc<Mutex<Vec<SeenRequest>>>,
    connections: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl LocalServer {
    fn seen(&self) -> Vec<SeenRequest> {
        self.seen.lock().unwrap().clone()
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// A minimal HTTP/1.1 server on 127.0.0.1 that keeps a connection open across requests
/// unless a reply says otherwise, so pooling, redirect and retry behavior of the real
/// client can be observed on real sockets. `reply` receives the request index (across all
/// connections) and the request body.
async fn local_server<F>(reply: F) -> LocalServer
where
    F: Fn(usize, &[u8]) -> LocalReply + Send + Sync + 'static,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind a local listener");
    let url = format!("http://{}/v1/systemone", listener.local_addr().unwrap());
    let seen: Arc<Mutex<Vec<SeenRequest>>> = Arc::new(Mutex::new(Vec::new()));
    let connections = Arc::new(AtomicUsize::new(0));
    let reply = Arc::new(reply);
    let task = tokio::spawn({
        let seen = Arc::clone(&seen);
        let connections = Arc::clone(&connections);
        async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let connection = connections.fetch_add(1, Ordering::SeqCst);
                let seen = Arc::clone(&seen);
                let reply = Arc::clone(&reply);
                tokio::spawn(async move {
                    let mut buffer: Vec<u8> = Vec::new();
                    loop {
                        // One request: headers up to the blank line, then `Content-Length` bytes.
                        let header_end = loop {
                            if let Some(position) =
                                buffer.windows(4).position(|window| window == b"\r\n\r\n")
                            {
                                break position + 4;
                            }
                            let mut chunk = [0u8; 4096];
                            match socket.read(&mut chunk).await {
                                Ok(0) | Err(_) => return,
                                Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                            }
                        };
                        let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
                        let content_length = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().ok())
                                    .flatten()
                            })
                            .unwrap_or(0);
                        while buffer.len() < header_end + content_length {
                            let mut chunk = [0u8; 4096];
                            match socket.read(&mut chunk).await {
                                Ok(0) | Err(_) => return,
                                Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                            }
                        }
                        let body = buffer[header_end..header_end + content_length].to_vec();
                        buffer.drain(..header_end + content_length);
                        let index = {
                            let mut seen = seen.lock().unwrap();
                            seen.push(SeenRequest {
                                headers,
                                body: body.clone(),
                                connection,
                            });
                            seen.len() - 1
                        };
                        let reply = reply(index, &body);
                        let mut response = format!(
                            "HTTP/1.1 {} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
                            reply.status,
                            reply.body.len()
                        );
                        for header in &reply.extra_headers {
                            response.push_str(header);
                            response.push_str("\r\n");
                        }
                        if reply.close_after {
                            response.push_str("Connection: close\r\n");
                        }
                        response.push_str("\r\n");
                        let mut bytes = response.into_bytes();
                        bytes.extend_from_slice(&reply.body);
                        if socket.write_all(&bytes).await.is_err() {
                            return;
                        }
                        let _ = socket.flush().await;
                        if reply.close_after {
                            let _ = socket.shutdown().await;
                            return;
                        }
                    }
                });
            }
        }
    });
    LocalServer {
        url,
        seen,
        connections,
        task,
    }
}

fn local_evaluator(server: &LocalServer, settings: HttpsSettings) -> JevEvaluator {
    let transport = HttpsTransport::with_endpoint_for_tests(
        SecretString::new("local-test-key"),
        settings,
        server.url.clone(),
    )
    .expect("client builds");
    JevEvaluator::new(Arc::new(transport), EvaluatorConfig::default()).expect("valid config")
}

fn answer_for_wire(body: &[u8]) -> Vec<u8> {
    let decoded: Value = serde_json::from_slice(body).expect("request body is JSON");
    answer_bytes(&decoded)
}

#[tokio::test]
async fn https_client_reuses_one_idle_connection_and_sends_the_documented_headers() {
    let server = local_server(|_, body| ok_reply(answer_for_wire(body))).await;
    let evaluator = local_evaluator(&server, HttpsSettings::default());

    let first = evaluator
        .evaluate(request(vec![noul_question("a")]))
        .await
        .expect("first request over the local listener");
    let second = evaluator
        .evaluate(request(vec![noul_question("b")]))
        .await
        .expect("second request over the local listener");
    assert_eq!(first.noul(&id("a")).unwrap().noul, 0.9);
    assert_eq!(second.noul(&id("b")).unwrap().noul, 0.9);
    assert_eq!(first.requests[0].http_status, Some(200));
    assert_eq!(first.usage, Usage::reported(100, 10));

    let seen = server.seen();
    assert_eq!(seen.len(), 2, "exactly one POST per request");
    assert_eq!(
        server.connections.load(Ordering::SeqCst),
        1,
        "the idle connection was reused for the second request"
    );
    assert_eq!(seen[0].connection, seen[1].connection);
    let headers = seen[0].headers.to_ascii_lowercase();
    assert!(
        headers.starts_with("post /v1/systemone http/1.1\r\n"),
        "{headers}"
    );
    assert!(
        headers.contains("authorization: bearer local-test-key"),
        "{headers}"
    );
    assert!(
        headers.contains("content-type: application/json"),
        "{headers}"
    );
    assert!(headers.contains("user-agent: codemap-search/"), "{headers}");
    let posted: Value = serde_json::from_slice(&seen[0].body).unwrap();
    assert_eq!(posted["state"][TASK_QUERY_FIELD], TASK_QUERY);
    assert_eq!(seen[0].body.len(), first.requests[0].request_bytes);
    assert_eq!(
        first.requests[0].request_sha256,
        hex(Sha256::digest(&seen[0].body))
    );
}

#[tokio::test]
async fn https_client_replaces_a_connection_idle_beyond_the_configured_timeout() {
    // The configured idle timeout reaches the real pool: after idling longer than it, the
    // next request opens a fresh connection instead of reusing the stale one.
    let server = local_server(|_, body| ok_reply(answer_for_wire(body))).await;
    let evaluator = local_evaluator(
        &server,
        HttpsSettings {
            pool_idle_timeout: Duration::from_millis(100),
            ..HttpsSettings::default()
        },
    );
    evaluator
        .evaluate(request(vec![noul_question("a")]))
        .await
        .expect("first request");
    tokio::time::sleep(Duration::from_millis(400)).await;
    evaluator
        .evaluate(request(vec![noul_question("b")]))
        .await
        .expect("second request after the idle timeout");
    let seen = server.seen();
    assert_eq!(seen.len(), 2);
    assert_ne!(
        seen[0].connection, seen[1].connection,
        "the idle connection was not reused"
    );
    assert_eq!(server.connections.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn https_client_never_retries_or_follows_redirects() {
    // 429: one request, no second attempt.
    let server = local_server(|_, _| LocalReply {
        status: 429,
        body: b"slow down".to_vec(),
        close_after: false,
        extra_headers: Vec::new(),
    })
    .await;
    let evaluator = local_evaluator(&server, HttpsSettings::default());
    let failure = evaluator
        .evaluate(request(vec![noul_question("n")]))
        .await
        .expect_err("rate limited");
    assert_eq!(failure.error, JevError::RateLimited { status: 429 });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.seen().len(), 1, "a 429 must not be retried");
    drop(server);

    // 302 with a Location: the client reports the status instead of following it, so the
    // credential and the evidence are never re-sent elsewhere.
    let server = local_server(|_, _| LocalReply {
        status: 302,
        body: Vec::new(),
        close_after: false,
        extra_headers: vec!["Location: http://127.0.0.1:9/elsewhere".into()],
    })
    .await;
    let evaluator = local_evaluator(&server, HttpsSettings::default());
    let failure = evaluator
        .evaluate(request(vec![noul_question("n")]))
        .await
        .expect_err("redirect is not followed");
    assert_eq!(
        failure.error,
        JevError::Http {
            status: 302,
            excerpt: String::new()
        }
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.seen().len(), 1);
    drop(server);

    // The server closes the connection after each reply: the next request must open a fresh
    // connection and be counted exactly once (no phantom re-send on the closed one).
    let server = local_server(|_, body| LocalReply {
        status: 200,
        body: answer_for_wire(body),
        close_after: true,
        extra_headers: Vec::new(),
    })
    .await;
    let evaluator = local_evaluator(&server, HttpsSettings::default());
    evaluator
        .evaluate(request(vec![noul_question("a")]))
        .await
        .expect("first request");
    evaluator
        .evaluate(request(vec![noul_question("b")]))
        .await
        .expect("second request on a fresh connection");
    let seen = server.seen();
    assert_eq!(seen.len(), 2);
    assert_ne!(seen[0].connection, seen[1].connection);
    assert_eq!(server.connections.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn https_client_abandons_bodies_above_the_limit() {
    let server = local_server(|_, _| ok_reply(vec![b' '; MAX_RESPONSE_BODY_BYTES + 1])).await;
    let evaluator = local_evaluator(&server, HttpsSettings::default());
    let failure = evaluator
        .evaluate(request(vec![noul_question("n")]))
        .await
        .expect_err("oversized body");
    assert_eq!(
        failure.error,
        JevError::Transport(TransportError::ResponseTooLarge {
            limit: MAX_RESPONSE_BODY_BYTES
        })
    );
    assert_eq!(failure.requests.len(), 1);
    assert_eq!(server.seen().len(), 1);
}
