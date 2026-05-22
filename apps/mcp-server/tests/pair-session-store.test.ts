import { describe, expect, it, vi } from "vitest";
import type { AgentAdapter } from "../src/agents/common/types.js";
import { PairSessionStore } from "../src/tools/pair-session-store.js";

function fakeAgent(overrides: Partial<AgentAdapter> = {}): AgentAdapter {
    return {
        id: overrides.id ?? "fake",
        label: "Fake",
        command: "fake",
        async askPair() {
            throw new Error("not used");
        },
        async continuePair() {
            throw new Error("not used");
        },
        async closePair() {
            // no-op
        },
        ...overrides,
    };
}

describe("PairSessionStore", () => {
    it("stores and retrieves a session", async () => {
        const store = new PairSessionStore({ idleTimeoutMs: 60_000, maxSessions: 10, resolveAgent: () => fakeAgent() });
        await store.remember("s1", "fake");
        expect(store.get("s1")?.agentId).toBe("fake");
    });

    it("evicts the oldest session when over capacity", async () => {
        const closeMock = vi.fn().mockResolvedValue(undefined);
        const store = new PairSessionStore({
            idleTimeoutMs: 60_000,
            maxSessions: 2,
            resolveAgent: () => fakeAgent({ closePair: closeMock }),
        });
        await store.remember("a", "fake");
        await store.remember("b", "fake");
        await store.remember("c", "fake");
        expect(store.get("a")).toBeUndefined();
        expect(store.get("b")?.agentId).toBe("fake");
        expect(store.get("c")?.agentId).toBe("fake");
        expect(closeMock).toHaveBeenCalledWith("a");
    });

    it("cleans up expired sessions", async () => {
        const closeMock = vi.fn().mockResolvedValue(undefined);
        const store = new PairSessionStore({
            idleTimeoutMs: 10,
            maxSessions: 10,
            resolveAgent: () => fakeAgent({ closePair: closeMock }),
        });
        await store.remember("old", "fake");
        await new Promise((resolveSleep) => setTimeout(resolveSleep, 30));
        await store.cleanupExpired();
        expect(store.get("old")).toBeUndefined();
        expect(closeMock).toHaveBeenCalledWith("old");
    });

    it("close returns false for unknown session", async () => {
        const store = new PairSessionStore({ idleTimeoutMs: 60_000, maxSessions: 10, resolveAgent: () => undefined });
        expect(await store.close("missing")).toBe(false);
    });
});
