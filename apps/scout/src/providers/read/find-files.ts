import { stat } from "node:fs/promises";
import { isAbsolute, relative, sep } from "node:path";
import { type GlobEntry, globby } from "globby";
import { FIND_FILES_RESULT_LIMIT, FIND_FILES_TRUNCATION_MESSAGE } from "../../config/defaults.js";
import { assertPathWithinRoot, isAbsolutePathWithinRoot } from "../../security/read-guard.js";

/** `find_files` 입력. 내부는 camelCase, MCP 키(snake_case) 매핑은 Integration이 담당한다. */
export interface FindFilesInput {
    /** 탐색할 glob 패턴. `**`,`*`,`?`,`[]`,`{}` 지원(picomatch 계열 globby). */
    pattern: string;
    /** 탐색 기준 디렉터리. 미지정 시 repositoryRoot(cwd). */
    path?: string | undefined;
}

/**
 * glob 패턴으로 파일 경로를 찾는 읽기 primitive(`find_files`). Claude Code Glob을
 * 충실히 모사한다 — 백엔드는 zoekt도 ripgrep도 아닌 JS glob 라이브러리(globby).
 *
 * 동작 고정 기본(DESIGN §4.2): `.gitignore` 무시 + 숨김 포함. mtime 오래된 순 정렬,
 * 상한 100건, 초과 시 마지막 줄에 절단 문구. 색인·webserver에 의존하지 않으므로
 * 색인 빌드 전이나 바이너리 미설치 상태에서도 동작한다.
 */
export class FindFilesProvider {
    private readonly repositoryRoot: string;

    constructor(options: { repositoryRoot: string }) {
        this.repositoryRoot = options.repositoryRoot;
    }

    async find(input: FindFilesInput): Promise<string> {
        // 1차 차단: 패턴 자체가 절대경로이거나 `..` 상향 세그먼트를 포함하면 거부한다.
        // globby(fast-glob)는 cwd를 보안 경계로 쓰지 않아 이런 패턴이 repositoryRoot 밖을
        // 가리키므로(실측), globby 호출 전에 사전 차단한다(DESIGN §3.4 단일 경계).
        this.assertSafePattern(input.pattern);

        const baseDirectory = await this.resolveBaseDirectory(input.path);

        // 동작 고정: .gitignore 무시 + 숨김 포함(Claude Code 재현). objectMode+stats로
        // 별도 stat 왕복 없이 mtime을 한 번에 얻어 정렬에 쓴다. followSymbolicLinks:false로
        // 루트 안에 외부를 가리키는 심볼릭링크 디렉터리를 추적해 경계 밖을 읽는 것을 막는다.
        const entries = await globby(input.pattern, {
            cwd: baseDirectory,
            gitignore: false,
            dot: true,
            onlyFiles: true,
            absolute: true,
            followSymbolicLinks: false,
            objectMode: true,
            stats: true,
        });

        // 결과 재검증(핵심 보안 경계): globby의 cwd 한정은 보안 경계가 아니므로, 돌려준
        // 각 절대경로를 realpath 기준으로 repositoryRoot 안인지 다시 확인해 밖이면 버린다.
        // 사전 패턴 차단·followSymbolicLinks:false를 우회하는 경로(예: 경계 안에서 시작해
        // 밖으로 빠지는 심볼릭링크 파일)까지 여기서 최종 차단한다(DESIGN §3.4).
        const withinRoot = await this.filterWithinRoot(entries);

        if (withinRoot.length === 0) {
            return "No files found";
        }

        // mtime 오래된 순(oldest first) 정렬. Glob `--sort=modified` 재현.
        // stats가 없는 엔트리는 안정성을 위해 mtime 0으로 취급한다.
        const sorted = withinRoot
            .map((entry) => ({ path: entry.path, mtimeMs: entry.stats?.mtimeMs ?? 0 }))
            .sort((left, right) => left.mtimeMs - right.mtimeMs);

        // 절단 시 오래된 FIND_FILES_RESULT_LIMIT개만 유지(의도된 동작).
        const truncated = sorted.length > FIND_FILES_RESULT_LIMIT;
        const kept = truncated ? sorted.slice(0, FIND_FILES_RESULT_LIMIT) : sorted;

        const lines = kept.map((entry) => this.relativizeToRoot(entry.path));
        if (truncated) {
            lines.push(FIND_FILES_TRUNCATION_MESSAGE);
        }
        return lines.join("\n");
    }

    /**
     * 패턴 사전 검증: 절대경로 패턴(`/etc/hosts`, Windows 드라이브 등)이나 `..` 상향
     * 세그먼트를 포함한 패턴을 거부한다. globby는 이런 패턴으로 cwd 밖 파일을 반환하므로
     * (실측: `../../*.txt`, `/etc/hosts`) repositoryRoot 경계를 우회하지 못하게 막는다.
     */
    private assertSafePattern(pattern: string): void {
        if (isAbsolute(pattern)) {
            throw new Error(`Absolute path patterns are not allowed: ${pattern}`);
        }
        // 백슬래시·슬래시 구분자 양쪽을 고려해 `..` 세그먼트를 검사한다(플랫폼 독립).
        const segments = pattern.split(/[\\/]/);
        if (segments.includes("..")) {
            throw new Error(`Parent-directory ('..') patterns are not allowed: ${pattern}`);
        }
    }

    /**
     * globby가 돌려준 절대경로 결과 중 repositoryRoot 경계 밖 항목을 걸러낸다(realpath 기준).
     * globby의 cwd는 보안 경계가 아니므로 이 재검증이 단일 repositoryRoot 경계를 보장한다.
     */
    private async filterWithinRoot(entries: readonly GlobEntry[]): Promise<GlobEntry[]> {
        const checks = await Promise.all(
            entries.map(async (entry) => ({
                entry,
                within: await isAbsolutePathWithinRoot(entry.path, this.repositoryRoot),
            })),
        );
        return checks.filter((check) => check.within).map((check) => check.entry);
    }

    /**
     * 탐색 기준 디렉터리를 결정한다. path 미지정 시 repositoryRoot,
     * 제공 시 read-guard로 경계 검증 후 존재·디렉터리 여부를 확인한다.
     */
    private async resolveBaseDirectory(inputPath: string | undefined): Promise<string> {
        if (inputPath == null || inputPath.trim().length === 0) {
            return this.repositoryRoot;
        }

        const canonicalPath = await assertPathWithinRoot(inputPath, this.repositoryRoot);
        const stats = await stat(canonicalPath);
        if (!stats.isDirectory()) {
            throw new Error(`Search path is not a directory: ${inputPath}`);
        }
        return canonicalPath;
    }

    /**
     * 절대경로를 repositoryRoot(cwd) 하위 상대경로로 바꾼다. 출력 구분자는 POSIX `/`로
     * 통일한다(플랫폼 독립). 헤더·줄번호는 붙이지 않는다.
     */
    private relativizeToRoot(absolutePath: string): string {
        const relativePath = relative(this.repositoryRoot, absolutePath);
        if (relativePath.length === 0) {
            return absolutePath;
        }
        return relativePath.split(sep).join("/");
    }
}
