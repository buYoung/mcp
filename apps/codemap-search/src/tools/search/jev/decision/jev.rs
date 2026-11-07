//! TypeSafe Jev: one plain-text state holds the task and a group of candidates, each under
//! its question ID, and one Noul per candidate names that ID.

use super::*;

pub(super) fn request(
    input: &FilterInput,
    indices: &[usize],
) -> Result<EvaluationRequest, JevError> {
    let candidates = indices
        .iter()
        .map(|index| {
            format!(
                "[{}]\n{}",
                question_id(*index),
                code_text(&input.entities[*index])
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    EvaluationRequest::new(
        json!(format!(
            "Task:\n{}\n\nCandidates:\n\n{candidates}",
            input.task.task_query
        )),
        indices
            .iter()
            .map(|index| {
                noul(
                    *index,
                    json!(format!(
                        "About candidate [{}] only: {QUESTION}",
                        question_id(*index)
                    )),
                )
            })
            .collect::<Result<_, _>>()?,
    )
}

/// Consecutive candidates share a request while the group still fits one batch.
pub(super) fn groups(
    input: &FilterInput,
    asked: Vec<usize>,
    policy: &FilterPolicy,
) -> Result<CandidateRequests, JevError> {
    let mut requests = Vec::new();
    let mut group: Vec<usize> = Vec::new();
    for index in asked {
        let mut grown = group.clone();
        grown.push(index);
        if group.is_empty() || fits(&request(input, &grown)?, policy) {
            group = grown;
            continue;
        }
        let closed = std::mem::replace(&mut group, vec![index]);
        let closed_request = request(input, &closed)?;
        requests.push((closed, closed_request));
    }
    let last = request(input, &group)?;
    requests.push((group, last));
    Ok(requests)
}
