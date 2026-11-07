//! Connection-local task text. `initial_instructions` stores the user's request once;
//! enabled Jev filters judge their output against it, and retrieval queries never supply or
//! replace it.

use serde_json::{json, Value};

/// Task memory bound, independent of the evaluation context budget.
pub const MAX_TASK_BYTES: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredTask {
    pub task_query: String,
}

impl RegisteredTask {
    pub fn masked(&self) -> Self {
        Self {
            task_query: crate::redact::source(&self.task_query).into_owned(),
        }
    }
}

fn invalid(reason: &str) -> (i64, String) {
    (
        -32602,
        format!("Invalid initial_instructions task: {reason}. Pass the user's complete request as task."),
    )
}

/// The `task` argument of `initial_instructions`; `None` keeps the current task.
pub(crate) fn parse(arguments: &Value) -> Result<Option<RegisteredTask>, (i64, String)> {
    let Some(task) = arguments.get("task").filter(|task| !task.is_null()) else {
        return Ok(None);
    };
    let Some(text) = task.as_str().filter(|text| !text.trim().is_empty()) else {
        return Err(invalid("task must be a nonempty string"));
    };
    if text.len() > MAX_TASK_BYTES {
        return Err(invalid("task exceeds 65536 bytes"));
    }
    Ok(Some(RegisteredTask {
        task_query: text.to_string(),
    }))
}

pub(crate) fn schema_property() -> Value {
    json!({"type":"string","minLength":1,"description":"The user's complete request, verbatim. Enabled Jev filters judge grep, read and search output against it."})
}
