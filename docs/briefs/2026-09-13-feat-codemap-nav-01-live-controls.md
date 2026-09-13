# [feat] Add focused live-code output modes

## Work Type
feat

## Current State (As-Is)
- [confirmed] Ground this plan in the working tree inspected on 2026-09-13 at `/Users/buyong/workspace/private/buyong-mcp`, with HEAD `16192bc8a6f44a89c9a5863232ebcce849b9f2cb` and substantial uncommitted codemap-search work. Evidence: `git rev-parse HEAD` and `git status --short`; select and record the actual implementation baseline before editing.
- [confirmed] MCP `read` always appends live symbol context, and content-mode `grep` does the same. Evidence: the `read` and `grep` dispatch arms in `apps/codemap-search/src/mcp/mod.rs` and `live_symbols::append()`.
- [confirmed] The advertised `read` schema contains `file_path`, `offset`, and `limit`; `grep.output_mode` distinguishes content, file names, and counts. Neither schema exposes context presentation controls. Evidence: `tools/list` construction in `apps/codemap-search/src/tools/mod.rs`; preserve the existing search-only meaning of `search.caller_context`.
- [confirmed] `read` obtains its live line window through `resolve_window_args()` and returns `LiveOutput` with source anchors. `grep` gathers requested context lines and derives anchors through `content_anchors()`. Evidence: `apps/codemap-search/src/tools/read.rs` and `apps/codemap-search/src/tools/grep.rs`.
- [confirmed] The user supplied C1, C3, and C4 below, reported approximately 6,200 source/line-number characters plus 1,700 context characters for C1, and reported that C3 returned 150 of 296 rows. These are user observations, not author-reproduced measurements or a comparison between versions.
- [confirmed] The user reported `read_history_window_bounds` at lines 733–746, 14 lines, and `move_window_bounds_by_id_excluding_window_id` at lines 1768–1817, 50 lines. Evidence: the supplied evaluation; relocate both by identifier before measuring because line numbers can drift.
- [confirmed] Exact-identifier lookup, desktop/Rust scoping, and immediate disk reads were useful in the supplied workflow. The user did not establish natural-language search quality, collection-analysis accuracy, or index-refresh latency. Preserve these distinctions in the evidence report.

## Desired Outcome (To-Be)
- Request source results without symbol, caller, callee, reference, or event metadata when checking a local condition.
- Select definition-focused or relation-focused presentation without manually stripping `# results` or performing whole-response post-processing.
- Collapse unresolved calls into an accurate count on request while retaining an explicit way to inspect their names and reasons.
- Read the containing callable's complete declaration and body without calculating a large `-A` or manually chaining `overview` to `read`.
- Preserve exact source text and line positions, existing scoping, explicit caller options, and clear output-limit notices.

## Scope
### In Scope
- Add optional presentation controls to MCP `read` and content-mode `grep`, with a shared internal contract available to subsequent Rust and event-relation work.
- Add opt-in callable-boundary expansion for live reads and grep matches, including named functions and methods supported by existing source parsers.
- Implement source-only, definition-focused, relation-focused, and existing full presentation; provide unresolved-list versus count-only control.
- Define option precedence, output budgets, callable deduplication, multi-match handling, and continuation behavior as one read/grep request contract.
- Update advertised schemas, tool guidance, configuration documentation when a setting is added, and executable command examples.
- Add focused regression cases within the existing test layout, as explicitly approved for this briefset.
### Out of Scope
- [hard] Do not change Corral's window-movement behavior, completion-confirmation count, event handlers, or source files.
- [hard] Do not repurpose `grep.output_mode` or change its existing `content`, `files_with_matches`, and `count` semantics.
- [deferred] Natural-language ranking improvements and a new full public-language benchmark campaign are not required by this child.
- [deferred] Macro-expansion attribution and unrelated bundled-grammar changes belong to the existing implementation work, not this briefset.

