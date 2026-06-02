import { type ChildProcess, spawn } from "node:child_process";
import { type AddressInfo, createServer } from "node:net";
import { WEBSERVER_HEALTH_POLL_INTERVAL_MS, WEBSERVER_HEALTH_TIMEOUT_MS } from "../../config/defaults.js";
import { httpGet } from "./http-get.js";

/**
 * Manages the long-lived `zoekt-webserver` child process bound to loopback on an
 * ephemeral port (DESIGN 결정 5). The server is kept warm across queries and is
 * restarted when the shard directory or build generation changes, or when it
 * dies. Shutdown kills the child to avoid orphaned processes.
 */
export class WebserverLifecycle {
    private readonly zoektWebserverPath: string;
    private readonly healthTimeoutMs: number;
    private readonly healthPollIntervalMs: number;

    private child: ChildProcess | undefined;
    private port: number | undefined;
    private shardDirectory: string | undefined;
    private startedGeneration = -1;
    private startPromise: Promise<number> | undefined;

    constructor(options: {
        zoektWebserverPath: string;
        healthTimeoutMs?: number;
        healthPollIntervalMs?: number;
    }) {
        this.zoektWebserverPath = options.zoektWebserverPath;
        this.healthTimeoutMs = options.healthTimeoutMs ?? WEBSERVER_HEALTH_TIMEOUT_MS;
        this.healthPollIntervalMs = options.healthPollIntervalMs ?? WEBSERVER_HEALTH_POLL_INTERVAL_MS;
    }

    async ensureRunning(shardDirectory: string, buildGeneration: number): Promise<number> {
        if (this.isHealthyFor(shardDirectory, buildGeneration) && this.port != null) {
            return this.port;
        }
        if (this.startPromise == null) {
            this.startPromise = this.restart(shardDirectory, buildGeneration);
            this.startPromise = this.startPromise.finally(() => {
                this.startPromise = undefined;
            });
        }
        return this.startPromise;
    }

    /** Forces the next `ensureRunning` to restart (used after a query connection failure). */
    markUnhealthy(): void {
        this.shutdown();
    }

    shutdown(): void {
        const child = this.child;
        this.child = undefined;
        this.port = undefined;
        this.shardDirectory = undefined;
        this.startedGeneration = -1;
        if (child != null && child.exitCode == null && child.signalCode == null) {
            child.kill("SIGTERM");
        }
    }

    private isHealthyFor(shardDirectory: string, buildGeneration: number): boolean {
        return (
            this.child != null &&
            this.child.exitCode == null &&
            this.child.signalCode == null &&
            this.shardDirectory === shardDirectory &&
            this.startedGeneration === buildGeneration
        );
    }

    private async restart(shardDirectory: string, buildGeneration: number): Promise<number> {
        this.shutdown();
        const port = await findFreePort();
        const child = spawn(this.zoektWebserverPath, ["-index", shardDirectory, "-listen", `127.0.0.1:${port}`], {
            stdio: ["ignore", "ignore", "pipe"],
            windowsHide: true,
        });
        child.stderr?.on("data", (chunk: Buffer) => {
            process.stderr.write(`[code-nav][zoekt-webserver] ${chunk.toString("utf8")}`);
        });
        child.on("exit", () => {
            if (this.child === child) {
                this.child = undefined;
                this.port = undefined;
                this.shardDirectory = undefined;
                this.startedGeneration = -1;
            }
        });

        this.child = child;
        this.port = port;
        this.shardDirectory = shardDirectory;
        this.startedGeneration = buildGeneration;

        await this.waitForHealth(port);
        return port;
    }

    private async waitForHealth(port: number): Promise<void> {
        const deadline = Date.now() + this.healthTimeoutMs;
        const healthUrl = `http://127.0.0.1:${port}/`;
        while (Date.now() < deadline) {
            if (this.child == null || this.child.exitCode != null || this.child.signalCode != null) {
                throw new Error("zoekt-webserver exited before becoming healthy.");
            }
            try {
                const response = await httpGet(healthUrl, this.healthPollIntervalMs * 4);
                if (response.statusCode > 0) {
                    return;
                }
            } catch {
                // not up yet; keep polling
            }
            await delay(this.healthPollIntervalMs);
        }
        throw new Error(`zoekt-webserver did not become healthy within ${this.healthTimeoutMs}ms.`);
    }
}

function findFreePort(): Promise<number> {
    return new Promise<number>((resolve, reject) => {
        const probeServer = createServer();
        probeServer.once("error", reject);
        probeServer.listen(0, "127.0.0.1", () => {
            const address = probeServer.address() as AddressInfo | null;
            if (address == null) {
                probeServer.close();
                reject(new Error("Failed to acquire a free port for zoekt-webserver."));
                return;
            }
            const { port } = address;
            probeServer.close(() => {
                resolve(port);
            });
        });
    });
}

function delay(milliseconds: number): Promise<void> {
    return new Promise<void>((resolve) => {
        setTimeout(resolve, milliseconds);
    });
}
