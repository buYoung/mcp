# [feat] Add NavigationFile types, TS query extraction, and callee switch

## Work Type
feat

## Current State (As-Is)
- As of branch `main` (recent commit `13deb03`), caller/callee discovery in `apps/codemap-search/src/callers/callees.rs` (L17–63) uses a plain text scan: it searches function-body text for `identifier(` patterns with no tree-sitter structural awareness.
- `apps/codemap-search/src/parser/types.rs` defines `ExtractedFile`, `ExtractedSymbol`, and `CodeRange` but contains no navigation types (`NavigationFile`, `CallSite`, `ImportEntry`, `LocalBinding`, `ReferenceSite` are absent).
- `apps/codemap-search/src/index/engine.rs` sets `EXTRACTION_FORMAT_VERSION = "v7-owner-tokens-indexed"` (L123); no navigation field exists in the serialized `ExtractedFile`, so no version bump has occurred for this feature.
- `apps/codemap-search/src/lang/typescript.rs` embeds the TypeScript tree-sitter query as an inline Rust string constant `TS_QUERY_STR` (L10–64). No `tags.scm` or `navigation.scm` file exists; the `queries/` directory does not exist in the repository at all.
- `apps/codemap-search/src/lang/mod.rs` defines the `LanguageSpec` trait with no `navigation_query` hook or navigation-related method.
- `apps/codemap-search/src/parser/mod.rs` runs a single `QueryCursor` pass using only the symbols query (`TreeSitterExtractor::extract`, L34–288). It has no concat-query path for navigation captures.
- `apps/codemap-search/src/tools/search/mod.rs` calls `annotate_results` (L462) with no engine-state arguments; no `AnnotationRuntimeState` is passed.
- The text scan in `callees.rs` produces false positives from identifiers appearing in string literals, comments, definition headers, and property access expressions — these are indistinguishable from actual call sites without structural parsing.
- `navigation: None` vs `Some(NavigationFile { calls: vec![], ... })` distinction does not exist; there is no way to tell whether navigation extraction was attempted or simply absent.

## Desired Outcome (To-Be)
- `apps/codemap-search/src/parser/types.rs` defines `NavigationFile`, `CallSite`, `ReferenceSite`, `LocalBinding`, `ImportEntry`, and `ImportKind` as specified in the design doc §5.
- `ExtractedFile` gains an `#[serde(default, skip_serializing_if = "Option::is_none")] pub navigation: Option<NavigationFile>` field; `navigation: None` means extraction was not run or failed, `Some(empty)` means extraction succeeded with no observations — this distinction is enforced and preserved.
- `apps/codemap-search/queries/typescript/tags.scm` (proposed) captures the minimum TypeScript/TSX definition and reference nodes using standard tags-compatible capture names (`@definition.*`, `@reference.*`); it compiles against both the TypeScript grammar and the TSX grammar.
- `apps/codemap-search/queries/typescript/navigation.scm` (proposed) captures call sites, receivers, local bindings, and import entries using `@nav.*` and `@local.*` capture names as defined in the design doc §3; it compiles against both grammars.
- `apps/codemap-search/src/lang/typescript.rs` exposes a navigation query getter and wires `navigation.scm` into the extractor; `apps/codemap-search/src/lang/mod.rs` adds the corresponding `LanguageSpec` hook.
- `apps/codemap-search/src/parser/mod.rs` runs a single concat `Query::new(symbols_query + navigation_query)` pass per file; `tags.scm` is NOT included in the runtime pass — it is a compile/fixture gate only.
- `apps/codemap-search/src/index/engine.rs` bumps `EXTRACTION_FORMAT_VERSION` in the same commit that introduces the `navigation` field serialization, forcing a one-time full re-index.
- `apps/codemap-search/src/callers/callees.rs` uses `navigation.calls` from the indexed `ExtractedFile` as the primary callee source; it falls back to the existing text-scan path when `navigation` is `None`, when the snapshot is potentially stale (disk file is newer than index), or when the language is not navigation-enabled.
- `apps/codemap-search/src/tools/search/mod.rs` constructs and passes an `AnnotationRuntimeState` (with `is_warming`, `has_refresh_error`, `is_dead_or_stale` fields) to `annotate_results` so callee precise suppression can be applied in warming/stale/error conditions.
- False positives from string literals, comments, definition headers, and plain property accesses are eliminated for TypeScript/TSX files that have a successful navigation extraction.

