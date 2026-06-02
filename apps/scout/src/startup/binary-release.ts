/**
 * 현재 OS/아키텍처를 릴리스 자산으로 매핑한다. 릴리스 아카이브는 플랫폼별로
 * `zoekt-ctags-<platform>.<ext>` 하나에 zoekt(index/webserver/git-index)와
 * Universal Ctags 바이너리를 모두 번들한다 — 한 번 받으면 필수 바이너리가 전부 해결된다.
 *
 * 아카이브 내부는 `<platform>/<binary>` 구조라 추출 시 선두 디렉터리 1단계를
 * strip 한다(`archiveLeadingDirectory`는 참고/로깅용).
 *
 * 릴리스 v0.0.3부터 모든 OS × (x86_64/amd64, arm64) 6종이 모두 제공된다
 * (linux/macos/windows × amd64/arm64). 자산이 없는 플랫폼은 다운로드 단계에서
 * 자산 부재(404)로 graceful 폴백된다.
 */
export interface PlatformAsset {
    /** 릴리스 자산 파일명 (예: `zoekt-ctags-macos-arm64.tar.gz`). */
    assetName: string;
    /** 아카이브 내부 선두 디렉터리명 (예: `macos-arm64`). */
    archiveLeadingDirectory: string;
    /** zip 아카이브 여부(Windows). false면 gzip tarball. */
    isZip: boolean;
}

interface PlatformDescriptor {
    /** 자산명에 쓰이는 OS 토큰 (`linux` | `macos` | `windows`). */
    osToken: string;
    /** 자산명에 쓰이는 아키텍처 토큰 (`amd64` | `arm64`). */
    archToken: string;
    isZip: boolean;
}

/**
 * `process.platform`/`process.arch` → 릴리스 자산. 지원하지 않는 조합이면
 * `undefined`를 반환해 호출부가 수동 설치 안내로 폴백하게 한다.
 */
export function resolvePlatformAsset(
    platform: NodeJS.Platform = process.platform,
    architecture: string = process.arch,
): PlatformAsset | undefined {
    const descriptor = describePlatform(platform, architecture);
    if (descriptor == null) {
        return undefined;
    }
    const leadingDirectory = `${descriptor.osToken}-${descriptor.archToken}`;
    const extension = descriptor.isZip ? "zip" : "tar.gz";
    return {
        assetName: `zoekt-ctags-${leadingDirectory}.${extension}`,
        archiveLeadingDirectory: leadingDirectory,
        isZip: descriptor.isZip,
    };
}

function describePlatform(platform: NodeJS.Platform, architecture: string): PlatformDescriptor | undefined {
    const archToken = mapArchitecture(architecture);
    if (archToken == null) {
        return undefined;
    }
    if (platform === "linux") {
        return { osToken: "linux", archToken, isZip: false };
    }
    if (platform === "darwin") {
        return { osToken: "macos", archToken, isZip: false };
    }
    if (platform === "win32") {
        return { osToken: "windows", archToken, isZip: true };
    }
    return undefined;
}

function mapArchitecture(architecture: string): string | undefined {
    if (architecture === "x64") {
        return "amd64";
    }
    if (architecture === "arm64") {
        return "arm64";
    }
    return undefined;
}
