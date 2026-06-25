# [feat] Apply same-file function priority and pin fn/method kind filter in callee narrowing

## Work Type
feat

## Current State (As-Is)
- As of branch `main` (recent commit `13deb03`), `apps/codemap-search/src/callers/callees.rs` performs callee discovery exclusively via a text scan: the function body is read from disk, every `identifier(` pattern is matched, and the resulting identifiers are intersected with a global `fn`-name set (`SymbolIndex.fn_names`). There is no priority ordering — all matching names from the entire snapshot are treated as equally valid candidates.
- `apps/codemap-search/src/callers/symbols.rs` builds a `SymbolIndex` (line 22–43) with three fields: `by_name` (all symbols keyed by name), `fn_names` (global set of `fn`-kind names), and `fn_def_counts` (per-name `fn` definition count). The index has no same-file lookup path and no kind filter beyond the `fn` intersection in `callees.rs`.
- The current callee discovery in `callees.rs` (line 17–63) checks `ident != sym.name` and `fn_names.contains(&ident)`, but it does not check `sym.kind == "method"` — method-kind symbols in `fn_names` are included incidentally, not by an explicit `fn || method` filter.
- `callee_display` in `callees.rs` (line 71–83) qualifies a callee name only when exactly one `fn` of that name exists in the snapshot. This qualification is name-count based, not priority-based, and it does not distinguish a same-file definition from a global one.
- Child 02 (`nav-types-callee`) introduces `navigation.calls` as the primary callee source and adds a callee candidate lookup helper stub in `src/callers/symbols.rs`. Child 03 builds on top of that helper to add same-file priority and kind filtering; it does not exist yet.
- When the same function name appears in multiple files (e.g., `save`, `validate`, `render`), the current code either over-attributes to the first-found definition or renders the bare name without qualification. There is no mechanism to prefer the definition in the same file as the call site.

## Desired Outcome (To-Be)
- `apps/codemap-search/src/callers/symbols.rs` exposes a `lookup_same_file_candidates` helper (or equivalently-named function) that, given a callee name and the call-site file path, returns the subset of `SymbolIndex.by_name[name]` entries whose `file_path` matches the call-site file and whose `kind` is `"fn"` or `"method"`.
- `apps/codemap-search/src/callers/callees.rs` applies a two-phase candidate ordering when resolving each `CallSite` from `navigation.calls`: (1) query same-file candidates first; if exactly one is found, return `Precise(candidate)`; (2) if same-file candidates are zero or more than one, fall through to global candidates filtered to `fn`/`method` kind; if exactly one global candidate remains, return `Precise(candidate)`; otherwise return `Fallback`.
- The `fn`/`method` kind filter is explicit and unconditional: any snapshot symbol whose `kind` is neither `"fn"` nor `"method"` is excluded from the candidate set at every priority level. A same-name `class`, `variable`, `field`, or other kind does not narrow the candidate count toward `Precise`.
- When same-file candidates exist but number more than one (ambiguity within the same file), the resolution falls back to `Fallback` — no `Precise` result is returned. Global candidates are not consulted as a tiebreaker in this case; same-file ambiguity is terminal.
- When same-file candidates are zero and global `fn`/`method` candidates number exactly one, that global candidate is used for `Precise`. This preserves the existing `callee_display` qualification behavior while integrating it into the new priority-ordered path.
- The text-scan fallback path (`discover_callees` via disk read) remains intact and is still used when `navigation` is `None`, when the snapshot is potentially stale, or when the language is not navigation-enabled. The same-file priority logic is applied only on the navigation-based `navigation.calls` path introduced by child 02.
- `callee_display` in `callees.rs` continues to render qualified names for unambiguous global resolutions; the new same-file priority path uses the same `qualified_name` helper for rendering.

## Scope
### In Scope
- Add `lookup_same_file_candidates(name, file_path, index)` (or equivalent) in `src/callers/symbols.rs` that filters `SymbolIndex.by_name[name]` to entries matching `file_path` and `kind == "fn" || kind == "method"`.
- Modify the navigation-based callee resolution path in `src/callers/callees.rs` to apply same-file priority ordering: same-file `fn`/`method` candidates first, global `fn`/`method` candidates as fallback when same-file yields zero results.
- Pin the explicit `fn`/`method` kind filter at the candidate lookup level so non-fn/non-method same-name symbols are excluded unconditionally regardless of priority level.
- Enforce the same-file ambiguity rule: if same-file `fn`/`method` candidates number ≥ 2, resolve to `Fallback` without consulting global candidates.
- Ensure the text-scan fallback path (`discover_callees` using `read_workspace_file`) remains unchanged and is still invoked for `navigation: None`, stale-disk, and non-navigation-enabled language conditions.

### Out of Scope
- [hard] Import alias resolution (`local_name` → `imported_name` + `source_hint`) — this is child 04 (`import-caller`) scope. Child 03 does not consult `NavigationFile.imports`.
- [hard] `NavigationIndex` (`calls_by_name` reverse index) and caller-direction budget — child 04 and child 05 scope.
- [hard] Receiver/owner hint inference (`call.receiver` → type-hint lookup) — child 05 (`receiver-hint`) scope.
- [hard] `scope_id`-based lexical shadowing — child 06 (`lexical-scope`) scope. Child 03 does not attempt to determine whether a local binding shadows an import alias; that is explicitly deferred to child 06.
- [hard] `local_shadow` check (find local binding by `call.name` and `scope_id`) — deferred to child 06; child 03 has no `scope_id` available.
- [hard] Config keys (`navigation_context_default`, `navigation_callsite_budget`, `navigation_store_references`) — child 04 scope; do not touch `src/config.rs`.
- [deferred] Same-file priority for the caller-direction (reverse) resolution — child 04 and child 05 concern; child 03 only covers callee direction.
- [hard] Do not modify `src/parser/types.rs`, `src/index/engine.rs`, `src/parser/mod.rs`, or any language query files — child 03 is narrowly scoped to `src/callers/symbols.rs` and `src/callers/callees.rs`.

