# @buyong-mcp/acp-bridge

MCP server that bridges other coding agents over Zed's **Agent Client Protocol (ACP)**.
A calling agent (e.g. Claude Code) can consult Codex, Gemini CLI, or another Claude Code
instance as MCP tools — read-only, pair-programming style. Each pair runs as a cold child
process and returns a **structured opinion** (agree / disagree / partial), not just prose, so
the caller gets a second perspective without handing over the decision.

## Installation

Not published to npm — build locally, then register the built entry point with your MCP host.

```bash
# from the monorepo root
pnpm install
pnpm --filter @buyong-mcp/acp-bridge build   # emits apps/mcp-server/dist/index.js
```

Register with Claude Code (`.mcp.json` for project scope, `~/.claude.json` for user scope).
`ACP_BRIDGE_PROMPT_TIMEOUT_MS` is **required** — the server refuses to start without it:

```json
{
  "mcpServers": {
    "acp-bridge": {
      "command": "node",
      "args": ["/absolute/path/to/apps/mcp-server/dist/index.js"],
      "env": { "ACP_BRIDGE_PROMPT_TIMEOUT_MS": "600000" }
    }
  }
}
```

The `claude-code` and `codex` pair binaries are resolved from this package's
`node_modules/.bin`, so install those agents in the package (or override the command via the
env vars below); `gemini-cli` defaults to `gemini --acp` on your `PATH`. For local debugging
with the MCP Inspector:

```bash
ACP_BRIDGE_PROMPT_TIMEOUT_MS=600000 pnpm --filter @buyong-mcp/acp-bridge inspect
```

## Tools

`list_agents`, `ask_pair`, `continue_pair`, and `consult_panel` require `user_request` — the
verbatim user ask that triggered the consult (`close_pair` takes only `session_id`).
`ask_pair` and `consult_panel` also require `main_agent_position`, the caller's tentative
stance, so the pair can agree or push back instead of being handed the decision (this guards
against cognitive offloading).

| Tool | Purpose | Key arguments |
|---|---|---|
| `list_agents` | List available pair agents and their ids. | `user_request` |
| `ask_pair` | Open a new read-only session with one agent. | `agent_id`, `prompt`, `main_agent_position`, `user_request`, `files?` |
| `continue_pair` | Continue an existing session. | `session_id`, `prompt`, `user_request`, `files?` |
| `consult_panel` | Ask 2–`max_consult_panel_agents` agents in parallel; returns each opinion plus a stance tally. Cost scales linearly with agent count. | `agent_ids`, `prompt`, `main_agent_position`, `user_request`, `files?` |
| `close_pair` | Close a session when consultation is done. | `session_id` |

`files` is an array of absolute paths the pair reads directly (validated to stay within the
working directory), so the pair isn't limited to the caller's summary. Pairs have read-only
permission — they never edit, move, delete, or run commands.

### Response shape

Each pair response keeps the raw `answer` and adds `structured_opinion`:

- `stance` — `agree` / `disagree` / `partial` / `insufficient_info`
- `summary`, `agreements`, `concerns`, `recommendation`, `follow_up_questions`
- `parse_status` — `parsed`, or `fallback` when the pair's JSON couldn't be parsed

On `fallback`, the original text is preserved in `raw_answer` and `recommendation` is left
empty so the caller doesn't mistake a fallback for advice. `meta` carries `elapsed_ms`,
`stop_reason`, `agent_id`, and `agent_model` when available. `consult_panel` additionally
returns a `stance_tally` counting each stance across the panel.

`user_request` is **not** regex-gated. If the host supports MCP elicitation, the first
consult per process asks the user to confirm; hosts without elicitation are not blocked — the
bridge just writes one `[acp-bridge] pair-consult invoked ...` line to stderr.

Requests for the same `session_id` are serialized. Idle sessions are reaped after 30 minutes
and at most 20 pair sessions are kept (both tunable below).

## Configuration

On startup the server creates `.acp_bridge/config.toml` in the working directory if it is
missing (it never overwrites an existing file). Set each adapter's ACP model id there; empty
fields fall back to adapter defaults. `permission` is parsed for backward compatibility but
does not change the fixed read-only behavior.

```toml
[agents.claude-code]
model = ""
permission = ""
reasoning = ""      # → effort; Claude Code always runs in plan mode

[agents.codex]
model = ""
permission = ""
reasoning = ""      # → reasoning_effort; Codex always runs read-only

[agents.gemini-cli]
model = ""          # session model only — Gemini has no reasoning; setting it fails init

# Operational limits. Omit a key to fall back to the ACP_BRIDGE_* env var, then the default.
[limits]
# max_pair_sessions = 20
# pair_session_idle_timeout_ms = 1800000
# max_consult_panel_agents = 5
# operation_timeout_ms = 180000
# prompt_timeout_ms = 600000
# stderr_ring_buffer_chars = 16384
```

### Environment variables

`[limits]` keys can equivalently be set via `ACP_BRIDGE_*` env vars (precedence: TOML value →
env var → built-in default).

| Variable | Required | Default | Purpose |
|---|---|---|---|
| `ACP_BRIDGE_PROMPT_TIMEOUT_MS` | ✅ | — | Max wait for one pair turn, positive integer ms (e.g. `600000` = 10 min). Also settable as `[limits] prompt_timeout_ms`. |
| `ACP_BRIDGE_MAX_PAIR_SESSIONS` | ❌ | `20` | Max concurrent pair sessions. |
| `ACP_BRIDGE_PAIR_SESSION_IDLE_TIMEOUT_MS` | ❌ | `1800000` | Idle-session reaper (30 min). |
| `ACP_BRIDGE_MAX_CONSULT_PANEL_AGENTS` | ❌ | `5` | Upper bound on `consult_panel` agent count. |
| `ACP_BRIDGE_OPERATION_TIMEOUT_MS` | ❌ | `180000` | Per-operation timeout. |
| `ACP_BRIDGE_STDERR_RING_BUFFER_CHARS` | ❌ | `16384` | Captured child-stderr ring-buffer size. |
| `ACP_BRIDGE_{CLAUDE_CODE,CODEX,GEMINI_CLI}_COMMAND` | ❌ | auto (see above) | Override an adapter's executable path. |
| `ACP_BRIDGE_{CLAUDE_CODE,CODEX,GEMINI_CLI}_ARGS` | ❌ | adapter default | Override adapter args (JSON-array string, e.g. `'["--acp"]'`). |

ACP permission requests are fixed to read-only: `read`, `search`, `fetch`, and `think` are
allowed; mutation, execution, mode switching, and unknown tool kinds are rejected.
`ACP_BRIDGE_PERMISSION_POLICY` is ignored, kept only for backward compatibility.
