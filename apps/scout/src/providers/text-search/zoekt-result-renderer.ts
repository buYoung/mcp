import type { OutputMode } from "../../config/defaults.js";
import type { ZoektFileMatch, ZoektLineMatch, ZoektSearchResult } from "./zoekt-search-client.js";

export interface RenderOptions {
    outputMode: OutputMode;
    showLineNumbers: boolean;
    headLimit: number;
    offset: number;
    contextLines: number;
}

export function renderSearchResult(result: ZoektSearchResult, options: RenderOptions): string {
    if (options.outputMode === "count") {
        return renderCount(result);
    }
    if (options.outputMode === "files_with_matches") {
        return renderFilesWithMatches(result, options);
    }
    return renderContent(result, options);
}

function renderCount(result: ZoektSearchResult): string {
    const { matchCount, fileCount } = result.stats;
    if (matchCount === 0) {
        return "No matches found";
    }
    return `Found ${matchCount} total occurrence(s) across ${fileCount} file(s).`;
}

function renderFilesWithMatches(result: ZoektSearchResult, options: RenderOptions): string {
    const fileNames = result.fileMatches.map((fileMatch) => fileMatch.fileName);
    if (fileNames.length === 0) {
        return "No files found";
    }
    const { page, truncated } = paginate(fileNames, options.offset, options.headLimit);
    const lines = [`Found ${result.stats.fileCount} file(s)`, ...page];
    if (truncated) {
        lines.push(buildPaginationFooter(options.offset, page.length, fileNames.length));
    }
    return lines.join("\n");
}

function renderContent(result: ZoektSearchResult, options: RenderOptions): string {
    const contentLines: string[] = [];
    for (const fileMatch of result.fileMatches) {
        for (const lineMatch of fileMatch.lineMatches) {
            contentLines.push(formatContentLine(fileMatch, lineMatch, options));
        }
    }
    if (contentLines.length === 0) {
        return "No matches found";
    }
    const { page, truncated } = paginate(contentLines, options.offset, options.headLimit);
    if (truncated) {
        page.push(buildPaginationFooter(options.offset, page.length, contentLines.length));
    }
    return page.join("\n");
}

function formatContentLine(fileMatch: ZoektFileMatch, lineMatch: ZoektLineMatch, options: RenderOptions): string {
    const lineText = reconstructLine(lineMatch);
    if (options.showLineNumbers) {
        return `${fileMatch.fileName}:${lineMatch.lineNumber}: ${lineText}`;
    }
    return `${fileMatch.fileName}: ${lineText}`;
}

/**
 * Reassembles a matched line from its fragments. Each fragment carries the text
 * before its match (`pre`) and the match itself; the trailing `post` of the last
 * fragment completes the line.
 */
function reconstructLine(lineMatch: ZoektLineMatch): string {
    if (lineMatch.fragments.length === 0) {
        return "";
    }
    let text = "";
    for (const fragment of lineMatch.fragments) {
        text += fragment.pre + fragment.match;
    }
    const lastFragment = lineMatch.fragments[lineMatch.fragments.length - 1];
    if (lastFragment != null) {
        text += lastFragment.post;
    }
    return text.replace(/\n$/, "");
}

interface PaginationResult {
    page: string[];
    truncated: boolean;
}

function paginate(items: string[], offset: number, headLimit: number): PaginationResult {
    const safeOffset = offset > 0 ? offset : 0;
    const limit = headLimit > 0 ? headLimit : items.length;
    const page = items.slice(safeOffset, safeOffset + limit);
    const truncated = safeOffset > 0 || safeOffset + page.length < items.length;
    return { page, truncated };
}

function buildPaginationFooter(offset: number, shownCount: number, totalCount: number): string {
    const safeOffset = offset > 0 ? offset : 0;
    return `[Showing results ${safeOffset + 1}-${safeOffset + shownCount} of ${totalCount}; pass a larger head_limit or offset to page further.]`;
}
