//! Mask presentation copies only. All detection offsets refer to the original UTF-8 source.
mod rules;
mod syntax;
mod text;

use std::borrow::Cow;
use std::cell::Cell;
use std::ops::Range;
use std::path::Path;

const MARKER: &str = "[REDACTED]";

thread_local! {
    static IS_MCP_RESPONSE: Cell<bool> = const { Cell::new(false) };
}

pub(crate) struct RequestGuard(bool);

pub(crate) fn begin_request() -> RequestGuard {
    RequestGuard(IS_MCP_RESPONSE.replace(true))
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        IS_MCP_RESPONSE.set(self.0);
    }
}

pub(crate) fn is_enabled() -> bool {
    IS_MCP_RESPONSE.get() && crate::config::get().is_redact_enabled
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DetectionKind {
    SensitiveField,
    Token,
    Credential,
    PrivateKey,
    Custom,
}

/// No raw values are retained in detection metadata.
#[derive(Debug, Clone)]
struct Detection {
    range: Range<usize>,
    rule_id: String,
    kind: DetectionKind,
}

impl Detection {
    fn field(range: Range<usize>) -> Self {
        Self {
            range,
            rule_id: "field.sensitive".into(),
            kind: DetectionKind::SensitiveField,
        }
    }
}

#[derive(Default)]
pub(crate) struct SourceScan {
    detections: Vec<Detection>,
    literals: Vec<(Range<usize>, usize)>,
}

impl SourceScan {
    pub(crate) fn new(path: &Path, source: &str) -> Self {
        if !is_enabled() {
            return Self::default();
        }
        let syntax = syntax::parse(path, source).unwrap_or_default();
        Self::collect(source, Some(path), syntax)
    }

    pub(crate) fn with_tree(source: &str, tree: &tree_sitter::Tree) -> Self {
        if !is_enabled() {
            return Self::default();
        }
        Self::collect(source, None, syntax::inspect(tree.root_node(), source))
    }

    fn collect(source: &str, path: Option<&Path>, syntax: syntax::Context) -> Self {
        let mut detections = rules::detect(source);
        detections.extend(
            text::detect(source, path)
                .into_iter()
                .filter_map(|candidate| {
                    (!syntax
                        .decided
                        .iter()
                        .any(|range| range.contains(&candidate.key_start)))
                    .then_some(candidate.detection)
                }),
        );
        detections.extend(syntax.detections);
        let mut scan = Self::finish(source, detections);
        scan.literals = syntax.literals;
        scan
    }

    fn finish(source: &str, mut detections: Vec<Detection>) -> Self {
        let config = crate::config::get();
        detections.retain(|detection| {
            detection.range.start < detection.range.end
                && source.get(detection.range.clone()).is_some_and(|value| {
                    !config.redact.exceptions.iter().any(|exception| {
                        exception.rule_id == detection.rule_id && exception.value == value
                    })
                })
        });
        detections.sort_unstable_by_key(|detection| {
            (detection.range.start, detection.range.end, detection.kind)
        });
        Self {
            detections,
            literals: Vec::new(),
        }
    }

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

pub(crate) fn in_file<'a>(path: &Path, source: &'a str) -> Cow<'a, str> {
    SourceScan::new(path, source).render(source)
}

pub(crate) fn node_text<'a>(node: tree_sitter::Node<'_>, source: &'a str) -> Cow<'a, str> {
    if !is_enabled() {
        return Cow::Borrowed(&source[node.byte_range()]);
    }
    let offset = node.start_byte();
    let mut context = syntax::inspect(node, source);
    for range in &mut context.decided {
        *range = range.start - offset..range.end - offset;
    }
    for detection in &mut context.detections {
        detection.range = detection.range.start - offset..detection.range.end - offset;
    }
    for (range, _) in &mut context.literals {
        *range = range.start - offset..range.end - offset;
    }
    let fragment = &source[node.byte_range()];
    SourceScan::collect(fragment, None, context).render(fragment)
}

pub(crate) fn source(text: &str) -> Cow<'_, str> {
    if !is_enabled() {
        return Cow::Borrowed(text);
    }
    SourceScan::collect(text, None, syntax::Context::default()).render(text)
}

pub(crate) fn named_value<'a>(name: &str, value: &'a str) -> Cow<'a, str> {
    if !is_enabled() {
        return Cow::Borrowed(value);
    }
    let mut detections = rules::detect(value);
    if rules::is_sensitive_key(name) {
        detections.push(Detection::field(0..value.len()));
    }
    SourceScan::finish(value, detections).render(value)
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

fn mask_ranges(text: &str, mut ranges: Vec<Range<usize>>) -> String {
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

fn response_text(text: &str, is_tool_text: bool) -> String {
    if !is_enabled() {
        return text.to_string();
    }
    if is_tool_text {
        // Source producers inspect full originals before slicing/formatting. Only
        // context-free rules run again here: guessing assignments in formatted code
        // would undo AST decisions about references and type declarations.
        return SourceScan::finish(text, rules::detect(text))
            .render(text)
            .into_owned();
    }
    // Unrelated metadata lines must not share a quote/block continuation.
    text.split_inclusive('\n')
        .map(|line| source(line).into_owned())
        .collect()
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

#[cfg(test)]
mod tests;
