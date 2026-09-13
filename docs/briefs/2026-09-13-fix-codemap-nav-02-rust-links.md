# [fix] Preserve proven Rust caller links

## Work Type
fix

## Current State (As-Is)
- [confirmed] Inspect the current working tree before applying this plan; the author reviewed HEAD `16192bc8a6f44a89c9a5863232ebcce849b9f2cb` plus uncommitted changes on 2026-09-13. Evidence: repository Git status; the reviewed implementation is not identical to that commit alone.
- [confirmed] `render_symbol_annotation()` checks `own_def_count >= cfg.caller_omit_def_threshold` before calling `precise_navigation_callers()`. Evidence: `apps/codemap-search/src/callers/annotate.rs`, the `Too-many-definitions short-circuit` branch. At the threshold, this path skips the attempt to obtain source-confirmed callers.
- [confirmed] `SourceResolver` already follows source/module/import evidence and masks test regions when loading source. Evidence: `apps/codemap-search/src/callers/resolution.rs`, `source()`, `resolve_call()`, `rust_lookup()`, and `imports_in_scope()`; preserve this filtering rather than diagnosing unfiltered candidates as an established defect.
- [confirmed] Corral's common gateway imports `crate::features::platform::window`. Its platform module conditionally reexports `self::macos::{listener, permission, window}` for `target_os = "macos"` and the Windows module for `not(target_os = "macos")`. Evidence: the inspected `use` and `#[cfg(...)]` clauses in the two Corral entry points below.
- [confirmed] `move_window_bounds_by_id()` has different calls under those platform conditions, while `move_window_to_bounds()` delegates to `window::move_window_to_bounds_by_id()`. Evidence: the inspected function bodies in Corral's `common/window_gateway.rs`.
- [confirmed] The user reported C2's five-definition omission and an unresolved internal call. This author verified the dispatch and short-circuit code, not an end-to-end reproduction of that response on a pinned binary.
- [inferred] Prioritizing confirmed callers and evaluating available platform/import evidence can remove some manual hops between the gateway and native implementation. Confirm the exact recoverable edges using the C2 replay and source-traced target definitions; do not promise that all unresolved receivers become resolvable.

## Reproduction
- Use a disposable copy of `/Users/buyong/workspace/private/corral` on macOS with a pinned baseline binary and recorded configuration. Preserve the original repository and live MCP processes.
- Locate both exact identifiers before replaying C2, and record the original payload and any checkout-root substitution separately. Save full text rather than extracting only the omission notice.
- Exercise both the neutral analysis context and an explicitly known macOS target context once the candidate supports it; do not substitute the host OS for a project's compilation target.
- Frequency: the code short-circuit is deterministic when the compatible definition count reaches the configured threshold. The supplied five-definition output is user-reported, not an independently measured frequency.
- Observed: callers are omitted before the precise path runs, and some `window::...` callees remain unresolved. Expected: proven links survive name-count suppression, while genuinely ambiguous or budget-limited relations retain an explicit notice and no guessed definition.

## Desired Outcome (To-Be)
- Follow source-confirmed callers and callees by exact definition file and line even when five or more compatible definitions share a name.
- Distinguish the regular movement and history-restoration routes through the common gateway, selected platform module, and native implementation.
- Apply known module paths, import aliases, reexports, receiver/type evidence, and platform conditions before deciding that a relation is unresolved.
- Show the remaining uncertainty and budget limits without turning a partially resolved or unavailable scan into an empty-caller claim.

## Scope
### In Scope
- Correct the caller-suppression ordering and preserve confirmed results separately from the approximate-name fallback.
- Improve the Rust source/import path needed by C2, including conditional module/reexport selection under an explicit analysis target.
- Keep precise/approximate/unresolved attribution and definition locations consistent in search, read, and grep relation views.
- Carry relevant target/configuration inputs into source caches and final consumers, and invalidate them when those inputs change.
- Add the approved positive and negative regression cases using the existing Rust caller and MCP test infrastructure.
### Out of Scope
- [hard] Do not change Corral's movement, Undo, cache, or platform-selection behavior.
- [hard] Do not resolve a call by choosing the nearest spelling, first same-name definition, or host platform when target evidence is absent.
- [deferred] Full Rust compiler semantics, build-script execution, proc-macro evaluation, whole-program dynamic dispatch, and all Cargo feature combinations are not required.
- [deferred] Event-specific matching and generic live-output/expansion controls are owned by the sibling briefs.
- [deferred] The in-flight native macro and language-parser work remains outside this child.

