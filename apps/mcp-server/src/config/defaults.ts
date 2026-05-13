export const SUPPORTED_AGENT_IDS = ["claude-code", "codex", "gemini-cli"] as const;

export type SupportedAgentId = (typeof SUPPORTED_AGENT_IDS)[number];

export const DEFAULT_ACP_PERMISSION_POLICY = "approve_reads";

export const DEFAULT_CLAUDE_CODE_PERMISSION_MODE = "plan";

export const DEFAULT_CODEX_PERMISSION_MODE = "read-only";

export const DEFAULT_OPERATION_TIMEOUT_MS = 180_000;

export const PROMPT_TIMEOUT_ENVIRONMENT_VARIABLE = "ACP_BRIDGE_PROMPT_TIMEOUT_MS";
