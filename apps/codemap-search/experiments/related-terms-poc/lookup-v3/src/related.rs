//! `related`: dataset-only related-term lookup over the `rt-related/1` SQLite artifact.
//!
//! A query matches dataset terms as a whole and through its parts (`eventLoop` -> the whole
//! `event loop`, plus `event` and `loop`). Every relation of every matched term becomes a
//! candidate, ranked by the cosine similarity between the query's vector and the
//! candidate's vector (stored embeddings; no model runs at lookup time). Nothing is cut off.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;

use crate::morphology::inflection_bases;
use crate::normalize::exact_key;
use crate::related_keys::{
    is_protected, protected_candidates, query_words, spans, strip_outer, term_keys, RELATED_KEYS_VERSION,
};

pub const SCHEMA_VERSION: &str = "rt-related/1";
const OUTPUT_SCHEMA: &str = "rt-related-lookup/1";
const REQUIRED_TABLES: &[&str] = &["related_meta", "sources", "terms", "relations"];

#[derive(Debug, Clone)]
pub struct TermRow {
    pub term_id: i64,
    pub surface: String,
    pub match_key: String,
    pub is_atomic: bool,
}

pub struct RelatedDataset {
    connection: Connection,
    pub dataset_id: String,
    pub vector_dimensions: usize,
}

impl RelatedDataset {
    pub fn open(path: &Path) -> Result<Self, String> {
        if !path.is_file() {
            return Err(format!("dataset unavailable: {} is not a file", path.display()));
        }
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
            .map_err(|error| format!("dataset unavailable: {error}"))?;
        Self::from_connection(connection)
    }

