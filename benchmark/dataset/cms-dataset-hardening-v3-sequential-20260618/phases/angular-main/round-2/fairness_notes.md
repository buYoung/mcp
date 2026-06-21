# Fairness Notes

- Confirmed: the public question does not expose file paths, symbol names, class names, method names, issue numbers, subsystem names, or checklist-style routing.
- Confirmed: the public question avoids the banned direct terms from the strategy, including the HTML element name, Angular API names, and render-hook terminology.
- Confirmed: negative self-check probes with the final wording produced no direct hits in the Angular source tree for `single-pick form field`, `whole objects`, `view is still settling`, `remaining choice`, `immediate full re-selection`, `disappearing item stays in the DOM a little longer`, `equality checks must not balloon`, or `re-applies the model`.
- Confirmed: broad source exploration still leaves a real but nontrivial route. A diligent solver can reach the forms accessor path through generic choice-control behavior, but the complete answer still requires crossing from forms into core render scheduling and distinguishing the single-choice path from the multiple-choice path.
- Confirmed: the obvious shallow answer is wrong or incomplete. Saying only "it compares objects" or only "it waits until after render" misses parent-owned bookkeeping, teardown guards, and the contrast case that makes the behavior unique.
- Confirmed: Sonnet no-MCP found the main deferred-write rule but still missed the application-injector teardown rationale and the multiple-choice contrast, which indicates the prompt is difficult without relying on an unfairly narrow key.
- Inference: the phrase `single-pick form field` is a weak hint toward forms, but it does not reveal the decisive accessor file, the queued-write mechanism, or the core after-render registration path.
