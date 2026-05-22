import type { AcpBridgeAgentConfiguration } from "../../config/acp-bridge-config.js";
import { createAcpAgentAdapter } from "../common/acp-agent-adapter.js";
import {
    readAgentCommandConfig,
    readOperationTimeoutMs,
    readPromptTimeoutMs,
    resolvePermissionProfile,
} from "../common/environment.js";

const commandConfig = readAgentCommandConfig("ACP_BRIDGE_GEMINI_CLI", {
    command: "gemini",
    commandArguments: ["--acp"],
});

export function createGeminiCliAgent(configuration: AcpBridgeAgentConfiguration = {}) {
    return createAcpAgentAdapter({
        id: "gemini-cli",
        label: "Gemini CLI",
        description:
            "Gemini CLI through ACP mode over stdio. Override ACP_BRIDGE_GEMINI_CLI_COMMAND or ACP_BRIDGE_GEMINI_CLI_ARGS if needed.",
        launchOptions: {
            command: commandConfig.command,
            commandArguments: commandConfig.commandArguments,
            cwd: process.cwd(),
            model: configuration.model,
            operationTimeoutMs: readOperationTimeoutMs(),
            permissionProfile: resolvePermissionProfile(configuration.permission),
            promptTimeoutMs: readPromptTimeoutMs(),
        },
    });
}
