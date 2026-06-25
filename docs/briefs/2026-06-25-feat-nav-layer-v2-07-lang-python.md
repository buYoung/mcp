# [feat] Add Python navigation queries, fixtures, and callee/caller wiring

## Work Type
feat

## Current State (As-Is)
- As of branch `main` (recent commit `13deb03`), `apps/codemap-search/src/lang/python.rs` defines `PythonSpec` with an inline `PYTHON_QUERY_STR` constant (L8–26) that captures class/function/assignment definitions and literals. No `tags.scm`, `navigation.scm`, or `queries/python/` directory exists.
- `PythonSpec` implements the full `LanguageSpec` trait (`grammar`, `query`, `extensions`, `is_import_line`, `docstring_anchor`, `docstring_fallback`, `is_test`, `is_exported`, `is_deprecated`, `find_owner`, `owner_stop_kinds`, `owner_type_container_kinds`, `owner_passthrough_kinds`). None of these hooks carry a navigation query getter — the `LanguageSpec` trait in `src/lang/mod.rs` has no `navigation_query` method or navigation activation hook as of the As-Is baseline.
- Callee discovery for `.py` files in `src/callers/callees.rs` uses the text-scan path exclusively. No `navigation.calls` primary path exists for Python because `NavigationFile` is not yet part of `ExtractedFile` (that infrastructure is introduced by child 02).
- `src/lang/mod.rs` will gain a `navigation_query() -> Option<&'static str>` hook (or equivalent) in child 02 when the TypeScript/TSX wiring is confirmed. As of the As-Is baseline that hook does not exist; the child-07 agent should read the hook shape from the child-02 output before editing `python.rs`.
- Python import forms — `import x`, `from x import y`, `from x import y as z` — are not tree-sitter-parsed for alias resolution. `is_import_line` does a simple `starts_with("import ")` / `starts_with("from ")` string check.
- No fixture files exist under `tests/fixtures/navigation/python/`.
- Child 01 (`scm-extract`) will have moved `PYTHON_QUERY_STR` content to `apps/codemap-search/queries/python/symbols.scm` (proposed) before this child runs. This child depends on that file being present.

## Desired Outcome (To-Be)
- `queries/python/tags.scm` (proposed) compiles against `tree_sitter_python::LANGUAGE` and captures minimum Python definition and reference nodes using `@definition.*` / `@reference.*` names per the §3 tags contract. It is used as a compile/fixture gate only — not included in the runtime concat pass.
- `queries/python/navigation.scm` (proposed) compiles against `tree_sitter_python::LANGUAGE` and captures the full Python capture contract from design doc §8 "1. Python": `import_statement` / `import_from_statement` / `aliased_import` for `@nav.import.*` captures; `call` + `identifier` and `call` + `attribute` for `@nav.call` / `@nav.call.name` / `@nav.call.receiver`; `assignment` + `identifier` for `@local.definition` / `@local.scope` (fixture-confirm required for scope node boundary).
- `src/lang/python.rs` exposes a navigation query getter wired to `navigation.scm` via `include_str!` and implements the `navigation_query` hook (or equivalent) introduced by child 02 in `LanguageSpec`, enabling Python for the runtime concat single-pass extraction in `src/parser/mod.rs`.
- `src/lang/mod.rs` receives NO structural change. Per design doc §8 Stage 8, the `navigation_query()` hook is a `LanguageSpec` trait method with a `None` default already added by child 02 for TypeScript; each language activates by overriding it in its own `src/lang/<lang>.rs`. `PythonSpec` overrides `navigation_query()` in `src/lang/python.rs` only. Child 07 is the Wave-6 pilot in the sense that it is the first Wave-6 child to land and confirms that the trait-default-override activation pattern established in child 02 works for a non-TypeScript language; it does not introduce a new registration mechanism in `src/lang/mod.rs`. All specs are already registered in `spec_for_ext`/`ALL_SPECS`, so no registration entry is added.
- `tests/fixtures/navigation/python/` (proposed directory) contains at least `basic.py`, `expected.tags.json`, and `expected.navigation.json`. The fixture covers: function definition, method definition, plain call, attribute/method call, `import x`, `from x import y`, `from x import y as z`, and a local variable assignment with a type-annotated form where applicable.
- `from x import y as z` followed by `z()` narrows the callee candidate to `y` via the `nav.import.alias` → `nav.import.name` resolution path established by child 04 (`import-caller`). Child 07 delivers the observation (alias capture in `navigation.scm`); the resolution logic lives in child 04.
- Fallback is preserved for all files that do not pass navigation extraction: files with parse failure, non-Python files, and any case where `navigation` is `None`, continue to use the existing text-scan path in `callees.rs` without regression.

