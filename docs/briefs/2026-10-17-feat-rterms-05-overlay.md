# [feat] Fit synonyms answers to the indexed codebase

## Work Type
feat

## Current State (As-Is)
- [confirmed] As of `e473b5f48` on `feat/related-terms-lookup` (inspected 2026-10-17), the indexer publishes whole `Arc<Vec<ExtractedFile>>` snapshots after successful passes — Evidence: `apps/codemap-search/AGENTS.md` section "Single-writer refresh model"; `PublishedIndexSnapshot` in `apps/codemap-search/src/index/indexer.rs` and the publish assignment `*snapshot.lock().unwrap() = Arc::new(published_snapshot);`.
- [confirmed] The first index pass starts at server start, and later snapshots are published only when a pass changes something or a publish is forced — Evidence: comment "immediately so the first request need not block on it" in `apps/codemap-search/src/mcp/mod.rs`; the `should_force_publish` parameter in `apps/codemap-search/src/index/indexer.rs`. A dataset that becomes ready after the first publish therefore may not see another publish for a long time.
- [confirmed] `ExtractedFile` holds `file_path`, `total_lines`, `symbols`, `literals`, `docstrings`, and `navigation` — Evidence: `pub struct ExtractedFile` in `apps/codemap-search/src/parser/types.rs`.
- [confirmed] Symbol names and their split tokens are gathered during indexing — Evidence: symbol field filling in `apps/codemap-search/src/index/engine.rs` (around the `split_identifier` use for symbols); `split_identifier()` in `apps/codemap-search/src/parser/tokenize.rs`.
- [confirmed] A healthy watcher sends path-scoped refreshes for edits and escalates to a full refresh on git HEAD changes — Evidence: AGENTS.md section "Watcher-first freshness".
- [confirmed] `search` and `overview` call `self.engine.ensure_alive()` then `self.engine.trigger_refresh()` on each request; the request-triggered refresh "runs only when the watcher is absent (`watch = false`), failed to start, or died" — Evidence: `tools/call` branches in `apps/codemap-search/src/mcp/mod.rs`; doc comment on `trigger_refresh()` in `apps/codemap-search/src/index/supervisor.rs`.
- [confirmed] The key rule folds case and inner separators, so `AbortSignal`, `abort_signal`, and `abort signal` share the key `abortsignal` — Evidence: `INNER_SEPARATORS` and `word_key()` in `apps/codemap-search/experiments/related-terms-poc/term_canonical.py`.
- [confirmed] The 12 fixed repositories are checked out locally at `.codemap/poc/related-terms/workspaces/checkouts/<repository>` with pinned tags and commits — Evidence: `docs/related-terms/sources.md` section "코드 저장소 12개".
- [confirmed] Those checkouts are shallow clones with one tag each, so no checkout can switch to a second revision as it is — Evidence: `.git/shallow` exists and `git tag` lists one tag in the `gin`, `express`, and `tokio` checkouts.
- [confirmed] The eval set has 240 items, 20 per repository (bitwarden-server, express, flask, gin, gutenberg, keycloak, redis, sentry, terminal, tokio, vector, woocommerce), 224 with `expected_fields` — Evidence: `apps/codemap-search/experiments/related-terms-poc/sense-eval/eval-set.json` fields `repository` and `expected_fields`.
- [confirmed] The PoC shows a word only once per answer: a word placed in an earlier group is dropped from later groups — Evidence: `place()` in `apps/codemap-search/experiments/related-terms-poc/sense_lookup.py` (`fresh = [k for k in dict.fromkeys(keys) if k not in placed]`). Display groups therefore under-report each sense's words.
- [confirmed] The user chose "dictionary ∩ codebase" (no new synonym mining), updates on every branch change, a strong verification bar, and a starting threshold of "no expected-field hit loss and wrong removals ≤ 5%" to be finalized by the user after measurement; below the threshold, unsupported senses are moved back instead of removed — Evidence: `docs/related-terms/codemap-integration.md` table "결정한 것", rows "코드베이스 맞춤" and "뜻 빼기 기준", and section 6 "검증".
- [inferred] The `synonyms` tool, start-up readiness state, caps (6 senses), per-sense full word sets, and the MCP evaluation driver exist from the MCP tool and lookup children — Confirm by: reading `docs/related-terms/integration-log/04-mcp-tool.md` and `docs/related-terms/integration-log/01-lookup.md` in Stage 1.
- [inferred] Recomputing evidence from scratch on the largest of the 12 repositories may take long enough to need incremental updates or a cache under `.codemap/` — Confirm by: the timing in Stage 2.

