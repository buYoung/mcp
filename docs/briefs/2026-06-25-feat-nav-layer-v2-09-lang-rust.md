# [feat] Add Rust navigation queries, fixtures, and callee/caller wiring

## Work Type
feat

## Current State (As-Is)
- As of branch `main` (recent commit `13deb03`), `apps/codemap-search/src/lang/rust.rs` embeds the Rust tree-sitter query as an inline Rust string constant `RUST_QUERY_STR` (L8–56); after child 01 (`scm-extract`) completes this constant is replaced with `include_str!("../../queries/rust/symbols.scm")`, but no `tags.scm` or `navigation.scm` exists.
- The `queries/rust/` directory does not exist; only `queries/rust/symbols.scm` will be present after child 01 lands.
- `apps/codemap-search/src/lang/rust.rs` implements `RustSpec` with `grammar()` returning `tree_sitter_rust::LANGUAGE.into()` and `query()` returning the compiled symbols query. It has no `navigation_query` hook, no navigation query getter, and no `NavigationFile` wiring.
- `apps/codemap-search/src/lang/mod.rs` defines the `LanguageSpec` trait; after child 02 (`nav-types-callee`) adds the `navigation_query()` hook, Rust has not yet implemented it, so `RustSpec::navigation_query()` returns the default `None`, leaving Rust in the text-scan fallback path.
- Caller/callee discovery for `.rs` files uses the text-scan path in `src/callers/callees.rs` (no `navigation.calls`), producing false positives from identifiers in string literals, macro invocations, comments, and definition headers.
- `use ... as ...` alias resolution is absent: a call to the alias is not mapped back to the original name.
- Glob import (`use path::*`) is inherently ambiguous and has no fallback designation yet.
- Trait method calls (e.g., `trait_obj.method()`) produce multiple candidates that cannot be narrowed without type inference; no conservative fallback policy for this case has been declared for Rust.
- `tests/fixtures/navigation/rust/` does not exist; there are no Rust-specific navigation fixtures.

## Desired Outcome (To-Be)
- `apps/codemap-search/queries/rust/tags.scm` (proposed) captures minimum Rust definition and reference nodes using standard tags-compatible capture names (`@definition.*`, `@reference.*`); it compiles against `tree_sitter_rust::LANGUAGE`.
- `apps/codemap-search/queries/rust/navigation.scm` (proposed) captures call sites, receivers, import entries, and local bindings using `@nav.*` and `@local.*` capture names as specified in design doc §8 section 3; it compiles against `tree_sitter_rust::LANGUAGE`.
- `apps/codemap-search/src/lang/rust.rs` exposes a `navigation_query()` getter returning `Some(include_str!("../../queries/rust/navigation.scm"))` (or equivalent constant), wiring Rust into the concat single-pass extraction introduced in child 02.
- `apps/codemap-search/src/lang/mod.rs` requires no new trait changes (the `navigation_query()` hook was added in child 02); `RustSpec` simply overrides the existing hook.
- `use path::name as alias` imports are captured as `ImportEntry { local_name: alias, imported_name: name, source: path, kind: Named }` so downstream alias resolution (child 04) can map `alias()` call sites back to `name`.
- Glob imports (`use path::*`) are captured with `@nav.import.glob` and always produce a fallback result; no attempt is made to enumerate glob-imported names.
- Trait method calls produce multiple candidates and are conservatively kept as `approximate` fallback; the brief mandates this fallback policy is documented and enforced.
- `Struct::assoc()` calls are captured via `scoped_identifier` (path + name), with path becoming `nav.call.receiver` — fixture-confirmed against the tree-sitter-rust node shape.
- `apps/codemap-search/tests/fixtures/navigation/rust/` (proposed) contains at minimum `basic.rs`, `expected.tags.json`, and `expected.navigation.json`; they serve as the activation gate for the Rust navigation layer.
- `RustSpec` is navigation-enabled purely by overriding `navigation_query()` in `src/lang/rust.rs` (no `src/lang/mod.rs` edit), so `.rs` files use `navigation.calls` as the primary callee source, falling back to text-scan when `navigation` is `None`, stale, or when `AnnotationRuntimeState.is_warming` is true.

