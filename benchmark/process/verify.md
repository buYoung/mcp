# 독립 검증 보고서 (result-metric-verify)

생성일: 2026-06-20  
검증 대상: cms-official-benchmark-20260619-03 result-metric-correction phase 교정값  
검증 방법: raw stdout.txt 직접 파싱, result_metrics.json 손 계산 — aggregate.mjs 미사용

---

## 종합 결과

**overall_pass: TRUE** — 5개 주장 전수 검증 통과. 불일치 0건. 예상 못한 과소집계 0건.

---

## 검증1: codex MCP arm 27 에피소드 backend_exercised 플립 (주장1)

**결과: PASS**

- 방법: 각 stdout.txt NDJSON 파싱 → `type=item.completed` & `item.type=mcp_tool_call` & `item.server=배정서버` 필터 → ok/failed 카운트
- 배정 server 매핑: codex-gpt54-codegraph→codegraph, codemap→codemap-search, serena→serena
- 27개 전수 total(ok+failed) ≥ 1 확인 → 모두 backend_exercised=true
- ok/failed 카운트 교정 테이블 대비 **0 mismatch** (배정 server 필터 적용 기준)

**주의사항**: 배정 server 필터 미적용 시 2건 불일치 발생.
- serena ClickHouse-master round-3: codex 내장 `list_mcp_resources`(server='codex') 포함 시 ok=16, 교정값 ok=15
- serena deno-main round-1: list_mcp_resources + list_mcp_resource_templates 포함 시 ok=4, 교정값 ok=2
- 결론: 교정 기준이 옳음. codex 내장 도구 제외 후 값이 맞음.

---

## 검증2: backend_off 재계산 old=39→new=12 (주장2)

**결과: PASS**

독립 재계산:
- 원본 scored_episodes.json: non-no-mcp backend_exercised=false = **39개**
  - codex-gpt54: 27, opencode-deepseek: 3, opencode-mimo: 8, opencode-minimax: 1
- raw stdout 기반 교정 후: **12개** (codex 27개 전수 true 전환)
  - 잔여: opencode-deepseek(3), opencode-mimo(8), opencode-minimax(1)
- 12개 전수 비-codex 확인 ✓

---

## 검증3: codex tool_call breakdown 표본 대조 (주장3)

**결과: PASS**

| 셀 | 관측 합계 | 관측 breakdown | 교정값 합계 | 일치 |
|---|---|---|---|---|
| codex-gpt54-codegraph / deno-main / round-3 | 44 | search(29), node(10), explore(5) | 44 | ✓ |
| codex-gpt54-codemap / ClickHouse-master / round-3 | 21 | codemap:grep(12), codemap:read(9) | 21 | ✓ |
| codex-gpt54-serena / ClickHouse-master / round-1 | 26 | serena:search_for_pattern(21), get_symbols_overview(3), initial_instructions(1), find_symbol(1) | 26 | ✓ |

---

## 검증4: 다른 런타임에서 예상 못한 과소집계 탐지 (중요)

**결과: 예상 못한 누락 없음**

**claude-sonnet**: 27개 에피소드에서 ToolSearch(메타도구)가 표 합계에서 제외됨. 교정 문서에 명시된 알려진 패턴("ToolSearch는 tool_search_count 열로 별도 표기"). 예상 외 누락 없음.

**opencode**: 48개 불일치 에피소드 확인됨.
- 제외 도구: ToolSearch, invalid, todowrite, skill → 모두 교정 문서에 명시된 알려진 패턴
- task 위임(서브에이전트 내부 미집계): 46개 에피소드 → 전수 표에 `task(N)[inner_untracked]` 표기됨. 일관성 있음.
- **예상 못한 대형 과소집계: 0건**

---

## 검증5: tok_in 공식 표본 검증 (주장5)

**결과: PASS**

**claude-sonnet / codegraph / ClickHouse-master / round-2**:
- result_metrics.json: input=14, cache_read=263172, cache_creation=54619
- 공식 적용: 14 + 263172 + 54619 = **317,805**
- 표 tokens_in: 317,805 ✓

**codex-gpt54 / codegraph / deno-main / round-3**:
- result_metrics.json: input_tokens=899,619 (cached_input_tokens=770,048)
- 공식 적용: input_tokens 그대로 = **899,619**
- 표 tokens_in: 899,619 ✓
- 이중계산 검증: cached_input_tokens(770,048) ≤ input_tokens(899,619) → 포함관계 확인됨

---

## 불일치 목록

없음.

## 예상 못한 도구 과소집계 목록

없음.