## Desired Outcome (To-Be)
- With the feature on, codemap-search keeps, per indexed workspace, counts (files, occurrences) of dictionary keys that appear in the code, built from contiguous 1–4 token groups of split symbol names (plus docstrings or literals if verification selects them).
- When the dataset becomes ready, evidence is computed from the latest snapshot already published; afterwards it follows every publish by subtracting and adding only changed files, so branch switches and edits are reflected.
- Each `synonyms` call requests freshness the same way `search` does, so evidence also updates when the watcher is off, failed to start, or died.
- Sense and field decisions use each sense's full word set; display de-duplication and caps apply afterwards.
- With removal off (the shipped default until the user confirms thresholds), senses without codebase evidence move behind supported senses and absent words move behind present words; no sense is dropped except by the existing caps, whose notes give the full count.
- With removal on, a sense whose words (except the input itself) never appear is removed, and a field with no evidence in the whole workspace is removed from all answers.
- Filtering is skipped when the input itself is absent from the codebase, when filtering would leave no sense, and while evidence is not ready; answers say how many senses were removed or moved back.
- Verification on the 12 repositories shows the measured numbers needed for the user's decision.

## Scope
### In Scope
- Evidence collection off the MCP loop, in memory per workspace: initial computation from the latest published snapshot, then incremental update per publish.
- Comparison of three evidence scopes: symbols only; symbols + docstrings; symbols + docstrings + literals.
- Applying evidence in `synonyms` answers: sense demotion, word reordering, sense removal, field removal, exceptions, removed or moved-back counts, and an "evidence preparing" status.
- Freshness request on each `synonyms` call through `ensure_alive()` and `trigger_refresh()`.
- A removal switch that is off in shipped builds until the user confirms thresholds, plus a non-public evaluation route that turns it on for measurement runs.
- e2e tests for: evidence updates after a file change, observed through a `synonyms` answer; no evidence work when the feature is off.
- Verification script for the 12 repositories under `apps/codemap-search/experiments/related-terms-poc/sense-eval/`.
- Evidence log `docs/related-terms/integration-log/05-overlay.md`.
### Out of Scope
- [hard] Do not modify the downloaded dataset or the tracked rts5 file; evidence lives outside them.
- [hard] Do not mine new synonyms or add words absent from the dictionary.
- [hard] Do not make removal the shipped default before the user confirms the final thresholds; measurement runs through the evaluation route are allowed.
- [hard] Do not watch `.git/HEAD` separately; evidence follows working-tree content through index publishes.
- [hard] Do not change `search` ranking or the Tantivy schema.
- [hard] Do not add config keys; the removal switch is a code-level default until the user decides otherwise.
- [hard] Do not fetch tags into or switch revisions in the shared checkouts under `.codemap/poc/related-terms/workspaces/checkouts/`; use a separate scratch clone for the branch check.
- [hard] Do not add test files beyond the two e2e areas above.
- [deferred] Per-scope (per-app) evidence in monorepos.
- [deferred] Quality improvements (vectors, K7, K9, stage-15 noise).

