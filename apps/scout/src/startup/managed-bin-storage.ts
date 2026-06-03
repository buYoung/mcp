import { homedir } from "node:os";
import { join } from "node:path";
import { BINARY_RELEASE_TAG, SCOUT_DIRECTORY_NAME } from "../config/defaults.js";

/**
 * 다운로드된(관리형) 바이너리의 설치 디렉터리를 해석한다. 태그별 하위 디렉터리로
 * 분리해, 핀 태그가 바뀌면 새 디렉터리에 받고 옛 버전과 섞이지 않게 한다.
 *
 * 경로는 `os.homedir()` 기준으로 고정되며 설정/env 오버라이드를 받지 않는다.
 * 설치기가 시작 시 이 디렉터리를 통째로 비우므로(rm -rf), 반환 경로는
 * **항상 `~/.scout/bin/<tag>` 소유 하위 경로**여야 한다 — 이렇게 스코프를 좁혀
 * 사용자의 다른 디렉터리를 실수로 지우는 사고를 막는다.
 */
export function resolveManagedBinDirectory(): string {
    return join(homedir(), SCOUT_DIRECTORY_NAME, "bin", BINARY_RELEASE_TAG);
}
