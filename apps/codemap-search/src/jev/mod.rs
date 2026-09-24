//! Reusable Jev decision runtime: typed Score/Choice/Noul questions over caller-supplied
//! JSON state, evaluated through the TypeSafe HTTP API (`POST /v1/systemone`).
//!
//! The module is independent of MCP dispatch, the index, the workspace, the current
//! directory and the global config loader. Adapters (search task judgments) build an [`EvaluationRequest`] from presentation copies of their own data,
//! hand it to an [`Evaluator`], and recompute their policies from the raw typed answers. A
//! request is sent only when a caller explicitly evaluates one; linking this module never
//! starts network traffic, and the mock evaluator/transport in [`mock`] exercise every
//! consumer without credentials.
//!
//! Contract summary (see `validation/jev-native/01-runtime.md` for the handoff record):
//! - Questions carry a request-local [`QuestionId`]; each one owns or references its own
//!   evidence and never reads sibling instructions or answers. Independent questions sharing
//!   one state are batched together; a dependent second stage is a new request. The state is
//!   caller-owned JSON (string, object or array); the runtime adds nothing to it.
//! - Transport ceilings are the initial safety policy: at most [`MAX_BATCH_BYTES`] encoded
//!   request bytes per batch, at most [`MAX_IN_FLIGHT_REQUESTS`] HTTP requests in flight, at
//!   least [`MIN_REQUEST_SPACING`] between request starts, no automatic retries or redirects,
//!   and a finite whole-call deadline that includes queue time. Hosts may tighten these through
//!   [`EvaluatorConfig`]; callers may shorten a deadline with an absolute instant
//!   ([`RequestPolicy::deadline_at`]), never extend it.
//! - Finite request/response bounds: [`MAX_QUESTIONS_PER_REQUEST`] questions and
//!   [`MAX_BATCHES_PER_REQUEST`] batches per request, [`MAX_RESPONSE_BODY_BYTES`] per
//!   response. Exceeding one is a typed budget failure before dispatch (or after a bounded
//!   read), never a silent truncation of evidence.
//! - Both published Jev 1.13 context limits are checked with a documented byte-based
//!   estimate ([`batch::estimate_tokens`]). The estimate is conservative, not the provider
//!   tokenizer; a provider-side context rejection surfaces as [`JevError::Rejected`] rather
//!   than silently dropping input.
//! - Typed validation (pinned model, exact response-ID set without duplicate keys, answer
//!   types, finite values, probability ranges, distribution sums, Score weighted-sum
//!   consistency, Choice maximum consistency) and transport failures share one failure
//!   contract: [`EvaluationFailure`] carries the error plus every attempted request identity
//!   and the usage known so far.

mod answer;
mod batch;
mod evaluator;
pub mod mock;
mod question;
#[cfg(test)]
mod tests;
mod transport;

pub use answer::{
    distribution_sum_tolerance, score_weighted_sum_tolerance, Answer, ChoiceAnswer,
    EvaluationFailure, EvaluationOutcome, NoulAnswer, RequestIdentity, ScoreAnswer, Timing, Usage,
    CHOICE_TIE_TOLERANCE, PROBABILITY_PRECISION, PROBABILITY_SUM_TOLERANCE, SCORE_TOLERANCE,
};
pub use batch::{estimate_tokens, BatchLimits, TokenLimitKind, ESTIMATED_BYTES_PER_TOKEN};
pub use evaluator::{is_safe_duration, EvaluatorConfig, JevEvaluator, MAX_SAFE_DURATION};
pub use question::{
    EvaluationRequest, NoulCriteria, Question, QuestionId, QuestionKind, RequestPolicy,
    TASK_QUERY_FIELD,
};
pub use transport::{
    HttpsSettings, HttpsTransport, SecretString, Transport, TransportError, TransportResponse,
};

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// The provider model this runtime pins by default. Alias names (`jev-latest`,
/// `jev-preview`) resolve to a concrete version on the provider side, which would fail the
/// exact model validation; configure a concrete version instead.
pub const DEFAULT_MODEL: &str = "jev-1.13.0";

/// The only endpoint this runtime posts to. There is no runtime override: tests inject a
/// [`Transport`] or an [`Evaluator`] instead of redirecting production traffic (a local
/// listener is reachable only from this crate's own `#[cfg(test)]` code).
pub const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";

