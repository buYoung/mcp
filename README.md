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
      tools/                 # MCP 도구 표면 (list_agents, ask_pair, continue_pair, consult_panel, close_pair)
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
