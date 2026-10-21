# Brief Set: Integrate related-terms lookup into codemap-search

## Purpose
- codemap-search offers an opt-in, read-only `synonyms` MCP tool that answers from the rts5 sense dataset alone, downloads that dataset from a GitHub release only when enabled, and fits answers to the indexed codebase with verified evidence.

## 현재 실행 조건 (2026-10-17 사용자 결정)
- 구현은 주 에이전트가 직접 수행한다. 아래의 하위 브리프와 병렬화 설명은 하위 에이전트 실행 승인이 아니다.
- 기능 구현을 모두 마친 뒤 Docker 안에서 `docs/related-terms/sources.md`의 12개 저장소를 새로 clone하고, 고정 태그와 커밋을 확인한 뒤 index·평가한다. 호스트의 기존 PoC 체크아웃과 인덱스는 재사용하지 않는다.
- Docker 평가에서도 기존 이벤트 탐색 설정을 임의로 끄지 않는다. 이미지, 실행 환경, 소스·바이너리·데이터셋 식별값, 저장소별 결과와 실패를 기록한다. 실행 명령은 `docs/related-terms/codemap-integration.md`의 Docker 평가 절차를 따른다.
- GitHub 태그 푸시·릴리스 게시, 모델을 사용하는 도구 평가와 독립 제거 판정은 보류한다. 자동 측정과 표본 생성만 수행하며, 제품의 뜻 제거는 계속 끈다.

## Child Briefs
- [x] `docs/briefs/2026-10-17-feat-rterms-01-lookup.md` — Rust lookup and index preparation on an `rt-sense/5` file, matching the PoC on all 176,351 words.
- [ ] `docs/briefs/2026-10-17-chore-rterms-02-data-release.md` — provenance-checked `tar.zst` data release, not marked latest, with related-terms license notices removed from the repository.
- [x] `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md` — `related_terms` config keys and a corruption-safe download of the pinned dataset.
- [ ] `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md` — opt-in `synonyms` tool with start-up preparation, caps, statuses, and MCP agent evaluation.
- [ ] `docs/briefs/2026-10-17-feat-rterms-05-overlay.md` — codebase evidence, branch-following updates, 12-repository verification, and user-confirmed removal thresholds.

현재 구현 상태: 02의 로컬 배포 준비, 04·05의 기능 구현과 자동 검증은 끝났다. Docker의 12개 저장소·36개 근거 범위 평가와 브랜치 전환 검증도 끝났다. 02의 게시와 04·05의 모델 평가·독립 제거 판정·최종 제거 기준은 사용자 요청으로 보류되어 전체 하위 브리프 체크는 열어 둔다. 결과와 미충족 품질 조건은 `docs/related-terms/integration-log/05-overlay.md`에 있다.

## Execution Order
- Wave 1 — `docs/briefs/2026-10-17-feat-rterms-01-lookup.md`: Start: the tracked rts5 file and the local PoC inputs (`rtr1`, `rtpoc3`) are present; Deliverable: lookup module and comparison evidence; Location: `docs/related-terms/integration-log/01-lookup.md` (proposed); Done: comparison report has "other" = 0 over 176,351 words and the log records the per-call term cap; Handoff: the MCP tool child receives the log and `apps/codemap-search/src/related_terms/`.
- Wave 1 — `docs/briefs/2026-10-17-chore-rterms-02-data-release.md`: Start: the tracked rts5 file matches the pinned SHA-256; Deliverable: public release with archive and checksum; Location: `docs/related-terms/integration-log/02-data-release.md` (proposed); Done: the public URL yields the pinned SHA-256 and `releases/latest` still points to a `codemap-v*` tag; Handoff: global acceptance receives the log and URL.
- Wave 1 — `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md`: Start: `CONFIG_VERSION` is 29 and no `related_terms` key exists; Deliverable: config keys, fetch module, download-integrity e2e tests; Location: `docs/related-terms/integration-log/03-fetch-config.md` (proposed); Done: `related_terms_fetch` e2e tests pass and the log names the fetch API and statuses; Handoff: the MCP tool child receives the log and the fetch module.
- Wave 2 — `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`: Start: both `docs/related-terms/integration-log/01-lookup.md` and `docs/related-terms/integration-log/03-fetch-config.md` exist; Deliverable: opt-in tool, start-up wiring, contract e2e tests, MCP agent evaluation; Location: `docs/related-terms/integration-log/04-mcp-tool.md` (proposed); Done: `related_terms_tool` e2e tests pass and the log has an evaluation row per model; Handoff: the overlay child receives the log, the tool handler, and the evaluation driver.
- Wave 3 — `docs/briefs/2026-10-17-feat-rterms-05-overlay.md`: Start: `docs/related-terms/integration-log/04-mcp-tool.md` exists; Deliverable: evidence store, filtering, 12-repository results, user threshold decision; Location: `docs/related-terms/integration-log/05-overlay.md` (proposed); Done: the log states the evidence scope, the user's final thresholds, and the removal switch state; Handoff: global acceptance receives the log.

