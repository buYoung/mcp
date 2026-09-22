//! Reusable, request-local Jev decisions. No MCP, index, configuration or credentials live here.
//! Questions in a batch share state, but never see each other's instructions or answers.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{watch, Mutex, Semaphore};

#[cfg(test)]
mod tests;

pub const MODEL: &str = "jev-1.13.0";
pub const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    Score { instructions: Value, criteria: Vec<Value> },
    Choice { instructions: Value, criteria: BTreeMap<String, Value> },
    Noul { instructions: Value, #[serde(skip_serializing_if = "Option::is_none")] criteria: Option<NoulCriteria> },
}

#[derive(Clone, Serialize)]
pub struct NoulCriteria {
    #[serde(rename = "true")]
    pub yes: Value,
    #[serde(rename = "false")]
    pub no: Value,
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    Score { score: f64, legend: BTreeMap<String, Value>, probabilities: BTreeMap<String, f64>, confidence: f64 },
    Choice { choice: String, probabilities: BTreeMap<String, f64>, confidence: f64 },
    Noul { noul: f64 },
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone, Copy)]
pub struct Policy {
    pub deadline: Duration,
    pub max_batch_bytes: usize,
    pub max_in_flight_requests: usize,
    pub request_spacing: Duration,
    pub pool_idle_timeout: Duration,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            deadline: Duration::from_millis(45_000),
            max_batch_bytes: 80_000,
            max_in_flight_requests: 3,
            request_spacing: Duration::from_millis(300),
            pool_idle_timeout: Duration::from_millis(30_000),
        }
    }
}

// Intentionally no Debug on request/state, transport bytes, or provider response: source and
// credentials must not accidentally appear in diagnostics.
pub struct EvaluationRequest {
    pub task_query: String,
    pub state: Value,
    pub questions: BTreeMap<String, Question>,
    pub policy: Policy,
    pub cancellation: Option<watch::Receiver<bool>>,
}

pub struct Evaluation {
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
    pub usage: Usage,
    pub request_ids: Vec<String>,
    pub http_elapsed_ms: u128,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    InvalidInput,
    InputLimit,
    Deadline,
    Cancelled,
    Transport,
    ProviderStatus(u16),
    InvalidResponse,
}
#[derive(Debug)]
pub struct Failure {
    pub kind: FailureKind,
    pub completed_batches: usize,
    pub usage: Usage,
    pub elapsed_ms: u128,
    pub http_elapsed_ms: u128,
}

pub struct TransportResponse {
    pub status: u16,
    pub body: Vec<u8>,
}
pub trait Transport: Send + Sync {
    fn post<'a>(&'a self, body: Vec<u8>, timeout: Duration) -> Pin<Box<dyn Future<Output = Result<TransportResponse, FailureKind>> + Send + 'a>>;
}

pub struct HttpsTransport {
    client: reqwest::Client,
    api_key: String,
}
impl HttpsTransport {
    pub fn new(api_key: String, idle_timeout: Duration) -> Result<Self, FailureKind> {
        let client = reqwest::Client::builder()
            .pool_idle_timeout(idle_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build().map_err(|_| FailureKind::Transport)?;
        Ok(Self { client, api_key })
    }
}
impl Transport for HttpsTransport {
    fn post<'a>(&'a self, body: Vec<u8>, timeout: Duration) -> Pin<Box<dyn Future<Output = Result<TransportResponse, FailureKind>> + Send + 'a>> {
        Box::pin(async move {
            let response = self.client.post(ENDPOINT)
                .bearer_auth(&self.api_key)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .timeout(timeout)
                .body(body)
                .send().await.map_err(|_| FailureKind::Transport)?;
            let status = response.status().as_u16();
            let body = response.bytes().await.map_err(|_| FailureKind::Transport)?;
            if body.len() > 4 * 1024 * 1024 { return Err(FailureKind::InvalidResponse); }
            Ok(TransportResponse { status, body: body.to_vec() })
        })
    }
}

pub trait Evaluator: Send + Sync {
    fn evaluate<'a>(&'a self, request: EvaluationRequest) -> Pin<Box<dyn Future<Output = Result<Evaluation, Failure>> + Send + 'a>>;
}

