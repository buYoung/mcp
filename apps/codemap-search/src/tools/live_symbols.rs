//! Indexed declaration sections above live filesystem results.
pub(crate) mod callable;
mod context;
mod diagnostics;
mod implementations_trailer;
mod importers;
pub(crate) mod jev;
mod locations;
mod outside_callees;
mod outside_callers;
mod references;
pub(crate) mod shared_keys;

pub(crate) use implementations_trailer::HEADER as IMPLEMENTATIONS_HEADER;
pub(crate) use importers::HEADER as IMPORTERS_HEADER;
pub(crate) use outside_callees::HEADER as OUTSIDE_CALLEES_HEADER;
pub(crate) use outside_callers::HEADER as OUTSIDE_CALLERS_HEADER;
pub(crate) use shared_keys::HEADER as SHARED_KEYS_HEADER;
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
    /// A read window: source view appends callers from outside it.
    pub should_list_outside_callers: bool,
    /// Original requested end line when pre-clipped for a connection-scoped delivery cap.
    pub requested_end: Option<usize>,
    /// Whether the read requested `expand=callable`, preserved for continuation notices.
    pub clipped_expand_callable: bool,
}

impl LiveOutput {
    pub(crate) fn source_files(
        &self,
        options: LiveOptions,
    ) -> Vec<crate::analyze::FileObservation> {
        use crate::tools::live_options::LiveView;
        if !matches!(
            options.view,
            LiveView::Full | LiveView::Source | LiveView::SourceGrouped
        ) {
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
                if matches!(options.view, LiveView::Full | LiveView::SourceGrouped) {
                    // Full view removes the producer-written path prefixes under file headings.
                    let prefix_bytes = self
                        .path_prefixes
                        .iter()
                        .filter(|prefix| {
                            span.start_byte <= prefix.start && prefix.end <= span.end_byte
                        })
                        .map(|prefix| prefix.end - prefix.start)
                        .sum::<usize>();
                    result_bytes = result_bytes.saturating_sub(prefix_bytes);
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
        // The grep schema already defines the grouped row markers.
        if options.view != LiveView::SourceGrouped {
            text.push_str("Locations without a path refer to the enclosing file. Scope: enclosing declarations and members; detailed relationships cover returned source anchors.\n");
        }
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
        if options.view != LiveView::SourceGrouped && output.files.len() > OUTLINED_FILE_LIMIT {
            text.push_str(&format!(
                "[File limit: {} returned files not outlined; narrow the path for their context.]\n",
                output.files.len() - OUTLINED_FILE_LIMIT,
            ));
        }
        for (i, file) in output.files.iter().enumerate() {
            let path = file.file_path.replace('\r', "\\r").replace('\n', "\\n");
            text.push_str(&format!("\n\n## {}. {path}\n\n", i + 1));
            if options.view != LiveView::SourceGrouped {
                let context = &contexts[i];
                if context.is_empty() {
                    text.push_str("[Symbol context omitted by output/file budget.]\n");
                } else {
                    text.push_str(context);
                }
            }
            if matches!(options.view, LiveView::Full | LiveView::SourceGrouped) {
                if options.view == LiveView::Full {
                    text.push_str("\n### results\n");
                }
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

/// Collect source trailers under a fair-share total budget so an early trailer (e.g. outside
/// callers) never starves outgoing callees, implementations, shared keys, or importers, while
/// redistributing any surplus from smaller trailers (water-filling) so budgeted headroom is never
/// wasted.
fn collect_fair_trailers(
    engine: &EngineSupervisor,
    output: &LiveOutput,
    caps: [usize; 5],
    total_budget: usize,
) -> Vec<String> {
    if !output.should_list_outside_callers || total_budget < 128 {
        return Vec::new();
    }
    let query = |idx: usize, cap: usize| -> Option<String> {
        let cap = cap.min(total_budget.saturating_sub(1));
        match idx {
            0 => outside_callers::trailer(engine, output, cap),
            1 => outside_callees::trailer(engine, output, cap),
            2 => implementations_trailer::trailer(engine, output, cap),
            3 => shared_keys::trailer(engine, output, cap),
            _ => importers::trailer(engine, output, cap),
        }
    };
    let mut results: [Option<String>; 5] = [
        query(0, caps[0]),
        query(1, caps[1]),
        query(2, caps[2]),
        query(3, caps[3]),
        query(4, caps[4]),
    ];
    let total_bytes: usize = results
        .iter()
        .flatten()
        .map(|trailer| trailer.len() + 1)
        .sum();
    if total_bytes > total_budget {
        let mut remaining_budget = total_budget;
        let mut active: Vec<usize> = (0..5).filter(|&idx| results[idx].is_some()).collect();
        let mut alloc = [0usize; 5];
        while !active.is_empty() {
            let share = remaining_budget / active.len();
            let mut locked_any = false;
            let mut next_active = Vec::with_capacity(active.len());
            for idx in active {
                let size = results[idx].as_ref().map_or(0, |t| t.len() + 1);
                if size <= share {
                    alloc[idx] = size;
                    remaining_budget = remaining_budget.saturating_sub(size);
                    locked_any = true;
                } else {
                    next_active.push(idx);
                }
            }
            if !locked_any {
                let extra = remaining_budget % next_active.len();
                for (pos, idx) in next_active.into_iter().enumerate() {
                    alloc[idx] = share + usize::from(pos < extra);
                }
                break;
            }
            active = next_active;
        }
        for (idx, slot) in results.iter_mut().enumerate() {
            if slot.as_ref().is_some_and(|t| t.len() + 1 > alloc[idx]) {
                let cap = alloc[idx].saturating_sub(1);
                *slot = if cap >= 128 { query(idx, cap) } else { None };
            }
        }
    }
    let mut used = 0usize;
    let mut out = Vec::new();
    for trailer in results.into_iter().flatten() {
        if used + trailer.len() < total_budget {
            used += trailer.len() + 1;
            out.push(trailer);
        }
    }
    out
}

/// Source rows stay byte-for-byte compatible; a read window may end with its outside callers.
/// Rich views reserve every returned source row and file heading before sharing one bounded
/// context budget across files.
fn render(
    engine: &EngineSupervisor,
    output: LiveOutput,
    output_byte_cap: Option<usize>,
    options: LiveOptions,
    copies: &mut Vec<jev::SourceCopy>,
) -> Result<String, (i64, String)> {
    if options.view == LiveView::Source {
        let mut output = output;
        copies.push(jev::SourceCopy {
            original: 0..output.text.len(),
            rendered_start: 0,
        });
        let mut source = std::mem::take(&mut output.text);
        let mut text = if output.notices.is_empty() {
            source.clone()
        } else {
            format!("{source}\n{}", output.notices.join("\n"))
        };
        if output_byte_cap.is_some_and(|cap| text.len() > cap) {
            return Err((
                -32602,
                "Source output plus expansion notices exceeds the output cap; narrow the request."
                    .into(),
            ));
        }
        if crate::config::session_delivery_byte_cap().is_some() {
            const CAPPED_CAPS: [usize; 5] = [768, 512, 576, 768, 512];
            let cap = output_byte_cap.unwrap_or(usize::MAX);
            let free_room = cap.saturating_sub(text.len());
            let trailer_budget = if free_room >= 1_600 {
                free_room.min(2_400)
            } else {
                1_600.min(cap / 3)
            };
            let mut trailers = collect_fair_trailers(engine, &output, CAPPED_CAPS, trailer_budget);
            let trailers_bytes: usize = trailers.iter().map(|t| t.len() + 1).sum();
            if text.len().saturating_add(trailers_bytes) > cap
                && trailers_bytes > 0
                && output.anchors.len() == 1
            {
                if let Some((start, end)) =
                    output.anchors[0].start_line.zip(output.anchors[0].end_line)
                {
                    let non_clip_notices: Vec<String> = output
                        .notices
                        .iter()
                        .filter(|notice| !notice.starts_with("[Clipped at L"))
                        .cloned()
                        .collect();
                    let non_clip_bytes: usize = non_clip_notices.iter().map(|n| n.len() + 1).sum();
                    let orig_end = output.requested_end.unwrap_or(end);
                    let expand_hint = if output.clipped_expand_callable {
                        "expand=none, "
                    } else {
                        ""
                    };
                    let clip_notice_reserve = 148usize;
                    let target_source =
                        cap.saturating_sub(trailers_bytes + non_clip_bytes + clip_notice_reserve);
                    let lines: Vec<&str> = source.split('\n').collect();
                    let mut kept = 0usize;
                    let mut used = 0usize;
                    for (i, line) in lines.iter().enumerate() {
                        let added = line.len() + usize::from(i > 0);
                        if kept > 0 && used.saturating_add(added) > target_source {
                            break;
                        }
                        used = used.saturating_add(added);
                        kept += 1;
                    }
                    if kept > 0 && kept < lines.len() {
                        source = lines[..kept].join("\n");
                        let new_end = start.saturating_add(kept - 1);
                        output.anchors[0].end_line = Some(new_end);
                        if let Some(copy) = copies.last_mut() {
                            copy.original = 0..source.len();
                        }
                        let mut notices = non_clip_notices;
                        notices.push(format!(
                            "[Clipped at L{new_end} of requested L{start}-L{orig_end} (10 KiB client limit); call read with {expand_hint}offset={} if needed.]",
                            new_end + 1
                        ));
                        text = format!("{source}\n{}", notices.join("\n"));
                        let remaining = cap.saturating_sub(text.len());
                        trailers = collect_fair_trailers(engine, &output, CAPPED_CAPS, remaining);
                    }
                }
            }
            for trailer in trailers {
                if text.len().saturating_add(1 + trailer.len()) <= cap {
                    text.push('\n');
                    text.push_str(&trailer);
                }
            }
            return Ok(text);
        }
        // The trailers are auxiliary: use only the room left under the cap, bounded to keep
        // multi-trailer windows compact while giving every individual trailer its full headroom.
        const STANDARD_CAPS: [usize; 5] = [1_536, 1_024, 1_024, 1_536, 1_536];
        const STANDARD_TRAILER_BUDGET: usize = 4_608;
        let room = output_byte_cap.map_or(usize::MAX, |cap| cap.saturating_sub(text.len()));
        let budget = room.min(STANDARD_TRAILER_BUDGET);
        for trailer in collect_fair_trailers(engine, &output, STANDARD_CAPS, budget) {
            if output_byte_cap.is_none_or(|cap| text.len().saturating_add(1 + trailer.len()) <= cap)
            {
                text.push('\n');
                text.push_str(&trailer);
            }
        }
        return Ok(text);
    }
    if options.view == LiveView::SourceGrouped {
        let text = frame(&output, &[], options, copies);
        if output_byte_cap.is_some_and(|cap| text.len() > cap) {
            return Err((
                -32602,
                "Grouped source output exceeds the output cap; narrow the request.".into(),
            ));
        }
        return Ok(text);
    }
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
    let text = frame(&output, &contexts, options, copies);
    if text.len() > limit {
        return Err((
            -32602,
            "Read window plus symbol context exceeds the output cap; retry with a smaller limit."
                .into(),
        ));
    }
    Ok(text)
}

pub(crate) struct PreparedLiveOutput {
    pub text: String,
    pub source_files: Vec<crate::analyze::FileObservation>,
    pub capture: jev::Capture,
    pub copies: Vec<jev::SourceCopy>,
}

/// Freeze the exact rendered response and source mappings before asynchronous judgment.
pub(crate) fn prepare(
    engine: &EngineSupervisor,
    mut output: LiveOutput,
    output_byte_cap: Option<usize>,
    options: LiveOptions,
) -> Result<PreparedLiveOutput, (i64, String)> {
    let source_files = output.source_files(options);
    let capture = std::mem::take(&mut output.jev);
    let mut copies = Vec::new();
    let text = render(engine, output, output_byte_cap, options, &mut copies)?;
    Ok(PreparedLiveOutput {
        text,
        source_files,
        capture,
        copies,
    })
}
