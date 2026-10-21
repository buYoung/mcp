# [feat] Add the opt-in synonyms MCP tool

## Work Type
feat

## Current State (As-Is)
- [confirmed] As of `e473b5f48` on `feat/related-terms-lookup` (inspected 2026-10-17), tools are listed in `list_tools()` in `apps/codemap-search/src/tools/mod.rs`, each with `annotations` `readOnlyHint: true` and an `openWorldHint` value, and `_meta` `anthropic/maxResultSizeChars` set from the client output limit — Evidence: `list_tools()` and the line `tool["_meta"] = serde_json::json!({ "anthropic/maxResultSizeChars": limit });`.
- [confirmed] `tools/call` dispatch lives in `apps/codemap-search/src/mcp/mod.rs`; MCP-facing helpers return `Result<String, (i64, String)>` and failures become `isError` results — Evidence: `apps/codemap-search/AGENTS.md` sections "MCP protocol and tool contract boundary" and "Result shapes".
- [confirmed] Tool descriptions are Markdown files under `apps/codemap-search/src/tools/instructions/tools/` (`search.md`, `read.md`, ...), and `initial_instructions` guidance must keep annotations and monorepo descriptions intact — Evidence: that directory; AGENTS.md route "Navigation guidance route".
- [confirmed] The default client output cap is 100,000 bytes — Evidence: `apps/codemap-search/src/config/output.rs`, client cap defaults.
- [confirmed] The MCP loop is a sequential current-thread line reader; indexing runs on the `codemap-indexer` thread — Evidence: AGENTS.md section "Sequential protocol with off-thread indexing".
- [confirmed] The agent evaluation used a shell wrapper and `usage-short.md`; with it, call rates were 83% (haiku) and 92% (luna), tool output 16,781 and 46,414 characters, "답에 쓴 출력" 0.39 and 0.30; the compact wording without "when to use" dropped call rates to 33–42% — Evidence: `docs/related-terms/agent-usage-eval.md` section 3 result table; `apps/codemap-search/experiments/related-terms-poc/sense-eval/agent-usage/usage-short.md`.
- [confirmed] The evaluation driver is `apps/codemap-search/experiments/related-terms-poc/sense-eval/agent_usage_eval.py`, running agents with MCP turned off and a `synonyms` shell command — Evidence: `docs/related-terms/agent-usage-eval.md` table row "사용법".
- [inferred] The lookup module from the lookup child exposes preparation and batch lookup functions plus a per-call term cap, and the fetch module from the fetch child exposes a fetch function with ready / in-progress / failed statuses and the `related_terms` config accessors — Confirm by: reading `docs/related-terms/integration-log/01-lookup.md` and `docs/related-terms/integration-log/03-fetch-config.md` in Stage 1.

## Desired Outcome (To-Be)
- With `related_terms.enabled = false` (default), `tools/list` has no `synonyms`, no background work starts, no `related-terms` folder is created, and no network request is made.
- With the feature on, server start launches a background task: fetch (or use `dataset_path`) → prepare indexes → ready; the MCP loop never waits for it.
- `synonyms` is a read-only tool that takes `terms` (identifiers or multi-word expressions), lets the server split identifiers, and returns per input: the resolved inputs, senses as `field — topic`, and per sense the direct relations, near terms, and same-topic words.
- Output obeys caps (initially 6 senses, 20 direct, 10 near, 10 same topic) with "showing M of N" notes, and stays under the client cap.
- Status answers distinguish downloading, unavailable (with reason; next call retries), and not in the dictionary.
- The tool description is based on `usage-short.md` without the identifier-splitting instruction.
- An MCP-based agent evaluation reports call rate, tool-output use, and output size next to the shell-CLI baseline.

## Scope
### In Scope
- Start-up orchestration of fetch and preparation when enabled, with a shared readiness state and retry on the next call after a failure.
- Tool registration in `list_tools()` only when enabled, dispatch in `src/mcp/mod.rs`, handler module, input schema with `terms` (min 1, max = the per-call cap from the lookup log).
- Output rendering, caps, truncation notes, and statuses.
- Description file `apps/codemap-search/src/tools/instructions/tools/synonyms.md` and, when enabled, a `synonyms` line in tool-selection guidance.
- README (en/ko) tool list entry.
- e2e tests for: feature off has no side effects; tool schema and output caps.
- Agent evaluation driver change to call the MCP tool from a source-built binary.
- Evidence log `docs/related-terms/integration-log/04-mcp-tool.md`.
### Out of Scope
- [hard] Do not change the `search` tool's ranking, schema, or output.
- [hard] Do not change existing tools' annotations, descriptions, or `initial_instructions` text when the feature is off.
- [hard] Do not add codebase evidence filtering or reordering; that is the overlay child.
- [hard] Do not add config keys for caps or the URL.
- [hard] Do not add test files beyond the two e2e areas above.
- [deferred] Config keys for synonyms output caps.
- [deferred] Quality improvements (vectors, K7, K9, stage-15 noise).

