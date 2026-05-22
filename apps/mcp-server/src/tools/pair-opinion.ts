import { parseJsonAnswer } from "./json-extract.js";

export type PairStance = "agree" | "disagree" | "partial" | "insufficient_info";

export interface PairOpinion {
    stance: PairStance;
    summary: string;
    agreements: string[];
    concerns: string[];
    recommendation: string;
    follow_up_questions: string[];
    parse_status: "parsed" | "fallback";
    raw_answer?: string;
}

export function parsePairOpinion(answer: string): PairOpinion {
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
        stance: readStanceProperty(parsedValue, "stance"),
        summary,
        agreements: readStringArrayProperty(parsedValue, "agreements"),
        concerns: readStringArrayProperty(parsedValue, "concerns"),
        recommendation,
        follow_up_questions: readStringArrayProperty(parsedValue, "follow_up_questions"),
        parse_status: "parsed",
    };
}

function createFallbackPairOpinion(answer: string): PairOpinion {
    return {
        stance: "insufficient_info",
        summary: "Pair agent did not return structured JSON. See raw_answer.",
        agreements: [],
        concerns: [],
        recommendation: "",
        follow_up_questions: [],
        parse_status: "fallback",
        raw_answer: answer,
    };
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

function readStanceProperty(value: Record<string, unknown>, key: string): PairStance {
    const propertyValue = value[key];
    if (
        propertyValue === "agree" ||
        propertyValue === "disagree" ||
        propertyValue === "partial" ||
        propertyValue === "insufficient_info"
    ) {
        return propertyValue;
    }
    return "insufficient_info";
}
