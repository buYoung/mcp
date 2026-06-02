import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { BINARY_RELEASE_TAG, ENVIRONMENT_BIN_DIRECTORY } from "../config/defaults.js";

/**
 * 다운로드된(관리형) 바이너리의 설치 디렉터리를 해석한다. 태그별 하위 디렉터리로
 * 분리해, 핀 태그가 바뀌면 새 디렉터리에 받고 옛 버전과 섞이지 않게 한다.
 *
 * **항상 베이스 아래 `code-nav/bin/<tag>`를 만든다.** 오버라이드(`CODE_NAV_BIN_DIR`)도
 * 베이스로만 취급해 그대로 쓰지 않는다 — 설치기가 시작 시 이 디렉터리를 통째로 비우므로
 * (rm -rf), 오버라이드 루트(예: `~/bin`, `/usr/local/bin`)를 그대로 지우는 사고를 막기
 * 위해 설치기 전용 하위 경로로만 스코프한다.
 */
export function resolveManagedBinDirectory(): string {
    return join(resolveCacheBaseDirectory(), "code-nav", "bin", BINARY_RELEASE_TAG);
}

function resolveCacheBaseDirectory(): string {
    const override = process.env[ENVIRONMENT_BIN_DIRECTORY];
    if (override != null && override.trim().length > 0) {
        return resolve(override.trim());
    }
    const xdgCacheHome = process.env.XDG_CACHE_HOME;
    return xdgCacheHome != null && xdgCacheHome.trim().length > 0 ? xdgCacheHome.trim() : join(homedir(), ".cache");
}
