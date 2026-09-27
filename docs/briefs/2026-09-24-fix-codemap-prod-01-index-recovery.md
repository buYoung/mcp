# [fix] Make index recovery ownership-safe

## Work Type
fix

## Current State (As-Is)
- [confirmed] Inspect revision `8b8222003584a3875ce3804557d3d5e70113ea4b` on `docs/jev-integrate`; the earlier `dcbbc7fe` audit's runtime code is unchanged — Evidence: Git history between those revisions and `TantivySearchEngine::new()` in `apps/codemap-search/src/index/engine.rs`.
- [confirmed] `TantivySearchEngine::new()` treats a missing/different `codemap.format` beside `meta.json` as a reason to wipe the entire configured directory; generic open failures reach the same `remove_dir_all(path)` branch — Evidence: `needs_wipe`, `open_index`, and `match opened` in `src/index/engine.rs`.
- [confirmed] Config parsing checks that `index_path` is nonempty, not that the directory belongs to this application — Evidence: `assign_config_key()` and `as_nonempty_string()` in `src/config.rs`.
- [confirmed] `FormatUpgradeLock::process_is_alive()` returns true on non-Unix systems — Evidence: the `cfg(not(unix))` implementation in `src/index/engine.rs`.
- [inferred] An unmanaged directory can lose unrelated files, and an abandoned Windows upgrade lock can block later starts — Confirm by the isolated destructive-recovery and abandoned-owner cases below; neither failure was dynamically reproduced during the audit.
- [confirmed] Existing corruption/format tests use dedicated temporary index directories and pass in the audit — Evidence: `test_engine_corrupt_recovery()` and `test_format_version_mismatch_rebuilds_exactly_once()`.

## Reproduction
- Use disposable directories only. Create an unmanaged directory containing an unrelated sentinel file and an unrelated `meta.json`, omit `codemap.format`, and pass that directory to `TantivySearchEngine::new()`. Expected: refuse unsafe adoption without changing any sentinel bytes. The static path predicts deletion; record the actual pre-fix result.
- Create a valid owned index, invalidate its extraction format, and start two isolated processes against it. Inject process termination around lock acquisition and replacement. Expected: at most one destructive migration and no live owner displaced.
- On Windows x64 and arm64, terminate a process after acquiring the upgrade lock and start another. Expected: recover an abandoned lock without manual deletion while still excluding a live owner. Record the target and actual frequency rather than assuming an observed Unix result applies to Windows.
- Inject permission and transient open failures without corrupting an owned index. Expected: preserve bytes and return an actionable failure rather than reinterpret every error as corruption.

## Desired Outcome (To-Be)
- Recovery deletes only verified application-owned index state and never adopts an arbitrary populated directory destructively.
- Concurrent starts, upgrade failure, and process death leave a recoverable index or an explicit non-destructive error on every supported target.
- Existing valid indexes and recognized legacy indexes retain a documented, safe upgrade route.

## Scope
### In Scope
- Index-directory ownership checks, error classification, format migration, and the lock protecting the complete migration transaction.
- Windows abandoned-owner recovery and Unix concurrency/PID-reuse safeguards required by the same transaction.
- Synthetic fault-injection and subprocess regression coverage, plus recovery instructions in the configuration documentation.
### Out of Scope
- [hard] Search scoring, extracted symbol schemas, and a general-purpose filesystem permission redesign.
- [hard] Deleting real user state, automatically renaming unknown user directories, or accessing signing/publishing credentials.
- [hard] Editing MCP framing or its test population; Child 07 may run concurrently.

## Constraints
- Keep the configured `index.path` spelling and existing default `.codemap/index`; reject unsafe targets at the owning engine boundary rather than silently changing the selected directory.
- Preserve typed failure information until deciding whether recovery is justified. A permission, lock, or temporary I/O failure is not evidence of corruption.
- Missing ownership evidence in a populated directory must fail closed. Recognize legacy indexes only through an explicit format/schema check; an arbitrary file named `meta.json` is insufficient.
- The upgrade lock must cover ownership revalidation, rebuild, and format publication. A stale-owner observation alone must not authorize replacing a subsequently acquired live lock.
- Preserve the single-writer model and watcher-before-indexer shutdown order. Do not introduce a lock held while waiting for a thread whose shutdown requires that lock.
- The user authorized focused reproduction, regression, and load-test additions. Do not add lint/formatter setup or unrelated tests.
- All commands run from the repository root. Native platform execution evidence is mandatory in Child 08; a local pass is not a seven-target pass.

## Related Files / Entry Points
- `apps/codemap-search/src/index/engine.rs` — start at `TantivySearchEngine::new()` and `FormatUpgradeLock` before changing recovery ownership.
- `apps/codemap-search/src/config.rs` — inspect `index_path` resolution and preserve its public configuration contract.
- `apps/codemap-search/Cargo.toml` — inspect existing platform dependency features before selecting lock APIs; coordinate any necessary feature change with Child 08's MSRV verification.
- `apps/codemap-search/src/index/supervisor.rs` — trace recovery errors through `ensure_alive()` and bounded restart attempts.
- `apps/codemap-search/src/main.rs` — trace index creation for MCP, search, index, and benchmark commands.
- `apps/codemap-search/tests/index_recovery.rs` (proposed) — colocate subprocess/ownership regressions without writing Child 07's MCP test file.
- `apps/codemap-search/docs/configuration.md` — document non-destructive refusal and the supported legacy recovery route.
- `apps/codemap-search/docs/configuration.ko.md` — keep recovery instructions aligned with the English contract.

