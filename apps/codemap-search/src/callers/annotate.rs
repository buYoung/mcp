//! Render / protocol stage: turns the scan + snapshot into the markdown annotation block,
//! enforces the two-counter byte budget, and exposes the render→emit→commit dedup contract
//! the server-side renderer (`mcp.rs`) drives. This is the top of the pipeline — it consumes
//! [`super::scan::ScanResult`] and the symbol index; nothing in `callers/` depends back on it.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

use crate::parser::{CallSite, ExtractedFile, ExtractedSymbol, ImportKind};

use super::callees::{discover_callees, discover_callees_with_navigation, DiscoveredCallee};
use super::scan::{scan_workspace, ScanResult};
use super::source::SourceSyntax;
use super::symbols::{
    build_navigation_index, build_symbol_index, definition_is_compatible, enclosing_fn,
    is_within_same_named_fn, lookup_compatible_candidates, lookup_source_hint_candidates,
    NavigationIndex, SourceHintResolution, SymbolIndex,
};
use super::{
    decorator_lines_above, extension_of, is_import_line, qualified_name, AnnotationRuntimeState,
    CallerConfig,
};

/// One-line stand-in emitted when a symbol's full annotation would overflow the remaining
/// byte budget, so an omission is visible instead of silent. Also used by `mcp.rs` at the
/// note-attach point when snippets have since consumed the overall-cap headroom.
pub const ANNOTATION_OMITTED_MARKER: &str =
    "  - _call-context annotation omitted (byte budget exhausted)_\n";

/// The observation-scope caveat appended whenever a `fn` shows no discoverable callers —
/// so a zero is never read as "dead code".
const OBSERVATION_SCOPE_CAVEAT: &str =
    "_(no direct caller observed — scope: indexed source eligible for caller lookup, \
direct `name(` calls; callbacks, higher-order/method-reference passing, macro-wrapped, and \
event/dispatch calls are not counted; approximate)_";

#[derive(Debug, Default)]
struct NavigationMetrics {
    navigation_precise_count: usize,
    navigation_fallback_count: usize,
    navigation_fallback_reason: HashMap<&'static str, usize>,
    navigation_callsite_count: usize,
    navigation_snapshot_bytes: usize,
    navigation_snapshot_load_ms: u128,
    navigation_annotation_ms: u128,
    navigation_receiver_hint_precise_count: usize,
    navigation_receiver_hint_fallback_count: usize,
}

impl NavigationMetrics {
    fn record_fallback(&mut self, reason: &'static str, count: usize) {
        if count == 0 {
            return;
        }
        self.navigation_fallback_count += count;
        *self.navigation_fallback_reason.entry(reason).or_insert(0) += count;
    }

    fn trace(&self) {
        tracing::debug!(
            navigation_precise_count = self.navigation_precise_count,
            navigation_fallback_count = self.navigation_fallback_count,
            navigation_fallback_reason = ?self.navigation_fallback_reason,
            navigation_callsite_count = self.navigation_callsite_count,
            navigation_snapshot_bytes = self.navigation_snapshot_bytes,
            navigation_snapshot_load_ms = self.navigation_snapshot_load_ms,
            navigation_annotation_ms = self.navigation_annotation_ms,
            navigation_receiver_hint_precise_count = self.navigation_receiver_hint_precise_count,
            navigation_receiver_hint_fallback_count = self.navigation_receiver_hint_fallback_count,
            "navigation attribution metrics"
        );
    }
}

fn navigation_snapshot_bytes(snapshot: &[ExtractedFile]) -> usize {
    snapshot
        .iter()
        .filter_map(|file| file.navigation.as_ref())
        .filter_map(|navigation| serde_json::to_vec(navigation).ok())
        .map(|bytes| bytes.len())
        .sum()
}

fn navigation_callsite_count(snapshot: &[ExtractedFile]) -> usize {
    snapshot
        .iter()
        .filter_map(|file| file.navigation.as_ref())
        .map(|navigation| navigation.calls.len())
        .sum()
}

fn navigation_receiver_callsite_count(snapshot: &[ExtractedFile]) -> usize {
    snapshot
        .iter()
        .filter_map(|file| file.navigation.as_ref())
        .map(|navigation| {
            navigation
                .calls
                .iter()
                .filter(|call| call.receiver.is_some())
                .count()
        })
        .sum()
}

fn symbol_identity_matches(
    candidate_file: &ExtractedFile,
    candidate: &ExtractedSymbol,
    target_file_path: &str,
    target: &ExtractedSymbol,
) -> bool {
    candidate_file.file_path == target_file_path
        && candidate.name == target.name
        && candidate.range.start_line == target.range.start_line
        && candidate.range.end_line == target.range.end_line
}

fn import_hint_for_call(call: &CallSite, call_file: &ExtractedFile) -> Option<(String, String)> {
    let navigation = call_file.navigation.as_ref()?;
    for import in &navigation.imports {
        match import.kind {
            ImportKind::Namespace => {
                if call.receiver.as_deref() == Some(import.local_name.as_str()) {
                    if let Some(source) = &import.source {
                        return Some((call.name.clone(), source.clone()));
                    }
                }
            }
            ImportKind::Named | ImportKind::Default => {
                if call.receiver.is_none() && import.local_name == call.name {
                    if let Some(source) = &import.source {
                        let lookup_name = import
                            .imported_name
                            .clone()
                            .unwrap_or_else(|| call.name.clone());
                        return Some((lookup_name, source.clone()));
                    }
                }
            }
            ImportKind::Glob => {}
        }
    }
    None
}

fn has_non_function_local_shadow(
    call: &CallSite,
    call_file: &ExtractedFile,
    index: &SymbolIndex<'_>,
) -> bool {
    let Some(navigation) = &call_file.navigation else {
        return false;
    };
    let is_nix = Path::new(&call_file.file_path)
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("nix");
    let has_known_function = index.function_definition_count(&call.name) > 0;
    navigation.local_bindings.iter().any(|binding| {
        if binding.name != call.name || (has_known_function && !is_nix) {
            return false;
        }
        match (binding.scope_id, call.scope_id) {
            (Some(binding_scope), Some(call_scope)) => binding_scope == call_scope,
            _ => binding.range.start_line <= call.range.start_line,
        }
    })
}

fn resolve_navigation_call<'a>(
    call: &CallSite,
    call_file: &'a ExtractedFile,
    snapshot: &'a [ExtractedFile],
    index: &'a SymbolIndex<'a>,
    navigation_index: &NavigationIndex,
    is_member: bool,
) -> Option<(&'a ExtractedFile, &'a ExtractedSymbol)> {
    if has_non_function_local_shadow(call, call_file, index) {
        return None;
    }

    if let Some((lookup_name, source)) = import_hint_for_call(call, call_file) {
        match lookup_source_hint_candidates(
            &lookup_name,
            &call_file.file_path,
            &source,
            snapshot,
            navigation_index,
        ) {
            Ok(candidates) if candidates.len() == 1 => {
                let (file, symbol) = candidates[0];
                return definition_is_compatible(
                    &call_file.file_path,
                    is_member,
                    &file.file_path,
                    symbol,
                )
                .then_some((file, symbol));
            }
            Ok(_) | Err(SourceHintResolution::SourceUnresolved) => return None,
            Err(SourceHintResolution::UnsupportedSourceForm) => {
                // Non-relative imports cannot be resolved to a workspace source path. Keep the
                // same-file/global exact-one fallback available rather than dropping the call.
            }
        }
    }

    let global = lookup_compatible_candidates(&call.name, &call_file.file_path, is_member, index);
    let same_file: Vec<_> = global
        .iter()
        .copied()
        .filter(|(file, _)| file.file_path == call_file.file_path)
        .collect();
    if same_file.len() == 1 {
        return Some(same_file[0]);
    }
    if same_file.len() > 1 {
        return None;
    }
    if global.len() == 1 {
        Some(global[0])
    } else {
        None
    }
}

struct ConfirmedCallers {
    entries: Vec<String>,
    is_partial: bool,
}

