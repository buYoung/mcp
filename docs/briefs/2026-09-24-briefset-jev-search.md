# Brief Set: Reduce Jev search context without losing evidence

## Purpose
- Restore the pre-Jev overview contract and make task-specific function judgments reduce the source delivered by search.
- Coordinate the approved registration break, bounded evaluation, output selection, and fresh coverage-preserving token comparison as independently verifiable work.

## Child Briefs
- [x] `docs/briefs/2026-09-24-fix-jev-search-01-overview.md` — Restore the pre-Jev overview contract; exists because exact historical navigation restoration has a separate oracle from new search behavior.
- [x] `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md` — Evaluate search functions with task questions; exists because required registration must reach real provider questions and final search decisions as one atomic contract.
- [x] `docs/briefs/2026-09-24-perf-jev-search-03-selection.md` — Select search evidence before rendering; exists because usable judgments alone do not determine how much source the main LLM receives.
- [ ] `docs/briefs/2026-09-24-perf-jev-search-04-measurement.md` — Verify token savings and retained coverage; exists because local payload reduction cannot establish whole-agent token success.

## Execution Order
- Wave 1 — `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`: Start: inspected checkout and pre-Jev baseline are available; Deliverable: exact overview restoration with a populated contract comparison; Location: `apps/codemap-search/validation/jev-search/01-overview.json` (proposed); Done: completed=true with successful existing codemap checks and zero unexplained baseline differences; Handoff: child 02 receives baseline restoration evidence and retained search boundaries.
- Wave 2 — `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`: Start: child 01 restoration evidence is complete; Deliverable: mandatory task-question registration reaching grouped function evaluation and the final search consumer; Location: `apps/codemap-search/validation/jev-search/02-task-judgments.json` (proposed); Done: completed=true with captured question propagation and successful existing MCP/Jev checks; Handoff: child 03 receives the schema and per-candidate decision contract.
- Wave 3 — `docs/briefs/2026-09-24-perf-jev-search-03-selection.md`: Start: child 02 task/evaluation contract is complete; Deliverable: pre-render selection with a matched payload and evidence comparison; Location: `apps/codemap-search/validation/jev-search/03-selection.json` (proposed); Done: completed=true with lower paired output and preserved required evidence; Handoff: child 04 receives frozen source identity and selection policy.
- Wave 4 — `docs/briefs/2026-09-24-perf-jev-search-04-measurement.md`: Start: child 03 matched comparison and source identity are complete; Deliverable: fresh four-group comparison and user-facing chat results; Location: `apps/codemap-search/validation/jev-search/04-measurement.json` (proposed); Done: accepted=true with lower mean main-model total tokens and non-regressing coverage; Handoff: parent receives reproducible evidence for global acceptance.

