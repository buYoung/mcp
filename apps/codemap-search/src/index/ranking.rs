use super::{SearchResult, SearcherHandle};
use crate::parser::{ExtractedFile, ExtractedLiteral, ExtractedSymbol};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::TantivyDocument;

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

/// A name specific enough that exact equality with a query term means intent: multi-token
/// identifiers (snake/camel compounds) or long single tokens. Short single-word names
/// ("new", "write", "Error") are too generic to treat as a definition request.
fn is_discriminative_name(name: &str) -> bool {
    name.len() >= 8 || crate::parser::split_identifier(name).len() >= 2
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

        let query = match parse_query_catching_panic(|| query_parser.parse_query(query_str)) {
            Some(Ok(q)) => q,
            // Primary parse failed or panicked (e.g. a bare `*` in tantivy 0.26):
            // strip special characters to spaces and retry as a plain term query.
            _ => {
                let escaped: String = query_str
                    .to_lowercase()
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c.is_whitespace() {
                            c
                        } else {
                            ' '
                        }
                    })
                    .collect();
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

        let mut results = Vec::new();
        let query_lower = query_str.to_lowercase();

        for (score, doc_address) in top_docs {
            let doc = searcher
                .doc::<TantivyDocument>(doc_address)
                .map_err(|e| e.to_string())?;

            let file_path = doc
                .get_first(self.file_path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let extracted_json = doc
                .get_first(self.extracted_json_field)
                .and_then(|v| v.as_str())
                .unwrap_or("{}");

            let extracted_file: ExtractedFile = serde_json::from_str(extracted_json)
                .unwrap_or_else(|_| ExtractedFile {
                    file_path: file_path.clone(),
                    total_lines: 0,
                    symbols: Vec::new(),
                    literals: Vec::new(),
                    docstrings: Vec::new(),
                });

            let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

            // Capture the file's total line count before the partial moves of
            // `.symbols`/`.literals` below borrow `extracted_file` apart.
            let total_lines = extracted_file.total_lines;
            let all_symbols = extracted_file.symbols;

            // Matched-symbol selection. All-terms is the precision baseline, but agent
            // queries carry glue words ("definition", "handler") no symbol can match,
            // which used to classify nearly every multi-word query as fallback — killing
            // snippets and caller/callee annotations. Two promotions relax it:
            //  - exact name: a query term that IS a discriminative symbol name (the same
            //    signal as EXACT_NAME_SCORE_BOOST) marks that symbol matched outright;
            //  - partial coverage: in a 3+ term query, a symbol matching at least half
            //    the terms is matched (glue words no longer veto everything).
            // Selection is ordered exact-first then by matched-term count, so the
            // renderer's symbol cap keeps the strongest evidence instead of line order.
            let mut scored_symbols: Vec<(bool, usize, &ExtractedSymbol)> = all_symbols
                .iter()
                .filter_map(|sym| {
                    if query_terms.is_empty() {
                        return None;
                    }
                    let term_match_count = query_terms
                        .iter()
                        .filter(|&&term| symbol_matches_term(sym, term))
                        .count();
                    // Exact-name intent fires two ways: a query term equals the whole
                    // symbol name, OR the query spells a multi-token name's sub-tokens as
                    // separate terms ("default port" / "inspect default port" → DEFAULT_PORT,
                    // which plain BM25 buries in an 8k-line file under port-heavy and generic
                    // `default()` files). The subset form requires >=2 sub-tokens (so a
                    // single-token glue name like `default` can't qualify) and every sub-token
                    // to appear among the query terms — extra glue terms in the query (e.g.
                    // "inspect") don't block it. Backlog #1: align query tokenization with the
                    // index-side symbol sub-tokens.
                    let name_subtokens = crate::parser::split_identifier(&sym.name);
                    let exact_hit = is_discriminative_name(&sym.name)
                        && (query_terms.iter().any(|&t| t == sym.name.to_lowercase())
                            || (name_subtokens.len() >= 2
                                && name_subtokens.iter().all(|st| {
                                    let st = st.to_lowercase();
                                    query_terms.iter().any(|&t| t == st)
                                })));
                    let all_terms_hit = term_match_count == query_terms.len();
                    // Partial coverage additionally requires NAME evidence (at least one
                    // term hitting the symbol name itself) — see `term_hits_symbol_name`.
                    let partial_hit = query_terms.len() >= 3
                        && term_match_count >= partial_match_threshold(query_terms.len())
                        && query_terms.iter().any(|&t| term_hits_symbol_name(sym, t));
                    (exact_hit || all_terms_hit || partial_hit)
                        .then_some((exact_hit, term_match_count, sym))
                })
                .collect();
            scored_symbols.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then(b.1.cmp(&a.1))
                    .then(a.2.range.start_line.cmp(&b.2.range.start_line))
            });

            // Post-rank adjustment (see the constants above): an exact discriminative
            // symbol-name hit boosts the file, a test/bench-looking path demotes it. Both
            // re-rank only within the BM25 top `limit` — the candidate set is unchanged.
            let exact_name_hit = scored_symbols.iter().any(|(exact, _, _)| *exact);
            let mut adjusted_score = score;
            if exact_name_hit {
                adjusted_score *= EXACT_NAME_SCORE_BOOST;
            }
            if is_test_like_path(&file_path) {
                adjusted_score *= TEST_PATH_SCORE_WEIGHT;
            }
            let mut matched_symbols: Vec<ExtractedSymbol> = scored_symbols
                .into_iter()
                .map(|(_, _, sym)| sym.clone())
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
            let matched_literals: Vec<ExtractedLiteral> = extracted_file
                .literals
                .into_iter()
                .filter(|lit| {
                    if query_terms.is_empty() {
                        return false;
                    }
                    let lit_lower = lit.text.to_lowercase();
                    let term_match_count = query_terms
                        .iter()
                        .filter(|&&term| lit_lower.contains(term))
                        .count();
                    term_match_count == query_terms.len()
                        || query_terms.iter().any(|&t| t == lit_lower)
                        || (query_terms.len() >= 3
                            && term_match_count >= partial_match_threshold(query_terms.len()))
                })
                .collect();

            results.push(SearchResult {
                file_path,
                score: adjusted_score,
                total_lines,
                matched_symbols,
                matched_literals,
                symbol_fallback,
            });
        }

        // Re-sort by the adjusted scores (BM25 order only holds for the raw scores).
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }
}
