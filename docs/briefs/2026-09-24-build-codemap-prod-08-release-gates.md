# [build] Gate production readiness on executed evidence

## Work Type
build

## Current State (As-Is)
- [confirmed] The inspected release target set contains Linux x64 GNU/musl, Linux arm64 musl, macOS arm64/x64, and Windows x64/arm64 MSVC — Evidence: the `build.strategy.matrix.include` entries in `.github/workflows/codemap-search-release.yml` at revision `8b8222003584a3875ce3804557d3d5e70113ea4b`.
- [confirmed] Windows release builds are best-effort and arm64 Windows is build-only; publication currently depends on the build job — Evidence: `continue-on-error`, `best_effort`, the arm64 comment, and `publish-release`/`publish-crate` dependencies in that workflow.
- [confirmed] CI executes `cargo test` on Ubuntu and native release builds on three operating systems, not runtime verification of every distributed target — Evidence: `.github/workflows/codemap-search-ci.yml` jobs.
- [confirmed] `Cargo.toml` declares edition 2021 without `rust-version` — Evidence: the package manifest. The audit used rustc/cargo 1.98.1, which is not proof of a minimum supported version.
- [confirmed] Earlier audit checks passed 102 e2e and 101 unit tests, with one Clang-dependent e2e ignored; no seven-target runtime or release performance matrix was executed — Evidence: the audit's exact Cargo commands/results, not a production-readiness claim.
- [inferred] A green current release build can still ship incomplete/unexecuted target coverage — Confirm by inspecting and negatively exercising publication-gate conditions after implementation; do not publish to prove the gate.

## Desired Outcome (To-Be)
- All seven distributed targets are required build-and-execute targets; unavailable, missing, skipped, or failed required evidence blocks production acceptance and publication.
- The package declares and verifies an evidence-backed minimum Rust version with the locked dependency graph while retaining edition 2021.
- One integrated revision satisfies the safety, correctness, resource, performance, and compatibility contracts of all seven implementation children through final consumer tests.
- Packaged binary identity is tied to execution evidence, and the final handoff reports actual results rather than recommended future work.

## Scope
### In Scope
- Minimum Rust/toolchain declaration and verification, mandatory target CI/release gates, target-compatible test execution, and artifact identity checks.
- Replay of every predecessor's concrete fixture/measurement contract plus integrated interactions, operator refusal/recovery guidance, and documentation alignment.
- Focused test-harness binary selection or portable verification adapters needed to execute the actual target binary rather than accidentally testing a host binary.
### Out of Scope
- [hard] Creating a release/tag, publishing packages/assets, accessing signing credentials, changing signing identities, or provisioning privileged/external runners without separate execution authorization.
- [hard] Dropping targets, retaining best-effort success, converting required cases to ignored, or waiving the user-approved 10% p95 gate.
- [hard] An unrelated language-extraction/ranking overhaul, edition migration, mass dependency update, or lint/formatter setup.
- [deferred] New language feature coverage and unrelated whole-repository audits; newly discovered blockers on the requested paths return to the parent for an explicit bounded correction route.

