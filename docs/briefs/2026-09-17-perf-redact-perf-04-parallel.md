# [perf] Overlap symbol preparation and redaction safely

## Work Type
perf

## Current State (As-Is)
- [confirmed] The inspected implementation is commit `e5fc44172faaa87a8e0fd358d160bfe881d5e47b` on 2026-09-17 — Evidence: `git rev-parse HEAD` and `docs/briefs/evidence/redact-perf/baseline-e5fc441.json`.
- [confirmed] MCP runs on a single-thread Tokio runtime with sequential request/response dispatch; the indexer has its own OS thread — Evidence: `apps/codemap-search/src/main.rs`, `#[tokio::main(flavor = "current_thread")]`, and `apps/codemap-search/src/index/indexer.rs`, `codemap-indexer`.
- [confirmed] Read/grep build their source output before `live_symbols::append()` prepares rich context — Evidence: `apps/codemap-search/src/mcp/mod.rs`, the read/grep dispatch arms.
- [confirmed] Source-only views bypass rich context — Evidence: `apps/codemap-search/src/tools/live_symbols.rs`, `append()` and `LiveView::Source`.
- [confirmed] Context collection and budget-sensitive rendering are currently combined — Evidence: `apps/codemap-search/src/tools/live_symbols/context.rs`, `build()`, `Outline::new()`, annotation and remaining-cap handling.
- [confirmed] `SourceResolver` includes `Rc` and `RefCell`, while request/MCP settings are thread-local — Evidence: `apps/codemap-search/src/callers/resolution.rs`, `SourceResolver`, and `apps/codemap-search/src/config.rs` / `apps/codemap-search/src/redact.rs` request guards.
- [confirmed] Index extraction reads complete UTF-8 source but can also use composite masks and native-expanded virtual source — Evidence: `apps/codemap-search/src/index/engine.rs`, extraction loop, `apps/codemap-search/src/parser/mod.rs`, `extract_parts()`, and `apps/codemap-search/src/index/preprocess/mod.rs`, expanded extraction.
- [inferred] Independent symbol/callable preparation can overlap detector work, but source-only requests may have no work to overlap and small jobs may lose time to dispatch — Confirm with explicit overlap proof and separate source/rich cold/hit measurements.

## Baseline Measurement
- Reference the captured release measurement at `docs/briefs/evidence/redact-perf/baseline-e5fc441.json`: 10,000 lines, 439,058 bytes, ten synthetic secrets, two warm-up calls and 21 measured calls per operation/condition with rotating condition order. The timer covers stdin write/flush through the complete stdout frame, excluding client JSON decoding, startup, and indexing.
- Median milliseconds for `off_with_secrets / on_with_secrets / on_without_secrets`: full read `2.989 / 36.897 / 37.295`; one-line read `0.936 / 34.664 / 34.397`; plain grep `3.501 / 36.935 / 37.457`; callable grep `73.716 / 142.082 / 140.566`; search `7.041 / 40.641 / 40.508`.
- Treat these as historical measurements, not portable latency limits or proof that parsing alone consumes the delta. The captured run has no hardware/compiler inventory and no redaction cache; reproduce the baseline with Child 01's harness and record the current environment.
- Target a reproducible reduction in repeated-request masking overhead while retaining correctness. The user chose no fixed millisecond SLO. Report median/p95 and first-request, post-edit, index-startup, and resource effects separately; unresolved repeatable regressions return to the parent.
- Compare against the serial shared-analysis/cache revision recorded by `docs/briefs/evidence/redact-perf/03-cache.md`, not only the original implementation. Attribute cache wins and parallel wins separately.
- Retain the five historical source-only operations and add Child 01's named rich-view population; do not claim a symbol-parallel benefit from source-only timings.

## Desired Outcome (To-Be)
- Eligible cache-miss requests overlap CPU redaction work with independent symbol/callable preparation on the same verified input/policy contract.
- Request ordering, output assembly, limits, configuration pinning, and final masking remain deterministic.
- Worker lifecycle and overload/failure behavior remain bounded without skipping inspection or widening the public MCP/CLI contract.
- The index-time prewarming route has a measured, explicitly recorded adopt/defer decision based on raw-source compatibility and startup cost.

