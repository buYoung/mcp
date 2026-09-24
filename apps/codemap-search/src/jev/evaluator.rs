//! The bounded asynchronous evaluator: batching, concurrency permits, request spacing,
//! one absolute deadline including queue time, caller cancellation, response validation and
//! usage/timing accounting.

use super::answer::{
    parse_answer, parse_usage, Answer, EvaluationFailure, EvaluationOutcome, RequestIdentity,
    Timing, Usage,
};
use super::batch::{pack, Batch, BatchLimits};
use super::question::{EvaluationRequest, Question, QuestionId};
use super::transport::{HttpsSettings, HttpsTransport, SecretString, Transport, TransportResponse};
use super::{
    BoxFuture, CancelToken, Evaluator, JevError, DEFAULT_MODEL, MAX_BATCHES_PER_REQUEST,
    MAX_BATCH_BYTES, MAX_IN_FLIGHT_REQUESTS, MAX_RESPONSE_BODY_BYTES, MIN_REQUEST_SPACING,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::Instant;

/// Transport policy shared by every request through one evaluator. The defaults are the
/// initial safety policy; a host may only tighten them (see [`EvaluatorConfig::validate`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluatorConfig {
    /// Concrete provider model; validated against every response.
    pub model: String,
    /// HTTP requests in flight at once: `1..=MAX_IN_FLIGHT_REQUESTS`.
    pub max_in_flight_requests: usize,
    /// Minimum interval between two request starts: at least `MIN_REQUEST_SPACING`.
    pub request_spacing: Duration,
    /// Encoded request bytes per batch: `1..=MAX_BATCH_BYTES`. A single question above this
    /// fails explicitly; a value too small for any question fails every request explicitly.
    pub max_batch_bytes: usize,
    /// Whole-call ceiling measured from `evaluate` entry, including queue time. A caller
    /// deadline (`RequestPolicy::deadline_at`) can only bring the effective deadline forward.
    pub deadline: Duration,
    /// Operating limit for state plus all questions, checked with the byte estimate.
    pub max_estimated_tokens: u64,
    /// Operating limit for state plus the longest question, checked with the byte estimate.
    pub max_estimated_state_plus_longest_question_tokens: u64,
}

impl Default for EvaluatorConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.into(),
            max_in_flight_requests: MAX_IN_FLIGHT_REQUESTS,
            request_spacing: MIN_REQUEST_SPACING,
            max_batch_bytes: MAX_BATCH_BYTES,
            deadline: Duration::from_millis(45_000),
            max_estimated_tokens: super::DEFAULT_REQUEST_TOKEN_BUDGET,
            max_estimated_state_plus_longest_question_tokens:
                super::DEFAULT_STATE_QUESTION_TOKEN_BUDGET,
        }
    }
}

/// Longest timeout any Jev setting accepts: seven days, far beyond any whole-call budget
/// and comfortably inside the monotonic clock and timer ranges of every platform.
pub const MAX_SAFE_DURATION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// `true` when `duration` is positive, at most [`MAX_SAFE_DURATION`] and `now + duration`
/// is representable on the monotonic clock (the range check hosts apply to configured
/// timeouts before turning them into deadlines).
pub fn is_safe_duration(duration: Duration) -> bool {
    !duration.is_zero()
        && duration <= MAX_SAFE_DURATION
        && Instant::now().checked_add(duration).is_some()
}

