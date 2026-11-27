//! Stream selected callable groups into a bounded result; never retain workspace bodies.
use super::{cap_line, LineHit};
use crate::tools::live_symbols::{callable, LiveAnchor, LiveOutput};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(super) struct ExpansionPage {
    offset: usize,
    limit: usize,
    total: usize,
    next: usize,
    cap: usize,
    used: usize,
    rendered_used: usize,
    is_grouped_session_cap: bool,
    last_file: Option<String>,
    has_full_page: bool,
    text: Vec<String>,
    anchors: Vec<LiveAnchor>,
    source_lines: Vec<Vec<usize>>,
    path_prefixes: Vec<Vec<std::ops::Range<usize>>>,
    jev: crate::tools::live_symbols::jev::Capture,
}

impl ExpansionPage {
    pub fn new(offset: usize, limit: usize, view: crate::tools::live_options::LiveView) -> Self {
        let base_cap = crate::config::get().grep_output_byte_cap;
        let session = crate::config::session_delivery_byte_cap();
        let cap = match session {
            Some(session_cap) => {
                let effective = base_cap.min(session_cap);
                let reserve = match view {
                    crate::tools::live_options::LiveView::Full => 1_800,
                    _ => 384,
                };
                effective.saturating_sub(reserve)
            }
            None => base_cap.saturating_sub(512),
        };
        let is_grouped_session_cap = session.is_some()
            && matches!(
                view,
                crate::tools::live_options::LiveView::SourceGrouped
                    | crate::tools::live_options::LiveView::Full
            );
        Self {
            offset,
            limit: if limit == 0 { usize::MAX } else { limit },
            total: 0,
            next: offset,
            cap,
            used: 0,
            rendered_used: 0,
            is_grouped_session_cap,
            last_file: None,
            has_full_page: false,
            text: Vec::new(),
            anchors: Vec::new(),
            source_lines: Vec::new(),
            path_prefixes: Vec::new(),
            jev: Default::default(),
        }
    }

