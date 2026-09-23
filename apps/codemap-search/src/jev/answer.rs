//! Typed answers, usage/timing accounting and the shared failure contract.

use super::question::{Question, QuestionId, QuestionKind};
use super::JevError;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

/// Half of the rounding step observed in provider answers: recorded Score values,
/// confidences and probabilities carry two decimals, so each reported number may deviate
/// from the exact value by up to 0.005. Every tolerance below derives from this precision;
/// none of them re-normalizes or corrects an answer, they only decide acceptance.
pub const PROBABILITY_PRECISION: f64 = 0.005;
/// Smallest accepted deviation of a probability distribution sum from 1.0 (four entries at
/// two decimals). Larger distributions use [`distribution_sum_tolerance`].
pub const PROBABILITY_SUM_TOLERANCE: f64 = 0.02;
/// Accepted overshoot of a Score value beyond `[0, levels - 1]`.
pub const SCORE_TOLERANCE: f64 = 0.02;
/// Two options rounded to two decimals can swap order by at most one rounding step; the
/// chosen option must be within this much of the highest reported probability.
pub const CHOICE_TIE_TOLERANCE: f64 = 0.01;
const PROBABILITY_RANGE_TOLERANCE: f64 = 1e-9;

/// Accepted deviation of a distribution sum from 1.0 for `entries` rounded probabilities:
/// one rounding step per entry, never below [`PROBABILITY_SUM_TOLERANCE`].
pub fn distribution_sum_tolerance(entries: usize) -> f64 {
    (entries as f64 * PROBABILITY_PRECISION).max(PROBABILITY_SUM_TOLERANCE)
        + PROBABILITY_RANGE_TOLERANCE
}

/// Accepted deviation between a reported Score and `sum(level_index * probability)` for
/// `levels` rounded probabilities plus a rounded score: `0.005 * (0 + 1 + … + levels-1)`
/// from the probabilities and `0.005` from the score itself.
pub fn score_weighted_sum_tolerance(levels: usize) -> f64 {
    let index_sum = (levels.saturating_sub(1) * levels) as f64 / 2.0;
    PROBABILITY_PRECISION * index_sum + PROBABILITY_PRECISION + PROBABILITY_RANGE_TOLERANCE
}

/// Score answer: the probability-weighted level plus the full level distribution. Adapters
/// recompute qualification from `probabilities`; `confidence` describes distribution
/// concentration, not correctness.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreAnswer {
    pub score: f64,
    /// Index = level number; always `levels.len()` entries summing to ~1.
    pub probabilities: Vec<f64>,
    pub confidence: Option<f64>,
}

impl ScoreAnswer {
    pub fn level_count(&self) -> usize {
        self.probabilities.len()
    }

    pub fn probability(&self, level: usize) -> f64 {
        self.probabilities.get(level).copied().unwrap_or(0.0)
    }

    /// Probability mass of `level` and every higher level.
    pub fn mass_at_or_above(&self, level: usize) -> f64 {
        self.probabilities.iter().skip(level).sum()
    }

    /// Probability mass strictly below `level`.
    pub fn mass_below(&self, level: usize) -> f64 {
        self.probabilities.iter().take(level).sum()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceAnswer {
    pub choice: String,
    pub probabilities: BTreeMap<String, f64>,
    pub confidence: Option<f64>,
}

impl ChoiceAnswer {
    pub fn probability(&self, option: &str) -> f64 {
        self.probabilities.get(option).copied().unwrap_or(0.0)
    }
}

/// Noul answer: the probability that the answer is yes. There is no confidence field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoulAnswer {
    pub noul: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Score(ScoreAnswer),
    Choice(ChoiceAnswer),
    Noul(NoulAnswer),
}

impl Answer {
    pub fn wire_type(&self) -> &'static str {
        match self {
            Self::Score(_) => "score",
            Self::Choice(_) => "choice",
            Self::Noul(_) => "noul",
        }
    }

    pub fn as_score(&self) -> Option<&ScoreAnswer> {
        match self {
            Self::Score(answer) => Some(answer),
            _ => None,
        }
    }

    pub fn as_choice(&self) -> Option<&ChoiceAnswer> {
        match self {
            Self::Choice(answer) => Some(answer),
            _ => None,
        }
    }

    pub fn as_noul(&self) -> Option<&NoulAnswer> {
        match self {
            Self::Noul(answer) => Some(answer),
            _ => None,
        }
    }
}

/// Provider-reported token usage. `None` means the provider did not report a value; an
/// unknown count is never treated as zero when summing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    /// Responses that carried a usage block.
    pub reported_responses: usize,
    /// Responses that omitted usage; their tokens are unknown, not zero.
    pub unreported_responses: usize,
}

