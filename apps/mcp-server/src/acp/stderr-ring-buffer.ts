import { StringDecoder } from "node:string_decoder";

/**
 * Bounded buffer that accumulates UTF-8 stderr chunks, drops the oldest chunks
 * once the running character cap is exceeded, and joins on read.
 *
 * Designed for the agent stderr path: tail context for diagnostics without
 * the per-chunk O(n) cost of `value = (value + chunk).slice(-CAP)`.
 */
export class StderrRingBuffer {
    private readonly chunks: string[] = [];
    private totalCharacters = 0;
    private readonly decoder = new StringDecoder("utf8");

    constructor(private readonly maximumCharacters: number) {
        if (!Number.isSafeInteger(maximumCharacters) || maximumCharacters <= 0) {
            throw new Error(`StderrRingBuffer requires a positive cap; got ${maximumCharacters}`);
        }
    }

    write(chunk: Buffer | string): void {
        const decodedChunk = typeof chunk === "string" ? chunk : this.decoder.write(chunk);
        if (decodedChunk.length === 0) {
            return;
        }
        this.chunks.push(decodedChunk);
        this.totalCharacters += decodedChunk.length;
        this.evictExcess();
    }

    read(): string {
        const tail = this.decoder.end();
        if (tail.length > 0) {
            this.chunks.push(tail);
            this.totalCharacters += tail.length;
            this.evictExcess();
        }
        return this.chunks.join("");
    }

    private evictExcess(): void {
        while (this.totalCharacters > this.maximumCharacters && this.chunks.length > 1) {
            const removed = this.chunks.shift();
            if (removed != null) {
                this.totalCharacters -= removed.length;
            }
        }
        // If a single chunk still exceeds the cap, keep the tail only.
        const onlyChunk = this.chunks[0];
        if (this.chunks.length === 1 && onlyChunk != null && onlyChunk.length > this.maximumCharacters) {
            const tail = onlyChunk.slice(onlyChunk.length - this.maximumCharacters);
            this.chunks[0] = tail;
            this.totalCharacters = tail.length;
        }
    }
}
