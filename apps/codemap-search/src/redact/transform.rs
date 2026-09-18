//! Apply detected ranges to presentation copies, preserving line breaks and size limits.
use super::{detection::SourceScan, is_enabled};
use std::borrow::Cow;
use std::ops::Range;

pub(super) const MARKER: &str = "[REDACTED]";

impl SourceScan {
    pub(crate) fn render<'a>(&self, source: &'a str) -> Cow<'a, str> {
        self.render_range(source, 0..source.len())
    }

    pub(crate) fn render_range<'a>(&self, source: &'a str, range: Range<usize>) -> Cow<'a, str> {
        let text = &source[range.clone()];
        let ranges: Vec<_> = self
            .detections
            .iter()
            .filter_map(|detection| {
                let start = detection.range.start.max(range.start);
                let end = detection.range.end.min(range.end);
                (start < end).then(|| start - range.start..end - range.start)
            })
            .collect();
        if ranges.is_empty() {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(mask_ranges(text, ranges))
        }
    }

    /// Use original offsets, never a comparison of shortened rendered lines. Ambiguous
    /// duplicate values inherit every overlapping detection; stale values stay hidden.
    pub(crate) fn literal(
        &self,
        source: &str,
        literal: &crate::parser::ExtractedLiteral,
    ) -> String {
        if !is_enabled() {
            return literal.text.clone();
        }
        if literal.text.is_empty() {
            return String::new();
        }
        let mut found = false;
        let mut ranges = Vec::new();
        for (range, line) in &self.literals {
            if *line != literal.line || source.get(range.clone()) != Some(literal.text.as_str()) {
                continue;
            }
            found = true;
            for detection in &self.detections {
                let left = range.start.max(detection.range.start);
                let right = range.end.min(detection.range.end);
                if left < right {
                    ranges.push(left - range.start..right - range.start);
                }
            }
        }
        if found {
            mask_ranges(&literal.text, ranges)
        } else {
            hidden(&literal.text)
        }
    }
}

/// Replacement never grows text, preserving the existing output byte ceilings and CR/LF.
pub(crate) fn hidden(value: &str) -> String {
    value
        .split_inclusive('\n')
        .map(|line| {
            let body = line.trim_end_matches(['\r', '\n']);
            let replacement = if body.len() >= MARKER.len() {
                MARKER.to_string()
            } else {
                "*".repeat(body.len())
            };
            replacement + &line[body.len()..]
        })
        .collect()
}

pub(super) fn mask_ranges(text: &str, mut ranges: Vec<Range<usize>>) -> String {
    ranges.sort_unstable_by_key(|range| (range.start, range.end));
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges.into_iter().filter(|range| range.start < range.end) {
        if let Some(last) = merged.last_mut().filter(|last| range.start <= last.end) {
            last.end = last.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    let mut output = String::with_capacity(text.len());
    let mut start = 0;
    for range in merged {
        output.push_str(&text[start..range.start]);
        output.push_str(&hidden(&text[range.clone()]));
        start = range.end;
    }
    output.push_str(&text[start..]);
    output
}
