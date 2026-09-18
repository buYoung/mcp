//! Find sensitive ranges in original source without changing its contents.
use super::{is_enabled, pii, rules, syntax, text};
use std::ops::Range;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum DetectionKind {
    SensitiveField,
    Token,
    Credential,
    PrivateKey,
    Custom,
    Pii,
}

/// No raw values are retained in detection metadata.
#[derive(Debug, Clone)]
pub(super) struct Detection {
    pub(super) range: Range<usize>,
    pub(super) rule_id: String,
    pub(super) kind: DetectionKind,
}

impl Detection {
    pub(super) fn field(range: Range<usize>) -> Self {
        Self {
            range,
            rule_id: "field.sensitive".into(),
            kind: DetectionKind::SensitiveField,
        }
    }
}

#[derive(Default)]
pub(crate) struct SourceScan {
    pub(super) detections: Vec<Detection>,
    pub(super) literals: Vec<(Range<usize>, usize)>,
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

    pub(super) fn collect(source: &str, path: Option<&Path>, syntax: syntax::Context) -> Self {
        let mut detections = detect_patterns(source);
        let mut protected = syntax.protected;
        protected.sort_unstable_by_key(|range| (range.start, range.end));
        let mut merged: Vec<Range<usize>> = Vec::new();
        for range in protected {
            if let Some(last) = merged.last_mut().filter(|last| range.start <= last.end) {
                last.end = last.end.max(range.end);
            } else {
                merged.push(range);
            }
        }
        detections.retain(|detection| {
            let index = merged.partition_point(|range| range.end <= detection.range.start);
            detection.kind != DetectionKind::Pii
                || merged
                    .get(index)
                    .is_none_or(|range| range.start >= detection.range.end)
        });
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

    pub(super) fn finish(source: &str, mut detections: Vec<Detection>) -> Self {
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
}

pub(super) fn detect_patterns(source: &str) -> Vec<Detection> {
    let mut detections = rules::detect(source);
    detections.extend(pii::detect(source));
    detections
}
