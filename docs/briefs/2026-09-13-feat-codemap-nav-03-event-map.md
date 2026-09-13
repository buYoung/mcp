# [feat] Index event publication and subscription links

## Work Type
feat

## Current State (As-Is)
- [confirmed] The author inspected the 2026-09-13 working tree at HEAD `16192bc8a6f44a89c9a5863232ebcce849b9f2cb` plus uncommitted changes. Refresh the actual implementation baseline rather than assuming the commit contains all inspected code.
- [confirmed] `PublishedIndexSnapshot` stores immutable codemap and static collection records, indexed by file and collection identity, after a successful indexing pass. Evidence: `apps/codemap-search/src/index/indexer.rs`, `PublishedIndexSnapshot` and `from_files_and_edges()`.
- [confirmed] `StaticCollectionEdge` represents producer/consumer endpoints with owning type, field, source context, value, and range. Evidence: `apps/codemap-search/src/parser/types.rs`; this is a collection-navigation hint, not an event-delivery contract.
- [confirmed] The TypeScript collection query recognizes direct member `push`, iteration, return, and indexed reads. Evidence: `apps/codemap-search/queries/typescript/static_collection_edges.scm`; it does not itself model `on`/`emit` publication and subscription.
- [confirmed] The search renderer requires source/type evidence before emitting `Related write/read paths (static hint)`. Evidence: `apps/codemap-search/src/tools/search/render.rs`, `has_static_owner_connection()` and `render_static_collection_edges()`; type identity alone is not proof of one runtime bus instance.
- [confirmed] The user approved an event map built during indexing, matched by bus identity and event key, with publication, subscription-registration, and handler-definition locations. The user selected actual usage plus explicit publish/subscribe examples first, extensible through user rules, and approved focused regression tests.
- [inferred] The existing snapshot publication model can carry a separate event index without scanning the full workspace on each query. Confirm parser-to-storage-to-refresh propagation and request-time lookup cost in Stage 1 before choosing storage details.
- [inferred] Corral may require framework-specific or cross-language event rules beyond the illustrative `bus.on("saved", handleSaved)` and `bus.emit("saved")` pair. A literal-name probe did not establish its bus API; Stage 1 must inventory actual registration/publication code before claiming an adapter is supported.

## Desired Outcome (To-Be)
- Query an indexed event key and inspect its bus identity, publishers, subscription registrations, and handler definitions by exact original file and line.
- Match endpoints only when the bus identity and event-key evidence support the relationship; retain explicit reasons for unresolved endpoints.
- Show event relations separately from direct calls and distinguish static registration evidence from guaranteed runtime handler execution.
- Refresh, delete, and reload event records with the same coherent generation as the associated source/symbol snapshot.
- Support the first proven usage patterns with an extensible, user-configurable API rule mechanism rather than hardcoded matching of arbitrary method names.

## Scope
### In Scope
- Build a language-neutral event endpoint and provenance model with per-file records and indexes keyed by proven bus identity plus event key.
- Inventory Corral's actual event mechanisms and the explicit TypeScript/JavaScript `on`/`emit` example, then implement their statically provable patterns. Treat an absent mechanism as an evidenced absence, not as a reason to fabricate an adapter.
- Recognize publication, subscription registration, and resolvable handlers through known or explicitly configured API semantics, including identifier aliases and supported static event constants.
- Add user rules for custom APIs, with explicit receiver/callee identity, operation role, argument positions, and supported bus/key/handler extraction rules. Keep configuration assumptions distinguishable from source-proven identity.
- Persist necessary event metadata, publish matching immutable snapshots, and integrate update/delete/restart/configuration behavior.
- Expose event relationships through the existing navigation surfaces and the first child's presentation contract, with forward and reverse lookup and bounded output.
- Add the approved event matching, non-matching, lifecycle, exclusion, and output regressions in the existing test layout.
### Out of Scope
- [hard] Do not instrument or execute application handlers, run project build scripts to discover subscriptions, or change application event names, payloads, routing, or source files.
- [hard] Do not equate matching type/variable names or matching event strings with the same bus instance.
- [hard] Do not label an event relation as a direct precise call or assert that a handler certainly executes.
- [deferred] A catalog covering every supported language and every event framework is not part of the approved first delivery.
- [deferred] Runtime tracing, dynamic dependency-injection topology, external broker topology, arbitrary reflection, and complete registration-order/unsubscription simulation remain outside the initial static envelope.
- [deferred] Unrelated macro-expansion attribution, parser fixes, and natural-language ranking work remain with their existing owners.

