export interface PairAskInput {
    prompt: string;
    context?: string;
}

export interface PairContinueInput {
    sessionId: string;
    prompt: string;
    context?: string;
}

export interface PairTurnResult {
    sessionId: string;
    answer: string;
}

export class PairSessionClosedError extends Error {
    constructor(
        readonly sessionId: string,
        message: string,
    ) {
        super(message);
        this.name = "PairSessionClosedError";
    }
}

export function isPairSessionClosedError(error: unknown): error is PairSessionClosedError {
    return error instanceof PairSessionClosedError;
}

export interface AgentAdapter {
    readonly id: string;
    readonly label: string;
    readonly description?: string;
    askPair(input: PairAskInput): Promise<PairTurnResult>;
    continuePair(input: PairContinueInput): Promise<PairTurnResult>;
}