## Scope
### In Scope
- Define `NavigationFile`, `CallSite`, `ReferenceSite`, `LocalBinding`, `ImportEntry`, `ImportKind` in `src/parser/types.rs`.
- Add `pub navigation: Option<NavigationFile>` to `ExtractedFile` with correct serde attributes.
- Bump `EXTRACTION_FORMAT_VERSION` in `src/index/engine.rs` in the same commit.
- Create `queries/typescript/tags.scm` (proposed) with minimum TypeScript/TSX definition/reference tags-compatible captures.
- Create `queries/typescript/navigation.scm` (proposed) with minimum `@nav.call`, `@nav.call.name`, `@nav.call.receiver`, `@nav.import.*`, `@local.definition`, `@local.type`, `@local.value_type` captures.
- Create the TypeScript and TSX navigation fixtures (design doc §4 fixture requirement): `tests/fixtures/navigation/typescript/basic.ts` (proposed), `tests/fixtures/navigation/typescript/expected.tags.json` (proposed), `tests/fixtures/navigation/typescript/expected.navigation.json` (proposed), and `tests/fixtures/navigation/tsx/basic.tsx` (proposed), `tests/fixtures/navigation/tsx/expected.tags.json` (proposed), `tests/fixtures/navigation/tsx/expected.navigation.json` (proposed). Child 02 is the sole creator of these directories and files; downstream children (e.g. child 04, child 06) only update the existing TypeScript `expected.navigation.json`.
- Add navigation query getter to `src/lang/typescript.rs` and corresponding `LanguageSpec` trait hook in `src/lang/mod.rs`.
- Implement the concat single-pass extraction in `src/parser/mod.rs`; route `@nav.*` / `@local.*` captures into `NavigationFile`.
- Switch `src/callers/callees.rs` to use `navigation.calls` as primary; preserve text-scan fallback for `None` navigation, stale-disk conditions, and unsupported languages.
- Introduce the `AnnotationRuntimeState` struct (child 02 is the sole introducer; design doc §6 defines its three fields `is_warming`, `has_refresh_error`, `is_dead_or_stale`) for callee precise suppression, and thread it from `src/tools/search/mod.rs` into `annotate_results`. Child 04 later reuses and extends this same struct for the caller direction — it must not redefine it.
- Add callee lookup helper in `src/callers/symbols.rs` for navigation-based candidate resolution.

### Out of Scope
- [hard] `navigation_callsite_budget`, `navigation_context_default`, `navigation_store_references` config keys — these belong to child 04 (`import-caller`) per decision 6.
- [hard] `NavigationIndex` and `calls_by_name` reverse-index — these are a child 04 concern; child 02 does not build the reverse caller index.
- [hard] `ImportEntry` alias resolution and source-hint-based candidate lookup — import alias handling is child 04 scope.
- [hard] same-file function priority ranking — child 03 (`same-file-prio`) scope.
- [hard] receiver/owner hint inference — child 05 (`receiver-hint`) scope.
- [hard] `scope_id`-based lexical shadowing — child 06 (`lexical-scope`) scope.
- [deferred] Python, Go, Rust, Java/Kotlin, C/C++ navigation queries — child 02 only wires TypeScript/TSX.
- [deferred] `ReferenceSite` population in the runtime pass — design doc §5 permits skipping or capping references in the first implementation; do not store `ReferenceSite` unless the concat query naturally produces them at zero extra cost.
- [hard] Do not touch `src/config.rs` — no new config keys in this child.
- [hard] Do not create or modify `queries/typescript/symbols.scm` — that file is the output of child 01 (`scm-extract`); child 02 depends on it being present from child 01.

