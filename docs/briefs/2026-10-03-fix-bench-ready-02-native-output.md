# [fix] 도구 기본 응답과 전달 기록 보존

> 2026-10-03 완료. 사용자 요청으로 이전 벤치마크를 제거하고 현재 1.0.0 준비 구성에 맞춰 갱신했다. [검증 근거](evidence/bench-ready/02-delivery.json)를 기록했으며 실제 풀이·채점·채점 보정은 실행하지 않았다.

## Work Type
fix

## Current State (As-Is)
- [confirmed] `benchmark/route_transport.py`가 제품의 초기화 정보·도구 목록·원본 응답을 보존하는 stdio/HTTP 중계를 제공한다.
- [confirmed] `benchmark/responses.py`는 도구 스키마와 원문 위치 해석만 담당하며 인위적인 응답 절단 기능을 제공하지 않는다.
- [confirmed] `benchmark/runner.py`는 제품군에 준비된 MCP를 연결하며, 기본 에이전트 기준군은 MCP 없이 기본 셸·파일 탐색을 사용한다.

## Reproduction
- Trigger: 중립 조회 결과가 32000바이트 또는 250행을 넘는다.
- Before: 제거한 이전 중계기는 제품 응답을 하네스 제한으로 잘라 원문 응답과 모델 전달 관측을 혼동했다.
- Current verification: 선행 설치·계약 준비 후 `python3 -m benchmark.route_transport inspect`로 기존 중립 전달 검증을 실행한다. 고정 rg의 원본 stdout과 MCP 중계 결과가 바이트 단위로 같고 두 과거 제한을 모두 넘는지 확인한다. 실제 문항이나 모델은 실행하지 않는다.
- Expected: 원본 응답·구조화 필드가 유지되고 Codex 전달 미관측은 별도로 남는다. 제품 기본 제한은 변경하지 않는다.

## Desired Outcome (To-Be)
- Give the solver the native result the selected product actually returned, including its own default sparsity or truncation, without a common harness size limit.
- Support real MCP initialization, stdio and Streamable HTTP, native tool discovery/instructions, and a truthful record of raw result, relay delivery, and observed client history.
- Expose native product tools first. Add rg and/or raw reading only where a documented required capability is absent, with no wrapper-generated extra search or answer enrichment.

## Scope
### In Scope
- Correct the new route transport path and Codex caller chain, using only the current execution contract.
- Preserve MCP content blocks, structured content, errors, annotations, instructions, and tool schemas where supported. Record unsupported client modalities explicitly instead of flattening them silently.
- Replace the six-name assumption with per-product native discovery and an explicit query-phase tool policy. Preparation mutations run outside solver access.
- Remove the artificial rg 250-line cap and host 32000-byte cap on the route path. Preserve CLI exit codes and raw stdout/stderr, including no-match semantics.
- Record initialization identity, actual tool schemas/defaults, response units, raw/delivered fingerprints, and unresolved client-delivery limits for metrics consumers.
### Out of Scope
- [hard] Increasing competitors' query budgets, forcing every tool to 100000, adding automatic pagination, compressing/enriching results, or patching product defaults.
- [hard] Solver/grader/calibration calls, benchmark execution, or using the three frozen question prompts for a trial.
- [hard] New test files/cases, broad changes to the Codex client, or enabling unrestricted shell/network tools for the solver.
- [deferred] General MCP conformance certification and support for unrelated transport types.

## Constraints
- This is one correctness unit: receiving a full response is insufficient if the relay, client configuration, or code-mode output stage silently cuts it later.
- Distinguish UTF-8 bytes, text characters, lines, and model tokens. codemap-search's effective 100000-byte default does not authorize 100000 tokens for every tool.
- Forward actual client identity and product-provided guidance faithfully. Do not impersonate unsupported capabilities or inject codemap-specific guidance into other products.
- Audit the route-specific `65536` override and code-mode budgets. Do not claim that deleting an override yields unlimited history. Preserve actual client limits, record where content becomes unobservable, and attribute client clipping separately from product detail.
- Product-native narrow responses remain narrow. Ordinary solver-chosen follow-up searches remain normal behavior, but the harness must not fetch extra content on the solver's behalf.
- Classify auxiliary necessity before any scored run, using native schemas/documentation and a neutral capability probe. Sparse output, empty results, lower coverage, response caps, or a README recommendation alone do not justify assistance. Record the missing operation, why native tools cannot perform it, the smallest auxiliary set, and the resulting assisted-arm label.
- 기준군 `rg`는 추가 검색 제품이 없는 기본 에이전트다. 전용 rg MCP 대신 Codex 기본 셸·파일 탐색을 제공하고 명령 선택은 LLM에 맡긴다. 출력·셸·code mode의 기본값을 유지하고 고정 rg를 명령 PATH에 둔다. 최소 실행 경로와 Grafana 원문만 읽을 수 있도록 권한을 제한한다. 제품군의 read 예외는 같은 원문 reader를 사용하며 zvec의 후속 rg 안내는 무시한다.
- Use the contract from child 01. Allow query operations inside the frozen source root; do not rely only on parameter names `path/file/file_path` when a product uses `root` or `projectPath`.

## Related Files / Entry Points
- `benchmark/route_transport.py`
- `benchmark/responses.py`
- `benchmark/runner.py`
- `benchmark/native_baseline.py`
- `benchmark/checks.py`
- `apps/codemap-search/docs/configuration.md`
- `docs/briefs/evidence/bench-ready/02-delivery.json`

