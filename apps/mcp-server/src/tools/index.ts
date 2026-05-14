import type { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { CallToolRequestSchema, ListToolsRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import { isPairSessionClosedError } from "../agents/common/types.js";
import { agentRegistry } from "../agents/registry.js";

interface PairSessionRecord {
    agentId: string;
    lastAccessedAt: number;
}

interface PairOpinion {
    summary: string;
    agreements: string[];
    concerns: string[];
    recommendation: string;
    confidence: "low" | "medium" | "high";
    follow_up_questions: string[];
    parse_status: "parsed" | "fallback";
    raw_answer?: string;
}

const pairSessions = new Map<string, PairSessionRecord>();
const PAIR_SESSION_IDLE_TIMEOUT_MS = 30 * 60 * 1000;
const MAX_PAIR_SESSIONS = 20;
const EXPLICIT_PAIR_REQUEST_PATTERNS = [
    /\bfair programming\b/i,
    /\bpair programming\b/i,
    /페어\s*프로그래밍/,
    /공정한\s*프로그래밍/,
    /다른\s*(?:코드\s*)?(?:에이전트|agent|모델|AI)/i,
    /(?:에이전트|agent|모델|AI).*(?:의견|생각|검토|리뷰)/i,
    /(?:의견|생각|검토|리뷰).*(?:에이전트|agent|모델|AI)/i,
    /크로스\s*체크/,
    /교차\s*검토/,
];

export function registerTools(server: Server): void {
    server.setRequestHandler(ListToolsRequestSchema, async () => {
        return {
            tools: [
                {
                    name: "list_agents",
                    description:
                        "List available pair-review agents. Call only when user_request quotes an explicit user request for fair programming, pair programming, or another agent's opinion.",
                    inputSchema: {
                        type: "object",
                        properties: {
                            user_request: {
                                type: "string",
                                description:
                                    "Verbatim user request that explicitly asked for fair programming, pair programming, or another agent's opinion.",
                            },
                        },
                        required: ["user_request"],
                    },
                },
                {
                    name: "list_models",
                    description:
                        "Deprecated alias for list_agents. Call only when user_request quotes an explicit user request for another coding agent's opinion.",
                    inputSchema: {
                        type: "object",
                        properties: {
                            user_request: {
                                type: "string",
                                description:
                                    "Verbatim user request that explicitly asked for fair programming, pair programming, or another agent's opinion.",
                            },
                        },
                        required: ["user_request"],
                    },
                },
                {
                    name: "ask_pair",
                    description:
                        "Ask one pair-review agent for a read-only opinion in a new session. Call only when user_request quotes an explicit user request for fair programming, pair programming, or another agent's opinion.",
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
                                    "Verbatim user request that explicitly asked for fair programming, pair programming, or another agent's opinion.",
                            },
                            context: {
                                type: "string",
                                description: "Optional shared context such as files, constraints, or prior decisions.",
                            },
                            main_agent_position: {
                                type: "string",
                                description:
                                    "Optional current position or recommendation from the calling agent, used so the pair reviewer can agree or challenge it.",
                            },
                        },
                        required: ["agent_id", "prompt", "user_request"],
                    },
                },
                {
                    name: "continue_pair",
                    description:
                        "Continue a prior pair-review conversation. Call only when user_request quotes an explicit user request to continue consulting another agent.",
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
                                    "Verbatim user request that explicitly asked to continue fair programming, pair programming, or another agent's opinion.",
                            },
                            context: {
                                type: "string",
                                description: "Optional additional context for this follow-up turn.",
                            },
                            main_agent_position: {
                                type: "string",
                                description:
                                    "Optional updated position or recommendation from the calling agent, used so the pair reviewer can agree or challenge it.",
                            },
                        },
                        required: ["session_id", "prompt", "user_request"],
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
        await cleanupExpiredPairSessions();
        const argumentsValue = readArguments(req.params.arguments);

        if (req.params.name === "list_agents" || req.params.name === "list_models") {
            readExplicitUserRequest(argumentsValue);
            return textResult({
                agents: agentRegistry.list().map((agent) => ({
                    agent_id: agent.id,
                    label: agent.label,
                    description: agent.description,
                })),
            });
        }

        if (req.params.name === "ask_pair") {
            readExplicitUserRequest(argumentsValue);
            const agentId = readRequiredString(argumentsValue, "agent_id");
            const agent = agentRegistry.get(agentId);
            if (!agent) {
                throw new Error(`Unknown agent_id: ${agentId}`);
            }

            const pairTurnResult = await agent.askPair({
                prompt: readRequiredString(argumentsValue, "prompt"),
                context: buildPairContext(argumentsValue),
            });
            await rememberPairSession(pairTurnResult.sessionId, agent.id);

            return textResult(createPairTurnResponse(pairTurnResult.sessionId, agent.id, pairTurnResult.answer));
        }

        if (req.params.name === "continue_pair") {
            readExplicitUserRequest(argumentsValue);
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
                    context: buildPairContext(argumentsValue),
                });
                touchPairSession(pairTurnResult.sessionId);

                return textResult(createPairTurnResponse(pairTurnResult.sessionId, agent.id, pairTurnResult.answer));
            } catch (error) {
                if (isPairSessionClosedError(error)) {
                    pairSessions.delete(sessionId);
                }
                throw error;
            }
        }

        if (req.params.name === "close_pair") {
            const sessionId = readRequiredString(argumentsValue, "session_id");
            const wasClosed = await closePairSession(sessionId);
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

function createPairTurnResponse(sessionId: string, agentId: string, answer: string) {
    return {
        session_id: sessionId,
        agent_id: agentId,
        answer,
        structured_opinion: parsePairOpinion(answer),
    };
}

function buildPairContext(argumentsValue: Record<string, unknown>): string | undefined {
    const contextParts: string[] = [];
    const mainAgentPosition = readOptionalString(argumentsValue, "main_agent_position")?.trim();
    const context = readOptionalString(argumentsValue, "context")?.trim();

    if (mainAgentPosition != null && mainAgentPosition.length > 0) {
        contextParts.push(`Main agent position:\n${mainAgentPosition}`);
    }
    if (context != null && context.length > 0) {
        contextParts.push(context);
    }

    return contextParts.length > 0 ? contextParts.join("\n\n") : undefined;
}

function parsePairOpinion(answer: string): PairOpinion {
    const parsedValue = parseJsonAnswer(answer);
    if (parsedValue == null) {
        return createFallbackPairOpinion(answer);
    }

    const summary = readStringProperty(parsedValue, "summary");
    const recommendation = readStringProperty(parsedValue, "recommendation");
    if (summary == null || recommendation == null) {
        return createFallbackPairOpinion(answer);
    }

    return {
        summary,
        agreements: readStringArrayProperty(parsedValue, "agreements"),
        concerns: readStringArrayProperty(parsedValue, "concerns"),
        recommendation,
        confidence: readConfidenceProperty(parsedValue, "confidence"),
        follow_up_questions: readStringArrayProperty(parsedValue, "follow_up_questions"),
        parse_status: "parsed",
    };
}

function createFallbackPairOpinion(answer: string): PairOpinion {
    return {
        summary: answer,
        agreements: [],
        concerns: [],
        recommendation: answer,
        confidence: "low",
        follow_up_questions: [],
        parse_status: "fallback",
        raw_answer: answer,
    };
}

function parseJsonAnswer(answer: string): Record<string, unknown> | undefined {
    const candidate = extractJsonCandidate(answer);
    if (candidate == null) {
        return undefined;
    }

    try {
        const parsedValue = JSON.parse(candidate) as unknown;
        if (typeof parsedValue === "object" && parsedValue != null && !Array.isArray(parsedValue)) {
            return parsedValue as Record<string, unknown>;
        }
    } catch {
        return undefined;
    }
    return undefined;
}

function extractJsonCandidate(answer: string): string | undefined {
    const fencedJsonMatch = /```(?:json)?\s*([\s\S]*?)```/.exec(answer);
    const fencedJsonCandidate = fencedJsonMatch?.[1]?.trim();
    if (fencedJsonCandidate != null && fencedJsonCandidate.length > 0) {
        return fencedJsonCandidate;
    }

    const objectStartIndex = answer.indexOf("{");
    const objectEndIndex = answer.lastIndexOf("}");
    if (objectStartIndex === -1 || objectEndIndex <= objectStartIndex) {
        return undefined;
    }
    return answer.slice(objectStartIndex, objectEndIndex + 1);
}

function readStringProperty(value: Record<string, unknown>, key: string): string | undefined {
    const propertyValue = value[key];
    if (typeof propertyValue !== "string" || propertyValue.trim().length === 0) {
        return undefined;
    }
    return propertyValue;
}

function readStringArrayProperty(value: Record<string, unknown>, key: string): string[] {
    const propertyValue = value[key];
    if (!Array.isArray(propertyValue)) {
        return [];
    }
    return propertyValue.filter((item): item is string => typeof item === "string" && item.trim().length > 0);
}

function readConfidenceProperty(value: Record<string, unknown>, key: string): "low" | "medium" | "high" {
    const propertyValue = value[key];
    if (propertyValue === "low" || propertyValue === "medium" || propertyValue === "high") {
        return propertyValue;
    }
    return "low";
}

async function rememberPairSession(sessionId: string, agentId: string): Promise<void> {
    pairSessions.set(sessionId, { agentId, lastAccessedAt: Date.now() });
    await enforcePairSessionLimit();
}

function touchPairSession(sessionId: string): void {
    const pairSession = pairSessions.get(sessionId);
    if (pairSession) {
        pairSession.lastAccessedAt = Date.now();
    }
}

async function cleanupExpiredPairSessions(): Promise<void> {
    const now = Date.now();
    const expiredSessionIds = [...pairSessions.entries()]
        .filter(([, pairSession]) => now - pairSession.lastAccessedAt > PAIR_SESSION_IDLE_TIMEOUT_MS)
        .map(([sessionId]) => sessionId);

    await Promise.all(expiredSessionIds.map((sessionId) => closePairSession(sessionId)));
}

async function enforcePairSessionLimit(): Promise<void> {
    const excessSessionCount = pairSessions.size - MAX_PAIR_SESSIONS;
    if (excessSessionCount <= 0) {
        return;
    }

    const oldestSessionIds = [...pairSessions.entries()]
        .sort(([, leftSession], [, rightSession]) => leftSession.lastAccessedAt - rightSession.lastAccessedAt)
        .slice(0, excessSessionCount)
        .map(([sessionId]) => sessionId);

    await Promise.all(oldestSessionIds.map((sessionId) => closePairSession(sessionId)));
}

async function closePairSession(sessionId: string): Promise<boolean> {
    const pairSession = pairSessions.get(sessionId);
    if (!pairSession) {
        return false;
    }

    pairSessions.delete(sessionId);
    const agent = agentRegistry.get(pairSession.agentId);
    if (agent) {
        await agent.closePair(sessionId);
    }
    return true;
}

function readExplicitUserRequest(argumentsValue: Record<string, unknown>): string {
    const userRequest = readRequiredString(argumentsValue, "user_request");
    if (!hasExplicitPairRequest(userRequest)) {
        throw new Error(
            "Expected user_request to quote an explicit user request for fair programming, pair programming, or another agent's opinion.",
        );
    }
    return userRequest;
}

function hasExplicitPairRequest(userRequest: string): boolean {
    return EXPLICIT_PAIR_REQUEST_PATTERNS.some((pattern) => pattern.test(userRequest));
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
