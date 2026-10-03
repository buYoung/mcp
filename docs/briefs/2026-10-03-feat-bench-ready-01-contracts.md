# [feat] 1.0.0 벤치마크 실행 계약 고정

> 2026-10-03 완료. 사용자 요청으로 이전 벤치마크를 제거하고 현재 1.0.0 준비 구성에 맞춰 갱신했다. [검증 근거](evidence/bench-ready/01-contract.json)를 기록했으며 실제 풀이·채점·채점 보정은 실행하지 않았다.

## Work Type
feat

## Current State (As-Is)
- [confirmed] 현재 프로필은 `grafana-routes` 하나이며 Grafana 3문항 × 7개 비교군 × 1회로 21개 슬롯을 만든다.
- [confirmed] 문항·근거·채점 JSON과 비교 대상 소스 핀은 유지한다. README와 문서 메타데이터 해시는 현재 안내에 맞춰 갱신했다. 초기 개정과 이전 실행기는 현재 작업 트리의 의존성이 아니다.
- [confirmed] 실행 조건은 `benchmark/data/grafana-execution-v1.json` 하나에서 읽으며 모델·추론 강도·한도와 평가 해석을 포함한다.

## Desired Outcome (To-Be)
- Introduce a versioned route contract consumed by preparation, transport, grading, metrics, and operator entry points, with one authoritative definition of the seven arms and three question IDs.
- Bind one selected solver: default `gpt-6.1-sol`/`medium` with Codex CLI or `claude-opus-5-5`/`medium` with Claude Code CLI; always retain grader `gpt-6-astra`/`high`, one repetition, and 21 planned executions without running a model.
- Keep frozen source facts, product identities, prompt isolation, intact.

## Scope
### In Scope
- Load and validate the route dataset's real shape directly. Separate solver-visible question text from private labels, scope interpretation, cross-language bindings, evidence, and grading notes.
- Define a new route profile, target-aware execution contract, compatibility boundary, and deterministic 21-row schedule contract. Expose only the current Grafana profile.
- Add an explicit policy for native tool output, client delivery observability, product-only tools with necessary auxiliary exceptions, local preparation mode, execution limits, and preparation-only status.
- Define minimum handoff shapes shared by the remaining children, including source/runtime/artifact identities, index readiness, judgment dimensions, raw/delivered output provenance, and decision provenance.
### Out of Scope
- [hard] Editing either frozen Grafana dataset version or the comparison lock, changing questions or difficulty coverage, executing solver/grader/calibration calls, or running a benchmark.
- [hard] Relabeling the current codemap-search package as 1.0.0 without a real version change. The evaluation snapshot must retain its actual version.
- [hard] Adding test files/cases or changing lint/formatter configuration without a separate explicit request.
- [deferred] More repositories, repeated trials, confidence intervals, and a general benchmark framework migration.

## Constraints
- Dataset manifest SHA256 is `ee37ee238fe1dc6fc858cf18816c42e707917aae6760b203e6dec85626a71e24`; target lock SHA256 is `b7203a8a1664d327d17ec49af610ad7a1f50771cafeae6290bb5b454e971640c`; Grafana commit is `c6fad8695a96577eb466d425e6ac4a759ca30f47`.
- Required arms are `codemap-search`, `codegraph`, `zvec-grep`, `graphify`, `codebase-memory-mcp`, and `rg`. `codegraph` means the user-selected `colbymchenry/codegraph`.
- 현재 실행 계약만 유지한다. 이전 SPEC·프로필·데이터셋·결과·답변 재사용 코드는 제거하며 고정한 현재 문항과 비교 대상 핀은 보존한다.
- 기존 회차별 시간·호출·토큰 한도와 seed를 유지하며 제품 응답 기본값과 구분한다. 사용자 요청에 따라 기본 동시 실행 수는 4로 고정하고 준비 설정·비교군별 실행 설정·메뉴까지 같은 값을 전달한다. 사전 색인을 재사용하며 같은 비교군의 문항은 색인·홈 복원 충돌을 막기 위해 순차 실행하는 계약이다. 실제 병렬 실행은 이 준비 브리프의 범위에 포함하지 않는다.
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
- No-op when: an existing versioned contract already validates all frozen inputs, isolates solver prompts, exposes the exact seven arms and 21-row schedule, and uses only current consumers.
- No-op handoff: publish the same complete `docs/briefs/evidence/bench-ready/01-contract.json` with observed implementation locators and unchanged hashes for children 02, 03, and 05 to continue.
- Deliverable: the proposed contract schema and a recorded integrity inventory for all seven route files and all four comparison-lock files.
- Verify: `Inspect each checksum entry against its file and trace the complete route configuration call chain`; Inputs: both frozen directories and the named settings functions; Expected: zero mismatches and one current schema without fields from retired campaigns.
- Ends when:
  - [x] The seven target identities, three question IDs, counts, and both manifest bindings are exact.
  - [x] Solver-visible and grader-only fields have separate consumers.
- Handoff: Stage 2 receives the contract shape and verified immutable inputs.
- Replan when: a frozen checksum or source identity differs. Stop consumers, report the mismatch to the parent, and resolve the input identity before any adapter work.

### Stage 2 — Implement the route profile and dependency contract
- Starts when: Stage 1 has verified the frozen inputs and defined the compatibility boundary.
- Work: Add route loading and identity hashing, select the exact models, retain `medium`/`high` efforts, and define one repetition with deterministic ordering. Store tool availability and policy provenance separately from observed execution results. Define prepared artifacts, indexes, dimension judgments, and output-delivery fields with versioned schemas that later children can extend without editing frozen content.
- Deliverable: route configuration and reader, plus `docs/briefs/evidence/bench-ready/01-contract.json` containing `schema_version`, `status`, `input_hashes`, `source_commit`, `question_ids`, `arms`, `solver`, `grader`, `repetitions`, `planned_solver_runs`, `limits`, `policy_decisions`, `contract_paths`, and verification evidence.
- Verify: `Inspect the route profile's resolved configuration and enumerate the question × arm × repetition product without starting execution`; Inputs: the new route contract plus both frozen manifests; Expected: 21 unique planned keys, selected solver and its backend, default gpt-6.1-sol/medium, unchanged grader gpt-6-astra/high, zero observed runs, and exactly one current profile.
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
- [x] The planned count excludes future grader calls and does not imply 21 completed observations.
- [x] 기준군의 native_agent_baseline, 제품군의 native_only/assisted 조건을 구분하며 낮은 점수나 검색 품질 때문에 보조 도구를 추가하지 않는다.

## Acceptance Criteria
- [x] Inspect the resolved route contract and find exactly the seven required arms, three fixed question IDs, one repetition, and 21 planned solver keys.
- [x] Recompute the two stated identity hashes and every member checksum and observe no changes to frozen files.
- [x] Inspect `01-contract.json` and confirm all required fields, policy decision provenance, zero executed counts, and usable addresses for every downstream consumer.
- [x] Trace a route prompt construction path and confirm it reads question text and common instructions only; confirm retired profile entry points are absent.

## Open Questions
- None — the user selected native product tools first with necessary rg/read exceptions, and local-only search/index preparation. Per-arm capability classification is implementation investigation under those decisions.
