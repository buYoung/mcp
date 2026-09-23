//! Typed question and request shapes. Question IDs are routing identifiers only; the whole
//! judgment, including backticked references to named state or instruction fields, lives in
//! the instructions.

use super::{CancelToken, JevError};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::Instant;

/// The state field the two code-navigation adapters use for the caller's explicit task
/// intent. It is a convention of those adapters, not a runtime requirement: the runtime
/// sends whatever state the caller built, and other Rust callers name their own fields.
pub const TASK_QUERY_FIELD: &str = "task_query";

/// Request-local identifier of one question: 1-64 characters of `[A-Za-z0-9_.:-]`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuestionId(String);

impl QuestionId {
    pub const MAX_LEN: usize = 64;

    pub fn new(id: impl Into<String>) -> Result<Self, JevError> {
        let id = id.into();
        let is_valid = !id.is_empty()
            && id.len() <= Self::MAX_LEN
            && id.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':')
            });
        if !is_valid {
            return Err(JevError::InvalidQuestionId(id));
        }
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for QuestionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Optional Noul criteria clarifying what counts as yes (`true`) and no (`false`).
#[derive(Clone, Debug, PartialEq)]
pub struct NoulCriteria {
    pub when_true: Value,
    pub when_false: Value,
}

/// The three System One primitives supported by the runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum QuestionKind {
    /// Ordered level descriptions, low to high. The answer is validated against
    /// `levels.len()`, never against a hardcoded scale.
    Score { levels: Vec<Value> },
    /// Option key to description. The answer must name one of these keys.
    Choice {
        options: std::collections::BTreeMap<String, Value>,
    },
    /// Yes-probability question. Noul has no confidence field.
    Noul { criteria: Option<NoulCriteria> },
}

impl QuestionKind {
    pub fn wire_type(&self) -> &'static str {
        match self {
            Self::Score { .. } => "score",
            Self::Choice { .. } => "choice",
            Self::Noul { .. } => "noul",
        }
    }

    pub fn level_count(&self) -> Option<usize> {
        match self {
            Self::Score { levels } => Some(levels.len()),
            _ => None,
        }
    }
}

/// One typed question. Construct through [`Question::score`], [`Question::choice`] or
/// [`Question::noul`], which enforce the published criteria limits.
#[derive(Clone, PartialEq)]
pub struct Question {
    id: QuestionId,
    instructions: Value,
    kind: QuestionKind,
}

impl Question {
    pub const MIN_SCORE_LEVELS: usize = 2;
    pub const MAX_SCORE_LEVELS: usize = 10;
    pub const MIN_CHOICE_OPTIONS: usize = 2;
    pub const MAX_CHOICE_OPTIONS: usize = 255;

    pub fn score(
        id: QuestionId,
        instructions: Value,
        levels: Vec<Value>,
    ) -> Result<Self, JevError> {
        validate_instructions(&id, &instructions)?;
        if levels.len() < Self::MIN_SCORE_LEVELS || levels.len() > Self::MAX_SCORE_LEVELS {
            return Err(JevError::InvalidCriteria {
                id,
                reason: format!(
                    "score needs {}-{} levels, got {}",
                    Self::MIN_SCORE_LEVELS,
                    Self::MAX_SCORE_LEVELS,
                    levels.len()
                ),
            });
        }
        for (index, level) in levels.iter().enumerate() {
            if !is_description(level, false) {
                return Err(JevError::InvalidCriteria {
                    id,
                    reason: format!("level {index} must be a non-blank string or an object"),
                });
            }
        }
        Ok(Self {
            id,
            instructions,
            kind: QuestionKind::Score { levels },
        })
    }

