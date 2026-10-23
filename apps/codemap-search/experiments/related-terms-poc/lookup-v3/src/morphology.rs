//! Rule-based English inflection and repository morphology (`rtl-2` H5–H7).
//!
//! Inflection rules serve both the dictionary lemma fallback and morphology typing. The
//! repository side reads only the index-time `word_stems` table: the query string is
//! stemmed here, the vocabulary never is, and no vocabulary scan happens at lookup time.

use std::collections::HashSet;

use serde::Serialize;

use crate::lookup::LookupPolicy;
use crate::normalize::{is_stemmable, stem, stem_analyzer, QueryKeys};
use crate::stats::RepositoryStats;

const MIN_INFLECTION_BASE_LENGTH: usize = 3;

/// Regular-inflection base forms of a lowercase alphabetic word, in the order lemma fallback
/// tries them: -s (not -ss), -es, -ies→-y, then -ing and -ed. An -ing/-ed base is tried with
/// e-restoration first (`pars` → `parse`; this is also the -d rule), then bare, then with a
/// doubled final consonant undoubled (`runn` → `run`).
pub fn inflection_bases(word: &str) -> Vec<String> {
    if !word.bytes().all(|byte| byte.is_ascii_lowercase()) {
        return Vec::new();
    }
    let mut bases: Vec<String> = Vec::new();
    if let Some(base) = word.strip_suffix('s').filter(|_| !word.ends_with("ss")) {
        bases.push(base.to_string());
    }
    if let Some(base) = word.strip_suffix("es") {
        bases.push(base.to_string());
    }
    if let Some(base) = word.strip_suffix("ies") {
        bases.push(format!("{base}y"));
    }
    for suffix in ["ing", "ed"] {
        if let Some(base) = word.strip_suffix(suffix) {
            bases.push(format!("{base}e"));
            bases.push(base.to_string());
            if let Some(undoubled) = undouble_final_consonant(base) {
                bases.push(undoubled);
            }
        }
    }
    let mut seen = HashSet::new();
    bases
        .into_iter()
        .filter(|base| base.len() >= MIN_INFLECTION_BASE_LENGTH && base != word && seen.insert(base.clone()))
        .collect()
}

fn undouble_final_consonant(base: &str) -> Option<String> {
    let bytes = base.as_bytes();
    let [.., previous, last] = bytes else {
        return None;
    };
    (previous == last && !b"aeiou".contains(last)).then(|| base[..base.len() - 1].to_string())
}

/// How two same-stem words relate under the inflection rules, if they do.
fn inflection_basis(query: &str, word: &str) -> Option<&'static str> {
    let query_bases = inflection_bases(query);
    let word_bases = inflection_bases(word);
    if query_bases.iter().any(|base| base == word) {
        Some("base_of_query")
    } else if word_bases.iter().any(|base| base == query) {
        Some("inflection_of_query")
    } else if query_bases.iter().any(|base| word_bases.contains(base)) {
        // Sibling inflections of one base, e.g. `executing` / `executed` (base `execute`).
        Some("sibling_inflection")
    } else {
        None
    }
}

#[derive(Debug, Serialize)]
pub struct MorphologyWord {
    pub word: String,
    /// `inflection`, `derivation`, or `same_root`.
    #[serde(rename = "type")]
    pub morphology_type: &'static str,
    /// Why the word was typed so: an inflection basis, `same_stem`, or `prefix_removed:<p>` /
    /// `prefix_added:<p>` for the prefix family.
    pub basis: String,
    pub file_count: u64,
}

#[derive(Debug, Serialize)]
pub struct Morphology {
    /// `ok`, `empty`, `not_single_token`, `not_stemmable`, or `repository_not_ready`.
    pub status: &'static str,
    pub query_word: Option<String>,
    pub query_stem: Option<String>,
    /// The query's repository file count (multi-token queries: concatenated identifier,
    /// else the smallest component). `Some(0)` when absent, `None` when not ready.
    pub query_file_count: Option<u64>,
    pub query_absent_in_repository: bool,
    /// Distinct repository words found before `morphology_limit` was applied.
    pub candidates_found: usize,
    pub words: Vec<MorphologyWord>,
}

