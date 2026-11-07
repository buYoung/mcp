//! Relevance requests for candidate bodies, shared by every decision model. All models get
//! the same question and criteria about the same candidate fields; each model's module only
//! arranges them in the request format that model reads best:
//! - [`jev`]: TypeSafe Jev, also when served through OpenRouter;
//! - [`openrouter_gpt`]: OpenAI GPT decision models through OpenRouter;
//! - [`openrouter_default`]: every other OpenRouter decision model.

mod jev;
mod openrouter_default;
mod openrouter_gpt;

use super::*;

pub(super) const MAX_GROUP_BYTES: usize = crate::jev::MAX_BATCH_BYTES;

/// Requests of one judging pass, each with the candidates it asks about.
type CandidateRequests = Vec<(Vec<usize>, EvaluationRequest)>;

/// The same question for every candidate; code turns its answer into a retention decision.
pub(super) const QUESTION: &str =
    "Does this code handle something the task names, even if only part of the task?";
pub(super) const WHEN_TRUE: &str = "It implements, calls, stores, fetches, filters, configures or passes data for at least one feature, data item or step the task names, including shared infrastructure such as a queue, bus, store, watcher or registry that the named flow passes through.";
pub(super) const WHEN_FALSE: &str = "It handles only features, data or steps the task does not name, and the named flow does not pass through it.";

pub(super) fn question_id(entity: usize) -> QuestionId {
    QuestionId::new(format!("b{entity}")).expect("generated question ids are valid")
}

/// The decision model family whose request format a configured model ID selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Model {
    Jev,
    OpenRouterGpt,
    OpenRouterDefault,
}

impl Model {
    /// Bare Jev versions and OpenRouter's `typesafe/` models are Jev; OpenRouter serves
    /// `openai/` models through OpenAI's Decisions API; any other `author/slug` is another
    /// OpenRouter decision model.
    fn of(model: &str) -> Self {
        match model.split_once('/') {
            None | Some(("typesafe", _)) => Self::Jev,
            Some(("openai", _)) => Self::OpenRouterGpt,
            Some(_) => Self::OpenRouterDefault,
        }
    }

    fn request(
        self,
        input: &FilterInput,
        indices: &[usize],
    ) -> Result<EvaluationRequest, JevError> {
        match self {
            Self::Jev => jev::request(input, indices),
            Self::OpenRouterGpt => openrouter_gpt::request(input, indices),
            Self::OpenRouterDefault => openrouter_default::request(input, indices),
        }
    }

    fn groups(
        self,
        input: &FilterInput,
        asked: Vec<usize>,
        policy: &FilterPolicy,
    ) -> Result<CandidateRequests, JevError> {
        match self {
            Self::Jev => jev::groups(input, asked, policy),
            Self::OpenRouterGpt => openrouter_gpt::groups(input, asked),
            Self::OpenRouterDefault => openrouter_default::groups(input, asked),
        }
    }
}

/// The candidate as a model reads it: identity lines, then the numbered source.
fn code_text(entity: &FilterEntity) -> String {
    format!(
        "file: {}\nkind: {}\nname: {}\nsource:\n{}",
        entity.path,
        entity.symbol.kind,
        crate::redact::source(&entity.qualified_name()),
        entity.body.as_deref().unwrap_or_default(),
    )
}

/// One Noul about candidate `index` with the shared criteria.
fn noul(index: usize, instructions: serde_json::Value) -> Result<Question, JevError> {
    Question::noul(
        question_id(index),
        instructions,
        Some(NoulCriteria {
            when_true: json!(WHEN_TRUE),
            when_false: json!(WHEN_FALSE),
        }),
    )
}

fn limits(policy: &FilterPolicy) -> crate::jev::BatchLimits {
    crate::jev::BatchLimits {
        max_batch_bytes: policy.max_group_bytes.min(MAX_GROUP_BYTES),
        ..crate::jev::BatchLimits::default()
    }
}

/// A request fits when it is sent as one batch: its state is written for its own
/// candidates, so it is never split.
fn fits(request: &EvaluationRequest, policy: &FilterPolicy) -> bool {
    crate::jev::request_batch_count(request, &policy.model, &limits(policy))
        .is_ok_and(|batches| batches == 1)
}

pub(super) fn candidate_fits(input: &FilterInput, index: usize, policy: &FilterPolicy) -> bool {
    Model::of(&policy.model)
        .request(input, &[index])
        .is_ok_and(|request| fits(&request, policy))
}

/// The requests asking about every candidate that fits a request alone, each with the
/// candidates it asks about. `None` when no candidate fits.
pub(super) fn request_for(
    input: &FilterInput,
    indices: &[usize],
    policy: &FilterPolicy,
) -> Result<Option<CandidateRequests>, JevError> {
    let asked: Vec<usize> = indices
        .iter()
        .copied()
        .filter(|index| {
            // A single oversized candidate stays unjudged; other candidates still benefit.
            let fits = candidate_fits(input, *index, policy);
            if !fits {
                tracing::info!(
                    candidate_index = *index,
                    "jev candidate preserved: context budget"
                );
            }
            fits
        })
        .collect();
    if asked.is_empty() {
        return Ok(None);
    }
    Model::of(&policy.model)
        .groups(input, asked, policy)
        .map(Some)
}
