import { join, resolve } from "node:path";
import { SCOUT_DIRECTORY_NAME } from "../../config/defaults.js";

export interface IndexPaths {
    repositoryDirectory: string;
    shardDirectory: string;
    metaFilePath: string;
}

export function resolveIndexPaths(repositoryRoot: string): IndexPaths {
    // 인덱스는 전역 캐시가 아니라 레포 안 <repo>/.scout/zoekt 에 고정 배치한다.
    // (전역 캐시 + repo-path 해시 디렉터리 방식은 폐기됨)
    const repositoryDirectory = join(resolve(repositoryRoot), SCOUT_DIRECTORY_NAME, "zoekt");
    return {
        repositoryDirectory,
        shardDirectory: join(repositoryDirectory, "shards"),
        metaFilePath: join(repositoryDirectory, "meta.json"),
    };
}
