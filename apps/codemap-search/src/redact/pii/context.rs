//! Require a nearby label for ambiguous identifiers without NLP or confidence scoring.
use regex::{Regex, RegexBuilder};

pub(super) struct LabelContext(Regex);

impl LabelContext {
    pub fn new(labels: &[String]) -> Self {
        let alternatives: Vec<_> = labels
            .iter()
            .map(|label| {
                label
                    .split(|character: char| !character.is_alphanumeric())
                    .filter(|word| !word.is_empty())
                    .map(regex::escape)
                    .collect::<Vec<_>>()
                    .join(r"[\s_\-]*")
            })
            .filter(|label| !label.is_empty())
            .collect();
        // Optional field suffixes cover memberId / passportNumber without accepting
        // unrelated identifiers such as membershipCount or passportTimeoutMs.
        let pattern = format!(
            r"\b(?:{})(?:[\s_\-]*(?:id|number|code|no))?\b",
            alternatives.join("|")
        );
        Self(
            RegexBuilder::new(&pattern)
                .case_insensitive(true)
                .build()
                .expect("built-in PII labels"),
        )
    }

    pub fn is_relevant(&self, text: &str, start: usize) -> bool {
        // Keep context in the current field/statement. One preceding line is allowed
        // only when the value starts on its own line after an assignment or label.
        let mut window_start = start.saturating_sub(160);
        while !text.is_char_boundary(window_start) {
            window_start += 1;
        }
        let prefix = &text[window_start..start];
        let mut left = prefix
            .rfind([';', ',', '{', '}', '[', ']'])
            .map_or(0, |i| i + 1);
        if let Some(newline) = prefix[left..].rfind('\n').map(|i| left + i) {
            let continuation = prefix[newline + 1..]
                .trim_matches(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '`'));
            let previous = prefix[left..newline].trim_end();
            if continuation.is_empty() && previous.ends_with(['=', ':']) {
                left += previous.rfind('\n').map_or(0, |i| i + 1);
            } else {
                left = newline + 1;
            }
        }
        let context = &prefix[left..];
        let label = context
            .rfind(['=', ':'])
            .map_or(context, |operator| &context[..operator]);
        self.0.is_match(label)
    }
}
