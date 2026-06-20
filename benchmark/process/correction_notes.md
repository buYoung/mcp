# metric 재산출 교정 노트 (cms-official-benchmark-20260619-03)

생성일: 2026-06-20  
원본 run: cms-official-benchmark-20260619-02  
교정 범위: tok_in 표기 + tool_call 괄호 분해  
불변 항목: scorer_score · per_fact_score · valid/invalid 분류 · backend_exercised

---

## 1. 결함1 교정 — tok_in 런타임별 규약

### 실증 근거 (stdout usage 오브젝트 직접 확인)

#### claude-sonnet
파일: `phases/solver-episodes/claude-sonnet-codegraph/ClickHouse-master/round-1/stdout.txt`  
이벤트 타입: `type=result` (마지막 누적 usage)

```json
{
  "input_tokens": 10,
  "cache_creation_input_tokens": 34965,
  "cache_read_input_tokens": 151268,
  "output_tokens": 7195
}
```

**해석**: `input_tokens=10`은 캐시를 제외한 신규 입력 토큰만. 캐시는 별도 필드(`cache_read_input_tokens`, `cache_creation_input_tokens`)로 분리됨.  
**규약**: 캐시 **제외** 런타임 → `tok_in = input_tokens + cache_read_input_tokens + cache_creation_input_tokens`

#### opencode-*
파일: `phases/solver-episodes/opencode-deepseek-codegraph/ClickHouse-master/round-1/stdout.txt`  
이벤트 타입: `type=step_finish` (마지막 step)

```json
{
  "tokens": {
    "total": 13350,
    "input": 3896,
    "output": 216,
    "reasoning": 22,
    "cache": { "write": 0, "read": 9216 }
  }
}
```

result_metrics.json 동일 에피소드: `input_tokens=3896, cache_read_input_tokens=9216`  
**해석**: `input`(=input_tokens)은 캐시 제외 신규분. `cache.read`(=cache_read_input_tokens)는 별도.  
**규약**: 캐시 **제외** 런타임 → claude와 동일 공식  

#### codex-gpt54
파일: `phases/solver-episodes/codex-gpt54-codegraph/ClickHouse-master/round-1/stdout.txt`  
이벤트 타입: `type=turn.completed` (마지막 usage)

```json
{
  "input_tokens": 190248,
  "cached_input_tokens": 106752,
  "output_tokens": 2353,
  "reasoning_output_tokens": 851
}
```

**해석**: `input_tokens=190248`은 캐시 포함 총 입력. `cached_input_tokens=106752`는 그 중 캐시된 부분.  
캐시를 추가로 더하면 이중계산(190248+106752=297000으로 부풀려짐).  
**규약**: 캐시 **포함** 런타임 → `tok_in = input_tokens` (그대로)

### 적용 공식 (런타임별)

| 런타임 | tok_in 공식 | 근거 |
|---|---|---|
| claude-sonnet | `input_tokens + cache_read_input_tokens + cache_creation_input_tokens` | result usage: input=신규분만 |
| opencode-deepseek/mimo/minimax | `input_tokens + cache_read_input_tokens + cache_creation_input_tokens` | step_finish tokens: input=신규분 |
| codex-gpt54 | `input_tokens` (그대로) | turn.completed: input=캐시포함 합산 |

### 보정 전후 비교 (ClickHouse round-1 claude-sonnet-codegraph)

| 항목 | 보정 전 | 보정 후 |
|---|---|---|
| tok_in | 10 (input_tokens만) | 186,243 (=10+151268+34965) |
| 의미 | "입력 토큰이 거의 없는 것처럼" 보임 | 실제 총 입력 정확 반영 |

**보정 후 비교 가능성**: tok_in은 전 런타임 공통 "캐시 포함 총 입력 토큰". codex 190k와 claude ~186k는 동일 기준으로 직접 비교 가능.  
`tok_in - cache_tokens = 신규 입력`으로 일관 해석 가능.

---

## 2. 결함2 교정 — tool_call 괄호 분해 + codex stdout 재구성

### 2-1. codex-gpt54 추출기 갭 발견 및 재구성

**문제**: scored_episodes의 모든 codex-gpt54 에피소드에서 `tool_call_distribution: {}`. result_metrics도 동일.

**원인 파악**: codex의 stdout 포맷이 다른 런타임과 다름.
- claude: `type=assistant` 이벤트의 `message.content[].type=tool_use`
- opencode: `type=step_finish` 이벤트의 `tokens`
- codex: `type=item.completed` 이벤트의 `item.type=mcp_tool_call` / `item.type=command_execution`

추출기가 codex의 `item.completed/mcp_tool_call` 포맷을 파싱하지 못해 분포 미기록.

**확인 증거**: `codex-gpt54-codegraph/ClickHouse-master/round-1/stdout.txt`에서  
실제 도구 호출 확인: `mcp_tool_call(codegraph_explore×8), mcp_tool_call(codegraph_search×14), mcp_tool_call(codegraph_node×4)`.  
그러나 `result_metrics.json`의 `tool_call_distribution: {}`, `backend_exercised: False`.

