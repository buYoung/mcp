import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { access, chmod, mkdir, open, rename, rm } from "node:fs/promises";
import { dirname, join } from "node:path";
import { promisify } from "node:util";
import {
    ARCHIVE_DOWNLOAD_MAX_BYTES,
    ARCHIVE_EXTRACT_MAX_BUFFER_BYTES,
    ARCHIVE_EXTRACT_TIMEOUT_MS,
    BINARY_DOWNLOAD_TIMEOUT_MS,
    BINARY_RELEASE_DOWNLOAD_BASE_URL,
    BINARY_RELEASE_TAG,
    CTAGS_BINARY,
    CTAGS_RELEASE_BINARY,
    ZOEKT_GIT_INDEX_BINARY,
    ZOEKT_INDEX_BINARY,
    ZOEKT_WEBSERVER_BINARY,
} from "../config/defaults.js";
import { type PlatformAsset, resolvePlatformAsset } from "./binary-release.js";
import { resolveManagedBinDirectory } from "./managed-bin-storage.js";

const execFileAsync = promisify(execFile);

/** 아카이브에서 추출되는 바이너리들(플랫폼 접미사 제외). `ctags`는 추출 후 rename 결과명. */
const BUNDLED_BINARY_BASENAMES = [
    ZOEKT_INDEX_BINARY,
    ZOEKT_WEBSERVER_BINARY,
    ZOEKT_GIT_INDEX_BINARY,
    CTAGS_BINARY,
] as const;

export type InstallOutcome =
    | { status: "installed"; directory: string; tag: string; binaries: string[] }
    | { status: "unsupported-platform"; platform: string; architecture: string }
    | { status: "failed"; message: string };

/**
 * 핀 고정된 릴리스 태그에서 현재 플랫폼용 아카이브를 받아 SHA-256 검증 후 관리형
 * bin 디렉터리에 설치한다. 무결성 실패·네트워크 오류·플랫폼 미지원은 던지지 않고
 * 결과 객체로 보고한다(호출부가 안내 텍스트로 변환).
 *
 * 안전성: 모든 작업은 **스테이징 디렉터리**에서 수행하고, 검증·추출·정리가 끝난 뒤에야
 * 최종 디렉터리로 원자적 교체(rename)한다 — 다운로드 중 오류가 나도 기존 설치를 건드리지
 * 않고, 실행 중인 자식(zoekt-webserver 등)이 쓰는 파일을 도중에 지우지 않는다.
 */
export async function installManagedBinaries(): Promise<InstallOutcome> {
    const asset = resolvePlatformAsset();
    if (asset == null) {
        return { status: "unsupported-platform", platform: process.platform, architecture: process.arch };
    }

    const finalDirectory = resolveManagedBinDirectory();
    const stagingDirectory = `${finalDirectory}.staging`;
    const archivePath = join(stagingDirectory, asset.assetName);

    try {
        await rm(stagingDirectory, { recursive: true, force: true });
        await mkdir(stagingDirectory, { recursive: true });

        const actualSha = await downloadArchiveToFile(
            `${BINARY_RELEASE_DOWNLOAD_BASE_URL}/${asset.assetName}`,
            archivePath,
            asset,
        );
        const expectedSha = parseSha256(
            await downloadText(`${BINARY_RELEASE_DOWNLOAD_BASE_URL}/${asset.assetName}.sha256`),
        );
        if (expectedSha == null) {
            await rm(stagingDirectory, { recursive: true, force: true });
            return {
                status: "failed",
                message: `SHA256 체크섬 파일을 해석하지 못했습니다(${asset.assetName}.sha256).`,
            };
        }
        if (actualSha.toLowerCase() !== expectedSha.toLowerCase()) {
            await rm(stagingDirectory, { recursive: true, force: true });
            return {
                status: "failed",
                message: `SHA256 무결성 검증 실패. 다운로드를 폐기했습니다.\n  기대값: ${expectedSha}\n  실제값: ${actualSha}`,
            };
        }

        await extractArchive(archivePath, stagingDirectory, asset.isZip);
        await rm(archivePath, { force: true });
        const binaries = await finalizeBinaries(stagingDirectory);

        await swapIntoPlace(stagingDirectory, finalDirectory);
        return { status: "installed", directory: finalDirectory, tag: BINARY_RELEASE_TAG, binaries };
    } catch (error) {
        await rm(stagingDirectory, { recursive: true, force: true }).catch(() => undefined);
        return { status: "failed", message: describeError(error, asset) };
    }
}

/**
 * 응답 본문을 스트리밍하며 파일로 쓰고 SHA-256을 증분 계산한다. AbortController 타이머를
 * **본문 수신 내내** 유지해(헤더 도착 후가 아니라) 본문이 멈추는 slow-loris/지연도 타임아웃
 * 시킨다. 최대 크기를 넘으면 즉시 중단한다.
 */
async function downloadArchiveToFile(url: string, filePath: string, asset: PlatformAsset): Promise<string> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), BINARY_DOWNLOAD_TIMEOUT_MS);
    try {
        const response = await fetch(url, { redirect: "follow", signal: controller.signal });
        if (!response.ok) {
            throw httpDownloadError(response.status, asset);
        }
        assertContentLengthWithinLimit(response.headers.get("content-length"), asset);

        const hash = createHash("sha256");
        let totalBytes = 0;
        const fileHandle = await open(filePath, "w");
        try {
            const body = response.body;
            if (body == null) {
                throw new Error("응답 본문이 비어 있습니다.");
            }
            for await (const chunk of body as AsyncIterable<Uint8Array>) {
                totalBytes += chunk.byteLength;
                if (totalBytes > ARCHIVE_DOWNLOAD_MAX_BYTES) {
                    throw new Error(`아카이브가 허용 크기(${ARCHIVE_DOWNLOAD_MAX_BYTES} bytes)를 초과했습니다.`);
                }
                hash.update(chunk);
                await fileHandle.write(chunk);
            }
        } finally {
            await fileHandle.close();
        }
        return hash.digest("hex");
    } finally {
        clearTimeout(timer);
    }
}

