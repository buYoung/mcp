import type { AcpBridgeAgentConfiguration } from "../../config/acp-bridge-config.js";
import { DEFAULT_CLAUDE_CODE_PERMISSION_MODE } from "../../config/defaults.js";
import type { ResolvedLimits } from "../../config/limits-resolver.js";
import { createAcpAgentAdapter } from "../common/acp-agent-adapter.js";
import { readAgentCommandConfig, resolvePermissionProfile } from "../common/environment.js";
import { resolveLocalNodeBinaryCommand } from "../common/local-binary.js";

const commandConfig = readAgentCommandConfig(
    "ACP_BRIDGE_CLAUDE_CODE",
    resolveLocalNodeBinaryCommand("@agentclientprotocol/claude-agent-acp", "claude-agent-acp"),
);

export function createClaudeCodeAgent(configuration: AcpBridgeAgentConfiguration = {}, limits: ResolvedLimits) {
    return createAcpAgentAdapter({
        id: "claude-code",
        label: "Claude Code",
        description:
            "Claude Code through the official Claude Agent ACP adapter. Override ACP_BRIDGE_CLAUDE_CODE_COMMAND or ACP_BRIDGE_CLAUDE_CODE_ARGS if needed.",
        launchOptions: {
            command: commandConfig.command,
            commandArguments: commandConfig.commandArguments,
            configOptionOrder: ["mode", "model", "effort"],
            configOptions: {
                mode: "",
                effort: configuration.reasoning ?? "",
                model: configuration.model ?? "",
            },
            cwd: process.cwd(),
            mode: DEFAULT_CLAUDE_CODE_PERMISSION_MODE,
            operationTimeoutMs: limits.operationTimeoutMs,
            permissionProfile: resolvePermissionProfile(configuration.permission),
            promptTimeoutMs: limits.promptTimeoutMs,
            stderrRingBufferChars: limits.stderrRingBufferChars,
        },
    });
}
