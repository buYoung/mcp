//! Compile portable regexes and preserve candidate boundaries in original UTF-8 text.
use super::{context::LabelContext, validators};
use regex::{Regex, RegexBuilder, RegexSet, RegexSetBuilder};
use serde::Deserialize;
use std::ops::Range;

#[derive(Deserialize)]
pub(super) struct RuleDefinition {
    pub entity: String,
    validator: validators::Validator,
    is_case_insensitive: bool,
    context: Vec<String>,
    patterns: Vec<PatternDefinition>,
}

#[derive(Deserialize)]
struct PatternDefinition {
    name: String,
    regex: String,
    left_boundary: String,
    requires_context: bool,
    field_regex: Option<String>,
}

struct Pattern {
    name: String,
    search: Regex,
    at_start: Regex,
    after_character: Regex,
    left_boundary: Option<Regex>,
    requires_context: bool,
    is_field_pattern: bool,
}

pub(super) struct CompiledRule {
    pub id: String,
    entity: String,
    validator: validators::Validator,
    candidates: RegexSet,
    patterns: Vec<Pattern>,
    context: LabelContext,
}

impl CompiledRule {
    pub fn new(definition: &RuleDefinition) -> Self {
        let compile = |pattern: &str| {
            RegexBuilder::new(pattern)
                .case_insensitive(definition.is_case_insensitive)
                .multi_line(true)
                .dot_matches_new_line(true)
                .build()
                .unwrap_or_else(|error| panic!("PII regex {}: {error}", definition.entity))
        };
        let patterns: Vec<Pattern> = definition
            .patterns
            .iter()
            .flat_map(|pattern| {
                std::iter::once((pattern, pattern.regex.as_str(), false)).chain(
                    pattern
                        .field_regex
                        .as_deref()
                        .map(|regex| (pattern, regex, true)),
                )
            })
            .map(|(pattern, expression, is_field_pattern)| Pattern {
                name: pattern.name.clone(),
                // Locate a superset cheaply, then verify the original pattern anchored
                // at each candidate, retaining the preceding Unicode character.
                search: compile(&expression.replace(r"\b", "")),
                at_start: compile(&format!(r"\A(?P<pii_candidate>{expression})")),
                after_character: compile(&format!(r"\A.(?P<pii_candidate>{expression})")),
                left_boundary: (!is_field_pattern && !pattern.left_boundary.is_empty())
                    .then(|| compile(&format!("\\A(?:{})\\z", pattern.left_boundary))),
                requires_context: pattern.requires_context,
                is_field_pattern,
            })
            .collect();
        Self {
            id: format!(
                "pii.{}",
                definition.entity.to_ascii_lowercase().replace('_', "-")
            ),
            entity: definition.entity.clone(),
            validator: definition.validator,
            context: LabelContext::new(&definition.context),
            candidates: RegexSetBuilder::new(
                patterns.iter().map(|pattern| pattern.search.as_str()),
            )
            .case_insensitive(definition.is_case_insensitive)
            .multi_line(true)
            .dot_matches_new_line(true)
            .build()
            .expect("built-in PII candidate patterns"),
            patterns,
        }
    }

    #[cfg(test)]
    pub fn find(&self, text: &str) -> Vec<Range<usize>> {
        // Upstream compatibility tests exclude adapters which replace prose labels
        // with code-field context. Their production behavior is verified separately.
        self.find_filtered(text, |pattern, _| !pattern.is_field_pattern)
    }

    pub fn detect(&self, text: &str, can_use_labels: bool) -> Vec<Range<usize>> {
        self.find_filtered(text, |pattern, start| {
            !(pattern.requires_context || pattern.is_field_pattern)
                || (can_use_labels && self.context.is_relevant(text, start))
        })
        .into_iter()
        .map(|mut range| {
            // URL quotation marks and a trailing VAT separator belong to the
            // surrounding source, not the sensitive value.
            if self.entity == "URL" {
                let value = &text[range.clone()];
                if value.starts_with(['\'', '"']) && value.ends_with(['\'', '"']) {
                    range.start += 1;
                    range.end -= 1;
                }
            }
            if self.entity == "IT_VAT_CODE" {
                range.end = range.start + text[range.clone()].trim_end_matches([' ', '_']).len();
            }
            range
        })
        .collect()
    }