#[allow(clippy::too_many_arguments)]
fn precise_navigation_callers(
    target: &ExtractedSymbol,
    target_file_path: &str,
    snapshot: &[ExtractedFile],
    index: &SymbolIndex<'_>,
    navigation_index: &NavigationIndex,
    cfg: &CallerConfig,
    runtime_state: AnnotationRuntimeState,
    root: &Path,
    resolver: &super::resolution::SourceResolver<'_>,
) -> Option<ConfirmedCallers> {
    if (!cfg.navigation_context_default && !super::resolution::supports(target_file_path))
        || runtime_state.suppresses_navigation()
    {
        return None;
    }
    if navigation_index.calls_by_name.is_empty() && !super::resolution::supports(target_file_path) {
        return Some(ConfirmedCallers {
            entries: Vec::new(),
            is_partial: false,
        });
    }
    let mut inspected = 0usize;
    let mut caller_entries = Vec::new();
    let mut seen = HashSet::new();
    // Aliases are only candidate spellings. Every site still has to resolve to
    // this exact source definition, including through conditional reexports.
    let mut relevant_names = HashSet::from([target.name.as_str()]);
    let mut is_alias_limited = false;
    if target_file_path.ends_with(".rs") {
        for round in 0..16 {
            let before = relevant_names.len();
            for import in snapshot
                .iter()
                .filter(|file| file.file_path.ends_with(".rs"))
                .filter_map(|file| file.navigation.as_ref())
                .flat_map(|navigation| &navigation.imports)
            {
                if import
                    .imported_name
                    .as_deref()
                    .is_some_and(|name| relevant_names.contains(name))
                {
                    if !relevant_names.contains(import.local_name.as_str())
                        && relevant_names.len() >= 256
                    {
                        is_alias_limited = true;
                        break;
                    }
                    relevant_names.insert(import.local_name.as_str());
                }
            }
            if relevant_names.len() == before || is_alias_limited {
                break;
            }
            if round == 15 {
                is_alias_limited = true;
            }
        }
    }
    for call_file in snapshot.iter().filter(|file| file.navigation.is_some()) {
        let navigation = call_file.navigation.as_ref().unwrap();
        let relevant = |call: &&CallSite| {
            relevant_names.contains(call.name.as_str())
                || navigation.imports.iter().any(|import| {
                    import.local_name == call.name
                        && import.imported_name.as_deref() == Some(&target.name)
                })
        };
        if !navigation.calls.iter().any(|call| relevant(&call)) {
            continue;
        }
        let has_source_resolution = super::resolution::supports(&call_file.file_path);
        let syntax = if has_source_resolution {
            None
        } else {
            super::read_workspace_file(&call_file.file_path, root)
                .and_then(|source| SourceSyntax::parse(&call_file.file_path, source.as_bytes()))
        };
        for call in navigation.calls.iter().filter(relevant) {
            if !index.includes(&call_file.file_path, &call.range) {
                continue;
            }
            inspected += 1;
            if inspected > cfg.navigation_callsite_budget {
                return Some(ConfirmedCallers {
                    entries: caller_entries,
                    is_partial: true,
                });
            }
            let resolved = if has_source_resolution {
                resolver
                    .resolve_call(call_file, call)
                    .filter(|target| target.is_precise)
                    .map(|target| (target.file, target.symbol))
            } else {
                resolve_navigation_call(
                    call,
                    call_file,
                    snapshot,
                    index,
                    navigation_index,
                    syntax
                        .as_ref()
                        .and_then(|syntax| syntax.is_member_call(call))
                        .unwrap_or(call.receiver.is_some()),
                )
            };
            let Some((candidate_file, candidate)) = resolved else {
                continue;
            };
            if !symbol_identity_matches(candidate_file, candidate, target_file_path, target) {
                continue;
            }
            let caller_name = if has_source_resolution {
                resolver.caller_name(call_file, call.range.start_line)
            } else {
                enclosing_fn(call_file, call.range.start_line)
                    .map(|enclosing| qualified_name(enclosing, &call_file.file_path))
            };
            let entry = match caller_name {
                Some(name) => {
                    format!(
                        "{} ({}:{})",
                        name, call_file.file_path, call.range.start_line
                    )
                }
                None => format!(
                    "{}:{} (top-level/unindexed)",
                    call_file.file_path, call.range.start_line
                ),
            };
            if seen.insert(entry.clone()) {
                caller_entries.push(entry);
            }
        }
    }
    Some(ConfirmedCallers {
        entries: caller_entries,
        is_partial: is_alias_limited,
    })
}

/// One matched `fn` symbol's annotation, kept in THREE separable parts so the P2 caller-block
/// dedup can be applied at the actual emission point in render order (not here, in scan order).
///
/// Rendering layout is always `prefix` + (caller block) + `suffix`. The caller block is held
/// apart from the fixed parts because only it is deduped: a later same-named symbol whose
/// `caller_block` is byte-identical to one ALREADY EMITTED collapses to a "same as `name`
/// above" back-reference. `prefix` (decorator/entry-point label) and `suffix` (callees) are
/// never deduped. Holding the parts separate is what guarantees a "same as above" line is
/// only ever emitted AFTER its referenced original block was actually printed — the original
/// is chosen in render order at emission time, so a scan-order original that the renderer
/// later drops (summary container, symbol cap, byte budget) can no longer leave a dangling
/// back-reference (the live A/B "dangling `same as __iter__ above`" defect).
#[derive(Debug, Clone)]
struct SymbolAnnotation {
    /// The matched symbol's name (dedup key for the caller block, per file).
    name: String,
    /// Decorator/entry-point label rendered before the caller block (may be empty).
    prefix: String,
    /// The caller list / omission note / observation caveat block. The only deduped part.
    caller_block: String,
    /// The depth-1 callee block rendered after the caller block (may be empty).
    suffix: String,
}

impl SymbolAnnotation {
    /// Total rendered length (the three parts emitted back-to-back). Used for the byte-budget
    /// accounting in `annotate_results`, which reserves against the FULL (un-deduped) length —
    /// the render-order dedup only ever emits this or LESS, so the reserved cap never overflows.
    fn full_len(&self) -> usize {
        self.prefix.len() + self.caller_block.len() + self.suffix.len()
    }

    /// Render in render order, deduping the caller block against `seen_caller_blocks` (a
    /// view-wide `name → already-emitted caller block` map owned by the renderer, shared across
    /// files). When this symbol's caller block byte-matches one already emitted for the same name,
    /// the caller block collapses to a one-line back-reference; otherwise it renders in full. The
    /// fixed `prefix` / `suffix` always render verbatim.
    ///
    /// The map is NOT mutated here: a full caller block is recorded as the back-reference
    /// target only once the renderer confirms it actually emitted the text (the renderer may
    /// still drop it for byte budget). The second tuple element is that record intent — the
    /// `(name, caller block)` to insert on successful emission, or `None` when this render is
    /// already a back-reference / has no caller block to record.
    fn render(
        &self,
        seen_caller_blocks: &HashMap<String, String>,
    ) -> (String, Option<(String, String)>) {
        let mut out = String::with_capacity(self.full_len());
        out.push_str(&self.prefix);
        let record = match seen_caller_blocks.get(&self.name) {
            Some(prev) if *prev == self.caller_block && !self.caller_block.is_empty() => {
                out.push_str(&format!("  - _callers: same as `{}` above_\n", self.name));
                None
            }
            _ => {
                out.push_str(&self.caller_block);
                if self.caller_block.is_empty() {
                    None
                } else {
                    Some((self.name.clone(), self.caller_block.clone()))
                }
            }
        };
        out.push_str(&self.suffix);
        (out, record)
    }
}