## Execution Plan
### Stage 1 — Pin ownership and crash failures
- Starts when: The inspected revision and disposable fixture directories are available, and no other worker is modifying index recovery.
- Work: Add focused reproductions for unmanaged sentinels, valid/legacy indexes, typed open failures, concurrent starts, live locks, and abandoned owners. Identify the exact ownership evidence accepted by each current format.
- No-op when: The current code already passes every ownership/crash criterion with executed evidence, including the Windows cases rather than an unconditional liveness stub.
- No-op handoff: Write the complete proof to `docs/briefs/evidence/codemap-prod/01-index-recovery.md` (proposed); the parent validates it before allowing Child 02 and Child 08 to consume it without edits.
- Deliverable: `docs/briefs/evidence/codemap-prod/01-index-recovery.md` (proposed), with revision, fixture hashes, ownership decision table, expected/actual failures, target-specific evidence, and exact commands.
- Verify: `Inspect the recorded reproductions against TantivySearchEngine::new and FormatUpgradeLock`; Inputs: disposable unmanaged/owned/legacy directories and live/dead-owner process cases; Expected: nonempty sentinels are identified by hash and every failure is observed or explicitly blocked, never assumed reproduced.
- Ends when:
  - [ ] The failing ownership and lock interleavings are pinned, or a complete no-change proof is recorded.
  - [ ] The accepted legacy formats and non-destructive refusal cases are enumerated.
- Handoff: Stage 2 consumes the ownership/error matrix in `docs/briefs/evidence/codemap-prod/01-index-recovery.md`.
- Replan when: Reproduction disproves an audit inference or legacy ownership cannot be distinguished safely; stop dependent changes, return to the parent for bounded correction and re-verification, and recalculate handoffs before proceeding.

### Stage 2 — Implement the protected recovery transaction
- Starts when: Stage 1 has pinned the ownership/error matrix.
- Work: Enforce ownership-safe admission, classify failures, and implement crash-recoverable exclusive upgrade ownership across supported platforms. Keep valid-index opening and legacy migration in the same transaction contract.
- Deliverable: The integrated recovery implementation and migration/error contract recorded in `docs/briefs/evidence/codemap-prod/01-index-recovery.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml --locked`; Inputs: the modified engine, callers, and platform-gated lock code; Expected: exit 0 on the current host with other target checks explicitly assigned to Child 08.
- Ends when:
  - [ ] No destructive branch is reachable without verified ownership and exclusive upgrade protection.
  - [ ] Failed recovery preserves unrelated data and supplies a supported operator action.
- Handoff: Stage 3 receives the protected transaction and the exact reproduction matrix.
- Replan when: Safe recovery requires changing an existing public storage/CLI contract beyond non-destructive refusal; stop and return the compatibility decision to the parent.
- Worker decision: Choose a process-lifetime OS lock or verified platform-specific ownership mechanism using existing facilities first; justify any indispensable dependency and its target/MSRV effects.

### Stage 3 — Verify recovery and publish the handoff
- Starts when: Stage 2 is integrated and all Stage 1 fixtures remain available.
- Work: Run the new recovery target and existing engine tests, verify sentinel hashes after every failure, repeat the concurrency cases, and document the safe legacy route. Register unexecuted target cases with Child 08 without marking them passed.
- Deliverable: `docs/briefs/evidence/codemap-prod/01-index-recovery.md`, containing implementation revision, exact commands, nonzero case counts, sentinel results, ownership/lock contract, platform results, and the target replay recipe.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib index::engine::tests:: -- --test-threads=1`; Inputs: existing engine tests plus the new executed recovery matrix recorded in the handoff; Expected: all selected existing tests pass and the separate new target records nonzero executed cases with unchanged unmanaged sentinels.
- Ends when:
  - [ ] Local recovery and side-effect checks have executed results and no unresolved local failure.
  - [ ] The exact cross-target replay procedure is consumable by Child 08.
- Handoff: Child 02 and Child 08 receive `docs/briefs/evidence/codemap-prod/01-index-recovery.md`; the parent keeps global acceptance incomplete until all seven targets have passed.
- Replan when: A fault/concurrency case fails; return to Stage 2, stop successors that depend on the failed contract, re-verify, and reconcile parent ordering before resuming.

## Side Effect Checkpoints
- [ ] Default and custom index directories still open successfully through all four engine-creating CLI paths.
- [ ] Valid legacy indexes rebuild at most once and do not delete neighboring application/user files.
- [ ] Existing extraction sidecars, index-writer contention retries, stale-result notices, and capped auto-restarts keep their meanings.
- [ ] Watcher/config-watcher shutdown still releases every sender before the indexer is joined.

## Acceptance Criteria
- [ ] Every unmanaged sentinel survives attempted initialization/recovery byte-for-byte, including symlinked/custom targets and typed I/O failures.
- [ ] Executed host cases prove live owners are not reclaimed and concurrent/crashed upgrades recover safely; the identical seven-target replay population carries those assertions to Child 08, which owns final cross-target execution acceptance.
- [ ] Existing engine tests and the nonempty focused recovery population pass on the integrated revision.
- [ ] The handoff and both configuration documents describe the implemented refusal/migration/recovery behavior; missing target evidence remains a blocking item for the parent.

## Open Questions
- None — The user requires all seven target validations; ownership and lock implementation choices are bounded above.
