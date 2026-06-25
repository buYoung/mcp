# Brief Set: Tree-sitter Navigation Layer v2 — caller/callee attribution precision

## Purpose
- Switch caller/callee discovery from text-based scanning to tree-sitter structural analysis to reduce false positives and raise attribution precision. The 13 child briefs are divided into a foundational refactor (Wave 1), TypeScript-first validation (Waves 2–5), and per-language extension (Wave 6); each child is an independent execution unit with explicit predecessor/successor dependencies.
- Assign `Precise` attribution only when the target can be confirmed; otherwise keep the existing `approximate` fallback. The patterns validated on TypeScript are extended to 7 additional languages.

## Child Briefs
- [ ] `docs/briefs/2026-06-25-refactor-nav-layer-v2-01-scm-extract.md` — SCM file split (Stage 0); exists to move the 9 languages' inline query constants into `queries/<lang>/symbols.scm`, establishing the query-file reference base that every subsequent child relies on.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-02-nav-types-callee.md` — Add NavigationFile types + TS tags/navigation + callee switch (Stages 1 and 2); exists to fix the common interface — the `NavigationFile` type and the EXTRACTION_FORMAT_VERSION bump — through which downstream children access the navigation field.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-03-same-file-prio.md` — same-file function priority (Stage 3); exists to reduce false positives by applying same-file priority and an fn/method kind filter to callee candidate narrowing.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-04-import-caller.md` — import alias resolution + NavigationIndex-based caller attribution (Stages 4 and 5); exists to raise caller attribution precision through import alias back-tracing and a callsite budget.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-05-receiver-hint.md` — receiver/owner hint based method caller candidate narrowing (Stage 6); exists to narrow method-call candidates by receiver type, with metrics-validated conditional activation.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-06-lexical-scope.md` — introduce `scope_id` to handle local binding shadowing (Stage 7); exists to guarantee a correct fallback when a local binding shadows an import alias.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-07-lang-python.md` — write Python navigation query and activate attribution (Stage 8 / Python); exists to activate attribution via per-language Python tags+navigation queries and fixtures, and to fix the `src/lang/mod.rs` change pattern as the Wave 6 pilot.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-08-lang-go.md` — write Go navigation query and activate attribution (Stage 8 / Go); exists to activate Go attribution, including a fixture that distinguishes package selectors from value receivers.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-09-lang-rust.md` — write Rust navigation query and activate attribution (Stage 8 / Rust); exists to activate Rust attribution, including glob import fallback and trait method fallback fixtures.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-10-lang-java.md` — write Java navigation query and activate attribution (Stage 8 / Java); exists to activate Java attribution, including an import + class method owner fixture.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-11-lang-kotlin.md` — write Kotlin navigation query and activate attribution (Stage 8 / Kotlin); exists to activate Kotlin attribution, including a fixture that handles `tree_sitter_kotlin_ng` grammar quirks.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-12-lang-c.md` — write C navigation query and activate attribution (Stage 8 / C); exists to activate C attribution, including a free-function-priority and static-exclusion fixture.
- [ ] `docs/briefs/2026-06-25-feat-nav-layer-v2-13-lang-cpp.md` — write C++ navigation query and activate attribution (Stage 8 / C++); exists to activate C++ attribution, including overload/template/operator fallback and out-of-line member owner fixtures.

## Execution Order
- Wave 1 (strictly sequential): run `2026-06-25-refactor-nav-layer-v2-01-scm-extract.md` alone. Creating the 9 languages' `queries/<lang>/symbols.scm` is a precondition for every subsequent child.
- Wave 2 (strictly sequential): run `2026-06-25-feat-nav-layer-v2-02-nav-types-callee.md` alone. The `NavigationFile` type and the EXTRACTION_FORMAT_VERSION bump must be fixed before downstream children can access the navigation field.
- Wave 3 (strictly sequential): run `2026-06-25-feat-nav-layer-v2-03-same-file-prio.md` alone. Same-file priority can be applied only once Wave 2's navigation call data is available.
- Wave 4 (strictly sequential): run `2026-06-25-feat-nav-layer-v2-04-import-caller.md` alone. It adds import alias resolution and the NavigationIndex on top of Wave 3's same-file lookup helper.
- Wave 5 (must be serialized): run in the order `2026-06-25-feat-nav-layer-v2-05-receiver-hint.md` → `2026-06-25-feat-nav-layer-v2-06-lexical-scope.md`. Both children modify `src/callers/annotate.rs`, so parallel execution is forbidden.
- Wave 6 (parallelizable, pilot first): run `2026-06-25-feat-nav-layer-v2-07-lang-python.md` first as the pilot to fix the `src/lang/mod.rs` interface change pattern. After the pilot completes, `08-lang-go`, `09-lang-rust`, `10-lang-java`, `11-lang-kotlin`, `12-lang-c`, `13-lang-cpp` can run in parallel.