## Scope
### In Scope
- Create `queries/rust/tags.scm` (proposed) with minimum `@definition.function`, `@definition.method`, `@definition.class`/`@definition.type`/`@definition.struct`, `@definition.constant`, `@reference.call` captures compiling against `tree_sitter_rust::LANGUAGE`.
- Create `queries/rust/navigation.scm` (proposed) per the design doc §8 section 3 capture contract: `use_declaration`/`scoped_identifier` import, `use_as_clause` alias import, `use_wildcard` glob import, `call_expression`/`identifier` plain call, `call_expression`/`field_expression` method call, `call_expression`/`scoped_identifier` associated-function call, `let_declaration`/`pattern`+`type` and `let_declaration`/`pattern` local bindings.
- Add a `navigation_query()` override to `RustSpec` in `src/lang/rust.rs` returning the content of `navigation.scm` via `include_str!`.
- Create `tests/fixtures/navigation/rust/basic.rs` (proposed), `tests/fixtures/navigation/rust/expected.tags.json` (proposed), and `tests/fixtures/navigation/rust/expected.navigation.json` (proposed) covering: plain function call, method call, associated-function call, `use name as alias` import, glob import, `let x: T = ...` binding, and `let x = ...` binding without type.
- Wire `src/lang/rust.rs` into the navigation activation path so `.rs` files participate in `navigation.calls`-based callee discovery after child 02's concat-pass infrastructure is in place.
- Treat `src/lang/mod.rs` as a read-only reference. Per design doc §8 Stage 8 the `navigation_query()` hook (default `None`) is established by child 02 and confirmed working by the child 07 pilot; `RustSpec` activates by overriding it in `src/lang/rust.rs` only. This child does NOT structurally edit `src/lang/mod.rs`, so it shares no merge hotspot there with sibling Wave-6 children.

### Out of Scope
- [hard] Do not modify `queries/rust/symbols.scm` — that file is output of child 01 (`scm-extract`); this child reads it via `include_str!` only.
- [hard] Do not implement import alias resolution logic in the callee narrowing path — that belongs to child 04 (`import-caller`). This child only ensures `ImportEntry` data is captured and stored.
- [hard] Do not implement `NavigationIndex`, `calls_by_name`, or caller reverse-index — child 04 concern.
- [hard] Do not implement same-file function priority ranking — child 03 (`same-file-prio`) scope.
- [hard] Do not implement receiver/owner hint inference for Rust — child 05 (`receiver-hint`) scope, which handles TypeScript first; Rust receiver hints are not in-scope here.
- [hard] Do not implement `scope_id`-based lexical shadowing — child 06 (`lexical-scope`) scope.
- [deferred] Trait object method resolution (e.g., `dyn Trait` dispatch) — requires type inference unavailable at query level; always fallback.
- [deferred] Macro-generated call sites — tree-sitter does not expand macros; calls inside macro bodies may be missed or misattributed; document as known limitation.
- [deferred] Path-qualified imports (`use std::collections::HashMap`) where the path has more than one segment — first implementation captures the immediate `scoped_identifier` parent only; deeper path traversal is deferred.

## Constraints
- `queries/rust/navigation.scm` must compile successfully via `Query::new(&tree_sitter_rust::LANGUAGE.into(), ...)` — a compile failure must be caught in the fixture gate and must not cause a runtime panic. If compile fails, navigation for `.rs` files must fall back gracefully to `None`.
- `tags.scm` must NOT be included in the runtime concat pass; it is a compile/fixture validation gate only (design doc §3).
- The runtime extraction must remain a single `Query::new(symbols_query + navigation_query)` concat pass per file; do not add a second `QueryCursor` pass for navigation.
- `use_wildcard` glob imports must always fall back with a dedicated reason `navigation_fallback_reason=glob_import` (NOT `unsupported_language` — Rust is a navigation-supported language; `unsupported_language` is reserved for languages/extensions outside the navigation layer such as ASM per design doc §9). No candidate narrowing attempt is valid for glob imports.
- Trait method calls must be conservatively kept as `approximate` when the receiver type cannot be determined from local bindings alone — this includes all `dyn Trait` and untyped receiver patterns.
- The `(fixture-confirm)` items in the design doc capture contract — specifically `scoped_identifier` path node shape for `Struct::assoc()` and `let_declaration` scope boundary — must be confirmed against actual tree-sitter-rust parse output before the fixture expected JSONs are written. If a node shape differs from the documented contract, the query must be adjusted and the discrepancy noted in the brief's open questions.
- This child depends on child 01 (`scm-extract`) having already created `queries/rust/symbols.scm` (proposed in child 01) and on child 06 (`lexical-scope`) having completed the `scope_id` infrastructure in `src/parser/mod.rs`. Do not start before child 06 is merged.
- `src/lang/mod.rs` is read-only for this child: the `navigation_query()` trait hook (default `None`) is fixed by child 02 and confirmed by the child 07 pilot. `RustSpec` only overrides the hook in `src/lang/rust.rs`; do not add a trait method or registration entry to `src/lang/mod.rs`. Confirm the `navigation_query()` interface is stable (by reading the committed child 07 output) before wiring `RustSpec`.

