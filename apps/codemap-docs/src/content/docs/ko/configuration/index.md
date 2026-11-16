---
title: 설정 레퍼런스
description: codemap-search의 config.toml 설정 키, auth.toml 인증정보, 클라이언트 전달 한도, 제외 규칙, Jev 단계, 이벤트 탐색의 전체 레퍼런스입니다.
---

설정은 필수가 아닙니다. 바꾸고 싶은 키만 적으면 나머지는 전역 설정이나 내장 기본값을 따릅니다.

이벤트 탐색(`output.event_navigation`)과 매크로 확장(`output.macro_expansion`)은 기본으로 켜져 있습니다. `[analysis].target_os`는 생략하면 전역 설정을 이어받고 `""`을 적으면 상속값을 지우며, 지정하지 않으면 호스트 OS를 추정하지 않고 미지정 상태로 분석합니다. `[output.jev]` 필터는 직접 켜기 전까지 모든 단계가 꺼져 있습니다([선택적 Jev 판단 단계](#선택적-jev-판단-단계) 참고).

## 섹션 구성과 출력 한도

동작 설정은 `config.toml`에 기능별로 나누어 적고, API 키는 `auth.toml`에 따로 둡니다. 상위 계층에 값을 적으면 빈 배열(`[]`)이라도 하위 값을 덮어쓰므로, 전역 값이나 기본값을 상속하려면 해당 키를 지우거나 주석 처리하세요.

| 섹션 | 역할 |
| --- | --- |
| `output` | 공통 MCP 응답 바이트 상한과 마스킹 스위치 |
| `output.client` | Claude 문자 한도, Codex 토큰 한도, pi 및 opencode 바이트 한도 |
| `output.search` | 순위 기반 파일, 심볼, 코드 조각 출력 |
| `output.read` | 직접 파일 읽기 응답 |
| `output.grep` | 일치 행과 함수 본문 확장 응답 |
| `output.context` | 호출자와 호출 대상 표시 수, 관계 출력 예산 |
| `output.navigation` | 소스 구조 기반 탐색과 호출자 스캔 예산 |
| `output.macro_expansion` | C, C++, 어셈블리 매크로 전처리와 생성 선언 탐색 |
| `output.event_navigation` | 색인된 이벤트와 소스 경로 탐색 결과 |
| `output.redact` | 추가 마스킹 규칙, PII 유형, 예외 목록 |
| `output.jev` | 선택적 `search`, `read`, `grep` 관련성 필터(기본 꺼짐) |
| `output.context.exclude` | `search` 호출 관계, `read`/`grep` 자동 문맥, 이벤트 및 소스 경로의 공통 테스트 제외 |
| `index`, `index.refresh`, `index.language_support` | 색인 저장 경로, 갱신 주기, 선택 언어 활성화 |
| `index.exclude` | 색인, `search`, 호출자 스캔, `find`, `grep`의 공통 디렉터리 제외 |
| `analysis` | Rust 분석용 명시적 `target_os` |
| `global_instructions` | 사용자 전역 클라이언트 지침 한 줄 관리(저장소 설정은 무시) |

`search`는 색인에 들어 있는 파일을 대상으로 하며, `find`와 `grep`은 같은 디렉터리 제외 규칙을 공유합니다. 직접 `read` 호출에는 디렉터리 제외를 적용하지 않지만, 함께 붙는 자동 문맥에는 `output.context.exclude`를 적용합니다.

`output.max_bytes`는 선택 항목입니다. 응답 크기 한도는 **저장소 도구별 설정 → 저장소 공통값 → 전역 도구별 설정 → 전역 공통값 → 기본값** 순서로 정해집니다. 아무것도 정하지 않으면 `search`는 `1 MiB`, `read`는 `5 MiB`가 기본이며, 함수 본문으로 확장된 `grep`은 같은 계층에 `grep`이나 공통 한도가 없을 때 `output.read.max_bytes`를 이어받습니다. `output.context.max_bytes`는 관계 주석 전용 예산으로, 검색 응답의 남은 공간 안에서 적용됩니다.

명시한 한도를 넘기면 `search`는 부분 결과와 잘림 안내를 돌려주고 `read`는 더 좁은 줄 범위를 요청합니다. `find`, `initial_instructions`, 비확장 `grep`은 응답을 조용히 자르지 않고 범위 축소 오류를 반환합니다. 한도는 MCP 응답 본문 텍스트 기준이며, JSON 봉투나 오류 메시지, 일반 CLI 명령 출력은 자르지 않습니다.

### 클라이언트 전달 한도

`output.client`는 각 코딩 에이전트가 모델에 넘기는 최종 응답의 최대 크기입니다. 도구별 `max_bytes`가 후보 수집, Jev 입력, 렌더링, 페이지 분할 단계에서 먼저 작동하더라도, 최종 응답은 항상 설정된 클라이언트 한도 중 가장 작은 값 안에 들어가야 합니다.

한도는 UTF-8 바이트 기준으로 비교합니다. Claude(`claude_max_result_chars`)는 문자 한도를 UTF-8 바이트 상한으로 적용해 멀티바이트 문자가 포함된 출력도 Claude의 문자 예산을 넘지 않도록 하며, Codex(`codex_output_token_limit`)는 `floor(토큰 수 × 3.5)`로 여유 추정치를 계산하고, `pi`와 `opencode`는 바이트 값을 그대로 씁니다. 네 키 모두 기본값이 `100000`이므로 별도 설정이 없으면 최종 전달 상한은 `100000`바이트입니다.

검사는 Jev 생략, 마스킹, 중복 제거가 모두 끝난 최종 텍스트에 적용됩니다.

- **`search`**는 일치 본문과 불확실 본문을 먼저 남기고, `min(output.search.max_bytes, 한도 - min(512, 한도 / 8))`을 넘는 낮은 우선순위 본문을 정확한 `read` 범위 안내로 바꿉니다. 남겨 둔 여유분은 관계 주석, 후속 탐색, 마스킹과 중복 표식을 위한 공간입니다.
- **`read` (`ranges`)**는 제목과 항목별 오류를 포함한 전체 묶음을 `output.read.max_bytes`와 클라이언트 한도 중 작은 값 안에 맞추고, 처음 미룬 항목을 알리는 안내 공간을 확보합니다. 미룬 항목은 전달 이력에 기록하지 않습니다.
- **단일 `read`, `grep` 및 기타 호출**은 클라이언트가 출력을 잘라내지 않도록 더 좁은 범위나 작은 `head_limit`, 다음 `offset`을 제안하는 오류를 반환합니다.

클라이언트별 세부 동작은 다음과 같습니다.

- **Claude Code (`output.client.claude_max_result_chars`, `1~500000`, 기본 `100000`)**: 각 도구의 `tools/list` 메타데이터에 `_meta["anthropic/maxResultSizeChars"]`를 실어 보냅니다. 값을 바꾼 뒤에는 MCP를 다시 연결해야 반영됩니다([Claude Code 공식 문서](https://code.claude.com/docs/en/mcp#raise-the-limit-for-a-specific-tool)).
- **Codex (`output.client.codex_output_token_limit`, 기본 `100000`)**: `codemap-search codex-config`(서버 ID가 다르면 `--server-name 이름` 추가)를 실행해 출력된 `mcp_servers.codemap-search.tools.<tool>.output_token_limit` 조각을 Codex 설정에 합치세요. Codex Code Mode 연결(`params.clientInfo.name == "codex-mcp-client"`)에는 첫 줄에 `// @exec: {"max_output_tokens": N}`을 적고 `wait`에도 같은 예산을 쓰라는 안내를 함께 보냅니다. 단, Codex 자체 `tool_output_token_limit`와 여러 호출이 합쳐진 `exec` 셀 한도는 서버가 제어할 수 없습니다([Codex MCP 공식 문서](https://learn.chatgpt.com/docs/extend/mcp#other-configuration-options), [Codex 설정 문서](https://learn.chatgpt.com/docs/config-file/config-reference#configtoml)).
- **`pi` (`output.client.pi_max_bytes`, 기본 `100000`)**: `tools/list`에 `_meta["pi/maxOutputBytes"]`를 실어 보냅니다. [@buyong/pi-mcp-adapter](https://www.npmjs.com/package/@buyong/pi-mcp-adapter)는 이 값을 `mcp.json`의 `maxOutputBytes`보다 먼저 적용하며, `pi` 내장 MCP는 메타데이터를 무시하고 20 KB에서 출력을 자릅니다. 변경 후 MCP를 재연결하세요.
- **`opencode` (`output.client.opencode_max_bytes`, 기본 `100000`)**: OpenCode는 서버 메타데이터를 읽지 않으며 기본 [`tool_output.max_bytes`](https://opencode.ai/v2/docs/config)가 `51200`바이트라 초과분은 파일로 저장합니다. OpenCode 쪽 `tool_output.max_bytes`를 서버 값과 맞춰 주세요. OpenCode의 줄 수 한도(기본 `2000`줄)는 서버가 강제하지 않습니다.

진단용 `MCP client delivery context` 로그는 `params._meta.callId` 유무(`exec-<uuid>` 형식만 값 기록), 응답 바이트 수, Codex 한도를 남기지만 출력 동작 자체에는 영향을 주지 않습니다.

## 설정 위치와 우선순위

설정은 저장소 파일과 전역 파일을 **키 단위**로 합치며 우선순위는 **저장소 → 전역 → 내장 기본값**입니다. 예를 들어 저장소 파일에 `[output.search].detail_file_limit`만 있어도 나머지 키는 전역 설정이나 기본값을 그대로 따릅니다.

| 범위 | 경로 |
| --- | --- |
| 저장소 | `<repo>/.codemap/config.toml` |
| 전역 | `$CODEMAP_HOME/config.toml`, 미지정 시 `~/.codemap/config.toml` |

### 전역 지침

`[global_instructions].enabled`의 기본값은 `false`이며 **전역 설정에서만** 읽습니다(저장소 설정에 적으면 경고 후 무시).

```toml
[global_instructions]
enabled = true
```

MCP 초기화 시 서버는 `clientInfo.name`과 상속받은 환경 변수를 확인해 클라이언트별 전역 지침 파일을 찾습니다.

| 클라이언트 이름 | 기본 파일 | 경로 변경과 기존 파일 우선순위 |
| --- | --- | --- |
| `codex-mcp-client` | `~/.codex/AGENTS.md` | `CODEX_HOME`. 비어 있지 않은 `AGENTS.override.md`가 우선 |
| `claude-code` | `~/.claude/CLAUDE.md` | `CLAUDE_CONFIG_DIR` |
| `pi` | `~/.pi/agent/AGENTS.md` | `PI_CODING_AGENT_DIR`. `AGENTS.override.md`, `AGENTS.md`, `AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD` 중 첫 번째 존재하는 파일 |
| `opencode` | `$XDG_CONFIG_HOME/opencode/AGENTS.md` (보통 `~/.config/opencode/AGENTS.md`) | `OPENCODE_CONFIG_DIR`. v1은 자체 `AGENTS.md`가 없고 Claude 지침이 켜져 있으면 기존 `~/.claude/CLAUDE.md`를 사용. v2는 `AGENTS.md` 사용 |

이 경로 규칙은 [Codex](https://learn.chatgpt.com/docs/agent-configuration/agents-md), [Claude Code](https://code.claude.com/docs/en/memory), [pi](https://pi.dev/docs/latest/configuration), [OpenCode v1](https://dev.opencode.ai/docs/rules/) 및 [v2](https://opencode.ai/v2/docs/instructions/) 지침(그리고 OpenCode의 [전역 경로 구현](https://github.com/anomalyco/opencode/blob/dev/packages/core/src/global.ts))을 따릅니다. 알 수 없는 클라이언트 이름이나 Claude 대체 파일이 있는데 OpenCode 버전을 확인할 수 없는 경우는 건너뜁니다.

활성화되면 로캘과 관계없이 아래 영문 한 줄을 추가하거나 갱신합니다.

```md
- Prefer codemap-search for code navigation when available. Call `initial_instructions` first and follow the returned guidance. <!-- codemap-search:managed -->
```

실제 적용 경로와 소유 줄은 codemap 전역 디렉터리의 `global-instructions.json`에 기록합니다. `enabled = false`로 바꾸면 기록해 둔 줄만 제거하며 기존 사용자 지침, 파일 권한, 줄바꿈, 심볼릭 링크는 건드리지 않습니다. 소유 기록이 없는 표식, 수동 수정된 줄, 중복 줄은 그대로 두고 경고만 남깁니다. 초기화 시점이나 설정 재로드 직후 요청에서 반영되며, 새 에이전트 세션을 열어야 클라이언트가 변경된 파일을 읽습니다.

### 인증정보 (`auth.toml`)

decision 모델용 API 키는 `config.toml`이 아니라 `<repo>/.codemap/auth.toml`이나 `$CODEMAP_HOME/auth.toml`(기본 `~/.codemap/auth.toml`)에 따로 보관하세요.

```toml
# .codemap/auth.toml — 버전 관리에서 제외하세요
[jev]
api_key = "<발급받은 TypeSafe 키>"

[openrouter]
api_key = "<발급받은 OpenRouter 키>"
```

각 키는 **저장소 auth → 전역 auth → 환경 변수**(`TYPESAFE_API_KEY` 또는 `OPENROUTER_API_KEY`) 순서로 독립적으로 읽고, `[output.jev].model`이 고른 제공자 키만 사용합니다. 빈 문자열이나 공백만 있으면 다음 소스로 넘어가며, 파싱 실패나 잘못된 항목은 비밀 값을 찍지 않고 경고한 뒤 다음 소스로 대체합니다. OpenRouter 키는 OpenRouter Decisions API(`https://openrouter.ai/api/alpha/decisions`)에만 쓰며 채팅 API 등에는 보내지 않습니다([OpenRouter decision 모델](#openrouter-decision-모델) 참고).

`[update].config_auto_update = true`이면 MCP 시작 시 누락된 저장소 `auth.toml` 템플릿을 Unix 기준 `0600` 권한(Windows는 상속 ACL)으로 만들어 줍니다. 이때 환경 변수의 키를 복사하거나 전역 파일을 만들지는 않습니다. 이름이 `auth.toml`인 파일은 대소문자 구분 없이 색인과 `search`, 기본 `find`/`grep`에서 제외됩니다(`read`/`parse`나 `include_ignored: true`로는 열람 가능). Git 무시 설정은 직접 추가해야 합니다.

## 설정 읽기와 자동 작성

설정 파일은 없어도 무방합니다. TOML 문법이 깨지면 해당 파일 계층만 건너뛰고 `stderr`에 경고를 남기며, 알 수 없는 키나 잘못된 값은 키 단위로 경고 후 하위 계층 값으로 대체합니다(`저장소 → 전역 → 기본값`).

`[update].config_auto_update = true`(기본값)일 때 `mcp` 서버는 시작 시 빠진 저장소 `config.toml`과 `auth.toml`을 생성하고, 기존 저장소 `config.toml`을 현재 형식(`# codemap-config-version: 32`)에 맞춰 동기화합니다.

```toml
# codemap-config-version: 32
```

- **초기 생성**: 새 저장소 설정을 만들 때 공통 제외 폴더와 감지된 프로젝트용 재귀 glob을 채우고, 내장 기본값을 활성 키로 적어 줍니다(`output.max_bytes`, `compilation_database`, `index.mode`, `index.auto_max_files`, `analysis.target_os` 같은 선택 예시는 주석 처리). 주석 언어는 한국어 OS 로캘이면 한국어, 그 밖에는 영어를 씁니다.
- **비파괴 동기화**: 기존 저장소 파일을 갱신할 때는 새 설정을 주석 예시로 덧붙이고 생성 주석만 최신화하며, 사용자가 고친 값과 `[]` 목록, 사용자 메모는 그대로 둡니다. 지원하는 이전 키 이름도 충돌이 없는 한 자동으로 새 위치로 옮겨 줍니다.
- **제외 목록 유지**: 한 번 만들어진 `[index.exclude].excluded_directories`는 항목을 지우거나 `[]`로 비우고 주석 처리해도 다시 채우지 않습니다.
- **전역 파일 보호**: 전역 `config.toml`과 `auth.toml`은 자동으로 만들거나 고치지 않습니다(이전 키 별칭은 계속 읽음). `config_auto_update = false`로 두면 템플릿 생성과 저장소 자동 쓰기를 모두 끄며, 심볼릭 링크로 된 설정 파일도 링크를 유지합니다.

### 자동 작성을 껐을 때 수동 전환

`config_auto_update = false` 상태에서 이전 설정 파일을 직접 손볼 때는 명시한 `excluded_directories` 배열이 내장 기본 제외 목록(`node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor`)에 더해지지 않고 통째로 대체한다는 점에 유의하세요. 계속 제외할 폴더가 있다면 목록에 함께 넣은 뒤 `# codemap-config-version: 32`로 표식을 올리고, 전역 제외를 다시 상속하려면 저장소 키를 주석 처리하면 됩니다.

## 디렉터리 제외 규칙

`[index.exclude].excluded_directories`는 **선택적으로 제외할 디렉터리 규칙의 전체 목록**입니다. 숨겨진 기본 목록에 합쳐지지 않으므로 `[]`를 쓰면 선택적 제외가 모두 꺼지고, 키를 생략하면 전역 목록이나 기본 fallback 목록을 상속합니다.

공통 초기 목록은 다음과 같습니다.

```toml
[index.exclude]
excluded_directories = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index"]
```

`.idea`, `.vscode`, `.vs`는 필요에 따라 빼거나 고칠 수 있습니다. 반면 버전 관리 내부 폴더(`.git`, `.svn`, `.hg`, `.bzr`, `.jj`, `.sl`), `.codemap`, `.codemap-index`, 그리고 실제 색인 폴더는 목록에서 빼거나 `include_ignored: true`를 주더라도 탐색에서 항상 빠집니다.

어느 계층에도 `excluded_directories`가 없으면 공통 목록에 `node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor`가 더해진 fallback 목록을 씁니다. 반면 자동 생성된 저장소 설정에는 감지한 프로젝트 유형에 맞춰 `**/node_modules`, `**/build`, `**/target` 같은 **재귀 glob**이 들어가므로 모노레포나 하위 패키지에도 한 번에 적용됩니다. 같은 이름의 실제 소스 폴더가 있다면 더 좁은 경로 규칙으로 바꾸세요.

| 규칙 | 의미 |
| --- | --- |
| `build` | 모든 깊이에서 해당 이름의 폴더 |
| `**/build` | 작업공간 루트를 포함해 모든 깊이의 해당 폴더 |
| `./build` | 작업공간 루트의 해당 폴더만 |
| `apps/web/build` | 해당 폴더와 그 하위 전체 |
| `apps/api/**/__pycache__` | 해당 프로젝트 하위 모든 깊이의 캐시 폴더 |
| `apps/native/cmake-build-*` | 해당 프로젝트의 패턴과 일치하는 CMake 출력 폴더 |

규칙은 작업공간 루트를 기준으로 대소문자를 구분해 디렉터리만 걸러냅니다(파일명에는 미적용). Windows 경로 구분자는 `/`로 정규화하며, 절대 경로나 `..`, 빈 문자열, 잘못된 glob이 포함되면 배열 전체를 거부하고 하위 계층 값으로 대체합니다. 단순 이름 규칙은 `allowed_roots`로 허용한 외부 소스 트리에도 적용되지만, 작업공간 기준 상대 경로는 외부 트리에 적용되지 않습니다.

이 규칙은 색인, 코드맵, 호출자 스캔, 파일 변경 감시, `find`, `grep`에 모두 적용되며 제외된 하위 폴더 안을 직접 지정해 검색해도 우회되지 않습니다. `find`/`grep`의 `include_ignored: true`는 선택적 제외와 `.gitignore`, `.codemapignore`를 건너뛰지만 필수 내부 및 색인 폴더는 계속 제외합니다. 색인, 호출자 스캔, 감시는 작업공간 안에 체크아웃한 git 연결 worktree(`.git` 파일이 저장소의 `worktrees` 폴더를 가리키는 디렉터리)도 건너뛰며, `find`, `grep`, 코드맵은 계속 탐색하고 서브모듈도 평소처럼 색인합니다.

### 최초 프로젝트 감지

프로젝트 감지는 누락된 저장소 설정을 처음 생성할 때(또는 이전 형식의 설정을 갱신할 때)만 실행됩니다. 빌드를 실행하거나 매니페스트를 평가하지 않고 프로젝트 식별 파일명만 확인하며, 디렉터리 심볼릭 링크를 따라가지 않은 채 작업공간 전체에 적용할 재귀 glob을 한 번씩 추가합니다.

| 프로젝트 식별 파일 | 작업공간 전체에 적용하는 재귀 glob |
| --- | --- |
| `package.json` | `**/node_modules`, `**/.yarn`, `**/dist`, `**/build`, `**/coverage`, `**/.next`, `**/.nuxt`, `**/.output`, `**/.svelte-kit`, `**/.astro`, `**/.turbo`, `**/.parcel-cache` |
| `pyproject.toml`, `setup.py`, `setup.cfg`, `Pipfile`, `requirements*.txt` | `**/.venv`, `**/venv`, `**/__pycache__`, `**/.pytest_cache`, `**/.mypy_cache`, `**/.ruff_cache`, `**/.tox`, `**/.nox`, `**/build`, `**/dist`, `**/*.egg-info` |
| `Cargo.toml` | `**/target` |
| `go.mod`, `go.work` | `**/vendor` |
| `pom.xml` | `**/target` |
| `build.gradle`, `build.gradle.kts`, `settings.gradle`, `settings.gradle.kts` | `**/.gradle`, `**/build` |
| `build.sbt` | `**/target`, `**/project/target`, `**/.bloop`, `**/.metals` |
| `*.csproj`, `*.sln`, `*.slnx` | `**/bin`, `**/obj` |
| `composer.json` | `**/vendor` |
| `Gemfile`, `*.gemspec` | `**/vendor/bundle` |
| `Package.swift` | `**/.build` |
| `Podfile` / `Cartfile` | `Podfile`은 `**/Pods`, `Cartfile`은 `**/Carthage/Build` |
| `pubspec.yaml` | `**/.dart_tool`, `**/build` |
| `CMakeLists.txt` | `**/build`, `**/cmake-build-*`, `**/CMakeFiles`, `**/_deps` |
| `MODULE.bazel`, `WORKSPACE`, `WORKSPACE.bazel`, `BUILD`, `BUILD.bazel` | `**/bazel-bin`, `**/bazel-out`, `**/bazel-testlogs` |
| `.tf` 파일이 있는 디렉터리 | `**/.terraform` |

SQL, Lua, PowerShell, 독립 셸, 웹, 설정 파일과 문서에는 출력 폴더를 추측해 넣지 않으며, 사용자가 바꾼 별도 빌드 경로는 직접 추가해야 합니다. `gradle` wrapper 폴더는 유지하고 `.gradle` 캐시만 제외합니다.

## 변경 적용 시점

MCP는 `[index.refresh].watch`와 별개로 시작 시 존재하는 설정 디렉터리의 `config.toml`과 `auth.toml`을 감시하다가, 변경이 생기면 약 **1000ms** 동안 이벤트를 모아 다시 읽습니다. 시작 당시 설정 디렉터리가 없었다면 파일을 만든 뒤 서버를 재시작하세요. CLI 명령은 호출할 때마다 설정을 새로 읽습니다.

| 설정 | 적용 시점 |
| --- | --- |
| `[index.exclude]`, `[output.context.exclude]`, 모든 `[index.language_support]` 스위치 | 재로드 시 전체 색인 갱신 요청(완료 후 결과에 반영) |
| 모든 `[output.macro_expansion]` 설정 | 재로드 시 전체 색인 갱신 요청(비활성화할 때도 포함) |
| 출력 및 호출 관계 렌더링, `output.is_redact_enabled`, `[output.redact]`, `[filesystem_permissions]` | 재로드 이후 다음 도구 요청부터 즉시 적용 |
| `index.refresh.index_staleness_ms`, `index.refresh.indexer_auto_restart` | 이후 갱신 및 복구 판단부터 적용 |
| `index.max_file_bytes` | 이후 탐색 및 갱신부터 적용(단독 변경 시 전체 갱신 미요청) |
| `index.store_references` | 이후 파일 파싱부터 적용(변경 없는 파일은 기존 색인 재사용) |
| `index.path`, `index.mode`, `index.auto_max_files`, `index.refresh.watch`, `index.refresh.watch_debounce_ms` | 서버 재시작 필요 |
| `config_auto_update` | 다음 MCP 시작 시 적용 |
| `[global_instructions].enabled` (전역 전용) | MCP 초기화 또는 재로드 후 다음 요청(새 에이전트 세션에서 변경 파일 반영) |
| `auth.toml`의 `[jev].api_key`, `[openrouter].api_key` | 재로드 후 다음 Jev 요청부터 적용(공용 HTTPS 클라이언트 재생성) |
| `[output.client].claude_max_result_chars`, `pi_max_bytes` | 재로드 후 전달 한도 반영(클라이언트 메타데이터 갱신은 MCP 재연결 필요) |
| `[output.client].codex_output_token_limit` | 재로드 후 전달 한도 반영(`codex-config` 다시 내보내기 및 병합 필요) |
| `[output.client].opencode_max_bytes` | 재로드 후 전달 한도 반영(`opencode.json` 수동 일치 필요) |

## 설정 키

모든 숫자 키는 `1` 이상의 양의 정수를 받으며, `[output.grep].max_columns`만 `0`도 허용합니다. 호환성을 위해 이전 최상위 키 이름도 읽을 수 있지만 같은 파일 안에 섹션 키가 있으면 섹션 키가 우선합니다.

바이트 크기 키에는 정수 바이트나 `b`, `kb`, `mb`, `gb` 접미사가 붙은 따옴표 문자열을 쓸 수 있습니다(대소문자 무관, 1024 거듭제곱 기준: `"50mb"` = `52428800`, `"50 MB"`처럼 공백 허용). 소수, 0, 음수, 알 수 없는 단위, 범위 초과 값은 경고 후 하위 계층으로 대체되며, TOML 문법상 단위가 붙은 값은 반드시 따옴표로 감싸야 합니다(`50mb`는 오류). 이 규칙은 `index.max_file_bytes`, `output.max_bytes`, 도구별 `output.*.max_bytes`, `output.client.pi_max_bytes`, `output.client.opencode_max_bytes`, `output.macro_expansion.max_output_bytes`, `output.jev.max_batch_bytes`에 적용됩니다.

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
| `[output.grep].max_bytes` | 정수 바이트 또는 크기 문자열 | 공통 한도 상속 | `grep` 응답 상한. 함수 본문 확장 시 `output.read.max_bytes`로 대체 가능 |
| `[output.context].is_enabled` | bool | `true` | `search` 호출에서 `caller_context` 생략 시 기본 호출 관계 표시 여부 |
| `[output.context].caller_limit` | 정수 | `1000` | 심볼당 최대 호출자 또는 비호출 참조 수 |
| `[output.context].callee_limit` | 정수 | `1000` | 심볼당 최대 호출 대상 수 |
| `[output.context].max_bytes` | 정수 바이트 또는 크기 문자열 | `"128kb"` (`131072`) | `output.search.max_bytes` 내부의 호출 관계 주석 서브 예산 |
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
| `[index.exclude].excluded_directories` | 문자열 배열(상대 디렉터리 glob) | 공통 + 기본 fallback (생성 파일은 재귀 glob) | 선택적 디렉터리 제외 전체 목록. [디렉터리 제외 규칙](#디렉터리-제외-규칙) 참고 |
| `[index.exclude].use_git_exclude` | bool | `true` | `.git/info/exclude` 적용 여부(`.gitignore`와 무관) |
| `[output.context.exclude].should_include_test_code` | bool | `false` | 자동 심볼 및 호출 문맥에 테스트 코드 포함 여부 |
| `[output.context.exclude].test_file_patterns` | 문자열 배열 | 테스트 코드 문맥 참고 | 테스트 파일 glob(`[]`는 경로 판별 해제) |
| `[output.context.exclude].test_attributes` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 언어별 속성 및 어노테이션 패턴(언어 단위로 하위 목록 대체) |
| `[output.context.exclude].test_decorators` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 언어별 데코레이터 패턴(`[]`는 해당 언어 해제) |
| `[output.context.exclude].test_calls` | 언어 → 문자열 배열 | 테스트 코드 문맥 참고 | 언어별 테스트 호출 패턴(`[]`는 해당 언어 해제) |
| `[output.jev].enabled` | bool | `false` | Jev 필터링 전체 스위치(`false`면 모든 도구를 로컬로 유지) |
| `[output.jev].scope` | 문자열 배열 | `["search", "read", "grep"]` | 활성화 시 필터링할 전체 도구 목록(`[]`는 선택 없음) |
| `[output.jev].model` | 문자열 | `"jev-1.14.0"` | 고정 제공자 모델(버전만 적으면 TypeSafe, `author/slug`는 [OpenRouter](#openrouter-decision-모델), 별칭은 실패) |
| `[output.jev].timeout_ms` | 양의 정수(ms), 최대 7일 | `45000` | 준비와 대기 시간을 포함한 호출당 절대 마감 시간 |
| `[output.jev].max_in_flight_requests` | 정수, 1~8 | `8` | 평가당 최대 동시 HTTP 요청 수 |
| `[output.jev].request_spacing_ms` | 정수(ms), 100 이상 | `100` | 요청 시작 간 최소 간격 |
| `[output.jev].max_batch_bytes` | 정수 바이트 또는 크기 문자열, 1~168000 | `168000` | 배치당 인코딩 페이로드 상한(단일 질문 초과 시 실패) |
| `[output.jev].pool_idle_timeout_ms` | 양의 정수(ms), 최대 7일 | `30000` | 유휴 HTTPS 연결 유지 시간 |
| `[output.jev].search_filter_min_unrelated_probability` | 유한한 수, `0.5 < 값 <= 1.0` | `0.70` (잠정값) | 본문 생략에 필요한 최소 "아니오" 확률("예"에도 대칭 적용, 사이는 불확실로 유지) |
| `[filesystem_permissions].find` | 문자열 | `"anywhere"` | `find` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].grep` | 문자열 | `"anywhere"` | `grep` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].read` | 문자열 | `"anywhere"` | `read` 경로 정책: `workspace`, `allowed_roots`, `anywhere` |
| `[filesystem_permissions].allowed_roots` | 문자열 배열 | `[]` | 정책이 `allowed_roots`인 도구에 허용할 외부 루트 경로 |
| `[related_terms].enabled` | bool | `false` | `synonyms` 도구 활성화(`false`일 때는 다운로드 및 준비 없음) |
| `[related_terms].dataset_path` | 문자열 | 미지정 | 다운로드 대신 사용할 사용자 소유 유의어 데이터셋 경로 |
| `[update].config_auto_update` | bool | `true` | `mcp` 시작 시 빠진 저장소 `config.toml`/`auth.toml` 생성 및 주석 동기화 |

### 색인 시작 시점

`[index].mode`는 MCP 서버가 언제 작업공간을 색인할지 결정합니다.

| 모드 | 동작 |
| --- | --- |
| `"auto"` (기본값) | 서버 시작 시가 아니라 첫 `search`, `read`, `grep`, `synonyms` 또는 모노레포 `initial_instructions` 호출 때 색인을 시작합니다. 작업공간이 저장소(`.git`, `.hg`, `.jj` 중 하나가 있되 홈 디렉터리 자체는 제외) 안에 있고 색인 대상 파일이 `[index].auto_max_files` 이하일 때만 실행됩니다. |
| `"on"` | 저장소 여부나 파일 수 제한 없이 서버 시작과 동시에 색인합니다. |
| `"off"` | 작업공간을 색인하지 않습니다. |

`auto` 모드가 색인을 건너뛰면(예: 여러 저장소가 들어 있는 상위 폴더) `search`는 사유와 함께 1~2단계 아래에서 찾은 저장소 최대 3곳을 `Index unavailable:` 메시지로 알려 주며, `read`, `grep`, `find`는 색인 선언이나 호출자 없이 실시간 디스크 결과를 돌려줍니다. 저장소 밖에서는 `.codemap/` 폴더를 만들지 않고 임시 빈 색인을 썼다가 종료 시 지웁니다. 파일 수 제한은 첫 색인 기반 호출 때 검사하므로 파일 수를 넘는 저장소라도 시작 시 `.codemap/`은 생성됩니다. 이런 저장소를 색인하려면 `mode = "on"`으로 설정하고 서버를 재시작하세요.

아직 색인이 없는 git worktree는 같은 저장소의 다른 worktree에서 가장 최근에 갱신된 호환 색인을 원본 변경 없이 복사해 시작합니다. 이어서 첫 패스 동안 내용이 다른 파일과 매크로 확장 대상 네이티브 파일만 다시 추출한 뒤 결과를 반환합니다.

### 색인과 파일 제외

사용자 홈 디렉터리(`~`) 자체는 MCP 작업공간이나 명시적 `index`/`benchmark` 대상이 될 수 없지만, 그 하위 프로젝트 폴더는 정상 동작합니다.

`auth.toml`, `.txt`, `*.lock`, 주요 패키지 매니저 lockfile, `*.map`, 압축 및 번들 파일은 대소문자 구분 없이 색인, 코드맵, 호출자 스캔에서 빠집니다([지원 언어와 파일 제외](/ko/usage/languages/) 참고). 이 파일들은 `find`와 `grep`에서도 기본으로 숨겨지지만 `include_ignored: true`로 볼 수 있고 직접 `read`나 `parse`로 열 수도 있습니다. `index.max_file_bytes`보다 큰 파일도 색인에서 제외되며, 색인은 유효한 UTF-8 파일만 허용합니다(잘못된 UTF-8 파일은 기존 색인 심볼도 지우고 `read`에서 첫 오류 바이트 오프셋을 알려 줍니다).

다섯 가지 `[index.language_support]` 스위치는 색인 기반 탐색과 감시 갱신만 켜고 끄며 실시간 `find`, `grep`, `read`, 직접 `parse`에는 영향을 주지 않습니다. `[index.exclude].use_git_exclude`는 `.git/info/exclude`만 제어하므로 `false`여도 `.gitignore`, 전역 gitignore, `.codemapignore`는 계속 적용됩니다.

### 매크로 확장

`codemap-search`는 설치된 Clang과 NASM을 실행해 C, C++, CPP 기반 어셈블리 매크로를 전처리하고, NASM 확장 목록으로 생성된 `.asm` 레이블을 원래 호출 줄에 연결합니다. 도구가 없으면 원문 선언을 유지하고 미해결 안내를 붙입니다. [clangd의 컴파일 데이터베이스 탐색 방식](https://clangd.llvm.org/design/compile-commands)을 따르며 `clangd`를 띄우거나 `.clangd` 설정을 읽지는 않습니다.

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

| `[output.macro_expansion]` 키 | 자료형 / 기본값 | 동작 |
| --- | --- | --- |
| `is_enabled` | bool / `true` | 색인된 C/C++/ASM 파일과 직접 `parse`/`codemap` 호출을 자동 전처리. `false`로 끌 수 있음 |
| `compilation_database` | 비어 있지 않은 문자열 / 생략 | `compile_commands.json`(또는 그 디렉터리)이나 `compile_flags.txt` 경로(작업공간 상대 또는 절대 경로) |
| `clang_path`, `nasm_path` | 비어 있지 않은 문자열 / `"clang"`, `"nasm"` | `PATH`의 실행 파일명 또는 명시적 경로 |
| `clang_flags`, `nasm_flags` | 문자열 배열 / `[]` | 빌드 플래그 뒤에 붙일 추가 인자(저장소 배열이 전역 대체) |
| `timeout_ms` | 양의 정수 / `5000` | 프로세스당 제한 시간(ms). NASM은 전처리와 목록 생성을 모두 실행 |
| `max_output_bytes` | 정수 바이트 또는 크기 문자열 / `"8mb"` (`8388608`) | 전처리 결과 또는 NASM 목록의 최대 바이트 수 |

`compilation_database`를 생략하면 소스 부모 디렉터리에서 작업공간 루트까지 올라가며 `compile_commands.json`이나 `compile_flags.txt`를 찾습니다. JSON 데이터베이스는 정규화된 파일 경로가 정확히 일치하는 항목을 요구하며(`arguments` 배열이 `command` 문자열보다 우선하고, 래퍼나 빌드 스크립트 대신 지정된 Clang/NASM 바이너리만 실행), 출력 및 의존성 플래그를 빼고 인클루드, 매크로 정의, 타깃, 표준, `++`, `-x` 플래그만 반영합니다. 지원하지 않는 플래그, 응답 파일, 컴파일러 플러그인은 미해결 안내를 남깁니다.

확장에 성공하면 원문 선언 대신 `macro expansion` 태그가 붙은 확장 선언을 채우고 원래 매크로 정의도 함께 유지하며, 호출 관계와 상수 참조, `read`, `grep`은 원래 소스 줄을 그대로 읽습니다. 실패하면 `Macro expansion unresolved` 사유와 함께 원문 추출로 복귀합니다. NASM 확장은 `-Le -Lm -Lf` 옵션이 필요하고(NASM 2.16.03 기준 검증) `..@` 매크로 지역 레이블은 빼며, GNU assembler `.macro` 확장은 지원하지 않습니다.

활성화 상태에서는 네이티브 소스, 헤더, `compile_commands.json`/`compile_flags.txt`가 바뀔 때마다 전체 네이티브 대조가 일어나 기록된 입력이나 더 가까운 빌드 설정 후보가 바뀐 파일만 다시 전처리합니다(감시자는 ignore된 네이티브 헤더와 빌드 DB도 함께 감시). 실패한 파일은 파일 자체, 빌드 설정 후보, 확장 설정이 바뀌거나 새 헤더가 추가될 때 재시도하므로 외부 의존성을 설치한 뒤에는 소스나 DB 파일을 touch하세요. 네이티브 실행은 macOS arm64에서 검증되었으며 Windows와 Linux는 미검증 상태입니다.

### 민감값 마스킹

`[output].is_redact_enabled = true`일 때 MCP 응답(소스 보기, 색인된 리터럴, 선언 및 상수 미리보기, 오류 메시지)에서 탐지된 인증정보와 선택한 PII를 `[REDACTED]`(더 짧은 값은 별표)로 가립니다. 줄바꿈을 깨지 않고 출력 바이트도 늘리지 않습니다.

PII 탐지는 기본으로 꺼져 있습니다. `[output.redact].pii_entities`에 대소문자를 맞춰 정확한 유형 이름을 지정해 켜세요(명시적 `[]`는 기본 인증정보 규칙을 유지한 채 상속된 PII 설정만 끕니다). 전체 90종 목록과 문맥 조건은 [PII 마스킹](/ko/configuration/pii-redaction/)을 참고하세요.

Tree-sitter는 리터럴 대입이나 기본 인수와 변수 참조 및 타입을 구분해 `password: string = externalValue`는 그대로 두고 `password: string = "hardcoded-value"`만 가리며, 문자열 연결과 보간의 정적 부분도 함께 검사합니다. 파싱되지 않은 코드와 ENV/INI 값, 최종 JSON-RPC 텍스트 패스에는 정규식과 필드 규칙을 적용합니다.

내장 규칙 ID는 다음과 같습니다.

| 규칙 ID | 탐지 대상 |
| --- | --- |
| `field.sensitive` | `API_KEY`, `access_token`, `client_secret`, `password` 같은 민감 필드 값 |
| `token.aws-access-key`, `token.github`, `token.openai`, `token.google` | 해당 접두사 패턴의 토큰 |
| `token.slack`, `token.jwt` | Slack 토큰과 JWT 형식 |
| `token.stripe`, `token.gitlab`, `token.npm`, `token.sendgrid` | Stripe 비밀/제한 키와 GitLab, npm, SendGrid 토큰 |
| `credential.authorization`, `credential.bearer`, `credential.url-password` | Authorization Bearer/Basic 헤더, Bearer 값, URL 비밀번호 |
| `private-key.pem` | PEM 개인키 블록(종료 마커가 없으면 파일 끝까지 마스킹) |

사용자 규칙과 정확한 예외 지정 예시:

```toml
[output.redact]
sensitive_fields = ["internalCredential"]
rules = [{ id = "custom.acme", pattern = 'ACME_[A-Z0-9]+' }]
exceptions = [{ rule_id = "custom.acme", value = "ACME_EXAMPLE" }]
```

- `sensitive_fields`는 정규화된 이름과 정확히 일치하는 항목(`internalCredential`은 `internal_credential` 포함)을 잡으며, 내장 필드는 기존 접미사 규칙을 유지합니다.
- 사용자 정규식 `id`는 고유해야 하며 `custom.`으로 시작해 영숫자, `.`, `_`, `-`만 쓸 수 있습니다. 패턴은 Rust `regex` 문법(역참조와 전후방 탐색 제외)을 따르며 `(?P<secret>...)` 캡처 그룹을 넣으면 해당 부분만 마스킹합니다(여러 줄은 `(?s)` 등 지정).
- `exceptions`는 `rule_id`와 **따옴표를 뺀 전체 원문 탐지 값**(이스케이프 시퀀스는 디코딩하지 않음)이 모두 정확히 같을 때만 예외 처리합니다. `custom.acme`에서 `ACME_EXAMPLE`을 빼더라도 `ACME_EXAMPLEPLUS`나 `password = "ACME_EXAMPLE"`(`field.sensitive` 규칙)는 계속 가려집니다.
- 각 목록은 하위 계층을 통째로 대체하며(`저장소 → 전역 → 기본값`), 잘못된 항목이나 빈 문자열에 매칭되는 정규식은 비밀값이나 패턴을 찍지 않고 경고한 뒤 해당 키만 하위 계층으로 넘깁니다.
- 디스크의 원본 파일, 로컬 색인, 검색 순위, 일반 CLI 명령, stderr 로그는 마스킹하지 않습니다. 검색 결과가 하나도 없을 때는 검증되지 않은 비밀 조각이 되돌아가지 않도록 응답에서 질의 문자열을 생략합니다.

### 출력과 호출 관계

`search`는 상위 파일을 자세히 보여 준 뒤 나머지를 간략 꼬리 목록으로 붙입니다. 페이지가 32 KiB를 넘으면 상위 파일마다 일치 사유, 읽을 범위, 일치한 선언, 최대 2줄의 일치 행, 연결된 위치 위주로 압축합니다. 생성 파일과 번역 리소스는 일반 질의에서 순위가 낮아지지만 경로, 심볼, 리소스 키, 인용 구절을 직접 주면 바로 찾을 수 있습니다.

- `output.search.max_bytes`에는 부분 출력 안내 문구까지 포함됩니다(`search`에는 `offset`이 없으므로 질의를 좁히거나 안내된 범위로 `read`하세요). 남은 꼬리 예산이 있으면 [순위 밖 구절 일치](/ko/usage/tools/)가 채워집니다.
- `output.search.anchor_snippet_limit`는 파일당 전체 발췌 수를 제한하며, 그 이후 앵커는 최대 3줄짜리 시그니처만 출력합니다.
- `output.read.max_bytes`는 줄 번호, 문맥, 제목을 포함하고 함수 본문으로 확장된 `grep`도 함께 제한합니다(`limit` 없는 비확장 전체 파일 `read`에는 별도 256 KiB 상한 적용).
- `output.read.max_bytes`를 키워도 검색, 관계 주석(`128 KiB`), 실시간 read/grep 문맥(`16 KiB` 공유 상한), 확장 파싱 제한(`min(max_file_bytes, 4 MiB)`)은 따로 유지됩니다.

`search`의 호출 관계는 `[output.context]`(`caller_context` 인자 생략 시)와 `[output.navigation]`이 제어합니다.

- 기본은 이름 기반 추정치입니다. `[output.navigation].is_enabled = true`를 켜면 import와 지역 바인딩을 검사해 단일 대상을 확인한 뒤 `precise`로 표시합니다(`callsite_budget` 초과 시 이름 스캔으로 전환).
- `[index].store_references`는 파싱 시 호출 외 참조 위치를 색인에 저장하며, 호출 대상 확인에 꼭 필요하지는 않습니다.
- 확정된 호출은 `name — file:line` 형식으로 출력됩니다. `read`와 `grep`은 색인된 상수(JS/TS `const` 바인딩 포함)의 단순 식별자 참조도 `references (same-file constants, approximate)`로 함께 보여 주며, 코드를 실행하지 않고 초기값을 최대 240자까지 미리 보여 줍니다.
- 같은 이름의 정의가 `common_name_threshold`개 이상이면 모호함 경고 라벨을 붙이고, `caller_omit_def_threshold`개 이상이면 추정 호출자 목록 대신 `grep` 힌트를 안내합니다(호출 대상과 구조적으로 확인된 호출자는 유지).

### 테스트 코드 문맥

`should_include_test_code = false`(기본값)일 때 `[output.context.exclude]`는 테스트 파일과 테스트 영역을 `read`/`grep` 자동 심볼 문맥, `search` 호출 관계, 동명 정의 개수 집계, 이벤트 및 소스 경로 분석에서 제외합니다. 물론 테스트 파일에 직접 `read`, `grep`, `search`를 호출해 내용을 보는 것은 가능합니다. 테스트 문맥까지 포함하려면 `should_include_test_code = true`로 바꾸세요(디렉터리와 ignore 제외는 그대로 유지).

`test_file_patterns`와 `test_attributes`, `test_decorators`, `test_calls` 아래의 각 언어 목록(`rust`, `python`, `typescript`, `csharp` 같은 표준 언어 이름)은 지정하는 순간 하위 계층을 **대체**합니다. 언어를 생략하면 상속하고, `[]`를 적으면 해당 언어 규칙을 끄며, `{}`는 모든 언어 항목을 상속합니다.

- `test_file_patterns`: 작업공간 기준 대소문자 구분 glob(`/`가 없으면 어느 깊이에서나 파일명과 매칭; 절대 경로, `..`, 빈 패턴, `!` 부정은 거부).
- `test_attributes`: `#[...]`나 `@`를 뺀 속성 및 어노테이션 glob(Rust `::` 경로는 전체 유지, `cfg(test)`는 `all`/`any`/`not` 조합도 판별하며 `cfg(not(test))`는 보존).
- `test_decorators`: `@`와 인자를 뺀 데코레이터 glob.
- `test_calls`: `test`나 `describe.*` 같은 호출 표현식 glob(호출식과 콜백 본문을 테스트 영역으로 묶음).

규칙들은 OR로 합쳐지며 테스트 영역 안의 중첩 헬퍼까지 적용되지만 그 안에서 부르는 일반 함수까지는 번지지 않습니다. 변경 시 색인 재구축 없이 다음 요청부터 반영됩니다.

일부 기본 규칙만 유지하고 Java 속성과 TypeScript 호출 이름 탐지를 끄는 예시:

```toml
[output.context.exclude]
should_include_test_code = false
test_file_patterns = ["**/tests/**", "*_test.go", "*.test.ts", "checks/**"]

[output.context.exclude.test_attributes]
rust = ["test", "tokio::test", "cfg(test)", "company::case"]
java = []

[output.context.exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "company_test"]

[output.context.exclude.test_calls]
typescript = []
```

내장 기본 목록(표에 없는 언어는 해당 항목 기본값 없음):

```toml
[output.context.exclude]
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

### 파일시스템 권한

`find`, `grep`, `read`에 각각 접근 정책을 지정할 수 있습니다.

- `workspace`: 현재 작업공간 내부만 허용.
- `allowed_roots`: 작업공간과 `allowed_roots`에 적은 비어 있지 않은 외부 루트 허용(상대 경로는 작업공간 루트 기준, 존재하는 경로는 심볼릭 링크를 실제 경로로 풀어 `..` 탈출 차단).
- `anywhere`: 서버 프로세스가 접근할 수 있는 모든 경로 허용(기본값).

`allowed_roots`를 지정해도 시작 폴더의 색인에 외부 트리가 추가되지는 않습니다. 외부 저장소를 `workspace_scope`로 `search`할 때는 `read` 정책이 그 저장소 폴더 전체를 허용하는 경우에만 자체 색인을 만들고 쓰며, 외부 저장소의 `[filesystem_permissions]`가 현재 세션 권한을 넓히지는 않습니다.

### 색인 갱신

`watch = true`이면 파일 편집을 `watch_debounce_ms` 간격으로 모아 백그라운드에서 색인을 갱신합니다(`.gitignore`, `.ignore`, `.codemapignore`로 숨긴 경로의 변경은 무시하되 해당 ignore 파일이 바뀌면 전체 스캔). 감시를 끄거나 쓸 수 없으면 `search`가 `index_staleness_ms` 주기로 백그라운드 갱신을 요청하면서 마지막 커밋 스냅샷을 즉시 돌려줍니다.

`indexer_auto_restart = true`이면 백그라운드 색인기가 멈췄을 때 다음 `search`가 제한 횟수 안에서 자동 재시작하며, `false`이면 재시작할 때까지 검색 결과가 고정됩니다(어느 쪽이든 실시간 `read`/`find`/`grep`은 계속 작동). 내용 변경 없이 수정 시간만 바뀐 파일은 색인 폴더 안 `content-mtimes.json`에 기록해 불필요한 재파싱을 건너뜁니다.

## `config.toml` 예시

자주 조정하는 항목 위주로 구성한 실용적인 `<repo>/.codemap/config.toml` 예시입니다(전체 69개 키와 기본값은 위 [설정 키](#설정-키) 표 참고).

```toml
# codemap-config-version: 32
# 바꿀 키만 남기세요. 생략한 키는 전역 설정이나 내장 기본값을 따릅니다.

[output]
is_redact_enabled = true
# max_bytes = "1mb"

[output.client]
claude_max_result_chars = 100000
codex_output_token_limit = 100000
pi_max_bytes = 100000
opencode_max_bytes = 100000

[output.search]
detail_file_limit = 24
max_bytes = "1mb"

[output.read]
max_bytes = "5mb"
is_outside_callers_enabled = true

[output.redact]
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD"]
sensitive_fields = ["internalCredential"]

[output.jev]
enabled = false
scope = ["search", "read", "grep"]
model = "jev-1.14.0"

[index]
# mode = "auto"           # "auto", "on", "off" (변경 후 재시작 필요)
max_file_bytes = "1mb"
store_references = false

[index.exclude]
excluded_directories = [".git", ".codemap", ".codemap-index", "**/node_modules", "**/target"]
use_git_exclude = true

[index.language_support]
is_document_support_enabled = false
is_shell_support_enabled = false

[analysis]
# target_os = "linux"     # ""을 적으면 상속된 전역 target_os 해제

[filesystem_permissions]
find = "anywhere"
grep = "anywhere"
read = "anywhere"
allowed_roots = []

[related_terms]
enabled = false
```

### 작업공간 전용 권한

세 도구를 모두 작업공간 안에만 묶습니다.

```toml
[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"
allowed_roots = []
```

### 지정한 외부 루트 허용

`find`와 `grep`은 공유 외부 소스 트리를 볼 수 있게 하고 `read`는 작업공간으로 제한합니다.

```toml
[filesystem_permissions]
find = "allowed_roots"
grep = "allowed_roots"
read = "workspace"
allowed_roots = ["G:/shared/source", "D:/vendor-src"]
```

### `find`만 전체 디스크 접근

`find`만 `anywhere`를 유지하고 `grep`과 `read`는 작업공간에 제한합니다.

```toml
[filesystem_permissions]
find = "anywhere"
grep = "workspace"
read = "workspace"
allowed_roots = []
```

## `.codemap/`과 무시 파일

- `.codemap/index/`와 `.codemap/config.toml`은 저장소의 `.codemap/` 디렉터리에 함께 있습니다. `codemap-search`는 `.codemap/`을 탐색에서 항상 빼며 Git 파일을 직접 고치지 않으므로, `git status`에서 감추려면 `.gitignore`나 `.git/info/exclude`에 `.codemap/`을 직접 추가하세요.
- 저장소의 `.codemapignore`는 **gitignore 문법**으로 색인, `find`, `grep`에서 감출 경로를 `.gitignore`와 별개로 추가합니다.

## read·grep 요청별 출력 제어

알려진 여러 구간을 한 번에 읽으려면 최상위 `file_path`(및 별칭) 대신 `read`에 `ranges: [{file_path, offset?, limit?}, ...]`(1~20개)를 넘기세요. 항목마다 단일 `read`와 같은 경로 권한, 확장자 확인, 바이트 한도, 별칭 우선순위(`file_path > path > file > query`; `offset`/`limit > start_line`/`end_line > start`/`end`, camel/kebab 표기 및 문자열 숫자 허용)가 적용됩니다. 기본값은 `offset`이 `1`, `limit`이 EOF이며 최상위 `offset`/`limit`는 상속하지 않습니다.

공통 `view`, `expand`, `unresolved`, `include_events`, `debug`, `include_seen` 옵션은 최상위에 적으며 항목 내부의 출력 옵션은 무시합니다. 항목 하나가 실패해도 그 자리에 오류를 적고 나머지는 계속 읽습니다. 전달 이력은 같은 연결의 앞선 항목 및 다른 도구 호출과 공유되며(`include_seen=true`는 중복 제거만 건너뜀), 묶음이 출력 상한에 걸리면 처음 미룬 항목의 1부터 시작하는 범위 번호와 0부터 시작하는 `ranges` 인덱스를 안내합니다.

아래 옵션은 TOML 설정이 아닌 요청 인자입니다(`search.caller_context`와는 별개).

| 인자 | 기본값 | 결과 |
| --- | --- | --- |
| `view` | `"full"` | `full`: 심볼 + 원문, `source`: 색인 문맥과 관계 없이 실시간 원문만 출력, `definitions`: 선언만 출력, `relations`: 원문 없이 대상/소유자 식별자와 호출/상수 참조만 출력 |
| `unresolved` | `"list"` | `count`는 미해결 총합만 출력, `list`는 제한된 이름 목록까지 출력(`full`/`relations` 적용) |
| `expand` | 본문 모드 `grep`: `"callable"`, `read`: `"none"` | `callable`은 실시간 UTF-8 버퍼에서 감싸는 이름 있는 함수 경계까지 확장, `none`은 일치 행이나 요청 줄 범위 유지 |

`read`에서 `expand="callable"`은 유효한 `offset`/`start` 줄을 기준으로 `limit`/`end`를 덮어쓰며, 지원하는 함수가 없으면 안내와 함께 원래 범위를 반환합니다(파싱 상한 `min(max_file_bytes, 4 MiB)`). 붙어 있는 속성/데코레이터를 포함하고, 중첩 함수는 가장 안쪽 경계를, 익명 클로저는 바깥쪽 이름 있는 함수를 고릅니다. 복합 파일, 문법 오류, 본문 없는 프로토타입, 다른 코드와 줄을 공유하는 함수는 지원 불가 안내를 돌려줍니다.

본문 모드 `grep`에서는 `source`를 포함한 모든 보기에서 `expand="callable"`이 기본입니다. `-A/-B/-C`는 무시하고 패턴, 경로, glob, 대소문자, 유형, 제외 필터를 그대로 적용합니다. 페이지 이동(`offset`, `head_limit`, `next_offset`)은 소스 줄이 아니라 고유 함수 또는 대체 묶음 단위로 세며, `head_limit=0`은 묶음 수 제한만 끌 뿐 바이트 한도는 유지합니다. 일치 행이나 줄 단위 페이지 이동, `-A/-B/-C` 문맥이 필요하면 `expand="none"`을 쓰세요. `count`와 `files_with_matches` 모드는 생략 시 확장하지 않으며 명시적 `expand="callable"`은 오류를 냅니다. 한도를 넘는 큰 함수는 조용히 자르지 않으므로 안내된 줄 범위를 `expand="none"`으로 다시 요청하세요.

## Rust 분석 대상 지정

```toml
[analysis]
target_os = "macos"
```

`target_os`는 실행 호스트에서 추론하지 않습니다. 저장소 설정이 전역 설정을 덮어쓰며 `""`은 상속값을 지우고, 잘못된 식별자는 경고 후 하위 계층으로 넘어갑니다. 설정을 다시 읽으면 색인을 다시 만들지 않고 다음 요청부터 새 조건 해석기가 적용됩니다(응답 하나 안에서는 일관된 스냅샷 사용).

Rust 해석기는 `target_os = "value"`, `all(...)`, `any(...)`, `not(...)`, 불리언 리터럴을 3치 논리로 평가합니다(`not(target_os="macos")`는 명시한 다른 모든 대상에 참이 되지만, 대상 미지정, 다른 키/플래그, `cfg_attr`는 미확정으로 남고 단독 `cfg(test)`는 `[output.context.exclude]`에 위임). 자세한 문법은 [Rust 조건부 컴파일 문서](https://doc.rust-lang.org/reference/conditional-compilation.html)를 참고하세요.

import, 재수출, 선언, 호출 위치, 부모 모듈 조건을 모두 확인해야 대상을 `precise`로 표시합니다. 구조로 확인된 호출자는 `caller_omit_def_threshold`에 걸리지 않고 남으며, 별칭 탐색(최대 16단계, 256개 이름)은 후보 필터로 쓰여 각 링크마다 원문 확인을 거칩니다.

## 선택적 Jev 판단 단계

Jev 필터링은 decision 모델(TypeSafe Jev 또는 OpenRouter)로 `search`, `read`, `grep` 응답에서 현재 작업과 관련 없는 함수 본문을 걸러냅니다.

```toml
[output.jev]
enabled = true
scope = ["search", "read", "grep"]
search_filter_min_unrelated_probability = 0.70
```

API 키는 [`auth.toml`](#인증정보-authtoml)에 넣거나, 두 auth 파일에 키가 없을 때 환경 변수를 쓰세요.

```sh
export TYPESAFE_API_KEY="<발급받은 키>"
export OPENROUTER_API_KEY="<발급받은 키>"
```

TypeSafe 대신 OpenRouter로 보내려면 `author/slug` 형식의 모델 ID를 지정합니다.

```toml
[output.jev]
enabled = true
model = "openai/gpt-6-luna-decisions"
```

Jev는 기본으로 꺼져 있습니다. `enabled = true`일 때 `scope`에 넣은 도구(`search`, `read`, `grep`; 중복 항목은 무시, 알 수 없는 항목은 전체 목록을 거절하고 하위 계층 상속, `[]`는 미적용)만 필터링하며 `initial_instructions`, `find`, `analyze`, CLI 명령은 Jev를 부르지 않습니다.

### 작업 전달

`enabled = true`이고 `scope`가 비어 있지 않으면 `tools/list`가 `initial_instructions`에 필수 문자열 파라미터 `task`를 노출합니다(스키마 표시를 보려면 MCP 재연결 필요, 서버는 어느 쪽이든 `task` 수용). 필터링된 도구를 부르기 전에 사용자 요청 원문을 그대로 넘기고 작업이 바뀌면 다시 호출하세요.

```json
{ "task": "응답 바이트 상한은 어디에서 적용되는가?" }
```

| 필드 | 계약 |
| --- | --- |
| `task` | 비어 있지 않은 문자열, 최대 65,536바이트(사용자 요청 원문 그대로) |

`task`는 연결 단위로 유지되며 `initialize` 때 초기화됩니다. `task` 없이 다시 부르면 기존 작업을 유지하고, 다른 작업을 넘기면 중복 제거 이력을 새로 시작하며, 빈 문자열이나 문자열이 아닌 값을 넘기면 `isError: true` 오류를 반환하고 이전 작업을 비웁니다. 필터가 켜졌을 때 `task`가 없으면 인수 오류가 나지만, API 키가 없거나 평가가 실패하면 일반 필터링 전 결과를 그대로 돌려줍니다. 활성화된 필터 도구는 `openWorldHint: true`(`readOnlyHint: true` 유지)를 광고합니다.

생략 안내가 가리킨 정확한 범위를 `read`하거나 한 함수 안에 꼭 들어오는 범위를 읽을 때는 재판정 없이 본문을 돌려주며, 더 넓은 범위는 평소처럼 판정합니다. `include_seen=true`는 공통 중복 제거만 우회합니다.

### 질문 판단과 근거

판정 대상 단위마다 서버가 생성하는 고정 예/아니오 관련성 질문(코드가 작업에서 언급한 내용이나 그 경로가 거치는 큐, 버스, 저장소, 감시자, 레지스트리 같은 공용 기반을 다루는지 여부)을 보냅니다. 모든 모델이 같은 필드(파일, 종류, 이름, 마스킹된 소스)를 받으며 요청 형식만 제공자에 따릅니다.

| 모델 | 요청 구성 |
| --- | --- |
| TypeSafe Jev(`jev-1.14.0` 등) 또는 OpenRouter `typesafe/*` | 작업과 후보 묶음(`[b3]` 등)을 담은 일반 텍스트 state로 연속 후보를 한 배치에 묶어 전송 |
| OpenRouter `openai/*` | 후보마다 별도 요청을 보내 한 질문의 OpenAI 거부가 다른 후보까지 실패시키지 않도록 분리 |
| 그 밖의 OpenRouter decision 모델 | state에 마스킹된 `task_query`만 담고 각 질문에 후보를 JSON 객체로 첨부 |

`search_filter_min_unrelated_probability`(기본 `0.70`, 잠정값)는 본문을 빼는 데 필요한 최소 "아니오" 확률이자 후속 확장의 시드로 삼는 최소 "예" 확률의 대칭 임계값입니다. 그 사이 점수는 불확실로 보고 본문을 남깁니다.

부분 본문, 오래된 본문, 크기 초과 본문, 마스킹 불능, 심볼 식별이 확인되지 않은 본문과 불확실 판정은 모두 유지하며, 비호출 선언과 소스 위치, 읽기 안내도 남깁니다. 생략 안내는 [읽기 동작 지표](/ko/usage/cli-and-analysis/#횟수와-바이트-해석)의 전달 원문 바이트에 들어가지 않습니다.

호출당 배치는 단일 마감 시간 안에 병렬 실행되며 `max_batch_bytes`와 운영 토큰 추정치(`state` + 최장 질문 `28,000`토큰, `state` + 전체 질문 `56,000`토큰, 즉 32k/64k 상한 대비 12.5% 여유)를 따릅니다. 단일 단위가 너무 크면 판정 없이 남기고, 배치가 실패하거나 답변이 빠지면 도구 전체 기본 결과를 보존합니다. 정확한 `event_key` 조회는 Jev를 거치지 않습니다.

### 렌더링 전 선택

`search`는 일반 예산 안에서 순위 후보를 먼저 계획한 뒤 Jev로 거릅니다. 일치 본문은 직접 연결된 보조 본문을 보호하고(포함 본문은 겹치는 영역을 보호) 불확실하거나 작은 본문으로 전파하지는 않습니다. 생략 안내보다 크지 않은 고립 본문과 무조건 유지되는 상위 본문 안의 본문은 평가를 건너뜁니다.

각 본문의 마스킹된 소스만 전송합니다(관계 주석, 이벤트, 호출자 코드는 제외). 필터링이 성공하고 색인이 정상이면서 기본 결과가 잘리지 않고 `caller_context != false`이면, 긍정 일치 함수당 최대 8개(4 KiB 주석 예산 및 탐색 한도 안)의 후속 색인 호출 위치를 붙이고 인접 후보 함수도 최대 8개까지 판정해 관련 함수에 `(Jev: related)`를 붙여 먼저 보여 줍니다.

생략 안내는 정확한 `read` 좌표를 남기며 생략된 본문의 개요 행과 관계 안내를 함께 뺍니다(모든 본문이 빠진 파일은 제목과 생략 안내만 유지). 클라이언트 전달 한도가 빡빡하면 Jev가 보호한 본문을 가장 나중에 미루고 빈 공간을 채우지 않습니다. 평가 전 `jev candidate evidence` 로그는 소스와 작업 텍스트 없이 마스킹된 경로, 범위, 크기, 해시만 기록합니다.

### read·grep의 실제 원문 판정

`read`의 `full`/`source` 보기와 `grep`의 `source_grouped` 보기에서는 도구가 읽은 실시간 버퍼의 단위를 그대로 판정합니다.

- 완전히 반환된 함수 또는 메서드 본문(`grep`에서는 일치 행이 나온 함수 본문 전체)
- `read` 창이 중간을 자른 함수 또는 메서드의 반환 행
- 함수 밖에 연속해 놓인 최상위/동일 선언 소속 행 묶음(앞뒤 빈 줄 제외)

열 잘림 행, 비연속 행, 문법과 심볼 식별이 확인되지 않은 단위, 마스킹 불능 구간, 크기 초과 단위는 그대로 남기며 `definitions`/`relations` 보기와 비본문 `grep` 모드는 Jev를 건너뜁니다.

생략된 본문은 `_omitted body:`(`read`) 또는 `_omitted matches:`(`grep`), 생략된 조각이나 행 묶음은 `_omitted lines: L<시작>-<끝> (<이름>)` 안내로 바뀝니다. 한 파일의 인접 생략은 최대 3개 이름과 `+N more`를 담은 하나의 안내로 합칩니다. 같은 컨테이너의 멤버/콜백 연결 최대 8개가 보조 근거로 쓰이며, 생략으로 빈 바이트를 다음 페이지로 채우거나 디스크에서 바뀐 버퍼를 섞지 않습니다.

### 제공자에 전송하는 데이터

활성화된 필터는 마스킹된 작업 텍스트와 판정 단위의 마스킹된 소스, 경로, 종류, 이름을 HTTPS로 TypeSafe 또는 OpenRouter(및 업스트림 모델 제공자)에 보냅니다. 전송 전 마스킹이 먼저 돌지만 등록되지 않은 사용자 정의 비밀 패턴은 빠져나갈 수 있습니다. API 키는 HTTPS 인증 헤더에만 실립니다.

### OpenRouter decision 모델

`[output.jev].model`에 `author/slug`를 넣으면 `[openrouter].api_key`(`OPENROUTER_API_KEY` 대체)를 써서 `POST https://openrouter.ai/api/alpha/decisions`로만 요청을 보냅니다.

- **decision 모델 전용**: 채팅 모델이나 알 수 없는 모델은 사전 HTTP 400(`http_status` fallback)으로 거부되고 도구는 일반 출력을 돌려줍니다.
- **고정 ID만 허용**: OpenRouter는 `:variant` 접미사를 뺀 정식 slug(선택적 `-YYYYMMDD` 날짜 접미사 포함)로 응답해야 검증을 통과합니다. 별칭(`~author/slug`)은 전송 전에 거절합니다.
- **컨텍스트 한도**: 운영 기본값은 `64k` 이상 컨텍스트를 가정합니다. OpenRouter에서 약 32 KB가 넘는 요청은 더 느린 long-context 모드로 돌아가므로 필요하면 `max_batch_bytes`를 낮추세요.

`2026-10-24` 기준 `64k` 이상 컨텍스트로 확인된 OpenRouter decision 모델:

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

(`scaledown/classify-1`은 OpenRouter JSON 구조의 객체 `state` 필드를 거부합니다.) 최신 목록은 `GET https://openrouter.ai/api/v1/models?output_modalities=decisions`로 조회하세요.

### 전송 한도

| 설정 또는 한도 | 계약 |
| --- | --- |
| `max_in_flight_requests` | 동시 요청 1~8개 |
| `request_spacing_ms` | 최소 100ms 간격 |
| `max_batch_bytes` | 인코딩 기준 1~168,000바이트(더 작게 명시한 값 우선) |
| 토큰 추정치 | 운영 기본값: `state` + 최장 질문 `28,000`, `state` + 전체 질문 `56,000` (`ceil(인코딩 바이트 / 3)`) |
| `timeout_ms` | 양수, 최대 7일(준비와 모든 배치를 합친 단일 마감 시간) |
| `pool_idle_timeout_ms` | 양수, 최대 7일 |
| 배치/응답 한도 | 단계당 최대 8,192개 질문 및 128개 배치, HTTP 응답당 4 MiB |
| 재시도/리다이렉트 | 없음 |

각 단계는 시작 시점의 설정 스냅샷을 고정하므로 재로드된 설정은 다음 요청부터 반영되고 진행 중인 마감 시간을 늘리지 않습니다.

### 결과와 진단

| 결과 | 의미 |
| --- | --- |
| `applied` | 평가 완료(보존 규칙에 따라 모든 본문이 남을 수도 있음) |
| `bypassed` | 건너뜀(예: API 키 없음, 적격 본문 없음, 색인 워밍 중, 공간 부족) |
| `fallback` | 평가 실패(마감 초과, HTTP 또는 속도 제한 오류, 잘못된 응답, 판정 결과 매핑 누락) |

건너뛰거나 실패하거나 모두 유지된 경우 일반 출력과 바이트 단위로 같은 결과를 반환하며, 필수 `task` 누락 시에만 인수 오류가 납니다. 실행할 때마다 `stderr`에 `info` 수준의 `jev stage` 로그 한 줄(`codemap_search::mcp::jev`)로 개수, 토큰, 시간만 남기며 작업 텍스트, 소스, 자격 증명, 오류 본문은 절대 기록하지 않습니다.

Rust에서 직접 쓰려면 [`codemap_search::jev`](https://github.com/buYoung/mcp/blob/main/apps/codemap-search/src/jev/mod.rs)와 [판단 예제](https://github.com/buYoung/mcp/blob/main/apps/codemap-search/examples/jev_decisions.rs)(`--mock`은 오프라인, `--live`는 API 요청 실행)를 참고하세요.

## 이벤트 관계 탐색

이벤트 탐색은 기본으로 켜져 있으며 핸들러나 빌드 스크립트를 실행하지 않고 색인 세대마다 불변 이벤트 지도를 함께 만듭니다.

```toml
[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = []
```

18개 언어의 데이터 저장 및 소비 간선인 `Source routes`도 설정된 이벤트와 함께 자동으로 나오며 콜백 후보, 인자 전달, 데이터 소비자를 구분합니다. `is_enabled = false`나 `include_events = false`는 둘 다 끄고, `use_builtin_rules = false`와 `rules = []`는 규칙 기반 이벤트만 끕니다(`event_key`는 이벤트 지도만 조회). 언어별 상세 계약은 [소스 경로 탐색 계약](https://github.com/buYoung/mcp/blob/main/apps/codemap-search/docs/source-routes.ko.md)을 참고하세요.

| 요청 | 동작 |
| --- | --- |
| `read` / 본문 `grep` (`include_events` 생략 또는 `true`) | `view=full` 또는 `view=relations`에서 반환 줄이나 지원 정의와 맞닿은 이벤트를 자동 표시(없으면 빈 안내나 예산 차감 없음; `source`/`definitions`는 생략) |
| `search` (`include_events` 생략 또는 `true`) | 순위 결과 경로의 관련 이벤트를 `caller_context`와 무관하게 자동 첨부 |
| `read` / `grep` / `search` (`include_events: false`) | 해당 요청의 자동 이벤트 문맥 숨김 |
| `search` (`event_key: "saved"`) | 순위 검색 대신 정확한 이벤트 키 조회(`query` 필수, `workspace_scope` 적용) |

이벤트 라우팅은 단순 `on`/`emit` 문자열 일치가 아니라 색인된 API와 import 근거를 바탕으로 발행자, 등록 지점, 핸들러를 정적 등록 조건과 함께 분리합니다(런타임 실행 순서나 횟수는 미보장). 내장 규칙은 Node `EventEmitter`(`events`/`node:events` 이름 지정 ESM import), Tauri 프런트엔드 이벤트(`@tauri-apps/api/event`), Tauri Rust `AppHandle`/`App`/`Window`/`WebviewWindow`/`Webview`(`Emitter`/`Listener`) 호출을 다룹니다.

버스 식별은 지원하는 상대 ESM import, 별칭, 재수출을 따라 추적된 불변 모듈 할당을 요구하며 가변 지역 변수, 매개변수, 동적 키, 런타임 DI, 외부 브로커는 미해결로 남습니다. Tauri 프런트엔드 버스는 가장 가까운 색인된 `src-tauri/tauri.conf.json`이나 `package.json` 경계로 범위를 제한하고 정적 타깃과 채널이 정확히 일치해야 연결합니다.

### 사용자 규칙

`src/known.ts`의 `export class KnownBus`에 대한 사용자 규칙 예시:

```toml
[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = [
  { id = "known-on", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "on", role = "subscribe", event_arg = 0, handler_arg = 1, bus = "receiver" },
  { id = "known-emit", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "emit", role = "publish", event_arg = 0, bus = "receiver" },
]
```

규칙 선택자에는 정확한 `language`(`typescript`, `javascript`, `rust`), `module`(작업공간 상대 파일 경로 또는 지원되는 import 모듈), `symbol`, 선택적 `method`를 지정해야 하며 같은 선택자의 내장 규칙을 덮어씁니다. 최대 64개까지 등록할 수 있고(인자 인덱스 `< 16`, 식별자/타깃 `<= 256`바이트, 비어 있지 않고 줄바꿈/NUL 없는 키 `<= 256`바이트), 중복이나 와일드카드, 잘못된 필드가 있으면 전체 목록을 거부합니다.

| 필드 | 계약 |
| --- | --- |
| `role` | `publish`, `subscribe`, `unsubscribe` |
| `event_arg` / `event_key` | 둘 중 하나 필수: 0부터 시작하는 키 인자 인덱스 또는 고정 이벤트 키 문자열 |
| `handler_arg` | 0부터 시작하는 콜백 인자 인덱스(`subscribe` 시 필수) |
| `bus="receiver"` | `method`와 불변 수신자 객체로 확인된 할당이 필요 |
| `bus="argument"`, `bus_arg` | 지정한 인자 인덱스에서 버스 할당 읽기 |
| `bus="fixed"`, `bus_identity` | 명시적 공용 버스 가정(예: Rust 래퍼와 JS 프런트엔드 연결 시 설정 가정으로 표기) |
| `bus="framework"` | 색인된 앱 경계 안의 인식된 Tauri 프런트엔드 API에만 허용 |
| `target_arg` / `target` | 선택적 정적 타깃 인자 인덱스 또는 `any` 같은 고정 한정자(상호 배타, Tauri 프런트엔드에는 고정 타깃 불가) |
| `channel` | 선택적 정확한 채널 이름(기본 `default`) |
| `is_once` | 일회성 등록 여부를 표시하는 선택적 불리언 |

예를 들어 `src/shared/output/mod.rs`의 Rust 래퍼 `dispatch_event_json(key, payload)`에 `bus="fixed"`, `bus_identity="application-events"`, `target="any"`를 주고 프런트엔드 규칙에 같은 ID를 맞추면, 두 지점의 연결이 사용자 설정 가정이라는 표식과 함께 표시됩니다.

### 갱신과 상한

입력은 UTF-8만 허용하며 색인, 디렉터리, Git, 테스트 문맥 제외 규칙을 따릅니다. 테스트 문맥, 규칙, 대상 OS, 제외 설정을 바꾸면 새 세대로 갱신하며 수정된 근거 파일의 오래된 끝점은 즉시 숨깁니다.

추출과 질의 한도는 파일당 `512 KiB`, 스냅샷당 `64 MiB` 및 `4,096`파일, 파일당 `256`개와 스냅샷당 `8,192`개 끝점(Source routes와 공유), 끝점 직렬화당 `8 KiB`, JS/TS 바인딩 32단계(Rust 값 재귀 16단계)입니다. 질의당 후보 `512`개, 렌더링 `128`개, 검증 파일 `128`개(`4 MiB`)로 제한하며, 잘림이나 오래된 근거, 제외 사실은 출력에 보고하고 질의 때문에 작업공간을 다시 전체 스캔하지는 않습니다.

## 검색 표시 압축과 grep 원문 묶음

`search`는 Jev 근거를 모은 뒤 표시 주석을 압축합니다. 비호출 참조는 미리보기 없이 파일과 행 위치만 유지하고, 미해결 호출 이름은 한 줄로 묶으며, 반복되는 주의 문구는 `[N1]` 형식으로 한 번만 정의해 이후 참조로 대체합니다. 전체 본문 안에 이미 보이는 리터럴 미리보기도 생략되며, 확정된 관계 위치와 소스 본문은 그대로 유지됩니다.

`grep`에서 `view="source_grouped"`를 쓰면 파일당 하나의 제목 아래에 소스 행을 묶어 보여 줍니다(`expand="none"`의 `42:match`/`43-context` 표기와 `expand="callable"`을 모두 지원하며 선언과 관계 분석은 건너뜀). 기존 행별 경로 형식의 `view="source"`는 그대로 유지되며 `source_grouped`는 본문 모드 `grep` 전용입니다.

## 이전 설정 이름

이전 루트 키와 섹션 이름도 계속 읽힙니다. 같은 파일 안에서는 표준 이름이 우선하고 잘못된 표준 값은 하위 계층으로 넘어갑니다. 저장소 설정만 자동 이전되며 전역 파일은 수정하지 않습니다.

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