## Dependencies
- `2026-06-25-feat-nav-layer-v2-02-nav-types-callee.md` depends on `2026-06-25-refactor-nav-layer-v2-01-scm-extract.md` — the `queries/<lang>/symbols.scm` files must exist for Wave 2's `include_str!` references to compile.
- `2026-06-25-feat-nav-layer-v2-03-same-file-prio.md` depends on `2026-06-25-feat-nav-layer-v2-02-nav-types-callee.md` — the same-file priority filter requires the `NavigationFile` type and navigation call data.
- `2026-06-25-feat-nav-layer-v2-04-import-caller.md` depends on `2026-06-25-feat-nav-layer-v2-03-same-file-prio.md` — it builds import alias back-tracing and the NavigationIndex on top of the same-file lookup helper.
- `2026-06-25-feat-nav-layer-v2-05-receiver-hint.md` depends on `2026-06-25-feat-nav-layer-v2-04-import-caller.md` — the receiver hint filter is added after the NavigationIndex and LocalBinding structures are fixed.
- `2026-06-25-feat-nav-layer-v2-06-lexical-scope.md` depends on `2026-06-25-feat-nav-layer-v2-05-receiver-hint.md` — `scope_id` is introduced after the receiver hint work, without conflicting edits to `src/callers/annotate.rs`.
- `2026-06-25-feat-nav-layer-v2-07-lang-python.md` depends on `2026-06-25-feat-nav-layer-v2-06-lexical-scope.md` — the Python extension starts from the navigation interface validated on TypeScript/JavaScript.
- `2026-06-25-feat-nav-layer-v2-08-lang-go.md` depends on `2026-06-25-feat-nav-layer-v2-06-lexical-scope.md` — same reason.
- `2026-06-25-feat-nav-layer-v2-09-lang-rust.md` depends on `2026-06-25-feat-nav-layer-v2-06-lexical-scope.md` — same reason.
- `2026-06-25-feat-nav-layer-v2-10-lang-java.md` depends on `2026-06-25-feat-nav-layer-v2-06-lexical-scope.md` — same reason.
- `2026-06-25-feat-nav-layer-v2-11-lang-kotlin.md` depends on `2026-06-25-feat-nav-layer-v2-06-lexical-scope.md` — same reason.
- `2026-06-25-feat-nav-layer-v2-12-lang-c.md` depends on `2026-06-25-feat-nav-layer-v2-06-lexical-scope.md` — same reason.
- `2026-06-25-feat-nav-layer-v2-13-lang-cpp.md` depends on `2026-06-25-feat-nav-layer-v2-06-lexical-scope.md` — same reason.

## Parallelization
- Waves 1–5: fully sequential execution. Parallel execution is impossible because each wave's output is a precondition for the next.
- Within Wave 5: `05-receiver-hint` and `06-lexical-scope` jointly modify `src/callers/annotate.rs`, so parallel execution is forbidden. Start 06 after 05 completes.
- Within Wave 6 — pilot first: `07-lang-python` runs first as a standalone pilot to confirm that the activation pattern — overriding the `navigation_query()` hook (default implementation `None`) added by child 02 in each language's `src/lang/<lang>.rs` — works for a non-TypeScript language. Child 07 does not structurally edit `src/lang/mod.rs`.
- Within Wave 6 — parallel allowed (after 07 completes): `08-lang-go`, `09-lang-rust`, `10-lang-java`, `11-lang-kotlin`, `12-lang-c`, `13-lang-cpp` each modify only their own `src/lang/<lang>.rs` (C/C++ use `c_family/<lang>.rs`) and `queries/<lang>/` paths and reference `src/lang/mod.rs` read-only, so they can run in parallel without conflicting with one another. They start after the child 07 pilot confirms the override pattern.
- `12-lang-c` and `13-lang-cpp` only read the `src/lang/c_family/` shared helpers (`name_for_cfn`, `c_has_static_storage`), so there is no conflict when run in parallel.

