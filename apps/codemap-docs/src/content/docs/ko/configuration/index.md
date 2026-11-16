---
title: 설정 레퍼런스
description: codemap-search의 config.toml 설정 키, auth.toml 인증정보, 제외 규칙, Jev 필터, 이전 키 이름 레퍼런스입니다.
---

## 설정 파일과 우선순위

바꿀 키만 적으면 되며, 키 단위로 `저장소 > 전역 > 기본값` 순으로 적용됩니다.

| 범위 | `config.toml` | `auth.toml` (API 키) |
| --- | --- | --- |
| 저장소 | `<repo>/.codemap/config.toml` | `<repo>/.codemap/auth.toml` |
| 전역 | `$CODEMAP_HOME/config.toml` (기본 `~/.codemap/config.toml`) | `$CODEMAP_HOME/auth.toml` (기본 `~/.codemap/auth.toml`) |

`.gitignore`, 전역 gitignore, `.git/info/exclude`는 기본으로 적용됩니다. Git에서 추적하는 경로를 색인, `find`, `grep`에서 추가로 빼려면 저장소 루트에 `.codemapignore`(gitignore 문법)를 두세요.

## `config.toml` 예시

```toml
[output.search]
max_bytes = "1mb"

[output.redact]
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD"]
sensitive_fields = ["internalCredential"]

[index]
mode = "auto" # "auto", "on", "off" (변경 후 재시작 필요)

[index.exclude]
excluded_directories = [".idea", ".vscode", "node_modules", "target", "dist", "build", "vendor"]

[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"
```

## 설정 키

바이트 크기 키에는 정수 바이트 또는 `b`, `kb`, `mb`, `gb` 접미사가 붙은 문자열(1024 거듭제곱 기준, 예: `"50mb"` = `52428800`)을 적습니다.

