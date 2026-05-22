import { describe, expect, it } from "vitest";
import { parsePairOpinion } from "../src/tools/pair-opinion.js";

describe("parsePairOpinion", () => {
    it("parses a complete JSON object", () => {
        const answer = JSON.stringify({
            stance: "disagree",
            summary: "Diverges from main agent's framing",
            agreements: ["scope is right"],
            concerns: ["misses TOCTOU"],
            recommendation: "Add a re-check after read",
            follow_up_questions: ["What about Windows?"],
        });

        const opinion = parsePairOpinion(answer);
        expect(opinion.parse_status).toBe("parsed");
        expect(opinion.stance).toBe("disagree");
        expect(opinion.summary).toContain("Diverges");
        expect(opinion.concerns).toEqual(["misses TOCTOU"]);
        expect(opinion.raw_answer).toBeUndefined();
    });

    it("clamps an unknown stance to insufficient_info", () => {
        const opinion = parsePairOpinion(JSON.stringify({ stance: "maybe", summary: "x", recommendation: "y" }));
        expect(opinion.parse_status).toBe("parsed");
        expect(opinion.stance).toBe("insufficient_info");
    });

    it("falls back when summary is missing", () => {
        const opinion = parsePairOpinion(JSON.stringify({ stance: "agree", recommendation: "x" }));
        expect(opinion.parse_status).toBe("fallback");
        expect(opinion.stance).toBe("insufficient_info");
        expect(opinion.raw_answer).toBeDefined();
    });

    it("falls back when answer is not JSON at all", () => {
        const opinion = parsePairOpinion("Just some prose without JSON.");
        expect(opinion.parse_status).toBe("fallback");
        expect(opinion.raw_answer).toBe("Just some prose without JSON.");
    });

    it("filters non-string entries from string arrays", () => {
        const opinion = parsePairOpinion(
            JSON.stringify({
                stance: "partial",
                summary: "s",
                recommendation: "r",
                agreements: ["ok", 1, "", null, "good"],
                concerns: [],
                follow_up_questions: ["why?"],
            }),
        );
        expect(opinion.agreements).toEqual(["ok", "good"]);
        expect(opinion.follow_up_questions).toEqual(["why?"]);
    });
});
