# [refactor] Split parser domain types and tokenizer into parser/ module

## Work Type
refactor

## Current State (As-Is)
- `apps/codemap-search/src/parser.rs` is a single 2,795-line file mixing shared domain types with tree-sitter extraction machinery.
- Domain types live at `parser.rs:9-62`: `CodeRange`, `SymbolFlags`, `ExtractedSymbol`, `ExtractedLiteral`, `ExtractedFile`. Consumers (`index.rs:1`, `callers.rs:23`, `codemap.rs`, `indexer.rs:20`, `benchmark.rs:2`) depend on these types, not on tree-sitter — yet they import the whole parser module.
- `split_identifier` (tokenizer) lives at `parser.rs:2127-2184`; it is pure (no dependency on other parser helpers) and is used by the index/search layers and the `tokenize` CLI subcommand (`main.rs:104-109`).
- `range_strictly_contains` — pure `CodeRange` semantics — is misplaced in `apps/codemap-search/src/codemap.rs:82-89` and consumed cross-module by `callers.rs:313`, creating a `callers → codemap` edge.
- In-file tests live at `parser.rs:2186-2795` (~610 lines) and reference `crate::parser::{CodeRange, SymbolFlags}` style paths.

## Behavior Contract
- Locked: `ExtractedFile`/`ExtractedSymbol` serde JSON shape (used by the `parse` CLI subcommand and index storage), `split_identifier` token output, `range_strictly_contains` truth table.
- Contract artifacts: parser in-file tests (`parser.rs:2186-2795`), callers/codemap in-file tests, e2e suite `apps/codemap-search/tests/e2e_tests.rs`.
- Verification: `cargo test` passes with test bodies unchanged (only `use` paths may change); `codemap-search parse <file>` JSON output is byte-identical on a sample file.

## Desired Outcome (To-Be)
- `parser.rs` becomes directory module `apps/codemap-search/src/parser/` (proposed): `mod.rs` (CodeExtractor trait + `TreeSitterExtractor::extract` body + queries + language helpers, all unchanged in this child), `types.rs` (the five domain types + `range_strictly_contains` moved from codemap), `tokenize.rs` (`split_identifier`).
- `parser/mod.rs` re-exports `types::*` and `tokenize::split_identifier` as the canonical public paths, so `crate::parser::ExtractedFile` etc. keep working — no compatibility façade, the module root *is* the canonical path.
- `callers.rs:313` calls `crate::parser::range_strictly_contains` (or `parser::types::`), removing the `callers → codemap` edge.
- In-file tests move with the code they test.

## Scope
### In Scope
- Mechanical relocation of types, tokenizer, and `range_strictly_contains`; `use`-path updates in all consumers (`index.rs`, `callers.rs`, `codemap.rs`, `indexer.rs`, `benchmark.rs`, `main.rs`).
### Out of Scope
- [hard] No changes to `extract` body, query strings, or language helper functions — those move in children 08/09. The frozen extraction semantics (marked by in-code "frozen" comments) must not be touched.
- [hard] No new fields, no type renames, no serde attribute changes.
- [deferred] Splitting queries/language helpers out of `parser/mod.rs` — children 08/09.

## Related Files / Entry Points
- `docs/briefs/2026-06-13-briefset-cms-restructure.md` — execution-management parent; this child is wave 2 of 9, executed after child 01 (workspace); "children 08/09" under Out of Scope are the lang-migration children of this set. Cited line numbers describe the pre-restructure tree — re-locate by symbol name if they have shifted.
- `apps/codemap-search/src/parser.rs` — the file being converted to `apps/codemap-search/src/parser/` (proposed) — read lines 9-62 for types, 2127-2184 for tokenizer first.
- `apps/codemap-search/src/codemap.rs` — remove `range_strictly_contains` (lines 82-89).
- `apps/codemap-search/src/callers.rs` — update call site at line 313.
- `apps/codemap-search/src/lib.rs` — module declaration unchanged (`pub mod parser;` still resolves to the directory).

## Side Effect Checkpoints
- [ ] `main.rs` `parse` and `tokenize` subcommands compile and produce identical output.
- [ ] Index document build (`index.rs` uses `ExtractedSymbol`/`ExtractedLiteral` field access) compiles without changes beyond imports.
- [ ] codemap in-file tests still pass after `range_strictly_contains` removal (they may reference it — move the corresponding test cases to `parser/types.rs` tests).
- [ ] No remaining `crate::codemap::range_strictly_contains` references anywhere.

## Acceptance Criteria
- [ ] `parser/` contains exactly `mod.rs`, `types.rs`, `tokenize.rs` after this child.
- [ ] `cargo test` passes; total workspace test count is unchanged, with exactly one relocation: the `range_strictly_contains` cases move from codemap's test module to `parser/types.rs` tests (parser count grows by those cases, codemap count shrinks by the same).
- [ ] `codemap-search parse apps/codemap-search/src/main.rs` JSON output identical before/after (manual diff).
- [ ] `grep -rn "codemap::range_strictly_contains" apps/codemap-search/src` returns zero matches.

## Open Questions
- None — pure mechanical move with locked semantics; relocation targets confirmed during plan review.
