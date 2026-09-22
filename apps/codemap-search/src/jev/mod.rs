//! Reusable, request-local Jev decisions. No tool, index, or MCP state enters this module.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::sync::{watch, Mutex, Semaphore};
use tokio::task::JoinSet;

pub const MODEL: &str = "jev-1.13.0";
pub const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_ESTIMATED_REQUEST_TOKENS: usize = 64_000;
const MAX_ESTIMATED_STATE_AND_QUESTION_TOKENS: usize = 32_000;
const PROBABILITY_TOLERANCE: f64 = 0.02;

#[derive(Clone, Debug)]
pub struct Policy {
    pub deadline: Duration,
    pub max_batch_bytes: usize,
    pub max_in_flight_requests: usize,
    pub request_spacing: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            deadline: Duration::from_millis(45_000),
            max_batch_bytes: 80_000,
            max_in_flight_requests: 3,
            request_spacing: Duration::from_millis(300),
        }
    }
}

/// A question ID is a routing key; all evidence and judgment semantics belong in
/// named state and instructions. Structured values are preserved on the wire.
#[derive(Clone, Debug)]
pub enum Question {
    Score {
        instructions: Value,
        criteria: Vec<Value>,
    },
    Choice {
        instructions: Value,
        criteria: BTreeMap<String, Value>,
    },
    Noul {
        instructions: Value,
        criteria: Option<BTreeMap<String, Value>>,
    },
}

impl Question {
    fn wire(&self) -> Value {
        match self {
            Self::Score {
                instructions,
                criteria,
            } => json!({"type":"score","instructions":instructions,"criteria":criteria}),
            Self::Choice {
                instructions,
                criteria,
            } => json!({"type":"choice","instructions":instructions,"criteria":criteria}),
            Self::Noul {
                instructions,
                criteria,
            } => {
                let mut wire = json!({"type":"noul","instructions":instructions});
                if let Some(criteria) = criteria {
                    wire["criteria"] = json!(criteria);
                }
                wire
            }
        }
    }

    fn validate(&self) -> Result<(), JevError> {
        let instructions = match self {
            Self::Score { instructions, .. }
            | Self::Choice { instructions, .. }
            | Self::Noul { instructions, .. } => instructions,
        };
        if !is_structured_text(instructions) {
            return Err(JevError::InvalidRequest);
        }
        match self {
            Self::Score { criteria, .. } => {
                if !(2..=10).contains(&criteria.len()) || !criteria.iter().all(is_structured_text) {
                    return Err(JevError::InvalidRequest);
                }
            }
            Self::Choice { criteria, .. } => {
                if criteria.len() < 2
                    || criteria.len() > 255
                    || criteria.iter().any(|(key, value)| {
                        key.is_empty() || !(value.is_null() || is_structured_text(value))
                    })
                {
                    return Err(JevError::InvalidRequest);
                }
            }
            Self::Noul { criteria, .. } => {
                if criteria.as_ref().is_some_and(|criteria| {
                    criteria.keys().any(|key| key != "true" && key != "false")
                        || criteria.values().any(|value| !is_structured_text(value))
                }) {
                    return Err(JevError::InvalidRequest);
                }
            }
        }
        Ok(())
    }
}

fn is_structured_text(value: &Value) -> bool {
    value.is_string() || value.is_object() || value.is_array()
}

#[derive(Clone, Debug)]
pub struct Request {
    pub state: Value,
    pub questions: BTreeMap<String, Question>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Score {
        score: f64,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Noul {
        probability: f64,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone, Debug)]
pub struct BatchRecord {
    pub request_index: usize,
    pub question_ids: Vec<String>,
    pub http_elapsed: Duration,
    pub usage: Usage,
}

#[derive(Clone, Debug)]
pub struct Evaluation {
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
    pub usage: Usage,
    pub elapsed: Duration,
    pub batches: Vec<BatchRecord>,
}

/// Error variants intentionally carry no source, credentials, response body, or
/// request state, including through Debug formatting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JevError {
    InvalidRequest,
    RequestTooLarge,
    EstimatedContextLimit,
    Deadline,
    Cancelled,
    Transport,
    ProviderRejected,
    ProviderContextLimit,
    InvalidResponse,
}

