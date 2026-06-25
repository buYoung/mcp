# [feat] Enable navigation layer for C (queries/c/ tags + navigation + fixtures)

## Work Type
feat

## Current State (As-Is)
- As of branch `main` (recent commit `13deb03`), `apps/codemap-search/src/lang/c_family/c.rs` defines `CSpec` implementing `LanguageSpec` with `C_QUERY_STR` inlined at L21–64. After child 01 (`scm-extract`) completes, this constant is replaced by `include_str!("../../../queries/c/symbols.scm")` (three `../` steps, because `c.rs` lives under `src/lang/c_family/` — one level deeper than top-level `src/lang/*.rs` which use two `../` steps), but no navigation hooks exist on `CSpec`.
- `apps/codemap-search/src/lang/mod.rs` defines the `LanguageSpec` trait with no `navigation_query` hook or navigation-related method; this hook is added by child 02 (`nav-types-callee`) as part of the TypeScript wiring, and its interface is fixed before Wave 6 starts.
- `queries/c/symbols.scm` (proposed, created in child 01) holds the existing C symbol/literal extraction query; `queries/c/tags.scm` and `queries/c/navigation.scm` do not exist.
- `tests/fixtures/navigation/c/` does not exist; no expected tags or navigation JSON is present for C.
- C callee discovery in `src/callers/callees.rs` uses the text-scan fallback for all C files because `navigation` is `None` for every C `ExtractedFile` (no navigation extraction is wired for C).
- `apps/codemap-search/src/lang/c_family/mod.rs` provides two shared read-only helpers used by `CSpec`: `name_for_cfn` (declarator-chain name walk) and `c_has_static_storage` (static storage-class detection). These helpers are not modified by this child.
- C has no overloading, no namespaces, and no class/method dispatch — free function priority and static-storage exclusion are the only C-specific candidate narrowing concerns.
- `.h` files are served by `CppSpec` (C++ grammar), not `CSpec`; `CSpec.extensions()` returns `&["c"]` only.

## Desired Outcome (To-Be)
- `queries/c/tags.scm` (proposed) captures the minimum C definition and reference nodes using standard tags-compatible capture names (`@definition.function`, `@definition.struct`, `@definition.enum`, `@definition.type`, `@definition.constant`, `@reference.call`), compiling against `tree_sitter_c::LANGUAGE`.
- `queries/c/navigation.scm` (proposed) captures `#include` as `@nav.import` / `@nav.import.source`, free function calls as `@nav.call` / `@nav.call.name`, member/arrow calls as `@nav.call` / `@nav.call.receiver` / `@nav.call.name`, and function-scope local variable declarations as `@local.definition` / `@local.type` / `@local.scope`, compiling against `tree_sitter_c::LANGUAGE`.
- `CSpec` in `apps/codemap-search/src/lang/c_family/c.rs` exposes a navigation query getter wired to `queries/c/navigation.scm` via `include_str!("../../../queries/c/navigation.scm")` (three `../` steps, matching the `c_family/` depth), and implements the `navigation_query` hook added to `LanguageSpec` by child 02.
- `apps/codemap-search/src/lang/mod.rs` requires no structural changes. Per design doc §8 Stage 8, the `navigation_query()` hook (default `None`) is established by child 02 and the child 07 pilot only confirms the override pattern — it does not adjust the hook signature. `CSpec` activates by overriding `navigation_query()` in `src/lang/c_family/c.rs`; `src/lang/mod.rs` stays a read-only reference.
- `tests/fixtures/navigation/c/basic.c` (proposed), `tests/fixtures/navigation/c/expected.tags.json` (proposed), and `tests/fixtures/navigation/c/expected.navigation.json` (proposed) are created; the fixture confirms: `#include` imports captured, free function calls captured, arrow-vs-dot field expression calls captured, function-scope local bindings captured, and `static` function definitions excluded from exported candidates.
- After activation, C files with successful navigation extraction produce `navigation: Some(NavigationFile { calls: [...], imports: [...], locals: [...] })` in `ExtractedFile`; C files without navigation support (e.g., a parse failure) fall back to `navigation: None` and the existing text-scan callee path.
- Free function callee candidates for C files follow same-file priority (established by child 03) and are excluded when the target definition carries `static` storage class (enforced via `CSpec.is_exported` which already calls `c_has_static_storage`).

