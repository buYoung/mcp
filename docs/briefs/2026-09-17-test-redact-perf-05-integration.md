# [test] Verify redaction safety and performance end to end

## Work Type
test

## Current State (As-Is)
- [confirmed] The inspected implementation is commit `e5fc44172faaa87a8e0fd358d160bfe881d5e47b` on 2026-09-17 — Evidence: `git rev-parse HEAD` and `docs/briefs/evidence/redact-perf/baseline-e5fc441.json`.
- [confirmed] The large-file regression covers ten synthetic secrets, interior reads, grep expansion, search evidence, raw matching, and unchanged source bytes — Evidence: `apps/codemap-search/tests/e2e/redact/large_file.rs`.
- [confirmed] Existing redaction coverage includes custom rules/exceptions, fallback, metadata, output caps, and ordinary CLI behavior — Evidence: `apps/codemap-search/src/redact/tests.rs` and `apps/codemap-search/tests/e2e/redact.rs`.
- [confirmed] `docs/briefs/evidence/redact-perf/baseline-e5fc441.json` records the three requested conditions before this briefset's optimizations; it does not prove future cache freshness or worker lifecycle behavior.
- [inferred] Shared parsing, cached negatives, and background completion create new stale-source/policy and concurrency risks — Confirm at both the completed-analysis boundary and the actual JSON-RPC response after Children 02–04 are implemented.

## Desired Outcome (To-Be)
- The optimized implementation has direct evidence that secrets remain hidden across every affected output route, including cache hits, edits, config changes, and worker failures.
- A reproducible before/after report separates duplicate-removal, cache, and parallel effects and reports the costs moved to first requests, edits, or indexing.
- Maintainers can reproduce the accepted measurements from repository-owned inputs and documented commands without requiring an external MCP client.

## Scope
### In Scope
- Integration coverage across shared analysis, cache completion, workers, and final response assembly using synthetic inputs.
- Final release-mode three-condition measurements, distinct rich-view/parallel measurements, and targeted compatibility checks.
- Durable evidence and developer documentation for the accepted behavior, measurement method, limitations, and any deferred prewarming.
### Out of Scope
- [hard] Landing production fixes under this test child; route defects to the owning performance child, then rerun the affected integrated checks.
- [hard] New detection features, public flags/schema keys, dependencies for external MCP clients, or a fixed millisecond gate.
- [deferred] Unrelated pre-existing watcher/default-threshold test failures, broad benchmark services, and additional language performance corpora.

## Constraints
- Preserve MCP-only masking: ordinary `parse` CLI output, source files, persisted indexes, matching, ranking, counts, and public JSON-RPC schemas retain their current contracts.
- Preserve `[tool_output].is_redact_enabled`, the three `[redact]` lists, repo/global/default precedence, exact rule/value exceptions, and request-pinned configuration. Add no user-facing configuration keys or schema migration for this work.
- Add no redaction byte, candidate-count, or time scan limits. Retain `parser::parse_source`'s existing 5000ms deadline, existing tool input/output limits, and complete text fallback. Cache eviction or worker saturation must never mean skipped inspection.
- Detect against the complete original presentation source before clipping, escaping, or truncation. Preserve UTF-8 byte coordinates, BOM/CRLF handling, multiline values, PEM interiors, and conservative changed/unavailable-source behavior.
- Keep the final JSON-RPC response guard, contextual named-string handling, and existing treatment of parent objects, arrays, numeric values, and booleans. Never release an uninspected response while work is pending.
- Use only synthetic credentials in fixtures and measurement output. Do not log source text, secret values, exception values, or credential-bearing query strings in profiling records.
- Use stdio JSON-RPC through the existing CLI binary as the final consumer boundary. Do not require a GUI, IDE, or separately installed MCP client.
- Use actual output and executed-case counts; compiling a test, observing an empty response, or checking only an invalidation flag is insufficient evidence.
- Prove actual worker overlap with deterministic barriers and verify output afterward. Keep timing assertions out of correctness tests.
- Label complete clean analysis separately from unsupported-language text-only completion, transient parse failure, and skipped/unavailable work. Inspect every fallback output before success.
- Compare like workloads, binary modes, environment, index readiness, and actual cache states. Report median/p95 and variability without inventing a fixed SLO or silently merging cold and warm samples.
- For serial/parallel comparisons, use a test-only or internal diagnostic control/probe; do not add a user-facing toggle just for measurement.