## Constraints
- Required targets are exactly `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, and `aarch64-pc-windows-msvc`.
- A cross-build, mocked OS result, host-binary execution, or missing artifact is not target execution. Use a target-compatible operating environment, record native/virtualized/translated execution explicitly, and never compare performance across different environment classes.
- Environment inventory is technical investigation, not permission to acquire credentials or provision infrastructure. If any required runner/toolchain/Clang/NASM capability is unavailable, finish safe preparation but keep the overall result incomplete until authorized evidence is supplied.
- Select an MSRV from the locked dependency requirements and used stable APIs, then verify it; do not choose the author's installed compiler merely for convenience. Check the latest supported stable toolchain separately without silently changing dependency versions.
- Performance comparisons use baseline `8b8222003584a3875ce3804557d3d5e70113ea4b` and the final candidate on the same target/host/toolchain/config/fixture. Use two warm-ups, 100 samples per ordinary operation, three paired runs, nearest-rank p95, and raw ordered data. Each run/operation must meet candidate p95 <= 1.10 * baseline p95; an unstable or failed comparison is unresolved, not waived.
- Replay ordinary operations separately: ping, exact-name search, folder overview, 200-line read, grep content without expansion, default callable grep, count, and files-with-matches. Freeze exact nonempty corpus/request hashes before timing; separate cold/readiness and edit/overload scenarios from warm-request latency.
- Re-run resource/counter populations from each child and prove final output correctness. Stable wall time or RSS alone does not prove a bounded buffer or linear scan. Ordinary latency cells must deliver equivalent positive output on baseline and candidate; approved read refusals and other explicit negative cases belong to separate admission/error populations and cannot manufacture a speedup.
- Replay grep's four mode/expansion populations with response caps unset/configured and finite/zero `head_limit`. Preserve unconfigured non-expanded full results, `-32602` for configured final masked-text overflow, and callable body-budget pagination plus its optional final response check. Verify cap-minus-one/exact/plus-one final text bytes, UTF-8/masking length changes, and config precedence without substituting a new default cap.
- Distinguish grep's bounded auxiliary/capture buffers from required ordering metadata and requested/serialized output. Uncapped `head_limit=0` may scale with returned bytes; require removal of duplicate/off-page body staging without falsely claiming a constant total-memory ceiling.
- Tests may be added as authorized. Keep ordinary functional tests free of flaky hardware timing assertions; run deterministic counter assertions normally and hardware measurements in a required controlled verification job.
- Publication gates must require successful evidence from all seven targets, exact binary/archive identity, and all required test populations. Guard against skipped jobs, zero matched tests, partial artifact lists, and `continue-on-error` masking.
- Preserve signing/notarization and package formats. Verify the packaged payload identity after any signing mutation, and execute that payload or provide an equally explicit artifact-linked execution step before publication.
- All repository commands start at the repository root. Do not treat this plan-authoring request as permission to run publication or modify external infrastructure.

## Related Files / Entry Points
- `apps/codemap-search/Cargo.toml` — start at package metadata and dependencies before declaring the verified `rust-version`.
- `apps/codemap-search/Cargo.lock` — determine the resolved dependency/toolchain floor without an unrelated update.
- `.github/workflows/codemap-search-ci.yml` — establish mandatory test/MSRV/runtime verification routes.
- `.github/workflows/codemap-search-release.yml` — remove best-effort acceptance and gate both publication jobs on complete verified target evidence.
- `apps/codemap-search/tests/e2e/helpers.rs` — verify which executable the test harness launches before adding a narrow explicit binary adapter.
- `apps/codemap-search/verify` — retain the existing project verification entry point.
- `apps/codemap-search/scripts/verify.py` — reuse the current Cargo/verification orchestration where suitable without treating public-corpus checks as the new runtime benchmark.
- `apps/codemap-search/tests/e2e/` — replay final MCP, watcher, permissions, masking, Jev, and native-consumer populations.
- `apps/codemap-search/docs/distribution/` — update the Windows best-effort/build-only wording and minimum source-build requirements to match verified policy.
- `apps/codemap-search/docs/configuration.md` — reconcile final safety limits, native refresh, and recovery contracts.
- `apps/codemap-search/docs/configuration.ko.md` — align numeric values and user-visible behavior in Korean.
- `apps/codemap-search/README.md` — update only affected supported-target/toolchain/read-limit guidance.
- `apps/codemap-search/README.ko.md` — mirror the final public contract.

## Execution Plan
### Stage 1 — Audit handoffs and establish the target matrix
- Starts when: `docs/briefs/evidence/codemap-prod/01-index-recovery.md`, `docs/briefs/evidence/codemap-prod/02-path-boundary.md`, `docs/briefs/evidence/codemap-prod/03-refresh-state.md`, `docs/briefs/evidence/codemap-prod/04-refresh-work.md`, `docs/briefs/evidence/codemap-prod/05-bounded-read.md`, `docs/briefs/evidence/codemap-prod/06-bounded-grep.md`, and `docs/briefs/evidence/codemap-prod/07-mcp-framing.md` provide locally verified integrated contracts, concrete target replay cases, and raw measurement identities.
- Work: Reconcile all handoffs against one candidate revision, inventory usable target environments and native tools, determine the evidence-backed MSRV candidate, freeze per-target workload manifests including Child 06's mode/cap/head-limit success-error matrix and memory categories, and identify missing artifact/runtime/publishing-gate checks.
- No-op when: The exact integrated revision already has complete seven-target artifact-linked execution, verified MSRV/latest-stable coverage, enforced fail-closed publication gates, and every global acceptance result.
- No-op handoff: Record the full proof in `docs/briefs/evidence/codemap-prod/08-production-acceptance.md` (proposed); the parent may accept a no-edit outcome only after verifying all nonempty target/case/sample populations.
- Deliverable: `docs/briefs/evidence/codemap-prod/08-production-acceptance.md` (proposed), with integrated revision, predecessor provenance, seven-target environment/workload matrix, MSRV evidence, required test counts, missing capabilities, and gate design.
- Verify: `Inspect all seven predecessor handoffs and the release matrix against the integrated revision`; Inputs: the seven exact evidence paths above, Cargo manifest/lock, both workflows, and target environment inventory; Expected: every target and audit concern has an assigned executable proof with no unsupported success claim or unowned blocker.
- Ends when:
  - [ ] The candidate revision, MSRV candidate, artifact identities, and complete target/case/sample populations are fixed.
  - [ ] Missing environments are explicitly blocking and authorized preparation is separated from external provisioning.
- Handoff: Stage 2 receives the reconciled matrix in `docs/briefs/evidence/codemap-prod/08-production-acceptance.md`.
- Replan when: A predecessor contract is unverified or a required environment is unavailable; stop dependent acceptance, return to the parent, activate bounded correction/environment fulfillment plus re-verification, and recalculate topology/handoffs before resuming.

### Stage 2 — Enforce toolchain and publication proof
- Starts when: Stage 1 has fixed the toolchain/target matrix and the implementation environment can perform the required checks without unauthorized actions.
- Work: Declare and verify MSRV, implement mandatory build/runtime/measurement gates for every target, link test execution to the intended binary/artifact, and make both publish paths depend on complete successful verification. Add negative gate fixtures for a failed/missing/skipped target and missing/incorrect artifact identity.
- Deliverable: Integrated manifest/workflow/harness changes and the exact gate/replay commands in `docs/briefs/evidence/codemap-prod/08-production-acceptance.md`.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml --locked --all-targets`; Inputs: the integrated crate on the explicitly recorded MSRV and latest stable toolchains; Expected: exit 0 on each recorded compiler, plus non-publishing negative gate inspection/replay proving failed or missing required evidence prevents both publish paths.
- Ends when:
  - [ ] Declared MSRV matches executed evidence and all seven target checks are mandatory.
  - [ ] No best-effort, skipped-job, zero-test, host-binary, or missing-artifact path can produce an accepted release result.
