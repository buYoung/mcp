# [feat] Add Java navigation queries and wire caller/callee attribution

## Work Type
feat

## Current State (As-Is)
- As of branch `main` (recent commit `13deb03`), `apps/codemap-search/src/lang/java.rs` holds `JAVA_QUERY_STR` as an inline Rust string constant (L8–36) covering type declarations (`class_declaration`, `interface_declaration`, `enum_declaration`, `record_declaration`), enum constants, methods, constructors, fields, and string literals. No navigation query exists.
- `apps/codemap-search/src/lang/mod.rs` defines the `LanguageSpec` trait with no `navigation_query` hook or `has_navigation`-style method; the hook was introduced by child 02 (`nav-types-callee`) and confirmed stable after child 07 (`lang-python`) served as the Wave 6 pilot.
- `queries/java/symbols.scm` (proposed, created in child 01) contains the `JAVA_QUERY_STR` content moved to a `.scm` file; child 10 must not recreate or inline it.
- No `queries/java/tags.scm` or `queries/java/navigation.scm` exist in the repository; the `queries/java/` directory is created by child 01 for `symbols.scm` only.
- No fixture directory `tests/fixtures/navigation/java/` exists; fixture files for Java navigation are absent.
- `src/callers/callees.rs` currently uses a text-scan fallback for Java because no `navigation` field is populated for `.java` files. `navigation: None` is set for Java files post-child-02 because `JavaSpec` does not implement a navigation query getter.
- The `java_is_public` helper in `src/lang/java.rs` correctly identifies `public` modifier presence; `generic_find_owner` resolves method owners to enclosing `class_declaration`, `interface_declaration`, `enum_declaration`, or `record_declaration` using the existing `owner_type_container_kinds` / `owner_passthrough_kinds` / `owner_stop_kinds` hooks — this owner attribution logic is already in place and must be preserved without modification.
- Java import structure is hierarchical (`import com.example.UserService`), modeled as `scoped_identifier` with `scope` + `name` fields in tree-sitter-java grammar. No `import.scm` capture contract exists yet.
- `object_creation_expression` (`new Class(...)`) represents constructor calls in the tree-sitter-java grammar; the `type` child holds the class name, which acts as an owner hint for `local_value_type`. This node shape requires fixture confirmation (`(fixture-confirm)` in the design doc §8 §4).
- `local_variable_declaration` contains a `type_identifier` child (the declared type) and a `variable_declarator` child with a `name` field; this is the scope node for local binding extraction. Node shape also requires fixture confirmation.

## Desired Outcome (To-Be)
- `apps/codemap-search/queries/java/tags.scm` (proposed) captures minimum Java definition and reference nodes using standard tags-compatible capture names (`@definition.class`, `@definition.method`, `@definition.interface`, `@reference.call`); it compiles against `tree_sitter_java::LANGUAGE`.
- `apps/codemap-search/queries/java/navigation.scm` (proposed) captures all required navigation observations per the design doc §8 §4 contract: `import_declaration` → `@nav.import` + `@nav.import.source` + `@nav.import.name`; `method_invocation` → `@nav.call` + `@nav.call.name` + optional `@nav.call.receiver`; `object_creation_expression` → `@nav.call` + `@nav.call.name` (fixture-confirmed); `local_variable_declaration` → `@local.scope` + `@local.type` + `@local.definition` (fixture-confirmed). It compiles against `tree_sitter_java::LANGUAGE`.
- `apps/codemap-search/src/lang/java.rs` exposes a navigation query getter and implements the `LanguageSpec` navigation hook introduced by child 02 and stabilized by child 07, wiring `navigation.scm` via `include_str!` into the concat extraction pass.
- `apps/codemap-search/src/lang/mod.rs` receives the `JavaSpec` navigation hook registration; no trait shape changes are needed — the interface was fixed by child 07. This file is the Wave 6 shared hotspot and must be edited with awareness that children 07–13 all touch it; apply only the minimum addition for Java.
- `tests/fixtures/navigation/java/basic.java` (proposed), `tests/fixtures/navigation/java/expected.tags.json` (proposed), and `tests/fixtures/navigation/java/expected.navigation.json` (proposed) cover: a class with a public method that calls another method on an injected object; an `import com.example.UserService` statement; a `UserService userService = new UserService()` local binding; a `userService.save()` call site. These fixtures gate Java activation.
- After child 10, `.java` files produce `navigation: Some(NavigationFile { ... })` in extracted output; `callees.rs` uses `navigation.calls` as the primary callee source for Java, falling back to text scan when `navigation` is `None`, stale, or warming.
- The design doc's completion condition is satisfied: `import com.example.UserService` followed by `userService.save()` call narrows to the `UserService.save` candidate (when combined with child 05 receiver hint logic or child 04 import resolution, depending on activation order).
- Fallback behavior is preserved: navigation-unsupported files, warming/stale/error states, and query compile failure at runtime all fall back to the existing text-scan approximate path without regression.

