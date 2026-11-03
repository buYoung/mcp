//! Recall of query phrases that one indexed docstring or string literal states verbatim, for
//! files the ranked search left out. Common query terms can fill the ranked pool with other
//! files while the file whose comment or message states the phrase ranks far below it. This
//! runs after ranking and never changes the ranked results; the search tool only admits the
//! matches into tail rows and bytes the ranked output left unused.

use super::engine::INDEXED_LITERAL_MAX_CHARS;
use super::ranking::{restrict_to_scope, DOCSTRING_SCORE_BOOST, LITERAL_SCORE_BOOST};
use super::{SearchResult, SearcherHandle};
use crate::parser::ExtractedFile;
use std::collections::HashSet;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, PhraseQuery, Query};
use tantivy::schema::{Field, Value};
use tantivy::tokenizer::TextAnalyzer;
use tantivy::{DocAddress, Searcher, TantivyDocument, Term};

/// The record that states a recalled phrase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PhraseAnchor {
    Docstring {
        symbol: String,
        start_line: usize,
        end_line: usize,
    },
    Literal {
        line: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PhraseMatch {
    pub file_path: String,
    pub total_lines: usize,
    pub anchor: PhraseAnchor,
}

/// Query-grammar words; a query using them keeps its operator meaning only.
const OPERATOR_WORDS: [&str; 4] = ["AND", "OR", "NOT", "IN"];

fn analyze(analyzer: &mut TextAnalyzer, text: &str) -> Vec<(usize, String)> {
    let mut tokens = Vec::new();
    let mut stream = analyzer.token_stream(text);
    while stream.advance() {
        let token = stream.token();
        tokens.push((token.position, token.text.clone()));
    }
    tokens
}

fn is_consecutive(tokens: &[(usize, String)]) -> bool {
    tokens.windows(2).all(|pair| pair[1].0 == pair[0].0 + 1)
}

/// True when `tokens` hold `unit` at consecutive positions, the way a phrase query matches.
fn holds_phrase(tokens: &[(usize, String)], unit: &[String]) -> bool {
    tokens.windows(unit.len()).any(|window| {
        is_consecutive(window)
            && window
                .iter()
                .zip(unit)
                .all(|((_, text), term)| text == term)
    })
}

/// Quotes open after whitespace or the query start and close before whitespace or the end;
/// anything else is left to the query parser's own reading.
fn has_whole_word_quotes(query: &str) -> bool {
    let chars: Vec<char> = query.chars().collect();
    let quotes: Vec<usize> = (0..chars.len()).filter(|&at| chars[at] == '"').collect();
    quotes.len().is_multiple_of(2)
        && quotes.iter().enumerate().all(|(order, &at)| {
            let neighbor = if order % 2 == 0 {
                at.checked_sub(1).map(|before| chars[before])
            } else {
                chars.get(at + 1).copied()
            };
            neighbor.is_none_or(char::is_whitespace)
        })
}

fn push_unit(units: &mut Vec<Vec<String>>, unit: Vec<String>) {
    if !units.contains(&unit) {
        units.push(unit);
    }
}

fn push_pairs(units: &mut Vec<Vec<String>>, run: &mut Vec<String>) {
    for pair in run.windows(2) {
        push_unit(units, pair.to_vec());
    }
    run.clear();
}

/// Phrase units of a plain multi-word query, in query order: every quoted phrase of two or
/// more tokens as a whole, and every adjacent word pair inside a run of unquoted words. A
/// pair never joins words across a quote, and a word the analyzer splits or drops ends its
/// run. A single word or any query-grammar syntax (fields, operators, boosts, negation,
/// wildcards, escapes, qualified names) yields no units.
fn phrase_units(query: &str, analyzer: &mut TextAnalyzer) -> Vec<Vec<String>> {
    let is_plain = query
        .chars()
        .all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '_' || c == '"')
        && has_whole_word_quotes(query)
        && query.split_whitespace().count() > 1
        && !query
            .split(|c: char| c.is_whitespace() || c == '"')
            .any(|word| OPERATOR_WORDS.contains(&word));
    if !is_plain {
        return Vec::new();
    }
    let mut units = Vec::new();
    for (index, segment) in query.split('"').enumerate() {
        if index % 2 == 1 {
            let tokens = analyze(analyzer, segment);
            // A dropped long token leaves a position gap the phrase cannot cross.
            if tokens.len() >= 2 && is_consecutive(&tokens) {
                push_unit(
                    &mut units,
                    tokens.into_iter().map(|(_, text)| text).collect(),
                );
            }
            continue;
        }
        let mut run = Vec::new();
        for word in segment.split_whitespace() {
            let mut tokens = analyze(analyzer, word);
            if tokens.len() == 1 {
                run.push(tokens.remove(0).1);
            } else {
                push_pairs(&mut units, &mut run);
            }
        }
        push_pairs(&mut units, &mut run);
    }
    units
}

