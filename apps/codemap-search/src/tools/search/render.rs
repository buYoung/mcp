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

// Each file render owns this cache; nothing survives the current request.
pub(super) struct RenderSource<'a> {
    file_path: &'a str,
    content: std::cell::OnceCell<Option<String>>,
    masked: std::cell::OnceCell<Option<String>>,
    scan: std::cell::OnceCell<crate::redact::SourceScan>,
}

impl<'a> RenderSource<'a> {
    pub(super) fn new(file_path: &'a str) -> Self {
        Self {
            file_path,
            content: std::cell::OnceCell::new(),
            masked: std::cell::OnceCell::new(),
            scan: std::cell::OnceCell::new(),
        }
    }

    fn content(&self) -> Option<&str> {
        tracing::trace!(target: "codemap_search::render_source", file_path = self.file_path,
            cache_hit = self.content.get().is_some(), "source access");
        self.content
            .get_or_init(|| std::fs::read_to_string(self.file_path).ok())
            .as_deref()
    }

    fn displayed_content(&self) -> Option<&str> {
        self.masked
            .get_or_init(|| {
                self.content()
                    .map(|source| self.scan(source).render(source).into_owned())
            })
            .as_deref()
    }

    fn scan(&self, source: &str) -> &crate::redact::SourceScan {
        self.scan.get_or_init(|| {
            crate::redact::SourceScan::new(std::path::Path::new(self.file_path), source)
        })
    }

    pub(super) fn literal(&self, literal: &crate::parser::ExtractedLiteral) -> String {
        if !crate::redact::is_enabled() {
            return literal.text.clone();
        }
        self.content().map_or_else(
            || crate::redact::hidden(&literal.text),
            |source| self.scan(source).literal(source, literal),
        )
    }

    /// A qualified-literal hint has no line of its own. Hide the whole label if any
    /// matching occurrence is sensitive, rather than choosing a less-masked duplicate.
    pub(super) fn matched_literal_value(
        &self,
        value: &str,
        literals: &[crate::parser::ExtractedLiteral],
    ) -> String {
        if !crate::redact::is_enabled() {
            return value.to_string();
        }
        let mut has_match = false;
        for literal in literals.iter().filter(|literal| literal.text == value) {
            has_match = true;
            if self.literal(literal) != value {
                return crate::redact::hidden(value);
            }
        }
        if has_match {
            value.to_string()
        } else {
            crate::redact::hidden(value)
        }
    }
}

