import type { AgentAdapter } from "./common/types.js";

class AgentRegistry {
    private readonly agents = new Map<string, AgentAdapter>();

    register(agent: AgentAdapter): void {
        this.agents.set(agent.id, agent);
    }

    get(id: string): AgentAdapter | undefined {
        return this.agents.get(id);
    }

    list(): AgentAdapter[] {
        return [...this.agents.values()];
    }
}

export const agentRegistry = new AgentRegistry();
