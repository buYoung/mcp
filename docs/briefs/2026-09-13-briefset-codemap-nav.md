# Brief Set: Focused code navigation and event relationships

## Purpose
- Deliver purpose-shaped code output and source-grounded relationship navigation for the supplied Corral workflow.
- Coordinate the live-tool contract, Rust relation correction, and new event index as separate executable units with one integrated acceptance result.

## Child Briefs
- [x] `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md` — Focused live-code controls; exists because read/grep requests need one compatible presentation and callable-expansion contract.
- [x] `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md` — Proven Rust links; exists because same-name suppression currently prevents a source-confirmed caller lookup.
- [x] `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md` — Event relationship map; exists because publication/subscription navigation needs dedicated indexed endpoint evidence.

## Execution Order
- Wave 1 — `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`: Start: select a coherent non-conflicting implementation baseline and preserve the supplied C1/C3/C4 inputs; Deliverable: final live presentation and callable-expansion contract with replay evidence; Location: `docs/briefs/evidence/codemap-nav/live-controls.json` (proposed); Done: the child criteria are evaluated and its report has outcome verified or already-satisfied with side_effects_clear=true; Handoff: the Rust and event children receive the exact option and rendering contract.
- Wave 2 — `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md`: Start: the live-controls report supplies a compatible verified contract; Deliverable: corrected Rust relation policy and the C2 evidence; Location: `docs/briefs/evidence/codemap-nav/rust-relations.json` (proposed); Done: the child criteria are evaluated and its report has outcome verified or already-satisfied with side_effects_clear=true; Handoff: the event child receives explicit-target and source-identity behavior.
- Wave 3 — `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md`: Start: both predecessor reports are available and their implementation changes are integrated; Deliverable: indexed event navigation and final matching/lifecycle evidence; Location: `docs/briefs/evidence/codemap-nav/event-map.json` (proposed); Done: the child criteria are evaluated and its report has outcome verified or already-satisfied with side_effects_clear=true; Handoff: whole-set acceptance consumes all three reports against the final candidate.

## Dependencies
- Predecessor: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`; Deliverable path: `docs/briefs/evidence/codemap-nav/live-controls.json` (proposed); Format: outcome, binary/source/config hashes, parameter schema, mode semantics, budget/precedence rules, replay evidence, check results, side_effects_clear; Successor: `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md`; Starts when: the report is successful and its contract is present in the implementation checkout; Verify: `Inspect the report and advertised tools/list schema`; Inputs: live-controls.json and the selected implementation binary; Expected: matching schema/defaults and successful legacy checks.
- Predecessor: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`; Deliverable path: `docs/briefs/evidence/codemap-nav/live-controls.json` (proposed); Format: outcome, parameter schema, mode semantics, budget/precedence rules, replay evidence, side_effects_clear; Successor: `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md`; Starts when: the report identifies the integrated presentation extension point; Verify: `Inspect source-only and relation-focused replay evidence`; Inputs: live-controls.json and its raw-output paths; Expected: source-only bypasses relation metadata and relation mode has a documented event-extension boundary.
- Predecessor: `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md`; Deliverable path: `docs/briefs/evidence/codemap-nav/rust-relations.json` (proposed); Format: outcome, target inputs, supported predicates, confidence policy, expected/actual C2 links, binary/source/config hashes, check results, side_effects_clear; Successor: `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md`; Starts when: the source-identity and target policy is integrated with the live presentation contract; Verify: `Inspect the C2 positive and negative relation results`; Inputs: rust-relations.json and its non-empty definition/edge populations; Expected: proven links survive suppression and ambiguous targets remain unlinked.

## Parallelization
- Must not overlap: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md` and `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md` — serialize presentation and caller-policy edits in Waves 1 then 2. Join when: the live contract is integrated and Rust relation output passes its compatibility checks.
- Must not overlap: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md` and `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md` — serialize shared MCP/schema/rendering/config changes in Waves 1 then 3. Join when: event rendering consumes the final presentation contract without changing omitted-option behavior.
- Must not overlap: `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md` and `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md` — serialize source-identity/target-policy integration in Waves 2 then 3. Join when: event evidence respects the integrated target/confidence policy and both child reports pass their checks.

