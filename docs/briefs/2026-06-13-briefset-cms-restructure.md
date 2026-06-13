# Brief Set: codemap-search structural restructure (folder layout + separation of concerns)

## Purpose
- Restructure `apps/codemap-search` so each module owns one concern: dissolve the two god modules (`mcp.rs` ~1,350 lines mixing protocol/lifecycle/tools/rendering; `parser.rs` 2,795 lines mixing types/queries/helpers/extraction), align `tools/` with the 5-tool product surface, and centralize language knowledge so the planned top-20 tree-sitter language expansion costs one file + one registry entry + one Cargo dependency per language.
- Every child is behavior-preserving (child 07 adds tests only); the set is sequenced so each child leaves the crate compiling and green.

## Child Briefs
- [ ] `docs/briefs/2026-06-13-refactor-cms-restructure-01-workspace.md` — Create workspace.rs for shared fs/path infrastructure; exists because reverse dependencies (`tools → mcp`, `index → mcp`) and duplicated path logic block every later boundary.
- [ ] `docs/briefs/2026-06-13-refactor-cms-restructure-02-parser-types.md` — Split parser domain types and tokenizer into parser/; exists because every subsystem depends on the data types, not the tree-sitter machinery, and `range_strictly_contains` must leave codemap before the callers split.
- [ ] `docs/briefs/2026-06-13-refactor-cms-restructure-03-callers-split.md` — Split callers.rs into pipeline-boundary modules; exists because caller scanning, callee discovery, shared symbol index, and the render/byte-budget protocol are four concerns cohabiting one 944-line (non-test) file.
- [ ] `docs/briefs/2026-06-13-refactor-cms-restructure-04-index-folder.md` — Group index subsystem and introduce EngineSupervisor; exists because engine/indexer/watcher are one coupled subsystem and engine supervision currently lives in the MCP server where it does not belong.
- [ ] `docs/briefs/2026-06-13-refactor-cms-restructure-05-tools-mcp.md` — Extract search/overview into tools/, convert mcp.rs to mcp/; exists because the tool folder must match the 5-tool product surface and the MCP module must contain only MCP contract code.
- [ ] `docs/briefs/2026-06-13-refactor-cms-restructure-06-codemap-split.md` — Split codemap.rs into view/summary/tree modules; exists because the shared summary/tree core dominates the views and a dead field (`original_files`) needs removal.
- [ ] `docs/briefs/2026-06-13-test-cms-restructure-07-extract-snaps.md` — Add per-language extraction snapshot tests; exists because the lang/ migration (children 08/09) converts ~17 language-branch sites and the current test net cannot catch a silently flipped per-language flag (owner explicitly opted into adding these tests).
- [ ] `docs/briefs/2026-06-13-refactor-cms-restructure-08-lang-registry.md` — Introduce lang/ LanguageSpec registry and convert parser hooks; exists because inline per-extension branch chains do not scale to the top-20 language roadmap.
- [ ] `docs/briefs/2026-06-13-refactor-cms-restructure-09-lang-adopt.md` — Migrate callers language knowledge and SOURCE_EXTENSIONS to the registry; exists because language tables outside `lang/` (callers separators/import-lines, the hand-maintained extension array) would otherwise drift per added language.

## Execution Order
- Wave 1: `2026-06-13-refactor-cms-restructure-01-workspace` runs alone.
- Wave 2: `2026-06-13-refactor-cms-restructure-02-parser-types` runs alone.
- Wave 3: `2026-06-13-refactor-cms-restructure-03-callers-split` runs alone.
- Wave 4: `2026-06-13-refactor-cms-restructure-04-index-folder` runs alone.
- Wave 5: `2026-06-13-refactor-cms-restructure-05-tools-mcp` runs alone.
- Wave 6: `2026-06-13-refactor-cms-restructure-06-codemap-split` runs alone.
- Wave 7: `2026-06-13-test-cms-restructure-07-extract-snaps` runs alone.
- Wave 8: `2026-06-13-refactor-cms-restructure-08-lang-registry` runs alone.
- Wave 9: `2026-06-13-refactor-cms-restructure-09-lang-adopt` runs alone.
- Strictly one child at a time — every child edits `apps/codemap-search/src/lib.rs` and most edit `apps/codemap-search/src/main.rs` (see Conflict Hotspots). Child line-number references describe pre-restructure file state; later children must re-locate symbols by name (grep) rather than trusting shifted line numbers.

