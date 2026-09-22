use super::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

fn request() -> EvaluationRequest {
    EvaluationRequest {
        request_id: "mixed".into(),
        state: json!({"task_query":"Keep useful implementation evidence"}),
        questions: BTreeMap::from([
            (
                "score".into(),
                Question::Score {
                    instructions: json!({"question":"Rate evidence"}),
                    criteria: vec![
                        json!("absent"),
                        json!({"level":"supporting"}),
                        json!("direct"),
                    ],
                },
            ),
            (
                "choice".into(),
                Question::Choice {
                    instructions: json!(["Choose disposition"]),
                    criteria: BTreeMap::from([
                        ("keep".into(), Value::Null),
                        ("omit".into(), json!(["irrelevant"])),
                    ]),
                },
            ),
            (
                "noul".into(),
                Question::Noul {
                    instructions: json!("Is the evidence useful?"),
                    criteria: None,
                },
            ),
        ]),
    }
}

fn response() -> Value {
    json!({"model":MODEL,"usage":{"input_tokens":123,"output_tokens":11},"answers":{
        "score":{"type":"score","score":2.0,"probabilities":{"0":0.0,"1":0.0,"2":1.0},"confidence":1.0,"legend":{"0":"absent","1":"supporting","2":"direct"}},
        "choice":{"type":"choice","choice":"keep","probabilities":{"keep":0.9,"omit":0.1},"confidence":0.8},
        "noul":{"type":"noul","noul":0.9}
    }})
}

struct Fake {
    response: Value,
    delay_ms: u64,
    calls: AtomicUsize,
    failure: Option<FailureKind>,
}
impl Fake {
    fn good() -> Self {
        Self {
            response: response(),
            delay_ms: 0,
            calls: AtomicUsize::new(0),
            failure: None,
        }
    }
}
impl Transport for Fake {
    fn send(&self, _body: Vec<u8>) -> DecisionFuture<'_, Result<Vec<u8>, FailureKind>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
            if let Some(error) = self.failure {
                return Err(error);
            }
            Ok(serde_json::to_vec(&self.response).unwrap())
        })
    }
}