/// Extract a symbol's source range with `read`-style line numbers (`␠␠␠␠␠1→content`).
/// Numbered so the agent can cite exact lines straight from the detail view instead of
/// re-reading the file to confirm them (the dominant post-discovery turn cost observed).
fn get_code_snippet(source: &RenderSource<'_>, range: &crate::parser::CodeRange) -> String {
    if let Some(content) = source.displayed_content() {
        let lines: Vec<&str> = content.lines().collect();
        if range.start_line > 0 && range.start_line <= lines.len() {
            let start = range.start_line - 1;
            let end = std::cmp::min(range.end_line_inclusive(), lines.len());
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

fn evidence_window(
    source: &RenderSource<'_>,
    range: &crate::parser::CodeRange,
    query: &crate::parser::QueryTokens,
    max_lines: usize,
) -> (String, usize) {
    if query.is_identifier_lookup()
        || max_lines == 0
        || range.end_line_inclusive().saturating_sub(range.start_line) < max_lines
    {
        return (get_code_snippet(source, range), range.start_line);
    }
    let Some(content) = source.content() else {
        return (String::new(), range.start_line);
    };
    let lines = content.lines().collect::<Vec<_>>();
    let start = range.start_line.saturating_sub(1).min(lines.len());
    let end = range.end_line_inclusive().min(lines.len());
    if start >= end {
        return (String::new(), range.start_line);
    }
    let coverage = lines[start..end]
        .iter()
        .map(|line| {
            let tokens = crate::parser::QueryTokens::parse(line);
            query
                .tokens()
                .iter()
                .enumerate()
                .filter_map(|(i, token)| tokens.contains_token(token).then_some(i))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut counts = vec![0usize; query.tokens().len()];
    let mut body_counts = vec![0usize; query.tokens().len()];
    let mut covered = 0;
    let mut body_covered = 0;
    let mut best = (0, 0, 0);
    for (index, hits) in coverage.iter().enumerate() {
        for &hit in hits {
            if counts[hit] == 0 {
                covered += 1;
            }
            counts[hit] += 1;
        }
        if index > 0 {
            for &hit in hits {
                if body_counts[hit] == 0 {
                    body_covered += 1;
                }
                body_counts[hit] += 1;
            }
        }
        if index >= max_lines {
            for &hit in &coverage[index - max_lines] {
                counts[hit] -= 1;
                if counts[hit] == 0 {
                    covered -= 1;
                }
            }
        }
        if index > max_lines {
            for &hit in &coverage[index - max_lines] {
                body_counts[hit] -= 1;
                if body_counts[hit] == 0 {
                    body_covered -= 1;
                }
            }
        }
        let window = index.saturating_add(1).saturating_sub(max_lines);
        if (covered, body_covered) > (best.0, best.1) {
            best = (covered, body_covered, window);
        }
    }
    let first_hit = coverage
        .iter()
        .enumerate()
        .skip(best.2)
        .take(max_lines)
        .find(|(_, hits)| !hits.is_empty())
        .map_or(best.2, |(i, _)| i);
    let window_start = start + best.2.max(first_hit.saturating_sub(2));
    let window_end = (window_start + max_lines).min(end);
    let displayed = source
        .displayed_content()
        .unwrap_or("")
        .lines()
        .collect::<Vec<_>>();
    let snippet = displayed[window_start..window_end]
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>6}→{}", window_start + i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");
    (snippet, window_start + 1)
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

const STATIC_RELATION_MAX_BYTES: usize = 1536;
const STATIC_RELATION_MAX_GROUPS: usize = 4;
const STATIC_RELATION_HEADER: &str =
    "\n\n## Related write/read paths (static hint)\n\n_Inferred from direct, typed collection identity; confirm in source._\n";

fn static_relation_inline(text: &str, max_chars: usize) -> String {
    let normalized = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('`', "'");
    if normalized.chars().count() > max_chars {
        format!(
            "{}…",
            normalized.chars().take(max_chars).collect::<String>()
        )
    } else {
        normalized
    }
}

fn static_relation_context(edge: &crate::parser::StaticCollectionEdge) -> String {
    match (&edge.source_owner, &edge.source_symbol) {
        (Some(owner), Some(symbol)) => format!("{owner}.{symbol}"),
        (Some(owner), None) => owner.clone(),
        (None, Some(symbol)) => symbol.clone(),
        (None, None) => "<file scope>".to_string(),
    }
}

fn has_static_owner_connection(
    snapshot: &crate::index::PublishedIndexSnapshot,
    producer: &crate::index::StaticCollectionRecord,
    consumer: &crate::index::StaticCollectionRecord,
) -> bool {
    let producer_is_self = matches!(producer.edge.owner_expression.as_str(), "self" | "this");
    let consumer_is_self = matches!(consumer.edge.owner_expression.as_str(), "self" | "this");
    let same_lexical_owner = producer_is_self
        && consumer_is_self
        && producer.file_path == consumer.file_path
        && producer.edge.source_owner == consumer.edge.source_owner
        && producer.edge.source_owner_range.is_some()
        && producer.edge.source_owner_range == consumer.edge.source_owner_range
        && producer.edge.collection_owner_type == consumer.edge.collection_owner_type;
    if same_lexical_owner {
        return true;
    }

    // Cross-file lexical-self joins are safe only for an indexed owner name with a single
    // declaration. A qualified external type never has that local declaration evidence.
    if producer_is_self
        && consumer_is_self
        && producer.edge.source_owner == consumer.edge.source_owner
        && producer.edge.collection_owner_type == consumer.edge.collection_owner_type
    {
        return producer.edge.source_owner_range.is_some()
            && consumer.edge.source_owner_range.is_some()
            && snapshot.type_declaration_count(&producer.edge.collection_owner_type) == 1;
    }

    // A non-self receiver (parameter/member) is never joined by name alone. It must name a
    // type declared in the receiver's file and meet a lexical-self endpoint for that same type.
    let (typed, lexical) = if producer_is_self && !consumer_is_self {
        (consumer, producer)
    } else if !producer_is_self && consumer_is_self {
        (producer, consumer)
    } else {
        return false;
    };
    typed.file_path == lexical.file_path
        && snapshot.unique_type_declaration_range_in_path(
            &typed.file_path,
            &typed.edge.collection_owner_type,
        ) == lexical.edge.source_owner_range.as_ref()
        && lexical.edge.source_owner.as_deref() == Some(typed.edge.collection_owner_type.as_str())
}

/// Render resolved producer-to-consumer pairs in bytes left after the ranked result body.
pub(super) fn render_static_collection_edges(
    anchors: &[(String, usize, usize)],
    snapshot: &crate::index::PublishedIndexSnapshot,
    records: Vec<&crate::index::StaticCollectionRecord>,
    workspace_scope: Option<&str>,
    available_bytes: usize,
) -> String {
    use crate::parser::StaticCollectionEdgeKind;

    let section_cap = available_bytes.min(STATIC_RELATION_MAX_BYTES);
    if section_cap <= STATIC_RELATION_HEADER.len() {
        return String::new();
    }

    let mut producers: Vec<_> = records
        .iter()
        .filter(|record| record.edge.kind == StaticCollectionEdgeKind::Producer)
        .collect();
    let is_displayed = |record: &crate::index::StaticCollectionRecord| {
        anchors.iter().any(|(path, start, end)| {
            path == &record.file_path
                && *start <= record.edge.range.end_line_inclusive()
                && record.edge.range.start_line <= *end
        })
    };
    // Prioritize endpoints in the displayed evidence. The deterministic record order still
    // resolves ties, but irrelevant counterparts can no longer consume the pair budget first.
    producers.sort_by_key(|record| !is_displayed(record));
    let mut consumers_by_collection = std::collections::HashMap::new();
    for record in records
        .iter()
        .filter(|record| record.edge.kind == StaticCollectionEdgeKind::Consumer)
    {
        let edge = &record.edge;
        consumers_by_collection
            .entry((
                edge.collection_owner_type.as_str(),
                edge.collection_field.as_str(),
            ))
            .or_insert_with(Vec::new)
            .push(record);
    }
    for consumers in consumers_by_collection.values_mut() {
        consumers.sort_by_key(|record| !is_displayed(record));
    }
    let mut seen_edges = std::collections::HashSet::new();
    let mut group_order = Vec::new();
    let mut resolved = Vec::new();

    const STATIC_RELATION_PAIR_MAX_PER_GROUP: usize = 64;
    let mut accepted_pairs_by_group = std::collections::HashMap::new();
    let mut has_more_edges = false;
    for producer in producers {
        let Some(consumers) = consumers_by_collection.get(&(
            producer.edge.collection_owner_type.as_str(),
            producer.edge.collection_field.as_str(),
        )) else {
            continue;
        };
        for consumer in consumers {
            if !is_displayed(producer) && !is_displayed(consumer) {
                continue;
            }
            if workspace_scope.is_some_and(|scope| {
                !super::monorepo::path_is_under_scope(&producer.file_path, scope)
                    || !super::monorepo::path_is_under_scope(&consumer.file_path, scope)
            }) || !has_static_owner_connection(snapshot, producer, consumer)
            {
                continue;
            }
            let collection = format!(
                "{}.{}",
                producer.edge.collection_owner_type, producer.edge.collection_field
            );
            let accepted_pairs = accepted_pairs_by_group
                .entry(collection.clone())
                .or_insert(0usize);
            if *accepted_pairs >= STATIC_RELATION_PAIR_MAX_PER_GROUP {
                has_more_edges = true;
                continue;
            }
            let dedup_key = format!(
                "{}:{}:{}:{}:{}:{}",
                producer.file_path,
                producer.edge.range.start_line,
                consumer.file_path,
                consumer.edge.range.start_line,
                producer.edge.collection_owner_type,
                producer.edge.collection_field
            );
            if !seen_edges.insert(dedup_key) {
                continue;
            }
            *accepted_pairs += 1;
            if !group_order.contains(&collection) {
                group_order.push(collection.clone());
            }
            resolved.push((collection, producer, consumer));
        }
    }
    if resolved.is_empty() {
        return String::new();
    }

    let has_more_groups = group_order.len() > STATIC_RELATION_MAX_GROUPS;
    let selected_groups: Vec<String> = group_order
        .into_iter()
        .take(STATIC_RELATION_MAX_GROUPS)
        .collect();
    let mut lines = Vec::new();
    for collection in &selected_groups {
        let members: Vec<_> = resolved
            .iter()
            .filter(|(name, ..)| name == collection)
            .collect();
        let (_, producer, consumer) = members[0];
        let producer_sites: std::collections::HashSet<_> = members
            .iter()
            .map(|(_, record, ..)| (record.file_path.as_str(), record.edge.range.start_line))
            .collect();
        let consumer_sites: std::collections::HashSet<_> = members
            .iter()
            .map(|(_, _, record)| (record.file_path.as_str(), record.edge.range.start_line))
            .collect();
        let extras = match (producer_sites.len() - 1, consumer_sites.len() - 1) {
            (0, 0) => String::new(),
            (producer_count, consumer_count) => {
                format!(" (+{producer_count} write sites, +{consumer_count} read sites)")
            }
        };
        lines.push(format!(
            "- `{}`: `{}` at `{}:{}` → `{}` at `{}:{}`{}\n",
            static_relation_inline(collection, 100),
            static_relation_inline(&static_relation_context(&producer.edge), 100),
            static_relation_inline(&producer.file_path, 180),
            producer.edge.range.start_line,
            static_relation_inline(&static_relation_context(&consumer.edge), 100),
            static_relation_inline(&consumer.file_path, 180),
            consumer.edge.range.start_line,
            extras,
        ));
    }

    const OMITTED: &str = "- _More relation groups omitted._\n";
    const OMITTED_SHORT: &str = "- _More edges omitted._\n";
    let mut output = String::from(STATIC_RELATION_HEADER);
    let mut shown = 0usize;
    for (index, line) in lines.iter().enumerate() {
        let will_have_more = has_more_groups || has_more_edges || index + 1 < lines.len();
        let reserved_marker_bytes = if will_have_more {
            OMITTED_SHORT.len()
        } else {
            0
        };
        if output.len() + line.len() + reserved_marker_bytes > section_cap {
            break;
        }
        output.push_str(line);
        shown += 1;
    }
    if shown == 0 {
        return String::new();
    }
    if has_more_groups || has_more_edges || shown < lines.len() {
        if output.len() + OMITTED.len() <= section_cap {
            output.push_str(OMITTED);
        } else if output.len() + OMITTED_SHORT.len() <= section_cap {
            output.push_str(OMITTED_SHORT);
        } else {
            return String::new();
        }
    }
    output
}

/// A compact 2-line declaration summary for a CONTAINER whose member matched (P1 §4): the
/// container's first source lines, line-numbered like [`get_code_snippet`], so the agent sees
/// the class/impl header (and often its opening docstring) without the whole body being dumped.
/// Bounded to at most 2 physical lines of the symbol's range. Applies to any container enclosing
/// an anchor member — including a Tier-1 container that holds a Tier-1 member, which is demoted
/// to this summary so the matched member's own full snippet is what carries the detail.
fn get_summary_snippet(source: &RenderSource<'_>, range: &crate::parser::CodeRange) -> String {
    const SUMMARY_LINES: usize = 2;
    if let Some(content) = source.displayed_content() {
        let lines: Vec<&str> = content.lines().collect();
        if range.start_line > 0 && range.start_line <= lines.len() {
            let start = range.start_line - 1;
            let end = std::cmp::min(
                std::cmp::min(range.end_line_inclusive(), lines.len()),
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
    source: &RenderSource<'_>,
    range: &crate::parser::CodeRange,
    max_lines: usize,
) -> (String, usize) {
    if let Some(content) = source.displayed_content() {
        let lines: Vec<&str> = content.lines().collect();
        if range.start_line > 0 && range.start_line <= lines.len() {
            let start = range.start_line - 1;
            let symbol_end = std::cmp::min(range.end_line_inclusive(), lines.len());
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
    text: &mut super::grouped::FileOutput,
    source: &RenderSource<'_>,
    symbols: Vec<&crate::parser::ExtractedSymbol>,
    query: &crate::parser::QueryTokens,
    caps: &AnchoredRenderCaps,
    caller_annotations: Option<&crate::callers::DetailAnnotations>,
    caller_block_dedup: &mut crate::callers::CallerBlockDedup,
) -> AnchoredRenderOutcome {
    let file_path = source.file_path;
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
        a.range.start_line.cmp(&b.range.start_line).then(
            b.range
                .end_line_inclusive()
                .cmp(&a.range.end_line_inclusive()),
        )
    });

    // P1 2-tier anchoring. Tier-1 = NAME exactly equals a query word (strict). Tier-2 = the
    // looser sub-token/owner match. Anchor on Tier-1 when the file holds any, else Tier-2.
    let tier1_ranges: Vec<(usize, usize)> = render_order
        .iter()
        .filter(|s| symbol_is_tier1(s, query))
        .map(|s| (s.range.start_line, s.range.end_line_inclusive()))
        .collect();
    let has_tier1 = query.is_identifier_lookup() && !tier1_ranges.is_empty();
    let tier2_ranges: Vec<(usize, usize)> = render_order
        .iter()
        .filter(|s| symbol_matches_query(s, query))
        .map(|s| (s.range.start_line, s.range.end_line_inclusive()))
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
                    && !encloses_anchor_range(s.range.start_line, s.range.end_line_inclusive())
            })
            .take(anchor_snippet_limit)
            .map(|s| s.range.start_line)
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let mut emitted_ranges: Vec<(usize, usize)> = Vec::new();
    for sym in render_order {
        let (start, end) = (sym.range.start_line, sym.range.end_line_inclusive());
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
        if !text.start_symbol(sym, source.content()) {
            return AnchoredRenderOutcome {
                budget_hit: true,
                emitted_starts,
            };
        }
        emitted_starts.insert(start);
        let is_anchor = is_anchor_sym(sym);
        let encloses_anchor = encloses_anchor_range(start, end);
        let is_summary_container = any_match && encloses_anchor && (has_tier1 || !is_anchor);
        let is_full_anchor = any_match && is_anchor && !is_summary_container;
        if is_full_anchor {
            text.anchor(start, start);
        }
        let is_overcap_anchor = is_full_anchor && !promoted_anchor_starts.contains(&start);
        // Over-match suppression (#3): when NO symbol matched the query (`!any_match` — the file
        // ranked in on path/docstring and these are its own symbols, e.g. forward declarations or
        // large non-anchor container methods), each symbol is demoted from a full snippet to a
        // 3-line signature instead of dumping its whole body. Previously `!any_match` short-
        // circuited `is_match` to true and rendered every symbol in full. With `any_match` this is
        // byte-identical to before, so the common (a-symbol-matched) path does not regress; the
        // `symbol_fallback` branch never reaches here with non-matching symbols (it passes only
        // matching symbols or an empty set), so pure path/docstring hits stay visible.
        let is_match = is_full_anchor && !is_overcap_anchor;
        if !is_match && !is_summary_container {
            // Demoted symbol (C1/C3/#3): over-cap anchor, Tier-2 hit, or an over-match symbol in a
            // no-symbol-matched file → 3 signature lines; an `any_match` non-matching symbol → 1
            // signature line. No caller/callee annotation.
            let is_tier2_hit = symbol_matches_query(sym, query);
            let sig_lines = if is_overcap_anchor || is_tier2_hit || !any_match {
                3
            } else {
                1
            };
            let (sig, more_lines) = get_signature_snippet(source, &sym.range, sig_lines);
            if sig.is_empty() {
                emitted_ranges.push((start, start));
            } else {
                let body = if more_lines > 0 {
                    format!("{sig}\n… ({more_lines} more lines)")
                } else {
                    sig.clone()
                };
                let remaining = text.remaining_bytes().saturating_sub(40);
                if remaining < 64 {
                    return AnchoredRenderOutcome {
                        budget_hit: true,
                        emitted_starts,
                    };
                }
                let capped = cap_snippet(&body, sig_lines + 1, remaining);
                let is_clipped = capped.ends_with("\n… (truncated)");
                text.budget_hit |= is_clipped;
                if !text.push_source(&format!("```\n{capped}\n```\n")) {
                    return AnchoredRenderOutcome {
                        budget_hit: true,
                        emitted_starts,
                    };
                }
                let shown = capped
                    .lines()
                    .filter(|line| line.contains('→'))
                    .count()
                    .saturating_sub(usize::from(is_clipped));
                let displayed_end = start + shown.saturating_sub(1);
                emitted_ranges.push((start, displayed_end));
                if is_full_anchor {
                    text.anchor(start, displayed_end);
                }
            }
            continue;
        }
        let (snippet, snippet_start) = if is_summary_container {
            // A container preview must not cover a complete short member and
            // suppress that member's declaration/caller annotation as duplicate.
            let first_member = anchor_ranges
                .iter()
                .filter(|&&(member_start, member_end)| {
                    start <= member_start
                        && member_end <= end
                        && (start, end) != (member_start, member_end)
                })
                .map(|&(member_start, _)| member_start)
                .min()
                .unwrap_or(end.saturating_add(1));
            (
                get_summary_snippet(source, &sym.range)
                    .lines()
                    .take(first_member.saturating_sub(start))
                    .collect::<Vec<_>>()
                    .join("\n"),
                start,
            )
        } else {
            evidence_window(source, &sym.range, query, snippet_max_lines)
        };
        let snippet_lines = snippet.lines().count();
        let mut displayed_lines = snippet_lines.min(snippet_max_lines);
        let mut displayed_end = snippet_start + displayed_lines.saturating_sub(1);
        let mut is_byte_clipped = false;
        if !snippet.is_empty() {
            let needs_notice = !is_summary_container
                && (snippet_start > start
                    || displayed_end < end
                    || snippet.len() + 8 > text.remaining_bytes());
            let notice_reserve = if needs_notice {
                serde_json::json!({"file_path":file_path,"offset":usize::MAX,"limit":usize::MAX,"view":"source"}).to_string().len() + 160
            } else {
                0
            };
            let remaining = text
                .remaining_bytes()
                .saturating_sub(8 + notice_reserve + if needs_notice { 32 } else { 0 });
            if remaining == 0 {
                return AnchoredRenderOutcome {
                    budget_hit: true,
                    emitted_starts,
                };
            }
            let capped = cap_snippet(&snippet, snippet_max_lines, remaining);
            let shown_lines = capped
                .lines()
                .filter_map(|line| {
                    line.split_once('→')
                        .and_then(|(number, _)| number.trim().parse::<usize>().ok())
                })
                .collect::<Vec<_>>();
            displayed_lines = shown_lines.len();
            displayed_end = shown_lines.last().copied().unwrap_or(snippet_start);
            is_byte_clipped = capped.ends_with("\n… (truncated)");
            text.budget_hit |= is_byte_clipped;
            let mut source_block = String::new();
            if needs_notice && displayed_lines > 0 {
                // Repeat a partially printed last line; never skip its hidden suffix.
                let next = if is_byte_clipped {
                    displayed_end
                } else if displayed_end < end {
                    displayed_end + 1
                } else {
                    start
                };
                let request = serde_json::json!({"file_path":file_path,"offset":next,"limit":end.saturating_sub(next).saturating_add(1).min(snippet_max_lines.max(1)),"view":"source"});
                source_block.push_str(&format!("- source window: L{snippet_start}-{displayed_end} of L{start}-{end}; remaining source omitted. Next: read {request}\n"));
            }
            source_block.push_str(&format!("```\n{}\n```\n", capped));
            if !text.push_source(&source_block) {
                return AnchoredRenderOutcome {
                    budget_hit: true,
                    emitted_starts,
                };
            }
        }
        if !is_summary_container {
            if let Some(annotations) = caller_annotations {
                if let Some(prepared) =
                    annotations.render_for_symbol(file_path, sym, caller_block_dedup)
                {
                    if text.push_annotation(prepared.text()) {
                        prepared.commit(caller_block_dedup);
                    } else if !text.push_annotation(crate::callers::ANNOTATION_OMITTED_MARKER) {
                        text.budget_hit = true;
                    }
                }
            }
        }
        if displayed_lines > 0 {
            let complete_end = displayed_end.saturating_sub(usize::from(is_byte_clipped));
            if complete_end >= snippet_start {
                emitted_ranges.push((snippet_start, complete_end));
                if is_full_anchor {
                    text.anchor(snippet_start, complete_end);
                }
            }
        }
    }
    AnchoredRenderOutcome {
        budget_hit: false,
        emitted_starts,
    }
}