## Conflict Hotspots
- `src/lang/mod.rs` — the `LanguageSpec` trait's `navigation_query()` hook (default implementation `None`) is added exactly once by child 02 for TypeScript. Per design doc §8 Stage 8, Wave 6 children (07–13) **do not structurally edit** `src/lang/mod.rs`; each language activates by overriding `navigation_query()` in its own `src/lang/<lang>.rs` (C/C++ use `c_family/<lang>.rs`). Every spec is already registered in `spec_for_ext`/`ALL_SPECS`, so no registration entry is added either. Therefore no `src/lang/mod.rs` merge conflict arises among Wave 6 children. Child 07 (Python) acts only as the pilot that confirms this override pattern works for a non-TypeScript language.
- `AnnotationRuntimeState` struct — child 02 **introduces it alone** (the three callee-suppression fields `is_warming`/`has_refresh_error`/`is_dead_or_stale`, design doc §6). Child 04 **reuses and extends** this struct without redefining it, applying it to the caller direction. No other child creates the struct anew.
- The three navigation config keys (`navigation_context_default`, `navigation_callsite_budget`, `navigation_store_references`) — per briefset decision 6, **all three are owned by child 04**. Only child 04 adds the three keys to `src/config.rs` and bumps `CONFIG_VERSION` once. Child 05 only reads `navigation_context_default`.
- `src/callers/annotate.rs` — both 05 (`receiver-hint`) and 06 (`lexical-scope`) modify it. Conflicts are avoided by serial execution within Wave 5.
- `src/callers/symbols.rs` — 03 (`same-file-prio`), 04 (`import-caller`), and 05 (`receiver-hint`) modify it in sequence. It is protected by the serial execution rule of Waves 3–5.
- `src/parser/types.rs` — 02 (`nav-types-callee`) adds the `NavigationFile` type, and subsequent children only read it. There is no conflict risk once Wave 2 completes.
- `src/index/engine.rs` — 02 (`nav-types-callee`) adds the navigation field storage logic. Subsequent children only read it.
- `src/lang/*.rs` (Wave 1) — child 01 modifies the 9 language files at once. Wave 1 is a standalone child, so it has no internal conflict. However, starting another child before 01 completes produces an `include_str!` path error.

## Shared Constraints
- The `EXTRACTION_FORMAT_VERSION` bump is performed only in child 02. Subsequent children do not change the version.
- `queries/<lang>/symbols.scm` is a file created in child 01. Subsequent children only reference it and do not modify it.
- ASM (`queries/asm/symbols.scm`) is included in child 01's split scope but is excluded from navigation-layer activation. `.s`, `.S`, and `.asm` files are not call-site capture targets, and no child writes `tags.scm`/`navigation.scm` for them.
- Wave 6 children (07–13) activate by overriding the `LanguageSpec::navigation_query()` hook (default implementation `None`) that child 02 fixed, each in its own `src/lang/<lang>.rs`, and do not structurally edit `src/lang/mod.rs` (design doc §8 Stage 8). Child 07 is the pilot that confirms this override pattern on a non-TypeScript language.
- The Kotlin grammar crate is `tree_sitter_kotlin_ng`. Do not confuse it with other language crate names (`tree_sitter_python`, `tree_sitter_go`, etc.).
- Every `.scm` file is embedded into the binary via `include_str!`; no file path is looked up at runtime.
- Each Wave 6 child validates matching with a fixture (`tests/fixtures/navigation/<lang>/`) before activating attribution. It does not activate without a fixture.

## Global Acceptance Criteria
- [ ] Wave 1 complete: the 9 languages' `queries/<lang>/symbols.scm` are created, and the existing symbol extraction behavior is confirmed unchanged (`cargo build` succeeds, existing tests pass).
- [ ] Wave 2 complete: the `NavigationFile` type is added to `src/parser/types.rs`, the TypeScript navigation queries (`tags.scm`, `navigation.scm`) are written, and EXTRACTION_FORMAT_VERSION is bumped. `cargo check` passes.
- [ ] Wave 4 complete (config keys confirmed): the three navigation config keys — `navigation_context_default`, `navigation_callsite_budget`, `navigation_store_references` — are all added to `src/config.rs` (`ResolvedConfig` field + `CONFIG_TEMPLATE` comment + `MIGRATIONS` entry), and `CONFIG_VERSION` is confirmed bumped once. All three keys are owned by child 04 (briefset decision 6). Child 05's read reference to `navigation_context_default` is confirmed to resolve against an existing key.
- [ ] Wave 5 complete: TypeScript caller/callee attribution is switched to tree-sitter based, and metrics validation confirms precision improves over the prior baseline. Quantitative criterion: after navigation activation, `navigation_precise_count` records at least 1 on the TypeScript fixture (i.e. at least one caller or callee is confirmed precise), and the §7 counters confirm a decrease in false-positive call sites on the same fixture compared to navigation disabled. This is a required gate before Wave 6 starts.
- [ ] Wave 6 pilot (07) complete: the `src/lang/mod.rs` navigation activation hook interface is fixed, and the Python fixture validation passes. This is a precondition for starting the 08–13 parallel execution.
- [ ] Wave 6 full (07–13) complete: navigation queries and fixtures for the 7 additional languages (Python, Go, Rust, Java, Kotlin, C, C++) are written and attribution is activated. Each language's `tests/fixtures/navigation/<lang>/` fixture validation passes.
- [ ] ASM exclusion confirmed: `.s`, `.S`, and `.asm` files are excluded from navigation caller/callee attribution, and only `queries/asm/symbols.scm` is confirmed to exist.
- [ ] Full set complete: `cargo build --release` succeeds, existing symbol search behavior is preserved, and navigation attribution precision metrics improvement is confirmed.

## Open Questions
- None — the per-language unconfirmed node names (in particular the `class`/`interface` shared node in Kotlin's `tree_sitter_kotlin_ng` grammar) and fixture matching details are delegated to each child's Open Questions, and nothing at the set level requires an advance decision.
