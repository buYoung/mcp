import type { AcpBridgeAgentConfiguration } from "../../config/acp-bridge-config.js";
import { DEFAULT_CODEX_PERMISSION_MODE } from "../../config/defaults.js";
import { createAcpAgentAdapter } from "../common/acp-agent-adapter.js";
import {
    readAgentCommandConfig,
    readOperationTimeoutMs,
    readPromptTimeoutMs,
    resolvePermissionProfile,
} from "../common/environment.js";
import { resolveLocalNodeBinaryCommand } from "../common/local-binary.js";

const commandConfig = readAgentCommandConfig(
    "ACP_BRIDGE_CODEX",
    resolveLocalNodeBinaryCommand("@zed-industries/codex-acp", "codex-acp"),
);

export function createCodexAgent(configuration: AcpBridgeAgentConfiguration = {}) {
    return createAcpAgentAdapter({
        id: "codex",
        label: "Codex",
        description:
            "Codex through the official Codex ACP adapter. Override ACP_BRIDGE_CODEX_COMMAND or ACP_BRIDGE_CODEX_ARGS if needed.",
        launchOptions: {
            command: commandConfig.command,
            commandArguments: commandConfig.commandArguments,
            configOptionOrder: ["mode", "model", "reasoning_effort"],
            configOptions: {
                mode: "",
                model: configuration.model ?? "",
                reasoning_effort: configuration.reasoning ?? "",
            },
            cwd: process.cwd(),
            mode: DEFAULT_CODEX_PERMISSION_MODE,
            operationTimeoutMs: readOperationTimeoutMs(),
            permissionProfile: resolvePermissionProfile(configuration.permission),
            promptTimeoutMs: readPromptTimeoutMs(),
        },
    });
}
