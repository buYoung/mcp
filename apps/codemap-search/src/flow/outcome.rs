//! Semantic facts, independent of language-specific proof and presentation.
use super::evaluate::Evidence;
use super::index::Location;

#[derive(Clone, Debug)]
pub(super) enum Condition {
    NotType {
        subject: Location,
        expected: ValueType,
    },
}

#[derive(Clone, Debug)]
pub(super) enum ValueType {
    JsonObject,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ReturnedValue {
    Constant(super::Constant),
    Tuple(Vec<ReturnedValue>),
    Variant {
        name: String,
        value: Box<ReturnedValue>,
    },
    Unknown,
}

#[derive(Clone, Debug)]
pub(super) struct ConditionalOutcome {
    pub condition: Condition,
    pub returned_value: ReturnedValue,
    pub is_early_return: bool,
    pub location: Location,
    pub owner: Location,
    pub callback: Location,
    pub evidence: Vec<Location>,
    pub certainty: Evidence,
    pub limitation: Option<String>,
}

impl ConditionalOutcome {
    pub fn key(&self) -> String {
        format!(
            "outcome:{}:{}:{}",
            self.location.path, self.location.range.start_line, self.location.range.start_col
        )
    }
    pub fn overlaps(&self, anchors: &[(String, usize, usize)]) -> bool {
        super::index::overlaps(&self.location, anchors)
            || super::index::overlaps(&self.owner, anchors)
    }
    pub fn explains_passing(&self, step: &super::evaluate::Step) -> bool {
        step.relation == "function value passed"
            && step.from.path == self.callback.path
            && step.from.range == self.callback.range
            && step.to.path == self.location.path
            && super::index::contains(&self.location.range, &step.to.range)
    }
}