impl Usage {
    pub fn reported(input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            input_tokens: Some(input_tokens),
            output_tokens: Some(output_tokens),
            reported_responses: 1,
            unreported_responses: 0,
        }
    }

    pub fn unreported() -> Self {
        Self {
            input_tokens: None,
            output_tokens: None,
            reported_responses: 0,
            unreported_responses: 1,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.reported_responses == 0 && self.unreported_responses == 0
    }
}

impl std::ops::Add for Usage {
    type Output = Usage;

    /// Sum two accountings; a `None` on either side makes that total unknown.
    fn add(self, other: Usage) -> Usage {
        let add_known = |left: Option<u64>, right: Option<u64>, is_empty: bool| {
            if is_empty {
                return right;
            }
            match (left, right) {
                (Some(a), Some(b)) => Some(a.saturating_add(b)),
                _ => None,
            }
        };
        if other.is_empty() {
            return self;
        }
        Usage {
            input_tokens: add_known(self.input_tokens, other.input_tokens, self.is_empty()),
            output_tokens: add_known(self.output_tokens, other.output_tokens, self.is_empty()),
            reported_responses: self.reported_responses + other.reported_responses,
            unreported_responses: self.unreported_responses + other.unreported_responses,
        }
    }
}

/// Whole-call timing. `elapsed` is wall time from `evaluate` entry to exit, including
/// queueing behind concurrency permits and request spacing; `http` sums only the HTTP
/// round trips; `queue_wait` sums the per-batch waits before dispatch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timing {
    pub elapsed: Duration,
    pub queue_wait: Duration,
    pub http: Duration,
    pub request_count: usize,
}

/// Identity of one attempted HTTP request. The SHA-256 of the encoded body lets a later
/// replay prove which exact request produced an answer without storing the body here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestIdentity {
    pub batch_index: usize,
    pub question_ids: Vec<QuestionId>,
    pub request_bytes: usize,
    pub request_sha256: String,
    pub http_status: Option<u16>,
    pub http_elapsed: Duration,
    pub queue_wait: Duration,
}

/// Successful evaluation: typed answers, the raw answer objects for replay, and
/// accounting. Question/policy versions are owned by the adapter that built the request.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationOutcome {
    pub model: String,
    pub answers: BTreeMap<QuestionId, Answer>,
    pub raw_answers: BTreeMap<QuestionId, Value>,
    pub usage: Usage,
    pub timing: Timing,
    pub requests: Vec<RequestIdentity>,
}

impl EvaluationOutcome {
    pub fn score(&self, id: &QuestionId) -> Option<&ScoreAnswer> {
        self.answers.get(id).and_then(Answer::as_score)
    }

    pub fn choice(&self, id: &QuestionId) -> Option<&ChoiceAnswer> {
        self.answers.get(id).and_then(Answer::as_choice)
    }

    pub fn noul(&self, id: &QuestionId) -> Option<&NoulAnswer> {
        self.answers.get(id).and_then(Answer::as_noul)
    }
}

/// Failed evaluation with everything that is still known: the bounded error, every
/// attempted request identity (including the failing one) and the usage reported so far.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationFailure {
    pub error: JevError,
    pub usage: Usage,
    pub timing: Timing,
    pub requests: Vec<RequestIdentity>,
}

impl EvaluationFailure {
    pub fn before_dispatch(error: JevError, elapsed: Duration) -> Self {
        Self {
            error,
            usage: Usage::default(),
            timing: Timing {
                elapsed,
                ..Timing::default()
            },
            requests: Vec::new(),
        }
    }
}

impl From<JevError> for EvaluationFailure {
    fn from(error: JevError) -> Self {
        Self::before_dispatch(error, Duration::ZERO)
    }
}

impl std::fmt::Display for EvaluationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for EvaluationFailure {}

fn finite(value: &Value, field: &str) -> Result<f64, String> {
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("'{field}' must be a finite number"))
}

fn probability(value: &Value, field: &str) -> Result<f64, String> {
    let number = finite(value, field)?;
    if !(-PROBABILITY_RANGE_TOLERANCE..=1.0 + PROBABILITY_RANGE_TOLERANCE).contains(&number) {
        return Err(format!("'{field}' must lie in [0, 1], got {number}"));
    }
    Ok(number.clamp(0.0, 1.0))
}

fn optional_confidence(raw: &serde_json::Map<String, Value>) -> Result<Option<f64>, String> {
    match raw.get("confidence") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => probability(value, "confidence").map(Some),
    }
}

fn check_distribution_sum(sum: f64, entries: usize, field: &str) -> Result<(), String> {
    let tolerance = distribution_sum_tolerance(entries);
    if (sum - 1.0).abs() > tolerance {
        return Err(format!(
            "'{field}' sums to {sum:.4}, outside 1 ± {tolerance:.3}"
        ));
    }
    Ok(())
}

