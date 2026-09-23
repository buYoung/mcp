use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Mutex, PoisonError};
use std::task::Poll;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::Semaphore;
use tokio::time::Instant;

use super::answer::{parse_answer, Answer, ChoiceAnswer, NoulAnswer, ScoreAnswer};
use super::batch::{self, Batch};
use super::cancellation::CancellationToken;
use super::error::EvaluationFailure;
use super::question::Question;
use super::transport::{ApiKey, HttpsTransport, JevTransport};
use super::wire::{classify_http_failure, parse_response, WireResponse};
use super::{BoxFuture, JevError};

/// Pinned provider model. Aliases can move under a caller, so they are rejected.
pub const DEFAULT_MODEL: &str = "jev-1.13.0";
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(45_000);
pub const DEFAULT_MAX_IN_FLIGHT_REQUESTS: usize = 3;
pub const DEFAULT_REQUEST_SPACING: Duration = Duration::from_millis(300);
pub const DEFAULT_MAX_BATCH_BYTES: usize = 80_000;
pub const DEFAULT_POOL_IDLE_TIMEOUT: Duration = Duration::from_millis(30_000);

pub const MAX_TIMEOUT: Duration = Duration::from_millis(600_000);
pub const MAX_IN_FLIGHT_REQUESTS_LIMIT: usize = 16;
pub const MAX_REQUEST_SPACING: Duration = Duration::from_millis(10_000);
pub const MIN_BATCH_BYTES: usize = 4_096;
pub const MAX_BATCH_BYTES_LIMIT: usize = 512_000;
pub const MAX_POOL_IDLE_TIMEOUT: Duration = Duration::from_millis(600_000);

/// Transport budgets for one evaluator. A caller may shorten a single evaluation with
/// `EvaluationRequest::with_deadline`, never extend it.
#[derive(Clone, Debug, PartialEq)]
pub struct TransportPolicy {
    /// Versioned model id such as `jev-1.13.0`.
    pub model: String,
    /// Whole-call deadline covering validation, permit queueing, spacing, and every batch.
    pub timeout: Duration,
    pub max_in_flight_requests: usize,
    /// Minimum gap between consecutive request starts on this evaluator.
    pub request_spacing: Duration,
    /// Ceiling for one encoded request body.
    pub max_batch_bytes: usize,
    /// Pooled connections idle for longer are closed instead of reused.
    pub pool_idle_timeout: Duration,
}

impl Default for TransportPolicy {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            timeout: DEFAULT_TIMEOUT,
            max_in_flight_requests: DEFAULT_MAX_IN_FLIGHT_REQUESTS,
            request_spacing: DEFAULT_REQUEST_SPACING,
            max_batch_bytes: DEFAULT_MAX_BATCH_BYTES,
            pool_idle_timeout: DEFAULT_POOL_IDLE_TIMEOUT,
        }
    }
}

impl TransportPolicy {
    pub fn validate(&self) -> Result<(), JevError> {
        let invalid = |reason: &str| {
            Err(JevError::InvalidPolicy {
                reason: reason.to_string(),
            })
        };
        if !is_versioned_model(&self.model) {
            return invalid("model must be a versioned id such as `jev-1.13.0`, not an alias");
        }
        if self.timeout.is_zero() || self.timeout > MAX_TIMEOUT {
            return invalid("timeout must be between 1ms and 600000ms");
        }
        if !(1..=MAX_IN_FLIGHT_REQUESTS_LIMIT).contains(&self.max_in_flight_requests) {
            return invalid("max_in_flight_requests must be between 1 and 16");
        }
        if self.request_spacing > MAX_REQUEST_SPACING {
            return invalid("request_spacing must be at most 10000ms");
        }
        if !(MIN_BATCH_BYTES..=MAX_BATCH_BYTES_LIMIT).contains(&self.max_batch_bytes) {
            return invalid("max_batch_bytes must be between 4096 and 512000");
        }
        if self.pool_idle_timeout.is_zero() || self.pool_idle_timeout > MAX_POOL_IDLE_TIMEOUT {
            return invalid("pool_idle_timeout must be between 1ms and 600000ms");
        }
        Ok(())
    }
}

