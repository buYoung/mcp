//! Explicit-input, bounded decisions. This module has no tool, index, or config dependency.
mod transport;
mod types;
pub use transport::HttpsTransport;
pub use types::*;

use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;
use tokio::time::{Duration, Instant};

pub const MODEL: &str = "jev-1.13.0";
pub const MAX_BATCH_BYTES: usize = 80_000;
const MAX_QUESTIONS: usize = 16_384;
const MAX_BATCHES: usize = 512;
const DISTRIBUTION_TOLERANCE: f64 = 0.001;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    pub timeout_ms: u64,
    pub max_in_flight_requests: usize,
    pub request_spacing_ms: u64,
    pub max_batch_bytes: usize,
    pub pool_idle_timeout_ms: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            timeout_ms: 45_000,
            max_in_flight_requests: 3,
            request_spacing_ms: 300,
            max_batch_bytes: MAX_BATCH_BYTES,
            pool_idle_timeout_ms: 30_000,
        }
    }
}

impl Policy {
    pub fn validate(&self) -> Result<(), FailureKind> {
        if self.timeout_ms == 0
            || self.timeout_ms > 45_000
            || !(1..=3).contains(&self.max_in_flight_requests)
            || self.request_spacing_ms < 300
            || self.request_spacing_ms > 45_000
            || !(1024..=MAX_BATCH_BYTES).contains(&self.max_batch_bytes)
            || self.pool_idle_timeout_ms == 0
            || self.pool_idle_timeout_ms > 30_000
        {
            return Err(FailureKind::InvalidPolicy);
        }
        Ok(())
    }
}

/// A host can inject a transport without adding a production endpoint override.
pub trait Transport: Send + Sync {
    fn send(&self, body: Vec<u8>) -> DecisionFuture<'_, Result<Vec<u8>, FailureKind>>;
}

pub trait Evaluator: Send + Sync {
    fn evaluate(
        &self,
        request: EvaluationRequest,
        options: EvaluationOptions,
    ) -> DecisionFuture<'_, Result<Evaluation, Failure>>;
}

pub struct Runtime {
    transport: Arc<dyn Transport>,
    policy: Policy,
    permits: Arc<Semaphore>,
    next_dispatch: Arc<tokio::sync::Mutex<Instant>>,
}

impl Runtime {
    pub fn new(transport: Arc<dyn Transport>, policy: Policy) -> Result<Self, FailureKind> {
        policy.validate()?;
        Ok(Self {
            transport,
            permits: Arc::new(Semaphore::new(policy.max_in_flight_requests)),
            next_dispatch: Arc::new(tokio::sync::Mutex::new(Instant::now())),
            policy,
        })
    }

    async fn run(
        &self,
        request: EvaluationRequest,
        options: EvaluationOptions,
    ) -> Result<Evaluation, Failure> {
        let started = Instant::now();
        let deadline = options
            .deadline
            .min(started + Duration::from_millis(self.policy.timeout_ms));
        let metrics = Arc::new(Mutex::new(Metrics::default()));
        let result = async {
            let batches = pack(&request, self.policy.max_batch_bytes)?;
            let mut pending = batches.into_iter();
            let mut tasks = tokio::task::JoinSet::new();
            let mut answers = std::collections::BTreeMap::new();
            loop {
                while tasks.len() < self.policy.max_in_flight_requests {
                    let Some(batch) = pending.next() else { break };
                    let transport = self.transport.clone();
                    let permits = self.permits.clone();
                    let next_dispatch = self.next_dispatch.clone();
                    let metrics = metrics.clone();
                    let spacing = Duration::from_millis(self.policy.request_spacing_ms);
                    tasks.spawn(async move {
                        let _permit = permits
                            .acquire()
                            .await
                            .map_err(|_| FailureKind::Cancelled)?;
                        // Hold only the scheduling lock while waiting, never an index/config lock.
                        let mut next = next_dispatch.lock().await;
                        tokio::time::sleep_until(*next).await;
                        *next = Instant::now() + spacing;
                        drop(next);
                        let http_started = Instant::now();
                        metrics.lock().unwrap().requests_started += 1;
                        let response = transport.send(batch.bytes).await;
                        {
                            let mut counts = metrics.lock().unwrap();
                            counts.http_elapsed_ms += http_started.elapsed().as_millis() as u64;
                            counts.requests_completed += 1;
                        }
                        let bytes = response?;
                        let raw: serde_json::Value = serde_json::from_slice(&bytes)
                            .map_err(|_| FailureKind::InvalidResponse)?;
                        if let Some(usage) = raw
                            .get("usage")
                            .and_then(|v| serde_json::from_value::<Usage>(v.clone()).ok())
                        {
                            metrics.lock().unwrap().usage.add(&usage);
                        }
                        validate_response(&batch.questions, raw)
                    });
                }
                let Some(result) = tasks.join_next().await else {
                    break;
                };
                answers.extend(result.map_err(|_| FailureKind::Transport)??);
            }
            Ok(answers)
        };
        let result = tokio::select! {
            biased;
            _ = options.cancellation.cancelled() => Err(FailureKind::Cancelled),
            _ = tokio::time::sleep_until(deadline) => Err(FailureKind::Deadline),
            result = result => result,
        };
        let mut metrics = metrics.lock().unwrap().clone();
        metrics.elapsed_ms = started.elapsed().as_millis() as u64;
        match result {
            Ok(answers) => Ok(Evaluation {
                request_id: request.request_id,
                model: MODEL.into(),
                answers,
                metrics,
            }),
            Err(kind) => Err(Failure { kind, metrics }),
        }
    }
}

