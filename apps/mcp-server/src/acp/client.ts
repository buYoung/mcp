import { type ChildProcessWithoutNullStreams, spawn } from "node:child_process";
import { Readable, Writable } from "node:stream";
import {
    type Client,
    ClientSideConnection,
    ndJsonStream,
    type PermissionOptionKind,
    PROTOCOL_VERSION,
    type RequestPermissionRequest,
    type RequestPermissionResponse,
    type SessionConfigOption,
    type SessionNotification,
    type SetSessionModeRequest,
    type SetSessionModeResponse,
    type ToolKind,
} from "@agentclientprotocol/sdk";
import {
    isPairSessionClosedError,
    type PairAskInput,
    type PairContinueInput,
    PairSessionClosedError,
    type PairTurnMeta,
    type PairTurnResult,
} from "../agents/common/types.js";
import {
    DEFAULT_OPERATION_TIMEOUT_MS,
    DEFAULT_PERMISSION_PROFILE,
    type PermissionProfile,
} from "../config/defaults.js";
import { parseJsonAnswer } from "../tools/json-extract.js";
import { type LayerZeroHit, layerZeroCheckFromRawInput } from "./layer-zero.js";
import { decidePermission } from "./permission-decision.js";

export interface AcpAgentLaunchOptions {
    command: string;
    commandArguments?: readonly string[];
    configOptionOrder?: readonly string[];
    configOptions?: Readonly<Record<string, string>>;
    cwd?: string;
    environmentVariables?: NodeJS.ProcessEnv;
    model?: string;
    mode?: string;
    permissionProfile?: PermissionProfile;
    operationTimeoutMs?: number;
    promptTimeoutMs?: number;
}

export interface AcpAgentSession {
    askPair(input: PairAskInput): Promise<PairTurnResult>;
    continuePair(input: PairContinueInput): Promise<PairTurnResult>;
    close(): Promise<void>;
}

export async function launchAcpAgent(_options: AcpAgentLaunchOptions): Promise<AcpAgentSession> {
    const options = {
        commandArguments: [] as readonly string[],
        configOptionOrder: [] as readonly string[],
        configOptions: {} as Readonly<Record<string, string>>,
        cwd: process.cwd(),
        environmentVariables: {},
        operationTimeoutMs: DEFAULT_OPERATION_TIMEOUT_MS,
        permissionProfile: DEFAULT_PERMISSION_PROFILE as PermissionProfile,
        promptTimeoutMs: undefined as number | undefined,
        ..._options,
    };
    if (options.promptTimeoutMs == null) {
        throw new Error("ACP prompt timeout is required. Set ACP_BRIDGE_PROMPT_TIMEOUT_MS to a positive integer.");
    }

    const agentProcess = spawn(options.command, [...options.commandArguments], {
        cwd: options.cwd,
        env: {
            ...process.env,
            ...options.environmentVariables,
        },
        stdio: ["pipe", "pipe", "pipe"],
    });

    const session = new StdioAcpAgentSession(
        agentProcess,
        options.permissionProfile,
        options.cwd,
        options.configOptionOrder,
        options.configOptions,
        options.model,
        options.mode,
        options.operationTimeoutMs,
        options.promptTimeoutMs,
    );
    try {
        await session.initialize();
        return session;
    } catch (error) {
        await session.close();
        throw error;
    }
}

class StdioAcpAgentSession implements AcpAgentSession {
    private readonly client: AcpBridgeClient;
    private readonly connection: ClientSideConnection;
    private readonly processFailure: Promise<never>;
    private stderrOutput = "";

