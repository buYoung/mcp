# codemap-search 에이전트 벤치마크 — 에피소드 실행 runbook v2 (고정)

모든 에피소드는 이 문서의 명령을 **그대로** 사용한다 (v4 교훈: 프롬프트/명령 재구성은 측정을 오염시킨다).
바이너리: `/Users/buyong/workspace/private/buyong-mcp/apps/codemap-search/target/release/codemap-search`

v2 변경 (파일럿 1차 FAIL 반영): claude arm의 `--safe-mode`가 명시적 `--mcp-config`까지 차단하는 것이
프로브로 확인되어 `--setting-sources ""`로 교체 (MCP 노출 + 훅 오염 0 검증 완료). 차단 도구에
`Workflow,Agent,Skill` 추가. 메트릭 스키마 단일화. macOS에는 `timeout` 명령이 없음 — Bash 도구
타임아웃(600000ms)으로 대체하고 wall clock은 `date +%s`로 측정.

## 0. 사전 인덱싱 + 배선 점검 (repo당 1회, 에피소드 시작 전)

```bash
cd <REPO_PATH> && /Users/buyong/workspace/private/buyong-mcp/apps/codemap-search/target/release/codemap-search index .
```

- 인덱스 포맷 sidecar(`.codemap/index/codemap.format`)가 `v7-owner-tokens-indexed`인지 확인.
- 메트릭 산정 주의(차기 이터레이션): `first_answer_turn`/`turns` 집계에서 ToolSearch(하니스 메커니즘)는 도구 호출로 세지 않는다. `answer_text`는 요약 없이 축어 기록.
- **동시성 규칙**: 같은 repo에 대한 에피소드는 반드시 순차 실행 (tantivy writer lock). repo가 다르면 병렬 가능.
  - **(2026-06-12 정정)** 위 규칙은 과잉 보수로 판명 — 사전 인덱싱된 미변경 repo는 writer를 획득하지
    않아 동일 repo 병렬이 안전하다(동시 8프로세스 실측 0에러). 차기 캠페인은 playbook §4를 따른다.
- (선택) 배선 사전 점검: 아래 claude 명령에서 프롬프트를 "사용 가능한 도구 이름만 나열해. 도구를 호출하지 마."로
  바꿔 1회 실행 → 출력에 `mcp__codemap-search__search` 등 5종이 보여야 한다.

## 1. codex arm (gpt-5.5, reasoning medium, pure MCP)

```bash
codex exec -C <REPO_PATH> --skip-git-repo-check --ignore-user-config --ephemeral \
  -s read-only -m gpt-5.5 -c model_reasoning_effort="medium" \
  -c approval_policy="never" \
  -c 'mcp_servers.codemap-search.command="/Users/buyong/workspace/private/buyong-mcp/apps/codemap-search/target/release/codemap-search"' \
  -c 'mcp_servers.codemap-search.args=["mcp"]' \
  --json "<PROMPT>" > <OUT_DIR>/<EPISODE_ID>.codex.jsonl 2> <OUT_DIR>/<EPISODE_ID>.codex.stderr
```

## 2. claude arm (sonnet, pure MCP)

```bash
cd <REPO_PATH> && claude -p --model sonnet --setting-sources "" \
  --mcp-config /tmp/benchmark-data/mcp-codemap.json --strict-mcp-config \
  --allowedTools "mcp__codemap-search__search,mcp__codemap-search__overview,mcp__codemap-search__read,mcp__codemap-search__find,mcp__codemap-search__grep" \
  --disallowedTools "Bash,Read,Glob,Grep,Edit,Write,WebFetch,WebSearch,Task,NotebookEdit,TodoWrite,Workflow,Agent,Skill" \
  --output-format stream-json --verbose \
  "<PROMPT>" > <OUT_DIR>/<EPISODE_ID>.claude.jsonl 2> <OUT_DIR>/<EPISODE_ID>.claude.stderr
```

