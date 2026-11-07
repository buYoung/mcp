//! Every other OpenRouter decision model, in OpenRouter's own request format: the state holds
//! only the task, each question carries its candidate as a JSON object, and the runtime splits
//! the questions into batches that share that state. A model that reads only the start of a
//! long state still sees every candidate.

use super::*;

fn question(input: &FilterInput, index: usize) -> Result<Question, JevError> {
    let entity = &input.entities[index];
    noul(
        index,
        json!({
            "code": {
                "file": entity.path,
                "kind": entity.symbol.kind,
                "name": crate::redact::source(&entity.qualified_name()),
                "source": entity.body,
            },
            "question": QUESTION,
        }),
    )
}

pub(super) fn request(
    input: &FilterInput,
    indices: &[usize],
) -> Result<EvaluationRequest, JevError> {
    EvaluationRequest::new(
        json!({ crate::jev::TASK_QUERY_FIELD: input.task.task_query }),
        indices
            .iter()
            .map(|index| question(input, *index))
            .collect::<Result<_, _>>()?,
    )
}

/// One request for every asked candidate; the runtime batches its questions.
pub(super) fn groups(
    input: &FilterInput,
    asked: Vec<usize>,
) -> Result<CandidateRequests, JevError> {
    let request = request(input, &asked)?;
    Ok(vec![(asked, request)])
}
