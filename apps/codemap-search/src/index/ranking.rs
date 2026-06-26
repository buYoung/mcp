use super::{SearchRankingSignal, SearchResult, SearcherHandle};
use crate::parser::{ExtractedFile, ExtractedLiteral, ExtractedSymbol, QueryTokens};
use std::collections::{HashMap, HashSet};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::{DocAddress, TantivyDocument};

/// Runs a query-parse attempt and converts a panic into `None`. tantivy 0.26's
/// query grammar `panic!`s instead of returning `Err` on some adversarial inputs
/// — e.g. a bare `*` hits `expect("Exist query without a field isn't allowed")`.
/// A malformed search query must degrade gracefully (never-exit contract), not
/// abort the server, so callers treat `None` like a parse failure and fall back.
///
/// We deliberately do NOT swap the process-global panic hook to mute the message:
/// this is a stdio MCP server whose diagnostics go to stderr, a channel separate
/// from the JSON-RPC stdout, so a rare caught-panic line on stderr is harmless to
/// the protocol. Muting the hook globally — even briefly — would swallow panic
/// diagnostics from the indexer/watcher threads if they panic during this window,
/// which is a worse trade than one stray stderr line.
fn parse_query_catching_panic(
    run_parse: impl FnOnce() -> Result<Box<dyn tantivy::query::Query>, tantivy::query::QueryParserError>,
) -> Option<Result<Box<dyn tantivy::query::Query>, tantivy::query::QueryParserError>> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_parse)).ok()
}

/// Post-rank multiplier for files whose path looks like test/bench scaffolding. Tests and
/// benches repeat domain terms heavily, so raw BM25 term frequency lets them crowd the
/// defining sources out of the top ranks; implementations should surface first (the test
/// files stay in the results, just lower).
const TEST_PATH_SCORE_WEIGHT: f32 = 0.3;

/// Post-rank multiplier when a query term exactly equals a discriminative symbol name
/// defined in the file. An exact identifier in the query ("TransactionReadonly", "put_tb")
/// is the strongest signal the user wants its definition, yet under plain BM25 generic
/// co-terms ("error", "enum") can outvote it via term frequency in unrelated files.
const EXACT_NAME_SCORE_BOOST: f32 = 3.0;

/// Post-rank multiplier when a qualified-name query word (`encoding::base64::decode`, a `::`/`.`
/// path) exactly equals one of a file's string literals but the file has NO symbol-exact hit.
/// Legacy dispatch/lookup tables map such a name in a string literal (`"encoding::base64::decode"
/// => …`) that lives ONLY in the literal field (boost 1.0), so under plain BM25 it is buried far
/// below the exec path's owner-qualified symbols. This lifts the qualified-literal file enough to
/// land in the detail/tail beside the exec match — co-exposure, not re-rank: it is held strictly
/// BELOW `EXACT_NAME_SCORE_BOOST` (3.0) so a symbol-exact exec member always ranks first. Gated to
/// qualified (`::`/`.`) forms so a bare common word in a literal can't earn it.
const QUALIFIED_LITERAL_SCORE_BOOST: f32 = 1.6;
/// Gates ONLY the qualified-literal score lift (#1) below. Held off pending Lever-B validation
/// (run cms-perf-improve-20260615): the lift's co-exposure mechanism is proven safe but its
/// weak-model payoff is unconfirmed. `qualified_literal_hit` is still computed and carried, so the
/// literal-exposure label (#2) and cross-path note (#4) stay live — only the rank promotion is
/// dormant. Flip back to `true` to re-enable the lift in one edit.
const ENABLE_QUALIFIED_LITERAL_BOOST: bool = false;

const SYMBOL_SIGNAL_SCORE_CAP: f32 = 1.6;

/// Secondary plain-token query for qualified names is a recall supplement, not the primary
/// ranking signal. Keep its raw score below the primary Tantivy query and let exact owner/member
/// evidence earn its way upward during post-ranking.
const QUALIFIED_TOKEN_SUPPLEMENT_SCORE_WEIGHT: f32 = 0.6;

const COMMON_EXACT_NAMES: &[&str] = &[
    "constructor",
    "definition",
    "function",
    "default",
    "handler",
    "manager",
    "service",
    "context",
    "options",
    "settings",
];

