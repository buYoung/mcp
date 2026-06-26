//! The `search` tool body: BM25 lookup over the committed index snapshot, hybrid
//! detail/tail rendering, and depth-1 caller/callee annotation. Orchestration only — the
//! snippet/tier/anchoring helpers live in [`render`].
//!
//! The dispatch arm (`crate::mcp`) calls `EngineSupervisor::ensure_alive`/`trigger_refresh`
//! before delegating here; this body only reads the committed snapshot through `ctx.engine`,
//! so it never needs `&mut` access to the engine.

mod monorepo;
pub mod render;

use crate::tools::ToolContext;

const SEARCH_CAP_FOOTER: &str = "\n_Partial search output: reached `search_detail_byte_cap`. Continue by narrowing the query or reading the listed file ranges with `read`._\n";
pub(crate) const DEFAULT_SEARCH_LIMIT: usize = 100;

fn truncate_to_char_boundary(text: &mut String, max_len: usize) {
    if text.len() <= max_len {
        return;
    }
    let mut end = max_len;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
}

fn has_open_markdown_fence(text: &str) -> bool {
    text.lines().filter(|line| line.trim() == "```").count() % 2 == 1
}

fn finish_search_output(mut text: String, byte_cap: usize, is_partial: bool) -> String {
    if !is_partial && text.len() <= byte_cap {
        return text;
    }
    if byte_cap == 0 {
        return String::new();
    }
    if SEARCH_CAP_FOOTER.len() >= byte_cap {
        let mut footer = SEARCH_CAP_FOOTER.to_string();
        truncate_to_char_boundary(&mut footer, byte_cap);
        return footer;
    }

    let mut fence_close = "";
    loop {
        let reserve = SEARCH_CAP_FOOTER.len() + fence_close.len();
        let allowed = byte_cap.saturating_sub(reserve);
        truncate_to_char_boundary(&mut text, allowed);
        let next_fence_close = if has_open_markdown_fence(&text) {
            "\n```\n"
        } else {
            ""
        };
        if next_fence_close == fence_close {
            break;
        }
        fence_close = next_fence_close;
    }

    format!("{text}{fence_close}{SEARCH_CAP_FOOTER}")
}

fn append_preserved_partial_note(text: &mut String, byte_cap: usize, note: &str) {
    if byte_cap <= SEARCH_CAP_FOOTER.len() {
        return;
    }

    let mut note = note.to_string();
    let note_room = byte_cap - SEARCH_CAP_FOOTER.len();
    if note.len() >= note_room {
        text.clear();
        truncate_to_char_boundary(&mut note, note_room);
        text.push_str(&note);
        return;
    }

    let mut fence_close = "";
    loop {
        let reserve = SEARCH_CAP_FOOTER.len() + fence_close.len() + note.len();
        let allowed = byte_cap.saturating_sub(reserve);
        truncate_to_char_boundary(text, allowed);
        let next_fence_close = if has_open_markdown_fence(text) {
            "\n```\n"
        } else {
            ""
        };
        if next_fence_close == fence_close {
            break;
        }
        fence_close = next_fence_close;
    }

    text.push_str(fence_close);
    text.push_str(&note);
}

fn tail_omission_note(omitted_count: usize) -> String {
    format!("{omitted_count} files omitted; narrow the query.\n")
}

fn compact_symbol_names(symbols: &[crate::parser::ExtractedSymbol], limit: usize) -> String {
    let mut names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    let shown: Vec<&str> = names.iter().take(limit).copied().collect();
    let mut text = shown.join(", ");
    if names.len() > shown.len() {
        text.push_str(&format!(", +{} more", names.len() - shown.len()));
    }
    text
}

fn signal_symbol_label(signal: &crate::index::SearchRankingSignal) -> String {
    signal
        .symbol_owner
        .as_deref()
        .filter(|owner| !owner.is_empty())
        .map(|owner| format!("{owner}.{}", signal.symbol_name))
        .unwrap_or_else(|| signal.symbol_name.clone())
}

