# Failed Candidate Evidence

- Prior corrected target-root Angular candidate failed at `0.625` and `0.65625`.
- That failed candidate asked about deferred reconciliation for object-backed single-choice controls in forms/render scheduling.
- This attempt is materially different:
  - repository area: router lifetime and injector cleanup rather than forms accessors;
  - difficulty mechanism: competing lifetime explanations and hidden stored-handle ancestry rather than delayed write scheduling;
  - expected decoy: visible-tree or generic caching intuition rather than post-render re-selection logic.
- Validation outcome:
  - valid target-root Sonnet no-MCP run completed with score `0.88`;
  - Sonnet recovered the core detach/store rule, stored live route tree, `pathFromRoot` ancestry walk, opt-in `NavigationEnd` cleanup timing, and the later handle-drop cleanup story;
  - only the explicit `shouldDestroyInjector(route)` gate and descendant force-destroy cascade were materially missed.
- Additional blocker signal:
  - `packages/router/docs/injector_cleanup.md` contains prose documentation for the same parent-survival and stored-handle mechanism, so the candidate is not robust against a grep+read solver even with restrained wording.
