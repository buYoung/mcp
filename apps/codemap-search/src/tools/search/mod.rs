//! The `search` tool body: BM25 lookup over the committed index snapshot, hybrid
//! detail/tail rendering, and depth-1 caller/callee annotation. Orchestration only — the
//! snippet/tier/anchoring helpers live in [`render`].
//!
//! The dispatch arm (`crate::mcp`) calls `EngineSupervisor::ensure_alive`/`trigger_refresh`
//! before delegating here; this body only reads the committed snapshot through `ctx.engine`,
//! so it never needs `&mut` access to the engine.

pub mod render;

use crate::tools::ToolContext;

/// Run the `search` tool and return the rendered detail/tail text. The MCP dispatch arm wraps
/// the returned string in the JSON-RPC `content` envelope.
pub fn run(ctx: &ToolContext) -> Result<String, (i64, String)> {
    let query = ctx.arguments
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "Missing query parameter".to_string()))?;

    // Caller/callee context (default on). Precedence: the per-call
    // parameter, when present, always wins (an explicit `false`
    // overrides the default); the repo-level config key only decides
    // the default when the parameter is omitted.
    let caller_context_enabled = ctx.arguments
        .get("caller_context")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| crate::config::get().caller_context_default);

    let results = ctx.engine
        .search(query, 100)
        .map_err(|e| (-32603, format!("Search error: {}", e)))?;

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
    // Hybrid rendering: the top-ranked files (BM25 order) always get the
    // full detail view — snippets plus call context — and every match
    // beyond the threshold is appended as a compact, ranked one-line
    // list. A broad multi-word query thus still answers with usable
    // detail for its strongest hits instead of a wall of file headers
    // that forces a follow-up grep (the dominant agent-observed failure
    // mode of the old overview-only branch).
    let detail_results =
        &results[..results.len().min(result_branch_threshold)];
    let remaining_results = &results[detail_results.len()..];
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
            let requests: Vec<crate::callers::AnnotationRequest<'_>> =
                detail_results
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
            };
            let available = byte_cap.saturating_sub(text.len());
            let root = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            crate::callers::annotate_results(
                &requests,
                &snapshot,
                &caller_cfg,
                available,
                &root,
            )
        } else {
            None
        };

        // Two query sets (P1, 2-tier anchoring). Tier-1: whole-identifier
        // query words for exact NAME equality (strict — the primary anchor).
        // Tier-2: sub-token set from the SAME splitter the index uses (loose
        // — owner/sub-token match, used only when a file has no Tier-1 hit).
        let query_word_set = render::query_words(query);
        let query_token_set = render::query_tokens(query);

        let mut budget_hit = false;
        'files: for res in detail_results {
            if text.len() >= byte_cap {
                budget_hit = true;
                break;
            }
            text.push_str(&format!(
                "### File: {} ({} lines)\n",
                res.file_path, res.total_lines
            ));

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
                let mut matched_in_fallback: Vec<&crate::parser::ExtractedSymbol> =
                    res.matched_symbols
                        .iter()
                        .filter(|s| {
                            render::symbol_is_tier1(s, &query_word_set)
                        })
                        .collect();
                if matched_in_fallback.len() < FALLBACK_SNIPPET_CAP {
                    for sym in res.matched_symbols.iter().filter(|s| {
                        !render::symbol_is_tier1(s, &query_word_set)
                            && render::symbol_matches_query(s, &query_token_set)
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
                let mut snippet_starts: std::collections::HashSet<usize> =
                    matched_in_fallback
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
                    &query_word_set,
                    &query_token_set,
                    &render_caps,
                    caller_annotations.as_ref(),
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
                        sym.name,
                        sym.kind,
                        sym.range.start_line,
                        sym.range.end_line
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
                let skipped_for_cap = res
                    .matched_symbols
                    .len()
                    .saturating_sub(symbol_limit);
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
                    &query_word_set,
                    &query_token_set,
                    &render_caps,
                    caller_annotations.as_ref(),
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
            text.push_str(
                "\n_Detail view truncated at the output budget; refine the query or use overview/read to inspect specific files._\n",
            );
        }
    }

    // Ranked tail: one line per remaining match, strongest first, with
    // up to 3 matched symbols inline. The index's matched-symbol
    // selection (exact-name/partial promotions included, strongest
    // first) is reused as-is; a fallback entry (path/docstring-only
    // match) shows a bare header instead of leaking unrelated symbols.
    // Count-capped by `search_overview_file_limit` and byte-capped like
    // the detail view.
    if !remaining_results.is_empty() {
        let tail_cfg = crate::config::get();
        let tail_file_limit = tail_cfg.search_overview_file_limit;
        let tail_byte_cap = tail_cfg.search_detail_byte_cap;
        text.push_str(&format!(
            "\n## Other matches — {} more files, ranked by relevance\n",
            remaining_results.len()
        ));
        let mut shown_tail = 0usize;
        for res in remaining_results.iter().take(tail_file_limit) {
            if text.len() >= tail_byte_cap {
                break;
            }
            let symbol_notes: Vec<String> = if res.symbol_fallback {
                Vec::new()
            } else {
                res.matched_symbols
                    .iter()
                    .take(3)
                    .map(|sym| {
                        format!(
                            "{} [L{}-{}]",
                            sym.name,
                            sym.range.start_line,
                            sym.range.end_line
                        )
                    })
                    .collect()
            };
            if symbol_notes.is_empty() {
                text.push_str(&format!(
                    "- {} ({} lines)\n",
                    res.file_path, res.total_lines
                ));
            } else {
                text.push_str(&format!(
                    "- {} ({} lines) — {}\n",
                    res.file_path,
                    res.total_lines,
                    symbol_notes.join(", ")
                ));
            }
            shown_tail += 1;
        }
        if remaining_results.len() > shown_tail {
            text.push_str(&format!(
                "- _… {} more files not shown; refine the query to narrow._\n",
                remaining_results.len() - shown_tail
            ));
        }
    }

    Ok(text)
}
