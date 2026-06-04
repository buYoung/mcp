import { createReadStream } from "node:fs";
import { open, stat } from "node:fs/promises";
import {
    READ_FILE_BYTE_CAP,
    READ_FILE_FAST_PATH_MAX_SIZE,
    READ_FILE_STREAM_HIGH_WATER_MARK,
} from "../../config/defaults.js";
import { assertPathWithinRoot, assertReadableFileType } from "../../security/read-guard.js";
import { formatLinesWithNumbers } from "./line-numbering.js";
import { ReadStateStore } from "./read-state-store.js";

/**
 * limit 미지정 + 파일 크기가 `READ_FILE_BYTE_CAP`를 초과할 때 throw하는 에러.
 * 절단이 아니라 throw로 처리한다(DESIGN §4.1, 바이트 캡 비대칭성 보존).
 */
export class FileTooLargeError extends Error {}

/** read_file 입력. MCP 키는 snake_case이며 Integration 단계 zod에서 매핑된다. */
export interface ReadFileInput {
    file_path: string;
    offset?: number | undefined;
    limit?: number | undefined;
}

/**
 * 파일이 그대로일 때 반환하는 변경 없음 스텁(DESIGN §4.1 `FILE_UNCHANGED_STUB`).
 * 같은 파일을 같은 offset·limit로 다시 읽고, mtime도 동일하면 본문을 다시 싣지 않는다.
 */
const FILE_UNCHANGED_STUB =
    "<system-reminder>FILE_UNCHANGED: the file has not been modified since it was last read. Its contents are unchanged.</system-reminder>";

/** 빈 파일 안내(정확 재현, DESIGN §4.1). */
const EMPTY_FILE_REMINDER = "<system-reminder>Warning: the file exists but the contents are empty.</system-reminder>";

/**
 * `read_file` primitive — 단일 파일을 cat -n 형식으로 읽어 문자열로 돌려준다.
 * Claude Code의 Read(FileReadTool)를 충실히 모사한다(DESIGN §4.1).
 *
 * 토큰 캡·CYBER_RISK 문구·MAX_LINES_TO_READ 강제·`pages` 파라미터는 드롭했고,
 * 유일한 상한은 limit 미지정 시의 256KB 바이트 캡이다(DESIGN §4.1).
 */
export class ReadFileProvider {
    private readonly repositoryRoot: string;
    private readonly stateStore = new ReadStateStore();

    constructor(options: { repositoryRoot: string }) {
        this.repositoryRoot = options.repositoryRoot;
    }

    async read(input: ReadFileInput): Promise<string> {
        // 1) 경로 정규화·경계 검증 → 절대경로. UNC/차단 디바이스/경계 밖은 throw.
        const absolutePath = await assertPathWithinRoot(input.file_path, this.repositoryRoot);

        // 2) 파일 종류 게이트(이미지/문서/바이너리 거부, SVG·텍스트 허용).
        assertReadableFileType(absolutePath);

        // 3) stat — 디렉터리면 EISDIR류 에러, 그 외 정보로 분기.
        const fileStat = await stat(absolutePath);
        if (fileStat.isDirectory()) {
            throw new Error(
                `EISDIR: illegal operation on a directory, read. The path is a directory, not a file. Use find_files to list its contents: ${input.file_path}`,
            );
        }

        // 4) offset/limit 런타임 기본값. offset===0 과 1 모두 첫 줄을 가리킨다.
        const offset = input.offset ?? 1;
        const limit = input.limit;
        const lineOffset = offset === 0 ? 0 : offset - 1;
        // 줄 번호는 raw offset부터 시작한다(0이면 1로 보정해 1-기반 유지).
        const startLineNumber = offset === 0 ? 1 : offset;

        const timestamp = Math.floor(fileStat.mtimeMs);

        // 5) 빈 파일은 본문 없이 안내만 반환한다(상태도 기록해 dedup 일관성 유지).
        if (fileStat.size === 0) {
            this.stateStore.set(absolutePath, { timestamp, offset, limit });
            return EMPTY_FILE_REMINDER;
        }

        // 6) file_unchanged dedup: 같은 offset·limit + 동일 mtime이면 스텁 반환.
        const previousState = this.stateStore.get(absolutePath);
        if (
            previousState != null &&
            previousState.timestamp === timestamp &&
            previousState.offset === offset &&
            previousState.limit === limit
        ) {
            return FILE_UNCHANGED_STUB;
        }

        // 7) 바이트 캡: limit 미지정 시에만 전 파일 크기에 적용, 초과 시 throw.
        if (limit === undefined && fileStat.size > READ_FILE_BYTE_CAP) {
            throw new FileTooLargeError(
                `File content (${fileStat.size} bytes) exceeds the maximum read size of ${READ_FILE_BYTE_CAP} bytes. ` +
                    "Use the offset and limit parameters to read the file in smaller chunks.",
            );
        }

        // 8) 본문 읽기(대용량은 스트리밍). UTF-8 텍스트로 디코드.
        const content = await this.readContent(absolutePath, fileStat.size);

        // 9) 줄 분할 후 offset/limit 슬라이스.
        const allLines = splitLines(content);
        const totalLines = allLines.length;

        // 10) offset이 파일 줄 수를 초과하면 안내 반환.
        if (lineOffset >= totalLines) {
            this.stateStore.set(absolutePath, { timestamp, offset, limit });
            return offsetBeyondReminder(offset, totalLines);
        }

        const endLine = limit === undefined ? totalLines : lineOffset + limit;
        const selectedLines = allLines.slice(lineOffset, endLine);

        const rendered = formatLinesWithNumbers(selectedLines, startLineNumber);

        // 11) 읽기 성공 — dedup 상태 기록 후 반환.
        this.stateStore.set(absolutePath, { timestamp, offset, limit });
        return rendered;
    }

