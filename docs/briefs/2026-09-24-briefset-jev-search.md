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


## 2026-09-24 추가 개선과 단일 실측

- 사용자 지시에 따라 기존 상태를 `c813bec9e`에 먼저 커밋했다. 커밋 훅이 요구한 서식과 단일 원소 반복문만 정리한 뒤 새 개선을 적용했다.
- ID 자동 생성, 등록 전체 64 KiB, 28k/56k 추정 토큰 예산, 후보별 근거 배분과 직접 호출자 원문 공유, 반복 안내 축소를 적용했다. 기존 임계값과 LLM의 탐색 자율성은 유지했다.
- 사용자가 #2만 1회 평가하도록 변경했다. rg·#11·#13·off·#1·#1+#2는 재측정하지 않았다. 새 세션은 정확히 1개다.
- 관측값: 메인 584,554토큰, Jev 91,118토큰, 176.952초. 등록은 ID 없이 1회 성공했고 도구 오류·Jev fallback은 0회다.
- 기존 검증 115개와 Clippy·릴리스 빌드가 통과했다. 대상 소스는 변경되지 않았고 설정은 복원됐다.
- 핵심 6/6, 확장 1/5다. ServiceCandidate·custom 우회·MIO 우선 분기·manual extra 계약 누락이 남았다. 단일 실행의 낮은 토큰 수를 안정적인 성능 개선이나 전체 품질 무저하의 증명으로 취급하지 않는다. 기존 04 수용 조건은 완료로 표시하지 않는다.
- 상세 근거: `apps/codemap-search/validation/jev-search/05-context-budget.json` 및 /Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/20260924-jev-search-context-02-single/data/report.md
- 추가 모델 실측은 실행하지 않는다.

## 2026-09-24 품질 개선과 단일 실측

- 사용자 승인 후 직접 피호출자 근거 공유, 흐름 조건·메시지 계약·우회 경로의 관련성 안내, 후속 호출 후보, 최종 설명의 누락 점검 안내를 추가했다. 평가 대상 이름을 제품 코드에 넣거나 LLM의 탐색 순서를 고정하지 않았다.
- #2만 새 세션 1개를 실행했고 rg·#11 및 모든 이전 비교 결과는 유지했다. 핵심 6/6, 확장 4/5로 ServiceCandidate·custom 우회·MIO 오류 처리가 추가됐다. manual extra는 생산자 미전달만 설명해 조건부 이력 저장과의 연결은 미충족이다.
- 메인 1,069,306토큰, Jev 100,693토큰, 313.483초다. 직전 단일 관측 대비 메인 토큰이 82.93% 증가했으므로 품질과 토큰 절감을 함께 달성했다고 수용하지 않는다. 등록·도구 오류·Jev fallback·출력 잘림은 0회다.
- 측정 후 후속 후보의 시작점을 긍정적인 작업 일치 판정으로 좁혔다. 판단 불가·불확실성 때문에 보존한 선언은 확장하지 않는다. 이 마지막 수정의 모델 효과는 재측정하지 않았다. 측정 소스와 최종 소스의 해시를 별도로 남긴다.
- 측정 전 기존 검증 115개가 통과했고 마지막 수정 후 기존 Jev 통합 검증 19개와 컴파일·Clippy를 재확인했다. 새 테스트 사례는 추가하지 않았다. 대상 설정과 소스는 복원·보존됐고 추가 모델 실행은 하지 않는다.
- 상세 근거: `apps/codemap-search/validation/jev-search/06-quality.json` 및 /Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/20260924-jev-search-quality-03-single/data/report.md

## 2026-09-24 현재 수정본 3회 병렬 측정

- 사용자가 현재 상태로 3회 병렬 측정하도록 추가 승인했다. 긍정적인 작업 일치 함수에서만 후속 후보를 생성하는 최종 수정본을 그대로 사용했다. 소스 313개와 릴리스 바이너리 해시를 확인했고 제품 코드 변경이나 프로젝트 검증 재실행은 없었다.
- #2 세션 3개를 최대 0.008737초 간격으로 시작했다. 세 프로세스가 모두 실행 중인 구간은 265.740초다. 기존 rg·#11·#13 및 단일 실측은 재실행하지 않았고 대체 세션도 없다.
- 메인 토큰은 1,304,883 / 1,553,773 / 1,417,622, 평균 1,425,426이다. Jev 토큰은 93,148 / 192,027 / 149,991이다. 세 실행 모두 메인 100만 토큰을 넘었으며 유지된 비교 결과보다 평균이 높다.
- 핵심은 모두 6/6, 확장은 5/5·5/5·4/5다. manual_extra만 3번에서 부분 설명에 머물렀다. 모든 항목의 일반적인 정답률이나 안정적인 인과 효과로 해석하지 않는다.
- 1번·3번에서 존재하지 않는 `src/worker` 조회 오류가 각 1건 있었고 이후 정상 완료했다. 2번·3번의 첫 검색 출력은 32,000토큰 상한에서 각 1회 잘렸다. 세션을 제외하거나 교체하지 않았다. Jev 실제 판정 소비는 모두 확인됐고 fallback은 0회다.
- 대상 소스는 유지됐고 설정을 복원했으며 남은 소유 프로세스는 없다. 측정 후 제품 코드도 그대로 유지한다. 상세 근거: `apps/codemap-search/validation/jev-search/07-quality-parallel3.json` 및 /Users/buyong/.codex/checkpoints/codemap-search-comparison/runs/20260924-jev-search-quality-04-parallel3/data/report.md