## Related Files / Entry Points
- `apps/codemap-search/tests/e2e/redact.rs` and `apps/codemap-search/tests/e2e/redact/large_file.rs` — extend CLI JSON-RPC output assertions and reuse the canonical fixture.
- `apps/codemap-search/src/redact/tests.rs` — integrate deterministic policy, cache, failure, and worker probes with the existing request guards.
- `apps/codemap-search/src/redact/analysis.rs` (proposed), `apps/codemap-search/src/redact/cache.rs` (proposed), and `apps/codemap-search/src/redact/parallel.rs` (proposed) — read the interfaces delivered by Children 02–04; production corrections return to their owners.
- `apps/codemap-search/tests/e2e/search.rs` — preserve the existing composite-query coverage/body/continuation regression.
- `apps/codemap-search/scripts/benchmark_redact.py` (proposed) — use Child 01's stable invocation and extend only missing integration measurement populations.
- `apps/codemap-search/docs/redaction-performance.md` (proposed) — record the accepted measurement procedure, source/cache/policy boundaries, worker behavior, and known limitations for maintainers.
- `docs/briefs/evidence/redact-perf/01-baseline.md` (proposed), `docs/briefs/evidence/redact-perf/02-shared-analysis.md` (proposed), `docs/briefs/evidence/redact-perf/03-cache.md` (proposed), and `docs/briefs/evidence/redact-perf/04-parallel.md` (proposed) — consume exact interfaces, previous results, proof populations, and the prewarming decision.
- `docs/briefs/evidence/redact-perf/baseline-e5fc441.json` — preserve the historical baseline independently of new measurements.

## Execution Plan
### Stage 1 — Close the integrated coverage matrix
- Starts when: `docs/briefs/evidence/redact-perf/04-parallel.md` contains final worker/lifecycle evidence and the prewarming disposition, and `docs/briefs/evidence/redact-perf/01-baseline.md`, `docs/briefs/evidence/redact-perf/02-shared-analysis.md`, and `docs/briefs/evidence/redact-perf/03-cache.md` contain the reproducible baseline, shared-analysis, and cache contracts.
- Work: Map each invariant below to an existing executed assertion or a missing case. Add only the missing integrated cases, including changed-source/policy and failure behavior at final JSON-RPC output.
- No-op when: Existing integrated coverage and final same-host measurements already satisfy every acceptance criterion with current revision and fixture identities.
- No-op handoff: Record the checked proof at `docs/briefs/evidence/redact-perf/05-acceptance.md` (proposed); the parent performs global acceptance without demanding duplicate tests.
- Deliverable: `docs/briefs/evidence/redact-perf/05-acceptance.md` (proposed), coverage matrix naming the non-empty input, internal assertion, final-output assertion, owning child, and verification command for each invariant.
- Verify: `Inspect the matrix against each child's acceptance criteria and run its listed targeted cases`; Inputs: ten-secret/clean fixtures, policy edits, same-size edits, transformed sources, evictions, worker barriers/failures, and disabled mode; Expected: every required behavior has an executed, observable outcome or an explicit gap routed to its owner.
- Ends when:
  - [ ] Positive and negative hits, misses, edits, policy changes, same-key coordination, overload, failure, and shutdown each have a deterministic non-empty case.
  - [ ] Metadata paths, literal provenance, multiline/PEM interior ranges, and final response guards are covered alongside source rows.
- Handoff: Stage 2 receives the complete coverage matrix at `docs/briefs/evidence/redact-perf/05-acceptance.md`.
- Replan when: A missing assertion exposes a production defect or ownership gap; stop acceptance, return the correction to Child 02, 03, or 04, and recalculate the affected handoffs after re-verification.