## Constraints
- `EXTRACTION_FORMAT_VERSION` bump and `ExtractedFile.navigation` field addition must land in exactly one commit together; splitting them leaves the sidecar in an inconsistent state.
- The runtime extractor must remain a single `Query::new(...)` + single `QueryCursor` pass per file. Do not add a second query pass for navigation — concat the query strings instead.
- `tags.scm` must NOT be included in the runtime concat pass; it is a compile/fixture validation gate only (design doc §3).
- `navigation: None` vs `Some(NavigationFile { calls: vec![], ... })` semantics must be preserved exactly — `#[serde(default)]` on the navigation field allows old index JSON to deserialize without error, but must not silently produce `Some(empty)` for files that were never navigation-extracted.
- When the snapshot's indexed file is potentially stale (disk mtime is newer than indexed snapshot), `callees.rs` must fall back to the live-disk text scan, not use the `navigation.calls` from the stale index.
- Precise callee suppression (i.e., not using navigation results even when `navigation.calls` is present) must be applied when `AnnotationRuntimeState.is_warming == true`, `has_refresh_error == true`, or `is_dead_or_stale == true`.
- This child depends on child 01 (`scm-extract`) having already created `queries/typescript/symbols.scm` (proposed in child 01). The concat pass in `src/parser/mod.rs` reads `symbols.scm` via `include_str!`; child 02 must not inline the symbols query string again.

## Related Files / Entry Points
- `apps/codemap-search/src/parser/types.rs` (existing) — add all new navigation types here; `ExtractedFile` modification starts at its struct definition.
- `apps/codemap-search/src/index/engine.rs` (existing) — `EXTRACTION_FORMAT_VERSION` constant at L123; bump value here in the same commit as the types change.
- `apps/codemap-search/src/parser/mod.rs` (existing) — `TreeSitterExtractor::extract` at L34–288; add concat-query path and `@nav.*`/`@local.*` capture routing here.
- `apps/codemap-search/src/lang/typescript.rs` (existing) — holds `TS_QUERY_STR` inline at L10–64; add navigation query getter referencing `navigation.scm` via `include_str!`.
- `apps/codemap-search/src/lang/mod.rs` (existing) — `LanguageSpec` trait definition; add `navigation_query() -> Option<&'static str>` or equivalent hook.
- `apps/codemap-search/src/callers/callees.rs` (existing) — `discover_callees` text scan at L17–63; switch to `navigation.calls` primary path with stale-disk fallback.
- `apps/codemap-search/src/callers/symbols.rs` (existing) — `SymbolIndex` and `build_symbol_index`; add callee candidate lookup helper for navigation-based resolution.
- `apps/codemap-search/src/tools/search/mod.rs` (existing) — `annotate_results` call at L462; `is_warming()` check at L390; construct `AnnotationRuntimeState` here and thread it into `annotate_results`.
- `apps/codemap-search/queries/typescript/tags.scm` (proposed) — new file; TypeScript/TSX tags-compatible definition/reference gate query.
- `apps/codemap-search/queries/typescript/navigation.scm` (proposed) — new file; TypeScript/TSX runtime navigation capture query.
- `apps/codemap-search/queries/typescript/symbols.scm` (proposed) — created by child 01; child 02 reads it via `include_str!` in the concat pass; do not recreate.
- `apps/codemap-search/tests/fixtures/navigation/typescript/` (proposed) — new directory created by child 02; holds `basic.ts`, `expected.tags.json`, `expected.navigation.json`. Child 02 is the sole creator; downstream children only update `expected.navigation.json`.
- `apps/codemap-search/tests/fixtures/navigation/tsx/` (proposed) — new directory created by child 02; holds `basic.tsx`, `expected.tags.json`, `expected.navigation.json` for the TSX grammar (design doc §4 requires both TypeScript and TSX fixtures).
- `docs/briefs/2026-06-25-briefset-nav-layer-v2.md` (proposed) — parent brief for this briefset; see for execution order and set-level acceptance criteria.

