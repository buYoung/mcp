# personal-mcp

코딩 에이전트들 사이의 페어 프로그래밍 브리지.

이 monorepo는 두 개의 독립 stdio MCP(Model Context Protocol) 서버를 제공한다.

첫째는 ACP(Zed Agent Client Protocol) client 역할을 하는 페어 브리지
(`@buyong-mcp/acp-bridge`)다. 즉, 호출하는 에이전트
(예: Claude Code)는 MCP 도구처럼 다른 코딩 에이전트(Codex, Gemini CLI 등)를
부를 수 있고, 내부적으로는 그 에이전트를 ACP 서버 프로세스로 띄워서 대화를
중계한다.

둘째는 zoekt + Universal Ctags 기반 코드 탐색 서버 `scout`(`@buyong-mcp/scout`)로,
코드 검색·읽기 primitive를 코딩 에이전트에 노출한다(`apps/scout/DESIGN.md` 참조).

## 가설

코딩 에이전트마다 강점이 다르다. 사람 페어 프로그래밍에서 driver/navigator가
역할을 바꾸듯, 에이전트도 서로에게 의견을 구하면서 작업하면 단일 루프보다
견고한 결과가 나온다.

## 구조

```
apps/
  mcp-server/                # ACP 페어 브리지 MCP 서버 (@buyong-mcp/acp-bridge)
    src/
      index.ts               # 엔트리
      tools/                 # MCP 도구 표면 (list_agents, ask_pair, continue_pair, consult_panel, close_pair)
      agents/                # 에이전트 어댑터 + registry
        common/              # 공통 AgentAdapter + ACP 어댑터 생성기
        claude-code/         # Claude Code ACP 설정
        codex/               # Codex ACP 설정
        gemini-cli/          # Gemini CLI ACP 설정
      acp/                   # ACP client wrapper
  scout/                     # zoekt + ctags 코드 탐색 MCP 서버 (@buyong-mcp/scout, search_text 출하 — DESIGN.md)
packages/
  typescript-config/         # 공유 tsconfig (base, node)
```

에이전트별로 따로 떼어내야 할 만큼 커지면 그때 `packages/agents/*`로 분리한다.

## 개발

```bash
pnpm install
ACP_BRIDGE_PROMPT_TIMEOUT_MS=600000 pnpm dev                                  # turbo dev (전체)
ACP_BRIDGE_PROMPT_TIMEOUT_MS=600000 pnpm --filter @buyong-mcp/acp-bridge dev      # MCP 서버만 watch
ACP_BRIDGE_PROMPT_TIMEOUT_MS=600000 pnpm --filter @buyong-mcp/acp-bridge inspect  # MCP Inspector로 디버깅 UI 띄우기
pnpm check-types
pnpm build
pnpm lint                                 # Biome check
pnpm format                               # Biome check --write
```

## 에이전트 설정

기본 페어 후보는 `claude-code`, `codex`, `gemini-cli` 세 개다. `list_agents`로 후보의
`agent_id`를 조회하고, `ask_pair`에 `agent_id`와 `main_agent_position`(필수)을 넘겨
새 세션을 만든 뒤, 반환된 `session_id`를 `continue_pair`에 넘겨 같은 후보와 이어서
대화한다. 여러 후보에게 동시에 묻고 싶으면 `consult_panel`에 `agent_ids`를 배열로
넘긴다. `consult_panel`은 후보별로 별도 child process를 띄우므로 비용이 후보 수만큼
선형 증가한다.

`ask_pair`/`consult_panel`은 호출하는 에이전트가 자기 **잠정 입장**을 결정한 뒤에만
사용해야 한다. `main_agent_position`이 비어 있으면 호출은 거절된다 — 페어에게 결정을
떠넘기는 인지 위탁(cognitive offloading)을 막기 위함이다.

페어가 메인 에이전트의 컨텍스트 요약에만 의존해 메아리방이 되는 것을 막으려면
`files`(절대 경로 문자열 배열)에 페어가 직접 읽어야 할 파일들을 명시한다. 페어는
read-only 권한만 가지므로 수정은 불가능하다.

페어 응답은 원문 `answer`와 함께 `structured_opinion`, `meta`를 반환한다.
`structured_opinion`은 `stance`(`agree`/`disagree`/`partial`/`insufficient_info`),
`summary`, `agreements`, `concerns`, `recommendation`, `follow_up_questions` 키를
가진다. JSON 파싱에 실패하면 한 번 재요청한 뒤에도 실패 시 `parse_status`가
`fallback`인 구조로 `raw_answer`만 보존되고, `recommendation`은 빈 문자열로 둔다
(메인이 fallback 응답을 권장사항으로 오인하지 않도록).