    pub fn choice(
        id: QuestionId,
        instructions: Value,
        options: std::collections::BTreeMap<String, Value>,
    ) -> Result<Self, JevError> {
        validate_instructions(&id, &instructions)?;
        if options.len() < Self::MIN_CHOICE_OPTIONS || options.len() > Self::MAX_CHOICE_OPTIONS {
            return Err(JevError::InvalidCriteria {
                id,
                reason: format!(
                    "choice needs {}-{} options, got {}",
                    Self::MIN_CHOICE_OPTIONS,
                    Self::MAX_CHOICE_OPTIONS,
                    options.len()
                ),
            });
        }
        for (option, description) in &options {
            if option.trim().is_empty() || option.len() > 128 {
                return Err(JevError::InvalidCriteria {
                    id,
                    reason: "option keys must be 1-128 non-blank characters".into(),
                });
            }
            if !is_description(description, true) {
                return Err(JevError::InvalidCriteria {
                    id,
                    reason: format!(
                        "option '{option}' must describe itself with a string, object, array or null"
                    ),
                });
            }
        }
        Ok(Self {
            id,
            instructions,
            kind: QuestionKind::Choice { options },
        })
    }

    pub fn noul(
        id: QuestionId,
        instructions: Value,
        criteria: Option<NoulCriteria>,
    ) -> Result<Self, JevError> {
        validate_instructions(&id, &instructions)?;
        if let Some(criteria) = &criteria {
            for (label, description) in [
                ("true", &criteria.when_true),
                ("false", &criteria.when_false),
            ] {
                if !is_description(description, false) {
                    return Err(JevError::InvalidCriteria {
                        id,
                        reason: format!(
                            "noul criteria '{label}' must be a non-blank string, object or array"
                        ),
                    });
                }
            }
        }
        Ok(Self {
            id,
            instructions,
            kind: QuestionKind::Noul { criteria },
        })
    }

    pub fn id(&self) -> &QuestionId {
        &self.id
    }

    pub fn instructions(&self) -> &Value {
        &self.instructions
    }

    pub fn kind(&self) -> &QuestionKind {
        &self.kind
    }

    /// The documented wire object for this question.
    pub fn to_wire(&self) -> Value {
        let mut wire = Map::new();
        wire.insert("type".into(), Value::String(self.kind.wire_type().into()));
        wire.insert("instructions".into(), self.instructions.clone());
        match &self.kind {
            QuestionKind::Score { levels } => {
                wire.insert("criteria".into(), Value::Array(levels.clone()));
            }
            QuestionKind::Choice { options } => {
                wire.insert(
                    "criteria".into(),
                    Value::Object(
                        options
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    ),
                );
            }
            QuestionKind::Noul { criteria } => {
                if let Some(criteria) = criteria {
                    let mut object = Map::new();
                    object.insert("true".into(), criteria.when_true.clone());
                    object.insert("false".into(), criteria.when_false.clone());
                    wire.insert("criteria".into(), Value::Object(object));
                }
            }
        }
        Value::Object(wire)
    }
}

impl std::fmt::Debug for Question {
    // Instructions carry candidate evidence (source text, docs); keep them out of Debug.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Question")
            .field("id", &self.id)
            .field("type", &self.kind.wire_type())
            .field("instruction_bytes", &encoded_len(&self.instructions))
            .finish()
    }
}

fn validate_instructions(id: &QuestionId, instructions: &Value) -> Result<(), JevError> {
    let is_valid = match instructions {
        Value::String(text) => !text.trim().is_empty(),
        Value::Object(fields) => !fields.is_empty(),
        Value::Array(items) => !items.is_empty(),
        _ => false,
    };
    if is_valid {
        Ok(())
    } else {
        Err(JevError::InvalidInstructions {
            id: id.clone(),
            reason:
                "instructions must be a non-blank string, a non-empty object or a non-empty array"
                    .into(),
        })
    }
}

fn is_description(value: &Value, allow_null: bool) -> bool {
    match value {
        Value::String(text) => !text.trim().is_empty(),
        Value::Object(fields) => !fields.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Null => allow_null,
        _ => false,
    }
}