## Scope
### In Scope
- Create `queries/java/tags.scm` (proposed) with minimum `@definition.*` and `@reference.*` captures for Java, compiling against `tree_sitter_java::LANGUAGE`.
- Create `queries/java/navigation.scm` (proposed) with the full navigation capture contract from design doc §8 §4: import captures (`@nav.import`, `@nav.import.source`, `@nav.import.name`), call captures (`@nav.call`, `@nav.call.name`, `@nav.call.receiver`), constructor call capture (`@nav.call` on `object_creation_expression` — fixture-confirmed node shape), and local binding captures (`@local.scope`, `@local.type`, `@local.definition` on `local_variable_declaration` — fixture-confirmed).
- Add navigation query getter to `src/lang/java.rs` using `include_str!` and register it via the `LanguageSpec` navigation hook (interface fixed by child 07).
- Add the `JavaSpec` navigation hook entry to `src/lang/mod.rs` (Wave 6 shared hotspot — minimum addition only).
- Create `tests/fixtures/navigation/java/` directory (proposed) with `basic.java`, `expected.tags.json`, and `expected.navigation.json`.
- Verify that `queries/java/symbols.scm` (proposed, created in child 01) is loaded via `include_str!` in `src/lang/java.rs` as the symbols query; do not re-inline `JAVA_QUERY_STR`.

### Out of Scope
- [hard] Do not modify `queries/java/symbols.scm` — that file is child 01's output; child 10 only reads it.
- [hard] Do not add new `LanguageSpec` trait methods — the navigation hook interface was fixed by child 02 and piloted by child 07; child 10 implements, it does not extend.
- [hard] Do not modify `src/callers/callees.rs`, `src/callers/annotate.rs`, or `src/callers/symbols.rs` — the callee/caller wiring uses the generic navigation path introduced by child 02–06; Java activation comes entirely from `JavaSpec` returning a non-`None` navigation query.
- [hard] Do not touch `src/config.rs`, `src/index/engine.rs`, or `src/parser/types.rs` — no new config keys, no format version bump, no type additions are needed for a language activation child.
- [deferred] Static import (`import static com.example.Class.method`) — the design doc contract captures only `scoped_identifier` under `import_declaration`; static import variants with additional path depth may not match the minimum pattern and should fall back gracefully rather than be special-cased here.
- [deferred] Annotation-based owner attribution (e.g., `@Autowired` field injection) — receiver type inference for injected fields requires `value_type_hint` propagation from field declarations, which is beyond the `local_variable_declaration` scope of this child.
- [deferred] Multi-catch, try-with-resources, and lambda call sites — these are edge-case patterns beyond the minimum `method_invocation` / `object_creation_expression` contract.