## Scope
### In Scope
- Create `queries/c/tags.scm` (proposed) with minimum C definition/reference tags-compatible captures.
- Create `queries/c/navigation.scm` (proposed) with minimum `@nav.import`, `@nav.import.source`, `@nav.call`, `@nav.call.name`, `@nav.call.receiver`, `@local.definition`, `@local.type`, `@local.scope` captures per the design doc §8 C contract.
- Add navigation query getter to `apps/codemap-search/src/lang/c_family/c.rs` and implement the `navigation_query` `LanguageSpec` hook for `CSpec`.
- Create `tests/fixtures/navigation/c/basic.c` (proposed), `tests/fixtures/navigation/c/expected.tags.json` (proposed), and `tests/fixtures/navigation/c/expected.navigation.json` (proposed).
- Confirm `queries/c/tags.scm` and `queries/c/navigation.scm` compile successfully via `Query::new(...)` against `tree_sitter_c::LANGUAGE`.
- Confirm free function priority (same-file lookup, established in child 03) applies to C callee candidates.
- Confirm `static` function definitions are excluded from import-source-hint callee candidates (via existing `CSpec.is_exported` → `c_has_static_storage` path).

### Out of Scope
- [hard] Do not modify `apps/codemap-search/src/lang/c_family/mod.rs` shared helpers (`name_for_cfn`, `c_has_static_storage`) — these are read-only from this child's perspective and are also used by child 13 (`lang-cpp`).
- [hard] Do not modify `apps/codemap-search/src/lang/c_family/cpp.rs` or any C++ query files — C++ navigation is child 13 scope.
- [hard] Do not add `.h` extension handling to `CSpec` — `.h` belongs to `CppSpec` per the existing grammar decision.
- [hard] Do not add new navigation config keys (`navigation_context_default`, `navigation_callsite_budget`, `navigation_store_references`) — these are child 04 scope.
- [hard] Do not modify `NavigationIndex`, `calls_by_name`, or caller reverse-index logic — child 04 and child 05 scope.
- [hard] Do not bump `EXTRACTION_FORMAT_VERSION` — this was done in child 02 and must not be touched again.
- [deferred] C++ `field_expression` arrow vs dot disambiguation for `->` operator — child 13 scope.
- [deferred] C macro expansion call sites (`#define FOO(x)` call patterns) — not in the minimum navigation contract; text-scan fallback applies.
- [deferred] `ReferenceSite` population for C — skip or zero-cap per design doc §5 guidance; direct call coverage is the primary goal.

## Constraints
- The `navigation_query` hook interface on `LanguageSpec` is established by child 02 and locked after child 07 (Python, Wave-6 pilot) completes. This child must implement the hook exactly as that interface specifies; do not invent or pre-empt a different signature.
- `queries/c/navigation.scm` must compile against `tree_sitter_c::LANGUAGE` only (not the C++ grammar). If compilation fails, navigation must be disabled for `.c` files with a logged warning and text-scan fallback — no panic.
- `tags.scm` must NOT be included in the runtime concat pass; it is a compile/fixture validation gate only per design doc §3.
- The runtime extractor must remain a single `Query::new(symbols_query + navigation_query)` + single `QueryCursor` pass per file. Do not add a second query pass for C navigation.
- The `field_expression` arrow-vs-dot distinction in `navigation.scm` is marked `(fixture-confirm)` in the design doc §8 C contract; confirm the actual node shape from `tree_sitter-c` `node-types.json` before writing the query, and reflect the confirmed shape in the fixture.
- The `declaration` local binding capture in `navigation.scm` is also marked `(fixture-confirm)` in the design doc — confirm the exact field names (`type`, `declarator` sub-node) before finalizing the query.
- `src/lang/c_family/` shared helpers (`name_for_cfn`, `c_has_static_storage`) are read-only for this child. Child 13 (`lang-cpp`) shares these helpers concurrently in Wave 6; do not create merge conflicts by touching `c_family/mod.rs`.
- `src/lang/mod.rs` is read-only for this child. Per design doc §8 Stage 8, no Wave-6 child structurally edits `src/lang/mod.rs`; this child adds only the `navigation_query()` override inside `CSpec` in `c.rs`. No `mod.rs` structural change is expected or permitted.