| 키 | 자료형 | 기본값 | 설명 |
| --- | --- | --- | --- |
| `[global_instructions].enabled` | bool | `false` | 클라이언트 전역 지침 파일에 탐색 안내 한 줄 자동 관리(전역 설정 전용) |
| `[output].is_redact_enabled` | bool | `true` | MCP 출력의 인증정보 및 선택 PII 마스킹 |
| `[output].max_bytes` | 정수 바이트 또는 크기 문자열 | 미지정 | 도구별 `max_bytes`가 없을 때 적용할 공통 응답 상한 |
| `[output.client].claude_max_result_chars` | 정수(문자), 1~500000 | `100000` | Claude Code 전달 한도(변경 후 MCP 재연결) |
| `[output.client].codex_output_token_limit` | 양의 정수(토큰) | `100000` | Codex 출력 토큰 한도(`codemap-search codex-config` 출력을 `~/.codex/config.toml`에 반영) |
| `[output.client].pi_max_bytes` | 정수 바이트 또는 크기 문자열 | `100000` | `pi`(`@buyong/pi-mcp-adapter`) 전달 한도(변경 후 MCP 재연결) |
| `[output.client].opencode_max_bytes` | 정수 바이트 또는 크기 문자열 | `100000` | OpenCode 전달 한도(`opencode.json`의 `tool_output.max_bytes`, 기본 `51200`과 일치 권장) |
| `[output.search].detail_file_limit` | 정수 | `24` | 상세 발췌를 표시할 상위 파일 수 |
| `[output.search].overview_file_limit` | 정수 | `80` | 간략 순위 목록에 표시할 최대 파일 수 |
| `[output.search].snippet_max_lines` | 정수 | `500` | `search`에서 심볼당 최대 발췌 줄 수 |
| `[output.search].symbol_limit` | 정수 | `100` | `search`에서 파일당 표시할 최대 심볼 수 |
| `[output.search].max_bytes` | 정수 바이트 또는 크기 문자열 | `"1mb"` (`1048576`) | `search` 1회 응답 크기 상한 |
| `[output.search].literal_max_chars` | 정수(문자) | `1200` | 일치한 문자열 리터럴의 최대 표시 길이 |
| `[output.search].literal_limit` | 정수 | `60` | `search`에서 파일당 최대 리터럴 표시 수 |
| `[output.search].anchor_snippet_limit` | 정수 | `20` | 파일당 전체 발췌 최대 심볼 수(나머지는 선언부만 표시) |
| `[output.read].max_bytes` | 정수 바이트 또는 크기 문자열 | `"5mb"` (`5242880`) | `read` 및 함수 확장 `grep`의 출력 크기 상한 |
| `[output.read].is_outside_callers_enabled` | bool | `true` | `read view="source"` 끝에 범위 밖 호출자·import 파일 표시 |
| `[output.grep].max_columns` | 정수 | `0` | 본문 모드 `grep`의 줄 길이 상한(`0`이면 제한 없음) |
| `[output.grep].max_bytes` | 정수 바이트 또는 크기 문자열 | `[output].max_bytes` | `grep` 응답 크기 상한 |
| `[output.context].is_enabled` | bool | `true` | `search`에서 `caller_context` 생략 시 기본 호출 관계 표시 여부 |
| `[output.context].caller_limit` | 정수 | `1000` | 심볼당 최대 호출자·참조 표시 수 |
| `[output.context].callee_limit` | 정수 | `1000` | 심볼당 최대 호출 대상 표시 수 |
| `[output.context].max_bytes` | 정수 바이트 또는 크기 문자열 | `"128kb"` (`131072`) | `[output.search].max_bytes` 안의 호출 관계 표시 예산 |
| `[output.context].common_name_threshold` | 정수 | `2` | 이름 기반 관계에 모호함 표시를 붙이는 동명 정의 수 |
| `[output.context].caller_omit_def_threshold` | 정수 | `5` | 추정 호출자 목록 대신 `grep` 안내로 전환하는 동명 정의 수 |
| `[output.navigation].is_enabled` | bool | `false` | 호출 위치를 구조적으로 확인해 확정 대상을 `precise`로 표시 |
| `[output.navigation].callsite_budget` | 정수 | `1000` | 이름 기반 탐색으로 넘어가기 전 구조적으로 검사할 최대 호출 수 |
| `[output.navigation].scan_limit` | 정수 | `16000` | 심볼 이름 간 공유하는 호출자 스캔 상한(이름당 최소 25건) |
| `[output.redact].pii_entities` | 문자열 배열 | `[]` | 마스킹할 PII 유형 목록([PII 마스킹](/ko/configuration/pii-redaction/) 참고) |
| `[output.redact].sensitive_fields` | 문자열 배열 | `[]` | 비밀값으로 처리할 추가 필드명(대소문자·구분자 정규화) |
| `[output.redact].rules` | 인라인 테이블 배열 | `[]` | 사용자 정규식 마스킹 규칙(`{ id = "custom.*", pattern = "..." }`) |
| `[output.redact].exceptions` | 인라인 테이블 배열 | `[]` | 마스킹에서 제외할 `{ rule_id, value }` 쌍 |
| `[index].mode` | `"auto"`, `"on"`, `"off"` | `"auto"` | 색인 실행 모드(변경 후 재시작 필요): `"auto"`(`auto_max_files` 이하 저장소), `"on"`(항상), `"off"`(비활성) |
| `[index].auto_max_files` | 정수 | `50000` | `auto` 모드의 최대 색인 가능 파일 수(변경 후 재시작 필요) |
| `[index].path` | 문자열 | `".codemap/index"` | 색인 저장 경로(변경 후 재시작 필요) |
| `[index].max_file_bytes` | 정수 바이트 또는 크기 문자열 | `"1mb"` (`1048576`) | 색인 시 건너뛸 파일 크기 기준 |
| `[index].store_references` | bool | `false` | 함수 호출 외 심볼 참조 위치도 색인에 저장 |
| `[index.refresh].watch` | bool | `true` | 백그라운드 파일 변경 감시 사용(변경 후 재시작 필요) |
| `[index.refresh].watch_debounce_ms` | 정수(ms) | `500` | 파일 변경 이벤트 묶음 대기 시간(변경 후 재시작 필요) |
| `[index.refresh].index_staleness_ms` | 정수(ms) | `5000` | `watch = false`일 때 요청 기반 갱신 확인 간격 |
| `[index.refresh].indexer_auto_restart` | bool | `true` | 백그라운드 색인 스레드 중단 시 자동 재시작 |
| `[index.language_support].is_document_support_enabled` | bool | `false` | `.md`, `.mdx` 파일 색인 포함 |
| `[index.language_support].is_shell_support_enabled` | bool | `false` | `.sh`, `.bash`, `.zsh` 스크립트 색인 포함 |
| `[index.language_support].is_infrastructure_support_enabled` | bool | `false` | HCL/Terraform, Dockerfile, Nix 파일 색인 포함 |
| `[index.language_support].is_interface_support_enabled` | bool | `false` | Protocol Buffers, GraphQL 파일 색인 포함 |
| `[index.language_support].is_build_support_enabled` | bool | `false` | Make, CMake, Bazel 빌드 파일 색인 포함 |
| `[index.exclude].excluded_directories` | 문자열 배열 | [디렉터리 제외 규칙](#디렉터리-제외-규칙) 참고 | 선택적 디렉터리 제외 glob 목록 |
| `[index.exclude].use_git_exclude` | bool | `true` | `.git/info/exclude` 적용 여부 |
| `[output.context.exclude].should_include_test_code` | bool | `false` | 호출 관계 문맥에 테스트 코드 포함 여부 |
| `[output.context.exclude].test_file_patterns` | 문자열 배열 | [테스트 코드 문맥](#테스트-코드-문맥) 참고 | 테스트 파일 경로 glob 목록 |
| `[output.context.exclude].test_attributes` | 언어 → 문자열 배열 | [테스트 코드 문맥](#테스트-코드-문맥) 참고 | 언어별 테스트 속성·어노테이션 이름 |
| `[output.context.exclude].test_decorators` | 언어 → 문자열 배열 | [테스트 코드 문맥](#테스트-코드-문맥) 참고 | 언어별 테스트 데코레이터 이름 |
| `[output.context.exclude].test_calls` | 언어 → 문자열 배열 | [테스트 코드 문맥](#테스트-코드-문맥) 참고 | 언어별 테스트 호출 이름 |
| `[output.jev].enabled` | bool | `false` | [Jev 필터](#jev-필터) 활성화 |
| `[output.jev].scope` | 문자열 배열 | `["search", "read", "grep"]` | Jev 활성화 시 필터링할 도구 목록 |
| `[output.jev].model` | 문자열 | `"jev-1.14.0"` | 판정(decision) 모델 ID(`jev-*`는 TypeSafe, `author/slug`는 OpenRouter) |
| `[output.jev].timeout_ms` | 양의 정수(ms), 최대 7일 | `45000` | 호출당 Jev 요청 제한 시간 |
| `[output.jev].max_in_flight_requests` | 정수, 1~8 | `8` | Jev 호출당 최대 동시 HTTP 요청 수 |
| `[output.jev].request_spacing_ms` | 정수(ms), 100 이상 | `100` | Jev 배치 요청 간 최소 간격 |
| `[output.jev].max_batch_bytes` | 정수 바이트 또는 크기 문자열, 1~168000 | `168000` | Jev 배치당 최대 페이로드 크기 |
| `[output.jev].pool_idle_timeout_ms` | 양의 정수(ms), 최대 7일 | `30000` | 유휴 HTTPS 연결 유지 시간 |
| `[output.jev].search_filter_min_unrelated_probability` | 수, `0.5 < 값 <= 1.0` | `0.70` | 무관한 코드로 판정해 생략하는 최소 확률 |
| `[filesystem_permissions].find` | 문자열 | `"anywhere"` | `find` 접근 범위: `"workspace"`, `"allowed_roots"`, `"anywhere"` |
| `[filesystem_permissions].grep` | 문자열 | `"anywhere"` | `grep` 접근 범위: `"workspace"`, `"allowed_roots"`, `"anywhere"` |
| `[filesystem_permissions].read` | 문자열 | `"anywhere"` | `read` 접근 범위: `"workspace"`, `"allowed_roots"`, `"anywhere"` |
| `[filesystem_permissions].allowed_roots` | 문자열 배열 | `[]` | 접근 범위가 `"allowed_roots"`일 때 허용할 외부 경로 |
| `[related_terms].enabled` | bool | `false` | `synonyms` 도구 활성화 |
| `[related_terms].dataset_path` | 문자열 | 미지정 | 다운로드 대신 사용할 로컬 뜻 사전 파일 경로 |
| `[update].config_auto_update` | bool | `true` | `mcp` 시작 시 빠진 저장소 `config.toml`/`auth.toml` 템플릿 생성 |

## 주요 설정 상세

### 전역 지침

감지된 클라이언트의 전역 지침 파일 경로입니다. `[global_instructions].enabled`를 바꾼 뒤에는 새 에이전트 세션을 여세요.

| 클라이언트 | 기본 파일 | 경로 변경 환경 변수 |
| --- | --- | --- |
| Codex (`codex-mcp-client`) | `~/.codex/AGENTS.md` (비어 있지 않은 `AGENTS.override.md` 우선) | `CODEX_HOME` |
| Claude Code (`claude-code`) | `~/.claude/CLAUDE.md` | `CLAUDE_CONFIG_DIR` |
| `pi` | `~/.pi/agent/AGENTS.md` | `PI_CODING_AGENT_DIR` |
| OpenCode (`opencode`) | `~/.config/opencode/AGENTS.md` | `OPENCODE_CONFIG_DIR` |

### 디렉터리 제외 규칙

`[index.exclude].excluded_directories` 기본 목록은 `[".idea", ".vscode", ".vs", "node_modules", ".yarn", "target", "dist", "build", "vendor"]`입니다. 기본 제외를 모두 끄려면 `[]`로 설정하세요. VCS 폴더와 `.codemap`은 설정과 관계없이 항상 제외됩니다.

| 패턴 | 일치 범위 |
| --- | --- |
| `build` 또는 `**/build` | 모든 깊이의 `build` |
| `./build` | 저장소 루트의 `build`만 |
| `apps/web/build` | 해당 디렉터리와 하위 전체 |
| `apps/api/**/__pycache__` | `apps/api` 아래 모든 깊이의 `__pycache__` |
| `apps/native/cmake-build-*` | `apps/native` 아래 와일드카드 디렉터리 |

### 테스트 코드 문맥

`[output.context.exclude].should_include_test_code = false`일 때 호출 관계 문맥에서 제외하는 기본 테스트 패턴입니다.

```toml
[output.context.exclude]
should_include_test_code = false
test_file_patterns = ["**/tests/**", "**/test/**", "**/__tests__/**", "test_*.py", "*_test.*", "*.test.*", "*_spec.*", "*.spec.*", "*Test.java", "*Tests.java", "*IT.java"]
[output.context.exclude.test_attributes]
rust = ["test", "tokio::test", "async_std::test", "rstest", "rstest::rstest", "cfg(test)"]
java = ["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Nested", "BeforeEach", "AfterEach", "BeforeAll", "AfterAll"]
kotlin = ["Test", "ParameterizedTest", "RepeatedTest", "BeforeTest", "AfterTest", "BeforeEach", "AfterEach"]
csharp = ["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestFixture", "SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown"]
swift = ["Test", "Suite"]
php = ["Test"]
[output.context.exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "unittest.skip", "unittest.skipIf", "unittest.skipUnless", "unittest.expectedFailure"]
[output.context.exclude.test_calls]
javascript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
typescript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
dart = ["test", "group", "testWidgets"]
ruby = ["describe", "context", "it", "specify"]
powershell = ["Describe", "Context", "It"]
```

### 민감값 마스킹

`[output.redact].rules`의 규칙 `id`는 `custom.`으로 시작해야 합니다. 정규식에 `(?P<secret>...)` 캡처 그룹을 쓰면 해당 부분만 마스킹합니다.

```toml
[output.redact]
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD"]
sensitive_fields = ["internalCredential"]
rules = [{ id = "custom.acme", pattern = 'ACME_(?P<secret>[A-Z0-9]+)' }]
exceptions = [{ rule_id = "custom.acme", value = "ACME_EXAMPLE" }]
```

내장 PII 유형 90종 목록은 [PII 마스킹](/ko/configuration/pii-redaction/)을 참고하세요.

### 매크로 확장과 이벤트 규칙

C/C++/어셈블리 매크로 전처리(`clang` / `nasm`)와 사용자 이벤트 버스 규칙(`typescript`, `javascript`, `rust`) 설정 예시입니다.

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

[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = [
  { id = "known-on", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "on", role = "subscribe", event_arg = 0, handler_arg = 1, bus = "receiver" },
  { id = "known-emit", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "emit", role = "publish", event_arg = 0, bus = "receiver" },
]
```

### Jev 필터

Jev는 판정 모델로 `search`, `read`, `grep` 결과에서 작업과 무관한 코드 블록을 생략합니다. `auth.toml`(또는 `TYPESAFE_API_KEY` / `OPENROUTER_API_KEY` 환경 변수)에 API 키를 넣고 `config.toml`에서 `[output.jev]`를 켜세요.

```toml
# .codemap/auth.toml
[jev]
api_key = "[REDACTED]"
[openrouter]
api_key = "[REDACTED]"
```

```toml
# .codemap/config.toml
[output.jev]
enabled = true
scope = ["search", "read", "grep"]
model = "jev-1.14.0" # 또는 아래 OpenRouter 모델 ID
```

#### OpenRouter 판정 모델

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

## 이전 설정 키

이전 키도 계속 인식되며, 같은 파일에 두 키가 모두 있으면 현재 키가 우선합니다.

| 이전 키 | 현재 키 |
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
| `analysis.navigation.*` | `output.navigation.*` |
| `analysis.macro_expansion.*` | `output.macro_expansion.*` |
| `analysis.event_navigation.*` | `output.event_navigation.*` |