/// Compact `match_reason` label (Child 05 byte-flatten): a short enum-style phrase, not a verbose
/// sentence. When the file ranked in on a qualified-name string literal (`encoding::base64::decode`),
/// that literal is named verbatim — it is the discriminative reason this file matched, and surfacing
/// it stops a legacy dispatch table from being mislabeled by a weak module symbol.
fn match_reason(res: &crate::index::SearchResult) -> String {
    if let Some(literal) = &res.qualified_literal_hit {
        return format!("matched literal: `{literal}`");
    }
    if let Some(signal) = &res.ranking_signal {
        let symbol_label = signal_symbol_label(signal);
        if signal.exact_boost_applied {
            if signal.owner_match_count > 0 {
                return format!("owner-exact `{symbol_label}`");
            }
            if signal.path_match_count > 0 {
                return format!("path-exact `{symbol_label}`");
            }
            return format!("exact `{symbol_label}`");
        }
        if signal.exact_name_hit {
            return format!("exact(unboosted) `{symbol_label}`");
        }
        if signal.owner_match_count > 0 {
            return format!(
                "owner `{symbol_label}` ({}/{})",
                signal.matched_token_count, signal.query_token_count
            );
        }
        return format!(
            "token {}/{} `{symbol_label}`",
            signal.matched_token_count, signal.query_token_count
        );
    }
    if !res.symbol_fallback {
        return format!("symbol: {}", compact_symbol_names(&res.matched_symbols, 3));
    }
    if !res.matched_literals.is_empty() {
        return format!("literal/path/docstring ({})", res.matched_literals.len());
    }
    "path/docstring".to_string()
}

/// The last `::`/`.`-separated segment of a qualified name (`encoding::base64::decode` → `decode`),
/// lowercased. The segment a dispatch/lookup literal shares with the symbol that implements it, so
/// the cross-path scan can match the legacy literal to its exec definition site.
fn qualified_name_leaf(qualified: &str) -> String {
    qualified
        .rsplit([':', '.'])
        .find(|seg| !seg.is_empty())
        .unwrap_or(qualified)
        .to_lowercase()
}

/// Cross-path presence of qualified names across the whole result set (Child 05 repair): for each
/// qualified literal that ranked in, how many distinct files reference it as a string literal
/// (legacy dispatch/lookup) vs define its leaf as a symbol (exec implementation). When both sides
/// are non-empty, the same qualified name is handled in more than one place — the path-aware signal
/// that replaces the old same-name-symbol-count noise, telling a weak model to check both routes.
struct CrossPathPresence {
    /// qualified literal → (count of files matching it as a literal, count defining its leaf as a symbol).
    counts: std::collections::HashMap<String, (usize, usize)>,
}

impl CrossPathPresence {
    fn build(results: &[crate::index::SearchResult]) -> Self {
        // Distinct qualified literals that ranked in anywhere.
        let qualified_literals: std::collections::HashSet<String> = results
            .iter()
            .filter_map(|res| res.qualified_literal_hit.clone())
            .collect();
        let mut counts = std::collections::HashMap::new();
        for literal in qualified_literals {
            let leaf = qualified_name_leaf(&literal);
            let mut literal_paths = std::collections::HashSet::new();
            let mut symbol_paths = std::collections::HashSet::new();
            for res in results {
                if res.qualified_literal_hit.as_deref() == Some(literal.as_str()) {
                    literal_paths.insert(res.file_path.as_str());
                }
                let defines_leaf = res.matched_symbols.iter().any(|sym| {
                    sym.name.to_lowercase() == leaf
                        || crate::parser::split_identifier(&sym.name)
                            .last()
                            .is_some_and(|seg| *seg == leaf)
                });
                if defines_leaf {
                    symbol_paths.insert(res.file_path.as_str());
                }
            }
            counts.insert(literal, (literal_paths.len(), symbol_paths.len()));
        }
        Self { counts }
    }