## Conflict Hotspots
- `apps/codemap-search/src/callers/annotate.rs` — Children: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`, `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md`; Access: serialized; Owner: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`; Rule: establish unresolved presentation first, then correct suppression while preserving that contract.
- `apps/codemap-search/src/callers/resolution.rs` — Children: `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md`, `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md`; Access: serialized; Owner: `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md`; Rule: stabilize source/target identity before event-specific consumers extend it.
- `apps/codemap-search/src/mcp/mod.rs` — Children: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`, `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md`; Access: serialized; Owner: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`; Rule: add event rendering through the established dispatch/presentation contract.
- `apps/codemap-search/src/tools/mod.rs` — Children: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`, `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md`; Access: serialized; Owner: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`; Rule: merge advertised options without repurposing existing schemas or grep modes.
- `apps/codemap-search/src/tools/live_symbols/render.rs` — Children: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`, `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md`; Access: serialized; Owner: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`; Rule: retain mode and byte-budget behavior when caller confidence changes.
- `apps/codemap-search/src/tools/live_symbols/render.rs` — Children: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`, `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md`; Access: serialized; Owner: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`; Rule: keep event output separate and respect source-only suppression.
- `apps/codemap-search/src/config.rs` — Children: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`, `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md`; Access: serialized; Owner: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`; Rule: coordinate any presentation and target-setting schema additions against the current version and preserve existing values.
- `apps/codemap-search/src/config.rs` — Children: `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md`, `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md`; Access: serialized; Owner: `docs/briefs/2026-09-13-fix-codemap-nav-02-rust-links.md`; Rule: integrate event rule reload/migration after target-policy propagation is stable.
- `apps/codemap-search/docs/configuration.md` — Children: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`, `docs/briefs/2026-09-13-feat-codemap-nav-03-event-map.md`; Access: serialized; Owner: `docs/briefs/2026-09-13-feat-codemap-nav-01-live-controls.md`; Rule: publish one compatible option contract with its Korean companion and final event-rule examples.

## Shared Constraints
- Authoring this briefset does not execute implementation. When execution is requested, select and pin a coherent baseline without overwriting the in-flight macro/parser/public-validation changes observed on 2026-09-13.
- Execute the three waves serially. Each child owns its stages and final acceptance, while this parent owns ordering, shared-file integration, and whole-work completion; no subagent coordination is required.
- Keep schema and final-consumer changes atomic within each child. Keep event serialization, persistence, extraction-format handling, and snapshot propagation together so metadata cannot be dropped between stages.
- Preserve the current JSON-RPC text envelope, read-only annotations, filesystem permissions, directory/Git exclusions, test-context choices, original source locations, optional-argument aliases, and omitted-option behavior.
- Preserve `grep.output_mode`, existing line/context/pagination parameters, `search.caller_context`, and current byte/scan budgets. New presentation/expansion behavior is opt-in and must reach the final consumer.
- Record static provenance separately from runtime guarantees. Unknown receivers, targets, event keys, or instance identities receive no guessed links or false precise labels.
- Keep the original Corral checkout, configuration, index, event delivery, and window behavior unchanged. Run index-backed or mutating lifecycle verification in disposable checkouts/fixtures and record any root substitutions.
- The user approved actual event usage and explicit publish/subscribe examples first, extension through user rules, and new focused regression cases within the existing tests. This does not authorize a full language/framework catalog or new lint/formatter infrastructure.
- Use the supplied observations as input, not as independently measured baselines. Preserve all four payloads and distinguish code output, context output, characters, bytes, optional tokenizer-based tokens, and process/cache conditions.
- Do not claim general search ranking, index-refresh speed, or version-to-version gains from the earlier grep/read experience. Measure the changed behavior under matched conditions and report remaining limits.
- Coordinate configuration version/template/documentation changes against the version actually present when each wave starts; do not reset, regenerate, or silently widen user exclusion lists.
- Do not install, publish, stage, or commit as part of brief authoring. Future implementation delivery must follow the user's then-current requested scope and repository instructions.
- [deferred] General compiler/runtime dataflow, every event framework, unrelated macro/parser issues, and broad public-corpus requalification are outside this initiative.

## Global Acceptance Criteria
- [x] All three child checklists can be marked complete from their final addressable reports, including approved no-change proofs, after their stages and side-effect checkpoints finish.
- [x] On one final integrated candidate, C1/C3/C4 option variants and C2 relation probes preserve raw-source/legacy behavior while demonstrating the new focused output, exact callable boundaries, and source-proven Rust links.
- [x] Event publication/registration/handler output uses the same presentation and target-evidence contracts, remains distinct from direct calls, and passes the approved matching, non-matching, custom-rule, and lifecycle scenarios.
- [x] From `apps/codemap-search`, run `cargo check --locked` and the non-empty relevant suites named by each child on the final checkout, recording commands, source/binary identities, exit codes, and failure details. Expected: exit 0 for required checks, with pre-existing unrelated failures distinguished and not relabelled as passes.
- [x] Inspect `docs/briefs/evidence/codemap-nav/live-controls.json`, `docs/briefs/evidence/codemap-nav/rust-relations.json`, and `docs/briefs/evidence/codemap-nav/event-map.json` together. Expected: compatible contract identities, successful final integration checks, explicit remaining unsupported patterns, and no stale cross-generation links.
- [x] Negative controls prove that their intended source/endpoint populations were non-empty before reporting no wrong links; positive controls prove that matches were not removed merely to pass exclusions.
- [x] Inspect the final advertised schemas, English/Korean configuration docs, and command examples. Expected: matching defaults/options, explicit scope/uncertainty, and ready-to-run before/after requests without claiming an unmeasured speedup.
- [x] Review the execution diff and original Corral source/config/index identities. Expected: implementation changes stay in codemap-search and task evidence, with no changes to application behavior or unrelated in-flight work.

## Open Questions
- None — the user selected both Stage 4 recommendations: actual usage plus explicit examples with user-rule extensibility, and focused regression tests. Remaining technical uncertainties have bounded investigation and replan owners in the children.


## 실행 완료 — 2026-09-14

기존 매크로·개발 언어 검증 작업을 완료하고 `eafa0d4c8`로 커밋한 뒤 이 브리프를 열어 실행했다. 세 Wave는 지시한 순서대로 구현했으며, 최종 검증만 서로 독립적인 작업공간에서 병행했다.

[통합 결과](evidence/codemap-nav/integration.json)의 고정 release 바이너리로 C1~C4와 이벤트 요청을 재실행했다. 각 자식의 초기 인계 식별자는 보존했고, `final_integration`에서 최종 후보와 호환성을 확인할 수 있다. 기본 원문, 새로운 출력 모드, 정확한 함수 범위, 미해결 개수, Rust 연결 6개의 양방향 확인, 이벤트의 정방향·역방향·부정 사례·갱신을 통과했다. 패키지 검증은 같은 최종 소스의 debug 테스트이고, 실제 프로토콜 재실행과 설치 검증은 고정 release 바이너리임을 구분해 기록했다.

설치 SHA-256: `3fe45db1420fb8df2e6a64872d18842e5b9672f7513246fd65e630fe1de2b9ef`.

원본 Corral 보존은 소스·활성 설정과 통제된 최종 실행 전후 비교에서 확인했다. 최초 기록과의 설정 주석·이전 v27 색인 차이는 [관측 기록](evidence/codemap-nav/original-corral-drift.json)으로 분리했으며 실행 주체는 미확인이다. 이를 대화 전체의 색인 불변으로 표시하지 않는다. 이전 공개 저장소 검증은 해당 커밋·바이너리의 역사적 결과이고, 이번 브리프에서 전체 공개 코퍼스를 다시 검증했다고 주장하지 않는다.

바로 실행할 수 있는 명령은 [품질 확인 명령](../../apps/codemap-search/docs/development-language-commands.ko.md)에 정리했다. 이미 연결된 MCP 클라이언트는 새 옵션을 받으려면 서버 연결을 다시 시작해야 한다. `cm`은 설치본으로 별도 확인했다.
