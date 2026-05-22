import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { beforeEach, describe, expect, it } from "vitest";
import { ensureAcpBridgeConfiguration } from "../src/config/acp-bridge-config.js";

describe("ensureAcpBridgeConfiguration", () => {
    let baseDirectory: string;

    beforeEach(async () => {
        baseDirectory = await mkdtemp(join(tmpdir(), "acp-bridge-config-"));
    });

    it("creates the default template and parses to empty agent overrides", async () => {
        const configuration = await ensureAcpBridgeConfiguration(baseDirectory);
        expect(configuration.agents["claude-code"]).toEqual({});
        expect(configuration.agents.codex).toEqual({});
        expect(configuration.agents["gemini-cli"]).toEqual({});
    });

    it("parses model / permission / reasoning values", async () => {
        const configurationPath = join(baseDirectory, ".acp_bridge", "config.toml");
        await ensureAcpBridgeConfiguration(baseDirectory);
        await writeFile(
            configurationPath,
            [
                "[agents.claude-code]",
                'model = "claude-opus-4-7"',
                'permission = "edit"',
                'reasoning = "high"',
                "",
                "[agents.codex]",
                'model = "gpt-5"',
                "",
                "[agents.gemini-cli]",
                'model = "gemini-2.5-pro"',
                "",
            ].join("\n"),
        );

        const configuration = await ensureAcpBridgeConfiguration(baseDirectory);
        expect(configuration.agents["claude-code"]).toEqual({
            model: "claude-opus-4-7",
            permission: "edit",
            reasoning: "high",
        });
        expect(configuration.agents.codex).toEqual({ model: "gpt-5" });
        expect(configuration.agents["gemini-cli"]).toEqual({ model: "gemini-2.5-pro" });
    });

    it("strips inline comments and ignores blank lines", async () => {
        const configurationPath = join(baseDirectory, ".acp_bridge", "config.toml");
        await ensureAcpBridgeConfiguration(baseDirectory);
        await writeFile(
            configurationPath,
            [
                "# top-level comment",
                "[agents.claude-code]",
                'model = "claude-opus-4-7" # inline',
                'permission = "" # empty values are ignored',
                "",
            ].join("\n"),
        );

        const configuration = await ensureAcpBridgeConfiguration(baseDirectory);
        expect(configuration.agents["claude-code"]).toEqual({ model: "claude-opus-4-7" });
    });

    it("rejects reasoning for gemini-cli", async () => {
        const configurationPath = join(baseDirectory, ".acp_bridge", "config.toml");
        await ensureAcpBridgeConfiguration(baseDirectory);
        await writeFile(configurationPath, ["[agents.gemini-cli]", 'reasoning = "high"', ""].join("\n"));

        await expect(ensureAcpBridgeConfiguration(baseDirectory)).rejects.toThrow(/does not support reasoning/);
    });

    it("throws on unsupported entries", async () => {
        const configurationPath = join(baseDirectory, ".acp_bridge", "config.toml");
        await ensureAcpBridgeConfiguration(baseDirectory);
        await writeFile(configurationPath, ["[agents.claude-code]", "weird = true", ""].join("\n"));

        await expect(ensureAcpBridgeConfiguration(baseDirectory)).rejects.toThrow();
    });
});