- `--setting-sources ""`: 사용자/프로젝트 설정(훅 포함) 미로드. `--safe-mode`는 사용 금지(MCP까지 차단).
- `ToolSearch`는 차단하지 않는다(하니스 메커니즘 — strict-mcp-config + 차단 목록 하에서 우회 수단이 못 됨).
- **오염 검사**: jsonl에서 `Serena MCP Tool Policy` 문자열이 보이면 격리 실패 → 에피소드 무효, 하니스 결함으로 보고.

## 3. 공통 규칙

- `<PROMPT>`는 tasks-*.json의 `prompt` 필드를 **글자 그대로** 사용 (jq -r로 추출, 한 글자도 추가/수정 금지).
- 타임아웃: 에피소드당 10분 (Bash 도구 timeout 600000ms). 초과 시 `harness_error: "timeout"` 기록.
- wall clock: 명령 전후 `date +%s`로 `duration_s` 산출 (codex jsonl에는 타임스탬프가 없음).
- 하니스 수준 실패(프로세스 비정상 종료, MCP 연결 실패, 출력 파싱 불가)는 **1회만** 재시도. 오답은 재시도 금지.
- 에피소드 산출물(jsonl/stderr)은 `<OUT_DIR> = /tmp/benchmark-data/results/<iteration>/<repo>/` 아래 보존.

## 4. 메트릭 추출 (에피소드당 1 JSON — 단일 표준 스키마)

codex `--json`: `item.completed` 이벤트에서 `mcp_tool_call` / `command_execution` 집계.
claude `stream-json`: `assistant` 메시지의 `tool_use` 블록과 대응 `tool_result` 집계.

```json
{
  "episode_id": "<repo>-<arm>-<task_id>-r<rep>",
  "repo": "ollama|clickhouse", "arm": "claude-sonnet|codex-gpt55", "task_id": "o1", "rep": 1,
  "duration_s": 0, "harness_error": null, "auth_variant": "setting-sources-empty|n/a",
  "turns": 0,
  "tool_calls": {"search": 0, "grep": 0, "read": 0, "overview": 0, "find": 0},
  "shell_bypass_calls": 0,
  "denied_builtin_attempts": 0,
  "contamination_found": false,
  "duplicate_calls": 0,
  "mcp_response_bytes_total": 0,
  "first_answer_turn": null,
  "answer_text": "<contestant 최종 메시지 전문>",
  "score": "correct|partial|wrong",
  "score_rationale": "rubric 적용 근거 1-2문장",
  "notes": "하니스 관찰 사항 (없으면 빈 문자열)"
}
```

- `turns` = 도구 호출 총수 (두 하니스 동일 정의).
- `shell_bypass_calls`: codex의 `command_execution` 수 (claude는 0 고정 — Bash 차단됨).
- `denied_builtin_attempts`: claude에서 거부된 비허용 도구 시도 수 (codex는 0 고정).
- `duplicate_calls`: 동일 도구+동일 인자 반복 호출 수.
- `mcp_response_bytes_total`: MCP tool_result 텍스트 바이트 합 (산출 불가 시 null).
- `first_answer_turn`: expected.file(상대경로)이 도구 **결과**에 처음 등장한 호출 순번 (1-indexed, 없으면 null).
- `score`: 해당 task의 `rubric`을 기계적으로 적용.

## 5. 파일럿 통과 기준 (본 매트릭스 진입 게이트)

1. 두 arm 모두 에피소드 정상 종료 + 출력 파싱 가능
2. 각 transcript에 codemap-search MCP 호출 ≥ 1 (MCP 배선 증명)
3. claude transcript에 훅/사용자 설정 오염 없음 (`Serena MCP Tool Policy` 부재)
4. claude arm에서 빌트인 파일/셸 도구 사용 0 (pure 보장)
5. 메트릭 JSON이 4절 표준 스키마대로 추출되고 rubric 채점 가능
6. tantivy lock 에러 / MCP 연결 에러 0
