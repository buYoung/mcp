# [feat] Add C++ navigation query, fixtures, and callee/caller wiring

## Work Type
feat

## Current State (As-Is)
- As of branch `main` (recent commit `13deb03`), `apps/codemap-search/src/lang/c_family/cpp.rs` defines `CppSpec` implementing `LanguageSpec` with `CPP_QUERY_STR` at L24, but the `LanguageSpec` trait has no `navigation_query` hook — the trait definition in `src/lang/mod.rs` (L54–206) exposes only `grammar`, `query`, `extensions`, and symbol/owner hooks.
- `CPP_QUERY_STR` will be extracted to `queries/cpp/symbols.scm` by child 01 (`scm-extract`); after child 01 completes, `src/lang/c_family/cpp.rs` reads it via `include_str!`. Child 13 depends on child 01's output being present.
- No `queries/cpp/tags.scm` or `queries/cpp/navigation.scm` file exists. The `queries/` directory tree is entirely absent from the repository as of `main`.
- The `LanguageSpec` trait in `src/lang/mod.rs` has no `navigation_query()` hook; child 02 (`nav-types-callee`) adds it for TypeScript first. Child 13 must not add this hook itself — it must wire the hook that child 02 defines.
- `cpp_outofline_owner()` (L107–129 in `cpp.rs`) already extracts the owning class name from `qualified_identifier`-scoped out-of-line definitions (`void Foo::bar() {}`). This logic handles the `Class::method` owner extraction at the symbol-extraction layer but is not surfaced as a `nav.call.receiver` capture in any navigation query.
- `cpp_member_is_exported()` and `cpp_nearest_access_specifier()` (L206–229) provide `access_specifier`-based export detection for class members, relevant for import-source-hint precise candidate filtering.
- `c_has_static_storage` (imported from `super`) marks C-family functions with static storage as file-local (`is_exported = false`); the same rule applies to C++ free functions, and the same exclusion must be reflected in the precise-candidate filter.
- `src/lang/c_family/` contains `c.rs`, `cpp.rs`, and `mod.rs`. C++ and C share `name_for_cfn` and `c_has_static_storage` helpers from `src/lang/c_family/mod.rs`; these are read-only shared helpers, no coordination needed with child 12 (`lang-c`).
- callee discovery for `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx`, and `.h` files relies entirely on the text scan in `src/callers/callees.rs` with no structural call-site awareness — overloaded function names, template instantiations, and `operator()` calls all produce false positives indistinguishable from actual call sites.
- No fixture directory `tests/fixtures/navigation/cpp/` exists.

## Desired Outcome (To-Be)
- `queries/cpp/tags.scm` (proposed) captures minimum C++ definition and reference nodes using `@definition.*` and `@reference.*` capture names. It compiles against `tree_sitter_cpp::LANGUAGE` and passes a fixture gate confirming node shapes for `function_definition`, `field_declaration` with `function_declarator`, `class_specifier`, `struct_specifier`, `enum_specifier`, and `call_expression`.
- `queries/cpp/navigation.scm` (proposed) captures `@nav.import.*`, `@nav.call`, `@nav.call.name`, `@nav.call.receiver`, `@local.definition`, `@local.type`, `@local.scope` for the patterns defined in the design doc §8 C++ capture contract: `preproc_include` for both `string_literal` and `system_lib_string` paths; `call_expression` with `identifier`, `field_expression`, and `qualified_identifier` function shapes; `declaration` and `init_declarator` local binding shapes (fixture-confirm required per design doc).
- `queries/cpp/navigation.scm` compiles against `tree_sitter_cpp::LANGUAGE`. A compile failure logs a warning and disables navigation for all C++ extensions; it does not panic.
- `src/lang/c_family/cpp.rs` implements the `navigation_query()` hook (defined by child 02 on the `LanguageSpec` trait) by returning `Some(include_str!("../../../queries/cpp/navigation.scm"))`, wiring the navigation pass for all extensions served by `CppSpec` (`.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx`).
- `src/lang/mod.rs` receives no structural changes in this child. The `navigation_query` hook is already added by child 02; this child only implements it in `CppSpec`. `src/lang/mod.rs` is a Wave 6 shared hotspot — child 13 must not re-add or modify the trait definition.
- `tests/fixtures/navigation/cpp/basic.cpp` (proposed), `tests/fixtures/navigation/cpp/expected.tags.json` (proposed), and `tests/fixtures/navigation/cpp/expected.navigation.json` (proposed) form the activation gate. The fixture must cover: a free function call, a member method call via dot (`obj.method()`), a pointer member call via arrow (`ptr->method()`), a scoped call (`Scope::func()`), an out-of-line member definition (`void Foo::bar() {}`), a `#include "file.h"` directive, a `#include <stdlib.h>` system include, and at least one local variable declaration.
- overload/template/operator calls produce 2 or more candidates and always fall back to `approximate`; the fallback path is exercised and fixture-confirmed.
- out-of-line member definitions (`void Foo::bar() {}`) correctly identify `Foo` as the owner via the existing `cpp_outofline_owner()` logic; the `qualified_identifier` scope node in the navigation query produces a `nav.call.receiver` for `Scope::func()` call-expression forms (fixture-confirm: scope node shape).
- `init_declarator` local binding capture (`(init_declarator declarator: (identifier) @local.definition) @local.scope`) is fixture-confirmed or narrowed per actual tree-sitter-cpp node shapes before enabling.
- callee discovery for C++ files uses `navigation.calls` as the primary source when navigation extraction succeeds and the index is current; the existing text-scan path is the fallback for `navigation: None`, stale index, warming state, and error state — consistent with the fallback contract established by child 02.
- `navigation_fallback_reason=unsupported_language` is never emitted for C++ extensions; C++ is a navigation-enabled language after this child lands.