## Constraints
- Annotations: `readOnlyHint: true`, `openWorldHint: false`; the tool call itself reads only the local dataset, and downloading happens in the start-up task.
- The background task runs on its own named thread and holds no lock the MCP loop needs while downloading or indexing.
- When disabled, construct nothing: no thread, no folder, no file handle, no HTTP client.
- When `dataset_path` is set, pass the user-owned flag to preparation so it indexes a copy.
- Output obeys `apps/codemap-search/AGENTS.md` "Output budgets": emit truncation notes instead of silently dropping words, and keep the full response under the client cap.
- Caps start at 6 senses, 20 direct, 10 near, 10 same topic; change them only with evaluation evidence recorded in the log.
- New e2e tests cover only "feature off has no side effects" and "tool schema and output caps".
- Run `cargo check --manifest-path apps/codemap-search/Cargo.toml` after Rust changes.
- Evaluate with a binary built from this branch, not the installed codemap-search, because the installed binary exposes a different schema (`docs/related-terms/codemap-integration.md` section 8).
- The evidence log is Korean in "~다" style, like other `docs/related-terms` documents.

## Related Files / Entry Points
- `docs/related-terms/integration-log/01-lookup.md` (proposed) — read first: lookup API, per-call term cap, latency.
- `docs/related-terms/integration-log/03-fetch-config.md` (proposed) — read first: fetch API, statuses, config keys.
- `apps/codemap-search/src/tools/mod.rs` — start at `list_tools()` to add the conditional `synonyms` entry with annotations and `_meta`.
- `apps/codemap-search/src/mcp/mod.rs` — `tools/call` dispatch and server start-up where the background task is launched.
- `apps/codemap-search/src/main.rs` — `Commands::Mcp` start path, if the task must start there.
- `apps/codemap-search/src/tools/instructions/tools/search.md` — description style to follow.
- `apps/codemap-search/experiments/related-terms-poc/sense-eval/agent-usage/usage-short.md` — base text for the description.
- `apps/codemap-search/src/config/output.rs` — client cap.
- `apps/codemap-search/src/tools/synonyms.rs` (proposed) — handler and rendering.
- `apps/codemap-search/src/tools/instructions/tools/synonyms.md` (proposed) — tool description.
- `apps/codemap-search/tests/e2e/mcp.rs` — existing MCP contract tests to keep passing.
- `apps/codemap-search/tests/e2e/related_terms_tool.rs` (proposed) — new e2e tests.
- `apps/codemap-search/experiments/related-terms-poc/sense-eval/agent_usage_eval.py` — evaluation driver to switch to the MCP tool.
- `docs/related-terms/agent-usage-eval.md` — baseline numbers to report against.
- `docs/related-terms/integration-log/04-mcp-tool.md` (proposed) — evidence log handed to the overlay child.

## Execution Plan
### Stage 1 — Wire start-up behind the flag
- Starts when: `docs/related-terms/integration-log/01-lookup.md` and `docs/related-terms/integration-log/03-fetch-config.md` exist, and `cargo check --manifest-path apps/codemap-search/Cargo.toml` exits 0 with both modules present.
- Work: Launch the background fetch → prepare task at server start only when `related_terms.enabled` is true, keep a readiness state the tool handler can read, and retry on the next call after a failure.
- No-op when: None — no start-up wiring for related terms exists before this child.
- No-op handoff: None — this child must add the wiring and the tool.
- Deliverable: Start-up orchestration and readiness state in the MCP server.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the workspace with the orchestration; Expected: exit 0.
- Ends when:
  - [ ] With the feature on and `dataset_path` pointing at a copy of the rts5 file, the readiness state reaches ready and the indexed copy appears under `<CODEMAP_HOME>/related-terms/local/rts5-75e02862daa29308/`, not in the download folder.
  - [ ] With the feature off, a server session leaves `<CODEMAP_HOME>` without a `related-terms` folder.
- Handoff: Stage 2 receives the readiness state API.
- Replan when: Preparation on the rts5 copy takes longer than the e2e helpers' index wait; record the time and make the tool answer "preparing" instead of blocking.

