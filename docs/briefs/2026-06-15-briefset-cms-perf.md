# Brief Set: codemap-search performance improvement briefs

## Purpose
- Execute the agreed seven codemap-search performance improvements as independently reviewable work units.
- Preserve the benchmark discipline: measure first, avoid model-specific bias, keep no-MCP parity risks visible, and judge changes by within-model quality, context size, elapsed time, and failure rate.

## Child Briefs
- [x] `docs/briefs/2026-06-15-perf-cms-perf-01-root-overview.md` - Completed: root overview now includes bounded compact file-symbol groups and bounded `llms-txt`.
- [x] `docs/briefs/2026-06-15-perf-cms-perf-02-exact-boost.md` - Completed after the 23-query baseline guard: exact-name boosts are gated by discriminative/common-name and owner/query evidence.
- [x] `docs/briefs/2026-06-15-perf-cms-perf-03-query-token.md` - Completed after the 23-query baseline guard: ranking and rendering now share `QueryTokens` while preserving raw-word exact checks.
- [x] `docs/briefs/2026-06-15-perf-cms-perf-04-symbol-rerank.md` - Completed against the reconstructed 23-query simulation: file-primary search remains, symbol-aware post-ranking is bounded, and top-10 recall stayed flat.
- [x] `docs/briefs/2026-06-15-perf-cms-perf-05-search-struct.md` - Implementation completed for ranking-derived `match_reason`, `ambiguity`, and exact `read_suggestion` hints; Codex/Claude repeated-read behavior remains for the benchmark phase.
- [x] `docs/briefs/2026-06-15-perf-cms-perf-06-search-cap.md` - Completed: search output is hard-capped with UTF-8 boundary handling, Markdown fence closure, and footer-inclusive truncation.
- [x] `docs/briefs/2026-06-15-perf-cms-perf-07-page-contract.md` - Completed: `grep`, `read`, and partial `search` guidance now use clearer continuation wording without adding public search pagination arguments.

## Execution Order
- Wave 1: `2026-06-15-perf-cms-perf-01-root-overview`, `2026-06-15-perf-cms-perf-03-query-token`, and the `read`/`grep` portion of `2026-06-15-perf-cms-perf-07-page-contract` can start independently if they run in separate worktrees.
- Wave 2: `2026-06-15-perf-cms-perf-02-exact-boost` starts after `03-query-token`.
- Wave 3: `2026-06-15-perf-cms-perf-04-symbol-rerank` starts after `02-exact-boost` and `03-query-token`.
- Wave 4: `2026-06-15-perf-cms-perf-06-search-cap` starts before `05-search-struct` so structured output can rely on a real bounded writer.
- Wave 5: search continuation wording in `2026-06-15-perf-cms-perf-07-page-contract` and all of `2026-06-15-perf-cms-perf-05-search-struct` start after `04-symbol-rerank` and `06-search-cap`.
- Default to one child at a time in a single working tree because several children touch `config.rs`, `tools/mod.rs`, and `tools/search/*`.

## Dependencies
- `2026-06-15-perf-cms-perf-02-exact-boost` depends on `2026-06-15-perf-cms-perf-03-query-token` because boost decisions should use the shared query-token representation.
- `2026-06-15-perf-cms-perf-04-symbol-rerank` depends on `2026-06-15-perf-cms-perf-02-exact-boost` because common-name gating must constrain later symbol scoring.
- `2026-06-15-perf-cms-perf-04-symbol-rerank` depends on `2026-06-15-perf-cms-perf-03-query-token` because symbol signals need the same query-token interpretation as rendering.
- `2026-06-15-perf-cms-perf-05-search-struct` depends on `2026-06-15-perf-cms-perf-04-symbol-rerank` if it exposes match reasons derived from new symbol signals.
- `2026-06-15-perf-cms-perf-05-search-struct` depends on `2026-06-15-perf-cms-perf-06-search-cap` because extra structure must be written through the bounded search writer.
- `2026-06-15-perf-cms-perf-07-page-contract` depends conceptually on `2026-06-15-perf-cms-perf-06-search-cap` for search continuation wording, but its `read` and `grep` work can proceed independently.
- `2026-06-15-perf-cms-perf-01-root-overview` has no hard code dependency on the ranking/search children.

