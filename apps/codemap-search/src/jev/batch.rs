//! Independent batching of questions that share one state, bounded by encoded request
//! bytes and by a documented token estimate for both published Jev 1.13 context limits.
//!
//! Token estimation: the provider tokenizer is not published, so the runtime estimates
//! `ceil(bytes / ESTIMATED_BYTES_PER_TOKEN)` over the compact JSON encoding of each part.
//! Three bytes per token is deliberately pessimistic for English prose (roughly four bytes
//! per token) and about right for source code and JSON punctuation; CJK text encodes to
//! three UTF-8 bytes per character and often one token per character, so the estimate is
//! close to exact there. The estimate is a transport bound only: it never proves a batch
//! is within the provider limit, which is why a provider-side rejection stays an explicit
//! [`JevError::Rejected`] instead of a silent truncation.

use super::question::{Question, QuestionId};
use super::JevError;
use serde_json::Value;

/// Documented conservative estimate used for the token-limit checks.
pub const ESTIMATED_BYTES_PER_TOKEN: u64 = 3;

/// Which published context limit an estimate exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenLimitKind {
    /// 64k tokens for the state plus every question in one request.
    Total,
    /// 32k tokens for the state plus the longest single question in one request.
    StateWithLongestQuestion,
}

impl std::fmt::Display for TokenLimitKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Total => "state plus all questions",
            Self::StateWithLongestQuestion => "state plus the longest question",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchLimits {
    pub max_batch_bytes: usize,
    pub max_estimated_tokens: u64,
    pub max_estimated_state_plus_longest_question_tokens: u64,
    /// Batches one request may produce; see [`super::MAX_BATCHES_PER_REQUEST`].
    pub max_batches: usize,
}

impl Default for BatchLimits {
    fn default() -> Self {
        Self {
            max_batch_bytes: super::MAX_BATCH_BYTES,
            max_estimated_tokens: super::DEFAULT_REQUEST_TOKEN_BUDGET,
            max_estimated_state_plus_longest_question_tokens:
                super::DEFAULT_STATE_QUESTION_TOKEN_BUDGET,
            max_batches: super::MAX_BATCHES_PER_REQUEST,
        }
    }
}

/// One encoded request: the exact body that is posted plus the question IDs it carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Batch {
    pub index: usize,
    pub question_ids: Vec<QuestionId>,
    pub body: Vec<u8>,
}

pub fn estimate_tokens(bytes: usize) -> u64 {
    (bytes as u64).div_ceil(ESTIMATED_BYTES_PER_TOKEN)
}

fn encode(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("serde_json::Value always serializes")
}

struct PendingBatch {
    ids: Vec<QuestionId>,
    entries: Vec<(String, Vec<u8>)>,
    entry_bytes: usize,
    longest_question_tokens: u64,
    question_tokens: u64,
}

impl PendingBatch {
    fn new() -> Self {
        Self {
            ids: Vec::new(),
            entries: Vec::new(),
            entry_bytes: 0,
            longest_question_tokens: 0,
            question_tokens: 0,
        }
    }

    /// Exact encoded size of `{"model":..,"state":..,"questions":{...}}` with these entries.
    fn request_bytes(&self, envelope_bytes: usize) -> usize {
        let separators = self.entries.len().saturating_sub(1);
        envelope_bytes + self.entry_bytes + separators
    }
}

fn entry_bytes(id: &QuestionId, encoded_question: &[u8]) -> usize {
    // `"id":` + encoded question object.
    id.as_str().len() + 3 + encoded_question.len()
}