### Stage 2 — Add the tool
- Starts when: Stage 1's readiness state exists.
- Work: Register `synonyms` only when enabled, dispatch it, render answers with caps, truncation notes, and statuses, and add the description and README entry.
- Deliverable: Handler `apps/codemap-search/src/tools/synonyms.rs` (proposed) and description `apps/codemap-search/src/tools/instructions/tools/synonyms.md` (proposed).
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: the workspace with the tool; Expected: exit 0.
- Ends when:
  - [ ] A `tools/call` for `synonyms` with `terms: ["appVersion", "abort signal", "token"]` returns three input sections, and `appVersion` lists `app version` and its words as resolved inputs.
  - [ ] A request with more terms than the cap fails with an `isError` result naming the cap.
- Handoff: Stage 3 receives the working tool.
- Replan when: The rendered answer for a common word exceeds the client cap even with the initial caps; lower caps and record the measured size.
- Worker decision: Exact output layout and wording, status message text, and where the background task is launched (`main.rs` or `src/mcp/`).

### Stage 3 — Prove the contract
- Starts when: Stage 2's tool answers in an in-process server.
- Work: Add e2e tests: with the feature off, `tools/list` lacks `synonyms` and no `related-terms` folder appears; with the feature on and a small fixture `rt-sense/5` dataset built in the test, the schema has `readOnlyHint: true`, `openWorldHint: false`, and the `terms` limits, caps produce truncation notes, and a not-ready dataset yields the downloading status.
- Deliverable: Test module `apps/codemap-search/tests/e2e/related_terms_tool.rs` (proposed) registered in `tests/e2e/mod.rs`.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests related_terms_tool`; Inputs: the new test module; Expected: exit 0 and `test result: ok` with at least 4 passed tests whose names contain `related_terms_tool` (a filter that matches nothing also exits 0).
- Ends when:
  - [ ] `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests mcp` exits 0 (existing MCP contract tests unchanged).
- Handoff: Stage 4 receives the passing tests.
- Replan when: The existing `tools/list` e2e assertions hard-code the tool count in a way that the conditional tool breaks; adjust only for the enabled case and record it.

### Stage 4 — Evaluate agents through MCP
- Starts when: Stage 3's tests pass.
- Work: Measure expected-field hits on the 224 eval words in the lookup result before caps and in the final tool answer after caps; switch the agent evaluation driver to give agents the `synonyms` MCP tool from a branch-built binary with `dataset_path` set to the rts5 file, run the same identifiers and models as the shell baseline, and write the evidence log.
- Deliverable: Evidence log `docs/related-terms/integration-log/04-mcp-tool.md` (proposed) with the tool schema, output layout, statuses, caps, start-up entry point, expected-field hits before and after caps (with the words the caps cut), and an evaluation table (call rate, tool-output use, tool output characters, prompt tokens per model) next to the `usage-short.md` shell rows.
- Verify: `bounded inspection of docs/related-terms/integration-log/04-mcp-tool.md`; Inputs: the log file; Expected: an evaluation row for each model with run count, call rate, tool-output use, and output characters.
- Ends when:
  - [ ] Each model's call rate is above 42% (the `usage-compact.md` level); otherwise the description was revised and re-evaluated, and both runs are in the log.
- Handoff: The overlay child (`docs/briefs/2026-10-17-feat-rterms-05-overlay.md`) receives `docs/related-terms/integration-log/04-mcp-tool.md`, the tool handler, and the evaluation driver.
- Replan when: The evaluation agent cannot load an MCP server; record the blocker and report to the parent instead of falling back to the shell wrapper.

## Side Effect Checkpoints
- [ ] With default config, the `tools/list` result is identical to the one before this child (same names, descriptions, annotations).
- [ ] Existing MCP e2e tests pass: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests mcp` exits 0.
- [ ] Existing search e2e tests pass: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests search` exits 0.
- [ ] While the background task downloads or indexes, a `search` call still returns without waiting for it.
- [ ] The tracked rts5 file's SHA-256 is unchanged after the agent evaluation.

## Acceptance Criteria
- [ ] Default config: no `synonyms` tool, no background task, no `related-terms` folder, no network request.
- [ ] Enabled config: `synonyms` returns senses and related words for identifiers and multi-word expressions, with caps and truncation notes, under the client cap.
- [ ] The tool reports downloading, unavailable (with reason), and not-in-dictionary states distinctly.
- [ ] The evidence log compares MCP evaluation results with the shell `usage-short.md` baseline for both models.

## Open Questions
- None — the user decided a separate read-only tool, server-side splitting, output caps, default off, and core e2e tests; remaining choices are bounded in Constraints and Worker decision.
