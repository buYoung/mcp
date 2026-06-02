import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { ENVIRONMENT_INDEX_DIRECTORY } from "../../config/defaults.js";

export interface IndexPaths {
    repositoryDirectory: string;
    shardDirectory: string;
    metaFilePath: string;
}

export function resolveCacheRootDirectory(): string {
    const override = process.env[ENVIRONMENT_INDEX_DIRECTORY];
    if (override != null && override.trim().length > 0) {
        return resolve(override.trim());
    }
    const xdgCacheHome = process.env.XDG_CACHE_HOME;
    const baseDirectory =
        xdgCacheHome != null && xdgCacheHome.trim().length > 0 ? xdgCacheHome.trim() : join(homedir(), ".cache");
    return join(baseDirectory, "scout", "zoekt");
}

export function resolveIndexPaths(repositoryRoot: string): IndexPaths {
    const normalizedRoot = resolve(repositoryRoot);
    const repositoryHash = createHash("sha256").update(normalizedRoot).digest("hex").slice(0, 16);
    const repositoryDirectory = join(resolveCacheRootDirectory(), repositoryHash);
    return {
        repositoryDirectory,
        shardDirectory: join(repositoryDirectory, "shards"),
        metaFilePath: join(repositoryDirectory, "meta.json"),
    };
}
