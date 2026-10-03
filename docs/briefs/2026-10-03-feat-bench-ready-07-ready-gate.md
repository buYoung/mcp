# [feat] 단일 벤치마크 준비 게이트 통합

> 2026-10-03 완료. 사용자 요청으로 이전 벤치마크를 제거하고 현재 1.0.0 준비 구성에 맞춰 갱신했다. [검증 근거](evidence/bench-ready/07-readiness.json)를 기록했으며 실제 풀이·채점·채점 보정은 실행하지 않았다.

## Work Type
feat

## Current State (As-Is)
- [confirmed] `benchmark/cli.py`는 catalog/prepare만 제공한다. CUI는 단일 프로필의 6개 비교군을 준비한 뒤 종료한다.
- [confirmed] `benchmark/route_prepare.py`가 선행 기록·소스·바이너리·색인·복원본·조회 기록을 확인하고 18개의 미실행 슬롯과 풀이 입력을 만든다.
- [confirmed] 실제 풀이·채점·보정은 준비 범위에서 제외한다. 전체 캠페인의 실행 연결은 별도 작업으로 남아 있으며 과거 실행기를 이용한 우회 경로는 없다.

## Desired Outcome (To-Be)
- Provide a working preparation-only operator path that selects the requested Grafana profile, validates all six arms, and produces exactly 18 planned single-attempt rows.
- Produce an auditable final readiness manifest bound to every frozen and prepared input, with explicit invalidation and per-arm native-only/necessary-assistance conditions.
- End at `prepared_not_executed`: no solver, grader, calibration, or benchmark run is part of this briefset.

## Scope
### In Scope
- Integrate route catalog/profile selection, preparation, exact planned targets, model settings, and readiness output with the current CLI/CUI.
- Prevent preparation/import/startup hooks from invoking a solver, grader, calibration, or external generative model. Keep a distinct launch boundary for a future separately scoped task. Required local embedding computation remains allowed for index preparation and neutral capability probes.
- Aggregate artifact/index/transport/grading/metric evidence, validate identity consistency, and expose meaningful blockers instead of silently dropping an arm.
- Document the exact prepared condition, fresh-state policy, invalidation rules, planned count, and limitations in the existing benchmark README.
### Out of Scope
- [hard] Calling `start` or `resume` to run any solver, executing gold questions, grading/calibrating, or generating benchmark observations.
- [hard] Adding auxiliary tools without a functional necessity record, substituting tools/models, reducing the six-arm set, or relabeling an actual 0.11.0 subject build as 1.0.0.
- [hard] New test files/cases, release publication, pushes, or broad CI changes.
- [deferred] Real model delivery verification, empirical difficulty confirmation, judge calibration execution, and the actual 18-run campaign.

## Constraints
- Frozen manifest SHA256 is `8fb24b5b8cbc8a3530c61991dda2e7c3e80d859a96354adeb76f28ec9cc51cf4`; target lock SHA256 is `ff93c37b8c6588a5830738132440194ab331488cd122b46e849ef5d885fe7a51`.
- Solver is exactly `gpt-6.1-sol`/`medium`, grader stays `gpt-6-astra`/`high`, and the plan is three fixed questions × six required arms × one repetition. Eighteen counts solver slots, not future grader invocations.
- Use each tool's native output/default tool surface as recorded, with no common response-size tuning, artificial rg line cap, or compensating result enrichment.
- The user chose local search/indexing and native product tools first. Freeze each arm's minimal rg/read exception, if any, from child 04's capability evidence before producing the final plan. Do not add assistance for native short responses or weak retrieval.
- 현재 단일 프로필과 고정 문항만 유지한다. 과거 답변 재사용·후보 선택·실행 재개 경로는 제공하지 않는다.
- A technical blocker or missing arm prevents a ready status. Lack of actual solver/grader execution is an explicit scope limitation, not a failed preparation check.

## Related Files / Entry Points
- `benchmark/cli.py`
- `benchmark/start.mjs`
- `benchmark/ready.py`
- `benchmark/execution.py`
- `benchmark/route_prepare.py`
- `benchmark/README.md`
- `benchmark/checks.py`
- `benchmark/cui_checks.py`
- `docs/briefs/evidence/bench-ready/07-readiness.json`

## Execution Plan
### Stage 1 — Reconcile dependency evidence and actual entry points
- Starts when: `docs/briefs/evidence/bench-ready/04-indexes.json` verifies all six prepared states and `docs/briefs/evidence/bench-ready/06-metrics.json` verifies measurement/report contracts, with their transitive contract, delivery, artifact, and grading hashes available.
- Work: Recompute dependency hashes and inspect the current CLI/CUI call chain. Confirm every prepared artifact is the artifact selected by the route plan. Define invalidation when model/config, source, product binary/dependencies, embedding weights, index, tool schemas/defaults, client version, or harness code changes.
- No-op when: the current preparation path already emits a valid exact-18 manifest with all dependency identities, zero solver/grader/calibration calls, visible user decisions and auxiliary necessity evidence, and a verified stop before launch.
- No-op handoff: publish this evidence at `docs/briefs/evidence/bench-ready/07-readiness.json` for final user review, with no execution implied.
- Deliverable: dependency-consistency record and an integration map from operator selection to prepared plan.
- Verify: `Resolve every dependency manifest and compare its hashes with the artifacts selected by catalog/prepare`; Inputs: all six child evidence files, route profile, and actual CLI/CUI entry points; Expected: no stale identity, no missing arm, and no implicit call to start/resume or a solver/grader entry point.
- Ends when:
  - [x] The route plan consumes the intended models, source, product defaults, and judgment schema.
  - [x] Every identity change has an explicit readiness invalidation rule.
