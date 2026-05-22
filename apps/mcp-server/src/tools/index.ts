import type { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { CallToolRequestSchema, ListToolsRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import { isCommandAvailable } from "../agents/common/binary-availability.js";
import { isPairSessionClosedError, type PairTurnMeta, type PairTurnResult } from "../agents/common/types.js";
import { agentRegistry } from "../agents/registry.js";
import { validateFilesWithinCwd } from "./files-validation.js";
import { type PairStance, parsePairOpinion } from "./pair-opinion.js";
import { PairSessionStore } from "./pair-session-store.js";

const PAIR_SESSION_IDLE_TIMEOUT_MS = 30 * 60 * 1000;
const MAX_PAIR_SESSIONS = 20;
const MAX_CONSULT_PANEL_AGENTS = 5;

const pairSessionStore = new PairSessionStore({
    idleTimeoutMs: PAIR_SESSION_IDLE_TIMEOUT_MS,
    maxSessions: MAX_PAIR_SESSIONS,
    resolveAgent: (agentId) => agentRegistry.get(agentId),
});

const ELICITATION_TTL_MS = 10 * 60 * 1000;
const MAX_ELICITATION_ENTRIES = 64;
const elicitationConfirmations = new Map<string, number>();

export function registerTools(server: Server): void {
    server.setRequestHandler(ListToolsRequestSchema, async () => {
        return {
            tools: [
                {
                    name: "list_agents",
                    description:
                        "List available pair-review agents. Each agent is a separate coding model spawned as a cold child process per ask_pair call — only call when the user explicitly asked for another agent's opinion.",
                    inputSchema: {
                        type: "object",
                        properties: {
                            user_request: {
                                type: "string",
                                description:
                                    "Verbatim user request that asked for another coding agent's opinion. Kept for traceability.",
                            },
                        },
                        required: ["user_request"],
                    },
                },
                {
                    name: "ask_pair",
                    description:
                        "Ask one pair-review agent for a read-only opinion in a new session. Spawns a cold child process — non-trivial latency and token cost. The caller must commit to a tentative position in `main_agent_position` so the pair can agree or push back.",
                    inputSchema: {
                        type: "object",
                        properties: {
                            agent_id: {
                                type: "string",
                                description: "Pair agent id returned by list_agents.",
                            },
                            prompt: {
                                type: "string",
                                description: "Narrow question or task to send to the pair-review agent.",
                            },
                            user_request: {
                                type: "string",
                                description:
                                    "Verbatim user request that asked for another coding agent's opinion. Kept for traceability.",
                            },
                            main_agent_position: {
                                type: "string",
                                description:
                                    "REQUIRED. Calling agent's tentative position or recommendation. The pair will agree or challenge it.",
                            },
                            context: {
                                type: "string",
                                description: "Optional shared context such as constraints or prior decisions.",
                            },
                            files: {
                                type: "array",
                                items: { type: "string" },
                                description:
                                    "Optional absolute file paths the pair agent should read directly (read-only). Prefer this over pasting code into `context` to avoid amplifying the main agent's framing.",
                            },
                        },
                        required: ["agent_id", "prompt", "user_request", "main_agent_position"],
                    },
                },
                {
                    name: "continue_pair",
                    description:
                        "Continue a prior pair-review conversation. Same cold-start cost does NOT apply — reuses the existing child process for this session_id.",
                    inputSchema: {
                        type: "object",
                        properties: {
                            session_id: {
                                type: "string",
                                description: "Session id returned by ask_pair.",
                            },
                            prompt: {
                                type: "string",
                                description: "Follow-up question or instruction for the same pair-review agent.",
                            },
                            user_request: {
                                type: "string",
                                description:
                                    "Verbatim user request that asked to continue consulting another agent. Kept for traceability.",
                            },
                            main_agent_position: {
                                type: "string",
                                description:
                                    "Optional updated position from the calling agent for this follow-up turn.",
                            },
                            context: {
                                type: "string",
                                description: "Optional additional context for this follow-up turn.",
                            },
                            files: {
                                type: "array",
                                items: { type: "string" },
                                description: "Optional additional absolute file paths the pair agent should read.",
                            },
                        },
                        required: ["session_id", "prompt", "user_request"],
                    },
                },
                {
                    name: "consult_panel",
                    description:
                        "Ask multiple pair-review agents in parallel and return their independent opinions plus a stance tally. Each agent_id spawns its own cold child process — cost scales linearly with the number of agents.",
                    inputSchema: {
                        type: "object",
                        properties: {
                            agent_ids: {
                                type: "array",
                                items: { type: "string" },
                                description: `Two to ${MAX_CONSULT_PANEL_AGENTS} pair agent ids returned by list_agents. Each must be unique.`,
                                minItems: 2,
                                maxItems: MAX_CONSULT_PANEL_AGENTS,
                                uniqueItems: true,
                            },
                            prompt: {
                                type: "string",
                                description: "Narrow question or task to send to each pair-review agent.",
                            },
                            user_request: {
                                type: "string",
                                description:
                                    "Verbatim user request that asked for another coding agent's opinion. Kept for traceability.",
                            },
                            main_agent_position: {
                                type: "string",
                                description:
                                    "REQUIRED. Calling agent's tentative position. The pair panel will agree or challenge it.",
                            },
                            context: {
                                type: "string",
                                description: "Optional shared context such as constraints or prior decisions.",
                            },
                            files: {
                                type: "array",
                                items: { type: "string" },
                                description: "Optional absolute file paths the pair agents should read directly.",
                            },
                        },
                        required: ["agent_ids", "prompt", "user_request", "main_agent_position"],
                    },
                },
                {
                    name: "close_pair",
                    description: "Close a pair-review session when the consultation is complete.",
                    inputSchema: {
                        type: "object",
                        properties: {
                            session_id: {
                                type: "string",
                                description: "Session id returned by ask_pair.",
                            },
                        },
                        required: ["session_id"],
                    },
                },
            ],
        };
    });

    server.setRequestHandler(CallToolRequestSchema, async (req) => {
        await pairSessionStore.cleanupExpired();
        const argumentsValue = readArguments(req.params.arguments);

        if (req.params.name === "list_agents") {
            const userRequest = readRequiredString(argumentsValue, "user_request");
            await ensureUserConsent(server, userRequest);
            const agents = await Promise.all(
                agentRegistry.list().map(async (agent) => ({
                    agent_id: agent.id,
                    label: agent.label,
                    description: agent.description,
                    model: agent.model,
                    available: await isCommandAvailable(agent.command),
                })),
            );
            return textResult({ agents });
        }

        if (req.params.name === "ask_pair") {
            const userRequest = readRequiredString(argumentsValue, "user_request");
            await ensureUserConsent(server, userRequest);
            const mainAgentPosition = readMainAgentPosition(argumentsValue, true);
            const agentId = readRequiredString(argumentsValue, "agent_id");
            const agent = agentRegistry.get(agentId);
            if (!agent) {
                throw new Error(`Unknown agent_id: ${agentId}`);
            }

            const pairTurnResult = await agent.askPair({
                prompt: readRequiredString(argumentsValue, "prompt"),
                context: buildPairContext(argumentsValue, mainAgentPosition),
                files: await validateFilesWithinCwd(readOptionalStringArray(argumentsValue, "files")),
            });
            await pairSessionStore.remember(pairTurnResult.sessionId, agent.id);

            return textResult(createPairTurnResponse(pairTurnResult, agent.id, agent.model));
        }

        if (req.params.name === "continue_pair") {
            const userRequest = readRequiredString(argumentsValue, "user_request");
            await ensureUserConsent(server, userRequest);
            const mainAgentPosition = readMainAgentPosition(argumentsValue, false);
            const sessionId = readRequiredString(argumentsValue, "session_id");
            const pairSession = pairSessionStore.get(sessionId);
            if (!pairSession) {
                throw new Error(`Unknown session_id: ${sessionId}`);
            }

            const agent = agentRegistry.get(pairSession.agentId);
            if (!agent) {
                throw new Error(`Unknown agent_id for session_id ${sessionId}: ${pairSession.agentId}`);
            }

            try {
                const pairTurnResult = await agent.continuePair({
                    sessionId,
                    prompt: readRequiredString(argumentsValue, "prompt"),
                    context: buildPairContext(argumentsValue, mainAgentPosition),
                    files: await validateFilesWithinCwd(readOptionalStringArray(argumentsValue, "files")),
                });
                pairSessionStore.touch(pairTurnResult.sessionId);

                return textResult(createPairTurnResponse(pairTurnResult, agent.id, agent.model));
            } catch (error) {
                if (isPairSessionClosedError(error)) {
                    pairSessionStore.delete(sessionId);
                }
                throw error;
            }
        }

        if (req.params.name === "consult_panel") {
            const userRequest = readRequiredString(argumentsValue, "user_request");
            await ensureUserConsent(server, userRequest);
            const mainAgentPosition = readMainAgentPosition(argumentsValue, true);
            const agentIds = readRequiredStringArray(argumentsValue, "agent_ids");
            if (agentIds.length < 2) {
                throw new Error("consult_panel requires at least two agent_ids.");
            }
            if (agentIds.length > MAX_CONSULT_PANEL_AGENTS) {
                throw new Error(
                    `consult_panel accepts at most ${MAX_CONSULT_PANEL_AGENTS} agent_ids; got ${agentIds.length}.`,
                );
            }
            const uniqueAgentIds = [...new Set(agentIds)];
            if (uniqueAgentIds.length !== agentIds.length) {
                throw new Error("consult_panel agent_ids must be unique.");
            }
            const prompt = readRequiredString(argumentsValue, "prompt");
            const context = buildPairContext(argumentsValue, mainAgentPosition);
            const files = await validateFilesWithinCwd(readOptionalStringArray(argumentsValue, "files"));

            const settled = await Promise.allSettled(
                agentIds.map(async (agentId) => {
                    const agent = agentRegistry.get(agentId);
                    if (!agent) {
                        throw new Error(`Unknown agent_id: ${agentId}`);
                    }
                    const pairTurnResult = await agent.askPair({ prompt, context, files });
                    await pairSessionStore.remember(pairTurnResult.sessionId, agent.id);
                    return { agent, pairTurnResult };
                }),
            );

            const results: Array<Record<string, unknown>> = [];
            const errors: Array<{ agent_id: string; message: string }> = [];
            const stanceTally: Record<PairStance, number> = {
                agree: 0,
                disagree: 0,
                partial: 0,
                insufficient_info: 0,
            };

            settled.forEach((outcome, index) => {
                const agentId = agentIds[index] ?? "<unknown>";
                if (outcome.status === "fulfilled") {
                    const { agent, pairTurnResult } = outcome.value;
                    const response = createPairTurnResponse(pairTurnResult, agent.id, agent.model);
                    results.push(response);
                    stanceTally[response.structured_opinion.stance] += 1;
                } else {
                    const message = outcome.reason instanceof Error ? outcome.reason.message : String(outcome.reason);
                    errors.push({ agent_id: agentId, message });
                }
            });

            return textResult({
                results,
                stance_tally: stanceTally,
                errors,
            });
        }

        if (req.params.name === "close_pair") {
            const sessionId = readRequiredString(argumentsValue, "session_id");
            const wasClosed = await pairSessionStore.close(sessionId);
            if (!wasClosed) {
                throw new Error(`Unknown session_id: ${sessionId}`);
            }

            return textResult({
                session_id: sessionId,
                closed: true,
            });
        }

        throw new Error(`Unknown tool: ${req.params.name}`);
    });
}

function createPairTurnResponse(pairTurnResult: PairTurnResult, agentId: string, agentModel: string | undefined) {
    const meta: PairTurnMeta = {
        ...pairTurnResult.meta,
        agent_id: agentId,
    };
    if (agentModel != null && agentModel.trim().length > 0) {
        meta.agent_model = agentModel;
    }
    return {
        session_id: pairTurnResult.sessionId,
        agent_id: agentId,
        answer: pairTurnResult.answer,
        structured_opinion: parsePairOpinion(pairTurnResult.answer),
        meta,
    };
}

function buildPairContext(
    argumentsValue: Record<string, unknown>,
    mainAgentPosition: string | undefined,
): string | undefined {
    const contextParts: string[] = [];
    const context = readOptionalString(argumentsValue, "context")?.trim();

    if (mainAgentPosition != null && mainAgentPosition.length > 0) {
        contextParts.push(`Main agent position:\n${mainAgentPosition}`);
    }
    if (context != null && context.length > 0) {
        contextParts.push(context);
    }

    return contextParts.length > 0 ? contextParts.join("\n\n") : undefined;
}

function readMainAgentPosition(argumentsValue: Record<string, unknown>, required: boolean): string | undefined {
    const rawValue = argumentsValue.main_agent_position;
    if (rawValue == null) {
        if (required) {
            throw new Error(
                "main_agent_position must be non-empty. Decide a tentative position before consulting a pair.",
            );
        }
        return undefined;
    }
    if (typeof rawValue !== "string" || rawValue.trim().length === 0) {
        if (required) {
            throw new Error(
                "main_agent_position must be non-empty. Decide a tentative position before consulting a pair.",
            );
        }
        return undefined;
    }
    return rawValue.trim();
}

async function ensureUserConsent(server: Server, userRequest: string): Promise<void> {
    const key = elicitationKey(userRequest);
    pruneExpiredElicitationConfirmations();

    if (isElicitationConfirmed(key)) {
        process.stderr.write(`[acp-bridge] pair-consult invoked (cached consent): ${userRequest}\n`);
        return;
    }

    const clientCapabilities = server.getClientCapabilities();
    const supportsElicitation = clientCapabilities?.elicitation != null;
    if (!supportsElicitation) {
        process.stderr.write(`[acp-bridge] pair-consult invoked (no elicitation support): ${userRequest}\n`);
        rememberElicitationConfirmation(key);
        return;
    }

    try {
        const elicitResult = await server.elicitInput({
            message: `Did the user explicitly ask for another coding agent's opinion?\n\nQuoted request: ${userRequest}`,
            requestedSchema: {
                type: "object",
                properties: {
                    confirm: {
                        type: "boolean",
                        title: "Explicit user request",
                        description: "Yes if the user explicitly asked for another agent's opinion.",
                    },
                },
                required: ["confirm"],
            },
        });

        if (elicitResult.action !== "accept" || elicitResult.content?.confirm !== true) {
            throw new Error(
                "Pair consultation was not confirmed by the user. Only call pair tools after an explicit user request.",
            );
        }
        rememberElicitationConfirmation(key);
        process.stderr.write(`[acp-bridge] pair-consult confirmed: ${userRequest}\n`);
    } catch (error) {
        if (error instanceof Error && error.message.startsWith("Pair consultation was not confirmed")) {
            throw error;
        }
        process.stderr.write(
            `[acp-bridge] pair-consult elicitation failed, proceeding with log only: ${userRequest}\n`,
        );
        rememberElicitationConfirmation(key);
    }
}

function elicitationKey(userRequest: string): string {
    return userRequest.trim();
}

function isElicitationConfirmed(key: string): boolean {
    const expiresAt = elicitationConfirmations.get(key);
    if (expiresAt == null) {
        return false;
    }
    if (expiresAt < Date.now()) {
        elicitationConfirmations.delete(key);
        return false;
    }
    return true;
}

function rememberElicitationConfirmation(key: string): void {
    elicitationConfirmations.set(key, Date.now() + ELICITATION_TTL_MS);
    enforceElicitationCapacity();
}

function pruneExpiredElicitationConfirmations(): void {
    const now = Date.now();
    for (const [key, expiresAt] of elicitationConfirmations) {
        if (expiresAt < now) {
            elicitationConfirmations.delete(key);
        }
    }
}

function enforceElicitationCapacity(): void {
    const excess = elicitationConfirmations.size - MAX_ELICITATION_ENTRIES;
    if (excess <= 0) {
        return;
    }
    const oldestKeys = [...elicitationConfirmations.entries()]
        .sort(([, left], [, right]) => left - right)
        .slice(0, excess)
        .map(([key]) => key);
    for (const key of oldestKeys) {
        elicitationConfirmations.delete(key);
    }
}

function textResult(value: unknown) {
    return {
        content: [{ type: "text" as const, text: JSON.stringify(value, null, 2) }],
    };
}

function readArguments(argumentsValue: unknown): Record<string, unknown> {
    if (argumentsValue == null) {
        return {};
    }
    if (typeof argumentsValue !== "object" || Array.isArray(argumentsValue)) {
        throw new Error("Expected object arguments.");
    }
    return argumentsValue as Record<string, unknown>;
}

function readRequiredString(argumentsValue: Record<string, unknown>, key: string): string {
    const value = argumentsValue[key];
    if (typeof value !== "string" || value.trim().length === 0) {
        throw new Error(`Expected non-empty string argument: ${key}`);
    }
    return value;
}

function readOptionalString(argumentsValue: Record<string, unknown>, key: string): string | undefined {
    const value = argumentsValue[key];
    if (value == null) {
        return undefined;
    }
    if (typeof value !== "string") {
        throw new Error(`Expected string argument: ${key}`);
    }
    return value;
}

function readRequiredStringArray(argumentsValue: Record<string, unknown>, key: string): string[] {
    const value = argumentsValue[key];
    if (!Array.isArray(value)) {
        throw new Error(`Expected string array argument: ${key}`);
    }
    const result = value.filter((item): item is string => typeof item === "string" && item.trim().length > 0);
    if (result.length === 0) {
        throw new Error(`Expected non-empty string array argument: ${key}`);
    }
    return result;
}

function readOptionalStringArray(argumentsValue: Record<string, unknown>, key: string): string[] | undefined {
    const value = argumentsValue[key];
    if (value == null) {
        return undefined;
    }
    if (!Array.isArray(value)) {
        throw new Error(`Expected string array argument: ${key}`);
    }
    const result = value.filter((item): item is string => typeof item === "string" && item.trim().length > 0);
    return result.length > 0 ? result : undefined;
}