## Constraints
- Consume both `docs/briefs/evidence/codemap-nav/live-controls.json` and `docs/briefs/evidence/codemap-nav/rust-relations.json` before introducing presentation or Rust identity behavior.
- Use a dedicated event identity/provenance model; reuse snapshot/storage infrastructure without reusing the weaker collection-hint type/field key as bus-instance proof.
- Record each endpoint's role, source file/range, enclosing symbol, canonical API/rule identity, bus evidence, event-key evidence, handler evidence when available, conditions, and resolution status/reason. A handler definition and its subscription call are separate locations.
- Preserve string literals as event-key data only inside a recognized/configured event API. Strings, comments, unrelated `on`/`emit` methods, and free-text mentions must not become event relations.
- Resolve shared-module bindings and aliases only with source evidence. Distinct allocations, shadowed bindings, opaque parameters, factories, and multiple instances remain distinct or unresolved unless stronger evidence is available.
- Support literal keys and statically resolved constants/enums/types only within the proven language/API envelope. Dynamic keys, unknown handlers, wildcard rules, and unknown target scopes require explicit handling rather than broad joins.
- Record conditional registration, removal/once behavior, target/channel qualifiers, and ordering uncertainty when recognized. The relation means a statically identified subscription route, not observed runtime delivery.
- Keep file exclusions at the indexing boundary and test-context inclusion choices at their existing request/context boundary. Do not silently remove all indexed tests or bypass ancestor exclusions.
- Keep serde defaults, extraction-format migration, parser output, stored records, and snapshot consumers coherent as one atomic change. Old data must rebuild or load compatibly and must not silently reuse missing event records as a complete result.
- Version or invalidate event evidence when sources, imported bus/key definitions, API rules, relevant analysis-target inputs, or exclusion settings change. Reapply current filters before rendering.
- Bound indexing work, candidate joins, records per key/file, and output; use per-key lookups on requests. If a cap hides endpoints, disclose the omitted population rather than presenting an exhaustive map.

## Related Files / Entry Points
- `apps/codemap-search/src/parser/types.rs` — define optional event endpoint/provenance data without weakening existing `CallSite` or collection contracts.
- `apps/codemap-search/src/parser/mod.rs` — trace `IndexAuxiliary` and `extract_for_index()` before adding event extraction.
- `apps/codemap-search/src/lang/mod.rs` — inspect language hooks and keep API-specific extraction bounded.
- `apps/codemap-search/queries/typescript/static_collection_edges.scm`, `apps/codemap-search/queries/javascript/static_collection_edges.scm` — use the existing query layout as a reference, not as an event-semantic matcher.
- `apps/codemap-search/src/callers/resolution.rs`, `apps/codemap-search/src/callers/resolution/local.rs` — consume source/import evidence and the preceding child's explicit-target policy.
- `apps/codemap-search/src/index/engine.rs` — persist event records, handle old extraction formats, and remove stale records on file updates/deletions.
- `apps/codemap-search/src/index/indexer.rs` — publish the event lookup maps in one coherent `PublishedIndexSnapshot` generation.
- `apps/codemap-search/src/index/supervisor.rs`, `apps/codemap-search/src/index/watcher.rs` — verify request fallback, dependency freshness, and watched updates.
- `apps/codemap-search/src/config.rs`, `apps/codemap-search/src/config_template.toml`, `apps/codemap-search/src/config_template.ko.toml` — add rule validation/precedence/reload and coordinate schema migration.
- `apps/codemap-search/src/tools/search/mod.rs`, `apps/codemap-search/src/tools/search/render.rs` — select and render bounded forward/reverse event relationships.
- `apps/codemap-search/src/tools/live_symbols.rs`, `apps/codemap-search/src/tools/live_symbols/render.rs`, `apps/codemap-search/src/tools/mod.rs`, `apps/codemap-search/src/mcp/mod.rs` — integrate event display with the established presentation modes and advertised contract.
- `apps/codemap-search/tests/e2e/tools.rs`, `apps/codemap-search/tests/e2e/search.rs`, `apps/codemap-search/tests/e2e/watcher.rs` — extend the existing MCP and refresh test infrastructure.
- `apps/codemap-search/tests/fixtures/event_navigation` (proposed) — keep the approved static event examples and negative controls together.
- `apps/codemap-search/docs/configuration.md`, `apps/codemap-search/docs/configuration.ko.md`, `apps/codemap-search/docs/development-language-commands.ko.md` — document rules, supported patterns, provenance, limits, and exact query examples.
- `/Users/buyong/workspace/private/corral/apps/desktop/src-tauri/src` — inspect actual event APIs read-only before defining Rust/framework rules.
- `/Users/buyong/workspace/private/corral/apps/desktop/src` — follow frontend publication/registration wrappers and imports read-only, including any source-supported bridge to the Rust side.
- `docs/briefs/evidence/codemap-nav/event-map.json` (proposed) — publish the event map contract, supported-pattern inventory, and final evidence.