## Scope
### In Scope
- A bounded CPU executor and explicit immutable detector-job inputs/results using Children 02 and 03's contracts.
- Separating independent preparation from budget-sensitive formatting so symbol work can overlap masking without changing output selection.
- Synchronous fallback, same-key in-flight coordination, stale completion protection, shutdown, and deterministic concurrency tests.
- A bounded evaluation/prototype of index-time prewarming using existing original bytes/trees where compatible; retain it only with correct provenance and measured benefit/cost.
### Out of Scope
- [hard] Parallel processing of separate client JSON-RPC requests or concurrent writes to stdout.
- [hard] Passing `SourceResolver`, `RefCell`/`Rc` graphs, or same-thread request guards to workers through unsafe trait assertions.
- [hard] Replacing the existing index writer/watch lifecycle or changing test exclusion, composite, native preprocessing, ranking, or caller-resolution semantics.
- [deferred] Broad project-wide thread-pool adoption or background scanning of files that no current request/index path reads.

## Constraints
- Preserve MCP-only masking: ordinary `parse` CLI output, source files, persisted indexes, matching, ranking, counts, and public JSON-RPC schemas retain their current contracts.
- Preserve `[tool_output].is_redact_enabled`, the three `[redact]` lists, repo/global/default precedence, exact rule/value exceptions, and request-pinned configuration. Add no user-facing configuration keys or schema migration for this work.
- Add no redaction byte, candidate-count, or time scan limits. Retain `parser::parse_source`'s existing 5000ms deadline, existing tool input/output limits, and complete text fallback. Cache eviction or worker saturation must never mean skipped inspection.
- Detect against the complete original presentation source before clipping, escaping, or truncation. Preserve UTF-8 byte coordinates, BOM/CRLF handling, multiline values, PEM interiors, and conservative changed/unavailable-source behavior.
- Keep the final JSON-RPC response guard, contextual named-string handling, and existing treatment of parent objects, arrays, numeric values, and booleans. Never release an uninspected response while work is pending.
- Use only synthetic credentials in fixtures and measurement output. Do not log source text, secret values, exception values, or credential-bearing query strings in profiling records.
- CPU work requires actual worker execution; adding async wrappers on `current_thread` alone does not satisfy this child.
- Use the explicit policy established by Child 02; no worker may infer activation from `IS_MCP_RESPONSE` or obtain a newer config generation during the same job.
- Keep thread-affine context on its owner thread. Send owned immutable source/compatible tree/policy data and return completed metadata or explicit failure only.
- Share common parsing before the fork where representations match; do not reintroduce duplicate parsing merely to run two branches at once.
- Join detection before response assembly. Compute byte-budget-sensitive text selection after the actual masked source size is known, preserving current anchors, context allocation, omission notices, and ordering.
- Bound concurrent jobs and retained inputs. Use inline full inspection/backpressure when admission is unavailable; do not create one unbounded thread per file, wait while holding a cache lock, or invent a new scan cutoff.
- Reuse existing dependencies where practical. Document and justify any necessary dependency before adding it; do not change the runtime flavor solely to obtain CPU parallelism.
- Background preparation is an optimization only: unchanged index entries, non-indexed/ignored/large files, transformed inputs, and misses still have an authoritative live-source scan path.

## Related Files / Entry Points
- `apps/codemap-search/src/redact/parallel.rs` (proposed) — bounded execution, job ownership, join, fallback, and shutdown behavior.
- `apps/codemap-search/src/redact.rs` — integrate module ownership and explicit policy/result transfer while preserving final-response helpers.
- `apps/codemap-search/src/redact/tests.rs` and `apps/codemap-search/tests/e2e/redact.rs` — add deterministic worker/lifecycle and returned-response cases.
- `apps/codemap-search/src/redact/analysis.rs` (proposed) and `apps/codemap-search/src/redact/cache.rs` (proposed) — consume the finalized explicit inputs and completed-result publication APIs.
- `apps/codemap-search/src/main.rs` and `apps/codemap-search/src/mcp/mod.rs` — preserve sequential request framing and request-scoped policy while integrating worker ownership.
- `apps/codemap-search/src/tools/live_symbols/context.rs` — start at `build()` to isolate independent preparation from cap-sensitive rendering.
- `apps/codemap-search/src/tools/live_symbols.rs` and `apps/codemap-search/src/tools/live_symbols/structure.rs` — retain source-only bypass, anchor semantics, filtered source, and final context budgets.
- `apps/codemap-search/src/tools/live_symbols/callable.rs` and `apps/codemap-search/src/tools/grep/expansion.rs` — overlap compatible callable preparation with redaction without another full scan.
- `apps/codemap-search/src/callers/resolution.rs` — leave thread-affine resolver state on its owner thread.
- `apps/codemap-search/src/index/engine.rs` and `apps/codemap-search/src/index/indexer.rs` — inspect original-source extraction and writer ownership before prototyping prewarming.
- `apps/codemap-search/src/parser/mod.rs` and `apps/codemap-search/src/index/preprocess/mod.rs` — reject incompatible composite/native-expanded tree reuse.
- `docs/briefs/evidence/redact-perf/02-shared-analysis.md` (proposed) and `docs/briefs/evidence/redact-perf/03-cache.md` (proposed) — consume compatible-source/policy contracts and cache publication/failure rules.

