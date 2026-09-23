# Jev native MCP integration handoff (child 04)

Status: implemented and verified offline on 2026-09-23 on the current `opus-5.5` branch
(nothing staged or committed). It connects the runtime (`01-runtime.md`), the overview
adapter (`02-overview.md`) and the search filter (`03-search-filter.md`) to MCP `overview`
and `search` calls.

## Deviations from the brief

| Brief | Implemented | Reason |
|-------|-------------|--------|
| Work on `feat/codemap-jev` from `97e3ebc8e` | Current `opus-5.5` branch | The user prohibited inspecting other branches and worktrees. No branch was created or switched. |
| Keys `overview_enabled`, `search_filter_enabled` | `is_overview_enabled`, `is_search_filter_enabled` | This matches the repository's boolean keys (`is_enabled`, `is_redact_enabled`, `is_stats_enabled`) and the user's `is`/`has` naming rule. The proposed names were never released, so no aliases exist, and an unknown key warns as usual. |
| `api_key_env` in the Jev group | Read only from the global layer. A repository value warns and is ignored; its value is never echoed. | Otherwise a cloned repository could choose which process environment variable is sent to the provider as a credential. |
| Concrete defaults are active in generated files | Every `[analysis.jev]` key is commented in both templates and in the v24 migration block, and `api_key_env` is omitted. | Active repository values would override a global opt-in in every scaffolded repository. |

## Canonical keys and defaults

The group is `[analysis.jev]`, merged per key as repo > global > default, except for `api_key_env`.
Its defaults come from the runtime constants (`crate::jev`), plus the filter constant for the threshold.

| Key | Accepted | Default |
|-----|----------|---------|
| `is_overview_enabled` | bool | `false` |
| `is_search_filter_enabled` | bool | `false` |
| `model` | versioned id `<lowercase-name>-<major>.<minor>.<patch>` (`is_versioned_model`); aliases such as `jev-latest` are rejected | `"jev-1.13.0"` |
| `api_key_env` | `[A-Z_][A-Z0-9_]*`, ≤128 bytes, global file only | `"TYPESAFE_API_KEY"` |
| `timeout_ms` | integer 1–600000 | `45000` |
| `max_in_flight_requests` | integer 1–16 | `3` |
| `request_spacing_ms` | integer 0–10000 | `300` |
| `max_batch_bytes` | integer or byte-size string, 4096–512000 | `80000` |
| `pool_idle_timeout_ms` | integer 1–600000 | `30000` |
| `search_filter_min_unrelated_probability` | finite number, `0.5 < value <= 1.0` (float or integer, not a string) | `0.70`, provisional and uncalibrated |

An invalid value warns and falls back to the lower layer, as other keys do; non-finite
thresholds are included. `JevConfig::transport_policy()` builds the runtime `TransportPolicy`.
The threshold goes only to `FilterOptions`, never to the HTTP runtime.

Schema version 24:

- **Migration.** `KeyPlacement::SectionEnd("analysis")` inserts the commented block before the next header after `[analysis]`, or at the end of the file. Uncommenting it therefore never captures `target_os`.
- **Templates.** Both templates carry an active `[analysis.jev]` header with commented keys. The English and Korean templates parse to the same TOML.
- **Header note.** The file-level note says `request_spacing_ms` also accepts 0. The previous wording was added to `legacy_comments.txt` so that v23 files are refreshed.

## Invocation contract

The arguments are an optional `task_query` on `overview` and `search`, with schema `{"type":"string","maxLength":2000}`:

- It is never required.
- `search.query` and the overview path aliases (`path`/`file_path`/`file`/`query`) keep their meaning.
- `tools::task_query::parse` runs before any lifecycle work or side effect:
  - an absent or whitespace-only value is no intent;
  - a non-string (including `null`) is `-32602 "Invalid 'task_query': expected a string with the user's original task."`;
  - more than 2,000 characters is `-32602 "Invalid 'task_query': expected at most 2000 characters."`.
- A valid value is trimmed.
- The value is used only for the call that carries it. There is no history, transcript or search-query fallback.

Decision order in `mcp::jev::JevHost` (first match wins):

| # | Condition | overview | search |
|---|-----------|----------|--------|
| 1 | Mode disabled in the pinned config | base text, no note | regular output, no note |
| 2 | No intent | `bypassed (no_task_query)` on the root view; no note on folder/file views | `bypassed (no_task_query)` |
| 3 | Readiness | `bypassed (index_warming)` when the prepared snapshot was read while warming | `bypass_reason`: `event_lookup`, `no_results`, `index_warming` |
| 4 | Scope | `bypassed (not_repository_root)` when `root_snapshot` is `None` (folder, file, workspace folder, `llms-txt`, notices) | — |
| 5 | Key variable unset or blank | `bypassed (missing_credential)` | same |
| 6 | Key not Unicode, rejected by `ApiKey::new`, or client build failed | `fallback (<label>)` with `requests=0` | same |
| 7 | Evaluation | `applied · status=matched\|no_match\|insufficient_evidence`, `bypassed (output_budget)`, or `fallback (<label>)` | `applied · omitted=N/M bodies · judged=K · threshold=T`, `bypassed (no_eligible_bodies)`, or `fallback (<label>)` |

