# [feat] Wire Go navigation queries, fixtures, and attribution activation

## Work Type
feat

## Current State (As-Is)
- As of branch `main` (recent commit `13deb03`), `apps/codemap-search/src/lang/go.rs` defines `GoSpec` and `GO_QUERY_STR` (L12–42) as an inline Rust string constant. After child 01 (`scm-extract`) completes, this constant will be replaced by `include_str!("../../queries/go/symbols.scm")`.
- `apps/codemap-search/src/lang/mod.rs` defines the `LanguageSpec` trait (L54–206) with no `navigation_query` hook or navigation-related method. After child 02 (`nav-types-callee`) completes, the trait will gain a `navigation_query() -> Option<&'static str>` hook (or equivalent); child 07 (`lang-python`) pilots the exact interface shape for Wave 6 peers.
- No `queries/go/` directory exists. `queries/go/symbols.scm` (proposed, created in child 01), `queries/go/tags.scm` (proposed), and `queries/go/navigation.scm` (proposed) are all absent.
- No `tests/fixtures/navigation/go/` directory exists (proposed). Go-specific navigation fixture files `basic.go`, `expected.tags.json`, and `expected.navigation.json` are all absent.
- `GoSpec` in `src/lang/go.rs` already implements `go_receiver_owner` (L143–153), `go_base_type_name` (L115–139), and `go_is_exported` (L54–56), which are directly relevant to owner-hint extraction from value receivers (`func (s *Server) Start()`). No `navigation_query` getter exists.
- `callers/callees.rs` uses a plain text scan for callee discovery. After child 02 completes, it uses `navigation.calls` as primary and falls back to the text scan when `navigation` is `None` or the file is stale.
- Go selector expressions (`pkg.Func()`) produce a `selector_expression` node whose `operand` can be a package alias or a value receiver variable — distinguishing these requires `nav.import.alias` + `nav.call.receiver` cross-referencing in the Rust attribution logic, which operates post-extraction.

## Desired Outcome (To-Be)
- `apps/codemap-search/queries/go/tags.scm` (proposed) compiles against `tree_sitter_go::LANGUAGE` and captures `@definition.function`, `@definition.method`, `@definition.type`, and `@reference.call` for the minimum Go definition/reference matching gate.
- `apps/codemap-search/queries/go/navigation.scm` (proposed) compiles against `tree_sitter_go::LANGUAGE` and captures, at minimum: `@nav.import` + `@nav.import.source` for bare `import "path"` specs; `@nav.import` + `@nav.import.alias` + `@nav.import.source` for aliased imports; `@nav.call` + `@nav.call.name` for bare `Func()` call expressions; `@nav.call` + `@nav.call.receiver` + `@nav.call.name` for selector `pkg.Func()` / `receiver.Method()` calls; `@local.definition` + `@local.scope` for `short_var_declaration` and `var_declaration` local bindings (fixture-confirm required for scope node boundary).
- `apps/codemap-search/src/lang/go.rs` exposes a navigation query getter (`get_go_navigation_query`) and implements the `LanguageSpec::navigation_query` hook added by child 02, returning `Some(...)` for the `.go` extension.
- `apps/codemap-search/src/lang/mod.rs` already has the `LanguageSpec::navigation_query` hook fixed by child 07 (`lang-python`) pilot. Child 08 adds the Go implementation only; no further trait changes are permitted in this child.
- `tests/fixtures/navigation/go/basic.go` (proposed) contains at least: one bare function call `Func()`, one package-selector call `pkg.Func()`, one method call on a local variable `s.Start()` where `s` is declared as `*Server`, one `import "path/to/pkg"`, one aliased import `import alias "path/to/other"`, and one value-receiver method declaration `func (s *Server) Start()`.
- `tests/fixtures/navigation/go/expected.tags.json` (proposed) pins the tags fixture: each function/method/type definition in `basic.go` maps to the expected `@definition.*` capture name and line range.
- `tests/fixtures/navigation/go/expected.navigation.json` (proposed) pins the navigation fixture: each call site, import entry, and local binding maps to the expected capture name, receiver/source/alias fields, and line range.
- `pkg.Func()` selector calls produce a `CallSite` with `receiver = Some("pkg")`; when `pkg` matches a `nav.import.alias`, the Rust attribution logic narrows the callee candidate via the import source path.
- `func (s *Server) Start()` value-receiver method declarations produce `owner_hint = Some("Server")` via the existing `go_receiver_owner` function, enabling method caller/callee disambiguation.
- Navigation attribution for Go `.go` files is activated end-to-end: `GoSpec::navigation_query` returns `Some(...)`, the concat-pass in `src/parser/mod.rs` includes Go navigation captures, and `callees.rs` uses `navigation.calls` as the primary callee source for Go files.
- For Go `.go` files where navigation extraction succeeds, false positives from string literals, comments, and function definition headers are eliminated from `CallSite` entries.
- The existing text-scan fallback is preserved for Go files where `navigation` is `None`, where the snapshot is potentially stale, or where `AnnotationRuntimeState.is_warming` / `has_refresh_error` / `is_dead_or_stale` is true.

