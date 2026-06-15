//! Search-detail rendering helpers: snippet extraction, query tokenization, the P1 2-tier
//! anchoring rules, and the shared anchored-render path. These are pure functions over a
//! file's already-selected symbols and the parsed query — no `self`, no engine, no I/O
//! beyond reading the matched source files for snippet bodies.
//!
//! Caller-annotation dedup contract: [`render_anchored_symbols`] fulfils the
//! [`crate::callers::DetailAnnotations`] dedup protocol. For each emitted symbol it asks the
//! annotations for a [`crate::callers::PreparedAnnotation`] keyed on the current
//! [`crate::callers::CallerBlockDedup`] state (so a repeated caller block renders as a "same as
//! above" back-reference rather than the full list); only after the prepared block actually fits
//! the byte budget and is pushed does it call [`crate::callers::PreparedAnnotation::commit`] to
//! record those callers in the dedup set. The render → fits-check → emit → commit sequence MUST
//! stay in that order: committing before the fit check would suppress a later block that was
//! never actually emitted.

/// Cap a code snippet at `max_lines` AND `max_bytes`, appending an elision marker when
/// truncated, so the `search` detail view never emits a 1,000-line symbol body — nor a
/// single multi-hundred-KB minified line (the line cap alone leaves byte size unbounded).
fn cap_snippet(snippet: &str, max_lines: usize, max_bytes: usize) -> String {
    let lines: Vec<&str> = snippet.lines().collect();
    let mut out = if lines.len() > max_lines {
        let shown = lines[..max_lines].join("\n");
        format!("{shown}\n… ({} more lines)", lines.len() - max_lines)
    } else {
        snippet.to_string()
    };
    if max_bytes > 0 && out.len() > max_bytes {
        // Truncate at a UTF-8 char boundary, then mark the cut.
        let mut end = max_bytes.min(out.len());
        while end > 0 && !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        out.push_str("\n… (truncated)");
    }
    out
}

/// Truncate a matched literal to `max_len` characters with an ellipsis, so a long
/// SQL/template literal can't bloat the `search` detail view.
pub(super) fn truncate_literal(literal: &str, max_len: usize) -> String {
    if literal.chars().count() > max_len {
        let truncated: String = literal.chars().take(max_len).collect();
        format!("{truncated}…")
    } else {
        literal.to_string()
    }
}