    constructor(
        private readonly agentProcess: ChildProcessWithoutNullStreams,
        permissionProfile: PermissionProfile,
        private readonly cwd: string,
        private readonly configOptionOrder: readonly string[],
        private readonly configOptions: Readonly<Record<string, string>>,
        private readonly model: string | undefined,
        private readonly mode: string | undefined,
        private readonly operationTimeoutMs: number,
        private readonly promptTimeoutMs: number,
    ) {
        this.client = new AcpBridgeClient(permissionProfile);

        const input = Writable.toWeb(agentProcess.stdin);
        const output = Readable.toWeb(agentProcess.stdout);
        this.connection = new ClientSideConnection(() => this.client, ndJsonStream(input, output));

        agentProcess.stderr.on("data", (chunk: Buffer | string) => {
            this.stderrOutput = `${this.stderrOutput}${String(chunk)}`.slice(-4000);
        });

        this.processFailure = new Promise<never>((_resolve, reject) => {
            agentProcess.once("error", (error) => {
                reject(error);
            });
            agentProcess.once("exit", (exitCode, signal) => {
                reject(
                    new Error(
                        `ACP agent process exited before completing the request. exitCode=${exitCode} signal=${signal}`,
                    ),
                );
            });
        });
        this.processFailure.catch(() => undefined);
    }

    async initialize(): Promise<void> {
        try {
            await Promise.race([
                this.connection.initialize({
                    protocolVersion: PROTOCOL_VERSION,
                    clientInfo: {
                        name: "acp-bridge",
                        version: "0.0.0",
                    },
                    clientCapabilities: {},
                }),
                this.processFailure,
                rejectAfterTimeout(this.operationTimeoutMs, new Error("ACP initialize timed out.")),
            ]);
        } catch (error) {
            throw this.withStderrContext(error);
        }
    }

    async askPair(input: PairAskInput): Promise<PairTurnResult> {
        try {
            const sessionResult = await Promise.race([
                this.connection.newSession({
                    cwd: this.cwd,
                    mcpServers: [],
                }),
                this.processFailure,
                rejectAfterTimeout(this.operationTimeoutMs, new Error("ACP newSession timed out.")),
            ]);

            await this.setSessionConfigOptions(sessionResult.sessionId, sessionResult.configOptions);
            await this.setSessionModel(sessionResult.sessionId);
            await this.setSessionMode(sessionResult.sessionId);
            return this.prompt(sessionResult.sessionId, input.prompt, input.context, input.files);
        } catch (error) {
            throw this.withStderrContext(error);
        }
    }

    async continuePair(input: PairContinueInput): Promise<PairTurnResult> {
        try {
            return await this.prompt(input.sessionId, input.prompt, input.context, input.files);
        } catch (error) {
            throw this.withStderrContext(error);
        }
    }

    async close(): Promise<void> {
        if (this.agentProcess.exitCode == null && this.agentProcess.signalCode == null) {
            try {
                this.agentProcess.kill("SIGTERM");
            } catch {
                // Process may have already exited between the check and the kill call.
            }

            const exited = await waitForProcessExit(this.agentProcess, SIGTERM_GRACE_PERIOD_MS);
            if (!exited && this.agentProcess.exitCode == null && this.agentProcess.signalCode == null) {
                try {
                    this.agentProcess.kill("SIGKILL");
                } catch {
                    // Process may have already exited.
                }
                await waitForProcessExit(this.agentProcess, SIGKILL_GRACE_PERIOD_MS);
            }
        }
        await this.connection.closed.catch(() => undefined);
    }

    private async prompt(
        sessionId: string,
        prompt: string,
        context?: string,
        files?: readonly string[],
    ): Promise<PairTurnResult> {
        const firstTurn = await this.runPromptTurn(sessionId, formatPrompt(prompt, context, files));
        if (!firstTurn.hadText || parseJsonAnswer(firstTurn.result.answer) != null) {
            return firstTurn.result;
        }

        const retryTurn = await this.runPromptTurn(sessionId, REJSON_REQUEST_PROMPT);
        if (!retryTurn.hadText) {
            return firstTurn.result;
        }
        return retryTurn.result;
    }