## Execution Plan
### Stage 1 — Establish the executable overlap and lifecycle contract
- Starts when: `docs/briefs/evidence/redact-perf/03-cache.md` provides the completed cache API and freshness/failure evidence, and `docs/briefs/evidence/redact-perf/02-shared-analysis.md` provides compatible source/policy/thread ownership.
- Work: Pin the serial cached baseline. Identify the actual independent symbol/callable work and split preparation from final cap-sensitive formatting. Specify job input, ownership, scheduling, join, inline fallback, panic/error cleanup, and shutdown. Evaluate prewarming against original, filtered, composite, macro-expanded, and already-indexed inputs.
- No-op when: Existing worker execution already proves real overlap, explicit pinned-policy transfer, unchanged budgets/ordering, safe failure/shutdown, and the required measured benefit.
- No-op handoff: Record that proof and the prewarming disposition at `docs/briefs/evidence/redact-perf/04-parallel.md` (proposed); Child 05 verifies the existing implementation instead of requiring another executor.
- Deliverable: `docs/briefs/evidence/redact-perf/04-parallel.md` (proposed), contract section with serial baseline, concrete fork/join call sites, job/result types, thread ownership, admission/fallback/shutdown rules, and prewarming adopt/defer criteria.
- Verify: `Inspect the fork/join route and compile a bounded ownership/overlap probe using the selected job types`; Inputs: compatible raw source/policy, callable/full-view preparation, and the thread-affine resolver boundaries; Expected: a Send-safe job without unsafe assertions and a concrete main-thread task that can execute before the worker result is joined.
- Ends when:
  - [ ] Source-only, cache-hit, small-job, and no-worker paths have an explicit serial fast path where dispatch has no useful overlap.
  - [ ] Both branch results can be assembled under the existing output budgets after joining, without using an estimated unmasked budget to change the result.
  - [ ] Prewarming uses only proven original identities and has an explicit cold-start/index-resource measurement route.
- Handoff: Stage 2 receives the fork/join/lifecycle contract at `docs/briefs/evidence/redact-perf/04-parallel.md`.
- Replan when: No independent work can be isolated without a public/output contract break; return to the parent for a bounded private preparation split rather than silently dropping the requested runtime parallelism.
- Worker decision: Choose the smallest bounded executor and measured dispatch policy. Decide prewarming adoption from provenance and measured startup/interactive costs; record why an unsafe or unhelpful prewarm path remains deferred.

### Stage 2 — Integrate parallel preparation with safe completion
- Starts when: Stage 1 has a compiled ownership probe and named fork/join call sites at `docs/briefs/evidence/redact-perf/04-parallel.md`.
- Work: Integrate the executor and parallel branches, preserve pinned policy and source identity, and keep completed-result publication atomic. Add deterministic barriers/failure injection for actual overlap, worker failure, reload/edit races, overload fallback, and shutdown.
- Deliverable: The integrated parallel path and lifecycle/concurrency evidence recorded at `docs/briefs/evidence/redact-perf/04-parallel.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml && cargo test --manifest-path apps/codemap-search/Cargo.toml --lib redact::tests`; Inputs: new worker/barrier tests using synthetic source, old/new policies, same-key requests, saturated admission, and worker failure; Expected: real branch overlap, all successful outputs masked under their pinned policy, no stale/partial clean publication, and all jobs/waiters terminate.
- Ends when:
  - [ ] A delayed old-policy or old-content result cannot replace a newer request's metadata.
  - [ ] Worker failures cause explicit safe fallback/error handling before response output, never an unmasked success.
  - [ ] No cache lock is held across analysis or join, and shutdown terminates/reaps owned workers without changing existing indexer ownership.
