# [feat] Introduce scope_id for lexical shadowing detection

## Work Type
feat

## Current State (As-Is)
- As of branch `main` (recent commit `13deb03`), `apps/codemap-search/src/callers/callees.rs` `discover_callees` (L17–63) performs a plain character-by-character text scan for `identifier(` patterns inside the function body range. There is no concept of lexical scope; a local variable named `bar` and an import alias named `bar` are both treated as calls to the same global `fn`.
- `apps/codemap-search/src/parser/types.rs` does not yet define `NavigationFile`, `CallSite`, `LocalBinding`, or any `scope_id` field (these are added by child 02). As of child 05 completion, `CallSite` and `LocalBinding` carry an `Option<usize>` `scope_id` field as specified in the design doc §5, but the field is always `None` — no scope nodes are captured from the query, and no assignment logic exists in the parser.
- `apps/codemap-search/queries/typescript/navigation.scm` (proposed, created in child 02) contains `@nav.call`, `@nav.call.name`, `@nav.import.*`, and `@local.definition` captures, but it does not yet include `@local.scope` or `@local.reference` captures. The scope boundary is therefore unobservable to the Rust consumer.
- `apps/codemap-search/src/parser/mod.rs` `TreeSitterExtractor::extract` (L34–288) has no logic to number scope nodes or to populate `scope_id` on call sites, references, or local bindings. It routes `@nav.*` and `@local.*` captures into `NavigationFile` but does not track the enclosing `@local.scope` node during that routing pass.
- `apps/codemap-search/src/callers/callees.rs` (as extended by child 02–05) uses `navigation.calls` as its primary callee source, with same-file priority (child 03) and import alias resolution (child 04). When a call site name collides with both a local binding and an import alias, the current logic consults the import alias and may produce a precise attribution that is semantically wrong because the local binding shadows the import alias within the enclosing scope.
- `apps/codemap-search/src/callers/annotate.rs` `annotate_results` (L409–469) similarly has no scope-awareness in caller attribution. A call site with a name that is both a local binding and an import alias in the calling file is not distinguished; the import alias path may be incorrectly followed.
- The design doc §5 documents two-stage shadowing: stage 1 (line-range approximation, introduced in child 03) and stage 2 (`scope_id`-based lexical judgment introduced in this child). Stage 2 is the precise gate for the shadowing case described in §9: "a local binding shadows an import alias."

