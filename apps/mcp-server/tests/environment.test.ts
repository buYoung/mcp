import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
    readDefaultPermissionProfile,
    readOperationTimeoutMs,
    readPromptTimeoutMs,
    resolvePermissionProfile,
} from "../src/agents/common/environment.js";

const ENV_KEYS = [
    "ACP_BRIDGE_PERMISSION_POLICY",
    "ACP_BRIDGE_OPERATION_TIMEOUT_MS",
    "ACP_BRIDGE_PROMPT_TIMEOUT_MS",
] as const;

describe("readDefaultPermissionProfile", () => {
    const original = new Map<string, string | undefined>();

    beforeEach(() => {
        for (const key of ENV_KEYS) {
            original.set(key, process.env[key]);
            delete process.env[key];
        }
    });

    afterEach(() => {
        for (const key of ENV_KEYS) {
            const previousValue = original.get(key);
            if (previousValue == null) {
                delete process.env[key];
            } else {
                process.env[key] = previousValue;
            }
        }
    });

    it("falls back to read-only when env is unset", () => {
        expect(readDefaultPermissionProfile()).toBe("read-only");
    });

    it("accepts each valid profile", () => {
        for (const profile of ["read-only", "edit", "full"] as const) {
            process.env.ACP_BRIDGE_PERMISSION_POLICY = profile;
            expect(readDefaultPermissionProfile()).toBe(profile);
        }
    });

    it("throws on invalid values", () => {
        process.env.ACP_BRIDGE_PERMISSION_POLICY = "approve_reads";
        expect(() => readDefaultPermissionProfile()).toThrow(/one of/);
    });

    it("treats whitespace-only env as unset", () => {
        process.env.ACP_BRIDGE_PERMISSION_POLICY = "   ";
        expect(readDefaultPermissionProfile()).toBe("read-only");
    });
});

describe("resolvePermissionProfile", () => {
    it("uses per-agent value when set", () => {
        expect(resolvePermissionProfile("edit", "read-only")).toBe("edit");
    });

    it("trims whitespace", () => {
        expect(resolvePermissionProfile("  full  ", "read-only")).toBe("full");
    });

    it("falls back when per-agent value is empty", () => {
        expect(resolvePermissionProfile("", "edit")).toBe("edit");
        expect(resolvePermissionProfile(undefined, "edit")).toBe("edit");
    });

    it("throws on invalid per-agent value", () => {
        expect(() => resolvePermissionProfile("yolo", "read-only")).toThrow(/Invalid per-agent/);
    });
});

describe("readOperationTimeoutMs", () => {
    const ENV_KEY = "ACP_BRIDGE_OPERATION_TIMEOUT_MS";
    const original = process.env[ENV_KEY];

    afterEach(() => {
        if (original == null) {
            delete process.env[ENV_KEY];
        } else {
            process.env[ENV_KEY] = original;
        }
    });

    it("returns default when unset", () => {
        delete process.env[ENV_KEY];
        expect(readOperationTimeoutMs()).toBe(180_000);
    });

    it("returns parsed env when set", () => {
        process.env[ENV_KEY] = "5000";
        expect(readOperationTimeoutMs()).toBe(5000);
    });

    it("throws on zero / negative / non-integer", () => {
        for (const invalid of ["0", "-1", "1.5", "abc", " 5 "]) {
            process.env[ENV_KEY] = invalid;
            expect(() => readOperationTimeoutMs()).toThrow();
        }
    });
});

describe("readPromptTimeoutMs", () => {
    const ENV_KEY = "ACP_BRIDGE_PROMPT_TIMEOUT_MS";
    const original = process.env[ENV_KEY];

    afterEach(() => {
        if (original == null) {
            delete process.env[ENV_KEY];
        } else {
            process.env[ENV_KEY] = original;
        }
    });

    it("throws when unset", () => {
        delete process.env[ENV_KEY];
        expect(() => readPromptTimeoutMs()).toThrow();
    });

    it("returns positive int when set", () => {
        process.env[ENV_KEY] = "60000";
        expect(readPromptTimeoutMs()).toBe(60000);
    });
});