## Scope
### In Scope
- Create `queries/python/tags.scm` (proposed) with minimum `@definition.function`, `@definition.method`, `@definition.class`, `@reference.call`, `@reference.identifier` captures against `tree_sitter_python::LANGUAGE`.
- Create `queries/python/navigation.scm` (proposed) implementing the full §8 "1. Python" capture contract: three import patterns (`import_statement`, `import_from_statement`, `import_from_statement` + `aliased_import`), two call patterns (`call` with `identifier` callee, `call` with `attribute` callee), one local binding pattern (`assignment` with `identifier` left-hand side). Mark `@local.scope` boundary as fixture-confirm per the design doc.
- Add a navigation query getter in `src/lang/python.rs` using `include_str!` referencing `navigation.scm`, and implement the `navigation_query` hook on `PythonSpec` using the interface confirmed by child 02.
- Activate Python navigation by overriding `navigation_query()` on `PythonSpec` in `src/lang/python.rs` (returning `Some(include_str!(...))`); do NOT structurally edit `src/lang/mod.rs`. As the first Wave-6 child to land, child 07 confirms the trait-default-override pattern (established by child 02 for TypeScript) works for Python; `src/lang/mod.rs` stays a read-only reference for all Wave-6 children.
- Create `tests/fixtures/navigation/python/` (proposed) with `basic.py`, `expected.tags.json`, and `expected.navigation.json` covering the minimum fixture items listed in the Desired Outcome.
- Verify that `apps/codemap-search/queries/python/symbols.scm` (proposed) is read via `include_str!` in `apps/codemap-search/src/lang/python.rs` before the concat pass.

### Out of Scope
- [hard] Do not implement import alias resolution logic — that is child 04 (`import-caller`) scope. Child 07 delivers the `@nav.import.alias` capture; the `from x import y as z` → `z()` resolution is child 04.
- [hard] Do not implement same-file function priority ranking — child 03 (`same-file-prio`) scope.
- [hard] Do not implement receiver/owner hint inference — child 05 (`receiver-hint`) scope.
- [hard] Do not implement `scope_id`-based lexical shadowing — child 06 (`lexical-scope`) scope.
- [hard] Do not change `EXTRACTION_FORMAT_VERSION` — that bump belongs exclusively to child 02; child 07 only wires the navigation query getter on top of the infrastructure child 02 creates.
- [hard] Do not create or modify `apps/codemap-search/queries/python/symbols.scm` (proposed) — that file is created by child 01. Child 07 reads it via `include_str!`.
- [deferred] Python-specific `ReferenceSite` population — design doc §5 permits skipping references in the first implementation; do not store `ReferenceSite` unless the concat query naturally produces them at zero extra cost.
- [deferred] Go, Rust, Java, Kotlin, C, C++ navigation queries — those are children 08–13.
- [hard] Do not touch `src/config.rs` or add new config keys.
- [hard] Do not modify the `NavigationFile`, `CallSite`, `ImportEntry`, `LocalBinding`, or `ReferenceSite` type definitions — those are owned by child 02.

