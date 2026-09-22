use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fake {
    calls: AtomicUsize,
    status: u16,
    delay: Duration,
    corrupt: bool,
}
impl Transport for Fake {
    fn post<'a>(&'a self, body: Vec<u8>, _: Duration) -> Pin<Box<dyn Future<Output = Result<TransportResponse, FailureKind>> + Send + 'a>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            let request: Value = serde_json::from_slice(&body).unwrap();
            let mut answers = serde_json::Map::new();
            for (id, question) in request["questions"].as_object().unwrap() {
                let answer = match question["type"].as_str().unwrap() {
                    "score" => json!({"type":"score","score":1.0,"confidence":1.0,"legend":{"0":"no","1":"yes"},"probabilities":{"0":0.0,"1":1.0}}),
                    "choice" => json!({"type":"choice","choice":"keep","confidence":1.0,"probabilities":{"keep":1.0,"omit":0.0}}),
                    "noul" => json!({"type":"noul","noul":0.9}),
                    _ => unreachable!(),
                };
                answers.insert(id.clone(), answer);
            }
            if self.corrupt { answers.remove(answers.keys().next().unwrap().clone().as_str()); }
            Ok(TransportResponse { status: self.status, body: json!({"model":MODEL,"answers":answers,"usage":{"input_tokens":10,"output_tokens":2}}).to_string().into_bytes() })
        })
    }
}
fn fake() -> Arc<Fake> {
    Arc::new(Fake { calls: AtomicUsize::new(0), status: 200, delay: Duration::ZERO, corrupt: false })
}
fn request() -> EvaluationRequest {
    EvaluationRequest {
        task_query: "Find a relevant function".into(), state: json!({"evidence":"fn run() {}"}),
        questions: BTreeMap::from([
            ("a".into(), Question::Score { instructions: json!("How relevant is `evidence`?"), criteria: vec![json!("no"),json!("yes")] }),
            ("b".into(), Question::Choice { instructions: json!("Choose"), criteria: BTreeMap::from([("keep".into(),Value::Null),("omit".into(),Value::Null)]) }),
            ("c".into(), Question::Noul { instructions: json!(["Is evidence relevant?"]), criteria: None }),
        ]), policy: Policy { request_spacing: Duration::ZERO, ..Policy::default() }, cancellation: None,
    }
}
#[tokio::test]
async fn test_jev_mixed_answers_usage_and_batching() {
    let fake = fake();
    let result = JevEvaluator::new(fake.clone(), 3).evaluate(request()).await.unwrap();
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    assert_eq!(result.answers.len(), 3);
    assert_eq!(result.usage, Usage { input_tokens: 10, output_tokens: 2 });
    assert!(matches!(result.answers["c"], Answer::Noul { noul: 0.9 }));
}
#[tokio::test]
async fn test_jev_invalid_and_bounded_inputs_do_not_call_transport() {
    let fake = fake(); let evaluator = JevEvaluator::new(fake.clone(), 3);
    let mut req = request(); req.questions.get_mut("a").map(|q| *q = Question::Score { instructions: json!("why"), criteria: vec![json!("only one")] });
    assert_eq!(evaluator.evaluate(req).await.err().unwrap().kind, FailureKind::InvalidInput);
    let mut req = request(); req.state = json!("a".repeat(35_000));
    assert_eq!(evaluator.evaluate(req).await.err().unwrap().kind, FailureKind::InputLimit);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn test_jev_bounded_batches_and_reused_transport() {
    let fake = fake();
    let evaluator = JevEvaluator::new(fake.clone(), 3);
    let mut req = request();
    req.questions = (0..6).map(|i| (format!("q{i}"), Question::Noul {
        instructions: json!({"judgment":"Is this relevant?", "evidence":"x".repeat(2_000)}),
        criteria: None,
    })).collect();
    req.policy.max_batch_bytes = 5_000;
    let result = evaluator.evaluate(req).await.unwrap();
    assert_eq!(result.answers.len(), 6);
    assert!(fake.calls.load(Ordering::SeqCst) > 1);
    // A second independent request reuses the same transport instance; its state never leaks.
    evaluator.evaluate(request()).await.unwrap();
    assert_eq!(fake.calls.load(Ordering::SeqCst), result.request_ids.len() + 1);
}

#[tokio::test]
async fn test_jev_incomplete_and_provider_failure() {
    for (status, corrupt, expected) in [(200, true, FailureKind::InvalidResponse), (422, false, FailureKind::ProviderStatus(422))] {
        let fake = Arc::new(Fake { calls: AtomicUsize::new(0), status, delay: Duration::ZERO, corrupt });
        let failure = JevEvaluator::new(fake, 3).evaluate(request()).await.err().unwrap();
        assert_eq!(failure.kind, expected);
        assert_eq!(failure.completed_batches, 0);
    }
}
#[tokio::test]
async fn test_jev_deadline_and_cancellation() {
    let fake = Arc::new(Fake { calls: AtomicUsize::new(0), status: 200, delay: Duration::from_millis(50), corrupt: false });
    let evaluator = JevEvaluator::new(fake, 3);
    let mut req = request(); req.policy.deadline = Duration::from_millis(5);
    assert_eq!(evaluator.evaluate(req).await.err().unwrap().kind, FailureKind::Deadline);
    let (sender, receiver) = watch::channel(true);
    let mut req = request(); req.cancellation = Some(receiver);
    drop(sender);
    assert_eq!(evaluator.evaluate(req).await.err().unwrap().kind, FailureKind::Cancelled);
}
#[test]
fn test_jev_criteria_bounds_and_structured_fields() {
    let score = |levels| Question::Score { instructions: json!({"question":"rate `evidence`"}),
        criteria:(0..levels).map(|i| json!({"level":i})).collect() };
    assert!(!valid_question(&score(1)) && valid_question(&score(2)));
    assert!(valid_question(&score(10)) && !valid_question(&score(11)));
    let choice = |count| Question::Choice { instructions:json!(["choose", "the best"]),
        criteria:(0..count).map(|i| (format!("key-{i}"),Value::Null)).collect() };
    assert!(valid_question(&choice(255)) && !valid_question(&choice(256)));
    let noul = Question::Noul {instructions:json!({"question":"true?"}), criteria:Some(NoulCriteria {yes:json!(["yes"]),no:json!({"no":"unlikely"})})};
    assert!(valid_question(&noul));
}

#[tokio::test]
async fn test_jev_total_context_bound_splits_without_dropping_questions() {
    let fake = fake();
    let evaluator = JevEvaluator::new(fake.clone(),3);
    let mut req = request();
    req.questions = (0..30).map(|index| (format!("q-{index}"),Question::Noul {
        instructions:json!({"judgment":"related?","evidence":"a".repeat(2_500)}),criteria:None,
    })).collect();
    let result = evaluator.evaluate(req).await.unwrap();
    assert_eq!(result.answers.len(),30);
    assert!(result.request_ids.len() >= 2);
    assert_eq!(fake.calls.load(Ordering::SeqCst),result.request_ids.len());
}

#[tokio::test]
async fn test_jev_spacing_queue_is_inside_deadline() {
    let fake = fake();
    let evaluator = JevEvaluator::new(fake,3);
    let mut req = request();
    req.questions = (0..2).map(|index| (format!("q-{index}"),Question::Noul {
        instructions:json!({"judgment":"related?","evidence":"a".repeat(900)}),criteria:None,
    })).collect();
    req.policy.max_batch_bytes = 1_500;
    req.policy.request_spacing = Duration::from_millis(200);
    req.policy.deadline = Duration::from_millis(50);
    let failure = evaluator.evaluate(req).await.err().unwrap();
    assert_eq!(failure.kind,FailureKind::Deadline);
    assert_eq!(failure.completed_batches,1);
    assert_eq!(failure.usage.input_tokens,10);
    assert!(failure.http_elapsed_ms <= failure.elapsed_ms);
}

#[test]
fn test_jev_answer_validation() {
    let q = Question::Noul { instructions: json!("yes?"), criteria: None };
    assert!(validate_answer(&q, &Answer::Noul { noul: 0.0 }));
    assert!(!validate_answer(&q, &Answer::Noul { noul: f64::NAN }));
    let q = Question::Choice { instructions: json!("choose"), criteria: BTreeMap::from([("keep".into(),Value::Null),("omit".into(),Value::Null)]) };
    assert!(!validate_answer(&q, &Answer::Choice { choice: "keep".into(), probabilities: BTreeMap::from([("keep".into(),0.1),("omit".into(),0.9)]), confidence: 0.1 }));
}
