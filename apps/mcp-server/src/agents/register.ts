import type { AcpBridgeConfiguration } from "../config/acp-bridge-config.js";
import { SUPPORTED_AGENT_IDS } from "../config/defaults.js";
import type { ResolvedLimits } from "../config/limits-resolver.js";
import { createClaudeCodeAgent } from "./claude-code/index.js";
import { createCodexAgent } from "./codex/index.js";
import { createGeminiCliAgent } from "./gemini-cli/index.js";
import { agentRegistry } from "./registry.js";

const [CLAUDE_CODE_AGENT_ID, CODEX_AGENT_ID, GEMINI_CLI_AGENT_ID] = SUPPORTED_AGENT_IDS;

export function registerDefaultAgents(configuration: AcpBridgeConfiguration, limits: ResolvedLimits): void {
    agentRegistry.register(createClaudeCodeAgent(configuration.agents[CLAUDE_CODE_AGENT_ID], limits));
    agentRegistry.register(createCodexAgent(configuration.agents[CODEX_AGENT_ID], limits));
    agentRegistry.register(createGeminiCliAgent(configuration.agents[GEMINI_CLI_AGENT_ID], limits));
}
