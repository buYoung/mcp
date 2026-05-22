import type { AgentAdapter } from "../agents/common/types.js";

interface PairSessionRecord {
    agentId: string;
    lastAccessedAt: number;
}

export interface PairSessionStoreOptions {
    idleTimeoutMs: number;
    maxSessions: number;
    resolveAgent: (agentId: string) => AgentAdapter | undefined;
}

export class PairSessionStore {
    private readonly sessions = new Map<string, PairSessionRecord>();

    constructor(private readonly options: PairSessionStoreOptions) {}

    get(sessionId: string): PairSessionRecord | undefined {
        return this.sessions.get(sessionId);
    }

    delete(sessionId: string): void {
        this.sessions.delete(sessionId);
    }

    touch(sessionId: string): void {
        const session = this.sessions.get(sessionId);
        if (session) {
            session.lastAccessedAt = Date.now();
        }
    }

    async remember(sessionId: string, agentId: string): Promise<void> {
        this.sessions.set(sessionId, { agentId, lastAccessedAt: Date.now() });
        await this.enforceLimit();
    }

    async cleanupExpired(): Promise<void> {
        const now = Date.now();
        const expired = [...this.sessions.entries()]
            .filter(([, session]) => now - session.lastAccessedAt > this.options.idleTimeoutMs)
            .map(([sessionId]) => sessionId);
        await Promise.all(expired.map((sessionId) => this.close(sessionId)));
    }

    async close(sessionId: string): Promise<boolean> {
        const session = this.sessions.get(sessionId);
        if (!session) {
            return false;
        }
        this.sessions.delete(sessionId);
        const agent = this.options.resolveAgent(session.agentId);
        if (agent) {
            await agent.closePair(sessionId);
        }
        return true;
    }

    private async enforceLimit(): Promise<void> {
        const excess = this.sessions.size - this.options.maxSessions;
        if (excess <= 0) {
            return;
        }
        const oldest = [...this.sessions.entries()]
            .sort(([, left], [, right]) => left.lastAccessedAt - right.lastAccessedAt)
            .slice(0, excess)
            .map(([sessionId]) => sessionId);
        await Promise.all(oldest.map((sessionId) => this.close(sessionId)));
    }
}