/// Build the full annotation block for ONE matched `fn` symbol. Returns `None` to omit the
/// annotation entirely (e.g. byte budget already exhausted) — never an error.
///
/// `byte_budget` is the bytes still available for THIS symbol's annotation: the lesser of
/// the remaining `annotation_sub_budget` and the remaining `search_detail_byte_cap` space,
/// computed by the caller (the two-counter). The full rendered length never exceeds it.
///
/// The caller block is returned UN-deduped (separated from the fixed prefix/suffix). The P2
/// "same as above" collapse is deferred to [`SymbolAnnotation::render`] so it runs in the
/// renderer's emission order over only the symbols actually emitted — see the type doc.
#[allow(clippy::too_many_arguments)]
fn render_symbol_annotation(
    sym: &ExtractedSymbol,
    file_path: &str,
    scan: &ScanResult,
    snapshot: &[ExtractedFile],
    index: &SymbolIndex<'_>,
    navigation_index: &NavigationIndex,
    cfg: &CallerConfig,
    byte_budget: usize,
    root: &Path,
    runtime_state: AnnotationRuntimeState,
    resolver: &super::resolution::SourceResolver<'_>,
    should_list_unresolved: bool,
) -> Option<SymbolAnnotation> {
    if byte_budget == 0 {
        return None;
    }
    let mut prefix = String::new();
    if file_path.ends_with(".rs") {
        if let Some(target_os) = crate::config::get().analysis_target_os.as_deref() {
            prefix.push_str(&format!("  - _Rust analysis target_os={target_os}; static source conditions, not a runtime execution guarantee._\n"));
        }
    }

    // --- Decorator / attribute entry-point label (on-demand source re-read). ---
    let decorators = decorator_lines_above(file_path, sym.range.start_line, root);
    if !decorators.is_empty() {
        prefix.push_str(&format!(
            "  - _framework entry-point candidate (verbatim, approximate):_ `{}`\n",
            decorators.join(" ")
        ));
    }

    // --- Callers (built into its own block so identical repeats can be deduped per file). ---
    let own_def_count = lookup_compatible_candidates(&sym.name, file_path, false, index).len();
    let mut caller_block = String::new();
    let mut used_precise_callers = false;
    let mut precise_caller_entries: HashSet<String> = HashSet::new();
    if let Some(confirmed) = precise_navigation_callers(
        sym,
        file_path,
        snapshot,
        index,
        navigation_index,
        cfg,
        runtime_state,
        root,
        resolver,
    ) {
        let caller_entries = confirmed.entries;
        if confirmed.is_partial {
            caller_block.push_str("  - _Caller resolution hit its callsite or alias budget; proven entries below are retained and additional sites may be unresolved._\n");
        }
        if !caller_entries.is_empty() {
            caller_block.push_str(if cfg.navigation_context_default {
                "  - _callers (tree-sitter precise; import/source resolution confirmed):_\n"
            } else {
                "  - _callers (source/import resolution checked; approximate):_\n"
            });
            for entry in &caller_entries {
                precise_caller_entries.insert(entry.clone());
            }
            for entry in caller_entries.iter().take(cfg.caller_list_cap) {
                caller_block.push_str(&format!("    - {entry}\n"));
            }
            if caller_entries.len() > cfg.caller_list_cap {
                caller_block.push_str(&format!(
                    "    - _… {} more not shown._\n",
                    caller_entries.len() - cfg.caller_list_cap
                ));
            }
            used_precise_callers = true;
        }
    }

    // Source-confirmed identities survive high spelling counts. Only the
    // uncertain name-match fallback is governed by the omission threshold.
    if own_def_count >= cfg.caller_omit_def_threshold {
        if used_precise_callers {
            caller_block.push_str(&format!("  - _Additional name-matching callers omitted: `{}` has {} definitions; proven callers above are retained. Use grep to enumerate remaining sites._\n", sym.name, own_def_count));
        } else {
            caller_block.push_str(&format!(
                "  - _callers omitted: `{}` has {} definitions — attribution ambiguous; use grep \"{}(\" to enumerate call sites_\n",
                sym.name, own_def_count, sym.name
            ));
        }
    } else {
        let is_common = own_def_count >= cfg.common_name_threshold;
        // Map this name's call-site hits to their enclosing fn.
        let mut caller_entries: Vec<String> = Vec::new();
        let mut seen_callers: HashSet<String> = HashSet::new();
        let mut unattributed_hits = 0usize;
        for hit in scan.hits.iter().filter(|h| h.name == sym.name && h.is_call) {
            if !definition_is_compatible(&hit.file_path, hit.is_member_access, file_path, sym) {
                continue;
            }
            // Exclude hits inside ANY same-named `fn` definition's range: a definition header
            // (`fn name(`) classifies as a call, and a call within a same-named body is
            // (self-)recursion — neither is a caller. Covers the symbol's own range and, for
            // common names, every sibling definition.
            if is_within_same_named_fn(hit, &sym.name, index) {
                continue;
            }
            let file = snapshot.iter().find(|f| f.file_path == hit.file_path);
            if super::resolution::supports(&hit.file_path)
                && !file.is_some_and(|file| {
                    resolver.matches_caller(file, &hit.name, hit.line_number, file_path, sym)
                })
            {
                unattributed_hits += 1;
                continue;
            }
            let caller_name = file.and_then(|file| {
                if super::resolution::supports(&file.file_path) {
                    resolver.caller_name(file, hit.line_number)
                } else {
                    enclosing_fn(file, hit.line_number)
                        .map(|enclosing| qualified_name(enclosing, &file.file_path))
                }
            });
            let entry = match caller_name {
                Some(qn) => {
                    format!("{} ({}:{})", qn, hit.file_path, hit.line_number)
                }
                None => {
                    // File absent from snapshot, or line in no symbol range → never drop.
                    format!(
                        "{}:{} (top-level/unindexed)",
                        hit.file_path, hit.line_number
                    )
                }
            };
            if precise_caller_entries.contains(&entry) {
                continue;
            }
            if seen_callers.insert(entry.clone()) {
                caller_entries.push(entry);
            }
        }
        if unattributed_hits > 0 {
            caller_block.push_str(&format!("  - _`{}` has {own_def_count} definitions; {unattributed_hits} name-matching call site(s) not attributed to this declaration._\n", sym.name));
        }
        let scan_truncated = scan.truncated_names.contains(&sym.name);
        if caller_entries.is_empty() {
            if !used_precise_callers {
                // No direct callers: surface non-call references (the dead-code antidote), then
                // always the observation-scope caveat — never a bare "0 callers".
                let mut refs: Vec<String> = Vec::new();
                let mut seen_refs: HashSet<String> = HashSet::new();
                for hit in scan
                    .hits
                    .iter()
                    .filter(|h| h.name == sym.name && !h.is_call)
                {
                    // Exclude references inside same-named definition ranges and import/use lines.
                    if !definition_is_compatible(
                        &hit.file_path,
                        hit.is_member_access,
                        file_path,
                        sym,
                    ) {
                        continue;
                    }
                    if is_within_same_named_fn(hit, &sym.name, index) {
                        continue;
                    }
                    let hit_ext = extension_of(&hit.file_path);
                    if is_import_line(&hit.line_text, hit_ext) {
                        continue;
                    }
                    let entry = format!(
                        "{}:{}: `{}`",
                        hit.file_path,
                        hit.line_number,
                        hit.line_text.trim()
                    );
                    if seen_refs.insert(entry.clone()) {
                        refs.push(entry);
                    }
                    if refs.len() >= cfg.caller_list_cap {
                        break;
                    }
                }
                if refs.is_empty() {
                    caller_block.push_str(&format!("  - {OBSERVATION_SCOPE_CAVEAT}\n"));
                } else {
                    caller_block.push_str(
                "  - _referenced in a non-call position (possible callback / handler registration, approximate):_\n",
            );
                    for r in refs {
                        caller_block.push_str(&format!("    - {r}\n"));
                    }
                }
                // A truncated scan must never read as a confident zero.
                if scan_truncated {
                    caller_block.push_str(
                    "    - _(caller scan hit its per-name hit cap — sites may have been missed)_\n",
                );
                }
            } else if scan_truncated {
                caller_block.push_str(
                    "    - _(caller scan hit its per-name hit cap — additional sites may have been missed)_\n",
                );
            }
        } else {
            let shown = caller_entries.len().min(cfg.caller_list_cap);
            if used_precise_callers {
                caller_block.push_str(
                    "  - _additional callers (fallback name-match; precise resolution did not cover these sites):_\n",
                );
            } else if is_common {
                // Common matched name: a name-match scan cannot tell which definition each
                // site targets — render the list anyway, labeled, instead of suppressing it.
                caller_block.push_str(&format!(
                    "  - _callers (file:line positions exact; name-match attribution approximate — `{}` has {} definitions, call sites may target any of them):_\n",
                    sym.name, own_def_count
                ));
            } else {
                caller_block.push_str(
                    "  - _callers (file:line positions exact; name-match attribution approximate):_\n",
                );
            }
            for entry in caller_entries.iter().take(shown) {
                caller_block.push_str(&format!("    - {entry}\n"));
            }
            if caller_entries.len() > shown {
                caller_block.push_str(&format!(
                    "    - _… {} more not shown._\n",
                    caller_entries.len() - shown
                ));
            }
            if scan_truncated {
                caller_block.push_str(
                    "    - _(caller scan hit its per-name hit cap — list may be incomplete)_\n",
                );
            }
        }
    } // end caller-list branch (skipped when callers are omitted for too-many-defs)

    // --- Callees: depth-1, name-match only. Held in the fixed `suffix` part — never deduped,
    // always rendered after the (possibly back-referenced) caller block.
    //
    // Target-ambiguous callees (a name with ≥ `common_name_threshold` definitions) carry near-zero
    // navigational signal — the agent can't act on "context (8 defs, target ambiguous)". They are
    // SUPPRESSED from the per-name list and collapsed into a single trailing count, so the block
    // shows only disambiguated, actionable callees plus a visible "(+N ambiguous suppressed)" note
    // instead of a wall of unactionable lines. ---
    let mut suffix = String::new();
    let current_file = snapshot.iter().find(|file| file.file_path == file_path);
    let callees: Vec<DiscoveredCallee> = current_file
        .map(|file| {
            discover_callees_with_navigation(
                sym,
                file,
                index,
                runtime_state,
                cfg.navigation_context_default,
                root,
                resolver,
            )
        })
        .unwrap_or_else(|| discover_callees(sym, file_path, index, root));
    if !callees.is_empty() {
        let unresolved = callees.iter().filter(|callee| callee.is_unresolved).count();
        if unresolved > 0 {
            suffix.push_str(&format!("  - _{unresolved} callee(s) unresolved in indexed source; no definition attributed._\n"));
            if should_list_unresolved {
                for callee in callees
                    .iter()
                    .filter(|callee| callee.is_unresolved)
                    .take(cfg.callee_list_cap)
                {
                    suffix.push_str(&format!("    - {} (unresolved)\n", callee.name));
                }
                if unresolved > cfg.callee_list_cap {
                    suffix.push_str(&format!(
                        "    - _… {} more unresolved calls not shown._\n",
                        unresolved - cfg.callee_list_cap
                    ));
                }
            }
        }
        let callees: Vec<_> = callees
            .iter()
            .filter(|callee| !callee.is_unresolved)
            .collect();
        let shown = callees.len().min(cfg.callee_list_cap);
        let mut rendered_lines = String::new();
        let mut ambiguous_suppressed = 0usize;
        let mut has_precise = false;
        for callee in callees.iter().take(shown) {
            let def_count = index.function_definition_count(&callee.name);
            if !callee.is_precise
                && callee.display == callee.name
                && def_count >= cfg.common_name_threshold
            {
                ambiguous_suppressed += 1;
            } else {
                has_precise |= callee.is_precise;
                if callee.is_precise {
                    rendered_lines.push_str(&format!("    - {} (precise)\n", callee.display));
                } else {
                    rendered_lines.push_str(&format!("    - {}\n", callee.display));
                }
            }
        }
        // Only emit the block header when at least one actionable callee or a suppressed-count
        // note will follow it, so a symbol whose callees are all ambiguous doesn't print a bare
        // header. The header still summarizes what was found.
        if !rendered_lines.is_empty() || ambiguous_suppressed > 0 || callees.len() > shown {
            if has_precise {
                suffix.push_str("  - _calls (depth 1, tree-sitter precise where marked; fallback approximate):_\n");
            } else if super::resolution::supports(file_path) {
                suffix.push_str("  - _calls (depth 1, source/import resolution checked):_\n");
            } else {
                suffix.push_str("  - _calls (depth 1, approximate, name-match only):_\n");
            }
            suffix.push_str(&rendered_lines);
            if ambiguous_suppressed > 0 {
                suffix.push_str(&format!(
                    "    - _… {ambiguous_suppressed} ambiguous callee(s) suppressed (multiple defs — use grep to enumerate)._\n"
                ));
            }
            if callees.len() > shown {
                suffix.push_str(&format!(
                    "    - _… {} more not shown._\n",
                    callees.len() - shown
                ));
            }
        }
    }

    let annotation = SymbolAnnotation {
        name: sym.name.clone(),
        prefix,
        caller_block,
        suffix,
    };
    // Budget check against the FULL (un-deduped) length: the render-order dedup only ever
    // emits this length or LESS, so reserving the full length keeps the cap safe. Over budget
    // → degrade to the one-line marker when even that fits (a visible omission, never silent),
    // else drop entirely. Snippets keep priority; never a partial line. The marker is carried
    // as a self-contained `prefix` (no caller block, so it is never deduped or back-referenced).
    if annotation.full_len() > byte_budget {
        if ANNOTATION_OMITTED_MARKER.len() <= byte_budget {
            return Some(SymbolAnnotation {
                name: sym.name.clone(),
                prefix: ANNOTATION_OMITTED_MARKER.to_string(),
                caller_block: String::new(),
                suffix: String::new(),
            });
        }
        return None;
    }
    Some(annotation)
}

