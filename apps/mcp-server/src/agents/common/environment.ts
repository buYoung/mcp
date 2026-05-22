import {
    DEFAULT_OPERATION_TIMEOUT_MS,
    DEFAULT_PERMISSION_PROFILE,
    isPermissionProfile,
    OPERATION_TIMEOUT_ENVIRONMENT_VARIABLE,
    PERMISSION_PROFILE_ENVIRONMENT_VARIABLE,
    PERMISSION_PROFILES,
    type PermissionProfile,
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

// Backwards-compatible alias. New code should import `PermissionProfile` directly.
export type PermissionPolicy = PermissionProfile;

export function readAgentCommandConfig(environmentPrefix: string, defaults: AgentCommandDefaults): AgentCommandConfig {
    const command = readEnvironmentString(`${environmentPrefix}_COMMAND`);
    const commandArguments = readEnvironmentStringArray(`${environmentPrefix}_ARGS`);

    return {
        command: command ?? defaults.command,
        commandArguments: commandArguments ?? (command == null ? [...defaults.commandArguments] : []),
    };
}

export function readDefaultPermissionProfile(): PermissionProfile {
    const value = readEnvironmentString(PERMISSION_PROFILE_ENVIRONMENT_VARIABLE);
    if (value == null) {
        return DEFAULT_PERMISSION_PROFILE;
    }
    if (!isPermissionProfile(value)) {
        throw new Error(
            `Expected ${PERMISSION_PROFILE_ENVIRONMENT_VARIABLE} to be one of: ${PERMISSION_PROFILES.join(", ")}`,
        );
    }
    return value;
}

export function resolvePermissionProfile(
    perAgentValue: string | undefined,
    fallback: PermissionProfile = readDefaultPermissionProfile(),
): PermissionProfile {
    if (perAgentValue == null || perAgentValue.trim().length === 0) {
        return fallback;
    }
    const trimmed = perAgentValue.trim();
    if (!isPermissionProfile(trimmed)) {
        throw new Error(
            `Invalid per-agent permission profile "${trimmed}". Expected one of: ${PERMISSION_PROFILES.join(", ")}`,
        );
    }
    return trimmed;
}

export function readOperationTimeoutMs(): number {
    const value = readEnvironmentString(OPERATION_TIMEOUT_ENVIRONMENT_VARIABLE);
    if (value == null) {
        return DEFAULT_OPERATION_TIMEOUT_MS;
    }
    return parsePositiveInteger(OPERATION_TIMEOUT_ENVIRONMENT_VARIABLE, value);
}

export function readPromptTimeoutMs(): number {
    const value = readEnvironmentString(PROMPT_TIMEOUT_ENVIRONMENT_VARIABLE);
    if (value == null) {
        throw new Error(`Expected ${PROMPT_TIMEOUT_ENVIRONMENT_VARIABLE} to be a positive integer in milliseconds.`);
    }
    return parsePositiveInteger(PROMPT_TIMEOUT_ENVIRONMENT_VARIABLE, value);
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

function parsePositiveInteger(name: string, rawValue: string): number {
    if (!/^[1-9]\d*$/.test(rawValue)) {
        throw new Error(`Expected ${name} to be a positive integer in milliseconds.`);
    }
    const parsedValue = Number(rawValue);
    if (!Number.isSafeInteger(parsedValue)) {
        throw new Error(`Expected ${name} to be a positive integer in milliseconds.`);
    }
    return parsedValue;
}
