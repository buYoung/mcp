use serde_json::Value;

/// Reject ignored controls before starting search or index lifecycle work. Match
/// the existing readers: only get_arg-based controls accept normalized aliases.
pub(crate) fn validate(arguments: &Value) -> Result<(), (i64, String)> {
    let object = arguments.as_object().ok_or_else(|| {
        (
            -32602,
            "Invalid search arguments: expected an object.".into(),
        )
    })?;
    let unsupported: Vec<_> = object
        .keys()
        .filter(|key| {
            !matches!(
                key.as_str(),
                "query" | "caller_context" | "language_hint" | "extension_hint"
            ) && !matches!(
                crate::tools::normalize_arg_key(key).as_str(),
                "includeevents" | "eventkey" | "workspacescope" | "scope" | "debug"
            )
        })
        .collect();
    if unsupported.is_empty() {
        crate::tools::live_options::debug_requested(arguments)?;
        return Ok(());
    }
    let names = unsupported
        .iter()
        .take(8)
        .map(|key| {
            let mut name: String = key.chars().take(64).collect();
            if key.chars().count() > 64 {
                name.push('…');
            }
            serde_json::json!(name).to_string()
        })
        .collect::<Vec<_>>()
        .join(", ");
    let more = if unsupported.len() > 8 { ", …" } else { "" };
    Err((-32602, format!(
        "Unsupported search arguments: {names}{more}. Supported: query, caller_context, language_hint, extension_hint, debug, include_events, event_key, workspace_scope (alias: scope). Use a workspace_scope listed by root overview to restrict search; path and per-request limit are not supported."
    )))
}
