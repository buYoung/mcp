use serde_json::Value;
use sha2::{Digest, Sha256};

use super::error::TokenBudget;
use super::question::Question;
use super::JevError;

/// Jev 1.13 budget for state plus all questions in one request.
pub const TOTAL_CONTEXT_TOKEN_LIMIT: usize = 64_000;
/// Jev 1.13 budget for state plus the single longest question.
pub const STATE_WITH_LONGEST_QUESTION_TOKEN_LIMIT: usize = 32_000;
/// Fixed per-request allowance in the local estimate; tiny historical requests reported
/// about 300 input tokens.
pub const REQUEST_OVERHEAD_TOKENS: usize = 512;

/// Conservative token estimate for encoded request text. No provider tokenizer is
/// published, so this counts two ASCII bytes or half a non-ASCII character per token
/// plus `REQUEST_OVERHEAD_TOKENS`. It exceeded the provider-reported input tokens of
/// every historical PoC request, but it is a guard, not a tokenizer: provider
/// context-limit rejections still surface as `JevError::ProviderContextLimit`.
pub fn estimate_tokens(text: &[u8]) -> usize {
    TextCount::of(text).estimated_tokens()
}

#[derive(Clone, Copy, Default)]
struct TextCount {
    ascii_bytes: usize,
    non_ascii_chars: usize,
}

impl TextCount {
    fn of(text: &[u8]) -> Self {
        let ascii_bytes = text.iter().filter(|byte| byte.is_ascii()).count();
        // UTF-8 lead bytes start every non-ASCII scalar; continuation bytes are 0b10xxxxxx.
        let non_ascii_chars = text.iter().filter(|byte| **byte >= 0xC0).count();
        Self {
            ascii_bytes,
            non_ascii_chars,
        }
    }

    fn plus(self, other: Self) -> Self {
        Self {
            ascii_bytes: self.ascii_bytes + other.ascii_bytes,
            non_ascii_chars: self.non_ascii_chars + other.non_ascii_chars,
        }
    }

    fn estimated_tokens(self) -> usize {
        self.ascii_bytes.div_ceil(2) + self.non_ascii_chars * 2 + REQUEST_OVERHEAD_TOKENS
    }
}

/// One encoded HTTP body and the caller's question indexes it answers.
pub(crate) struct Batch {
    pub(crate) question_indexes: Vec<usize>,
    pub(crate) body: Vec<u8>,
    pub(crate) estimated_tokens: usize,
    pub(crate) body_sha256: String,
}

struct EncodedQuestion {
    index: usize,
    /// `"id":{...}` exactly as it appears inside the `questions` object.
    entry: Vec<u8>,
}

const BODY_PREFIX: &[u8] = b"{\"model\":";
const QUESTIONS_OPEN: &[u8] = b",\"questions\":{";
const STATE_OPEN: &[u8] = b"},\"state\":";
const BODY_SUFFIX: &[u8] = b"}";

