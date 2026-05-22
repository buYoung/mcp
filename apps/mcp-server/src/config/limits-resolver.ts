import type { AcpBridgeLimitsConfiguration } from "./acp-bridge-config.js";
import { DEFAULT_OPERATION_TIMEOUT_MS } from "./defaults.js";

export interface ResolvedLimits {
    maxPairSessions: number;
    pairSessionIdleTimeoutMs: number;
    maxConsultPanelAgents: number;
    operationTimeoutMs: number;
    promptTimeoutMs: number;
    stderrRingBufferChars: number;
}

const DEFAULT_LIMITS: ResolvedLimits = {
    maxPairSessions: 20,
    pairSessionIdleTimeoutMs: 30 * 60 * 1000,
    maxConsultPanelAgents: 5,
    operationTimeoutMs: DEFAULT_OPERATION_TIMEOUT_MS,
    promptTimeoutMs: 0,
    stderrRingBufferChars: 16_384,
};

export function resolveLimits(tomlLimits: AcpBridgeLimitsConfiguration = {}): ResolvedLimits {
    return {
        maxPairSessions: pickPositiveInteger(
            "max_pair_sessions",
            tomlLimits.max_pair_sessions,
            "ACP_BRIDGE_MAX_PAIR_SESSIONS",
            DEFAULT_LIMITS.maxPairSessions,
            { allowZero: false },
        ),
        pairSessionIdleTimeoutMs: pickPositiveInteger(
            "pair_session_idle_timeout_ms",
            tomlLimits.pair_session_idle_timeout_ms,
            "ACP_BRIDGE_PAIR_SESSION_IDLE_TIMEOUT_MS",
            DEFAULT_LIMITS.pairSessionIdleTimeoutMs,
            { allowZero: false },
        ),
        maxConsultPanelAgents: pickPositiveInteger(
            "max_consult_panel_agents",
            tomlLimits.max_consult_panel_agents,
            "ACP_BRIDGE_MAX_CONSULT_PANEL_AGENTS",
            DEFAULT_LIMITS.maxConsultPanelAgents,
            { allowZero: false },
        ),
        operationTimeoutMs: pickPositiveInteger(
            "operation_timeout_ms",
            tomlLimits.operation_timeout_ms,
            "ACP_BRIDGE_OPERATION_TIMEOUT_MS",
            DEFAULT_LIMITS.operationTimeoutMs,
            { allowZero: false },
        ),
        promptTimeoutMs: pickRequiredPositiveInteger(
            "prompt_timeout_ms",
            tomlLimits.prompt_timeout_ms,
            "ACP_BRIDGE_PROMPT_TIMEOUT_MS",
        ),
        stderrRingBufferChars: pickPositiveInteger(
            "stderr_ring_buffer_chars",
            tomlLimits.stderr_ring_buffer_chars,
            "ACP_BRIDGE_STDERR_RING_BUFFER_CHARS",
            DEFAULT_LIMITS.stderrRingBufferChars,
            { allowZero: false },
        ),
    };
}

function pickPositiveInteger(
    tomlKey: string,
    tomlValue: number | undefined,
    envKey: string,
    fallbackValue: number,
    options: { allowZero: boolean },
): number {
    const candidate = tomlValue ?? parsePositiveIntegerEnv(envKey);
    if (candidate == null) {
        return fallbackValue;
    }
    if (!Number.isSafeInteger(candidate) || (options.allowZero ? candidate < 0 : candidate <= 0)) {
        throw new Error(`Invalid limits.${tomlKey} / ${envKey} value: ${candidate}`);
    }
    return candidate;
}

function pickRequiredPositiveInteger(tomlKey: string, tomlValue: number | undefined, envKey: string): number {
    const candidate = tomlValue ?? parsePositiveIntegerEnv(envKey);
    if (candidate == null) {
        throw new Error(`Missing limits.${tomlKey}. Set the TOML key or ${envKey} to a positive integer.`);
    }
    if (!Number.isSafeInteger(candidate) || candidate <= 0) {
        throw new Error(`Invalid limits.${tomlKey} / ${envKey} value: ${candidate}`);
    }
    return candidate;
}

function parsePositiveIntegerEnv(envKey: string): number | undefined {
    const value = process.env[envKey];
    if (value == null || value.trim().length === 0) {
        return undefined;
    }
    if (!/^[1-9]\d*$/.test(value)) {
        throw new Error(`Expected ${envKey} to be a positive integer; got "${value}".`);
    }
    return Number(value);
}
