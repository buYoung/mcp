# Brief Set: Read-only fd-style navigation and tokei statistics

## Purpose
- Deliver fd-style path discovery and configuration-controlled indexed-code statistics while preserving the current codemap-search read-only tool contract and existing indexing lifecycle.
- Coordinate two independently verifiable features that share MCP schemas and public documentation without introducing a new runtime write capability.

## Child Briefs
- [ ] `docs/briefs/2026-09-18-feat-fd-tokei-01-find.md` — Extend read-only find with fd-style navigation; exists because path discovery needs its own argument, traversal, permission, and ordering contract.
- [ ] `docs/briefs/2026-09-18-feat-fd-tokei-02-stats.md` — Add read-only tokei statistics to overview; exists because indexed code counting needs its own dependency, source identity, memory cache, and numeric-output contract.

## Execution Order
- Wave 1 — `docs/briefs/2026-09-18-feat-fd-tokei-01-find.md`: Start: current checkout and approved feature/write boundaries are available; Deliverable: integrated find options with preserved defaults and existing-check/read-only evidence; Location: `docs/briefs/evidence/2026-09-18-fd-tokei-find.md` (proposed); Done: the handoff records passing package cargo check and non-empty tools/MCP checks plus evaluated find acceptance criteria; Handoff: child 02 receives the final find schema and shared-file edits from the handoff.
- Wave 2 — `docs/briefs/2026-09-18-feat-fd-tokei-02-stats.md`: Start: child 01 handoff is available and its final contract is integrated; Deliverable: embedded tokei statistics with indexed-only scope, memory-only cache, and shared-feature verification evidence; Location: `docs/briefs/evidence/2026-09-18-fd-tokei-stats.md` (proposed); Done: the handoff records passing package cargo check and non-empty codemap/MCP/tools checks plus evaluated statistics acceptance criteria; Handoff: the parent receives both evidence files to evaluate global completion.

## Dependencies
- Predecessor: `docs/briefs/2026-09-18-feat-fd-tokei-01-find.md`; Deliverable path: `docs/briefs/evidence/2026-09-18-fd-tokei-find.md` (proposed); Format: Markdown containing revision, changed shared files, final option/default table, actual verification commands/results, observed MCP calls, read-only call-path evidence, and limitations; Successor: `docs/briefs/2026-09-18-feat-fd-tokei-02-stats.md`; Starts when: the handoff exists and find acceptance is complete without unresolved blocking failures; Verify: `Inspect the find handoff against src/tools/mod.rs find registration and src/tools/find.rs final consumers`; Inputs: the handoff and both files under apps/codemap-search; Expected: all advertised options/defaults match integrated code and recorded required checks pass on the predecessor state.

## Parallelization
- Must not overlap: `docs/briefs/2026-09-18-feat-fd-tokei-01-find.md` and `docs/briefs/2026-09-18-feat-fd-tokei-02-stats.md` — serialize find first, then statistics, because both edit the MCP schema and README tool contracts. Join when: child 02 has preserved the predecessor's additions and recorded passing tools/MCP checks on the combined implementation.

## Conflict Hotspots
- `apps/codemap-search/src/tools/mod.rs` — Children: `docs/briefs/2026-09-18-feat-fd-tokei-01-find.md`, `docs/briefs/2026-09-18-feat-fd-tokei-02-stats.md`; Access: serialized; Owner: `docs/briefs/2026-09-18-feat-fd-tokei-01-find.md`; Rule: child 01 completes find matching/schema edits before child 02 updates overview guidance and preserves all find additions without adding a statistics request flag.
- `apps/codemap-search/README.md` — Children: `docs/briefs/2026-09-18-feat-fd-tokei-01-find.md`, `docs/briefs/2026-09-18-feat-fd-tokei-02-stats.md`; Access: serialized; Owner: `docs/briefs/2026-09-18-feat-fd-tokei-01-find.md`; Rule: child 01 publishes find behavior first and child 02 retains it while documenting statistics enabled by default at repository and monorepo project roots.
- `apps/codemap-search/README.ko.md` — Children: `docs/briefs/2026-09-18-feat-fd-tokei-01-find.md`, `docs/briefs/2026-09-18-feat-fd-tokei-02-stats.md`; Access: serialized; Owner: `docs/briefs/2026-09-18-feat-fd-tokei-01-find.md`; Rule: apply the same ownership order as the English README and verify both language versions at the join.