## Scope
### In Scope
- Create `queries/go/tags.scm` (proposed) with minimum `@definition.*` and `@reference.*` captures compiled against `tree_sitter_go::LANGUAGE`.
- Create `queries/go/navigation.scm` (proposed) with the full capture contract described in Desired Outcome: import specs (bare and aliased), call expressions (bare identifier and selector), and local bindings (`short_var_declaration`, `var_declaration`) — fixture-confirm required for scope node boundary.
- Add `get_go_navigation_query()` getter in `src/lang/go.rs` and implement the `navigation_query` trait hook.
- Create fixture directory `tests/fixtures/navigation/go/` (proposed) with `basic.go`, `expected.tags.json`, and `expected.navigation.json` that pin tags and navigation results.
- Verify that the concat-pass in `src/parser/mod.rs` (added in child 02) correctly routes `@nav.*` and `@local.*` captures from Go files into `NavigationFile`.
- Verify that `GoSpec::owner_hint` is correctly derived from value-receiver method declarations using the existing `go_receiver_owner` function, and confirm this is reachable from the navigation attribution path.
- Wire end-to-end activation so Go `.go` files produce `navigation: Some(NavigationFile { ... })` after extraction.

### Out of Scope
- [hard] Do not modify the `LanguageSpec` trait in `src/lang/mod.rs` beyond what child 07 pilot has already fixed — the interface is frozen for Wave 6 peers after child 07 completes.
- [hard] Do not create or modify `queries/go/symbols.scm` — that file is the output of child 01 (`scm-extract`); child 08 depends on it being present.
- [hard] Do not modify `src/parser/mod.rs` concat-pass logic — the single-pass architecture is child 02 scope; child 08 only wires the Go navigation query into the existing hook.
- [hard] Do not modify `src/callers/callees.rs` primary/fallback switching logic — that is child 02 scope; child 08 activates Go by returning `Some(...)` from the navigation hook.
- [hard] Do not add import alias resolution in the Rust attribution layer — that is child 04 (`import-caller`) scope. Child 08 only emits `nav.import.alias` and `nav.import.source` captures so child 04 can consume them.
- [hard] Do not add `NavigationIndex` or reverse-caller index — child 04 scope.
- [hard] Do not add `scope_id`-based lexical shadowing — child 06 (`lexical-scope`) scope; `scope_id` fields may be emitted as `None` by Go local binding captures if the scope node boundary cannot be confirmed by fixture.
- [deferred] Distinguishing package-selector from value-receiver in the Rust attribution logic beyond what capture names already expose — that belongs to child 04/05 cross-referencing with import entries; child 08 ensures the captures are emitted correctly.
- [hard] Do not touch `src/config.rs` — no new config keys in this child.
- [hard] Do not add ASM navigation queries — ASM is excluded per §8 of the design doc.