    private async runPromptTurn(
        sessionId: string,
        text: string,
    ): Promise<{ result: PairTurnResult; hadText: boolean }> {
        this.client.beginTurn(sessionId);
        const startedAt = performance.now();
        try {
            const promptResponse = await Promise.race([
                this.connection.prompt({
                    sessionId,
                    prompt: [
                        {
                            type: "text",
                            text,
                        },
                    ],
                }),
                this.processFailure,
                rejectAfterTimeout(this.promptTimeoutMs, new AcpPromptTimeoutError(sessionId, this.promptTimeoutMs)),
            ]);

            const elapsedMs = Math.round(performance.now() - startedAt);
            const stopReason = String(promptResponse.stopReason ?? "unknown");
            const answer = this.client.finishTurn(sessionId);
            const meta: PairTurnMeta = {
                elapsed_ms: elapsedMs,
                stop_reason: stopReason,
                agent_id: "",
            };
            if (answer.length > 0) {
                return { result: { sessionId, answer, meta }, hadText: true };
            }

            return {
                result: {
                    sessionId,
                    answer: `ACP agent completed without text output. stopReason=${stopReason}`,
                    meta,
                },
                hadText: false,
            };
        } catch (error) {
            this.client.finishTurn(sessionId);
            if (error instanceof AcpPromptTimeoutError) {
                await this.cancelTimedOutPrompt(sessionId);
                await this.close();
                throw new PairSessionClosedError(
                    sessionId,
                    `Pair session ${sessionId} was closed after ACP prompt timed out after ${this.promptTimeoutMs}ms.`,
                );
            }
            throw error;
        }
    }

    private async cancelTimedOutPrompt(sessionId: string): Promise<void> {
        await Promise.race([
            this.connection.cancel({ sessionId }),
            rejectAfterTimeout(2_000, new Error(`ACP cancel timed out. sessionId=${sessionId}`)),
        ]).catch(() => undefined);
    }

    private async setSessionModel(sessionId: string): Promise<void> {
        if (this.model == null || this.model.trim().length === 0) {
            return;
        }

        await Promise.race([
            this.connection.unstable_setSessionModel({
                sessionId,
                modelId: this.model,
            }),
            this.processFailure,
            rejectAfterTimeout(this.operationTimeoutMs, new Error(`ACP set_model timed out. model=${this.model}`)),
        ]);
    }

    private async setSessionMode(sessionId: string): Promise<void> {
        if (this.mode == null || this.mode.trim().length === 0) {
            return;
        }

        await Promise.race([
            this.connection.setSessionMode({
                sessionId,
                modeId: this.mode,
            }),
            this.processFailure,
            rejectAfterTimeout(this.operationTimeoutMs, new Error(`ACP set_mode timed out. mode=${this.mode}`)),
        ]);
    }

    private async setSessionConfigOptions(
        sessionId: string,
        availableConfigOptions: Awaited<ReturnType<ClientSideConnection["newSession"]>>["configOptions"],
    ): Promise<void> {
        const configurationEntries = sortConfigEntries(
            Object.entries(this.configOptions).filter(([, configurationValue]) => configurationValue.trim().length > 0),
            this.configOptionOrder,
        );
        if (configurationEntries.length === 0) {
            return;
        }
        if (availableConfigOptions == null) {
            throw new Error(
                `ACP agent did not provide config_options. Requested options: ${configurationEntries
                    .map(([configurationKey]) => configurationKey)
                    .join(", ")}`,
            );
        }

        let currentConfigOptions = availableConfigOptions;
        for (const [configurationKey, configurationValue] of configurationEntries) {
            if (!hasSelectConfigValue(currentConfigOptions, configurationKey, configurationValue)) {
                throw new Error(
                    `Invalid ACP config option value. option=${configurationKey} value=${configurationValue}`,
                );
            }

            const response = await Promise.race([
                this.connection.setSessionConfigOption({
                    sessionId,
                    configId: configurationKey,
                    value: configurationValue,
                }),
                this.processFailure,
                rejectAfterTimeout(
                    this.operationTimeoutMs,
                    new Error(
                        `ACP set_config_option timed out. option=${configurationKey} value=${configurationValue}`,
                    ),
                ),
            ]);
            currentConfigOptions = response.configOptions;
        }
    }

    private withStderrContext(error: unknown): Error {
        if (isPairSessionClosedError(error)) {
            return error;
        }
        const message = error instanceof Error ? error.message : String(error);
        const stderrMessage = this.stderrOutput.trim();
        if (stderrMessage.length === 0) {
            return error instanceof Error ? error : new Error(message);
        }
        return new Error(`${message}\n\nACP agent stderr:\n${stderrMessage}`);
    }
}