    fn find_filtered(
        &self,
        text: &str,
        accepts: impl Fn(&Pattern, usize) -> bool,
    ) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        for index in self.candidates.matches(text) {
            let pattern = &self.patterns[index];
            let mut cursor = 0;
            while let Some(candidate) = pattern.search.find_at(text, cursor) {
                let start = candidate.start();
                let (offset, anchored) = text[..start]
                    .char_indices()
                    .next_back()
                    .map_or((0, &pattern.at_start), |(offset, _)| {
                        (offset, &pattern.after_character)
                    });
                let Some(captures) = anchored.captures(&text[offset..]) else {
                    cursor = start + text[start..].chars().next().unwrap().len_utf8();
                    continue;
                };
                let found = captures
                    .name("secret")
                    .or_else(|| captures.name("pii_candidate"))
                    .unwrap();
                let range = offset + found.start()..offset + found.end();
                // Adapted lookarounds may consume a delimiter outside `secret`.
                // Resume at the secret's end so that delimiter remains available.
                cursor = range.end;
                if let Some(boundary) = &pattern.left_boundary {
                    if let Some((offset, _)) = text[..range.start].char_indices().next_back() {
                        if boundary.is_match(&text[offset..range.start]) {
                            cursor = start + text[start..].chars().next().unwrap().len_utf8();
                            continue;
                        }
                    }
                }
                if !accepts(pattern, range.start) {
                    continue;
                }
                if self.entity == "IBAN_CODE" {
                    // A greedy candidate can include the next IBAN's country prefix.
                    // Check every complete token boundary, then resume at the accepted
                    // end instead of skipping the rest of the regex match.
                    if let Some(range) = self.iban_range(text, range.clone()) {
                        cursor = range.end;
                        ranges.push(range);
                    } else {
                        cursor = range.start + 1; // IBANs always start with ASCII.
                    }
                } else if is_pattern_candidate(&self.entity, &pattern.name, found.as_str())
                    && validators::is_candidate_accepted(&self.validator, found.as_str())
                {
                    ranges.push(range);
                }
            }
        }
        ranges.sort_unstable_by_key(|range| (range.start, std::cmp::Reverse(range.end)));
        ranges.dedup();
        let mut selected: Vec<Range<usize>> = Vec::new();
        for range in ranges {
            if selected.last().is_none_or(|other| range.end > other.end) {
                selected.push(range);
            }
        }
        selected
    }

    fn iban_range(&self, text: &str, candidate: Range<usize>) -> Option<Range<usize>> {
        let mut end = candidate.end;
        let mut fallback = None;
        while end > candidate.start {
            let previous = text.as_bytes()[end - 1];
            let is_token_end = (previous.is_ascii_uppercase() || previous.is_ascii_digit())
                && text
                    .as_bytes()
                    .get(end)
                    .is_none_or(|next| !next.is_ascii_uppercase() && !next.is_ascii_digit());
            if is_token_end
                && validators::is_candidate_accepted(&self.validator, &text[candidate.start..end])
            {
                if validators::has_exact_iban_length(&text[candidate.start..end]) {
                    return Some(candidate.start..end);
                }
                fallback.get_or_insert(candidate.start..end);
            }
            end -= 1; // The IBAN candidate alphabet is entirely ASCII.
        }
        fallback
    }
}

fn is_pattern_candidate(entity: &str, name: &str, value: &str) -> bool {
    match entity {
        "CREDIT_CARD" => {
            !(value.starts_with('1')
                && value.chars().count() == 13
                && value.chars().all(char::is_numeric))
        }
        "DE_PLZ" => !matches!(value, "01000" | "99999"),
        "UK_NINO" => !["BG", "GB", "NK", "KN", "NT", "TN", "ZZ"]
            .iter()
            .any(|prefix| {
                value
                    .get(..2)
                    .is_some_and(|v| v.eq_ignore_ascii_case(prefix))
            }),
        "IN_PAN" if name == "PAN (Low)" => {
            value.chars().any(|c| c.is_ascii_alphabetic())
                && value
                    .as_bytes()
                    .windows(4)
                    .any(|part| part.iter().all(u8::is_ascii_digit))
        }
        "US_HEALTH_INSURANCE_MEMBER_ID" => {
            (6..=20).contains(&value.len()) && value.chars().any(|c| c.is_ascii_digit())
        }
        _ => true,
    }
}
