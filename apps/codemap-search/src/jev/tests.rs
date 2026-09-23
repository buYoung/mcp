use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Map, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::Instant;

use super::answer::parse_answer;
use super::*;

type Reply = Box<dyn Fn(usize, &Value) -> (u16, Vec<u8>) + Send + Sync>;

/// Offline transport: records every body and answers from a script after a delay.
struct ScriptedTransport {
    reply: Reply,
    delay: Duration,
    calls: AtomicUsize,
    in_flight: Arc<AtomicUsize>,
    max_in_flight: AtomicUsize,
    starts: Mutex<Vec<Instant>>,
    bodies: Mutex<Vec<Value>>,
}

struct InFlightGuard(Arc<AtomicUsize>);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl ScriptedTransport {
    fn new(reply: impl Fn(usize, &Value) -> (u16, Vec<u8>) + Send + Sync + 'static) -> Self {
        Self {
            reply: Box::new(reply),
            delay: Duration::ZERO,
            calls: AtomicUsize::new(0),
            in_flight: Arc::new(AtomicUsize::new(0)),
            max_in_flight: AtomicUsize::new(0),
            starts: Mutex::new(Vec::new()),
            bodies: Mutex::new(Vec::new()),
        }
    }

    fn valid() -> Self {
        Self::new(|_, request| (200, encode(&valid_response(request))))
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl JevTransport for ScriptedTransport {
    fn post(&self, body: Vec<u8>) -> BoxFuture<'_, Result<TransportResponse, TransportError>> {
        Box::pin(async move {
            let call_index = self.calls.fetch_add(1, Ordering::SeqCst);
            self.starts.lock().unwrap().push(Instant::now());
            let current = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_in_flight.fetch_max(current, Ordering::SeqCst);
            let _guard = InFlightGuard(self.in_flight.clone());
            tokio::time::sleep(self.delay).await;
            let request: Value = serde_json::from_slice(&body).expect("request bodies are JSON");
            let (status, body) = (self.reply)(call_index, &request);
            self.bodies.lock().unwrap().push(request);
            Ok(TransportResponse { status, body })
        })
    }
}

fn encode(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

/// A valid answer for every question in a request body.
fn valid_answers(request: &Value) -> Map<String, Value> {
    let mut answers = Map::new();
    for (id, question) in request["questions"].as_object().unwrap() {
        let answer = match question["type"].as_str().unwrap() {
            "score" => {
                let level_count = question["criteria"].as_array().unwrap().len();
                let probabilities: Map<String, Value> = (0..level_count)
                    .map(|level| {
                        (
                            level.to_string(),
                            json!(if level == level_count - 1 { 1.0 } else { 0.0 }),
                        )
                    })
                    .collect();
                json!({"type": "score", "score": (level_count - 1) as f64,
                       "probabilities": probabilities, "confidence": 1.0, "legend": {}})
            }
            "choice" => {
                let options: Vec<String> = question["criteria"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .cloned()
                    .collect();
                let probabilities: Map<String, Value> = options
                    .iter()
                    .map(|option| {
                        (
                            option.clone(),
                            json!(if *option == options[0] { 1.0 } else { 0.0 }),
                        )
                    })
                    .collect();
                json!({"type": "choice", "choice": options[0], "probabilities": probabilities, "confidence": 1.0})
            }
            "noul" => json!({"type": "noul", "noul": 0.25}),
            other => panic!("unexpected question type {other}"),
        };
        answers.insert(id.clone(), answer);
    }
    answers
}

fn valid_response(request: &Value) -> Value {
    json!({"model": DEFAULT_MODEL, "answers": valid_answers(request),
           "usage": {"input_tokens": 100, "output_tokens": 10}})
}

fn score_levels() -> Vec<Value> {
    vec![
        json!("No useful evidence."),
        json!("Tangential background."),
        json!("Important supporting evidence."),
        json!("Directly implements the requested behavior."),
    ]
}

fn mixed_questions() -> Vec<Question> {
    vec![
        Question::score(
            "fragment-1",
            json!({"question": "How useful is `fragment` for `task_query`?", "fragment": "fn retry() {}"}),
            score_levels(),
        )
        .unwrap(),
        Question::choice(
            "role-1",
            json!("Which role does the declaration in `task_query` play?"),
            vec![
                ChoiceOption::new("entry", json!("Receives the inputs.")),
                ChoiceOption::new("unrelated", Value::Null),
            ],
        )
        .unwrap(),
        Question::noul(
            "body-1",
            json!({"question": "Is `declaration` unrelated to `task_query`?", "declaration": {"name": "retry"}}),
            Some(NoulCriteria::new(Some(json!("Unrelated.")), Some(json!("Related or uncertain.")))),
        )
        .unwrap(),
    ]
}

fn state() -> Value {
    json!({"task_query": "Where is the retry loop implemented?"})
}

/// Questions whose encoded instructions are about `instruction_bytes` long.
fn sized_questions(count: usize, instruction_bytes: usize) -> Vec<Question> {
    (0..count)
        .map(|index| {
            Question::noul(
                format!("q{index:03}"),
                json!({"question": "Is `evidence` unrelated to `task_query`?", "evidence": "x".repeat(instruction_bytes)}),
                None,
            )
            .unwrap()
        })
        .collect()
}

fn policy() -> TransportPolicy {
    TransportPolicy {
        request_spacing: Duration::ZERO,
        ..TransportPolicy::default()
    }
}

async fn evaluate_with(
    transport: ScriptedTransport,
    policy: TransportPolicy,
    request: EvaluationRequest,
) -> (
    Result<Evaluation, EvaluationFailure>,
    Evaluator<ScriptedTransport>,
) {
    let evaluator = Evaluator::new(transport, policy).unwrap();
    let result = evaluator.evaluate(request).await;
    (result, evaluator)
}

#[tokio::test]
async fn test_jev_mixed_score_choice_noul_questions_return_typed_answers() {
    let (result, evaluator) = evaluate_with(
        ScriptedTransport::valid(),
        policy(),
        EvaluationRequest::new(state(), mixed_questions()),
    )
    .await;
    let evaluation = result.unwrap();
    assert_eq!(evaluation.model, DEFAULT_MODEL);
    assert_eq!(evaluation.answers.len(), 3);
    let score = evaluation.score("fragment-1").unwrap();
    assert_eq!(score.probabilities, vec![0.0, 0.0, 0.0, 1.0]);
    assert_eq!(score.score, 3.0);
    let choice = evaluation.choice("role-1").unwrap();
    assert_eq!(choice.choice, "entry");
    assert_eq!(choice.probability("unrelated"), 0.0);
    assert_eq!(evaluation.noul("body-1").unwrap().noul, 0.25);
    assert_eq!(
        evaluation.usage,
        Usage {
            input_tokens: 100,
            output_tokens: 10
        }
    );
    let record = &evaluation.requests[0];
    assert_eq!(evaluation.requests.len(), 1);
    assert_eq!(record.outcome, RequestOutcome::Succeeded);
    assert_eq!(record.http_status, Some(200));
    assert_eq!(record.question_count, 3);
    assert_eq!(record.body_sha256.len(), 64);

    let bodies = evaluator.transport().bodies.lock().unwrap();
    let body = &bodies[0];
    assert_eq!(body["model"], json!(DEFAULT_MODEL));
    assert_eq!(body["state"], state());
    assert_eq!(body["questions"]["fragment-1"]["type"], json!("score"));
    assert_eq!(
        body["questions"]["fragment-1"]["criteria"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        body["questions"]["role-1"]["criteria"]["unrelated"],
        Value::Null
    );
    assert_eq!(
        body["questions"]["body-1"]["criteria"]["true"],
        json!("Unrelated.")
    );
    assert_eq!(
        body["questions"]["body-1"]["criteria"]["false"],
        json!("Related or uncertain.")
    );
    assert_eq!(record.body_bytes, encode(body).len());
}

#[test]
fn test_jev_question_constructors_enforce_documented_limits() {
    let levels = |count: usize| {
        (0..count)
            .map(|level| json!(format!("level {level}")))
            .collect::<Vec<_>>()
    };
    assert!(Question::score("s", json!("q"), levels(1)).is_err());
    assert!(Question::score("s", json!("q"), levels(2)).is_ok());
    assert!(Question::score("s", json!("q"), levels(10)).is_ok());
    assert!(Question::score("s", json!("q"), levels(11)).is_err());
    assert!(Question::score("s", json!("q"), vec![json!("a"), json!("")]).is_err());
    assert!(Question::score("s", json!("q"), vec![json!({"level": "a"}), json!(["b"])]).is_ok());

    let options = |count: usize| {
        (0..count)
            .map(|index| ChoiceOption::new(format!("option-{index}"), Value::Null))
            .collect::<Vec<_>>()
    };
    assert!(Question::choice("c", json!("q"), options(1)).is_err());
    assert!(Question::choice("c", json!("q"), options(255)).is_ok());
    assert!(Question::choice("c", json!("q"), options(256)).is_err());
    let duplicated = vec![
        ChoiceOption::new("keep", Value::Null),
        ChoiceOption::new("keep", Value::Null),
    ];
    assert!(Question::choice("c", json!("q"), duplicated).is_err());
    let blank_description = vec![
        ChoiceOption::new("keep", json!(" ")),
        ChoiceOption::new("omit", Value::Null),
    ];
    assert!(Question::choice("c", json!("q"), blank_description).is_err());

    assert!(Question::noul("n", json!("q"), None).is_ok());
    assert!(Question::noul("n", json!({"question": "q", "evidence": [1]}), None).is_ok());
    assert!(Question::noul("n", json!(["q"]), None).is_ok());
    for instructions in [json!(""), json!({}), json!([]), json!(3), Value::Null] {
        assert!(Question::noul("n", instructions, None).is_err());
    }
    assert!(Question::noul(
        "n",
        json!("q"),
        Some(NoulCriteria::new(Some(json!("")), None))
    )
    .is_err());
    for id in [
        "",
        "has space",
        "path/like",
        &"x".repeat(MAX_QUESTION_ID_BYTES + 1),
    ] {
        assert!(
            Question::noul(id, json!("q"), None).is_err(),
            "{id:?} must be rejected"
        );
    }
    assert!(Question::noul("file-0001:part.2_a", json!("q"), None).is_ok());
}

#[test]
fn test_jev_answer_validation_accepts_raw_values_and_rejects_malformed_answers() {
    let score = Question::score("s", json!("q"), score_levels()).unwrap();
    let choice = Question::choice(
        "c",
        json!("q"),
        vec![
            ChoiceOption::new("keep", Value::Null),
            ChoiceOption::new("omit", Value::Null),
        ],
    )
    .unwrap();
    let noul = Question::noul("n", json!("q"), None).unwrap();
    let valid_score = json!({"type": "score", "score": 2.0, "confidence": 0.8,
        "probabilities": {"0": 0.0, "1": 0.05, "2": 0.9, "3": 0.05}});
    let accepted = [
        (&score, valid_score.clone()),
        (
            &score,
            json!({"type": "score", "score": 2.0, "confidence": 0.8,
            "probabilities": {"0": 0.0, "1": 0.05, "2": 0.89, "3": 0.05}}),
        ),
        (
            &choice,
            json!({"type": "choice", "choice": "keep", "confidence": 0.6,
            "probabilities": {"keep": 0.8, "omit": 0.2}}),
        ),
        (
            &choice,
            json!({"type": "choice", "choice": "omit", "confidence": 0.0,
            "probabilities": {"keep": 0.5, "omit": 0.5}}),
        ),
        (&noul, json!({"type": "noul", "noul": 0.9})),
        (&noul, json!({"type": "noul", "noul": 1, "confidence": 0.3})),
    ];
    for (question, answer) in accepted {
        assert!(
            parse_answer(question, &answer).is_ok(),
            "{answer} should be accepted"
        );
    }
    let parsed = parse_answer(&score, &valid_score).unwrap();
    assert_eq!(
        parsed,
        Answer::Score(ScoreAnswer {
            score: 2.0,
            probabilities: vec![0.0, 0.05, 0.9, 0.05],
            confidence: 0.8
        })
    );

    let rejected = [
        (&score, json!("score")),
        (
            &score,
            json!({"type": "choice", "score": 2.0, "confidence": 0.8,
            "probabilities": {"0": 0.0, "1": 0.05, "2": 0.9, "3": 0.05}}),
        ),
        (
            &score,
            json!({"type": "score", "score": 2.0, "confidence": 0.8,
            "probabilities": {"0": 0.0, "1": 0.05, "2": 0.9}}),
        ),
        (
            &score,
            json!({"type": "score", "score": 2.0, "confidence": 0.8,
            "probabilities": {"0": 0.0, "1": 0.05, "2": 0.9, "3": 0.05, "4": 0.0}}),
        ),
        (
            &score,
            json!({"type": "score", "score": 2.0, "confidence": 0.8,
            "probabilities": {"0": 0.0, "1": 0.05, "2": 0.85, "3": 0.05}}),
        ),
        (
            &score,
            json!({"type": "score", "score": 3.5, "confidence": 0.8,
            "probabilities": {"0": 0.0, "1": 0.05, "2": 0.9, "3": 0.05}}),
        ),
        (
            &score,
            json!({"type": "score", "score": 2.0, "confidence": 0.8,
            "probabilities": {"0": -0.1, "1": 0.15, "2": 0.9, "3": 0.05}}),
        ),
        (
            &score,
            json!({"type": "score", "score": 2.0,
            "probabilities": {"0": 0.0, "1": 0.05, "2": 0.9, "3": 0.05}}),
        ),
        (
            &score,
            json!({"type": "score", "score": 2.0, "confidence": 0.8,
            "probabilities": {"0": "0", "1": 0.05, "2": 0.9, "3": 0.05}}),
        ),
        (
            &choice,
            json!({"type": "choice", "choice": "maybe", "confidence": 0.6,
            "probabilities": {"keep": 0.8, "omit": 0.2}}),
        ),
        (
            &choice,
            json!({"type": "choice", "choice": "keep", "confidence": 0.6,
            "probabilities": {"keep": 0.3, "omit": 0.7}}),
        ),
        (
            &choice,
            json!({"type": "choice", "choice": "keep", "confidence": 0.6,
            "probabilities": {"keep": 1.0}}),
        ),
        (
            &choice,
            json!({"type": "choice", "confidence": 0.6,
            "probabilities": {"keep": 0.8, "omit": 0.2}}),
        ),
        (&noul, json!({"type": "noul", "noul": 1.2})),
        (&noul, json!({"type": "noul"})),
        (&noul, json!({"type": "noul", "noul": "0.5"})),
    ];
    for (question, answer) in rejected {
        let error = parse_answer(question, &answer).unwrap_err();
        assert!(
            matches!(error, JevError::InvalidAnswer { .. }),
            "{answer} gave {error:?}"
        );
    }
}

#[tokio::test]
async fn test_jev_incomplete_or_foreign_answer_sets_fail_with_partial_usage() {
    let missing = ScriptedTransport::new(|_, request| {
        let mut response = valid_response(request);
        response["answers"]
            .as_object_mut()
            .unwrap()
            .remove("role-1");
        (200, encode(&response))
    });
    let (result, _) = evaluate_with(
        missing,
        policy(),
        EvaluationRequest::new(state(), mixed_questions()),
    )
    .await;
    let failure = result.unwrap_err();
    assert_eq!(
        failure.error,
        JevError::AnswerSetMismatch {
            missing_count: 1,
            unexpected_count: 0
        }
    );
    assert_eq!(
        failure.usage,
        Usage {
            input_tokens: 100,
            output_tokens: 10
        }
    );
    assert_eq!(failure.requests[0].outcome, RequestOutcome::Failed);

    let unexpected = ScriptedTransport::new(|_, request| {
        let mut response = valid_response(request);
        response["answers"]["other"] = json!({"type": "noul", "noul": 0.5});
        (200, encode(&response))
    });
    let (result, _) = evaluate_with(
        unexpected,
        policy(),
        EvaluationRequest::new(state(), mixed_questions()),
    )
    .await;
    assert_eq!(
        result.unwrap_err().error,
        JevError::AnswerSetMismatch {
            missing_count: 0,
            unexpected_count: 1
        }
    );

    let other_model = ScriptedTransport::new(|_, request| {
        let mut response = valid_response(request);
        response["model"] = json!("jev-1.14.0");
        (200, encode(&response))
    });
    let (result, _) = evaluate_with(
        other_model,
        policy(),
        EvaluationRequest::new(state(), mixed_questions()),
    )
    .await;
    assert!(matches!(
        result.unwrap_err().error,
        JevError::ModelMismatch { .. }
    ));

    let not_json = ScriptedTransport::new(|_, _| (200, b"<html>busy</html>".to_vec()));
    let (result, _) = evaluate_with(
        not_json,
        policy(),
        EvaluationRequest::new(state(), mixed_questions()),
    )
    .await;
    let failure = result.unwrap_err();
    assert!(matches!(failure.error, JevError::MalformedResponse { .. }));
    assert_eq!(failure.usage, Usage::default());
}

#[tokio::test]
async fn test_jev_requests_are_validated_before_dispatch() {
    let cases = [
        EvaluationRequest::new(json!(""), mixed_questions()),
        EvaluationRequest::new(json!({}), mixed_questions()),
        EvaluationRequest::new(json!(42), mixed_questions()),
        EvaluationRequest::new(state(), Vec::new()),
        EvaluationRequest::new(state(), [mixed_questions(), mixed_questions()].concat()),
    ];
    for request in cases {
        let (result, evaluator) =
            evaluate_with(ScriptedTransport::valid(), policy(), request).await;
        assert!(matches!(
            result.unwrap_err().error,
            JevError::InvalidRequest { .. }
        ));
        assert_eq!(evaluator.transport().call_count(), 0);
    }
}

#[tokio::test]
async fn test_jev_packing_splits_by_bytes_without_losing_questions() {
    let policy = TransportPolicy {
        max_batch_bytes: MIN_BATCH_BYTES,
        ..policy()
    };
    let questions = sized_questions(40, 500);
    let (result, evaluator) = evaluate_with(
        ScriptedTransport::valid(),
        policy,
        EvaluationRequest::new(state(), questions),
    )
    .await;
    let evaluation = result.unwrap();
    assert_eq!(evaluation.answers.len(), 40);
    assert!(evaluation.requests.len() > 1);
    assert_eq!(
        evaluation
            .requests
            .iter()
            .map(|record| record.question_count)
            .sum::<usize>(),
        40
    );
    for record in &evaluation.requests {
        assert!(
            record.body_bytes <= MIN_BATCH_BYTES,
            "{} bytes",
            record.body_bytes
        );
    }
    let bodies = evaluator.transport().bodies.lock().unwrap();
    for body in bodies.iter() {
        assert_eq!(body["state"], state());
        assert!(encode(body).len() <= MIN_BATCH_BYTES);
    }
    let answered: usize = bodies
        .iter()
        .map(|body| body["questions"].as_object().unwrap().len())
        .sum();
    assert_eq!(answered, 40);
}

#[tokio::test]
async fn test_jev_oversized_question_fails_before_dispatch() {
    let policy = TransportPolicy {
        max_batch_bytes: MIN_BATCH_BYTES,
        ..policy()
    };
    let mut questions = sized_questions(3, 100);
    questions.push(
        Question::noul(
            "too-large",
            json!({"question": "q", "evidence": "y".repeat(MIN_BATCH_BYTES)}),
            None,
        )
        .unwrap(),
    );
    let (result, evaluator) = evaluate_with(
        ScriptedTransport::valid(),
        policy,
        EvaluationRequest::new(state(), questions),
    )
    .await;
    let failure = result.unwrap_err();
    assert!(matches!(
        failure.error,
        JevError::RequestTooLarge { ref question_id, max_batch_bytes: MIN_BATCH_BYTES, .. } if question_id == "too-large"
    ));
    assert_eq!(evaluator.transport().call_count(), 0);
    assert!(failure.requests.is_empty());
}

#[tokio::test]
async fn test_jev_token_budgets_split_or_fail_before_dispatch() {
    let wide_policy = TransportPolicy {
        max_batch_bytes: MAX_BATCH_BYTES_LIMIT,
        ..policy()
    };
    // State plus one question estimated above 32k tokens can never be sent.
    let (result, evaluator) = evaluate_with(
        ScriptedTransport::valid(),
        wide_policy.clone(),
        EvaluationRequest::new(state(), sized_questions(1, 70_000)),
    )
    .await;
    let failure = result.unwrap_err();
    assert!(matches!(
        failure.error,
        JevError::TokenBudgetExceeded {
            budget: TokenBudget::StateWithLongestQuestion,
            limit_tokens: STATE_WITH_LONGEST_QUESTION_TOKEN_LIMIT,
            ..
        }
    ));
    assert_eq!(evaluator.transport().call_count(), 0);

    // Questions that fit individually are split so each body stays under the 64k estimate
    // even when the byte ceiling would allow more.
    let (result, evaluator) = evaluate_with(
        ScriptedTransport::valid(),
        wide_policy,
        EvaluationRequest::new(state(), sized_questions(10, 30_000)),
    )
    .await;
    let evaluation = result.unwrap();
    assert_eq!(evaluation.answers.len(), 10);
    assert_eq!(evaluation.requests.len(), 3);
    for record in &evaluation.requests {
        assert!(record.estimated_tokens <= TOTAL_CONTEXT_TOKEN_LIMIT);
        assert!(record.body_bytes > DEFAULT_MAX_BATCH_BYTES || record.question_count < 4);
    }
    assert_eq!(evaluator.transport().call_count(), 3);
}

#[test]
fn test_jev_token_estimate_is_conservative_for_ascii_and_multibyte_text() {
    assert_eq!(estimate_tokens(b""), REQUEST_OVERHEAD_TOKENS);
    assert_eq!(
        estimate_tokens(&[b'a'; 1_000]),
        500 + REQUEST_OVERHEAD_TOKENS
    );
    assert_eq!(
        estimate_tokens("가나다".as_bytes()),
        6 + REQUEST_OVERHEAD_TOKENS
    );
    assert_eq!(
        estimate_tokens("a😀".as_bytes()),
        1 + 2 + REQUEST_OVERHEAD_TOKENS
    );
}

#[tokio::test]
async fn test_jev_provider_context_limit_failures_are_explicit() {
    let cases: [(u16, &str, JevError); 4] = [
        (
            422,
            r#"{"detail":"state plus the longest question exceeds the 32k token context limit"}"#,
            JevError::ProviderContextLimit { status: 422 },
        ),
        (
            400,
            r#"{"error":"request exceeds the maximum context length of 64000 tokens"}"#,
            JevError::ProviderContextLimit { status: 400 },
        ),
        (413, "", JevError::ProviderContextLimit { status: 413 }),
        (
            422,
            r#"{"detail":"field required: questions.q1.criteria"}"#,
            JevError::HttpStatus {
                status: 422,
                class: HttpFailureClass::InvalidRequest,
            },
        ),
    ];
    for (status, body, expected) in cases {
        let transport = ScriptedTransport::new(move |_, _| (status, body.as_bytes().to_vec()));
        let (result, evaluator) = evaluate_with(
            transport,
            policy(),
            EvaluationRequest::new(state(), mixed_questions()),
        )
        .await;
        let failure = result.unwrap_err();
        assert_eq!(failure.error, expected);
        assert_eq!(failure.requests[0].http_status, Some(status));
        assert_eq!(evaluator.transport().call_count(), 1);
    }
}

#[tokio::test]
async fn test_jev_http_failures_are_classified_without_retry_or_body_retention() {
    let cases = [
        (401, HttpFailureClass::Unauthorized),
        (429, HttpFailureClass::RateLimited),
        (529, HttpFailureClass::Overloaded),
        (500, HttpFailureClass::Server),
    ];
    for (status, class) in cases {
        let transport =
            ScriptedTransport::new(move |_, _| (status, b"secret-body-marker".to_vec()));
        let (result, evaluator) = evaluate_with(
            transport,
            policy(),
            EvaluationRequest::new(state(), mixed_questions()),
        )
        .await;
        let failure = result.unwrap_err();
        assert_eq!(failure.error, JevError::HttpStatus { status, class });
        assert_eq!(
            evaluator.transport().call_count(),
            1,
            "HTTP {status} must not be retried"
        );
        assert!(!format!("{failure:?} {failure}").contains("secret-body-marker"));
    }
}

#[tokio::test(start_paused = true)]
async fn test_jev_deadline_includes_queue_time_and_releases_permits() {
    let policy = TransportPolicy {
        max_in_flight_requests: 1,
        ..policy()
    };
    let evaluator = Evaluator::new(
        ScriptedTransport::valid().with_delay(Duration::from_secs(10)),
        policy,
    )
    .unwrap();
    let started = Instant::now();
    let holder = evaluator.evaluate(EvaluationRequest::new(state(), mixed_questions()));
    let queued = evaluator.evaluate(
        EvaluationRequest::new(state(), mixed_questions())
            .with_deadline(started + Duration::from_secs(5)),
    );
    let (held, queued) = tokio::join!(holder, queued);
    assert!(held.is_ok());
    let failure = queued.unwrap_err();
    assert_eq!(
        failure.error,
        JevError::DeadlineExceeded {
            elapsed: Duration::from_secs(5)
        }
    );
    assert_eq!(failure.requests[0].outcome, RequestOutcome::NotCompleted);
    assert_eq!(evaluator.transport().call_count(), 1);
    assert_eq!(evaluator.available_permits(), 1);
}

#[tokio::test(start_paused = true)]
async fn test_jev_policy_timeout_and_shorter_caller_deadline_stop_slow_requests() {
    let short_policy = TransportPolicy {
        timeout: Duration::from_secs(2),
        ..policy()
    };
    let evaluator = Evaluator::new(
        ScriptedTransport::valid().with_delay(Duration::from_secs(10)),
        short_policy,
    )
    .unwrap();
    let failure = evaluator
        .evaluate(EvaluationRequest::new(state(), mixed_questions()))
        .await
        .unwrap_err();
    assert_eq!(
        failure.error,
        JevError::DeadlineExceeded {
            elapsed: Duration::from_secs(2)
        }
    );
    assert_eq!(
        evaluator.available_permits(),
        DEFAULT_MAX_IN_FLIGHT_REQUESTS
    );
    assert_eq!(evaluator.transport().in_flight.load(Ordering::SeqCst), 0);

    let evaluator = Evaluator::new(
        ScriptedTransport::valid().with_delay(Duration::from_secs(10)),
        policy(),
    )
    .unwrap();
    let failure = evaluator
        .evaluate(
            EvaluationRequest::new(state(), mixed_questions())
                .with_deadline(Instant::now() + Duration::from_secs(1)),
        )
        .await
        .unwrap_err();
    assert_eq!(
        failure.error,
        JevError::DeadlineExceeded {
            elapsed: Duration::from_secs(1)
        }
    );
    assert_eq!(failure.timing.http_elapsed, Duration::ZERO);
    assert_eq!(
        evaluator.available_permits(),
        DEFAULT_MAX_IN_FLIGHT_REQUESTS
    );
}

#[tokio::test(start_paused = true)]
async fn test_jev_cancellation_stops_in_flight_requests() {
    let evaluator = Evaluator::new(
        ScriptedTransport::valid().with_delay(Duration::from_secs(10)),
        policy(),
    )
    .unwrap();
    let token = CancellationToken::new();
    let canceller = {
        let token = token.clone();
        async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            token.cancel();
        }
    };
    let (result, ()) = tokio::join!(
        evaluator.evaluate(
            EvaluationRequest::new(state(), mixed_questions()).with_cancellation(token.clone())
        ),
        canceller
    );
    let failure = result.unwrap_err();
    assert_eq!(failure.error, JevError::Cancelled);
    assert_eq!(failure.timing.elapsed, Duration::from_secs(1));
    assert_eq!(failure.requests[0].outcome, RequestOutcome::NotCompleted);
    assert_eq!(
        evaluator.available_permits(),
        DEFAULT_MAX_IN_FLIGHT_REQUESTS
    );
    assert_eq!(evaluator.transport().in_flight.load(Ordering::SeqCst), 0);

    let calls_before = evaluator.transport().call_count();
    let failure = evaluator
        .evaluate(EvaluationRequest::new(state(), mixed_questions()).with_cancellation(token))
        .await
        .unwrap_err();
    assert_eq!(failure.error, JevError::Cancelled);
    assert_eq!(evaluator.transport().call_count(), calls_before);
}

#[tokio::test(start_paused = true)]
async fn test_jev_concurrency_spacing_and_timing_are_bounded() {
    let policy = TransportPolicy {
        max_batch_bytes: MIN_BATCH_BYTES,
        request_spacing: Duration::from_millis(300),
        ..TransportPolicy::default()
    };
    let (result, evaluator) = evaluate_with(
        ScriptedTransport::valid().with_delay(Duration::from_secs(1)),
        policy,
        EvaluationRequest::new(state(), sized_questions(21, 1_000)),
    )
    .await;
    let evaluation = result.unwrap();
    let transport = evaluator.transport();
    assert_eq!(evaluation.requests.len(), 7);
    assert_eq!(transport.max_in_flight.load(Ordering::SeqCst), 3);
    let starts = transport.starts.lock().unwrap();
    for pair in starts.windows(2) {
        assert!(pair[1] - pair[0] >= Duration::from_millis(300));
    }
    // Seven one-second requests, three at a time, with 300ms spacing.
    assert_eq!(evaluation.timing.http_elapsed, Duration::from_secs(7));
    assert!(evaluation.timing.elapsed < evaluation.timing.http_elapsed);
    assert!(evaluation.timing.elapsed >= Duration::from_millis(3_000));
    assert_eq!(
        evaluation.usage,
        Usage {
            input_tokens: 700,
            output_tokens: 70
        }
    );
}

#[tokio::test(start_paused = true)]
async fn test_jev_first_failure_stops_remaining_batches() {
    let policy = TransportPolicy {
        max_batch_bytes: MIN_BATCH_BYTES,
        max_in_flight_requests: 1,
        ..policy()
    };
    let transport = ScriptedTransport::new(|call_index, request| {
        if call_index == 0 {
            (529, Vec::new())
        } else {
            (200, encode(&valid_response(request)))
        }
    })
    .with_delay(Duration::from_millis(100));
    let (result, evaluator) = evaluate_with(
        transport,
        policy,
        EvaluationRequest::new(state(), sized_questions(12, 1_000)),
    )
    .await;
    let failure = result.unwrap_err();
    assert_eq!(evaluator.transport().call_count(), 1);
    assert_eq!(failure.requests[0].outcome, RequestOutcome::Failed);
    assert!(failure.requests[1..].iter().all(|record| record.outcome
        == RequestOutcome::NotCompleted
        && record.http_status.is_none()));
    assert_eq!(evaluator.available_permits(), 1);
}

#[test]
fn test_jev_policy_rejects_aliases_and_unbounded_budgets() {
    assert!(TransportPolicy::default().validate().is_ok());
    for model in [
        "jev-latest",
        "jev-preview",
        "Jev-1.13.0",
        "jev-1.13",
        "jev-1.13.0-rc",
        "1.13.0",
    ] {
        let policy = TransportPolicy {
            model: model.to_string(),
            ..TransportPolicy::default()
        };
        assert!(policy.validate().is_err(), "{model} must be rejected");
    }
    let invalid_policies = [
        TransportPolicy {
            timeout: Duration::ZERO,
            ..TransportPolicy::default()
        },
        TransportPolicy {
            timeout: MAX_TIMEOUT + Duration::from_millis(1),
            ..TransportPolicy::default()
        },
        TransportPolicy {
            max_in_flight_requests: 0,
            ..TransportPolicy::default()
        },
        TransportPolicy {
            max_in_flight_requests: MAX_IN_FLIGHT_REQUESTS_LIMIT + 1,
            ..TransportPolicy::default()
        },
        TransportPolicy {
            request_spacing: MAX_REQUEST_SPACING + Duration::from_millis(1),
            ..TransportPolicy::default()
        },
        TransportPolicy {
            max_batch_bytes: MIN_BATCH_BYTES - 1,
            ..TransportPolicy::default()
        },
        TransportPolicy {
            pool_idle_timeout: Duration::ZERO,
            ..TransportPolicy::default()
        },
    ];
    for policy in invalid_policies {
        assert!(
            matches!(policy.validate(), Err(JevError::InvalidPolicy { .. })),
            "{policy:?}"
        );
        assert!(Evaluator::new(ScriptedTransport::valid(), policy).is_err());
    }
}

#[test]
fn test_jev_debug_output_omits_credentials_and_source_data() {
    assert!(ApiKey::new("  ").is_err());
    assert!(ApiKey::new("key with space").is_err());
    let api_key = ApiKey::new(" secret-key-marker ").unwrap();
    assert_eq!(format!("{api_key:?}"), "ApiKey([redacted])");
    let transport = HttpsTransport::new(&api_key, &TransportPolicy::default()).unwrap();
    let transport_debug = format!("{transport:?}");
    assert!(transport_debug.contains(ENDPOINT));
    assert!(!transport_debug.contains("secret-key-marker"));

    let question = Question::noul(
        "q1",
        json!({"question": "q", "evidence": "secret-source-marker"}),
        None,
    )
    .unwrap();
    let request = EvaluationRequest::new(
        json!({"task_query": "secret-state-marker"}),
        vec![question.clone()],
    );
    let debug = format!("{question:?} {request:?}");
    assert!(!debug.contains("secret-source-marker"));
    assert!(!debug.contains("secret-state-marker"));
    assert!(debug.contains("q1"));
}

/// Minimal HTTP/1.1 keep-alive server that answers Jev bodies and counts connections.
async fn serve_loopback(listener: tokio::net::TcpListener, accepted: Arc<AtomicUsize>) {
    loop {
        let (mut stream, _) = listener.accept().await.unwrap();
        accepted.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(async move {
            let mut buffer = Vec::new();
            loop {
                let header_end = loop {
                    if let Some(position) =
                        buffer.windows(4).position(|window| window == b"\r\n\r\n")
                    {
                        break position + 4;
                    }
                    let mut chunk = [0u8; 4096];
                    let read = stream.read(&mut chunk).await.unwrap_or(0);
                    if read == 0 {
                        return;
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                };
                let headers = String::from_utf8_lossy(&buffer[..header_end]).to_ascii_lowercase();
                assert!(headers.starts_with("post /v1/systemone http/1.1"));
                assert!(headers.contains("authorization: bearer loopback-key"));
                assert!(headers.contains("content-type: application/json"));
                let content_length: usize = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .map(|value| value.trim().parse().unwrap())
                    .unwrap();
                while buffer.len() < header_end + content_length {
                    let mut chunk = [0u8; 4096];
                    let read = stream.read(&mut chunk).await.unwrap_or(0);
                    if read == 0 {
                        return;
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                }
                let request: Value =
                    serde_json::from_slice(&buffer[header_end..header_end + content_length])
                        .unwrap();
                buffer.drain(..header_end + content_length);
                let body = encode(&valid_response(&request));
                let head = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
                    body.len()
                );
                if stream.write_all(head.as_bytes()).await.is_err()
                    || stream.write_all(&body).await.is_err()
                {
                    return;
                }
            }
        });
    }
}

#[tokio::test]
async fn test_jev_transport_reuses_idle_connections_only_within_the_idle_timeout() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let accepted = Arc::new(AtomicUsize::new(0));
    let server = tokio::spawn(serve_loopback(listener, accepted.clone()));
    let policy = TransportPolicy {
        pool_idle_timeout: Duration::from_millis(300),
        ..policy()
    };
    let api_key = ApiKey::new("loopback-key").unwrap();
    let transport = HttpsTransport::loopback_for_test(
        &format!("http://{address}/v1/systemone"),
        &api_key,
        &policy,
    )
    .unwrap();
    let evaluator = Evaluator::new(transport, policy).unwrap();

    for _ in 0..3 {
        let evaluation = evaluator
            .evaluate(EvaluationRequest::new(state(), mixed_questions()))
            .await
            .unwrap();
        assert_eq!(evaluation.answers.len(), 3);
    }
    assert_eq!(
        accepted.load(Ordering::SeqCst),
        1,
        "back-to-back requests reuse one connection"
    );

    tokio::time::sleep(Duration::from_millis(900)).await;
    evaluator
        .evaluate(EvaluationRequest::new(state(), mixed_questions()))
        .await
        .unwrap();
    assert_eq!(
        accepted.load(Ordering::SeqCst),
        2,
        "an idle-expired connection is replaced"
    );
    server.abort();
}