## Constraints
- `queries/java/navigation.scm` must use exactly the capture names defined in design doc §3 (`@nav.call`, `@nav.call.name`, `@nav.call.receiver`, `@nav.import`, `@nav.import.source`, `@nav.import.name`, `@local.scope`, `@local.type`, `@local.definition`). Deviating from these names breaks the Rust consumer in `src/parser/mod.rs` which routes by capture name string.
- `tags.scm` must NOT be included in the runtime concat pass; it is a compile/fixture validation gate only (design doc §3). Only `symbols.scm` + `navigation.scm` are concatenated for the single runtime `Query::new(...)` pass.
- `object_creation_expression` node shape and `local_variable_declaration` node shape must be confirmed via fixtures before marking Java navigation as activated; both carry `(fixture-confirm)` in the design doc. If a node shape cannot be confirmed, mark the corresponding capture as `(fixture-confirm)` in `navigation.scm` and note the open question.
- `src/lang/mod.rs` is the Wave 6 shared hotspot: children 07–13 all register their language navigation hooks here. Apply only the `JavaSpec` navigation hook entry; do not reformat or restructure adjacent code.
- The runtime concat query must compile successfully against `tree_sitter_java::LANGUAGE`; a compile failure must disable navigation for Java and fall back gracefully, not panic.
- This child depends on child 01 (`scm-extract`) having already created `queries/java/symbols.scm` (proposed) and on child 06 (`lexical-scope`) having completed so the navigation infrastructure (`NavigationFile`, concat pass, `AnnotationRuntimeState`) is in place. Child 07 (`lang-python`) must be complete so the `src/lang/mod.rs` interface is confirmed stable.

## Related Files / Entry Points
- `apps/codemap-search/src/lang/java.rs` (existing) — add navigation query getter at the bottom of the file and implement the `LanguageSpec` navigation hook; the `JAVA_QUERY_STR` inline constant should be removed and replaced by an `include_str!` call pointing at the symbols.scm file created by child 01.
- `apps/codemap-search/src/lang/mod.rs` (existing) — `LanguageSpec` trait definition and `spec_for_ext` / `ALL_SPECS` registry; add the `JavaSpec` navigation hook registration here (Wave 6 shared hotspot).
- `apps/codemap-search/queries/java/symbols.scm` (proposed) — symbols query file created by child 01; child 10 reads it via `include_str!` in `src/lang/java.rs`; do not recreate.
- `apps/codemap-search/queries/java/tags.scm` (proposed) — new file; Java tags-compatible definition/reference gate query for `tree_sitter_java::LANGUAGE`.
- `apps/codemap-search/queries/java/navigation.scm` (proposed) — new file; Java runtime navigation capture query per design doc §8 §4 contract.
- `apps/codemap-search/tests/fixtures/navigation/java/` (proposed) — new directory; `basic.java`, `expected.tags.json`, `expected.navigation.json` fixture files confirming `object_creation_expression` and `local_variable_declaration` node shapes.
- `apps/codemap-search/src/lang/typescript.rs` (existing) — reference implementation: see how the TypeScript navigation query getter and `include_str!` wiring was done in child 02; follow the same pattern for Java.
- `apps/codemap-search/src/lang/python.rs` (existing) — Wave 6 pilot reference: see how Python navigation was registered in child 07; follow the same registration pattern for Java in the mod.rs registry.
- `docs/briefs/2026-06-25-feat-nav-layer-v2-02-nav-types-callee.md` (existing) — child 02 brief; describes the `LanguageSpec` navigation hook interface that `JavaSpec` must implement.
- `docs/briefs/2026-06-25-briefset-nav-layer-v2.md` (proposed) — parent brief for this briefset; see for Wave 6 execution order and set-level acceptance criteria.