impl EvaluatorConfig {
    pub fn validate(&self) -> Result<(), JevError> {
        if self.model.trim().is_empty() {
            return Err(JevError::InvalidConfig("model is empty".into()));
        }
        if self.max_in_flight_requests == 0 || self.max_in_flight_requests > MAX_IN_FLIGHT_REQUESTS
        {
            return Err(JevError::InvalidConfig(format!(
                "max_in_flight_requests must be between 1 and {MAX_IN_FLIGHT_REQUESTS}"
            )));
        }
        if self.request_spacing < MIN_REQUEST_SPACING {
            return Err(JevError::InvalidConfig(format!(
                "request_spacing must be at least {}ms",
                MIN_REQUEST_SPACING.as_millis()
            )));
        }
        if self.max_batch_bytes == 0 || self.max_batch_bytes > MAX_BATCH_BYTES {
            return Err(JevError::InvalidConfig(format!(
                "max_batch_bytes must be between 1 and {MAX_BATCH_BYTES}"
            )));
        }
        if !is_safe_duration(self.deadline) {
            return Err(JevError::InvalidConfig(
                "deadline must be positive and within the monotonic clock range".into(),
            ));
        }
        if self.max_estimated_tokens == 0
            || self.max_estimated_state_plus_longest_question_tokens == 0
        {
            return Err(JevError::InvalidConfig(
                "estimated token limits must be positive".into(),
            ));
        }
        Ok(())
    }

    fn limits(&self) -> BatchLimits {
        BatchLimits {
            max_batch_bytes: self.max_batch_bytes,
            max_estimated_tokens: self.max_estimated_tokens,
            max_estimated_state_plus_longest_question_tokens: self
                .max_estimated_state_plus_longest_question_tokens,
            max_batches: MAX_BATCHES_PER_REQUEST,
        }
    }
}

/// HTTPS-backed [`Evaluator`]. One instance shares its connection pool, permits and
/// spacing clock across every request; construct it once per process.
pub struct JevEvaluator {
    transport: Arc<dyn Transport>,
    config: EvaluatorConfig,
    permits: Arc<Semaphore>,
    next_start: Arc<Mutex<Option<Instant>>>,
}

