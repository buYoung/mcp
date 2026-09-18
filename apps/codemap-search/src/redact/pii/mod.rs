//! Opt-in, local PII rules adapted from Presidio's pattern recognizers.
//! See docs/pii-redaction.md and vendor/presidio/LICENSE for scope and attribution.
mod context;
mod patterns;
mod validators;

use super::detection::{Detection, DetectionKind};
use patterns::{CompiledRule, RuleDefinition};
use std::sync::OnceLock;

struct Entry {
    definition: RuleDefinition,
    compiled: OnceLock<CompiledRule>,
}

fn catalog() -> &'static [Entry] {
    static CATALOG: OnceLock<Vec<Entry>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let definitions: Vec<RuleDefinition> =
            serde_json::from_str(include_str!("catalog.json")).expect("built-in PII catalog");
        definitions
            .into_iter()
            .map(|definition| Entry {
                definition,
                compiled: OnceLock::new(),
            })
            .collect()
    })
}

pub(crate) fn is_supported(entity: &str) -> bool {
    catalog()
        .iter()
        .any(|entry| entry.definition.entity == entity)
}

pub(super) fn detect(text: &str) -> Vec<Detection> {
    detect_with_context(text, true)
}

pub(super) fn detect_unambiguous(text: &str) -> Vec<Detection> {
    detect_with_context(text, false)
}

fn detect_with_context(text: &str, can_use_labels: bool) -> Vec<Detection> {
    let config = crate::config::get();
    if config.redact.pii_entities.is_empty() {
        return Vec::new();
    }
    let mut detections = Vec::new();
    for entry in catalog().iter().filter(|entry| {
        config
            .redact
            .pii_entities
            .contains(&entry.definition.entity)
    }) {
        let rule = entry
            .compiled
            .get_or_init(|| CompiledRule::new(&entry.definition));
        detections.extend(
            rule.detect(text, can_use_labels)
                .into_iter()
                .map(|range| Detection {
                    range,
                    rule_id: rule.id.clone(),
                    kind: DetectionKind::Pii,
                }),
        );
    }
    detections
}

#[cfg(test)]
mod tests;
