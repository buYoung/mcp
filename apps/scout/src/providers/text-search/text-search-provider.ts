import type { OutputMode } from "../../config/defaults.js";
import type { ResolvedScoutConfig } from "../../config/scout-config.js";
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
    private readonly config: ResolvedScoutConfig;
    private readonly indexLifecycle: IndexLifecycle;
    private readonly webserverLifecycle: WebserverLifecycle;

    constructor(options: {
        zoektIndexPath: string;
        zoektWebserverPath: string;
        repositoryRoot: string;
        config: ResolvedScoutConfig;
    }) {
        this.repositoryRoot = options.repositoryRoot;
        this.config = options.config;
        // excludedDirectories는 index.ts에서 gitignore union을 이미 마친 최종 목록이 전달된다.
        this.indexLifecycle = new IndexLifecycle({
            zoektIndexPath: options.zoektIndexPath,
            excludedDirectoryNames: this.config.index.excludedDirectories,
            stalenessCheckTtlMs: this.config.index.stalenessCheckMs,
            indexBuildTimeoutMs: this.config.limits.indexBuildTimeoutMs,
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

        const outputMode = input.outputMode ?? this.config.output.mode;
        const headLimit = input.headLimit ?? this.config.output.headLimit;
        const contextLines = input.contextLines ?? this.config.output.contextLines;
        const offset = input.offset ?? 0;
        const showLineNumbers = input.showLineNumbers ?? this.config.output.showLineNumbers;
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

        const searchOptions = { maxHits, contextLines, timeoutMs: this.config.limits.searchRequestTimeoutMs };
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