impl std::fmt::Debug for JevEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JevEvaluator")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl JevEvaluator {
    /// Wrap any transport (the HTTPS one or a mock) with the bounded runtime policy.
    pub fn new(transport: Arc<dyn Transport>, config: EvaluatorConfig) -> Result<Self, JevError> {
        config.validate()?;
        Ok(Self {
            transport,
            permits: Arc::new(Semaphore::new(config.max_in_flight_requests)),
            next_start: Arc::new(Mutex::new(None)),
            config,
        })
    }

    /// The production constructor: credentials resolved by the caller, HTTPS transport.
    pub fn https(
        api_key: SecretString,
        config: EvaluatorConfig,
        settings: HttpsSettings,
    ) -> Result<Self, JevError> {
        let transport = HttpsTransport::new(api_key, settings)?;
        Self::new(Arc::new(transport), config)
    }

    pub fn config(&self) -> &EvaluatorConfig {
        &self.config
    }

    #[cfg(test)]
    pub(super) fn available_permits(&self) -> usize {
        self.permits.available_permits()
    }

    // The failure deliberately carries usage, timing and request identities of the attempt.
    #[allow(clippy::result_large_err)]
    async fn evaluate_inner(
        &self,
        request: EvaluationRequest,
    ) -> Result<EvaluationOutcome, EvaluationFailure> {
        let started = Instant::now();
        // The effective deadline is the earliest of the caller's absolute instant and this
        // evaluator's own ceiling; the reported `deadline` is that instant relative to entry.
        let own_deadline_at = started.checked_add(self.config.deadline).unwrap_or(started);
        let deadline_at = request
            .policy()
            .deadline_at
            .map_or(own_deadline_at, |caller| caller.min(own_deadline_at));
        let deadline = deadline_at.saturating_duration_since(started);
        let cancel = request.policy().cancel.clone();
        if cancel.as_ref().is_some_and(CancelToken::is_cancelled) {
            return Err(EvaluationFailure::before_dispatch(
                JevError::Cancelled,
                started.elapsed(),
            ));
        }
        if deadline.is_zero() {
            // The caller's budget was spent before this call (for example during synchronous
            // preparation): report it without building or posting anything.
            return Err(EvaluationFailure::before_dispatch(
                JevError::DeadlineExceeded { deadline },
                started.elapsed(),
            ));
        }
        let batches = match pack(
            &self.config.model,
            request.state(),
            request.questions(),
            &self.config.limits(),
        ) {
            Ok(batches) => batches,
            Err(error) => return Err(EvaluationFailure::before_dispatch(error, started.elapsed())),
        };
        let questions: Arc<BTreeMap<QuestionId, Question>> = Arc::new(
            request
                .questions()
                .iter()
                .map(|question| (question.id().clone(), question.clone()))
                .collect(),
        );
        let progress = Arc::new(Mutex::new(Progress::default()));
        let mut tasks = JoinSet::new();
        for batch in batches {
            let context = BatchContext {
                transport: Arc::clone(&self.transport),
                permits: Arc::clone(&self.permits),
                next_start: Arc::clone(&self.next_start),
                spacing: self.config.request_spacing,
                model: self.config.model.clone(),
                questions: Arc::clone(&questions),
                deadline,
                deadline_at,
                progress: Arc::clone(&progress),
            };
            tasks.spawn(run_batch(context, batch));
        }
        let result = tokio::select! {
            biased;
            _ = cancelled(cancel) => Err(JevError::Cancelled),
            result = tokio::time::timeout_at(deadline_at, collect(&mut tasks)) => match result {
                Ok(result) => result,
                Err(_) => Err(JevError::DeadlineExceeded { deadline }),
            },
        };
        // Abort whatever is still queued or in flight and wait for those tasks to unwind, so
        // their permits are back and nothing is recorded after the accounting below.
        tasks.shutdown().await;
        let Progress { usage, requests } = std::mem::take(&mut *lock(&progress));
        let requests: Vec<RequestIdentity> = requests.into_values().collect();
        let timing = Timing {
            elapsed: started.elapsed(),
            queue_wait: requests.iter().map(|identity| identity.queue_wait).sum(),
            http: requests.iter().map(|identity| identity.http_elapsed).sum(),
            request_count: requests.len(),
        };
        match result {
            Ok(mut outcomes) => {
                outcomes.sort_by_key(|outcome| outcome.index);
                let mut answers = BTreeMap::new();
                let mut raw_answers = BTreeMap::new();
                for outcome in outcomes {
                    for (id, (answer, raw)) in outcome.answers {
                        answers.insert(id.clone(), answer);
                        raw_answers.insert(id, raw);
                    }
                }
                Ok(EvaluationOutcome {
                    model: self.config.model.clone(),
                    answers,
                    raw_answers,
                    usage,
                    timing,
                    requests,
                })
            }
            Err(error) => Err(EvaluationFailure {
                error,
                usage,
                timing,
                requests,
            }),
        }
    }
}

