# 02 — indexed overview recommendation handoff

## Callable boundary

- `tools::overview::prepare(&ToolContext) -> OverviewPreparation` renders the existing base overview and captures the eligible root candidate `CodemapSnapshot` plus publication identity from the same `PublishedIndexSnapshot`. `overview::run` remains the synchronous compatibility entry point. Folder/file, llms-txt, empty, warming, dead, or refresh-error views have no recommendation snapshot and make no evaluator request.
- `overview::recommend(preparation, task_query, evaluator, policy, cancellation, output_cap)` is independently callable. It returns `RecommendationResult` with base-plus-recommendation text, status, snapshot identity, evaluated file/fragment counts, raw Score/Choice answers, adapter question/policy versions, usage and elapsed time. `RecommendationFailure` returns a bounded reason and known usage. The MCP caller retains the prepared base text on failure.

## Candidate and selection policy

- Every file in the captured root snapshot yields at least one fragment. Symbols and file docs are split into groups of 12 and indexed possible calls into groups of 24; no BM25 or hand-selected file list prefilters candidates. Each question carries its own redacted file/path/evidence in named instructions; shared state carries only the caller's redacted `task_query`. No source-body read or exported index is needed.
- Score levels 0–3 mean no useful evidence, tangential, important support, and direct implementation. A file qualifies if any fragment has `P(2)+P(3) > P(0)+P(1)`. Qualification precedes the 24-file cap. Qualified files sort by maximum fragment score and then path. Equal probability masses are uncertain. With no qualified files, complete negative evidence yields `no_match`; ties or no usable indexed declarations/docs/calls yield `insufficient_evidence`. Neither status asserts source-level absence.
- Only selected files proceed to a dependent Choice role stage. Each displayed declaration has one representative role among implementation, configuration, caller, consumer, and unrelated; up to two supported declarations are attached per file. The file remains recommended if none qualify for a declaration role. Rendered rows include original indexed ranges, possible call candidates and bounded `read` arguments. The addition is limited to 32 KiB and also respects an explicit overview cap; insufficient room returns the untouched base view with `output_limit` fallback.
- The immutable `Arc` snapshot stays owned across both awaits. A delayed-evaluator regression replaces a separate publication during the call and confirms the output still names only the captured file and identity.

## Verification

- `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib tools::overview`: 5 passed, 0 failed. Populated cases cover complete catalog before 24-file selection, all-negative no match, tied/absent evidence, output-cap fallback, and held-snapshot identity.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::jev`: previously passed the injected root overview and independent mode activation. The final post-join result is recorded in `05-verification.md`.
- Existing root/folder/file output paths were also covered by `e2e::codemap` (15 passed) before final joining.

Policy version: `overview-qualification-v1-experimental`; question version: `overview-fragment-v1`. No live quality measurement or calibration is claimed.
