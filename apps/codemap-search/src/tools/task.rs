//! Connection-local task questions. The caller derives these from the user's task once;
//! retrieval queries never supply or replace them.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

pub const MAX_QUESTIONS: usize = 8;
pub const MAX_GOAL_BYTES: usize = 8_192;
pub const MAX_TEXT_BYTES: usize = 2_048;
pub const MAX_REGISTRATION_BYTES: usize = 16_384;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    #[default]
    All,
    Any,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskQuestion {
    pub id: String,
    pub question: String,
    pub when_true: String,
    pub when_false: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RegisteredTask {
    pub task_query: String,
    pub questions: Vec<TaskQuestion>,
    #[serde(default, rename = "match")]
    pub match_mode: MatchMode,
}

impl RegisteredTask {
    pub fn masked(&self) -> Self {
        let mut task = self.clone();
        task.task_query = crate::redact::source(&task.task_query).into_owned();
        for question in &mut task.questions {
            question.id = crate::redact::source(&question.id).into_owned();
            question.question = crate::redact::source(&question.question).into_owned();
            question.when_true = crate::redact::source(&question.when_true).into_owned();
            question.when_false = crate::redact::source(&question.when_false).into_owned();
        }
        task
    }

    fn validate(&self) -> Result<(), (i64, String)> {
        let text_ok = |text: &str, limit| !text.trim().is_empty() && text.len() <= limit;
        if !text_ok(&self.task_query, MAX_GOAL_BYTES) {
            return Err(invalid(
                "task_query must be nonempty and at most 8192 UTF-8 bytes",
            ));
        }
        if self.questions.is_empty() || self.questions.len() > MAX_QUESTIONS {
            return Err(invalid(
                "questions must contain 1 to 8 focused yes/no questions",
            ));
        }
        let mut ids = HashSet::new();
        for question in &self.questions {
            if question.id.is_empty()
                || question.id.len() > 32
                || !question
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                || !ids.insert(&question.id)
            {
                return Err(invalid(
                    "question ids must be unique, 1 to 32 characters of [A-Za-z0-9_-]",
                ));
            }
            if [
                &question.question,
                &question.when_true,
                &question.when_false,
            ]
            .iter()
            .any(|text| !text_ok(text, MAX_TEXT_BYTES))
            {
                return Err(invalid("question, when_true and when_false must be nonempty strings of at most 2048 UTF-8 bytes each"));
            }
        }
        Ok(())
    }
}

fn invalid(reason: &str) -> (i64, String) {
    (-32602, format!("Invalid Jev task registration: {reason}. Call initial_instructions with task_query and questions (id, question, when_true, when_false); match is all or any. Register again when the user's task changes."))
}

pub(crate) fn parse(
    arguments: &Value,
    is_required: bool,
) -> Result<Option<RegisteredTask>, (i64, String)> {
    if serde_json::to_vec(arguments).map_or(usize::MAX, |bytes| bytes.len())
        > MAX_REGISTRATION_BYTES
    {
        return Err(invalid("encoded registration exceeds 16384 bytes"));
    }
    if !is_required && arguments.get("questions").is_none() {
        // Disabled Jev keeps ordinary initialization and legacy text-only initialization
        // usable, but cannot leave an incomplete task for a later enabled search.
        if let Some(goal) = arguments.get("task_query").filter(|value| !value.is_null()) {
            if !goal
                .as_str()
                .is_some_and(|text| text.len() <= MAX_GOAL_BYTES)
            {
                return Err(invalid(
                    "task_query must be a string of at most 8192 UTF-8 bytes",
                ));
            }
        }
        return Ok(None);
    }
    let task: RegisteredTask = serde_json::from_value(arguments.clone())
        .map_err(|_| invalid("a goal and a nonempty structured questions list are required; legacy text-only registration is unsupported when search filtering is enabled"))?;
    task.validate()?;
    Ok(Some(task))
}

pub(crate) fn schema(is_required: bool) -> Value {
    json!({
        "type":"object", "additionalProperties":false,
        "properties": {
            "task_query":{"type":"string","minLength":1,"maxLength":MAX_GOAL_BYTES,"description":"The complete current task goal, preserving the user's target, direction and coverage. At most 8192 UTF-8 bytes. Register once per task, not per search query."},
            "questions":{"type":"array","minItems":1,"maxItems":MAX_QUESTIONS,"description":"Derive focused yes/no questions from the task. A yes means the candidate matches that criterion. Preserve indirect and bidirectional flows and ask about supplied evidence; absent cross-file facts mean uncertainty, not false. Entire registration: at most 16384 encoded bytes.","items":{
                "type":"object","additionalProperties":false,"required":["id","question","when_true","when_false"],"properties":{
                    "id":{"type":"string","pattern":"^[A-Za-z0-9_-]{1,32}$","description":"Unique stable criterion id within this task."},
                    "question":{"type":"string","minLength":1,"maxLength":MAX_TEXT_BYTES,"description":"One explicit yes/no question about this candidate and its supplied supporting evidence; at most 2048 UTF-8 bytes."},
                    "when_true":{"type":"string","minLength":1,"maxLength":MAX_TEXT_BYTES,"description":"Facts that establish yes; at most 2048 UTF-8 bytes."},
                    "when_false":{"type":"string","minLength":1,"maxLength":MAX_TEXT_BYTES,"description":"Facts that establish no; missing context alone is insufficient. At most 2048 UTF-8 bytes."}
                }
            }},
            "match":{"type":"string","enum":["all","any"],"default":"all","description":"all requires every criterion; any accepts at least one. Code composes yes/no/uncertain decisions, not a joint probability."}
        },
        "required": if is_required { vec!["task_query","questions"] } else { vec![] }
    })
}