- Handoff: Stage 3 receives the integrated fork/join path and deterministic concurrency evidence at `docs/briefs/evidence/redact-perf/04-parallel.md`.
- Replan when: Safety requires sending thread-affine state, removing the final guard, weakening fallback, or changing request ordering; stop this integration and revise its contract with the parent.

### Stage 3 — Measure parallel benefit and prewarming tradeoffs
- Starts when: Stage 2's overlap/failure tests and source/policy invariants pass.
- Measurement procedure: In addition to the primary command below, execute the exact state/rich/phase invocations recorded in `docs/briefs/evidence/redact-perf/01-baseline.md`; give each revision, cache state, and serial/parallel variant its own result path and record the commands in this child's handoff. Never overwrite the original baseline or an earlier child's raw samples.
- Work: Measure serial versus parallel operation at the same cache state, using the three primary conditions plus the separate rich-view population. Measure first calls, changed-source calls, startup/index time, and bounded resource behavior. Adopt prewarming only if its original-source identity and measured tradeoff satisfy the recorded criteria.
- Deliverable: `docs/briefs/evidence/redact-perf/04-parallel.md` containing the integrated route, executed tests, serial/parallel raw-result paths, actual cache-state evidence, dispatch choices, startup/resource results, and explicit prewarming decision/residuals.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests redact:: && cargo build --manifest-path apps/codemap-search/Cargo.toml --release && python3 apps/codemap-search/scripts/benchmark_redact.py --binary apps/codemap-search/target/release/codemap-search --output docs/briefs/evidence/redact-perf/04-parallel.json --rounds 21 --warmups 2`; Inputs: both equal-size fixtures, all three conditions, actual misses/hits, and the deterministic worker test population; Expected: coverage/order/caps are preserved, runtime overlap is independently demonstrated, and cache versus parallel effects are separately measured.
- Ends when:
  - [ ] Measured results distinguish useful overlap from work merely moved to another thread; no fixed ms target or skipped scan is introduced.
  - [ ] Repeatable regressions or startup/resource tradeoffs have an explicit parent disposition before global acceptance.
- Handoff: Child 05 receives `docs/briefs/evidence/redact-perf/04-parallel.md` with final implementation paths, concurrency proofs, measurement evidence, and the prewarming decision.
- Replan when: Claimed speedup disappears at equal cache states, prewarming warms an incompatible source, or a lifecycle test fails; stop global acceptance and correct this child's owned implementation before rerunning evidence.

## Side Effect Checkpoints
- [ ] Request IDs, stdout framing, sequential client semantics, final response guards, error codes, and read-only tool schemas remain unchanged.
- [ ] Custom fields/rules/exceptions and disabled mode operate identically on the main thread and workers, including config reload during a blocked job.
- [ ] Raw/test-filtered/composite/native-expanded sources are never mistaken for one cache/AST identity.
- [ ] Source and rich output retain their existing source-priority byte budgets, annotation allocations, order, and omission notices after joining.
- [ ] Saturation, failure, and shutdown tests use non-empty queued/in-flight populations and prove inspection or sanitized error behavior rather than silent dropping.
- [ ] Existing parser cancellation/deadline and index writer/watch shutdown ordering remain intact; prewarm jobs never own the Tantivy writer.

## Acceptance Criteria
- [ ] `docs/briefs/evidence/redact-perf/04-parallel.md` proves eligible symbol/callable preparation and CPU redaction actually overlap, rather than only exposing async syntax or a cache hit.
- [ ] Worker policy/source transfer, stale completion, cache failure, overload fallback, and shutdown are verified at internal completion and returned-response boundaries.
- [ ] Same-state serial/parallel measurements and the original three-condition comparison establish the benefit and disclose first-request/index/resource tradeoffs without a fabricated ms SLO.
- [ ] Prewarming is either safely integrated with measured evidence or explicitly deferred with a technical reason; its absence does not remove required request-path parallelism.

## Open Questions
- None — The user selected improvement/regression verification without a fixed millisecond SLO; remaining implementation choices are specified in this brief.