`meta`에는 `elapsed_ms`, `stop_reason`, `agent_id`, 가능하면 `agent_model`이 들어가
메인이 cost/latency를 인지할 수 있게 한다.

`user_request`는 인자로 유지하되 정규식 게이트는 두지 않는다. 클라이언트가 MCP
elicitation을 지원하면 첫 호출 시 사용자에게 명시 확인을 받는다(프로세스 lifetime
동안 한 번). 미지원 클라이언트에서는 차단하지 않고 stderr에 `[acp-bridge]
pair-consult invoked: ...` 한 줄만 남긴다.

상담이 끝나면 `close_pair`로 세션을 닫는다. 서버는 유휴 세션을 30분 뒤 정리하고, 동시에
최대 20개 세션만 유지한다. 같은 `session_id`에 대한 후속 요청과 종료 요청은 순서대로 처리한다.

페어 child process는 풀링하지 않는다(cold-start 정책). 매 `ask_pair`/`consult_panel`
호출마다 새 child process를 띄우므로, 단발성 의견 조회보다 `continue_pair`로
같은 세션을 이어 쓰는 편이 비용 면에서 유리하다.

MCP 서버가 초기화될 때 현재 작업 디렉터리에 `.acp_bridge/config.toml`을 만든다. 파일이
이미 있으면 덮어쓰지 않는다. 빈 문자열이면 해당 어댑터의 기본값을 사용한다.
`permission`은 이전 설정 파일과의 호환을 위해 파싱하지만 읽기 전용 동작을 바꾸지 않는다.

```toml
[agents.claude-code]
model = ""
permission = ""
reasoning = ""

[agents.codex]
model = ""
permission = ""
reasoning = ""

[agents.gemini-cli]
model = ""
permission = ""
```

전달 경로는 어댑터마다 다르다.

- `claude-code`: `model`, `effort` 설정 옵션을 전달하고, 권한 모드는 항상 `plan`으로 고정한다.
- `codex`: `model`, `reasoning_effort` 설정 옵션을 전달하고, 권한 모드는 항상 `read-only`로 고정한다.
- `gemini-cli`: `model`만 세션 모델로 전달한다. Gemini ACP 경로에는 `reasoning`이 없으므로 설정 파일에 `reasoning` 값을 넣으면 초기화가 실패한다.

`claude-code`와 `codex` 어댑터 실행 파일은 이 패키지의 `node_modules/.bin`에서
자동으로 찾는다. 전역 설치 위치가 다른 실행 파일을 쓰고 싶을 때만 아래 환경 변수로
덮어쓴다. `*_ARGS` 값은 JSON 문자열 배열이어야 한다.

```bash
ACP_BRIDGE_CLAUDE_CODE_COMMAND=/path/to/claude-agent-acp
ACP_BRIDGE_CLAUDE_CODE_ARGS='[]'

ACP_BRIDGE_CODEX_COMMAND=/path/to/codex-acp
ACP_BRIDGE_CODEX_ARGS='[]'

ACP_BRIDGE_GEMINI_CLI_COMMAND=gemini
ACP_BRIDGE_GEMINI_CLI_ARGS='["--acp"]'
```

`ACP_BRIDGE_PROMPT_TIMEOUT_MS`는 필수이며 양의 정수 밀리초 값이어야 한다. 예를 들어
`600000`은 페어 후보의 한 응답 턴을 10분까지 기다린다.

ACP 도구 실행 권한은 항상 읽기 전용으로 동작한다. `read`, `search`, `fetch`, `think`
권한 요청만 허용하고 파일 수정, 삭제, 이동, 명령 실행, 모드 전환 요청은 거절한다.
이전 호환을 위해 `ACP_BRIDGE_PERMISSION_POLICY` 값이 있어도 읽지만 동작에는 반영하지 않는다.

## scout 설정

scout는 검색 대상 레포지토리를 인덱싱할 때 해당 레포 안에 `<repo>/.scout/` 디렉터리를
만들고 그 아래 `zoekt/`(인덱스 샤드와 메타데이터 등)를 둔다. 산출물이 레포 안에 생기긴
하지만 git 워킹트리를 더럽히지 않도록, scout는 부팅 시 `<repo>/.scout/`를 그 레포의
`.git/info/exclude`에 자동 등록한다(멱등 — 이미 등록돼 있으면 다시 추가하지 않는다). 이
자동 등록은 git 저장소가 아니거나 git이 없으면 조용히 건너뛰며, 어떤 경우에도 프로세스를
종료시키지 않는다. 자동 등록을 끄고 싶으면 아래 `register_git_exclude`를 `false`로 둔다
(전역 설정에서만 적용).

