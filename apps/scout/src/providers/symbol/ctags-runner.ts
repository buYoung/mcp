import { execFile } from "node:child_process";
import { readdir, stat } from "node:fs/promises";
import { join, relative, sep } from "node:path";
import { CTAGS_EXEC_MAX_BUFFER_BYTES, CTAGS_EXEC_TIMEOUT_MS, CTAGS_OUTPUT_FIELDS } from "../../config/defaults.js";

/**
 * 구조화된 단일 ctags 태그. ctags JSON 출력의 한 줄을 도구 표면 형태로 매핑한 것.
 * `scope`/`signature`는 파서·언어가 제공할 때만 채워진다(미제공 시 undefined).
 */
export interface CtagsTag {
    /** 심볼 이름. */
    symbol: string;
    /** 심볼 종류(function/class/method/...). ctags `kind` 필드. */
    kind: string;
    /** 언어 이름(ctags 표기, 예: "TypeScript"). `--fields=+l`로 채운다. */
    language: string;
    /** repository root 기준 상대 경로(POSIX 구분자). */
    file: string;
    /** 1-기반 줄 번호. */
    line: number;
    /** 소속 스코프(클래스/네임스페이스 등). 없으면 undefined. */
    scope: string | undefined;
    /** 함수/메서드 시그니처. 파서가 제공할 때만. 없으면 undefined. */
    signature: string | undefined;
}

/**
 * ctags JSON 한 줄을 파싱하기 위한 원시 형태. ctags `--output-format=json`의
 * 각 줄은 이 형태의 객체(혹은 `_type`이 "tag"가 아닌 메타 줄)다.
 */
interface RawCtagsLine {
    _type?: string;
    name?: string;
    path?: string;
    line?: number;
    kind?: string;
    language?: string;
    scope?: string;
    signature?: string;
}

/**
 * 작업 트리 walk 결과. ctags에 넣을 소스 파일 절대경로 목록과, fingerprint에 fold할
 * 디렉터리 mtime 최댓값을 함께 돌려준다(`index-lifecycle.ts`와 동일 전략).
 */
export interface SourceWalkResult {
    /** ctags `-L`에 넣을 소스 파일 절대경로 목록. */
    files: string[];
    /** walk 중 만난 디렉터리들의 최신 mtime(ms). add/remove(rename/swap) 감지용. */
    maxDirectoryModifiedAtMs: number;
}

/**
 * 작업 트리(또는 path 하위)를 walk해 ctags에 넣을 소스 파일 절대경로 목록을 모은다.
 * `index-lifecycle.ts`의 walk 패턴을 따른다: 심볼릭 링크는 skip, `excludedDirectoryNames`에
 * 든 디렉터리는 통째로 skip. 읽을 수 없는 항목은 조용히 건너뛴다(부팅·조회를 막지 않음).
 *
 * 파일 목록과 함께 **디렉터리 mtime 최댓값**도 모은다 — fingerprint가 파일 mtime만
 * 보면 파일 수가 같은 rename/swap을 놓치므로(DESIGN §3.2), 디렉터리 mtime을 fold해
 * add/remove를 잡는다(`index-lifecycle.ts`의 working-tree fingerprint와 동일 전략).
 */
export async function collectSourceFiles(
    startDirectory: string,
    excludedDirectoryNames: ReadonlySet<string>,
): Promise<SourceWalkResult> {
    const files: string[] = [];
    let maxDirectoryModifiedAtMs = 0;

    async function walk(directory: string): Promise<void> {
        const entries = await readDirectoryEntries(directory);
        // 디렉터리 자체의 mtime을 fold한다(파일 add/remove 시 부모 디렉터리 mtime이 바뀜).
        try {
            const directoryStat = await stat(directory);
            if (directoryStat.mtimeMs > maxDirectoryModifiedAtMs) {
                maxDirectoryModifiedAtMs = directoryStat.mtimeMs;
            }
        } catch {
            // 읽을 수 없는 디렉터리 stat은 무시한다.
        }
        for (const entry of entries) {
            if (entry.isSymbolicLink()) {
                continue;
            }
            const entryPath = join(directory, entry.name);
            if (entry.isDirectory()) {
                if (excludedDirectoryNames.has(entry.name)) {
                    continue;
                }
                await walk(entryPath);
            } else if (entry.isFile()) {
                files.push(entryPath);
            }
        }
    }

    await walk(startDirectory);
    return { files, maxDirectoryModifiedAtMs };
}