/// Packs questions that share one state into request bodies, in caller order, without
/// splitting or dropping any question. Every body repeats the whole state and stays within
/// `max_batch_bytes` and both token estimates; a question that cannot fit alone fails the
/// whole evaluation before dispatch.
pub(crate) fn pack(
    model: &str,
    state: &Value,
    questions: &[Question],
    max_batch_bytes: usize,
) -> Result<Vec<Batch>, JevError> {
    let model_json = serde_json::to_vec(model).expect("a string always encodes");
    let state_json = serde_json::to_vec(state).expect("a JSON value always encodes");
    let envelope_bytes = BODY_PREFIX.len()
        + model_json.len()
        + QUESTIONS_OPEN.len()
        + STATE_OPEN.len()
        + state_json.len()
        + BODY_SUFFIX.len();
    let state_count = TextCount::of(&state_json);
    let envelope_count = state_count.plus(TextCount {
        ascii_bytes: envelope_bytes - state_json.len(),
        non_ascii_chars: TextCount::of(&model_json).non_ascii_chars,
    });

    let mut batches = Vec::new();
    let mut current: Vec<EncodedQuestion> = Vec::new();
    let mut current_bytes = envelope_bytes;
    let mut current_count = envelope_count;
    for (index, question) in questions.iter().enumerate() {
        let entry = encode_entry(question);
        let entry_count = TextCount::of(&entry);
        let longest_estimate = state_count.plus(entry_count).estimated_tokens();
        if longest_estimate > STATE_WITH_LONGEST_QUESTION_TOKEN_LIMIT {
            return Err(JevError::TokenBudgetExceeded {
                question_id: question.id().to_string(),
                budget: TokenBudget::StateWithLongestQuestion,
                estimated_tokens: longest_estimate,
                limit_tokens: STATE_WITH_LONGEST_QUESTION_TOKEN_LIMIT,
            });
        }
        if envelope_bytes + entry.len() > max_batch_bytes {
            return Err(JevError::RequestTooLarge {
                question_id: question.id().to_string(),
                encoded_bytes: envelope_bytes + entry.len(),
                max_batch_bytes,
            });
        }
        let alone_estimate = envelope_count.plus(entry_count).estimated_tokens();
        if alone_estimate > TOTAL_CONTEXT_TOKEN_LIMIT {
            return Err(JevError::TokenBudgetExceeded {
                question_id: question.id().to_string(),
                budget: TokenBudget::TotalRequest,
                estimated_tokens: alone_estimate,
                limit_tokens: TOTAL_CONTEXT_TOKEN_LIMIT,
            });
        }
        let separator = usize::from(!current.is_empty());
        let candidate_bytes = current_bytes + separator + entry.len();
        let candidate_count = current_count.plus(entry_count).plus(TextCount {
            ascii_bytes: separator,
            non_ascii_chars: 0,
        });
        let is_full = candidate_bytes > max_batch_bytes
            || candidate_count.estimated_tokens() > TOTAL_CONTEXT_TOKEN_LIMIT;
        if is_full && !current.is_empty() {
            batches.push(assemble(
                &model_json,
                &state_json,
                std::mem::take(&mut current),
                current_count,
            ));
            current_bytes = envelope_bytes + entry.len();
            current_count = envelope_count.plus(entry_count);
        } else {
            current_bytes = candidate_bytes;
            current_count = candidate_count;
        }
        current.push(EncodedQuestion { index, entry });
    }
    if !current.is_empty() {
        batches.push(assemble(&model_json, &state_json, current, current_count));
    }
    Ok(batches)
}

fn encode_entry(question: &Question) -> Vec<u8> {
    let mut entry = serde_json::to_vec(question.id()).expect("a string always encodes");
    entry.push(b':');
    serde_json::to_writer(&mut entry, &question.to_wire()).expect("a JSON value always encodes");
    entry
}

/// Joins pre-encoded parts into `{"model":…,"questions":{…},"state":…}`. Question entries
/// are ordered by id so identical inputs produce identical bodies and hashes.
fn assemble(
    model_json: &[u8],
    state_json: &[u8],
    mut questions: Vec<EncodedQuestion>,
    count: TextCount,
) -> Batch {
    questions.sort_by(|left, right| left.entry.cmp(&right.entry));
    let mut body = Vec::with_capacity(count.ascii_bytes + count.non_ascii_chars * 4);
    body.extend_from_slice(BODY_PREFIX);
    body.extend_from_slice(model_json);
    body.extend_from_slice(QUESTIONS_OPEN);
    for (position, question) in questions.iter().enumerate() {
        if position > 0 {
            body.push(b',');
        }
        body.extend_from_slice(&question.entry);
    }
    body.extend_from_slice(STATE_OPEN);
    body.extend_from_slice(state_json);
    body.extend_from_slice(BODY_SUFFIX);
    let body_sha256 = Sha256::digest(&body)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Batch {
        question_indexes: questions.iter().map(|question| question.index).collect(),
        estimated_tokens: TextCount::of(&body).estimated_tokens(),
        body,
        body_sha256,
    }
}