impl JevError {
    pub fn label(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::RequestTooLarge => "request_too_large",
            Self::EstimatedContextLimit => "estimated_context_limit",
            Self::Deadline => "deadline",
            Self::Cancelled => "cancelled",
            Self::Transport => "transport",
            Self::ProviderRejected => "provider_rejected",
            Self::ProviderContextLimit => "provider_context_limit",
            Self::InvalidResponse => "invalid_response",
        }
    }
}

#[derive(Clone)]
pub struct Cancellation {
    sender: watch::Sender<bool>,
}

impl Cancellation {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(false);
        Self { sender }
    }

    pub fn cancel(&self) {
        self.sender.send_replace(true);
    }

    async fn cancelled(&self) {
        let mut receiver = self.sender.subscribe();
        while !*receiver.borrow_and_update() {
            if receiver.changed().await.is_err() {
                return;
            }
        }
    }
}

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Transport: Send + Sync {
    fn post<'a>(
        &'a self,
        request: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<Value, JevError>> + Send + 'a>>;
}

/// Production HTTPS transport. reqwest's idle pool timeout replaces idle
/// connections after 30 seconds; a POST is never retried automatically.
pub struct HttpTransport {
    client: reqwest::Client,
    api_key: String,
    endpoint: String,
}

impl HttpTransport {
    pub fn new(api_key: String, pool_idle_timeout: Duration) -> Result<Self, JevError> {
        if api_key.is_empty() {
            return Err(JevError::InvalidRequest);
        }
        let client = reqwest::Client::builder()
            .pool_idle_timeout(pool_idle_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| JevError::Transport)?;
        Ok(Self {
            client,
            api_key,
            endpoint: ENDPOINT.into(),
        })
    }

    #[cfg(test)]
    fn with_test_endpoint(
        api_key: String,
        pool_idle_timeout: Duration,
        endpoint: String,
    ) -> Result<Self, JevError> {
        let mut transport = Self::new(api_key, pool_idle_timeout)?;
        transport.endpoint = endpoint;
        Ok(transport)
    }
}

impl Transport for HttpTransport {
    fn post<'a>(
        &'a self,
        request: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<Value, JevError>> + Send + 'a>> {
        Box::pin(async move {
            let mut response = self
                .client
                .post(&self.endpoint)
                .bearer_auth(&self.api_key)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(request)
                .send()
                .await
                .map_err(|_| JevError::Transport)?;
            let status = response.status();
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| JevError::Transport)? {
                if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                    return Err(JevError::InvalidResponse);
                }
                bytes.extend_from_slice(&chunk);
            }
            if !status.is_success() {
                let is_context_limit = matches!(status.as_u16(), 413 | 422) && {
                    let body = String::from_utf8_lossy(&bytes).to_lowercase();
                    body.contains("context") || body.contains("token limit")
                };
                return Err(if is_context_limit {
                    JevError::ProviderContextLimit
                } else {
                    JevError::ProviderRejected
                });
            }
            serde_json::from_slice(&bytes).map_err(|_| JevError::InvalidResponse)
        })
    }
}

pub trait Evaluator: Send + Sync {
    fn evaluate<'a>(
        &'a self,
        request: Request,
        policy: Policy,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<Evaluation, JevError>> + Send + 'a>>;
}

pub struct BatchEvaluator {
    transport: Arc<dyn Transport>,
}

impl BatchEvaluator {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self { transport }
    }
}

impl Evaluator for BatchEvaluator {
    fn evaluate<'a>(
        &'a self,
        request: Request,
        policy: Policy,
        cancellation: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<Evaluation, JevError>> + Send + 'a>> {
        Box::pin(async move {
            let started = Instant::now();
            let batches = pack_batches(&request, &policy)?;
            let deadline = tokio::time::Instant::from_std(started + policy.deadline);
            let work = self.evaluate_batches(batches, policy);
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => Err(JevError::Cancelled),
                result = tokio::time::timeout_at(deadline, work) =>
                    result.map_err(|_| JevError::Deadline).and_then(|result| result)
                    .map(|mut evaluation| { evaluation.elapsed = started.elapsed(); evaluation }),
            }
        })
    }
}