/// Validate one raw answer object against its question and convert it to a typed answer.
pub(crate) fn parse_answer(question: &Question, raw: &Value) -> Result<Answer, String> {
    let object = raw.as_object().ok_or("answer must be an object")?;
    let wire_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or("answer 'type' must be a string")?;
    if wire_type != question.kind().wire_type() {
        return Err(format!(
            "answer type '{wire_type}' does not match question type '{}'",
            question.kind().wire_type()
        ));
    }
    match question.kind() {
        QuestionKind::Score { levels } => {
            let score = finite(object.get("score").unwrap_or(&Value::Null), "score")?;
            let raw_probabilities = object
                .get("probabilities")
                .and_then(Value::as_object)
                .ok_or("'probabilities' must be an object keyed by level")?;
            let mut probabilities = Vec::with_capacity(levels.len());
            for level in 0..levels.len() {
                let key = level.to_string();
                let value = raw_probabilities
                    .get(&key)
                    .ok_or_else(|| format!("'probabilities' is missing level {key}"))?;
                probabilities.push(probability(value, &format!("probabilities.{key}"))?);
            }
            if let Some(extra) = raw_probabilities.keys().find(|key| {
                key.parse::<usize>()
                    .map_or(true, |level| level >= levels.len())
            }) {
                return Err(format!(
                    "'probabilities' names level '{extra}' outside the {} declared levels",
                    levels.len()
                ));
            }
            check_distribution_sum(probabilities.iter().sum(), levels.len(), "probabilities")?;
            let top = (levels.len() - 1) as f64;
            if !(-SCORE_TOLERANCE..=top + SCORE_TOLERANCE).contains(&score) {
                return Err(format!("'score' {score} lies outside [0, {top}]"));
            }
            // The score is the probability-weighted level; a reported value that disagrees
            // with its own distribution beyond rounding is an inconsistent answer.
            let weighted: f64 = probabilities
                .iter()
                .enumerate()
                .map(|(level, probability)| level as f64 * probability)
                .sum();
            let tolerance = score_weighted_sum_tolerance(levels.len());
            if (weighted - score).abs() > tolerance {
                return Err(format!(
                    "'score' {score} disagrees with the probability-weighted level {weighted:.4} beyond ± {tolerance:.3}"
                ));
            }
            Ok(Answer::Score(ScoreAnswer {
                score: score.clamp(0.0, top),
                probabilities,
                confidence: optional_confidence(object)?,
            }))
        }
        QuestionKind::Choice { options } => {
            let choice = object
                .get("choice")
                .and_then(Value::as_str)
                .ok_or("'choice' must be a string")?;
            if !options.contains_key(choice) {
                return Err(format!("'choice' names unknown option '{choice}'"));
            }
            let raw_probabilities = object
                .get("probabilities")
                .and_then(Value::as_object)
                .ok_or("'probabilities' must be an object keyed by option")?;
            let mut probabilities = BTreeMap::new();
            for option in options.keys() {
                let value = raw_probabilities
                    .get(option)
                    .ok_or_else(|| format!("'probabilities' is missing option '{option}'"))?;
                probabilities.insert(
                    option.clone(),
                    probability(value, &format!("probabilities.{option}"))?,
                );
            }
            if let Some(extra) = raw_probabilities
                .keys()
                .find(|key| !options.contains_key(*key))
            {
                return Err(format!("'probabilities' names unknown option '{extra}'"));
            }
            check_distribution_sum(probabilities.values().sum(), options.len(), "probabilities")?;
            // The chosen option must be one of the most probable ones (ties within rounding).
            let highest = probabilities.values().copied().fold(0.0_f64, f64::max);
            let chosen = probabilities.get(choice).copied().unwrap_or(0.0);
            if chosen + CHOICE_TIE_TOLERANCE + PROBABILITY_RANGE_TOLERANCE < highest {
                return Err(format!(
                    "'choice' {choice} has probability {chosen} while another option reaches {highest}"
                ));
            }
            Ok(Answer::Choice(ChoiceAnswer {
                choice: choice.to_string(),
                probabilities,
                confidence: optional_confidence(object)?,
            }))
        }
        QuestionKind::Noul { .. } => {
            let noul = probability(object.get("noul").unwrap_or(&Value::Null), "noul")?;
            // Noul has no confidence field; a present value is ignored, never required.
            Ok(Answer::Noul(NoulAnswer { noul }))
        }
    }
}

pub(crate) fn parse_usage(raw: Option<&Value>) -> Usage {
    let Some(object) = raw.and_then(Value::as_object) else {
        return Usage::unreported();
    };
    match (
        object.get("input_tokens").and_then(Value::as_u64),
        object.get("output_tokens").and_then(Value::as_u64),
    ) {
        (Some(input_tokens), Some(output_tokens)) => Usage::reported(input_tokens, output_tokens),
        _ => Usage::unreported(),
    }
}