/// One matched-file's identity for annotation lookup: its workspace-relative path plus the
/// list of its non-fallback matched `fn` symbols.
pub struct AnnotationRequest<'a> {
    pub file_path: &'a str,
    pub symbols: &'a [ExtractedSymbol],
    /// `symbol_fallback` results are not annotated (ranked in via path/docstring).
    pub is_fallback: bool,
}

/// The opt-in scan/annotation result for a whole detail view. `annotations` maps
/// `(file_path, symbol_start_line)` to its (un-deduped) [`SymbolAnnotation`]. Performs EXACTLY
/// ONE workspace walk across all matched `fn` names of all detail files. Returns `None` on any
/// failure (the caller then renders the un-annotated detail view).
///
/// The P2 caller-block dedup is NOT applied here — it is applied by [`Self::render`] at the
/// renderer's emission point, in render order, over only the symbols actually emitted (the
/// renderer skips summary containers / cap overflow). This is what prevents a "same as `name`
/// above" back-reference whose original block was never emitted (the live A/B dangling defect).
pub struct DetailAnnotations {
    annotations: HashMap<(String, usize), SymbolAnnotation>,
    exact_annotations: HashMap<(String, usize, usize), SymbolAnnotation>,
}

/// Caller-block dedup state owned by the renderer across emitted symbols: `name → already-emitted
/// caller block`. Construct ONE for the whole detail view (shared across files) and thread it
/// through every [`DetailAnnotations::render`] / [`PreparedAnnotation::commit`] call in emission
/// order, so a caller block that repeats across file boundaries collapses to a "same as `name`
/// above" back-reference instead of re-printing (cross-file dedup). The back-reference target is
/// recorded only on actual emission, so it always points at a block already printed earlier in the
/// view regardless of which file printed it.
pub type CallerBlockDedup = HashMap<String, String>;

/// A rendered-but-not-yet-committed annotation: the text to emit plus the dedup record intent.
/// The renderer emits [`Self::text`] only if it fits the byte cap, then calls [`Self::commit`]
/// to record the back-reference target — so a full caller block becomes a back-reference target
/// for later same-named symbols ONLY when it was actually emitted (never when the renderer drops
/// it for budget). This is what keeps every "same as `name` above" pointing at a printed block.
pub struct PreparedAnnotation {
    text: String,
    record: Option<(String, String)>,
}

impl PreparedAnnotation {
    /// The rendered annotation text (full caller block, or a back-reference, plus prefix/suffix).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Commit this annotation's caller block as the back-reference target for later same-named
    /// symbols across the whole detail view. Call ONLY after the text was actually emitted. A no-op
    /// when the render was itself a back-reference or carried no caller block.
    pub fn commit(self, seen: &mut CallerBlockDedup) {
        if let Some((name, block)) = self.record {
            seen.insert(name, block);
        }
    }
}

impl DetailAnnotations {
    pub(crate) fn render_for_symbol(
        &self,
        file_path: &str,
        symbol: &ExtractedSymbol,
        seen: &CallerBlockDedup,
    ) -> Option<PreparedAnnotation> {
        self.exact_annotations
            .get(&(
                file_path.to_string(),
                symbol.range.start_line,
                symbol.range.start_col,
            ))
            .map(|annotation| {
                let (text, record) = annotation.render(seen);
                PreparedAnnotation { text, record }
            })
    }