impl BatchEvaluator {
    async fn evaluate_batches(
        &self,
        batches: Vec<PreparedBatch>,
        policy: Policy,
    ) -> Result<Evaluation, JevError> {
        let permits = Arc::new(Semaphore::new(policy.max_in_flight_requests));
        let next_request = Arc::new(Mutex::new(Instant::now()));
        let mut jobs = JoinSet::new();
        for batch in batches {
            let transport = Arc::clone(&self.transport);
            let permits = Arc::clone(&permits);
            let next_request = Arc::clone(&next_request);
            let spacing = policy.request_spacing;
            jobs.spawn(async move {
                let _permit = permits
                    .acquire_owned()
                    .await
                    .map_err(|_| JevError::Transport)?;
                {
                    let mut next = next_request.lock().await;
                    tokio::time::sleep_until(tokio::time::Instant::from_std(*next)).await;
                    *next = Instant::now() + spacing;
                }
                let started = Instant::now();
                let response = transport.post(batch.bytes).await?;
                let http_elapsed = started.elapsed();
                let (answers, usage) = parse_response(&batch.questions, &response)?;
                Ok::<_, JevError>((
                    batch.index,
                    batch.question_ids,
                    answers,
                    usage,
                    http_elapsed,
                ))
            });
        }
        let mut answers = BTreeMap::new();
        let mut usage = Usage::default();
        let mut records = Vec::new();
        while let Some(result) = jobs.join_next().await {
            let (request_index, question_ids, batch_answers, batch_usage, http_elapsed) =
                result.map_err(|_| JevError::Transport)??;
            answers.extend(batch_answers);
            usage.input_tokens = usage.input_tokens.saturating_add(batch_usage.input_tokens);
            usage.output_tokens = usage
                .output_tokens
                .saturating_add(batch_usage.output_tokens);
            records.push(BatchRecord {
                request_index,
                question_ids,
                http_elapsed,
                usage: batch_usage,
            });
        }
        records.sort_by_key(|record| record.request_index);
        Ok(Evaluation {
            model: MODEL.into(),
            answers,
            usage,
            elapsed: Duration::ZERO,
            batches: records,
        })
    }
}

struct PreparedBatch {
    index: usize,
    bytes: Vec<u8>,
    question_ids: Vec<String>,
    questions: BTreeMap<String, Question>,
}

fn pack_batches(request: &Request, policy: &Policy) -> Result<Vec<PreparedBatch>, JevError> {
    if !is_structured_text(&request.state)
        || request.questions.is_empty()
        || policy.deadline.is_zero()
        || policy.deadline > Duration::from_millis(45_000)
        || policy.max_batch_bytes == 0
        || policy.max_batch_bytes > 80_000
        || policy.max_in_flight_requests == 0
        || policy.max_in_flight_requests > 3
        || policy.request_spacing < Duration::from_millis(300)
    {
        return Err(JevError::InvalidRequest);
    }
    let state_bytes = serde_json::to_vec(&request.state)
        .map_err(|_| JevError::InvalidRequest)?
        .len();
    let base_bytes = serde_json::to_vec(&wire_request(&request.state, &BTreeMap::new()))
        .map_err(|_| JevError::InvalidRequest)?
        .len();
    let byte_limit = policy.max_batch_bytes.min(MAX_ESTIMATED_REQUEST_TOKENS);
    let mut result = Vec::new();
    let mut pending = BTreeMap::new();
    let mut pending_bytes = base_bytes;
    for (id, question) in &request.questions {
        if id.is_empty() || question.validate().is_err() {
            return Err(JevError::InvalidRequest);
        }
        let question_bytes = serde_json::to_vec(&question.wire())
            .map_err(|_| JevError::InvalidRequest)?
            .len();
        // UTF-8/JSON bytes are a conservative, bounded token estimate, not a
        // provider tokenizer. The provider can still reject a request.
        if state_bytes.saturating_add(question_bytes) > MAX_ESTIMATED_STATE_AND_QUESTION_TOKENS {
            return Err(JevError::EstimatedContextLimit);
        }
        let id_bytes = serde_json::to_vec(id)
            .map_err(|_| JevError::InvalidRequest)?
            .len();
        let entry_bytes = id_bytes.saturating_add(1).saturating_add(question_bytes);
        if base_bytes.saturating_add(entry_bytes) > byte_limit {
            return Err(JevError::RequestTooLarge);
        }
        let additional = entry_bytes + usize::from(!pending.is_empty());
        if pending_bytes.saturating_add(additional) > byte_limit {
            result.push(make_batch(
                result.len(),
                &request.state,
                std::mem::take(&mut pending),
            )?);
            pending_bytes = base_bytes;
        }
        pending_bytes =
            pending_bytes.saturating_add(entry_bytes + usize::from(!pending.is_empty()));
        pending.insert(id.clone(), question.clone());
    }
    if !pending.is_empty() {
        result.push(make_batch(result.len(), &request.state, pending)?);
    }
    Ok(result)
}