    /**
     * 파일 본문을 UTF-8 문자열로 읽는다. `READ_FILE_FAST_PATH_MAX_SIZE` 미만은
     * 한 번에 메모리로 읽고, 이상은 highWaterMark `READ_FILE_STREAM_HIGH_WATER_MARK`
     * 스트리밍으로 읽어 메모리 급증을 막는다(DESIGN §4.1 대용량 동작 재현).
     */
    private async readContent(absolutePath: string, fileSize: number): Promise<string> {
        if (fileSize < READ_FILE_FAST_PATH_MAX_SIZE) {
            const handle = await open(absolutePath, "r");
            try {
                const buffer = await handle.readFile();
                return buffer.toString("utf8");
            } finally {
                await handle.close();
            }
        }
        return await readStreamToString(absolutePath);
    }
}

/**
 * 스트리밍으로 파일을 읽어 UTF-8 문자열로 합친다(대용량 경로).
 */
function readStreamToString(absolutePath: string): Promise<string> {
    return new Promise<string>((resolvePromise, rejectPromise) => {
        const stream = createReadStream(absolutePath, {
            highWaterMark: READ_FILE_STREAM_HIGH_WATER_MARK,
            encoding: "utf8",
        });
        let accumulated = "";
        stream.on("data", (chunk) => {
            accumulated += chunk;
        });
        stream.on("end", () => {
            resolvePromise(accumulated);
        });
        stream.on("error", (error) => {
            rejectPromise(error);
        });
    });
}

/**
 * 본문을 줄 단위로 분할한다. `\r\n`·`\n` 모두 줄 끝으로 처리한다.
 *
 * 파일이 개행 하나로 끝나면(예: `"a\nb\nc\n"`) split이 마지막에 가짜 빈 원소를 만들어
 * totalLines가 1 부풀고 가짜 빈 줄("4\t")이 렌더되며 "M lines"도 1 오차가 난다.
 * `cat -n`·Claude Code Read는 trailing newline을 빈 줄로 세지 않으므로, split 전에
 * **끝의 단일 개행 하나만** 제거한다. 내부 빈 줄·연속 빈 줄(진짜 빈 줄)은 보존된다.
 */
function splitLines(content: string): string[] {
    return content.replace(/\r?\n$/, "").split(/\r?\n/);
}

/**
 * offset 초과 안내(정확 재현, DESIGN §4.1).
 * N = 요청 offset, M = 파일의 실제 줄 수.
 */
function offsetBeyondReminder(offset: number, totalLines: number): string {
    return `<system-reminder>Warning: the file exists but is shorter than the provided offset (${offset}). The file has ${totalLines} lines.</system-reminder>`;
}
