# [fix] Make refresh invalidation bounded and lossless

## Work Type
fix

## Current State (As-Is)
- [confirmed] At revision `8b8222003584a3875ce3804557d3d5e70113ea4b`, ordinary watcher paths become `RefreshPaths`, while `.git` internals except HEAD hints are filtered — Evidence: `accumulate()`, `classify_event_path()`, and `GIT_REF_HINT_PATHS` in `apps/codemap-search/src/index/watcher.rs`.
- [confirmed] A healthy watcher suppresses request-triggered reconciliation — Evidence: `EngineSupervisor::trigger_refresh()` in `src/index/supervisor.rs`.
- [confirmed] With macro expansion disabled, path refresh has no explicit ignore-policy invalidation branch; with the default enabled, ordinary paths instead force full reconciliation — Evidence: `refresh_paths_with_reload()` in `src/index/engine.rs` and `MacroExpansionConfig::default()`.
- [confirmed] Filesystem and config watchers receive unbounded channels; the filesystem debounce thread can block sending to a capacity-one indexer queue — Evidence: `spawn_watcher()`, `run_debounce_loop()`, and `spawn_config_watcher()`.
- [confirmed] `accumulate()` continues retaining paths after the 1,024-path full-walk threshold; `IndexerHandle::trigger_refresh()` drops a full-queue full-refresh request without checking the queued command kind — Evidence: the respective implementations in `src/index/watcher.rs` and `src/index/indexer.rs`.
- [inferred] Ignore-only changes or saturated command/event queues can leave stale results or unbounded retained event data — Confirm by the rule-change and blocked-indexer fixtures below, not by the audit's successful ordinary-edit tests.

## Reproduction
- Disable macro expansion, wait for index readiness, modify only an existing `.gitignore` or `.codemapignore`, and leave affected source bytes unchanged. Test both exclusion and reinclusion. Expected: search/overview converge to a fresh full-index oracle without restart or another source edit.
- Enable `use_git_exclude`, edit only `.git/info/exclude` in place, and repeat in a linked worktree using its actual Git metadata location. Expected: the same autonomous convergence regardless of macro expansion.
- Hold the indexer with a deterministic test gate, saturate the command queue, then generate repeated paths and more than 1,024 distinct paths. Expected: finite retained slots/bytes, a durable full-reconciliation signal, and complete convergence after release.
- Queue `RefreshPaths`, then request a full refresh from config/request fallback. Expected: the full invalidation is not treated as redundant with a partial path batch.
- Disconnect and shut down at each gate; inject backend errors/rescan events. Expected: no lost required reconciliation, deadlock, or permanently healthy-but-inactive watcher. Establish actual pre-fix results and frequency on disposable fixtures.

## Desired Outcome (To-Be)
- Every supported ignore-policy change invalidates the affected indexed population even when no source file changes.
- Event buffering and pending path state have finite item and byte budgets without silently losing refresh obligations.
- Overflow, retry, publication, and shutdown preserve one explicit reconciliation state until the matching generation is committed and published.

## Scope
### In Scope
- Ignore-policy change detection, full-versus-path command merging, watcher/config-watcher buffering, overflow escalation, and health fallback.
- Inventorying `.gitignore`, `.codemapignore`, `.ignore`, Git exclude, and supported parent/global ignore sources used by the shared walker; monitor or safely recheck resolved out-of-root policy inputs.
- Deterministic saturation/retry/shutdown regressions and final search/overview versus full-index comparisons.
### Out of Scope
- [hard] Optimizing which native-source events need a full walk; Child 04 owns that after this correctness contract stabilizes.
- [hard] Changing directory exclusion semantics, ignore precedence, or `include_ignored` behavior.
- [hard] A new asynchronous request scheduler, parser parallelism, or public watcher-debug MCP API.

## Constraints
- Preserve `IndexCommand::Refresh`, `RefreshPaths`, single writer ownership, atomic published generations, and watcher-before-indexer shutdown semantics; private queue plumbing may change atomically with all senders/consumers.
- A full invalidation must dominate partial batches and remain pending through contention/failure until successful publication. Coalescing is not dropping evidence.
- Bound both message count and payload bytes. After full reconciliation is required, stop retaining redundant individual paths. Do not block the notify callback indefinitely.
- Keep access events and internal index writes from creating feedback loops. Read-only permissions for source tools remain those established by Child 02.
- Freeze exact private queue/path/byte bounds in Stage 2 with saturation evidence; values must be finite, enforced at admission, and reported, not left as implementation TODOs.
- Compare ordinary-request p95 against immutable baseline `8b8222003584a3875ce3804557d3d5e70113ea4b` on the same release host/toolchain/config with at most 10% regression. Freeze exact nonempty fixture/request hashes before edits, use two warm-ups and 100 measured calls per operation in each of three paired runs, retain raw samples, and compute nearest-rank p95. Measure ordinary read/search/overview/grep requests separately from edit/overload convergence.
- The user authorized focused tests and measurement tools. Keep all fixtures isolated and synthetic and run commands from the repository root.

## Related Files / Entry Points
- `apps/codemap-search/src/index/watcher.rs` — start at event classification, debounce, threshold escalation, and send backpressure.
- `apps/codemap-search/src/index/indexer.rs` — make full/path requests and retry/publication acknowledgement lossless.
- `apps/codemap-search/src/index/supervisor.rs` — preserve healthy-watcher suppression only when refresh obligations remain serviceable.
- `apps/codemap-search/src/config.rs` — apply the same bounded notification and durable refresh rule to config reloads.
- `apps/codemap-search/src/workspace.rs` — enumerate the actual ignore sources and preserve their precedence.
- `apps/codemap-search/tests/e2e/watcher.rs` — add autonomous ignore-only and saturation convergence checks.
- `apps/codemap-search/tests/e2e/exclusions.rs` — preserve config and ancestor exclusion behavior.
- `apps/codemap-search/docs/configuration.md` — document policy-change convergence and overload/recovery behavior.
- `apps/codemap-search/docs/configuration.ko.md` — align the same operational contract in Korean.