## Scope
### In Scope
- Create `queries/cpp/tags.scm` (proposed) with minimum C++ `@definition.*`/`@reference.*` tags-compatible captures for `function_definition`, method declarations, `class_specifier`, `struct_specifier`, `enum_specifier`, `call_expression`.
- Create `queries/cpp/navigation.scm` (proposed) with the capture contract from design doc §8 C++ section: `preproc_include` import captures, `call_expression` with `identifier`/`field_expression`/`qualified_identifier` function shapes, `declaration`/`init_declarator` local binding shapes (with fixture-confirm gate).
- Implement `navigation_query()` on `CppSpec` in `src/lang/c_family/cpp.rs`, returning the `include_str!` path to `navigation.scm`.
- Create fixture directory and files: `tests/fixtures/navigation/cpp/basic.cpp` (proposed), `tests/fixtures/navigation/cpp/expected.tags.json` (proposed), `tests/fixtures/navigation/cpp/expected.navigation.json` (proposed).
- Fixture must confirm: free function calls, dot/arrow member calls, scoped calls, out-of-line member definitions, `#include` directives (quoted and angled), local declarations, and an overload/template scenario exercising the fallback path.
- Verify `queries/cpp/tags.scm` and `queries/cpp/navigation.scm` compile via `Query::new(...)` against `tree_sitter_cpp::LANGUAGE`; compile failure must be a logged warning, not a panic.
- Confirm the concat runtime pass (`symbols.scm` + `navigation.scm`) produces correct `@nav.*` / `@local.*` capture output alongside existing `@symbol.*` / `@literal.*` output for the C++ fixture.

### Out of Scope
- [hard] Do not add or modify the `navigation_query` hook signature in `src/lang/mod.rs` — that is child 02's responsibility. Implement only the `CppSpec` method body.
- [hard] Do not touch `src/lang/c_family/c.rs` or `src/lang/c_family/mod.rs` beyond reading shared helpers — these are child 12's (`lang-c`) territory or shared read-only helpers.
- [hard] Do not create or modify `queries/cpp/symbols.scm` — that file is created by child 01 (`scm-extract`). Child 13 depends on it being present; do not recreate or inline it.
- [hard] Do not touch `src/callers/callees.rs` primary/fallback routing logic — the navigation-first + text-scan-fallback switch is child 02's responsibility. Child 13 only enables C++ so the existing routing can reach it.
- [hard] Do not modify `EXTRACTION_FORMAT_VERSION` — that bump happened in child 02. No new serialization fields are added in this child.
- [deferred] C++ module import (`import <module>`, `import "header"`) — C++20 modules use different node shapes not in the current tree-sitter-cpp grammar support; leave as fallback.
- [deferred] Template specialization owner attribution (e.g., `Foo<int>::bar()`) — the existing `cpp_outofline_owner` skips `template_type` scope names (returns `None`); this is the correct conservative behavior and is not changed here.
- [deferred] `ReferenceSite` population for C++ — follow the same deferral policy as child 02; do not store `ReferenceSite` unless the concat query produces them at zero extra cost.
- [deferred] `operator()` precise resolution — operator calls are structurally captured as `call_expression` but map to `operator()` names and cannot be precisely resolved to a unique definition; always fall back.

