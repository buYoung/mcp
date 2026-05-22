import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { parse as parseToml } from "smol-toml";
import { SUPPORTED_AGENT_IDS } from "./defaults.js";

export interface AcpBridgeAgentConfiguration {
    model?: string;
    permission?: string;
    reasoning?: string;
}

export interface AcpBridgeLimitsConfiguration {
    max_pair_sessions?: number;
    pair_session_idle_timeout_ms?: number;
    max_consult_panel_agents?: number;
    operation_timeout_ms?: number;
    prompt_timeout_ms?: number;
    stderr_ring_buffer_chars?: number;
}

export interface AcpBridgeConfiguration {
    configurationPath: string;
    agents: Record<string, AcpBridgeAgentConfiguration>;
    limits: AcpBridgeLimitsConfiguration;
}

const CONFIG_DIRECTORY_NAME = ".acp_bridge";
const CONFIG_FILE_NAME = "config.toml";
const [CLAUDE_CODE_AGENT_ID, CODEX_AGENT_ID, GEMINI_CLI_AGENT_ID] = SUPPORTED_AGENT_IDS;

const SUPPORTED_AGENT_KEYS = new Set<keyof AcpBridgeAgentConfiguration>(["model", "permission", "reasoning"]);
const SUPPORTED_LIMIT_KEYS = new Set<keyof AcpBridgeLimitsConfiguration>([
    "max_pair_sessions",
    "pair_session_idle_timeout_ms",
    "max_consult_panel_agents",
    "operation_timeout_ms",
    "prompt_timeout_ms",
    "stderr_ring_buffer_chars",
]);

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

# Operational limits. Omit a key to fall back to ACP_BRIDGE_* env var, then the built-in default.
[limits]
# max_pair_sessions = 20
# pair_session_idle_timeout_ms = 1800000
# max_consult_panel_agents = 5
# operation_timeout_ms = 180000
# prompt_timeout_ms = 600000
# stderr_ring_buffer_chars = 16384
`;

export async function ensureAcpBridgeConfiguration(baseDirectory = process.cwd()): Promise<AcpBridgeConfiguration> {
    const configurationDirectoryPath = join(baseDirectory, CONFIG_DIRECTORY_NAME);
    const configurationPath = join(configurationDirectoryPath, CONFIG_FILE_NAME);

    await mkdir(configurationDirectoryPath, { recursive: true });
    await writeFileIfMissing(configurationPath, DEFAULT_CONFIG_TEMPLATE);

    const configurationContents = await readFile(configurationPath, "utf8");
    let parsed: Record<string, unknown>;
    try {
        parsed = parseToml(configurationContents) as Record<string, unknown>;
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new Error(`Failed to parse ${configurationPath}: ${message}`);
    }

    return {
        configurationPath,
        agents: parseAgentConfigurations(parsed, configurationPath),
        limits: parseLimitsConfiguration(parsed, configurationPath),
    };
}

function parseAgentConfigurations(
    parsed: Record<string, unknown>,
    configurationPath: string,
): Record<string, AcpBridgeAgentConfiguration> {
    const agents: Record<string, AcpBridgeAgentConfiguration> = {};
    for (const agentId of SUPPORTED_AGENT_IDS) {
        agents[agentId] = {};
    }

    const agentsTable = parsed.agents;
    if (agentsTable == null) {
        return agents;
    }
    if (typeof agentsTable !== "object" || Array.isArray(agentsTable)) {
        throw new Error(`Expected [agents] table at ${configurationPath}`);
    }

    for (const [agentId, agentValue] of Object.entries(agentsTable as Record<string, unknown>)) {
        if (!SUPPORTED_AGENT_IDS.includes(agentId as (typeof SUPPORTED_AGENT_IDS)[number])) {
            throw new Error(`Unsupported agent id "${agentId}" at ${configurationPath}`);
        }
        if (typeof agentValue !== "object" || agentValue == null || Array.isArray(agentValue)) {
            throw new Error(`Expected [agents.${agentId}] table at ${configurationPath}`);
        }
        const agentConfiguration: AcpBridgeAgentConfiguration = {};
        for (const [key, value] of Object.entries(agentValue as Record<string, unknown>)) {
            if (!SUPPORTED_AGENT_KEYS.has(key as keyof AcpBridgeAgentConfiguration)) {
                throw new Error(`Unsupported agent configuration key "${key}" at ${configurationPath}`);
            }
            if (typeof value !== "string") {
                throw new Error(
                    `Expected agents.${agentId}.${key} to be a string at ${configurationPath}; got ${typeof value}`,
                );
            }
            const trimmed = value.trim();
            if (trimmed.length === 0) {
                continue;
            }
            if (agentId === GEMINI_CLI_AGENT_ID && key === "reasoning") {
                throw new Error(`Gemini CLI does not support reasoning at ${configurationPath}`);
            }
            agentConfiguration[key as keyof AcpBridgeAgentConfiguration] = trimmed;
        }
        agents[agentId] = agentConfiguration;
    }

    return agents;
}

function parseLimitsConfiguration(
    parsed: Record<string, unknown>,
    configurationPath: string,
): AcpBridgeLimitsConfiguration {
    const limitsTable = parsed.limits;
    if (limitsTable == null) {
        return {};
    }
    if (typeof limitsTable !== "object" || Array.isArray(limitsTable)) {
        throw new Error(`Expected [limits] table at ${configurationPath}`);
    }

    const limits: AcpBridgeLimitsConfiguration = {};
    for (const [key, value] of Object.entries(limitsTable as Record<string, unknown>)) {
        if (!SUPPORTED_LIMIT_KEYS.has(key as keyof AcpBridgeLimitsConfiguration)) {
            throw new Error(`Unsupported limits key "${key}" at ${configurationPath}`);
        }
        if (typeof value !== "number" && typeof value !== "bigint") {
            throw new Error(
                `Expected limits.${key} to be a positive integer at ${configurationPath}; got ${typeof value}`,
            );
        }
        const numericValue = typeof value === "bigint" ? Number(value) : value;
        if (!Number.isSafeInteger(numericValue) || numericValue <= 0) {
            throw new Error(`Expected limits.${key} to be a positive integer at ${configurationPath}; got ${value}`);
        }
        limits[key as keyof AcpBridgeLimitsConfiguration] = numericValue;
    }
    return limits;
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
