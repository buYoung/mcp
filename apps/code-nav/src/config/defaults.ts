export const SERVER_NAME = "code-nav";

export const SERVER_VERSION = "0.0.0";

export const ZOEKT_INDEX_BINARY = "zoekt-index";

export const ZOEKT_WEBSERVER_BINARY = "zoekt-webserver";

export const ZOEKT_GIT_INDEX_BINARY = "zoekt-git-index";

export const CTAGS_BINARY = "ctags";

/**
 * Universal Ctags 바이너리는 릴리스 아카이브 안에서 `universal-ctags`라는 이름으로
 * 들어 있다(설치 후 `ctags`로 rename). PATH에 `ctags` 없이 `universal-ctags`만
 * 있는 환경도 있어, 탐색 시 이 이름도 후보로 시도한다.
 */
export const CTAGS_RELEASE_BINARY = "universal-ctags";

/**
 * 사전 빌드된 zoekt + Universal Ctags 바이너리를 받는 릴리스 저장소(사용자 개인 레포).
 * 태그는 재현성·검증 안정성을 위해 특정 버전으로 고정한다(결정: 태그 고정).
 */
export const BINARY_RELEASE_REPOSITORY = "buYoung/zoetk-ctags-release";

export const BINARY_RELEASE_TAG = "v0.0.2";

export const BINARY_RELEASE_DOWNLOAD_BASE_URL = `https://github.com/${BINARY_RELEASE_REPOSITORY}/releases/download/${BINARY_RELEASE_TAG}`;

/**
 * 관리형(다운로드된) 바이너리 캐시 베이스 디렉터리 오버라이드. 설치 경로는 이 값을
 * 그대로 쓰지 않고 항상 그 아래 `code-nav/bin/<tag>` 하위를 만든다 — 설치기가 시작 시
 * 디렉터리를 통째로 비우므로(rm), 오버라이드 루트 자체를 지우지 않도록 설치기 전용
 * 하위 경로로 스코프한다. 미지정 시 `$XDG_CACHE_HOME`(없으면 `~/.cache`)가 베이스.
 */
export const ENVIRONMENT_BIN_DIRECTORY = "CODE_NAV_BIN_DIR";

export const BINARY_DOWNLOAD_TIMEOUT_MS = 180_000;

export const ARCHIVE_EXTRACT_TIMEOUT_MS = 120_000;

export const ARCHIVE_EXTRACT_MAX_BUFFER_BYTES = 8 * 1024 * 1024;

/**
 * 다운로드 아카이브 최대 크기(스트리밍 중 초과 시 중단). 자산은 ~30MB 수준이라
 * 넉넉한 상한으로 변조/오류로 인한 무제한 메모리·디스크 사용을 막는 방어선이다.
 */
export const ARCHIVE_DOWNLOAD_MAX_BYTES = 256 * 1024 * 1024;

/**
 * Directories excluded from the working-tree index. `zoekt-index` only excludes
 * `.git,.hg,.svn` by default, so the rest must be passed explicitly (DESIGN §6.5).
 */
export const EXCLUDED_DIRECTORY_NAMES = [
    ".git",
    ".hg",
    ".svn",
    ".bzr",
    ".jj",
    ".sl",
    "node_modules",
    "dist",
    "build",
    "out",
    "target",
    ".turbo",
    ".next",
    ".idea",
    ".gradle",
    ".cache",
    ".venv",
    "vendor",
] as const;

export const DEFAULT_OUTPUT_MODE = "files_with_matches" as const;

export const OUTPUT_MODES = ["content", "files_with_matches", "count"] as const;

export type OutputMode = (typeof OUTPUT_MODES)[number];

export const DEFAULT_HEAD_LIMIT = 250;

export const DEFAULT_CONTEXT_LINES = 0;

export const ENVIRONMENT_INDEX_DIRECTORY = "CODE_NAV_INDEX_DIR";

export const STALENESS_CHECK_TTL_MS = 2_000;

export const INDEX_BUILD_TIMEOUT_MS = 600_000;

export const INDEX_BUILD_MAX_BUFFER_BYTES = 64 * 1024 * 1024;

export const CTAGS_VERSION_TIMEOUT_MS = 5_000;

export const WEBSERVER_HEALTH_TIMEOUT_MS = 15_000;

export const WEBSERVER_HEALTH_POLL_INTERVAL_MS = 150;

export const SEARCH_REQUEST_TIMEOUT_MS = 15_000;