## Side Effect Checkpoints
- [ ] All existing symbol and literal extraction results for `.java` files remain identical after the concat-query path change — `@symbol.*` and `@literal.*` capture routing must not regress; verify against existing Java extraction fixtures if any exist, or add a symbols-only fixture.
- [ ] `cargo build` on `apps/codemap-search` succeeds with no new warnings after all changes.
- [ ] `src/lang/mod.rs` compiles correctly with the new `JavaSpec` navigation hook entry alongside sibling Wave 6 language hook entries (Python, Go, Rust, Kotlin, C, C++) — no naming conflicts or trait method signature mismatches.
- [ ] The `java_is_public` visibility helper and `generic_find_owner` owner resolution in `src/lang/java.rs` continue to function correctly after the file is restructured to use `include_str!` for the symbols query; these are distinct from the navigation query path and must not be touched.
- [ ] Navigation disabled fallback fires correctly for Java files when `navigation` is `None` (e.g., in warming/stale/error states); the text-scan callee path in `callees.rs` must not regress for Java.
- [ ] `tags.scm` compiles successfully via `Query::new(...)` against `tree_sitter_java::LANGUAGE`; a compile failure at this gate must surface during development (not silently pass).
- [ ] `navigation.scm` compiles successfully via `Query::new(...)` against `tree_sitter_java::LANGUAGE`; a compile failure must disable navigation for Java and fall back gracefully, not panic.
- [ ] No other Wave 6 language (`lang-python`, `lang-go`, `lang-rust`, `lang-kotlin`, `lang-c`, `lang-cpp`) is affected by the `src/lang/mod.rs` edit; verify sibling specs still pass their own navigation fixture checks.

## Acceptance Criteria
- [ ] `queries/java/tags.scm` compiles successfully via `Query::new(...)` against `tree_sitter_java::LANGUAGE`.
- [ ] `queries/java/navigation.scm` compiles successfully via `Query::new(...)` against `tree_sitter_java::LANGUAGE`.
- [ ] `tests/fixtures/navigation/java/basic.java` fixture with an `import com.example.UserService` statement, a `UserService userService = new UserService()` local binding, and a `userService.save()` call produces `expected.navigation.json` that includes: one `@nav.import` entry with `@nav.import.source` = `com.example` and `@nav.import.name` = `UserService`; one `@nav.call` entry with `@nav.call.name` = `save` and `@nav.call.receiver` = `userService`; one `@nav.call` entry with `@nav.call.name` = `UserService` from `object_creation_expression` (fixture-confirmed node shape).
- [ ] `tests/fixtures/navigation/java/expected.tags.json` matches the actual tags output from running `tags.scm` against `basic.java`, confirming at least one `@definition.class` and one `@definition.method` capture.
- [ ] A `.java` source file processed through the concat extraction pass produces `navigation: Some(NavigationFile { ... })` — not `navigation: None` — confirming Java is navigation-enabled after child 10.
- [ ] A `.java` source file with no method calls produces `navigation: Some(NavigationFile { calls: vec![], ... })` — not `None` — confirming that successful extraction with zero observations is distinguishable from extraction not having run.
- [ ] String literals containing `name(` patterns in `.java` source do NOT appear as `CallSite` entries in `NavigationFile.calls`; only structural `method_invocation` and `object_creation_expression` nodes produce call entries.
- [ ] Java method definition headers (`void save() { ... }`) do NOT appear as `CallSite` entries; only call-expression nodes are captured.
- [ ] `cargo build` on `apps/codemap-search` succeeds with no new warnings.
- [ ] Existing callee and caller results for non-Java files (TypeScript, Python, Go, Rust, Kotlin, C, C++) are not affected by the `src/lang/mod.rs` and `src/lang/java.rs` changes.

## Open Questions
- None — implementation choices are fully bounded by the design doc §8 §4 capture contract, the `(proposed)` token rules, the `LanguageSpec` navigation hook interface fixed by child 02 and piloted by child 07, and the Wave 6 `src/lang/mod.rs` hotspot coordination rule. The two `(fixture-confirm)` items (`object_creation_expression` type node shape and `local_variable_declaration` scope node shape) are delegated to the downstream agent to resolve via fixture testing before marking Java activated.