impl Evaluator for JevEvaluator {
    fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> BoxFuture<'_, Result<EvaluationOutcome, EvaluationFailure>> {
        Box::pin(self.evaluate_inner(request))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn cancelled(token: Option<CancelToken>) {
    match token {
        Some(token) => token.cancelled().await,
        None => std::future::pending().await,
    }
}

/// Join every batch; the first failure ends the call (the caller aborts the rest).
async fn collect(
    tasks: &mut JoinSet<Result<BatchOutcome, JevError>>,
) -> Result<Vec<BatchOutcome>, JevError> {
    let mut outcomes = Vec::with_capacity(tasks.len());
    while let Some(joined) = tasks.join_next().await {
        let outcome = joined.map_err(|error| {
            JevError::Internal(format!("batch task ended abnormally: {error}"))
        })??;
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

/// Shared accounting across batch tasks. Identities are recorded at dispatch (so a request
/// cut off by a deadline or cancellation still counts as attempted) and replaced with the
/// completed identity and usage when a response arrives.
#[derive(Default)]
struct Progress {
    usage: Usage,
    requests: BTreeMap<usize, RequestIdentity>,
}

impl Progress {
    fn dispatched(&mut self, identity: &RequestIdentity) {
        self.requests.insert(identity.batch_index, identity.clone());
    }

    fn completed(&mut self, identity: &RequestIdentity, usage: Usage) {
        self.usage = self.usage + usage;
        self.requests.insert(identity.batch_index, identity.clone());
    }
}

struct BatchContext {
    transport: Arc<dyn Transport>,
    permits: Arc<Semaphore>,
    next_start: Arc<Mutex<Option<Instant>>>,
    spacing: Duration,
    model: String,
    questions: Arc<BTreeMap<QuestionId, Question>>,
    deadline: Duration,
    deadline_at: Instant,
    progress: Arc<Mutex<Progress>>,
}

struct BatchOutcome {
    index: usize,
    answers: BTreeMap<QuestionId, (Answer, Value)>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn excerpt(body: &[u8]) -> String {
    String::from_utf8_lossy(&body[..body.len().min(512)])
        .chars()
        .take(256)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn error_for_status(status: u16, body: &[u8]) -> JevError {
    match status {
        401 | 403 => JevError::Unauthorized { status },
        422 => JevError::Rejected {
            status,
            excerpt: excerpt(body),
        },
        429 => JevError::RateLimited { status },
        529 => JevError::Overloaded { status },
        _ => JevError::Http {
            status,
            excerpt: excerpt(body),
        },
    }
}

async fn run_batch(context: BatchContext, batch: Batch) -> Result<BatchOutcome, JevError> {
    let Batch {
        index,
        question_ids,
        body,
    } = batch;
    let queue_started = Instant::now();
    // The permit covers the whole round trip and is released on every exit path, including
    // abort: dropping this future drops the permit.
    let _permit = Arc::clone(&context.permits)
        .acquire_owned()
        .await
        .map_err(|_| JevError::Internal("concurrency permits are closed".into()))?;
    // Request spacing: reserve the next start slot under the lock, sleep outside it.
    let delay = {
        let mut next_start = lock(&context.next_start);
        let now = Instant::now();
        let start_at = next_start.map_or(now, |slot| slot.max(now));
        *next_start = Some(start_at + context.spacing);
        start_at.saturating_duration_since(now)
    };
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }
    let remaining = context
        .deadline_at
        .saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(JevError::DeadlineExceeded {
            deadline: context.deadline,
        });
    }
    let mut identity = RequestIdentity {
        batch_index: index,
        question_ids,
        request_bytes: body.len(),
        request_sha256: sha256_hex(&body),
        http_status: None,
        http_elapsed: Duration::ZERO,
        queue_wait: queue_started.elapsed(),
    };
    lock(&context.progress).dispatched(&identity);
    let http_started = Instant::now();
    let response = context.transport.post(body, remaining).await;
    identity.http_elapsed = http_started.elapsed();
    let (result, usage) = match response {
        Ok(response) => {
            identity.http_status = Some(response.status);
            interpret(&context, index, &identity.question_ids, &response)
        }
        Err(error) => (Err(JevError::Transport(error)), Usage::default()),
    };
    lock(&context.progress).completed(&identity, usage);
    result.map(|answers| BatchOutcome { index, answers })
}

type BatchAnswers = BTreeMap<QuestionId, (Answer, Value)>;

/// A JSON walker that rejects an object with a repeated key anywhere in the document.
/// `serde_json::Value` would keep the last duplicate silently, which could hide an answer
/// object that names one question twice with different values.
struct NoDuplicateKeys;

impl<'de> serde::Deserialize<'de> for NoDuplicateKeys {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Walker;

        impl<'de> serde::de::Visitor<'de> for Walker {
            type Value = NoDuplicateKeys;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("any JSON value without duplicate object keys")
            }

            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                Ok(NoDuplicateKeys)
            }
            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(NoDuplicateKeys)
            }
            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(NoDuplicateKeys)
            }
            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(NoDuplicateKeys)
            }
            fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
                Ok(NoDuplicateKeys)
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(NoDuplicateKeys)
            }
            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(NoDuplicateKeys)
            }
            fn visit_some<D: serde::Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> Result<Self::Value, D::Error> {
                <NoDuplicateKeys as serde::Deserialize>::deserialize(deserializer)
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                while seq.next_element::<NoDuplicateKeys>()?.is_some() {}
                Ok(NoDuplicateKeys)
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut seen: HashSet<String> = HashSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !seen.insert(key.clone()) {
                        return Err(serde::de::Error::custom(format!(
                            "duplicate object key '{}'",
                            key.chars().take(64).collect::<String>()
                        )));
                    }
                    map.next_value::<NoDuplicateKeys>()?;
                }
                Ok(NoDuplicateKeys)
            }
        }

        deserializer.deserialize_any(Walker)
    }
}

