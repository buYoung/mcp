# [refactor] Create workspace.rs for shared fs/path infrastructure

## Work Type
refactor

## Current State (As-Is)
- Shared filesystem/path infrastructure is scattered across layers, creating reverse dependencies onto the MCP server module.
- `canonicalize_path_lenient` is defined in `apps/codemap-search/src/mcp.rs` (around line 57, `pub(crate)`) and consumed by `apps/codemap-search/src/tools/mod.rs:126`, `apps/codemap-search/src/tools/find.rs:55` and `:92`, and `apps/codemap-search/src/index.rs:293` — i.e. `tools → mcp` and `index → mcp` reverse dependencies.
- `is_safe_path` in `apps/codemap-search/src/mcp.rs` (around line 87) and `resolve_within_cwd` in `apps/codemap-search/src/tools/mod.rs:110-134` implement the same three-step logic (lexical collapse → `canonicalize_path_lenient` → `starts_with(cwd)`) as duplicates.
- Walker/filter infrastructure lives in `apps/codemap-search/src/tools/mod.rs`: `SOURCE_EXTENSIONS` (line 24), `is_source_extension` (line 80), `build_walker`, `is_minified_bundle`, `read_source_for_parse`. It is consumed crate-wide: `index.rs:320/346/385/666`, `callers.rs:222/236`, `watcher.rs:324`, `main.rs:126-137`, `benchmark.rs:62` — lower layers depend on the `tools/` (product-surface) module for plumbing.
- Path display/key helpers live inside the index engine: `normalize_relative_path` (`index.rs:262`), `relative_index_path` (`index.rs:280`), `stored_index_key` (`index.rs:293`). `callers.rs` (around line 203, inside `scan_workspace`) duplicates display-path normalization to match `relative_index_path` output.

## Behavior Contract
- Locked: all five MCP tool outputs (search/overview/read/find/grep) byte-identical for identical inputs; path canonicalization, walk filtering (gitignore/.codemapignore/EXCLUDED_DIRS), and index key computation behave exactly as before.
- Contract artifacts: existing in-file unit tests (`index.rs` `mod tests`, `tools` tests if present) and the black-box e2e suite `apps/codemap-search/tests/e2e_tests.rs` (spawns the binary over stdio).
- Verification: `cargo test` in `apps/codemap-search` passes unchanged; no test modified except import paths.

## Desired Outcome (To-Be)
- New module `apps/codemap-search/src/workspace.rs` (proposed) owns: `canonicalize_path_lenient`, the consolidated safe-path check (single function replacing `is_safe_path` + `resolve_within_cwd`), `build_walker`, `SOURCE_EXTENSIONS` + `is_source_extension`, `is_minified_bundle`, `read_source_for_parse`, `normalize_relative_path`, `relative_index_path`, `stored_index_key`.
- No module under `src/` references `crate::mcp::` for path/fs helpers; `tools → mcp` and `index → mcp` edges are gone.
- `callers.rs` display-path duplication (around line 203) is replaced by a call into `workspace.rs`.
- Dependency direction documented in a module-level comment: constants (e.g. excluded dirs, max indexed bytes) are owned by `workspace.rs`; `config.rs` may reference them (config → workspace one-way); `build_walker`'s runtime `config::get()` access is an accepted exception (global singleton).

## Scope
### In Scope
- Moving the functions/constants listed above into `workspace.rs` and updating all call sites (`mcp.rs`, `tools/*`, `index.rs`, `callers.rs`, `watcher.rs`, `main.rs`, `benchmark.rs`).
- Physically moving the shared walk/limit constants into `workspace.rs` as well — the excluded-dirs list and the max-indexed-file-bytes limit currently referenced by `config.rs` defaults (around `config.rs:242-243`); `config.rs` then references them from `workspace.rs` (config → workspace one-way).
- Consolidating `is_safe_path` and `resolve_within_cwd` into one function with both call sites migrated.
### Out of Scope
- [hard] No behavior changes to walk filtering, canonicalization, or index keys — pure relocation + duplicate consolidation.
- [hard] Do not move MCP argument-coercion helpers (`get_arg`, `arg_bool`, `arg_usize`, `lenient_usize`, `arg_required_str` in `tools/mod.rs:297-361`) — they are tool-arg parsing, not fs infrastructure; they stay in `tools/mod.rs`.
- [deferred] Deriving `SOURCE_EXTENSIONS` from a language registry — handled by child 09 (`lang` adoption).

## Related Files / Entry Points
- `docs/briefs/2026-06-13-briefset-cms-restructure.md` — execution-management parent; this child is wave 1 of 9 (the "child 09" referenced under Out of Scope is the ninth child of this set).
- `apps/codemap-search/src/workspace.rs` (proposed) — new home; create first, move in dependency-free order (constants → walker → path helpers).
- `apps/codemap-search/src/tools/mod.rs` — source of walker/extension infrastructure; first edit site.
- `apps/codemap-search/src/mcp.rs` — source of `canonicalize_path_lenient` / `is_safe_path`.
- `apps/codemap-search/src/index.rs` — source of `normalize_relative_path` / `relative_index_path` / `stored_index_key`; consumer updates at lines 293/320/346/385/666.
- `apps/codemap-search/src/lib.rs` — add `pub mod workspace;`.

## Side Effect Checkpoints
- [ ] `callers.rs` workspace scan still produces identical display paths after deduplication removal (compare a search detail output before/after on this repo).
- [ ] Incremental indexing (`index_files_changed`, `refresh_paths`) still matches stored keys — no full reindex triggered by key-format drift.
- [ ] CLI subcommands (`parse`, `codemap`, `index`, `search`, `benchmark`) still compile and run — `main.rs:126-137` uses the moved walker helpers.
- [ ] `overview` tool path-safety rejection behavior unchanged (single consolidated function now serves the former `is_safe_path` call site).

## Acceptance Criteria
- [ ] `grep -rn "crate::mcp::" apps/codemap-search/src/tools apps/codemap-search/src/index.rs` returns zero matches.
- [ ] Exactly one safe-path implementation exists in the crate (the consolidated function in `workspace.rs`).
- [ ] `cargo test` in `apps/codemap-search` passes with no test logic edits (import-path updates only).
- [ ] `cargo build` emits no new warnings.

## Open Questions
- None — relocation targets and the consolidation decision were locked during plan review; no user-owned choices remain.
