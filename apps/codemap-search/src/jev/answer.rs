use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::question::{Question, QuestionType};
use super::JevError;

/// Maximum accepted |Σp − 1| for Score and Choice distributions. Historical Jev 1.13
/// responses deviated by at most 0.01 because the provider rounds some probabilities.
pub const PROBABILITY_SUM_TOLERANCE: f64 = 0.02;
/// Slack for single values that should lie in a closed interval.
const RANGE_EPSILON: f64 = 1e-9;

/// One typed answer with its raw provider values. Confidence describes how concentrated
/// the distribution is; it is not correctness or permission to act.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Score(ScoreAnswer),
    Choice(ChoiceAnswer),
    Noul(NoulAnswer),
}

impl Answer {
    pub fn question_type(&self) -> QuestionType {
        match self {
            Answer::Score(_) => QuestionType::Score,
            Answer::Choice(_) => QuestionType::Choice,
            Answer::Noul(_) => QuestionType::Noul,
        }
    }

    pub fn as_score(&self) -> Option<&ScoreAnswer> {
        match self {
            Answer::Score(answer) => Some(answer),
            _ => None,
        }
    }

    pub fn as_choice(&self) -> Option<&ChoiceAnswer> {
        match self {
            Answer::Choice(answer) => Some(answer),
            _ => None,
        }
    }

    pub fn as_noul(&self) -> Option<&NoulAnswer> {
        match self {
            Answer::Noul(answer) => Some(answer),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScoreAnswer {
    /// Probability-weighted level; it can land between levels.
    pub score: f64,
    /// Probability of each level, indexed from level 0.
    pub probabilities: Vec<f64>,
    pub confidence: f64,
}

impl ScoreAnswer {
    pub fn probability(&self, level: usize) -> f64 {
        self.probabilities.get(level).copied().unwrap_or(0.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceAnswer {
    pub choice: String,
    pub probabilities: BTreeMap<String, f64>,
    pub confidence: f64,
}

impl ChoiceAnswer {
    pub fn probability(&self, option: &str) -> f64 {
        self.probabilities.get(option).copied().unwrap_or(0.0)
    }
}

/// Probability of yes. Noul answers carry no confidence field.
#[derive(Clone, Debug, PartialEq)]
pub struct NoulAnswer {
    pub noul: f64,
}

pub(crate) fn parse_answer(question: &Question, value: &Value) -> Result<Answer, JevError> {
    let invalid = |reason: &str| JevError::InvalidAnswer {
        question_id: question.id().to_string(),
        reason: reason.to_string(),
    };
    let Some(object) = value.as_object() else {
        return Err(invalid("answer is not an object"));
    };
    let expected_type = question.question_type();
    if object.get("type").and_then(Value::as_str) != Some(expected_type.wire_name()) {
        return Err(invalid("answer type does not match the question type"));
    }
    match expected_type {
        QuestionType::Score => {
            let level_count = question.score_level_count().unwrap_or_default();
            let level_keys: Vec<String> = (0..level_count).map(|level| level.to_string()).collect();
            let probabilities = distribution(object, &level_keys).map_err(invalid)?;
            let score = bounded_value(object, "score", (level_count.saturating_sub(1)) as f64)
                .ok_or_else(|| invalid("score is missing or outside the level range"))?;
            let confidence = bounded_value(object, "confidence", 1.0)
                .ok_or_else(|| invalid("confidence is missing or outside [0, 1]"))?;
            Ok(Answer::Score(ScoreAnswer {
                score,
                probabilities: level_keys
                    .iter()
                    .map(|key| probabilities[key.as_str()])
                    .collect(),
                confidence,
            }))
        }
        QuestionType::Choice => {
            let option_keys: Vec<String> = question
                .choice_keys()
                .map(|keys| keys.map(str::to_string).collect())
                .unwrap_or_default();
            let probabilities = distribution(object, &option_keys).map_err(invalid)?;
            let Some(choice) = object.get("choice").and_then(Value::as_str) else {
                return Err(invalid("choice is missing"));
            };
            let Some(chosen_probability) = probabilities.get(choice).copied() else {
                return Err(invalid("choice is not one of the supplied options"));
            };
            let highest_probability = probabilities.values().copied().fold(0.0, f64::max);
            if chosen_probability + PROBABILITY_SUM_TOLERANCE < highest_probability {
                return Err(invalid("choice is not the highest-probability option"));
            }
            let confidence = bounded_value(object, "confidence", 1.0)
                .ok_or_else(|| invalid("confidence is missing or outside [0, 1]"))?;
            Ok(Answer::Choice(ChoiceAnswer {
                choice: choice.to_string(),
                probabilities,
                confidence,
            }))
        }
        QuestionType::Noul => {
            let noul = bounded_value(object, "noul", 1.0)
                .ok_or_else(|| invalid("noul is missing or outside [0, 1]"))?;
            Ok(Answer::Noul(NoulAnswer { noul }))
        }
    }
}

/// Parses `probabilities` whose keys must equal `expected_keys` exactly, each value a
/// finite probability, summing to 1 within `PROBABILITY_SUM_TOLERANCE`.
fn distribution(
    object: &Map<String, Value>,
    expected_keys: &[String],
) -> Result<BTreeMap<String, f64>, &'static str> {
    let Some(raw) = object.get("probabilities").and_then(Value::as_object) else {
        return Err("probabilities are missing");
    };
    let expected: BTreeSet<&str> = expected_keys.iter().map(String::as_str).collect();
    let returned: BTreeSet<&str> = raw.keys().map(String::as_str).collect();
    if expected != returned {
        return Err("probability keys do not match the question criteria");
    }
    let mut probabilities = BTreeMap::new();
    for (key, value) in raw {
        let Some(probability) = value.as_f64().filter(|value| is_within(*value, 1.0)) else {
            return Err("a probability is not a finite value in [0, 1]");
        };
        probabilities.insert(key.clone(), probability);
    }
    let total: f64 = probabilities.values().sum();
    if (total - 1.0).abs() > PROBABILITY_SUM_TOLERANCE {
        return Err("probabilities do not sum to 1 within tolerance");
    }
    Ok(probabilities)
}

fn bounded_value(object: &Map<String, Value>, key: &str, upper_bound: f64) -> Option<f64> {
    object
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| is_within(*value, upper_bound))
}

fn is_within(value: f64, upper_bound: f64) -> bool {
    value.is_finite() && value >= -RANGE_EPSILON && value <= upper_bound + RANGE_EPSILON
}