class AcpBridgeClient implements Client {
    private readonly answerBuffers = new Map<string, string[]>();
    private enforceProfile: PermissionProfile;

    constructor(initialProfile: PermissionProfile) {
        this.enforceProfile = initialProfile;
    }

    async requestPermission(params: RequestPermissionRequest): Promise<RequestPermissionResponse> {
        const layerZeroHit = layerZeroCheckFromRawInput(params.toolCall.rawInput, params.toolCall.locations);
        if (layerZeroHit) {
            auditLog({
                event: "request_permission",
                sessionId: params.sessionId,
                enforceProfile: this.enforceProfile,
                toolKind: params.toolCall.kind ?? null,
                decision: "reject",
                layer: "layer_0",
                code: layerZeroHit.code,
                category: layerZeroHit.category,
                pattern: layerZeroHit.pattern,
                matchedToken: layerZeroHit.matchedToken,
                visibility: "partial",
            });
            return selectPermissionOption(params, ["reject_once", "reject_always"]);
        }

        const decision = decidePermission(this.enforceProfile, params.toolCall.kind);
        auditLog({
            event: "request_permission",
            sessionId: params.sessionId,
            enforceProfile: this.enforceProfile,
            toolKind: params.toolCall.kind ?? null,
            decision,
            layer: "mode_policy",
            visibility: "visible",
        });
        if (decision === "allow") {
            return selectPermissionOption(params, ["allow_once", "allow_always"]);
        }
        return selectPermissionOption(params, ["reject_once", "reject_always"]);
    }

    async setSessionMode(params: SetSessionModeRequest): Promise<SetSessionModeResponse> {
        // ACP allows agent-initiated mode changes, but our enforcement profile is independent
        // of the agent's self-reported mode. We acknowledge the call so the agent doesn't fault,
        // but never mutate `enforceProfile`. See docs/permission.md §0, §1 (P-02).
        auditLog({
            event: "agent_set_session_mode",
            sessionId: params.sessionId,
            enforceProfile: this.enforceProfile,
            requestedModeId: params.modeId,
            decision: "ignored_for_enforcement",
            visibility: "visible",
        });
        return {};
    }

    async sessionUpdate(params: SessionNotification): Promise<void> {
        const update = params.update;
        if (update.sessionUpdate === "current_mode_update") {
            auditLog({
                event: "current_mode_update",
                sessionId: params.sessionId,
                enforceProfile: this.enforceProfile,
                requestedModeId: update.currentModeId,
                decision: "audit_only",
                visibility: "visible",
            });
            return;
        }
        if (update.sessionUpdate !== "agent_message_chunk") {
            return;
        }
        if (update.content.type !== "text") {
            return;
        }

        const answerBuffer = this.answerBuffers.get(params.sessionId);
        if (!answerBuffer) {
            return;
        }
        answerBuffer.push(update.content.text);
    }

    beginTurn(sessionId: string): void {
        this.answerBuffers.set(sessionId, []);
    }

    finishTurn(sessionId: string): string {
        const answer = (this.answerBuffers.get(sessionId) ?? []).join("").trim();
        this.answerBuffers.delete(sessionId);
        return answer;
    }
}

interface AuditEntry {
    event: string;
    sessionId: string;
    enforceProfile: PermissionProfile;
    decision: string;
    visibility: "visible" | "partial" | "external";
    toolKind?: ToolKind | null;
    requestedModeId?: string;
    layer?: "layer_0" | "user_policy" | "mode_policy" | "confirm_gate";
    code?: string;
    category?: LayerZeroHit["category"];
    pattern?: string;
    matchedToken?: string;
}

function auditLog(entry: AuditEntry): void {
    const payload = {
        ts: new Date().toISOString(),
        component: "acp-bridge",
        ...entry,
    };
    try {
        process.stderr.write(`${JSON.stringify(payload)}\n`);
    } catch {
        // Never throw from the audit path.
    }
}

const REJSON_REQUEST_PROMPT =
    "Previous response was not valid JSON. Re-emit only the JSON object matching the schema. No prose, no fenced block prefix beyond the JSON itself.";