## 2026-09-24 메인 토큰 세션 분석과 Jev 단독 검증

- 사용자가 이후 전체 실측을 다시 #2 단일 세션으로 제한했다. 이번에는 저장된 3개 세션을 분석하고 Jev만 실제 호출했다. 새 벤치마크용 메인 모델 실행과 기존 기준선 재측정은 0회다. 앞선 3회 병렬 측정 규정은 이후 실행에 적용하지 않는다.
- 메인 합계의 88.0~89.5%는 캐시 입력이다. 큰 문맥이 15~16회 요청에 누적되는 구조와, 본문 선별 뒤 남는 참조·안내를 분리했다. 2·3번에서는 로거 본문 754바이트에 비해 비호출 참조 301행만 29,890바이트였다. 서로 다른 LLM 탐색 경로 자체를 오류로 분류하지 않았다.
- 원래 Jev 요청 23개의 바이트·해시와 원래 검색 출력 3개를 모두 동일하게 재생했다. 본문 162개·중복 제거한 비호출 참조 301개를 실제 Jev로 각각 한 번 판정했다. HTTP 27회, Jev 445,488토큰이며 오류·사용량 미보고·자동 재시도는 없다.
- 본문·확인된 관계를 유지한 참조 선별·반복 안내 압축은 첫 검색 바이트를 3.30% / 26.00% / 24.60% 줄였다. 본문 판정 기준 강화는 +5.81% / -2.15% / -11.16%로 일관되지 않아 우선 적용하지 않는다. 메인 토큰 감소나 새 최종 답변 품질을 검증한 수치로 확대하지 않는다.
- 제품 소스 313개와 릴리스 바이너리·대상 소스는 동일하며 설정은 복원했다. 이번 단계는 분석 기록과 외부 실험 산출물만 추가했다. 04 수용 조건은 미완료로 유지한다.
- 상세 원인·결과·추천 순서·검증 한계: `apps/codemap-search/validation/jev-search/08-main-token-diagnosis.md`. 기계 판독 기록: `apps/codemap-search/validation/jev-search/08-main-token-diagnosis.json`.

## 2026-09-24 출력 개선과 단일 실측

- 사용자 승인에 따라 판정 뒤 검색 주석 압축, 표시 예산 연결, `grep`의 선택형 `source_grouped`, webhook 토큰 마스킹을 구현했다. Jev 질문·근거·임계값·선별 정책과 기존 원문 보기 계약은 유지했다. 이전 미커밋 변경도 보존했다.
- 같은 검색 3개를 저장된 Jev 답변으로 재생한 결과 검색 출력은 61,850 / 107,514 / 108,928바이트다. 기존에 첫 검색에 있던 채점 근거 행을 유지했고, 지연 본문은 명시적인 범위로 복구 가능했다. 원문 `grep` 64건의 위치·내용 3,231행은 동일하며 출력 바이트는 40.58% 감소했다. 이 단계의 메인·Jev API 호출은 0회다.
- #2 메인 세션은 정확히 1개다. 메인 960,703토큰, Jev 130,802토큰, 303.094초를 관측했다. 메인은 이전 3회 평균보다 32.60% 감소했지만 시간은 9.67% 증가했다. rg·#11·#13·off·이전 결과와 `CHECKPOINT.md`는 재측정·변경하지 않았다.
- 핵심 6/6, 확장 4/5다. 이번에는 ServiceCandidate 호출자 누락이 있다. `processPendingMeasurements` 호출자 검색을 `src/techAdmin` 안으로 제한해 외부 모듈 호출자를 확보하지 못했다. 출력 절단·표시 예산 본문 지연은 0이며, 형식 변경이 범위 선택을 유발한 인과는 미확인이다. 토큰 감소 전부를 품질을 보존한 효율 향상으로 해석하지 않는다.
- 기존 영향 범위 검증 154개가 통과했고 1개는 기존 환경 의존 제외다. 마지막 보호 변경 후 컴파일·Clippy·기존 Jev 단위 26개와 통합 10개·릴리스 빌드가 통과했다. 새 테스트 사례는 추가하지 않았다. 측정 소스 327개와 바이너리 해시가 유지됐고 설정은 복원됐다.
- 구현·허용된 실측·보고는 마쳤지만 품질 유지 수용과 기존 04 전체 수용 조건은 미완료다. 측정 후 제품 변경·추가 모델 실행·커밋은 하지 않았다. 상세 근거: `apps/codemap-search/validation/jev-search/09-output-single.md` 및 `09-output-single.json`.
