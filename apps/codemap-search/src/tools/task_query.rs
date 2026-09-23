//! The caller's original task intent for the optional Jev modes of `overview` and `search`.
//! It is taken only from an explicit `task_query` argument: never from `query`, a path, an
//! earlier request, or a transcript.

use serde_json::Value;

/// Advertised as the schema `maxLength`, so it counts characters like JSON Schema does.
pub(crate) const MAX_TASK_QUERY_CHARS: usize = 2_000;

/// `Ok(None)` when the argument is absent or blank, so a blank intent never starts an
/// evaluation. Any other non-string value is an invalid-params error.
pub(crate) fn parse(arguments: &Value) -> Result<Option<String>, (i64, String)> {
    let Some(value) = arguments.get("task_query") else {
        return Ok(None);
    };
    let Some(text) = value.as_str() else {
        return Err((
            -32602,
            "Invalid 'task_query': expected a string with the user's original task.".into(),
        ));
    };
    if text.chars().count() > MAX_TASK_QUERY_CHARS {
        return Err((
            -32602,
            format!("Invalid 'task_query': expected at most {MAX_TASK_QUERY_CHARS} characters."),
        ));
    }
    let text = text.trim();
    Ok((!text.is_empty()).then(|| text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_a_non_blank_bounded_string_is_an_intent() {
        assert_eq!(parse(&json!({})).unwrap(), None);
        assert_eq!(parse(&json!({"task_query": "  \n "})).unwrap(), None);
        assert_eq!(
            parse(&json!({"task_query": " Why does retry stop? "})).unwrap(),
            Some("Why does retry stop?".to_string())
        );
        let longest = "가".repeat(MAX_TASK_QUERY_CHARS);
        assert_eq!(
            parse(&json!({ "task_query": longest })).unwrap(),
            Some(longest)
        );
        for invalid in [
            json!({"task_query": null}),
            json!({"task_query": 7}),
            json!({"task_query": ["retry"]}),
            json!({"task_query": {"text": "retry"}}),
            json!({"task_query": "a".repeat(MAX_TASK_QUERY_CHARS + 1)}),
        ] {
            assert_eq!(parse(&invalid).unwrap_err().0, -32602, "{invalid}");
        }
        assert_eq!(parse(&json!({"query": "src/lib.rs"})).unwrap(), None);
    }
}