    /// The cross-path note for one result, if its qualified literal is handled in both a literal
    /// (dispatch) site and a symbol (implementation) site.
    fn note_for(&self, res: &crate::index::SearchResult) -> Option<String> {
        let literal = res.qualified_literal_hit.as_ref()?;
        let (literal_count, symbol_count) = self.counts.get(literal).copied()?;
        if literal_count > 0 && symbol_count > 0 {
            let paths = literal_count + symbol_count;
            return Some(format!(
                "`{literal}` 이름은 {paths}개 경로에 존재 (구현 심볼 {symbol_count} + dispatch 리터럴 {literal_count}) — 둘 다 확인",
            ));
        }
        None
    }
}

fn ambiguity_note(
    res: &crate::index::SearchResult,
    cross_path: &CrossPathPresence,
) -> Option<String> {
    // Path-aware cross-path signal first: when this file's qualified literal is also implemented
    // as a symbol elsewhere in the results, that multi-route fact beats the same-name count.
    if let Some(note) = cross_path.note_for(res) {
        return Some(note);
    }
    if let Some(signal) = &res.ranking_signal {
        if signal.same_name_candidate_count > 1 {
            return Some(format!(
                "{} same-name `{}` symbols in ranked candidates",
                signal.same_name_candidate_count, signal.symbol_name
            ));
        }
    }
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for symbol in &res.matched_symbols {
        *counts.entry(symbol.name.as_str()).or_default() += 1;
    }
    let ambiguous: Vec<String> = counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .take(3)
        .map(|(name, count)| format!("{count} `{name}` symbols"))
        .collect();
    (!ambiguous.is_empty()).then(|| ambiguous.join(", "))
}

/// Compact read-suggestion (Child 05 byte-flatten): a short `read <path>:<offset> (<limit> lines)`
/// form instead of the full JSON argument object. Shortened, not removed — codex may still parse
/// the path:line hint to drive a targeted `read` (offset/limit are recoverable from the form).
fn format_read_suggestion(file_path: &str, offset: usize, limit: usize) -> String {
    format!("read {file_path}:{offset} ({limit} lines)")
}

fn read_suggestion(res: &crate::index::SearchResult) -> Option<String> {
    if !res.symbol_fallback {
        return res.matched_symbols.first().map(|symbol| {
            let limit = symbol
                .range
                .end_line
                .saturating_sub(symbol.range.start_line)
                .saturating_add(1);
            format_read_suggestion(&res.file_path, symbol.range.start_line, limit)
        });
    }
    res.matched_literals
        .first()
        .map(|literal| format_read_suggestion(&res.file_path, literal.line, 1))
}