struct CandidateFile {
    raw_score: f32,
    file_path: String,
    total_lines: usize,
    symbols: Vec<ExtractedSymbol>,
    literals: Vec<ExtractedLiteral>,
}

struct SymbolMatch<'a> {
    exact_hit: bool,
    exact_boost_eligible: bool,
    term_match_count: usize,
    owner_match_count: usize,
    path_match_count: usize,
    signal_score: f32,
    symbol: &'a ExtractedSymbol,
}

/// A name specific enough that exact equality with a query term means intent: multi-token
/// identifiers (snake/camel compounds) or long single tokens. Short single-word names
/// ("new", "write", "Error") are too generic to treat as a definition request.
fn is_discriminative_name(name: &str) -> bool {
    name.len() >= 8 || crate::parser::split_identifier(name).len() >= 2
}

fn is_common_exact_name(name: &str, name_frequency: usize) -> bool {
    let lower = name.to_lowercase();
    COMMON_EXACT_NAMES.contains(&lower.as_str())
        || (crate::parser::split_identifier(name).len() <= 1 && name_frequency >= 3)
}

fn term_hits_owner(sym: &ExtractedSymbol, term: &str) -> bool {
    sym.owner.as_ref().is_some_and(|owner| {
        owner.to_lowercase().contains(term)
            || crate::parser::split_identifier(owner)
                .iter()
                .any(|t| t.contains(term))
    })
}

fn term_hits_path(path: &str, term: &str) -> bool {
    path.to_lowercase().contains(term)
}

fn owner_query_match(sym: &ExtractedSymbol, query: &QueryTokens) -> bool {
    query.tokens().iter().any(|term| term_hits_owner(sym, term))
}

fn owner_query_match_count(sym: &ExtractedSymbol, query: &QueryTokens) -> usize {
    query
        .tokens()
        .iter()
        .filter(|term| term_hits_owner(sym, term))
        .count()
}

fn whole_exact_name_hit(sym: &ExtractedSymbol, query: &QueryTokens) -> bool {
    let symbol_name = sym.name.to_lowercase();
    query.contains_word(&symbol_name) || query.contains_raw_word(&symbol_name)
}

fn subtoken_exact_name_hit(sym: &ExtractedSymbol, query: &QueryTokens) -> bool {
    let name_subtokens = crate::parser::split_identifier(&sym.name);
    name_subtokens.len() >= 2
        && name_subtokens
            .iter()
            .all(|subtoken| query.contains_token(subtoken))
}

fn exact_name_hit(sym: &ExtractedSymbol, query: &QueryTokens) -> bool {
    whole_exact_name_hit(sym, query) || subtoken_exact_name_hit(sym, query)
}

fn exact_boost_eligible(
    sym: &ExtractedSymbol,
    query: &QueryTokens,
    _file_path: &str,
    name_frequency: usize,
) -> bool {
    let whole_exact = whole_exact_name_hit(sym, query);
    let subtoken_exact = subtoken_exact_name_hit(sym, query);
    if !(whole_exact || subtoken_exact) {
        return false;
    }
    let owner_match_count = owner_query_match_count(sym, query);
    let has_owner_evidence = owner_match_count > 0;
    if whole_exact
        && query.has_qualified_word()
        && !query.contains_raw_word(&sym.name.to_lowercase())
        && !has_owner_evidence
    {
        return false;
    }
    if subtoken_exact && !whole_exact && query.has_qualified_word() && !has_owner_evidence {
        return false;
    }
    if !is_discriminative_name(&sym.name) && !has_owner_evidence {
        return false;
    }
    if is_common_exact_name(&sym.name, name_frequency) && owner_match_count < 2 {
        return false;
    }
    true
}

/// One query term hits a symbol's NAME when it appears in the raw name, any split sub-token
/// of it, or — for owned members — the owner (enclosing-type) name or its sub-tokens. Name
/// evidence is what gates the partial-coverage promotion: a docstring-only partial match must
/// not unlock snippet rendering (observed: one file whose fn docstrings each grazed 3 of 5
/// query words rendered 11 snippets — 32KB — and starved the rest of the detail view).
/// Owner is folded in here (not just at index time) so an owner-qualified query like
/// "StorageFactory get" actually SELECTS the owned `get` symbol for the detail snippet,
/// matching the index-side owner tokens added to the symbol field.
fn term_hits_symbol_name(sym: &ExtractedSymbol, term: &str) -> bool {
    sym.name.to_lowercase().contains(term)
        || crate::parser::split_identifier(&sym.name)
            .iter()
            .any(|t| t.contains(term))
        || sym.owner.as_ref().is_some_and(|owner| {
            owner.to_lowercase().contains(term)
                || crate::parser::split_identifier(owner)
                    .iter()
                    .any(|t| t.contains(term))
        })
}

