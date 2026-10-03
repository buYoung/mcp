# [feat] 검색 도구별 측정과 결과 기록 보존

> 2026-10-03 완료. 사용자 요청으로 이전 벤치마크를 제거하고 현재 1.0.0 준비 구성에 맞춰 갱신했다. [검증 근거](evidence/bench-ready/06-metrics.json)를 기록했으며 실제 풀이·채점·채점 보정은 실행하지 않았다.

## Work Type
feat

## Current State (As-Is)
- [confirmed] `benchmark/route_metrics.py`가 도구 응답·중계·모델 전달 관측, 호출·시간·사용량, 사실별 판정과 실패 사유를 분리해 기록한다.
- [confirmed] `benchmark/usage.py`와 `benchmark/runner.py`가 응답 ID별 사용량, 종료·취소·회수 기록과 불완전한 관측을 처리한다.
- [confirmed] 동일 에이전트로 검색 도구별 최종 답변 품질과 탐색 비용을 비교한다. 답변 작성 영향은 공통 사용 조건으로 허용하며 근거 단계별 분리 집계를 필수로 요구하지 않는다.

## Desired Outcome (To-Be)
- Capture enough provenance to attribute future quality and resource results to the correct arm, model, native response, client delivery, and source state.
- Preserve the frozen route rubric's known/false/unknown dimensions through reports, with separate core and extended diagnostics.
- Prepare target-aware result/report contracts for 18 single-attempt slots without producing fabricated observations or statistical claims.

## Scope
### In Scope
- Generic target/tool/call attribution, raw and delivered output accounting, usage preservation, source-read observability, and cutoff/failure status handling.
- Route-specific metrics and reports consuming the judgment schema from child 05, with distinct known counts, unknown counts, and denominators.
- Explicit one-attempt schedule/result linkage, resume identity checks, and separation of infrastructure failure from graded quality.
- Preserve preparation metadata separately from future search latency, token consumption, and grading results.
### Out of Scope
- [hard] Solver/grader calls, trial execution, fabricated performance numbers, or treating prepared artifacts as measured benchmark results.
- [hard] Filling unavailable observations with zero, counting undocumented source reads as known, or improving sparse tool responses.
- [hard] New test files/cases, product telemetry changes, or modifications to frozen datasets .
- [deferred] Pricing studies, repeated-sample statistics, significance testing, p95 claims, and generalized rankings across codebases.

## Constraints
- A response has separate product-native, relay, and client-observed stages. Record content/hash/byte counts at observable stages and use null plus reason where client-visible extent is unknown.
- Preserve all supported native result fields in raw logs. Derived text and source-line counts must not replace the raw record or invent source ranges from summaries.
- Namespaced target/tool identity is authoritative; literal names such as `read` or `search` alone are not enough across six products.
- Optional auxiliary rg/reader usage must be separately attributable, including the predeclared necessity reason and native-only/assisted arm label. An answer obtained with no product-specific calls cannot be presented as evidence of that product's retrieval contribution. Report assisted and native-only conditions transparently rather than claiming they expose identical tool sets.
- Keep raw provider usage fields alongside normalized totals, cached input, reasoning/output, and unknown indicators. Do not infer token counts from bytes or silently substitute a model when usage metadata differs.
- Report per-question results first. A three-route macro aggregate is descriptive of these routes only; expose any fact-weighted aggregate as a distinct denominator. Unknown dimensions must not disappear or become false in either view.
- One solver attempt means no automatic re-solve after a quality failure. Operational interruption/resume must preserve the same slot identity and clearly record whether a model invocation occurred. This profile does not reuse answers.
- 기본 에이전트의 탐색 호출은 `command_execution` item ID로 세고 started/completed를 중복 합산하지 않는다. 명령 선택과 후속 탐색은 LLM에 맡기며 특정 검색 도구 호출을 강제하거나 미사용을 품질 실패로 바꾸지 않는다. CLI 출력과 모델 전달 관측은 분리한다.
- JSON과 결과표에 `conditions_valid`, 구체적 오류·실패 사유, 경계 위반, 호출 로그 완결성, 원자료 경로를 보존한다. 무효 조건과 미관측 조건을 구별하고 모든 18개 슬롯을 유지한다.

## Related Files / Entry Points
- `benchmark/route_metrics.py`
- `benchmark/usage.py`
- `benchmark/runner.py`
- `benchmark/responses.py`
- `benchmark/route_grading.py`
- `docs/briefs/evidence/bench-ready/06-metrics.json`

