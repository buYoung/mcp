import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { DEFAULT_HEAD_LIMIT, OUTPUT_MODES } from "../config/defaults.js";
import type { TextSearchProvider } from "../providers/text-search/text-search-provider.js";

/** 바이너리가 갖춰졌으면 provider를, 아니면 누락 안내 텍스트를 한 번의 해석으로 돌려준다. */
export type SearchProviderResolution =
    | { kind: "ready"; provider: TextSearchProvider }
    | { kind: "missing"; guidance: string };

/**
 * lookup_symbol은 ctags가 있어야 동작한다. ctags가 갖춰졌으면 조회 함수를,
 * 아니면 설치 안내 텍스트를 돌려준다(search_text의 저하 패턴과 동일).
 */
export type SymbolProviderResolution =
    | { kind: "ready"; lookup: (input: LookupSymbolArguments) => Promise<string> }
    | { kind: "missing"; guidance: string };

/** read_file 핸들러가 provider에 넘기는 입력(snake_case 키 그대로). */
export interface ReadFileArguments {
    file_path: string;
    offset?: number | undefined;
    limit?: number | undefined;
}

/** find_files 핸들러가 provider에 넘기는 입력. */
export interface FindFilesArguments {
    pattern: string;
    path?: string | undefined;
}

/** lookup_symbol 핸들러가 provider에 넘기는 입력(MCP snake_case 키). */
export interface LookupSymbolArguments {
    symbol_name: string;
    kind?: string | undefined;
    path?: string | undefined;
    language?: string | undefined;
    is_prefix_match?: boolean | undefined;
    head_limit?: number | undefined;
}

export interface ToolDependencies {
    /** search_text 한 호출당 단 한 번 바이너리를 해석한다(저하 시 중복 ctags 점검 방지). */
    resolveSearchProvider: () => Promise<SearchProviderResolution>;
    /** 사전 빌드 바이너리를 내려받아 설치하고, 에이전트에게 보여줄 결과 텍스트를 반환. */
    installBinaries: () => Promise<string>;
    /** 단일 파일을 cat -n 형식으로 읽는다(ReadFileProvider.read 바인딩). 바이너리 독립. */
    readFile: (input: ReadFileArguments) => Promise<string>;
    /** glob 패턴으로 파일 경로를 찾는다(FindFilesProvider.find 바인딩). 바이너리 독립. */
    findFiles: (input: FindFilesArguments) => Promise<string>;
    /** lookup_symbol 한 호출당 단 한 번 ctags를 해석한다(저하 시 안내 텍스트 반환). */
    resolveSymbolProvider: () => Promise<SymbolProviderResolution>;
}

/**
 * search_text 입력 스키마. 키는 MCP 규약대로 snake_case이며, SDK가 이 zod object로
 * 인자를 검증하고 ListTools용 JSON Schema를 자동 생성한다.
 *
 * `z.strictObject`를 쓰는 이유: SDK는 raw shape를 `z4mini.object(shape)`로 감싸는데
 * Zod v4 object는 알 수 없는 키를 거부하지 않고 조용히 제거(strip)한다(실측). 명시적
 * ZodObject(strict)를 넘기면 SDK가 그대로 보존하므로, 추가 키가 에러로 거부된다(spec §4.1).
 */
const searchTextInputSchema = z.strictObject({
    pattern: z.string().min(1).describe("검색할 정규식. zoekt RE2 문법."),
    path: z.string().optional().describe("검색 범위를 제한할 리포 내부 디렉터리. 미지정 시 리포 루트 전체."),
    glob: z.string().optional().describe("파일 필터 glob. 예: '*.{ts,tsx}'."),
    type: z.string().optional().describe("파일 타입 약칭(ts, py, go, kt 등). 내부에서 zoekt lang:/file: 로 변환."),
    output_mode: z
        .enum([...OUTPUT_MODES])
        .optional()
        .describe("출력 모드. 기본 files_with_matches."),
    case_insensitive: z.boolean().optional().describe("대소문자 무시. 기본 false."),
    show_line_numbers: z.boolean().optional().describe("content 모드에서 줄 번호 표시. 기본 true."),
    context_lines: z.number().int().min(0).optional().describe("각 매치 앞뒤 컨텍스트 줄 수(-C). content 모드만 적용."),
    context_before_lines: z
        .number()
        .int()
        .min(0)
        .optional()
        .describe("매치 앞 컨텍스트 줄 수(-B). zoekt 대칭 컨텍스트라 -A/-B/-C 중 최대값이 적용된다."),
    context_after_lines: z
        .number()
        .int()
        .min(0)
        .optional()
        .describe("매치 뒤 컨텍스트 줄 수(-A). zoekt 대칭 컨텍스트라 -A/-B/-C 중 최대값이 적용된다."),
    head_limit: z
        .number()
        .int()
        .min(0)
        .optional()
        .describe(`결과 상한. 미지정 시 ${DEFAULT_HEAD_LIMIT}, 0이면 무제한.`),
    offset: z.number().int().min(0).optional().describe("head_limit 적용 전 건너뛸 결과 수. 기본 0."),
});

