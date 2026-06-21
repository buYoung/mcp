# Failed Candidate Evidence

- No redesign was required for the active round-2 candidate because the first Sonnet no-MCP validation scored `0.5625`, which is under the `0.6` threshold.
- The current run still preserved useful failure evidence from the model response:
  - Claude did not explain why the application injector is required instead of the node injector during teardown.
  - Claude did not pin the shared one-shot after-render registration and cleanup semantics in core render scheduling.
  - Claude did not contrast the single-choice path with the multiple-choice path that updates immediately per child mutation.
- These misses are the remaining evidence for where a future hardening pass could deepen the candidate if a lower score target were needed.