## Constraints
- The same-file ambiguity rule (≥ 2 same-file `fn`/`method` candidates → `Fallback`, do not consult global candidates) is not negotiable. Falling through to global candidates when same-file is ambiguous would produce over-attribution identical to the current behavior and would defeat the purpose of same-file priority.
- The kind filter (`fn` or `method` only) must be applied before priority comparison — it is not a post-filter on an already-narrowed candidate set. A symbol with `kind == "class"` or `kind == "variable"` that shares a name with the callee must be excluded before counting candidates at any priority level.
- The text-scan fallback path in `discover_callees` (the `read_workspace_file` + `identifier(` loop at `callees.rs` line 17–63) must not be removed or gated; it is the sole fallback for `navigation: None`, stale-disk states, and unsupported languages, and removing it would cause recall regression for those cases.
- This child depends on child 02 (`nav-types-callee`) having already introduced `NavigationFile`, `CallSite`, and the `navigation.calls` primary path in `callees.rs`. The same-file lookup helper added here is consumed by the navigation-based resolution path, not the text-scan path.
- `callee_display` and `qualified_name` are existing helpers in `callees.rs` and `src/callers/mod.rs`; child 03 reuses them rather than duplicating display logic.

## Related Files / Entry Points
- `apps/codemap-search/src/callers/symbols.rs` (existing) — add `lookup_same_file_candidates` helper here; `SymbolIndex.by_name` at line 14 is the data source; `build_symbol_index` at line 22 does not need modification.
- `apps/codemap-search/src/callers/callees.rs` (existing) — `discover_callees` at line 17 is the text-scan path (preserve unchanged); the navigation-based resolution path added by child 02 is where same-file priority ordering is wired in; `callee_display` at line 71 is reused for rendering.
- `docs/briefs/2026-06-25-briefset-nav-layer-v2.md` (proposed) — parent brief for this briefset; execution order and set-level acceptance criteria.
- `docs/briefs/2026-06-25-feat-nav-layer-v2-02-nav-types-callee.md` — prerequisite child brief; establishes the `NavigationFile`/`CallSite` types and the navigation-based callee path that child 03 extends.

## Side Effect Checkpoints
- [ ] The existing text-scan callee path (`discover_callees` in `callees.rs` lines 17–63) still executes without modification for `navigation: None`, stale-disk, and non-navigation-enabled language conditions — verify no regression in callee count for TypeScript files without navigation support and for all non-TypeScript files.
- [ ] `callee_display` (line 71) still qualifies a callee name when exactly one global `fn` of that name exists in the snapshot — verify the unambiguous-global case renders a qualified name and the ambiguous case renders a bare name.
- [ ] The `SymbolIndex` built by `build_symbol_index` is not mutated or extended by child 03 — verify `by_name`, `fn_names`, and `fn_def_counts` fields are unchanged after this change.
- [ ] Callee results for a TypeScript function whose callees all have unique global names (zero same-file candidates) remain identical before and after the same-file priority change — the global-single-candidate path must produce the same output as the prior `callee_display` path.
- [ ] Callee results for a TypeScript function that calls a name present in the same file AND in other files switch to the same-file candidate — verify the call is attributed to the same-file definition rather than the global one.
- [ ] `AnnotationRuntimeState.is_warming == true` still suppresses navigation-based precise resolution — the same-file priority path must respect the warming suppression introduced by child 02 and not bypass it.

## Acceptance Criteria
- [ ] `cargo build` succeeds with no new warnings on the `apps/codemap-search` crate after all changes.
- [ ] Given a snapshot with function `save` defined in both `apps/foo/a.ts` and `apps/bar/b.ts`, when resolving a `CallSite { name: "save", .. }` from a `NavigationFile` in `apps/foo/a.ts`, the resolution returns the `a.ts` definition as `Precise`, not the `b.ts` definition.
- [ ] Given a snapshot with two `save` definitions in `apps/foo/a.ts` (overloads or two distinct functions with the same name), when resolving a `CallSite { name: "save", .. }` from `a.ts`, the resolution returns `Fallback` — same-file ambiguity is terminal; the `b.ts` definition is not consulted.
- [ ] Given a snapshot with `save` defined only in `apps/bar/b.ts` (no same-file definition), when resolving a `CallSite { name: "save", .. }` from `apps/foo/a.ts`, the resolution returns `Precise(b.ts/save)` when that is the single global `fn`/`method` candidate.
- [ ] A snapshot symbol with `kind == "class"` (or any non-fn, non-method kind) sharing a callee name does not count as a candidate at any priority level — a call site whose only matching snapshot entry is a `class` resolves to `Fallback`, not `Precise`.
- [ ] A snapshot symbol with `kind == "method"` is included as a valid candidate at both same-file and global priority levels — verify a `method`-kind symbol resolves to `Precise` under the same single-candidate rule as `fn`-kind.
- [ ] The text-scan `discover_callees` function returns the same results as before this change for a file with `navigation: None` — no regression in callee names or count.
- [ ] `callee_display` for a name with exactly one global `fn` definition still returns the qualified form (`Owner::name` or `file::name`) after the same-file priority change — the rendering helper is not affected by the priority logic.

## Open Questions
- None — the same-file priority rule, kind filter, and ambiguity fallback semantics are fully specified in design doc §6 (candidate priority, candidate filter) and §8 Stage 3. The two user-owned decisions that could have affected this child (split vs merge of stages, config key placement) are resolved in `decisions.md` and are handled by child 04.
