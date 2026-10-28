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

impl McpServer {
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
        let cap = config
            .client_output
            .delivery_byte_cap()
            .map_or(config.read_output_byte_cap, |delivery| {
                delivery.min(config.read_output_byte_cap)
            });
        // Reserve a whole-item continuation note, including its separator. Never cut
        // a UTF-8 character, source row, callable or duplicate marker mid-item.
        let reserve = truncation_note(ranges.len(), ranges.len()).len() + 2;
        if cap < reserve.max(truncation_note(1, ranges.len()).len()) {
            return Err((-32602, format!("Read ranges output cap ({cap} bytes) cannot fit the truncation notice for range 1. Increase output.read.max_bytes/output.client limits or use a single read.")));
        }
        let _ = self.engine.activate();
        let mut text = String::new();
        for (index, range) in ranges.iter().enumerate() {
            let checkpoint = self.source_history.pending_checkpoint();
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
                        &config,
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
                                    file.result_bytes =
                                        file.result_bytes.saturating_sub(*bytes as u64);
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
            let heading = read::range_heading(index + 1, range);
            let heading = crate::redact::source(&heading);
            let body = response["content"][0]["text"].as_str().unwrap_or("");
            let separator = if text.is_empty() { "" } else { "\n\n" };
            let next_len = text.len() + separator.len() + heading.len() + body.len();
            let remaining_reserve = if index + 1 < ranges.len() { reserve } else { 0 };
            if next_len.saturating_add(remaining_reserve) > cap {
                self.source_history.restore_pending(checkpoint);
                text.push_str(separator);
                text.push_str(&truncation_note(index + 1, ranges.len()));
                break;
            }
            text.push_str(separator);
            text.push_str(&heading);
            text.push_str(body);
            self.pending_source_files.extend(source_files);
        }
        Ok(text_content(&text))
    }
}
