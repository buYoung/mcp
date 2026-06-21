# Fairness Notes

- Confirmed: the public question is symptom-first and operational. It speaks in terms of preserved pages, hidden workspace state, later cleanup, and explicit purge, without exposing file paths, symbol names, subsystem names, or checklist-shaped subquestions.
- Confirmed: direct phrase probes against the Angular source tree produced no hits for `custom screen-reuse policy`, `preserved deep child`, `workspace-scoped resources`, `hidden shell as still active`, or `drop the preserved page state`.
- Confirmed: the prompt does not contain the decisive tokens that directly expose the answer path, such as `pathFromRoot`, `retrieveStoredRouteHandles`, `shouldDestroyInjector`, or `withExperimentalAutoCleanupInjectors`.
- Confirmed: a solver still has a fair static route. The answer is uniquely grounded once they reconcile the detach/store fork, the stored live route tree, and the later `NavigationEnd` cleanup walk.
- Confirmed: the candidate remains objectively gradable. Broad answers about generic reuse, generic caching, or the visible router tree are incomplete against the pinned evidence.
- Confirmed: this attempt nevertheless fails the hardness bar. Sonnet recovered nearly the entire intended mechanism from source in one valid no-MCP run and only missed the last destruction gate details.
- Confirmed: there is also explicit prose documentation in `packages/router/docs/injector_cleanup.md` that describes the same parent-survival-through-stored-handle mechanism, which makes this candidate non-robust even if the wording avoids direct token leakage.
- Inference: given the prior corrected target-root forms failure plus this router cleanup failure and the existing Angular redesign guidance, Angular currently looks blocked by self-named and/or documented mechanisms rather than by an answer-key defect.