/**
 * read_file 입력 스키마. Claude Code의 Read에 대응한다. `z.strictObject`로 등록해
 * 알 수 없는 키를 strip이 아니라 에러로 거부한다(spec §4.1, FileReadTool strictObject 모사).
 */
const readFileInputSchema = z.strictObject({
    file_path: z.string().min(1).describe("읽을 파일 경로. 리포 루트(cwd) 경계 안이어야 한다."),
    offset: z
        .number()
        .int()
        .min(0)
        .optional()
        .describe("읽기 시작 줄 번호(1-기반). 0과 1 모두 첫 줄을 가리킨다. 미지정 시 첫 줄부터."),
    limit: z.number().int().min(1).optional().describe("읽을 줄 수. 미지정 시 256KB 바이트 상한까지 전부 읽는다."),
});

/**
 * find_files 입력 스키마. Claude Code의 Glob에 대응한다. `z.strictObject`로 등록해
 * 알 수 없는 키를 strip이 아니라 에러로 거부한다(spec §4.1).
 */
const findFilesInputSchema = z.strictObject({
    pattern: z.string().min(1).describe("파일 경로를 매칭할 glob 패턴. 예: '**/*.{ts,tsx}'."),
    path: z.string().optional().describe("탐색 기준 디렉터리. 미지정 시 리포 루트(cwd)."),
});

/**
 * lookup_symbol 입력 스키마. 심볼 '정의(definition)' 위치만 조회한다. 호출부/사용처
 * (call-site)는 search_text가 정확하다(ctags는 선언을 잡는다). `z.strictObject`로 등록해
 * 알 수 없는 키를 strip이 아니라 에러로 거부한다(spec §4.1).
 */
const lookupSymbolInputSchema = z.strictObject({
    symbol_name: z.string().min(1).describe("찾을 심볼 이름. 기본 정확 일치."),
    kind: z.string().optional().describe("심볼 종류 필터(function/class/struct/method 등). 미지정 시 전체."),
    path: z.string().optional().describe("스코프를 제한할 리포 내부 디렉터리/파일. 미지정 시 리포 루트(cwd)."),
    language: z.string().optional().describe("언어 필터(ctags --languages= 표기, 예: TypeScript)."),
    is_prefix_match: z.boolean().optional().describe("접두 일치 허용. 기본 false(정확 일치)."),
    head_limit: z
        .number()
        .int()
        .min(0)
        .optional()
        .describe(`결과 상한. 미지정 시 ${DEFAULT_HEAD_LIMIT}, 0이면 무제한.`),
});