## Constraints
- Consume the presentation contract from `docs/briefs/evidence/codemap-nav/live-controls.json` before changing relation rendering.
- Keep `caller_omit_def_threshold`, `common_name_threshold`, scan/callsite budgets, caller/callee list caps, and runtime unavailable-state handling effective for the uncertain fallback. A high name count must not itself hide a proven link.
- Retain source/test exclusion semantics and proof of current declaration identity. The source resolver's existing test masking is counterevidence against a blanket claim that raw snapshots bypass test exclusion.
- Treat `#[cfg(not(target_os = "macos"))]` exactly as written; do not narrow it to Windows-only logic.
- Use an explicit, recorded analysis-target input where platform selection is needed. Missing target values, unsupported predicates, conflicting aliases, or remaining multiple candidates stay unresolved or condition-labelled.
- Support boolean composition of understood predicates without claiming arbitrary `cfg_attr`, feature, environment, or build-script evaluation. Inventory the observed predicates in Stage 1 and define the supported subset before patching.
- Keep source spans exact and retain existing confidence vocabulary. Do not mark a relation precise solely because an import name or receiver type has one textual match.
- Preserve any already-proven results when a budget prevents resolving the remaining population, and distinguish unresolved items from omitted items.

## Related Files / Entry Points
- `apps/codemap-search/src/callers/annotate.rs` — reproduce and correct `render_symbol_annotation()` ordering around `precise_navigation_callers()`.
- `apps/codemap-search/src/callers/callees.rs` — keep callee definition locations and unresolved presentation consistent with caller results.
- `apps/codemap-search/src/callers/resolution.rs` — inspect Rust lookup, imports, module destination, source masking, and cache inputs.
- `apps/codemap-search/src/callers/resolution/local.rs` — preserve the shared non-Rust resolution boundary if common helpers change.
- `apps/codemap-search/src/callers/symbols.rs` — distinguish compatible-name counts from evidence-backed target identity.
- `apps/codemap-search/src/callers/source.rs`, `apps/codemap-search/src/callers/test_code.rs` — retain syntax and test-region filtering boundaries.
- `apps/codemap-search/src/config.rs`, `apps/codemap-search/src/config_template.toml`, `apps/codemap-search/src/config_template.ko.toml` — route any new target option through loading, cache invalidation, migration, and both templates.
- `apps/codemap-search/src/tools/search/mod.rs`, `apps/codemap-search/src/tools/live_symbols/render.rs` — verify final source/import confidence and location display across surfaces.
- `apps/codemap-search/tests/e2e/mcp.rs`, `apps/codemap-search/tests/e2e/cross_feature.rs` — extend actual tool-boundary coverage after the caller-level reproduction.
- `apps/codemap-search/scripts/public_validation.py` — reuse its query and small Rust/Go regression operations.
- `/Users/buyong/workspace/private/corral/apps/desktop/src-tauri/src/features/platform/common/window_gateway.rs` — read the two movement routes and the `window` import without editing them.
- `/Users/buyong/workspace/private/corral/apps/desktop/src-tauri/src/features/platform/mod.rs` — read the conditional reexports that determine the gateway's target.
- `/Users/buyong/workspace/private/corral/apps/desktop/src-tauri/src/features/platform/macos/window/native_client.rs` — confirm terminal macOS definitions and the history-read path by identifier.
- `docs/briefs/evidence/codemap-nav/rust-relations.json` (proposed) — publish the resolution contract, C2 comparison, and proven/remaining edges.