- Handoff: Stage 2 receives the integrated identity map and remaining operator wiring work.
- Replan when: any dependency evidence is stale or contradicts another. Stop the final gate, return the mismatch to its owning child, and revalidate the affected dependency chain before resuming.

### Stage 2 — Implement preparation-only operator flow
- Starts when: Stage 1 has reconciled all component identities and entry points.
- Work: Wire route catalog and preparation, expose only the current profile, and expose a reviewable plan that stops before launch. Add a preparation-mode guard around solver/grader invocation boundaries, not just a CUI confirmation label. Present six required arms, per-question source/reset policy, configured limits, actual subject version, native output policy, local-only mode, and any necessarily assisted arm labels. Keep future launch APIs separate and do not call them.
- Deliverable: integrated preparation-only CLI/CUI flow and a deterministic 18-row plan with all observed results absent.
- Verify: `Exercise the preparation path with the existing fake-Codex/CUI facilities and inspect the produced plan and process log`; Inputs: route profile, six prepared states, and current entry points; Expected: exactly 18 unique planned keys, zero solver/grader/calibration invocations, no fallback model/tool, and an explicit stop before launch.
- Ends when:
  - [x] Preparation and readiness imports/hooks cannot accidentally start a solver, grader, or external generative model.
  - [x] Repeated preparation preserves verified input identity and creates no solver attempt or reused answer.
  - [x] A missing required arm produces a blocker rather than a smaller plan.
- Handoff: Stage 3 receives the integrated flow and generated preparation plan for final evidence.
- Replan when: any preparation path invokes a solver/grader/calibration or bypasses native-output policy. Stop integration, correct the boundary in the owning module, and repeat the existing offline verification before continuing.

### Stage 3 — Verify the final stop boundary and publish readiness
- Starts when: the integrated preparation path produces the exact plan and stops before execution.
- Work: Run existing offline suites, validate frozen hashes and all component evidence, inspect the actual task-owned prepared artifacts, and document operational limitations. Publish readiness with separate technical readiness, per-arm auxiliary necessity evidence, and unexecuted scientific validation. Update README commands only to the preparation boundary and label future execution/calibration as outside this briefset.
- Deliverable: `docs/briefs/evidence/bench-ready/07-readiness.json` with `schema_version`, `status: prepared_not_executed`, `technical_ready`, `execution_authorized: false`, `input_hashes`, `host`, `models`, six `arms`, 18 `planned_runs`, `policy_decisions`, `invalidation_rules`, `checks`, `solver_runs: 0`, `grader_runs: 0`, `calibration_runs: 0`, and `limitations`, plus updated operator documentation.
- Verify: `python3 -m unittest benchmark.checks benchmark.cui_checks`; Inputs: existing suites from the repository root plus the integrated preparation manifest and frozen checksum lists; Expected: suites pass, all frozen hashes match, six arms and 18 unique planned rows are present, and solver/grader/calibration counters remain zero.
- Ends when:
  - [x] README and manifest distinguish prepared artifacts from actual model/quality measurements.
  - [x] Technical blockers are resolved or the manifest remains explicitly not ready.
  - [x] Every auxiliary exception is bound to evidence and an explicit assisted-arm label, and the local-only condition is fixed.
- Handoff: the parent and user receive `docs/briefs/evidence/bench-ready/07-readiness.json` and the preparation instructions. Stop here; the benchmark campaign is a separate task.
- Replan when: any final check fails. Stop final readiness, activate bounded correction in the owning child, update parent topology/handoffs if ownership changes, and re-run only affected verification plus this integration gate.

## Side Effect Checkpoints
- [x] Frozen dataset/lock files remain byte-identical and archived runs remain readable.
- [x] No scheduler, startup hook, or readiness command can accidentally execute a gold question, solver, grader, or calibration call.
- [x] Manifest records actual subject version and client/runtime versions without claiming future release completion.
- [x] Native product output omissions remain visible, while delivery faults remain separately attributable.
- [x] Personal daemons/caches and repository source content remain intact after preparation.

## Acceptance Criteria
- [x] The operator can select the route profile and obtain a validated six-arm, three-question, one-attempt plan through the preparation path.
- [x] `07-readiness.json` binds all frozen and prepared identities, invalidates on material drift, and contains exactly 18 unique planned slots with zero observations.
- [x] Existing offline suites pass and actual prepared artifacts/indexes have component-level non-question probe evidence.
- [x] Solver, grader, and calibration invocation counters remain zero throughout integration.
- [x] Final documentation explicitly states remaining empirical limitations and any necessarily assisted arm conditions, and stops before benchmark execution.

## Open Questions
- None — the user selected local-only preparation and native product tools first with unavoidable rg/read exceptions. Actual benchmark execution is excluded from this task.