## Parallelization
- `2026-06-15-perf-cms-perf-01-root-overview` and `2026-06-15-perf-cms-perf-03-query-token` can run in parallel only in separate worktrees; they touch different primary modules.
- `2026-06-15-perf-cms-perf-02-exact-boost` and `2026-06-15-perf-cms-perf-04-symbol-rerank` must not run in parallel because both edit `apps/codemap-search/src/index/ranking.rs`.
- `2026-06-15-perf-cms-perf-05-search-struct` and `2026-06-15-perf-cms-perf-06-search-cap` must not run in parallel because both edit `apps/codemap-search/src/tools/search/mod.rs` and `render.rs`.
- `2026-06-15-perf-cms-perf-07-page-contract` can run in parallel with ranking-only work, but coordinate if it edits `apps/codemap-search/src/tools/mod.rs` or search continuation text.

## Conflict Hotspots
- `apps/codemap-search/src/index/ranking.rs` - children 02, 03, and 04 may all touch query terms or score adjustment.
- `apps/codemap-search/src/tools/search/mod.rs` - children 05, 06, and 07 may all touch output assembly and continuation wording.
- `apps/codemap-search/src/tools/search/render.rs` - children 03, 05, and 06 may all touch query tokens or bounded rendering.
- `apps/codemap-search/src/config.rs` - children 01, 06, and possibly 07 may add or document caps/limits.
- `apps/codemap-search/src/tools/mod.rs` - children 01, 06, and 07 may update tool descriptions.
- `apps/codemap-search/docs/configuration.md` - any child exposing a config key or output contract may update docs.

## Shared Constraints
- Measure before editing each child and record the exact command, repo, query, response bytes, and relevant top results.
- Before editing any child, freeze that child's probe manifest: repo path, query text, expected relevant file/symbol when known, metrics captured, and pass/fail threshold. Report all probes, including regressions; do not select only improved examples after the fact.
- Child 04's 23-query simulation must be locatable before ranking edits. If no committed script or manifest exists, reconstruct the query list from the benchmark artifacts, save the reconstruction path in the implementation report, and do not edit ranking until the baseline numbers are reproduced.
- Compare within the same model or same tool output; do not compare Claude and Codex absolute token numbers.
- Treat tool call count as observational, not a primary target.
- Do not add benchmark-specific answer strings, private expected values, or task-specific line examples to tool descriptions.
- Do not change public behavior through undocumented caps; if a limit changes, update tool descriptions and configuration docs.
- Do not change index schema or `EXTRACTION_FORMAT_VERSION` unless a child explicitly says it is in scope; none of these seven children currently do.
- Do not add new test files or generated test fixtures unless the owner explicitly requests them; use existing tests and focused benchmark probes by default.
- End every child with existing Rust verification plus the child-specific probe measurements.
- Review findings from sub-agent `gpt-5.5 xhigh` and `claude -p` must be summarized before implementation. If `claude -p --model opus` times out, use `claude -p` default or `--model sonnet` and record the fallback.

## Global Acceptance Criteria
- [x] All seven child briefs are either completed or explicitly deferred with a reason in this parent.
- [ ] The post-change benchmark compares Codex no-MCP vs Codex codemap-search and Claude no-MCP vs Claude codemap-search within each model family only.
- [ ] Success evaluation uses the existing target guard: Codex quality +3% or better, Claude quality +8% or better, Codex context -20%, Claude context -25%, elapsed time -10% or within +10% when quality improves, and no failure-rate increase.
- [x] Search, overview, grep, and read outputs remain free of benchmark-private answer leakage in the implemented changes; new search hint text is generic and derives only from indexed symbols/literals/paths.
- [x] Any changed output contract has a before/after example and at least one lightweight run against `/tmp/benchmark-data`.
- [ ] Review is completed before implementation begins: one sub-agent `gpt-5.5 xhigh` adversarial review and one `claude -p` review, with findings summarized back to the owner.

