# [test] Add per-language extraction snapshot tests before lang/ migration

## Work Type
test

## Current State (As-Is)
- The tree-sitter extraction semantics in `apps/codemap-search/src/parser/mod.rs` (post child 02; originally `parser.rs`) are marked frozen in code comments, but the behavior-preservation net for the upcoming `lang/` migration (children 08/09) is coarse: parser in-file unit tests (~610 lines) assert selected fields on small inline snippets, and the e2e suite asserts tool-level output — neither pins the **full** `ExtractedFile` output per language.
- Children 08/09 will convert ~17 inline language-branch sites (is_test/is_exported/is_deprecated/docstring/owner/query selection) into `LanguageSpec` hooks — a wide regression surface where a silently flipped flag (e.g. `is_exported` on one Kotlin node kind) would pass existing tests.
- Supported languages today: Rust, Python, TypeScript (ts/tsx, also js/jsx via the same query), Go, Java, Kotlin, C, C++, GAS assembly — grammar crates pinned in `apps/codemap-search/Cargo.toml` (lines 19-38).
- `ExtractedFile` is `serde`-serializable (the `parse` CLI subcommand already prints it as pretty JSON via `main.rs` `Commands::Parse`).

## Desired Outcome (To-Be)
- A fixture corpus under `apps/codemap-search/tests/fixtures/extract/` (proposed): one source file per supported extension (`sample.rs`, `sample.py`, `sample.ts`, `sample.tsx`, `sample.js`, `sample.jsx`, `sample.go`, `sample.java`, `sample.kt`, `sample.c`, `sample.cpp`, `sample.hpp`, `sample.s`) — js/jsx included deliberately because the lang/ migration changes their grammar/query routing even though they share the TS query today — each exercising the language's branch-sensitive constructs: functions/methods, types with owners (impl/receiver/class/out-of-line C++ members), test-flagged symbols, exported vs private symbols, deprecated symbols, docstrings/doc comments, string literals.
- A snapshot test (proposed `apps/codemap-search/tests/extract_snapshots.rs`) that runs `TreeSitterExtractor::extract` on every fixture and compares pretty-printed `ExtractedFile` JSON against committed golden files (`apps/codemap-search/tests/fixtures/extract/golden/*.json`), with an env-var regeneration path (e.g. `UPDATE_SNAPSHOTS=1`) for intentional updates.
- Golden files generated from the working tree's behavior at the time this child executes (i.e. after children 01-06 of the parent briefset, none of which touch extraction) — the snapshots **document** current output; they are not corrections.

## Scope
### In Scope
- Fixture files, golden JSON files, the snapshot test harness, a short README in the fixture directory explaining the regeneration flow.
### Out of Scope
- [hard] No changes to extraction behavior — if a fixture reveals a bug, record it in the brief set's parent Open Questions instead of fixing it here; snapshots pin current behavior, bugs included.
- [hard] No new external dev-dependencies (e.g. `insta`) — `serde_json` is already a dependency; plain golden-file comparison with a readable diff on mismatch is sufficient.
- [deferred] Fixtures for languages added in the future — each new language added after the `lang/` migration brings its own fixture as part of its own task.
- [hard] This suite pins extraction (`TreeSitterExtractor::extract`) only. Callers-side language behavior (separators, import-line filtering) is pinned by the callers in-file tests + e2e suite, and the `SOURCE_EXTENSIONS` membership is pinned by child 09's exact-membership equality test — do not duplicate those nets here.

## Related Files / Entry Points
- `docs/briefs/2026-06-13-briefset-cms-restructure.md` — execution-management parent; this child is wave 7 of 9 and immediately precedes the lang-migration children (08/09) it protects.
- `apps/codemap-search/src/parser.rs` — extraction entry (`TreeSitterExtractor::extract`); after child 02 this is `apps/codemap-search/src/parser/mod.rs` (proposed) — adapt to whichever state the repo is in.
- `apps/codemap-search/tests/e2e_tests.rs` — existing integration-test layout to mirror (test target conventions, helpers location `apps/codemap-search/tests/e2e/`).
- `apps/codemap-search/Cargo.toml` — confirm no dev-dependency additions are needed.
- `apps/codemap-search/tests/fixtures/extract/` (proposed) — fixture corpus home.

## Side Effect Checkpoints
- [ ] Fixture files are excluded from the crate's own index/walk behavior in other tests (they live under `tests/`, which the walker's test runs do not treat as production source — verify no existing test walks `tests/fixtures/`).
- [ ] Snapshot test is deterministic across platforms (no absolute paths, no timestamp, no hash-map ordering in serialized output — `ExtractedFile` serializes `Vec`s, verify field order stability).
- [ ] `cargo test` wall-time stays reasonable (single-digit seconds added; tree-sitter parsing of ~11 small fixtures is cheap).

## Acceptance Criteria
- [ ] Every supported extension (rs, py, ts, tsx, js, jsx, go, java, kt, c, cpp, hpp, s) has a fixture and a committed golden file.
- [ ] Each fixture exercises at minimum: one test-flagged symbol, one exported symbol, one non-exported symbol, one owned symbol (method/member), one docstring, one string literal, one deprecated symbol — each item applying only where the language supports the construct (e.g. GAS assembly has no docstrings/test markers/deprecation; cover what exists and list the omissions in the fixture README).
- [ ] `cargo test` passes with the new snapshot test green against current behavior.
- [ ] Deleting any single golden file makes the suite fail (the harness detects missing goldens rather than skipping).

## Open Questions
- None — the no-new-dependency constraint and the pin-current-behavior policy were locked during plan review; fixture content is a bounded implementation choice.
