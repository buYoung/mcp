export function extractJsonCandidate(answer: string): string | undefined {
    const fencedJsonMatch = /```(?:json)?\s*([\s\S]*?)```/.exec(answer);
    const fencedJsonCandidate = fencedJsonMatch?.[1]?.trim();
    if (fencedJsonCandidate != null && fencedJsonCandidate.length > 0) {
        return fencedJsonCandidate;
    }
    return extractFirstBalancedJsonObject(answer);
}

export function extractFirstBalancedJsonObject(text: string): string | undefined {
    let index = 0;
    while (index < text.length) {
        const openIndex = text.indexOf("{", index);
        if (openIndex === -1) {
            return undefined;
        }

        const endIndex = scanBalancedObjectEnd(text, openIndex);
        if (endIndex !== -1) {
            return text.slice(openIndex, endIndex + 1);
        }
        index = openIndex + 1;
    }
    return undefined;
}

export function parseJsonAnswer(answer: string): Record<string, unknown> | undefined {
    const candidate = extractJsonCandidate(answer);
    if (candidate == null) {
        return undefined;
    }
    try {
        const parsedValue = JSON.parse(candidate) as unknown;
        if (typeof parsedValue === "object" && parsedValue != null && !Array.isArray(parsedValue)) {
            return parsedValue as Record<string, unknown>;
        }
    } catch {
        return undefined;
    }
    return undefined;
}

function scanBalancedObjectEnd(text: string, startIndex: number): number {
    let depth = 0;
    let inString = false;
    let escapeNext = false;
    for (let index = startIndex; index < text.length; index += 1) {
        const character = text[index];
        if (inString) {
            if (escapeNext) {
                escapeNext = false;
                continue;
            }
            if (character === "\\") {
                escapeNext = true;
                continue;
            }
            if (character === '"') {
                inString = false;
            }
            continue;
        }
        if (character === '"') {
            inString = true;
            continue;
        }
        if (character === "{") {
            depth += 1;
            continue;
        }
        if (character === "}") {
            depth -= 1;
            if (depth === 0) {
                return index;
            }
        }
    }
    return -1;
}