function formatPrompt(prompt: string, context?: string, files?: readonly string[]): string {
    const pairReviewInstructions = [
        "You are a read-only pair reviewer (navigator) consulted only because the user explicitly asked for another coding agent's opinion.",
        "Do not modify files, run commands, change modes, or perform work beyond read-only analysis.",
        "Take a clear position against the main agent's stated position. Challenge assumptions when warranted, but keep feedback actionable.",
        "Return only a single JSON object with these keys: stance, summary, agreements, concerns, recommendation, follow_up_questions.",
        '`stance` is one of: "agree", "disagree", "partial", "insufficient_info". Set stance against the main agent\'s position. If main_agent_position is empty or missing, use "insufficient_info".',
        'If stance is not "agree", `concerns` must list specific reasons. `agreements`, `concerns`, and `follow_up_questions` are arrays of strings. `summary` and `recommendation` are strings.',
        "Do not include a `confidence` field; do not invent fields outside the schema.",
    ].join("\n");

    const sections: string[] = [pairReviewInstructions];
    if (files != null && files.length > 0) {
        sections.push(
            `Before answering, you may read these files with your read tool. Final reply must still be a single JSON object.\n${files.join("\n")}`,
        );
    }
    if (context != null && context.trim().length > 0) {
        sections.push(`Context:\n${context}`);
    }
    sections.push(`Question:\n${prompt}`);
    return sections.join("\n\n");
}

function hasSelectConfigValue(
    availableConfigOptions: readonly SessionConfigOption[],
    configurationKey: string,
    configurationValue: string,
): boolean {
    const configOption = availableConfigOptions.find(
        (availableConfigOption) => availableConfigOption.id === configurationKey,
    );
    if (!configOption || configOption.type !== "select") {
        return false;
    }

    return configOption.options.some((optionOrGroup) => {
        if ("value" in optionOrGroup) {
            return optionOrGroup.value === configurationValue;
        }
        return optionOrGroup.options.some((option) => option.value === configurationValue);
    });
}

function sortConfigEntries(
    configurationEntries: Array<[string, string]>,
    configOptionOrder: readonly string[],
): Array<[string, string]> {
    const orderByConfigurationKey = new Map(
        configOptionOrder.map((configurationKey, index) => [configurationKey, index]),
    );

    return [...configurationEntries].sort(([leftKey], [rightKey]) => {
        return (orderByConfigurationKey.get(leftKey) ?? 99) - (orderByConfigurationKey.get(rightKey) ?? 99);
    });
}

function rejectAfterTimeout(timeoutMs: number, error: Error): Promise<never> {
    return new Promise((_resolve, reject) => {
        setTimeout(() => {
            reject(error);
        }, timeoutMs);
    });
}

const SIGTERM_GRACE_PERIOD_MS = 2_000;
const SIGKILL_GRACE_PERIOD_MS = 1_000;

function waitForProcessExit(agentProcess: ChildProcessWithoutNullStreams, timeoutMs: number): Promise<boolean> {
    if (agentProcess.exitCode != null || agentProcess.signalCode != null) {
        return Promise.resolve(true);
    }
    return new Promise<boolean>((resolveExit) => {
        const timer = setTimeout(() => {
            agentProcess.removeListener("exit", onExit);
            resolveExit(false);
        }, timeoutMs);
        const onExit = () => {
            clearTimeout(timer);
            resolveExit(true);
        };
        agentProcess.once("exit", onExit);
    });
}

class AcpPromptTimeoutError extends Error {
    constructor(
        readonly sessionId: string,
        readonly timeoutMs: number,
    ) {
        super(`ACP prompt timed out. sessionId=${sessionId} timeoutMs=${timeoutMs}`);
        this.name = "AcpPromptTimeoutError";
    }
}

function selectPermissionOption(
    params: RequestPermissionRequest,
    optionKinds: readonly [PermissionOptionKind, ...PermissionOptionKind[]],
): RequestPermissionResponse {
    for (const optionKind of optionKinds) {
        const option = params.options.find((permissionOption) => permissionOption.kind === optionKind);
        if (option) {
            return { outcome: { outcome: "selected", optionId: option.optionId } };
        }
    }
    return { outcome: { outcome: "cancelled" } };
}
