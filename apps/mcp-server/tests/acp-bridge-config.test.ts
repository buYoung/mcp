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

    it("parses [limits] integers", async () => {
        const configurationPath = join(baseDirectory, ".acp_bridge", "config.toml");
        await ensureAcpBridgeConfiguration(baseDirectory);
        await writeFile(
            configurationPath,
            [
                "[limits]",
                "max_pair_sessions = 7",
                "pair_session_idle_timeout_ms = 5000",
                "max_consult_panel_agents = 3",
                "operation_timeout_ms = 9000",
                "prompt_timeout_ms = 4000",
                "stderr_ring_buffer_chars = 2048",
                "",
            ].join("\n"),
        );

        const configuration = await ensureAcpBridgeConfiguration(baseDirectory);
        expect(configuration.limits).toEqual({
            max_pair_sessions: 7,
            pair_session_idle_timeout_ms: 5000,
            max_consult_panel_agents: 3,
            operation_timeout_ms: 9000,
            prompt_timeout_ms: 4000,
            stderr_ring_buffer_chars: 2048,
        });
    });

    it("rejects negative or non-integer [limits]", async () => {
        const configurationPath = join(baseDirectory, ".acp_bridge", "config.toml");
        await ensureAcpBridgeConfiguration(baseDirectory);
        await writeFile(configurationPath, ["[limits]", "max_pair_sessions = 0", ""].join("\n"));
        await expect(ensureAcpBridgeConfiguration(baseDirectory)).rejects.toThrow(/max_pair_sessions/);
    });

    it("rejects unknown [limits] keys", async () => {
        const configurationPath = join(baseDirectory, ".acp_bridge", "config.toml");
        await ensureAcpBridgeConfiguration(baseDirectory);
        await writeFile(configurationPath, ["[limits]", "unknown_limit = 42", ""].join("\n"));
        await expect(ensureAcpBridgeConfiguration(baseDirectory)).rejects.toThrow(/unknown_limit/);
    });
});
