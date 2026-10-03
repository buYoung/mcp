# [feat] 고정 문항의 익명 채점 계약 연결

> 2026-10-03 완료. 사용자 요청으로 이전 벤치마크를 제거하고 현재 1.0.0 준비 구성에 맞춰 갱신했다. [검증 근거](evidence/bench-ready/05-grading.json)를 기록했으며 실제 풀이·채점·채점 보정은 실행하지 않았다.

## Work Type
feat

## Current State (As-Is)
- [confirmed] `benchmark/route_grading.py`가 핵심 21개·확장 8개와 사실별 correct/supported의 true/false/indeterminate를 처리한다.
- [confirmed] 인용 파싱은 보조 정보이며 최종 판단은 LLM이 맡는다. 도구·모델명 노출은 입력 확정 전에 차단하고 오류 메시지의 비교군 경로도 가린다.
- [confirmed] `benchmark/grading_support.py`는 원답변 단위와 가역 경로 익명화를 제공한다. 채점 모델은 gpt-6-astra/high이며 실제 채점·보정은 실행하지 않았다.

## Desired Outcome (To-Be)
- Make the existing frozen rubric executable without changing its facts, thresholds, or grader model/effort.
- Preserve independent fact/dimension uncertainty and isolate extended diagnostics from core completeness.
- Prepare blind grader inputs, source access, judgment validation, and deterministic classification without calling the grader or calibrating it.

## Scope
### In Scope
- Versioned route judgment schema, rubric packet construction, validation, per-fact classification, and deterministic aggregation semantics.
- Blind answer packets with preserved citation scope, equivalent source citations, and stable fact IDs bound to the fixed Grafana commit.
- Prepare the future calibration input/instruction contract and mark calibration unexecuted. Reuse existing verification mechanisms or bounded truth-table inspection without adding test cases.
- Keep grader configuration at `gpt-6-astra`/`high` and leave solver prompts free of grading material.
### Out of Scope
- [hard] Actual grading, calibration, solver calls, benchmark execution, or filling missing observations with invented scores.
- [hard] Changing frozen facts, turning notes into requirements, adding a new rubric or new test files/cases.
- [deferred] Judge-model comparisons, human adjudication campaigns, and claims of empirically validated difficulty or grading reliability.

## Constraints
- Preserve exact core counts 6/7/8 and extended counts 2/3/3. Never require every evidence ID, file, function name, or negative warning to be repeated in an answer.
- `fulfilled` means both dimensions true. Any false means `not_fulfilled`, preserving an unknown in the other dimension. Otherwise an unknown means `indeterminate`.
- `core_complete` is false when any core fact fails, indeterminate when no core fact fails and some are unknown, and true only when every core fact is fulfilled.
- An extended error/unknown does not automatically change core completeness. A real contradiction to a core fact affects that specific core fact. Independent extra claims remain separate diagnostics.
- Missing citations are not evaluator uncertainty. Conversely, evaluator source-access failure is not evidence that the answer is wrong. Preserve reasons independently for every false/unknown dimension.
- Grade citations actually supplied by the answer. The private answer key can validate interpretation but must not become invented answer citations. Equivalent citations at the fixed commit are allowed.
- 인용 파싱·source access 표시는 보조 정보이며 LLM의 `supported` 판단을 강제하지 않는다. 파싱되지 않은 표기도 원답변과 answer unit으로 평가할 수 있다. 원답변의 의미와 표기를 보존하고 도구·모델명 노출은 정확한 구간을 별도 기록한 뒤 채점 입력 확정을 차단한다.
- Packet byte limits for the grader are a separate contract from search-tool output defaults. Preserve evidence through explicit packet composition/splitting rather than silent rubric or citation truncation.

## Related Files / Entry Points
- `benchmark/route_grading.py`
- `benchmark/grading_support.py`
- `benchmark/data/grafana-routes-v2/grading.json`
- `benchmark/data/grafana-routes-v2/dataset.json`
- `benchmark/data/grafana-routes-v2/evidence.json`
- `docs/briefs/evidence/bench-ready/05-grading.json`