/// The parent directory of a workspace-relative path (`a/b/c.rs` → `a/b`), or `""` for a
/// top-level file. The diversity unit for the detail-head reorder.
fn parent_dir(file_path: &str) -> &str {
    file_path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Minimum score ratio (displacing candidate ÷ same-dir candidate) for the diversity reorder to
/// defer a same-dir candidate in favor of a different-dir one. Below this gap the same-dir file is
/// clearly stronger and keeps its slot, so a much weaker different-dir file never displaces a
/// genuinely top match (the plan's "yield only when the score gap is small" guard).
const DIVERSITY_MIN_SCORE_RATIO: f32 = 0.6;

/// Soft directory diversity for the detail head: stop one directory from monopolizing the top
/// `head_len` detail slots. Walks results in score order and greedily fills the head, but defers a
/// candidate once its parent directory already holds `per_dir_cap` head slots — pulling the next
/// different-directory candidate forward — UNLESS no diverse candidate remains OR the best
/// remaining diverse candidate is much weaker (its score is below `DIVERSITY_MIN_SCORE_RATIO`× this
/// candidate's score), in which case the stronger same-dir file keeps its slot. Deferred same-dir
/// candidates keep their relative order and are appended right after, so nothing is dropped and the
/// tail still sees them ranked. Score order is otherwise preserved; this only reorders, never
/// re-scores (Lever B path-diversity aid; conservative so a genuinely single-directory answer is
/// barely perturbed). Returns indices into `results` in the new order.
fn diversified_order(
    results: &[crate::index::SearchResult],
    head_len: usize,
    per_dir_cap: usize,
) -> Vec<usize> {
    if results.len() <= 1 || head_len == 0 {
        return (0..results.len()).collect();
    }
    let mut head: Vec<usize> = Vec::with_capacity(head_len);
    let mut deferred: Vec<usize> = Vec::new();
    let mut dir_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (index, res) in results.iter().enumerate() {
        if head.len() >= head_len {
            // Head is full; everything else keeps score order in the tail.
            deferred.push(index);
            continue;
        }
        let dir = parent_dir(&res.file_path);
        let dir_count = dir_counts.get(dir).copied().unwrap_or(0);
        // Remaining head capacity after this candidate would be placed.
        let head_room_left = head_len - head.len();
        // The strongest different-dir candidate still ahead (results are score-sorted, so the first
        // such one is the strongest). Defer only when it both exists in enough number to fill the
        // remaining room AND is not much weaker than this candidate — otherwise displacing the
        // stronger same-dir file would trade a real top match for a noticeably weaker one.
        let diverse_ahead: Vec<&crate::index::SearchResult> = results[index + 1..]
            .iter()
            .filter(|other| parent_dir(&other.file_path) != dir)
            .collect();
        let strongest_diverse_score = diverse_ahead.first().map(|other| other.score);
        let score_gap_small = match strongest_diverse_score {
            Some(diverse_score) if res.score > 0.0 => {
                diverse_score >= res.score * DIVERSITY_MIN_SCORE_RATIO
            }
            // A non-positive base score can't anchor a ratio; treat any diverse candidate as close.
            Some(_) => true,
            None => false,
        };
        if dir_count >= per_dir_cap && diverse_ahead.len() >= head_room_left && score_gap_small {
            deferred.push(index);
            continue;
        }
        head.push(index);
        *dir_counts.entry(dir).or_insert(0) += 1;
    }
    head.extend(deferred);
    head
}

/// Run the `search` tool and return the rendered detail/tail text. The MCP dispatch arm wraps
/// the returned string in the JSON-RPC `content` envelope.
pub fn run(ctx: &ToolContext) -> Result<String, (i64, String)> {
    if monorepo::should_use(ctx) {
        return monorepo::run(ctx);
    }
    run_inner(ctx, None, DEFAULT_SEARCH_LIMIT)
}

pub(crate) fn run_inner(
    ctx: &ToolContext,
    workspace_scope: Option<&str>,
    search_limit: usize,
) -> Result<String, (i64, String)> {
    let query = ctx
        .arguments
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Missing query parameter".to_string()))?;

    // Caller/callee context (default on). Precedence: the per-call
    // parameter, when present, always wins (an explicit `false`
    // overrides the default); the repo-level config key only decides
    // the default when the parameter is omitted.
    let caller_context_enabled = ctx
        .arguments
        .get("caller_context")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| crate::config::get().caller_context_default);
    let search_context = crate::index::SearchQueryContext {
        language_hint: ctx
            .arguments
            .get("language_hint")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
        extension_hint: ctx
            .arguments
            .get("extension_hint")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
    };

    let mut results = ctx
        .engine
        .search_with_context(query, search_limit, &search_context)
        .map_err(|e| (-32603, format!("Search error: {}", e)))?;
    if let Some(scope) = workspace_scope {
        results.retain(|result| monorepo::result_is_under_scope(result, scope));
        results.truncate(DEFAULT_SEARCH_LIMIT);
    }

    // Result-branch threshold: at or below it, return file details;
    // above it, return a codemap overview. Config-driven (Child 05),
    // default 5.
    let result_branch_threshold = crate::config::get().result_threshold;

    let mut text = String::new();
    // While the initial background index builds, results can be empty or
    // partial — say so, and point at the always-live tools meanwhile.
    if ctx.engine.is_dead() {
        text.push_str(
            "_Background indexer stopped — search results are frozen at the last index and may be stale; restart the server to recover. read/find/grep stay live._\n\n",
        );
    } else if ctx.engine.is_warming() {
        text.push_str(
            "_Index is warming up (initial background indexing) — results may be empty or partial; retry shortly, or use grep/find for live results._\n\n",
        );
    } else if let Some(err) = ctx.engine.last_error() {
        text.push_str(&format!(
            "_Last background index refresh failed: {err} — results may be stale._\n\n"
        ));
    }
    if let Some(scope) = workspace_scope {
        text.push_str(&format!(
            "_Workspace scope: `{scope}`. Pass `workspace_scope: \"all\"` for repo-wide search._\n\n"
        ));
    }
    if results.is_empty() {
        if let Some(scope) = workspace_scope {
            text.push_str(&format!(
                "No indexed matches for `{query}` inside workspace scope `{scope}`."
            ));
        } else {
            text.push_str(&format!("No indexed matches for `{query}`."));
        }
        return Ok(text);
    }
    // Cross-path presence over the FULL result set (Child 05 repair, computed once): which
    // qualified names appear both as a dispatch/lookup literal and as an implementing symbol, so
    // the per-file ambiguity note can flag multi-route names path-aware.
    let cross_path = CrossPathPresence::build(&results);

    // Hybrid rendering: the top-ranked files (BM25 order) always get the
    // full detail view — snippets plus call context — and every match
    // beyond the threshold is appended as a compact, ranked one-line
    // list. A broad multi-word query thus still answers with usable
    // detail for its strongest hits instead of a wall of file headers
    // that forces a follow-up grep (the dominant agent-observed failure
    // mode of the old overview-only branch).
    //
    // Soft directory diversity: reorder so a single directory cluster can't monopolize the detail
    // head (Lever B). Conservative — score order is otherwise preserved and deferred same-dir files
    // fall straight into the tail, so nothing is dropped.
    const DETAIL_DIR_CAP: usize = 3;
    let ordered: Vec<&crate::index::SearchResult> =
        diversified_order(&results, result_branch_threshold, DETAIL_DIR_CAP)
            .into_iter()
            .map(|index| &results[index])
            .collect();
    let detail_results = &ordered[..ordered.len().min(result_branch_threshold)];
    let remaining_results = &ordered[detail_results.len()..];
    let mut output_was_capped = false;
    {
        // Detail view: enclosing code scopes for the pinpointed files,
        // bounded by config caps so a few large or fallback-matched files
        // can't dump the whole tree into the agent's context.
        let cfg = crate::config::get();
        let snippet_max_lines = cfg.search_detail_snippet_max_lines;
        let symbol_limit = cfg.search_detail_symbol_limit;
        let byte_cap = cfg.search_detail_byte_cap;
        let literal_max_len = cfg.search_literal_max_len;
        let literal_limit = cfg.search_literal_limit;

        // Caller/callee context (default on): one workspace scan across
        // every matched `fn` in the detail view, attributed off the codemap
        // snapshot + the Phase-A owner field. Any failure → `None` → the
        // detail view renders exactly as today (failure isolation). The
        // annotation byte budget is what is still free under `byte_cap`
        // at this point (snippets keep priority, two-counter inside).
        let caller_annotations = if caller_context_enabled {
            let snapshot = ctx.engine.codemap_snapshot();
            let requests: Vec<crate::callers::AnnotationRequest<'_>> = detail_results
                .iter()
                .map(|res| crate::callers::AnnotationRequest {
                    file_path: &res.file_path,
                    symbols: &res.matched_symbols,
                    is_fallback: res.symbol_fallback,
                })
                .collect();
            let caller_cfg = crate::callers::CallerConfig {
                scan_cap: cfg.scan_cap,
                caller_list_cap: cfg.caller_list_cap,
                callee_list_cap: cfg.callee_list_cap,
                annotation_sub_budget: cfg.annotation_sub_budget,
                common_name_threshold: cfg.common_name_threshold,
                caller_omit_def_threshold: cfg.caller_omit_def_threshold,
                max_file_size: cfg.max_file_size,
                navigation_context_default: cfg.navigation_context_default,
                navigation_callsite_budget: cfg.navigation_callsite_budget,
                navigation_store_references: cfg.navigation_store_references,
            };
            let runtime_state = crate::callers::AnnotationRuntimeState {
                is_warming: ctx.engine.is_warming(),
                has_refresh_error: ctx.engine.last_error().is_some(),
                is_dead_or_stale: ctx.engine.is_dead() || ctx.engine.last_error().is_some(),
            };
            let available = byte_cap.saturating_sub(text.len());
            let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            crate::callers::annotate_results_with_state(
                &requests,
                &snapshot,
                &caller_cfg,
                available,
                &root,
                runtime_state,
            )
        } else {
            None
        };

        // Shared query model (P1, 2-tier anchoring). Tier-1 uses whole query
        // words for exact NAME equality; Tier-2 uses the same sub-token set
        // ranking consumes, so rendering and ranking interpret punctuation
        // and identifier boundaries identically.
        let query_tokens = crate::parser::QueryTokens::parse(query);

        let detail_result_count = detail_results.len();
        let mut rendered_detail_count = 0usize;
        let mut budget_hit = false;
        // Cross-file caller-block dedup (Child 05 / over-match repair): owned ACROSS the whole
        // detail loop, not per file, so a repeated caller list spanning file boundaries collapses
        // to a "same as `name` above" back-reference instead of re-printing. Threaded into every
        // file's `render_anchored_symbols` call.
        let mut caller_block_dedup = crate::callers::CallerBlockDedup::new();
        'files: for &res in detail_results {
            if text.len() >= byte_cap {
                budget_hit = true;
                break;
            }
            rendered_detail_count += 1;
            text.push_str(&format!(
                "### File: {} ({} lines)\n",
                res.file_path, res.total_lines
            ));
            // Hints, compressed (Child 05 byte-flatten): match_reason and the cross-path/ambiguity
            // note share one line; the read suggestion is its own short line.
            let mut hint_line = format!("- match: {}", match_reason(res));
            if let Some(ambiguity) = ambiguity_note(res, &cross_path) {
                hint_line.push_str(&format!("; {ambiguity}"));
            }
            let mut hints = format!("{hint_line}\n");
            if let Some(suggestion) = read_suggestion(res) {
                hints.push_str(&format!("- {suggestion}\n"));
            }
            if text.len() + hints.len() + SEARCH_CAP_FOOTER.len() < byte_cap {
                text.push_str(&hints);
            }

            if res.symbol_fallback {
                // Matched via docstring/path, not a symbol name — render
                // symbol names + ranges ONLY (no snippets) for the bulk so
                // we never `cat` the file. Still count-capped.
                //
                // P1 §6 exception: even in this list-only fallback, if some
                // of the file's symbols DO match the query, render the
                // matching ones in detail first — the agent ranked this file
                // in and a matching symbol here is the likely target — then
                // the count-capped name list for the rest. P1 §4 priority:
                // Tier-1 (exact-name) symbols are taken FIRST, then Tier-2
                // (sub-token/owner) symbols fill the remaining slots, so an
                // exactly-named symbol is never crowded out of the cap by a
                // loose token match — Tier-1 → Tier-2 only, never a
                // non-matching filler.
                //
                // P2-loop-2 C1/C3 unification: the selected matching symbols
                // go through the SAME shared anchoring/render path as the
                // name-matched branch — full snippets only for promoted
                // anchors (`search_anchor_snippet_limit`, Tier-1 first), a
                // 2-line summary for a container enclosing an anchor member
                // (e.g. `Signal` around its `send`), and a ≤3-line signature
                // for over-cap / Tier-2 demotions — instead of the prior
                // loop-1 "5 full snippets, no caps" rule that flooded a large
                // fallback file. With zero matching symbols the render set is
                // empty and the behavior is unchanged (P1 §5): pure
                // path/docstring/literal hits do not regress.
                const FALLBACK_SNIPPET_CAP: usize = 5;
                let mut matched_in_fallback: Vec<&crate::parser::ExtractedSymbol> = res
                    .matched_symbols
                    .iter()
                    .filter(|s| render::symbol_is_tier1(s, &query_tokens))
                    .collect();
                if matched_in_fallback.len() < FALLBACK_SNIPPET_CAP {
                    for sym in res.matched_symbols.iter().filter(|s| {
                        !render::symbol_is_tier1(s, &query_tokens)
                            && render::symbol_matches_query(s, &query_tokens)
                    }) {
                        if matched_in_fallback.len() >= FALLBACK_SNIPPET_CAP {
                            break;
                        }
                        matched_in_fallback.push(sym);
                    }
                }
                matched_in_fallback.truncate(FALLBACK_SNIPPET_CAP);
                // Every symbol handed to the render path is "shown in detail"
                // and must be excluded from the residual name list — even one
                // the path deduped (a member fully inside an already-shown
                // range) or dropped at the byte cap — so a matching symbol is
                // never both rendered AND re-listed below.
                let mut snippet_starts: std::collections::HashSet<usize> = matched_in_fallback
                    .iter()
                    .map(|s| s.range.start_line)
                    .collect();
                let render_caps = render::AnchoredRenderCaps {
                    snippet_max_lines,
                    anchor_snippet_limit: cfg.search_anchor_snippet_limit,
                    byte_cap,
                };
                let outcome = render::render_anchored_symbols(
                    &mut text,
                    &res.file_path,
                    matched_in_fallback,
                    &query_tokens,
                    &render_caps,
                    caller_annotations.as_ref(),
                    &mut caller_block_dedup,
                );
                snippet_starts.extend(outcome.emitted_starts);
                if outcome.budget_hit {
                    budget_hit = true;
                    break 'files;
                }
                // Name list for the remaining symbols (those not already
                // shown in detail), count-capped as before.
                let mut listed = 0usize;
                for sym in res.matched_symbols.iter() {
                    if snippet_starts.contains(&sym.range.start_line) {
                        continue;
                    }
                    if listed >= symbol_limit {
                        break;
                    }
                    text.push_str(&format!(
                        "- Symbol: {} ({}) [L{}-{}]\n",
                        sym.name, sym.kind, sym.range.start_line, sym.range.end_line
                    ));
                    listed += 1;
                }
                let remaining = res
                    .matched_symbols
                    .len()
                    .saturating_sub(snippet_starts.len() + listed);
                if remaining > 0 {
                    text.push_str(&format!(
                        "- _… {remaining} more symbols not shown; use overview/read to inspect._\n"
                    ));
                }
            } else {
                // Name-matched file: emit capped snippets via the shared
                // anchoring/render path (P1 2-tier + P2-loop-2 C1/C3). The
                // symbol cap is applied on the SELECTION order (strongest
                // matches first — exact-name hits lead) BEFORE the function's
                // internal range sort, so a promoted symbol deep in a large
                // file is never cut in favor of earlier-but-weaker matches,
                // and the function's promoted-anchor pick stays rank-ordered.
                let skipped_for_cap = res.matched_symbols.len().saturating_sub(symbol_limit);
                let symbols: Vec<&crate::parser::ExtractedSymbol> =
                    res.matched_symbols.iter().take(symbol_limit).collect();
                let render_caps = render::AnchoredRenderCaps {
                    snippet_max_lines,
                    anchor_snippet_limit: cfg.search_anchor_snippet_limit,
                    byte_cap,
                };
                let outcome = render::render_anchored_symbols(
                    &mut text,
                    &res.file_path,
                    symbols,
                    &query_tokens,
                    &render_caps,
                    caller_annotations.as_ref(),
                    &mut caller_block_dedup,
                );
                if outcome.budget_hit {
                    budget_hit = true;
                    break 'files;
                }
                if skipped_for_cap > 0 {
                    text.push_str(&format!(
                        "- _… {skipped_for_cap} more symbols not shown; use overview/read to inspect._\n"
                    ));
                }
            }

            // Literals: length-truncated and count-capped.
            for lit in res.matched_literals.iter().take(literal_limit) {
                text.push_str(&format!(
                    "- Literal: {:?} [L{}]\n",
                    render::truncate_literal(&lit.text, literal_max_len),
                    lit.line
                ));
            }
            if res.matched_literals.len() > literal_limit {
                text.push_str(&format!(
                    "- _… {} more literals not shown._\n",
                    res.matched_literals.len() - literal_limit
                ));
            }
        }
        if budget_hit {
            output_was_capped = true;
        }
        if output_was_capped {
            let omitted_detail_count = detail_result_count.saturating_sub(rendered_detail_count);
            if omitted_detail_count > 0 || !remaining_results.is_empty() {
                let note = tail_omission_note(omitted_detail_count + remaining_results.len());
                append_preserved_partial_note(&mut text, byte_cap, &note);
            }
        }
    }

    // Ranked tail: one line per remaining match, strongest first, with
    // up to 3 matched symbols inline. The index's matched-symbol
    // selection (exact-name/partial promotions included, strongest
    // first) is reused as-is; a fallback entry (path/docstring-only
    // match) shows a bare header instead of leaking unrelated symbols.
    // Count-capped by `search_overview_file_limit` and byte-capped like
    // the detail view.
    let byte_cap = crate::config::get().search_detail_byte_cap;
    if !output_was_capped && !remaining_results.is_empty() {
        let tail_cfg = crate::config::get();
        let tail_file_limit = tail_cfg.search_overview_file_limit;
        let mut tail = format!(
            "\n## Other matches — {} more files, ranked by relevance\n",
            remaining_results.len()
        );
        let mut shown_tail = 0usize;
        for res in remaining_results.iter().take(tail_file_limit) {
            // The matched qualified literal leads the tail note (#2): it is the discriminative
            // reason a legacy dispatch/lookup file ranked in, and surfacing it in BOTH the
            // non-fallback (symbol-notes) and the bare-path (fallback) sub-branches fixes the
            // `encoding [L20]` mislabel — the file reads as its dispatch table, not as a plain mod.
            let mut notes: Vec<String> = Vec::new();
            if let Some(literal) = &res.qualified_literal_hit {
                notes.push(format!("matched literal: `{literal}`"));
            }
            if !res.symbol_fallback {
                notes.extend(res.matched_symbols.iter().take(3).map(|sym| {
                    format!(
                        "{} [L{}-{}]",
                        sym.name, sym.range.start_line, sym.range.end_line
                    )
                }));
            }
            if notes.is_empty() {
                tail.push_str(&format!(
                    "- {} ({} lines)\n",
                    res.file_path, res.total_lines
                ));
            } else {
                tail.push_str(&format!(
                    "- {} ({} lines) — {}\n",
                    res.file_path,
                    res.total_lines,
                    notes.join(", ")
                ));
            }
            shown_tail += 1;
        }
        if remaining_results.len() > shown_tail {
            if shown_tail == 0 {
                tail.push_str(&format!(
                    "- _Showing tail files 0 of {}; {} files not shown. Continue by narrowing the query._\n",
                    remaining_results.len(),
                    remaining_results.len()
                ));
            } else {
                tail.push_str(&format!(
                    "- _Showing tail files 1-{shown_tail} of {}; {} more files not shown. Continue by narrowing the query._\n",
                    remaining_results.len(),
                    remaining_results.len() - shown_tail
                ));
            }
        }
        if text.len() + tail.len() <= byte_cap {
            text.push_str(&tail);
        } else {
            let note = tail_omission_note(remaining_results.len());
            append_preserved_partial_note(&mut text, byte_cap, &note);
            output_was_capped = true;
        }
    }

    let is_partial = output_was_capped || text.len() > byte_cap;
    Ok(finish_search_output(text, byte_cap, is_partial))
}
