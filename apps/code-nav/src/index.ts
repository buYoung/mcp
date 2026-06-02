#!/usr/bin/env node
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { SERVER_NAME, SERVER_VERSION } from "./config/defaults.js";
import { TextSearchProvider } from "./providers/text-search/text-search-provider.js";
import { ensureRequiredBinaries } from "./startup/ensure-required-binaries.js";
import { registerTools } from "./tools/index.js";

async function main(): Promise<void> {
    const binaries = await ensureRequiredBinaries();
    const repositoryRoot = process.cwd();

    const textSearchProvider = new TextSearchProvider({
        zoektIndexPath: binaries.zoektIndexPath,
        zoektWebserverPath: binaries.zoektWebserverPath,
        repositoryRoot,
    });

    let alreadyShutDown = false;
    const shutdown = (): void => {
        if (alreadyShutDown) {
            return;
        }
        alreadyShutDown = true;
        textSearchProvider.shutdown();
    };
    const shutdownAndExit = (exitCode: number): void => {
        shutdown();
        process.exit(exitCode);
    };
    process.on("SIGINT", () => shutdownAndExit(0));
    process.on("SIGTERM", () => shutdownAndExit(0));
    process.on("exit", shutdown);

    const server = new Server(
        {
            name: SERVER_NAME,
            version: SERVER_VERSION,
        },
        {
            capabilities: {
                tools: {},
            },
        },
    );

    registerTools(server, { textSearchProvider });

    // A client that closes stdin (instead of signalling) must still shut the
    // webserver child down — otherwise the live child keeps the event loop alive
    // and the process leaks an orphaned zoekt-webserver. StdioServerTransport only
    // listens for stdin "data"/"error" (never "end"/"close"), so its onclose does
    // not fire on EOF — we listen for stdin end/close ourselves.
    process.stdin.on("end", () => shutdownAndExit(0));
    process.stdin.on("close", () => shutdownAndExit(0));

    const transport = new StdioServerTransport();
    transport.onclose = () => shutdownAndExit(0);
    await server.connect(transport);
}

main().catch((error) => {
    console.error("[code-nav] fatal:", error);
    process.exit(1);
});