    /// Live member context: retain depth-one call relations, excluding decorator and
    /// non-call reference payloads. Native search continues to use `render` unchanged.
    pub(crate) fn render_live_relations(
        &self,
        file_path: &str,
        start_line: usize,
        start_col: Option<usize>,
    ) -> Option<String> {
        let annotation = match start_col {
            Some(column) => {
                self.exact_annotations
                    .get(&(file_path.to_string(), start_line, column))
            }
            None => self.annotations.get(&(file_path.to_string(), start_line)),
        };
        annotation.map(|ann| {
            if ann.caller_block.is_empty() && ann.suffix.is_empty() {
                return ANNOTATION_OMITTED_MARKER.to_string();
            }
            let callers = if ann.caller_block.contains("referenced in a non-call position") {
                format!("  - {OBSERVATION_SCOPE_CAVEAT} (non-call references excluded; scan may be capped)\n")
            } else { ann.caller_block.clone() };
            format!("{callers}{}", ann.suffix)
        })
    }

    /// Prepare the annotation for a specific symbol AT ITS EMISSION POINT, deduping its caller
    /// block against the symbols already emitted for this file (`seen`). Returns the prepared
    /// text + record intent, or `None` when the symbol has no annotation. Callers MUST invoke
    /// this exactly once per emitted symbol, in emission order, threading the SAME per-file
    /// `seen` map and calling [`PreparedAnnotation::commit`] after emitting — so the first
    /// EMITTED symbol of a name prints the full caller block and later same-named ones collapse
    /// to a back-reference that always has a real original above it.
    pub fn render(
        &self,
        file_path: &str,
        start_line: usize,
        seen: &CallerBlockDedup,
    ) -> Option<PreparedAnnotation> {
        self.annotations
            .get(&(file_path.to_string(), start_line))
            .map(|ann| {
                let (text, record) = ann.render(seen);
                PreparedAnnotation { text, record }
            })
    }
}

