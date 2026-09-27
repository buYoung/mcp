//! Offline doubles reused by the runtime tests, adapter tests, the in-crate example and the
//! MCP regression suite. Nothing here is reachable from a production request: a mock is
//! only used when Rust code constructs one and injects it.

use super::answer::{Answer, EvaluationFailure, EvaluationOutcome, RequestIdentity, Timing, Usage};
use super::question::{EvaluationRequest, QuestionId};
use super::transport::{Transport, TransportError, TransportResponse};
use super::{BoxFuture, Evaluator, JevError};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One scripted HTTP outcome for [`MockTransport`].
#[derive(Clone, Debug)]
pub struct ScriptedResponse {
    pub delay: Duration,
    pub result: Result<(u16, Vec<u8>), TransportError>,
}

impl ScriptedResponse {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self::status(200, body)
    }

    pub fn status(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            delay: Duration::ZERO,
            result: Ok((status, body.into())),
        }
    }

    pub fn error(error: TransportError) -> Self {
        Self {
            delay: Duration::ZERO,
            result: Err(error),
        }
    }

    pub fn after(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

/// One recorded POST: the decoded request body plus dispatch timing on the Tokio clock.
#[derive(Clone, Debug)]
pub struct RecordedPost {
    /// The decoded request body.
    pub body: Value,
    /// The exact bytes that were posted (what the request identity hashes).
    pub bytes: Vec<u8>,
    pub timeout: Duration,
    pub started_at: tokio::time::Instant,
    /// Number of posts in flight when this one started, including itself.
    pub concurrent_posts: usize,
}

type ResponseHandler = dyn Fn(&Value) -> ScriptedResponse + Send + Sync;

/// A transport that never touches the network. Either replays a script in order or lets a
/// handler derive each response from the decoded request body.
pub struct MockTransport {
    script: Mutex<VecDeque<ScriptedResponse>>,
    handler: Option<Arc<ResponseHandler>>,
    posts: Mutex<Vec<RecordedPost>>,
    in_flight: AtomicUsize,
}

impl MockTransport {
    pub fn scripted(responses: Vec<ScriptedResponse>) -> Self {
        Self {
            script: Mutex::new(responses.into()),
            handler: None,
            posts: Mutex::new(Vec::new()),
            in_flight: AtomicUsize::new(0),
        }
    }

    pub fn with_handler<F>(handler: F) -> Self
    where
        F: Fn(&Value) -> ScriptedResponse + Send + Sync + 'static,
    {
        Self {
            script: Mutex::new(VecDeque::new()),
            handler: Some(Arc::new(handler)),
            posts: Mutex::new(Vec::new()),
            in_flight: AtomicUsize::new(0),
        }
    }

    pub fn posts(&self) -> Vec<RecordedPost> {
        self.posts.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn post_count(&self) -> usize {
        self.posts.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}

impl Transport for MockTransport {
    fn post(
        &self,
        body: Vec<u8>,
        timeout: Duration,
    ) -> BoxFuture<'_, Result<TransportResponse, TransportError>> {
        Box::pin(async move {
            let decoded: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let concurrent_posts = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.posts
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(RecordedPost {
                    body: decoded.clone(),
                    bytes: body.clone(),
                    timeout,
                    started_at: tokio::time::Instant::now(),
                    concurrent_posts,
                });
            let scripted = match &self.handler {
                Some(handler) => handler(&decoded),
                None => self
                    .script
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .pop_front()
                    .unwrap_or_else(|| {
                        ScriptedResponse::error(TransportError::Io(
                            "mock transport script exhausted".into(),
                        ))
                    }),
            };
            let started = tokio::time::Instant::now();
            if !scripted.delay.is_zero() {
                tokio::time::sleep(scripted.delay).await;
            }
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            scripted.result.map(|(status, body)| TransportResponse {
                status,
                body,
                elapsed: started.elapsed(),
            })
        })
    }
}

/// A two-phase gate for tests that must act while an evaluation is in flight: wait for
/// [`Gate::entered`], change the environment, then [`Gate::release`] the evaluator.
#[derive(Debug, Default)]
pub struct Gate {
    entered: AtomicBool,
    released: AtomicBool,
    entered_notify: tokio::sync::Notify,
    released_notify: tokio::sync::Notify,
}

impl Gate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn release(&self) {
        self.released.store(true, Ordering::Release);
        self.released_notify.notify_waiters();
    }

    /// Resolves once an evaluation has reached the gate.
    pub async fn entered(&self) {
        let notified = self.entered_notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.entered.load(Ordering::Acquire) {
            return;
        }
        notified.await;
    }

    async fn pass(&self) {
        self.entered.store(true, Ordering::Release);
        self.entered_notify.notify_waiters();
        let notified = self.released_notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.released.load(Ordering::Acquire) {
            return;
        }
        notified.await;
    }
}

/// What a [`MockEvaluator`] saw for one request: the wire state and wire questions so
/// adapter tests can assert exact evidence composition, plus the caller's deadline.
#[derive(Clone, Debug)]
pub struct RecordedRequest {
    pub state: Value,
    pub questions: BTreeMap<QuestionId, Value>,
    pub deadline_at: Option<tokio::time::Instant>,
}

impl RecordedRequest {
    /// The adapters' task intent field of an object state, when present.
    pub fn task_query(&self) -> Option<&str> {
        self.state
            .get(super::TASK_QUERY_FIELD)
            .and_then(Value::as_str)
    }
}

type AnswerHandler =
    dyn Fn(&EvaluationRequest) -> Result<BTreeMap<QuestionId, Answer>, JevError> + Send + Sync;

/// An [`Evaluator`] that answers from a handler without any transport work. The handler
/// receives the full request and returns typed answers by question ID (or an error to
/// exercise fallback paths).
pub struct MockEvaluator {
    handler: Arc<AnswerHandler>,
    model: String,
    usage: Usage,
    delay: Duration,
    gate: Option<Arc<Gate>>,
    requests: Mutex<Vec<RecordedRequest>>,
}

impl MockEvaluator {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(&EvaluationRequest) -> Result<BTreeMap<QuestionId, Answer>, JevError>
            + Send
            + Sync
            + 'static,
    {
        Self {
            handler: Arc::new(handler),
            model: super::DEFAULT_MODEL.into(),
            usage: Usage::reported(0, 0),
            delay: Duration::ZERO,
            gate: None,
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Fail every request with `error`.
    pub fn failing(error: JevError) -> Self {
        Self::new(move |_| Err(error.clone()))
    }

    pub fn with_usage(mut self, usage: Usage) -> Self {
        self.usage = usage;
        self
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn with_gate(mut self, gate: Arc<Gate>) -> Self {
        self.gate = Some(gate);
        self
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn request_count(&self) -> usize {
        self.requests
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }
}

impl Evaluator for MockEvaluator {
    fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> BoxFuture<'_, Result<EvaluationOutcome, EvaluationFailure>> {
        Box::pin(async move {
            self.requests
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(RecordedRequest {
                    state: request.state().clone(),
                    questions: request
                        .questions()
                        .iter()
                        .map(|question| (question.id().clone(), question.to_wire()))
                        .collect(),
                    deadline_at: request.policy().deadline_at,
                });
            if let Some(gate) = &self.gate {
                gate.pass().await;
            }
            if !self.delay.is_zero() {
                tokio::time::sleep(self.delay).await;
            }
            if let Some(cancel) = &request.policy().cancel {
                if cancel.is_cancelled() {
                    return Err(EvaluationFailure::from(JevError::Cancelled));
                }
            }
            let answers = (self.handler)(&request).map_err(EvaluationFailure::from)?;
            let missing: Vec<QuestionId> = request
                .questions()
                .iter()
                .map(|question| question.id().clone())
                .filter(|id| !answers.contains_key(id))
                .collect();
            if !missing.is_empty() {
                return Err(EvaluationFailure::from(JevError::IncompleteAnswers {
                    batch_index: 0,
                    missing,
                    unexpected: Vec::new(),
                }));
            }
            let raw_answers = answers
                .iter()
                .map(|(id, answer)| (id.clone(), raw_answer(answer)))
                .collect();
            Ok(EvaluationOutcome {
                model: self.model.clone(),
                answers,
                raw_answers,
                usage: self.usage,
                timing: Timing {
                    elapsed: self.delay,
                    request_count: 1,
                    ..Timing::default()
                },
                requests: vec![RequestIdentity {
                    batch_index: 0,
                    question_ids: request
                        .questions()
                        .iter()
                        .map(|question| question.id().clone())
                        .collect(),
                    request_bytes: 0,
                    request_sha256: "mock".into(),
                    http_status: None,
                    http_elapsed: Duration::ZERO,
                    queue_wait: Duration::ZERO,
                }],
            })
        })
    }
}

/// The wire shape of a typed answer, so mock outcomes carry replayable raw answers too.
pub fn raw_answer(answer: &Answer) -> Value {
    match answer {
        Answer::Score(score) => serde_json::json!({
            "type": "score",
            "score": score.score,
            "confidence": score.confidence,
            "probabilities": score
                .probabilities
                .iter()
                .enumerate()
                .map(|(level, probability)| (level.to_string(), Value::from(*probability)))
                .collect::<serde_json::Map<_, _>>(),
        }),
        Answer::Choice(choice) => serde_json::json!({
            "type": "choice",
            "choice": choice.choice,
            "confidence": choice.confidence,
            "probabilities": choice.probabilities,
        }),
        Answer::Noul(noul) => serde_json::json!({ "type": "noul", "noul": noul.noul }),
    }
}

/// Convenience constructors for scripted answers.
pub mod answers {
    use super::super::answer::{Answer, ChoiceAnswer, NoulAnswer, ScoreAnswer};
    use std::collections::BTreeMap;

    /// A Score answer with the given level distribution; `score` is the weighted mean.
    pub fn score(probabilities: &[f64]) -> Answer {
        let score = probabilities
            .iter()
            .enumerate()
            .map(|(level, probability)| level as f64 * probability)
            .sum();
        Answer::Score(ScoreAnswer {
            score,
            probabilities: probabilities.to_vec(),
            confidence: None,
        })
    }

    pub fn choice(choice: &str, probabilities: &[(&str, f64)]) -> Answer {
        Answer::Choice(ChoiceAnswer {
            choice: choice.into(),
            probabilities: probabilities
                .iter()
                .map(|(option, probability)| ((*option).into(), *probability))
                .collect::<BTreeMap<_, _>>(),
            confidence: None,
        })
    }

    pub fn noul(noul: f64) -> Answer {
        Answer::Noul(NoulAnswer { noul })
    }
}