## Execution Plan
### Stage 1 — Pin invalidation and overload interleavings
- Starts when: `docs/briefs/evidence/codemap-prod/02-path-boundary.md` supplies the verified source-root contract and local reader regressions.
- Work: Enumerate supported policy inputs, pin rule-only changes and command/event saturation with deterministic gates, and capture release baseline latency plus retained event slots/bytes. Compare each final indexed population with an independently built full-index oracle.
- No-op when: Every policy-input and saturation case already passes with finite enforced budgets, durable invalidation, and complete final-output evidence.
- No-op handoff: Write the proof to `docs/briefs/evidence/codemap-prod/03-refresh-state.md` (proposed); the parent validates it before releasing Child 04 and Child 08.
- Deliverable: `docs/briefs/evidence/codemap-prod/03-refresh-state.md` (proposed), with policy-input inventory, failing interleavings, full-index oracle identities, baseline samples, and proposed bounded reconciliation state transitions.
- Verify: `Inspect the policy inventory and gated replay traces against the full-index oracle`; Inputs: both macro settings, exclude/reinclude edits, Git worktree paths, queue-full commands, backend errors, and shutdown gates; Expected: every supported policy source has a detection route and every negative case compares a known nonempty source population.
- Ends when:
  - [ ] Failures or a complete no-op proof are recorded before queue/state changes.
  - [ ] The state transition table explains when a full invalidation is set and cleared.
- Handoff: Stage 2 consumes the transition table and baseline at `docs/briefs/evidence/codemap-prod/03-refresh-state.md`.
- Replan when: An external ignore source cannot be observed under the existing policy; stop successors, return to the parent for bounded detection/recheck work, and revise handoffs after re-verification.

### Stage 2 — Integrate bounded reconciliation
- Starts when: Stage 1 has pinned policy invalidation and saturation behavior.
- Work: Implement bounded event ingress, durable full-refresh escalation, safe path coalescing, policy-input invalidation, and retry/publication acknowledgement as one correctness boundary. Keep shutdown responsive and feedback filtering intact.
- Deliverable: The integrated watcher/config/indexer state machine and numeric limits in `docs/briefs/evidence/codemap-prod/03-refresh-state.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml --locked`; Inputs: all event producers, command senders, and indexer publication consumers; Expected: exit 0 and a bounded-inspection trace showing every full-refresh producer reaches the durable invalidation state.
- Ends when:
  - [ ] Item/byte limits and overflow actions are explicit and enforced before buffering.
  - [ ] A queued path refresh cannot consume or clear an unrelated full invalidation.
- Handoff: Stage 3 receives the integrated state machine, gates, and limits.
- Replan when: A queue design must drop an obligation or can block shutdown; reject the design, return to Stage 2's owning parent route, and preserve the last correct behavior until re-verified.
- Worker decision: Choose a bounded channel plus coalesced dirty state or equivalent ownership model; retain existing observable configuration and result semantics.

### Stage 3 — Prove convergence under overload
- Starts when: Stage 2 is integrated and the deterministic gates exercise its actual final consumers.
- Work: Replay edits, saturated queues, contention retries, backend failures, and shutdown. Measure ordinary-request release latency against Stage 1 and record maximum admitted/retained slots and bytes. Verify both macro settings rather than relying on default full walks.
- Deliverable: `docs/briefs/evidence/codemap-prod/03-refresh-state.md`, containing transition/limit contracts, exact commands, nonzero case counts, oracle comparisons, raw performance data, and target replay instructions.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests -- e2e::watcher:: e2e::exclusions:: e2e::config:: --test-threads=1`; Inputs: existing and added policy/saturation fixtures; Expected: all selected tests execute and pass with final search/overview membership equal to the full-index oracle.
- Ends when:
  - [ ] No admitted workload exceeds the frozen budgets or loses required reconciliation.
  - [ ] Ordinary-request p95 is at most 1.10 times baseline and local lifecycle checkpoints pass.
- Handoff: Child 04 and Child 08 receive `docs/briefs/evidence/codemap-prod/03-refresh-state.md` with a correctness-preserving refresh contract.
- Replan when: Saturation or a p95 threshold fails; stop the dependent optimization, correct the owning state-machine path, rerun affected evidence, and reconcile the parent before proceeding.

## Side Effect Checkpoints
- [ ] Ordinary create/modify/delete, atomic save, directory rename, HEAD change, and custom index-directory exclusion still converge.
- [ ] `watch=false`, unavailable watcher, and dead-indexer fallback retain their documented behavior.
- [ ] Config reload remains independent of the filesystem watch switch and preserves request-pinned settings.
- [ ] No sender clone, blocked send, retry loop, or pending dirty state prevents watcher-first shutdown.
- [ ] Live find/grep ignore semantics remain consistent with the reconciled index; permissions and source identity stay unchanged.

## Acceptance Criteria
- [ ] Every inventoried ignore source has a verified change-detection/recheck route; exclusion and reinclusion work without touching affected source files.
- [ ] Saturation retains no more than the frozen item/byte limits and always recovers the same final population as full indexing.
- [ ] Full invalidations survive partial queues, writer contention, and failed publication until success.
- [ ] Lifecycle regressions and ordinary-request p95 gates pass on the implementation host, with all native replay cases handed to Child 08.

## Open Questions
- None — The user approved bounded-resource validation and the 10% p95 regression gate; private capacities are frozen by the measured Stage 2 contract.
