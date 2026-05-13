import type { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { CallToolRequestSchema, ListToolsRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import { isPairSessionClosedError } from "../agents/common/types.js";
import { agentRegistry } from "../agents/registry.js";

interface PairSessionRecord {
    agentId: string;
}

const pairSessions = new Map<string, PairSessionRecord>();

export function registerTools(server: Server): void {
    server.setRequestHandler(ListToolsRequestSchema, async () => {
        return {
            tools: [
                {
                    name: "list_models",
                    description: "List available pair-programming candidates. Use once near the start of a session.",
                    inputSchema: {
                        type: "object",
                        properties: {},
                    },
                },
                {
                    name: "ask_pair",
                    description:
                        "Ask a narrow one-off question to a pair candidate in a new session. Returns a session_id for follow-up.",
                    inputSchema: {
                        type: "object",
                        properties: {
                            agent_id: {
                                type: "string",
                                description: "Pair agent id returned by list_models.",
                            },
                            prompt: {
                                type: "string",
                                description: "Narrow question or task to send to the pair candidate.",
                            },
                            context: {
                                type: "string",
                                description: "Optional shared context such as files, constraints, or prior decisions.",
                            },
                        },
                        required: ["agent_id", "prompt"],
                    },
                },
                {
                    name: "continue_pair",
                    description: "Continue a prior pair conversation using the session_id returned by ask_pair.",
                    inputSchema: {
                        type: "object",
                        properties: {
                            session_id: {
                                type: "string",
                                description: "Session id returned by ask_pair.",
                            },
                            prompt: {
                                type: "string",
                                description: "Follow-up question or instruction for the same pair candidate.",
                            },
                            context: {
                                type: "string",
                                description: "Optional additional context for this follow-up turn.",
                            },
                        },
                        required: ["session_id", "prompt"],
                    },
                },
            ],
        };
    });

    server.setRequestHandler(CallToolRequestSchema, async (req) => {
        const argumentsValue = req.params.arguments ?? {};

        if (req.params.name === "list_models") {
            return textResult({
                models: agentRegistry.list().map((agent) => ({
                    agent_id: agent.id,
                    label: agent.label,
                    description: agent.description,
                })),
            });
        }

        if (req.params.name === "ask_pair") {
            const agentId = readRequiredString(argumentsValue, "agent_id");
            const agent = agentRegistry.get(agentId);
            if (!agent) {
                throw new Error(`Unknown agent_id: ${agentId}`);
            }

            const pairTurnResult = await agent.askPair({
                prompt: readRequiredString(argumentsValue, "prompt"),
                context: readOptionalString(argumentsValue, "context"),
            });
            pairSessions.set(pairTurnResult.sessionId, { agentId: agent.id });

            return textResult({
                session_id: pairTurnResult.sessionId,
                agent_id: agent.id,
                answer: pairTurnResult.answer,
            });
        }

        if (req.params.name === "continue_pair") {
            const sessionId = readRequiredString(argumentsValue, "session_id");
            const pairSession = pairSessions.get(sessionId);
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
                    context: readOptionalString(argumentsValue, "context"),
                });

                return textResult({
                    session_id: pairTurnResult.sessionId,
                    agent_id: agent.id,
                    answer: pairTurnResult.answer,
                });
            } catch (error) {
                if (isPairSessionClosedError(error)) {
                    pairSessions.delete(sessionId);
                }
                throw error;
            }
        }

        throw new Error(`Unknown tool: ${req.params.name}`);
    });
}

function textResult(value: unknown) {
    return {
        content: [{ type: "text" as const, text: JSON.stringify(value, null, 2) }],
    };
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
