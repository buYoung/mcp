import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { resolveLimits } from "../src/config/limits-resolver.js";

const ENV_KEYS = [
    "ACP_BRIDGE_MAX_PAIR_SESSIONS",
    "ACP_BRIDGE_PAIR_SESSION_IDLE_TIMEOUT_MS",
    "ACP_BRIDGE_MAX_CONSULT_PANEL_AGENTS",
    "ACP_BRIDGE_OPERATION_TIMEOUT_MS",
    "ACP_BRIDGE_PROMPT_TIMEOUT_MS",
    "ACP_BRIDGE_STDERR_RING_BUFFER_CHARS",
] as const;

describe("resolveLimits", () => {
    const original = new Map<string, string | undefined>();

    beforeEach(() => {
        for (const key of ENV_KEYS) {
            original.set(key, process.env[key]);
            delete process.env[key];
        }
        process.env.ACP_BRIDGE_PROMPT_TIMEOUT_MS = "60000";
    });

    afterEach(() => {
        for (const key of ENV_KEYS) {
            const previous = original.get(key);
            if (previous == null) {
                delete process.env[key];
            } else {
                process.env[key] = previous;
            }
        }
    });

    it("uses built-in defaults when only prompt_timeout is provided via env", () => {
        const limits = resolveLimits();
        expect(limits.maxPairSessions).toBe(20);
        expect(limits.pairSessionIdleTimeoutMs).toBe(30 * 60 * 1000);
        expect(limits.maxConsultPanelAgents).toBe(5);
        expect(limits.operationTimeoutMs).toBe(180_000);
        expect(limits.promptTimeoutMs).toBe(60_000);
        expect(limits.stderrRingBufferChars).toBe(16_384);
    });

    it("TOML values win over env values", () => {
        process.env.ACP_BRIDGE_MAX_PAIR_SESSIONS = "9999";
        const limits = resolveLimits({ max_pair_sessions: 8, prompt_timeout_ms: 1000 });
        expect(limits.maxPairSessions).toBe(8);
        expect(limits.promptTimeoutMs).toBe(1000);
    });

    it("env values are honored when TOML omits a key", () => {
        process.env.ACP_BRIDGE_OPERATION_TIMEOUT_MS = "12345";
        const limits = resolveLimits({});
        expect(limits.operationTimeoutMs).toBe(12345);
    });

    it("throws when prompt_timeout is unset both in TOML and env", () => {
        delete process.env.ACP_BRIDGE_PROMPT_TIMEOUT_MS;
        expect(() => resolveLimits({})).toThrow(/prompt_timeout_ms/);
    });

    it("rejects non-positive or non-integer TOML values", () => {
        expect(() => resolveLimits({ max_pair_sessions: 0, prompt_timeout_ms: 1 })).toThrow(/max_pair_sessions/);
        expect(() => resolveLimits({ max_consult_panel_agents: -1, prompt_timeout_ms: 1 })).toThrow(
            /max_consult_panel_agents/,
        );
    });

    it("rejects malformed env values", () => {
        process.env.ACP_BRIDGE_OPERATION_TIMEOUT_MS = "abc";
        expect(() => resolveLimits({})).toThrow(/ACP_BRIDGE_OPERATION_TIMEOUT_MS/);
    });
});
