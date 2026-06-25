# [feat] Receiver/owner hint based method-candidate narrowing

## Work Type
feat

## Current State (As-Is)
- As of branch `main` (recent commit `13deb03`), `apps/codemap-search/src/callers/callees.rs` `discover_callees` (L17–63) performs a plain text scan for `identifier(` patterns inside a symbol's body; it has no concept of a call receiver or owner type.
- `apps/codemap-search/src/callers/annotate.rs` `render_symbol_annotation` (L108–320) invokes `discover_callees` and `callee_display` with no receiver context; `callee_display` (L71–83) resolves a callee name to its qualified form by counting `fn` definitions in `SymbolIndex`, but has no mechanism to filter by owner type.
- `apps/codemap-search/src/callers/symbols.rs` `SymbolIndex` (L13–20) holds `by_name`, `fn_names`, and `fn_def_counts` only; it exposes no owner-indexed lookup.
- Child 04 (`import-caller`) adds `NavigationIndex` and `LocalBinding` to the snapshot and wires import-alias resolution; child 05 depends on those structures being available.
- `LocalBinding` (defined in `src/parser/types.rs` by child 02) carries `value_type_hint: Option<String>` (populated from `new Foo()` constructor expressions) and `type_hint: Option<String>` (populated from explicit type annotations such as `const user: User`); these fields are available to the caller/callee resolution layer after child 04 completes.
- `CallSite` carries `receiver: Option<String>` (the identifier to the left of `.` in a member-call expression, e.g. `user` in `user.save()`); this field is populated by the navigation extraction pass introduced in child 02.
- When `user.save()` and `file.save()` both exist in scope, `save` has two `fn` definitions; `callee_display` returns the bare name with no disambiguation, and the caller block labels both as `approximate` with no owner context.
- There is no `infer_owner_hint` function, no owner-indexed symbol lookup, and no metrics counter for receiver-hint precise hits or fallbacks anywhere in `src/callers/`.

## Desired Outcome (To-Be)
- A new `infer_owner_hint` function (proposed in `src/callers/symbols.rs`) accepts a receiver identifier name and a slice of `LocalBinding` entries for the current file, and returns `Option<String>` — the inferred type name to use as the owner hint. It checks `value_type_hint` first (`new User()` → `"User"`), then `type_hint` (`const user: User` → `"User"`), then returns `None` if neither is present.
- `apps/codemap-search/src/callers/callees.rs` `discover_callees` is extended to accept an optional `locals: &[LocalBinding]` parameter (or a wrapper containing the `NavigationFile`). For each `CallSite` entry in `navigation.calls` whose `receiver` is `Some`, `infer_owner_hint` is invoked to produce an owner hint. When the owner hint is present, candidate lookup is further filtered to `fn`/`method` definitions whose `owner` field matches the hint string exactly (case-sensitive). A candidate set of exactly 1 after owner+name filtering is the only condition that qualifies for `Precise` attribution.
- A new owner-indexed lookup helper (proposed in `src/callers/symbols.rs`) — `lookup_by_owner_and_name` — accepts `owner: &str` and `name: &str` and returns all `fn`/`method` definitions in `SymbolIndex.by_name` whose owner matches. This is the narrowing step called after `infer_owner_hint` returns `Some`.
- `apps/codemap-search/src/callers/annotate.rs` `annotate_results` passes `navigation` locals into the callee resolution path so `infer_owner_hint` can operate at annotation time.
- Two new metrics counters are added (proposed in the metrics/config layer touched by child 04): `navigation_receiver_hint_precise_count` increments when receiver+name match yields exactly 1 candidate and `Precise` attribution is applied; `navigation_receiver_hint_fallback_count` increments when a receiver is present but owner inference fails or yields 0 or 2+ candidates.
- Receiver-hint precise attribution is **default-off** behind `navigation_context_default` (the existing config key introduced by child 04). It is enabled only after metrics collected in a staging run confirm that the precise conversion rate justifies the narrowing. The feature flag check is evaluated at annotation time in `render_symbol_annotation`.
- All explicit fallback cases (`this.save()`, optional chaining `user?.save()`, destructured call `const { save } = user; save()`, factory return type `getUser().save()`, and interface dispatch where multiple types implement the same method) remain in the approximate path; they do not attempt receiver-hint narrowing.