## Constraints
- Keep the existing JSON-RPC text-content envelope, read-only annotations, argument aliases, line numbering, and omitted-option behavior compatible. New presentation and expansion behavior must be explicit opt-ins.
- Keep `search.caller_context=false` effective and independent of newly introduced read/grep options; do not overwrite caller-selected options with global defaults.
- Keep `read_output_byte_cap`, annotation budgets, and grep caps effective. Record bytes and characters separately; record token counts only with a named tokenizer and do not infer them from characters.
- Source-only mode must bypass expensive relation construction rather than merely discard its formatted result. Filesystem errors and permission checks must remain visible.
- Resolve expanded bounds from the same live source bytes returned to the user, or prove an indexed range matches those bytes. Do not trust a stale line range or guess a body after parser failure.
- Preserve the existing interpretation of `offset`, `limit`, `-A`, `-B`, `-C`, `head_limit`, `next_offset`, and `read_suggestion` when expansion is omitted. Freeze and document conflicts and continuation units for explicit expansion before implementation.
- Treat the user's broad `-A` requests and output pagination as valid current behavior. Separate the new capability from an alleged defect in returning requested context.
- Compare old and new binaries against the same source revision, configuration, scope, target context, and cold/warm process condition. Do not attribute invocation mistakes, wrong paths, or JavaScript orchestration errors to codemap-search.

## Related Files / Entry Points
- `apps/codemap-search/src/mcp/mod.rs` — start at the `read` and `grep` arms that append context unconditionally.
- `apps/codemap-search/src/tools/mod.rs` — extend advertised schemas and shared argument handling without changing existing grep modes.
- `apps/codemap-search/src/tools/read.rs` — trace `resolve_window_args()` through live reads and anchors.
- `apps/codemap-search/src/tools/grep.rs` — trace matched rows, context rows, pagination, and `content_anchors()` before adding expansion.
- `apps/codemap-search/src/tools/live_symbols.rs` — define the shared presentation boundary around `LiveOutput` and `append()`.
- `apps/codemap-search/src/tools/live_symbols/structure.rs` — reuse validated declaration ranges and signatures without copying neighboring bodies.
- `apps/codemap-search/src/tools/live_symbols/render.rs` — apply definition/relation selection and byte budgets to the final output.
- `apps/codemap-search/src/callers/annotate.rs` — route unresolved-count/list selection to its producer instead of hiding already-built strings.
- `apps/codemap-search/src/tools/search/mod.rs` — preserve the existing `caller_context` contract and align reusable presentation guidance.
- `apps/codemap-search/src/config.rs` — inspect defaults and migrations if persistent options are introduced.
- `apps/codemap-search/docs/configuration.md`, `apps/codemap-search/docs/configuration.ko.md`, `apps/codemap-search/docs/development-language-commands.ko.md` — publish the final option contract and before/after commands.
- `apps/codemap-search/tests/e2e/tools.rs`, `apps/codemap-search/tests/e2e/mcp.rs` — extend existing live-tool and schema regression coverage.
- `apps/codemap-search/scripts/public_validation.py` — use its existing `query` operation when pinning an explicit binary for replay.
- `/Users/buyong/workspace/private/corral/apps/desktop/src-tauri/src/features/platform/macos/window/native_client.rs` — read-only acceptance source for C1, C3, and C4.
- `docs/briefs/evidence/codemap-nav/live-controls.json` (proposed) — final addressable presentation contract and verification handoff.

## Execution Plan
### Stage 1 — Pin the live-read baseline and option contract
- Starts when: this child and the supplied C1/C3/C4 inputs are available, and an isolated or otherwise non-conflicting implementation baseline has been selected.
- Work: Record the exact binary, source/configuration hashes, current schemas, existing limits, and separate source/context output sizes. Relocate the named functions. Freeze additive parameter names, defaults, presentation semantics, expansion precedence, cap behavior, and continuation units without changing legacy requests.
- No-op when: all desired presentation modes, unresolved summaries, and live callable expansion already satisfy this child's criteria on the selected baseline.
- No-op handoff: write the same contract and proof to `docs/briefs/evidence/codemap-nav/live-controls.json` (proposed), mark `outcome` as `already-satisfied`, and pass it to the parent so successors continue without duplicate edits. If proof fails, resume this child's correction route before successors start.
- Deliverable: `docs/briefs/evidence/codemap-nav/live-baseline.json` (proposed) containing binary/source/config identities, C1/C3/C4 raw responses, relocated callable ranges, character/byte counts, and the proposed option matrix.
- Verify: `Inspect tools/list and replay C1, C3, and C4 individually through the existing MCP query operation`; Inputs: the exact JSON cases below and a pinned binary on a disposable Corral checkout; Expected: source rows agree with disk and baseline context/pagination are recorded independently of tool-call orchestration.
- Ends when:
  - [x] Every supplied input has a preserved raw response or an explicit unavailable reason, and the non-empty target population is identified.
  - [x] The mode/precedence matrix states what omitted options and every new option combination return.