## Dependencies
- Predecessor: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`; Deliverable path: `apps/codemap-search/validation/jev-search/01-overview.json` (proposed); Format: JSON with completed, baseline_revision, source_revision, source_diff_hash, surfaces, checks, and remaining_differences; Successor: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Starts when: exact overview restoration is proven; Verify: `bounded inspection of the overview restoration handoff and its referenced comparisons`; Inputs: the handoff plus baseline metadata/output artifacts for every enumerated overview surface; Expected: completed=true, nonempty surface/check populations, zero unexplained differences, and zero overview Jev calls.
- Predecessor: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Deliverable path: `apps/codemap-search/validation/jev-search/02-task-judgments.json` (proposed); Format: JSON with completed, source_revision, source_diff_hash, schema, question_examples, composition, limits, consumer_trace, failure_behavior, and checks; Successor: `docs/briefs/2026-09-24-perf-jev-search-03-selection.md`; Starts when: required questions drive actual function judgments and read/grep remain original-source tools; Verify: `bounded inspection of the registered schema and captured request-to-output trace`; Inputs: the handoff and existing MCP/Jev verification records; Expected: completed=true, populated question/consumer mappings, successful checks, and zero Jev calls from non-search tools.
- Predecessor: `docs/briefs/2026-09-24-perf-jev-search-03-selection.md`; Deliverable path: `apps/codemap-search/validation/jev-search/03-selection.json` (proposed); Format: JSON with completed, source_revision, source_diff_hash, policy, baseline, paired_results, coverage, recovery, and checks; Successor: `docs/briefs/2026-09-24-perf-jev-search-04-measurement.md`; Starts when: final selection preserves required evidence and improves matched payload; Verify: `bounded inspection of paired request identities and delivered-source evidence`; Inputs: the handoff, original recorded requests, final output artifacts, and source hashes; Expected: completed=true, nonempty matched population, lower aggregate output, and preserved required evidence.

## Parallelization
- Must not overlap: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md` and `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md` — restore shared metadata/configuration before introducing the new task contract. Join when: child 01 evidence is complete and child 02 consumes that exact source state.
- Must not overlap: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md` and `docs/briefs/2026-09-24-perf-jev-search-03-selection.md` — selection follows the restored navigation and task contract. Join when: predecessor evidence is integrated and overview remains unchanged by selection.
- Must not overlap: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md` and `docs/briefs/2026-09-24-perf-jev-search-04-measurement.md` — timed runs require a frozen restored implementation. Join when: restoration and all later source changes are frozen before measurement.
- Must not overlap: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md` and `docs/briefs/2026-09-24-perf-jev-search-03-selection.md` — stabilize the judgment contract before selection consumes it. Join when: child 02 is complete and child 03 preserves its schema/failure boundaries.
- Must not overlap: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md` and `docs/briefs/2026-09-24-perf-jev-search-04-measurement.md` — runtime/schema edits invalidate timed comparisons. Join when: final task/runtime source is frozen before measurement.
- Must not overlap: `docs/briefs/2026-09-24-perf-jev-search-03-selection.md` and `docs/briefs/2026-09-24-perf-jev-search-04-measurement.md` — complete paired selection verification before whole-agent measurement. Join when: child 03 handoff identifies the final frozen policy and source.