## Side Effect Checkpoints
- [ ] All existing symbol/literal extraction fixture results remain identical after the concat-query path change — the `@symbol.*` and `@literal.*` capture routing must not regress.
- [ ] `EXTRACTION_FORMAT_VERSION` bump triggers a full re-index on next startup; confirm that sidecar files from the previous version are detected as stale and reindexed rather than silently loaded with `navigation: None`.
- [ ] `callers/scan.rs` text-scan path (`scan_workspace`) is still invoked for languages without navigation support and for warming/stale/error states; confirm no regression in caller coverage for non-TypeScript files.
- [ ] `annotate_results` signature change (adding `AnnotationRuntimeState` parameter) must be updated at all call sites in `src/tools/search/mod.rs` and any other callers.
- [ ] The concat query string must compile successfully against both `LANGUAGE_TYPESCRIPT` and `LANGUAGE_TSX` grammars — a compile failure in either grammar must not panic at runtime; it must disable navigation for that grammar and fall back gracefully.
- [ ] `callees.rs` text-scan fallback fires correctly when `navigation` is `None` (no extraction attempted) — do not confuse this with `Some(NavigationFile { calls: vec![] })` (extraction ran, zero calls observed); the fallback trigger is `None` only.
- [ ] `is_warming()` state from `EngineSupervisor` (L154) and `Indexer` (L84) is correctly forwarded into `AnnotationRuntimeState.is_warming`; verify with warming state set to true that callee results do not flip to navigation-based precise.

## Acceptance Criteria
- [ ] `cargo build` succeeds with no new warnings on the `apps/codemap-search` crate after all changes.
- [ ] `tags.scm` and `navigation.scm` compile successfully via `Query::new(...)` against both the TypeScript grammar and the TSX grammar; a compile failure produces a logged warning and disables navigation for that grammar, not a panic.
- [ ] Concat single-pass extraction produces identical `symbols` and `literals` fields in `ExtractedFile` compared to pre-change extraction on the same TypeScript source fixture.
- [ ] A TypeScript source file with at least one function call produces `navigation: Some(NavigationFile { calls: [CallSite { name: ..., .. }], .. })` after extraction.
- [ ] A TypeScript source file with no function calls produces `navigation: Some(NavigationFile { calls: vec![], .. })` — not `None`.
- [ ] A file for a language without navigation support produces `navigation: None` after extraction.
- [ ] String literals containing `name(` patterns (e.g., `const s = "foo()"`) do NOT appear as `CallSite` entries in `NavigationFile.calls`.
- [ ] Comment lines containing `name(` patterns do NOT appear as `CallSite` entries.
- [ ] Function definition headers (e.g., `function foo()`) do NOT appear as `CallSite` entries.
- [ ] `discover_callees` returns results consistent with the old text-scan path when `navigation` is `None` or when `is_dead_or_stale` is true — no regression in callee count for those cases.
- [ ] When `AnnotationRuntimeState.is_warming == true`, callee discovery does not attempt navigation-based precise resolution regardless of whether `navigation.calls` is populated.
- [ ] `EXTRACTION_FORMAT_VERSION` value in `src/index/engine.rs` is a new string (not `"v7-owner-tokens-indexed"`) and differs from the previous version.
- [ ] An `ExtractedFile` JSON produced before the version bump deserializes successfully with `navigation: None` (via `#[serde(default)]`) — no deserialization panic or error.
- [ ] The fixture directories `tests/fixtures/navigation/typescript/` and `tests/fixtures/navigation/tsx/` exist, each containing `basic.ts`/`basic.tsx`, `expected.tags.json`, and `expected.navigation.json`; the `basic.ts` and `basic.tsx` extraction output matches the respective `expected.tags.json` and `expected.navigation.json` exactly.

## Open Questions
- None — implementation choices are bounded by the design doc §5 type definitions, the `(proposed)` token rules, and the constraints above. The two user-owned decisions that could have affected this child (decision 1: merge vs split stages 1+2; decision 6: config keys placement) are resolved via the recommended defaults in `decisions.md`.
