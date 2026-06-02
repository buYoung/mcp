import { httpGet } from "./http-get.js";

export interface ZoektFragment {
    pre: string;
    match: string;
    post: string;
}

export interface ZoektLineMatch {
    lineNumber: number;
    fragments: ZoektFragment[];
}

export interface ZoektFileMatch {
    fileName: string;
    language: string;
    lineMatches: ZoektLineMatch[];
    before: string;
    after: string;
}

export interface ZoektStats {
    matchCount: number;
    fileCount: number;
    durationNs: number;
}

export interface ZoektSearchResult {
    fileMatches: ZoektFileMatch[];
    stats: ZoektStats;
    parsedQuery: string;
}

export interface SearchOptions {
    maxHits: number;
    contextLines: number;
    timeoutMs: number;
}

/** Thrown when the webserver cannot be reached so the caller can restart and retry. */
export class WebserverUnreachableError extends Error {}

interface RawZoektResponse {
    result?: {
        Query?: string;
        Stats?: {
            MatchCount?: number;
            FileCount?: number;
            Duration?: number;
        };
        FileMatches?: Array<{
            FileName?: string;
            Language?: string;
            Before?: string;
            After?: string;
            Matches?: Array<{
                LineNum?: number;
                Fragments?: Array<{
                    Pre?: string;
                    Match?: string;
                    Post?: string;
                }>;
            }>;
        }>;
    };
}

/**
 * Queries the `zoekt-webserver` JSON endpoint
 * (`GET /search?q=&format=json&num=&ctx=`, DESIGN §6.1 실측) and normalizes the
 * UI-shaped response into camelCase records.
 */
export async function searchZoekt(port: number, query: string, options: SearchOptions): Promise<ZoektSearchResult> {
    const params = new URLSearchParams({
        q: query,
        format: "json",
        num: String(options.maxHits),
        ctx: String(options.contextLines),
    });
    const url = `http://127.0.0.1:${port}/search?${params.toString()}`;

    let response: { statusCode: number; body: string };
    try {
        response = await httpGet(url, options.timeoutMs);
    } catch (error) {
        throw new WebserverUnreachableError(`zoekt-webserver request failed: ${(error as Error).message}`);
    }
    if (response.statusCode !== 200) {
        throw new WebserverUnreachableError(`zoekt-webserver returned HTTP ${response.statusCode}.`);
    }

    const parsed = JSON.parse(response.body) as RawZoektResponse;
    const result = parsed.result;
    if (result == null) {
        throw new Error("zoekt-webserver response did not contain a 'result' object.");
    }

    return {
        parsedQuery: result.Query ?? "",
        stats: {
            matchCount: result.Stats?.MatchCount ?? 0,
            fileCount: result.Stats?.FileCount ?? 0,
            durationNs: result.Stats?.Duration ?? 0,
        },
        fileMatches: (result.FileMatches ?? []).map((fileMatch) => ({
            fileName: fileMatch.FileName ?? "",
            language: fileMatch.Language ?? "",
            before: fileMatch.Before ?? "",
            after: fileMatch.After ?? "",
            lineMatches: (fileMatch.Matches ?? []).map((lineMatch) => ({
                lineNumber: lineMatch.LineNum ?? 0,
                fragments: (lineMatch.Fragments ?? []).map((fragment) => ({
                    pre: fragment.Pre ?? "",
                    match: fragment.Match ?? "",
                    post: fragment.Post ?? "",
                })),
            })),
        })),
    };
}
