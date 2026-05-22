import {
    DEFAULT_PERMISSION_PROFILE,
    isPermissionProfile,
    PERMISSION_PROFILE_ENVIRONMENT_VARIABLE,
    PERMISSION_PROFILES,
    type PermissionProfile,
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