pub(crate) fn encoded_len(value: &Value) -> usize {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

/// Caller-supplied limits for one evaluation.
///
/// `deadline_at` is an absolute monotonic instant. The effective deadline of the call is the
/// earliest of this instant and the evaluator's own configured ceiling (measured from the
/// moment `evaluate` is entered); a caller can therefore shorten a call, never extend it.
/// Passing the same instant to several requests (for example the two stages of one adapter)
/// makes them share one deadline: the second request receives only what the first left.
#[derive(Clone, Debug, Default)]
pub struct RequestPolicy {
    pub deadline_at: Option<Instant>,
    pub cancel: Option<CancelToken>,
}

/// One evaluation: caller-owned JSON state (a string, object or array shared by every
/// question of the request) plus independent questions over that state.
#[derive(Clone)]
pub struct EvaluationRequest {
    state: Value,
    questions: Vec<Question>,
    policy: RequestPolicy,
}

impl EvaluationRequest {
    /// `state` is sent as-is: it must be a non-blank string, an object or an array (the
    /// provider's documented state shapes). The runtime never adds or reads fields of it;
    /// callers that need a task intent put it under a name of their choosing (the built-in
    /// adapters use [`TASK_QUERY_FIELD`]). Questions need unique IDs, and a request may carry
    /// at most [`super::MAX_QUESTIONS_PER_REQUEST`] of them.
    pub fn new(state: Value, questions: Vec<Question>) -> Result<Self, JevError> {
        let is_supported_state = match &state {
            Value::String(text) => !text.trim().is_empty(),
            Value::Object(_) | Value::Array(_) => true,
            Value::Null | Value::Bool(_) | Value::Number(_) => false,
        };
        if !is_supported_state {
            return Err(JevError::InvalidState(
                "state must be a non-blank string, an object or an array".into(),
            ));
        }
        if questions.is_empty() {
            return Err(JevError::EmptyQuestions);
        }
        if questions.len() > super::MAX_QUESTIONS_PER_REQUEST {
            return Err(JevError::TooManyQuestions {
                count: questions.len(),
                limit: super::MAX_QUESTIONS_PER_REQUEST,
            });
        }
        let mut seen = HashSet::new();
        for question in &questions {
            if !seen.insert(question.id().clone()) {
                return Err(JevError::DuplicateQuestionId(question.id().clone()));
            }
        }
        Ok(Self {
            state,
            questions,
            policy: RequestPolicy::default(),
        })
    }

    pub fn with_policy(mut self, policy: RequestPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Bound the call by an absolute monotonic instant (see [`RequestPolicy`]).
    pub fn with_deadline_at(mut self, deadline_at: Instant) -> Self {
        self.policy.deadline_at = Some(deadline_at);
        self
    }

    /// Bound the call by `deadline` counted from *now*, the moment this method runs. Prefer
    /// [`Self::with_deadline_at`] when several requests must share one deadline.
    pub fn with_deadline(self, deadline: Duration) -> Self {
        let deadline_at = Instant::now()
            .checked_add(deadline)
            .unwrap_or_else(Instant::now);
        self.with_deadline_at(deadline_at)
    }

    pub fn with_cancel(mut self, cancel: CancelToken) -> Self {
        self.policy.cancel = Some(cancel);
        self
    }

    /// The shared state exactly as it is sent on the wire.
    pub fn state(&self) -> &Value {
        &self.state
    }

    /// One named field of an object state (`None` for other state shapes or absent keys).
    pub fn state_field(&self, name: &str) -> Option<&Value> {
        self.state.get(name)
    }

    pub fn questions(&self) -> &[Question] {
        &self.questions
    }

    pub fn policy(&self) -> &RequestPolicy {
        &self.policy
    }
}

impl std::fmt::Debug for EvaluationRequest {
    // State and instructions hold source-derived evidence; Debug reports shape only.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_shape = match &self.state {
            Value::String(text) => format!("string({} bytes)", text.len()),
            Value::Object(fields) => format!("object{:?}", fields.keys().collect::<Vec<_>>()),
            Value::Array(items) => format!("array({} items)", items.len()),
            other => format!("{other:?}"),
        };
        f.debug_struct("EvaluationRequest")
            .field("state", &state_shape)
            .field("question_count", &self.questions.len())
            .field(
                "question_ids",
                &self.questions.iter().map(Question::id).collect::<Vec<_>>(),
            )
            .field("policy", &self.policy)
            .finish()
    }
}