**교정 방법**: stdout의 `item.completed` 이벤트를 파싱하여 재구성.
- `item.type=mcp_tool_call` → `mcp__{server}__{tool}` 형태로 집계
- `item.type=command_execution` → `command_execution` 으로 집계
- `tool_call_source=stdout_rebuilt`로 표기

**주의**: `backend_exercised` 필드는 불변(frozen). scored_episodes의 값 그대로 유지.  
codex-gpt54의 `backend_off 3/3` caveat도 동결값. 단, tool_call_breakdown은 stdout 재구성으로 보완.

### 2-2. 도구 분류 및 정규화

opencode는 MCP 접두사 없이 짧은 키 사용 (`codegraph_codegraph_explore` 등) → 정규화 처리:

| 원본 키 | 정규화 |
|---|---|
| `codegraph_codegraph_explore` | `mcp__codegraph__codegraph_explore` |
| `codemap-search_search` | `mcp__codemap-search__search` |
| `read`, `grep`, `glob` | `Read`, `Grep`, `Glob` |

### 2-3. 특수 도구 처리

| 도구 | 처리 | 이유 |
|---|---|---|
| ToolSearch | `tool_calls_total` 제외, `tool_search_count` 열로 별도 | 스키마 로딩 전용 메타도구 |
| task | `tools` 제외, `task(N)[inner_untracked]`로 표시 | opencode 서브에이전트 스폰. 내부 도구 호출이 parent에 기록되지 않아 추적 불가 |
| invalid | 제외 | 파싱 실패로 추정되는 내부 아티팩트 |
| skill, todowrite | 제외 | 내부 도구 |
| command_execution | `shell(N)`로 표시 | codex no-mcp의 쉘 명령 실행 |

### 2-4. codegraph operation 분해 시도 결과

요청: codegraph의 `codegraph_explore` 인자에 `mode`/`kind` 같은 operation 구분자가 있는지 확인.

실측 결과 (`claude-sonnet-codegraph/ClickHouse-master/round-1/stdout.txt`):
```json
{
  "name": "mcp__codegraph__codegraph_explore",
  "input": { "query": "dictGet dictGetOrDefault fallback type inference throwIf" }
}
```

`codegraph_explore`의 인자는 `{"query": "..."}` 뿐. `mode`, `kind`, `operation` 등 분류 인자 없음.  
**결론**: operation별 재분해 불가 → 도구명 단위 분해 (explore/node/search/callers).  
caveat 추가: 단일 codegraph 콜이 serena 여러 콜의 일을 하므로 콜수 직접 비교 시 주의.

### 2-5. 괄호 분해 예시

| backend | 런타임 | 예시 breakdown |
|---|---|---|
| codegraph(claude) | claude-sonnet | `explore(3), Read(3)` |
| codegraph(codex)  | codex-gpt54   | `search(14), explore(8), node(4)` [rebuilt] |
| codegraph(opencode) | opencode-minimax | `explore(11), search(2)` |
| serena(claude)    | claude-sonnet | `serena:search_for_pattern(7), Read(11)` |
| serena(codex)     | codex-gpt54   | `serena:search_for_pattern(42), serena:find_symbol(2), serena:get_symbols_overview(6)` [rebuilt] |
| codemap(claude)   | claude-sonnet | `codemap:search(3), codemap:grep(1), Read(4)` |
| no-mcp(claude)    | claude-sonnet | `Read(N), Grep(M), Glob(K), Bash(J)` |
| no-mcp(codex)     | codex-gpt54   | `shell(34)` [rebuilt] |
| codegraph(opencode, task) | opencode-deepseek | `task(1)[inner_untracked]` |

---

## 3. 파이프라인 변경 요약

| 파일 | 변경 내용 |
|---|---|
| `-03/analysis/aggregate.mjs` | ROOT 분리(읽기=-02, 쓰기=-03). `computeTokIn()` 런타임별 규약. `rebuildCodexToolDist()` stdout 재구성. `buildToolBreakdown()` 괄호 분해. `task` 별도 표기. |
| `-03/analysis/render_tables.mjs` | ROOT_OUT=-03. tok_in 각주 테이블 추가. 효율 표 컬럼 교체(tool_breakdown, ts_cnt, tool_src). 각주 보강. |

**원본 -02는 일절 수정하지 않음.**

---

## 4. 불변 항목 확인

- `scorer_score`: scored_episodes 값 그대로 사용, 재계산 없음
- `per_fact_score`: 그대로
- `valid/invalid` 분류: `harness_valid`, `timed_out` 원본값 그대로
- `backend_exercised`: scored_episodes 원본값 그대로 (codex backend_off caveat 동결)
- `paired_delta`, `mean_score` 등 품질표: quality_stats 원본값 그대로