Mode enablement is independent: with only one mode on, the other tool returns exactly its
regular output and its `task_query` is ignored.

Sample calls (see the configuration guide for setup):

```json
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"overview","arguments":{"task_query":"How does upload retry decide to stop?"}}}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"search","arguments":{"query":"retry upload size","task_query":"How does upload retry decide to stop?"}}}
```

Observed endings (offline scripted transport):

```text
[jev overview: applied · status=matched · requests=2 · input_tokens=… · output_tokens=… · elapsed_ms=… · http_ms=…]
[jev search: applied · omitted=1/2 bodies · judged=2 · threshold=0.70 · requests=1 · input_tokens=… · output_tokens=… · elapsed_ms=… · http_ms=…]
[jev search: bypassed (missing_credential) · original output unchanged]
[jev search: fallback (http_status) · original output preserved · requests=1 · input_tokens=… · output_tokens=… · elapsed_ms=… · http_ms=…]
```

## Tool metadata and guidance

- **Annotations.** `readOnlyHint` stays `true`. `openWorldHint` equals `is_overview_enabled` for overview and `is_search_filter_enabled` for search, and is `false` on every other tool.
- **Descriptions.** Each tool description gains its mode text (`instructions/tools/overview.jev.md` or `search.jev.md`, with the effective threshold formatted `{:.2}`) only while that mode is enabled.
- **`task_query` description.**
  - While the mode is enabled, it names `api.typesafe.ai`, the data sent with the intent, and the threshold.
  - While the mode is disabled, it says `ignored while analysis.jev.is_*_enabled=false`.
- **`initial_instructions`.** It appends `instructions/navigation.jev.md` for the enabled modes. The monorepo bootstrap still renders its root overview through `overview::run`, which never evaluates.
- **When changes appear.** `tools/list` reads the pinned config per request, so descriptions and hints follow reloads at the next listing.

## Lifecycle decisions

- **Startup.** `McpServer::new` is unchanged. `JevHost::from_environment()` only selects the transport source: HTTPS, or the debug-only script below. Nothing is built, read or sent at startup, and enablement alone never sends anything.
- **Evaluator.**
  - Each eligible call reads the variable named by `api_key_env` (`std::env::var`) after the bypass checks.
  - The first such call builds `Evaluator::https(&ApiKey, TransportPolicy)`.
  - The evaluator is cached and reused while the policy and key are unchanged, so permits and request spacing carry across calls.
  - A changed policy or key builds a new evaluator.
  - There are no retries and no answer cache.
  - `pool_idle_timeout_ms` closes idle connections.
- **Request model.**
  - `handle_request` and `handle_request_inner` are now `async`, and the run loop awaits them inline.
  - The stdin loop reads the next line only after the response is written, so request order, notification no-response behavior, protocol negotiation, tool names and JSON-RPC error codes are unchanged.
  - The loop stays sequential, with no concurrent dispatch and no cancellation notifications.
- **Request-local state.**
  - `config::pin_request()` and `redact::begin_request()` guards stay alive across the awaits. They are thread-local, and the `current_thread` runtime polls the future on the same thread.
  - The threshold and deadline (`now + timeout_ms`) are captured before the first await.
  - A config reload during an evaluation applies to the next request. This is verified by the delayed config-change test below.
- **Owned inputs.**
  - `overview::prepare` returns the text, the `Arc` of the rendered snapshot and the readiness observed before the read.
  - `search::prepare` returns an owned `PreparedSearch`, including the readiness captured with ranked results.
  - No index or config lock is held during network work.
- **Output caps.**
  - Overview: the recommendation section is rendered within `cap − base − 256`, and notes use `append_note` with the same 256-byte margin. The existing `redact::response` and `enforce_response_cap` then run on the final text as before.
  - Search: `render` returns `None` when the filtered text plus its note would exceed `output.search.max_bytes`. The host then logs a warning and returns the regular output.
  - Search bypass and fallback notes are appended only if they fit.
- **Accounting.**
  - Search stores `output.source_files` from the final (possibly filtered) output, so omitted bodies are not recorded as returned source.
  - Response bytes are measured on the final masked text.
  - Overview updates the active workspace scope from its arguments after evaluation, never from recommendations.