/// The first docstring, then the first literal, that states `unit` within one record. A
/// literal is checked only over its indexed prefix, so an unindexed tail never anchors a match.
fn phrase_anchor(
    file: &ExtractedFile,
    unit: &[String],
    analyzer: &mut TextAnalyzer,
) -> Option<PhraseAnchor> {
    let mut symbols: Vec<_> = file
        .symbols
        .iter()
        .filter(|symbol| symbol.docstring.is_some())
        .collect();
    symbols.sort_by_key(|symbol| (symbol.range.start_line, symbol.range.end_line_inclusive()));
    for symbol in symbols {
        let docstring = symbol.docstring.as_deref().unwrap_or_default();
        if holds_phrase(&analyze(analyzer, docstring), unit) {
            return Some(PhraseAnchor::Docstring {
                symbol: symbol.name.clone(),
                start_line: symbol.range.start_line,
                end_line: symbol.range.end_line_inclusive(),
            });
        }
    }
    let mut literals: Vec<_> = file.literals.iter().collect();
    literals.sort_by_key(|literal| literal.line);
    literals.into_iter().find_map(|literal| {
        let indexed: String = literal
            .text
            .chars()
            .take(INDEXED_LITERAL_MAX_CHARS)
            .collect();
        holds_phrase(&analyze(analyzer, &indexed), unit)
            .then_some(PhraseAnchor::Literal { line: literal.line })
    })
}

struct RecallBudget<'a> {
    ranked_paths: HashSet<&'a str>,
    decodes: usize,
    decode_limit: usize,
}

impl SearcherHandle {
    fn phrase_query(&self, unit: &[String]) -> Box<dyn Query> {
        let clause = |field: Field, boost: f32| -> (Occur, Box<dyn Query>) {
            let terms = unit
                .iter()
                .map(|token| Term::from_field_text(field, token))
                .collect();
            let phrase = Box::new(PhraseQuery::new(terms));
            (Occur::Should, Box::new(BoostQuery::new(phrase, boost)))
        };
        Box::new(BooleanQuery::new(vec![
            clause(self.docstring_field, DOCSTRING_SCORE_BOOST),
            clause(self.literal_field, LITERAL_SCORE_BOOST),
        ]))
    }

    /// The highest-scoring file outside the ranked results whose own record states `unit`.
    /// Equal scores are ordered by path. Returns `None` once the decode budget is spent.
    fn first_unranked_record(
        &self,
        searcher: &Searcher,
        analyzer: &mut TextAnalyzer,
        unit: &[String],
        hits: &[(f32, DocAddress)],
        budget: &mut RecallBudget<'_>,
    ) -> Result<Option<PhraseMatch>, String> {
        let mut start = 0;
        while start < hits.len() {
            let score = hits[start].0;
            let end = hits[start..]
                .iter()
                .position(|(other, _)| *other != score)
                .map_or(hits.len(), |offset| start + offset);
            let mut group = Vec::with_capacity(end - start);
            for (_, address) in &hits[start..end] {
                if budget.decodes >= budget.decode_limit {
                    return Ok(None);
                }
                budget.decodes += 1;
                let doc = searcher
                    .doc::<TantivyDocument>(*address)
                    .map_err(|error| error.to_string())?;
                let path = doc
                    .get_first(self.file_path_field)
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                group.push((path, doc));
            }
            group.sort_by(|left, right| left.0.cmp(&right.0));
            for (path, doc) in group {
                if budget.ranked_paths.contains(path.as_str()) {
                    continue;
                }
                let Ok(file) = serde_json::from_str::<ExtractedFile>(
                    doc.get_first(self.extracted_json_field)
                        .and_then(|value| value.as_str())
                        .unwrap_or("{}"),
                ) else {
                    continue;
                };
                if let Some(anchor) = phrase_anchor(&file, unit, analyzer) {
                    return Ok(Some(PhraseMatch {
                        file_path: path,
                        total_lines: file.total_lines,
                        anchor,
                    }));
                }
            }
            start = end;
        }
        Ok(None)
    }