## Execution Plan
### Stage 1 — Inventory event semantics and freeze matching evidence
- Starts when: `docs/briefs/evidence/codemap-nav/live-controls.json` and `docs/briefs/evidence/codemap-nav/rust-relations.json` report compatible contracts and successful side-effect checks, and the approved actual-usage/example scope is available.
- Work: Identify Corral's actual publication/registration APIs through bounded inspection of frontend and Rust call sites, wrappers, and imports; confirm their semantics from implementation or primary documentation and define the illustrative static on/emit protocol. Freeze supported rules, bus/key identity evidence, target qualifiers, unresolved reasons, query entry points, presentation gating, persistence/reload requirements, and a non-empty positive/negative scenario matrix. A failed literal-name search alone cannot establish API absence. Do not require the user to perform this technical inventory.
- No-op when: a current event map already meets every matching, custom-rule, lifecycle, exclusion, and final-output criterion on the approved scenarios.
- No-op handoff: write the proven existing contract and scenario results to `docs/briefs/evidence/codemap-nav/event-map.json` (proposed) with `outcome=already-satisfied` and pass it to the parent's global acceptance review. Failed proof returns to this child's bounded implementation/re-verification route.
- Deliverable: `docs/briefs/evidence/codemap-nav/event-contract.json` (proposed) with actual API locations, supported language/API patterns, user-rule schema, endpoint/provenance schema, identity and condition policy, query/presentation contract preserving omitted-option behavior, storage/version plan, and scenario matrix.
- Verify: `Inspect the source-grounded API inventory and its positive/negative scenario matrix`; Inputs: actual Corral publication/registration sites and the synthetic examples below; Expected: each supported pair names both endpoints and its bus/key proof, while every unsupported shape has an explicit reason and no invented target.
- Ends when:
  - [x] At least one explicit static pair is supported and verified against its API semantics, and each discovered actual pattern is mapped to a supported case or a bounded unresolved condition.
  - [x] The rule schema cannot match arbitrary same-name methods without receiver/callee evidence or an explicit recorded configuration assumption.
  - [x] Parser-to-persistence-to-snapshot ownership and all invalidation inputs are recorded.
- Handoff: Stage 2 receives `docs/briefs/evidence/codemap-nav/event-contract.json`.
- Replan when: an actual required pattern needs runtime-only topology or changes to an external application. Keep unresolved evidence, return the concrete case to the parent, and adjust scope explicitly before dependent implementation; do not silently claim complete support or widen to every framework.
- Worker decision: select dedicated modules, query hooks, and bounded indexes consistent with the existing language/index layout; add a dependency only if existing facilities cannot implement the approved static envelope.