- Handoff: Stage 2 receives `docs/briefs/evidence/codemap-nav/live-baseline.json` with the fixed option matrix and source anchors.
- Replan when: current source or schema changes invalidate the captured baseline, or implementing a mode requires a compatibility break. Refresh evidence and adjust this child's route with the parent before dependent edits.
- Worker decision: choose additive parameter names and the internal enum representation after inspecting existing helpers; do not reuse `grep.output_mode` or change default output.

### Stage 2 — Implement focused output and callable expansion
- Starts when: `docs/briefs/evidence/codemap-nav/live-baseline.json` contains the option contract and verified source anchors.
- Work: Integrate schema, dispatch, live range selection, and rendering so each requested view is produced within its budget. Resolve expansion against live syntax, deduplicate repeated callable matches, and preserve partial/unavailable notices. Include unresolved calls only as the selected summary or detailed list.
- Deliverable: the integrated implementation and `docs/briefs/evidence/codemap-nav/live-controls-draft.json` (proposed) with final schema, mode semantics, example requests, and coverage of parser failure, stale index, empty matches, multiple callables, and caps.
- Verify: `Run cargo check, trace option propagation, and replay one request per mode`; Inputs: `cargo check --locked` from `apps/codemap-search`, the fixed Stage 1 option matrix, dispatch-to-renderer call sites, and a pinned candidate MCP binary on the disposable source checkout; Expected: compilation exits 0, each option reaches its final consumer, and raw candidate responses follow the selected mode and live callable bounds.
- Ends when:
  - [x] Source-only requests skip relation preparation and preserve the original result text.
  - [x] Definition/relation modes and unresolved summaries obey the documented precedence and budgets.
  - [x] Expanded results end at the live callable boundary or explicitly report why a complete expansion was unavailable.
- Handoff: Stage 3 receives the implementation and `docs/briefs/evidence/codemap-nav/live-controls-draft.json`.
- Replan when: a source language cannot provide trustworthy bounds, or overlapping options silently change legacy pagination. Keep the legacy route working and document a bounded unsupported-expansion fallback before continuing.

### Stage 3 — Prove output usefulness and publish the handoff
- Starts when: Stage 2's implementation and draft contract are available.
- Work: Add the approved regression cases, replay the original requests plus the new option variants on identical inputs, and evaluate every side-effect checkpoint. Record source/context bytes, displayed unresolved names/counts, requests needed to reach each body, and elapsed times without inventing a speed target.
- Deliverable: `docs/briefs/evidence/codemap-nav/live-controls.json` (proposed) with `outcome`, baseline/candidate binary and source hashes, parameter schema, mode semantics, budget/precedence rules, replay requests and raw-output paths, check results, and `side_effects_clear`.
- Verify: `Run the listed package checks separately`; Inputs: `cargo test --locked --test e2e_tests e2e::tools::`, `cargo test --locked --test e2e_tests e2e::mcp::` from `apps/codemap-search`, using the existing non-empty tool/MCP suites plus the new approved cases in the package directory; Expected: exit 0, legacy requests retain their contracts, and replay proves the new modes and boundaries.
- Ends when:
  - [x] All supplied cases and new-mode counterparts have comparable recorded evidence.
  - [x] Schemas, descriptions, documentation, and implementation agree on the final option names and behavior.
- Handoff: the parent and both sibling briefs receive `docs/briefs/evidence/codemap-nav/live-controls.json` after evaluating this child's whole-work acceptance criteria.
- Replan when: any preservation check fails or a measured improvement depends on different source/configuration/process conditions. Correct this child and rerun affected checks before publishing a successful handoff.

The original user inputs are fixed baseline data. Do not add new options to these baseline payloads; record candidate variants separately.