## Related Files / Entry Points
- `apps/codemap-search/src/lang/rust.rs` (existing) — `RustSpec` implementation; add `navigation_query()` override here; `RUST_QUERY_STR` constant at L8 becomes `include_str!` after child 01.
- `apps/codemap-search/src/lang/mod.rs` (existing) — `LanguageSpec` trait with `navigation_query()` hook (default `None`) added in child 02; `spec_for_ext` registry and `ALL_SPECS` already list `"rs"`. Read-only reference for this child; not edited (activation is the `navigation_query()` override in `rust.rs`).
- `apps/codemap-search/queries/rust/symbols.scm` (proposed) — created in child 01; read via `include_str!` in the concat pass; do not recreate.
- `apps/codemap-search/queries/rust/tags.scm` (proposed) — new file; Rust tags-compatible definition/reference activation gate query; compiles against `tree_sitter_rust::LANGUAGE`.
- `apps/codemap-search/queries/rust/navigation.scm` (proposed) — new file; Rust runtime navigation capture query per design doc §8 section 3 contract.
- `apps/codemap-search/tests/fixtures/navigation/rust/` (proposed) — new directory; contains `basic.rs`, `expected.tags.json`, `expected.navigation.json`.
- `apps/codemap-search/src/callers/callees.rs` (existing) — `discover_callees` primary path; after child 02's navigation wiring, this file uses `navigation.calls`; confirm `.rs` files activate the primary path after this child.
- `apps/codemap-search/src/parser/types.rs` (existing) — `NavigationFile`, `CallSite`, `ImportEntry`, `ImportKind`, `LocalBinding` types added in child 02; read to understand the data contract before writing the query capture routing.
- `apps/codemap-search/src/parser/mod.rs` (existing) — concat-query extraction path added in child 02; `TreeSitterExtractor::extract` routes `@nav.*`/`@local.*` captures into `NavigationFile`; no changes needed here for Rust, only the `navigation_query()` override in `rust.rs` wires the language in.
- `docs/briefs/2026-06-25-feat-nav-layer-v2-06-lexical-scope.md` (proposed) — prerequisite child 06; confirms `scope_id` infrastructure is in place before this child starts.

## Side Effect Checkpoints
- [ ] All existing Rust symbol/literal extraction fixture results remain identical after adding the navigation concat pass — the `@symbol.*` and `@literal.*` capture routing for `.rs` files must not regress.
- [ ] `tags.scm` compile against `tree_sitter_rust::LANGUAGE` succeeds without error; a compile failure in `tags.scm` is caught at the fixture gate and does not affect runtime.
- [ ] `navigation.scm` compile against `tree_sitter_rust::LANGUAGE` succeeds; a runtime compile failure disables navigation for `.rs` files and falls back to `navigation: None` gracefully.
- [ ] `callers/scan.rs` text-scan path (`scan_workspace`) is still invoked for `.rs` files when `navigation` is `None` or when `AnnotationRuntimeState.is_warming == true`; confirm no regression in caller coverage for Rust files in warming/stale/error states.
- [ ] `src/lang/mod.rs` is unchanged by this child (activation is the `RustSpec::navigation_query()` override in `rust.rs`); there is no `src/lang/mod.rs` merge conflict with sibling Wave-6 children.
- [ ] Glob import entries (`use path::*`) are stored with `ImportKind::Glob` and do not cause a panic or a spurious `Precise` attribution downstream — verify against the fixture that a glob import call falls back to `approximate`.
- [ ] Trait method call entries captured as `CallSite` with a receiver are not narrowed to a single precise candidate unless the receiver type is confirmed — verify at least one trait method fixture case remains `approximate`.
- [ ] The `Struct::assoc()` scoped-identifier capture (`scoped_identifier` path node shape) correctly populates `CallSite.receiver` with the struct path — fixture-confirmed against `expected.navigation.json`.