/// Upper bound of [`EvaluatorConfig::max_in_flight_requests`]: the initial safety policy
/// allows at most three HTTP requests in flight per evaluator.
pub const MAX_IN_FLIGHT_REQUESTS: usize = 3;
/// Lower bound of [`EvaluatorConfig::request_spacing`]: request starts are at least 300 ms
/// apart on one evaluator.
pub const MIN_REQUEST_SPACING: Duration = Duration::from_millis(300);
/// Upper bound of [`EvaluatorConfig::max_batch_bytes`]: one encoded request body never
/// exceeds 80,000 bytes.
pub const MAX_BATCH_BYTES: usize = 80_000;
/// Questions accepted in one [`EvaluationRequest`]. The smallest useful question encodes to
/// roughly 120 bytes, so this bound alone keeps a request under about 1 MB of encoded
/// questions and its answers well under [`MAX_RESPONSE_BODY_BYTES`].
pub const MAX_QUESTIONS_PER_REQUEST: usize = 8_192;
/// Batches produced from one request. At [`MIN_REQUEST_SPACING`] this many batches need at
/// least 38.1 s of dispatch spacing, which already approaches the default 45 s deadline; the
/// bound also caps the encoded bodies held in memory at `128 * MAX_BATCH_BYTES` (10.24 MB).
pub const MAX_BATCHES_PER_REQUEST: usize = 128;
/// Bytes read from one HTTP response body before the read is abandoned. A full 80,000-byte
/// batch answers in well under 1 MB; 4 MiB leaves room for legends and provider extensions
/// while bounding memory at `MAX_IN_FLIGHT_REQUESTS` bodies.
pub const MAX_RESPONSE_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Boxed, sendable future used by the object-safe [`Evaluator`] and [`Transport`] traits.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The reusable decision boundary consumed by adapters. [`JevEvaluator`] is the HTTPS
/// implementation; [`mock::MockEvaluator`] answers offline for tests and examples.
pub trait Evaluator: Send + Sync {
    fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> BoxFuture<'_, Result<EvaluationOutcome, EvaluationFailure>>;
}

impl<E: Evaluator + ?Sized> Evaluator for Arc<E> {
    fn evaluate(
        &self,
        request: EvaluationRequest,
    ) -> BoxFuture<'_, Result<EvaluationOutcome, EvaluationFailure>> {
        (**self).evaluate(request)
    }
}

/// Caller-owned cancellation signal. Cloning shares the same signal; cancelling stops an
/// in-flight evaluation at its next await point and releases its concurrency permits.
#[derive(Clone, Debug, Default)]
pub struct CancelToken {
    inner: Arc<CancelState>,
}

#[derive(Debug, Default)]
struct CancelState {
    is_cancelled: AtomicBool,
    notify: tokio::sync::Notify,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.inner.is_cancelled.store(true, Ordering::Release);
        self.inner.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled.load(Ordering::Acquire)
    }

    /// Resolves once the token is cancelled; never resolves otherwise.
    pub async fn cancelled(&self) {
        // Register before checking the flag so a cancel between the check and the await
        // cannot be missed.
        let notified = self.inner.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.is_cancelled() {
            return;
        }
        notified.await;
    }
}

/// Bounded failure contract shared by typed validation and transport limits. Adapters map
/// every variant to a fallback with the original tool output preserved; the variant name is
/// the diagnostic they expose, never request content or credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JevError {
    /// The shared state is not one of the supported top-level JSON shapes.
    InvalidState(String),
    EmptyQuestions,
    /// More questions than [`MAX_QUESTIONS_PER_REQUEST`] in one request.
    TooManyQuestions {
        count: usize,
        limit: usize,
    },
    /// The request would need more than [`MAX_BATCHES_PER_REQUEST`] batches.
    TooManyBatches {
        count: usize,
        limit: usize,
    },
    DuplicateQuestionId(QuestionId),
    InvalidQuestionId(String),
    InvalidInstructions {
        id: QuestionId,
        reason: String,
    },
    InvalidCriteria {
        id: QuestionId,
        reason: String,
    },
    InvalidConfig(String),
    /// One question alone exceeds the per-batch encoded byte ceiling.
    QuestionTooLarge {
        id: QuestionId,
        request_bytes: usize,
        max_batch_bytes: usize,
    },
    /// The byte-based token estimate for one batch exceeds a published provider limit.
    EstimatedTokenLimit {
        kind: TokenLimitKind,
        batch_index: usize,
        estimated_tokens: u64,
        limit_tokens: u64,
    },
    Cancelled,
    /// The whole call, including queue time, exceeded the effective deadline.
    DeadlineExceeded {
        deadline: std::time::Duration,
    },
    Transport(TransportError),
    Unauthorized {
        status: u16,
    },
    /// HTTP 422: the provider rejected the request (schema or context-limit validation).
    Rejected {
        status: u16,
        excerpt: String,
    },
    RateLimited {
        status: u16,
    },
    Overloaded {
        status: u16,
    },
    Http {
        status: u16,
        excerpt: String,
    },
    ModelMismatch {
        expected: String,
        actual: String,
    },
    InvalidResponse {
        batch_index: usize,
        reason: String,
    },
    /// The answer set of one batch did not match its question set exactly.
    IncompleteAnswers {
        batch_index: usize,
        missing: Vec<QuestionId>,
        /// Response keys that no question in the batch used; kept as raw strings because they
        /// need not be valid question IDs.
        unexpected: Vec<String>,
    },
    InvalidAnswer {
        id: QuestionId,
        reason: String,
    },
    Internal(String),
}

