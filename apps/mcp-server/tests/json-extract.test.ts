import { describe, expect, it } from "vitest";
import { extractFirstBalancedJsonObject, extractJsonCandidate, parseJsonAnswer } from "../src/tools/json-extract.js";

describe("extractFirstBalancedJsonObject", () => {
    it("returns the first complete object", () => {
        expect(extractFirstBalancedJsonObject('prose {"a":1} trailing')).toBe('{"a":1}');
    });

    it("handles nested braces", () => {
        expect(extractFirstBalancedJsonObject('{"a":{"b":2}} extra')).toBe('{"a":{"b":2}}');
    });

    it("ignores braces inside strings", () => {
        expect(extractFirstBalancedJsonObject('{"a":"}{"}')).toBe('{"a":"}{"}');
    });

    it("returns undefined when no balanced object exists", () => {
        expect(extractFirstBalancedJsonObject("no braces here")).toBeUndefined();
        expect(extractFirstBalancedJsonObject("{ unclosed")).toBeUndefined();
    });
});

describe("extractJsonCandidate", () => {
    it("prefers fenced block", () => {
        const answer = 'prose\n```json\n{"a":1}\n```\nmore';
        expect(extractJsonCandidate(answer)).toBe('{"a":1}');
    });

    it("falls back to balanced object", () => {
        expect(extractJsonCandidate('prefix {"a":1} suffix')).toBe('{"a":1}');
    });
});

describe("parseJsonAnswer", () => {
    it("returns object for valid JSON", () => {
        expect(parseJsonAnswer('text {"x":1}')).toEqual({ x: 1 });
    });

    it("returns undefined for arrays at top level (must be object)", () => {
        expect(parseJsonAnswer("[1,2,3]")).toBeUndefined();
    });

    it("returns undefined when no JSON is present", () => {
        expect(parseJsonAnswer("plain text")).toBeUndefined();
    });

    it("returns undefined when candidate is invalid JSON", () => {
        // Balanced braces but not valid JSON.
        expect(parseJsonAnswer("{a:1}")).toBeUndefined();
    });
});
