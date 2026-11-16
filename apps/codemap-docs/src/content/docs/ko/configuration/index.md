---
title: 설정 레퍼런스
description: codemap-search의 config.toml 설정 키, auth.toml 인증정보, 클라이언트 전달 한도, 제외 규칙, Jev 단계, 이벤트 탐색의 전체 레퍼런스입니다.
---

설정은 필수가 아닙니다. 바꾸고 싶은 키만 적으면 나머지는 전역 설정이나 내장 기본값을 따릅니다. 동작 설정은 `config.toml`에, decision 모델 API 키는 `auth.toml`에 둡니다.

## 설정 위치와 우선순위

설정은 **키 단위**로 **저장소 > 전역 > 내장 기본값** 순서로 합쳐집니다. 상위 계층의 값(빈 배열 `[]` 포함)은 하위 계층 값을 덮어쓰므로, 전역 설정이나 기본값을 상속하려면 키를 지우거나 주석 처리하세요.

| 범위 | 경로 |
| --- | --- |
| 저장소 | `<repo>/.codemap/config.toml` |
| 전역 | `$CODEMAP_HOME/config.toml`, 미지정 시 `~/.codemap/config.toml` |

- **응답 바이트 한도**: **저장소 도구별 설정 > 저장소 공통값(`[output].max_bytes`) > 전역 도구별 설정 > 전역 공통값 > 기본값**(`search` `1 MiB`, `read` 및 확장 `grep` `5 MiB`) 순으로 적용되며 MCP 응답 본문만 제한합니다. 한도를 넘으면 `search`는 부분 결과와 잘림 안내를, `read`는 범위 축소 요청을, `find`, `initial_instructions`, 비확장 `grep`은 범위 축소 오류를 반환합니다.
- **`.codemap/`과 `.codemapignore`**: `.codemap/config.toml`과 `.codemap/index/`는 `.codemap/` 폴더를 함께 씁니다(`codemap-search`는 `.codemap/`을 항상 제외하므로 `git status`에서 감추려면 `.gitignore`나 `.git/info/exclude`에 직접 추가하세요). 저장소 루트의 `.codemapignore`는 gitignore 문법으로 색인, `find`, `grep` 제외 경로를 추가합니다.

### 전역 지침

`[global_instructions].enabled`의 기본값은 `false`이며 **전역 설정에서만** 읽습니다(저장소 설정은 경고 후 무시):

```toml
[global_instructions]
enabled = true
```

활성화하면 MCP 초기화 시 감지한 클라이언트의 전역 지침 파일에 영문 한 줄(`- Prefer codemap-search for code navigation when available. Call \`initial_instructions\` first and follow the returned guidance. <!-- codemap-search:managed -->`)을 추가하거나 갱신합니다. `enabled = false`는 기록된 줄만 지우고 사용자 지침은 유지하며, 새 에이전트 세션부터 반영됩니다:

| 클라이언트 이름 | 기본 파일 | 경로 변경과 기존 파일 우선순위 |
| --- | --- | --- |
| `codex-mcp-client` | `~/.codex/AGENTS.md` | `CODEX_HOME`. 비어 있지 않은 `AGENTS.override.md`가 우선 |
| `claude-code` | `~/.claude/CLAUDE.md` | `CLAUDE_CONFIG_DIR` |
| `pi` | `~/.pi/agent/AGENTS.md` | `PI_CODING_AGENT_DIR`. `AGENTS.override.md`, `AGENTS.md`, `AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD` 중 첫 번째 존재하는 파일 |
| `opencode` | `$XDG_CONFIG_HOME/opencode/AGENTS.md` (보통 `~/.config/opencode/AGENTS.md`) | `OPENCODE_CONFIG_DIR`. v1은 `AGENTS.md`가 없고 Claude 지침이 켜져 있으면 `~/.claude/CLAUDE.md` 사용, v2는 `AGENTS.md` 사용 |

### 인증정보 (`auth.toml`)

API 키는 `config.toml`이 아니라 `<repo>/.codemap/auth.toml`이나 `$CODEMAP_HOME/auth.toml`(기본 `~/.codemap/auth.toml`)에 보관하세요:

```toml
# .codemap/auth.toml — 버전 관리에서 제외하세요
[jev]
api_key = "<발급받은 TypeSafe 키>"
[openrouter]
api_key = "<발급받은 OpenRouter 키>"
```

- **키 우선순위**: 각 키는 **저장소 auth > 전역 auth > 환경 변수**(`TYPESAFE_API_KEY` 또는 `OPENROUTER_API_KEY`) 순서로 읽으며 `[output.jev].model`이 고른 제공자 키만 씁니다. OpenRouter 키는 OpenRouter Decisions API(`https://openrouter.ai/api/alpha/decisions`)에만 전송됩니다.
- **자동 생성과 제외**: `[update].config_auto_update = true`일 때 시작 시 누락된 저장소 `auth.toml`을 Unix `0600` 권한으로 생성합니다. 모든 `auth.toml`은 색인, `search`, 기본 `find`/`grep`에서 제외됩니다(Git 제외는 직접 설정하세요).

### 클라이언트 전달 한도

`[output.client]`는 마스킹, Jev 필터링, 중복 제거가 끝난 최종 MCP 응답을 UTF-8 바이트 기준으로 제한합니다. 네 키 모두 기본값이 `100000`입니다(`claude_max_result_chars`는 문자 수를 UTF-8 바이트 상한으로, `codex_output_token_limit`은 `floor(토큰 수 × 3.5)`로 계산하며 `pi_max_bytes`와 `opencode_max_bytes`는 바이트 단위). 한도를 넘으면 `search`는 낮은 우선순위 본문을 `read` 범위 안내로 바꾸고, 일괄 `read`(`ranges`)는 항목 단위로 미루며, 단일 `read`나 기타 도구는 범위 축소 오류를 반환합니다.

