# [feat] Grafana 소스와 제품별 초기 상태 준비

> 2026-10-03 완료. 사용자 요청으로 이전 벤치마크를 제거하고 현재 1.0.0 준비 구성에 맞춰 갱신했다. [검증 근거](evidence/bench-ready/04-indexes.json)를 기록했으며 실제 풀이·채점·채점 보정은 실행하지 않았다.

## Work Type
feat

## Current State (As-Is)
- [confirmed] `benchmark/tool_indexes.py`가 22,898개 추적 파일과 고정 근거 파일·발췌를 대조하고 제품별 색인·홈·복원용 상태를 분리한다.
- [confirmed] 색인을 사용하는 제품은 5개이며 기본 에이전트 기준군에는 색인이나 검색 MCP가 없다. 기준군은 기본 셸의 원문 읽기와 정답 접근·쓰기 차단을 확인한다.
- [confirmed] 조회 근거는 코드·명령·환경·권한·Codex 버전과 조회 후 노출 도구·정리·복원 기록의 해시에 결합한다. 현재 상태와 일치하지 않는 조회를 재인증하지 않는다.

## Desired Outcome (To-Be)
- Prepare the same full Grafana commit for all arms with verified tracked source content and separate product-generated state.
- Build each product's native local index, verify readiness through non-LLM probes, and define repeatable per-question source/cache/daemon reset behavior.
- Preserve native inclusion/exclusion behavior and report coverage limitations without tuning a product against the frozen answer key.

## Scope
### In Scope
- Full source materialization, source-manifest validation, generated-source availability, per-product index preparation, model-cache ownership, and query server lifecycle.
- Preparation readiness and environment evidence for all six arms, including the unindexed rg baseline.
- Define clean per-question launch state and cache semantics; record index preparation costs separately from future solver measurements.
- Exercise initialization, tool discovery, one neutral search/read capability probe, and cleanup with no solver/grader process.
### Out of Scope
- [hard] Running the three frozen questions, evaluating search quality, invoking solver/grader/calibration, or generating benchmark results.
- [hard] Changing product source, native output defaults, tool search quality, or corpus content to compensate for a competitor's omissions.
- [hard] New test files/cases, deleting personal caches, killing unrelated daemons, or using privileged account changes.
- [deferred] Cold-versus-warm performance trials, incremental-index benchmarks, and external-AI comparison conditions.

## Constraints
- Use Grafana commit `c6fad8695a96577eb466d425e6ac4a759ca30f47` and the entire tracked source population. Keep frozen evidence, grading files, and benchmark result directories outside the searchable source root.
- Distinguish shared corpus availability from native index coverage. Generated TypeScript and JSON remain readable even when a product's native indexing rules exclude them; record that limitation without forcing inclusion.
- Use valid Git metadata only where required by a pinned product, with the same source identity across arms. An empty `.git` directory must not masquerade as a valid repository.
- Keep each arm's source copy and generated files separate. Build the source manifest from the fixed tracked inventory rather than broadly ignoring names that might hide legitimate source files.
- The user-selected local-only mode uses Jev disabled, fresh Graphify code-only output, and zvec's documented local code embedding. This concerns provider access, not response budgets.
- Local embedding inference required for indexing and neutral search probes is allowed preparation. It is distinct from the prohibited solver, grader, calibration, and external generative-model calls.
- Native tools are the default query surface. Demonstrate any unavoidable missing operation with a neutral probe before enabling the minimum rg/read exception. A product's sparse response, native cap, or weak retrieval remains its own limitation and cannot trigger an exception.
- Implement route-specific source manifest/copy behavior in resources/tool_indexes. Treat runner.py as read-only in this child so child 06 can own normalization concurrently.
- Serialize query daemons when their ownership rules demand it. Track process IDs, ports, cache roots, source roots, and shutdown confirmation. Never stop an unrelated user's process to make a check pass.
- 중립 조회 근거에 실제 어댑터 코드·명령·환경·권한·계약·Codex 버전을 결합한다. `ready.json` 존재만으로 재사용하지 않으며 변경 시 기준 상태 복원 후 다시 조회한다. 최종 게시와 준비 검증에서도 동일한 결합을 확인한다. 기준군은 색인/MCP 없이 기본 셸의 원문 접근·정답 차단·쓰기 차단을 모델 호출 없이 점검한다.

## Related Files / Entry Points
- `benchmark/tool_indexes.py`
- `benchmark/tool_artifacts.py`
- `benchmark/runner.py`
- `benchmark/native_baseline.py`
- `docs/briefs/evidence/bench-ready/04-indexes.json`

## Execution Plan
### Stage 1 — Bind full source and product lifecycle contracts
- Starts when: `docs/briefs/evidence/bench-ready/02-delivery.json` supplies route transport interfaces and `docs/briefs/evidence/bench-ready/03-artifacts.json` supplies six verified launch artifacts.
- Work: Materialize the fixed source, verify its tracked manifest, and enumerate product-created paths separately. Specify readiness, query launch, reset, and shutdown for each tool from its pinned documentation. Check whether real Git metadata is required before choosing copy/worktree/export strategy.
- No-op when: six matching fresh preparation states already exist with full source hashes, exact artifact hashes, non-question probe evidence, and verified cleanup/reset behavior.
- No-op handoff: publish those verified states through `docs/briefs/evidence/bench-ready/04-indexes.json` for child 07, with no reused solver answer or query history.
- Deliverable: full-source manifest and six lifecycle recipes bound to artifact and delivery-contract hashes.
- Verify: `Compare all tracked source hashes and inspect the required generated TypeScript and TestData JSON paths in every arm`; Inputs: fixed Grafana source inventory, six source roots, and frozen evidence paths; Expected: identical tracked content, zero answer-key files inside source roots, and product state kept distinguishable.
- Ends when:
  - [x] Source identity is independent of index output, process caches, and path relocation.
  - [x] Each lifecycle recipe names its owning processes, cache paths, readiness signal, reset condition, and cleanup proof.