/// One query term hits one symbol when it appears in the name, the docstring, or any
/// split sub-token of the name. The match-count criterion behind matched-symbol selection.
fn symbol_matches_term(sym: &ExtractedSymbol, term: &str) -> bool {
    term_hits_symbol_name(sym, term)
        || sym
            .docstring
            .as_ref()
            .is_some_and(|d| d.to_lowercase().contains(term))
}

/// Minimum matched-term count for the partial-coverage promotion: half the query terms,
/// rounded up. Only consulted for 3+ term queries — at 1–2 terms it equals "all terms",
/// so the strict baseline already covers it.
fn partial_match_threshold(term_count: usize) -> usize {
    term_count.div_ceil(2)
}

fn symbol_name_frequencies(candidates: &[CandidateFile]) -> HashMap<String, usize> {
    let mut frequencies = HashMap::new();
    for candidate in candidates {
        for symbol in &candidate.symbols {
            *frequencies.entry(symbol.name.to_lowercase()).or_insert(0) += 1;
        }
    }
    frequencies
}

fn score_symbol_match<'a>(
    sym: &'a ExtractedSymbol,
    query: &QueryTokens,
    file_path: &str,
    name_frequency: usize,
) -> Option<SymbolMatch<'a>> {
    if query.is_empty() {
        return None;
    }

    let term_match_count = query
        .tokens()
        .iter()
        .filter(|term| symbol_matches_term(sym, term))
        .count();
    let name_match_count = query
        .tokens()
        .iter()
        .filter(|term| term_hits_symbol_name(sym, term))
        .count();
    let owner_match_count = query
        .tokens()
        .iter()
        .filter(|term| term_hits_owner(sym, term))
        .count();
    let path_match_count = query
        .tokens()
        .iter()
        .filter(|term| term_hits_path(file_path, term))
        .count();

    let exact_hit = exact_name_hit(sym, query);
    let exact_boost_eligible = exact_boost_eligible(sym, query, file_path, name_frequency);
    let all_terms_hit = term_match_count == query.tokens().len();
    let partial_hit = query.tokens().len() >= 3
        && term_match_count >= partial_match_threshold(query.tokens().len())
        && name_match_count > 0;
    if !(exact_hit || all_terms_hit || partial_hit) {
        return None;
    }

    let has_strong_symbol_evidence = owner_match_count > 0;
    let coverage = term_match_count as f32 / query.tokens().len().max(1) as f32;
    let signal_score = if has_strong_symbol_evidence {
        (coverage * 0.2)
            + if exact_boost_eligible { 0.1 } else { 0.0 }
            + if owner_match_count > 0 { 0.18 } else { 0.0 }
            + if path_match_count > 0 { 0.05 } else { 0.0 }
    } else {
        0.0
    };

    Some(SymbolMatch {
        exact_hit,
        exact_boost_eligible,
        term_match_count,
        owner_match_count,
        path_match_count,
        signal_score,
        symbol: sym,
    })
}

