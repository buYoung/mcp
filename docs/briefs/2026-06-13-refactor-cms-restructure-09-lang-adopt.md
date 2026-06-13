# [refactor] Migrate callers language knowledge and SOURCE_EXTENSIONS to lang registry

## Work Type
refactor

## Current State (As-Is)
- After child 08, `lang/` owns parser-side language knowledge, but two consumers still hold their own language tables:
- `callers` language helpers (in `callers/mod.rs` after child 03; originally `callers.rs`): `qualified_name` separator selection (per-extension `match` — `::` for Rust, `.` for Python/TS/Go/Java/Kotlin, etc.; originally `callers.rs:328-339`) and `is_import_line` (per-extension import/include/use prefix matching, originally `callers.rs:351-370`, already grown C/C++ and asm arms).
- `SOURCE_EXTENSIONS` is a hand-maintained literal array (in `workspace.rs` after child 01; originally `tools/mod.rs:24`) consumed by `is_source_extension` call sites in `index` (engine walk filter), `callers` (scan filter), `main.rs` (CLI codemap collection), and `benchmark.rs`. Adding a language currently requires editing this list separately from the parser — a drift hazard the registry eliminates.

## Behavior Contract
- Locked: caller/callee annotation output (qualified names, import-line filtering) byte-identical; the effective set of indexed/scanned extensions identical — the registry-derived list must equal the current `SOURCE_EXTENSIONS` array exactly.
- Contract artifacts: callers in-file tests (including `is_import_line` and `qualified_name` cases), golden snapshot suite (unchanged — extraction untouched), e2e suite.
- Verification: `cargo test`; assert-equality check (temporary or test) between the old literal array and the registry-derived set before deleting the literal.

## Desired Outcome (To-Be)
- `LanguageSpec` gains two hooks: `qualified_name_separator() -> &'static str` and `is_import_line(&self, line: &str) -> bool`; each language file implements its current behavior verbatim.
- `callers` resolves the spec via the registry (extension already available at both call sites) and delegates; the per-extension `match` arms in `callers/mod.rs` are deleted.
- `SOURCE_EXTENSIONS` becomes a registry-derived static (once-initialized union of every spec's `extensions()`), preserving the exact current membership; `is_source_extension` keeps its signature so the four consumer sites are untouched.
- End state: adding language #10 touches exactly one new `lang/` file, one registry entry, and one Cargo dependency — no edits in `callers`, `workspace`, `index`, or `tools`.

## Scope
### In Scope
- The two new trait hooks + per-language implementations, callers delegation, registry-derived extension set, removal of the hand-maintained array and the callers `match` arms.
### Out of Scope
- [hard] No new languages, no behavior changes to annotation output or walk filtering.
- [hard] Unknown-extension fallback behavior in `qualified_name`/`is_import_line` (the current default arm) must be preserved for extensions without a registered spec.
- [deferred] Migrating non-language walk policy (minified-bundle detection, excluded dirs) into specs — that is workspace policy, not language knowledge; it stays in `workspace.rs`.

## Related Files / Entry Points
- `apps/codemap-search/src/callers.rs` — current home of `qualified_name` (line 328) and `is_import_line` (line 351); after child 03 these live in `apps/codemap-search/src/callers/mod.rs` (proposed).
- `apps/codemap-search/src/tools/mod.rs` — current home of `SOURCE_EXTENSIONS` (line 24) and `is_source_extension` (line 80); after child 01 these live in `workspace.rs`.
- `apps/codemap-search/src/lang/mod.rs` (proposed) — exists after child 08; trait extension + registry derivation point.

## Side Effect Checkpoints
- [ ] Registry-derived extension set is exactly equal to the previous literal array (no extension gained or lost — `h`/`hh`/`hxx`/`cc`/`cxx`/`kts`/`S` variants are easy to drop accidentally).
- [ ] Caller annotation on a multi-language repo unchanged (qualified-name separators and import-line filtering per language verified via existing callers tests).
- [ ] Walk/index filtering unchanged: a full reindex of this repo indexes the same file count before/after.
- [ ] Default behavior for unregistered extensions (e.g. a stray `.md` hitting `qualified_name` paths) unchanged.

## Acceptance Criteria
- [ ] Zero per-extension `match` arms for separators or import lines remain outside `lang/`.
- [ ] The literal `SOURCE_EXTENSIONS` array is deleted; the derived set passes an exact-membership equality test against the documented previous list.
- [ ] `cargo test` and e2e pass; golden snapshots untouched.
- [ ] `lang/mod.rs` doc comment's add-a-language recipe is now fully true (verified by inspection: no language tables outside `lang/`).

## Open Questions
- None — hook shapes mirror existing functions one-to-one; the deferred boundary (walk policy stays in workspace) was locked during plan review.
