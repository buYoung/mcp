//! Apply redaction while preserving the JSON-RPC response structure.
use super::{detection::SourceScan, is_enabled, named_value, pii, rules, source};

fn response_text(text: &str, is_tool_text: bool) -> String {
    if !is_enabled() {
        return text.to_string();
    }
    if is_tool_text {
        // Source producers inspect full originals before slicing/formatting. Only
        // context-free rules run again here: guessing assignments in formatted code
        // would undo AST decisions about references and type declarations.
        let mut detections = rules::detect(text);
        // Label-based PII rules already ran on complete originals. Formatting can
        // place labels next to unrelated line numbers or symbol references.
        detections.extend(pii::detect_unambiguous(text));
        return SourceScan::finish(text, detections)
            .render(text)
            .into_owned();
    }
    // Unrelated metadata lines must not share a quote/block continuation.
    text.split_inclusive('\n')
        .map(|line| source(line).into_owned())
        .collect()
}

/// Masks text that leaves the process for external evaluation exactly as tool text is
/// masked in responses, so an evaluator never receives more than the client would.
pub(crate) fn presentation_text(text: &str) -> String {
    response_text(text, true)
}

/// Preserve JSON-RPC ids, object keys and schema structure. Named string fields keep
/// their existing treatment; parent objects/arrays do not acquire field sensitivity.
pub(crate) fn response(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => *text = response_text(text, false),
        serde_json::Value::Array(items) => items.iter_mut().for_each(response),
        serde_json::Value::Object(fields) => {
            let is_text_content =
                fields.get("type").and_then(serde_json::Value::as_str) == Some("text");
            for (name, value) in fields {
                if let serde_json::Value::String(text) = value {
                    *text = if rules::is_sensitive_key(name) {
                        named_value(name, text).into_owned()
                    } else {
                        response_text(text, is_text_content && name == "text")
                    };
                } else {
                    response(value);
                }
            }
        }
        _ => {}
    }
}