설정은 TOML 파일 두 곳에서 읽는다.

- 전역: `~/.scout/config.toml`
- repo별: `<repo>/.scout/config.toml`

우선순위는 **키 단위로 repo > 전역 > 기본값**이다. 각 키마다 가장 높은 우선순위에
존재하는 값을 통째로 채택하며(배열도 append가 아니라 replace), 어느 레이어도 주지 않은
키만 built-in 기본값으로 채운다. 전역 `~/.scout/config.toml`은 없으면 모든 값이 주석
처리된 템플릿으로 자동 생성되지만, repo별 `<repo>/.scout/config.toml`은 자동 생성하지
않는다(opt-in, 직접 만들어야 함). 잘못되거나 깨진 설정은 stderr에 한국어 경고를 남기고 그
값만 무시한 채 기본값으로 동작한다 — 절대 프로세스를 종료하지 않는다.

주요 설정 키는 다음과 같다.

| 테이블 | 키 | 타입 | 기본값 | 설명 |
| --- | --- | --- | --- | --- |
| `[output]` | `mode` | `"content"` \| `"files_with_matches"` \| `"count"` | `"files_with_matches"` | 검색 결과 출력 형식 |
| `[output]` | `head_limit` | 정수 ≥ 0 | `250` | 결과 상한(0 = 무제한) |
| `[output]` | `context_lines` | 정수 ≥ 0 | `0` | 매치 전후로 함께 보여줄 줄 수 |
| `[output]` | `show_line_numbers` | bool | `true` | 줄 번호 표시 여부 |
| `[index]` | `excluded_directories` | 문자열 배열 | built-in 목록(`.git`, `node_modules`, `dist` 등) | 인덱싱에서 제외할 디렉터리 이름(replace) |
| `[index]` | `staleness_check_ms` | 정수 > 0 | `2000` | 인덱스 신선도 재검사 주기(ms) |
| `[index]` | `respect_gitignore` | bool | `true` | repo `.gitignore`의 디렉터리 이름을 제외 집합에 union |
| `[index]` | `register_git_exclude` | bool | `true` | `<repo>/.scout/`를 `.git/info/exclude`에 자동 등록. **전역 설정에서만 적용**(repo 레이어 값은 경고 후 무시) |
| `[limits]` | `search_request_timeout_ms` | 정수 > 0 | `15000` | 단일 검색 요청 타임아웃(ms) |
| `[limits]` | `index_build_timeout_ms` | 정수 > 0 | `600000` | 인덱스 빌드 타임아웃(ms) |

```toml
[output]
mode = "files_with_matches"
head_limit = 250
context_lines = 0
show_line_numbers = true

[index]
excluded_directories = [".git", "node_modules", "dist"]
staleness_check_ms = 2000
respect_gitignore = true
register_git_exclude = true   # 전역 설정에서만 적용

[limits]
search_request_timeout_ms = 15000
index_build_timeout_ms = 600000
```

인덱스 경로(`<repo>/.scout/zoekt/`)와 관리형 바이너리 위치(`~/.scout/bin/<tag>`,
`os.homedir()` 기준 전역 공유)는 설정으로 바꿀 수 없는 고정 경로다. 바이너리 설치
디렉터리만 전역에서 공유하고, 인덱스는 레포마다 분리해 둔다.

## Pre-commit

`husky`와 `lint-staged`로 커밋 전 검사를 돌린다. 스테이징된 자바스크립트, 타입스크립트, JSON 파일에는 `biome check --write`를 실행하고 수정 결과를 다시 스테이징한다.

```bash
pnpm install                 # husky 훅 설치
pnpm exec lint-staged        # 스테이징된 파일에 수동 실행
```

설정은 `package.json`의 `lint-staged`와 `.husky/pre-commit`. 타입스크립트 파일이 스테이징되어 있으면 `tsc` 검사도 함께 실행한다.

## 사용 도구

- [Turborepo](https://turborepo.com), [pnpm workspaces](https://pnpm.io/workspaces)
- [@modelcontextprotocol/sdk](https://github.com/modelcontextprotocol/typescript-sdk)
- [@modelcontextprotocol/inspector](https://github.com/modelcontextprotocol/inspector) — MCP 서버 디버깅
- [@agentclientprotocol/sdk](https://github.com/zed-industries/agent-client-protocol)
- [Biome](https://biomejs.dev) — 린트 + 포매터 (ESLint+Prettier 대체)
- [Husky](https://typicode.github.io/husky), [lint-staged](https://github.com/lint-staged/lint-staged) — pre-commit 훅 러너