## Constraints
- `queries/cpp/tags.scm` must NOT be included in the runtime concat pass. It is a compile/fixture gate only (design doc §3). Only `symbols.scm` and `navigation.scm` enter the concat runtime query.
- The runtime extractor must remain a single `Query::new(...)` + single `QueryCursor` pass per file; do not add a second pass for C++ navigation captures.
- All `(fixture-confirm)` items in the design doc §8 C++ navigation contract (`field_expression` argument node for arrow vs dot, `qualified_identifier` scope node shape, `declaration` local binding, `init_declarator` local binding) must be verified against actual tree-sitter-cpp node shapes via fixture before the query pattern is activated. If the node shape does not match, replace or omit the pattern and document the discrepancy.
- `src/lang/mod.rs` is read-only for this child. Per design doc §8 Stage 8, no Wave-6 child structurally edits `src/lang/mod.rs`: the `navigation_query()` method (default `None`) is defined by child 02, and child 13 only adds the override `impl` body on `CppSpec` in `src/lang/c_family/cpp.rs`. Because Wave 6 starts only after child 02 (and the child 07 pilot) complete, the trait method is guaranteed present; do not add a new trait method or registration entry to `src/lang/mod.rs`.
- `src/lang/c_family/` helpers (`name_for_cfn`, `c_has_static_storage`, `cpp_outofline_owner`) are read-only shared with child 12 (`lang-c`). Child 13 must not rename, move, or modify these helpers. If a new shared C/C++ helper is needed for navigation, add it to `src/lang/c_family/mod.rs` only after confirming no conflict with child 12 activity.
- The `.h` extension is served by `CppSpec` (not `CSpec`) per the existing `spec_for_ext` routing in `src/lang/mod.rs` (L219). Navigation query wiring via `CppSpec::navigation_query()` therefore automatically covers `.h` files; no special-casing is needed.
- Wave 6 ordering: child 07 (`lang-python`) runs as pilot to confirm the `navigation_query` hook pattern in `src/lang/mod.rs`. Once child 07 is complete, the `src/lang/mod.rs` interface is stable and child 13 can safely implement `CppSpec::navigation_query()`. Child 13 must not redefine the interface observed in child 07.

## Related Files / Entry Points
- `apps/codemap-search/src/lang/c_family/cpp.rs` (existing) — `CppSpec` struct at L232; add `navigation_query()` impl here; `CPP_QUERY_STR` at L24 will have been moved to the proposed queries/cpp/symbols.scm by child 01.
- `apps/codemap-search/src/lang/mod.rs` (existing) — `LanguageSpec` trait at L54; `navigation_query` hook is added by child 02; read only to confirm the hook signature before implementing in `CppSpec`; do not modify.
- `apps/codemap-search/src/lang/c_family/mod.rs` (existing) — `name_for_cfn`, `c_has_static_storage` shared helpers; read-only reference for child 13.
- `queries/cpp/symbols.scm` (proposed) — created by child 01 (`scm-extract`); required as the `include_str!` source for the symbols half of the runtime concat query; must be present before child 13 work begins.
- `queries/cpp/tags.scm` (proposed) — new file; C++ tags-compatible definition/reference gate query; compile against `tree_sitter_cpp::LANGUAGE` only; not included in runtime pass.
- `queries/cpp/navigation.scm` (proposed) — new file; C++ runtime navigation capture query; wired via `CppSpec::navigation_query()`.
- `tests/fixtures/navigation/cpp/basic.cpp` (proposed) — source fixture covering free function calls, dot/arrow member calls, scoped calls, out-of-line member definition, `#include` directives, overload scenario.
- `tests/fixtures/navigation/cpp/expected.tags.json` (proposed) — tags capture expectations for `basic.cpp`.
- `tests/fixtures/navigation/cpp/expected.navigation.json` (proposed) — navigation capture expectations for `basic.cpp`.
- `docs/briefs/2026-06-25-briefset-nav-layer-v2.md` (proposed) — parent brief for this briefset; see for execution order and set-level acceptance criteria.
- `docs/briefs/2026-06-25-feat-nav-layer-v2-02-nav-types-callee.md` (existing) — child 02 defines `NavigationFile`, `CallSite`, `LocalBinding`, `ImportEntry`, and the `navigation_query` trait hook that child 13 implements; read for interface contract.
- `docs/briefs/2026-06-25-feat-nav-layer-v2-12-lang-c.md` (proposed) — sibling child 12 (`lang-c`); shares `src/lang/c_family/` read-only helpers; do not coordinate edits to those helpers without checking child 12 status.