### Stage 2 — Build and refresh the event index
- Starts when: `docs/briefs/evidence/codemap-nav/event-contract.json` fixes the matching and storage envelope.
- Work: Extract source-backed endpoints, resolve bus/key/handler identities within the approved rules, persist them, and publish per-file/per-key lookup indexes. Implement create/modify/delete/restart and dependency/rule/config invalidation with coherent snapshot publication and bounded work.
- Deliverable: the integrated extractor/index lifecycle and `docs/briefs/evidence/codemap-nav/event-index.json` (proposed) with schema/version details, endpoint populations, matching outcomes, update/delete/restart records, and limits.
- Verify: `Run the listed package checks separately`; Inputs: `cargo check --locked`, `cargo test --locked --lib` from `apps/codemap-search`, using the package and approved positive/negative/lifecycle cases; Expected: exit 0, true static pairs join by evidence, separate buses do not join, and stale endpoints disappear from published generations.
- Ends when:
  - [x] A non-empty event index supports forward and reverse lookup without a request-time full-workspace scan.
  - [x] File deletion, subscription/key edits, imported-identity changes, configuration changes, and restart cannot leave stale confirmed links.
  - [x] Index caps, invalid rules, unresolved evidence, and old-format data have explicit bounded behavior.
- Handoff: Stage 3 receives the implementation and `docs/briefs/evidence/codemap-nav/event-index.json`.
- Replan when: persistence drops endpoint evidence or source and relation generations can diverge. Stop dependent output work, repair the atomic producer/storage/snapshot path, and rerun lifecycle checks before continuing.

### Stage 3 — Expose event navigation and verify the complete flow
- Starts when: Stage 2's indexed records and lifecycle checks are complete and the first child's presentation contract remains available.
- Work: Render a separate event relationship section with publication, registration, handler, bus/key evidence, and uncertainty. Integrate source-only/definition/relation/full modes and output caps. Add approved MCP regressions, replay supported actual examples, update documentation, and evaluate every side-effect checkpoint.
- Deliverable: `docs/briefs/evidence/codemap-nav/event-map.json` (proposed) with `outcome`, consumed sibling-contract identities, final binary/source/config/rule hashes, API coverage, scenario requests and raw-output paths, snapshot/lifecycle results, indexing/lookup measurements, unresolved limits, and `side_effects_clear`.
- Verify: `Run the listed package checks separately`; Inputs: `cargo test --locked --test e2e_tests e2e::search::`, `cargo test --locked --test e2e_tests e2e::tools::`, `cargo test --locked --test e2e_tests e2e::watcher::` from `apps/codemap-search`, using existing non-empty suites and the new event cases on one final candidate binary; Expected: exit 0, correct original file/line locations and coherent updates, no false joins, and no event metadata in source-only mode.
- Ends when:
  - [x] Every supported API pattern has positive and negative final-output evidence.
  - [x] All lifecycle, scope, exclusion, uncertainty, and output-mode cases have recorded results.
  - [x] The documented custom-rule examples execute through the final parser/index/renderer path.
- Handoff: the parent receives `docs/briefs/evidence/codemap-nav/event-map.json` for whole-set acceptance on the integrated candidate.
- Replan when: output suggests guaranteed runtime delivery, static evidence cannot justify a displayed link, or integration breaks a sibling contract. Correct the owning child and repeat affected join checks before marking the set complete.

Use these synthetic cases as explicit acceptance inputs, not as existing repository paths or completed results:

```ts
// src/events.ts
export const appBus = new KnownBus();
appBus.on("saved", handleSaved);
// src/users.ts: imports the same appBus binding
appBus.emit("saved");
```

- Supply a confirmed or explicitly configured API rule for `KnownBus`, and a resolvable `handleSaved` definition; method spellings alone do not authorize a match.
- Add separate-bus/same-event, same-type/multiple-instance, alias/reexport, shadowed-binding, unknown receiver, dynamic-key, unknown-handler, unrelated-method, comment/string-only, conditional-registration, removal/once, target/channel, and output-cap controls.
- Add create/modify/delete, imported bus/key change, rule reload, target-context change where relevant, test-context toggling, directory exclusion, and restart controls. Prove the intended endpoint population is non-empty before asserting that a negative query has no links.

## Side Effect Checkpoints
- [x] Existing direct calls/callers and collection write/read hints retain their distinct schemas, confidence claims, and display behavior.
- [x] No event string, method spelling, type name, or variable name alone creates a cross-file link.
- [x] Canonical identities respect workspace scope, imports, aliases, source ranges, and configured analysis targets without collapsing separate instances.
- [x] Updated/deleted files and modified dependencies cannot retain stale event edges after a successful refresh, including after restart.
- [x] Directory/Git exclusions and current test-context choices apply to both endpoints and all displayed handler definitions.
- [x] User-rule validation, layer precedence, reload, disable/removal, and error fallback reach the final matching/index consumers.
- [x] Old index data rebuilds or deserializes with an explicit incomplete/unavailable state until event evidence is ready.
- [x] Conditional registration, unsubscription, once-only behavior, target qualifiers, and runtime ordering remain visible as limits of the static relation.
- [x] Relation lookup and rendering use bounded indexed data; measurements distinguish initial indexing, incremental refresh, and query time.
- [x] The original application sources, payload contracts, config, and live event delivery are unchanged.