#[tokio::test]
async fn test_mixed_primitives_and_usage_without_noul_confidence() {
    let fake = Arc::new(Fake::good());
    let runtime = Runtime::new(fake.clone(), Policy::default()).unwrap();
    let output = runtime
        .evaluate(request(), EvaluationOptions::default())
        .await
        .unwrap();
    assert!(matches!(
        output.answers["score"],
        Answer::Score { score: 2.0, .. }
    ));
    assert!(matches!(&output.answers["choice"],Answer::Choice {choice,..} if choice=="keep"));
    assert!(matches!(output.answers["noul"], Answer::Noul { noul: 0.9 }));
    assert_eq!(
        output.metrics.usage,
        Usage {
            input_tokens: 123,
            output_tokens: 11
        }
    );
    assert_eq!(output.model, MODEL);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn test_exact_ids_model_types_probabilities_and_weighted_score() {
    let questions = request().questions;
    let mutations: Vec<Box<dyn Fn(&mut Value)>> = vec![
        Box::new(|r| r["model"] = json!("jev-latest")),
        Box::new(|r| {
            r["answers"].as_object_mut().unwrap().remove("score");
        }),
        Box::new(|r| r["answers"]["extra"] = json!({"type":"noul","noul":1.0})),
        Box::new(|r| r["answers"]["noul"]["noul"] = json!(1.01)),
        Box::new(|r| r["answers"]["noul"]["noul"] = json!("0.9")),
        Box::new(|r| r["answers"]["choice"]["choice"] = json!("omit")),
        Box::new(|r| r["answers"]["choice"]["probabilities"]["keep"] = json!(-0.1)),
        Box::new(|r| r["answers"]["choice"]["probabilities"]["keep"] = json!(0.7)),
        Box::new(|r| r["answers"]["choice"]["confidence"] = json!(1.1)),
        Box::new(|r| r["answers"]["score"]["score"] = json!(1.0)),
        Box::new(|r| r["answers"]["score"]["legend"] = json!({"9":"wrong"})),
        Box::new(|r| {
            r.as_object_mut().unwrap().remove("usage");
        }),
    ];
    assert!(validate_response(&questions, response()).is_ok());
    for mutate in mutations {
        let mut raw = response();
        mutate(&mut raw);
        assert_eq!(
            validate_response(&questions, raw).err(),
            Some(FailureKind::InvalidResponse)
        );
    }
    assert!(!probability(f64::NAN));
    assert!(!probability(f64::INFINITY));
}

#[test]
fn test_score_criteria_two_to_ten_and_choice_255_options() {
    for count in [1, 2, 3, 10, 11] {
        let question = Question::Score {
            instructions: json!("Rate"),
            criteria: vec![json!("level"); count],
        };
        assert_eq!(question.validate().is_ok(), (2..=10).contains(&count));
    }
    for count in [0, 1, 255, 256] {
        let question = Question::Choice {
            instructions: json!("Choose"),
            criteria: (0..count).map(|n| (n.to_string(), Value::Null)).collect(),
        };
        assert_eq!(question.validate().is_ok(), (1..=255).contains(&count));
    }
    assert!(Question::Noul {
        instructions: json!("Check"),
        criteria: Some(BTreeMap::from([("true".into(), json!({"meaning":"yes"}))]))
    }
    .validate()
    .is_ok());
}

#[test]
fn test_pack_covers_every_question_without_truncation() {
    let mut input = request();
    input.questions = (0..120)
        .map(|i| {
            (
                format!("q{i:03}"),
                Question::Noul {
                    instructions: json!({"question":"Is it useful?","body":"x".repeat(1100)}),
                    criteria: None,
                },
            )
        })
        .collect();
    let batches = pack(&input, 8000).unwrap();
    assert!(batches.len() > 1);
    assert!(batches.iter().all(|batch| batch.bytes.len() <= 8000));
    let ids: Vec<_> = batches
        .iter()
        .flat_map(|batch| batch.questions.keys())
        .collect();
    assert_eq!(ids, input.questions.keys().collect::<Vec<_>>());
    assert_eq!(
        pack(&input, 1024).err(),
        Some(FailureKind::OversizedQuestion)
    );
    input.state = json!("x".repeat(33_000));
    assert_eq!(
        pack(&input, 80_000).err(),
        Some(FailureKind::ContextEstimate)
    );
}

#[test]
fn test_total_context_estimate_splits_below_transport_cap() {
    let mut input = request();
    input.questions = (0..100)
        .map(|i| {
            (
                format!("q{i:03}"),
                Question::Noul {
                    instructions: json!("x".repeat(1200)),
                    criteria: None,
                },
            )
        })
        .collect();
    let batches = pack(&input, 80_000).unwrap();
    assert!(batches.len() >= 2);
    assert!(batches
        .iter()
        .all(|batch| batch.bytes.len() + 1024 <= 64_000));
}

#[tokio::test]
async fn test_deadline_includes_queue_and_releases_permits() {
    let fake = Arc::new(Fake {
        delay_ms: 100,
        ..Fake::good()
    });
    let runtime = Runtime::new(fake.clone(), Policy::default()).unwrap();
    let options = EvaluationOptions {
        deadline: Instant::now() + Duration::from_millis(10),
        ..Default::default()
    };
    assert_eq!(
        runtime
            .evaluate(request(), options)
            .await
            .err()
            .unwrap()
            .kind,
        FailureKind::Deadline
    );
    tokio::task::yield_now().await;
    assert_eq!(runtime.permits.available_permits(), 3);
    let options = EvaluationOptions {
        deadline: Instant::now() + Duration::from_millis(10),
        ..Default::default()
    };
    assert_eq!(
        runtime
            .evaluate(request(), options)
            .await
            .err()
            .unwrap()
            .kind,
        FailureKind::Deadline
    );
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_cancellation_before_and_during_http() {
    let fake = Arc::new(Fake {
        delay_ms: 100,
        ..Fake::good()
    });
    let runtime = Runtime::new(fake.clone(), Policy::default()).unwrap();
    let options = EvaluationOptions::default();
    options.cancellation.cancel();
    assert_eq!(
        runtime
            .evaluate(request(), options)
            .await
            .err()
            .unwrap()
            .kind,
        FailureKind::Cancelled
    );
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    let options = EvaluationOptions::default();
    let cancellation = options.cancellation.clone();
    let cancel = async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        cancellation.cancel();
    };
    let (result, _) = tokio::join!(runtime.evaluate(request(), options), cancel);
    assert_eq!(result.err().unwrap().kind, FailureKind::Cancelled);
    tokio::task::yield_now().await;
    assert_eq!(runtime.permits.available_permits(), 3);
}

#[tokio::test]
async fn test_provider_context_failures_and_http_errors_never_retry() {
    // A provider may reject either its 64k aggregate or its 32k state+question budget.
    for (context, status) in [
        ("aggregate", 422),
        ("longest", 422),
        ("rate", 429),
        ("overloaded", 529),
    ] {
        let fake = Arc::new(Fake {
            failure: Some(FailureKind::Http(status)),
            ..Fake::good()
        });
        let runtime = Runtime::new(fake.clone(), Policy::default()).unwrap();
        let error = runtime
            .evaluate(request(), EvaluationOptions::default())
            .await
            .err()
            .unwrap();
        assert_eq!(error.kind, FailureKind::Http(status), "{context}");
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn test_invalid_response_reports_known_usage_and_no_partial_answers() {
    let mut fake = Fake::good();
    fake.response["answers"]
        .as_object_mut()
        .unwrap()
        .remove("noul");
    let runtime = Runtime::new(Arc::new(fake), Policy::default()).unwrap();
    let error = runtime
        .evaluate(request(), EvaluationOptions::default())
        .await
        .err()
        .unwrap();
    assert_eq!(error.kind, FailureKind::InvalidResponse);
    assert_eq!(error.metrics.usage.input_tokens, 123);
    assert_eq!(error.metrics.requests_completed, 1);
}

#[tokio::test]
async fn test_concurrency_ceiling_and_dispatch_spacing() {
    struct Concurrent {
        active: AtomicUsize,
        peak: AtomicUsize,
        starts: Mutex<Vec<Instant>>,
    }
    impl Transport for Concurrent {
        fn send(&self, body: Vec<u8>) -> DecisionFuture<'_, Result<Vec<u8>, FailureKind>> {
            Box::pin(async move {
                let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
                self.peak.fetch_max(active, Ordering::SeqCst);
                self.starts.lock().unwrap().push(Instant::now());
                tokio::time::sleep(Duration::from_millis(650)).await;
                self.active.fetch_sub(1, Ordering::SeqCst);
                let request: Value = serde_json::from_slice(&body).unwrap();
                let answers = request["questions"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(|key| (key.clone(), json!({"type":"noul","noul":0.5})))
                    .collect::<serde_json::Map<_, _>>();
                Ok(serde_json::to_vec(&json!({"model":MODEL,"answers":answers,"usage":{"input_tokens":1,"output_tokens":1}})).unwrap())
            })
        }
    }
    let transport = Arc::new(Concurrent {
        active: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
        starts: Mutex::new(Vec::new()),
    });
    let runtime = Runtime::new(
        transport.clone(),
        Policy {
            max_batch_bytes: 1024,
            ..Default::default()
        },
    )
    .unwrap();
    let mut input = request();
    input.questions = (0..6)
        .map(|index| {
            (
                format!("q{index}"),
                Question::Noul {
                    instructions: json!("x".repeat(600)),
                    criteria: None,
                },
            )
        })
        .collect();
    let result = runtime
        .evaluate(input, EvaluationOptions::default())
        .await
        .unwrap();
    assert_eq!(result.answers.len(), 6);
    assert_eq!(result.metrics.requests_started, 6);
    assert_eq!(transport.peak.load(Ordering::SeqCst), 3);
    let starts = transport.starts.lock().unwrap();
    assert_eq!(starts.len(), 6);
    assert!(starts
        .windows(2)
        .all(|pair| pair[1].duration_since(pair[0]) >= Duration::from_millis(300)));
}
