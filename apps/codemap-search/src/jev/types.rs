use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};
use tokio::time::{Duration, Instant};

pub type DecisionFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(super) fn is_structured(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Object(_) | Value::Array(_))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<BTreeMap<String, Value>>,
    },
}

impl Question {
    pub fn validate(&self) -> Result<(), FailureKind> {
        let valid = match self {
            Self::Score {
                instructions,
                criteria,
            } => {
                is_structured(instructions)
                    && (2..=10).contains(&criteria.len())
                    && criteria.iter().all(is_structured)
            }
            Self::Choice {
                instructions,
                criteria,
            } => {
                is_structured(instructions)
                    && (1..=255).contains(&criteria.len())
                    && criteria
                        .iter()
                        .all(|(k, v)| !k.is_empty() && (v.is_null() || is_structured(v)))
            }
            Self::Noul {
                instructions,
                criteria,
            } => {
                is_structured(instructions)
                    && criteria.as_ref().is_none_or(|c| {
                        c.iter()
                            .all(|(k, v)| (k == "true" || k == "false") && is_structured(v))
                    })
            }
        };
        if valid {
            Ok(())
        } else {
            Err(FailureKind::InvalidInput)
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    Score {
        score: f64,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
        legend: BTreeMap<String, Value>,
    },
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Noul {
        noul: f64,
    },
}

#[derive(Clone)]
pub struct EvaluationRequest {
    pub request_id: String,
    pub state: Value,
    pub questions: BTreeMap<String, Question>,
}

#[derive(Clone)]
pub struct Cancellation(Arc<tokio::sync::watch::Sender<bool>>);

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl Cancellation {
    pub fn new() -> Self {
        Self(Arc::new(tokio::sync::watch::channel(false).0))
    }
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
    pub async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }
}

#[derive(Clone)]
pub struct EvaluationOptions {
    pub deadline: Instant,
    pub cancellation: Cancellation,
}

impl Default for EvaluationOptions {
    fn default() -> Self {
        Self {
            deadline: Instant::now() + Duration::from_millis(45_000),
            cancellation: Cancellation::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}
impl Usage {
    pub fn add(&mut self, other: &Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Metrics {
    pub usage: Usage,
    pub elapsed_ms: u64,
    pub http_elapsed_ms: u64,
    pub requests_started: usize,
    pub requests_completed: usize,
}

impl Metrics {
    pub fn add(&mut self, other: &Self) {
        self.usage.add(&other.usage);
        self.elapsed_ms += other.elapsed_ms;
        self.http_elapsed_ms += other.http_elapsed_ms;
        self.requests_started += other.requests_started;
        self.requests_completed += other.requests_completed;
    }
}

#[derive(Clone)]
pub struct Evaluation {
    pub request_id: String,
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
    pub metrics: Metrics,
}

#[derive(Deserialize)]
pub(super) struct WireResponse {
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
    #[serde(rename = "usage")]
    pub _usage: Usage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    InvalidInput,
    InvalidPolicy,
    InvalidResponse,
    OversizedQuestion,
    ContextEstimate,
    RequestLimit,
    Deadline,
    Cancelled,
    Transport,
    Http(u16),
    ResponseTooLarge,
}

impl std::fmt::Display for FailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for FailureKind {}

#[derive(Debug, Clone)]
pub struct Failure {
    pub kind: FailureKind,
    pub metrics: Metrics,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}
impl std::error::Error for Failure {}
