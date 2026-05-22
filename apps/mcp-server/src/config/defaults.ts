export const SUPPORTED_AGENT_IDS = ["claude-code", "codex", "gemini-cli"] as const;

export type SupportedAgentId = (typeof SUPPORTED_AGENT_IDS)[number];

export const PERMISSION_PROFILES = ["read-only", "edit", "full"] as const;

export type PermissionProfile = (typeof PERMISSION_PROFILES)[number];

export const DEFAULT_PERMISSION_PROFILE: PermissionProfile = "read-only";

export const PERMISSION_PROFILE_ENVIRONMENT_VARIABLE = "ACP_BRIDGE_PERMISSION_POLICY";

export const DEFAULT_CLAUDE_CODE_PERMISSION_MODE = "plan";

export const DEFAULT_CODEX_PERMISSION_MODE = "read-only";

export const DEFAULT_OPERATION_TIMEOUT_MS = 180_000;

export function isPermissionProfile(value: unknown): value is PermissionProfile {
    return typeof value === "string" && (PERMISSION_PROFILES as readonly string[]).includes(value);
}