/// Extract a symbol's source range with `read`-style line numbers (`␠␠␠␠␠1→content`).
/// Numbered so the agent can cite exact lines straight from the detail view instead of
/// re-reading the file to confirm them (the dominant post-discovery turn cost observed).
fn get_code_snippet(file_path: &str, range: &crate::parser::CodeRange) -> String {
    if let Ok(content) = std::fs::read_to_string(file_path) {
        let lines: Vec<&str> = content.lines().collect();
        if range.start_line > 0 && range.start_line <= lines.len() {
            let start = range.start_line - 1;
            let end = std::cmp::min(range.end_line, lines.len());
            if start < end {
                return lines[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>6}\u{2192}{}", start + 1 + i, line))
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }
    }
    String::new()
}

/// Whether a symbol is a "query-matching symbol" (P1, Tier-2): any sub-token of its NAME, or
/// of its OWNER (the enclosing type/impl/class), intersects the query token set. The
/// owner-token path is load-bearing — it keeps a query like "StorageFactory get" anchoring the
/// `get` method whose owner is `StorageFactory`, even though `get` alone is generic.
///
/// Tier-2 is the loose tier: it powers the no-Tier-1 fallback rendering. When a file holds a
/// Tier-1 (exact-name) symbol the renderer prefers Tier-1 and demotes the merely-Tier-2 hits to
/// a one-line list, because token-intersection alone over-matches (e.g. the `select` sub-token
/// of `SELECT` crossing `get_select`, `get_extra_select`, … — the live A/B regression P1 fixed).
pub(super) fn symbol_matches_query(
    sym: &crate::parser::ExtractedSymbol,
    query: &crate::parser::QueryTokens,
) -> bool {
    if crate::parser::split_identifier(&sym.name)
        .iter()
        .any(|t| query.contains_token(t))
    {
        return true;
    }
    if let Some(owner) = &sym.owner {
        if crate::parser::split_identifier(owner)
            .iter()
            .any(|t| query.contains_token(t))
        {
            return true;
        }
    }
    false
}

/// Whether a symbol is a Tier-1 (exact-name) match (P1): its NAME, lowercased, equals one of the
/// query words as a whole identifier, or one raw query word when punctuation is part of the
/// symbol name (`operator=`, `~Ops`). This is strict — `get_select` is NOT Tier-1 for query word
/// `select` — so Tier-1 anchors only the symbols the agent actually named.
pub(super) fn symbol_is_tier1(
    sym: &crate::parser::ExtractedSymbol,
    query: &crate::parser::QueryTokens,
) -> bool {
    let symbol_name = sym.name.to_lowercase();
    query.contains_word(&symbol_name) || query.contains_raw_word(&symbol_name)
}

/// A compact 2-line declaration summary for a CONTAINER whose member matched (P1 §4): the
/// container's first source lines, line-numbered like [`get_code_snippet`], so the agent sees
/// the class/impl header (and often its opening docstring) without the whole body being dumped.
/// Bounded to at most 2 physical lines of the symbol's range. Applies to any container enclosing
/// an anchor member — including a Tier-1 container that holds a Tier-1 member, which is demoted
/// to this summary so the matched member's own full snippet is what carries the detail.
fn get_summary_snippet(file_path: &str, range: &crate::parser::CodeRange) -> String {
    const SUMMARY_LINES: usize = 2;
    if let Ok(content) = std::fs::read_to_string(file_path) {
        let lines: Vec<&str> = content.lines().collect();
        if range.start_line > 0 && range.start_line <= lines.len() {
            let start = range.start_line - 1;
            let end = std::cmp::min(
                std::cmp::min(range.end_line, lines.len()),
                start + SUMMARY_LINES,
            );
            if start < end {
                return lines[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>6}\u{2192}{}", start + 1 + i, line))
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }
    }
    String::new()
}

/// A signature mini-snippet for a DEMOTED symbol (P2-loop-2 C1/C3): the symbol's first
/// `max_lines` source lines, line-numbered like [`get_code_snippet`], plus a `… (N more
/// lines)` marker when its body extends past what is shown. Replaces the prior one-line stub
/// for symbols that aren't the file's full-snippet anchor — a Tier-2 hit demoted by a Tier-1
/// neighbor, an anchor demoted past the per-file full-snippet cap (`max_lines = 3`), or a
/// non-matching symbol in a name-matched file (`max_lines = 1`) — so the agent still sees the
/// declaration signature instead of reconstructing it with a follow-up whole-file read (the
/// measured regression). Returns `(snippet, more_lines)`: an empty snippet (unreadable file /
/// out-of-range) carries `more_lines = 0` so the caller falls back to the bare stub line.
fn get_signature_snippet(
    file_path: &str,
    range: &crate::parser::CodeRange,
    max_lines: usize,
) -> (String, usize) {
    if let Ok(content) = std::fs::read_to_string(file_path) {
        let lines: Vec<&str> = content.lines().collect();
        if range.start_line > 0 && range.start_line <= lines.len() {
            let start = range.start_line - 1;
            let symbol_end = std::cmp::min(range.end_line, lines.len());
            let shown_end = std::cmp::min(symbol_end, start + max_lines);
            if start < shown_end {
                let snippet = lines[start..shown_end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>6}\u{2192}{}", start + 1 + i, line))
                    .collect::<Vec<_>>()
                    .join("\n");
                let more_lines = symbol_end.saturating_sub(shown_end);
                return (snippet, more_lines);
            }
        }
    }
    (String::new(), 0)
}

/// Tunable caps threaded into [`render_anchored_symbols`] — the subset of search-detail
/// config the shared anchoring/render path needs, so both the name-matched branch and the
/// symbol-fallback branch render with one identical rule set (P2-loop-2 C1/C3 unification).
pub(super) struct AnchoredRenderCaps {
    pub(super) snippet_max_lines: usize,
    pub(super) anchor_snippet_limit: usize,
    pub(super) byte_cap: usize,
}

/// Result of [`render_anchored_symbols`]: whether the byte budget was hit mid-file (so the
/// caller emits the truncation notice and stops), plus the start lines actually emitted as a
/// snippet/summary. The fallback branch uses `emitted_starts` to skip those symbols when it
/// prints the residual name-only list, so a symbol is never both rendered AND re-listed.
pub(super) struct AnchoredRenderOutcome {
    pub(super) budget_hit: bool,
    pub(super) emitted_starts: std::collections::HashSet<usize>,
}

/// The shared P1 2-tier anchoring + P2-loop-2 C1/C3 render path for a name-matched file's
/// symbols. Extracted so the symbol-fallback branch renders matched symbols with EXACTLY the
/// same rules as the primary name-matched branch — full snippets only for promoted anchors
/// (`anchor_snippet_limit`, Tier-1 first), a 2-line declaration summary for any container
/// enclosing an anchor member, a ≤3-line signature for over-cap/Tier-2 demotions, and a
/// 1-line signature for a non-matching symbol — instead of the prior loop-1 "5 full snippets,
/// no caps" rule that flooded a large fallback file (the `Signal`/`dispatcher.py` regression).
///
/// `symbols` is the already-selected render set (caller applies any count cap / fallback
/// selection first). Anchoring is computed over THIS set; promoted-anchor selection walks it
/// in the given order (so callers pass it in rank order before any range sort). Containers
/// enclosing an anchor are excluded from the promoted cap so a summarized container never
/// steals a full-snippet slot from a real member anchor (P2-loop-2 promoted/summary interaction).
pub(super) fn render_anchored_symbols(
    text: &mut String,
    file_path: &str,
    symbols: Vec<&crate::parser::ExtractedSymbol>,
    query: &crate::parser::QueryTokens,
    caps: &AnchoredRenderCaps,
    caller_annotations: Option<&crate::callers::DetailAnnotations>,
) -> AnchoredRenderOutcome {
    let mut emitted_starts: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let AnchoredRenderCaps {
        snippet_max_lines,
        anchor_snippet_limit,
        byte_cap,
    } = *caps;

    // Promoted-anchor selection walks the caller's order (rank order), so the highest-ranked
    // anchors take the full-snippet slots. The render loop then walks outermost-first so a
    // container is emitted before its members.
    let ranked = symbols;
    let mut render_order: Vec<&crate::parser::ExtractedSymbol> = ranked.clone();
    render_order.sort_by(|a, b| {
        a.range
            .start_line
            .cmp(&b.range.start_line)
            .then(b.range.end_line.cmp(&a.range.end_line))
    });

    // P1 2-tier anchoring. Tier-1 = NAME exactly equals a query word (strict). Tier-2 = the
    // looser sub-token/owner match. Anchor on Tier-1 when the file holds any, else Tier-2.
    let tier1_ranges: Vec<(usize, usize)> = render_order
        .iter()
        .filter(|s| symbol_is_tier1(s, query))
        .map(|s| (s.range.start_line, s.range.end_line))
        .collect();
    let has_tier1 = !tier1_ranges.is_empty();
    let tier2_ranges: Vec<(usize, usize)> = render_order
        .iter()
        .filter(|s| symbol_matches_query(s, query))
        .map(|s| (s.range.start_line, s.range.end_line))
        .collect();
    let anchor_ranges: &[(usize, usize)] = if has_tier1 {
        &tier1_ranges
    } else {
        &tier2_ranges
    };
    let any_match = !anchor_ranges.is_empty();
    let is_anchor_sym = |sym: &crate::parser::ExtractedSymbol| {
        if has_tier1 {
            symbol_is_tier1(sym, query)
        } else {
            symbol_matches_query(sym, query)
        }
    };
    let encloses_anchor_range = |start: usize, end: usize| {
        anchor_ranges
            .iter()
            .any(|(ms, me)| start <= *ms && *me <= end && (start, end) != (*ms, *me))
    };
    // P2-loop-2 C3: per-file full-snippet cap, highest-ranked anchors first. A container
    // enclosing an anchor is summarized (not a full-snippet anchor), so it is excluded here
    // and never consumes a slot a real member anchor should get.
    let promoted_anchor_starts: std::collections::HashSet<usize> = if any_match {
        ranked
            .iter()
            .copied()
            .filter(|s| {
                is_anchor_sym(s)
                    && !encloses_anchor_range(s.range.start_line, s.range.end_line)
            })
            .take(anchor_snippet_limit)
            .map(|s| s.range.start_line)
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let mut emitted_ranges: Vec<(usize, usize)> = Vec::new();
    let mut caller_block_dedup = crate::callers::CallerBlockDedup::new();
    for sym in render_order {
        let (start, end) = (sym.range.start_line, sym.range.end_line);
        if emitted_ranges
            .iter()
            .any(|(es, ee)| *es <= start && end <= *ee)
        {
            continue;
        }
        if text.len() >= byte_cap {
            return AnchoredRenderOutcome {
                budget_hit: true,
                emitted_starts,
            };
        }
        text.push_str(&format!(
            "- Symbol: {} ({}) [L{}-{}]\n",
            sym.name, sym.kind, sym.range.start_line, sym.range.end_line
        ));
        emitted_starts.insert(start);
        let is_anchor = is_anchor_sym(sym);
        let encloses_anchor = encloses_anchor_range(start, end);
        let is_summary_container = any_match && encloses_anchor && (has_tier1 || !is_anchor);
        let is_full_anchor = any_match && is_anchor && !is_summary_container;
        let is_overcap_anchor = is_full_anchor && !promoted_anchor_starts.contains(&start);
        let is_match = !any_match || (is_full_anchor && !is_overcap_anchor);
        if !is_match && !is_summary_container {
            // Demoted symbol (C1/C3): over-cap anchor or Tier-2 hit → 3 signature lines;
            // a non-matching symbol → 1 signature line. No caller/callee annotation.
            let is_tier2_hit = symbol_matches_query(sym, query);
            let sig_lines = if is_overcap_anchor || is_tier2_hit { 3 } else { 1 };
            let (sig, more_lines) = get_signature_snippet(file_path, &sym.range, sig_lines);
            if sig.is_empty() {
                emitted_ranges.push((start, start));
            } else {
                let body = if more_lines > 0 {
                    format!("{sig}\n… ({more_lines} more lines)")
                } else {
                    sig.clone()
                };
                text.push_str(&format!("```\n{body}\n```\n"));
                let shown = sig.lines().count();
                let displayed_end = start + shown.saturating_sub(1);
                emitted_ranges.push((start, displayed_end));
            }
            continue;
        }
        let snippet = if is_summary_container {
            get_summary_snippet(file_path, &sym.range)
        } else {
            get_code_snippet(file_path, &sym.range)
        };
        let snippet_lines = snippet.lines().count();
        let displayed_lines = snippet_lines.min(snippet_max_lines);
        let displayed_end = start + displayed_lines.saturating_sub(1);
        if !snippet.is_empty() {
            let capped = cap_snippet(&snippet, snippet_max_lines, byte_cap);
            text.push_str(&format!("```\n{}\n```\n", capped));
        }
        if !is_summary_container {
            if let Some(annotations) = caller_annotations {
                if let Some(prepared) = annotations.render(file_path, start, &caller_block_dedup) {
                    if text.len() + prepared.text().len() <= byte_cap {
                        text.push_str(prepared.text());
                        prepared.commit(&mut caller_block_dedup);
                    } else if text.len() + crate::callers::ANNOTATION_OMITTED_MARKER.len() <= byte_cap
                    {
                        text.push_str(crate::callers::ANNOTATION_OMITTED_MARKER);
                    }
                }
            }
        }
        emitted_ranges.push((start, displayed_end));
    }
    AnchoredRenderOutcome {
        budget_hit: false,
        emitted_starts,
    }
}