## Execution Plan
### Stage 1 — Define observable quantities and provenance
- Starts when: `docs/briefs/evidence/bench-ready/02-delivery.json` supplies the raw/relay/client envelope and `docs/briefs/evidence/bench-ready/05-grading.json` supplies the route judgment schema.
- Work: Trace tool calls and usage events through normalization, finish handling, grading attachment, and report aggregation. Define each quantity's source, units, availability, and aggregation rule. Separate native response limits from relay/client clipping and known source reads from inferred coverage.
- No-op when: route records already preserve all delivery stages, target attribution, usage unknowns, and per-dimension grading with complete offline provenance evidence.
- No-op handoff: publish that evidence in `docs/briefs/evidence/bench-ready/06-metrics.json` for child 07 without inventing executed results.
- Deliverable: measurement/result schema with an origin and unit for every field, target-aware call categories, and unknown/failure rules.
- Verify: `Trace every proposed metric back to an observable event or explicit unknown reason`; Inputs: child 02 delivery schema, child 05 judgment schema, runner normalization, and execution reports; Expected: no byte-to-token inference, no guessed source ranges, and no unclassified whole-answer uncertainty coercion.
- Ends when:
  - [x] Tool-native and delivery-induced output loss have different attribution fields.
  - [x] Product calls and optional auxiliary rg/reader calls have distinct identities and counts.
  - [x] Preparation, solver, grader, and cleanup time have distinct scopes.
- Handoff: Stage 2 receives the measurement schema and observability boundaries.
- Replan when: a required field cannot be observed. Use an explicit unknown if the contract permits it, otherwise stop the consumer and return a bounded instrumentation correction to child 02 or 05.

### Stage 2 — Implement route result normalization and reporting
- Starts when: Stage 1 has defined units, provenance, and failure semantics.
- Work: Add target-aware call/usage normalization and route grading aggregation. Preserve partial known judgments, per-dimension reasons, raw events, output-limit ownership, and environment/index identity. Bind every future result to the exact question/arm/attempt key. Disable old A-result reuse for the route profile and make timeouts/cutoffs/errors operationally explicit.
- Deliverable: route result/report implementation and a schema-only report preview with all observed values absent and clearly marked unexecuted.
- Verify: `Inspect the route normalization and report derivations against every field in the delivery and grading contracts`; Inputs: new metric schema, existing captured-event helpers, all dimension combinations, and the 18 planned keys; Expected: units and unknowns preserved, no synthetic scored result, no answer reuse, and one stable attempt slot per key.
- Ends when:
  - [x] Core and extended counts remain independent and all unknown dimensions retain their reasons.
  - [x] Single-trial reports make no variance or significance claim.
  - [x] Missing usage/source-delivery evidence remains null with a reason rather than zero.
- Handoff: Stage 3 receives result and report implementations for verification publication.
- Replan when: a normalization or report shortcut changes the frozen grading semantics. Stop report readiness, correct the derivation using child 05's contract, and re-inspect all affected aggregates.

### Stage 3 — Publish measurement readiness without observations
- Starts when: Stage 2 has implemented route normalization and reporting for the single current profile.
- Work: Run the relevant existing offline checks, inspect report schema examples for the unexecuted state, and publish metric field provenance and remaining live-measurement limitations. Ensure future event delivery/cost claims are not implied by offline success.
- Deliverable: `docs/briefs/evidence/bench-ready/06-metrics.json` with `schema_version`, `status`, `input_hashes`, `measurement_schema`, `result_schema`, `field_provenance`, `unknown_rules`, `aggregation_rules`, `offline_verification`, `observed_solver_runs: 0`, and limitations.
- Verify: `python3 -m unittest benchmark.checks benchmark.cui_checks`; Inputs: existing repository suites plus bounded inspection of route report derivations; Expected: existing checks pass, no model calls occur, and the handoff contains schemas/evidence rather than fabricated trial results.
- Ends when:
  - [x] Child 07 can validate a planned 18-row report without materializing any completed result.
- Handoff: child 07 consumes `docs/briefs/evidence/bench-ready/06-metrics.json`.
- Replan when: existing checks fail or a report treats unknown as zero. Stop the final gate, correct the affected route path, and regenerate the measurement evidence.

## Side Effect Checkpoints
- [x] 현재 실행·기록 경로가 제거한 프로필이나 과거 결과를 읽지 않는다.
- [x] Optional auxiliary-tool usage and assisted-arm labels cannot be mistaken for a native-only product retrieval result.
- [x] Raw logs remain available for audit while grader-visible input stays blinded.
- [x] Error/timeout/cleanup failures are not silently converted into graded quality failures.
- [x] Preparation durations and native index sizes are not mixed into future solver metrics.

## Acceptance Criteria
- [x] Every route metric has an explicit event origin, unit, availability rule, and aggregation rule in `06-metrics.json`.
- [x] Native product truncation, harness/relay loss, client clipping, and unknown delivery are separately representable.
- [x] Per-fact true/false/indeterminate counts and reasons survive through per-route and aggregate reports without denominator manipulation.
- [x] The route result contract has exactly 18 planned keys, no reused A results, no completed observations, and no repeated-sample statistical claims.
- [x] Existing offline checks pass and real gpt-6.1-sol usage/delivery remains explicitly unmeasured.

## Open Questions
- None — native-output policy, one-attempt count, and the frozen grading contract determine this child's measurement boundary.