async function downloadText(url: string): Promise<string> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), BINARY_DOWNLOAD_TIMEOUT_MS);
    try {
        const response = await fetch(url, { redirect: "follow", signal: controller.signal });
        if (!response.ok) {
            throw new Error(`체크섬 다운로드 실패 (HTTP ${response.status}).`);
        }
        return await response.text();
    } finally {
        clearTimeout(timer);
    }
}

function httpDownloadError(status: number, asset: PlatformAsset): Error {
    // 404는 이 플랫폼 자산이 아직 릴리스에 없음을 뜻할 수 있다(매핑에 없거나 미업로드된 플랫폼).
    const hint =
        status === 404
            ? ` 이 플랫폼(${asset.assetName})의 자산이 릴리스에 없을 수 있습니다 — 수동 설치가 필요합니다.`
            : "";
    return new Error(`다운로드 실패 (HTTP ${status}).${hint}`);
}

function assertContentLengthWithinLimit(headerValue: string | null, asset: PlatformAsset): void {
    if (headerValue == null) {
        return;
    }
    const declared = Number(headerValue);
    if (Number.isFinite(declared) && declared > ARCHIVE_DOWNLOAD_MAX_BYTES) {
        throw new Error(`아카이브(${asset.assetName})가 허용 크기를 초과한다고 보고되었습니다(${declared} bytes).`);
    }
}

/** `<hex>␣␣<filename>` 형식의 체크섬 파일에서 첫 토큰(해시)만 추출한다. */
function parseSha256(text: string): string | undefined {
    const token = text.trim().split(/\s+/u)[0];
    return token != null && /^[0-9a-fA-F]{64}$/u.test(token) ? token : undefined;
}

/**
 * 시스템 `tar`로 추출하며 선두 플랫폼 디렉터리(`<platform>/`) 1단계를 strip 한다.
 * gzip tarball은 `-xzf`, zip(Windows)은 bsdtar의 zip 지원으로 `-xf`를 쓴다. Windows에서는
 * PATH의 `tar`가 zip을 못 읽는 GNU tar(MSYS/Git-for-Windows 등)일 수 있어, libarchive
 * 기반 번들 tar(`System32\\tar.exe`, Windows 10 1803+)를 **절대 경로로** 호출한다.
 */
async function extractArchive(archivePath: string, destinationDirectory: string, isZip: boolean): Promise<void> {
    const flags = isZip ? "-xf" : "-xzf";
    await execFileAsync(tarExecutablePath(), [flags, archivePath, "-C", destinationDirectory, "--strip-components=1"], {
        timeout: ARCHIVE_EXTRACT_TIMEOUT_MS,
        maxBuffer: ARCHIVE_EXTRACT_MAX_BUFFER_BYTES,
    });
}

function tarExecutablePath(): string {
    if (process.platform === "win32") {
        const systemRoot = process.env.SystemRoot ?? process.env.windir ?? "C:\\Windows";
        return join(systemRoot, "System32", "tar.exe");
    }
    return "tar";
}

/**
 * 추출 직후 정리: `universal-ctags`를 MCP가 탐색하는 이름(`ctags`)으로 rename 하고,
 * Unix에서 실행 비트를 보장한다. 실제 존재가 확인된 바이너리 목록을 돌려준다.
 */
async function finalizeBinaries(directory: string): Promise<string[]> {
    const executableSuffix = process.platform === "win32" ? ".exe" : "";

    const releasedCtags = join(directory, `${CTAGS_RELEASE_BINARY}${executableSuffix}`);
    const targetCtags = join(directory, `${CTAGS_BINARY}${executableSuffix}`);
    await rename(releasedCtags, targetCtags).catch(() => undefined);

    const present: string[] = [];
    for (const basename of BUNDLED_BINARY_BASENAMES) {
        const filePath = join(directory, `${basename}${executableSuffix}`);
        const exists = await access(filePath).then(
            () => true,
            () => false,
        );
        if (!exists) {
            continue;
        }
        if (process.platform !== "win32") {
            await chmod(filePath, 0o755).catch(() => undefined);
        }
        present.push(basename);
    }
    return present;
}

/**
 * 스테이징을 최종 위치로 원자적 교체한다. 기존 디렉터리를 제거한 뒤 rename 한다.
 * 드물게 다른 프로세스가 동시에 같은 곳에 교체 중이면 rename이 충돌할 수 있어 1회 재시도한다.
 */
async function swapIntoPlace(stagingDirectory: string, finalDirectory: string): Promise<void> {
    await mkdir(dirname(finalDirectory), { recursive: true });
    try {
        await rm(finalDirectory, { recursive: true, force: true });
        await rename(stagingDirectory, finalDirectory);
    } catch {
        await rm(finalDirectory, { recursive: true, force: true });
        await rename(stagingDirectory, finalDirectory);
    }
}

function describeError(error: unknown, asset: PlatformAsset): string {
    if (error instanceof Error) {
        if (error.name === "AbortError") {
            return `다운로드/추출 시간 초과(${asset.assetName}).`;
        }
        if ((error as NodeJS.ErrnoException).code === "ENOENT" && error.message.includes("tar")) {
            return "아카이브 추출 도구 `tar`를 찾을 수 없습니다. (Windows는 10 1803+에 tar.exe가 기본 포함됩니다. 수동 설치가 필요할 수 있습니다.)";
        }
        return error.message;
    }
    return String(error);
}