## Execution Plan
### Stage 1 — Map the rubric to an executable schema
- Starts when: `docs/briefs/evidence/bench-ready/01-contract.json` provides the frozen input hashes and versioned route consumer contract.
- Work: Map all 29 fact IDs, two dimensions, unknown reasons, citation references, and non-scoring notes into a route judgment schema. Trace packet creation through judgment validation to existing reporting. Use only the frozen route schema and validation path.
- No-op when: an existing route grader already implements every frozen dimension rule, source-citation boundary, blind packet requirement, and unchanged model configuration with offline evidence.
- No-op handoff: publish that verified schema and evidence at `docs/briefs/evidence/bench-ready/05-grading.json` for child 06 without changing the rubric.
- Deliverable: exact route judgment schema, semantic mapping, and source/citation access contract.
- Verify: `Compare every frozen grading clause and every fact ID with the proposed judgment schema and classification rules`; Inputs: dataset, grading, evidence, and current packet/validator paths; Expected: 21 core and eight extended facts represented, two three-valued dimensions per fact, and zero scoring requirements derived from notes.
- Ends when:
  - [x] Every rubric rule has a deterministic consumer and every uncertainty reason has a field.
  - [x] 현재 사실별·차원별 판정만 사용하며 문항 전체로 오류·보류를 확산하지 않는다.
- Handoff: Stage 2 receives the route schema and packet boundary.
- Replan when: satisfying a rule would require modifying frozen content or changing the grader. Stop and return the specific conflict to the parent rather than weakening the rubric.

### Stage 2 — Implement blind packets and dimension classification
- Starts when: Stage 1 has mapped all rubric rules and citation semantics.
- Work: Implement route-specific packet construction and validation. Hide target/model/tool identifiers from grader-visible wrappers while preserving code identifiers and citations. Resolve answer citations against the fixed source, accept equivalent source locations, retain exact cited ranges, and avoid exposing unrelated target metadata. Carry each dimension and reason through classification without whole-answer coercion.
- Deliverable: route packet/schema implementation, reversible blind-ID mapping outside grader-visible input, deterministic fact/core classification, and prepared calibration contract with zero invocations.
- Verify: `Inspect prepared packet fields and manually evaluate the complete 3×3 dimension truth table plus core/extended propagation rules`; Inputs: the route schema, all allowed dimension pairs, frozen rubric, and source supplementation path; Expected: exact fact-state semantics, no target identity leak, no invented citations, and unchanged grader gpt-6-astra/high.
- Ends when:
  - [x] Missing citation and evaluator access failure take different supported-dimension paths.
  - [x] Core failure preserves unrelated known and unknown dimensions rather than zeroing the answer.
  - [x] Extended-only problems and optional notes cannot silently enlarge the core denominator.
- Handoff: Stage 3 receives packet and classification implementations for evidence publication.
- Replan when: a packet limit would silently remove required evidence or rubric clauses. Stop packet readiness and correct bounded packet composition before continuing, without invoking the judge to discover the failure.

### Stage 3 — Publish grading readiness without calibration
- Starts when: packet inspection and deterministic semantic checks are complete.
- Work: Document the future grader entry path, unchanged model settings, schema hashes, private source bundle requirements, and unresolved empirical calibration status. Make validation failures explicit operational errors rather than zero scores or retries counted as extra solver attempts.
- Deliverable: `docs/briefs/evidence/bench-ready/05-grading.json` with `schema_version`, `status`, `contract_hash`, `rubric_hash`, `judgment_schema`, `packet_contract`, `fact_counts`, `truth_table_evidence`, `grader`, `calibration_runs: 0`, `grader_runs: 0`, and limitations.
- Verify: `Inspect the saved grading handoff against the frozen rubric and all deterministic classifications`; Inputs: `05-grading.json`, route implementation, and unchanged grading.json; Expected: all semantics represented, zero model calls, and calibration explicitly unvalidated.
- Ends when:
  - [x] Metrics can consume partial uncertainty without guessing a question-wide status.
- Handoff: child 06 consumes `docs/briefs/evidence/bench-ready/05-grading.json` and the route judgment schema.
- Replan when: a consumer cannot preserve the dimension contract. Stop that consumer, correct its mapping or this schema with the owning child, and regenerate the handoff before resuming.

## Side Effect Checkpoints
- [x] Grader and solver inputs remain separate even when source bundles share paths.
- [x] Blinding removes six-arm identity wrappers without corrupting legitimate source identifiers.
- [x] Alternative citations are checked against the same commit rather than rejected for differing from the answer-key path list.
- [x] 현재 실행·기록 경로가 제거한 프로필이나 과거 결과를 읽지 않는다.
- [x] Runtime-only claims and static-source limitations are not converted into new required facts.

## Acceptance Criteria
- [x] All nine dimension combinations classify as specified, and core completeness follows the frozen false/unknown/true precedence.
- [x] Prepared packets contain only required grading context and actual answer citations, with target identity mapping kept outside the grader-visible payload.
- [x] The 21 core, eight extended, and non-scoring notes remain separate throughout schema, classification, and handoff.
- [x] `05-grading.json` records unchanged gpt-6-astra/high and zero grader/calibration calls, with no claim of empirical validation.

## Open Questions
- None — the user retained the grader and the frozen rubric already determines the scoring contract.
