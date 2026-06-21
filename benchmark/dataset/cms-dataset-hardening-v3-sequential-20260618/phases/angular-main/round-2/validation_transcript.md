# Validation Transcript

## Prompt

In a large Angular app, a single-pick form field is bound to whole objects, not primitive ids. As the available choices are added, replaced, or removed while the view is still settling, two invariants have to hold at once: equality checks must not balloon, and the browser must not drift onto some other remaining choice just because a disappearing item stays in the DOM a little longer.

In the current codebase, what rule decides when Angular re-applies the model to that widget, and why are individual child mutations intentionally prevented from triggering an immediate full re-selection pass?

## Exact Command

```bash
claude -p --model sonnet --setting-sources '' --strict-mcp-config --allowedTools Bash\,Read\,Glob\,Grep --disallowedTools Edit\,Write\,WebFetch\,WebSearch\,Task\,NotebookEdit\,TodoWrite\,Workflow\,Agent\,Skill --output-format stream-json --verbose $'In a large Angular app, a single-pick form field is bound to whole objects, not primitive ids. As the available choices are added, replaced, or removed while the view is still settling, two invariants have to hold at once: equality checks must not balloon, and the browser must not drift onto some other remaining choice just because a disappearing item stays in the DOM a little longer.\n\nIn the current codebase, what rule decides when Angular re-applies the model to that widget, and why are individual child mutations intentionally prevented from triggering an immediate full re-selection pass?'
```

## Exit Code

0

## stderr

```text

```

## Raw Answer

```text
Let me find the relevant select control value accessor code in this Angular codebase.
Here is a precise account of both mechanisms.

---

## The deferred-write rule: `_writeValueAfterRender`

Every individual `<option>` mutation — `ngValue` set, `value` set, or the option destroyed — calls `_writeValueAfterRender()` on the parent `SelectControlValueAccessor` (`select_control_value_accessor.ts:173`).

That method does **not** immediately call `writeValue`. Instead it schedules the re-application for **after the current render cycle finishes**:

```ts
// select_control_value_accessor.ts:173-192
_writeValueAfterRender(): void {
  if (this._queuedWrite || this.appRefInjector.destroyed) {
    return;                          // ← already scheduled, skip
  }
  this._queuedWrite = true;
  afterNextRender(
    {
      write: () => {                 // ← runs once, in the write phase
        if (this.destroyRef.destroyed) return;
        this._queuedWrite = false;
        this.writeValue(this.value); // ← single full re-selection pass
      },
    },
    {injector: this.appRefInjector},
  );
}
```

The `_queuedWrite` boolean is the deduplication gate. The first option mutation in a render cycle sets it to `true` and books exactly one `afterNextRender` callback. Every subsequent mutation within the same cycle is a no-op because of the early return. When the cycle ends Angular runs the `write` phase, clears the flag, and calls `writeValue(this.value)` once — with the full, stable option map.

---

## Why individual mutations must not trigger an immediate full pass

The source comment (`select_control_value_accessor.ts:152-171`) names three concrete bugs that the deferred approach fixes:

### 1. Quadratic `compareWith` calls — issue #41330

`writeValue` resolves the selection by calling `_getOptionId`, which iterates every entry in `_optionMap` and invokes `_compareWith` on each one:

```ts
// select_control_value_accessor.ts:225-229
_getOptionId(value: any): string | null {
  for (const id of this._optionMap.keys()) {
    if (this._compareWith(this._optionMap.get(id), value)) return id;
  }
  return null;
}
```

If `writeValue` fired on every individual option mutation, adding N options would provoke N separate scans of a growing map — O(N²) comparisons. Batching all mutations into a single post-render write keeps it O(N).

### 2. Premature DOM write during option rendering — issue #14505

In Safari and IE 11, setting the `<select>`'s `value` property before an `<option>`'s child text node exists in the DOM made the browser unable to deselect the first option when no option matched the model. At the point each option directive fires its setter, only its `value` attribute exists; the text content is not yet attached. The `afterNextRender` write phase guarantees the full subtree is in the DOM before the selection is applied.

### 3. Animation-delayed element removal — issue #18430

When Angular animations are in play, a removed option stays in the DOM until its leave animation finishes. Without deferral, Angular would re-apply the model while the departing (previously selected) option was still physically present, so the `<select>` value would be correct in the model but not yet reflected in the DOM. Then when the animation finally detached the element, the browser would auto-advance to the first remaining option — causing the visible selection to drift away from the model even though no user interaction occurred. By deferring to `afterNextRender`, Angular waits until after the animation engine has had its chance to update the DOM before writing the selection.

---

## Summary

The governing rule is: **re-apply the model value to the `<select>` element exactly once per render cycle, in `afterNextRender`'s write phase, after every pending option addition and removal for that cycle has settled.** Individual child mutations are blocked from triggering their own immediate pass by the `_queuedWrite` flag, because doing so would cause quadratic equality checks, confuse browsers that need a complete DOM subtree before a selection can be set, and let animation-delayed removals silently override the model's chosen value.
```

## stdout Artifact

- `claude_stdout.jsonl` contains the full stream-json trace.
