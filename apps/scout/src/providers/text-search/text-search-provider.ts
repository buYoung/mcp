import {
    DEFAULT_CONTEXT_LINES,
    DEFAULT_HEAD_LIMIT,
    DEFAULT_OUTPUT_MODE,
    EXCLUDED_DIRECTORY_NAMES,
    type OutputMode,
    SEARCH_REQUEST_TIMEOUT_MS,
} from "../../config/defaults.js";
import { resolveRelativePathWithinRoot } from "../../security/path-guard.js";
import { IndexLifecycle } from "./index-lifecycle.js";
import { buildZoektQuery } from "./zoekt-query-builder.js";
import { renderSearchResult } from "./zoekt-result-renderer.js";
import { searchZoekt, WebserverUnreachableError } from "./zoekt-search-client.js";
import { WebserverLifecycle } from "./zoekt-webserver-lifecycle.js";

export interface SearchTextInput {
    pattern: string;
    path?: string | undefined;
    glob?: string | undefined;
    type?: string | undefined;
    outputMode?: OutputMode | undefined;
    caseInsensitive?: boolean | undefined;
    showLineNumbers?: boolean | undefined;
    contextLines?: number | undefined;
    headLimit?: number | undefined;
    offset?: number | undefined;
}

/**
 * The zoekt-backed text search primitive (`search_text`). Orchestrates the
 * pipeline: keep the working-tree index fresh, keep the webserver warm, query it,
 * and render the Grep-style result.
 */
export class TextSearchProvider {
    private readonly repositoryRoot: string;
    private readonly indexLifecycle: IndexLifecycle;
    private readonly webserverLifecycle: WebserverLifecycle;

    constructor(options: { zoektIndexPath: string; zoektWebserverPath: string; repositoryRoot: string }) {
        this.repositoryRoot = options.repositoryRoot;
        this.indexLifecycle = new IndexLifecycle({
            zoektIndexPath: options.zoektIndexPath,
            excludedDirectoryNames: EXCLUDED_DIRECTORY_NAMES,
        });
        this.webserverLifecycle = new WebserverLifecycle({
            zoektWebserverPath: options.zoektWebserverPath,
        });
    }

    async search(input: SearchTextInput): Promise<string> {
        const relativePathPrefix =
            input.path != null && input.path.trim().length > 0
                ? resolveRelativePathWithinRoot(input.path, this.repositoryRoot)
                : undefined;

        const outputMode = input.outputMode ?? DEFAULT_OUTPUT_MODE;
        const headLimit = input.headLimit ?? DEFAULT_HEAD_LIMIT;
        const contextLines = input.contextLines ?? DEFAULT_CONTEXT_LINES;
        const offset = input.offset ?? 0;
        const showLineNumbers = input.showLineNumbers ?? true;
        const maxHits = headLimit > 0 ? headLimit : 100_000;

        const query = buildZoektQuery({
            pattern: input.pattern,
            glob: input.glob,
            type: input.type,
            caseInsensitive: input.caseInsensitive,
            relativePathPrefix,
        });

        const result = await this.runQuery(query, maxHits, contextLines);

        return renderSearchResult(result, {
            outputMode,
            showLineNumbers,
            headLimit,
            offset,
            contextLines,
        });
    }

    shutdown(): void {
        this.webserverLifecycle.shutdown();
    }

    private async runQuery(query: string, maxHits: number, contextLines: number) {
        const { shardDirectory, buildGeneration } = await this.indexLifecycle.ensureFresh(this.repositoryRoot);
        let port = await this.webserverLifecycle.ensureRunning(shardDirectory, buildGeneration);

        const searchOptions = { maxHits, contextLines, timeoutMs: SEARCH_REQUEST_TIMEOUT_MS };
        try {
            return await searchZoekt(port, query, searchOptions);
        } catch (error) {
            if (!(error instanceof WebserverUnreachableError)) {
                throw error;
            }
            // The warm webserver died; restart once and retry.
            this.webserverLifecycle.markUnhealthy();
            port = await this.webserverLifecycle.ensureRunning(shardDirectory, buildGeneration);
            return await searchZoekt(port, query, searchOptions);
        }
    }
}
