# [refactor] Introduce lang/ LanguageSpec registry and convert parser hooks

## Work Type
refactor

## Current State (As-Is)
- All parser-side language knowledge lives inside `apps/codemap-search/src/parser/mod.rs` (post children 02/07; originally `parser.rs`, line refs below are from the pre-split file and shift after child 02):
  - Per-language query constants `RUST_QUERY_STR` … `ASM_QUERY_STR` (lines 81-549) and compiled-query getters `get_rust_query()` … `get_asm_query()` (lines 469-549).
  - Language helper free functions, ~1,025 lines (lines 551-1576): `go_*` (e.g. `go_is_exported` line 860, `go_receiver_owner` line 1210), `kotlin_*` (line 941+), `java_*` (line 963+), `cpp_*`/`c_*` (lines 1234-1442), `asm_*` (lines 1442-1503), Rust attribute checks (line 719), TS export collection (line 794), Python docstring handling (line 638). These take `(Node, source)` and sit outside the extract tree-walk.
  - The `extract` body (lines 1577-2126) is a single generic tree-walk with ~17 inline extension-branch sites: grammar/query selection (line 1583), string-literal handling (lines 1626/1631), C-family declarator handling (line 1725), docstring start-node branches (lines 1808-1866), `is_test` chain (lines 1879-1935), `is_exported` chain (lines 1937-2019), `is_deprecated` chain (lines 2021-2072), owner tables `owner_stop_kinds`/`owner_type_container_kinds`/`owner_passthrough_kinds` (lines 1048/1092/1120) and owner special cases (lines 1508/1515/1541/1553).
- At 9 languages this is strained; the roadmap targets the top ~20 tree-sitter languages, under which each inline chain grows to ~20 arms — the motivating scalability problem for this child.
- C and C++ share `c_declarator_name` (line 1234, shared extract branch at line 1725) and `c_has_static_storage` (line 1393, used by both `is_exported` arms at lines 1965/2006); C++-only helpers are `cpp_outofline_owner` (1301), `cpp_scope_to_name` (1327), `find_function_declarator` (1358, sole caller is `cpp_outofline_owner`), `cpp_nearest_access_specifier` (1413), `cpp_member_is_exported` (1431).
- One TS query string serves four extensions (ts/tsx/js/jsx) but **two grammars** (`LANGUAGE_TYPESCRIPT` vs `LANGUAGE_TSX`, selection at lines 1584-1592) — a spec must support per-extension grammar selection.
- Grammar crates: `apps/codemap-search/Cargo.toml` lines 19-38.

## Behavior Contract
- Locked: `ExtractedFile` output for every supported language is byte-identical — flags (`is_test`/`is_exported`/`is_deprecated`), owners, docstrings, literals, ranges, and symbol ordering all preserved.
- Contract artifacts: the per-language golden snapshot tests from child 07 (`apps/codemap-search/tests/fixtures/extract/golden/*.json`) — the primary net; parser in-file unit tests; e2e suite.
- Verification: after **each hook conversion step**, `cargo test` passes with zero golden-file updates. Any intentional golden change is a red flag — this child must not change extraction output.

## Desired Outcome (To-Be)
- New crate-level module `apps/codemap-search/src/lang/` (proposed) — the single home for parser-side language knowledge:
  - `mod.rs` — `LanguageSpec` trait (extensions, per-extension grammar, compiled query, hooks: `is_test`, `is_exported`, `is_deprecated`, `docstring`, string-literal cleaning, owner-kind tables, owner special-case resolution; default impls so language files state only their differences) + a static registry resolving `ext → &'static dyn LanguageSpec`.
  - `rust.rs`, `python.rs`, `typescript.rs` (one spec, 4 extensions, 2 grammars), `go.rs`, `java.rs`, `kotlin.rs`, `asm.rs` — each owning its query string, compiled query, and helpers.
  - `c_family/` — `mod.rs` (shared `c_declarator_name`, `c_has_static_storage`, spec re-exports), `c.rs`, `cpp.rs`.
- `parser/mod.rs` shrinks to the `CodeExtractor` trait + a generic tree-walk (~550 lines) that resolves a spec from the registry and calls hooks where the inline chains were.
- Migration is **hook-by-hook**, not language-by-language: convert one chain at a time (query/grammar selection → is_test → is_exported → is_deprecated → docstring → owner tables/special cases → string literals), with compile + full test pass between steps.

## Scope
### In Scope
- The `LanguageSpec` trait + registry design, the 8 language entries + c_family folder, hook-by-hook conversion of the extract body, relocation of the helper block (lines 551-1576) and query constants into language files.
### Out of Scope
- [hard] No new languages — exactly the current 9 grammars; "adding language #10" is the design's acceptance scenario but is not performed here.
- [hard] No extraction semantic changes — golden snapshots must not change.
- [hard] Do not migrate `callers` language knowledge or `SOURCE_EXTENSIONS` — child 09.
- [deferred] Splitting `typescript.rs` if js/jsx ever diverge from ts/tsx.

## Related Files / Entry Points
- `apps/codemap-search/src/parser.rs` — conversion source (post child 02: `apps/codemap-search/src/parser/mod.rs` (proposed)); start by reading the extract body's branch sites to fix the hook list.
- `apps/codemap-search/src/lang/` (proposed) — new module home.
- `apps/codemap-search/Cargo.toml` — grammar crate imports move expression-level into the per-language files under `apps/codemap-search/src/lang/` (proposed) — no dependency changes.
- `apps/codemap-search/src/lib.rs` — add `pub mod lang;`.

## Side Effect Checkpoints
- [ ] Golden snapshot suite (child 07) passes after every individual hook conversion — run per-step, not only at the end.
- [ ] Query compilation remains lazy/once-per-language (current `OnceLock`-style getters) — no per-extract recompilation regression.
- [ ] `parse` CLI subcommand output unchanged on a sample of each language.
- [ ] Unknown-extension behavior unchanged (extensions outside the registry are rejected/skipped exactly as before).
- [ ] tsx vs ts grammar selection verified via the tsx fixture (the two-grammar/one-spec case).

## Acceptance Criteria
- [ ] `lang/` contains `mod.rs`, 7 language files, and `c_family/{mod.rs,c.rs,cpp.rs}`; `parser/mod.rs` contains no per-language helper functions and no query strings.
- [ ] Zero `ext == "..."`-style language branches remain in `parser/mod.rs` except the single registry lookup.
- [ ] All golden snapshots byte-identical (zero regenerations) and `cargo test` passes.
- [ ] A design note (module-level doc comment in `lang/mod.rs`) states the add-a-language recipe: one file + one registry entry + one Cargo dependency.

## Open Questions
- None — trait-with-default-impls over data-table design, c_family subfolder shape, and hook-by-hook ordering were locked across three review rounds; remaining choices (hook signatures) are bounded by the behavior contract.