/// `<name>-<major>.<minor>.<patch>` with a lowercase name, as the provider reports it.
pub fn is_versioned_model(model: &str) -> bool {
    let Some((name, version)) = model.rsplit_once('-') else {
        return false;
    };
    let is_name_valid = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    let parts: Vec<&str> = version.split('.').collect();
    is_name_valid
        && parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl Usage {
    pub fn plus(self, other: Usage) -> Usage {
        Usage {
            input_tokens: self.input_tokens + other.input_tokens,
            output_tokens: self.output_tokens + other.output_tokens,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timing {
    /// Whole call, including permit queueing, spacing, and response validation.
    pub elapsed: Duration,
    /// Sum of completed HTTP exchanges; overlapping requests can make it exceed `elapsed`.
    pub http_elapsed: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestOutcome {
    Succeeded,
    Failed,
    /// Not dispatched, or dropped in flight after another batch failed, the deadline
    /// passed, or the caller cancelled.
    NotCompleted,
}

/// Identity and accounting for one HTTP body. Bodies themselves are never retained.
#[derive(Clone, Debug, PartialEq)]
pub struct RequestRecord {
    pub batch_index: usize,
    pub question_count: usize,
    pub body_bytes: usize,
    pub estimated_tokens: usize,
    pub body_sha256: String,
    pub outcome: RequestOutcome,
    pub http_status: Option<u16>,
    pub http_elapsed: Option<Duration>,
    pub usage: Option<Usage>,
}

/// Independent questions judged against one shared state. Put the caller's explicit task
/// intent and shared facts in named state fields; candidate evidence belongs in named
/// state or instruction fields that each question references itself.
pub struct EvaluationRequest {
    state: Value,
    questions: Vec<Question>,
    deadline: Option<Instant>,
    cancellation: Option<CancellationToken>,
}

impl EvaluationRequest {
    pub fn new(state: Value, questions: Vec<Question>) -> Self {
        Self {
            state,
            questions,
            deadline: None,
            cancellation: None,
        }
    }

    /// Shortens the policy timeout for this call; queue time counts toward it.
    pub fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    pub fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    pub fn questions(&self) -> &[Question] {
        &self.questions
    }
}

impl fmt::Debug for EvaluationRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state_kind = match &self.state {
            Value::String(_) => "string",
            Value::Object(_) => "object",
            Value::Array(_) => "array",
            _ => "other",
        };
        formatter
            .debug_struct("EvaluationRequest")
            .field("state_kind", &state_kind)
            .field("question_count", &self.questions.len())
            .field("has_deadline", &self.deadline.is_some())
            .field("has_cancellation", &self.cancellation.is_some())
            .finish()
    }
}

/// Every answer of a completed evaluation, keyed by question id.
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
    pub usage: Usage,
    pub timing: Timing,
    pub requests: Vec<RequestRecord>,
}

impl Evaluation {
    pub fn answer(&self, question_id: &str) -> Option<&Answer> {
        self.answers.get(question_id)
    }

    pub fn score(&self, question_id: &str) -> Option<&ScoreAnswer> {
        self.answer(question_id).and_then(Answer::as_score)
    }

    pub fn choice(&self, question_id: &str) -> Option<&ChoiceAnswer> {
        self.answer(question_id).and_then(Answer::as_choice)
    }

    pub fn noul(&self, question_id: &str) -> Option<&NoulAnswer> {
        self.answer(question_id).and_then(Answer::as_noul)
    }
}

/// The adapter-facing boundary. Production uses `Evaluator<HttpsTransport>`; tests inject
/// another transport or evaluator so no suite reaches the provider.
pub trait JevEvaluator: Send + Sync {
    fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> BoxFuture<'_, Result<Evaluation, EvaluationFailure>>;
}

/// Validates, batches, dispatches, and checks typed answers. One instance shares its
/// permits and request spacing across every evaluation it runs.
pub struct Evaluator<T> {
    transport: T,
    policy: TransportPolicy,
    permits: Semaphore,
    next_start: Mutex<Option<Instant>>,
}

impl Evaluator<HttpsTransport> {
    pub fn https(api_key: &ApiKey, policy: TransportPolicy) -> Result<Self, JevError> {
        policy.validate()?;
        let transport = HttpsTransport::new(api_key, &policy)?;
        Self::new(transport, policy)
    }
}

struct BatchResult {
    batch_index: usize,
    http_elapsed: Duration,
    http_status: Option<u16>,
    usage: Option<Usage>,
    outcome: Result<Vec<(String, Answer)>, JevError>,
}

impl<T: JevTransport> Evaluator<T> {
    pub fn new(transport: T, policy: TransportPolicy) -> Result<Self, JevError> {
        policy.validate()?;
        Ok(Self {
            transport,
            permits: Semaphore::new(policy.max_in_flight_requests),
            policy,
            next_start: Mutex::new(None),
        })
    }

    pub fn policy(&self) -> &TransportPolicy {
        &self.policy
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    #[cfg(test)]
    pub(crate) fn available_permits(&self) -> usize {
        self.permits.available_permits()
    }

    /// Boxes the failure to keep the returned `Result` small; `evaluate` unboxes it.
    async fn run(&self, request: EvaluationRequest) -> Result<Evaluation, Box<EvaluationFailure>> {
        let started = Instant::now();
        let policy_deadline = started + self.policy.timeout;
        let deadline = request
            .deadline
            .map_or(policy_deadline, |deadline| deadline.min(policy_deadline));
        let EvaluationRequest {
            state,
            questions,
            cancellation,
            ..
        } = request;
        let early_failure = |error: JevError| {
            Box::new(EvaluationFailure {
                error,
                usage: Usage::default(),
                timing: Timing {
                    elapsed: started.elapsed(),
                    http_elapsed: Duration::ZERO,
                },
                requests: Vec::new(),
            })
        };
        validate_request(&state, &questions).map_err(early_failure)?;
        let batches = batch::pack(
            &self.policy.model,
            &state,
            &questions,
            self.policy.max_batch_bytes,
        )
        .map_err(early_failure)?;
        if cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return Err(early_failure(JevError::Cancelled));
        }
        if Instant::now() >= deadline {
            return Err(early_failure(JevError::DeadlineExceeded {
                elapsed: started.elapsed(),
            }));
        }

        let mut records: Vec<RequestRecord> = batches
            .iter()
            .enumerate()
            .map(|(batch_index, batch)| RequestRecord {
                batch_index,
                question_count: batch.question_indexes.len(),
                body_bytes: batch.body.len(),
                estimated_tokens: batch.estimated_tokens,
                body_sha256: batch.body_sha256.clone(),
                outcome: RequestOutcome::NotCompleted,
                http_status: None,
                http_elapsed: None,
                usage: None,
            })
            .collect();
        let mut answers = BTreeMap::new();
        let dispatches: Vec<Option<BoxFuture<'_, BatchResult>>> = batches
            .into_iter()
            .enumerate()
            .map(|(batch_index, batch)| {
                Some(Box::pin(self.dispatch(batch_index, batch, &questions))
                    as BoxFuture<'_, BatchResult>)
            })
            .collect();
        let outcome = {
            let joined = join_batches(dispatches, &mut records, &mut answers);
            let cancelled = async {
                match &cancellation {
                    Some(token) => token.cancelled().await,
                    None => std::future::pending().await,
                }
            };
            // Dropping the losing branches drops in-flight requests and their permits.
            tokio::select! {
                biased;
                () = cancelled => Err(JevError::Cancelled),
                () = tokio::time::sleep_until(deadline) => Err(JevError::DeadlineExceeded {
                    elapsed: started.elapsed(),
                }),
                result = joined => result,
            }
        };

        let usage = records
            .iter()
            .filter_map(|record| record.usage)
            .fold(Usage::default(), Usage::plus);
        let timing = Timing {
            elapsed: started.elapsed(),
            http_elapsed: records
                .iter()
                .filter_map(|record| record.http_elapsed)
                .sum(),
        };
        tracing::debug!(
            batches = records.len(),
            outcome = outcome.as_ref().err().map_or("succeeded", JevError::label),
            input_tokens = usage.input_tokens,
            output_tokens = usage.output_tokens,
            elapsed_ms = timing.elapsed.as_millis() as u64,
            http_ms = timing.http_elapsed.as_millis() as u64,
            "jev evaluation finished"
        );
        match outcome {
            Ok(()) => Ok(Evaluation {
                model: self.policy.model.clone(),
                answers,
                usage,
                timing,
                requests: records,
            }),
            Err(error) => Err(Box::new(EvaluationFailure {
                error,
                usage,
                timing,
                requests: records,
            })),
        }
    }

    async fn dispatch(
        &self,
        batch_index: usize,
        batch: Batch,
        questions: &[Question],
    ) -> BatchResult {
        let _permit = self
            .permits
            .acquire()
            .await
            .expect("the evaluator never closes its semaphore");
        tokio::time::sleep_until(self.reserve_start()).await;
        let http_started = Instant::now();
        let response = self.transport.post(batch.body).await;
        let mut result = BatchResult {
            batch_index,
            http_elapsed: http_started.elapsed(),
            http_status: None,
            usage: None,
            outcome: Ok(Vec::new()),
        };
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                result.outcome = Err(error.into());
                return result;
            }
        };
        result.http_status = Some(response.status);
        if response.status != 200 {
            result.outcome = Err(classify_http_failure(response.status, &response.body));
            return result;
        }
        match parse_response(&response.body) {
            Ok(wire) => {
                result.usage = Some(wire.usage());
                result.outcome = self.checked_answers(&wire, &batch.question_indexes, questions);
            }
            Err(error) => result.outcome = Err(error),
        }
        result
    }

    /// Spaces request starts across every evaluation sharing this evaluator.
    fn reserve_start(&self) -> Instant {
        let mut next_start = self
            .next_start
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        let start = next_start
            .filter(|scheduled| *scheduled > now)
            .unwrap_or(now);
        *next_start = Some(start + self.policy.request_spacing);
        start
    }

    fn checked_answers(
        &self,
        wire: &WireResponse,
        question_indexes: &[usize],
        questions: &[Question],
    ) -> Result<Vec<(String, Answer)>, JevError> {
        if wire.model != self.policy.model {
            return Err(JevError::ModelMismatch {
                expected: self.policy.model.clone(),
                actual: wire.model.chars().take(64).collect(),
            });
        }
        let expected: BTreeSet<&str> = question_indexes
            .iter()
            .map(|index| questions[*index].id())
            .collect();
        let returned: BTreeSet<&str> = wire.answers.keys().map(String::as_str).collect();
        if expected != returned {
            return Err(JevError::AnswerSetMismatch {
                missing_count: expected.difference(&returned).count(),
                unexpected_count: returned.difference(&expected).count(),
            });
        }
        question_indexes
            .iter()
            .map(|index| {
                let question = &questions[*index];
                parse_answer(question, &wire.answers[question.id()])
                    .map(|answer| (question.id().to_string(), answer))
            })
            .collect()
    }
}

impl<T: JevTransport> JevEvaluator for Evaluator<T> {
    fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> BoxFuture<'_, Result<Evaluation, EvaluationFailure>> {
        Box::pin(async move { self.run(request).await.map_err(|failure| *failure) })
    }
}