## Dependencies
- `02-parser-types` depends on nothing structural but precedes `03` (callers split updates the `range_strictly_contains` call site to its new `parser` home).
- `03-callers-split` depends on `02-parser-types` (imports the relocated types path).
- `04-index-folder` depends on `01-workspace` (path-key helpers must leave `index.rs` before the engine/ranking split).
- `05-tools-mcp` depends on `04-index-folder` (ToolContext borrows `EngineSupervisor`) and `01-workspace` (path helpers already out of `mcp.rs`) and `03-callers-split` (search render code imports the `callers/` annotate API at its new paths).
- `06-codemap-split` depends on `02-parser-types` (`range_strictly_contains` already relocated out of codemap).
- `08-lang-registry` depends on `07-extract-snaps` (golden snapshots are the behavior net for hook conversion) and `02-parser-types` (parser/ directory exists).
- `09-lang-adopt` depends on `08-lang-registry` (registry exists), `03-callers-split` (migration source is `callers/mod.rs`), and `01-workspace` (`SOURCE_EXTENSIONS` lives in `workspace.rs`).
- `07-extract-snaps` has no hard dependency (tests against the public extract API) but is sequenced at wave 7 to sit immediately before its consumer.

## Parallelization
- No two children may run in parallel — all touch `apps/codemap-search/src/lib.rs` module declarations, and the chain `04 → 05` plus `03 → 05` shares `mcp.rs`/`callers` surfaces.
- `07-extract-snaps` is the only child that could theoretically run early in parallel (it only adds files under `tests/`), but keep it sequential to avoid golden churn from any extraction-adjacent move landing mid-generation.

## Conflict Hotspots
- `apps/codemap-search/src/lib.rs` — every child edits module declarations; one child at a time.
- `apps/codemap-search/src/main.rs` — children 01/02/04/05/06 update imports/wiring.
- `apps/codemap-search/src/mcp.rs` — edited by 01 (path helpers out), 04 (supervision out), then dissolved by 05; strict 01 → 04 → 05 order.
- `apps/codemap-search/src/callers.rs` (→ `callers/` after 03) — edited by 01, 02, 03, 09.
- `apps/codemap-search/src/workspace.rs` (created by 01) — edited again by 09 (extension-set derivation).

## Shared Constraints
- Behavior preservation is the set-wide contract: every child except 07 must leave all tool outputs, CLI outputs, and index state byte-identical; 07 adds tests without touching `src/`.
- Run `cargo test` (workspace `apps/codemap-search`) after each child; the e2e suite is the cross-child safety net (black-box binary, immune to module reshuffling).
- No compatibility re-export façades: new module roots (`parser/mod.rs`, `index/mod.rs`, `callers/mod.rs`, `codemap/mod.rs`) re-export their submodule items as the single canonical path; `main.rs` and test imports are updated directly.
- Keep the existing `mod.rs` directory-module style (consistent with current `tools/mod.rs`); do not mix in the `foo.rs`-beside-`foo/` style.
- No formatting/lint sweeps beyond the moved code; no new external dependencies anywhere in the set.
- Child briefs cite pre-restructure line numbers as discovery hints; verify by symbol name when executing later children.

## Global Acceptance Criteria
- [ ] All nine children checked off, each having ended with `cargo build` warning-clean and `cargo test` green.
- [ ] Final layout matches the agreed tree: `workspace.rs`, `parser/{mod,types,tokenize}.rs`, `index/{mod,engine,ranking,indexer,watcher,supervisor}.rs`, `callers/{mod,scan,symbols,callees,annotate}.rs`, `codemap/{mod,summary,tree}.rs`, `tools/{mod,overview,read,find,grep}.rs` + `tools/search/{mod,render}.rs`, `mcp/{mod,protocol}.rs`, `lang/` (7 language files + `c_family/`); flat `mcp.rs`, `parser.rs`, `index.rs`, `indexer.rs`, `watcher.rs`, `callers.rs`, `codemap.rs` no longer exist.
- [ ] `grep -rn "crate::mcp::" apps/codemap-search/src/tools apps/codemap-search/src/index` returns zero matches (reverse dependencies eliminated and never reintroduced).
- [ ] Golden snapshot suite green with zero regenerated goldens after children 08/09 (extraction behavior preserved through the lang migration).
- [ ] Benchmark re-run is **not** a completion criterion (owner decision — deferred; may be run optionally after the set completes).

## Open Questions
- None — owner decisions resolved during planning: lang work split into two children (08/09); snapshot tests explicitly approved (child 07); benchmark re-run excluded from completion criteria; no commit-precondition child (owner deemed it unnecessary to encode).
