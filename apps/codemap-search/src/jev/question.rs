use std::collections::BTreeSet;
use std::fmt;

use serde_json::{Map, Value};

use super::JevError;

pub const MAX_QUESTION_ID_BYTES: usize = 128;
pub const MIN_SCORE_LEVELS: usize = 2;
pub const MAX_SCORE_LEVELS: usize = 10;
pub const MIN_CHOICE_OPTIONS: usize = 2;
pub const MAX_CHOICE_OPTIONS: usize = 255;
const MAX_CHOICE_OPTION_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestionType {
    Score,
    Choice,
    Noul,
}

impl QuestionType {
    pub fn wire_name(self) -> &'static str {
        match self {
            QuestionType::Score => "score",
            QuestionType::Choice => "choice",
            QuestionType::Noul => "noul",
        }
    }
}

/// One Choice option: a routing key and its rubric description (`Value::Null` when the
/// key needs no extra detail).
#[derive(Clone)]
pub struct ChoiceOption {
    key: String,
    description: Value,
}

impl ChoiceOption {
    pub fn new(key: impl Into<String>, description: Value) -> Self {
        Self {
            key: key.into(),
            description,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

/// Optional descriptions of what a Noul yes and no mean.
#[derive(Clone, Default)]
pub struct NoulCriteria {
    when_true: Option<Value>,
    when_false: Option<Value>,
}

impl NoulCriteria {
    pub fn new(when_true: Option<Value>, when_false: Option<Value>) -> Self {
        Self {
            when_true,
            when_false,
        }
    }
}

#[derive(Clone)]
enum Criteria {
    Score { levels: Vec<Value> },
    Choice { options: Vec<ChoiceOption> },
    Noul { criteria: Option<NoulCriteria> },
}

/// A validated Score, Choice, or Noul question. The id is a request-local routing key
/// that the provider never sees as meaning; the instructions must carry the whole
/// judgment and name the state or instruction fields it relies on.
#[derive(Clone)]
pub struct Question {
    id: String,
    instructions: Value,
    criteria: Criteria,
}

impl Question {
    /// `levels` are ordered from level 0 upward; each must describe its level on its own.
    pub fn score(
        id: impl Into<String>,
        instructions: Value,
        levels: Vec<Value>,
    ) -> Result<Self, JevError> {
        let id = validated_id(id.into())?;
        validate_structured(&id, "instructions", &instructions)?;
        if !(MIN_SCORE_LEVELS..=MAX_SCORE_LEVELS).contains(&levels.len()) {
            return Err(invalid(
                &id,
                format!(
                    "Score needs {MIN_SCORE_LEVELS} to {MAX_SCORE_LEVELS} levels; got {}",
                    levels.len()
                ),
            ));
        }
        for level in &levels {
            validate_structured(&id, "Score level", level)?;
        }
        Ok(Self {
            id,
            instructions,
            criteria: Criteria::Score { levels },
        })
    }

    pub fn choice(
        id: impl Into<String>,
        instructions: Value,
        options: Vec<ChoiceOption>,
    ) -> Result<Self, JevError> {
        let id = validated_id(id.into())?;
        validate_structured(&id, "instructions", &instructions)?;
        if !(MIN_CHOICE_OPTIONS..=MAX_CHOICE_OPTIONS).contains(&options.len()) {
            return Err(invalid(
                &id,
                format!(
                    "Choice needs {MIN_CHOICE_OPTIONS} to {MAX_CHOICE_OPTIONS} options; got {}",
                    options.len()
                ),
            ));
        }
        let mut keys = BTreeSet::new();
        for option in &options {
            if option.key.trim().is_empty() || option.key.len() > MAX_CHOICE_OPTION_BYTES {
                return Err(invalid(
                    &id,
                    format!(
                        "Choice option keys must be 1 to {MAX_CHOICE_OPTION_BYTES} non-blank bytes"
                    ),
                ));
            }
            if !keys.insert(option.key.as_str()) {
                return Err(invalid(
                    &id,
                    "Choice option keys must be unique".to_string(),
                ));
            }
            if !option.description.is_null() {
                validate_structured(&id, "Choice option description", &option.description)?;
            }
        }
        Ok(Self {
            id,
            instructions,
            criteria: Criteria::Choice { options },
        })
    }

    pub fn noul(
        id: impl Into<String>,
        instructions: Value,
        criteria: Option<NoulCriteria>,
    ) -> Result<Self, JevError> {
        let id = validated_id(id.into())?;
        validate_structured(&id, "instructions", &instructions)?;
        if let Some(criteria) = &criteria {
            for description in [&criteria.when_true, &criteria.when_false]
                .into_iter()
                .flatten()
            {
                validate_structured(&id, "Noul criterion", description)?;
            }
        }
        Ok(Self {
            id,
            instructions,
            criteria: Criteria::Noul { criteria },
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn question_type(&self) -> QuestionType {
        match self.criteria {
            Criteria::Score { .. } => QuestionType::Score,
            Criteria::Choice { .. } => QuestionType::Choice,
            Criteria::Noul { .. } => QuestionType::Noul,
        }
    }

    pub(crate) fn score_level_count(&self) -> Option<usize> {
        match &self.criteria {
            Criteria::Score { levels } => Some(levels.len()),
            _ => None,
        }
    }

    pub(crate) fn choice_keys(&self) -> Option<impl Iterator<Item = &str>> {
        match &self.criteria {
            Criteria::Choice { options } => Some(options.iter().map(ChoiceOption::key)),
            _ => None,
        }
    }

    /// The documented wire object for this question, without its id.
    pub(crate) fn to_wire(&self) -> Value {
        let mut object = Map::new();
        match &self.criteria {
            Criteria::Score { levels } => {
                object.insert("criteria".to_string(), Value::Array(levels.clone()));
            }
            Criteria::Choice { options } => {
                let mut sorted: Vec<&ChoiceOption> = options.iter().collect();
                sorted.sort_by(|left, right| left.key.cmp(&right.key));
                let criteria = sorted
                    .into_iter()
                    .map(|option| (option.key.clone(), option.description.clone()))
                    .collect();
                object.insert("criteria".to_string(), Value::Object(criteria));
            }
            Criteria::Noul { criteria } => {
                if let Some(criteria) = criteria {
                    let mut descriptions = Map::new();
                    if let Some(when_false) = &criteria.when_false {
                        descriptions.insert("false".to_string(), when_false.clone());
                    }
                    if let Some(when_true) = &criteria.when_true {
                        descriptions.insert("true".to_string(), when_true.clone());
                    }
                    if !descriptions.is_empty() {
                        object.insert("criteria".to_string(), Value::Object(descriptions));
                    }
                }
            }
        }
        object.insert("instructions".to_string(), self.instructions.clone());
        object.insert(
            "type".to_string(),
            Value::String(self.question_type().wire_name().to_string()),
        );
        Value::Object(object)
    }
}

impl fmt::Debug for Question {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Question")
            .field("id", &self.id)
            .field("type", &self.question_type())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ChoiceOption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChoiceOption")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for NoulCriteria {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NoulCriteria")
            .field("has_true", &self.when_true.is_some())
            .field("has_false", &self.when_false.is_some())
            .finish()
    }
}

/// Ids are logged and routed, so they stay short printable ASCII.
fn validated_id(id: String) -> Result<String, JevError> {
    let is_valid = !id.is_empty()
        && id.len() <= MAX_QUESTION_ID_BYTES
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'));
    if is_valid {
        Ok(id)
    } else {
        Err(JevError::InvalidQuestion {
            question_id: "<invalid>".to_string(),
            reason: format!(
                "ids must be 1 to {MAX_QUESTION_ID_BYTES} bytes of ASCII letters, digits, `_`, `-`, `.`, or `:`"
            ),
        })
    }
}

/// Instructions and criteria accept a non-blank string, a non-empty object, or a
/// non-empty array, matching the documented `string | object | array` shapes.
fn validate_structured(id: &str, field: &str, value: &Value) -> Result<(), JevError> {
    let is_valid = match value {
        Value::String(text) => !text.trim().is_empty(),
        Value::Object(object) => !object.is_empty(),
        Value::Array(items) => !items.is_empty(),
        _ => false,
    };
    if is_valid {
        Ok(())
    } else {
        Err(invalid(
            id,
            format!("{field} must be a non-blank string, a non-empty object, or a non-empty array"),
        ))
    }
}

fn invalid(id: &str, reason: String) -> JevError {
    JevError::InvalidQuestion {
        question_id: id.to_string(),
        reason,
    }
}
