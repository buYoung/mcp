# [feat] Integrate optional Jev stages into native MCP calls

## Work Type
feat

## Current State (As-Is)
- [confirmed] MCP dispatch currently invokes synchronous search/overview handlers from a sequential Tokio loop — Evidence: `apps/codemap-search/src/mcp/mod.rs::run`, `handle_request`, and `handle_request_inner`.
- [confirmed] Each request pins config/redaction, then applies response masking/caps and call accounting — Evidence: `McpServer::handle_request`.
- [confirmed] Overview updates active workspace scope and search supplies pending source observations — Evidence: the overview/search dispatch arms in `handle_request_inner`.
- [confirmed] Search rejects unsupported arguments and overview already uses query as a path alias — Evidence: `tools/search/arguments.rs::validate` and `tools/overview.rs::run`.
- [confirmed] Config uses repository/global/default resolution, additive synchronization, and generated documentation — Evidence: `src/config.rs::ResolvedConfig` and the Configuration boundary in `apps/codemap-search/AGENTS.md`.
- [confirmed] The standalone CLI search currently prints matching paths and is a separate code path — Evidence: `apps/codemap-search/src/main.rs::Commands::Search` and its dispatch.

## Desired Outcome (To-Be)
- Activate #1/#2 through native MCP tool handling with independent default-off configuration.
- Accept explicit original task intent as an additive `task_query` argument without overloading existing query/path semantics.
- Keep request-local policy, redaction, final output caps, workspace updates, and source accounting valid across asynchronous Jev work.
- Provide operator setup and Rust reuse guidance so the common module is usable without a Python proxy.

## Scope
### In Scope
- Add Jev config fields, defaults, lenient parsing, validation, config-version synchronization, and generated documentation together.
- Wire shared client lifecycle and asynchronous adapter evaluation into existing MCP handling.
- Add task_query schema/validation and mode-aware tool descriptions, initial instructions, and examples.
- Add integration regressions for disabled/no-key/no-intent paths, mode independence, config changes, and fallback behavior.
- Keep normal CLI commands compatible and document the direct Rust module example.
### Out of Scope
- [hard] Do not add a new MCP server, standalone process, new tool family, or Python execution dependency.
- [hard] Do not change the meaning of search.query or overview path aliases, or infer task intent from an earlier request.
- [deferred] New CLI-facing Jev flags/subcommands, altered path-only CLI search output, and automatic Jev filtering of read/grep/find are outside this MCP adoption slice.
- [deferred] General concurrent MCP dispatch and new cancellation-notification protocol behavior are separate work; retain the current notification contract.

## Constraints
- Work on `feat/codemap-jev`, created from `main` / `origin/main` at `97e3ebc8e`. Preserve the existing untracked Jev PoC directory. Do not stage, commit, merge, publish, or run a release as part of this plan.
- Keep Jev inside the existing codemap-search Rust crate as an independently callable common module. Do not introduce a separate crate, Python runtime, stdio proxy, Unix-socket broker, or another MCP server.
- Keep overview recommendation and search filtering independently configurable and disabled by default. Enablement alone must not trigger background API requests.
- Use the original task intent supplied explicitly by the caller. Do not read an agent transcript, hardcode the benchmark question, or reuse another request's task intent.
- Preserve existing calls with their existing arguments: MCP content/error envelopes, path aliases, workspace selection, index readiness notices, output limits, and live read/find/grep behavior.
- Use new regression tests and minimal non-sensitive fixtures where needed; the user approved this scope. Keep CI offline through an injected evaluator or transport. Do not add lint/formatter setups.
- Keep the disposable PoC API key out of source, fixtures, logs, and documents. Resolve operator credentials at the integration boundary. Do not reuse the old key or run new paid API benchmarks without a fresh explicit execution instruction.
- Preserve existing redaction semantics for model-bound data and returned text. Keep original source files and the committed index unchanged. Do not send raw index metadata around the presentation-redaction boundary.
- Report API input/output usage, elapsed time, and applied/bypassed/fallback outcomes separately. Do not claim deterministic scores, general accuracy, or a Rust speedup from the historical Python measurements.
- Use `[analysis.jev]` as the proposed config group with `overview_enabled`, `search_filter_enabled`, `model`, `api_key_env`, `timeout_ms`, `max_in_flight_requests`, `request_spacing_ms`, `max_batch_bytes`, `pool_idle_timeout_ms`, and `search_filter_min_unrelated_probability`. Synchronize transport defaults with child 01 and the filter threshold with child 03. Default `api_key_env` to `TYPESAFE_API_KEY`; resolve credentials at the host boundary, not in the generic evaluator.
- Set `search_filter_min_unrelated_probability=0.70` as an explicitly provisional default; accept only finite `0.5 < value <= 1.0`, normalize invalid values through the existing config behavior, and pass the immutable effective value to the final Rust retention policy. Do not make it a generic HTTP-runtime setting or equate it with confidence/accuracy.
- For evaluated #1 calls, preserve `recommendation_status=matched`, `no_match`, or `insufficient_evidence` separately from applied/bypassed/fallback diagnostics. Zero recommendations must remain actionable through existing search/read/grep/find guidance and must not claim source-level absence.
- Keep config serde/defaults, parser normalization, generated comments, version synchronization, and user-facing activation together as an atomic config contract.
- Preserve the existing sequential MCP model; cancellation support in the reusable evaluator does not imply new protocol-level notification handling.

