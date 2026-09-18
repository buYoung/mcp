//! Adapt file, syntax-node and named-value inputs to the redaction pipeline.
use super::{
    detection::{detect_patterns, Detection, SourceScan},
    is_enabled, rules, syntax,
};
use std::borrow::Cow;
use std::path::Path;

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
    for range in &mut context.protected {
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
    let mut detections = detect_patterns(value);
    if rules::is_sensitive_key(name) {
        detections.push(Detection::field(0..value.len()));
    }
    SourceScan::finish(value, detections).render(value)
}