- **Claude Code (`[output.client].claude_max_result_chars`, `1~500000`)**: 도구 메타데이터로 전달되므로 변경 후 MCP를 재연결하세요([Claude Code 공식 문서](https://code.claude.com/docs/en/mcp#raise-the-limit-for-a-specific-tool)).
- **Codex (`[output.client].codex_output_token_limit`)**: `codemap-search codex-config`(`--server-name 이름` 지정 가능) 출력을 `~/.codex/config.toml`에 합치세요. Code Mode 클라이언트에는 첫 줄 `// @exec: {"max_output_tokens": N}`과 `wait` 예산 안내를 함께 보내며 Codex 자체 `tool_output_token_limit`도 계속 적용됩니다([Codex MCP 공식 문서](https://learn.chatgpt.com/docs/extend/mcp#other-configuration-options)).
- **`pi` (`[output.client].pi_max_bytes`)**: [@buyong/pi-mcp-adapter](https://www.npmjs.com/package/@buyong/pi-mcp-adapter)가 자동 적용하며 변경 후 MCP 재연결이 필요합니다(`pi` 내장 MCP는 메타데이터를 무시하고 20 KB에서 절단).
- **`opencode` (`[output.client].opencode_max_bytes`)**: OpenCode는 서버 메타데이터를 읽지 않으며 기본 [`tool_output.max_bytes`](https://opencode.ai/v2/docs/config)가 `51200`이므로 `opencode.json`을 서버 값과 맞추세요.

## 설정 읽기와 자동 작성

TOML 문법 오류가 있으면 해당 파일 계층만 건너뛰고 `stderr`에 경고하며, 잘못된 키는 키 단위로 경고 후 하위 계층 값으로 대체합니다(`저장소 > 전역 > 기본값`). `[update].config_auto_update = true`(기본값)이면 `mcp` 시작 시 빠진 저장소 `config.toml`과 `auth.toml`을 생성하고, 기존 저장소 설정을 `# codemap-config-version: 32`로 동기화해 새 키만 주석 예시로 추가합니다(명시한 값, `[]` 목록, 직접 편집한 `excluded_directories`는 유지).

전역 파일은 자동으로 만들거나 고치지 않습니다. `[update].config_auto_update = false`는 템플릿 생성과 자동 쓰기만 끄며 설정 읽기와 실시간 재로드는 유지합니다. `config_auto_update = false` 상태에서 이전 설정을 수동으로 올릴 때는 유지할 기본 제외 폴더(`node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor`)를 `[index.exclude].excluded_directories`에 함께 넣고 표식을 `# codemap-config-version: 32`로 갱신하세요.

## 디렉터리 제외 규칙

`[index.exclude].excluded_directories`는 **선택적 디렉터리 제외 전체 목록**입니다. 명시한 배열은 기본값을 대체하며, `[]`는 선택 제외를 끄고, 생략하면 전역 설정이나 기본 대체 목록(`.idea`, `.vscode`, `.vs`, `node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor`)을 상속합니다.

```toml
# 편집 가능한 기본값 예시(VCS 및 .codemap 폴더는 항상 제외됨)
[index.exclude]
excluded_directories = [".idea", ".vscode", ".vs", "node_modules", ".yarn", "target", "dist", "build", "vendor"]
```

- **필수 제외 폴더**: VCS 폴더(`.git`, `.svn`, `.hg`, `.bzr`, `.jj`, `.sl`), `.codemap`, `.codemap-index`, `[index].path`는 목록에서 빼거나 `include_ignored: true`를 주어도 항상 제외됩니다.
- **최초 자동 생성**: 누락된 저장소 `config.toml`을 처음 만들 때 프로젝트 식별 파일을 확인해 `**/node_modules`, `**/target`, `**/.venv` 같은 재귀 glob을 추가합니다(같은 이름의 소스 폴더가 있으면 좁은 경로로 바꾸고 사용자 지정 빌드 경로는 직접 추가).

| 규칙 | 의미 |
| --- | --- |
| `build` | 모든 깊이의 해당 폴더(`[filesystem_permissions].allowed_roots` 외부 트리에도 적용) |
| `**/build` | 작업공간 루트를 포함한 모든 깊이의 해당 폴더 |
| `./build` | 작업공간 루트의 해당 폴더만 |
| `apps/web/build` | 해당 폴더와 그 하위 전체 |
| `apps/api/**/__pycache__` | 해당 프로젝트 하위 모든 깊이의 캐시 폴더 |
| `apps/native/cmake-build-*` | 해당 프로젝트의 패턴과 일치하는 CMake 출력 폴더 |

규칙은 대소문자를 구분하는 작업공간 상대 디렉터리 glob입니다(Windows 구분자는 `/`로 정규화하며 `..`, 빈 문자열, 절대 경로가 있으면 배열 전체를 거부합니다). 색인, CLI `codemap`, 호출자 스캔, 감시, `find`, `grep`에 적용되며, 직접 `read`는 제외 규칙을 건너뛰고 자동 문맥에만 `[output.context.exclude]`를 적용합니다.

- **중첩 worktree와 서브모듈**: 작업공간 안의 git 연결 worktree는 색인과 감시에서만 건너뛰고 서브모듈은 색인하며, `find`, `grep`, CLI `codemap`은 둘 다 탐색합니다.

## 변경 적용 시점

MCP는 시작 시 존재하는 설정 디렉터리의 `config.toml`과 `auth.toml`을 감시하다가 약 1000ms 뒤 다시 읽습니다(CLI는 호출마다 설정을 읽음).

- **전체 색인 갱신**: `[index.exclude]`, `[output.context.exclude]`, `[index.language_support].*`, `[output.macro_expansion].*`을 바꾸면 전체 색인을 다시 만듭니다.
- **다음 요청 또는 갱신 주기**: `[output].*`, `[output.redact].*`, `[filesystem_permissions].*`, `[analysis].target_os`, `[index].max_file_bytes`, `[index.refresh].index_staleness_ms`, `[index.refresh].indexer_auto_restart`, `auth.toml`은 재로드 후 다음 요청이나 갱신 주기부터 적용되며 `[index].store_references`는 이후 파싱되는 파일부터 반영됩니다.
- **재시작, 재연결, 새 세션**: `[index].path`, `[index].mode`, `[index].auto_max_files`, `[index.refresh].watch`, `[index.refresh].watch_debounce_ms`와 새로 만든 설정 디렉터리는 서버 재시작이 필요합니다. `[global_instructions].enabled`는 새 에이전트 세션부터 반영되고, `[output.client].*`는 MCP 재연결, `codex-config`, `opencode.json` 동기화가 필요하며, `[update].config_auto_update`는 다음 MCP 시작 시 적용됩니다.

## 설정 키

숫자 키는 `1` 이상의 정수만 받으며 `[output.grep].max_columns`만 `0`을 허용합니다. 바이트 크기 키는 정수 바이트나 `b`, `kb`, `mb`, `gb` 접미사가 붙은 따옴표 문자열을 받습니다(1024 거듭제곱 기준: `"50mb"` = `52428800`; 따옴표 없는 `50mb`는 오류).

| 키 | 자료형 | 기본값 | 설명 |
| --- | --- | --- | --- |
| `[global_instructions].enabled` | bool | `false` | 전역 전용 클라이언트 지침 한 줄 관리. `false`면 기록된 줄 제거 |
| `[output].is_redact_enabled` | bool | `true` | MCP 응답의 인증정보와 선택한 PII 마스킹. 로컬 색인과 검색 일치는 원문 유지 |
| `[output].max_bytes` | 정수 바이트 또는 크기 문자열 | 미지정 | 공통 MCP 응답 바이트 상한. 같은 계층의 도구별 설정이 우선 |
| `[output.client].claude_max_result_chars` | 정수(문자), 1~500000 | `100000` | Claude `tools/list` 메타데이터와 최종 전달 한도(바이트 기준). 재연결 필요 |
| `[output.client].codex_output_token_limit` | 양의 정수(토큰) | `100000` | Codex 설정 내보내기 및 최종 전달 한도(토큰당 3.5바이트) |
| `[output.client].pi_max_bytes` | 정수 바이트 또는 크기 문자열 | `100000` | `pi` `tools/list` 메타데이터와 최종 전달 한도. 재연결 필요 |
| `[output.client].opencode_max_bytes` | 정수 바이트 또는 크기 문자열 | `100000` | `opencode` 최종 전달 한도 |
| `[output.search].detail_file_limit` | 정수 | `24` | 상세 결과를 렌더링할 상위 파일 수 |
| `[output.search].overview_file_limit` | 정수 | `80` | 간략 순위 목록에 표시할 최대 파일 헤더 수 |
| `[output.search].snippet_max_lines` | 정수 | `500` | 상세 보기에서 심볼당 최대 발췌 줄 수 |
| `[output.search].symbol_limit` | 정수 | `100` | 상세 보기에서 파일당 최대 심볼 수(경로나 docstring만 맞은 JSON/YAML/TOML의 미일치 키는 개수만 표시) |
| `[output.search].max_bytes` | 정수 바이트 또는 크기 문자열 | `"1mb"` (`1048576`) | 부분 출력 안내를 포함한 `search` 응답 전체 크기 제한 |
| `[output.search].literal_max_chars` | 정수(문자) | `1200` | 일치한 리터럴의 말줄임표 절단 전 최대 길이 |
| `[output.search].literal_limit` | 정수 | `60` | 상세 보기에서 파일당 최대 리터럴 표시 수 |
| `[output.search].anchor_snippet_limit` | 정수 | `20` | 파일당 전체 발췌 최대 심볼 수. 나머지는 최대 3줄 선언으로 축약 |
| `[output.read].max_bytes` | 정수 바이트 또는 크기 문자열 | `"5mb"` (`5242880`) | `read`와 함수 본문 확장 `grep`의 출력 한도(`read` 초과는 오류, 확장 `grep`은 페이지 분할) |
| `[output.read].is_outside_callers_enabled` | bool | `true` | `read view=source` 끝에 창 밖 호출자, 같은 키 사용처, import 파일을 덧붙임. `false`면 셋 다 생략 |
| `[output.grep].max_columns` | 정수 | `0` | `grep` 본문 모드 줄 길이 제한(`0`이면 해제). 초과 시 `[Omitted long matching line]`으로 대체 |
| `[output.grep].max_bytes` | 정수 바이트 또는 크기 문자열 | `[output].max_bytes` 상속 | `grep` 응답 상한. 함수 본문 확장 시 `[output.read].max_bytes`로 대체 가능 |
| `[output.context].is_enabled` | bool | `true` | `search` 호출에서 `caller_context` 생략 시 기본 호출 관계 표시 여부 |
| `[output.context].caller_limit` | 정수 | `1000` | 심볼당 최대 호출자 또는 비호출 참조 수 |
| `[output.context].callee_limit` | 정수 | `1000` | 심볼당 최대 호출 대상 수 |
| `[output.context].max_bytes` | 정수 바이트 또는 크기 문자열 | `"128kb"` (`131072`) | `[output.search].max_bytes` 내부의 호출 관계 주석 서브 예산 |
| `[output.context].common_name_threshold` | 정수 | `2` | 같은 이름 정의가 이 수 이상이면 이름 기반 관계에 모호함 표시 |
| `[output.context].caller_omit_def_threshold` | 정수 | `5` | 같은 이름 정의가 이 수 이상이면 추정 호출자 목록을 생략하고 `grep` 제안(호출 대상은 유지) |
| `[output.navigation].is_enabled` | bool | `false` | 소스 구조를 검사해 확인된 호출 대상을 `precise`로 표시 |
| `[output.navigation].callsite_budget` | 정수 | `1000` | 이름 기반 스캔으로 넘어가기 전 구조적으로 검사할 최대 호출 위치 수 |
| `[output.navigation].scan_limit` | 정수 | `16000` | 이름 간에 공유하는 총 호출자 스캔 적중 상한(이름당 최소 25건) |
| `[output.redact].pii_entities` | 문자열 배열 | `[]` | 마스킹할 대소문자 구분 PII 유형 이름. [지원 목록](/ko/configuration/pii-redaction/) 참고 |
| `[output.redact].sensitive_fields` | 문자열 배열 | `[]` | 대소문자와 구분자를 정규화한 뒤 정확히 매칭할 추가 민감 필드명 |
| `[output.redact].rules` | 인라인 테이블 배열 | `[]` | `id`(`custom.*`)와 `pattern`을 담은 사용자 정규식 규칙 |
| `[output.redact].exceptions` | 인라인 테이블 배열 | `[]` | 마스킹에서 제외할 정확한 `{ rule_id, value }` 쌍 |
| `[index].mode` | `"auto"`, `"on"`, `"off"` | `"auto"` | MCP 서버가 작업공간을 색인하는 시점. [색인 시작 시점](#색인-시작-시점) 참고 |
| `[index].auto_max_files` | 정수 | `50000` | `auto` 모드에서 허용하는 최대 색인 가능 파일 수 |
| `[index].path` | 문자열 | `".codemap/index"` | 색인 디렉터리(절대 경로 또는 작업공간 기준 상대 경로) |
| `[index].max_file_bytes` | 정수 바이트 또는 크기 문자열 | `"1mb"` (`1048576`) | 파싱과 색인 전에 건너뛸 파일 크기 기준 |
| `[index].store_references` | bool | `false` | 함수 호출 외 참조 위치도 색인에 저장 |
| `[index.refresh].watch` | bool | `true` | 백그라운드 파일시스템 감시 활성화 |
| `[index.refresh].watch_debounce_ms` | 정수(ms) | `500` | 감시 이벤트 묶음 처리 대기 시간 |
| `[index.refresh].index_staleness_ms` | 정수(ms) | `5000` | 요청 기반 대체 갱신 간격 |
| `[index.refresh].indexer_auto_restart` | bool | `true` | 백그라운드 색인 스레드 중단 시 자동 복구 |
| `[index.language_support].is_document_support_enabled` | bool | `false` | `.md`, `.mdx` 파일 색인 포함 |
| `[index.language_support].is_shell_support_enabled` | bool | `false` | `.sh`, `.bash`, `.zsh` 스크립트 색인 포함 |
| `[index.language_support].is_infrastructure_support_enabled` | bool | `false` | HCL/Terraform, Dockerfile, Nix 파일 색인 포함 |
| `[index.language_support].is_interface_support_enabled` | bool | `false` | Protocol Buffers, GraphQL 정의 색인 포함 |
| `[index.language_support].is_build_support_enabled` | bool | `false` | Make, CMake, Starlark/Bazel 빌드 파일 색인 포함 |
| `[index.exclude].excluded_directories` | 문자열 배열(상대 디렉터리 glob) | 기본 대체 목록; [디렉터리 제외 규칙](#디렉터리-제외-규칙) 참고 | 선택적 디렉터리 제외 전체 목록 |
| `[index.exclude].use_git_exclude` | bool | `true` | `.git/info/exclude` 적용 여부(`.gitignore`와 무관) |
| `[output.context.exclude].should_include_test_code` | bool | `false` | 자동 심볼 및 호출 문맥에 테스트 코드 포함 여부 |
| `[output.context.exclude].test_file_patterns` | 문자열 배열 | [테스트 코드 문맥](#테스트-코드-문맥) 참고 | 테스트 파일 glob(`[]`는 경로 판별 해제) |
| `[output.context.exclude].test_attributes` | 언어 → 문자열 배열 | [테스트 코드 문맥](#테스트-코드-문맥) 참고 | 언어별 속성 및 어노테이션 패턴(언어 단위로 하위 목록 대체) |
| `[output.context.exclude].test_decorators` | 언어 → 문자열 배열 | [테스트 코드 문맥](#테스트-코드-문맥) 참고 | 언어별 데코레이터 패턴(`[]`는 해당 언어 해제) |
| `[output.context.exclude].test_calls` | 언어 → 문자열 배열 | [테스트 코드 문맥](#테스트-코드-문맥) 참고 | 언어별 테스트 호출 패턴(`[]`는 해당 언어 해제) |
| `[output.jev].enabled` | bool | `false` | Jev 필터링 전체 스위치(`false`면 모든 도구를 로컬로 유지) |
| `[output.jev].scope` | 문자열 배열 | `["search", "read", "grep"]` | 활성화 시 필터링할 전체 도구 목록(`[]`는 선택 없음) |
| `[output.jev].model` | 문자열 | `"jev-1.14.0"` | 고정 제공자 모델(버전만 적으면 TypeSafe, `author/slug`는 [OpenRouter](#openrouter-decision-모델), 별칭은 실패) |
| `[output.jev].timeout_ms` | 양의 정수(ms), 최대 7일 | `45000` | 준비와 대기 시간을 포함한 호출당 절대 마감 시간 |
| `[output.jev].max_in_flight_requests` | 정수, 1~8 | `8` | 평가당 최대 동시 HTTP 요청 수 |
| `[output.jev].request_spacing_ms` | 정수(ms), 100 이상 | `100` | 요청 시작 간 최소 간격 |
| `[output.jev].max_batch_bytes` | 정수 바이트 또는 크기 문자열, 1~168000 | `168000` | 배치당 인코딩 페이로드 상한(단일 질문 초과 시 실패) |
| `[output.jev].pool_idle_timeout_ms` | 양의 정수(ms), 최대 7일 | `30000` | 유휴 HTTPS 연결 유지 시간 |
| `[output.jev].search_filter_min_unrelated_probability` | 유한한 수, `0.5 < 값 <= 1.0` | `0.70` (잠정값) | 작업과 무관하다고("no") 판정해 본문을 생략하는 최소 확률(관련 있음 판정에도 대칭 적용하며, 그 사이는 불확실로 보고 본문 유지) |
| `[filesystem_permissions].find` | 문자열 | `"anywhere"` | `find` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].grep` | 문자열 | `"anywhere"` | `grep` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].read` | 문자열 | `"anywhere"` | `read` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].allowed_roots` | 문자열 배열 | `[]` | 정책이 `allowed_roots`인 도구에 허용할 외부 루트 경로 |
| `[related_terms].enabled` | bool | `false` | `synonyms` 도구 활성화(`false`일 때는 다운로드 및 준비 없음) |
| `[related_terms].dataset_path` | 문자열 | 미지정 | 다운로드 대신 사용할 사용자 소유 유의어 데이터셋 경로 |
| `[update].config_auto_update` | bool | `true` | `mcp` 시작 시 빠진 저장소 `config.toml`/`auth.toml` 생성 및 주석 동기화 |

### 색인 시작 시점

- **`"auto"` (기본값)**: 첫 `search`, `read`, `grep`, `synonyms`, 모노레포 `initial_instructions` 호출 시 저장소(`.git`, `.hg`, `.jj`) 안에 있고 대상 파일이 `[index].auto_max_files` 이하일 때만 색인을 시작합니다. 다중 저장소 폴더나 초과 저장소에서는 `search`가 `Index unavailable:`을 반환하고 실시간 `read`, `grep`, `find`만 동작하므로, 초과 저장소는 `mode = "on"`으로 설정하고 재시작하세요.
- **`"on"` / `"off"`**: `"on"`은 서버 시작 시 즉시 색인하고, `"off"`는 색인하지 않습니다. 색인이 없는 새 git worktree는 같은 저장소의 최신 호환 색인을 복사한 뒤 바뀐 파일만 다시 추출합니다.
- **작업공간 루트 제한**: 홈 디렉터리(`~`) 자체는 작업공간으로 쓸 수 없습니다.
- **제외 파일과 언어 범위**: `auth.toml`, `.txt`, lockfile, `*.map`, 압축·번들 파일([지원 언어](/ko/usage/languages/) 참고), 비UTF-8 파일, `[index].max_file_bytes` 초과 파일, `.gitignore`·전역 gitignore·`.codemapignore` 일치 경로는 색인에서 빠집니다(`find`, `grep`은 `include_ignored: true`로 조회 가능하며, `[index.exclude].use_git_exclude = false`는 `.git/info/exclude`만 끕니다). `[index.language_support]` 스위치는 색인과 CLI `codemap`에만 영향을 줍니다.
- **감시와 갱신**: `[index.refresh].watch = true`면 파일 변경을 백그라운드에서 자동 반영하고, `false`면 `search` 요청 시 `[index.refresh].index_staleness_ms` 주기로 갱신합니다.

### 매크로 확장

C, C++, 어셈블리 파일은 설치된 `clang`과 `nasm`으로 기본 전처리합니다. `compilation_database`를 생략하면 소스 폴더에서 작업공간 루트까지 올라가며 `compile_commands.json`이나 `compile_flags.txt`를 찾고, 도구가 없거나 실패하면 원래 선언을 유지합니다(`read`와 `grep`은 항상 원래 소스를 보여 줌):

```toml
[output.macro_expansion]
is_enabled = true
compilation_database = "build/compile_commands.json"
clang_path = "clang"
nasm_path = "nasm"
clang_flags = ["-Iinclude", "-DFEATURE=1"]
nasm_flags = ["-Iinclude/", "-felf64"]
timeout_ms = 5000
max_output_bytes = "8mb"
```

### 민감값 마스킹

`[output].is_redact_enabled = true`일 때 MCP 응답에서 탐지한 인증정보와 선택한 PII를 `[REDACTED]`(짧은 값은 별표)로 가리며 디스크 파일, 로컬 색인, 검색 순위, 일반 CLI 출력은 건드리지 않습니다. 내장 규칙은 `field.sensitive`(`API_KEY`, `access_token`, `client_secret`, `password`), `token.*`(`aws-access-key`, `github`, `openai`, `google`, `slack`, `jwt`, `stripe`, `gitlab`, `npm`, `sendgrid`), `credential.*`(`authorization`, `bearer`, `url-password`), `private-key.pem`을 포함하며 PII 유형은 기본 꺼짐입니다(전체 90종은 [PII 마스킹](/ko/configuration/pii-redaction/) 참고).

```toml
[output.redact]
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD"]
sensitive_fields = ["internalCredential"] # 정규화 후 정확 일치(internal_credential 포함)
rules = [{ id = "custom.acme", pattern = 'ACME_(?P<secret>[A-Z0-9]+)' }]
exceptions = [{ rule_id = "custom.acme", value = "ACME_EXAMPLE" }]
```

- 사용자 정규식 `id`는 `custom.`으로 시작해야 하며 Rust `regex` 문법을 따릅니다(`(?P<secret>...)`는 해당 캡처만 마스킹).
- `exceptions`는 `rule_id`와 따옴표를 뺀 전체 탐지 값이 일치할 때만 제외되며, 각 목록은 하위 계층을 대체합니다(`[]`는 비움).
- 검색 결과가 없을 때는 검증되지 않은 비밀이 되돌아가지 않도록 응답에서 질의 문자열을 생략합니다.

### 출력과 호출 관계

- `search` 응답이 32 KiB를 넘으면 상위 파일마다 일치 사유, `read` 범위, 일치 선언, 최대 2줄의 일치 행, 연결 위치 위주로 압축합니다. 생성 파일과 번역 리소스는 직접 지정하지 않는 한 순위가 낮아집니다.
- `limit`과 `expand="callable"`이 없는 전체 파일 `read`는 256 KiB, 실시간 `read`/`grep` 문맥은 16 KiB 상한이 적용됩니다.
- 호출 관계는 기본이 이름 기반 추정입니다. `[output.navigation].is_enabled = true`를 켜면 단일 확인 대상을 `precise`(`name — file:line`)로 표시합니다. `read`와 `grep`에서는 `[index].store_references = false`여도 `references (same-file constants, approximate)` 미리보기(최대 240자)를 함께 보여 줍니다.
- `[output.context].common_name_threshold`는 모호함 경고를 추가하고, `[output.context].caller_omit_def_threshold`는 추정 호출자 대신 `grep` 안내를 표시합니다(호출 대상과 `precise` 호출자는 유지).

### 테스트 코드 문맥

`[output.context.exclude].should_include_test_code = false`(기본값)일 때 테스트 파일과 테스트 영역은 자동 문맥, 호출 관계, 정의 개수 집계, 이벤트 분석에서 빠지지만 직접 검색과 읽기는 동작합니다. 규칙은 OR로 결합되며 내부 중첩 헬퍼까지 적용됩니다. `test_file_patterns`와 아래 언어별 목록은 지정 시 하위 계층을 **대체**합니다(`[]`는 해제, 생략 또는 `{}`는 상속). 기본 목록:

```toml
[output.context.exclude]
should_include_test_code = false
test_file_patterns = ["**/tests/**", "**/test/**", "**/__tests__/**", "test_*.py", "*_test.*", "*.test.*", "*_spec.*", "*.spec.*", "*Test.java", "*Tests.java", "*IT.java"]
[output.context.exclude.test_attributes] # #[...], @ 제외
rust = ["test", "tokio::test", "async_std::test", "rstest", "rstest::rstest", "cfg(test)"]
java = ["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Nested", "BeforeEach", "AfterEach", "BeforeAll", "AfterAll"]
kotlin = ["Test", "ParameterizedTest", "RepeatedTest", "BeforeTest", "AfterTest", "BeforeEach", "AfterEach"]
csharp = ["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestFixture", "SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown"]
swift = ["Test", "Suite"]
php = ["Test"]
[output.context.exclude.test_decorators] # @ 제외
python = ["pytest.fixture", "pytest.mark.*", "unittest.skip", "unittest.skipIf", "unittest.skipUnless", "unittest.expectedFailure"]
[output.context.exclude.test_calls]
javascript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
typescript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
dart = ["test", "group", "testWidgets"]
ruby = ["describe", "context", "it", "specify"]
powershell = ["Describe", "Context", "It"]
```

### 파일시스템 권한

`find`, `grep`, `read`에 `"workspace"`, `"allowed_roots"`(작업공간과 비어 있지 않은 `[filesystem_permissions].allowed_roots` 허용, 상대 경로 및 심볼릭 링크 실제 경로 확인), `"anywhere"`(기본값)를 지정할 수 있습니다. `allowed_roots`가 외부 트리를 주 색인에 합치지는 않으며, 외부 저장소를 `search`의 `workspace_scope`로 색인하려면 `read`가 해당 저장소 폴더 전체를 허용해야 합니다.

## `config.toml` 예시

자주 바꾸는 `<repo>/.codemap/config.toml` 설정 예시입니다:

```toml
# codemap-config-version: 32
[output.search]
max_bytes = "1mb"

[output.redact]
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD"]
sensitive_fields = ["internalCredential"]

[output.jev]
enabled = false
scope = ["search", "read", "grep"]
model = "jev-1.14.0"

[index]
mode = "auto" # "auto", "on", "off" (변경 후 재시작 필요)

[index.exclude]
excluded_directories = [".git", ".codemap", ".codemap-index", "**/node_modules", "**/target"]
```

자주 쓰는 `[filesystem_permissions]` 프리셋(`find`, `grep`, `read` 기본값 `"anywhere"`):

```toml
# 1. 작업공간 내부만 허용
[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"

# 2. find/grep은 지정한 외부 루트 허용, read는 작업공간 제한
[filesystem_permissions]
find = "allowed_roots"
grep = "allowed_roots"
read = "workspace"
allowed_roots = ["/shared/source", "../vendor-src"]

# 3. find만 전체 디스크 허용
[filesystem_permissions]
find = "anywhere"
grep = "workspace"
read = "workspace"
```

## read·grep 요청별 출력 제어

여러 구간을 한 번에 읽으려면 최상위 `file_path` 대신 `read`에 `ranges: [{file_path, offset?, limit?}, ...]`(1~20개)를 넘기세요. 최상위 `view`, `expand`, `unresolved`, `include_events`, `debug`, `include_seen`이 전체 묶음에 적용되며(`offset` 기본 `1`, `limit` 기본 EOF), 항목 오류는 인라인으로 표시되고 바이트 한도에 걸리면 미룬 항목을 안내합니다.

| 인자 | 기본값 | 결과 |
| --- | --- | --- |
| `view` | `"full"` | `full`: 심볼과 원문, `source`: 실시간 원문만 출력, `source_grouped`: 본문 모드 `grep` 전용([파일별 묶음 출력](#검색-표시-압축과-grep-원문-묶음)), `definitions`: 선언만 출력, `relations`: 원문 없이 식별자와 호출 및 상수 참조만 출력 |
| `unresolved` | `"list"` | `count`는 미해결 총합만 출력, `list`는 제한된 이름 목록 출력(`full`/`relations`) |
| `expand` | 본문 모드 `grep`: `"callable"`, `read`: `"none"` | `callable`은 감싸는 이름 있는 함수 경계까지 확장(`min(max_file_bytes, 4 MiB)`), `none`은 일치 행이나 요청 범위 유지 |

본문 모드 `grep`은 모든 보기에서 `expand="callable"`이 기본이며 고유 함수 묶음 단위로 페이지를 나눕니다(`head_limit=0`은 묶음 수 제한만 해제). 일치 행과 `-A/-B/-C` 문맥이 필요하거나 한도를 넘는 큰 함수를 좁히려면 `expand="none"`을 쓰세요.

## Rust 분석 대상 지정

```toml
[analysis]
target_os = "macos"
```

`target_os`는 호스트 OS에서 추론하지 않으며 `""`로 전역값을 지울 수 있습니다. `#[cfg(...)]`의 `target_os`, `all(...)`, `any(...)`, `not(...)` 조건을 평가하며(`cfg(test)`는 `[output.context.exclude]`에 위임) 재로드 후 재색인 없이 다음 요청부터 반영됩니다.

## 선택적 Jev 판단 단계

Jev 필터링은 decision 모델(TypeSafe Jev 또는 OpenRouter)로 `search`, `read`, `grep` 응답에서 작업과 무관한 함수 본문을 생략합니다:

```toml
[output.jev]
enabled = true
scope = ["search", "read", "grep"]
model = "jev-1.14.0"
search_filter_min_unrelated_probability = 0.70
```

API 키는 [`auth.toml`](#인증정보-authtoml)이나 `TYPESAFE_API_KEY` / `OPENROUTER_API_KEY` 환경 변수에 설정합니다.

- **적용 범위와 `task`**: `[output.jev].enabled = true`일 때 `[output.jev].scope`(`search`, `read`, `grep`; `[]`는 미적용)에 넣은 도구만 필터링하며 `initial_instructions`, `find`, `analyze`, CLI는 로컬에서 동작합니다. `scope`가 비어 있지 않으면 `initial_instructions` 호출 시 사용자 요청 원문을 `task`(최대 65,536바이트)로 넘기고 작업이 바뀔 때마다 갱신하세요(`task` 누락 시 오류).
- **보존, 개인정보, 대체**: 마스킹된 `task`와 코드 단위를 HTTPS로 보냅니다. 불확실하거나 일부만 잘리거나 큰 본문은 남기며, 생략 시 `_omitted body:`, `_omitted matches:`, `_omitted lines:`와 정확한 `read` 좌표를 남깁니다(해당 좌표나 단일 함수 내부를 읽으면 재판정 없이 반환). 생략 안내는 [읽기 동작 지표](/ko/usage/cli-and-analysis/#횟수와-바이트-해석)의 전달 원문에 포함되지 않으며, 키 누락이나 제공자 오류 시 일반 미필터 출력으로 대체됩니다.

### OpenRouter decision 모델

`[output.jev].model`에 `author/slug`(예: `model = "openai/gpt-6-luna-decisions"`)를 넣으면 `[openrouter].api_key`(`OPENROUTER_API_KEY` 대체)를 써서 `POST https://openrouter.ai/api/alpha/decisions`로만 전송합니다.

고정된 decision 모델 ID만 허용되며(`~author/slug` 별칭과 비decision 모델은 HTTP 400으로 거부되어 일반 출력으로 복귀), 약 32 KB 초과 요청은 OpenRouter에서 더 느린 long-context 모드로 실행되므로 필요 시 `[output.jev].max_batch_bytes`를 낮추세요.

`2026-10-24` 기준 `64k` 이상 컨텍스트로 확인된 OpenRouter decision 모델(최신 목록은 `GET https://openrouter.ai/api/v1/models?output_modalities=decisions`):

| 제작사 | 모델 ID |
| --- | --- |
| OpenAI | `openai/gpt-6-luna-decisions` |
| TypeSafe | `typesafe/jev-1.14`, `typesafe/jev-1.13` |
| Perplexity | `perplexity/pplx-decider-v1.2`, `perplexity/pplx-decider-flash-v1.2` |
| Nace.AI | `nace-ai/drex-max`, `nace-ai/drex-v2.0`, `nace-ai/drex-v1.5` |
| Respan | `respan/span-02` |
| Upstage | `upstage/solar-decide`, `upstage/solar-decide-flash` |
| Cloudflare | `cloudflare/clef`, `cloudflare/clef-omni` |
| Liquid AI | `liquid/d1` |
| Inception | `inception/mercury-decide` |

## 이벤트 관계 탐색

이벤트 탐색은 기본 켜짐(`[output.event_navigation].is_enabled = true`, `use_builtin_rules = true`)이며 18개 언어의 `Source routes`와 Node `EventEmitter`, Tauri 이벤트 지도를 보여 줍니다. 요청별 비활성화는 `include_events: false`, 특정 키 조회는 `search`의 `event_key: "saved"`를 쓰세요.

사용자 규칙은 같은 선택자의 내장 규칙을 덮어씁니다(최대 64개, 인자 인덱스 `< 16`, 식별자 및 키 `<= 256`바이트):

```toml
[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = [
  { id = "known-on", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "on", role = "subscribe", event_arg = 0, handler_arg = 1, bus = "receiver" },
  { id = "known-emit", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "emit", role = "publish", event_arg = 0, bus = "receiver" },
]
```

- **선택자와 역할**: 정확한 `language`(`typescript`, `javascript`, `rust`), `module`, `symbol`, `role`(`publish`, `subscribe`, `unsubscribe`)을 지정하며 `method`는 생략할 수 있습니다.
- **이벤트와 핸들러**: `event_arg`(0부터 시작)와 `event_key`(고정 문자열) 중 하나를 적고, `role = "subscribe"`일 때는 `handler_arg`도 함께 지정합니다.
- **버스와 한정자**: `bus`는 `"receiver"`(`method` 필요), `"argument"`(`bus_arg`), `"fixed"`(`bus_identity`, 예: Rust-JS 연결), `"framework"`(Tauri 프런트엔드 전용) 중 하나를 고르며, 선택적으로 `target_arg`나 `target`, `channel`(`"default"`), `is_once`를 지정할 수 있습니다.

## 검색 표시 압축과 grep 원문 묶음

`search`는 확정된 관계 위치와 본문을 유지한 채 비호출 참조를 파일과 행 위치만 남기고, 반복되는 주의 문구를 `[N1]` 노트로 합치며, 본문에 이미 보이는 리터럴 미리보기를 생략합니다.

`grep`의 `view="source_grouped"`는 파일당 하나의 제목 아래에 행을 묶어 보여 줍니다(본문 모드 전용; `expand="none"`의 `42:match`/`43-context`와 `expand="callable"` 지원).

## 이전 설정 이름

이전 키도 계속 읽히며 같은 파일에서는 표준 이름이 우선합니다(자동 이전은 저장소 설정만 수정).

| 이전 경로 | 표준 경로 |
| --- | --- |
| `tool_output.is_redact_enabled` | `output.is_redact_enabled` |
| `search.result_threshold` | `output.search.detail_file_limit` |
| `search.search_overview_file_limit` | `output.search.overview_file_limit` |
| `search.search_detail_snippet_max_lines` | `output.search.snippet_max_lines` |
| `search.search_detail_symbol_limit` | `output.search.symbol_limit` |
| `search.search_detail_byte_cap` | `output.search.max_bytes` |
| `search.search_literal_max_len` | `output.search.literal_max_chars` |
| `search.search_literal_limit` | `output.search.literal_limit` |
| `search.search_anchor_snippet_limit` | `output.search.anchor_snippet_limit` |
| `tool_output.read_output_byte_cap` | `output.read.max_bytes` |
| `tool_output.grep_max_columns` | `output.grep.max_columns` |
| `caller_context.caller_context_default` | `output.context.is_enabled` |
| `caller_context.caller_list_cap` | `output.context.caller_limit` |
| `caller_context.callee_list_cap` | `output.context.callee_limit` |
| `caller_context.annotation_sub_budget` | `output.context.max_bytes` |
| `caller_context.common_name_threshold` | `output.context.common_name_threshold` |
| `caller_context.caller_omit_def_threshold` | `output.context.caller_omit_def_threshold` |
| `index.index_path` | `index.path` |
| `index.max_file_size` | `index.max_file_bytes` |
| `caller_context.navigation_store_references` | `index.store_references` |
| `refresh.watch` | `index.refresh.watch` |
| `refresh.watch_debounce_ms` | `index.refresh.watch_debounce_ms` |
| `refresh.index_staleness_ms` | `index.refresh.index_staleness_ms` |
| `refresh.indexer_auto_restart` | `index.refresh.indexer_auto_restart` |
| `language_support.is_document_support_enabled` | `index.language_support.is_document_support_enabled` |
| `language_support.is_shell_support_enabled` | `index.language_support.is_shell_support_enabled` |
| `language_support.is_infrastructure_support_enabled` | `index.language_support.is_infrastructure_support_enabled` |
| `language_support.is_interface_support_enabled` | `index.language_support.is_interface_support_enabled` |
| `language_support.is_build_support_enabled` | `index.language_support.is_build_support_enabled` |
| `caller_context.navigation_context_default` | `output.navigation.is_enabled` |
| `caller_context.navigation_callsite_budget` | `output.navigation.callsite_budget` |
| `caller_context.scan_cap` | `output.navigation.scan_limit` |
| `redact.*` | `output.redact.*` |
| `macro_expansion.*` | `output.macro_expansion.*` |
| `event_navigation.*` | `output.event_navigation.*` |
| `analysis.navigation.*` (v18) | `output.navigation.*` |
| `analysis.macro_expansion.*` (v18) | `output.macro_expansion.*` |
| `analysis.event_navigation.*` (v18) | `output.event_navigation.*` |