## Related Files / Entry Points
- `apps/codemap-search/src/mcp/mod.rs` — Own runtime lifecycle, async integration, final response handling, and call accounting.
- `apps/codemap-search/src/tools/mod.rs` — Extend schemas and conditional navigation guidance while preserving existing names/annotations.
- `apps/codemap-search/src/tools/instructions/` — Update initial usage guidance for explicit task intent and fallback.
- `apps/codemap-search/src/tools/search/arguments.rs` — Accept and validate the new optional argument before side effects.
- `apps/codemap-search/src/config.rs` — Keep defaults, parser, validation, synchronization, and generated config aligned.
- `apps/codemap-search/docs/configuration.md` — Document independent opt-in modes, credentials, deadlines, and input transmission.
- `apps/codemap-search/README.md` — Provide native usage and common-module example instructions.
- `apps/codemap-search/src/main.rs` — Preserve current runtime flavor and CLI command contracts.
- `apps/codemap-search/validation/jev-native/02-overview.md` (proposed) — Consume the overview adapter boundary.
- `apps/codemap-search/validation/jev-native/03-search-filter.md` (proposed) — Consume the structured search filter boundary.
- `apps/codemap-search/validation/jev-native/04-integration.md` (proposed) — Publish integrated activation and compatibility evidence.

