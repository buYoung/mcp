export const SERVER_NAME = "code-nav";

export const SERVER_VERSION = "0.0.0";

export const ZOEKT_INDEX_BINARY = "zoekt-index";

export const ZOEKT_WEBSERVER_BINARY = "zoekt-webserver";

export const CTAGS_BINARY = "ctags";

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