## Desired Outcome (To-Be)
- `apps/codemap-search/queries/typescript/navigation.scm` (proposed, created in child 02) is extended with `@local.scope`, `@local.definition`, and `@local.reference` captures that mark lexical scope boundaries and the local bindings defined within them, following the design doc §3 capture contract.
- `apps/codemap-search/src/parser/mod.rs` `TreeSitterExtractor::extract` assigns a stable, per-file monotonically increasing `scope_id: usize` to each `@local.scope` node encountered during the single-pass `QueryCursor`. Each `CallSite`, `LocalBinding`, and `ReferenceSite` whose query range falls within a `@local.scope` node receives that scope's id in its `scope_id` field. The assignment must be deterministic: same source text always produces the same `scope_id` values.
- `apps/codemap-search/src/callers/callees.rs` callee resolution checks, before consulting the import alias map, whether a `LocalBinding` with the same name as the call site exists in the same scope or an ancestor scope. If such a local binding exists and is not itself a known `fn`-kind symbol, the import alias lookup is skipped and the call is marked as falling back (`navigation_fallback_reason = local_shadow`).
- `apps/codemap-search/src/callers/annotate.rs` caller attribution applies the same scope-priority check: a call site whose name is shadowed by a local binding in the same or an ancestor scope does not proceed to import alias resolution; it falls back to the existing name-match + `approximate` path.
- When `scope_id` cannot be confirmed (the `@local.scope` capture is absent for a call site, or the scope-assignment pass did not reach the call site's range), the system falls back conservatively to the stage-1 line-range approximation introduced in child 03. The fallback is silent unless the ambiguity would cause a precise attribution — in that case `navigation_fallback_reason = scope_unconfirmed` is recorded.
- When the standard `@local.scope` / `@local.definition` / `@local.reference` captures are insufficient to stably assign `scope_id` for a given language or grammar (e.g., because the grammar represents scopes as synthetic ranges not as explicit nodes), the implementation falls back to the line-range approximation and does not attempt to introduce language-specific scope heuristics.
- A local `bar` that shadows an import alias `bar` is correctly handled: `bar()` at a call site enclosed in the local binding's scope does not follow the import alias to a precise callee; instead it falls back. A `bar()` call site outside that local binding's scope correctly continues through the import alias lookup as before.

## Scope
### In Scope
- Extend `apps/codemap-search/queries/typescript/navigation.scm` (proposed, created in child 02) with `@local.scope`, `@local.definition`, and `@local.reference` captures that identify lexical scope boundaries in TypeScript/TSX.
- Add `scope_id: Option<usize>` population logic to `apps/codemap-search/src/parser/mod.rs` `TreeSitterExtractor::extract`: assign a stable per-file `usize` id to each `@local.scope` capture node, then propagate that id to `CallSite`, `LocalBinding`, and `ReferenceSite` whose source range is enclosed by that scope node.
- Add same-scope and ancestor-scope local binding priority check in `apps/codemap-search/src/callers/callees.rs`: before import alias resolution, check if any `LocalBinding` with the same name covers the call site's `scope_id` or its ancestor scopes; if so, skip import alias and fall back.
- Apply the same scope-priority check in `apps/codemap-search/src/callers/annotate.rs` for caller attribution: a call site whose name is locally shadowed in scope does not follow the import alias path.
- Add `local_shadow` and `scope_unconfirmed` to the `navigation_fallback_reason` counter/log values.
- Ensure the conservative fallback path is taken when `scope_id` is `None` on either the call site or the candidate local binding — preserve stage-1 line-range behavior as the floor.

### Out of Scope
- [hard] Do not modify `apps/codemap-search/src/parser/types.rs` struct definitions — `CallSite.scope_id`, `LocalBinding.scope_id`, and `ReferenceSite.scope_id` fields as `Option<usize>` are already added by child 02; this child only populates them.
- [hard] Do not change `EXTRACTION_FORMAT_VERSION` — `scope_id` fields carry `#[serde(default)]` added by child 02; populating a previously-`None` field from `None` to `Some(n)` changes stored JSON but does not require a new version bump because consumers already tolerate `None` (an old index simply never gets precise scope attribution until it is re-indexed naturally).
- [hard] Do not introduce scope captures for languages other than TypeScript/TSX — scope extension for Python, Go, Rust, Java, Kotlin, C, C++ belongs to children 07–13.
- [hard] Do not touch `apps/codemap-search/src/config.rs` or add new config keys — all budget and activation config is child 04's scope.
- [deferred] Full `@local.reference` tracking for all identifier uses — stage 1 only needs scope boundaries and definitions. `ReferenceSite` population from `@local.reference` captures may be stored at zero extra cost if they appear naturally in the query pass, but it is not required.
- [deferred] Scope-aware disambiguation for namespace import member calls (`api.foo()`) — covered by child 04's source hint logic; this child only handles the local-binding-shadows-import-alias case.
- [deferred] Parent-scope chain traversal beyond immediate enclosing scope — the initial implementation needs to handle at most one level of scope nesting (function body scope vs. module scope). Full nested scope chain traversal can be added later if metrics show it matters.

## Constraints
- The `scope_id` assignment pass must remain within the existing single `QueryCursor` pass in `TreeSitterExtractor::extract`. Do not add a second parse or a second `QueryCursor` sweep to gather scope boundaries — the pass must stay single-pass as mandated by the design doc §3.
- `scope_id` values are per-file-only identifiers. They must not be stored in a cross-file index or compared across files.
- When `scope_id` is `None` on a call site (e.g., the call is at module top-level outside any `@local.scope` node), the absence of a scope does not itself imply shadowing. Only a `LocalBinding` that shares the same `scope_id` (or whose scope is an ancestor of the call site's scope) triggers the shadowing fallback.
- The fallback path must not regress recall: if shadowing detection fails to confirm, the import alias resolution path from child 04 must still execute as before. This child adds an earlier-exit condition, not a replacement of the import alias path.
- This child depends on child 05 (`receiver-hint`) having completed. Child 05 and this child share `apps/codemap-search/src/callers/annotate.rs` as a conflict surface; they must not run in parallel.

## Related Files / Entry Points
- `apps/codemap-search/src/parser/mod.rs` (existing) — `TreeSitterExtractor::extract` at L34–288; add `@local.scope` capture handling and `scope_id` assignment to `CallSite`/`LocalBinding` here within the existing single-pass routing block.
- `apps/codemap-search/src/callers/callees.rs` (existing) — `discover_callees` at L17–63; add same-scope local binding priority check before import alias lookup (added by child 04) within the navigation-based resolution path.
- `apps/codemap-search/src/callers/annotate.rs` (existing) — `annotate_results` at L409–469 and `render_symbol_annotation` at L108–320; apply scope-priority check before import alias resolution for caller attribution.
- `apps/codemap-search/queries/typescript/navigation.scm` (proposed) — created in child 02; extend with `@local.scope`, `@local.definition`, `@local.reference` captures for TypeScript/TSX scope boundaries.
- `apps/codemap-search/src/parser/types.rs` (existing) — reference only; `CallSite.scope_id`, `LocalBinding.scope_id` field definitions are already present from child 02; no struct changes needed.
- `docs/briefs/2026-06-25-feat-nav-layer-v2-05-receiver-hint.md` (proposed) — predecessor child; child 06 starts after child 05 completes and `apps/codemap-search/src/callers/annotate.rs` changes from child 05 are merged.

## Side Effect Checkpoints
- [ ] All existing callee results for TypeScript/TSX files where no local-shadow collision exists remain identical after this change — `scope_id` population must not alter the resolution path for non-shadowing call sites.
- [ ] The single-pass constraint is preserved: `cargo build` on `apps/codemap-search` must not introduce a second `QueryCursor` invocation or a second `Query::new` call in the extraction path.
- [ ] Call sites at module top-level (outside any `@local.scope` node, `scope_id = None`) are not incorrectly flagged as shadowed — the fallback to import alias lookup must fire for these.
- [ ] `apps/codemap-search/src/callers/annotate.rs` changes from child 05 (`receiver-hint`) are not regressed — the scope-priority check must be additive, not a replacement of the owner-hint logic added in child 05.
- [ ] The existing stage-1 line-range shadowing approximation from child 03 remains the floor: when `scope_id` is `None` on either the call site or the local binding, the behavior is identical to child 03's output — no regression in cases that child 03 already handled conservatively.
- [ ] `navigation_fallback_reason = local_shadow` is recorded when a local binding shadows an import alias and the call falls back, enabling metrics comparison before/after this child.
- [ ] Children 07–13 (language extension wave) are not affected — this child touches only TypeScript/TSX queries and language-neutral Rust logic; `src/lang/mod.rs` and language-specific files are not modified.

## Acceptance Criteria
- [ ] `cargo build` succeeds with no new warnings on `apps/codemap-search` after all changes.
- [ ] The extended `navigation.scm` with `@local.scope` / `@local.definition` / `@local.reference` captures compiles successfully via `Query::new(...)` against both the TypeScript grammar and the TSX grammar; a compile failure on either grammar disables scope-id assignment for that grammar and falls back gracefully without panic.
- [ ] Given a TypeScript fixture with `import { bar } from "./util"` followed by `function foo() { const bar = localValue(); bar(); }`: `bar()` inside `foo` does NOT produce a precise callee attribution pointing to the `./util` import; the result is `approximate` with `navigation_fallback_reason = local_shadow`.
- [ ] Given the same fixture with `bar()` called at module top-level (outside `foo`, where the local `bar` binding is not in scope): the call DOES proceed through import alias resolution and produces a precise callee attribution pointing to `./util` when exactly one exported function named `bar` is found there.
- [ ] When the `@local.scope` capture is absent for a call site (i.e., `scope_id = None`), the resolution path does not change compared to child 05 output — no new fallback is introduced for scope-free call sites.
- [ ] `scope_id` values assigned by the parser are stable: parsing the same TypeScript source file twice produces identical `scope_id` values on all `CallSite` and `LocalBinding` entries.
- [ ] `navigation_fallback_reason = scope_unconfirmed` is recorded when a potential shadowing case is detected but `scope_id` cannot be confirmed (e.g., both call site and local binding have `scope_id = None` and only the line-range approximation is available).
- [ ] The `@local.scope` / `@local.definition` captures in the extended `navigation.scm` produce the correct scope node ranges on the TypeScript navigation fixture (`tests/fixtures/navigation/typescript/`) — the `expected.navigation.json` is updated to reflect the new capture results.

## Open Questions
- None — implementation choices are bounded by the design doc §5 type definitions, the `scope_id` two-stage shadowing plan in §5, the `@local.scope`/`@local.definition`/`@local.reference` capture contract in §3, and the constraints above. Child 05 (`receiver-hint`) must be merged before this child begins; the annotate.rs conflict surface is resolved by the serial execution order in the final plan.