- Handoff: Stage 2 receives the source manifest and native lifecycle recipes.
- Replan when: a product requires a different corpus or mutation of tracked source to initialize. Stop its readiness path and return the precise incompatibility to the parent without weakening the fixed-source contract.

### Stage 2 — Prepare native indexes with isolated state
- Starts when: Stage 1 recipes and source identity checks are complete.
- Work: Prepare each arm using its verified native lifecycle recipe and keep its state isolated.
  - codemap-search: use isolated CODEMAP_HOME and its native index command, with Jev disabled.
  - CodeGraph: prepare the locked project graph and record direct/shared-daemon mode, native backend, watch behavior, and update/telemetry behavior affecting instructions.
  - zvec-grep: prepare the selected local model/index under isolated ZVEC_GREP_HOME, use its documented ready check, and retain the default search-only MCP toolset.
  - Graphify: use a new empty output directory with code-only extraction, then point serving at that exact graph.json.
  - codebase-memory-mcp: follow canonical cache/daemon rules with explicit indexing and record actual auto-watch configuration.
  - rg: prepare source-only state without an index.
- Deliverable: six prepared states with index hashes where applicable, source and artifact bindings, inclusion/exclusion summaries, model hashes, startup commands, and preparation logs.
- Verify: `Inspect each native index/status artifact and compare it with the six lifecycle recipes`; Inputs: product state directories, native readiness output, source manifest, and artifact hashes; Expected: ready indexes for five indexed arms, no index required for rg, and no inherited semantic layer or foreign query state.
- Ends when:
  - [x] Graphify's fresh output cannot retain a prior semantic extraction layer.
  - [x] zvec's model revision/cache and HTTP port ownership are explicit.
  - [x] codebase-memory-mcp's repository artifact and shared daemon ownership are accounted for without claiming undocumented semantic modes.
  - [x] Native omissions remain documented limitations rather than triggers for answer-driven tuning.
- Handoff: Stage 3 receives prepared states and exact launch/reset recipes.
- Replan when: indexing fails, never reaches readiness, or conflicts with a personal daemon. Stop that arm, preserve logs, and perform only bounded task-owned correction before re-verifying it. Do not mark partial indexes ready.

### Stage 3 — Probe transport readiness and publish reset evidence
- Starts when: all prepared states have native readiness signals and matching identities.
- Work: Initialize each product through child 02's transport, inspect tools and instructions, perform a neutral capability search/read unrelated to the frozen questions, and verify shutdown. Classify product arms as native-only or necessarily assisted using the recorded functional gate; classify the no-product coding-agent baseline separately as native_agent_baseline and inspect its native shell permission boundary. Add only the required rg/read capability when native operations cannot perform a required step, and record the evidence without testing gold-question quality. Confirm source relocation and fresh per-question state retain index validity or rebuild when the native format requires it. Record cache reuse versus reset explicitly and remove probe query state before declaring the baseline prepared.
- Deliverable: `docs/briefs/evidence/bench-ready/04-indexes.json` with `schema_version`, `status`, `input_hashes`, `source_manifest`, and six `prepared_states` containing index/model hashes, effective modes/defaults, native/assisted condition, auxiliary necessity evidence, query tool surface, process/cache ownership, reset recipe, readiness probes, cleanup evidence, and limitations.
- Verify: `Inspect six startup/discovery/probe/shutdown transcripts and recheck tracked source hashes after cleanup`; Inputs: six prepared states and transport transcripts; Expected: every arm is locally usable, owned processes are cleaned up, source hashes are unchanged, and solver/grader invocation counts are zero.
- Ends when:
  - [x] Every arm has a query-ready recipe and an explicit fresh-state policy for the future one-attempt schedule.
  - [x] Native tool defaults and actual initialization instructions are recorded without output-budget overrides.
- Handoff: child 07 consumes `docs/briefs/evidence/bench-ready/04-indexes.json` for the final preparation gate.
- Replan when: a probe reveals changed tool schemas, stale indexes, response loss, or cleanup leaks. Stop the final gate and route corrections to child 02, 03, or this child according to ownership, then regenerate all affected hashes.

## Side Effect Checkpoints
- [x] All source copies retain generated TypeScript, JSON, and original ignore/config files.
- [x] Product-generated paths cannot silently contaminate source identities or become shared between arms.
- [x] Closing an MCP frontend is not accepted as proof that a shared daemon stopped.
- [x] Probe content and query history are cleared or explicitly excluded from the future prepared baseline.
- [x] Index build duration and artifact size remain preparation evidence, not solver performance claims.

## Acceptance Criteria
- [x] `04-indexes.json` provides verified readiness and reset evidence for six arms on the same source commit.
- [x] Full tracked-source hash comparison passes before and after all preparation probes.
- [x] No required source evidence is removed by the harness, and each product's own coverage limitations are visible.
- [x] Every daemon/cache has a task owner and an observed cleanup result or an explicit blocker; blockers cannot coexist with a ready status.
- [x] 기준군의 native_agent_baseline과 제품군의 native_only/assisted를 명시하며 품질에 따른 자동 보조 도구 추가는 없다.
- [x] No frozen question, solver, grader, or calibration has executed.

## Open Questions
- None — local-only preparation and product-native tools with unavoidable minimal auxiliary exceptions were decided by the user.