pub struct JevEvaluator {
    transport: Arc<dyn Transport>,
    permits: Semaphore,
    next_request: Mutex<Instant>,
}
impl JevEvaluator {
    pub fn new(transport: Arc<dyn Transport>, max_in_flight_requests: usize) -> Self {
        Self {
            transport,
            permits: Semaphore::new(max_in_flight_requests.clamp(1, 3)),
            next_request: Mutex::new(Instant::now()),
        }
    }
}

fn is_structured(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Object(_) | Value::Array(_))
}
fn valid_question(question: &Question) -> bool {
    match question {
        Question::Score { instructions, criteria } => is_structured(instructions)
            && (2..=10).contains(&criteria.len()) && criteria.iter().all(is_structured),
        Question::Choice { instructions, criteria } => is_structured(instructions)
            && (2..=255).contains(&criteria.len()) && criteria.iter().all(|(key, value)| !key.is_empty() && (is_structured(value) || value.is_null())),
        Question::Noul { instructions, criteria } => is_structured(instructions) && criteria.as_ref().is_none_or(|criteria| is_structured(&criteria.yes) && is_structured(&criteria.no)),
    }
}
fn probability(value: f64) -> bool { value.is_finite() && (0.0..=1.0).contains(&value) }
fn distribution(probabilities: &BTreeMap<String, f64>, keys: &BTreeSet<String>) -> bool {
    probabilities.keys().cloned().collect::<BTreeSet<_>>() == *keys
        && probabilities.values().all(|&value| probability(value))
        && (probabilities.values().sum::<f64>() - 1.0).abs() <= 0.03
}
fn validate_answer(question: &Question, answer: &Answer) -> bool {
    match (question, answer) {
        (Question::Noul { .. }, Answer::Noul { noul }) => probability(*noul),
        (Question::Choice { criteria, .. }, Answer::Choice { choice, probabilities, confidence }) => {
            probability(*confidence) && criteria.contains_key(choice)
                && distribution(probabilities, &criteria.keys().cloned().collect())
                && probabilities[choice] + 0.01 >= probabilities.values().copied().fold(0.0, f64::max)
        }
        (Question::Score { criteria, .. }, Answer::Score { score, legend, probabilities, confidence }) => {
            let keys = (0..criteria.len()).map(|index| index.to_string()).collect();
            score.is_finite() && *score >= 0.0 && *score <= (criteria.len() - 1) as f64
                && probability(*confidence) && legend.keys().cloned().collect::<BTreeSet<_>>() == keys
                && criteria.iter().enumerate().all(|(index, level)| {
                    !level.is_string() || legend.get(&index.to_string()) == Some(level)
                })
                && distribution(probabilities, &keys)
                && (probabilities.iter().map(|(key, p)| key.parse::<f64>().unwrap_or(0.0) * p).sum::<f64>() - score).abs() <= 0.06
        }
        _ => false,
    }
}
#[derive(Deserialize)]
struct WireResponse { model: String, answers: BTreeMap<String, Answer>, usage: Usage }