## Related Files / Entry Points
- `apps/codemap-search/src/lang/c_family/c.rs` (existing) — `CSpec` struct at L73; add `navigation_query` hook implementation here alongside the existing query getter at L66–71.
- `apps/codemap-search/src/lang/c_family/mod.rs` (existing) — `name_for_cfn` at L20 and `c_has_static_storage` at L134; read-only reference for understanding the declarator walk; do not modify.
- `apps/codemap-search/src/lang/mod.rs` (existing) — `LanguageSpec` trait at L54 with the `navigation_query()` default-`None` hook added by child 02; `spec_for_ext` at L210 already maps `"c"` to `CSpec`. Read-only reference; not edited. Activation is the `navigation_query()` override in `c.rs`.
- `queries/c/symbols.scm` (proposed) — created by child 01 (`scm-extract`); existing C symbol/literal extraction query; do not modify; `navigation.scm` is concatenated with this at runtime.
- `queries/c/tags.scm` (proposed) — new file; minimum C definition/reference tags-compatible gate query; compiles against `tree_sitter_c::LANGUAGE`.
- `queries/c/navigation.scm` (proposed) — new file; C runtime navigation capture query for `#include`, function calls, field-expression calls, and local declarations.
- `tests/fixtures/navigation/c/basic.c` (proposed) — new file; minimal C source covering free function call, arrow/dot field call, `#include`, and a `static` function definition.
- `tests/fixtures/navigation/c/expected.tags.json` (proposed) — new file; expected tags-query output for `basic.c`.
- `tests/fixtures/navigation/c/expected.navigation.json` (proposed) — new file; expected navigation-query output for `basic.c`.
- `docs/briefs/2026-06-25-briefset-nav-layer-v2.md` (proposed) — parent brief for this briefset; see for wave ordering and set-level acceptance criteria.
- `docs/briefs/2026-06-25-feat-nav-layer-v2-02-nav-types-callee.md` (existing) — child 02 reference; defines `NavigationFile`, `CallSite`, `LanguageSpec::navigation_query` hook, and the concat single-pass extractor pattern that this child follows.
- `docs/briefs/2026-06-25-feat-nav-layer-v2-06-lexical-scope.md` (proposed) — child 06 prerequisite; `scope_id` infrastructure and Wave-5 completion gate before Wave 6 begins.

## Side Effect Checkpoints
- [ ] Existing C symbol/literal extraction fixture results remain identical after adding the navigation concat pass — `@symbol.*` and `@literal.string` captures must not regress.
- [ ] `CSpec.is_exported` (which calls `c_has_static_storage`) correctly marks `static` functions as non-exported; confirm that import-source-hint callee candidates exclude `static`-declared C functions.
- [ ] `queries/c/tags.scm` and `queries/c/navigation.scm` compile successfully via `Query::new(...)` against `tree_sitter_c::LANGUAGE`; a compile failure logs a warning and disables navigation for `.c` files rather than panicking.
- [ ] The text-scan callee fallback still fires for C files when `navigation` is `None` (e.g., parse failure or unsupported grammar state) — no regression in callee count for those cases.
- [ ] `.h` files continue to be processed by `CppSpec` (C++ grammar); `CSpec` navigation wiring does not affect `.h` extension routing in `spec_for_ext`.
- [ ] `src/lang/mod.rs` retains its existing trait method set after this child's edit; no extraneous trait methods or registry entries are introduced beyond the navigation hook.
- [ ] `src/lang/c_family/mod.rs` shared helpers (`name_for_cfn`, `c_has_static_storage`) are unmodified; child 13 (`lang-cpp`) can apply in parallel in Wave 6 without merge conflicts on these files.
- [ ] `cargo build` for `apps/codemap-search` succeeds with no new warnings after all C navigation changes.

## Acceptance Criteria
- [ ] `queries/c/tags.scm` and `queries/c/navigation.scm` compile successfully via `Query::new(...)` against `tree_sitter_c::LANGUAGE`; a compile failure must not panic — it must disable navigation for `.c` and fall back gracefully.
- [ ] `tests/fixtures/navigation/c/basic.c` fixture produces tag captures matching `expected.tags.json` — at minimum one `@definition.function` and one `@reference.call`.
- [ ] `tests/fixtures/navigation/c/basic.c` fixture produces navigation captures matching `expected.navigation.json` — at minimum one `@nav.import.source` (from a `#include`), one `@nav.call.name` (free function call), one field-expression call with `@nav.call.receiver` and `@nav.call.name`, and one `@local.definition` (function-scope variable).
- [ ] A C source file with at least one function call and one `#include` produces `navigation: Some(NavigationFile { calls: [...], imports: [...], ... })` after extraction via the concat single-pass extractor.
- [ ] A C source file with no function calls produces `navigation: Some(NavigationFile { calls: vec![], ... })` — not `None`.
- [ ] A `.c` file whose navigation query compilation fails produces `navigation: None` and callee discovery falls back to the text-scan path — no panic, no crash.
- [ ] A `static` C function definition appears in the symbol index with `is_exported = false`; this function is excluded from callee candidates when the call site is resolved via import-source hint.
- [ ] Free function callee candidates for a `.c` file are ranked by same-file priority (child 03 behavior) — a same-file function definition with a matching name is preferred over a global name-match from another file.
- [ ] Existing symbol and literal extraction results for C files are unchanged after adding the navigation concat pass — the `@symbol.*` and `@literal.string` captures are unaffected.
- [ ] `cargo build` succeeds on `apps/codemap-search` with no new warnings after all changes in this child.

## Open Questions
- None — the `navigation_query` hook interface is locked by child 02 and confirmed by the child 07 pilot before Wave-6 parallel execution begins; the C navigation.scm capture contract is specified in design doc §8 with explicit `(fixture-confirm)` markers for `field_expression` and `declaration` node shapes that the implementing agent resolves by inspecting `tree_sitter-c` `node-types.json` before writing the query.
