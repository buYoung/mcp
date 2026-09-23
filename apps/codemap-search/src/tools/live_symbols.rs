//! Indexed declaration sections above live filesystem results.
pub(crate) mod callable;
mod context;
mod diagnostics;
pub(crate) mod jev;
mod locations;
mod references;
mod render;
mod structure;

use super::live_options::{LiveOptions, LiveView};
use crate::index::EngineSupervisor;

pub(super) const PAYLOAD_BYTE_CAP: usize = 8192;
const OUTLINED_FILE_LIMIT: usize = 8;
const TEST_CONTEXT_EXCLUDED_NOTICE: &str = "[Test code excluded from automatic context; set output.context.exclude.should_include_test_code=true to include it.]\n";

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
    // Exact producer-written path prefixes to omit beneath a file heading.
    // Source view retains the original text, including these prefixes.
    pub path_prefixes: Vec<std::ops::Range<usize>>,
    pub jev: jev::Capture,
}

impl LiveOutput {
    fn append_file_source(
        &self,
        file: &LiveFileSpan,
        text: &mut String,
        copies: &mut Vec<jev::SourceCopy>,
    ) {
        let mut start = file.start_byte;
        for prefix in self
            .path_prefixes
            .iter()
            .filter(|prefix| file.start_byte <= prefix.start && prefix.end <= file.end_byte)
        {
            copies.push(jev::SourceCopy {
                original: start..prefix.start,
                rendered_start: text.len(),
            });
            text.push_str(&self.text[start..prefix.start]);
            start = prefix.end;
        }
        copies.push(jev::SourceCopy {
            original: start..file.end_byte,
            rendered_start: text.len(),
        });
        text.push_str(&self.text[start..file.end_byte]);
    }

    fn source_files(&self, options: LiveOptions) -> Vec<crate::analyze::FileObservation> {
        if !matches!(options.view, LiveView::Full | LiveView::Source) {
            return Vec::new();
        }
        let mut files = self
            .source_ranges
            .iter()
            .map(|(path, _, _)| (path.as_str(), 0u64))
            .collect::<std::collections::BTreeMap<_, _>>();
        for span in &self.files {
            if let Some(bytes) = files.get_mut(span.file_path.as_str()) {
                let mut result_bytes = span.end_byte.saturating_sub(span.start_byte);
                if options.view == LiveView::Full {
                    let prefixes = self
                        .path_prefixes
                        .iter()
                        .filter(|p| span.start_byte <= p.start && p.end <= span.end_byte)
                        .map(|p| p.end - p.start)
                        .sum::<usize>();
                    result_bytes = result_bytes.saturating_sub(prefixes);
                }
                *bytes = bytes.saturating_add(result_bytes as u64);
            }
        }
        files
            .into_iter()
            .map(|(path, result_bytes)| crate::analyze::FileObservation {
                path: path.into(),
                result_bytes,
            })
            .collect()
    }

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

fn frame(
    output: &LiveOutput,
    contexts: &[String],
    options: LiveOptions,
    copies: &mut Vec<jev::SourceCopy>,
) -> String {
    let mut text = String::from("# codemap-search\n\n");
    if output.files.is_empty() {
        copies.push(jev::SourceCopy {
            original: 0..output.text.len(),
            rendered_start: text.len(),
        });
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
        for (i, file) in output.files.iter().enumerate() {
            let path = file.file_path.replace('\r', "\\r").replace('\n', "\\n");
            text.push_str(&format!("\n\n## {}. {path}\n\n", i + 1));
            let context = &contexts[i];
            if context.is_empty() {
                text.push_str("[Symbol context omitted by output/file budget.]\n");
            } else {
                text.push_str(context);
            }
            if options.view == LiveView::Full {
                text.push_str("\n### results\n");
                output.append_file_source(file, &mut text, copies);
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

pub(crate) struct PreparedLiveOutput {
    pub text: String,
    pub source_files: Vec<crate::analyze::FileObservation>,
    pub capture: jev::Capture,
    pub copies: Vec<jev::SourceCopy>,
}

/// Finalize the base response once before any optional inference. Source copy boundaries
/// allow replacing bodies without reparsing formatted output or rerendering context.
pub(crate) fn prepare(
    engine: &EngineSupervisor,
    mut output: LiveOutput,
    output_byte_cap: Option<usize>,
    options: LiveOptions,
) -> Result<PreparedLiveOutput, (i64, String)> {
    let source_files = output.source_files(options);
    let mut copies = Vec::new();
    let text = if options.view == LiveView::Source {
        copies.push(jev::SourceCopy {
            original: 0..output.text.len(),
            rendered_start: 0,
        });
        let text = if output.notices.is_empty() {
            std::mem::take(&mut output.text)
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
        text
    } else {
        let empty = frame(
            &output,
            &vec![String::new(); output.files.len()],
            options,
            &mut Vec::new(),
        );
        let limit = output_byte_cap.unwrap_or(usize::MAX);
        if empty.len() > limit {
            return Err((-32602, "Read window leaves no room for file/section headers; retry with a smaller limit or view=source.".into()));
        }
        let cap = limit.saturating_sub(empty.len()).min(PAYLOAD_BYTE_CAP * 2);
        let contexts = context::build(engine, &output, cap, options);
        let text = frame(&output, &contexts, options, &mut copies);
        if text.len() > limit {
            return Err((-32602, "Read window plus symbol context exceeds the output cap; retry with a smaller limit.".into()));
        }
        text
    };
    Ok(PreparedLiveOutput {
        text,
        source_files,
        capture: output.jev,
        copies,
    })
}