## Shared Constraints
- Apply the user's 2026-09-18 decisions: statistics cover the current indexed-file population only, and the new features follow the existing codemap-search write boundary while existing indexing remains unchanged.
- Implement fd-style capabilities using existing embedded `ignore`/`regex` support. Do not add the fd CLI, `fd-find`, executable installation, downloads, wrappers, or subprocess calls.
- Integrate tokei as a Rust library over approved in-memory source inputs. Do not add a tokei executable dependency, independent repository walk, or arbitrary user-configuration loading.
- Add no runtime file creation, editing, deletion, rename, permission change, execution hook, report export, or persistent statistics/search cache. Use process memory for new statistics state.
- Preserve existing index/watch/recovery behavior. The approved indexing exception does not authorize storing statistics in the existing on-disk index or modifying source/configuration files as part of the new features.
- Preserve current tool names/count, the JSON-RPC text-content envelope, `readOnlyHint=true`, `openWorldHint=false`, stdout framing, and centralized response redaction.
- Preserve find defaults for glob matching, permissions, ignore/mandatory exclusions, hidden files, normalized paths, newest-100 selection, tie ordering, and empty/truncation messages. Add the new behavior only through explicit options.
- Preserve overview navigation, aliases, active workspace scope, file errors, output-format behavior, and warming/dead notices. Repository-root and selectable monorepo project-root overviews include statistics for their own indexed scope by default; `[tool_output].is_overview_stats_enabled = false` disables them. Other folder and file views remain statistics-free. This incorporates the user's project-root clarification on 2026-09-19.
- Keep `ExtractedFile::total_lines` and persisted index formats intact. New statistics must identify their indexed scope, physical-file coverage, counting convention, and incomplete/stale state.
- Use named local implementation choices and bounded investigation stages for dependency compatibility, parallel worker bounds, source identity, and cache ownership. Return material contract/scope changes to this parent before proceeding.
- Preserve all pre-existing uncommitted changes, especially dependency/configuration/MCP/redaction work observed during authoring. Recheck the actual working tree before edits rather than resetting it to the author's revision.
- Do not create or modify test files/cases, fixtures, lint/formatter setup, or verification automation without an explicit later user request. Use existing checks and bounded inspections/manual observations, and report their actual limits.
- Announce exact existing verification commands and working directories before execution. Use the repository root `/Users/buyonglee/Documents/work/private/buyong-mcp-2` for the commands in these briefs, or its equivalent root if executed in another checkout.
- Development edits, Cargo build artifacts, and the two proposed handoff records are execution artifacts, not product write capabilities. Only the three brief Markdown files are authored in this planning task; implementation and evidence artifacts are produced when the children execute.
- Keep child implementation stages cohesive: each feature's schema, final consumer, documentation, and compatibility verification complete together. Child 02 owns the combined verification join, so no status-only or verification-only third child is needed.
- If a child is already satisfied at execution time, use its declared no-op evidence route. If proof fails, stop successors/global completion, return to the responsible child for bounded correction and re-verification, then recalculate parent topology/handoffs before resuming.
- [deferred] Whole-repository statistics, extra fd filters/symlink modes, complexity scoring, eza, ast-grep, new CLI surfaces, and other integrations are excluded.

## Global Acceptance Criteria
- [ ] Both child acceptance criteria and side-effect checkpoints are complete, with evidence at the exact handoff paths and parent-only completion status updated in Child Briefs.
- [ ] Bounded inspection of final `apps/codemap-search/src/tools/mod.rs`, both tool implementations, and final `tools/list` output shows the existing tool set with additive find options and optional overview statistics, preserving the read-only annotations and JSON envelope.
- [ ] On the final combined source state, `cargo check --manifest-path apps/codemap-search/Cargo.toml` exits 0 and the existing `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::tools::`, `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::mcp::`, and `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::codemap::` each select non-zero tests and pass. Reuse child 02 evidence from that exact state rather than rerunning unchanged checks.
- [ ] The find handoff demonstrates that explicit entry type, depth, and regex options reach the returned results while omitted options retain the baseline contract.
- [ ] The statistics handoff identifies a non-empty actual indexed population, documents the numeric counting convention, and shows correct root counts/coverage, config-driven omission, monorepo behavior, and memory-only caching.
- [ ] Bounded inspection enumerates every new schema parameter, new feature entry point, cache/storage field, and newly reachable call path for both features; the enumerated population is non-empty and contains no added runtime filesystem mutation, subprocess execution, export destination, or persistent cache. Existing indexing is identified separately and remains unchanged.
- [ ] Shared English/Korean documentation and tool instructions describe the same implemented defaults, population, read-only boundary, and partial-result behavior; no fd/tokei executable installation is required or advertised.
- [ ] Actual checks, bounded inspections, manual observations, performance measurements if any, and unverified limits are recorded distinctly. No unrun test or unmeasured speedup is reported as successful.

## Open Questions
- None — the user approved the recommended integration direction, selected indexed-file statistics, and clarified that existing indexing remains while new feature writes are prohibited.

## 실행 상태 — 2026-09-19
- 구현은 `include_stats` 없이 저장소 루트와 모노레포 프로젝트 루트에 기본 통계를 붙이고, 설정으로 끄는 최종 사용자 결정까지 반영했다. 프로젝트별 집계 범위, 별칭·절대 경로, 비활성화 동작을 실제 MCP 응답으로 확인했다.
- 실행 근거: [find 기록](evidence/2026-09-18-fd-tokei-find.md), [통계 기록](evidence/2026-09-18-fd-tokei-stats.md).
- 최종 소스의 `cargo check`, codemap 15개, tools 35개가 통과했다. tools 1개는 기존 Clang 조건으로 제외됐다. MCP는 23개 통과·3개 실패이며, 실패 3개는 기준 버전 `9edd56999`에서도 재현한 기존 검색 출력 문제다.
- 모든 MCP 검증 통과라는 전역 완료 조건은 충족하지 않아 자식 및 전체 완료 표시는 남겨 둔다. 기존 검색 출력 문제를 이번 기능 구현으로 해결했다고 보고하지 않는다.