## Acceptance Criteria
- [x] Each approved static example shows the publisher, registration, handler definition, bus identity, and event key at the correct original locations.
- [x] A query from either side can find its source-confirmed event counterpart within the requested workspace scope and output budget.
- [x] Separate bus instances using the same event name produce no confirmed cross-link; unknown identity/key/handler cases disclose the reason instead of guessing.
- [x] The first actual-usage inventory and explicit on/emit protocol have verified supported cases, while unsupported dynamic conditions are itemized rather than hidden behind a blanket success claim.
- [x] A user rule for a custom API is validated, applied, reloaded, and removed through the complete extraction/index/query path without modifying that API's implementation.
- [x] Create/modify/delete/dependency/configuration/restart cases leave the final published map consistent with source and active rules.
- [x] Event relationships remain separate from direct calls and never claim guaranteed runtime execution; source-only mode contains no event metadata.
- [x] Approved regressions and relevant existing suites pass, every side-effect checkpoint is recorded, and the final report states supported patterns and measured costs without claiming universal framework coverage.

## Open Questions
- None — the user approved actual usage plus explicit examples first, user-rule extensibility, and focused regression tests; technical API inventory and storage choices are assigned to Stage 1.


## 실행 결과 — 2026-09-14

Waves 1·2의 검증 보고서를 받은 뒤 순차 실행했다. [이벤트 계약](evidence/codemap-nav/event-contract.json), [색인 단계](evidence/codemap-nav/event-index.json), [최종 결과](evidence/codemap-nav/event-map.json)에 생산·저장·스냅샷·질의·출력 경로와 검증을 기록했다. 설정 스키마는 12, 색인 형식은 `v28-indexed-event-inputs`다. 런타임 의존성은 추가하지 않았다.

공유 할당·별칭·상수·핸들러의 위치, 별도 인스턴스, 가려진 변수, 불투명한 수신자, 동적 키, 조건·once·제거, 대상·채널, 사용자 규칙과 내장 규칙 해제, 계층 우선순위, 갱신·삭제·재시작, 테스트·Git·디렉터리 제외를 검증했다. 다중 grep의 중복 후보 소모는 이전/이후 동일 입력으로 재현·수정했다. 소스 전용 출력에는 이벤트 자료가 섞이지 않는다.

Corral 복사본에서 사용자 규칙으로 `platform:window-op` 발행 `window_op_event/mod.rs:155`, 등록·인라인 핸들러 `windowOpEvent.ts:50`, 양쪽 상수 정의를 확인했다. 이 연결은 명시한 버스·대상 가정으로 표시한다. `stream-deck:state-changed`의 Rust AppHandle은 인스턴스를 입증할 수 없어 미해결이며, 개발 SSE와 런타임 전달은 자동 해석 범위 밖이다.

최종 코드의 검증 329개가 모두 통과했고, 같은 고정 release 바이너리로 앞선 두 브리프·실제 Corral·예제의 프로토콜 요청을 재실행했다. 설치본 CLI 6개 사례도 통과했다. 세부 명령·해시·원문 경로·측정 조건은 [통합 결과](evidence/codemap-nav/integration.json)를 따른다.

원본 Corral의 애플리케이션 소스·활성 설정은 유지됐다. 최초 기록 이후, 최종 재검증 이전에 이전 v27 실행에 해당하는 설정 주석·색인 갱신이 관측됐다. 변경 주체는 확인되지 않았으며 [별도 기록](evidence/codemap-nav/original-corral-drift.json)에 남겼다. 이 작업의 원본 보존 판정은 [통제된 최종 재실행 전후 비교](evidence/codemap-nav/preservation.json)를 뜻하며, 대화 전체 기간의 메타데이터 불변을 주장하지 않는다.