export function registerTools(server: McpServer, dependencies: ToolDependencies): void {
    server.registerTool(
        "install_binaries",
        {
            description:
                "필수 외부 바이너리(zoekt-index, zoekt-webserver, Universal Ctags)를 사전 빌드 릴리스에서 내려받아 설치한다. 다운로드 손상 여부는 SHA-256 체크섬으로 확인한다(전송 무결성 확인이며, 서명/출처 인증은 아니다). 네트워크에서 실행 파일을 받으므로, 호출 전 사용자에게 다운로드 동의를 구하라. search_text가 '바이너리 미설치'를 보고할 때 사용한다. 이미 설치돼 있으면 재설치(덮어쓰기)한다.",
            inputSchema: {},
        },
        async () => textResult(await dependencies.installBinaries()),
    );

    server.registerTool(
        "search_text",
        {
            description:
                "색인 기반 텍스트(정규식) 내용 검색 (zoekt 백엔드). Claude Code의 Grep에 대응하나 엔진은 ripgrep이 아닌 zoekt다. 광범위 후보, 호출부(call-site), 교차 언어 탐색에 사용한다. 심볼의 '정의' 위치만 찾으려면 (호출부가 아니라) 심볼 정의 도구를 써라 — 텍스트 검색은 호출부 탐색에 강하다.",
            inputSchema: searchTextInputSchema,
        },
        async (args) => {
            const resolved = await dependencies.resolveSearchProvider();
            if (resolved.kind === "missing") {
                return textResult(resolved.guidance);
            }
            const contextLines = resolveContextLines(args);
            const renderedText = await resolved.provider.search({
                pattern: args.pattern,
                path: args.path,
                glob: args.glob,
                type: args.type,
                outputMode: args.output_mode,
                caseInsensitive: args.case_insensitive,
                showLineNumbers: args.show_line_numbers,
                contextLines,
                headLimit: args.head_limit,
                offset: args.offset,
            });
            return textResult(renderedText);
        },
    );

    server.registerTool(
        "read_file",
        {
            description:
                "단일 파일을 cat -n 형식(줄 번호 + 내용)으로 읽는다. Claude Code의 Read에 대응한다. 검색 도구(search_text, lookup_symbol)가 돌려준 후보 위치를 실제 코드로 확인하는 '검증 단계'에 사용한다. 색인·바이너리에 의존하지 않으므로 색인 빌드 전에도 동작한다. 이미지/PDF/Jupyter 노트북은 v1에서 지원하지 않는다.",
            inputSchema: readFileInputSchema,
        },
        async (args) => {
            try {
                const rendered = await dependencies.readFile({
                    file_path: args.file_path,
                    offset: args.offset,
                    limit: args.limit,
                });
                return textResult(rendered);
            } catch (error) {
                return textResult(toErrorMessage(error));
            }
        },
    );

    server.registerTool(
        "find_files",
        {
            description:
                "glob 패턴으로 파일 경로를 찾는다. Claude Code의 Glob에 대응하며 백엔드는 JS glob 라이브러리다(zoekt·ripgrep 아님). 결과는 mtime 오래된 순으로 정렬되고 100건에서 절단된다. .gitignore는 무시하고 숨김 파일은 포함한다. 색인·바이너리에 의존하지 않는다.",
            inputSchema: findFilesInputSchema,
        },
        async (args) => {
            try {
                const rendered = await dependencies.findFiles({
                    pattern: args.pattern,
                    path: args.path,
                });
                return textResult(rendered);
            } catch (error) {
                return textResult(toErrorMessage(error));
            }
        },
    );

    server.registerTool(
        "lookup_symbol",
        {
            description:
                "심볼 '정의(definition)' 조회 (Universal Ctags 백엔드). 함수/클래스/타입 등의 선언 위치를 찾을 때만 사용한다. 호출부/사용처(call-site)나 교차 언어 사용을 찾으려면 search_text를 써라 — ctags는 호출부가 아니라 선언을 잡으므로 호출부 탐색에 부정확하다.",
            inputSchema: lookupSymbolInputSchema,
        },
        async (args) => {
            const resolved = await dependencies.resolveSymbolProvider();
            if (resolved.kind === "missing") {
                return textResult(resolved.guidance);
            }
            try {
                const rendered = await resolved.lookup({
                    symbol_name: args.symbol_name,
                    kind: args.kind,
                    path: args.path,
                    language: args.language,
                    is_prefix_match: args.is_prefix_match,
                    head_limit: args.head_limit,
                });
                return textResult(rendered);
            } catch (error) {
                return textResult(toErrorMessage(error));
            }
        },
    );
}

/** 던져진 에러를 에이전트에게 보여줄 텍스트로 변환한다(Error 메시지 우선). */
function toErrorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
}

/** zod로 검증된 search_text 인자 타입(snake_case 키). */
type SearchTextArguments = z.infer<typeof searchTextInputSchema>;

/**
 * zoekt webserver는 단일 대칭 컨텍스트 값(`ctx`)만 쓰므로,
 * Grep식 -A/-B/-C 입력은 최대값 하나로 수렴한다.
 */
function resolveContextLines(args: SearchTextArguments): number | undefined {
    const provided = [args.context_lines, args.context_before_lines, args.context_after_lines].filter(
        (value): value is number => value != null,
    );
    if (provided.length === 0) {
        return undefined;
    }
    return Math.max(...provided);
}

function textResult(text: string) {
    return {
        content: [{ type: "text" as const, text }],
    };
}
