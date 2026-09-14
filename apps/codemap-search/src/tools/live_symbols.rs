//! Indexed member and call context above untouched live filesystem results.
pub(crate) mod callable;
mod context;
mod diagnostics;
mod locations;
mod references;
mod render;
mod structure;

use super::live_options::{LiveOptions, LiveView};
use crate::index::EngineSupervisor;

pub(super) const PAYLOAD_BYTE_CAP: usize = 8192;
const OUTLINED_FILE_LIMIT: usize = 8;
const SHARED_NOTICE_CAP: usize = 256;
const TEST_CONTEXT_EXCLUDED_NOTICE: &str = "[Test code excluded from automatic context; set exclude.should_include_test_code=true to include it.]\n";

#[derive(Clone)]
pub(crate) struct LiveAnchor {
    pub file_path: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

pub(crate) struct LiveFileSpan {
    pub file_path: String,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Default)]
pub(crate) struct LiveOutput {
    pub text: String,
    pub anchors: Vec<LiveAnchor>,
    pub notices: Vec<String>,
    pub files: Vec<LiveFileSpan>,
    pub footer: Option<String>,
    // Fully emitted source only; matching anchors also include column omissions.
    pub source_ranges: Vec<(String, usize, usize)>,
}

impl LiveOutput {
    pub fn record_source(&mut self, path: &str, start: usize, end: usize) {
        if let Some(last) = self
            .source_ranges
            .last_mut()
            .filter(|last| last.0 == path && start >= last.1 && start <= last.2.saturating_add(1))
        {
            last.2 = last.2.max(end);
        } else {
            self.source_ranges.push((path.into(), start, end));
        }
    }

    /// Record boundaries while producing live text, never by parsing source or paths
    /// back out of formatted grep rows. Keep the producer's page/file order.
    pub fn record_file(&mut self, path: &str, start_byte: usize, end_byte: usize) {
        if let Some(last) = self.files.last_mut().filter(|last| last.file_path == path) {
            last.end_byte = end_byte;
        } else {
            self.files.push(LiveFileSpan {
                file_path: path.into(),
                start_byte,
                end_byte,
            });
        }
    }
}

pub(super) struct ReadHints<'a> {
    source_ranges: &'a [(String, usize, usize)],
    shown: std::collections::BTreeSet<(String, usize)>,
}

impl<'a> ReadHints<'a> {
    fn new(output: &'a LiveOutput, options: LiveOptions) -> Self {
        Self {
            source_ranges: if options.view == LiveView::Full {
                &output.source_ranges
            } else {
                &[]
            },
            shown: Default::default(),
        }
    }

    fn next(&self, path: &str, line: usize) -> Option<String> {
        if self.shown.len() >= 3
            || self.shown.contains(&(path.into(), line))
            || self
                .source_ranges
                .iter()
                .any(|(file, start, end)| file == path && *start <= line && line <= *end)
        {
            return None;
        }
        Some(format!(
            "read {}",
            serde_json::json!({"file_path":path,"offset":line.max(1),"limit":1})
        ))
    }

    fn record(&mut self, path: &str, line: usize) {
        self.shown.insert((path.into(), line));
    }
}

pub(super) fn bounded_notice(notice: &str, cap: usize) -> String {
    if notice.len() <= cap {
        return notice.into();
    }
    let fallback = "[Context omitted by output budget; narrow this file/window.]\n";
    if fallback.len() <= cap {
        fallback.into()
    } else {
        String::new()
    }
}

fn frame(output: &LiveOutput, contexts: &[String], notice: &str, options: LiveOptions) -> String {
    let mut text = String::from("# codemap-search\n\n");
    if output.files.is_empty() {
        text.push_str(&output.text);
    } else {
        text.push_str("Locations without a path refer to the enclosing file. Scope: enclosing declarations and members; detailed relationships cover returned source anchors.\n");
        if options.should_include_relations() {
            if let Some(target_os) = crate::config::get().analysis_target_os.as_deref() {
                if output
                    .files
                    .iter()
                    .any(|file| file.file_path.ends_with(".rs"))
                {
                    text.push_str(&format!("[Rust analysis target_os={target_os}; static source conditions, not a runtime execution guarantee.]\n"));
                }
            }
        }
        if output.files.len() > OUTLINED_FILE_LIMIT {
            text.push_str(&format!(
                "[File limit: {} returned files not outlined; narrow the path for their context.]\n",
                output.files.len() - OUTLINED_FILE_LIMIT,
            ));
        }
        text.push_str(notice);
        for (i, file) in output.files.iter().enumerate() {
            let path = file.file_path.replace('\r', "\\r").replace('\n', "\\n");
            let section = if options.view == LiveView::Relations {
                "relations"
            } else {
                "symbols"
            };
            text.push_str(&format!("\n\n## {}. {path}\n\n### {section}\n\n", i + 1));
            let context = &contexts[i];
            if context.is_empty() {
                text.push_str("[Symbol context omitted by output/file budget.]\n");
            } else {
                text.push_str(context);
            }
            if options.view == LiveView::Full {
                text.push_str("\n### results\n");
                text.push_str(&output.text[file.start_byte..file.end_byte]);
            }
        }
        if let Some(footer) = &output.footer {
            text.push('\n');
            text.push_str(footer);
        }
    }
    for notice in &output.notices {
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(notice);
    }
    text
}

/// Source stays byte-for-byte compatible. Rich views reserve every returned source
/// row and file heading before sharing one bounded context budget across files.
pub(crate) fn append(
    engine: &EngineSupervisor,
    output: LiveOutput,
    output_byte_cap: Option<usize>,
    options: LiveOptions,
) -> Result<String, (i64, String)> {
    if options.view == LiveView::Source {
        let text = if output.notices.is_empty() {
            output.text
        } else {
            format!("{}\n{}", output.text, output.notices.join("\n"))
        };
        if output_byte_cap.is_some_and(|cap| text.len() > cap) {
            return Err((
                -32602,
                "Source output plus expansion notices exceeds the output cap; narrow the request."
                    .into(),
            ));
        }
        return Ok(text);
    }
    let empty = frame(
        &output,
        &vec![String::new(); output.files.len()],
        "",
        options,
    );
    let limit = output_byte_cap.unwrap_or(usize::MAX);
    if empty.len() > limit {
        return Err((-32602, "Read window leaves no room for file/section headers; retry with a smaller limit or view=source.".into()));
    }
    let cap = limit.saturating_sub(empty.len()).min(PAYLOAD_BYTE_CAP * 2);
    let (contexts, notice) = context::build(engine, &output, cap, options);
    let text = frame(&output, &contexts, &notice, options);
    if text.len() > limit {
        return Err((
            -32602,
            "Read window plus symbol context exceeds the output cap; retry with a smaller limit."
                .into(),
        ));
    }
    Ok(text)
}