impl<T> fmt::Debug for Evaluator<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Evaluator")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

fn validate_request(state: &Value, questions: &[Question]) -> Result<(), JevError> {
    let is_state_valid = match state {
        Value::String(text) => !text.trim().is_empty(),
        Value::Object(object) => !object.is_empty(),
        Value::Array(items) => !items.is_empty(),
        _ => false,
    };
    if !is_state_valid {
        return Err(JevError::InvalidRequest {
            reason: "state must be a non-blank string, a non-empty object, or a non-empty array"
                .to_string(),
        });
    }
    if questions.is_empty() {
        return Err(JevError::InvalidRequest {
            reason: "at least one question is required".to_string(),
        });
    }
    let mut ids = BTreeSet::new();
    for question in questions {
        if !ids.insert(question.id()) {
            return Err(JevError::InvalidRequest {
                reason: format!("question id `{}` is repeated", question.id()),
            });
        }
    }
    Ok(())
}

/// Polls every batch in the caller's task and stops at the first failure; dropping the
/// remaining futures cancels their requests and releases their permits.
async fn join_batches<'a>(
    mut dispatches: Vec<Option<BoxFuture<'a, BatchResult>>>,
    records: &mut [RequestRecord],
    answers: &mut BTreeMap<String, Answer>,
) -> Result<(), JevError> {
    std::future::poll_fn(|context| {
        let mut is_pending = false;
        for slot in dispatches.iter_mut() {
            let Some(dispatch) = slot.as_mut() else {
                continue;
            };
            let Poll::Ready(result) = dispatch.as_mut().poll(context) else {
                is_pending = true;
                continue;
            };
            *slot = None;
            let record = &mut records[result.batch_index];
            record.http_elapsed = Some(result.http_elapsed);
            record.http_status = result.http_status;
            record.usage = result.usage;
            match result.outcome {
                Ok(batch_answers) => {
                    record.outcome = RequestOutcome::Succeeded;
                    answers.extend(batch_answers);
                }
                Err(error) => {
                    record.outcome = RequestOutcome::Failed;
                    return Poll::Ready(Err(error));
                }
            }
        }
        if is_pending {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    })
    .await
}
