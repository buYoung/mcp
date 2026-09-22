# 04 — native MCP and configuration handoff

## Activation contract

- Canonical group: `[analysis.jev]` in `src/config.rs`, English/Korean generated templates and both configuration references. Schema version 24 adds the section as comments to older repo files; a fresh generated file has both modes explicitly false. Repo/global/default precedence is per key, and invalid values warn and inherit.
- Defaults and validation: `overview_enabled=false`, `search_filter_enabled=false`, `model="jev-1.13.0"`, `api_key_env="TYPESAFE_API_KEY"`, `timeout_ms=45000` (1–45000), `max_in_flight_requests=3` (1–3), `request_spacing_ms=300` (300–60000), `max_batch_bytes=80000` (1–80000), `pool_idle_timeout_ms=30000` (1–30000), and `search_filter_min_unrelated_probability=0.70` (finite `0.5 < value <= 1.0`). The config stores only the environment variable name. Credentials are resolved only when an enabled call supplies intent.
- `task_query` is optional in both schemas, must be a string when present, and is request-local. It never replaces `search.query` or `overview` path/file/query aliases. Blank or absent intent bypasses Jev. Enabling a mode alone creates no provider request. The four mode combinations are independent; read/find/grep, CLI search/codemap/index, and initial-instructions root rendering do not invoke Jev.

## MCP lifecycle and result contract

- `McpServer::handle_request` now awaits the adapters inside the existing one-request-at-a-time stdio loop. Request config and redaction remain pinned across awaits. The shared production evaluator reuses a rustls HTTP client until the host key fingerprint or pool idle setting changes. `McpServer::with_jev_evaluator` supplies an offline library/test injection boundary without a production endpoint override. The generic evaluator retains caller cancellation and deadline inputs; no new JSON-RPC cancellation notification behavior or concurrent dispatch was introduced.
- The existing `content` / JSON-RPC error envelope remains. Enabled calls add `_meta.jev` with mode, `applied`/`bypassed`/`fallback`, non-sensitive reason when relevant, known input/output usage and elapsed milliseconds. Root recommendations also expose `recommendation_status`; search exposes effective Noul threshold and evaluated/omitted body counts. Unknown usage after a failed multi-batch call is `null`, not a fabricated zero. A short status line is appended to text only if its existing cap has room.
- Missing key/intent and unavailable indexed evidence preserve the base tool result. Invalid answers, transport failure, deadline and output-limit fallback also preserve it. The final response masking/cap check and call recording remain in the outer request handler. Search source observations come from the final filtered output.

## Operator calls

```toml
[analysis.jev]
overview_enabled = true
search_filter_enabled = false
api_key_env = "TYPESAFE_API_KEY"
search_filter_min_unrelated_probability = 0.70
```

```json
{"name":"overview","arguments":{"task_query":"Find the request cancellation path"}}
{"name":"search","arguments":{"query":"cancel","task_query":"Find the request cancellation path"}}
```

Set the named key in the MCP host environment before using an enabled mode. The search example evaluates only when `search_filter_enabled=true`; the two flags can be set independently. `cargo run --manifest-path apps/codemap-search/Cargo.toml --example jev_decisions -- --mock` demonstrates direct Rust reuse without credentials or MCP. `--live` requires an explicit operator choice and was not run.

## Compatibility evidence

- `cargo test --manifest-path apps/codemap-search/Cargo.toml --lib config`: 37 passed at the previous run. A new v24 migration regression was subsequently added and is included in final verification.
- `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests e2e::config`: 9 passed. `e2e::jev` previously passed injected enabled calls, 0.80 Noul under 0.70/0.90 after config reload, mode independence, keyless bypass and final source accounting. The final post-join result is in `05-verification.md`.
- `cargo check --manifest-path apps/codemap-search/Cargo.toml --all-targets`: passed before final test-only cases; rerun at final gate.

README, English/Korean configuration references, templates, tool schemas and conditional initial guidance describe the same `task_query` and mode behavior. No Python proxy, additional MCP server, or new CLI surface is needed. Cross-platform release builds and live provider behavior remain unrun.