## Constraints
- `tags.scm` must NOT be included in the runtime concat pass. It is a compile/fixture validation gate only (design doc §3). The runtime pass is `symbols.scm` + `navigation.scm` concat only.
- `navigation_query` must return `Some(...)` for `.go` extension and `None` for all other extensions managed by `GoSpec` (there are none — `GoSpec` serves `["go"]` only, so this is a single-branch implementation).
- The scope node boundary for `short_var_declaration` and `var_declaration` local bindings is marked `(fixture-confirm)` in the design doc. If fixture testing reveals the expected node shape does not compile or does not produce stable captures, emit `@local.definition` without `@local.scope` and document the confirmed node name in the fixture `expected.navigation.json`.
- `go_receiver_owner` (L143–153 in `src/lang/go.rs`) is already implemented and correct. Do not rewrite it. Only expose it to the navigation attribution path by wiring `owner_hint` derivation through the navigation hook.
- Child 07 (`lang-python`) is the Wave 6 pilot and must complete before child 08 starts, so that `src/lang/mod.rs` interface changes are fixed. Do not speculate on or pre-apply `src/lang/mod.rs` changes.
- `cargo build` must succeed with no new warnings after all changes.

## Related Files / Entry Points
- `apps/codemap-search/src/lang/go.rs` (existing) — add `get_go_navigation_query()` and implement `navigation_query` trait hook here; `GoSpec` struct at L155, trait `impl` at L157.
- `apps/codemap-search/src/lang/mod.rs` (existing) — `LanguageSpec` trait at L54; `navigation_query` hook will be present after child 07 pilot. Read-only for child 08 except to implement the hook.
- `apps/codemap-search/queries/go/symbols.scm` (proposed) — created by child 01; must be present before child 08 starts; child 08 reads via `include_str!` in the concat pass. Do not recreate.
- `apps/codemap-search/queries/go/tags.scm` (proposed) — new file; Go tags-compatible definition/reference gate query compiled against `tree_sitter_go::LANGUAGE`.
- `apps/codemap-search/queries/go/navigation.scm` (proposed) — new file; Go runtime navigation capture query: import specs, call expressions (bare + selector), local bindings.
- `apps/codemap-search/tests/fixtures/navigation/go/` (proposed) — new directory; contains `basic.go`, `expected.tags.json`, `expected.navigation.json`.
- `apps/codemap-search/src/parser/mod.rs` (existing) — concat-pass entry point (added by child 02); child 08 verifies Go captures route correctly but does not modify this file.
- `apps/codemap-search/src/callers/callees.rs` (existing) — primary/fallback switching (added by child 02); child 08 verifies Go activation path but does not modify this file.
- `docs/briefs/2026-06-25-feat-nav-layer-v2-07-lang-python.md` (proposed) — Wave 6 pilot sibling; must complete first to fix `src/lang/mod.rs` interface.

## Side Effect Checkpoints
- [ ] All existing Go symbol/literal extraction fixture results remain identical after adding the `navigation_query` hook — the `@symbol.*` and `@literal.*` capture routing in the concat pass must not regress for Go files.
- [ ] `queries/go/tags.scm` compiles successfully via `Query::new(...)` against `tree_sitter_go::LANGUAGE`; a compile failure must produce a logged warning and disable navigation for Go, not a panic.
- [ ] `queries/go/navigation.scm` compiles successfully via `Query::new(...)` against `tree_sitter_go::LANGUAGE`; same failure behavior as `tags.scm`.
- [ ] The existing `go_receiver_owner` function (L143–153 in `src/lang/go.rs`) is not modified; its behavior is verified unchanged by fixture: `func (s *Server) Start()` still produces `owner = Some("Server")` in extracted symbols.
- [ ] `src/lang/mod.rs` receives no trait-level changes in child 08 — only the `GoSpec impl` block in `src/lang/go.rs` is touched. Confirm by diffing `src/lang/mod.rs` before and after.
- [ ] Go files where navigation extraction returns `None` (e.g., parse failure) still fall back to the text-scan callee path — no regression in callee count for those files.
- [ ] The `src/lang/mod.rs` shared hotspot (Wave 6 conflict surface) is not edited by child 08. Any `src/lang/mod.rs` interface changes needed for Go activation must already be present from child 07 pilot. If child 07 did not add a required method, block child 08 and raise the gap to the orchestrator.
- [ ] `AnnotationRuntimeState.is_warming == true` suppresses Go precise callee resolution even when `navigation.calls` is populated — verify using the existing warming-state test path from child 02.