## Scope
### In Scope
- Add `infer_owner_hint(receiver: &str, locals: &[LocalBinding]) -> Option<String>` to `src/callers/symbols.rs`; check `value_type_hint` before `type_hint`; return `None` if neither field is set for a matching binding name.
- Add `lookup_by_owner_and_name<'a>(owner: &str, name: &str, index: &SymbolIndex<'a>) -> Vec<(&'a ExtractedFile, &'a ExtractedSymbol)>` to `src/callers/symbols.rs`; filter `by_name[name]` to entries whose `sym.owner == Some(owner)` and `sym.kind` is `"fn"` or `"method"`.
- Extend `discover_callees` in `src/callers/callees.rs` to consume `NavigationFile.calls` (introduced by child 02) and apply `infer_owner_hint` + `lookup_by_owner_and_name` when `call.receiver` is `Some`; gate the precise path on the feature flag.
- Thread `NavigationFile` locals into `render_symbol_annotation` in `src/callers/annotate.rs` so the receiver-hint narrowing path receives local bindings.
- Increment `navigation_receiver_hint_precise_count` on each receiver-hint precise hit and `navigation_receiver_hint_fallback_count` on each receiver-hint miss/ambiguity; wire both into the existing metrics sink introduced by child 04.
- Gate entire receiver-hint narrowing on `navigation_context_default == true` at annotation time; when the flag is false, skip `infer_owner_hint` and fall through to the existing approximate path unchanged.
- Document the explicit fallback cases — `this`, optional chaining, destructuring, factory return, interface dispatch — as `// fallback: <reason>` comments in `discover_callees` at the receiver-hint check site.

### Out of Scope
- [hard] `this.save()` implicit-receiver resolution — `this` requires class-scope tracking not available in child 05; falls back to approximate.
- [hard] Optional chaining (`user?.save()`) — tree-sitter `@nav.call.receiver` for optional-chain receivers is not guaranteed to match the binding pattern; falls back.
- [hard] Destructuring (`const { save } = user; save()`) — the call site has no receiver; `call.receiver` is `None`; falls back.
- [hard] Factory return-type inference (`getUser().save()`) — receiver is a call expression, not an identifier; `infer_owner_hint` only handles identifier receivers; falls back.
- [hard] Interface dispatch where multiple concrete types implement the same method — `lookup_by_owner_and_name` returns 2+ candidates; precise path is not taken; falls back.
- [hard] `scope_id`-based binding shadowing — lexical scope refinement is child 06 (`lexical-scope`) scope; child 05 uses the full `locals` list without scope filtering.
- [hard] Receiver-hint narrowing for caller reverse attribution — applying owner hints to the `NavigationIndex`-based caller path is deferred to a later iteration; child 05 covers callee direction only.
- [hard] Do not add new config keys beyond reusing `navigation_context_default`; child 04 owns the config surface.
- [deferred] Per-owner method-signature disambiguation (overloads) — not applicable to TypeScript in child 05; deferred to language-extension children (07–13) as needed.

## Constraints
- `infer_owner_hint` must consult `value_type_hint` before `type_hint`; constructor-based hints (`new User()`) are more precise than annotation-based hints (`const user: User`) because the annotation may be an interface type. The priority order must be preserved.
- Receiver-hint `Precise` attribution is allowed only when `lookup_by_owner_and_name` returns exactly 1 candidate **and** all six `Precise` preconditions from design doc §6 are satisfied (not warming, no refresh error, navigation extraction succeeded, budget not exceeded, exactly 1 candidate, tags/navigation fixtures passed).
- The feature is default-off; the metrics counters must fire even when the feature flag is off (to enable offline analysis before enabling). The flag gates the `Precise` attribution path, not the counter increment path.
- The existing `discover_callees` text-scan fallback path for `navigation: None` files must not be touched; child 05 only adds a branch inside the `navigation.calls`-primary path introduced by child 02.
- `infer_owner_hint` must not panic when `locals` is empty; it returns `None` in that case.
- `lookup_by_owner_and_name` must not return non-`fn`/non-`method` kinds; the kind filter is required regardless of whether the owner matches.

## Related Files / Entry Points
- `apps/codemap-search/src/callers/symbols.rs` (existing) — add `infer_owner_hint` and `lookup_by_owner_and_name` here; `SymbolIndex` struct at L13–20 and `build_symbol_index` at L22–43 are the context.
- `apps/codemap-search/src/callers/callees.rs` (existing) — extend `discover_callees` at L17–63; the navigation-primary call path is introduced by child 02; receiver-hint narrowing branches off inside that path.
- `apps/codemap-search/src/callers/annotate.rs` (existing) — `render_symbol_annotation` at L108–320 calls `discover_callees` (L264); thread `NavigationFile` locals through here; `annotate_results` at L409–469 is the outer caller that constructs the annotation runtime context.
- `apps/codemap-search/src/parser/types.rs` (existing) — `LocalBinding` struct (introduced by child 02) with `value_type_hint` and `type_hint` fields; `CallSite` struct with `receiver: Option<String>` field; review field names before implementing `infer_owner_hint`.
- `apps/codemap-search/src/config.rs` (existing) — `navigation_context_default` config key (introduced by child 04); read at annotation time to gate the receiver-hint precise path.
- `docs/briefs/2026-06-25-feat-nav-layer-v2-04-import-caller.md` (proposed) — predecessor child; confirms `NavigationIndex`, `LocalBinding`, and `navigation_context_default` are available before child 05 starts.

