# buyong-mcp

코딩 에이전트들 사이의 페어 프로그래밍 브리지.

이 monorepo는 MCP(Model Context Protocol) 서버 하나를 제공하며, 그 서버는
ACP(Zed Agent Client Protocol) client 역할을 한다. 즉, 호출하는 에이전트
(예: Claude Code)는 MCP 도구처럼 다른 코딩 에이전트(Codex, Gemini CLI 등)를
부를 수 있고, 내부적으로는 그 에이전트를 ACP 서버 프로세스로 띄워서 대화를
중계한다.

## 가설

코딩 에이전트마다 강점이 다르다. 사람 페어 프로그래밍에서 driver/navigator가
역할을 바꾸듯, 에이전트도 서로에게 의견을 구하면서 작업하면 단일 루프보다
견고한 결과가 나온다.

## 구조

```
apps/
  mcp-server/                # MCP stdio 서버 (단일 앱)
    src/
      index.ts               # 엔트리
      tools/                 # MCP 도구 표면 (list_agents, ask_pair, continue_pair, close_pair)
      agents/                # 에이전트 어댑터 + registry
        common/              # 공통 AgentAdapter + ACP 어댑터 생성기
        claude-code/         # Claude Code ACP 설정
        codex/               # Codex ACP 설정
        gemini-cli/          # Gemini CLI ACP 설정
      acp/                   # ACP client wrapper
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

기본 페어 후보는 `claude-code`, `codex`, `gemini-cli` 세 개다. 명시 요청 원문을
`user_request`로 넘겨 `list_agents`에서 후보의 `agent_id`를 조회하고, `ask_pair`에
`agent_id`를 넘겨 새 세션을 만든 뒤, 반환된 `session_id`를 `continue_pair`에 넘겨 같은
후보와 이어서 대화한다. `list_models`는 이전 호환을 위한 별칭이다.

`ask_pair`와 `continue_pair`는 사용자가 명시적으로 페어 검토를 요청했을 때만 호출해야 한다.
이를 강제하기 위해 두 도구 모두 `user_request` 인자를 요구한다. 이 값에는 `fair programming`,
`pair programming`, `다른 에이전트의 의견`, `교차 검토`처럼 사용자의 명시 요청 원문을 넣는다.
명시 요청으로 판단되지 않으면 도구 호출은 거절된다.

페어 응답은 기존 호환을 위해 원문 `answer`를 유지하면서, `structured_opinion`도 함께 반환한다.
페어 에이전트에는 `summary`, `agreements`, `concerns`, `recommendation`, `confidence`,
`follow_up_questions` 키를 가진 JSON 응답을 요청한다. JSON 파싱에 실패하면 `parse_status`가
`fallback`인 구조로 원문을 보존한다.

상담이 끝나면 `close_pair`로 세션을 닫는다. 서버는 유휴 세션을 30분 뒤 정리하고, 동시에
최대 20개 세션만 유지한다. 같은 `session_id`에 대한 후속 요청과 종료 요청은 순서대로 처리한다.

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