C1 — `read_focused_window_state_from_context` cache-path investigation, tool `read`:

```json
{
  "file_path": "/Users/buyong/workspace/private/corral/apps/desktop/src-tauri/src/features/platform/macos/window/native_client.rs",
  "offset": 1985,
  "limit": 119
}
```

Reported context fragments:

```text
29 callee(s) unresolved in indexed source; no definition attributed.
with (unresolved)
and_then (unresolved)
as_ref (unresolved)
borrow (unresolved)
clone (unresolved)
```

C3 — history read plus movement lookup, tool `grep`:

```json
{
  "path": "/Users/buyong/workspace/private/corral/apps/desktop/src-tauri/src/features/platform/macos/window/native_client.rs",
  "pattern": "fn move_window_bounds_by_id|fn read_history_window_bounds",
  "-B": 3,
  "-A": 105,
  "head_limit": 150
}
```

Reported continuation:

```text
Showing results 1-150 of 296; next_offset=150.
```

C4 — narrowed movement implementation, tool `grep`:

```json
{
  "path": "/Users/buyong/workspace/private/corral/apps/desktop/src-tauri/src/features/platform/macos/window/native_client.rs",
  "pattern": "^pub\\(crate\\) fn move_window_bounds_by_id_excluding_window_id\\(",
  "-A": 86,
  "head_limit": 90
}
```

## Side Effect Checkpoints
- [x] Existing read/grep text, aliases, line numbering, regex matches, case/glob/type filters, and non-content modes retain their previous meaning when new options are omitted.
- [x] Filesystem permission checks, directory exclusions, Git ignores, explicit `include_ignored`, and test-context policy still apply at their existing boundaries.
- [x] `search.caller_context=false` continues to override its configured default and does not accidentally hide source results.
- [x] Fresh source edits are visible independently of index readiness, and stale ranges cannot include unrelated neighboring functions under expansion.
- [x] Oversized bodies and multi-match responses never silently claim a complete function after clipping.
- [x] Anonymous closures, nested named callables, attached attributes, CRLF/BOM input, and unsupported parse shapes have explicit and source-faithful behavior.
- [x] Existing macro-origin/unavailable-context notices survive applicable modes without this child changing macro resolution.
- [x] No test or verification step changes the original Corral files or its existing index/configuration; use a disposable replay checkout for index-backed comparisons.

## Acceptance Criteria
- [x] A source-only variant of C1 returns the same live source/line-number portion with no symbol or relation metadata and without computing caller/event relations.
- [x] A summary variant records the same unresolved total as the detailed variant while omitting the individual unresolved names; a detailed variant remains available.
- [x] Definition-focused and relation-focused variants each prioritize their documented content without exceeding the existing effective output cap.
- [x] Callable expansion for C3/C4 returns the relocated target declaration/body boundaries and excludes the adjacent observation, application-termination, application-start-time, or next-function bodies unless separately matched.
- [x] Repeated matches inside one callable do not repeat its whole body; partial multi-callable output has an actionable continuation or narrowing notice.
- [x] All legacy and new-option regressions pass, the relevant existing suites execute non-zero tests successfully, and every side-effect checkpoint has a recorded result.
- [x] The final handoff contains comparable evidence and exact ready-to-run option examples; it makes no unsupported natural-language ranking, index-latency, or historical speedup claim.

## Open Questions
- None — the user approved targeted regression tests and preserved the requested opt-in behavior; reversible option naming and pagination details are bounded Stage 1 worker decisions.

## 실행 결과

2026-09-14: 단계별 기준·옵션 계약·C1/C2/C3/C4 재현은 [live-controls.json](evidence/codemap-nav/live-controls.json)에 기록했다. 기존 요청의 응답 및 source 변형의 원문을 고정 기준과 바이트로 대조했으며 도구 검사 29개, MCP 검사 26개를 통과했다. 후속 두 단계가 통합된 최종 후보에서 전역 완료 조건을 다시 확인한다.


최종 이벤트 통합 후에도 [통합 결과](evidence/codemap-nav/integration.json)의 고정 후보로 다시 통과했다. 초기 인계 보고서의 `final_integration`에 최종 소스·바이너리와 원본 보존 비교 범위를 기록했다.