impl Evaluator for JevEvaluator {
    fn evaluate<'a>(&'a self, request: EvaluationRequest) -> Pin<Box<dyn Future<Output = Result<Evaluation, Failure>> + Send + 'a>> {
        Box::pin(async move {
            let started = Instant::now();
            let http_elapsed_ms = std::sync::atomic::AtomicU64::new(0);
            let mut usage = Usage::default();
            let mut completed = 0;
            let fail = |kind, completed_batches, usage| Failure { kind, completed_batches, usage,
                elapsed_ms: started.elapsed().as_millis(),
                http_elapsed_ms: u128::from(http_elapsed_ms.load(std::sync::atomic::Ordering::Relaxed)) };
            let EvaluationRequest { task_query, state, questions, policy, mut cancellation } = request;
            if task_query.trim().is_empty() || !is_structured(&state) || questions.is_empty()
                || policy.deadline.is_zero() || policy.max_batch_bytes == 0
                || policy.max_in_flight_requests == 0 || policy.max_in_flight_requests > 3
                || questions.iter().any(|(id, question)| id.is_empty() || !valid_question(question)) {
                return Err(fail(FailureKind::InvalidInput, 0, usage));
            }
            let state = json!({ "task_query": task_query, "evidence": state });
            let state_bytes = serde_json::to_vec(&state).map_err(|_| fail(FailureKind::InvalidInput, 0, usage))?;
            // Byte lengths are a conservative bound on token count for text, NOT a provider tokenizer.
            // The provider may still reject a batch; never truncate input or silently skip a question.
            if state_bytes.len() >= 32 * 1024 { return Err(fail(FailureKind::InputLimit, 0, usage)); }
            let mut batches: Vec<BTreeMap<String, Question>> = Vec::new();
            let mut batch = BTreeMap::new();
            for (id, question) in questions {
                let mut proposed = batch.clone();
                proposed.insert(id.clone(), question.clone());
                let bytes = serde_json::to_vec(&json!({"state":state,"model":MODEL,"questions":proposed})).map_err(|_| fail(FailureKind::InvalidInput, 0, usage))?;
                let questions_bytes = serde_json::to_vec(&proposed).map_err(|_| fail(FailureKind::InvalidInput, 0, usage))?;
                // 64k total and 32k state+longest question, guarded by UTF-8 byte counts.
                let single_bytes = serde_json::to_vec(&question).map_err(|_| fail(FailureKind::InvalidInput, 0, usage))?;
                if state_bytes.len() + single_bytes.len() > 32 * 1024 {
                    return Err(fail(FailureKind::InputLimit, 0, usage));
                }
                if bytes.len() > policy.max_batch_bytes.min(80_000) || state_bytes.len() + questions_bytes.len() > 64 * 1024 {
                    if batch.is_empty() { return Err(fail(FailureKind::InputLimit, 0, usage)); }
                    batches.push(std::mem::take(&mut batch));
                }
                batch.insert(id, question);
            }
            if !batch.is_empty() { batches.push(batch); }
            let mut answers = BTreeMap::new();
            let mut request_ids = Vec::new();
            let deadline = started + policy.deadline;
            for (index, batch) in batches.into_iter().enumerate() {
                let body = serde_json::to_vec(&json!({"state":state,"model":MODEL,"questions":batch})).map_err(|_| fail(FailureKind::InvalidInput, completed, usage))?;
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() { return Err(fail(FailureKind::Deadline, completed, usage)); }
                let work = async {
                    let _permit = self.permits.acquire().await.map_err(|_| FailureKind::Transport)?;
                    let mut next = self.next_request.lock().await;
                    tokio::time::sleep_until(tokio::time::Instant::from_std(*next)).await;
                    *next = Instant::now() + policy.request_spacing;
                    drop(next);
                    let http_started = Instant::now();
                    let response = self.transport.post(body, deadline.saturating_duration_since(Instant::now())).await?;
                    Ok::<_, FailureKind>((response, http_started.elapsed().as_millis()))
                };
                let run = async {
                    match cancellation.as_mut() {
                        Some(signal) => tokio::select! {
                            result = work => result,
                            _ = async { if !*signal.borrow() { let _ = signal.changed().await; } } => Err(FailureKind::Cancelled),
                        },
                        None => work.await,
                    }
                };
                let (response, http_ms) = tokio::time::timeout(remaining, run).await
                    .map_err(|_| fail(FailureKind::Deadline, completed, usage))?
                    .map_err(|kind| fail(kind, completed, usage))?;
                http_elapsed_ms.fetch_add(http_ms as u64, std::sync::atomic::Ordering::Relaxed);
                if !(200..300).contains(&response.status) {
                    return Err(fail(FailureKind::ProviderStatus(response.status), completed, usage));
                }
                let wire: WireResponse = serde_json::from_slice(&response.body)
                    .map_err(|_| fail(FailureKind::InvalidResponse, completed, usage))?;
                if wire.model != MODEL || wire.answers.keys().collect::<Vec<_>>() != batch.keys().collect::<Vec<_>>()
                    || !wire.answers.iter().all(|(id, answer)| validate_answer(&batch[id], answer)) {
                    return Err(fail(FailureKind::InvalidResponse, completed, usage));
                }
                usage.input_tokens += wire.usage.input_tokens;
                usage.output_tokens += wire.usage.output_tokens;
                answers.extend(wire.answers);
                completed += 1;
                request_ids.push(format!("jev-batch-{}", index + 1));
            }
            Ok(Evaluation { model: MODEL.into(), answers, usage, request_ids,
                http_elapsed_ms: u128::from(http_elapsed_ms.load(std::sync::atomic::Ordering::Relaxed)),
                elapsed_ms: started.elapsed().as_millis() })
        })
    }
}