async function readDirectoryEntries(directory: string) {
    try {
        return await readdir(directory, { withFileTypes: true });
    } catch {
        return [];
    }
}

/**
 * 작업 트리의 저렴한 fingerprint. 파일 수 + 파일·디렉터리 mtime 최댓값(floor)으로,
 * 추가/삭제/수정이 모두 값을 바꾸도록 한다(`index-lifecycle.ts`와 동일 전략).
 * `Math.floor`로 묶어 부동소수 mtime 비교가 깨지지 않게 한다.
 *
 * 디렉터리 mtime을 fold하는 이유: 파일 mtime만 보면 파일 수가 같은 rename/swap을 놓쳐
 * stale 태그를 캐시에서 돌려줄 수 있다(major 수정으로 캐시가 살아 있으므로 위험이 실재).
 * 파일 add/remove·rename 시 부모 디렉터리 mtime이 바뀌므로 이를 함께 본다.
 *
 * @param filePaths walk로 수집한 소스 파일 절대경로 목록.
 * @param maxDirectoryModifiedAtMs walk 중 만난 디렉터리들의 최신 mtime(ms).
 */
export async function computeSourceFingerprint(
    filePaths: readonly string[],
    maxDirectoryModifiedAtMs: number,
): Promise<string> {
    let maxModifiedAtMs = maxDirectoryModifiedAtMs;
    for (const filePath of filePaths) {
        try {
            const fileStat = await stat(filePath);
            if (fileStat.mtimeMs > maxModifiedAtMs) {
                maxModifiedAtMs = fileStat.mtimeMs;
            }
        } catch {
            // 읽을 수 없는 파일은 fingerprint에서 무시한다.
        }
    }
    return `${filePaths.length}:${Math.floor(maxModifiedAtMs)}`;
}

/**
 * ctags를 실행해 구조화된 태그 배열을 돌려준다.
 *
 * 핵심 규약(DESIGN §3.2): `-R` 금지(node_modules·JSON까지 훑어 52s). 대신 소스 파일
 * 목록을 **stdin(`-L -`)** 으로 주입한다 — 임시 파일 없이 큰 목록도 안전하게 전달한다.
 * 출력은 `--output-format=json --fields=${CTAGS_OUTPUT_FIELDS}` + 언어명(`+l`)을 받는다.
 * `language`가 명시되면 `--languages=<language>`로 파서를 제한한다.
 *
 * @param ctagsPath startup이 이미 Universal 변형을 검증한 ctags 실행 경로.
 * @param sourceFiles ctags에 넘길 절대경로 목록(walk로 수집).
 * @param repositoryRoot 출력 file 경로를 상대화할 기준 루트.
 * @param language ctags 언어 필터(미지정 시 전체 언어).
 */
export async function runCtags(
    ctagsPath: string,
    sourceFiles: readonly string[],
    repositoryRoot: string,
    language: string | undefined,
): Promise<CtagsTag[]> {
    if (sourceFiles.length === 0) {
        return [];
    }

    // `--fields=${CTAGS_OUTPUT_FIELDS}`는 spec 고정 상수(line·kind·scope·signature·access).
    // 언어명은 상수에 없으므로 additive `--fields=+l`로 별도 요청한다(ctags는 --fields를
    // 누적 적용한다). `-f -`로 tags를 stdout에 쓰고, `-L -`로 파일 목록을 stdin에서 읽는다.
    const args = ["--output-format=json", `--fields=${CTAGS_OUTPUT_FIELDS}`, "--fields=+l"];
    if (language != null && language.trim().length > 0) {
        args.push(`--languages=${language.trim()}`);
    }
    args.push("-f", "-", "-L", "-");

    const stdout = await execCtags(ctagsPath, args, sourceFiles);
    return parseCtagsOutput(stdout, repositoryRoot);
}

