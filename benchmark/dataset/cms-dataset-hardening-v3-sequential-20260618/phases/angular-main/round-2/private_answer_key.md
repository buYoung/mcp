# Private Answer Key

## Candidate

- id: `angular-deferred-choice-reconciliation-v1`
- status: pending validation

## Correct Answer

The obvious "it just compares objects on every child update" explanation is wrong. In this codebase, the single-choice object-backed control does not perform a full immediate model-to-view rewrite every time a child choice is added, updated, or removed. Each child mutation updates parent-owned bookkeeping and requests one deferred reconciliation pass. That pass is coalesced behind a queued flag and runs as a one-shot post-render write callback, using the application injector rather than the node injector and aborting if destruction has already progressed.

## Why

1. The single-choice accessor keeps object-backed choices in parent-owned tracking state. It stores them in `_optionMap`, allocates generated ids, resolves the current model value back to an id with the comparator, and writes a synthesized DOM value string from that id. See `.agents/benchmark-data/angular-main/packages/forms/src/directives/select_control_value_accessor.ts:119-125`, `:143-191`, and `:198-235`.
2. Child choice registration and teardown do not directly perform the full rewrite. On add/update, the child stores or rewrites its value and then asks the parent for delayed reconciliation; on destroy, it removes its entry and asks again. See `.agents/benchmark-data/angular-main/packages/forms/src/directives/select_control_value_accessor.ts:273-300`.
3. The parent-side delayed path is `_writeValueAfterRender()`. It coalesces repeated child churn with `_queuedWrite` and refuses to schedule a new pass when the application injector is already destroyed. See `.agents/benchmark-data/angular-main/packages/forms/src/directives/select_control_value_accessor.ts:173-191`.
4. The actual rewrite is not performed inside the child setter or destroy hook. It is scheduled through a one-shot `afterNextRender` callback in the `write` phase, and that callback eventually calls `writeValue(this.value)`. See `.agents/benchmark-data/angular-main/packages/forms/src/directives/select_control_value_accessor.ts:180-191` and `.agents/benchmark-data/angular-main/packages/core/src/render3/after_render/hooks.ts:384-407`.
5. The scheduling uses the application injector specifically because teardown can mark the node injector destroyed before destroy hooks run. The queued callback also checks `destroyRef.destroyed` before it would clear the queue and call the writer. See `.agents/benchmark-data/angular-main/packages/forms/src/directives/select_control_value_accessor.ts:144-149` and `:180-190`.
6. The render hook is truly post-render and one-shot: `afterNextRender` delegates to shared registration with `once = true`, the registration resolves `AfterRenderManager` and a `DestroyRef` from the provided injector, and the manager executes ordered phases before removing and destroying one-shot sequences. See `.agents/benchmark-data/angular-main/packages/core/src/render3/after_render/hooks.ts:433-466` and `.agents/benchmark-data/angular-main/packages/core/src/render3/after_render/manager.ts:40-46` and `:71-109`.
7. The multiple-choice path is deliberately different. Its child updates and teardown call the parent `writeValue(...)` immediately and set selected flags per child, instead of using the queued post-render path. See `.agents/benchmark-data/angular-main/packages/forms/src/directives/select_multiple_control_value_accessor.ts:127-141` and `:235-273`.
8. The deferral is not only about performance. It avoids repeated comparator scans on child churn, avoids premature browser state changes before child content is fully attached, and avoids the late-removal case where the browser can fall onto another remaining choice after a visually delayed deletion. See `.agents/benchmark-data/angular-main/packages/forms/src/directives/select_control_value_accessor.ts:153-169` and `.agents/benchmark-data/angular-main/packages/forms/test/value_accessor_integration_spec.ts:1188-1250`, `:1280-1318`, and `:1333-1389`.

## Pinned Facts

- The single-choice object-backed path uses parent-owned id mapping plus comparator-based id lookup, not direct object identity writes to the DOM.
- Child add/update/remove events update parent bookkeeping and explicitly request a separate delayed reconciliation. For F2, both clauses are required for present; exactly one is partial; neither, or an explicit rejection of scheduling/delay, is absent.
- Reconciliation is coalesced with a queued flag so repeated child churn collapses into one pass.
- The actual model-to-view rewrite runs in a one-shot post-render write phase.
- The scheduling is bound to the application injector because the node injector can already be destroyed during teardown.
- The callback checks directive destruction before attempting the write.
- The multiple-choice control path is a contrast case because it updates immediately per child mutation.
- The reasons include both correctness and cost: avoid excess comparator work, avoid premature browser selection behavior during late insertion, and avoid wrong fallback selection during delayed removal.

## Insufficient Answers

- Answers that say only "it uses a comparator to find the matching choice."
- Answers that mention only a post-render callback but omit the parent-owned option bookkeeping and child mutation path.
- Answers that mention only performance or only browser quirks.
- Answers that describe the multiple-choice control behavior as if it were the same mechanism.
- Answers that miss the application-injector choice or the destruction guards while claiming to explain the exact rule.