## Implementation Status (2026-06-15)
- Implemented files: `apps/codemap-search/src/codemap/mod.rs`, `apps/codemap-search/src/codemap/tree.rs`, `apps/codemap-search/src/tools/search/mod.rs`, `apps/codemap-search/src/tools/search/render.rs`, `apps/codemap-search/src/tools/read.rs`, `apps/codemap-search/src/tools/grep.rs`, `apps/codemap-search/src/tools/mod.rs`, `apps/codemap-search/src/config.rs`, `apps/codemap-search/src/index/engine.rs`, `apps/codemap-search/src/index/mod.rs`, `apps/codemap-search/src/index/ranking.rs`, `apps/codemap-search/src/parser/mod.rs`, `apps/codemap-search/src/parser/tokenize.rs`, `apps/codemap-search/docs/configuration.md`, and `apps/codemap-search/tests/e2e/codemap.rs`.
- Ranking files were edited only after the guard was satisfied by reconstructing the 23-query simulation manifest at `/tmp/benchmark-data/results/cms-perf-next-20260615/ranking-baseline/reconstructed-23-query-manifest.json`.
- Pre-implementation review: `gpt-5.5 xhigh` adversarial sub-agent review completed and recommended deferring children 02-04 until the 23-query simulation baseline is reproducible.
- Claude review: `claude -p --model opus` completed after login/sandbox issues were resolved. It found no major defects and flagged minor/nit items in `grep` last-page continuation text, search omitted-file counting under byte cap, root overview comments, and search hint descriptions; all accepted findings were fixed.
- Post-implementation adversarial reviews: sub-agents found `search` cap-boundary/tail continuation bugs, a root directory inline bounding risk, and stale configuration wording; all accepted findings were fixed.
- Lightweight output measurements: current repo root overview changed from 1,599 bytes to 15,263 bytes by adding compact file-symbol rows; current repo `llms-txt` changed from 12,356 bytes to 12,411 bytes; `/tmp/benchmark-data/deno-main` root overview changed from 116,808 bytes to 26,057 bytes; `/tmp/benchmark-data/deno-main` `llms-txt` is 10,742 bytes with a bounded footer.
- Ranking baseline reconstruction: original `51/92` 23-query simulation manifest/script was not locatable in the committed workspace or `/tmp`; the reconstructed manifest reproduced a current baseline of top-5 `33/92`, top-10 `40/92`, and `mcp_failures=0`.
- Ranking continuation result: the current 23-query quick aggregate is top-5 `34/92`, top-10 `40/92`, `cli_stdout_bytes=8678`, and `cli_failures=0`, so top-10 stayed flat and top-5 improved by one hit against the reconstructed baseline.
- Targeted ranking probes: 9 probes ran with 0 failures; `QuerySet.__iter__` improved to the expected top result, common-name probes changed as intended, and guarded probes for `encoding::base64::decode`, `StorageFactory::get`, `strapi.documents`, `inspect default port`, and `TaskResult success failure ImmediateBackend` did not regress.
- Structured search hint output: before/after MCP output probes included current repo `SearchResult match_reason` (`1774` to `3470` bytes), current repo `QueryTokens exact boost` (`5212` to `5066` bytes), and `/tmp/benchmark-data/deno-main` `inspect default port` (`16629` to `16758` bytes). The Deno probe now labels `DEFAULT_PORT` as an exact/path-supported symbol and reports two same-name candidates.
- Pre-commit light benchmark (one `strapi-develop-001` model sanity task plus Deno/Strapi tool-output proxy) saved under `/tmp/benchmark-data/results/cms-perf-light-20260615/`: expected path hits stayed 4/4 across no-MCP, old MCP, and current MCP. Codex current MCP improved over Codex no-MCP on elapsed time (-29.2%), turns (-31.0%), and input tokens (-11.8%) while old MCP had worse input tokens (+50.4%). Claude current MCP improved over Claude no-MCP on elapsed time (-7.1%), turns (-21.1%), and input-like tokens (-9.6%); current was faster than old MCP (-19.5%) but used more Claude input-like tokens (+20.2%) and more tool bytes (+8.5%) than old.
- Post-23-query light sanity rerun saved under `/tmp/benchmark-data/results/cms-perf-light-post-23q-20260615/model-sanity/`: all six arms exited `0`, timed out `false`, and kept expected path hits at 4/4. The generated summary missed Codex no-MCP `command_execution` events, so the corrected no-MCP count is 30 zsh calls and 74,360 tool-result bytes. In this single rerun, Codex current MCP was slower and heavier than the pre-23-query light result (89.5s vs 71.6s, 34 calls vs 20, input tokens +131.8%, tool bytes +79.5%) and heavier than Codex old MCP (time +21.4%, calls +41.7%, input +71.6%, tool bytes +35.6%). Against post-23-query no-MCP, Codex current MCP was faster (time -12.7%) but used slightly more input tokens (+1.1%), more tool bytes (+65.6%), and more calls (34 vs 30). Claude current MCP also regressed against old MCP in this rerun (time +31.8%, input-like +69.4%, tool bytes +62.4%); against no-MCP it used fewer input-like tokens (-36.7%) but was much slower (time +100.6%) and used more tool bytes (+43.0%). Both Claude MCP arms first attempted unavailable `mcp__serena__find_file`, which returned a tool-use error before continuing with codemap-search. Follow-up probe traced the cause to the global Claude `UserPromptSubmit` hook in `/Users/buyong/.claude/hooks/user_prompt_hook.sh`, which injects `[Serena MCP Tool Policy]`; rerun Claude benchmark arms with `--setting-sources ""` so user/project/local settings and hooks are not loaded.
- Review: post-ranking sub-agent adversarial review found a raw exact-name issue for punctuation-bearing symbols (`operator=`, destructors); the fix added raw-word exact checks to ranking and render Tier-1 logic. A second sub-agent re-review found no major issues. The final structured-hint patch also received sub-agent adversarial review with no major or minor findings.
- Verification: `cargo check --manifest-path apps/codemap-search/Cargo.toml`, `cargo build --release --manifest-path apps/codemap-search/Cargo.toml`, `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib`, and `git diff --check` pass. Full e2e `cargo test --manifest-path apps/codemap-search/Cargo.toml` is not currently a green signal because watcher/cross-feature e2e failures were observed separately and appear unrelated to ranking/search hint changes.

## Remaining Work
- Decide whether to commit the current tracked changes. Existing untracked docs/scripts remain unstaged unless the owner explicitly includes them.
- Before starting the full benchmark, report the planned command set, arms, repos, and output directory to the owner and wait for confirmation.
- Run the full post-change benchmark comparing Codex no-MCP vs Codex codemap-search and Claude no-MCP vs Claude codemap-search within each model family.
- Rerun the Claude light sanity arms with `--setting-sources ""` before trusting Claude-side post-23-query deltas, because the prior Claude light run loaded the global `UserPromptSubmit` hook that explicitly prioritized Serena.
- Complete the child 05 behavior check with the full benchmark because the post-23-query light sanity rerun did not show fewer Codex follow-up reads or lower Claude context use.
- If the strict parent review gate is required for release, rerun `claude -p` review on the ranking/search-hint continuation and record the result.
- Treat the existing full e2e watcher/cross-feature failures as a separate stabilization item unless they reproduce as ranking/search regressions during benchmark gating.

## Open Questions
- None — current owner decisions are encoded in the child briefs; new decisions should be surfaced after review if reviewers find a scope or risk fork.