fn make_batch(
    index: usize,
    state: &Value,
    questions: BTreeMap<String, Question>,
) -> Result<PreparedBatch, JevError> {
    let bytes = serde_json::to_vec(&wire_request(state, &questions))
        .map_err(|_| JevError::InvalidRequest)?;
    let question_ids = questions.keys().cloned().collect();
    Ok(PreparedBatch {
        index,
        bytes,
        question_ids,
        questions,
    })
}

fn wire_request(state: &Value, questions: &BTreeMap<String, Question>) -> Value {
    let wire_questions: BTreeMap<_, _> = questions
        .iter()
        .map(|(id, question)| (id, question.wire()))
        .collect();
    json!({"model":MODEL,"state":state,"questions":wire_questions})
}

fn parse_response(
    questions: &BTreeMap<String, Question>,
    response: &Value,
) -> Result<(BTreeMap<String, Answer>, Usage), JevError> {
    if response.get("model").and_then(Value::as_str) != Some(MODEL) {
        return Err(JevError::InvalidResponse);
    }
    let raw_answers = response
        .get("answers")
        .and_then(Value::as_object)
        .ok_or(JevError::InvalidResponse)?;
    let expected: BTreeSet<_> = questions.keys().map(String::as_str).collect();
    let returned: BTreeSet<_> = raw_answers.keys().map(String::as_str).collect();
    if expected != returned {
        return Err(JevError::InvalidResponse);
    }
    let raw_usage = response.get("usage").ok_or(JevError::InvalidResponse)?;
    let usage = Usage {
        input_tokens: raw_usage
            .get("input_tokens")
            .and_then(Value::as_u64)
            .ok_or(JevError::InvalidResponse)?,
        output_tokens: raw_usage
            .get("output_tokens")
            .and_then(Value::as_u64)
            .ok_or(JevError::InvalidResponse)?,
    };
    let mut answers = BTreeMap::new();
    for (id, question) in questions {
        let raw = &raw_answers[id];
        let answer = match question {
            Question::Noul { .. } if raw.get("type").and_then(Value::as_str) == Some("noul") => {
                Answer::Noul {
                    probability: probability(raw.get("noul"))?,
                }
            }
            Question::Choice { criteria, .. }
                if raw.get("type").and_then(Value::as_str) == Some("choice") =>
            {
                let probabilities = distribution(
                    raw.get("probabilities"),
                    criteria.keys().map(String::as_str),
                )?;
                let choice = raw
                    .get("choice")
                    .and_then(Value::as_str)
                    .ok_or(JevError::InvalidResponse)?;
                let selected = probabilities.get(choice).ok_or(JevError::InvalidResponse)?;
                if probabilities
                    .values()
                    .any(|value| *value > *selected + 1e-6)
                {
                    return Err(JevError::InvalidResponse);
                }
                Answer::Choice {
                    choice: choice.into(),
                    probabilities,
                    confidence: probability(raw.get("confidence"))?,
                }
            }
            Question::Score { criteria, .. }
                if raw.get("type").and_then(Value::as_str) == Some("score") =>
            {
                let keys: Vec<String> =
                    (0..criteria.len()).map(|index| index.to_string()).collect();
                let probabilities =
                    distribution(raw.get("probabilities"), keys.iter().map(String::as_str))?;
                let legend = raw
                    .get("legend")
                    .and_then(Value::as_object)
                    .ok_or(JevError::InvalidResponse)?;
                if legend.len() != criteria.len()
                    || keys
                        .iter()
                        .any(|key| legend.get(key).and_then(Value::as_str).is_none())
                {
                    return Err(JevError::InvalidResponse);
                }
                let score = raw
                    .get("score")
                    .and_then(Value::as_f64)
                    .filter(|score| {
                        score.is_finite() && *score >= 0.0 && *score <= (criteria.len() - 1) as f64
                    })
                    .ok_or(JevError::InvalidResponse)?;
                let weighted: f64 = keys
                    .iter()
                    .enumerate()
                    .map(|(index, key)| index as f64 * probabilities[key])
                    .sum();
                if (weighted - score).abs() > 0.05 {
                    return Err(JevError::InvalidResponse);
                }
                Answer::Score {
                    score,
                    probabilities,
                    confidence: probability(raw.get("confidence"))?,
                }
            }
            _ => return Err(JevError::InvalidResponse),
        };
        answers.insert(id.clone(), answer);
    }
    Ok((answers, usage))
}