## Dependencies
- Predecessor: `docs/briefs/2026-10-17-feat-rterms-01-lookup.md`; Deliverable path: `docs/related-terms/integration-log/01-lookup.md` (proposed); Format: Korean log naming the module path, preparation and lookup functions, result type with display groups and per-sense full word sets, preparation lock and folder layout, comparison class counts, eval-set hits, latency table, per-call term cap; Successor: `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`; Starts when: the log exists and `cargo check` passes with the module; Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the branch with `apps/codemap-search/src/related_terms/` and the log; Expected: exit 0 and the log shows "other" = 0.
- Predecessor: `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md`; Deliverable path: `docs/related-terms/integration-log/03-fetch-config.md` (proposed); Format: Korean log naming the fetch function, dataset spec type, status values, config keys with defaults, added dependencies with reasons, e2e test names; Successor: `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`; Starts when: the log exists and the fetch e2e tests pass; Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests related_terms_fetch`; Inputs: the branch with the fetch module and tests; Expected: exit 0 and at least 6 passed tests whose names contain `related_terms_fetch`.
- Predecessor: `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`; Deliverable path: `docs/related-terms/integration-log/04-mcp-tool.md` (proposed); Format: Korean log with tool schema, output layout, statuses, caps, start-up entry point, expected-field hits before and after caps, MCP evaluation table; Successor: `docs/briefs/2026-10-17-feat-rterms-05-overlay.md`; Starts when: the log exists and the tool answers in a branch-built binary; Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests related_terms_tool`; Inputs: the branch with the tool and tests; Expected: exit 0 and at least 4 passed tests whose names contain `related_terms_tool`.

## Parallelization
- Can run together: `docs/briefs/2026-10-17-feat-rterms-01-lookup.md` and `docs/briefs/2026-10-17-chore-rterms-02-data-release.md` — the release child writes no Rust code and only reads the tracked dataset. Join when: both logs exist.
- Can run together: `docs/briefs/2026-10-17-feat-rterms-01-lookup.md` and `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md` — separate files inside `apps/codemap-search/src/related_terms/`; only the module registration lines are shared (see Conflict Hotspots). Join when: both logs exist and `cargo check` passes with both modules.
- Can run together: `docs/briefs/2026-10-17-chore-rterms-02-data-release.md` and `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md` — the fetch child pins names fixed in Shared Constraints and tests against local fixtures, not the public asset. Join when: both logs exist.
- Can run together: `docs/briefs/2026-10-17-chore-rterms-02-data-release.md` and `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md` — the tool child uses `dataset_path`, not the public asset. Join when: both logs exist.
- Can run together: `docs/briefs/2026-10-17-chore-rterms-02-data-release.md` and `docs/briefs/2026-10-17-feat-rterms-05-overlay.md` — the overlay child does not download. Join when: both logs exist and global acceptance runs.

## Conflict Hotspots
- `apps/codemap-search/src/lib.rs` — Children: `docs/briefs/2026-10-17-feat-rterms-01-lookup.md`, `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md`; Access: parallel-safe; Rule: the child that lands first adds the single `pub mod related_terms;` line; the other adds nothing here.
- `apps/codemap-search/src/related_terms/mod.rs` (proposed) — Children: `docs/briefs/2026-10-17-feat-rterms-01-lookup.md`, `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md`; Access: parallel-safe; Rule: the child that lands first creates the file; each child adds only its own submodule lines; the second rebases and keeps both.
- `apps/codemap-search/Cargo.toml` — Children: `docs/briefs/2026-10-17-feat-rterms-01-lookup.md`, `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md`; Access: parallel-safe; Rule: the fetch child owns the `tar` and `zstd` lines; the lookup child adds a dependency only with a reason in its log; the second to land regenerates `Cargo.lock` with `cargo check`.
- `apps/codemap-search/tests/e2e/mod.rs` — Children: `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md`, `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`; Access: serialized; Owner: `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md`; Rule: the fetch child registers `related_terms_fetch` first; the tool child adds `related_terms_tool` afterwards.
- `apps/codemap-search/tests/e2e/mod.rs` — Children: `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`, `docs/briefs/2026-10-17-feat-rterms-05-overlay.md`; Access: serialized; Owner: `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`; Rule: the overlay child adds `related_terms_overlay` after the tool child's line exists.
- `apps/codemap-search/README.md` — Children: `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md`, `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`; Access: serialized; Owner: `docs/briefs/2026-10-17-feat-rterms-03-fetch-config.md`; Rule: the fetch child edits the network sentence first; the tool child adds the tool-list entry afterwards; the same order applies to `README.ko.md`.
- `apps/codemap-search/src/tools/synonyms.rs` (proposed) — Children: `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`, `docs/briefs/2026-10-17-feat-rterms-05-overlay.md`; Access: serialized; Owner: `docs/briefs/2026-10-17-feat-rterms-04-mcp-tool.md`; Rule: the overlay child changes rendering only after the tool child's handler is merged.

