import { describe, expect, it } from "vitest";
import { StderrRingBuffer } from "../src/acp/stderr-ring-buffer.js";

describe("StderrRingBuffer", () => {
    it("returns empty when nothing was written", () => {
        const buffer = new StderrRingBuffer(100);
        expect(buffer.read()).toBe("");
    });

    it("concatenates string chunks in order", () => {
        const buffer = new StderrRingBuffer(100);
        buffer.write("hello ");
        buffer.write("world");
        expect(buffer.read()).toBe("hello world");
    });

    it("drops oldest chunks once the total exceeds the cap", () => {
        const buffer = new StderrRingBuffer(10);
        buffer.write("AAAAA");
        buffer.write("BBBBB");
        buffer.write("CCCCC");
        const output = buffer.read();
        expect(output.length).toBeLessThanOrEqual(10);
        expect(output.endsWith("CCCCC")).toBe(true);
    });

    it("truncates from the head when a single chunk exceeds the cap", () => {
        const buffer = new StderrRingBuffer(5);
        buffer.write("0123456789");
        expect(buffer.read()).toBe("56789");
    });

    it("decodes UTF-8 multi-byte sequences split across chunks", () => {
        const buffer = new StderrRingBuffer(100);
        const fullString = "안녕하세요";
        const fullBytes = Buffer.from(fullString, "utf8");
        buffer.write(fullBytes.subarray(0, 4));
        buffer.write(fullBytes.subarray(4));
        expect(buffer.read()).toBe(fullString);
    });

    it("throws on non-positive cap", () => {
        expect(() => new StderrRingBuffer(0)).toThrow();
        expect(() => new StderrRingBuffer(-1)).toThrow();
    });
});
