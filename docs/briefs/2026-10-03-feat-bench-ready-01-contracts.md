# [feat] 1.0.0 벤치마크 실행 계약 고정

> 2026-10-03 완료. 사용자 요청으로 이전 벤치마크를 제거하고 현재 1.0.0 준비 구성에 맞춰 갱신했다. [검증 근거](evidence/bench-ready/01-contract.json)를 기록했으며 실제 풀이·채점·채점 보정은 실행하지 않았다.

## Work Type
feat

## Current State (As-Is)
- [confirmed] 현재 프로필은 `grafana-routes` 하나이며 Grafana 3문항 × 6개 비교군 × 1회로 18개 슬롯을 만든다.
- [confirmed] 문항·근거·채점 JSON과 비교 대상 소스 핀은 유지한다. README와 문서 메타데이터 해시는 현재 안내에 맞춰 갱신했다. 초기 개정과 이전 실행기는 현재 작업 트리의 의존성이 아니다.
- [confirmed] 실행 조건은 `benchmark/data/grafana-execution-v1.json` 하나에서 읽으며 모델·추론 강도·한도와 평가 해석을 포함한다.

## Desired Outcome (To-Be)
- Introduce a versioned route contract consumed by preparation, transport, grading, metrics, and operator entry points, with one authoritative definition of the six arms and three question IDs.
- Bind solver `gpt-6.1-sol`/`medium`, grader `gpt-6-astra`/`high`, one repetition, and 18 planned executions without running a model.
- Keep frozen source facts, product identities, prompt isolation, intact.

## Scope
### In Scope
- Load and validate the route dataset's real shape directly. Separate solver-visible question text from private labels, scope interpretation, cross-language bindings, evidence, and grading notes.
- Define a new route profile, target-aware execution contract, compatibility boundary, and deterministic 18-row schedule contract. Expose only the current Grafana profile.
- Add an explicit policy for native tool output, client delivery observability, product-only tools with necessary auxiliary exceptions, local preparation mode, execution limits, and preparation-only status.
- Define minimum handoff shapes shared by the remaining children, including source/runtime/artifact identities, index readiness, judgment dimensions, raw/delivered output provenance, and decision provenance.
### Out of Scope
- [hard] Editing either frozen Grafana dataset version or the comparison lock, changing questions or difficulty coverage, executing solver/grader/calibration calls, or running a benchmark.
- [hard] Relabeling the current codemap-search package as 1.0.0 without a real version change. The evaluation snapshot must retain its actual version.
- [hard] Adding test files/cases or changing lint/formatter configuration without a separate explicit request.
- [deferred] More repositories, repeated trials, confidence intervals, and a general benchmark framework migration.

## Constraints
- Dataset manifest SHA256 is `8fb24b5b8cbc8a3530c61991dda2e7c3e80d859a96354adeb76f28ec9cc51cf4`; target lock SHA256 is `ff93c37b8c6588a5830738132440194ab331488cd122b46e849ef5d885fe7a51`; Grafana commit is `c6fad8695a96577eb466d425e6ac4a759ca30f47`.
- Required arms are `codemap-search`, `codegraph`, `zvec-grep`, `graphify`, `codebase-memory-mcp`, and `rg`. `codegraph` means the user-selected `colbymchenry/codegraph`.
- 현재 실행 계약만 유지한다. 이전 SPEC·프로필·데이터셋·결과·답변 재사용 코드는 제거하며 고정한 현재 문항과 비교 대상 핀은 보존한다.
- Retain current overall runtime limits and seed unless a concrete incompatibility requires replanning. These are separate from product response limits. Use sequential product execution for this preparation profile to avoid daemon and resource interference.
- Native response policy forbids harness byte/line caps, result enrichment, automatic follow-up fetching, and injected output-size overrides. It does not equate every product's defaults to codemap-search's 100000 setting.
- 제품군은 각 제품의 자체 도구를 우선하며 불가피한 경우에만 최소 보조 도구를 제공한다. `rg` 기준군은 추가 검색 제품이 없는 기본 코딩 에이전트로, 기본 셸·파일 탐색과 고정 rg 실행 파일을 제공하고 명령 선택은 LLM에 맡긴다. Graphify의 read 예외를 유지하고 zvec의 후속 rg 안내는 보조 도구 추가 근거로 쓰지 않는다.
- The user chose local search/indexing: Jev disabled, fresh Graphify code-only extraction, and zvec's documented local code embedding. External-AI tool modes are excluded. Solver/grader execution remains outside this briefset.

## Related Files / Entry Points
- `benchmark/route_contract.py`
- `benchmark/settings.py`
- `benchmark/data/grafana-execution-v1.json`
- `benchmark/data/grafana-routes-v2/manifest.json`
- `benchmark/data/grafana-comparison-targets-v1/targets.lock.json`
- `benchmark/execution.py`
- `docs/briefs/evidence/bench-ready/01-contract.json`

