import type { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { CallToolRequestSchema, ListToolsRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import { DEFAULT_HEAD_LIMIT, OUTPUT_MODES } from "../config/defaults.js";
import type { TextSearchProvider } from "../providers/text-search/text-search-provider.js";
import {
    readArguments,
    readOptionalBoolean,
    readOptionalEnum,
    readOptionalInteger,
    readOptionalString,
    readRequiredString,
} from "./arguments.js";

export interface ToolDependencies {
    textSearchProvider: TextSearchProvider;
}

export function registerTools(server: Server, dependencies: ToolDependencies): void {
    server.setRequestHandler(ListToolsRequestSchema, async () => {
        return {
            tools: [
                {
                    name: "search_text",
                    description:
                        "색인 기반 텍스트(정규식) 내용 검색 (zoekt 백엔드). Claude Code의 Grep에 대응하나 엔진은 ripgrep이 아닌 zoekt다. 광범위 후보, 호출부(call-site), 교차 언어 탐색에 사용한다. 심볼의 '정의' 위치만 찾으려면 (호출부가 아니라) 심볼 정의 도구를 써라 — 텍스트 검색은 호출부 탐색에 강하다.",
                    inputSchema: {
                        type: "object",
                        properties: {
                            pattern: {
                                type: "string",
                                description: "검색할 정규식. zoekt RE2 문법.",
                            },
                            path: {
                                type: "string",
                                description: "검색 범위를 제한할 리포 내부 디렉터리. 미지정 시 리포 루트 전체.",
                            },
                            glob: {
                                type: "string",
                                description: "파일 필터 glob. 예: '*.{ts,tsx}'.",
                            },
                            type: {
                                type: "string",
                                description: "파일 타입 약칭(ts, py, go, kt 등). 내부에서 zoekt lang:/file: 로 변환.",
                            },
                            output_mode: {
                                type: "string",
                                enum: [...OUTPUT_MODES],
                                description: "출력 모드. 기본 files_with_matches.",
                            },
                            case_insensitive: {
                                type: "boolean",
                                description: "대소문자 무시. 기본 false.",
                            },
                            show_line_numbers: {
                                type: "boolean",
                                description: "content 모드에서 줄 번호 표시. 기본 true.",
                            },
                            context_lines: {
                                type: "number",
                                description: "각 매치 앞뒤 컨텍스트 줄 수(-C). content 모드만 적용.",
                            },
                            context_before_lines: {
                                type: "number",
                                description:
                                    "매치 앞 컨텍스트 줄 수(-B). zoekt 대칭 컨텍스트라 -A/-B/-C 중 최대값이 적용된다.",
                            },
                            context_after_lines: {
                                type: "number",
                                description:
                                    "매치 뒤 컨텍스트 줄 수(-A). zoekt 대칭 컨텍스트라 -A/-B/-C 중 최대값이 적용된다.",
                            },
                            head_limit: {
                                type: "number",
                                description: `결과 상한. 미지정 시 ${DEFAULT_HEAD_LIMIT}, 0이면 무제한.`,
                            },
                            offset: {
                                type: "number",
                                description: "head_limit 적용 전 건너뛸 결과 수. 기본 0.",
                            },
                        },
                        required: ["pattern"],
                    },
                },
            ],
        };
    });

    server.setRequestHandler(CallToolRequestSchema, async (request) => {
        if (request.params.name === "search_text") {
            const argumentsValue = readArguments(request.params.arguments);
            const renderedText = await dependencies.textSearchProvider.search({
                pattern: readRequiredString(argumentsValue, "pattern"),
                path: readOptionalString(argumentsValue, "path"),
                glob: readOptionalString(argumentsValue, "glob"),
                type: readOptionalString(argumentsValue, "type"),
                outputMode: readOptionalEnum(argumentsValue, "output_mode", OUTPUT_MODES),
                caseInsensitive: readOptionalBoolean(argumentsValue, "case_insensitive"),
                showLineNumbers: readOptionalBoolean(argumentsValue, "show_line_numbers"),
                contextLines: resolveContextLines(argumentsValue),
                headLimit: readOptionalInteger(argumentsValue, "head_limit", { minimum: 0 }),
                offset: readOptionalInteger(argumentsValue, "offset", { minimum: 0 }),
            });
            return textResult(renderedText);
        }

        throw new Error(`Unknown tool: ${request.params.name}`);
    });
}

/**
 * zoekt's webserver uses a single symmetric context value (`ctx`), so the
 * Grep-style -A/-B/-C inputs collapse to their maximum.
 */
function resolveContextLines(argumentsValue: Record<string, unknown>): number | undefined {
    const contextLines = readOptionalInteger(argumentsValue, "context_lines", { minimum: 0 });
    const contextBeforeLines = readOptionalInteger(argumentsValue, "context_before_lines", { minimum: 0 });
    const contextAfterLines = readOptionalInteger(argumentsValue, "context_after_lines", { minimum: 0 });
    const provided = [contextLines, contextBeforeLines, contextAfterLines].filter(
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
