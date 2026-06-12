# AGENTS.md

## 1. Overview

`@buyong-mcp/acp-bridge` is a stdio MCP server that lets a calling coding agent consult other coding agents as read-only ACP pair reviewers.

## 2. Folder Structure

- `src/index.ts`: composition root; creates the MCP `Server`, ensures `.acp_bridge/config.toml`, resolves limits, registers default agents/tools, then connects `StdioServerTransport`.
- `src/tools/`: MCP tool surface and request handling.
    - `index.ts`: registers `list_agents`, `ask_pair`, `continue_pair`, `consult_panel`, and `close_pair`; validates raw MCP arguments; enforces user-consent elicitation; returns the shared text envelope.
    - `pair-opinion.ts`, `json-extract.ts`: parse pair responses into a structured opinion and provide tolerant JSON extraction.
    - `pair-session-store.ts`: in-memory `session_id` registry with idle expiry and max-session eviction.
    - `files-validation.ts`: realpath-based cwd containment for caller-supplied files.
- `src/agents/`: adapter registry and per-agent factories.
    - `register.ts`, `registry.ts`, `types.ts`: supported agent registration and the `AgentAdapter` boundary.
    - `claude-code/`, `codex/`, `gemini-cli/`: agent-specific ids, labels, descriptions, command/config option wiring.
    - `common/`: ACP adapter factory, binary discovery, environment helpers, and shared adapter error/types.
- `src/acp/`: ACP client layer; owns child process launch, ACP session lifecycle, permission handling, stderr capture, and tool-call inspection.
- `src/config/`: TOML config scaffold/parser, defaults, permission profiles, and env/TOML limit resolution.
- `docs/permission.md`: permission model reference for `src/acp/`; keep permission behavior and docs aligned.
- `tests/`: vitest coverage for module behavior; mirror the source module name when tests are explicitly requested.

## 3. Core Behaviors & Patterns

- **Composition root with dependency passing**: `index.ts` wires config -> limits -> registry -> tools once. Runtime modules receive dependencies (`limits`, adapters, `PairSessionStore`) instead of reaching across layers; `agentRegistry` is the only shared registry singleton.
- **Agent adapter boundary**: each agent implements `AgentAdapter` through `createAcpAgentAdapter`. Per-agent files only declare metadata and ACP launch options; shared ask/continue/close lifecycle lives in `agents/common/acp-agent-adapter.ts`.
- **Cold ACP child lifecycle**: `ask_pair` and each `consult_panel` member spawn a fresh ACP child; `continue_pair` reuses the stored session child. ACP initialize/newSession/prompt calls race process failure and timeouts, and close escalates `SIGTERM` to `SIGKILL`.
- **Read-only enforcement in layers**: `AcpBridgeClient.requestPermission` runs `layerZeroCheckFromRawInput` before profile decisions. Agent-initiated mode changes are acknowledged but never change the enforced profile, and every decision is emitted to stderr as JSON.
- **Consultation guardrails**: pair tools require `user_request`; `ask_pair` and `consult_panel` require non-empty `main_agent_position` so the caller states a tentative view before asking another agent.
- **Structured-output recovery**: pair prompts require one JSON object. Non-JSON replies trigger one re-emit request; `parsePairOpinion` still degrades to a `fallback` shape instead of treating prose as an actionable recommendation.
- **Bounded in-memory state**: `PairSessionStore` and the elicitation confirmation map use TTL expiry plus oldest-first capacity eviction. Session operations are serialized per `session_id` through the adapter queue.
- **Failure context**: ACP errors are wrapped with the child stderr tail via `withStderrContext`; prompt timeouts cancel the ACP turn, close the process, and surface `PairSessionClosedError` so callers evict stale sessions.

## 4. Conventions

- **Naming**: TypeScript code uses `camelCase` values/functions, `PascalCase` types/classes, and `UPPER_SNAKE_CASE` module constants. MCP tool names and JSON-schema keys use `snake_case`; handlers map them to internal `camelCase`.
- **Files and imports**: `.ts` files are kebab-case and ESM. Relative imports include `.js` for NodeNext output. Agent factories live in per-agent directories with `index.ts`.
- **Argument validation**: raw tool arguments go through `readRequiredString`, `readOptionalString`, and array readers before use; errors name the expected key, e.g. `Expected non-empty string argument: agent_id`.
- **Adapter factory shape**: `create*Agent(configuration, limits)` returns `createAcpAgentAdapter({ id, label, description, launchOptions })`. `configOptionOrder` controls ACP config application order, and empty config strings mean adapter defaults.
- **Config boundaries**: `.acp_bridge/config.toml` is scaffolded with `wx`, parsed with allow-lists, and rejects unknown agent/limit keys. `prompt_timeout_ms` is required via TOML or `ACP_BRIDGE_PROMPT_TIMEOUT_MS`.
- **Result envelope**: tool responses use `{ content: [{ type: "text", text: JSON.stringify(value, null, 2) }] }`; do not return raw objects directly from handlers.
- **Errors**: use plain `Error` for caller-facing validation and named errors with guards (`PairSessionClosedError`, `isPairSessionClosedError`) when control flow depends on the type.
- **Comments**: keep comments sparse and explanatory; permission comments should point back to `docs/permission.md` when behavior is policy-driven.

## 5. Working Agreements

See root `/AGENTS.md` for common working agreements.

Package-local verification: run `pnpm --filter @buyong-mcp/acp-bridge check-types` after TypeScript changes in this package.