## Side Effect Checkpoints
- [ ] All existing C++ symbol/literal extraction fixture results remain identical after wiring the navigation concat pass — `@symbol.*` and `@literal.*` captures must not regress for `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx`, and `.h` files.
- [ ] `src/lang/c_family/c.rs` (`CSpec`) behavior is unaffected — C navigation wiring belongs to child 12; child 13 must not accidentally enable C navigation through the C++ path.
- [ ] The `.h` extension, which is served by `CppSpec` (not `CSpec`), correctly activates C++ navigation rather than being left un-wired. Confirm `.h` files receive `navigation: Some(...)` after extraction.
- [ ] `cpp_outofline_owner()` continues to produce correct owner names for out-of-line member definitions (`void Foo::bar() {}`) — the existing symbol-level owner extraction must not be disrupted by the navigation query additions.
- [ ] The navigation compile failure path for C++ does not affect TypeScript, Python, Go, Rust, Java, Kotlin, or C navigation — failure isolation is per-grammar, per-language.
- [ ] `cargo build` for `apps/codemap-search` succeeds with no new warnings after all changes.
- [ ] The `queries/cpp/navigation.scm` concat with `queries/cpp/symbols.scm` (from child 01) compiles as a single query against `tree_sitter_cpp::LANGUAGE` — test the full concat string, not each file independently.
- [ ] Overload/template/operator call scenarios in the fixture produce `navigation: Some(NavigationFile { calls: [...] })` with the call name captured, but callee resolution returns `approximate` (more than one candidate) — confirm no regression from false precise attribution.

## Acceptance Criteria
- [ ] `queries/cpp/tags.scm` compiles successfully via `Query::new(...)` against `tree_sitter_cpp::LANGUAGE`; a compile failure produces a logged warning and does not panic.
- [ ] `queries/cpp/navigation.scm` compiles successfully via `Query::new(...)` against `tree_sitter_cpp::LANGUAGE`; a compile failure produces a logged warning and disables navigation for all C++ extensions, not a panic.
- [ ] The concat string `symbols_scm + navigation_scm` compiles as a single query against `tree_sitter_cpp::LANGUAGE` without error.
- [ ] `tests/fixtures/navigation/cpp/basic.cpp` extraction produces `navigation: Some(NavigationFile { calls: [...] })` with at least one `CallSite` entry for a free function call, one for a dot-member call, one for a scoped `Scope::func()` call, and one for an arrow-member call.
- [ ] A `#include "file.h"` directive in `basic.cpp` produces an `ImportEntry` with `source = "file.h"` and appropriate `ImportKind`.
- [ ] A `#include <stdlib.h>` directive in `basic.cpp` produces an `ImportEntry` with `source = "stdlib.h"` and appropriate `ImportKind`.
- [ ] A C++ source file with no function calls produces `navigation: Some(NavigationFile { calls: vec![], ... })` — not `None`.
- [ ] A file for a non-C++ language does not regress: `navigation` field production is unchanged for TypeScript, Python, Go, Rust, Java, Kotlin, and C files.
- [ ] `tests/fixtures/navigation/cpp/expected.tags.json` and `tests/fixtures/navigation/cpp/expected.navigation.json` match the actual capture output for `basic.cpp` — both files pass the fixture comparison gate.
- [ ] All `(fixture-confirm)` items in the design doc §8 C++ section (`field_expression` argument node for arrow vs dot, `qualified_identifier` scope node shape, `declaration` declarator shape, `init_declarator` declarator shape) are either confirmed correct or replaced/omitted with a code comment explaining the discrepancy.
- [ ] An overload scenario in the fixture (two functions with the same name, different signatures) does not produce a `Precise` callee — it falls back to `approximate` because the candidate count exceeds 1.
- [ ] `cargo build` succeeds on `apps/codemap-search` with no new warnings after all changes in this child.
- [ ] The out-of-line member definition fixture case (`void Foo::bar() {}`) correctly shows `owner = "Foo"` in the extracted symbol output — existing `cpp_outofline_owner()` behavior is preserved.
- [ ] `src/lang/c_family/c.rs` symbol/literal extraction output is byte-for-byte identical before and after child 13 changes — no side effect on C language behavior.

## Open Questions
- None — the C++ navigation contract is fully specified in design doc §8 with `(fixture-confirm)` markers flagging which node shapes require empirical verification. The Wave 6 ordering (child 07 as pilot before child 13) resolves the `navigation_query` trait interface uncertainty. The `(fixture-confirm)` items are resolved during implementation by inspecting tree-sitter-cpp grammar node-types.json and running the fixture gate; they do not require a user decision.