## Execution Plan
### Stage 1 — Reproduce suppression and trace conditional imports
- Starts when: `docs/briefs/evidence/codemap-nav/live-controls.json` has an accepted contract, successful legacy checks, and `side_effects_clear=true`, and the C2 source checkout is available.
- Work: Pin C2's binary/configuration/source state, enumerate exact same-name definitions, and trace both movement routes through imports and reexports. Separate the confirmed threshold short-circuit from unproven platform/type-resolution causes. Specify the observed target predicates and a bounded explicit-target input.
- No-op when: the selected baseline already preserves all source-proven links at the threshold and passes the positive/negative C2 and target-context checks.
- No-op handoff: publish that evidence and the existing resolution policy at `docs/briefs/evidence/codemap-nav/rust-relations.json` (proposed) with `outcome=already-satisfied`, then let the parent start the event child. If proof fails, activate this child's correction and re-verification route first.
- Deliverable: `docs/briefs/evidence/codemap-nav/rust-reproduction.json` (proposed) with raw C2 output, definition inventory, traced expected edges, target assumptions, predicate subset, and suppression/control cases.
- Verify: `Inspect render_symbol_annotation and replay C2 through a pinned MCP binary`; Inputs: C2, the non-empty same-name definition population, and the inspected Corral module/reexport clauses; Expected: the current omission branch or already-satisfied behavior is pinned, and each expected target has a source citation.
- Ends when:
  - [x] The reproduction differentiates same-name suppression, insufficient import/target evidence, and budget/runtime unavailability.
  - [x] The normal-movement and history-restoration paths have separate source-grounded expected-edge lists.
- Handoff: Stage 2 receives `docs/briefs/evidence/codemap-nav/rust-reproduction.json`.
- Replan when: the observed failure comes from another cause or a required link needs unsupported compiler semantics. Return the evidence to this child and parent, narrow the correction to proven causes, and keep unsupported cases explicit before continuing.
- Worker decision: choose where to carry explicit target facts and how to cache condition evaluation, while preserving neutral defaults and caller-selected inputs.

### Stage 2 — Resolve before suppressing uncertain callers
- Starts when: Stage 1 has a pinned failure and supported evidence envelope.
- Work: Preserve proven caller links before applying the ambiguous-name fallback, resolve the supported conditional-import path, and carry confidence, target context, and partial-budget state through all consumers.
- Deliverable: the corrected implementation and `docs/briefs/evidence/codemap-nav/rust-policy.json` (proposed) defining target inputs, supported predicates, provenance fields, confidence rules, and fallback behavior.
- Verify: `Run the listed package checks separately`; Inputs: `cargo check --locked`, `cargo test --locked --lib callers::` from `apps/codemap-search`, using the package implementation and its non-empty caller tests plus the approved threshold/import/condition cases; Expected: exit 0 with proven links retained and negative controls left unresolved.
- Ends when:
  - [x] A source-proven caller survives the configured same-name threshold.
  - [x] Explicit macOS and neutral/unknown contexts produce their documented, different evidence outcomes without guessing.
  - [x] Excluded tests, shadowed names, ambiguous receivers, and exhausted budgets cannot acquire false definition links.
- Handoff: Stage 3 receives the implementation and `docs/briefs/evidence/codemap-nav/rust-policy.json`.
- Replan when: a shared helper change affects another language or the target option fails to reach the final resolution cache/consumer. Isolate or correct that propagation before publishing results.