    pub fn add_file(
        &mut self,
        path: &str,
        bytes: Option<&[u8]>,
        hits: &[LineHit],
        show_lines: bool,
        max_columns: usize,
        should_capture_bodies: bool,
    ) {
        let source = bytes
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .map(|s| s.strip_prefix('\u{feff}').unwrap_or(s));
        let ranges = source
            .ok_or_else(|| "live UTF-8 buffer unavailable or exceeds parsing input cap".to_string())
            .and_then(|source| callable::bounds(Path::new(path), source));
        let mut groups: BTreeMap<(usize, usize), (BTreeSet<usize>, Option<String>)> =
            BTreeMap::new();
        for hit in hits.iter().filter(|h| h.is_match) {
            let line = hit.line_number as usize;
            let found = ranges
                .as_ref()
                .map_err(Clone::clone)
                .and_then(|r| callable::containing(r, line));
            let (key, reason) = match found {
                Ok(r) => ((r.start, r.end), None),
                Err(reason) => ((line, line), Some(reason)),
            };
            groups
                .entry(key)
                .or_insert_with(|| (BTreeSet::new(), reason))
                .0
                .insert(line);
        }
        // Parse and match original bytes; only the presentation copy is masked before slicing.
        let masked = source.map(|source| crate::redact::in_file(Path::new(path), source));
        let source_lengths = source
            .map(|s| {
                s.split('\n')
                    .map(|line| line.strip_suffix('\r').unwrap_or(line).len())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let lines: Vec<_> = masked
            .as_deref()
            .map(|s| {
                s.split('\n')
                    .map(|l| l.strip_suffix('\r').unwrap_or(l))
                    .collect()
            })
            .unwrap_or_default();
        for ((start, end), (matches, reason)) in groups {
            let index = self.total;
            self.total += 1;
            if index < self.offset || self.has_full_page {
                continue;
            }
            if index - self.offset >= self.limit {
                self.has_full_page = true;
                continue;
            }
            let mut text = String::new();
            if let Some(reason) = reason.as_ref() {
                text.push_str(&format!("[Callable expansion unavailable at {path}:{start}: {reason}. Showing matched source only.]\n"));
            }
            let mut has_omitted_columns = false;
            let mut source_lines = Vec::new();
            let mut path_prefixes = Vec::new();
            let mut filter_rows = Vec::new();
            let mut group_rendered_bytes = text.len();
            for line in start..=end {
                let value = lines
                    .get(line.saturating_sub(1))
                    .copied()
                    .or_else(|| {
                        hits.iter()
                            .find(|h| h.line_number as usize == line)
                            .map(|h| h.text.as_str())
                    })
                    .unwrap_or("");
                let is_match = matches.contains(&line);
                let sep = if is_match { ':' } else { '-' };
                let source_byte_len = source_lengths
                    .get(line.saturating_sub(1))
                    .copied()
                    .or_else(|| {
                        hits.iter()
                            .find(|hit| hit.line_number as usize == line)
                            .map(|hit| hit.source_byte_len)
                    })
                    .unwrap_or(value.len());
                has_omitted_columns |= max_columns > 0 && source_byte_len > max_columns;
                if max_columns == 0 || source_byte_len <= max_columns {
                    source_lines.push(line);
                }
                let filter_source = should_capture_bodies.then(|| value.to_string());
                let value = cap_line(value, source_byte_len, max_columns, is_match);
                let prefix_start = text.len();
                let prefix_len = path.len() + usize::from(show_lines);
                path_prefixes.push(prefix_start..prefix_start + prefix_len);
                if show_lines {
                    text.push_str(&format!("{path}{sep}{line}{sep}{value}\n"));
                } else {
                    text.push_str(&format!("{path}{sep}{value}\n"));
                }
                let added = text.len() - prefix_start;
                group_rendered_bytes += if self.is_grouped_session_cap {
                    added.saturating_sub(prefix_len)
                } else {
                    added
                };
                if let Some(content) = filter_source {
                    filter_rows.push(crate::tools::live_symbols::jev::SourceRow {
                        path: path.into(),
                        line,
                        range: prefix_start..text.len(),
                        is_complete: max_columns == 0 || source_byte_len <= max_columns,
                        content,
                    });
                }
                if group_rendered_bytes > self.cap {
                    break;
                }
            }
            if has_omitted_columns {
                let note = format!("[Callable source incomplete at {path}:{start}-{end}: long lines omitted. Use read with expand=none and this line range.]\n");
                group_rendered_bytes += note.len();
                text.push_str(&note);
            }
            let file_heading_bytes =
                if self.is_grouped_session_cap && self.last_file.as_deref() != Some(path) {
                    path.len() + 16
                } else {
                    0
                };
            if group_rendered_bytes + file_heading_bytes > self.cap {
                source_lines.clear();
                path_prefixes.clear();
                filter_rows.clear();
                text = format!("[Callable body unavailable: {path}:{start}-{end} exceeds the output cap. Read this range with expand=none and smaller offset/limit windows.]\n");
                group_rendered_bytes = text.len();
            }
            let group_budget_bytes = group_rendered_bytes + file_heading_bytes;
            if self.rendered_used + group_budget_bytes > self.cap {
                self.has_full_page = true;
                continue;
            }
            if !filter_rows.is_empty() {
                if let Some(source) = source {
                    self.jev.add_source(path, source);
                }
                for mut row in filter_rows {
                    row.range = self.used + row.range.start..self.used + row.range.end;
                    self.jev.rows.push(row);
                }
            }
            self.used += text.len();
            self.rendered_used += group_budget_bytes;
            if self.is_grouped_session_cap {
                self.last_file = Some(path.into());
            }
            self.text.push(text);
            self.source_lines.push(source_lines);
            self.path_prefixes.push(path_prefixes);
            self.anchors.push(LiveAnchor {
                file_path: path.into(),
                start_line: Some(start),
                end_line: Some(end),
            });
            self.next = index + 1;
        }
    }

    pub fn finish(self) -> LiveOutput {
        let mut notices = Vec::new();
        if self.next < self.total {
            notices.push(format!("[Callable groups: {} total; next_offset={}. Continue with expand=callable, offset={} and the same head_limit, or narrow path/pattern. Offset/head_limit count unique callable or fallback groups, not lines.]", self.total, self.next, self.next));
        } else if self.offset > 0 {
            notices.push(format!(
                "[Callable groups: {} total; last page at offset {}.]",
                self.total, self.offset
            ));
        }
        let mut output = LiveOutput {
            notices,
            jev: self.jev,
            ..LiveOutput::default()
        };
        for (((text, anchor), lines), prefixes) in self
            .text
            .into_iter()
            .zip(self.anchors)
            .zip(self.source_lines)
            .zip(self.path_prefixes)
        {
            let start = output.text.len();
            output.text.push_str(&text);
            output.path_prefixes.extend(
                prefixes
                    .into_iter()
                    .map(|range| start + range.start..start + range.end),
            );
            output.record_file(&anchor.file_path, start, output.text.len());
            for line in lines {
                output.record_source(&anchor.file_path, line, line);
            }
            output.anchors.push(anchor);
        }
        output
            .text
            .truncate(output.text.trim_end_matches('\n').len());
        if let Some(last) = output.files.last_mut() {
            last.end_byte = output.text.len();
        }
        for row in &mut output.jev.rows {
            row.range.end = row.range.end.min(output.text.len());
        }
        output
    }
}
