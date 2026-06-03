export const SERVER_NAME = "scout";

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

export const BINARY_RELEASE_TAG = "v0.0.3";

export const BINARY_RELEASE_DOWNLOAD_BASE_URL = `https://github.com/${BINARY_RELEASE_REPOSITORY}/releases/download/${BINARY_RELEASE_TAG}`;

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

export const STALENESS_CHECK_TTL_MS = 2_000;

export const INDEX_BUILD_TIMEOUT_MS = 600_000;

export const INDEX_BUILD_MAX_BUFFER_BYTES = 64 * 1024 * 1024;

export const CTAGS_VERSION_TIMEOUT_MS = 5_000;

export const WEBSERVER_HEALTH_TIMEOUT_MS = 15_000;

export const WEBSERVER_HEALTH_POLL_INTERVAL_MS = 150;

export const SEARCH_REQUEST_TIMEOUT_MS = 15_000;

/**
 * scout 전용 디렉터리 이름. repo의 `<repo>/.scout/`(index·repo 설정)와
 * 전역 `~/.scout/`(관리형 바이너리·전역 설정)에 공통으로 쓰여 경로를 일원화한다.
 */
export const SCOUT_DIRECTORY_NAME = ".scout";

/** scout 설정 파일 이름. repo·전역 레이어 모두 동일한 파일명을 사용한다. */
export const CONFIG_FILE_NAME = "config.toml";

/** 설정 미지정 시 검색 결과에 줄 번호를 표시할지의 기본값. */
export const DEFAULT_SHOW_LINE_NUMBERS = true;

/** 설정 미지정 시 repo `.gitignore`의 디렉터리 이름을 제외 집합에 합칠지의 기본값. */
export const DEFAULT_RESPECT_GITIGNORE = true;

/** 설정 미지정 시 `<repo>/.scout/`를 `.git/info/exclude`에 등록할지의 기본값. */
export const DEFAULT_REGISTER_GIT_EXCLUDE = true;
