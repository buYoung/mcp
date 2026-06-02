# AGENTS.md

## 1. Overview

`@buyong-mcp/acp-bridge` is an MCP stdio server that acts as an ACP client: the calling coding agent uses its tools to consult other coding agents (Claude Code, Codex, Gemini CLI) as read-only pair reviewers, each spawned as a child process and relayed over ACP.

## 2. Folder Structure

- `src/index.ts`: composition root — builds the MCP `Server`, ensures the `.acp_bridge/config.toml` config, resolves limits, registers agents and tools, then connects the stdio transport.
- `src/tools/`: the MCP tool surface and request handling.
    - `index.ts`: `registerTools` — ListTools/CallTool handlers, the five tools (`list_agents`, `ask_pair`, `continue_pair`, `consult_panel`, `close_pair`), argument readers, the `textResult` envelope, and the user-consent elicitation gate.
    - `pair-opinion.ts`: parses a pair agent's reply into a `structured_opinion`, with a `fallback` shape on parse failure.
    - `pair-session-store.ts`: in-memory `session_id` → agent registry with idle expiry and max-session eviction.
    - `files-validation.ts`: realpath-based cwd-containment check for caller-supplied file paths.
    - `json-extract.ts`: tolerant JSON extraction from agent prose.
- `src/agents/`: agent adapters and the registry.
    - `registry.ts`: the `agentRegistry` singleton mapping `agent_id` → `AgentAdapter`.
    - `register.ts`: registers the three default agents from config + limits.
    - `claude-code/`, `codex/`, `gemini-cli/`: per-agent adapter factories (id/label/description + ACP launch options only).
    - `common/`: shared `AgentAdapter` type and errors (`types.ts`), the ACP adapter factory (`acp-agent-adapter.ts`), PATH/binary probing (`binary-availability.ts`), env-var reading (`environment.ts`), and `node_modules/.bin` resolution (`local-binary.ts`).
- `src/acp/`: the ACP client layer.
    - `client.ts`: `launchAcpAgent` + `StdioAcpAgentSession` (initialize/newSession/prompt lifecycle, timeouts, process teardown) and the permission-enforcing `Client`.
    - `permission-decision.ts`: maps ACP tool kinds → allow/reject per permission profile.
    - `layer-zero.ts`: hard-block screen on raw tool input, run before the profile check.
    - `stderr-ring-buffer.ts`, `tool-call-extraction.ts`: bounded child-stderr capture and tool-call introspection.
- `src/config/`: config TOML read/scaffold (`acp-bridge-config.ts`), built-in constants and permission profiles (`defaults.ts`), and TOML+env limit resolution (`limits-resolver.ts`).
- `tests/`: vitest unit tests; add a `<module>.test.ts` mirroring the `src` module under test.
- `docs/permission.md`: the permission-model spec; keep aligned with `src/acp/`.

## 3. Core Behaviors & Patterns

- **Composition root + dependency passing**: `index.ts` wires config → limits → registry → tools once at startup. Modules receive their dependencies (`limits`, `pairSessionStore`, adapters) as parameters; the only module-level singleton is `agentRegistry`.
- **Agent adapter pattern**: every agent is an `AgentAdapter` (`askPair`/`continuePair`/`closePair`) produced by `createAcpAgentAdapter`. Per-agent files declare only `id`, `label`, `description`, and `launchOptions`. Add an agent by extending `SUPPORTED_AGENT_IDS`, adding a `create*Agent` factory, and registering it in `register.ts`.
- **Cold child-process lifecycle & resilience**: each `ask_pair`/`consult_panel` spawns a fresh ACP child (no pooling); `continue_pair` reuses the session's child. Every ACP operation races against a `processFailure` promise and a timeout (`Promise.race`). Teardown escalates SIGTERM → SIGKILL with grace periods. A prompt timeout cancels the turn, closes the session, and throws `PairSessionClosedError` so callers evict the dead session.
- **Read-only enforcement (defense in depth)**: `AcpBridgeClient.requestPermission` first runs a layer-0 hard block on raw input, then `decidePermission` by profile (default `read-only` allows only `read`/`search`/`fetch`/`think`). Agent-initiated mode changes are acknowledged but never mutate the enforced profile. Every decision is audit-logged to stderr as a JSON line.
- **Structured-output contract with recovery**: the system prompt instructs the pair to return one JSON object; `prompt()` retries once when the answer isn't parseable JSON; `parsePairOpinion` degrades to a `fallback` opinion (empty `recommendation`) rather than passing prose off as a recommendation.
- **Per-session serialization**: operations on a given `session_id` are serialized through `runSessionExclusive` (a per-session promise queue) so follow-ups and closes never interleave.
- **Bounded in-memory state**: both `PairSessionStore` and the elicitation-confirmation map enforce TTL expiry plus oldest-first eviction at a size cap.
- **Error context**: ACP failures are rethrown via `withStderrContext`, appending the captured child stderr tail to the message.

## 4. Conventions

- **Naming**: `camelCase` variables/functions, `PascalCase` types/classes, `UPPER_SNAKE_CASE` module constants. MCP tool names and their JSON-schema property keys are `snake_case` (`agent_id`, `main_agent_position`, `user_request`); internal TypeScript uses `camelCase`, with handlers mapping between the two.
- **Files & modules**: kebab-case `.ts` filenames; each agent adapter lives in its own directory with an `index.ts` factory. ESM throughout — relative imports carry the `.js` extension (NodeNext resolution).
- **Argument reading**: tool handlers never trust raw input. They go through `readRequiredString` / `readOptionalString` / `readOptional*Array` helpers that validate type and non-emptiness and throw `Expected ...: <key>` errors.
- **Adapter factory shape**: `create*Agent(configuration, limits)` returns `createAcpAgentAdapter({ id, label, description, launchOptions })`. `configOptionOrder` lists ACP config-option ids in apply order; an empty-string config value means "use the adapter default".
- **Config & boundary validation**: external config (`config.toml`, env vars, limits) is parsed against allow-lists (`SUPPORTED_AGENT_KEYS`, `SUPPORTED_LIMIT_KEYS`) and rejects unknown keys with a path-qualified error. `prompt_timeout_ms` is required (no built-in default). `permission` config is still parsed for back-compat but does not change read-only behavior.
- **Result envelope**: every tool returns `textResult(value)` = `{ content: [{ type: "text", text: JSON.stringify(value, null, 2) }] }`.
- **Errors**: throw plain `Error` with a descriptive message for caller-facing failures; use named error classes (`PairSessionClosedError`, `AcpPromptTimeoutError`) that carry the `sessionId` and are detected via `is*` type guards (`isPairSessionClosedError`).
- **Comments**: sparse, English, explaining non-obvious "why" (e.g. why agent mode changes are ignored for enforcement); reference `docs/permission.md` sections where relevant.

## 5. Working Agreements

See root `/AGENTS.md` for common working agreements.

Package-local verification: run `pnpm --filter @buyong-mcp/acp-bridge check-types` and `pnpm --filter @buyong-mcp/acp-bridge test` (vitest) after changes in this package.
