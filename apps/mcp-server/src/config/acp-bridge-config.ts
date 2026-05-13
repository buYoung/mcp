import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { SUPPORTED_AGENT_IDS } from "./defaults.js";

export interface AcpBridgeAgentConfiguration {
    model?: string;
    permission?: string;
    reasoning?: string;
}

export interface AcpBridgeConfiguration {
    configurationPath: string;
    agents: Record<string, AcpBridgeAgentConfiguration>;
}

type AgentConfigurationKey = keyof AcpBridgeAgentConfiguration;

const CONFIG_DIRECTORY_NAME = ".acp_bridge";
const CONFIG_FILE_NAME = "config.toml";
const [CLAUDE_CODE_AGENT_ID, CODEX_AGENT_ID, GEMINI_CLI_AGENT_ID] = SUPPORTED_AGENT_IDS;

const DEFAULT_CONFIG_TEMPLATE = `# acp-bridge configuration
# Set values to the exact ACP ids each adapter supports.
# Leave fields empty to use the adapter defaults.

[agents.${CLAUDE_CODE_AGENT_ID}]
model = ""
permission = ""
reasoning = ""

[agents.${CODEX_AGENT_ID}]
model = ""
permission = ""
reasoning = ""

[agents.${GEMINI_CLI_AGENT_ID}]
model = ""
permission = ""
`;

export async function ensureAcpBridgeConfiguration(baseDirectory = process.cwd()): Promise<AcpBridgeConfiguration> {
    const configurationDirectoryPath = join(baseDirectory, CONFIG_DIRECTORY_NAME);
    const configurationPath = join(configurationDirectoryPath, CONFIG_FILE_NAME);

    await mkdir(configurationDirectoryPath, { recursive: true });
    await writeFileIfMissing(configurationPath, DEFAULT_CONFIG_TEMPLATE);

    const configurationContents = await readFile(configurationPath, "utf8");
    return {
        configurationPath,
        agents: parseAgentConfigurations(configurationContents, configurationPath),
    };
}

function parseAgentConfigurations(
    configurationContents: string,
    configurationPath: string,
): Record<string, AcpBridgeAgentConfiguration> {
    const agents: Record<string, AcpBridgeAgentConfiguration> = {};
    let currentAgentId: string | undefined;

    configurationContents.split(/\r?\n/).forEach((line, lineIndex) => {
        const lineWithoutComment = stripInlineComment(line).trim();
        if (lineWithoutComment.length === 0) {
            return;
        }

        const sectionMatch = /^\[agents\.([A-Za-z0-9_-]+)\]$/.exec(lineWithoutComment);
        if (sectionMatch) {
            const agentId = sectionMatch[1];
            if (agentId == null) {
                throw new Error(`Missing agent id at ${configurationPath}:${lineIndex + 1}`);
            }
            currentAgentId = agentId;
            agents[agentId] ??= {};
            return;
        }

        const keyValueMatch = /^(model|permission|reasoning)\s*=\s*(.+)$/.exec(lineWithoutComment);
        if (keyValueMatch && currentAgentId) {
            const configurationKey = keyValueMatch[1];
            const sourceValue = keyValueMatch[2];
            if (configurationKey == null || sourceValue == null) {
                throw new Error(`Missing configuration value at ${configurationPath}:${lineIndex + 1}`);
            }
            if (!isAgentConfigurationKey(configurationKey)) {
                throw new Error(`Unsupported configuration key at ${configurationPath}:${lineIndex + 1}`);
            }
            const configurationValue = parseTomlString(sourceValue, configurationPath, lineIndex + 1).trim();
            if (configurationValue.length > 0) {
                if (currentAgentId === "gemini-cli" && configurationKey === "reasoning") {
                    throw new Error(`Gemini CLI does not support reasoning at ${configurationPath}:${lineIndex + 1}`);
                }
                const agentConfiguration = agents[currentAgentId];
                if (agentConfiguration == null) {
                    throw new Error(`Missing agent section at ${configurationPath}:${lineIndex + 1}`);
                }
                agentConfiguration[configurationKey] = configurationValue;
            }
            return;
        }

        throw new Error(`Unsupported ${CONFIG_FILE_NAME} entry at ${configurationPath}:${lineIndex + 1}`);
    });

    return agents;
}

function isAgentConfigurationKey(configurationKey: string): configurationKey is AgentConfigurationKey {
    return configurationKey === "model" || configurationKey === "permission" || configurationKey === "reasoning";
}

function parseTomlString(sourceValue: string, configurationPath: string, lineNumber: number): string {
    const trimmedValue = sourceValue.trim();
    if (!trimmedValue.startsWith('"') || !trimmedValue.endsWith('"')) {
        throw new Error(`Expected TOML string at ${configurationPath}:${lineNumber}`);
    }

    try {
        return JSON.parse(trimmedValue) as string;
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new Error(`Invalid TOML string at ${configurationPath}:${lineNumber}: ${message}`);
    }
}

function stripInlineComment(line: string): string {
    let isInsideString = false;
    let isEscaped = false;

    for (let characterIndex = 0; characterIndex < line.length; characterIndex += 1) {
        const character = line[characterIndex];

        if (isEscaped) {
            isEscaped = false;
            continue;
        }
        if (character === "\\") {
            isEscaped = true;
            continue;
        }
        if (character === '"') {
            isInsideString = !isInsideString;
            continue;
        }
        if (character === "#" && !isInsideString) {
            return line.slice(0, characterIndex);
        }
    }

    return line;
}

async function writeFileIfMissing(filePath: string, contents: string): Promise<void> {
    try {
        await writeFile(filePath, contents, { flag: "wx" });
    } catch (error) {
        if (isFileAlreadyExistsError(error)) {
            return;
        }
        throw error;
    }
}

function isFileAlreadyExistsError(error: unknown): boolean {
    return typeof error === "object" && error != null && "code" in error && error.code === "EEXIST";
}
