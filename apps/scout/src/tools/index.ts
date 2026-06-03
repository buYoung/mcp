import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { DEFAULT_HEAD_LIMIT, OUTPUT_MODES } from "../config/defaults.js";
import type { TextSearchProvider } from "../providers/text-search/text-search-provider.js";

/** 바이너리가 갖춰졌으면 provider를, 아니면 누락 안내 텍스트를 한 번의 해석으로 돌려준다. */
export type SearchProviderResolution =
    | { kind: "ready"; provider: TextSearchProvider }
    | { kind: "missing"; guidance: string };

export interface ToolDependencies {
    /** search_text 한 호출당 단 한 번 바이너리를 해석한다(저하 시 중복 ctags 점검 방지). */
    resolveSearchProvider: () => Promise<SearchProviderResolution>;
    /** 사전 빌드 바이너리를 내려받아 설치하고, 에이전트에게 보여줄 결과 텍스트를 반환. */
    installBinaries: () => Promise<string>;
}

/**
 * search_text 입력 스키마(raw shape). 키는 MCP 규약대로 snake_case이며,
 * SDK가 이 zod shape로 인자를 검증하고 ListTools용 JSON Schema를 자동 생성한다.
 */
const searchTextInputSchema = {
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
};

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
}

/** zod로 검증된 search_text 인자 타입(snake_case 키). */
type SearchTextArguments = z.infer<z.ZodObject<typeof searchTextInputSchema>>;

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
