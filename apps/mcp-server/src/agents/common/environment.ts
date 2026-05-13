import {
    DEFAULT_ACP_PERMISSION_POLICY,
    DEFAULT_OPERATION_TIMEOUT_MS,
    PROMPT_TIMEOUT_ENVIRONMENT_VARIABLE,
} from "../../config/defaults.js";

export interface AgentCommandDefaults {
    command: string;
    commandArguments: readonly string[];
}

export interface AgentCommandConfig {
    command: string;
    commandArguments: readonly string[];
}

export type PermissionPolicy = typeof DEFAULT_ACP_PERMISSION_POLICY;

export function readAgentCommandConfig(environmentPrefix: string, defaults: AgentCommandDefaults): AgentCommandConfig {
    const command = readEnvironmentString(`${environmentPrefix}_COMMAND`);
    const commandArguments = readEnvironmentStringArray(`${environmentPrefix}_ARGS`);

    return {
        command: command ?? defaults.command,
        commandArguments: commandArguments ?? (command == null ? [...defaults.commandArguments] : []),
    };
}

export function readPermissionPolicy(): PermissionPolicy {
    readEnvironmentString("ACP_BRIDGE_PERMISSION_POLICY");
    return DEFAULT_ACP_PERMISSION_POLICY;
}

export function readOperationTimeoutMs(): number {
    return DEFAULT_OPERATION_TIMEOUT_MS;
}

export function readPromptTimeoutMs(): number {
    return readRequiredPositiveInteger(PROMPT_TIMEOUT_ENVIRONMENT_VARIABLE);
}

function readEnvironmentString(name: string): string | undefined {
    const value = process.env[name];
    if (value == null || value.trim().length === 0) {
        return undefined;
    }
    return value;
}

function readEnvironmentStringArray(name: string): string[] | undefined {
    const value = readEnvironmentString(name);
    if (value == null) {
        return undefined;
    }

    const parsedValue = JSON.parse(value) as unknown;
    if (!Array.isArray(parsedValue) || parsedValue.some((item) => typeof item !== "string")) {
        throw new Error(`Expected ${name} to be a JSON string array.`);
    }

    return parsedValue;
}

function readRequiredPositiveInteger(name: string): number {
    const value = readEnvironmentString(name);
    if (value == null) {
        throw new Error(`Expected ${name} to be a positive integer in milliseconds.`);
    }
    if (!/^[1-9]\d*$/.test(value)) {
        throw new Error(`Expected ${name} to be a positive integer in milliseconds.`);
    }

    const parsedValue = Number(value);
    if (!Number.isSafeInteger(parsedValue)) {
        throw new Error(`Expected ${name} to be a positive integer in milliseconds.`);
    }
    return parsedValue;
}
