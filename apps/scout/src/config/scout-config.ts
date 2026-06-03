import { mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { parse as parseToml } from "smol-toml";
import {
    CONFIG_FILE_NAME,
    DEFAULT_CONTEXT_LINES,
    DEFAULT_HEAD_LIMIT,
    DEFAULT_OUTPUT_MODE,
    DEFAULT_REGISTER_GIT_EXCLUDE,
    DEFAULT_RESPECT_GITIGNORE,
    DEFAULT_SHOW_LINE_NUMBERS,
    EXCLUDED_DIRECTORY_NAMES,
    INDEX_BUILD_TIMEOUT_MS,
    OUTPUT_MODES,
    type OutputMode,
    SCOUT_DIRECTORY_NAME,
    SEARCH_REQUEST_TIMEOUT_MS,
    STALENESS_CHECK_TTL_MS,
} from "./defaults.js";

/**
 * 해석이 끝난 scout 설정. 우선순위(repo > global > default)와 타입 검증을 모두 거친
 * 최종값만 담는다. 식별자는 camelCase(provider 입력 인터페이스 규약)로 정규화한다.
 */
export interface ResolvedScoutConfig {
    output: {
        mode: OutputMode;
        headLimit: number;
        contextLines: number;
        showLineNumbers: boolean;
    };
    index: {
        excludedDirectories: readonly string[];
        stalenessCheckMs: number;
        respectGitignore: boolean;
        registerGitExclude: boolean;
    };
    limits: {
        searchRequestTimeoutMs: number;
        indexBuildTimeoutMs: number;
    };
}

/**
 * 단일 TOML 레이어(repo 또는 global)를 정규화·검증한 부분 구조. 키마다 "그 레이어가
 * 값을 명시했는가"를 표현하기 위해 모두 optional이며, 누락 키는 병합 단계에서 하위
 * 우선순위로 위임된다. 알 수 없는 키/타입 오류는 정규화 단계에서 경고 후 제거된다.
 */
interface ScoutConfigLayer {
    outputMode?: OutputMode;
    headLimit?: number;
    contextLines?: number;
    showLineNumbers?: boolean;
    excludedDirectories?: string[];
    stalenessCheckMs?: number;
    respectGitignore?: boolean;
    registerGitExclude?: boolean;
    searchRequestTimeoutMs?: number;
    indexBuildTimeoutMs?: number;
}

/** TOML `[output]` 테이블에서 인식하는 키 집합. 나머지는 알 수 없는 키로 경고+무시. */
const KNOWN_OUTPUT_KEYS = new Set(["mode", "head_limit", "context_lines", "show_line_numbers"]);

/** TOML `[index]` 테이블에서 인식하는 키 집합. */
const KNOWN_INDEX_KEYS = new Set([
    "excluded_directories",
    "staleness_check_ms",
    "respect_gitignore",
    "register_git_exclude",
]);

/** TOML `[limits]` 테이블에서 인식하는 키 집합. */
const KNOWN_LIMITS_KEYS = new Set(["search_request_timeout_ms", "index_build_timeout_ms"]);

/** 최상위에서 인식하는 테이블 이름 집합. 그 외 테이블은 알 수 없는 테이블로 경고+무시. */
const KNOWN_TABLES = new Set(["output", "index", "limits"]);

/**
 * 전역 `~/.scout/config.toml`이 없을 때 자동 생성하는 주석 처리 템플릿. 모든 값을
 * 주석 처리해 "미지정 = 기본값"이 기본 동작이 되게 한다(사용자가 필요한 줄만 해제).
 * repo `<repo>/.scout/config.toml`은 opt-in 읽기 전용이라 자동 생성하지 않는다.
 */
const DEFAULT_CONFIG_TEMPLATE = `# scout 설정
# 우선순위(키 단위): <repo>/.scout/config.toml > ~/.scout/config.toml > 기본값
# 아래 값은 모두 주석 처리되어 있다 — 줄을 해제(주석 제거)해야 적용된다.
# 미지정 키는 기본값을 사용한다.

[output]
# mode = "files_with_matches"   # "content" | "files_with_matches" | "count"
# head_limit = 250              # 정수 >= 0 (0 = 무제한)
# context_lines = 0             # 정수 >= 0
# show_line_numbers = true      # bool

[index]
# excluded_directories = [".git", "node_modules", "dist"]  # 문자열 배열(replace)
# staleness_check_ms = 2000     # 정수 > 0
# respect_gitignore = true      # bool — true면 repo .gitignore의 디렉터리 이름을 제외 집합에 합침
# register_git_exclude = true   # bool — 전역 설정에서만 적용(repo 레이어 값은 무시됨)

[limits]
# search_request_timeout_ms = 15000   # 정수 > 0
# index_build_timeout_ms = 600000     # 정수 > 0
`;

/**
 * scout 설정을 로드해 최종 `ResolvedScoutConfig`로 해석한다.
 *
 * 동작:
 * 1. 전역 디렉터리(`~/.scout`)를 보장하고, 전역 설정 파일이 없으면 주석 템플릿을 생성한다.
 * 2. repo·global 설정 파일을 읽어 파싱한다(없음/파싱 실패 → 경고 후 빈 레이어).
 * 3. 각 레이어를 정규화·검증한다(알 수 없는 키/타입 오류 → 경고 후 무시).
 * 4. 키 단위로 repo > global > default 우선순위 병합. 단 registerGitExclude는 global 전용.
 * 5. 전 과정을 try/catch로 감싸 어떤 예외도 프로세스를 죽이지 않게 한다(절대 exit/throw 안 함).
 */
export async function loadScoutConfig(repositoryRoot: string): Promise<ResolvedScoutConfig> {
    try {
        const globalDirectory = join(homedir(), SCOUT_DIRECTORY_NAME);
        const globalConfigPath = join(globalDirectory, CONFIG_FILE_NAME);
        const repoConfigPath = join(repositoryRoot, SCOUT_DIRECTORY_NAME, CONFIG_FILE_NAME);

        // 전역 디렉터리 보장 + 없을 때만 주석 템플릿 생성(repo는 자동 생성하지 않는다).
        await ensureGlobalTemplate(globalDirectory, globalConfigPath);

        const repoLayer = await readLayer(repoConfigPath);
        const globalLayer = await readLayer(globalConfigPath);

        return mergeLayers(repoLayer, globalLayer);
    } catch (error) {
        // never-exit 철학: 예기치 못한 예외도 전부 default로 흡수한다.
        warn(`설정 로드 중 예기치 못한 오류: ${describeError(error)} — 전부 기본값 사용`);
        return mergeLayers({}, {});
    }
}

/**
 * 전역 디렉터리를 보장하고, 전역 설정 파일이 없으면 주석 템플릿을 생성한다.
 * 모든 fs 오류는 경고만 남기고 흡수한다(파일이 없어도 이후 readLayer가 빈 레이어로 처리).
 */
async function ensureGlobalTemplate(globalDirectory: string, globalConfigPath: string): Promise<void> {
    try {
        await mkdir(globalDirectory, { recursive: true });
        // flag "wx": 이미 존재하면 EEXIST → 기존 사용자 설정을 덮어쓰지 않는다.
        await writeFile(globalConfigPath, DEFAULT_CONFIG_TEMPLATE, { flag: "wx" });
    } catch (error) {
        if (isFileAlreadyExistsError(error)) {
            return;
        }
        warn(`전역 설정 템플릿 생성 실패: ${globalConfigPath}: ${describeError(error)} — 무시하고 진행`);
    }
}

/**
 * 한 설정 파일을 읽어 정규화된 레이어로 변환한다.
 * - 파일 없음(ENOENT) → 빈 레이어.
 * - 파싱 실패 → 경고 후 빈 레이어.
 * - 성공 → normalizeLayer로 검증.
 */
async function readLayer(configPath: string): Promise<ScoutConfigLayer> {
    let contents: string;
    try {
        contents = await readFile(configPath, "utf8");
    } catch (error) {
        if (isFileNotFoundError(error)) {
            return {};
        }
        warn(`설정 읽기 실패: ${configPath}: ${describeError(error)} — 무시하고 기본값 사용`);
        return {};
    }

    let parsed: unknown;
    try {
        parsed = parseToml(contents);
    } catch (error) {
        warn(`설정 파싱 실패: ${configPath}: ${describeError(error)} — 무시하고 기본값 사용`);
        return {};
    }

    if (typeof parsed !== "object" || parsed == null || Array.isArray(parsed)) {
        warn(`설정 최상위가 테이블이 아님: ${configPath} — 무시하고 기본값 사용`);
        return {};
    }

    return normalizeLayer(parsed as Record<string, unknown>, configPath);
}

/**
 * 파싱된 TOML 한 레이어를 정규화·검증한다. 알 수 없는 테이블/키는 경고 후 무시하고,
 * 알려진 키의 타입 불일치도 그 키만 경고 후 무시한다(throw 금지).
 */
function normalizeLayer(parsed: Record<string, unknown>, configPath: string): ScoutConfigLayer {
    const layer: ScoutConfigLayer = {};

    for (const tableName of Object.keys(parsed)) {
        if (!KNOWN_TABLES.has(tableName)) {
            warn(`알 수 없는 설정 테이블 "[${tableName}]": ${configPath} — 무시`);
        }
    }

    normalizeOutputTable(parsed.output, configPath, layer);
    normalizeIndexTable(parsed.index, configPath, layer);
    normalizeLimitsTable(parsed.limits, configPath, layer);

    return layer;
}

/** `[output]` 테이블을 정규화한다. */
function normalizeOutputTable(table: unknown, configPath: string, layer: ScoutConfigLayer): void {
    const entries = asTable(table, "output", configPath);
    if (entries == null) {
        return;
    }

    for (const [key, value] of Object.entries(entries)) {
        if (!KNOWN_OUTPUT_KEYS.has(key)) {
            warn(`알 수 없는 설정 키 "output.${key}": ${configPath} — 무시`);
            continue;
        }
        switch (key) {
            case "mode": {
                const mode = readOutputMode(value, configPath);
                if (mode != null) {
                    layer.outputMode = mode;
                }
                break;
            }
            case "head_limit": {
                const parsedNumber = readNonNegativeInteger(value, "output.head_limit", configPath);
                if (parsedNumber != null) {
                    layer.headLimit = parsedNumber;
                }
                break;
            }
            case "context_lines": {
                const parsedNumber = readNonNegativeInteger(value, "output.context_lines", configPath);
                if (parsedNumber != null) {
                    layer.contextLines = parsedNumber;
                }
                break;
            }
            case "show_line_numbers": {
                const bool = readBoolean(value, "output.show_line_numbers", configPath);
                if (bool != null) {
                    layer.showLineNumbers = bool;
                }
                break;
            }
        }
    }
}

/** `[index]` 테이블을 정규화한다. */
function normalizeIndexTable(table: unknown, configPath: string, layer: ScoutConfigLayer): void {
    const entries = asTable(table, "index", configPath);
    if (entries == null) {
        return;
    }

    for (const [key, value] of Object.entries(entries)) {
        if (!KNOWN_INDEX_KEYS.has(key)) {
            warn(`알 수 없는 설정 키 "index.${key}": ${configPath} — 무시`);
            continue;
        }
        switch (key) {
            case "excluded_directories": {
                const directories = readStringArray(value, "index.excluded_directories", configPath);
                if (directories != null) {
                    // 배열은 per-key replace — 빈 배열도 사용자의 명시적 선택으로 허용한다.
                    layer.excludedDirectories = directories;
                }
                break;
            }
            case "staleness_check_ms": {
                const parsedNumber = readPositiveInteger(value, "index.staleness_check_ms", configPath);
                if (parsedNumber != null) {
                    layer.stalenessCheckMs = parsedNumber;
                }
                break;
            }
            case "respect_gitignore": {
                const bool = readBoolean(value, "index.respect_gitignore", configPath);
                if (bool != null) {
                    layer.respectGitignore = bool;
                }
                break;
            }
            case "register_git_exclude": {
                const bool = readBoolean(value, "index.register_git_exclude", configPath);
                if (bool != null) {
                    layer.registerGitExclude = bool;
                }
                break;
            }
        }
    }
}

/** `[limits]` 테이블을 정규화한다. */
function normalizeLimitsTable(table: unknown, configPath: string, layer: ScoutConfigLayer): void {
    const entries = asTable(table, "limits", configPath);
    if (entries == null) {
        return;
    }

    for (const [key, value] of Object.entries(entries)) {
        if (!KNOWN_LIMITS_KEYS.has(key)) {
            warn(`알 수 없는 설정 키 "limits.${key}": ${configPath} — 무시`);
            continue;
        }
        switch (key) {
            case "search_request_timeout_ms": {
                const parsedNumber = readPositiveInteger(value, "limits.search_request_timeout_ms", configPath);
                if (parsedNumber != null) {
                    layer.searchRequestTimeoutMs = parsedNumber;
                }
                break;
            }
            case "index_build_timeout_ms": {
                const parsedNumber = readPositiveInteger(value, "limits.index_build_timeout_ms", configPath);
                if (parsedNumber != null) {
                    layer.indexBuildTimeoutMs = parsedNumber;
                }
                break;
            }
        }
    }
}

/**
 * 키 단위로 repo > global > default 우선순위 병합. registerGitExclude만 global 전용이라
 * repo 레이어가 값을 주면 경고 후 무시하고 global > default만 적용한다.
 */
function mergeLayers(repoLayer: ScoutConfigLayer, globalLayer: ScoutConfigLayer): ResolvedScoutConfig {
    if (repoLayer.registerGitExclude !== undefined) {
        warn("register_git_exclude는 전역 설정에서만 적용됩니다 — repo 값 무시");
    }

    return {
        output: {
            mode: repoLayer.outputMode ?? globalLayer.outputMode ?? DEFAULT_OUTPUT_MODE,
            headLimit: repoLayer.headLimit ?? globalLayer.headLimit ?? DEFAULT_HEAD_LIMIT,
            contextLines: repoLayer.contextLines ?? globalLayer.contextLines ?? DEFAULT_CONTEXT_LINES,
            showLineNumbers: repoLayer.showLineNumbers ?? globalLayer.showLineNumbers ?? DEFAULT_SHOW_LINE_NUMBERS,
        },
        index: {
            excludedDirectories: repoLayer.excludedDirectories ??
                globalLayer.excludedDirectories ?? [...EXCLUDED_DIRECTORY_NAMES],
            stalenessCheckMs: repoLayer.stalenessCheckMs ?? globalLayer.stalenessCheckMs ?? STALENESS_CHECK_TTL_MS,
            respectGitignore: repoLayer.respectGitignore ?? globalLayer.respectGitignore ?? DEFAULT_RESPECT_GITIGNORE,
            // global 전용 키: repo 레이어 값은 위에서 경고만 남기고 여기서는 참조하지 않는다.
            registerGitExclude: globalLayer.registerGitExclude ?? DEFAULT_REGISTER_GIT_EXCLUDE,
        },
        limits: {
            searchRequestTimeoutMs:
                repoLayer.searchRequestTimeoutMs ?? globalLayer.searchRequestTimeoutMs ?? SEARCH_REQUEST_TIMEOUT_MS,
            indexBuildTimeoutMs:
                repoLayer.indexBuildTimeoutMs ?? globalLayer.indexBuildTimeoutMs ?? INDEX_BUILD_TIMEOUT_MS,
        },
    };
}

/**
 * 값이 테이블(객체)인지 확인해 그 엔트리를 반환한다. 테이블이 아니면(스칼라/배열) 경고 후
 * null을 반환해 해당 테이블 전체를 무시한다. 미지정(null/undefined)도 null로 처리한다.
 */
function asTable(value: unknown, tableName: string, configPath: string): Record<string, unknown> | null {
    if (value == null) {
        return null;
    }
    if (typeof value !== "object" || Array.isArray(value)) {
        warn(`설정 "[${tableName}]"가 테이블이 아님: ${configPath} — 무시`);
        return null;
    }
    return value as Record<string, unknown>;
}

/** output.mode 값을 OUTPUT_MODES enum으로 검증한다. 위반 시 경고 후 undefined. */
function readOutputMode(value: unknown, configPath: string): OutputMode | undefined {
    if (typeof value !== "string" || !OUTPUT_MODES.includes(value as OutputMode)) {
        warn(`output.mode는 ${OUTPUT_MODES.join(" | ")} 중 하나여야 함: ${configPath} — 무시하고 기본값 사용`);
        return undefined;
    }
    return value as OutputMode;
}

/** 0 이상의 정수(head_limit/context_lines)를 검증한다. 위반 시 경고 후 undefined. */
function readNonNegativeInteger(value: unknown, keyLabel: string, configPath: string): number | undefined {
    const numericValue = toNumber(value);
    if (numericValue == null || !Number.isSafeInteger(numericValue) || numericValue < 0) {
        warn(`${keyLabel}는 0 이상의 정수여야 함: ${configPath} — 무시하고 기본값 사용`);
        return undefined;
    }
    return numericValue;
}

/** 양의 정수(타임아웃·TTL)를 검증한다. 위반 시 경고 후 undefined. */
function readPositiveInteger(value: unknown, keyLabel: string, configPath: string): number | undefined {
    const numericValue = toNumber(value);
    if (numericValue == null || !Number.isSafeInteger(numericValue) || numericValue <= 0) {
        warn(`${keyLabel}는 양의 정수여야 함: ${configPath} — 무시하고 기본값 사용`);
        return undefined;
    }
    return numericValue;
}

/** bool 값을 검증한다. 위반 시 경고 후 undefined. */
function readBoolean(value: unknown, keyLabel: string, configPath: string): boolean | undefined {
    if (typeof value !== "boolean") {
        warn(`${keyLabel}는 true/false여야 함: ${configPath} — 무시하고 기본값 사용`);
        return undefined;
    }
    return value;
}

/**
 * 문자열 배열을 검증한다. 배열이 아니거나 문자열 외 원소가 섞이면 경고 후 undefined.
 * 빈 배열은 허용한다(사용자가 명시적으로 제외 목록을 비운 것).
 */
function readStringArray(value: unknown, keyLabel: string, configPath: string): string[] | undefined {
    if (!Array.isArray(value) || value.some((element) => typeof element !== "string")) {
        warn(`${keyLabel}는 문자열 배열이어야 함: ${configPath} — 무시하고 기본값 사용`);
        return undefined;
    }
    return value as string[];
}

/**
 * TOML 숫자 값을 number로 정규화한다. smol-toml은 큰 정수를 bigint로 줄 수 있으므로
 * 안전 범위 내 bigint도 number로 변환한다. number/bigint가 아니면 null.
 */
function toNumber(value: unknown): number | null {
    if (typeof value === "number") {
        return value;
    }
    if (typeof value === "bigint") {
        return Number(value);
    }
    return null;
}

/** ENOENT(파일 없음) 오류인지 판별한다. */
function isFileNotFoundError(error: unknown): boolean {
    return typeof error === "object" && error != null && "code" in error && error.code === "ENOENT";
}

/** EEXIST(이미 존재) 오류인지 판별한다(템플릿 생성 시 기존 파일 보존용). */
function isFileAlreadyExistsError(error: unknown): boolean {
    return typeof error === "object" && error != null && "code" in error && error.code === "EEXIST";
}

/** 오류를 사람이 읽을 메시지로 변환한다. */
function describeError(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
}

/** stderr로 한국어 경고를 남긴다. 절대 throw/exit하지 않는 never-exit 철학의 핵심. */
function warn(message: string): void {
    process.stderr.write(`[scout] ${message}\n`);
}
