//! Path-only role hints shared by candidate recall and post-ranking. No files are excluded
//! from the index or from the live filesystem tools.
use std::sync::LazyLock;

// Both regex engines match this same, case-sensitive path vocabulary. Tantivy matches
// complete STRING terms; the in-memory classifier adds anchors for identical semantics.
pub(super) const AUXILIARY_PATH_PATTERN: &str = r"(.*/)?(generated(/.*)?|[^/]*(_gen|[.]generated|[.]g|[.]pb)[.][^/]+|(locale|locales|i18n|l10n)/.*[.](json|yaml|yml))";

pub(super) const AUXILIARY_PATH_SCORE_WEIGHT: f32 = 0.3;

pub(super) fn is_auxiliary_path(path: &str) -> bool {
    static PATTERN: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(&format!("^(?:{AUXILIARY_PATH_PATTERN})$"))
            .expect("valid auxiliary path pattern")
    });
    PATTERN.is_match(path)
}
