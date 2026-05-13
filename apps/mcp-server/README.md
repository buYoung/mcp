# @buyong-mcp/acp-bridge

MCP server that bridges other coding agents via Zed's Agent Client Protocol
(ACP), so a calling agent (e.g. Claude Code) can consult them as MCP tools —
pair-programming style.

## Layout

```
src/
  index.ts          # MCP stdio entry
  tools/            # MCP tool surface (list_models, ask_pair, continue_pair)
  agents/           # Adapters per coding agent (registered into agentRegistry)
    common/         # Shared AgentAdapter contract + ACP adapter factory
    claude-code/    # Claude Code ACP config
    codex/          # Codex ACP config
    gemini-cli/     # Gemini CLI ACP config
  acp/              # ACP client wrapper used by adapters to drive external agents
```

## Run

```
ACP_BRIDGE_PROMPT_TIMEOUT_MS=600000 pnpm --filter @buyong-mcp/acp-bridge dev
```

## Agents

Registered agent ids:

- `claude-code`
- `codex`
- `gemini-cli`

On MCP server initialization, `acp-bridge` creates `.acp_bridge/config.toml` in the current working directory if it is missing. Set the actual ACP model id per adapter there:

```toml
[agents.claude-code]
model = ""
permission = ""
reasoning = ""

[agents.codex]
model = ""
permission = ""
reasoning = ""

[agents.gemini-cli]
model = ""
permission = ""
```

Leave fields empty to use the adapter defaults. `permission` is parsed for compatibility with existing config files, but it does not change read-only behavior. Claude Code receives `model` / `effort` config options and always uses `plan` mode. Codex receives `model` / `reasoning_effort` config options and always uses `read-only` mode. Gemini CLI receives `model` as the session model; Gemini does not support `reasoning`, so setting it fails during initialization.

Claude Code and Codex adapter binaries are resolved from this package's `node_modules/.bin` by default. Override commands with JSON-array argument variables only when needed:

```bash
ACP_BRIDGE_CLAUDE_CODE_ARGS='[]'
ACP_BRIDGE_CODEX_ARGS='[]'
ACP_BRIDGE_GEMINI_CLI_ARGS='["--acp"]'
```

`ACP_BRIDGE_PROMPT_TIMEOUT_MS` is required and must be a positive integer in milliseconds. ACP permission requests are fixed to read-only behavior: `read`, `search`, `fetch`, and `think` are allowed, while mutation, execution, mode switching, and unknown tool kinds are rejected. `ACP_BRIDGE_PERMISSION_POLICY` is ignored for backward compatibility.