## Conflict Hotspots
- `apps/codemap-search/src/tools/mod.rs` — Children: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`, `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Access: serialized; Owner: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`; Rule: restore overview metadata first, then add required search task questions without changing that restoration.
- `apps/codemap-search/src/mcp/mod.rs` — Children: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`, `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Access: serialized; Owner: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`; Rule: restore overview dispatch before changing task storage and local read/grep dispatch.
- `apps/codemap-search/src/mcp/jev.rs` — Children: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`, `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Access: serialized; Owner: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`; Rule: remove overview evaluation first and retain shared transport for the successor.
- `apps/codemap-search/src/config/jev.rs` — Children: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`, `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Access: serialized; Owner: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`; Rule: retire overview activation before retiring live-tool activation.
- `apps/codemap-search/src/tools/search/jev.rs` — Children: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`, `docs/briefs/2026-09-24-perf-jev-search-03-selection.md`; Access: serialized; Owner: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Rule: establish typed question answers before optimizing retention and selection.
- `apps/codemap-search/src/tools/search/mod.rs` — Children: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`, `docs/briefs/2026-09-24-perf-jev-search-03-selection.md`; Access: serialized; Owner: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Rule: wire the registered task end to end before moving selection relative to rendering.
- `apps/codemap-search/src/mcp/jev.rs` — Children: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`, `docs/briefs/2026-09-24-perf-jev-search-03-selection.md`; Access: serialized; Owner: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Rule: preserve the established failure/deadline contract while adding selection measurements.
- `apps/codemap-search/src/tools/mod.rs` — Children: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`, `docs/briefs/2026-09-24-perf-jev-search-03-selection.md`; Access: serialized; Owner: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Rule: selection guidance must build on the completed registration schema.
- `apps/codemap-search/docs/configuration.md` — Children: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`, `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Access: serialized; Owner: `docs/briefs/2026-09-24-fix-jev-search-01-overview.md`; Rule: apply overview removal before task/search-only documentation and keep the Korean counterpart synchronized.
- `apps/codemap-search/docs/configuration.md` — Children: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`, `docs/briefs/2026-09-24-perf-jev-search-03-selection.md`; Access: serialized; Owner: `docs/briefs/2026-09-24-feat-jev-search-02-task-judgments.md`; Rule: apply final selection/threshold explanations only after registration and runtime documentation is current.

## Shared Constraints
- Treat `apps/codemap-search/src/config.rs`, `src/config/layout.rs`, `src/config_template.toml`, `src/config_template.ko.toml`, `README.md`, and `README.ko.md` within the package as serialized shared surfaces: child 01 removes overview settings/examples before child 02 replaces task/live-tool settings/examples. Child 03 must preserve both completed contracts when adjusting selection policy.
- Authoring snapshot: `8b8222003584a3875ce3804557d3d5e70113ea4b` on `docs/jev-integrate`. Reinspect relevant diffs before implementation and preserve unrelated untracked `2026-09-24-*codemap-prod*` briefs.
- Overview restoration is exact against `97e3ebc8e3708df01062fdd3e90b9ac68f7b08f3`. Disabling the flag, hiding the recommendation section, or reverting whole integration commits does not satisfy it.
- The user approved mandatory task-question lists. With Jev search enabled, reject legacy text-only registration explicitly; with Jev disabled, preserve ordinary initialization without registration.
- The main LLM derives the task goal and focused questions from the user's request once per task. Do not add a separate generation service, hardcode the benchmark, or confuse retrieval `query` with task intent.
- Preserve the reference input verbatim in measurement: `codemap-search를 활용해서 이벤트버스 패턴 중 measure worker와 통신하는 구간 다 확인해줘.` Its two judgments cover event-flow participation and connection of that same flow to measure-worker input/output.
- Jev applies to search. Overview, read, and grep stay local original-evidence tools, including when stale removed enable flags are present.
- Preserve `workspace_scope`, exact `event_key` behavior, supported aliases/views, source locations, output ceilings, source observations, masking, permissions, and explicit incomplete/stale notices.
- Preserve shared transport limits, caller cancellation, absolute deadlines, connection pooling, provider response validation, and whole-search fallback on evaluation failure.
- Use focused Noul questions and explicit all/any composition. Keep uncertain evidence visible and do not claim mathematically calibrated joint probability from combining separate answers.
- Keep candidate bodies once per bounded group state, not once per question or the entire catalogue once per batch. Record batch/criterion identities for reproducible interpretation.
- Current documentation, schemas, generated English/Korean templates, and retained examples must agree after each owning child. Do not rewrite historical validation reports.
- Use existing tests and fixtures with necessary contract-input/expectation adjustments only. Add no test files/cases, lint/formatter setup, or new dependencies.
- Follow the package's existing cargo check and affected verification routes; announce exact commands/workdir before execution. Do not require an unscoped full-suite run for every child.
- Keep evidence handoffs at the exact repo-relative JSON paths declared above. Include source HEAD plus diff/source hashes, since implementation may remain uncommitted.
- Actual raw benchmark logs belong in a new external checkpoint run directory. Report result tables in conversation and leave `CHECKPOINT.md` and all historical raw runs unchanged.
- User-approved acceptance is lower three-run mean main-model total tokens than a fresh frozen `#13` comparison, with no additional omissions under the unchanged rubric. Do not invent a minimum percentage or substitute Jev-inclusive totals for that metric.
- Each benchmark group has three concurrent sessions; groups run sequentially. rg uses native output defaults, and codemap variants use the same recorded 32,000 output policy.
- Prove active Jev exercise in each improved-on run using registered questions, successful provider evaluation, and consumed selection decisions. Keep unexercised/bypass-only outcomes and report the active-Jev comparison as inconclusive rather than silently accepting or replacing them.
- Report frozen-#13 to improved-on, frozen-#13 to improved-off, and improved-off to improved-on deltas separately. Keep the user-approved overall target unchanged; the incremental Jev comparison explains attribution and adds no new percentage threshold.
- API credentials stay in the authorized environment/secure input path and never appear in saved artifacts. Do not commit, publish, or deploy as part of these children.
- [deferred] Persistent judgment caching, adaptive concurrency, new retrieval engines, nested task-expression languages, broader benchmark tasks, and production release work.
- BDR ownership: child 02 stays together under K1/K2 because task registration, invocation, and failure handling form one usable contract. Child 03 keeps selection/retention/render accounting together under K2. Other children have distinct historical-restoration and whole-agent-measurement oracles.