## Shared Constraints
- Dataset identity: id `rts5-75e02862daa29308`, schema `rt-sense/5`, uncompressed size 31,715,328 bytes, SHA-256 `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced`.
- Release identity: tag `related-terms-data-rts5-75e02862daa29308`, asset `rts5-75e02862daa29308.tar.zst` plus `.sha256`, URL `https://github.com/buYoung/mcp/releases/download/related-terms-data-rts5-75e02862daa29308/rts5-75e02862daa29308.tar.zst`; the archive holds only `dataset.sqlite3`.
- The data release is created with `--latest=false`, because `install.sh` installs from `releases/latest` by default.
- Dataset updates stay pinned to codemap-search releases (user decision): a codemap-search upgrade that keeps the same pinned dataset id reuses the files already under `<global_dir>/related-terms/<dataset_id>/` with no download; only a release that pins a new id downloads into a new folder. A changed index version rebuilds indexes on the existing file without downloading.
- `related_terms.enabled` defaults to `false`; when off there is no `synonyms` tool, no download, no local index, no codebase evidence, and no network request.
- The downloaded copy lives at `<global_dir>/related-terms/rts5-75e02862daa29308/` (`$CODEMAP_HOME` or `~/.codemap`); copies of a user-supplied `dataset_path` live at `<global_dir>/related-terms/local/<dataset_id>/`, and the user's file is never modified.
- Downloading and preparation each hold a cross-process lock; a prepared file is used only after its ready marker exists, and a copy is renamed into place only when complete.
- Codebase filtering decides on each sense's full word set; until the user confirms thresholds, the shipped behavior moves unsupported senses back instead of removing them.
- The Rust lookup uses only the sense dataset: no `rtr1`, no `rtpoc3`, no vectors; Python PoC code is a behavior reference and evaluation tool, not code to port.
- The release, the crate, and the repository's related-terms documents carry no third-party dictionary data, attribution, or CC BY notice; the local `rtr1` schema column `license_spdx` stays because it is a data format, not a notice.
- Do not modify tracked dataset files under `apps/codemap-search/experiments/related-terms-poc/datasets/`.
- New tests are limited to `tests/e2e/` core contracts: download integrity, feature-off has no side effects, tool schema and output caps, evidence updates after file changes. PoC comparison and 12-repository checks are evaluation scripts under `apps/codemap-search/experiments/related-terms-poc/sense-eval/`.
- The `synonyms` tool is read-only and never blocks the sequential MCP loop on download, indexing, or evidence work.
- Run `cargo check --manifest-path apps/codemap-search/Cargo.toml` after every Rust change.
- Evidence logs go in `docs/related-terms/integration-log/`, Korean, "~다" style.
- Publishing a release, pushing a tag, or any other outward-facing action needs the user's explicit approval at that moment.
- [deferred] Quality improvements after integration: vector-based sense order, headword spelling (K7), sense order (K9), symbol-input noise (stage 15).
- [deferred] Per-scope evidence in monorepos, config keys for output caps, proxy settings, and removing local folders of datasets that a newer release no longer pins.

## Global Acceptance Criteria
- [x] `cargo check --manifest-path apps/codemap-search/Cargo.toml` exits 0 on the final branch.
- [x] `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests` exits 0, and its output lists passed tests under each of `related_terms_fetch` (at least 6), `related_terms_tool` (at least 4), and `related_terms_overlay` (at least 2).
- [x] With a fresh `CODEMAP_HOME` and default config, a `codemap-search mcp` session lists no `synonyms` tool and leaves no `related-terms` folder.
- [ ] With a fresh `CODEMAP_HOME`, `related_terms.enabled = true`, and no `dataset_path`, a branch-built binary downloads the public asset, the extracted file's SHA-256 equals `90b77386962e7037969271f449e8f5253c38059f09bc10c91a415267669f9ced` before indexing, and `synonyms` with `terms: ["token"]` returns at least one sense.
- [ ] `gh api repos/buYoung/mcp/releases/latest --jq .tag_name` returns a tag starting with `codemap-v`.
- [x] `cargo package --list --allow-dirty --manifest-path apps/codemap-search/Cargo.toml` lists no `.sqlite3` or `.tar.zst` file. 미커밋 작업 트리의 목록 검사이며 게시하지 않는다.
- [x] All five evidence logs exist in `docs/related-terms/integration-log/`, and `docs/related-terms/integration-log/05-overlay.md` records the threshold decision's deferred status and the disabled removal switch until the user resumes that decision.
- [x] 기능 구현 후 Docker에서 새로 clone·index한 12개 고정 저장소의 자동 평가 결과가 있다. 36개 저장소·근거 범위 조합, 3개 적용 모드, 별도 클론의 브랜치 전환 검증을 기록하며, 실패·시간 초과를 성공으로 세지 않는다.

## Open Questions
- None — the user decided pinned dataset updates and deletion of license notices; child-level threshold confirmation stays in `docs/briefs/2026-10-17-feat-rterms-05-overlay.md`.
