use std::fmt;
use std::time::Duration;

use super::evaluator::{RequestOutcome, RequestRecord, Timing, Usage};

/// Which published Jev context budget a local estimate exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenBudget {
    /// State plus every question in one request (64k tokens for Jev 1.13).
    TotalRequest,
    /// State plus the single longest question (32k tokens for Jev 1.13).
    StateWithLongestQuestion,
}

/// Coarse class of a non-success provider status. Response bodies are never retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpFailureClass {
    Unauthorized,
    InvalidRequest,
    RateLimited,
    Overloaded,
    Server,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportErrorKind {
    Connect,
    Timeout,
    ResponseTooLarge,
    Other,
}

/// The single failure contract shared by question validation, transport budgets, and
/// response validation. Messages carry counts, identifiers, and statuses only: never
/// credentials, state, instructions, or provider response bodies.
#[derive(Clone, Debug, PartialEq)]
pub enum JevError {
    InvalidPolicy {
        reason: String,
    },
    InvalidCredential {
        reason: &'static str,
    },
    InvalidQuestion {
        question_id: String,
        reason: String,
    },
    InvalidRequest {
        reason: String,
    },
    RequestTooLarge {
        question_id: String,
        encoded_bytes: usize,
        max_batch_bytes: usize,
    },
    TokenBudgetExceeded {
        question_id: String,
        budget: TokenBudget,
        estimated_tokens: usize,
        limit_tokens: usize,
    },
    ProviderContextLimit {
        status: u16,
    },
    HttpStatus {
        status: u16,
        class: HttpFailureClass,
    },
    Transport {
        kind: TransportErrorKind,
        detail: String,
    },
    DeadlineExceeded {
        elapsed: Duration,
    },
    Cancelled,
    MalformedResponse {
        reason: String,
    },
    ModelMismatch {
        expected: String,
        actual: String,
    },
    AnswerSetMismatch {
        missing_count: usize,
        unexpected_count: usize,
    },
    InvalidAnswer {
        question_id: String,
        reason: String,
    },
}

impl JevError {
    /// Stable snake_case label for status lines and structured logs.
    pub fn label(&self) -> &'static str {
        match self {
            JevError::InvalidPolicy { .. } => "invalid_policy",
            JevError::InvalidCredential { .. } => "invalid_credential",
            JevError::InvalidQuestion { .. } => "invalid_question",
            JevError::InvalidRequest { .. } => "invalid_request",
            JevError::RequestTooLarge { .. } => "request_too_large",
            JevError::TokenBudgetExceeded { .. } => "token_budget_exceeded",
            JevError::ProviderContextLimit { .. } => "provider_context_limit",
            JevError::HttpStatus { .. } => "http_status",
            JevError::Transport { .. } => "transport",
            JevError::DeadlineExceeded { .. } => "deadline_exceeded",
            JevError::Cancelled => "cancelled",
            JevError::MalformedResponse { .. } => "malformed_response",
            JevError::ModelMismatch { .. } => "model_mismatch",
            JevError::AnswerSetMismatch { .. } => "answer_set_mismatch",
            JevError::InvalidAnswer { .. } => "invalid_answer",
        }
    }
}

impl fmt::Display for JevError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JevError::InvalidPolicy { reason } => write!(formatter, "invalid Jev policy: {reason}"),
            JevError::InvalidCredential { reason } => {
                write!(formatter, "invalid Jev credential: {reason}")
            }
            JevError::InvalidQuestion {
                question_id,
                reason,
            } => write!(formatter, "invalid Jev question `{question_id}`: {reason}"),
            JevError::InvalidRequest { reason } => write!(formatter, "invalid Jev request: {reason}"),
            JevError::RequestTooLarge {
                question_id,
                encoded_bytes,
                max_batch_bytes,
            } => write!(
                formatter,
                "Jev question `{question_id}` needs a {encoded_bytes}-byte request; the batch ceiling is {max_batch_bytes} bytes"
            ),
            JevError::TokenBudgetExceeded {
                question_id,
                budget,
                estimated_tokens,
                limit_tokens,
            } => {
                let budget = match budget {
                    TokenBudget::TotalRequest => "state plus all questions",
                    TokenBudget::StateWithLongestQuestion => "state plus the longest question",
                };
                write!(
                    formatter,
                    "Jev question `{question_id}` is estimated at {estimated_tokens} tokens for {budget}; the limit is {limit_tokens}"
                )
            }
            JevError::ProviderContextLimit { status } => write!(
                formatter,
                "Jev provider rejected the request context size (HTTP {status})"
            ),
            JevError::HttpStatus { status, class } => {
                write!(formatter, "Jev provider returned HTTP {status} ({class:?})")
            }
            JevError::Transport { kind, detail } => {
                write!(formatter, "Jev transport failed ({kind:?}): {detail}")
            }
            JevError::DeadlineExceeded { elapsed } => write!(
                formatter,
                "Jev evaluation exceeded its deadline after {}ms",
                elapsed.as_millis()
            ),
            JevError::Cancelled => write!(formatter, "Jev evaluation was cancelled"),
            JevError::MalformedResponse { reason } => {
                write!(formatter, "malformed Jev response: {reason}")
            }
            JevError::ModelMismatch { expected, actual } => write!(
                formatter,
                "Jev response model `{actual}` does not match the pinned model `{expected}`"
            ),
            JevError::AnswerSetMismatch {
                missing_count,
                unexpected_count,
            } => write!(
                formatter,
                "Jev response answered the wrong question set ({missing_count} missing, {unexpected_count} unexpected)"
            ),
            JevError::InvalidAnswer {
                question_id,
                reason,
            } => write!(formatter, "invalid Jev answer for `{question_id}`: {reason}"),
        }
    }
}

impl std::error::Error for JevError {}

/// A failed evaluation with the usage and request records gathered before it stopped.
/// Usage counts only provider responses that reported it, including rejected answers.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationFailure {
    pub error: JevError,
    pub usage: Usage,
    pub timing: Timing,
    pub requests: Vec<RequestRecord>,
}

impl EvaluationFailure {
    /// HTTP exchanges that finished before the failure; dropped or unsent bodies excluded.
    pub fn completed_request_count(&self) -> usize {
        self.requests
            .iter()
            .filter(|record| record.outcome != RequestOutcome::NotCompleted)
            .count()
    }
}

impl fmt::Display for EvaluationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for EvaluationFailure {}
