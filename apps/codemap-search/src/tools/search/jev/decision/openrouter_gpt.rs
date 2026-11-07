//! OpenAI GPT decision models through OpenRouter. OpenAI's Decisions API reads the evidence
//! from the shared input, so each candidate gets its own request whose plain-text state holds
//! the task and that code. OpenRouter fails a whole request when OpenAI refuses one of its
//! questions; one candidate per request confines a refusal to that candidate.

use super::*;

/// The task and the one candidate in `indices` in a plain-text state, asked once.
pub(super) fn request(
    input: &FilterInput,
    indices: &[usize],
) -> Result<EvaluationRequest, JevError> {
    let [index] = indices else {
        return Err(JevError::Internal(
            "an OpenRouter GPT request asks about one candidate".into(),
        ));
    };
    EvaluationRequest::new(
        json!(format!(
            "Task:\n{}\n\nCode:\n{}",
            input.task.task_query,
            code_text(&input.entities[*index])
        )),
        vec![noul(*index, json!(QUESTION))?],
    )
}

/// One request per asked candidate.
pub(super) fn groups(
    input: &FilterInput,
    asked: Vec<usize>,
) -> Result<CandidateRequests, JevError> {
    asked
        .into_iter()
        .map(|index| Ok((vec![index], request(input, &[index])?)))
        .collect()
}