/**
 * ctags를 자식 프로세스로 실행하고 stdout 문자열을 돌려준다. 파일 목록은 stdin으로
 * 흘려 넣는다(`-L -`). stderr는 ctags 경고(예: TOML 파서 경고)가 섞이므로 stdout만 쓴다.
 */
function execCtags(ctagsPath: string, args: readonly string[], sourceFiles: readonly string[]): Promise<string> {
    return new Promise<string>((resolvePromise, rejectPromise) => {
        const child = execFile(
            ctagsPath,
            args as string[],
            {
                timeout: CTAGS_EXEC_TIMEOUT_MS,
                maxBuffer: CTAGS_EXEC_MAX_BUFFER_BYTES,
                encoding: "utf8",
            },
            (error, stdout) => {
                if (error != null) {
                    rejectPromise(error);
                    return;
                }
                resolvePromise(stdout);
            },
        );
        // 파일 목록을 줄바꿈으로 구분해 stdin으로 주입한다(임시 파일 불필요).
        const stdin = child.stdin;
        if (stdin == null) {
            rejectPromise(new Error("ctags 자식 프로세스의 stdin을 열 수 없습니다."));
            return;
        }
        stdin.on("error", rejectPromise);
        // 라인 인젝션 방어: 개행(`\n`/`\r`)이 든 경로는 join 후 별도 줄로 쪼개져 ctags `-L`에
        // "-h" 같은 옵션 라인을 주입할 수 있고, `-`로 시작하는 경로는 그 자체가 ctags 옵션으로
        // 해석된다(유닉스에선 둘 다 합법 파일명). stdin에 쓰기 전 이런 경로를 제외한다.
        const safeSourceFiles = sourceFiles.filter(
            (sourceFile) => !/[\r\n]/.test(sourceFile) && !sourceFile.startsWith("-"),
        );
        stdin.end(`${safeSourceFiles.join("\n")}\n`);
    });
}

/**
 * ctags JSON 라인 출력을 구조화 태그 배열로 파싱한다. 줄 단위로 처리하며,
 * `{`로 시작하지 않는 줄(경고 등)과 JSON 파싱 실패 줄, `_type`이 "tag"가 아닌 줄은 건너뛴다.
 */
function parseCtagsOutput(stdout: string, repositoryRoot: string): CtagsTag[] {
    const tags: CtagsTag[] = [];
    for (const rawLine of stdout.split("\n")) {
        const trimmed = rawLine.trim();
        if (trimmed.length === 0 || trimmed[0] !== "{") {
            continue;
        }
        let parsed: RawCtagsLine;
        try {
            parsed = JSON.parse(trimmed) as RawCtagsLine;
        } catch {
            continue;
        }
        if (parsed._type !== "tag" || parsed.name == null || parsed.path == null) {
            continue;
        }
        tags.push({
            symbol: parsed.name,
            kind: parsed.kind ?? "",
            language: parsed.language ?? "",
            file: toRepositoryRelative(parsed.path, repositoryRoot),
            line: typeof parsed.line === "number" ? parsed.line : 0,
            scope: parsed.scope,
            signature: parsed.signature,
        });
    }
    return tags;
}

/**
 * ctags가 돌려준 절대경로를 repository root 기준 POSIX 상대경로로 변환한다.
 * 루트 밖(이론상 발생하지 않으나 방어)이면 원본 경로를 그대로 둔다.
 */
function toRepositoryRelative(absolutePath: string, repositoryRoot: string): string {
    const relativePath = relative(repositoryRoot, absolutePath);
    if (relativePath.length === 0 || relativePath.startsWith("..")) {
        return absolutePath;
    }
    return relativePath.split(sep).join("/");
}