## Acceptance Criteria
- [ ] `cargo build` succeeds with no new warnings on the `apps/codemap-search` crate after all changes.
- [ ] `queries/go/tags.scm` compiles successfully against `tree_sitter_go::LANGUAGE` via `Query::new(...)`.
- [ ] `queries/go/navigation.scm` compiles successfully against `tree_sitter_go::LANGUAGE` via `Query::new(...)`.
- [ ] A Go source file containing `pkg.Func()` produces a `CallSite` with `name = "Func"` and `receiver = Some("pkg")` in `NavigationFile.calls`.
- [ ] A Go source file containing `s.Start()` where `s` is a local variable produces a `CallSite` with `name = "Start"` and `receiver = Some("s")` in `NavigationFile.calls`.
- [ ] A Go source file containing `import alias "path/to/pkg"` produces an `ImportEntry` with `source = Some("path/to/pkg")` and `local_name = "alias"` in `NavigationFile.imports`. (`alias` is the local binding name, so it maps to `local_name`, not `imported_name`; Go has no per-package export-name concept for the alias, so `imported_name` is not used for this form.)
- [ ] A Go source file containing `func (s *Server) Start() {}` produces an extracted symbol with `owner = Some("Server")` — the existing `go_receiver_owner` behavior is preserved post-wiring.
- [ ] `tests/fixtures/navigation/go/basic.go` fixture, when extracted, produces `NavigationFile` output that matches `expected.navigation.json` exactly.
- [ ] `tests/fixtures/navigation/go/basic.go` fixture, when extracted with `tags.scm`, produces tags capture output that matches `expected.tags.json` exactly.
- [ ] A Go source file with no function calls produces `navigation: Some(NavigationFile { calls: vec![], ... })` — not `None`.
- [ ] A Go source file for which navigation extraction fails produces `navigation: None` and the text-scan callee fallback fires correctly.
- [ ] String literals containing `name(` patterns in Go source do NOT appear as `CallSite` entries in `NavigationFile.calls`.
- [ ] Comment lines containing `name(` patterns in Go source do NOT appear as `CallSite` entries.
- [ ] Function definition headers (e.g., `func Foo()`) do NOT appear as `CallSite` entries.
- [ ] The existing Go symbol extraction fixture results (symbols, literals, docstrings, owner attribution) are identical before and after child 08 changes — no regression.

## Open Questions
- The design doc marks `short_var_declaration` and `var_declaration` `@local.scope` node boundaries as `(fixture-confirm)`. If `tree_sitter_go` parses these differently than the query expects (e.g., the scope anchor node does not exist or has a different name), should child 08 emit `@local.definition` without `@local.scope` and defer scope-based shadowing to child 06, or should it block activation until the correct node name is confirmed? — Recommended: emit without `@local.scope` if the compile fails or the fixture does not match; document the confirmed node name in `expected.navigation.json` and proceed with activation. The `scope_id` field stays `None` for Go until child 06 explicitly adds it.
- The design doc states that distinguishing a package selector (`pkg.Func()`) from a value receiver call (`s.Method()`) at the Rust attribution layer requires cross-referencing `nav.call.receiver` against `nav.import.alias`. Child 08 ensures both captures are emitted. The actual cross-referencing is child 04 scope. If, after child 04, Go package-selector calls still do not narrow correctly (e.g., because `import_spec` path format differs from TS import source), should Go selector attribution fall back to `approximate` globally or only when the import source cannot be resolved? — Recommended: fall back per-callsite with `navigation_fallback_reason=source_unresolved`; global Go fallback would regress bare-function-call precision already established in this child.
