# [refactor] Split codemap.rs into view/summary/tree modules, remove dead field

## Work Type
refactor

## Current State (As-Is)
- `apps/codemap-search/src/codemap.rs` is 904 lines (599 non-test, 86 comment lines) holding the overview view generation.
- Shared core (~230 lines) dominates: `is_significant_symbol` (lines 96-107), `summarize_file` (lines 111-130), `build_directory_summaries` (lines 137-160), and the directory tree folding renderer (lines 162-327 — a self-contained algorithm with its own vocabulary documented at lines 187-233, input is `Vec<DirectorySummary>` only).
- View-specific code is comparatively small: `RootCodemap` Display (lines 329-381), `FolderCodemap` Display (lines 383-440), `DetailsCodemap` (lines 442-472), llms-txt view (lines 587-597), constructors (lines 507-584). Root and Folder views both call the shared tree renderer (call sites at lines 345 and 411); `is_significant_symbol` is used by all three views.
- Dead field: `original_files` is declared at lines 41 and 62, populated at lines 523 and 573, and **never read anywhere in the crate or tests** (verified by exhaustive grep); the adjacent comment ("Store reference to original files if needed", line 40) admits speculative design.
- `range_strictly_contains` has already been moved to `parser/types.rs` by child 02.
- A per-view split (root.rs/folder.rs/detail.rs) was evaluated and rejected during plan review: it would create 30-100-line files orbiting a 200+-line shared core, increasing the files touched per change.

## Behavior Contract
- Locked: overview tool output (root/folder/detail views) and `codemap --format llms-txt` CLI output byte-identical for identical snapshots.
- Contract artifacts: codemap in-file tests (lines 602-904), e2e suite overview assertions.
- Verification: `cargo test`; manual diff of `overview` MCP response and `codemap-search codemap` CLI output before/after.

## Desired Outcome (To-Be)
- Directory `apps/codemap-search/src/codemap/` (proposed):
  - `mod.rs` — view structs (`RootCodemap`, `FolderCodemap`, `DetailsCodemap`), Display impls, constructors, llms-txt view, `CodemapGenerator` (~300 lines).
  - `summary.rs` — `ExtractedSymbolSummary`, `ExtractedFileSummary`, `DirectorySummary`, `is_significant_symbol`, `summarize_file`, `build_directory_summaries`.
  - `tree.rs` — the folding directory-tree renderer (lines 162-327) with its vocabulary comment block intact.
- `original_files` field removed from both structs along with its two population sites.
- In-file tests move with the code they test.

## Scope
### In Scope
- The three-file split, dead-field removal, consumer import updates (overview tool, `main.rs` codemap subcommand).
### Out of Scope
- [hard] No view format changes — markdown output is pinned by the behavior contract.
- [hard] Do not split per-view (root.rs/folder.rs/detail.rs) — rejected boundary, shared core dominates.
- [hard] The CLI codemap file-collection loop stays in `main.rs` (codemap stays a pure snapshot→markdown transform with no I/O).

## Related Files / Entry Points
- `apps/codemap-search/src/codemap.rs` — split source; read the tree renderer's vocabulary comment (lines 187-233) before moving it.
- `apps/codemap-search/src/main.rs` — `Commands::Codemap` arm consumes `CodemapGenerator` (around lines 110-179).
- `apps/codemap-search/src/lib.rs` — `pub mod codemap;` resolves to directory unchanged.

## Side Effect Checkpoints
- [ ] Overview MCP tool output unchanged (uses the indexer snapshot path).
- [ ] All four CLI codemap modes (root, `--path` folder, `--path` file detail, `--format llms-txt`) produce identical output.
- [ ] `original_files` removal does not break any constructor caller (only the two internal population sites exist).
- [ ] Tree-folding edge cases (anchor/leaf/junction/terminal-group) covered by existing tests still pass after the move.

## Acceptance Criteria
- [ ] `codemap/` contains exactly `mod.rs`, `summary.rs`, `tree.rs`.
- [ ] `grep -rn "original_files" apps/codemap-search/src` returns zero matches.
- [ ] `cargo test` passes; codemap test count unchanged.
- [ ] Manual output diff (overview + CLI codemap) is byte-identical.

## Open Questions
- None — split boundaries and the dead-field removal were confirmed by two independent reviews with exhaustive-grep evidence.