## Constraints
- The `navigation_query` hook shape (method name, signature, return type) is defined by child 02 (`nav-types-callee`) as a `LanguageSpec` trait method with a `None` default. Child 07 must read the child-02 output (the committed `src/lang/mod.rs` trait definition and `src/lang/typescript.rs` override) before editing `src/lang/python.rs`; do not guess or invent the hook signature, and do not edit `src/lang/mod.rs`.
- `queries/python/navigation.scm` must compile via `Query::new(...)` against `tree_sitter_python::LANGUAGE`. A compile failure must disable navigation for Python and fall back gracefully — no panic at runtime.
- `tags.scm` must NOT be included in the runtime concat pass. It is a compile/fixture validation gate only (design doc §3).
- The runtime extraction must remain a single `Query::new(symbols_query + navigation_query)` + single `QueryCursor` pass per file. Do not add a second query pass.
- The `@local.scope` boundary in the `assignment` capture pattern is marked `(fixture-confirm)` in the design doc §8. The agent must validate the actual tree-sitter Python node shape against the grammar before writing the fixture expectation; if the scope boundary is not a stable node, fall back to the `assignment` node itself and note the deviation.
- `navigation: None` vs `Some(NavigationFile { calls: vec![], ... })` semantics must be preserved exactly as defined by child 02. Python files that pass navigation extraction must produce `Some(...)`, not `None`.
- Child 07 is the Wave-6 pilot: it confirms the activation pattern — overriding the `navigation_query()` trait method (default `None`, established by child 02) inside `src/lang/python.rs` — works for a non-TypeScript language. The pattern must be minimal and consistent with how `TypeScriptSpec` overrides the hook. Do NOT structurally edit `src/lang/mod.rs` (no new trait method, no registration table) and do not introduce Python-specific branching inside the trait default implementation. Children 08–13 replicate this same per-language override; `src/lang/mod.rs` is read-only for all of them.
- This child depends on child 06 (`lexical-scope`) being complete before execution, and on child 01 (`scm-extract`) having created `apps/codemap-search/queries/python/symbols.scm` (proposed).
- `navigation_context_default` gating policy: Python navigation **extraction** (the `navigation_query()` override producing `NavigationFile` data on `.py` `ExtractedFile`) is unconditional and is NOT gated behind `navigation_context_default`. The `navigation_context_default` config key (owned by child 04) gates only whether the annotation pipeline **uses** navigation data for precise caller/callee resolution at query time — that gate is language-neutral and lives in the child 04/05 annotation layer, not in `PythonSpec`. Child 07 must not add a Python-specific config check; activating Python means emitting navigation captures, and the precise-vs-approximate decision is left to the shared annotation gate.

## Related Files / Entry Points
- `apps/codemap-search/src/lang/python.rs` (existing) — `PythonSpec` struct at L72; add the `navigation_query` hook implementation here; also update `PYTHON_QUERY_STR` to `include_str!` referencing `apps/codemap-search/queries/python/symbols.scm` (proposed) if child 01 has not yet done so for Python.
- `apps/codemap-search/src/lang/mod.rs` (existing) — `LanguageSpec` trait definition at L54 with the `navigation_query()` hook (default `None`) added by child 02. Read-only reference for child 07: Python activation is done by overriding `navigation_query()` in `apps/codemap-search/src/lang/python.rs`, not by editing this file. No registration entry is added (all specs already registered in `spec_for_ext`/`ALL_SPECS`).
- `apps/codemap-search/queries/python/symbols.scm` (proposed) — the symbols query for Python created by child 01; child 07 reads it via `include_str!` in the concat pass.
- `apps/codemap-search/queries/python/tags.scm` (proposed) — new file; Python tags-compatible definition/reference gate query; compile against `tree_sitter_python::LANGUAGE`.
- `apps/codemap-search/queries/python/navigation.scm` (proposed) — new file; Python runtime navigation capture query per §8 "1. Python" capture contract.
- `apps/codemap-search/tests/fixtures/navigation/python/` (proposed) — new directory; `basic.py`, `expected.tags.json`, `expected.navigation.json`.
- `docs/briefs/2026-06-25-feat-nav-layer-v2-02-nav-types-callee.md` (existing) — child 02 brief; read for the `LanguageSpec::navigation_query` hook signature and `NavigationFile` type definitions before editing Python files.
- `docs/briefs/2026-06-25-briefset-nav-layer-v2.md` (proposed) — parent briefset; see for Wave-6 execution order and `apps/codemap-search/src/lang/mod.rs` conflict hotspot coordination.