## Side Effect Checkpoints
- [ ] `discover_callees` text-scan fallback path (triggered when `navigation` is `None`) is unmodified and produces identical output compared to pre-child-05 behavior for all non-navigation files.
- [ ] `callee_display` name qualification logic in `callees.rs` (L71–83) is not regressed; unambiguous non-receiver calls still produce qualified names via the existing `SymbolIndex` path.
- [ ] `annotate_results` signature or call sites in `src/tools/search/mod.rs` are not changed beyond threading the existing `NavigationFile` reference already present after child 02/04; no new public API surface is added.
- [ ] `render_symbol_annotation` byte budget check (L308–318) still fires correctly after adding the receiver-hint path; precise callee lines must be counted against the budget.
- [ ] `navigation_receiver_hint_precise_count` and `navigation_receiver_hint_fallback_count` counters increment for each receiver-bearing call site regardless of whether the feature flag is enabled.
- [ ] When `navigation_context_default` is `false` (default), all callee attribution labels remain `approximate`; no `Precise` label appears in output for receiver-bearing calls.
- [ ] `infer_owner_hint` returns `None` for a receiver name that has no matching `LocalBinding`, ensuring the existing approximate path fires without error.
- [ ] `lookup_by_owner_and_name` returns an empty vector when no `fn`/`method` with matching owner exists, causing the candidate count to be 0 and triggering fallback rather than a spurious `Precise`.

## Acceptance Criteria
- [ ] `cargo build` succeeds with no new warnings on the `apps/codemap-search` crate after all changes.
- [ ] Given a TypeScript fixture with `const user = new User(); user.save();` and a single `User.save` method in the snapshot, `discover_callees` returns `["User::save"]` (qualified) when `navigation_context_default` is `true` and the `AnnotationRuntimeState` is non-warming.
- [ ] Given the same fixture with both `User.save` and `File.save` in the snapshot, `discover_callees` does not mark either as `Precise`; output remains `approximate` because `lookup_by_owner_and_name("User", "save")` returns exactly 1 but the owner hint must match exactly — `File.save` is not returned; however the test must confirm the single `User.save` candidate qualifies correctly.
- [ ] Given `const user: User = getUser(); user.save();` with a single `User.save` in the snapshot, `infer_owner_hint("user", locals)` returns `Some("User")` via the `type_hint` path, and `User::save` is returned as the callee.
- [ ] Given `this.save()`, `infer_owner_hint("this", locals)` returns `None` (no binding named `"this"` in locals), and the call falls back to the approximate path.
- [ ] Given `user?.save()`, the call receiver is either absent or does not produce a valid binding lookup, and the call falls back to the approximate path.
- [ ] `infer_owner_hint` with an empty `locals` slice returns `None` without panic.
- [ ] `lookup_by_owner_and_name` with a `name` that exists in `by_name` but whose only entries have `kind == "variable"` (not `"fn"` or `"method"`) returns an empty vector.
- [ ] When `navigation_context_default` is `false`, `discover_callees` does not invoke `lookup_by_owner_and_name`; the `navigation_receiver_hint_fallback_count` counter still increments for each receiver-bearing call site.
- [ ] `navigation_receiver_hint_precise_count` is 0 and `navigation_receiver_hint_fallback_count` equals the number of receiver-bearing call sites when the feature flag is disabled.
- [ ] `navigation_receiver_hint_precise_count` increments by 1 for the `new User()` + `user.save()` fixture when the feature flag is enabled and the index contains exactly one `User.save`.

## Open Questions
- None — implementation choices are bounded by the design doc §6 `infer_owner_hint` algorithm, the `LocalBinding.value_type_hint`/`type_hint` priority order, the six `Precise` preconditions in §6, the explicit fallback list in §8 Stage 6, and the metrics-gated default-off activation in §7. The dependency on child 04 for `NavigationIndex` and `LocalBinding` is resolved by the wave-5 ordering in `final_plan.md`.