/// Turn one HTTP response into typed answers plus whatever usage it reported. Usage is
/// returned even when validation fails: the request was billed either way.
fn interpret(
    context: &BatchContext,
    index: usize,
    question_ids: &[QuestionId],
    response: &TransportResponse,
) -> (Result<BatchAnswers, JevError>, Usage) {
    if response.status != 200 {
        return (
            Err(error_for_status(response.status, &response.body)),
            Usage::default(),
        );
    }
    let invalid = |reason: String| JevError::InvalidResponse {
        batch_index: index,
        reason,
    };
    if response.body.len() > MAX_RESPONSE_BODY_BYTES {
        return (
            Err(invalid(format!(
                "body of {} bytes exceeds the {MAX_RESPONSE_BODY_BYTES}-byte limit",
                response.body.len()
            ))),
            Usage::unreported(),
        );
    }
    if let Err(error) = serde_json::from_slice::<NoDuplicateKeys>(&response.body) {
        return (
            Err(invalid(format!("body is not valid JSON: {error}"))),
            Usage::unreported(),
        );
    }
    let parsed: Value = match serde_json::from_slice(&response.body) {
        Ok(parsed) => parsed,
        Err(error) => {
            return (
                Err(invalid(format!("body is not JSON: {error}"))),
                Usage::unreported(),
            )
        }
    };
    let usage = parse_usage(parsed.get("usage"));
    (
        validate_answers(
            &context.model,
            &context.questions,
            index,
            question_ids,
            &parsed,
        ),
        usage,
    )
}

fn validate_answers(
    model: &str,
    questions: &BTreeMap<QuestionId, Question>,
    index: usize,
    question_ids: &[QuestionId],
    parsed: &Value,
) -> Result<BatchAnswers, JevError> {
    let invalid = |reason: &str| JevError::InvalidResponse {
        batch_index: index,
        reason: reason.into(),
    };
    let actual_model = parsed
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("'model' must be a string"))?;
    if actual_model != model {
        return Err(JevError::ModelMismatch {
            expected: model.into(),
            actual: actual_model.into(),
        });
    }
    let raw_answers = parsed
        .get("answers")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("'answers' must be an object"))?;
    let expected: BTreeSet<&str> = question_ids.iter().map(QuestionId::as_str).collect();
    let returned: BTreeSet<&str> = raw_answers.keys().map(String::as_str).collect();
    if expected != returned {
        return Err(JevError::IncompleteAnswers {
            batch_index: index,
            missing: expected
                .difference(&returned)
                .filter_map(|name| QuestionId::new(*name).ok())
                .collect(),
            unexpected: returned
                .difference(&expected)
                .map(|name| (*name).to_string())
                .collect(),
        });
    }
    let mut answers = BTreeMap::new();
    for id in question_ids {
        let question = questions
            .get(id)
            .ok_or_else(|| invalid("batch names a question outside the request"))?;
        let raw = &raw_answers[id.as_str()];
        let answer = parse_answer(question, raw).map_err(|reason| JevError::InvalidAnswer {
            id: id.clone(),
            reason,
        })?;
        answers.insert(id.clone(), (answer, raw.clone()));
    }
    Ok(answers)
}