## Execution Plan
### Stage 1 — Establish immutable inputs and contract boundaries
- Starts when: the inspected branch and both frozen input directories are available.
- Work: Verify every SHA256SUMS member and the manifest-to-lock binding. Trace `load_profile → execution_settings/profiles/targets → prepare_plan` and verify the single route schema. Specify the new profile and handoff schemas before consumers are edited. Define the functional necessity gate for auxiliary tools: a documented required operation is unavailable through the native tool surface, with evidence from a neutral capability probe. Mere upstream advice to use rg is not sufficient.
- No-op when: an existing versioned contract already validates all frozen inputs, isolates solver prompts, exposes the exact six arms and 18-row schedule, and uses only current consumers.
- No-op handoff: publish the same complete `docs/briefs/evidence/bench-ready/01-contract.json` with observed implementation locators and unchanged hashes for children 02, 03, and 05 to continue.
- Deliverable: the proposed contract schema and a recorded integrity inventory for all seven route files and all four comparison-lock files.
- Verify: `Inspect each checksum entry against its file and trace the complete route configuration call chain`; Inputs: both frozen directories and the named settings functions; Expected: zero mismatches and one current schema without fields from retired campaigns.
- Ends when:
  - [x] The six target identities, three question IDs, counts, and both manifest bindings are exact.
  - [x] Solver-visible and grader-only fields have separate consumers.
- Handoff: Stage 2 receives the contract shape and verified immutable inputs.
- Replan when: a frozen checksum or source identity differs. Stop consumers, report the mismatch to the parent, and resolve the input identity before any adapter work.

### Stage 2 — Implement the route profile and dependency contract
- Starts when: Stage 1 has verified the frozen inputs and defined the compatibility boundary.
- Work: Add route loading and identity hashing, select the exact models, retain `medium`/`high` efforts, and define one repetition with deterministic ordering. Store tool availability and policy provenance separately from observed execution results. Define prepared artifacts, indexes, dimension judgments, and output-delivery fields with versioned schemas that later children can extend without editing frozen content.
- Deliverable: route configuration and reader, plus `docs/briefs/evidence/bench-ready/01-contract.json` containing `schema_version`, `status`, `input_hashes`, `source_commit`, `question_ids`, `arms`, `solver`, `grader`, `repetitions`, `planned_solver_runs`, `limits`, `policy_decisions`, `contract_paths`, and verification evidence.
- Verify: `Inspect the route profile's resolved configuration and enumerate the question × arm × repetition product without starting execution`; Inputs: the new route contract plus both frozen manifests; Expected: 18 unique planned keys, solver gpt-6.1-sol/medium, grader gpt-6-astra/high, zero observed runs, and exactly one current profile.
- Ends when:
  - [x] Native product limits and client delivery limits occupy separate fields with explicit units.
  - [x] Every nested frozen input, relevant harness source, and effective config contributes to the route identity.
  - [x] Any unavailable exact model is reported as unavailable rather than silently substituted.
- Handoff: children 02, 03, and 05 consume `docs/briefs/evidence/bench-ready/01-contract.json`; the parent uses its hashes as the first dependency gate.
- Replan when: a downstream consumer requires an incompatible shared shape. Stop that consumer, revise this contract and its evidence, then revalidate affected dependency edges before resuming.
- Worker decision: use the current route API and remove retired callers. Keep preparation functions free of model invocation side effects.

## Side Effect Checkpoints
- [x] 현재 프로필과 고정 입력만 읽으며 삭제된 실행기나 데이터셋을 불러오지 않는다.
- [x] No private evidence or route labels enter a solver prompt, tool instruction, or source checkout.
- [x] The selected model is exact and fallback is disabled; account availability is not claimed from documentation alone.
- [x] The planned count excludes future grader calls and does not imply 18 completed observations.
- [x] 기준군의 native_agent_baseline, 제품군의 native_only/assisted 조건을 구분하며 낮은 점수나 검색 품질 때문에 보조 도구를 추가하지 않는다.

## Acceptance Criteria
- [x] Inspect the resolved route contract and find exactly the six required arms, three fixed question IDs, one repetition, and 18 planned solver keys.
- [x] Recompute the two stated identity hashes and every member checksum and observe no changes to frozen files.
- [x] Inspect `01-contract.json` and confirm all required fields, policy decision provenance, zero executed counts, and usable addresses for every downstream consumer.
- [x] Trace a route prompt construction path and confirm it reads question text and common instructions only; confirm retired profile entry points are absent.

## Open Questions
- None — the user selected native product tools first with necessary rg/read exceptions, and local-only search/index preparation. Per-arm capability classification is implementation investigation under those decisions.