fn symbol_signal_multiplier(scored_symbols: &[SymbolMatch<'_>]) -> f32 {
    let best_signal = scored_symbols
        .iter()
        .map(|scored| scored.signal_score)
        .fold(0.0_f32, f32::max);
    (1.0 + best_signal).min(SYMBOL_SIGNAL_SCORE_CAP)
}

fn candidate_from_doc(
    searcher: &tantivy::Searcher,
    handle: &SearcherHandle,
    score: f32,
    doc_address: DocAddress,
    score_weight: f32,
) -> Result<CandidateFile, String> {
    let doc = searcher
        .doc::<TantivyDocument>(doc_address)
        .map_err(|e| e.to_string())?;

    let file_path = doc
        .get_first(handle.file_path_field)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let extracted_json = doc
        .get_first(handle.extracted_json_field)
        .and_then(|v| v.as_str())
        .unwrap_or("{}");

    let extracted_file: ExtractedFile =
        serde_json::from_str(extracted_json).unwrap_or_else(|_| ExtractedFile {
            file_path: file_path.clone(),
            total_lines: 0,
            symbols: Vec::new(),
            literals: Vec::new(),
            docstrings: Vec::new(),
            navigation: None,
        });

    Ok(CandidateFile {
        raw_score: score * score_weight,
        file_path,
        total_lines: extracted_file.total_lines,
        symbols: extracted_file.symbols,
        literals: extracted_file.literals,
    })
}

fn has_owner_exact_symbol(candidate: &CandidateFile, query: &QueryTokens) -> bool {
    candidate.symbols.iter().any(|sym| {
        owner_query_match(sym, query) && exact_boost_eligible(sym, query, &candidate.file_path, 1)
    })
}

/// A query raw_word counts as "qualified" when it carries a path separator (`::` or `.`) — the
/// shape of a fully-qualified dispatch/lookup name (`encoding::base64::decode`). Bare words are
/// excluded so the qualified-literal boost can never fire on a common single token.
fn is_qualified_word(raw_word: &str) -> bool {
    raw_word.contains("::") || raw_word.contains('.')
}

/// The qualified-name literal in `literals` that EXACTLY equals a qualified query raw_word, if any.
/// Drives [`QUALIFIED_LITERAL_SCORE_BOOST`] and the renderer's literal-aware tail label. Exact
/// whole-value equality (not substring) on the lowercased literal text keeps a constants table that
/// merely contains the path as a fragment from earning the boost; the qualified gate keeps a bare
/// common word out. Returns the original-cased literal text for display. Takes the literal slice
/// (not the whole candidate) so it can run after `candidate.symbols` has been moved out.
fn qualified_literal_exact_hit(
    literals: &[ExtractedLiteral],
    query: &QueryTokens,
) -> Option<String> {
    let qualified_words: Vec<&String> = query
        .raw_words()
        .iter()
        .filter(|raw_word| is_qualified_word(raw_word))
        .collect();
    if qualified_words.is_empty() {
        return None;
    }
    literals
        .iter()
        .find(|lit| {
            let lit_lower = lit.text.to_lowercase();
            qualified_words
                .iter()
                .any(|raw_word| lit_lower == **raw_word)
        })
        .map(|lit| lit.text.clone())
}

fn should_supplement_qualified_query(query: &QueryTokens) -> bool {
    query.raw_words().iter().any(|raw_word| {
        raw_word.contains("::")
            || raw_word
                .split_once('.')
                .is_some_and(|(_, member)| member.starts_with('_') || member.ends_with('_'))
    })
}

/// True for paths that look like test/bench scaffolding rather than implementation.
fn is_test_like_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    let in_test_dir = ["tests/", "test/", "benches/", "bench/", "__tests__/"]
        .iter()
        .any(|dir| lower.starts_with(dir) || lower.contains(&format!("/{dir}")));
    let file_name = lower.rsplit('/').next().unwrap_or(&lower);
    let stem = file_name.split('.').next().unwrap_or(file_name);
    in_test_dir
        || stem.starts_with("test_")
        || stem.ends_with("_test")
        || file_name.contains(".test.")
        || file_name.contains(".spec.")
}

impl SearcherHandle {
    /// BM25 search over the committed index snapshot. Reads index/reader/field handles
    /// only — moved verbatim from the former `TantivySearchEngine::search`.
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
        if query_str.len() > 10000 {
            return Err("Query too long".to_string());
        }
        let searcher = self.reader.searcher();

        let mut query_parser = QueryParser::for_index(
            &self.index,
            vec![
                self.symbol_field,
                self.docstring_field,
                self.file_path_parts_field,
                self.literal_field,
            ],
        );

        query_parser.set_field_boost(self.symbol_field, 4.0);
        query_parser.set_field_boost(self.docstring_field, 2.0);
        query_parser.set_field_boost(self.file_path_parts_field, 1.0);
        // Lowest tier: a literal hit ranks a file in, but never outvotes a symbol or
        // docstring match for the same terms.
        query_parser.set_field_boost(self.literal_field, 1.0);

        if query_str.trim().is_empty() {
            return Ok(Vec::new());
        }

        let query_tokens = QueryTokens::parse(query_str);