- Handoff: Stage 3 receives the runnable target matrix and fail-closed gate contract.
- Replan when: A toolchain/target dependency requires a new compatibility break or external runner change; keep publication blocked, return the concrete decision to the parent, and re-verify after the bounded resolution.
- Worker decision: Use existing authorized CI/runner capabilities and small portable test adapters; do not introduce a new CI service or broad dependency migration.

### Stage 3 — Execute integrated seven-target acceptance
- Starts when: Stage 2's target-compatible environments, gates, and exact artifact-linked test route are available.
- Work: Build and execute every target, replay all predecessor correctness/counter/stress cases, exercise default native preprocessing with tools present, run the complete existing crate test suite, and repeat paired release performance populations. Inspect final documentation against actual caps/error/recovery behavior. Return any production defect to its owning child and invalidate dependent evidence before resuming.
- Deliverable: `docs/briefs/evidence/codemap-prod/08-production-acceptance.md`, with revision/target/compiler/OS identity, artifact hashes, exact commands, nonzero executed-case counts, all seven results, raw sample/counter paths, p95 ratios, doc parity, and explicit remaining blockers or a complete acceptance decision.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --locked`; Inputs: the complete integrated crate test suite and separately recorded target-specific binary/fixture replay commands for all seven targets; Expected: nonzero executed suites pass, every required native/OS-specific case actually runs, all artifact identities match, and each resource/linear-work/p95 gate passes.
- Ends when:
  - [ ] All seven target rows contain executed build/runtime evidence with matching packaged-binary identity, not just compile success.
  - [ ] Every local/global acceptance and side-effect check has current-revision evidence with no required ignored/unavailable/failed item.
- Handoff: The parent receives `docs/briefs/evidence/codemap-prod/08-production-acceptance.md` for whole-set acceptance only; publishing still requires separate authorization.
- Replan when: Any proof fails or is unavailable; stop acceptance, return to the parent, reactivate the bounded owning correction and re-verification, and recalculate dependent handoffs before any release-ready claim.

## Side Effect Checkpoints
- [ ] Both publish jobs require the complete verification gate; a deliberately failed/missing/skipped Windows arm64 result blocks publication just like Linux/macOS failure.
- [ ] Linux glibc/musl distinctions, macOS architecture/signing, Windows archive names, checksums, and install-channel contracts remain intact.
- [ ] Source installation on the declared MSRV and latest stable works with the locked graph and existing C toolchain requirements.
- [ ] Integrated source permissions, index ownership, ignore updates, saturation/shutdown, read masking, grep counts/pages, and frame boundaries retain every predecessor contract.
- [ ] Grep cap absence, configured `-32602` overflow, callable pagination/final checks, and `head_limit=0` semantics match Child 06's matrix after final masking; no new mandatory grep cap or truncated-success substitution is introduced.
- [ ] Jev deadlines, cancellation cleanup, disabled/provider-fallback behavior, and exact-source masking remain covered offline without accessing real API keys.
- [ ] English/Korean README, configuration, and distribution documents agree with exact implemented limits and mandatory target support.

## Acceptance Criteria
- [ ] The exact seven required targets each have successful build and real target-compatible runtime results tied to their packaged binary identity.
- [ ] MSRV is declared from evidence and the integrated crate passes it and latest stable without an unrelated dependency/edition migration.
- [ ] The complete existing suite, added focused populations, required native preprocessing case, and every predecessor final-consumer/counter replay pass with nonzero executed counts.
- [ ] Every target passes the grep mode/cap/head-limit success-error matrix at final masked-byte boundaries, preserving uncapped output and configured errors while proving bounded auxiliary retention separately from necessary metadata/result storage.
- [ ] Every ordinary operation on each required target meets candidate p95 <= 1.10 * baseline p95 in all three controlled paired release runs; scaled workloads prove the resource/linear-work contracts rather than only a favorable elapsed-time sample.
- [ ] Publication cannot proceed with incomplete evidence; no required case is excused as best-effort, merely cross-built, ignored, or environment-unavailable.
- [ ] The final report has no unresolved requested-scope blocker and all affected public documentation matches the implementation. Otherwise report incomplete, never production-ready.

## Open Questions
- None — The user selected all seven targets, necessary test additions, explicit over-cap read refusal, and mandatory resource/scaling/10% p95 gates; unavailable execution prerequisites remain blockers, not waived decisions.
