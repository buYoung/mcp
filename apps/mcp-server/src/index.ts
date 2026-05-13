#!/usr/bin/env node
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { registerDefaultAgents } from "./agents/register.js";
import { ensureAcpBridgeConfiguration } from "./config/acp-bridge-config.js";
import { registerTools } from "./tools/index.js";

async function main(): Promise<void> {
    const server = new Server(
        {
            name: "acp-bridge",
            version: "0.0.0",
        },
        {
            capabilities: {
                tools: {},
            },
        },
    );

    const configuration = await ensureAcpBridgeConfiguration();

    registerDefaultAgents(configuration);
    registerTools(server);

    const transport = new StdioServerTransport();
    await server.connect(transport);
}

main().catch((err) => {
    console.error("[acp-bridge] fatal:", err);
    process.exit(1);
});