impl JevError {
    /// Stable, secret-free label for diagnostics and status text.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::InvalidState(_) => "invalid_state",
            Self::EmptyQuestions => "empty_questions",
            Self::TooManyQuestions { .. } => "too_many_questions",
            Self::TooManyBatches { .. } => "too_many_batches",
            Self::DuplicateQuestionId(_) => "duplicate_question_id",
            Self::InvalidQuestionId(_) => "invalid_question_id",
            Self::InvalidInstructions { .. } => "invalid_instructions",
            Self::InvalidCriteria { .. } => "invalid_criteria",
            Self::InvalidConfig(_) => "invalid_config",
            Self::QuestionTooLarge { .. } => "question_too_large",
            Self::EstimatedTokenLimit { .. } => "estimated_token_limit",
            Self::Cancelled => "cancelled",
            Self::DeadlineExceeded { .. } => "deadline_exceeded",
            Self::Transport(_) => "transport",
            Self::Unauthorized { .. } => "unauthorized",
            Self::Rejected { .. } => "rejected",
            Self::RateLimited { .. } => "rate_limited",
            Self::Overloaded { .. } => "overloaded",
            Self::Http { .. } => "http_status",
            Self::ModelMismatch { .. } => "model_mismatch",
            Self::InvalidResponse { .. } => "invalid_response",
            Self::IncompleteAnswers { .. } => "incomplete_answers",
            Self::InvalidAnswer { .. } => "invalid_answer",
            Self::Internal(_) => "internal",
        }
    }
}

impl std::fmt::Display for JevError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidState(reason) => write!(f, "invalid state: {reason}"),
            Self::EmptyQuestions => write!(f, "a request needs at least one question"),
            Self::TooManyQuestions { count, limit } => {
                write!(f, "{count} questions exceed the {limit}-question request limit")
            }
            Self::TooManyBatches { count, limit } => {
                write!(f, "{count} batches exceed the {limit}-batch request limit")
            }
            Self::DuplicateQuestionId(id) => write!(f, "duplicate question id '{id}'"),
            Self::InvalidQuestionId(id) => write!(
                f,
                "invalid question id '{}' (1-{} chars of [A-Za-z0-9_.:-])",
                id.chars().take(80).collect::<String>(),
                QuestionId::MAX_LEN
            ),
            Self::InvalidInstructions { id, reason } => {
                write!(f, "question '{id}' has invalid instructions: {reason}")
            }
            Self::InvalidCriteria { id, reason } => {
                write!(f, "question '{id}' has invalid criteria: {reason}")
            }
            Self::InvalidConfig(reason) => write!(f, "invalid evaluator configuration: {reason}"),
            Self::QuestionTooLarge {
                id,
                request_bytes,
                max_batch_bytes,
            } => write!(
                f,
                "question '{id}' alone encodes to {request_bytes} bytes, above the {max_batch_bytes}-byte batch ceiling"
            ),
            Self::EstimatedTokenLimit {
                kind,
                batch_index,
                estimated_tokens,
                limit_tokens,
            } => write!(
                f,
                "batch {batch_index} estimates {estimated_tokens} tokens for {kind}, above the published {limit_tokens}-token limit"
            ),
            Self::Cancelled => write!(f, "evaluation cancelled by the caller"),
            Self::DeadlineExceeded { deadline } => {
                write!(f, "evaluation exceeded its {}ms deadline", deadline.as_millis())
            }
            Self::Transport(error) => write!(f, "transport failure: {error}"),
            Self::Unauthorized { status } => {
                write!(f, "provider rejected the credentials (HTTP {status})")
            }
            Self::Rejected { status, excerpt } => {
                write!(f, "provider rejected the request (HTTP {status}): {excerpt}")
            }
            Self::RateLimited { status } => write!(f, "provider rate limit (HTTP {status})"),
            Self::Overloaded { status } => write!(f, "provider overloaded (HTTP {status})"),
            Self::Http { status, excerpt } => write!(f, "unexpected HTTP {status}: {excerpt}"),
            Self::ModelMismatch { expected, actual } => {
                write!(f, "response model '{actual}' differs from requested '{expected}'")
            }
            Self::InvalidResponse {
                batch_index,
                reason,
            } => write!(f, "batch {batch_index} returned an invalid response: {reason}"),
            Self::IncompleteAnswers {
                batch_index,
                missing,
                unexpected,
            } => write!(
                f,
                "batch {batch_index} answer set mismatch: {} missing, {} unexpected",
                missing.len(),
                unexpected.len()
            ),
            Self::InvalidAnswer { id, reason } => {
                write!(f, "answer for '{id}' is invalid: {reason}")
            }
            Self::Internal(reason) => write!(f, "internal runtime error: {reason}"),
        }
    }
}

impl std::error::Error for JevError {}

impl From<TransportError> for JevError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}