## Constraints
- 2026-10-17 사용자 결정: 기능 구현을 모두 마친 뒤 Docker에서 `docs/related-terms/sources.md`의 12개 저장소를 새로 clone·index하고 평가한다. 호스트의 기존 체크아웃·인덱스는 본 평가의 입력으로 재사용하지 않는다. 이벤트 탐색을 포함한 기존 설정을 임의로 끄지 않는다.
- 2026-10-17 사용자 결정: 모델 도구 평가와 독립 제거 판정은 보류한다. 자동 측정과 제거 표본만 생성하고 판정값은 미평가로 남긴다. 제품의 제거 기본값은 `false`로 유지한다.
- Evidence work runs only when `related_terms.enabled` is true and the dataset is ready.
- Evidence computation never runs on the MCP request loop; answers during computation are unfiltered with the "evidence preparing" status.
- Tag each evidence result with the snapshot generation it came from, and never let an older generation overwrite a newer one.
- Count only keys that exist in the dictionary (the lookup's word-key set).
- Freshness requests reuse the supervisor's existing watcher-health rules; do not add a separate `watch = false` branch.
- The evaluation route for turning removal on is non-public (for example a Cargo feature used only by evaluation builds) and is recorded in the evidence log.
- Persist evidence under `.codemap/` only if from-scratch computation on the largest of the 12 repositories exceeds the time recorded as acceptable in the evidence log; otherwise keep it in memory.
- Each sampled removed sense gets two independent judgments; a disagreement gets a third. Report the wrong-removal rate from resolved judgments and, as an upper bound, the rate that counts unresolved disagreements as wrong removals.
- Measure expected-field hits twice: on the lookup result before caps and on the final MCP answer after caps; report both.
- The cross-repository control uses only cases where the input word appears in both repositories and no exception triggers; report the number of applicable and skipped cases, and compare in aggregate, not per case.
- The branch check uses a scratch clone of one repository at two revisions whose dictionary evidence differs, and compares evidence maps exactly (same keys, same file and occurrence counts).
- Run `cargo check --manifest-path apps/codemap-search/Cargo.toml` after Rust changes.
- The evidence log is Korean in "~다" style, like other `docs/related-terms` documents.

## Related Files / Entry Points
- `docs/related-terms/integration-log/04-mcp-tool.md` (proposed) — read first: tool handler, readiness state, caps, evaluation driver.
- `docs/related-terms/integration-log/01-lookup.md` (proposed) — per-sense full word sets in the lookup result.
- `apps/codemap-search/src/index/indexer.rs` — `PublishedIndexSnapshot`, the publish point, and `should_force_publish`.
- `apps/codemap-search/src/mcp/mod.rs` — `ensure_alive()` / `trigger_refresh()` calls in the `search` branch to mirror for `synonyms`.
- `apps/codemap-search/src/parser/types.rs` — `ExtractedFile` fields used as evidence input.
- `apps/codemap-search/src/parser/tokenize.rs` — `split_identifier()` for symbol names.
- `apps/codemap-search/src/index/watcher.rs` — refresh behavior that drives publishes on branch changes.
- `apps/codemap-search/src/index/supervisor.rs` — `trigger_refresh()` health rules.
- `apps/codemap-search/src/tools/synonyms.rs` (proposed) — where filtering applies to answers.
- `apps/codemap-search/src/related_terms/evidence.rs` (proposed) — evidence store and incremental update.
- `apps/codemap-search/tests/e2e/helpers.rs` — existing index-wait helpers (`response_is_waiting_for_index`) for observing publishes.
- `apps/codemap-search/tests/e2e/related_terms_overlay.rs` (proposed) — new e2e tests.
- `apps/codemap-search/experiments/related-terms-poc/sense-eval/eval-set.json` — words grouped by repository.
- `apps/codemap-search/experiments/related-terms-poc/sense-eval/overlay_eval.py` (proposed) — 12-repository verification script.
- `docs/related-terms/sources.md` — pinned tags and checkout paths of the 12 repositories.
- `docs/related-terms/codemap-integration.md` — design section 6.
- `docs/related-terms/integration-log/05-overlay.md` (proposed) — evidence log and decision record.

## Execution Plan
### Stage 1 — Collect evidence per workspace
- Starts when: `docs/related-terms/integration-log/04-mcp-tool.md` exists and the `synonyms` tool answers in a branch-built binary.
- Work: Build the evidence store (symbol names → `split_identifier()` → contiguous 1–4 token groups → word keys → dictionary keys only), off the MCP loop, for all three evidence scopes; compute it from the latest published snapshot when the dataset becomes ready, then keep it current on each publish by updating only changed files, with a generation guard.
- No-op when: None — no codebase evidence exists before this child.
- No-op handoff: None — this child must add evidence and verification.
- Deliverable: Evidence store module `apps/codemap-search/src/related_terms/evidence.rs` (proposed) hooked to dataset readiness and index publishes.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the workspace with the evidence module; Expected: exit 0.
- Ends when:
  - [ ] In a mock repository containing `AbortSignal` and `abort_signal`, the store reports key `abortsignal` with file count 2.
  - [ ] When the dataset becomes ready after the first publish and no file changes, the store still fills from the published snapshot.
  - [ ] Adding then deleting a file with a new symbol changes the store after each publish and restores the original counts after the deletion.
- Handoff: Stage 2 receives the evidence store and its update hook.
- Replan when: Publishes do not expose which files changed and full recomputation is needed each time; measure it in Stage 2 before choosing persistence.
- Worker decision: Changed-file detection method, data structures, thread placement.

### Stage 2 — Apply evidence to answers
- Starts when: Stage 1's store updates on publishes.
- Work: Decide sense and field evidence from each sense's full word set, then apply demotion and reordering (removal off) or removal (removal on), then display de-duplication and caps; add the exceptions, counts, and the "evidence preparing" status; request freshness on each call like `search`; add the evaluation route; time from-scratch computation on the largest of the 12 repositories.
- Deliverable: Filtering in `apps/codemap-search/src/tools/synonyms.rs` (proposed), the evaluation route, and the timing record.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the workspace with filtering; Expected: exit 0.
- Ends when:
  - [ ] With removal off, answers contain the same set of senses as before this child; unsupported senses come after supported ones.
  - [ ] Two senses that share a word present in the code are both treated as supported, even though display shows the word only once.
  - [ ] With removal on through the evaluation route, an input absent from the code returns unfiltered senses.
- Handoff: Stage 3 receives the filtering code, the evaluation route, and the timing.
- Replan when: From-scratch computation on the largest repository blocks the tool's readiness for longer than the initial index build; add a cache under `.codemap/` and record the measured time.

### Stage 3 — Prove updates with e2e tests
- Starts when: Stage 2's filtering compiles.
- Work: Add e2e tests: with the feature off, no evidence work runs; with the feature on, editing a file in a mock repository changes the order in a `synonyms` answer after the refresh is published.
- Deliverable: Test module `apps/codemap-search/tests/e2e/related_terms_overlay.rs` (proposed) registered in `tests/e2e/mod.rs`.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests related_terms_overlay`; Inputs: the new test module; Expected: exit 0 and `test result: ok` with at least 2 passed tests whose names contain `related_terms_overlay`.
- Ends when:
  - [ ] `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests related_terms_tool` still exits 0 with its tests listed.
- Handoff: Stage 4 receives the passing tests.
- Replan when: The in-process server helpers cannot observe a publish deterministically; add a way to wait for the next publish (for example a generation counter exposed to tests) and keep the end-to-end test; a store-only test may be added but does not replace it.

### Stage 4 — Verify on the 12 repositories
- Starts when: 기능 구현과 Stage 3의 테스트가 끝난다. 이후 Docker 이미지를 빌드하고 12개 저장소를 컨테이너에 새로 clone하여 고정 태그·커밋을 확인한다. 브랜치 검증에는 별도 임시 클론을 쓴다.
- Work: For each evidence scope, index each repository, look up its 20 eval words with filtering off, demotion, and removal through the evaluation route, and measure expected-field hits before and after caps, share of returned words found in that repository, output size change, and the cross-repository control. Collect a stratified sample of removed senses (at least 20 per repository and scope, or all if fewer). Then switch the scratch clone between its two tags and compare incremental evidence with from-scratch evidence. Independent removal judgments and MCP agent evaluations remain deferred by the user's 2026-10-17 decision; report their rates as unmeasured.
- Deliverable: Verification script `apps/codemap-search/experiments/related-terms-poc/sense-eval/overlay_eval.py` (proposed) and the per-scope result tables.
- Verify: `docs/related-terms/codemap-integration.md`의 Docker 평가 명령을 실행한다. 컨테이너의 `overlay_eval.py --clone`이 문서의 12개 태그를 새로 clone하고 커밋을 확인한다. 각 저장소의 MCP 초기 인덱스와 최신 근거 준비가 끝난 뒤 평가한다. Inputs: 컨테이너에서 빌드한 현재 작업 트리의 바이너리, 읽기 전용 rts5 파일, `eval-set.json`, 컨테이너의 신규 체크아웃과 별도 브랜치 검증 클론. Expected: exit 0, 36개 저장소·근거 범위 조합, 3개 적용 모드, 브랜치 전환 후 증분·초기 계산의 완전 일치다. 모델 평가는 이번 실행에서 제외한다.
- Ends when:
  - [x] Every word with lost expected-field hits is listed with its cause.
  - [x] The two tags of the scratch clone produce different evidence maps, and the incremental map after switching equals the from-scratch map.
  - [x] The cross-repository table states how many cases were applicable and how many were skipped.
- Handoff: Stage 5 receives the result tables and the recommended evidence scope.
- Replan when: No evidence scope meets the baseline (no hit loss, wrong removals ≤ 5%); keep removal off, record the shortfall, and still hand the numbers to the user.

### Stage 5 — Record and get the user's threshold decision
- Starts when: Stage 4's tables exist for every repository and evidence scope.
- Work: Write the evidence log with the evidence design, timing, per-scope results, branch check, agent evaluation, and a recommendation; present the measured numbers to the user and apply their final thresholds: make removal the shipped default for the chosen scope if the numbers meet them, otherwise keep demotion only.
- Deliverable: Evidence log `docs/related-terms/integration-log/05-overlay.md` (proposed) including the user's final thresholds and the resulting switch state.
- Verify: `bounded inspection of docs/related-terms/integration-log/05-overlay.md`; Inputs: the log file; Expected: it states the chosen evidence scope, the final thresholds with the date the user confirmed them, and whether removal is on.
- Ends when:
  - [ ] The shipped switch state in code matches the log.
  - [ ] `cargo check --manifest-path apps/codemap-search/Cargo.toml` exits 0 on the final state.
- Handoff: Global acceptance in `docs/briefs/2026-10-17-briefset-rterms.md` receives `docs/related-terms/integration-log/05-overlay.md` and the final switch state.
- Replan when: The user rejects all scopes or asks for a different evidence definition; return to Stage 1 with the new definition.

## Side Effect Checkpoints
- [ ] With the feature off, no evidence store is constructed and no evidence thread starts (checked by the disabled-case test in `related_terms_overlay`).
- [ ] Existing watcher e2e tests pass: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests watcher` exits 0.
- [ ] Existing search e2e tests pass: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests search` exits 0.
- [ ] The prepared dataset's SHA-256 is the same at the start and end of an evidence-updating session.
- [ ] A `search` call during evidence computation returns without waiting for it.
- [ ] The shared checkouts keep their pinned commits: `git -C .codemap/poc/related-terms/workspaces/checkouts/<repository> rev-parse HEAD` matches `docs/related-terms/sources.md` for all 12.

## Acceptance Criteria
- [ ] With the feature on, unsupported senses come after supported ones and codebase words come first in each group; answers note how many senses were moved back or removed.
- [ ] Evidence after switching the scratch clone between two tags equals evidence computed from scratch on the second tag.
- [ ] The evidence log reports, per repository and evidence scope: expected-field hits before and after filtering (before and after caps), wrong-removal rate with sample size and upper bound, found-in-repository share, output size change, and the cross-repository control with applicable-case counts.
- [ ] Removal is the shipped default only if the user confirmed final thresholds and the measured numbers meet them; otherwise answers use demotion only.

## Open Questions
- [non-blocking] What final thresholds must the measured numbers meet before sense and field removal becomes the shipped default? — Default: no expected-field hit loss on the 224 eval words and wrong removals ≤ 5% in the judged sample; Reconfirm before: Stage 5 makes removal the shipped default.