### Stage 2 — Run integrated safety and compatibility verification
- Starts when: Stage 1 has assigned every matrix row and the owning children have resolved blocking defects.
- Work: Execute the focused redaction/config/search checks and the parser/callable populations selected from the changed adapters. Verify formatting and all-target Clippy with the repository's existing commands. Record the exact commands, executed case counts, failures, and baseline-only limitations.
- Deliverable: `docs/briefs/evidence/redact-perf/05-acceptance.md`, verification section with all matrix outcomes, command results, source/binary identity, and resolved or remaining findings.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml && cargo test --manifest-path apps/codemap-search/Cargo.toml --lib redact::tests && cargo test --manifest-path apps/codemap-search/Cargo.toml --lib config::tests && cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests redact:: && cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests search::test_composite_query_preserves_coverage_body_evidence_and_continuation`; Inputs: the integrated optimized revision and targeted non-empty cases; Expected: all selected relevant cases execute and pass, with original source/index/CLI contracts and enabled response secrecy preserved.
- Ends when:
  - [ ] `cargo fmt --manifest-path apps/codemap-search/Cargo.toml --check` and `cargo clippy --manifest-path apps/codemap-search/Cargo.toml --all-targets -- -D warnings` pass using the existing toolchain/configuration.
  - [ ] Deterministic cache/config tests do not depend on an unverified watcher delay, and worker tests verify completion/reaping without unbounded waits.
  - [ ] Parser/callable regressions selected from actual changed adapters have named invocations and nonzero executed cases recorded in the handoff.
- Handoff: Stage 3 receives the verified revision and evidence section at `docs/briefs/evidence/redact-perf/05-acceptance.md`.
- Replan when: Coverage fails or outputs differ outside the preserved contract; route production fixes back to the owning child rather than waiving the safety gate or repairing unrelated baseline failures.

### Stage 3 — Publish the measured outcome and maintenance guide
- Starts when: Stage 2's affected checks pass and the final code/fixture identities are pinned.
- Measurement procedure: In addition to the primary command below, execute the exact state/rich/phase invocations recorded in `docs/briefs/evidence/redact-perf/01-baseline.md`; give each revision, cache state, and serial/parallel variant its own result path and record the commands in this child's handoff. Never overwrite the original baseline or an earlier child's raw samples.
- Work: Run the release harness on original and optimized revisions using the same host/procedure. Compare the original five operations in all three conditions, show incremental stage results, and separately report rich-view overlap, actual first misses, first calls after indexing, post-file/policy-edit requests, startup/index time, and bounded resource behavior.
- Deliverable: `docs/briefs/evidence/redact-perf/05-acceptance.md` with raw-result links, baseline/final provenance, median/p95/variability tables, correctness assertions, stage attribution, regression dispositions, prewarming decision, exact reproduction commands, and the completed maintenance guide.
- Verify: `cargo build --manifest-path apps/codemap-search/Cargo.toml --release && python3 apps/codemap-search/scripts/benchmark_redact.py --binary apps/codemap-search/target/release/codemap-search --output docs/briefs/evidence/redact-perf/05-final.json --rounds 21 --warmups 2`; Inputs: equal-size fixtures, pinned original/final release binaries, verified cache states, and serial/parallel diagnostic populations; Expected: every primary cell has 21 samples, both enabled conditions show reproducible repeated-request improvement, and first/edit/index/resource costs remain visible and accounted for.
- Ends when:
  - [ ] The report distinguishes measured improvement from inferred bottlenecks and does not attribute a cache hit's gain to parallel execution.
  - [ ] Repeatable regressions are corrected or have an explicit user-accepted tradeoff recorded by the parent; unmeasured or unresolved effects are not marked passed.
  - [ ] The maintenance guide reproduces fixtures/requests without historical temporary paths and matches the actual implementation, including any deferred prewarming.
- Handoff: The parent consumes `docs/briefs/evidence/redact-perf/05-acceptance.md` for global acceptance, checks all five child outcomes, and reports implementation, executed verification, and remaining limitations separately.
- Replan when: Benefits disappear in a same-state comparison, fixture/provenance checks fail, or costs were shifted into unmeasured startup/edit work; stop acceptance and return the specific measurement or implementation correction to its owner.

## Side Effect Checkpoints
- [ ] Preserve ten-secret masking, clean-source identity, safe references/type annotations, malformed/unsupported-source fallback, Unicode/BOM/CRLF coordinates, multiline values, and PEM body-only reads.
- [ ] Preserve qualified-literal labels, anchor maps, match reasons, cross-path hints, ranked tails, no-match query handling, errors, and the final response guard on hits and misses.
- [ ] Preserve raw match counts, pagination, column widths, caller options, callable boundaries, context allocations/order, omission notices, byte ceilings, and source priority after joins.
- [ ] Same-size/same-mtime edits, file replacement/deletion/unavailability, changed grammar/representation, and custom field/regex/exception changes cannot reuse stale safe results.
- [ ] Off-to-on transitions and policy reload during a blocked job retain each request's pinned policy; disabled and ordinary `parse` CLI paths preserve their existing behavior.
- [ ] Completed clean entries, transient parse/failure states, eviction, same-key failure cleanup, saturation, and non-empty shutdown workloads have independent assertions.
- [ ] Source files and persisted indexes remain original; filesystem permission checks, parser cancellation/deadline, index writer ownership, and read-only JSON-RPC contracts are unchanged.

## Acceptance Criteria
- [ ] `docs/briefs/evidence/redact-perf/05-acceptance.md` closes the full source-to-final-response coverage matrix with executed proof and no unresolved secret exposure, stale-safe reuse, or lifecycle failure.
- [ ] The three primary conditions and all five historical operations are reproducible from repository-owned data; state/rich/phase populations remain separately identifiable.
- [ ] Repeated-request overhead improves for both enabled conditions, with cache and parallel contributions distinguished and all repeatable regressions resolved or explicitly accepted; no fixed ms target is asserted.
- [ ] The report and maintenance guide describe the implemented boundaries and known limitations accurately. Prewarming adoption/deferment is recorded, and actual request-path runtime overlap remains proven.

## Open Questions
- None — The user selected improvement/regression verification without a fixed millisecond SLO; remaining implementation choices are specified in this brief.
