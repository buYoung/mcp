import { describe, expect, it } from "vitest";
import {
    extractToolCallArgv,
    extractToolCallPaths,
    normalizeToolCall,
    tokenizeShellCommand,
} from "../src/acp/tool-call-extraction.js";

describe("tokenizeShellCommand", () => {
    it("splits on whitespace", () => {
        expect(tokenizeShellCommand("git push --force origin main")).toEqual([
            "git",
            "push",
            "--force",
            "origin",
            "main",
        ]);
    });

    it("respects single quotes", () => {
        expect(tokenizeShellCommand("rm -rf 'a b'")).toEqual(["rm", "-rf", "a b"]);
    });

    it("respects double quotes with escapes", () => {
        expect(tokenizeShellCommand('echo "a\\"b"')).toEqual(["echo", 'a"b']);
    });

    it("handles bare backslash escapes outside quotes", () => {
        expect(tokenizeShellCommand("echo a\\ b")).toEqual(["echo", "a b"]);
    });

    it("returns empty for whitespace-only input", () => {
        expect(tokenizeShellCommand("   \t  ")).toEqual([]);
    });
});

describe("extractToolCallArgv", () => {
    it("returns empty for non-object input", () => {
        expect(extractToolCallArgv(null).argv).toEqual([]);
        expect(extractToolCallArgv("rm -rf /").argv).toEqual([]);
    });

    it("uses argv-shaped command field", () => {
        const { argv } = extractToolCallArgv({ command: ["git", "push", "--force"] });
        expect(argv).toEqual(["git", "push", "--force"]);
    });

    it("tokenizes string command field", () => {
        const { argv, rawCommand } = extractToolCallArgv({ command: "git push --force" });
        expect(argv).toEqual(["git", "push", "--force"]);
        expect(rawCommand).toBe("git push --force");
    });

    it("unwraps sh -c wrappers", () => {
        const { argv } = extractToolCallArgv({ command: ["bash", "-c", "rm -rf /tmp/x"] });
        expect(argv).toEqual(["rm", "-rf", "/tmp/x"]);
    });

    it("unwraps zsh -c with absolute path", () => {
        const { argv } = extractToolCallArgv({ command: ["/bin/zsh", "-c", "git push --force"] });
        expect(argv).toEqual(["git", "push", "--force"]);
    });

    it("prefers explicit argv over commandLine string", () => {
        const { argv } = extractToolCallArgv({ argv: ["ls", "-la"], commandLine: "ignored" });
        expect(argv).toEqual(["ls", "-la"]);
    });
});

describe("extractToolCallPaths", () => {
    it("deduplicates across locations and rawInput", () => {
        const paths = extractToolCallPaths({ path: "/a/b" }, [{ path: "/a/b" }, { path: "/c/d" }]);
        expect([...paths].sort()).toEqual(["/a/b", "/c/d"]);
    });

    it("ignores empty or non-string paths", () => {
        const paths = extractToolCallPaths({ path: "" }, [{ path: null }, { path: 42 } as never]);
        expect(paths).toEqual([]);
    });
});

describe("normalizeToolCall", () => {
    it("combines argv and paths", () => {
        const result = normalizeToolCall({ command: ["rm", "-rf", "/tmp/x"], path: "/tmp/x" }, [{ path: "/tmp/x" }]);
        expect(result.argv).toEqual(["rm", "-rf", "/tmp/x"]);
        expect(result.paths).toEqual(["/tmp/x"]);
    });
});