fn probability(value: Option<&Value>) -> Result<f64, JevError> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
        .ok_or(JevError::InvalidResponse)
}

fn distribution<'a>(
    value: Option<&Value>,
    expected: impl Iterator<Item = &'a str>,
) -> Result<BTreeMap<String, f64>, JevError> {
    let raw = value
        .and_then(Value::as_object)
        .ok_or(JevError::InvalidResponse)?;
    let expected: BTreeSet<_> = expected.collect();
    if raw.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err(JevError::InvalidResponse);
    }
    let probabilities = raw
        .iter()
        .map(|(key, value)| Ok((key.clone(), probability(Some(value))?)))
        .collect::<Result<BTreeMap<_, _>, JevError>>()?;
    let total: f64 = probabilities.values().sum();
    if (total - 1.0).abs() > PROBABILITY_TOLERANCE {
        return Err(JevError::InvalidResponse);
    }
    Ok(probabilities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct FixtureTransport {
        response: Value,
        delay: Duration,
        calls: AtomicUsize,
    }

    impl Transport for FixtureTransport {
        fn post<'a>(
            &'a self,
            _request: Vec<u8>,
        ) -> Pin<Box<dyn Future<Output = Result<Value, JevError>> + Send + 'a>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(self.delay).await;
                Ok(self.response.clone())
            })
        }
    }

    fn mixed_request() -> Request {
        Request {
            state: json!({"task_query":"Find route","candidate":"route handler"}),
            questions: BTreeMap::from([
                (
                    "score".into(),
                    Question::Score {
                        instructions: json!({"question":"Rate `candidate` for `task_query`."}),
                        criteria: vec![
                            json!("none"),
                            json!("weak"),
                            json!("useful"),
                            json!("direct"),
                        ],
                    },
                ),
                (
                    "choice".into(),
                    Question::Choice {
                        instructions: json!("Choose a role."),
                        criteria: BTreeMap::from([
                            ("keep".into(), json!("useful")),
                            ("omit".into(), json!("unrelated")),
                        ]),
                    },
                ),
                (
                    "noul".into(),
                    Question::Noul {
                        instructions: json!("Is candidate unrelated?"),
                        criteria: None,
                    },
                ),
            ]),
        }
    }

    fn mixed_response() -> Value {
        json!({
            "model":MODEL,
            "answers":{
                "score":{"type":"score","score":2.0,"confidence":1.0,
                    "legend":{"0":"none","1":"weak","2":"useful","3":"direct"},
                    "probabilities":{"0":0.0,"1":0.0,"2":1.0,"3":0.0}},
                "choice":{"type":"choice","choice":"keep","confidence":1.0,
                    "probabilities":{"keep":1.0,"omit":0.0}},
                "noul":{"type":"noul","noul":0.9}
            },
            "usage":{"input_tokens":123,"output_tokens":9}
        })
    }

    fn evaluator(response: Value, delay: Duration) -> (BatchEvaluator, Arc<FixtureTransport>) {
        let fixture = Arc::new(FixtureTransport {
            response,
            delay,
            calls: AtomicUsize::new(0),
        });
        (BatchEvaluator::new(fixture.clone()), fixture)
    }

    #[tokio::test]
    async fn mixed_answers_and_usage_are_validated_without_a_noul_confidence() {
        let (evaluator, fixture) = evaluator(mixed_response(), Duration::ZERO);
        let result = evaluator
            .evaluate(mixed_request(), Policy::default(), Cancellation::new())
            .await
            .unwrap();
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
        assert_eq!(result.answers["noul"], Answer::Noul { probability: 0.9 });
        assert_eq!(
            result.usage,
            Usage {
                input_tokens: 123,
                output_tokens: 9
            }
        );
        assert_eq!(result.batches.len(), 1);
    }

    #[tokio::test]
    async fn invalid_score_criteria_and_oversized_inputs_fail_before_transport() {
        let (evaluator, fixture) = evaluator(mixed_response(), Duration::ZERO);
        let mut request = mixed_request();
        request.questions.insert(
            "score".into(),
            Question::Score {
                instructions: json!("rate"),
                criteria: vec![json!("only one")],
            },
        );
        assert_eq!(
            evaluator
                .evaluate(request, Policy::default(), Cancellation::new())
                .await
                .err(),
            Some(JevError::InvalidRequest)
        );
        let mut request = mixed_request();
        request.state = json!("x".repeat(33_000));
        assert_eq!(
            evaluator
                .evaluate(request, Policy::default(), Cancellation::new())
                .await
                .err(),
            Some(JevError::EstimatedContextLimit)
        );
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn malformed_or_incomplete_response_rejects_the_whole_call() {
        for response in [
            {
                let mut value = mixed_response();
                value["model"] = json!("other");
                value
            },
            {
                let mut value = mixed_response();
                value["answers"].as_object_mut().unwrap().remove("noul");
                value
            },
            {
                let mut value = mixed_response();
                value["answers"]["choice"]["probabilities"]["keep"] = json!(0.4);
                value
            },
            {
                let mut value = mixed_response();
                value["answers"]["score"]["score"] = json!(0.0);
                value
            },
            {
                let mut value = mixed_response();
                value["answers"]["noul"]["noul"] = json!(1.2);
                value
            },
        ] {
            let (evaluator, _) = evaluator(response, Duration::ZERO);
            assert_eq!(
                evaluator
                    .evaluate(mixed_request(), Policy::default(), Cancellation::new())
                    .await
                    .err(),
                Some(JevError::InvalidResponse)
            );
        }
    }

    #[tokio::test]
    async fn whole_call_deadline_and_cancellation_include_transport_wait() {
        let (evaluator, _) = evaluator(mixed_response(), Duration::from_millis(100));
        let mut policy = Policy::default();
        policy.deadline = Duration::from_millis(5);
        assert_eq!(
            evaluator
                .evaluate(mixed_request(), policy.clone(), Cancellation::new())
                .await
                .err(),
            Some(JevError::Deadline)
        );
        let cancellation = Cancellation::new();
        cancellation.cancel();
        assert_eq!(
            evaluator
                .evaluate(mixed_request(), policy, cancellation)
                .await
                .err(),
            Some(JevError::Cancelled)
        );
    }

    struct EchoTransport;

    impl Transport for EchoTransport {
        fn post<'a>(
            &'a self,
            request: Vec<u8>,
        ) -> Pin<Box<dyn Future<Output = Result<Value, JevError>> + Send + 'a>> {
            Box::pin(async move {
                let request: Value =
                    serde_json::from_slice(&request).map_err(|_| JevError::InvalidRequest)?;
                let answers = request["questions"]
                    .as_object()
                    .ok_or(JevError::InvalidRequest)?
                    .keys()
                    .map(|id| (id.clone(), json!({"type":"noul","noul":0.8})))
                    .collect::<BTreeMap<_, _>>();
                Ok(json!({"model":MODEL,"answers":answers,
                    "usage":{"input_tokens":1,"output_tokens":1}}))
            })
        }
    }

    #[tokio::test]
    async fn independent_questions_split_at_context_and_byte_bounds() {
        let questions = (0..100)
            .map(|index| {
                (
                    format!("q{index}"),
                    Question::Noul {
                        instructions: json!("x".repeat(900)),
                        criteria: None,
                    },
                )
            })
            .collect();
        let request = Request {
            state: json!("shared"),
            questions,
        };
        let policy = Policy::default();
        let batches = pack_batches(&request, &policy).unwrap();
        assert!(batches.len() > 1);
        assert!(batches
            .iter()
            .all(|batch| batch.bytes.len() <= 64_000 && batch.bytes.len() <= 80_000));
        let result = BatchEvaluator::new(Arc::new(EchoTransport))
            .evaluate(request, policy, Cancellation::new())
            .await
            .unwrap();
        assert_eq!(result.answers.len(), 100);
        assert_eq!(result.usage.input_tokens, result.batches.len() as u64);

        let mut policy = Policy::default();
        policy.max_batch_bytes = 100;
        let request = Request {
            state: json!("shared"),
            questions: BTreeMap::from([(
                "large".into(),
                Question::Noul {
                    instructions: json!("x".repeat(1000)),
                    criteria: None,
                },
            )]),
        };
        assert_eq!(
            pack_batches(&request, &policy).err(),
            Some(JevError::RequestTooLarge)
        );
    }

    #[test]
    fn choice_limit_and_policy_ceiling_are_validated() {
        let criteria = (0..256)
            .map(|index| (index.to_string(), json!("option")))
            .collect();
        let request = Request {
            state: json!("task"),
            questions: BTreeMap::from([(
                "choice".into(),
                Question::Choice {
                    instructions: json!("Choose"),
                    criteria,
                },
            )]),
        };
        assert_eq!(
            pack_batches(&request, &Policy::default()).err(),
            Some(JevError::InvalidRequest)
        );
        let mut policy = Policy::default();
        policy.max_in_flight_requests = 4;
        assert_eq!(
            pack_batches(&mixed_request(), &policy).err(),
            Some(JevError::InvalidRequest)
        );
    }

    #[tokio::test]
    async fn http_pool_reuses_then_expires_idle_connection_without_provider_access() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let accepts = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&accepts);
        let server = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                seen.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut pending = Vec::new();
                    let mut chunk = [0u8; 4096];
                    loop {
                        let header_end = loop {
                            if let Some(offset) =
                                pending.windows(4).position(|window| window == b"\r\n\r\n")
                            {
                                break offset + 4;
                            }
                            let Ok(count) = stream.read(&mut chunk).await else {
                                return;
                            };
                            if count == 0 {
                                return;
                            }
                            pending.extend_from_slice(&chunk[..count]);
                        };
                        let header = String::from_utf8_lossy(&pending[..header_end]);
                        let length = header
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|value| value.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        while pending.len() < header_end + length {
                            let Ok(count) = stream.read(&mut chunk).await else {
                                return;
                            };
                            if count == 0 {
                                return;
                            }
                            pending.extend_from_slice(&chunk[..count]);
                        }
                        pending.drain(..header_end + length);
                        if stream.write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\n{}"
                        ).await.is_err() { return }
                    }
                });
            }
        });
        let transport = HttpTransport::with_test_endpoint(
            "fixture-key".into(),
            Duration::from_millis(30),
            format!("http://{address}"),
        )
        .unwrap();
        transport.post(b"{}".to_vec()).await.unwrap();
        transport.post(b"{}".to_vec()).await.unwrap();
        assert_eq!(accepts.load(Ordering::SeqCst), 1);
        tokio::time::sleep(Duration::from_millis(100)).await;
        transport.post(b"{}".to_vec()).await.unwrap();
        assert!(accepts.load(Ordering::SeqCst) >= 2);
        server.abort();
    }

    #[tokio::test]
    async fn provider_context_rejections_remain_explicit_without_retries() {
        for body in [
            "state plus longest question exceeds context limit",
            "state plus all questions exceeds token limit",
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let response = format!(
                "HTTP/1.1 422 Unprocessable Entity\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request).await.unwrap();
                stream.write_all(response.as_bytes()).await.unwrap();
            });
            let transport = HttpTransport::with_test_endpoint(
                "fixture-key".into(),
                Duration::from_secs(1),
                format!("http://{address}"),
            )
            .unwrap();
            assert_eq!(
                transport.post(b"{}".to_vec()).await.err(),
                Some(JevError::ProviderContextLimit)
            );
            server.await.unwrap();
        }
    }
}
