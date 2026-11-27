//! Multi-window delivery reuses the single-read pipeline, including Jev and history.
use super::{jev, source_history::SourceHistory, text_content, tool_error_result, McpServer};
use crate::tools::{
    live_options::{LiveOptions, LiveView},
    read,
};
use serde_json::Value;

fn truncation_note(first: usize, total: usize) -> String {
    if first == 1 {
        return format!("[Truncated before range 1 of {total}; retry from ranges[0] with a single read (file_path/offset/limit) or smaller windows.]");
    }
    format!("[Truncated before range {first} of {total}; retry from ranges[{}] with fewer ranges or smaller windows.]", first - 1)
}

fn session_truncation_note(first_index: usize, ranges: &[Value]) -> String {
    let deferred = &ranges[first_index..];
    let mut specs: Vec<String> = deferred
        .iter()
        .take(3)
        .map(|range| crate::redact::source(&read::range_brief_spec(range)).into_owned())
        .collect();
    if deferred.len() > 3 {
        specs.push(format!("+{} more", deferred.len() - 3));
    }
    let summary = specs.join(", ");
    if first_index == 0 {
        return format!(
            "[Deferred ranges[0..{}] ({summary}) for 10 KiB client limit; retry with a single read or smaller windows.]",
            ranges.len()
        );
    }
    format!(
        "[Deferred ranges[{first_index}..{}] ({summary}) for 10 KiB client limit; call read with only the remaining ranges you still need.]",
        ranges.len()
    )
}

impl McpServer {
    async fn read_range_item(
        &mut self,
        arguments: &Value,
        range: &Value,
        config: &crate::config::ResolvedConfig,
        deadline: Option<tokio::time::Instant>,
        item_cap: Option<usize>,
    ) -> (Value, Vec<crate::analyze::FileObservation>) {
        let _scope = item_cap.map(|c| crate::config::pin_session_delivery_cap(Some(c)));
        let mut source_files = Vec::new();
        let response = match read::range_arguments(arguments, range) {
            Ok(item_arguments) => {
                let tool_arguments = SourceHistory::tool_arguments("read", &item_arguments);
                match jev::live(
                    &mut self.jev,
                    &self.engine,
                    "read",
                    tool_arguments.as_ref(),
                    self.registered_task.as_ref(),
                    config,
                    deadline,
                )
                .await
                {
                    Ok(output) => {
                        source_files = output.source_files;
                        let mut response = text_content(&output.text);
                        crate::redact::response(&mut response);
                        let folded =
                            self.source_history
                                .prepare("read", &item_arguments, &mut response);
                        for file in &mut source_files {
                            if let Some(bytes) = folded.removed_source_bytes.get(&file.path) {
                                file.result_bytes = file.result_bytes.saturating_sub(*bytes as u64);
                            }
                        }
                        source_files.retain(|file| file.result_bytes > 0);
                        response
                    }
                    Err((_, message)) => tool_error_result(&crate::redact::source(&message)),
                }
            }
            Err((_, message)) => tool_error_result(&crate::redact::source(&message)),
        };
        (response, source_files)
    }

