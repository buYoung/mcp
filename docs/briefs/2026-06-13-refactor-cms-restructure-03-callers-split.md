# [refactor] Split callers.rs into pipeline-boundary modules

## Work Type
refactor

## Current State (As-Is)
- `apps/codemap-search/src/callers.rs` is 1,557 lines (944 non-test, ~239 comment lines); it mixes two independent analysis pipelines plus a rendering/protocol half in one file.
- Caller pipeline: `ScanHit` (line 60), `ClassifySink` (line 81), `ScanResult` (line 153), `escape_name` (line 166), `scan_workspace` (line 180) — one combined-regex grep scan over the workspace.
- Attribution helpers: `is_within_same_named_fn` (line 287), `enclosing_fn` (line 301).
- Callee pipeline: `discover_callees` (line 419) re-reads the symbol's body from disk and intersects with the snapshot's global `fn_names` — it does **not** consume `ScanResult`; `callee_display` (line 504), `is_ident_start` (line 467).
- Cross-cutting shared state: `SymbolIndex` (line 473) + `build_symbol_index` (line 482), consumed at lines 288, 505, 631, 758, 892; `CallerConfig` (line 32), built by the MCP server (search arm) and consumed at `scan_workspace` and the render half.
- Render/protocol half (~440 lines): `SymbolAnnotation` + dedup back-reference rules (line 543), `render_symbol_annotation` (line 606, assembles caller block at ~630 and callee suffix at ~749 in one function), `AnnotationRequest` (line 802), `DetailAnnotations` (line 818), `PreparedAnnotation` (line 834; its `commit` contract at ~848-853 is fulfilled by the server-side renderer's render→emit→commit sequence in `mcp.rs`), `annotate_results` (line 885, byte-budget accounting).
- Language/io helpers (44 lines total): `qualified_name` (line 328), `extension_of` (line 341), `is_import_line` (line 351, per-extension branches), `read_workspace_file` (line 375), `decorator_lines_above` (line 386).
- In-file tests at lines 945-1557 share fixtures (`sym`/`file`/`cfg` at ~959-995, `write_repo` at ~1087-1098) across helper unit tests and `annotate_results` pipeline tests.

## Behavior Contract
- Locked: search detail caller/callee annotation output — caller lists, callee suffixes, ambiguity labels, "no direct caller" caveats, dedup back-references, and byte-budget truncation are byte-identical for identical input.
- Contract artifacts: callers in-file tests (moved, not rewritten), e2e suite `apps/codemap-search/tests/e2e_tests.rs` (search detail assertions).
- Verification: `cargo test` passes; one manual `search` call on this repo diffed before/after.

## Desired Outcome (To-Be)
- `callers.rs` becomes `apps/codemap-search/src/callers/` (proposed) with pipeline-boundary files:
  - `mod.rs` — `CallerConfig`, public API re-exports (`annotate_results`, `AnnotationRequest`, `DetailAnnotations`, `CallerConfig`), and two small helper groups kept here because 44 lines is too little for their own files: the language helpers (`qualified_name`, `extension_of`, `is_import_line` — these later migrate to `lang/` in child 09) and the io helpers (`read_workspace_file`, `decorator_lines_above` — these are callers-specific annotation I/O and stay in `callers/` permanently; they do not migrate to `lang/`).
  - `scan.rs` — `ScanHit`, `ClassifySink`, `ScanResult`, `escape_name`, `scan_workspace` (~215 lines).
  - `symbols.rs` — `SymbolIndex`, `build_symbol_index`, `enclosing_fn`, `is_within_same_named_fn` (snapshot lookup + attribution; the cross-cutting state gets an explicit home).
  - `callees.rs` — `discover_callees`, `is_ident_start`, `callee_display` (~70 lines).
  - `annotate.rs` — `SymbolAnnotation`, `render_symbol_annotation`, `AnnotationRequest`, `DetailAnnotations`, `PreparedAnnotation`, `annotate_results` (~440 lines).
- `ScanResult` remains the single-producer/single-consumer interface between `scan.rs` and `annotate.rs`.
- Shared test fixtures move to a `#[cfg(test)]` fixture module inside `callers/`; per-file tests sit with the code they test.

## Scope
### In Scope
- Mechanical split along the boundaries above; visibility adjustments (`pub(crate)`/`pub(super)`) as needed; consumer import updates (server search arm).
### Out of Scope
- [hard] No output format changes, no cap/threshold changes, no new annotation features.
- [hard] Do not split caller vs callee into separate top-level modules — the boundary is the pipeline stage (scan/symbols/callees/annotate), confirmed against the actual call graph during plan review.
- [deferred] Migrating `qualified_name` separator and `is_import_line` per-language branches into `LanguageSpec` — child 09.

## Related Files / Entry Points
- `docs/briefs/2026-06-13-briefset-cms-restructure.md` — execution-management parent; this child is wave 3 of 9, executed after children 01 (workspace) and 02 (parser types). Cited line numbers describe the pre-restructure tree — re-locate by symbol name if they have shifted.
- `apps/codemap-search/src/callers.rs` — the file being split; read the module-level doc comment (lines 1-31) first, it documents the never-fails contract.
- `apps/codemap-search/src/mcp.rs` — consumer: builds `CallerConfig` and calls `annotate_results` in the search arm; render-side fulfills `PreparedAnnotation::commit`.
- `apps/codemap-search/src/lib.rs` — `pub mod callers;` resolves to the directory unchanged.

## Side Effect Checkpoints
- [ ] The render→emit→commit dedup sequence between server renderer and `PreparedAnnotation` still holds (search two symbols sharing a caller block; verify "same as above" back-reference appears).
- [ ] `caller_context=false` and config `caller_context_default` paths still short-circuit correctly.
- [ ] Scan-cap and byte-budget behavior unchanged (annotation never fails the search — `None` fallback intact).
- [ ] No circular imports introduced between `scan.rs`/`symbols.rs`/`annotate.rs`.

## Acceptance Criteria
- [ ] `callers/` contains exactly `mod.rs`, `scan.rs`, `symbols.rs`, `callees.rs`, `annotate.rs`.
- [ ] No file in `callers/` exceeds ~500 non-test lines (annotate.rs ~440 is the ceiling case).
- [ ] `cargo test` passes; callers test count unchanged.
- [ ] Search detail output on a fixed query against this repo is byte-identical before/after (manual diff). Default query: `spawn_indexer` (stable symbol exercising caller lists and callee suffixes); any deterministic query that triggers caller annotation is acceptable.

## Open Questions
- None — split boundaries were validated against producer/consumer analysis of `ScanResult`, `SymbolIndex`, and `CallerConfig` during two adversarial review rounds.
