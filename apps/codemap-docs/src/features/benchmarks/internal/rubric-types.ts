export type RubricTermId = "core" | "extended";

export type RubricQuestionId =
    | "git-sync-job-dispatch"
    | "alertmanager-config-autosync"
    | "scope-annotation-filter";

export const RUBRIC_QUESTION_IDS: readonly RubricQuestionId[] = [
    "git-sync-job-dispatch",
    "alertmanager-config-autosync",
    "scope-annotation-filter",
];

export type RubricLang = "TS" | "Go" | "TS ↔ Go";

export interface InlineSegment {
    text: string;
    code: boolean;
}

export interface RubricFactItem {
    lang: RubricLang;
    text: string;
    hasSideEffect?: boolean;
}

export interface RubricQuestion {
    id: RubricQuestionId;
    tabLabel: string;
    singleTitle: string;
    items: RubricFactItem[];
}

export interface RubricEntry {
    id: RubricTermId;
    popoverId: string;
    titleId: string;
    label: string;
    chipLabel: string;
    title: string;
    criteria: string;
    tablistLabel: string;
    sideEffectLabel: string;
    questions: [RubricQuestion, RubricQuestion, RubricQuestion];
    closeLabel: string;
}

export function splitInlineCode(text: string): InlineSegment[] {
    const parts: InlineSegment[] = [];
    let cursor = 0;
    const pattern = /`([^`]+)`/g;
    for (const match of text.matchAll(pattern)) {
        const start = match.index ?? 0;
        if (start > cursor) {
            parts.push({ text: text.slice(cursor, start), code: false });
        }
        parts.push({ text: match[1] ?? "", code: true });
        cursor = start + match[0].length;
    }
    if (cursor < text.length) {
        parts.push({ text: text.slice(cursor), code: false });
    }
    return parts;
}