        let query = match parse_query_catching_panic(|| query_parser.parse_query(query_str)) {
            Some(Ok(q)) => q,
            // Primary parse failed or panicked (e.g. a bare `*` in tantivy 0.26):
            // retry with the shared tokenizer's plain term query. If the tokenizer found no
            // identifier terms, strip special characters to spaces as the last fallback.
            _ => {
                let escaped = if query_tokens.is_empty() {
                    query_str
                        .to_lowercase()
                        .chars()
                        .map(|c| {
                            if c.is_alphanumeric() || c.is_whitespace() {
                                c
                            } else {
                                ' '
                            }
                        })
                        .collect()
                } else {
                    query_tokens.search_text().to_string()
                };
                if escaped.trim().is_empty() {
                    return Ok(Vec::new());
                }
                match parse_query_catching_panic(|| query_parser.parse_query(&escaped)) {
                    Some(Ok(q)) => q,
                    Some(Err(e)) => return Err(e.to_string()),
                    None => return Ok(Vec::new()),
                }
            }
        };

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit).order_by_score())
            .map_err(|e| e.to_string())?;

        let mut candidates = Vec::new();
        let mut seen_paths = HashSet::new();

        for (score, doc_address) in top_docs {
            let candidate = candidate_from_doc(&searcher, self, score, doc_address, 1.0)?;
            seen_paths.insert(candidate.file_path.clone());
            candidates.push(candidate);
        }

        if should_supplement_qualified_query(&query_tokens)
            && query_tokens.search_text().trim() != query_str.trim()
        {
            if let Some(Ok(token_query)) =
                parse_query_catching_panic(|| query_parser.parse_query(query_tokens.search_text()))
            {
                let supplemental_top_docs = searcher
                    .search(&token_query, &TopDocs::with_limit(limit).order_by_score())
                    .map_err(|e| e.to_string())?;
                for (score, doc_address) in supplemental_top_docs {
                    let candidate = candidate_from_doc(
                        &searcher,
                        self,
                        score,
                        doc_address,
                        QUALIFIED_TOKEN_SUPPLEMENT_SCORE_WEIGHT,
                    )?;
                    if seen_paths.contains(&candidate.file_path) {
                        continue;
                    }
                    if has_owner_exact_symbol(&candidate, &query_tokens) {
                        seen_paths.insert(candidate.file_path.clone());
                        candidates.push(candidate);
                    }
                }
            }
        }

        let name_frequencies = symbol_name_frequencies(&candidates);
        let mut results = Vec::new();

        for candidate in candidates {
            let all_symbols = candidate.symbols;

            // Matched-symbol selection. All-terms is the precision baseline, but agent
            // queries carry glue words ("definition", "handler") no symbol can match,
            // which used to classify nearly every multi-word query as fallback — killing
            // snippets and caller/callee annotations. Two promotions relax it:
            //  - exact name/sub-token subset: a query names the symbol directly;
            //  - partial coverage: in a 3+ term query, a symbol matching at least half
            //    the terms is matched (glue words no longer veto everything).
            // Selection is ordered by exact/boost/signal strength first, so the renderer's
            // symbol cap keeps the strongest evidence instead of line order.
            let mut scored_symbols: Vec<SymbolMatch<'_>> = all_symbols
                .iter()
                .filter_map(|sym| {
                    let frequency = name_frequencies
                        .get(&sym.name.to_lowercase())
                        .copied()
                        .unwrap_or(0);
                    score_symbol_match(sym, &query_tokens, &candidate.file_path, frequency)
                })
                .collect();
            scored_symbols.sort_by(|a, b| {
                b.exact_boost_eligible
                    .cmp(&a.exact_boost_eligible)
                    .then(b.exact_hit.cmp(&a.exact_hit))
                    .then(b.term_match_count.cmp(&a.term_match_count))
                    .then(b.owner_match_count.cmp(&a.owner_match_count))
                    .then(b.path_match_count.cmp(&a.path_match_count))
                    .then_with(|| {
                        b.signal_score
                            .partial_cmp(&a.signal_score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then(a.symbol.range.start_line.cmp(&b.symbol.range.start_line))
            });

            // Post-rank adjustment (see the constants above): an exact discriminative
            // symbol-name hit boosts the file, a bounded symbol-evidence multiplier blends in
            // owner/path/coverage signals, and a test/bench-looking path demotes it. Primary
            // BM25 still supplies the base candidate set; qualified-name supplemental hits can
            // add owner/exact candidates, then final results are truncated back to `limit`.
            let ranking_signal = scored_symbols.first().map(|scored| {
                let same_name_candidate_count = name_frequencies
                    .get(&scored.symbol.name.to_lowercase())
                    .copied()
                    .unwrap_or(1);
                SearchRankingSignal {
                    symbol_name: scored.symbol.name.clone(),
                    symbol_owner: scored.symbol.owner.clone(),
                    exact_name_hit: scored.exact_hit,
                    exact_boost_applied: scored.exact_boost_eligible,
                    matched_token_count: scored.term_match_count,
                    query_token_count: query_tokens.tokens().len(),
                    owner_match_count: scored.owner_match_count,
                    path_match_count: scored.path_match_count,
                    same_name_candidate_count,
                }
            });
            let exact_name_hit = scored_symbols
                .iter()
                .any(|scored| scored.exact_boost_eligible);
            // Qualified-name literal co-exposure: a file whose string literal IS a qualified
            // query word (`encoding::base64::decode`) but has no symbol-exact hit gets a bounded
            // lift so the legacy dispatch/lookup table surfaces beside the exec path. Applied ONLY
            // when there is no symbol-exact hit so it never stacks onto `EXACT_NAME_SCORE_BOOST`
            // (which would push a literal-only file above a real symbol match) — co-exposure, not
            // re-rank.
            // The field is computed unconditionally (drives the #2 literal label + #4 cross-path
            // note). The score lift below is held off via ENABLE_QUALIFIED_LITERAL_BOOST; keeping
            // this computation here means neutralizing the lift never strips the literal exposure.
            let qualified_literal_hit =
                qualified_literal_exact_hit(&candidate.literals, &query_tokens);
            let mut adjusted_score = candidate.raw_score;
            if exact_name_hit {
                adjusted_score *= EXACT_NAME_SCORE_BOOST;
            } else if ENABLE_QUALIFIED_LITERAL_BOOST && qualified_literal_hit.is_some() {
                // held pending Lever-B validation (run cms-perf-improve-20260615); field retained
                // for literal exposure (#2) + cross-path note (#4)
                adjusted_score *= QUALIFIED_LITERAL_SCORE_BOOST;
            }
            adjusted_score *= symbol_signal_multiplier(&scored_symbols);
            if is_test_like_path(&candidate.file_path) {
                adjusted_score *= TEST_PATH_SCORE_WEIGHT;
            }
            let mut matched_symbols: Vec<ExtractedSymbol> = scored_symbols
                .into_iter()
                .map(|scored| scored.symbol.clone())
                .collect();
            // The doc ranked in via some field (symbol/docstring/path). If the symbol
            // selection is empty (e.g. matched via docstring or path tokens), fall
            // back to the file's own symbols so the detail view never renders an empty
            // file header (Child 03 — OR/AND render consistency).
            let symbol_fallback = matched_symbols.is_empty();
            if symbol_fallback {
                matched_symbols = all_symbols;
            }

            // Matched-literal selection mirrors the symbol promotions: all-terms baseline,
            // plus an exact-value hit (a term that IS the whole literal, e.g. "8000") and
            // half-coverage for 3+ term queries (an error-message literal shouldn't be
            // vetoed by one glue word). Match decisions use `text`; `line` is carried
            // through for the detail view to render `[L<n>]`.
            let matched_literals: Vec<ExtractedLiteral> = candidate
                .literals
                .into_iter()
                .filter(|lit| {
                    if query_tokens.is_empty() {
                        return false;
                    }
                    let lit_lower = lit.text.to_lowercase();
                    let term_match_count = query_tokens
                        .tokens()
                        .iter()
                        .filter(|term| lit_lower.contains((*term).as_str()))
                        .count();
                    term_match_count == query_tokens.tokens().len()
                        || query_tokens.contains_word(&lit_lower)
                        || (query_tokens.tokens().len() >= 3
                            && term_match_count
                                >= partial_match_threshold(query_tokens.tokens().len()))
                })
                .collect();

            results.push(SearchResult {
                file_path: candidate.file_path,
                score: adjusted_score,
                total_lines: candidate.total_lines,
                matched_symbols,
                matched_literals,
                symbol_fallback,
                ranking_signal,
                qualified_literal_hit,
            });
        }

        // Re-sort by the adjusted scores (BM25 order only holds for the raw scores).
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.file_path.cmp(&b.file_path))
        });
        results.truncate(limit);

        Ok(results)
    }
}
