# [perf] Avoid unrelated full refresh work

## Work Type
perf

## Current State (As-Is)
- [confirmed] At revision `8b8222003584a3875ce3804557d3d5e70113ea4b`, native macro expansion is enabled by default — Evidence: `MacroExpansionConfig::default()` in `apps/codemap-search/src/config/macro_expansion.rs`.
- [confirmed] With that setting enabled, `refresh_paths_with_reload()` promotes any non-index event path to a full-root reconciliation, not only native dependencies — Evidence: the macro-expansion branch in `src/index/engine.rs`.
- [confirmed] `index_files_changed_with_reload()` revisits eligible native files whenever macro expansion is enabled — Evidence: `should_refresh_macro_files` and the mtime skip condition in `src/index/engine.rs`.
- [confirmed] Published derived snapshots are reconstructed from committed documents after successful changed passes — Evidence: `load_published_snapshot()` and `publish_pass_result()`; this cost is separate from filesystem traversal and native preprocessing.
- [inferred] Non-native repositories and unrelated edits incur unnecessary full walks/preprocessing — Confirm by counted release runs below; the audit did not measure a release speedup.

## Baseline Measurement
- No release baseline exists for this optimization. Build the immutable pre-initiative revision `8b8222003584a3875ce3804557d3d5e70113ea4b` in an isolated checkout and compare it with the integrated predecessor revision and the candidate using identical fixture/config/toolchain inputs.
- Generate non-native repositories of 1,000, 2,000, and 4,000 fixed-size source files; edit one ordinary Rust/TypeScript source. Record full-walk invocations, visited entries, parsed files, native subprocess starts, publication latency, CPU time, peak RSS, and unchanged request outputs.
- Add a mixed repository containing C/C++ translation units, internal/external headers, assembler includes, and compilation settings. Compare unrelated TypeScript edits with each actual native dependency change, missing-to-present dependency, and config toggle.
- Separate traversal/preprocessing costs from whole-snapshot publication. Do not claim constant-cost publication or extend this child into an incremental derived-index rewrite.
- Freeze exact fixture hashes and requests before optimization. Use release binaries, isolated `CODEMAP_HOME`, no concurrent builds, two warm-ups and 100 measured calls per operation in each of three paired baseline/candidate runs; retain raw samples and use nearest-rank p95.
- Required targets: unrelated non-native edits perform zero full-root walks and zero native subprocess launches after readiness, while final results equal full reconciliation. Per-operation ordinary-request p95 must be at most 1.10 times the paired baseline on the same host/config.

## Desired Outcome (To-Be)
- Ordinary changes use the narrow refresh path when they cannot affect native preprocessing.
- Native input changes still invalidate every affected translation unit, including known external inputs and missing dependencies that become available.
- Measurements distinguish reduced traversal/preprocessing from unchanged derived-snapshot costs and prove no correctness or latency regression.

## Scope
### In Scope
- Native refresh eligibility, dependency/config invalidation, conservative escalation, and test-only counters for actual work.
- Before/after fixture measurements and correctness comparisons against full reconciliation with macro expansion both enabled and disabled.
### Out of Scope
- [hard] Disabling default macro expansion to manufacture a speedup.
- [hard] Weakening Child 03's ignore/overflow full-reconciliation obligations or changing its queue/state contract.
- [deferred] Incremental construction of all derived event/flow/implementation indexes; measure that remaining cost but do not rewrite those algorithms here.
- [hard] Parser parallelism, external compiler feature expansion, and new public performance knobs.

## Constraints
- Consume the bounded reconciliation contract from Child 03; required full invalidations always take precedence over narrow-path optimization.
- Preserve compiler flags, include resolution, native-process timeout/output caps, diagnostics, and original-source coordinate semantics.
- Unknown dependency provenance must cause conservative full reconciliation, not a guessed partial update.
- Introduce no benchmark-only behavior that changes public output; counters must be opt-in/test-only and payload-free.
- The user authorized focused correctness/load tests and reproducible measurement tooling; use no real source secrets or production repositories in generated fixtures.
- Run commands from the repository root. Use already available authorized Clang/NASM environments, and keep missing target execution as a blocking item for Child 08 rather than accepting an ignored case.

## Related Files / Entry Points
- `apps/codemap-search/src/index/engine.rs` — start at `refresh_paths_with_reload()` and the native mtime bypass in full refreshes.
- `apps/codemap-search/src/index/preprocess/` — inspect recorded input stamps, compilation settings, and failed-expansion dependencies before choosing narrow invalidation.
- `apps/codemap-search/src/index/supervisor.rs` — preserve periodic checks for external macro inputs.
- `apps/codemap-search/src/config/macro_expansion.rs` — preserve default enablement and native-process limits.
- `apps/codemap-search/tests/e2e/watcher.rs` — verify non-native and native edit convergence.
- `apps/codemap-search/tests/e2e/tools.rs` — retain the existing native header-refresh test as a final-consumer oracle.
- `apps/codemap-search/scripts/` — place a focused opt-in measurement driver beside existing verification tools if the current harness cannot record work counters.
- `apps/codemap-search/docs/configuration.md` — describe the new precise native refresh behavior without promising incremental snapshot publication.
- `apps/codemap-search/docs/configuration.ko.md` — mirror the same runtime contract.

