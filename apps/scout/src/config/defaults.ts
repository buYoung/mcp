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
export const DEFAULT_USE_GITIGNORE = true;

// read_file (DESIGN §4.1)

/** limit 미지정 시에만 적용하는 전 파일 바이트 상한. 초과 시 절단이 아니라 throw한다. */
export const READ_FILE_BYTE_CAP = 256 * 1024; // 262144

/** 이 크기 미만은 메모리로 읽고, 이상은 스트리밍으로 읽는다(대용량 동작 재현). */
export const READ_FILE_FAST_PATH_MAX_SIZE = 10 * 1024 * 1024;

/** 스트리밍 읽기 경로의 highWaterMark(한 번에 읽는 청크 크기). */
export const READ_FILE_STREAM_HIGH_WATER_MARK = 512 * 1024;

// 확장자 분류 (DESIGN §4.1, §4.3)

/** 이미지 확장자 — read_file에서 ImageUnsupportedError로 거부한다. */
export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "gif", "webp"] as const;

/** PDF/Jupyter 문서 확장자 — read_file에서 DocumentUnsupportedError로 거부한다. */
export const UNSUPPORTED_DOCUMENT_EXTENSIONS = ["pdf", "ipynb"] as const;

/**
 * 바이너리로 간주해 거부할 확장자. 텍스트 SVG는 제외(허용)하며, 보수적으로 흔한
 * 바이너리만 모은다. 이미지도 바이너리지만 §4.3 전용 에러가 우선한다.
 */
export const BINARY_FILE_EXTENSIONS = [
    "exe",
    "dll",
    "so",
    "dylib",
    "bin",
    "class",
    "o",
    "a",
    "obj",
    "wasm",
    "zip",
    "gz",
    "tar",
    "tgz",
    "bz2",
    "xz",
    "7z",
    "rar",
    "jar",
    "war",
    "png",
    "jpg",
    "jpeg",
    "gif",
    "webp",
    "bmp",
    "ico",
    "tiff",
    "heic",
    "mp3",
    "mp4",
    "mov",
    "avi",
    "mkv",
    "wav",
    "flac",
    "ogg",
    "webm",
    "pdf",
    "doc",
    "docx",
    "xls",
    "xlsx",
    "ppt",
    "pptx",
    "woff",
    "woff2",
    "ttf",
    "otf",
    "eot",
    "psd",
    "sketch",
] as const;

/**
 * 차단할 디바이스/특수 경로 (DESIGN §4.1). 정확 일치 목록이며, `/proc/<pid>/fd/<n>`·
 * `/dev/fd/<n>` 패턴은 read-guard에서 정규식으로 별도 차단한다.
 */
export const BLOCKED_DEVICE_PATHS = [
    "/dev/zero",
    "/dev/random",
    "/dev/urandom",
    "/dev/full",
    "/dev/stdin",
    "/dev/tty",
    "/dev/console",
    "/dev/stdout",
    "/dev/stderr",
] as const;

// find_files (DESIGN §4.2)

/** find_files 결과 상한. 초과 시 오래된 순으로 이 개수만 유지하고 절단 문구를 붙인다. */
export const FIND_FILES_RESULT_LIMIT = 100;

/** find_files 결과 절단 시 마지막 줄에 붙이는 정확 문구. */
export const FIND_FILES_TRUNCATION_MESSAGE = "(Results are truncated. Consider using a more specific path or pattern.)";

// lookup_symbol (DESIGN §2.2, §3.2)

/** ctags `--fields=` 옵션 값: line·kind·scope·signature·access. */
export const CTAGS_OUTPUT_FIELDS = "+nKsSa";

/** ctags 실행 타임아웃. */
export const CTAGS_EXEC_TIMEOUT_MS = 30_000;

/** ctags 실행 출력 버퍼 상한. */
export const CTAGS_EXEC_MAX_BUFFER_BYTES = 64 * 1024 * 1024;
