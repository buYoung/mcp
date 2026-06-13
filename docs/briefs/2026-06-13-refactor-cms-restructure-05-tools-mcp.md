# [refactor] Extract search/overview into tools/, convert mcp.rs to mcp/ module

## Work Type
refactor

## Current State (As-Is)
- `apps/codemap-search/src/mcp.rs` (~1,350 lines) is the last god module after children 01/04: JSON-RPC protocol types + `LimitedLineReader`, run loop, `handle_request` dispatch, `tools/list` schema JSON, the `initialize` instructions string, the full `search` tool body (~360 lines in the `tools/call` search arm), the `overview` tool body (~105 lines), and ~390 lines of pure rendering helpers (`cap_snippet`, `truncate_literal`, `get_code_snippet`, `query_tokens`, `query_words`, `symbol_matches_query`, `symbol_is_tier1`, `get_summary_snippet`, `get_signature_snippet`, `render_anchored_symbols` + `AnchoredRenderCaps`/`AnchoredRenderOutcome` — all free functions, no `self`).
- `apps/codemap-search/src/tools/` holds only read/find/grep (`fn(&Value) -> Result<String, (i64, String)>` shape) plus tool-arg coercion helpers in `tools/mod.rs:297-361` — the tool folder does not match the 5-tool product surface.
- After child 04, the search/overview arms call `EngineSupervisor` (`ensure_alive`, `trigger_refresh`, `search`, `codemap_snapshot`, indexer status reads) — engine state access is already consolidated, but the tool bodies still live inside the server.
- The server-side renderer fulfills the callers dedup contract (render→emit→`PreparedAnnotation::commit` sequence) inside `render_anchored_symbols`.
- The search rendering helpers' only consumer is the search arm — they are search code, not shared server code.

## Behavior Contract
- Locked: all five tool outputs byte-identical; `initialize` response (instructions string, tool schemas) unchanged; refresh/restart triggers fire **only** on search/overview calls (not read/find/grep) — preserved by hoisting `ensure_alive()`/`trigger_refresh()` to the dispatch arms; JSON-RPC error codes and the line-length cap behavior unchanged.
- Contract artifacts: e2e suite `apps/codemap-search/tests/e2e_tests.rs` (covers initialize handshake + tool calls over stdio); in-file tests.
- Verification: `cargo test`; manual diff of one `search`, one `overview`, one `read` response before/after.

## Desired Outcome (To-Be)
- `apps/codemap-search/src/tools/search/` (proposed): `mod.rs` — search tool body taking a `ToolContext` (borrows from `EngineSupervisor` + parsed args), orchestration only; `render.rs` — the ~390 lines of snippet/tier/anchoring helpers, with a module comment documenting the dedup contract it fulfills against `callers::annotate` (`PreparedAnnotation::commit`).
- `apps/codemap-search/src/tools/overview.rs` (proposed): overview tool body, same `ToolContext` pattern.
- `apps/codemap-search/src/tools/mod.rs`: gains the `tools/list` schema definitions and the `initialize` instructions string (moved together to prevent schema/instructions drift), keeps arg-coercion helpers; declares `search`/`overview` submodules.
- `apps/codemap-search/src/mcp/` (proposed): `mod.rs` — `McpServer` (supervisor field + run loop + `handle_request` dispatch + `ToolContext` construction; calls `ensure_alive`/`trigger_refresh` on the search/overview arms only; ~150 lines), `protocol.rs` — `JsonRpcRequest`, `JsonRpcResponse`, `LimitedLineReader`.
- `lib.rs` keeps `pub mod mcp;` (directory now); module named `mcp` (not `server`) because its contents are the MCP contract.

## Scope
### In Scope
- Moving the search/overview arms, rendering helpers, schemas, and instructions string; defining `ToolContext`; converting `mcp.rs` → `mcp/{mod,protocol}.rs`; updating `main.rs` import paths.
### Out of Scope
- [hard] No tool output changes, no schema text changes, no new tools.
- [hard] read/find/grep keep their `fn(&Value)` signatures — do not force a uniform signature across all five tools; search/overview legitimately need `ToolContext`.
- [hard] Lifecycle methods stay on `EngineSupervisor` (child 04) — do not reintroduce them into `mcp/`.
- [deferred] Further `mcp/` subdivision (e.g. `dispatch.rs`) — only when MCP surface grows (resources/prompts/notifications).

## Related Files / Entry Points
- `apps/codemap-search/src/mcp.rs` — extraction source; read the "tools/call" dispatch match first to fix arm boundaries.
- `apps/codemap-search/src/tools/mod.rs` — receives schemas + instructions; submodule declarations.
- `apps/codemap-search/src/callers.rs` — contract counterpart (`DetailAnnotations`, `PreparedAnnotation`) consumed by `apps/codemap-search/src/tools/search/render.rs` (proposed); the callers path is the `apps/codemap-search/src/callers/` (proposed) directory after child 03.
- `apps/codemap-search/src/main.rs` — `Commands::Mcp` arm import updates.

## Side Effect Checkpoints
- [ ] read/find/grep do **not** trigger engine refresh/restart (behavior preserved — verify by code inspection of the dispatch arms).
- [ ] `initialize` response JSON byte-identical (instructions + schemas moved, not edited).
- [ ] Caller-annotation dedup ("same as above" back-references) still renders across multiple matched symbols in one search response.
- [ ] stdout remains pure JSON-RPC (logging still stderr-only) after the module conversion.
- [ ] Oversized-line rejection (`LimitedLineReader` cap) behavior unchanged.

## Acceptance Criteria
- [ ] `tools/` contains `mod.rs`, `search/mod.rs`, `search/render.rs`, `overview.rs`, `read.rs`, `find.rs`, `grep.rs` — one entry per MCP tool.
- [ ] No single new file exceeds ~500 non-test lines (`search/mod.rs` and `search/render.rs` each stay under; the old plan's single 754-line `search.rs` is explicitly rejected).
- [ ] `mcp/mod.rs` contains no tool business logic — dispatch arms only construct `ToolContext` and delegate.
- [ ] `cargo test` and the e2e suite pass; one manual search/overview/read response diff is byte-identical.

## Open Questions
- None — `ToolContext` shape (borrowed supervisor + args) and the schema/instructions co-location were locked during plan review.
