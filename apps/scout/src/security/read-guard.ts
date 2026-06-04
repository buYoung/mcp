import { realpath } from "node:fs/promises";
import { extname, isAbsolute, relative, resolve, sep } from "node:path";
import {
    BINARY_FILE_EXTENSIONS,
    BLOCKED_DEVICE_PATHS,
    IMAGE_EXTENSIONS,
    UNSUPPORTED_DOCUMENT_EXTENSIONS,
} from "../config/defaults.js";
import { expandPath } from "./path-guard.js";

/** 읽기 차단 사유. read_file이 적절한 에러/안내로 전환한다. */
export class ImageUnsupportedError extends Error {}

/** PDF/Jupyter 노트북은 v1 미지원. read_file이 안내로 전환한다. */
export class DocumentUnsupportedError extends Error {}

/** 바이너리 확장자 거부. read_file이 안내로 전환한다. */
export class BinaryFileError extends Error {}

/**
 * `/dev/fd/<n>`(예: `/dev/fd/0`) 패턴. BLOCKED_DEVICE_PATHS 정확 일치로는
 * 못 잡는 가변 파일 디스크립터 경로를 차단한다.
 */
const DEV_FD_PATTERN = /^\/dev\/fd\/\d+$/;

/**
 * `/proc/<pid>/fd/<n>`(예: `/proc/123/fd/0`) 패턴. 다른 프로세스의 열린 파일
 * 디스크립터를 통한 우회를 차단한다.
 */
const PROC_FD_PATTERN = /^\/proc\/\d+\/fd\/\d+$/;

/**
 * 이미지 확장자 집합(소문자). 점 없는 소문자 확장자로 비교한다.
 */
const IMAGE_EXTENSION_SET = new Set<string>(IMAGE_EXTENSIONS);

/**
 * 미지원 문서(PDF/Jupyter) 확장자 집합(소문자).
 */
const DOCUMENT_EXTENSION_SET = new Set<string>(UNSUPPORTED_DOCUMENT_EXTENSIONS);

/**
 * 바이너리 확장자 집합(소문자). 이미지와 겹치지만 분류 순서상 이미지가 우선한다.
 */
const BINARY_EXTENSION_SET = new Set<string>(BINARY_FILE_EXTENSIONS);

/**
 * 입력 경로를 정규화·검증해 절대경로를 돌려준다. 위반 시 throw.
 *
 * 순서(DESIGN §3.4): UNC(`\\` 또는 `//`) 거부 → `expandPath` → 차단 디바이스 경로
 * 거부 → realpath 기준 cwd(repositoryRoot) 경계 내부 확인(밖이면 거부).
 *
 * 경계 검사는 형제 앱 `files-validation.ts`의 realpath 기반 `isPathWithinBoundary`
 * 로직을 복사·적용한다. realpath 실패 시 `resolve`로 폴백한다.
 */
export async function assertPathWithinRoot(inputPath: string, repositoryRoot: string): Promise<string> {
    // UNC 판정은 파일 I/O 전에 한다. 단 선행 공백을 trim한 뒤 판정해야 한다 —
    // expandPath가 내부에서 trim하므로, " //server/share"처럼 공백으로 시작하는 입력을
    // raw 기준으로만 검사하면 1차 방어를 우회해 절대 UNC 경로가 되어버린다.
    const trimmed = inputPath.trim();
    if (trimmed.startsWith("\\\\") || trimmed.startsWith("//")) {
        throw new Error(`UNC paths are not allowed: ${inputPath}`);
    }

    const expandedPath = expandPath(inputPath);

    // 차단 디바이스/특수 경로 거부(정확 일치 + /dev/fd, /proc/<pid>/fd 패턴).
    if (isBlockedDevicePath(expandedPath)) {
        throw new Error(`Reading device/special paths is not allowed: ${inputPath}`);
    }

    const boundary = await resolveBoundary(repositoryRoot);
    const canonicalPath = await resolveBoundary(expandedPath);
    if (!isPathWithinBoundary(canonicalPath, boundary)) {
        throw new Error(
            `Path is outside the repository root and was rejected: ${inputPath} (resolved=${canonicalPath}, root=${boundary})`,
        );
    }
    return canonicalPath;
}