## Global Acceptance Criteria
- [ ] Child 01's nonempty restoration matrix proves every overview surface matches the pinned pre-Jev contract with zero unexplained differences and zero Jev overview calls.
- [ ] Captured requests and final search output prove that mandatory registered questions reach evaluation and selection without silently falling back to the original generic question.
- [ ] Read and grep can retrieve original source without disabling configuration, registering Jev questions, or using an API key.
- [ ] Child 03's paired evidence proves lower output on identical candidates without losing the recorded required evidence or hiding uncertain candidates.
- [ ] Child 04 accounts for four groups of three original outcomes with correct within-group concurrency, frozen identities, and restored environment state.
- [ ] All three improved-on runs prove successful Jev evaluation consumed by search selection; an enabled flag or attempted request alone cannot satisfy this check.
- [ ] Fresh improved search has lower mean main-model total tokens than fresh frozen `#13`, covers all six core criteria in all runs, and does not regress any of the eleven criteria's covered-run counts.
- [ ] New source-contradicting claims, original-source recovery, truncations, provider failures, cached/uncached tokens, wall time, and Jev cost are explicitly reported rather than concealed by one aggregate.
- [ ] The report separates overall improvement from the observed incremental Jev effect and does not attribute all savings against frozen #13 to Jev.
- [ ] The user receives the final comparison in conversation and all evidence pointers resolve; historical checkpoint documents and unrelated work remain unchanged.
- [ ] Any failed proof has returned to its owning child for bounded correction and re-verification, with parent dependencies and affected comparisons refreshed before completion.

## Open Questions
- None — the user confirmed mandatory question lists and coverage-preserving mean-token reduction; exact overview restoration is pinned from Git history.

## 실행 상태 — 2026-09-24

- 01–03 구현과 기존 검증을 완료했다. 현재 03 근거 버전은 `search-task-evidence/5`이며 관련 기존 검증 109개와 릴리스 빌드가 통과했다.
- 첫 12개 비교는 토큰 증가와 `manual_extra` 포함 횟수 감소로 불합격했다. 원본 결과와 세션은 `20260924-jev-search-implementation` 외부 기록 디렉터리에 보존했다.
- 03에서 본문이 표시되지 않은 호출자의 색인 근거를 보완했다. 같은 이전 질문·검색 3건에서 재조회 대상 함수가 불확실 상태로 보존됨을 확인했다. 비활성 원문 응답 312건의 일치, 고정 점수 재생의 감소·근거 보존과 원문 복구도 재확인했다.
- 사용자가 측정 범위를 정정했다. `rg`·`#11` 및 확보된 비교 결과를 유지하고 보정판 Jev 활성 1개 그룹(3개 동시 세션)만 추가했다. 불필요하게 시작한 추가 rg 3개는 중단했고 비교에 쓰지 않았다. 이후 다른 기준선이나 추가 Jev 그룹은 실행하지 않았다.
- 추가 Jev 평균은 총 964,410토큰이다. 유지된 #11.1 대비 +1.813%, #13 대비 +21.538%이며, `manual_extra`도 #13의 1/3에서 0/3으로 감소했다. 핵심 6개는 모두 3/3이다. 04는 측정 종료·불합격(`completed=false`, `accepted=false`)으로 남긴다.
- 마지막 측정의 질문 등록·실제 평가·최종 선별 소비와 토큰 산술을 확인했다. 설정을 복원했고 남은 소유 프로세스는 없다. 사용자 지시 없이 추가 모델 측정을 진행하지 않는다.
- 최종 수치·세션·제한·중단 기록: `apps/codemap-search/validation/jev-search/04-measurement.json`. 상세 보고서는 해당 handoff의 `report_path`를 따른다.
