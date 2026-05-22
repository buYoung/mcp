import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { isCommandAvailable } from "../src/agents/common/binary-availability.js";

describe("isCommandAvailable", () => {
    it("returns false for empty input", async () => {
        expect(await isCommandAvailable("   ")).toBe(false);
    });

    it("returns false for a missing absolute path", async () => {
        expect(await isCommandAvailable("/this/path/definitely/does-not-exist-xyz")).toBe(false);
    });

    it("resolves a command on PATH when present", async () => {
        const candidate = process.platform === "win32" ? "cmd" : "sh";
        expect(await isCommandAvailable(candidate)).toBe(true);
    });

    describe("with empty PATH", () => {
        let originalPath: string | undefined;

        beforeEach(() => {
            originalPath = process.env.PATH;
            process.env.PATH = "";
        });

        afterEach(() => {
            if (originalPath == null) {
                delete process.env.PATH;
            } else {
                process.env.PATH = originalPath;
            }
        });

        it("returns false for a bare command name", async () => {
            expect(await isCommandAvailable("ls")).toBe(false);
        });
    });
});