impl Evaluator for Runtime {
    fn evaluate(
        &self,
        request: EvaluationRequest,
        options: EvaluationOptions,
    ) -> DecisionFuture<'_, Result<Evaluation, Failure>> {
        Box::pin(self.run(request, options))
    }
}

struct Batch {
    bytes: Vec<u8>,
    questions: std::collections::BTreeMap<String, Question>,
}

fn encoded(
    state: &serde_json::Value,
    questions: &std::collections::BTreeMap<String, Question>,
) -> Result<Vec<u8>, FailureKind> {
    serde_json::to_vec(&serde_json::json!({"model": MODEL, "state": state, "questions": questions}))
        .map_err(|_| FailureKind::InvalidInput)
}

fn pack(request: &EvaluationRequest, cap: usize) -> Result<Vec<Batch>, FailureKind> {
    use std::collections::BTreeMap;
    if !is_structured(&request.state)
        || request.request_id.is_empty()
        || request.request_id.len() > 128
        || request.questions.is_empty()
        || request.questions.len() > MAX_QUESTIONS
    {
        return Err(FailureKind::InvalidInput);
    }
    let state_bytes = serde_json::to_vec(&request.state)
        .map_err(|_| FailureKind::InvalidInput)?
        .len();
    let mut batches = Vec::new();
    let mut current = BTreeMap::new();
    let envelope_bytes = encoded(&request.state, &current)?.len();
    let mut current_bytes = envelope_bytes;
    for (id, question) in &request.questions {
        if id.is_empty() || id.len() > 128 {
            return Err(FailureKind::InvalidInput);
        }
        question.validate()?;
        let question_bytes = serde_json::to_vec(question)
            .map_err(|_| FailureKind::InvalidInput)?
            .len();
        // Conservative byte-as-token estimate plus framing reserve, not the provider tokenizer.
        // Both provider limits can still fail; never truncate evidence to make a question fit.
        if state_bytes
            .saturating_add(question_bytes)
            .saturating_add(1024)
            > 32_000
        {
            return Err(FailureKind::ContextEstimate);
        }
        let entry_bytes = serde_json::to_vec(id)
            .map_err(|_| FailureKind::InvalidInput)?
            .len()
            + 1
            + question_bytes;
        let next_bytes = current_bytes
            .saturating_add(entry_bytes)
            .saturating_add(usize::from(!current.is_empty()));
        if next_bytes > cap || next_bytes.saturating_add(1024) > 64_000 {
            if current.is_empty() {
                return Err(FailureKind::OversizedQuestion);
            }
            batches.push(Batch {
                bytes: encoded(&request.state, &current)?,
                questions: current,
            });
            if batches.len() >= MAX_BATCHES {
                return Err(FailureKind::RequestLimit);
            }
            current = BTreeMap::new();
            current_bytes = envelope_bytes;
            if envelope_bytes.saturating_add(entry_bytes) > cap {
                return Err(FailureKind::OversizedQuestion);
            }
        }
        current_bytes += entry_bytes + usize::from(!current.is_empty());
        current.insert(id.clone(), question.clone());
    }
    if !current.is_empty() {
        batches.push(Batch {
            bytes: encoded(&request.state, &current)?,
            questions: current,
        });
    }
    Ok(batches)
}

fn probability(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

pub fn validate_response(
    questions: &std::collections::BTreeMap<String, Question>,
    raw: serde_json::Value,
) -> Result<std::collections::BTreeMap<String, Answer>, FailureKind> {
    let response: WireResponse =
        serde_json::from_value(raw).map_err(|_| FailureKind::InvalidResponse)?;
    if response.model != MODEL || response.answers.keys().ne(questions.keys()) {
        return Err(FailureKind::InvalidResponse);
    }
    for (id, question) in questions {
        let invalid = || FailureKind::InvalidResponse;
        match (question, &response.answers[id]) {
            (Question::Noul { .. }, Answer::Noul { noul }) if probability(*noul) => {}
            (
                Question::Score { criteria, .. },
                Answer::Score {
                    score,
                    probabilities,
                    confidence,
                    legend,
                },
            ) => {
                let keys: Vec<_> = (0..criteria.len()).map(|n| n.to_string()).collect();
                validate_distribution(probabilities, keys.iter(), *confidence)?;
                if legend.keys().ne(keys.iter())
                    || !score.is_finite()
                    || *score < 0.0
                    || *score > (criteria.len() - 1) as f64
                {
                    return Err(invalid());
                }
                let weighted: f64 = probabilities
                    .iter()
                    .map(|(key, p)| key.parse::<f64>().unwrap_or_default() * p)
                    .sum();
                if (weighted - score).abs() > DISTRIBUTION_TOLERANCE {
                    return Err(invalid());
                }
            }
            (
                Question::Choice { criteria, .. },
                Answer::Choice {
                    choice,
                    probabilities,
                    confidence,
                },
            ) => {
                validate_distribution(probabilities, criteria.keys(), *confidence)?;
                let selected = probabilities.get(choice).ok_or_else(invalid)?;
                if probabilities
                    .values()
                    .any(|p| p > &(selected + DISTRIBUTION_TOLERANCE))
                {
                    return Err(invalid());
                }
            }
            _ => return Err(invalid()),
        }
    }
    Ok(response.answers)
}

fn validate_distribution<'a>(
    values: &std::collections::BTreeMap<String, f64>,
    keys: impl Iterator<Item = &'a String>,
    confidence: f64,
) -> Result<(), FailureKind> {
    if values.keys().ne(keys)
        || !probability(confidence)
        || !values.values().all(|v| probability(*v))
        || (values.values().sum::<f64>() - 1.0).abs() > DISTRIBUTION_TOLERANCE
    {
        return Err(FailureKind::InvalidResponse);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
