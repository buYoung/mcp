//! Typed TypeSafe Jev decisions over caller-supplied JSON state.
//!
//! This module owns the wire contract, answer validation, batching, budgets, and the HTTPS
//! transport only. It does not import MCP, index, parser, workspace, or config code:
//! adapters own evidence, question wording, task-intent placement, and selection policy.
//! Linking it sends nothing; a request needs an explicitly constructed evaluator.
mod answer;
mod batch;
mod cancellation;
mod error;
mod evaluator;
mod question;
mod transport;
mod wire;

use std::future::Future;
use std::pin::Pin;

pub use answer::{Answer, ChoiceAnswer, NoulAnswer, ScoreAnswer, PROBABILITY_SUM_TOLERANCE};
pub use batch::{
    estimate_tokens, REQUEST_OVERHEAD_TOKENS, STATE_WITH_LONGEST_QUESTION_TOKEN_LIMIT,
    TOTAL_CONTEXT_TOKEN_LIMIT,
};
pub use cancellation::CancellationToken;
pub use error::{EvaluationFailure, HttpFailureClass, JevError, TokenBudget, TransportErrorKind};
pub use evaluator::{
    is_versioned_model, Evaluation, EvaluationRequest, Evaluator, JevEvaluator, RequestOutcome,
    RequestRecord, Timing, TransportPolicy, Usage, DEFAULT_MAX_BATCH_BYTES,
    DEFAULT_MAX_IN_FLIGHT_REQUESTS, DEFAULT_MODEL, DEFAULT_POOL_IDLE_TIMEOUT,
    DEFAULT_REQUEST_SPACING, DEFAULT_TIMEOUT, MAX_BATCH_BYTES_LIMIT, MAX_IN_FLIGHT_REQUESTS_LIMIT,
    MAX_POOL_IDLE_TIMEOUT, MAX_REQUEST_SPACING, MAX_TIMEOUT, MIN_BATCH_BYTES,
};
pub use question::{
    ChoiceOption, NoulCriteria, Question, QuestionType, MAX_CHOICE_OPTIONS, MAX_QUESTION_ID_BYTES,
    MAX_SCORE_LEVELS, MIN_CHOICE_OPTIONS, MIN_SCORE_LEVELS,
};
pub use transport::{
    ApiKey, HttpsTransport, JevTransport, TransportError, TransportResponse, ENDPOINT,
    MAX_RESPONSE_BYTES,
};

/// A boxed `Send` future, so evaluators and transports stay object-safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[cfg(test)]
mod tests;