    /// Fold and budget each item before accepting it. Only accepted items may enter
    /// pending history/accounting; the usual successful stdio flush commits them.
    pub(super) async fn read_ranges(&mut self, arguments: &Value) -> Result<Value, (i64, String)> {
        let ranges = read::ranges(arguments)?.expect("ranges dispatch requires the argument");
        crate::tools::reject_body_task_query(arguments)?;
        let options = LiveOptions::parse(arguments)?;
        let config = crate::config::get();
        if config.jev.is_enabled_for("read")
            && matches!(options.view, LiveView::Full | LiveView::Source)
        {
            jev::require_task(self.registered_task.as_ref(), "read")?;
        }
        // A batch is still one tool call: later windows do not restart Jev's timeout.
        let deadline = tokio::time::Instant::now().checked_add(config.jev.deadline());
        let is_session_capped = crate::config::session_delivery_byte_cap().is_some();
        let cap = crate::config::effective_client_delivery_byte_cap()
            .map_or(config.read_output_byte_cap, |delivery| {
                delivery.min(config.read_output_byte_cap)
            });
        // Reserve a whole-item continuation note, including its separator. Never cut
        // a UTF-8 character, source row, callable or duplicate marker mid-item.
        let reserve = if is_session_capped {
            (0..ranges.len())
                .map(|i| session_truncation_note(i, ranges).len())
                .max()
                .unwrap_or(0)
                + 2
        } else {
            truncation_note(ranges.len(), ranges.len()).len() + 2
        };
        let min_note = if is_session_capped {
            session_truncation_note(0, ranges).len()
        } else {
            truncation_note(1, ranges.len()).len()
        };
        if cap < reserve.max(min_note) {
            return Err((-32602, format!("Read ranges output cap ({cap} bytes) cannot fit the truncation notice for range 1. Increase output.read.max_bytes/output.client limits or use a single read.")));
        }
        let _ = self.engine.activate();
        let mut text = String::new();
        for (index, range) in ranges.iter().enumerate() {
            let checkpoint = self.source_history.pending_checkpoint();
            let heading = read::range_heading(index + 1, range);
            let heading = crate::redact::source(&heading).into_owned();
            let separator = if text.is_empty() { "" } else { "\n\n" };
            let remaining_reserve = if index + 1 < ranges.len() { reserve } else { 0 };
            let remaining_room = cap.saturating_sub(
                text.len() + separator.len() + heading.len() + remaining_reserve + 64,
            );
            // For connection-capped clients, tighten range[0]'s per-item budget by its own
            // heading + deferral note reserve so range[0] auto-clips cleanly instead of
            // returning an empty batch. Later ranges first attempt full delivery; if a later
            // range overflows but >= 2.4 KiB remains, re-fetch it clipped to fill the room
            // rather than leaving half the 10 KiB response empty.
            let initial_cap = (is_session_capped && index == 0).then(|| remaining_room.max(1_024));
            let (mut response, mut source_files) = self
                .read_range_item(arguments, range, &config, deadline, initial_cap)
                .await;
            let body_len = response["content"][0]["text"].as_str().map_or(0, str::len);
            let next_len = text.len() + separator.len() + heading.len() + body_len;
            if next_len.saturating_add(remaining_reserve) > cap {
                self.source_history.restore_pending(checkpoint);
                if is_session_capped && index > 0 && remaining_room >= 2_400 {
                    let retry_checkpoint = self.source_history.pending_checkpoint();
                    let (clipped_response, clipped_files) = self
                        .read_range_item(arguments, range, &config, deadline, Some(remaining_room))
                        .await;
                    let clipped_body = clipped_response["content"][0]["text"]
                        .as_str()
                        .unwrap_or("");
                    let clipped_len =
                        text.len() + separator.len() + heading.len() + clipped_body.len();
                    if clipped_len.saturating_add(remaining_reserve) <= cap {
                        response = clipped_response;
                        source_files = clipped_files;
                        let body = response["content"][0]["text"].as_str().unwrap_or("");
                        text.push_str(separator);
                        text.push_str(&heading);
                        text.push_str(body);
                        self.pending_source_files.extend(source_files);
                        if index + 1 < ranges.len() {
                            text.push_str("\n\n");
                            text.push_str(&session_truncation_note(index + 1, ranges));
                        }
                        break;
                    }
                    self.source_history.restore_pending(retry_checkpoint);
                }
                text.push_str(separator);
                let note = if is_session_capped {
                    session_truncation_note(index, ranges)
                } else {
                    truncation_note(index + 1, ranges.len())
                };
                text.push_str(&note);
                break;
            }
            let body = response["content"][0]["text"].as_str().unwrap_or("");
            text.push_str(separator);
            text.push_str(&heading);
            text.push_str(body);
            self.pending_source_files.extend(source_files);
        }
        Ok(text_content(&text))
    }
}