/**
 * 이미 절대경로인 후보가 repositoryRoot 경계 안인지 realpath 기준으로 판정한다.
 * `assertPathWithinRoot`와 같은 경계 로직(`resolveBoundary` + `isPathWithinBoundary`)을
 * 쓰지만, UNC·차단 디바이스·`expandPath` 전처리는 생략한다 — globby 등이 돌려준
 * **이미 정규화된 절대경로 결과를 렌더 전에 재검증**하는 용도다.
 *
 * globby(fast-glob)는 cwd를 보안 경계로 쓰지 않으므로(상향 `..`·절대 패턴·심볼릭링크
 * 디렉터리 추적으로 cwd 밖을 반환할 수 있음) 결과 절대경로를 이 함수로 한 번 더 걸러야
 * 단일 repositoryRoot 경계(DESIGN §3.4)가 유지된다.
 */
export async function isAbsolutePathWithinRoot(absolutePath: string, repositoryRoot: string): Promise<boolean> {
    const boundary = await resolveBoundary(repositoryRoot);
    const canonicalPath = await resolveBoundary(absolutePath);
    return isPathWithinBoundary(canonicalPath, boundary);
}

/**
 * read_file 전용 파일-종류 게이트. 확장자로 이미지/문서/바이너리를 분류해 각각
 * `ImageUnsupportedError` / `DocumentUnsupportedError` / `BinaryFileError`를 throw한다.
 * `.svg`는 텍스트로 허용(예외). 그 외는 통과한다.
 */
export function assertReadableFileType(absolutePath: string): void {
    const extension = extractExtension(absolutePath);
    if (extension === "") {
        return;
    }

    // 이미지가 BINARY와 겹치므로 이미지 분류를 먼저 한다(이미지 에러 우선).
    if (IMAGE_EXTENSION_SET.has(extension)) {
        throw new ImageUnsupportedError("Image reading is not supported in v1.");
    }
    if (DOCUMENT_EXTENSION_SET.has(extension)) {
        throw new DocumentUnsupportedError("PDF/Jupyter reading is not supported by this MCP.");
    }
    if (BINARY_EXTENSION_SET.has(extension)) {
        throw new BinaryFileError(`Reading binary files is not supported: .${extension}`);
    }
    // .svg를 포함한 그 외 확장자는 텍스트로 허용한다.
}

/**
 * 차단 디바이스/특수 경로 여부. `BLOCKED_DEVICE_PATHS` 정확 일치 또는
 * `/dev/fd/<n>`·`/proc/<pid>/fd/<n>` 패턴이면 true.
 */
function isBlockedDevicePath(absolutePath: string): boolean {
    if ((BLOCKED_DEVICE_PATHS as readonly string[]).includes(absolutePath)) {
        return true;
    }
    return DEV_FD_PATTERN.test(absolutePath) || PROC_FD_PATTERN.test(absolutePath);
}

/**
 * 경로를 canonical 형태로 정규화한다. realpath 실패(미존재 등) 시 `resolve` 폴백
 * (형제 `files-validation.ts`의 `resolveBoundary` 복사).
 */
async function resolveBoundary(absolutePath: string): Promise<string> {
    try {
        return await realpath(absolutePath);
    } catch {
        return resolve(absolutePath);
    }
}

/**
 * `candidatePath`가 `boundaryPath` 경계 내부인지 판정한다(형제 `files-validation.ts`의
 * `isPathWithinBoundary` 복사). 경계 자체도 내부로 본다.
 */
function isPathWithinBoundary(candidatePath: string, boundaryPath: string): boolean {
    if (candidatePath === boundaryPath) {
        return true;
    }
    const relativePath = relative(boundaryPath, candidatePath);
    if (relativePath.length === 0) {
        return true;
    }
    if (relativePath.startsWith("..")) {
        return false;
    }
    if (isAbsolute(relativePath)) {
        return false;
    }
    return !relativePath.split(sep).includes("..");
}

/**
 * 파일명에서 점 없는 소문자 확장자를 뽑는다. 확장자가 없으면 빈 문자열.
 */
function extractExtension(absolutePath: string): string {
    const extension = extname(absolutePath);
    if (extension === "") {
        return "";
    }
    return extension.slice(1).toLowerCase();
}