- **Logs (stderr).** Every call that ends with a note also logs one `jev evaluation` event:
  - bypassed events carry `tool`, `outcome` and `reason`;
  - fallback events add the error, requests, input and output tokens, `elapsed_ms` and `http_ms`;
  - applied events carry the same accounting plus question and policy versions; overview adds status and evaluated/recommended file counts, and search adds the model, omitted/delivered/judged counts, threshold, and completeness and retention counts.

  The key, `task_query` and evidence are not logged, and runtime errors carry counts, identifiers and statuses only.
- **CLI.** `main.rs`, CLI commands and the runtime flavor are untouched.

## Offline injection boundary

`src/mcp/jev/scripted.rs` is compiled only with `debug_assertions`:

- It is selected by the process environment variable `CODEMAP_TEST_JEV_SCRIPT` at server start. No MCP argument or config key can select it, and release builds do not contain it.
- When active, it logs a warning.
- It re-reads its JSON script for every request, records each request body as one JSON line, can delay or return a non-200 status, and answers Score, Choice or Noul questions from ordered rules. An unmatched question gets no answer.

The e2e harness:

- strips `TYPESAFE_API_KEY`, the script variable and the test key variable from every spawned server;
- isolates `CODEMAP_HOME`;
- names `CODEMAP_TEST_JEV_KEY` through the global `api_key_env` only in the Jev cases, which are `#[cfg(debug_assertions)]`.

## Operator documentation

- **`docs/configuration.md` and `configuration.ko.md`:**
  - the section row and schema 24 with its migration bullet;
  - reload timing;
  - the key rows and number/byte-size rules;
  - a new "Optional Jev judgments" section covering the modes, activation conditions, setup, sample calls, transmitted data and masking, outcome notes and reasons, Jev judgment versus host rules, the provisional threshold, time/byte/token limits, and Rust reuse (`--mock`/`--live`, the three primitive meanings, no calibration);
  - example blocks replaced by the current templates, which were checked to be identical.
- **`README.md` and `README.ko.md`:**
  - `task_query` in the tool table;
  - the API-key note in the introduction;
  - the network note beside the read-only statement;
  - a Jev section with setup, an example, data, outcomes and a link.

All documented keys, defaults, ranges, labels and limits were checked against `config/jev.rs`,
`jev/*`, `tools/search/jev.rs`, `tools/overview/jev.rs` and `tools/mod.rs`.

## Verification (2026-09-23)

| Command | Result |
|---------|--------|
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::config` | 12 passed, 0 failed |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::mcp` | 31 passed, 0 failed (150.53 s; the existing large-payload cases dominate) |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib` | 332 passed, 0 failed, 2 ignored (existing) |
| `cargo check --manifest-path apps/codemap-search/Cargo.toml` | exit 0 after the documentation changes |

New regressions:

- **Library:**
  - `config::jev`: defaults and policy, per-key independence, threshold range, transport ranges, global-only key variable;
  - `config`: template examples equal the parsed defaults, and the v24 block lands after `[analysis]` settings;
  - `tools::task_query`;
  - `tools::search::jev`: warming bypass, and a note that does not fit makes `render` return `None`.
- **`e2e::config`:**
  - modes off in the generated config and schema;
  - 0.70/0.90/repo overrides and invalid values (`nan`, `inf`, `0.5`, `0.2`, `1.5`, `"0.9"`) reaching the schema;
  - additive v23 → v24 sync that keeps comments and values, followed by no rewrite.
- **`e2e::mcp::jev`:**
  - Noul 0.8 omitted at 0.70 and kept at 0.90, with a config change during a delayed evaluation that keeps 0.70 for the in-flight call;
  - four mode combinations and hints;
  - zero-match overview with no forced recommendation and a folder bypass;
  - missing key, blank or invalid intent, 529, deadline, and unanswered question, each preserving the base output;
  - read/find/grep, `initial_instructions` and ping never evaluating.

Correction routed back from child 05:

- **Defect.** The full e2e run failed 3 `e2e::exclusions` cases: `generated_common_and_project_globs`, `v6_is_not_regenerated_on_restart` and `pre_v6_transition_and_opt_out`.
- **Cause.** They pin the current schema header in `CURRENT_CONFIG_HEADER`, which still read `# codemap-config-version: 23` after this child's v24 bump. The run reported `left: Some("# codemap-config-version: 24")`.
- **Fix.** The constant now names version 24. No production code changed, and no other test pins the current version.
- **Re-verification.** `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::exclusions` gave 4 passed, 0 failed. The full-suite rerun is recorded in `05-verification.md`.

A manual smoke run of the debug binary with a scripted transport also showed both tools'
`openWorldHint` as `true`, search `omitted=1/2`, overview `status=matched · requests=2`,
and the invalid `task_query` error.

Not run:

- live provider calls (no fresh execution instruction; the old key is not reused);
- Windows/Linux runs;
- release-build end-to-end runs of the scripted cases, which exist only in debug builds.
