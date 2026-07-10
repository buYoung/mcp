# AGENTS.md

## 1. Overview

`@buyong-mcp/acp-bridge` is a stdio MCP server that lets a coding agent consult other coding agents as read-only ACP pair reviewers. It owns the consultation tools, ACP child-process lifecycle, permission enforcement, and bounded pair-session state.

## 2. Ownership Map

### Stable Ownership Boundaries

- **MCP tool boundary**: Start in `src/tools/index.ts` when changing `list_agents`, `ask_pair`, `continue_pair`, `consult_panel`, or `close_pair`. It owns raw MCP argument validation, user-consent elicitation, `main_agent_position` requirements, session-store calls, and the text JSON result envelope; verify with `tests/pair-opinion.test.ts`, `tests/pair-session-store.test.ts`, and the package type-check.
- **ACP child boundary**: Start in `src/acp/client.ts` and `src/agents/common/acp-agent-adapter.ts` when changing agent launch, prompt, continuation, timeout, or close behavior. They own cold child startup, per-session serialization, stderr context, prompt retry for JSON output, and timeout-triggered session closure; preserve `PairSessionClosedError` eviction semantics.
- **Permission policy boundary**: Start in `src/acp/client.ts`, `src/acp/permission-decision.ts`, and `src/acp/layer-zero.ts` when changing read-only enforcement. This boundary owns layer-zero raw-input rejection, profile decisions, and JSON audit events; keep `docs/permission.md` aligned with any policy change.
- **Configuration and limits boundary**: Start in `src/config/acp-bridge-config.ts` and `src/config/limits-resolver.ts` when changing `.acp_bridge/config.toml`, supported agent keys, or operational limits. Preserve strict unknown-key rejection, positive integer validation, and env/TOML/default precedence; verify with the config and limits tests.

### Active Change Routes

- **Pair consultation guardrail route**: Within **MCP tool boundary**, start in `src/tools/index.ts` when changing pair prompt requirements or panel fan-out behavior. Recent changes cluster around the tool surface and ACP client, so keep consent, `user_request`, `main_agent_position`, file containment, and stance tally behavior consistent across single-agent and panel calls.
- **ACP adapter route**: Within **ACP child boundary**, start in `src/agents/common/acp-agent-adapter.ts` when changing session reuse or closure. The shared adapter is used by Claude Code, Codex, and Gemini factories, so a lifecycle change affects all agents even when the per-agent file looks small.

## 3. Core Behaviors & Patterns

- **Composition root with dependency passing**: `src/index.ts` creates the MCP `Server`, loads config, resolves limits, registers default agents, and passes `limits` into `registerTools`. Runtime code receives dependencies instead of importing config on demand, except for the shared `agentRegistry`.
- **Shared adapter factory**: Per-agent modules only declare ids, labels, descriptions, command names, model/config fields, and launch options. `createAcpAgentAdapter` owns the common `askPair`, `continuePair`, `closePair`, and per-session queue behavior for every agent.
- **Cold-start then session reuse**: `ask_pair` spawns a fresh ACP child and stores its session id; `continue_pair` finds the original agent through `PairSessionStore` and reuses that child. `close_pair`, idle expiry, and max-session eviction close the adapter-owned process.
- **Layered read-only enforcement**: ACP permission requests run `layerZeroCheckFromRawInput` before profile-based decisions. Agent-initiated mode changes are acknowledged but never mutate the enforced profile, and every decision is emitted as an audit event.
- **Structured-output recovery**: Pair prompts require a single JSON object. If the first answer has text but no parseable JSON, `AcpBridgeClient` sends one re-emit prompt; downstream parsing still degrades to a fallback opinion shape rather than trusting prose.
- **Failure context and cleanup**: ACP operations race process failure and explicit timeouts. Prompt timeout cancels the turn, closes the child, throws `PairSessionClosedError`, and causes callers to evict stale session ids; other ACP errors are wrapped with the stderr ring buffer tail.

## 4. Conventions

- **Naming**: TypeScript values/functions use `camelCase`, classes/types use `PascalCase`, and module constants use `UPPER_SNAKE_CASE`. MCP tool names and JSON-schema keys stay `snake_case`; handlers translate them to internal `camelCase`.
- **Files and imports**: Source files are kebab-case `.ts` files using ESM and NodeNext `.js` relative import extensions. Per-agent factories live under `src/agents/<agent>/index.ts`; shared launch and lifecycle code stays under `src/agents/common/`.
- **Tool argument validation**: Raw MCP arguments must go through `readRequiredString`, optional readers, and array readers before use. Caller-facing validation errors should name the expected key, such as `Expected non-empty string argument: agent_id`.
- **Adapter construction shape**: New agents should return `createAcpAgentAdapter({ id, label, description, launchOptions })`. Empty config strings mean adapter defaults; `configOptionOrder` controls the order of ACP config application.
- **Config boundaries**: `.acp_bridge/config.toml` is created with `wx`, parsed through allow-lists, and rejects unsupported agent ids, unknown keys, wrong value types, and invalid positive integers instead of silently accepting drift.
- **Result envelope**: Tool handlers return `{ content: [{ type: "text", text: JSON.stringify(value, null, 2) }] }`; do not return raw objects directly.
- **Typed control-flow errors**: Plain `Error` is enough for validation. Use named errors and guards, such as `PairSessionClosedError` and `isPairSessionClosedError`, only when control flow depends on the type.

## 5. Working Agreements

See root `/AGENTS.md` for common working agreements.

Package-local verification: run `pnpm --filter @buyong-mcp/acp-bridge check-types` after TypeScript changes in this package.
