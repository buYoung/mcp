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
    has_full_page: bool,
    text: Vec<String>,
    anchors: Vec<LiveAnchor>,
}

impl ExpansionPage {
    pub fn new(offset: usize, limit: usize) -> Self {
        Self {
            offset,
            limit: if limit == 0 { usize::MAX } else { limit },
            total: 0,
            next: offset,
            cap: crate::config::get()
                .read_output_byte_cap
                .saturating_sub(512),
            used: 0,
            has_full_page: false,
            text: Vec::new(),
            anchors: Vec::new(),
        }
    }

    pub fn add_file(
        &mut self,
        path: &str,
        bytes: Option<&[u8]>,
        hits: &[LineHit],
        show_lines: bool,
        max_columns: usize,
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
        let lines: Vec<_> = source
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
                has_omitted_columns |= max_columns > 0 && value.len() > max_columns;
                let value = cap_line(value, max_columns, is_match);
                if show_lines {
                    text.push_str(&format!("{path}{sep}{line}{sep}{value}\n"));
                } else {
                    text.push_str(&format!("{path}{sep}{value}\n"));
                }
                if text.len() > self.cap {
                    break;
                }
            }
            if has_omitted_columns {
                text.push_str(&format!("[Callable source incomplete at {path}:{start}-{end}: grep_max_columns omitted long lines. Use read with expand=none and this line range.]\n"));
            }
            if text.len() > self.cap {
                text = format!("[Callable body unavailable: {path}:{start}-{end} exceeds the output cap. Read this range with expand=none and smaller offset/limit windows.]\n");
            }
            if self.used + text.len() > self.cap {
                self.has_full_page = true;
                continue;
            }
            self.used += text.len();
            self.text.push(text);
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
            ..LiveOutput::default()
        };
        for (text, anchor) in self.text.into_iter().zip(self.anchors) {
            let start = output.text.len();
            output.text.push_str(&text);
            output.record_file(&anchor.file_path, start, output.text.len());
            output.anchors.push(anchor);
        }
        output
            .text
            .truncate(output.text.trim_end_matches('\n').len());
        if let Some(last) = output.files.last_mut() {
            last.end_byte = output.text.len();
        }
        output
    }
}