### Stage 3 — Verify navigation across tool surfaces
- Starts when: Stage 2's caller policy and implementation pass their focused checks.
- Work: Replay C2 and the exact named terminal functions through search/read/grep using the first child's presentation controls. Run relevant MCP/cross-feature regressions, evaluate every side-effect checkpoint, and record verified links separately from unresolved and omitted entries.
- Deliverable: `docs/briefs/evidence/codemap-nav/rust-relations.json` (proposed) with `outcome`, binary/source/config hashes, consumed presentation contract, target input and predicate policy, expected/actual edge lists, raw-output paths, check results, remaining reasons, and `side_effects_clear`.
- Verify: `Run the listed package checks separately`; Inputs: `cargo test --locked --test e2e_tests e2e::mcp::`, `cargo test --locked --test e2e_tests e2e::cross_feature::` from `apps/codemap-search`, using non-empty existing suites and the C2 replay evidence in this child; Expected: exit 0 and consistent file/line/confidence results at the final MCP boundary.
- Ends when:
  - [x] All source-proven C2 links have final tool-output evidence or an explicit replan-triggering failure.
  - [x] Confidence and omission notices stay consistent under full, relation-focused, and unresolved-summary presentation.
- Handoff: the parent and event child receive `docs/briefs/evidence/codemap-nav/rust-relations.json` after whole-child acceptance is evaluated.
- Replan when: a link is correct only under unrecorded target/configuration assumptions or a final tool hides confirmed results. Correct and recheck this child before successors start.

The original C2 payload and reported fragments are fixed baseline data; preserve them and record candidate variants separately.

C2 — tool `grep`:

```json
{
  "path": "/Users/buyong/workspace/private/corral/apps/desktop/src-tauri/src/features/platform/common/window_gateway.rs",
  "pattern": "pub.*fn move_window_to_bounds|pub.*fn move_window_bounds_by_id",
  "-A": 42,
  "head_limit": 98
}
```

Reported fragments:

```text
callers omitted: `move_window_bounds_by_id` has 5 definitions — attribution ambiguous; use grep "move_window_bounds_by_id(" to enumerate call sites
move_window_to_bounds_by_id (unresolved)
```

## Side Effect Checkpoints
- [x] Name-count thresholds still protect the approximate fallback without discarding confirmed identities.
- [x] Existing Rust/Go and shared local-resolution checks pass with a non-empty test population.
- [x] Excluded directories, test attributes/decorators/calls, and explicit inclusion choices affect target counts and final relation output consistently.
- [x] Source changes, target-context changes, and configuration reloads invalidate the appropriate cached evidence.
- [x] Warming/dead/stale-index and budget conditions remain distinguishable from zero callers.
- [x] Direct calls, conditional calls, framework entry-point candidates, and unresolved dynamic receivers retain distinct claims.
- [x] The existing macro-attribution boundary remains unchanged.
- [x] Corral's original files, config, index, and window state are unchanged by verification.

## Acceptance Criteria
- [x] The reproduced threshold case displays every proven caller within output limits even when the compatible definition count is five or above the configured threshold.
- [x] Regular movement and history restoration have separately traceable, source-correct paths through their applicable gateway/platform/native definitions.
- [x] Known platform conditions affect the intended imports and call sites; unknown conditions do not become implicit host-OS assumptions.
- [x] Ambiguous, excluded, shadowed, unsupported, and budget-limited controls receive no fabricated definition links or false precise labels.
- [x] Read, grep, and search present consistent definition locations and uncertainty for the same analysis context.
- [x] The focused existing and approved regression suites pass, every side-effect checkpoint is recorded, and the final handoff reports remaining limits without claiming universal Rust resolution.

## Open Questions
- None — targeted regression coverage is approved, compatibility defaults are preserved, and remaining technical predicate/alias questions are assigned to Stage 1.

## 실행 결과

2026-09-14: [rust-relations.json](evidence/codemap-nav/rust-relations.json)에 C2의 원문 보존과 6개 이동·이력 복원 연결의 양방향 확인, 대상·별칭 재적용, 원본 Corral 보존 결과를 기록했다. 명시적 대상은 `[analysis].target_os`이며, 이름 개수 생략은 확인된 연결을 제거하지 않는다. 최종 이벤트 통합 후 전역 검증을 다시 수행한다.


최종 이벤트 통합 후에도 [통합 결과](evidence/codemap-nav/integration.json)의 고정 후보로 다시 통과했다. 초기 인계 보고서의 `final_integration`에 최종 소스·바이너리와 원본 보존 비교 범위를 기록했다.