## Side Effect Checkpoints
- [ ] All existing Python symbol/literal extraction fixture results remain identical after adding the navigation query getter — the `@symbol.*` and `@literal.*` capture routing in the concat pass must not regress.
- [ ] `src/lang/mod.rs` is NOT structurally changed by child 07; the `PythonSpec::navigation_query()` override in `src/lang/python.rs` does not break the existing TypeScript, Go, Rust, Java, Kotlin, C, or C++ specs — each of those specs must still build and their `navigation_query` hook (or the inherited `None` default) must behave exactly as child 02 defined.
- [ ] The per-language override pattern confirmed by child 07 (overriding `navigation_query()` in `src/lang/python.rs`) is replicable verbatim by children 08–13 in their own `src/lang/<lang>.rs` files — verify the pattern is general (not Python-specific) and requires no `src/lang/mod.rs` edit before confirming child 07 complete.
- [ ] `src/parser/mod.rs` concat-query path (introduced by child 02) correctly routes `@nav.*` and `@local.*` captures from the Python navigation query into `NavigationFile` — no Python-specific routing logic should be required in `mod.rs`; if any is needed, surface as an open question.
- [ ] `callees.rs` text-scan fallback still fires correctly for `.py` files where `navigation` is `None` (parse failure, non-navigation-enabled state) — no regression in callee count for those cases.
- [ ] `queries/python/tags.scm` and `queries/python/navigation.scm` compile successfully via `Query::new(...)` against `tree_sitter_python::LANGUAGE`; a compile failure produces a logged warning and disables navigation for Python, not a panic.
- [ ] `tests/fixtures/navigation/python/expected.tags.json` and `expected.navigation.json` match the actual output of `tags.scm` and `navigation.scm` against `basic.py` — no silent drift between fixture and runtime behavior.
- [ ] `from x import y as z` is captured with `@nav.import.name = y` and `@nav.import.alias = z` in the navigation fixture, enabling child 04 to perform alias resolution without changes to the Python query.

## Acceptance Criteria
- [ ] `cargo build` succeeds with no new warnings on the `apps/codemap-search` crate after all changes.
- [ ] `queries/python/tags.scm` compiles via `Query::new(...)` against `tree_sitter_python::LANGUAGE`.
- [ ] `queries/python/navigation.scm` compiles via `Query::new(...)` against `tree_sitter_python::LANGUAGE`.
- [ ] A Python source file with at least one function call produces `navigation: Some(NavigationFile { calls: [CallSite { name: ..., .. }], .. })` after concat-pass extraction.
- [ ] A Python source file with no function calls produces `navigation: Some(NavigationFile { calls: vec![], .. })` — not `None`.
- [ ] A `.py` file with a parse failure produces `navigation: None` and falls back to text-scan callee discovery without error.
- [ ] String literals and comments containing `name(` patterns in a `.py` file do NOT appear as `CallSite` entries in `NavigationFile.calls`.
- [ ] Function definition headers (e.g., `def foo():`) do NOT appear as `CallSite` entries.
- [ ] `import x` is captured as `@nav.import` with `@nav.import.name = x`.
- [ ] `from x import y` is captured as `@nav.import` with `@nav.import.source = x` and `@nav.import.name = y`.
- [ ] `from x import y as z` is captured as `@nav.import` with `@nav.import.source = x`, `@nav.import.name = y`, and `@nav.import.alias = z`.
- [ ] `obj.method()` is captured as `@nav.call` with `@nav.call.receiver = obj` and `@nav.call.name = method`.
- [ ] `tests/fixtures/navigation/python/expected.tags.json` and `expected.navigation.json` match the actual extraction output on `basic.py`.
- [ ] Existing Python symbol extraction results (symbols, literals, docstrings) are byte-identical before and after adding the navigation query getter — regression check on the concat-pass routing.
- [ ] The `navigation_query()` override on `PythonSpec` in `src/lang/python.rs` follows the identical pattern shape used by `TypeScriptSpec` (child 02 output) — confirmed by reading both side by side. `src/lang/mod.rs` is unchanged by child 07.

## Open Questions
- None — the Python capture contract is fully specified in design doc §8 "1. Python" including the `(fixture-confirm)` callouts. The `navigation_query` hook signature is determined by child 02 output, which this child reads before editing. The Wave-6 pilot coordination rule (child 07 first, then 08–13 in parallel) is established in the final_plan.md execution wave table. No user-owned decisions remain for this child.