    /// Files outside `ranked` that state a phrase of `query` in one docstring or literal, at
    /// most `max_matches`, one per phrase. Phrases are tried from the rarest, then by query
    /// order; a phrase in no file or in more files than `candidate_limit` adds nothing, and
    /// document decodes stay within `candidate_limit`. A file two phrases choose is listed once.
    pub(crate) fn recall_unranked_phrases(
        &self,
        query: &str,
        workspace_scope: Option<&str>,
        ranked: &[SearchResult],
        max_matches: usize,
        candidate_limit: usize,
    ) -> Result<Vec<PhraseMatch>, String> {
        if max_matches == 0 {
            return Ok(Vec::new());
        }
        let mut analyzer = self
            .index
            .tokenizer_for_field(self.docstring_field)
            .map_err(|error| error.to_string())?;
        let units = phrase_units(query, &mut analyzer);
        if units.is_empty() || units.len() > candidate_limit {
            return Ok(Vec::new());
        }
        let searcher = self.reader.searcher();
        let mut pools = Vec::new();
        for (position, unit) in units.iter().enumerate() {
            let query = restrict_to_scope(
                self.phrase_query(unit),
                self.file_path_field,
                workspace_scope,
            )?;
            let hits = searcher
                .search(
                    &query,
                    &TopDocs::with_limit(candidate_limit + 1).order_by_score(),
                )
                .map_err(|error| error.to_string())?;
            if !hits.is_empty() && hits.len() <= candidate_limit {
                pools.push((hits, position));
            }
        }
        pools.sort_by_key(|(hits, position)| (hits.len(), *position));
        let mut budget = RecallBudget {
            ranked_paths: ranked
                .iter()
                .map(|result| result.file_path.as_str())
                .collect(),
            decodes: 0,
            decode_limit: candidate_limit,
        };
        let mut matches: Vec<PhraseMatch> = Vec::new();
        for (hits, position) in pools {
            if let Some(found) = self.first_unranked_record(
                &searcher,
                &mut analyzer,
                &units[position],
                &hits,
                &mut budget,
            )? {
                if !matches
                    .iter()
                    .any(|known| known.file_path == found.file_path)
                {
                    matches.push(found);
                }
            }
            if matches.len() == max_matches || budget.decodes >= budget.decode_limit {
                break;
            }
        }
        Ok(matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{CodeRange, ExtractedLiteral, ExtractedSymbol, SymbolFlags};

    fn analyzer() -> TextAnalyzer {
        tantivy::tokenizer::TokenizerManager::default()
            .get("default")
            .unwrap()
    }

    fn units(query: &str) -> Vec<String> {
        phrase_units(query, &mut analyzer())
            .into_iter()
            .map(|unit| unit.join(" "))
            .collect()
    }

    #[test]
    fn test_phrase_units_pair_adjacent_bare_words_and_keep_quotes_whole() {
        assert_eq!(
            units("git repository sync job message bus"),
            [
                "git repository",
                "repository sync",
                "sync job",
                "job message",
                "message bus"
            ]
        );
        assert_eq!(units("\"git\" \"message bus\""), ["message bus"]);
        assert_eq!(
            units("lease \"worker lease renewal\" state"),
            ["worker lease renewal"]
        );
        assert_eq!(units("sync sync job"), ["sync sync", "sync job"]);
        assert_eq!(
            units("결과 경로 해석 상태"),
            ["결과 경로", "경로 해석", "해석 상태"]
        );
        // An identifier the analyzer splits ends the run instead of pairing its parts.
        assert_eq!(units("plan_note_folding display order"), ["display order"]);
    }

    #[test]
    fn test_phrase_units_leave_identifier_and_grammar_queries_unchanged() {
        for query in [
            "plan_note_folding",
            "ensureUserConsent",
            "\"message bus\"x",
            "message AND bus",
            "message OR bus",
            "NOT message bus",
            "message -bus",
            "+message bus",
            "docstring:message bus",
            "message bus~2",
            "message^2 bus",
            "message* bus",
            "config.toml loader",
            "crate::index search",
            "message \\\"bus",
            "\"message bus",
            "\"message\"bus here",
            "(message bus)",
            "message",
            "\"message\"",
        ] {
            assert!(units(query).is_empty(), "{query}");
        }
    }

    #[test]
    fn test_phrase_anchor_requires_one_record_and_indexed_prefix() {
        let range = |line: usize| CodeRange {
            start_line: line,
            start_col: 1,
            end_line: line + 2,
            end_col: 2,
        };
        let symbol = |name: &str, line: usize, docstring: &str| ExtractedSymbol {
            name: name.into(),
            kind: "struct".into(),
            range: range(line),
            docstring: Some(docstring.into()),
            flags: SymbolFlags {
                has_todo: false,
                has_fixme: false,
                is_test: false,
                is_exported: true,
                is_deprecated: false,
            },
            owner: None,
        };
        let file = ExtractedFile {
            file_path: "src/lib.rs".into(),
            total_lines: 40,
            symbols: vec![
                symbol("Later", 30, "lease renewal again"),
                symbol("Split", 10, "the lease"),
                symbol("Owner", 20, "renewal then lease renewal"),
            ],
            literals: vec![ExtractedLiteral {
                text: format!("{} buffer rotation", "x ".repeat(200)),
                line: 5,
            }],
            docstrings: Vec::new(),
            member_names: Vec::new(),
            navigation: None,
        };
        let unit = |text: &str| text.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(
            phrase_anchor(&file, &unit("lease renewal"), &mut analyzer()),
            Some(PhraseAnchor::Docstring {
                symbol: "Owner".into(),
                start_line: 20,
                end_line: 22,
            })
        );
        assert_eq!(
            phrase_anchor(&file, &unit("lease then"), &mut analyzer()),
            None
        );
        assert_eq!(
            phrase_anchor(&file, &unit("buffer rotation"), &mut analyzer()),
            None
        );
        let short = ExtractedFile {
            literals: vec![ExtractedLiteral {
                text: "flush buffer rotation now".into(),
                line: 7,
            }],
            ..file
        };
        assert_eq!(
            phrase_anchor(&short, &unit("buffer rotation"), &mut analyzer()),
            Some(PhraseAnchor::Literal { line: 7 })
        );
    }
}