## Execution Plan
### Stage 1 — Define activation and request contracts
- Starts when: `apps/codemap-search/validation/jev-native/02-overview.md` and `apps/codemap-search/validation/jev-native/03-search-filter.md` exist with callable adapter APIs and successful offline checks.
- Work: Add an analysis.jev configuration section consistent with current grouping. Use separate overview_enabled and search_filter_enabled flags, both false, plus the documented Noul threshold default and range. Add optional task_query to overview/search while preserving search.query and all path aliases. Define explicit missing/empty intent and absent-key bypass behavior, immutable per-request settings, and non-secret diagnostics.
- No-op when: Native MCP integration already implements both accepted modes and passes config/schema, disabled-output, and fallback acceptance checks.
- No-op handoff: Write the verified settings, invocation matrix, and check results to `apps/codemap-search/validation/jev-native/04-integration.md` (proposed), then let the parent start whole-feature verification.
- Deliverable: An integrated config/schema contract with setup examples and tests.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::config`; Inputs: Existing config fixtures plus Jev defaults, threshold 0.70/0.90 overrides, invalid/non-finite threshold values, and additive-sync cases; Expected: Exit 0 with existing keys preserved, both modes false by default, and parser/template/schema agreement on the effective threshold
- Ends when:
  - [ ] Old calls retain their behavior and no key or task intent is required for normal offline use.
  - [ ] A non-string task_query is rejected through normal argument validation and blank intent does not start evaluation.
  - [ ] The documented API key source is an environment variable name, never a secret written by config sync.
- Handoff: Stage 2 receives the activation/schema contract.
- Replan when: The activation design requires a mandatory argument, credential persistence, or a default-on mode: stop and return the behavior change to the parent.
### Stage 2 — Wire asynchronous native evaluation
- Starts when: The new settings and task_query validation are defined and both adapters are available.
- Work: Use a shared evaluator lifecycle and await the selected adapter inside the existing request boundary. Retain request-local config/redaction, the effective Noul threshold, and the owned snapshot/evidence across awaits. Avoid blocking HTTP on Tokio or holding index/config locks during network work. Preserve workspace updates, recommendation status, response redaction/caps, and post-filter source observations before call recording.
- Deliverable: Native overview/search responses with applied/bypassed/fallback diagnostics, recommendation status, effective policy version/threshold, and unchanged MCP envelopes.
- Verify: `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::mcp`; Inputs: Existing protocol cases, independent modes, zero-match overview, and Noul=0.80 on an unprotected complete body under configs 0.70/0.90 including a delayed config change; Expected: Exit 0, omission only under 0.70, each in-flight call retaining its captured setting, valid JSON-RPC stdout, and no external requests
- Ends when:
  - [ ] Neither feature implicitly activates the other and both may be configured independently.
  - [ ] Missing credentials/intent, deadline expiry, invalid API answers, and readiness bypass preserve the base tool result.
  - [ ] Existing caller cancellation/deadline inputs are not replaced by a longer module default.
  - [ ] Read/find/grep and unrelated MCP methods remain outside Jev evaluation.
- Handoff: Stage 3 receives the wired native request path.
- Replan when: The implementation needs general concurrent dispatch or loses config/redaction scope across awaits: stop, redesign the bounded request integration, and re-verify before proceeding.
### Stage 3 — Document and prove operator use
- Starts when: Native integration passes its targeted compatibility checks.
- Work: Finish config templates and usage guidance with explicit task_query examples, independent enablement, credential setup, transmitted-data scope, byte-versus-token limits, no-match/uncertainty/fallback behavior, and the provisional Noul threshold. Keep tool annotations accurate and reflect external provider access. Document the direct `--mock` example and opt-in `--live` invocation, including the three primitive meanings and unperformed quality calibration.
- Deliverable: `apps/codemap-search/validation/jev-native/04-integration.md` (proposed) containing canonical keys/defaults, complete sample calls, lifecycle decisions, compatible response contracts, and check results.
- Verify: `cargo check --manifest-path apps/codemap-search/Cargo.toml`; Inputs: The integrated library/binary and README/configuration examples inspected against schemas; Expected: Exit 0 and every documented config key and invocation matches the implemented parser/schema
- Ends when:
  - [ ] Users can enable either mode with configuration and environment setup without replacing the MCP executable.
  - [ ] The default installation still works without a key, Python, or network access.
  - [ ] Documentation distinguishes Jev judgment from the host preservation rules.
- Handoff: The parent and verification child receive `apps/codemap-search/validation/jev-native/04-integration.md`.
- Replan when: Examples require a new CLI output contract or unsupported configuration: correct the documentation or return the scope change to the parent.

## Side Effect Checkpoints
- [ ] Keep initialize negotiation, supported protocol versions, notification no-response behavior, tool names, and JSON-RPC error codes compatible.
- [ ] Keep the existing `McpServer::new` and current callers usable or provide a compatibility constructor with evaluation disabled.
- [ ] Keep config precedence, per-request pinning, additive sync, and user comments intact.
- [ ] Ensure final response caps include Jev additions and status text rather than bypassing the existing cap check.
- [ ] Ensure logged source observations and response bytes describe the final delivered response.
- [ ] Keep normal CLI codemap/search/index commands and existing cargo packaging behavior intact.

## Acceptance Criteria
- [ ] Configured #1 with explicit task_query reaches the Rust overview adapter and configured #2 reaches the Rust filter adapter.
- [ ] The four enablement combinations work without changing required arguments or existing offline behavior.
- [ ] Jev failures and unavailable evidence yield the original bounded result and an inspectable fallback reason.
- [ ] The configured Noul threshold reaches the final retention policy and stays fixed for an in-flight request; no-match overview returns no forced recommendation.
- [ ] Config templates, schemas, initial instructions, and operator docs agree on actual activation behavior.
- [ ] No proxy executable is needed and the integration handoff records actual compatibility results.

## Open Questions
- None — The user selected an internal common module, independent default-off modes, and necessary regression tests. Remaining implementation choices are bounded in the stages.