/// Build annotations for every matched `fn` symbol across ALL detail-view result files in a
/// single workspace scan. `available_bytes` is the bytes still free under
/// `search_detail_byte_cap` when annotation begins; this enforces both that overall cap AND
/// the `annotation_sub_budget` (the two-counter, whichever binds first). Snippets keep
/// priority — an annotation that would overflow is dropped, never truncated mid-line.
///
/// Returns `None` on any setup/scan failure so the caller degrades to the un-annotated view.
pub fn annotate_results(
    requests: &[AnnotationRequest<'_>],
    snapshot: &[ExtractedFile],
    cfg: &CallerConfig,
    available_bytes: usize,
    root: &Path,
) -> Option<DetailAnnotations> {
    annotate_results_with_state(
        requests,
        snapshot,
        cfg,
        available_bytes,
        root,
        AnnotationRuntimeState::default(),
    )
}

pub fn annotate_results_with_state(
    requests: &[AnnotationRequest<'_>],
    snapshot: &[ExtractedFile],
    cfg: &CallerConfig,
    available_bytes: usize,
    root: &Path,
    runtime_state: AnnotationRuntimeState,
) -> Option<DetailAnnotations> {
    let resolver = super::resolution::SourceResolver::new(snapshot, root);
    annotate_with_presentation(
        requests,
        snapshot,
        cfg,
        available_bytes,
        root,
        AnnotationPresentation {
            runtime_state,
            should_list_unresolved: true,
            resolver: &resolver,
        },
    )
}

pub(crate) fn annotate_live_results(
    requests: &[AnnotationRequest<'_>],
    snapshot: &[ExtractedFile],
    cfg: &CallerConfig,
    available_bytes: usize,
    root: &Path,
    should_list_unresolved: bool,
    resolver: &super::resolution::SourceResolver<'_>,
) -> Option<DetailAnnotations> {
    annotate_with_presentation(
        requests,
        snapshot,
        cfg,
        available_bytes,
        root,
        AnnotationPresentation {
            runtime_state: AnnotationRuntimeState::default(),
            should_list_unresolved,
            resolver,
        },
    )
}

struct AnnotationPresentation<'a, 'source> {
    runtime_state: AnnotationRuntimeState,
    should_list_unresolved: bool,
    resolver: &'a super::resolution::SourceResolver<'source>,
}

fn annotate_with_presentation(
    requests: &[AnnotationRequest<'_>],
    snapshot: &[ExtractedFile],
    cfg: &CallerConfig,
    available_bytes: usize,
    root: &Path,
    presentation: AnnotationPresentation<'_, '_>,
) -> Option<DetailAnnotations> {
    let AnnotationPresentation {
        runtime_state,
        should_list_unresolved,
        resolver,
    } = presentation;
    let should_trace_navigation_metrics = tracing::enabled!(tracing::Level::DEBUG);
    let annotation_started = should_trace_navigation_metrics.then(Instant::now);
    let test_filter = super::test_code::TestCodeFilter::from_config(root);
    test_filter.retain_snapshot_cache(snapshot);
    let index = build_symbol_index(snapshot, Some(&test_filter));
    let should_build_navigation_index =
        cfg.navigation_context_default && !runtime_state.suppresses_navigation();
    let navigation_index_started =
        (should_trace_navigation_metrics && should_build_navigation_index).then(Instant::now);
    let navigation_index = if should_build_navigation_index {
        build_navigation_index(snapshot)
    } else {
        NavigationIndex::default()
    };
    let mut navigation_metrics = should_trace_navigation_metrics.then(|| {
        let mut metrics = NavigationMetrics {
            navigation_callsite_count: navigation_callsite_count(snapshot),
            navigation_snapshot_bytes: navigation_snapshot_bytes(snapshot),
            navigation_snapshot_load_ms: navigation_index_started
                .map(|started| started.elapsed().as_millis())
                .unwrap_or_default(),
            ..NavigationMetrics::default()
        };
        let receiver_callsite_count = navigation_receiver_callsite_count(snapshot);
        if !cfg.navigation_context_default {
            metrics.record_fallback("feature_disabled", metrics.navigation_callsite_count);
            metrics.navigation_receiver_hint_fallback_count = receiver_callsite_count;
        } else if runtime_state.suppresses_navigation() {
            metrics.record_fallback("runtime_suppressed", metrics.navigation_callsite_count);
            metrics.navigation_receiver_hint_fallback_count = receiver_callsite_count;
        }
        metrics
    });

    // Union of every non-fallback matched `fn` name across all detail files → one scan.
    let mut names: Vec<String> = Vec::new();
    for req in requests {
        if req.is_fallback || !index.can_attribute_calls(req.file_path) {
            continue;
        }
        for sym in req
            .symbols
            .iter()
            .filter(|s| s.kind == "fn" && !test_filter.is_excluded(req.file_path, &s.range))
        {
            names.push(sym.name.clone());
        }
    }
    names.sort();
    names.dedup();
    let scan = scan_workspace(&names, cfg, root, &index)?;

    let mut annotations: HashMap<(String, usize), SymbolAnnotation> = HashMap::new();
    let mut exact_annotations = HashMap::new();
    // Two-counter: the annotation budget is the smaller of the sub-budget and the
    // remaining overall-cap space; both deplete as annotations are reserved. Reservation is
    // against the FULL (un-deduped) length — the render-order dedup only ever emits that or
    // less, so the reserved cap is never exceeded at emission time.
    let mut sub_remaining = cfg.annotation_sub_budget;
    let mut overall_remaining = available_bytes;
    for req in requests {
        if req.is_fallback || !index.can_attribute_calls(req.file_path) {
            continue;
        }
        for sym in req
            .symbols
            .iter()
            .filter(|s| s.kind == "fn" && !test_filter.is_excluded(req.file_path, &s.range))
        {
            let budget = sub_remaining.min(overall_remaining);
            if budget == 0 {
                break;
            }
            if let Some(annotation) = render_symbol_annotation(
                sym,
                req.file_path,
                &scan,
                snapshot,
                &index,
                &navigation_index,
                cfg,
                budget,
                root,
                runtime_state,
                resolver,
                should_list_unresolved,
            ) {
                let reserved = annotation.full_len();
                sub_remaining = sub_remaining.saturating_sub(reserved);
                overall_remaining = overall_remaining.saturating_sub(reserved);
                exact_annotations.insert(
                    (
                        req.file_path.to_string(),
                        sym.range.start_line,
                        sym.range.start_col,
                    ),
                    annotation.clone(),
                );
                annotations.insert(
                    (req.file_path.to_string(), sym.range.start_line),
                    annotation,
                );
            }
        }
    }
    if let Some(metrics) = &mut navigation_metrics {
        let receiver_callsite_count = navigation_receiver_callsite_count(snapshot);
        metrics.navigation_precise_count = annotations
            .values()
            .map(|annotation| {
                annotation.prefix.matches("(precise)").count()
                    + annotation.caller_block.matches("(precise)").count()
                    + annotation
                        .caller_block
                        .matches("tree-sitter precise")
                        .count()
                    + annotation.suffix.matches("(precise)").count()
            })
            .sum();
        if cfg.navigation_context_default && !runtime_state.suppresses_navigation() {
            let fallback_count = metrics
                .navigation_callsite_count
                .saturating_sub(metrics.navigation_precise_count);
            metrics.record_fallback("not_precise", fallback_count);
            metrics.navigation_receiver_hint_precise_count = annotations
                .values()
                .map(|annotation| annotation.suffix.matches("(precise)").count())
                .sum();
            metrics.navigation_receiver_hint_fallback_count = receiver_callsite_count
                .saturating_sub(metrics.navigation_receiver_hint_precise_count);
        }
        metrics.navigation_annotation_ms = annotation_started
            .map(|started| started.elapsed().as_millis())
            .unwrap_or_default();
        metrics.trace();
    }
    Some(DetailAnnotations {
        annotations,
        exact_annotations,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_proven_callers_survive_name_threshold_and_partial_budget() {
        use crate::parser::{CodeExtractor, TreeSitterExtractor};
        let inputs=[
            ("src/lib.rs","mod a; mod b; mod c; mod d; mod e;\nuse crate::a::work;\npub fn first() { work(); }\npub fn second() { work(); }\npub fn foreign() { crate::b::work(); }\n"),
            ("src/a.rs","pub fn work() {}\n"),("src/b.rs","pub fn work() {}\n"),
            ("src/c.rs","pub fn work() {}\n"),("src/d.rs","pub fn work() {}\n"),
            ("src/e.rs","pub fn work() {}\n"),
        ];
        let (_temp, root) = crate::callers::fixtures::write_repo(&inputs);
        let files: Vec<_> = inputs
            .iter()
            .map(|(path, source)| TreeSitterExtractor::new().extract(source, path).unwrap())
            .collect();
        assert_eq!(
            files
                .iter()
                .flat_map(|f| &f.symbols)
                .filter(|s| s.name == "work")
                .count(),
            5
        );
        let target = &files[1];
        let request = [AnnotationRequest {
            file_path: &target.file_path,
            symbols: &target.symbols,
            is_fallback: false,
        }];
        for (budget, partial) in [(1000, false), (1, true)] {
            let cfg = CallerConfig {
                navigation_callsite_budget: budget,
                ..crate::callers::fixtures::cfg()
            };
            let annotations = annotate_results(&request, &files, &cfg, 8192, &root).unwrap();
            let out = annotations
                .render_live_relations(&target.file_path, 1, None)
                .unwrap();
            assert!(out.contains("first (src/lib.rs:3)"), "{out}");
            assert!(!out.contains("foreign ("), "{out}");
            assert!(out.contains("proven callers above are retained"), "{out}");
            if partial {
                assert!(out.contains("hit its callsite or alias budget"), "{out}");
            } else {
                assert!(out.contains("second (src/lib.rs:4)"), "{out}");
            }
        }
    }

    use super::*;
    use crate::callers::fixtures::{cfg, file, has_note, note, render_in_order, sym};
    use std::path::PathBuf;

    fn parsed_file(root: &Path, path: &str) -> ExtractedFile {
        use crate::parser::CodeExtractor;
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        let mut file = crate::parser::TreeSitterExtractor::new()
            .extract(&source, path)
            .unwrap();
        file.symbols.retain(|symbol| symbol.kind == "fn");
        file
    }

    fn repeated_caller_annotations() -> DetailAnnotations {
        let annotations = (1..=3).map(|line| (("t.rs".to_string(), line), SymbolAnnotation {
            name: "tick".to_string(), prefix: String::new(), suffix: String::new(),
            caller_block: "  - _callers (approximate; `tick` has 3 definitions):_\n    - driver (t.rs:5)\n".to_string(),
        })).collect();
        DetailAnnotations {
            annotations,
            exact_annotations: HashMap::new(),
        }
    }

    #[test]
    fn test_caller_line_shows_enclosing_symbol_and_file_line() {
        // `target_fn` is defined in def.rs and called from inside `caller_fn` in use.rs.
        let (_dir, root) = crate::callers::fixtures::write_repo(&[
            ("def.rs", "pub fn target_fn() {\n    let x = 1;\n}\n"),
            ("use.rs", "mod def; use def::target_fn; pub fn caller_fn() {\n    target_fn();\n}\n"),
            (
                "noise.rs",
                "fn noise() {\n    let text = \"한국어 target_fn(\";\n    let raw = r#\"target_fn(\"#;\n    // target_fn();\n    /* target_fn(); */\n    object./* member */target_fn();\n}\n",
            ),
            ("foreign.py", "def unrelated():\n    target_fn()\n"),
        ]);
        let snapshot = vec![parsed_file(&root, "def.rs"), parsed_file(&root, "use.rs")];
        let requests = vec![AnnotationRequest {
            file_path: "def.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "def.rs", 1);
        assert!(
            text.contains("callers"),
            "should have a callers section: {text}"
        );
        assert!(text.contains("caller_fn"), "enclosing symbol name: {text}");
        assert!(
            text.contains("use.rs:2"),
            "file:line of the call site: {text}"
        );
        assert!(text.contains("approximate"), "approximate label: {text}");
        assert!(
            !text.contains("noise.rs"),
            "non-code/member false caller: {text}"
        );
        assert!(
            !text.contains("foreign.py"),
            "cross-language false caller: {text}"
        );
    }

    #[test]
    fn test_callee_depth_one_d_calls_c() {
        // The requester's example: `d` calls `c`. Annotating `d` must list `c` at depth 1.
        let (_dir, root) = crate::callers::fixtures::write_repo(&[(
            "chain.rs",
            "pub fn c() {}\npub fn d() {\n    c();\n    let text = \"ghost(\";\n    let raw = r#\"ghost(\"#;\n    // ghost();\n    /* ghost(); */\n    object.ghost();\n}\nfn ghost() {}\n",
        )]);
        let snapshot = vec![parsed_file(&root, "chain.rs")];
        let requests = vec![AnnotationRequest {
            file_path: "chain.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "chain.rs", 2);
        assert!(text.contains("calls (depth 1"), "callee section: {text}");
        assert!(
            text.contains("- c — chain.rs:1"),
            "callee definition listed: {text}"
        );
        assert!(text.contains("approximate"), "approximate label: {text}");
        assert!(
            !text.contains("ghost —"),
            "unknown member must not link to the same-named function: {text}"
        );
        assert_eq!(
            text.matches("ghost (unresolved)").count(),
            1,
            "only the executable member call is retained: {text}"
        );
        // Literal text is excluded, but interpolation expressions are executable.
        // Exercise both caller and fallback callee consumers with LF and CRLF.
        for (path, source, first_definition) in [
            ("case.tsx", "function target() {}\nfunction ghost() {}\nfunction runner() {\n  return <div>ghost( {target()}</div>;\n}\n", 1),
            ("case.ts", "function target() {}\nfunction ghost() {}\nfunction runner() {\n  const text = `ghost( ${target()}`;\n  const pattern = /ghost()/;\n  /* ghost();\n     ghost(); */\n}\n", 1),
            ("case.py", "def target(): pass\ndef ghost(): pass\ndef runner():\n    text = \"\"\"ghost(\n    ghost(\"\"\"\n    result = f\"ghost( {target()}\"\n    # ghost()\n", 1),
            ("case.rb", "def target; end\ndef ghost; end\ndef runner\n  text = \"ghost( #{target()}\"\n  pattern = /ghost()/\n  # ghost()\nend\n", 1),
            ("case.go", "package example\nfunc target() {}\nfunc ghost() {}\nfunc runner() {\n  text := `ghost(\n  ghost(`\n  _ = text\n  /* ghost(); */\n  target()\n}\n", 2),
        ] {
            for line_ending in ["\n", "\r\n"] {
                let source = source.replace('\n', line_ending);
                let (_dir, root) = crate::callers::fixtures::write_repo(&[(path, &source)]);
                let snapshot = vec![parsed_file(&root, path)];
                let index = build_symbol_index(&snapshot, None);
                let callees = discover_callees(&snapshot[0].symbols[2], path, &index, &root);
                assert_eq!(callees.iter().map(|callee| callee.name.as_str()).collect::<Vec<_>>(), vec!["target"], "{path}: {callees:?}");
                let requests = [AnnotationRequest { file_path: path, symbols: &snapshot[0].symbols, is_fallback: false }];
                let annotations = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
                let target = note(&annotations, path, first_definition);
                assert!(target.contains("runner ("), "actual/interpolated call missing in {path}: {target}");
                let ghost = note(&annotations, path, first_definition + 1);
                assert!(ghost.contains("no direct caller observed"), "literal/comment reference in {path}: {ghost}");
            }
        }
    }

    #[test]
    fn test_receiver_hint_is_unresolved_without_confirmed_type_resolution() {
        let (_dir, root) = crate::callers::fixtures::write_repo(&[("nav.ts",
            "class User { save() {} }\nclass File { save() {} }\nfunction run(user: User) { user.save(); }\n")]);
        let snapshot = vec![parsed_file(&root, "nav.ts")];
        let requests = [AnnotationRequest {
            file_path: "nav.ts",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let mut config = cfg();
        config.navigation_context_default = true;
        let ann = annotate_results(&requests, &snapshot, &config, 100_000, &root).unwrap();
        let text = note(&ann, "nav.ts", 3);
        assert!(text.contains("save (unresolved)"), "{text}");
        assert!(
            !text.contains("save —"),
            "unconfirmed receiver has a definition link: {text}"
        );
        assert!(
            !text.contains("(precise)"),
            "unconfirmed receiver marked precise: {text}"
        );
    }

    #[test]
    fn test_navigation_precise_callee_disabled_by_default() {
        let (_dir, root) = crate::callers::fixtures::write_repo(&[(
            "nav.ts",
            "function save() {}\nfunction run() { save(); console.consoleLog(); }\n",
        )]);
        let snapshot = vec![parsed_file(&root, "nav.ts")];
        let requests = [AnnotationRequest {
            file_path: "nav.ts",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "nav.ts", 2);
        assert!(
            text.contains("save — nav.ts:1"),
            "confirmed local definition: {text}"
        );
        assert!(
            !text.contains("(precise)"),
            "default-off navigation marked precise: {text}"
        );
        assert!(
            text.contains("consoleLog (unresolved)"),
            "unindexed executable call missing: {text}"
        );
    }

    #[test]
    fn test_precise_callers_do_not_include_unresolved_name_matches() {
        let (_dir, root) = crate::callers::fixtures::write_repo(&[
            ("user.ts", "export function save() {}\n"),
            ("file.ts", "export function save() {}\n"),
            (
                "caller.ts",
                "import { save } from \"./user\";\nexport function run() {\n  save();\n}\n",
            ),
            ("other.ts", "export function run2() {\n  save();\n}\n"),
        ]);
        let snapshot: Vec<_> = ["user.ts", "file.ts", "caller.ts", "other.ts"]
            .iter()
            .map(|path| parsed_file(&root, path))
            .collect();
        let requests = [AnnotationRequest {
            file_path: "user.ts",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let mut config = cfg();
        config.navigation_context_default = true;
        let ann = annotate_results(&requests, &snapshot, &config, 100_000, &root).unwrap();
        let text = note(&ann, "user.ts", 1);
        assert!(
            text.contains("tree-sitter precise"),
            "confirmed import missing: {text}"
        );
        assert!(
            text.contains("run (caller.ts:3)"),
            "confirmed caller missing: {text}"
        );
        assert!(
            !text.contains("run2 (other.ts:2)"),
            "unresolved name attributed to a definition: {text}"
        );
        assert!(
            text.contains("not attributed"),
            "unresolved observations should remain visible: {text}"
        );
    }

    #[test]
    fn test_qualified_caller_method_from_owner() {
        // The caller is a Rust method `Engine::run` calling free `helper`. The owner field
        // (Phase A) must render the caller as `Engine::run`, exercising the owner path.
        let (_dir, root) = crate::callers::fixtures::write_repo(&[(
            "engine.rs",
            "pub fn helper() {}\nimpl Engine {\n    pub fn run(&self) {\n        helper();\n    }\n}\n",
        )]);
        let snapshot = vec![parsed_file(&root, "engine.rs")];
        let requests = vec![AnnotationRequest {
            file_path: "engine.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "engine.rs", 1); // helper's annotation lists its callers.
        assert!(
            text.contains("Engine::run"),
            "caller rendered with owner-qualified name: {text}"
        );
    }

    #[test]
    fn test_callee_and_caller_ambiguity_labels_for_common_name() {
        // `make` has two fn defs → as a callee it is target-ambiguous, so it is SUPPRESSED from
        // the callee list and collapsed into a "(N ambiguous callee(s) suppressed)" note (the
        // ambiguous callee line carries no actionable target). As a MATCHED name, its callers are
        // still rendered with an attribution-ambiguity label — never suppressed. The sibling
        // definition's own header line must not appear as a caller (it classifies as `make(` but
        // sits inside a same-named def range).
        let (_dir, root) = crate::callers::fixtures::write_repo(&[(
            "amb.rs",
            "pub fn make() {}\npub fn make() {}\npub fn user() {\n    make();\n}\n",
        )]);
        let snapshot = vec![parsed_file(&root, "amb.rs")];
        // Callee side: annotate `user`, which calls `make` (2 defs).
        let req_user = vec![AnnotationRequest {
            file_path: "amb.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&req_user, &snapshot, &cfg(), 100_000, &root).unwrap();
        let user_text = note(&ann, "amb.rs", 3);
        assert!(
            !user_text.contains("make (2 defs, target ambiguous)"),
            "ambiguous callee line is suppressed, not rendered: {user_text}"
        );
        assert!(
            user_text.contains("callee(s) unresolved"),
            "suppressed ambiguous callees collapse into a visible count note: {user_text}"
        );
        assert!(
            user_text.contains("make (unresolved)"),
            "unresolved call names remain visible: {user_text}"
        );
        assert!(
            !user_text.contains("make —"),
            "unresolved calls have no definition link: {user_text}"
        );
        // Caller side: annotating `make` itself → callers listed with an ambiguity label.
        let make_text = note(&ann, "amb.rs", 1);
        assert!(
            make_text.contains("has 2 definitions"),
            "common matched name → attribution-ambiguity label: {make_text}"
        );
        assert!(
            !make_text.contains("user (amb.rs:4)"),
            "an ambiguous call must not be attributed to either definition: {make_text}"
        );
        assert!(
            !make_text.contains("amb.rs:2"),
            "sibling definition header must not be listed as a caller: {make_text}"
        );
    }

    #[test]
    fn test_non_call_reference_label_for_zero_caller_fn() {
        // `handler` is never called as `handler(`, only registered via a callback pass.
        let (_dir, root) = crate::callers::fixtures::write_repo(&[
            ("h.rs", "pub fn handler() {}\n"),
            (
                "reg.rs",
                "pub fn setup() {\n    register(\"x\", handler);\n}\n",
            ),
        ]);
        let snapshot = vec![
            file("h.rs", vec![sym("handler", "fn", 1, 1, None)]),
            file("reg.rs", vec![sym("setup", "fn", 1, 3, None)]),
        ];
        let requests = vec![AnnotationRequest {
            file_path: "h.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "h.rs", 1);
        assert!(
            text.contains("non-call position"),
            "non-call reference label instead of bare 0 callers: {text}"
        );
        assert!(
            text.contains("reg.rs:2"),
            "the raw reference line:line: {text}"
        );
        assert!(
            !text.contains("0 callers"),
            "never a bare 0 callers: {text}"
        );
    }

    #[test]
    fn test_decorator_entry_point_label() {
        // A Python `@app.route(...)` decorator directly above the matched fn is surfaced
        // verbatim as a framework entry-point candidate.
        let (_dir, root) = crate::callers::fixtures::write_repo(&[(
            "app.py",
            "@app.route(\"/health\")\ndef health():\n    return ok\n",
        )]);
        let snapshot = vec![file("app.py", vec![sym("health", "fn", 2, 3, None)])];
        let requests = vec![AnnotationRequest {
            file_path: "app.py",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "app.py", 2);
        assert!(
            text.contains("framework entry-point candidate"),
            "entry-point label: {text}"
        );
        assert!(
            text.contains("@app.route(\"/health\")"),
            "verbatim decorator text: {text}"
        );
    }

    #[test]
    fn test_zero_caller_shows_observation_scope_caveat_never_bare_zero() {
        // `lonely` has no callers and no references anywhere → observation-scope caveat.
        let (_dir, root) =
            crate::callers::fixtures::write_repo(&[("lone.rs", "pub fn lonely() {}\n")]);
        let snapshot = vec![file("lone.rs", vec![sym("lonely", "fn", 1, 1, None)])];
        let requests = vec![AnnotationRequest {
            file_path: "lone.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "lone.rs", 1);
        assert!(
            text.contains("no direct caller observed"),
            "observation-scope caveat: {text}"
        );
        assert!(
            text.contains("scope: indexed source eligible for caller lookup"),
            "observation scope identifies the eligible indexed source: {text}"
        );
        assert!(
            !text.contains("0 callers"),
            "never a bare 0 callers: {text}"
        );
    }

    #[test]
    fn test_annotation_respects_byte_budget() {
        // With a tiny available budget, no annotation should be emitted (snippets keep
        // priority; an over-budget annotation is dropped, not truncated mid-line).
        let (_dir, root) = crate::callers::fixtures::write_repo(&[
            ("def.rs", "pub fn target_fn() {}\n"),
            ("use.rs", "pub fn caller_fn() {\n    target_fn();\n}\n"),
        ]);
        let snapshot = vec![
            file("def.rs", vec![sym("target_fn", "fn", 1, 1, None)]),
            file("use.rs", vec![sym("caller_fn", "fn", 1, 3, None)]),
        ];
        let requests = vec![AnnotationRequest {
            file_path: "def.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        // available_bytes = 10 → far below any annotation length → dropped entirely.
        let ann = annotate_results(&requests, &snapshot, &cfg(), 10, &root).unwrap();
        assert!(
            !has_note(&ann, "def.rs", 1),
            "annotation dropped when over budget"
        );
    }

    #[test]
    fn test_scan_failure_isolation_returns_none() {
        // A non-existent root makes the walk yield nothing; the scan itself still succeeds
        // (degrades to empty), so annotation is produced from snapshot-only data with the
        // observation-scope caveat — never a panic, never an error. This proves the
        // never-exit contract: the pipeline degrades rather than failing the response.
        let bogus = PathBuf::from("/nonexistent/path/for/codemap/test");
        let snapshot = vec![file("x.rs", vec![sym("foo", "fn", 1, 1, None)])];
        let requests = vec![AnnotationRequest {
            file_path: "x.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &bogus);
        // Did not panic / error out; produced a degraded annotation.
        let ann = ann.unwrap();
        let text = note(&ann, "x.rs", 1);
        assert!(
            text.contains("no direct caller observed"),
            "degraded to observation-scope caveat: {text}"
        );
    }

    #[test]
    fn test_fallback_results_not_annotated() {
        // A `symbol_fallback` result (ranked via path/docstring) is never annotated.
        let (_dir, root) =
            crate::callers::fixtures::write_repo(&[("def.rs", "pub fn target_fn() {}\n")]);
        let snapshot = vec![file("def.rs", vec![sym("target_fn", "fn", 1, 1, None)])];
        let requests = vec![AnnotationRequest {
            file_path: "def.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: true, // fallback → skip
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        assert!(!has_note(&ann, "def.rs", 1), "fallback not annotated");
    }

    #[test]
    fn test_self_recursion_not_counted_as_caller() {
        // A fn calling itself: the call inside its own body must not be reported as a caller.
        let (_dir, root) = crate::callers::fixtures::write_repo(&[(
            "rec.rs",
            "pub fn recurse(n: u32) {\n    if n > 0 { recurse(n - 1); }\n}\n",
        )]);
        let snapshot = vec![file("rec.rs", vec![sym("recurse", "fn", 1, 3, None)])];
        let requests = vec![AnnotationRequest {
            file_path: "rec.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        let text = note(&ann, "rec.rs", 1);
        // The only `recurse(` call is on its own definition line range → excluded → caveat.
        assert!(
            text.contains("no direct caller observed"),
            "self-recursion excluded from callers: {text}"
        );
    }

    #[test]
    fn test_p2_dedup_full_block_before_back_reference_in_render_order() {
        // Three same-named (< omit-threshold, so the list renders) `tick` fns sharing one caller
        // `driver`. The FIRST emitted in render order must carry the full caller block; the next
        // two collapse to "same as `tick` above". A back-reference must never appear before its
        // original — the live A/B dangling defect.
        let ann = repeated_caller_annotations();
        // Emit in line order 1,2,3 (the renderer's outermost-first order).
        let rendered = render_in_order(&ann, "t.rs", &[1, 2, 3]);
        assert!(
            rendered[0].contains("driver (t.rs:5)") && !rendered[0].contains("same as"),
            "first emitted carries the full caller block: {:?}",
            rendered[0]
        );
        assert!(
            rendered[1].contains("same as `tick` above")
                && rendered[2].contains("same as `tick` above"),
            "later same-named symbols collapse to a back-reference: {rendered:?}"
        );
        // Integrity: no "same as above" is emitted before the full block (defect 1).
        let first_back_ref = rendered.iter().position(|t| t.contains("same as"));
        let first_full = rendered
            .iter()
            .position(|t| t.contains("driver (t.rs:5)") && !t.contains("same as"));
        assert!(
            first_full.is_some() && first_full < first_back_ref,
            "the original full block precedes every back-reference: {rendered:?}"
        );
    }

    #[test]
    fn test_p2_dedup_promotes_full_block_when_first_symbol_is_skipped() {
        // The renderer suppresses some symbols (summary containers, cap overflow). When the
        // FIRST same-named symbol in line order is skipped, the next EMITTED one must render the
        // full block — never a dangling back-reference (defects 1 & 3). Emulated by simply not
        // emitting the L1 symbol: render only L2 then L3.
        let ann = repeated_caller_annotations();
        // L1 is "skipped" by the renderer (e.g. a summary container) → emit only L2, L3.
        let rendered = render_in_order(&ann, "t.rs", &[2, 3]);
        assert!(
            rendered[0].contains("driver (t.rs:5)") && !rendered[0].contains("same as"),
            "first EMITTED symbol (L2) carries the full block, not a dangling back-ref: {:?}",
            rendered[0]
        );
        // And the common-name label is restored on that promoted full block (defect 3): with 3
        // defs (≥ common_name_threshold 2) the label must carry the def count.
        assert!(
            rendered[0].contains("has 3 definitions"),
            "common-name label present on the promoted full block: {:?}",
            rendered[0]
        );
        assert!(
            rendered[1].contains("same as `tick` above"),
            "the later one back-references the now-emitted original: {rendered:?}"
        );
    }

    #[test]
    fn test_p3_omit_line_renders_in_render_order() {
        // A name with ≥ caller_omit_def_threshold (5) defs: every symbol's caller block is the
        // omission note. The FIRST emitted must show the full omit line (defect 2); the rest
        // back-reference it (they are byte-identical). Even if the renderer skips the first
        // line-order symbol, the omit line must still appear on the first EMITTED one.
        let body = (0..6).map(|_| "pub fn poll() {}\n").collect::<String>();
        let (_dir, root) = crate::callers::fixtures::write_repo(&[("p.rs", &body)]);
        let symbols: Vec<_> = (0..6)
            .map(|i| sym("poll", "fn", i + 1, i + 1, None))
            .collect();
        let snapshot = vec![file("p.rs", symbols)];
        let requests = vec![AnnotationRequest {
            file_path: "p.rs",
            symbols: &snapshot[0].symbols,
            is_fallback: false,
        }];
        let ann = annotate_results(&requests, &snapshot, &cfg(), 100_000, &root).unwrap();
        // Skip the L1 symbol (renderer suppressed it); first emitted is L2.
        let rendered = render_in_order(&ann, "p.rs", &[2, 3, 4]);
        assert!(
            rendered[0].contains("callers omitted: `poll` has 6 definitions"),
            "P3 omit line on the first emitted symbol even when L1 is skipped: {:?}",
            rendered[0]
        );
        assert!(
            rendered[0].contains("use grep \"poll(\""),
            "omit line keeps the grep pointer form: {:?}",
            rendered[0]
        );
        assert!(
            rendered[1].contains("same as `poll` above"),
            "subsequent omit blocks (byte-identical) collapse to a back-reference: {rendered:?}"
        );
    }
}