    pub fn from_connection(connection: Connection) -> Result<Self, String> {
        for table in REQUIRED_TABLES {
            let count: i64 = connection
                .query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [table], |row| row.get(0))
                .map_err(|error| error.to_string())?;
            if count != 1 {
                return Err(format!("dataset incompatible: missing table {table}"));
            }
        }
        let meta = |key: &str| -> Result<String, String> {
            connection
                .query_row("SELECT value FROM related_meta WHERE key = ?1", [key], |row| row.get(0))
                .map_err(|_| format!("dataset incompatible: related_meta.{key} missing"))
        };
        let schema = meta("schema_version")?;
        if schema != SCHEMA_VERSION {
            return Err(format!("dataset incompatible: schema {schema} != {SCHEMA_VERSION}"));
        }
        let keys_version = meta("keys_version")?;
        if keys_version != RELATED_KEYS_VERSION {
            return Err(format!("dataset incompatible: keys {keys_version} != {RELATED_KEYS_VERSION}"));
        }
        let dataset_id = meta("dataset_id")?;
        let vector_dimensions = meta("vector_dimensions")?
            .parse()
            .map_err(|_| "dataset incompatible: vector_dimensions is not a number".to_string())?;
        Ok(Self {
            connection,
            dataset_id,
            vector_dimensions,
        })
    }

    fn terms_where(&self, condition: &str, key: &str) -> Result<Vec<TermRow>, String> {
        let sql = format!("SELECT term_id, surface, match_key, is_atomic FROM terms WHERE {condition} ORDER BY term_id");
        let mut statement = self.connection.prepare_cached(&sql).map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([key], |row| {
                Ok(TermRow {
                    term_id: row.get(0)?,
                    surface: row.get(1)?,
                    match_key: row.get(2)?,
                    is_atomic: row.get::<_, i64>(3)? == 1,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|error| error.to_string())
    }

    pub fn by_exact_key(&self, key: &str) -> Result<Vec<TermRow>, String> {
        self.terms_where("exact_key = ?1", key)
    }

    fn by_protected_key(&self, key: &str) -> Result<Vec<TermRow>, String> {
        self.terms_where("exact_key = ?1 AND is_protected = 1", key)
    }

    fn by_match_key(&self, key: &str) -> Result<Vec<TermRow>, String> {
        self.terms_where("match_key = ?1 AND match_key <> ''", key)
    }

    fn by_joined_key(&self, key: &str) -> Result<Vec<TermRow>, String> {
        self.terms_where("joined_key = ?1 AND joined_key <> ''", key)
    }

    /// `(other term, kind, source, is_reverse)` for every relation touching `term_id`.
    fn relations(&self, term_id: i64) -> Result<Vec<(i64, String, String, bool)>, String> {
        let mut result = Vec::new();
        for (sql, is_reverse) in [
            ("SELECT to_term, kind, source FROM relations WHERE from_term = ?1", false),
            ("SELECT from_term, kind, source FROM relations WHERE to_term = ?1", true),
        ] {
            let mut statement = self.connection.prepare_cached(sql).map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([term_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, is_reverse)))
                .map_err(|error| error.to_string())?;
            for row in rows {
                result.push(row.map_err(|error| error.to_string())?);
            }
        }
        Ok(result)
    }

    fn surface_and_vector(&self, term_id: i64) -> Result<(String, Option<Vec<i8>>), String> {
        let mut statement = self
            .connection
            .prepare_cached("SELECT surface, vector FROM terms WHERE term_id = ?1")
            .map_err(|error| error.to_string())?;
        statement
            .query_row(params![term_id], |row| {
                let surface: String = row.get(0)?;
                let blob: Option<Vec<u8>> = row.get(1)?;
                Ok((surface, blob.map(|bytes| bytes.into_iter().map(|byte| byte as i8).collect::<Vec<i8>>())))
            })
            .map_err(|error| error.to_string())
            .and_then(|(surface, vector)| match vector {
                Some(vector) if vector.len() != self.vector_dimensions => Err(format!(
                    "dataset incompatible: vector of {surface:?} has {} dimensions, expected {}",
                    vector.len(),
                    self.vector_dimensions
                )),
                vector => Ok((surface, vector)),
            })
    }

    /// Every term's stored keys, for `related-check`.
    pub fn all_term_keys(&self) -> Result<Vec<(String, String, String, bool)>, String> {
        let mut statement = self
            .connection
            .prepare("SELECT exact_key, match_key, joined_key, is_protected FROM terms ORDER BY term_id")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get::<_, i64>(3)? == 1)))
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct QueryMatch {
    /// The query text that matched: the whole input, a protected spelling, or words.
    pub span: String,
    pub term: String,
    /// `exact`, `exact_stripped`, `exact_bare`, `protected`, `match_key`, `lemma`, `joined`,
    /// `joined_lemma`.
    #[serde(rename = "match")]
    pub match_kind: &'static str,
    /// `whole`, `protected`, or `part`.
    pub scope: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Skipped {
    pub span: String,
    /// `non_ascii`, `digits_only`, or `single_letter`.
    pub reason: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct QueryVector {
    /// `whole_term` (the whole query matched one term), `mean_of_parts`, or `none`.
    pub basis: &'static str,
    pub terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RelatedTerm {
    pub term: String,
    /// Cosine similarity to the query vector x 100, one decimal. `None` without vectors.
    pub similarity_pct: Option<f64>,
    /// Relation kinds that produced the candidate; `reverse:<kind>` when the candidate is the
    /// relation's source, `query_part` when it is a matched part of the query itself.
    pub kinds: Vec<String>,
    pub sources: Vec<String>,
    /// Matched query terms whose relations produced the candidate.
    pub via: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RelatedOutput {
    pub schema: &'static str,
    pub input: String,
    pub dataset_id: String,
    pub keys_version: &'static str,
    /// `ok`, `no_match`, or `matched_without_relations`.
    pub status: &'static str,
    pub query_vector: QueryVector,
    pub matches: Vec<QueryMatch>,
    pub skipped: Vec<Skipped>,
    pub related: Vec<RelatedTerm>,
}

struct MatchedTerm {
    row: TermRow,
    query_match: QueryMatch,
    words: Option<(usize, usize)>,
}

/// Matched terms in discovery order; a term is recorded once per scope.
#[derive(Default)]
struct Matches {
    items: Vec<MatchedTerm>,
    seen: HashSet<(i64, &'static str)>,
}

impl Matches {
    fn push(&mut self, row: TermRow, span: &str, match_kind: &'static str, scope: &'static str, words: Option<(usize, usize)>) {
        if self.seen.insert((row.term_id, scope)) {
            let term = row.surface.clone();
            self.items.push(MatchedTerm {
                row,
                query_match: QueryMatch {
                    span: span.to_string(),
                    term,
                    match_kind,
                    scope,
                },
                words,
            });
        }
    }
}

/// Every phrase stage for a run of query words, in priority order: the words as a match key,
/// with the last word's regular inflection base, the concatenated words, and both combined.
/// Spelling variants are all returned (`event handler`, `event-handler`, `eventhandler`,
/// `back up`/`backup`); a term is listed once, under its first stage.
fn find_phrase(dataset: &RelatedDataset, words: &[String]) -> Result<Vec<(TermRow, &'static str)>, String> {
    let mut hits: Vec<(TermRow, &'static str)> = Vec::new();
    let mut seen: HashSet<i64> = HashSet::new();
    let mut collect = |rows: Vec<TermRow>, match_kind: &'static str| {
        for row in rows {
            if seen.insert(row.term_id) {
                hits.push((row, match_kind));
            }
        }
    };
    collect(dataset.by_match_key(&words.join(" "))?, "match_key");
    let (last, head) = words.split_last().expect("phrase has words");
    let bases = inflection_bases(last);
    for base in &bases {
        let key = head.iter().chain(std::iter::once(base)).cloned().collect::<Vec<_>>().join(" ");
        collect(dataset.by_match_key(&key)?, "lemma");
    }
    collect(dataset.by_joined_key(&words.concat())?, "joined");
    for base in &bases {
        collect(dataset.by_joined_key(&format!("{}{base}", head.concat()))?, "joined_lemma");
    }
    Ok(hits)
}

/// A query that is a single run of digits (`2`, `404`) is not looked up as a phrase; only
/// its exact key can match.
fn is_single_word_query_digits(words: &[String]) -> bool {
    matches!(words, [word] if word.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Blanks every ASCII-case-insensitive occurrence of `needle` in `text` with spaces.
fn blank_out(text: &str, needle: &str) -> String {
    let lowered = text.to_ascii_lowercase();
    let mut bytes = text.as_bytes().to_vec();
    let mut start = 0;
    while let Some(offset) = lowered[start..].find(needle) {
        let begin = start + offset;
        for byte in &mut bytes[begin..begin + needle.len()] {
            *byte = b' ';
        }
        start = begin + needle.len();
    }
    // Only ASCII bytes inside a match were replaced, so the text stays valid UTF-8.
    String::from_utf8(bytes).expect("blanking ASCII keeps UTF-8 valid")
}

fn cosine(left: &[i8], right: &[i8]) -> Option<f64> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }
    let (mut dot, mut left_norm, mut right_norm) = (0i64, 0i64, 0i64);
    for (a, b) in left.iter().zip(right) {
        let (a, b) = (*a as i64, *b as i64);
        dot += a * b;
        left_norm += a * a;
        right_norm += b * b;
    }
    if left_norm == 0 || right_norm == 0 {
        return None;
    }
    Some(dot as f64 / ((left_norm as f64).sqrt() * (right_norm as f64).sqrt()))
}

fn cosine_f32(query: &[f32], vector: &[i8]) -> Option<f64> {
    if query.len() != vector.len() || query.is_empty() {
        return None;
    }
    let (mut dot, mut query_norm, mut vector_norm) = (0f64, 0f64, 0f64);
    for (q, v) in query.iter().zip(vector) {
        let (q, v) = (*q as f64, *v as f64);
        dot += q * v;
        query_norm += q * q;
        vector_norm += v * v;
    }
    if query_norm == 0.0 || vector_norm == 0.0 {
        return None;
    }
    Some(dot / (query_norm.sqrt() * vector_norm.sqrt()))
}

/// Normalized mean of unit-scaled vectors; `None` when no vector is available.
fn mean_vector(vectors: &[Vec<i8>]) -> Option<Vec<f32>> {
    let first = vectors.first()?;
    let mut sum = vec![0f32; first.len()];
    for vector in vectors.iter().filter(|vector| vector.len() == first.len()) {
        let norm = vector.iter().map(|v| (*v as f32) * (*v as f32)).sum::<f32>().sqrt();
        if norm == 0.0 {
            continue;
        }
        for (total, value) in sum.iter_mut().zip(vector) {
            *total += *value as f32 / norm;
        }
    }
    Some(sum)
}

pub fn related(dataset: &RelatedDataset, input: &str) -> Result<RelatedOutput, String> {
    let mut matches = Matches::default();
    let mut skipped: Vec<Skipped> = Vec::new();

    // 1. Protected spellings (`.net`, `c++`) match exactly and consume their characters, so
    //    their symbol-free words (`net`, `c`) are never looked up on their own.
    let trimmed = input.trim();
    let stripped = strip_outer(trimmed);
    let mut remaining = trimmed.to_string();
    let mut protected_hits: Vec<(String, Vec<TermRow>)> = Vec::new();
    for candidate in protected_candidates(trimmed) {
        let hits = dataset.by_protected_key(&candidate)?;
        if !hits.is_empty() {
            remaining = blank_out(&remaining, &candidate);
            protected_hits.push((candidate, hits));
        }
    }
    let has_protected = !protected_hits.is_empty();

    // 2. The whole input by exact key: as written, without outer decoration, and - unless a
    //    protected term matched - without any outer symbol (`.eventHandler`, `eventHandler+`).
    let bare = trimmed.trim_matches(|c: char| !c.is_alphanumeric());
    let mut exact_texts = vec![(trimmed, "exact"), (stripped, "exact_stripped")];
    if !has_protected {
        exact_texts.push((bare, "exact_bare"));
    }
    let mut tried: HashSet<String> = HashSet::new();
    for (text, match_kind) in exact_texts {
        let key = exact_key(text);
        if key.is_empty() || !tried.insert(key.clone()) {
            continue;
        }
        for row in dataset.by_exact_key(&key)? {
            matches.push(row, trimmed, match_kind, "whole", None);
        }
    }
    let is_whole_exact = !matches.items.is_empty();
    let mut is_whole_atomic = matches.items.iter().any(|term| term.row.is_atomic);
    for (candidate, hits) in protected_hits {
        if is_whole_exact && exact_key(stripped) == candidate {
            continue;
        }
        for row in hits {
            matches.push(row, &candidate, "protected", "protected", None);
        }
    }

    // 3. Words, the whole phrase, and every sub-phrase down to single words.
    let query = query_words(&remaining);
    for span in &query.skipped_non_ascii {
        skipped.push(Skipped {
            span: span.clone(),
            reason: "non_ascii",
        });
    }
    let mut words = query.words;
    let full_scope = if has_protected { "part" } else { "whole" };
    if !words.is_empty() && !is_single_word_query_digits(&words) {
        let hits = find_phrase(dataset, &words)?;
        let span_text = words.join(" ");
        // A concatenated spelling (`eventloop`) takes the matched term's word boundaries,
        // so its parts are looked up like those of `eventLoop`.
        let joined_words = hits
            .iter()
            .filter(|(_, match_kind)| matches!(*match_kind, "joined" | "joined_lemma"))
            .map(|(row, _)| row.match_key.split(' ').map(str::to_string).collect::<Vec<_>>())
            .max_by_key(Vec::len);
        if let Some(term_words) = joined_words.filter(|term_words| term_words.len() > words.len()) {
            words = term_words;
        }
        is_whole_atomic |= hits.iter().any(|(row, _)| row.is_atomic);
        let full_range = Some((0, words.len()));
        for (row, match_kind) in hits {
            matches.push(row, &span_text, match_kind, full_scope, full_range);
        }
    }
    if !is_whole_atomic {
        let mut atomic_spans: Vec<(usize, usize)> = Vec::new();
        for (start, end) in spans(words.len()) {
            if start == 0 && end == words.len() {
                continue;
            }
            if atomic_spans.iter().any(|&(outer_start, outer_end)| outer_start <= start && end <= outer_end) {
                continue;
            }
            let span_words = &words[start..end];
            let span_text = span_words.join(" ");
            if let [word] = span_words {
                if word.bytes().all(|byte| byte.is_ascii_digit()) {
                    skipped.push(Skipped {
                        span: span_text,
                        reason: "digits_only",
                    });
                    continue;
                }
                if word.len() == 1 {
                    skipped.push(Skipped {
                        span: span_text,
                        reason: "single_letter",
                    });
                    continue;
                }
            }
            let hits = find_phrase(dataset, span_words)?;
            if hits.iter().any(|(row, _)| row.is_atomic) {
                atomic_spans.push((start, end));
            }
            for (row, match_kind) in hits {
                matches.push(row, &span_text, match_kind, "part", Some((start, end)));
            }
        }
    }
    if is_single_word_query_digits(&words) && !is_whole_exact {
        skipped.push(Skipped {
            span: words[0].clone(),
            reason: "digits_only",
        });
    }
    let matched = matches.items;

    // 4. The query vector: the whole term's vector, else the mean over a left-to-right
    //    cover of the longest matched parts plus the protected terms.
    let whole_terms: Vec<&MatchedTerm> = matched.iter().filter(|term| term.query_match.scope == "whole").collect();
    let mut vector_terms: Vec<&MatchedTerm> = Vec::new();
    let mut basis = "none";
    if let Some(whole) = whole_terms.first() {
        vector_terms.push(whole);
        basis = "whole_term";
    } else {
        let mut parts: Vec<&MatchedTerm> = matched.iter().filter(|term| term.words.is_some()).collect();
        parts.sort_by_key(|term| {
            let (start, end) = term.words.expect("part has words");
            (std::cmp::Reverse(end - start), start, term.row.term_id)
        });
        let mut covered = vec![false; words.len()];
        for part in parts {
            let (start, end) = part.words.expect("part has words");
            if covered[start..end].iter().any(|is_covered| *is_covered) {
                continue;
            }
            covered[start..end].iter_mut().for_each(|is_covered| *is_covered = true);
            vector_terms.push(part);
        }
        vector_terms.extend(matched.iter().filter(|term| term.query_match.scope == "protected"));
        if !vector_terms.is_empty() {
            basis = "mean_of_parts";
        }
    }
    let mut query_vectors = Vec::new();
    for term in &vector_terms {
        if let (_, Some(vector)) = dataset.surface_and_vector(term.row.term_id)? {
            query_vectors.push(vector);
        }
    }
    let query_vector = mean_vector(&query_vectors);

    // 5. Candidates: every relation of every matched term, plus matched parts themselves.
    let whole_ids: HashSet<i64> = whole_terms.iter().map(|term| term.row.term_id).collect();
    struct Candidate {
        kinds: BTreeSet<String>,
        sources: BTreeSet<String>,
        via: BTreeSet<String>,
    }
    let mut candidates: HashMap<i64, Candidate> = HashMap::new();
    let mut candidate_order: Vec<i64> = Vec::new();
    let mut add = |term_id: i64, kind: String, source: Option<String>, via: &str| {
        let candidate = candidates.entry(term_id).or_insert_with(|| {
            candidate_order.push(term_id);
            Candidate {
                kinds: BTreeSet::new(),
                sources: BTreeSet::new(),
                via: BTreeSet::new(),
            }
        });
        candidate.kinds.insert(kind);
        if let Some(source) = source {
            candidate.sources.insert(source);
        }
        candidate.via.insert(via.to_string());
    };
    for term in &matched {
        if term.query_match.scope != "whole" && !whole_ids.contains(&term.row.term_id) {
            add(term.row.term_id, "query_part".to_string(), None, &term.query_match.span);
        }
        for (other, kind, source, is_reverse) in dataset.relations(term.row.term_id)? {
            if whole_ids.contains(&other) {
                continue;
            }
            let kind = if is_reverse { format!("reverse:{kind}") } else { kind };
            add(other, kind, Some(source), &term.row.surface);
        }
    }
    let mut related = Vec::with_capacity(candidate_order.len());
    for term_id in candidate_order {
        let candidate = candidates.remove(&term_id).expect("candidate recorded");
        let (surface, vector) = dataset.surface_and_vector(term_id)?;
        let similarity = match (&query_vector, &vector) {
            (Some(_), Some(vector)) if query_vectors.len() == 1 => cosine(&query_vectors[0], vector),
            (Some(query), Some(vector)) => cosine_f32(query, vector),
            _ => None,
        };
        related.push(RelatedTerm {
            term: surface,
            similarity_pct: similarity.map(|value| (value * 1000.0).round() / 10.0),
            kinds: candidate.kinds.into_iter().collect(),
            sources: candidate.sources.into_iter().collect(),
            via: candidate.via.into_iter().collect(),
        });
    }
    related.sort_by(|left, right| {
        let left_score = left.similarity_pct.unwrap_or(f64::NEG_INFINITY);
        let right_score = right.similarity_pct.unwrap_or(f64::NEG_INFINITY);
        right_score.total_cmp(&left_score).then_with(|| left.term.cmp(&right.term))
    });

    let status = if matched.is_empty() {
        "no_match"
    } else if related.is_empty() {
        "matched_without_relations"
    } else {
        "ok"
    };
    Ok(RelatedOutput {
        schema: OUTPUT_SCHEMA,
        input: input.to_string(),
        dataset_id: dataset.dataset_id.clone(),
        keys_version: RELATED_KEYS_VERSION,
        status,
        query_vector: QueryVector {
            basis,
            terms: vector_terms.iter().map(|term| term.row.surface.clone()).collect(),
        },
        matches: matched.into_iter().map(|term| term.query_match).collect(),
        skipped,
        related,
    })
}

/// Recomputes every term's keys with this reader's `rtk-1` implementation.
pub fn check_keys(dataset: &RelatedDataset) -> Result<serde_json::Value, String> {
    let mut checked = 0usize;
    let mut mismatches = Vec::new();
    for (stored_exact, stored_match, stored_joined, stored_protected) in dataset.all_term_keys()? {
        checked += 1;
        let (match_key, joined_key) = term_keys(&stored_exact);
        let is_consistent = exact_key(&stored_exact) == stored_exact
            && match_key == stored_match
            && joined_key == stored_joined
            && is_protected(&stored_exact) == stored_protected;
        if !is_consistent {
            mismatches.push(serde_json::json!({
                "exact_key": stored_exact, "match_key": stored_match, "joined_key": stored_joined,
                "is_protected": stored_protected, "rust_match_key": match_key, "rust_joined_key": joined_key,
                "rust_is_protected": is_protected(&stored_exact),
            }));
        }
    }
    Ok(serde_json::json!({
        "status": if mismatches.is_empty() { "pass" } else { "fail" },
        "keys_version": RELATED_KEYS_VERSION,
        "terms_checked": checked,
        "mismatches": mismatches.len(),
        "mismatch_examples": mismatches.into_iter().take(20).collect::<Vec<_>>(),
    }))
}

/// One line per related term: `<similarity>%  <term>  <kinds> <- <via>`.
pub fn render_text(output: &RelatedOutput) -> String {
    let mut lines = Vec::new();
    let matches: Vec<String> = output
        .matches
        .iter()
        .map(|item| format!("{} = {} ({}, {})", item.span, item.term, item.scope, item.match_kind))
        .collect();
    lines.push(format!("# {}  [{}]", output.input, output.status));
    if !matches.is_empty() {
        lines.push(format!("  matched: {}", matches.join("; ")));
    }
    if !output.skipped.is_empty() {
        let skipped: Vec<String> = output.skipped.iter().map(|item| format!("{} ({})", item.span, item.reason)).collect();
        lines.push(format!("  skipped: {}", skipped.join(", ")));
    }
    lines.push(format!("  query vector: {} {}", output.query_vector.basis, output.query_vector.terms.join(" + ")));
    for item in &output.related {
        let similarity = item.similarity_pct.map_or("  n/a".to_string(), |value| format!("{value:5.1}"));
        lines.push(format!(
            "  {similarity}%  {}  [{}] <- {}",
            item.term,
            item.kinds.join(","),
            item.via.join(", ")
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small `rt-related/1` dataset. Vectors are 4-dimensional and chosen so similarity
    /// order is known: `emitter` is closest to `event`, `iteration` closest to `loop`.
    fn fixture() -> RelatedDataset {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE related_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE sources (source TEXT PRIMARY KEY, license_spdx TEXT NOT NULL, description TEXT NOT NULL);
                 CREATE TABLE terms (term_id INTEGER PRIMARY KEY, surface TEXT NOT NULL, exact_key TEXT NOT NULL UNIQUE,
                   match_key TEXT NOT NULL, joined_key TEXT NOT NULL, is_protected INTEGER NOT NULL,
                   is_atomic INTEGER NOT NULL, vector BLOB);
                 CREATE TABLE relations (from_term INTEGER NOT NULL, to_term INTEGER NOT NULL, kind TEXT NOT NULL,
                   source TEXT NOT NULL, PRIMARY KEY (from_term, to_term, kind, source)) WITHOUT ROWID;
                 INSERT INTO related_meta VALUES ('schema_version', 'rt-related/1'), ('keys_version', 'rtk-1'),
                   ('dataset_id', 'test'), ('vector_dimensions', '4');
                 INSERT INTO sources VALUES ('test', 'CC0-1.0', 'fixture');",
            )
            .unwrap();
        let terms: &[(i64, &str, bool, [i8; 4])] = &[
            (1, "event", false, [100, 0, 0, 0]),
            (2, "loop", false, [0, 100, 0, 0]),
            (3, "event loop", false, [70, 70, 0, 0]),
            (4, "emitter", false, [90, 10, 0, 0]),
            (5, "iteration", false, [10, 90, 0, 0]),
            (6, "microtask", false, [60, 60, 10, 0]),
            (7, ".net", false, [0, 0, 100, 0]),
            (8, "net", false, [0, 0, 0, 100]),
            (9, "network", false, [0, 0, 10, 90]),
            (10, "c#", false, [0, 0, 90, 10]),
            (11, "c", false, [0, 0, 0, 100]),
            (12, "c++", false, [0, 0, 80, 20]),
            (13, "JavaScript", true, [50, 0, 50, 0]),
            (14, "java", false, [0, 50, 50, 0]),
            (15, "script", false, [50, 50, 0, 0]),
            (16, "listener", false, [95, 5, 0, 0]),
            (17, "event listener", false, [90, 0, 0, 10]),
            (18, "k8s", false, [0, 10, 10, 90]),
            (19, "kubernetes", false, [0, 10, 10, 85]),
            (20, "backup", false, [10, 10, 10, 10]),
            (21, "back up", false, [10, 10, 10, 9]),
            (22, "snapshot", false, [10, 10, 10, 11]),
            (23, "café", false, [1, 1, 1, 1]),
            (24, "http/2", false, [5, 5, 5, 5]),
            (25, "http", false, [5, 5, 5, 4]),
            (26, "s", false, [0, 1, 1, 1]),
            (27, "dotnet", false, [0, 0, 95, 0]),
            (28, "ecmascript", false, [50, 0, 49, 0]),
        ];
        for (term_id, surface, is_atomic, vector) in terms {
            let exact = exact_key(surface);
            let (match_key, joined_key) = term_keys(&exact);
            let blob: Vec<u8> = vector.iter().map(|value| *value as u8).collect();
            connection
                .execute(
                    "INSERT INTO terms VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![term_id, surface, exact, match_key, joined_key, is_protected(&exact) as i64, *is_atomic as i64, blob],
                )
                .unwrap();
        }
        let relations: &[(i64, i64, &str)] = &[
            (1, 4, "near"),
            (1, 16, "near"),
            (2, 5, "synonym"),
            (3, 6, "near"),
            (7, 27, "synonym"),
            (8, 9, "abbreviation"),
            (10, 7, "family"),
            (11, 12, "family"),
            (13, 28, "synonym"),
            (14, 2, "near"),
            (15, 1, "near"),
            (18, 19, "abbreviation"),
            (20, 22, "near"),
            (25, 24, "form"),
            (26, 1, "near"),
        ];
        for (from, to, kind) in relations {
            connection
                .execute("INSERT INTO relations VALUES (?1, ?2, ?3, 'test')", params![from, to, kind])
                .unwrap();
        }
        RelatedDataset::from_connection(connection).unwrap()
    }

    fn terms_of(output: &RelatedOutput) -> Vec<&str> {
        output.related.iter().map(|item| item.term.as_str()).collect()
    }

    #[test]
    fn camel_case_query_matches_whole_and_parts() {
        let dataset = fixture();
        let output = related(&dataset, "eventLoop").unwrap();
        let matched: Vec<(&str, &str)> = output.matches.iter().map(|item| (item.term.as_str(), item.scope)).collect();
        assert_eq!(matched, vec![("event loop", "whole"), ("event", "part"), ("loop", "part")]);
        assert_eq!(output.query_vector.basis, "whole_term");
        let related_terms = terms_of(&output);
        for expected in ["microtask", "emitter", "iteration", "listener", "event", "loop"] {
            assert!(related_terms.contains(&expected), "{expected} missing from {related_terms:?}");
        }
        assert!(!related_terms.contains(&"event loop"), "the whole match is not its own relative");
    }

    #[test]
    fn attached_symbols_do_not_change_the_result() {
        let dataset = fixture();
        let baseline = related(&dataset, "eventLoop").unwrap();
        for input in ["$eventLoop", "`eventLoop`", "event_loop", "EVENT_LOOP", "event-loop", "eventLoop()", "@eventLoop", "event loop", "eventloop"] {
            let output = related(&dataset, input).unwrap();
            assert_eq!(output.related, baseline.related, "{input}");
        }
    }

    #[test]
    fn results_are_sorted_by_similarity() {
        let dataset = fixture();
        let output = related(&dataset, "event").unwrap();
        let scores: Vec<f64> = output.related.iter().map(|item| item.similarity_pct.unwrap()).collect();
        assert!(scores.windows(2).all(|pair| pair[0] >= pair[1]), "{scores:?}");
        assert_eq!(terms_of(&output)[0], "listener");
        assert!(output.related.iter().all(|item| item.similarity_pct.unwrap() <= 100.0));
    }

    #[test]
    fn protected_terms_never_fall_back_to_bare_words() {
        let dataset = fixture();
        let output = related(&dataset, ".net").unwrap();
        assert_eq!(terms_of(&output), vec!["dotnet", "c#"]);
        assert!(!terms_of(&output).contains(&"network"));
        let output = related(&dataset, "C#").unwrap();
        assert_eq!(terms_of(&output), vec![".net"]);
        // `c` reaches `c++` only through an explicit relation, never as a match of the query.
        let output = related(&dataset, "c++").unwrap();
        let matched: Vec<&str> = output.matches.iter().map(|item| item.term.as_str()).collect();
        assert_eq!(matched, vec!["c++"]);
        assert_eq!(output.related.len(), 1);
        assert_eq!(output.related[0].term, "c");
        assert_eq!(output.related[0].kinds, vec!["reverse:family".to_string()]);
        let output = related(&dataset, "c++17").unwrap();
        assert!(!output.matches.iter().any(|item| item.term == "c"));
        let output = related(&dataset, "C#/.NET").unwrap();
        let matched: Vec<&str> = output.matches.iter().map(|item| item.term.as_str()).collect();
        assert_eq!(matched, vec![".net", "c#"]);
        assert!(!terms_of(&output).contains(&"network"));
    }

    #[test]
    fn atomic_names_are_not_split() {
        let dataset = fixture();
        let output = related(&dataset, "JavaScript").unwrap();
        assert_eq!(terms_of(&output), vec!["ecmascript"]);
        let output = related(&dataset, "JavaScript engine").unwrap();
        assert!(!output.matches.iter().any(|item| item.term == "java" || item.term == "script"));
    }

    #[test]
    fn numeronyms_and_digits_are_not_split() {
        let dataset = fixture();
        let output = related(&dataset, "k8s").unwrap();
        assert_eq!(terms_of(&output), vec!["kubernetes"]);
        assert!(!output.matches.iter().any(|item| item.term == "s"));
        let output = related(&dataset, "HTTP/2").unwrap();
        assert_eq!(output.matches[0].term, "http/2");
        assert!(output.skipped.iter().any(|item| item.span == "2" && item.reason == "digits_only"));
        assert!(!output.matches.iter().any(|item| item.span == "2"));
    }

    #[test]
    fn joined_spellings_are_merged_and_exact_comes_first() {
        let dataset = fixture();
        let output = related(&dataset, "backup").unwrap();
        let matched: Vec<(&str, &str)> = output.matches.iter().map(|item| (item.term.as_str(), item.match_kind)).collect();
        assert_eq!(matched, vec![("backup", "exact"), ("back up", "joined")]);
        assert_eq!(output.query_vector.terms, vec!["backup".to_string()]);
        assert_eq!(terms_of(&output), vec!["snapshot"]);
        let output = related(&dataset, "back up").unwrap();
        let matched: Vec<(&str, &str)> = output.matches.iter().map(|item| (item.term.as_str(), item.match_kind)).collect();
        assert_eq!(matched, vec![("back up", "exact"), ("backup", "joined")]);
        assert_eq!(output.query_vector.terms, vec!["back up".to_string()]);
    }

    #[test]
    fn protected_symbols_without_a_protected_term_are_stripped() {
        let dataset = fixture();
        let baseline = related(&dataset, "event").unwrap();
        for input in [".event", "event+", "event#", "event++"] {
            let output = related(&dataset, input).unwrap();
            assert_eq!(output.matches[0].term, "event", "{input}");
            assert_eq!(output.related, baseline.related, "{input}");
        }
    }

    #[test]
    fn non_ascii_words_are_skipped_not_stripped() {
        let dataset = fixture();
        let output = related(&dataset, "café").unwrap();
        assert_eq!(output.matches[0].term, "café");
        let output = related(&dataset, "café event").unwrap();
        assert!(output.skipped.iter().any(|item| item.span == "café" && item.reason == "non_ascii"));
        assert!(!output.matches.iter().any(|item| item.span.contains("caf")));
        assert_eq!(output.matches[0].term, "event");
    }

    #[test]
    fn plural_identifiers_reach_their_lemma() {
        let dataset = fixture();
        let output = related(&dataset, "eventListeners").unwrap();
        assert_eq!(output.matches[0].term, "event listener");
        assert_eq!(output.matches[0].match_kind, "lemma");
        let output = related(&dataset, "eventlisteners").unwrap();
        assert_eq!(output.matches[0].term, "event listener");
        assert_eq!(output.matches[0].match_kind, "joined_lemma");
    }

    #[test]
    fn single_letter_parts_are_skipped_but_single_letter_queries_match() {
        let dataset = fixture();
        let output = related(&dataset, "s").unwrap();
        assert_eq!(output.matches[0].term, "s");
        let output = related(&dataset, "sEvent").unwrap();
        assert!(output.skipped.iter().any(|item| item.span == "s" && item.reason == "single_letter"));
    }

    #[test]
    fn unknown_input_has_no_match() {
        let dataset = fixture();
        for input in ["", "   ", "!!!", "zzzz", "日本語"] {
            let output = related(&dataset, input).unwrap();
            assert_eq!(output.status, "no_match", "{input}");
            assert!(output.related.is_empty());
        }
    }

    #[test]
    fn mean_of_parts_is_used_without_a_whole_match() {
        let dataset = fixture();
        let output = related(&dataset, "loopEvent").unwrap();
        assert_eq!(output.query_vector.basis, "mean_of_parts");
        assert_eq!(output.query_vector.terms, vec!["loop".to_string(), "event".to_string()]);
    }

    #[test]
    fn keys_check_passes_on_fixture() {
        let dataset = fixture();
        assert_eq!(check_keys(&dataset).unwrap()["status"], "pass");
    }

    #[test]
    fn blank_out_is_case_insensitive() {
        assert_eq!(blank_out("C#/.NET", ".net"), "C#/    ");
        assert_eq!(blank_out("use c# and C#", "c#"), "use    and   ");
    }
}