## Acceptance Criteria
- [ ] `cargo build` succeeds with no new warnings on the `apps/codemap-search` crate after all changes in this child.
- [ ] `queries/rust/tags.scm` compiles successfully via `Query::new(&tree_sitter_rust::LANGUAGE.into(), tags_query_str)` — verified in the fixture gate test.
- [ ] `queries/rust/navigation.scm` compiles successfully via `Query::new(&tree_sitter_rust::LANGUAGE.into(), navigation_query_str)` — verified in the fixture gate test.
- [ ] The concat pass (symbols + navigation) produces identical `symbols` and `literals` fields in `ExtractedFile` compared to pre-change extraction on the same Rust source fixture.
- [ ] A `.rs` source file with at least one function call produces `navigation: Some(NavigationFile { calls: [CallSite { name: ..., .. }], .. })` after extraction.
- [ ] A `.rs` source file with no function calls produces `navigation: Some(NavigationFile { calls: vec![], .. })` — not `None`.
- [ ] `use path::name as alias` is stored as `ImportEntry { local_name: "alias", imported_name: Some("name"), source: Some("path"), kind: Named }` in `NavigationFile.imports`.
- [ ] `use path::*` glob import is stored as an entry with `ImportKind::Glob`; a call to any name after a glob import does not produce a `Precise` attribution — it remains `approximate` and records `navigation_fallback_reason=glob_import` (not `unsupported_language`).
- [ ] `let x: T = ...` is stored as `LocalBinding { name: "x", type_hint: Some("T"), .. }` in `NavigationFile.locals`.
- [ ] `let x = ...` (without explicit type) is stored as `LocalBinding { name: "x", type_hint: None, .. }`.
- [ ] `Struct::assoc()` call produces a `CallSite { name: "assoc", receiver: Some("Struct"), .. }` — confirmed by the fixture `expected.navigation.json`.
- [ ] String literals containing `name(` patterns in Rust source do NOT appear as `CallSite` entries in `NavigationFile.calls`.
- [ ] Comment lines containing `name(` patterns do NOT appear as `CallSite` entries.
- [ ] Function definition headers (e.g., `fn foo()`) do NOT appear as `CallSite` entries.
- [ ] `discover_callees` returns results consistent with the old text-scan path when `navigation` is `None` or when `is_dead_or_stale` is true — no regression in callee count for those Rust file cases.
- [ ] When `AnnotationRuntimeState.is_warming == true`, callee discovery for `.rs` files does not use `navigation.calls` for precise resolution regardless of whether the field is populated.
- [ ] The fixture `tests/fixtures/navigation/rust/expected.tags.json` and `expected.navigation.json` match the actual query output on `basic.rs` — the fixture serves as a non-regression gate for future query changes.

## Open Questions
- The design doc marks the `scoped_identifier` path node shape for `Struct::assoc()` as `(fixture-confirm)`. If the actual tree-sitter-rust node shape for the `path` field of `scoped_identifier` differs from the pattern shown (e.g., the path is a `scoped_identifier` itself rather than a plain `_` wildcard match), the query pattern must be adjusted. The implementing agent must parse a sample `Struct::assoc()` call and confirm the node shape before writing the fixture expected JSON.
- The design doc marks `let_declaration` scope boundary as `(fixture-confirm)`. Rust `let_declaration` does not naturally delimit a lexical scope in tree-sitter-rust; the `@local.scope` annotation on `let_declaration` may require using the enclosing `block` node instead. The implementing agent should verify the scope node shape and adjust the capture if needed; if scope confirmation is deferred to child 06, document that `@local.scope` captures on `let_declaration` are provisional.
- Per design doc §8 Stage 8, the activation mechanism is fixed: `navigation_query()` is a single `LanguageSpec` trait method (default `None`) added by child 02, and each language overrides it in its own `src/lang/<lang>.rs`. The child 07 pilot does not add new trait methods or a registration table to `src/lang/mod.rs` — it only confirms the override pattern works. `RustSpec` therefore activates solely by overriding `navigation_query()` in `src/lang/rust.rs`; no `src/lang/mod.rs` rebase is expected. The implementing agent confirms the `navigation_query()` signature from the committed child 02/07 output before finalizing `RustSpec`.
