import { execFile } from "node:child_process";
import { mkdir, readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { promisify } from "node:util";
import {
    INDEX_BUILD_MAX_BUFFER_BYTES,
    INDEX_BUILD_TIMEOUT_MS,
    SCOUT_DIRECTORY_NAME,
    STALENESS_CHECK_TTL_MS,
} from "../../config/defaults.js";
import { resolveIndexPaths } from "./index-storage.js";

const execFileAsync = promisify(execFile);

export interface EnsureFreshResult {
    shardDirectory: string;
    buildGeneration: number;
    rebuilt: boolean;
}

interface IndexMeta {
    builtAtMs: number;
    repositoryRoot: string;
    fingerprint: string;
}

/**
 * Keeps a zoekt index of the working tree current. `zoekt-index` has no
 * incremental/delta mode (DESIGN §6.1 실측), so freshness is implemented here:
 * a cheap working-tree fingerprint decides whether a full re-index is needed,
 * concurrent builds are coalesced behind a single promise, and unchanged trees
 * skip the build entirely.
 */
export class IndexLifecycle {
    private readonly zoektIndexPath: string;
    private readonly excludedDirectoryNames: ReadonlySet<string>;
    private readonly ignoreDirectoriesArgument: string;
    private readonly stalenessCheckTtlMs: number;
    private readonly indexBuildTimeoutMs: number;

    private buildGeneration = 0;
    private lastFingerprint: string | undefined;
    private lastCheckAtMs = 0;
    private buildPromise: Promise<void> | undefined;

    constructor(options: {
        zoektIndexPath: string;
        excludedDirectoryNames: readonly string[];
        stalenessCheckTtlMs?: number;
        indexBuildTimeoutMs?: number;
    }) {
        this.zoektIndexPath = options.zoektIndexPath;
        // 인덱스가 `<repo>/.scout/zoekt` 안에 생성되므로(DESIGN §6.2) `.scout`를 항상 강제
        // 제외한다. 설정이 excluded_directories를 비우거나 교체해도 무조건 적용된다 — 빠지면
        // zoekt-index가 자기 샤드를 인덱싱하고, fingerprint walk가 매 빌드 산출물의 mtime을
        // 잡아 staleness TTL마다 무한 재인덱싱 루프에 빠진다. 같은 set이 zoekt `-ignore_dirs`와
        // working-tree fingerprint walk 양쪽에 쓰이므로 여기서 한 번만 강제하면 둘 다 안전하다.
        const excludedNames = new Set([SCOUT_DIRECTORY_NAME, ...options.excludedDirectoryNames]);
        this.excludedDirectoryNames = excludedNames;
        this.ignoreDirectoriesArgument = [...excludedNames].join(",");
        this.stalenessCheckTtlMs = options.stalenessCheckTtlMs ?? STALENESS_CHECK_TTL_MS;
        // 설정에서 인덱스 빌드 타임아웃을 받되, 미지정 시 built-in 기본값으로 폴백한다.
        this.indexBuildTimeoutMs = options.indexBuildTimeoutMs ?? INDEX_BUILD_TIMEOUT_MS;
    }

    async ensureFresh(repositoryRoot: string): Promise<EnsureFreshResult> {
        const { shardDirectory, metaFilePath } = resolveIndexPaths(repositoryRoot);
        const shardsExist = await directoryHasShards(shardDirectory);
        const now = Date.now();

        const withinThrottleWindow =
            shardsExist && this.lastFingerprint != null && now - this.lastCheckAtMs < this.stalenessCheckTtlMs;
        if (withinThrottleWindow) {
            return { shardDirectory, buildGeneration: this.buildGeneration, rebuilt: false };
        }

        const currentFingerprint = fingerprintToString(
            await computeWorkingTreeFingerprint(repositoryRoot, this.excludedDirectoryNames),
        );
        this.lastCheckAtMs = now;
        const knownFingerprint = this.lastFingerprint ?? (await readMetaFingerprint(metaFilePath));

        if (!shardsExist || currentFingerprint !== knownFingerprint) {
            await this.runBuildOnce(repositoryRoot, shardDirectory, metaFilePath, currentFingerprint);
            return { shardDirectory, buildGeneration: this.buildGeneration, rebuilt: true };
        }

        this.lastFingerprint = currentFingerprint;
        return { shardDirectory, buildGeneration: this.buildGeneration, rebuilt: false };
    }

    private async runBuildOnce(
        repositoryRoot: string,
        shardDirectory: string,
        metaFilePath: string,
        fingerprint: string,
    ): Promise<void> {
        if (this.buildPromise != null) {
            await this.buildPromise;
            return;
        }
        this.buildPromise = this.build(repositoryRoot, shardDirectory, metaFilePath, fingerprint);
        try {
            await this.buildPromise;
        } finally {
            this.buildPromise = undefined;
        }
    }

    private async build(
        repositoryRoot: string,
        shardDirectory: string,
        metaFilePath: string,
        fingerprint: string,
    ): Promise<void> {
        // Clear prior shards so a rebuild never leaves stale ones behind: zoekt-index
        // overwrites same-named shards but would orphan higher-numbered shards if the
        // corpus shrank, and the webserver would then serve stale hits.
        await rm(shardDirectory, { recursive: true, force: true });
        await mkdir(shardDirectory, { recursive: true });
        await execFileAsync(
            this.zoektIndexPath,
            ["-index", shardDirectory, "-ignore_dirs", this.ignoreDirectoriesArgument, repositoryRoot],
            { timeout: this.indexBuildTimeoutMs, maxBuffer: INDEX_BUILD_MAX_BUFFER_BYTES },
        );
        this.buildGeneration += 1;
        this.lastFingerprint = fingerprint;
        const meta: IndexMeta = { builtAtMs: Date.now(), repositoryRoot, fingerprint };
        await writeFile(metaFilePath, JSON.stringify(meta, null, 2), "utf8");
    }
}

async function directoryHasShards(shardDirectory: string): Promise<boolean> {
    try {
        const entries = await readdir(shardDirectory);
        return entries.some((entry) => entry.endsWith(".zoekt"));
    } catch {
        return false;
    }
}

async function readMetaFingerprint(metaFilePath: string): Promise<string | undefined> {
    try {
        const raw = await readFile(metaFilePath, "utf8");
        const parsed = JSON.parse(raw) as Partial<IndexMeta>;
        return typeof parsed.fingerprint === "string" ? parsed.fingerprint : undefined;
    } catch {
        return undefined;
    }
}

interface WorkingTreeFingerprint {
    fileCount: number;
    maxModifiedAtMs: number;
}

function fingerprintToString(fingerprint: WorkingTreeFingerprint): string {
    return `${fingerprint.fileCount}:${Math.floor(fingerprint.maxModifiedAtMs)}`;
}

/**
 * Walks the working tree (skipping excluded directories and symlinks) and
 * returns a cheap fingerprint. File and directory mtimes are both folded in, so
 * additions, deletions, renames, and edits all change the fingerprint.
 */
async function computeWorkingTreeFingerprint(
    repositoryRoot: string,
    excludedDirectoryNames: ReadonlySet<string>,
): Promise<WorkingTreeFingerprint> {
    let fileCount = 0;
    let maxModifiedAtMs = 0;

    async function walk(directory: string): Promise<void> {
        const entries = await readDirectoryEntries(directory);
        try {
            const directoryStat = await stat(directory);
            if (directoryStat.mtimeMs > maxModifiedAtMs) {
                maxModifiedAtMs = directoryStat.mtimeMs;
            }
        } catch {
            // ignore unreadable directory stat
        }
        for (const entry of entries) {
            if (entry.isSymbolicLink()) {
                continue;
            }
            if (entry.isDirectory()) {
                if (excludedDirectoryNames.has(entry.name)) {
                    continue;
                }
                await walk(join(directory, entry.name));
            } else if (entry.isFile()) {
                fileCount += 1;
                try {
                    const fileStat = await stat(join(directory, entry.name));
                    if (fileStat.mtimeMs > maxModifiedAtMs) {
                        maxModifiedAtMs = fileStat.mtimeMs;
                    }
                } catch {
                    // ignore unreadable file stat
                }
            }
        }
    }

    await walk(repositoryRoot);
    return { fileCount, maxModifiedAtMs };
}

async function readDirectoryEntries(directory: string) {
    try {
        return await readdir(directory, { withFileTypes: true });
    } catch {
        return [];
    }
}