## Execution Plan
### Stage 1 — Capture counted refresh baselines
- Starts when: `docs/briefs/evidence/codemap-prod/03-refresh-state.md` supplies the verified invalidation state machine, queue limits, and full-reconciliation obligations.
- Work: Build the three comparison revisions as applicable, create the non-native/mixed fixtures, freeze requests and hashes, and instrument traversal, parse, native process, and publication counts independently.
- No-op when: Counted current runs already meet every narrow-refresh, native-correctness, and p95 criterion with no edit required.
- No-op handoff: Record the complete proof in `docs/briefs/evidence/codemap-prod/04-refresh-work.md` (proposed); the parent validates it and sends the same proof to Child 08.
- Deliverable: `docs/briefs/evidence/codemap-prod/04-refresh-work.md` (proposed), containing fixture/workload manifests, exact commands, environments, raw baseline samples, per-phase counters, and the proposed invalidation rule.
- Verify: `Inspect the counted runs against the Baseline Measurement population`; Inputs: 1,000/2,000/4,000-file non-native fixtures and mixed native dependency cases on release binaries; Expected: nonzero indexed populations, separately attributed costs, and baseline results captured before optimization.
- Ends when:
  - [ ] Required invalidations are distinguishable from unnecessary full walks by measured work counts.
  - [ ] The fixture/output oracle and per-operation p95 baseline are frozen.
- Handoff: Stage 2 receives the counted baseline and proposed rule at `docs/briefs/evidence/codemap-prod/04-refresh-work.md`.
- Replan when: The suspected cost is not reproduced or dependencies cannot be identified safely; stop optimization, return to the parent for a bounded revised rule or no-change proof, and recalculate handoffs after verification.

### Stage 2 — Narrow only provably unrelated refreshes
- Starts when: Stage 1 has frozen the correctness oracle and work-count baseline.
- Work: Integrate a conservative native-dependency decision that avoids unrelated full walks/process starts while preserving every forced-full obligation and failed/missing-input recovery path.
- Deliverable: The integrated invalidation decision and counter evidence in `docs/briefs/evidence/codemap-prod/04-refresh-work.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml --locked`; Inputs: engine/preprocessor/supervisor changes and the inherited refresh contract; Expected: exit 0 and a bounded inspection showing unknown provenance still escalates safely.
- Ends when:
  - [ ] Unrelated non-native edits reach the path refresh without a native subprocess.
  - [ ] Header/include/settings changes retain their complete affected translation-unit set.
- Handoff: Stage 3 receives the integrated rule and unchanged baseline populations.
- Replan when: An optimization misses a native/ignore/config dependency; restore the conservative decision, correct the owning rule, and re-verify before any successor is released.
- Worker decision: Reuse existing dependency stamps or a small explicit native-presence/dependency summary; avoid adding a new general dependency engine.

### Stage 3 — Verify equivalence and measured savings
- Starts when: Stage 2 is integrated with Child 03 and the same measurement inputs are available.
- Work: Replay all scaled fixtures and native mutations, compare final search/overview with full-reconciliation oracles, run the existing native integration test with required tools present, and repeat paired release measurements.
- Deliverable: `docs/briefs/evidence/codemap-prod/04-refresh-work.md`, with baseline/candidate raw samples, counters, final-output equivalence, exact commands, dependency edge cases, and native target replay requirements.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests test_macro_expansion_reaches_search_read_and_refreshes_header_changes -- --ignored --nocapture`; Inputs: the existing native header-refresh fixture with Clang installed; Expected: exactly one previously ignored test actually executes and passes, alongside nonzero added scaled/dependency cases recorded in the handoff.
- Ends when:
  - [ ] Unrelated edit counters meet the zero-full-walk/zero-native-process target at every fixture size.
  - [ ] Required native/ignore invalidations and the 1.10 p95 gate pass without changing defaults.
- Handoff: Child 08 receives `docs/briefs/evidence/codemap-prod/04-refresh-work.md` and the integrated native replay matrix.
- Replan when: A required tool/target is unavailable or output/counter/latency criteria fail; do not report acceptance, return to the parent for environment fulfillment or bounded correction and re-verification.

## Side Effect Checkpoints
- [ ] External header polling, compilation database changes, native config enable/disable, and newly supplied dependencies still refresh visible declarations.
- [ ] Macro-disabled ignore-only invalidation and saturated watcher recovery from Child 03 still pass.
- [ ] Preprocessor subprocesses retain bounded execution/output and cleanup on failure/shutdown.
- [ ] Search and codemap still consume the same committed generation with unchanged masking and source coordinates.

## Acceptance Criteria
- [ ] Every scaled non-native fixture avoids full-root traversal and native subprocesses for an unrelated single-file edit after readiness.
- [ ] Every affected native population matches a full-reconciliation oracle, including external and missing-to-present inputs.
- [ ] Paired release evidence proves the work reduction and p95 at most 1.10 times baseline for every ordinary operation; snapshot publication cost is reported separately.
- [ ] The native integration test executes rather than being silently ignored, and the final handoff supports all seven target replays in Child 08.

## Open Questions
- None — The user approved measurement-first work and the 10% p95 limit; conservative invalidation and bounded worker choices are specified.