## Execution Plan
### Stage 1 — Pin the loss points without model calls
- Starts when: `docs/briefs/evidence/bench-ready/01-contract.json` validates the route identity and native-output policy.
- Work: Reproduce the host trim and rg trim using existing verification helpers or bounded local inspection. Trace response fields from product to relay, Codex input, code-mode printing, history, and final captured events. Determine which delivery stages are observable without invoking a model.
- No-op when: the active route path already preserves native response fields and rg output, forwards correct initialization, and records every additional client limit with offline evidence.
- No-op handoff: record the same capability and evidence fields in `docs/briefs/evidence/bench-ready/02-delivery.json` for children 04 and 06, including any unobserved real-model stage.
- Deliverable: a bounded failure record naming every cap, its units, owner, and affected fields, plus the route transport interface consumed by preparation.
- Verify: `Trace both over-limit response branches and compare original, relayed, and captured fields using existing offline helpers`; Inputs: `trim_result`, the rg branch, `codex_options`, and existing fake-Codex helpers; Expected: independently identified harness loss points and no claim that an unexecuted real-model path passed.
- Ends when:
  - [x] Both 32000-byte and 250-line limits have a confirmed owner and reproduction condition.
  - [x] Native server limits, relay limits, code-mode cell limits, and history limits are separate observations.
- Handoff: Stage 2 receives the pinned failures and transport interface.
- Replan when: correction requires changing a product's native behavior. Stop that correction, return to the parent, and retain the product behavior as the measured condition.

### Stage 2 — Implement faithful transport and client delivery
- Starts when: Stage 1 has isolated the loss points and the route contract defines tool-access policy.
- Work: Implement stdio/HTTP transport selection and proper MCP lifecycle. Preserve native discovery and server instructions, distinguish hidden mutation tools from missing capability, and pass native results without wrapper caps or field loss. Remove rg's wrapper head limit and any default output-size argument injection. Implement the default native-only surface and a per-arm minimal auxiliary exception record. Keep the optional reader contract identical across assisted arms and its explicit range semantics separately recorded.
- Deliverable: native route transport, route-specific Codex configuration, complete raw/relay event envelopes, and explicit delivery-limit metadata.
- Verify: `Inspect native-to-relay field equality and the complete route Codex configuration using existing offline execution paths`; Inputs: representative text, structured content, error results, rg stdout, and current codemap tool discovery; Expected: no harness-imposed text/line cap, no silently discarded fields, no forced competitor budget, and exact tool/default metadata.
- Ends when:
  - [x] Product-native truncation survives unchanged and is never mistaken for wrapper truncation.
  - [x] HTTP session and stdio process cleanup have explicit ownership and bounded timeouts.
  - [x] Unknown client delivery remains unknown rather than reported as complete.
- Handoff: Stage 3 receives the corrected transport and observable delivery envelope.
- Replan when: the installed client cannot deliver a supported native result faithfully. Stop readiness for that arm, retain raw evidence, and specify the smallest client-compatible correction without altering product output defaults.

### Stage 3 — Verify and publish the delivery contract
- Starts when: Stage 2 has implemented the route path and removed retired dispatch.
- Work: Run relevant existing harness checks, inspect actual generated Codex options and relay payloads, and publish capability/evidence records. Product startup smoke results will be supplied later by child 04; do not fabricate them now.
- Deliverable: `docs/briefs/evidence/bench-ready/02-delivery.json` with `schema_version`, `status`, `contract_hash`, `transport_interfaces`, `native_output_policy`, `auxiliary_exception_contract`, `client_identity`, `client_limits`, `raw_relay_checks`, `unobserved_stages`, and verification commands/results.
- Verify: `python3 -m unittest benchmark.checks benchmark.cui_checks`; Inputs: existing suites from the repository root plus bounded inspection of the new route payloads; Expected: existing checks pass, route payloads preserve native data, and no solver or grader invocation occurs.
- Ends when:
  - [x] The handoff distinguishes implementation verification from pending live product probes and real-model delivery.
- Handoff: children 04 and 06 consume `docs/briefs/evidence/bench-ready/02-delivery.json` and its transport interfaces.
- Replan when: an existing check fails or a route envelope loses data. Stop consumers, correct the failing route or compatibility boundary, and regenerate delivery evidence.

## Side Effect Checkpoints
- [x] Product tool descriptions and initialize instructions are not rewritten to favor one search strategy.
- [x] No unrestricted shell, web, subagent, memory, or plugin tool is reintroduced into solver access.
- [x] Source-root confinement covers each native root parameter without rewriting search semantics.
- [x] Optional auxiliary tools are absent by default, never replace native product responses, and never perform silent follow-up searches.

## Acceptance Criteria
- [x] Inspect the active route code path and verify zero harness byte/line truncation or injected output-size tuning; the scan population includes transport, baseline, runner options, and code-mode delivery.
- [x] Inspect raw and relay envelopes from offline verification and find all supported fields preserved with separately recorded client limitations.
- [x] Existing harness checks pass and the new route no longer assumes six codemap tool names or one stdio command shape.
- [x] `02-delivery.json` is sufficient for product startup probes and metric attribution without implying that real solver delivery has been measured.

## Open Questions
- None — the user approved product-native tools first and rg/read only for demonstrated functional necessity; preparation determines each arm's minimal exception set.
