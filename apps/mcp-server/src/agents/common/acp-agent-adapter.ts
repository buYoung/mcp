import { type AcpAgentLaunchOptions, type AcpAgentSession, launchAcpAgent } from "../../acp/client.js";
import {
    type AgentAdapter,
    isPairSessionClosedError,
    type PairAskInput,
    type PairContinueInput,
    type PairTurnResult,
} from "./types.js";

export interface AcpAgentAdapterOptions {
    id: string;
    label: string;
    description: string;
    launchOptions: AcpAgentLaunchOptions;
}

export function createAcpAgentAdapter(options: AcpAgentAdapterOptions): AgentAdapter {
    const sessions = new Map<string, AcpAgentSession>();
    const sessionQueues = new Map<string, Promise<unknown>>();

    return {
        id: options.id,
        label: options.label,
        description: options.description,

        async askPair(input: PairAskInput): Promise<PairTurnResult> {
            const session = await launchAcpAgent(options.launchOptions);
            try {
                const pairTurnResult = await session.askPair(input);
                sessions.set(pairTurnResult.sessionId, session);
                return pairTurnResult;
            } catch (error) {
                await session.close();
                throw error;
            }
        },

        async continuePair(input: PairContinueInput): Promise<PairTurnResult> {
            return runSessionExclusive(input.sessionId, async () => {
                const session = sessions.get(input.sessionId);
                if (!session) {
                    throw new Error(`Unknown session_id for ${options.id}: ${input.sessionId}`);
                }

                try {
                    return await session.continuePair(input);
                } catch (error) {
                    if (isPairSessionClosedError(error)) {
                        sessions.delete(input.sessionId);
                    }
                    throw error;
                }
            });
        },

        async closePair(sessionId: string): Promise<void> {
            await runSessionExclusive(sessionId, async () => {
                const session = sessions.get(sessionId);
                sessions.delete(sessionId);
                if (session) {
                    await session.close();
                }
            });
        },
    };

    async function runSessionExclusive<T>(sessionId: string, operation: () => Promise<T>): Promise<T> {
        const previousQueue = sessionQueues.get(sessionId) ?? Promise.resolve();
        const currentQueue = previousQueue.catch(() => undefined).then(operation);
        sessionQueues.set(sessionId, currentQueue);

        try {
            return await currentQueue;
        } finally {
            if (sessionQueues.get(sessionId) === currentQueue) {
                sessionQueues.delete(sessionId);
            }
        }
    }
}