/// Same-stem repository words for a single-token query, plus the prefix family, ordered by
/// file count (most files first) and capped at `limit`. Never mixed into dictionary results.
pub fn morphology(repository: &RepositoryStats, policy: &LookupPolicy, keys: &QueryKeys, limit: usize) -> Morphology {
    let query_file_count = repository.term_file_count(&keys.index_tokens);
    let mut result = Morphology {
        status: "ok",
        query_word: None,
        query_stem: None,
        query_file_count,
        query_absent_in_repository: repository.is_ready() && query_file_count == Some(0),
        candidates_found: 0,
        words: Vec::new(),
    };
    if !repository.is_ready() {
        result.status = "repository_not_ready";
        return result;
    }
    let [query] = &keys.index_tokens[..] else {
        result.status = "not_single_token";
        return result;
    };
    result.query_word = Some(query.clone());
    if !is_stemmable(query) {
        result.status = "not_stemmable";
        return result;
    }
    let mut analyzer = stem_analyzer();
    let query_stem = stem(&mut analyzer, query);
    result.query_stem = Some(query_stem.clone());

    let mut found: Vec<MorphologyWord> = Vec::new();
    let mut seen: HashSet<String> = HashSet::from([query.clone()]);
    let same_stem = repository.words_with_stem(&query_stem).unwrap_or_default();
    for (word, file_count) in &same_stem {
        if !seen.insert(word.clone()) {
            continue;
        }
        let (morphology_type, basis) = match inflection_basis(query, word) {
            Some(basis) => ("inflection", basis.to_string()),
            None => ("derivation", "same_stem".to_string()),
        };
        found.push(MorphologyWord {
            word: word.clone(),
            morphology_type,
            basis,
            file_count: *file_count,
        });
    }

    // Prefix family, query side: `reconnect` -> words stemming like `connect`.
    let min_remainder = policy.morphology_min_prefix_remainder_length;
    for prefix in &policy.morphology_prefixes {
        let Some(remainder) = query.strip_prefix(prefix.as_str()).filter(|rest| rest.len() >= min_remainder) else {
            continue;
        };
        let remainder_stem = stem(&mut analyzer, remainder);
        for (word, file_count) in repository.words_with_stem(&remainder_stem).unwrap_or_default() {
            if seen.insert(word.clone()) {
                found.push(MorphologyWord {
                    word,
                    morphology_type: "same_root",
                    basis: format!("prefix_removed:{prefix}"),
                    file_count,
                });
            }
        }
    }
    // Prefix family, repository side: prefixed forms of the query's stem family that exist
    // as published words (`lock`/`locked` -> `unlock`/`unlocked`). Point lookups only.
    let stem_family: Vec<&str> = std::iter::once(query.as_str())
        .chain(same_stem.iter().map(|(word, _)| word.as_str()))
        .collect();
    for base in stem_family {
        for prefix in &policy.morphology_prefixes {
            let prefixed = format!("{prefix}{base}");
            if seen.contains(&prefixed) || !repository.has_stemmed_word(&prefixed) {
                continue;
            }
            seen.insert(prefixed.clone());
            let file_count = repository.file_count(&prefixed).unwrap_or(0);
            found.push(MorphologyWord {
                word: prefixed,
                morphology_type: "same_root",
                basis: format!("prefix_added:{prefix}"),
                file_count,
            });
        }
    }

    found.sort_by(|left, right| right.file_count.cmp(&left.file_count).then_with(|| left.word.cmp(&right.word)));
    result.candidates_found = found.len();
    found.truncate(limit);
    result.status = if found.is_empty() { "empty" } else { "ok" };
    result.words = found;
    result
}