/// Pack `questions` in order into the fewest byte-bounded batches, checking both estimated
/// token limits per batch. Returns the encoded bodies so the dispatcher never re-serializes.
pub(crate) fn pack(
    model: &str,
    state: &Value,
    questions: &[Question],
    limits: &BatchLimits,
) -> Result<Vec<Batch>, JevError> {
    let mut envelope = serde_json::Map::new();
    envelope.insert("model".into(), Value::String(model.into()));
    envelope.insert("state".into(), state.clone());
    envelope.insert("questions".into(), Value::Object(serde_json::Map::new()));
    let envelope_bytes = encode(&Value::Object(envelope)).len();
    let state_tokens = estimate_tokens(encode(state).len());

    let mut batches: Vec<Batch> = Vec::new();
    let mut current = PendingBatch::new();
    let flush = |current: &mut PendingBatch, batches: &mut Vec<Batch>| -> Result<(), JevError> {
        if current.entries.is_empty() {
            return Ok(());
        }
        let index = batches.len();
        if index >= limits.max_batches {
            return Err(JevError::TooManyBatches {
                count: index + 1,
                limit: limits.max_batches,
            });
        }
        let total = state_tokens + current.question_tokens;
        if total > limits.max_estimated_tokens {
            return Err(JevError::EstimatedTokenLimit {
                kind: TokenLimitKind::Total,
                batch_index: index,
                estimated_tokens: total,
                limit_tokens: limits.max_estimated_tokens,
            });
        }
        let with_longest = state_tokens + current.longest_question_tokens;
        if with_longest > limits.max_estimated_state_plus_longest_question_tokens {
            return Err(JevError::EstimatedTokenLimit {
                kind: TokenLimitKind::StateWithLongestQuestion,
                batch_index: index,
                estimated_tokens: with_longest,
                limit_tokens: limits.max_estimated_state_plus_longest_question_tokens,
            });
        }
        let mut body = Vec::with_capacity(current.request_bytes(envelope_bytes));
        // Splice the pre-encoded entries into the envelope's empty `questions` object.
        let prefix = format!(
            "{{\"model\":{},\"questions\":{{",
            serde_json::to_string(model).expect("string serializes")
        );
        body.extend_from_slice(prefix.as_bytes());
        for (position, (id, encoded)) in current.entries.iter().enumerate() {
            if position > 0 {
                body.push(b',');
            }
            body.extend_from_slice(
                serde_json::to_string(id)
                    .expect("string serializes")
                    .as_bytes(),
            );
            body.push(b':');
            body.extend_from_slice(encoded);
        }
        body.extend_from_slice(b"},\"state\":");
        body.extend_from_slice(&encode(state));
        body.push(b'}');
        let batch = Batch {
            index,
            question_ids: std::mem::take(&mut current.ids),
            body,
        };
        *current = PendingBatch::new();
        batches.push(batch);
        Ok(())
    };

    for question in questions {
        let encoded = encode(&question.to_wire());
        let added = entry_bytes(question.id(), &encoded);
        let alone = envelope_bytes + added;
        if alone > limits.max_batch_bytes {
            return Err(JevError::QuestionTooLarge {
                id: question.id().clone(),
                request_bytes: alone,
                max_batch_bytes: limits.max_batch_bytes,
            });
        }
        let question_tokens = estimate_tokens(encoded.len());
        let with_question = current.request_bytes(envelope_bytes)
            + added
            + usize::from(!current.entries.is_empty());
        let exceeds_tokens = state_tokens + current.question_tokens + question_tokens
            > limits.max_estimated_tokens
            || state_tokens + current.longest_question_tokens.max(question_tokens)
                > limits.max_estimated_state_plus_longest_question_tokens;
        if (with_question > limits.max_batch_bytes || exceeds_tokens) && !current.entries.is_empty()
        {
            flush(&mut current, &mut batches)?;
        }
        current.ids.push(question.id().clone());
        current
            .entries
            .push((question.id().as_str().to_string(), encoded));
        current.entry_bytes += added;
        current.question_tokens += question_tokens;
        current.longest_question_tokens = current.longest_question_tokens.max(question_tokens);
    }
    flush(&mut current, &mut batches)?;
    Ok(batches)
}

/// Offline preflight uses the same serializer and limits as actual dispatch.
pub(crate) fn request_batch_count(
    request: &super::EvaluationRequest,
    model: &str,
    limits: &BatchLimits,
) -> Result<usize, JevError> {
    pack(model, request.state(), request.questions(), limits).map(|batches| batches.len())
}
