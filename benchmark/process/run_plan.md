# run_plan — cms-official-benchmark-20260619-03

> `-02`(정식 벤치마크) 완료 후속. 새 episode를 돌리지 않는다.
> `-02`의 episode raw 산출물·`scored_episodes.json`은 **읽기전용 입력**으로 재사용(점수·fact 불변).
> 이 런은 진단 2건 + 효율 metric 표기 교정 1건이다.

## 권위 사양
- 상위: `docs/cms-official-benchmark-orchestration-prompt.md` (불변)
- 기반 런: `.agents/orchestration/cms-official-benchmark-20260619-02/` (data-of-record, read-only)

## 작업 의도(사용자 원문 3건)
1. codex MCP 도구 노출 여부를 **정확히 확인**. 확인 후 codex 전체 재시도 여부는 **사용자가** 결정(보류).
2. opencode-serena 전송 배선 **1회** 재시도.
3. `result` 표 교정 — `tok_out`만 있고 `tok_in`이 사실상 비어 보임(claude가 ~10), `tool_call`이 codegraph에서 이상하게 책정됨. tool_call은 모든 도구 실행 흔적(각 MCP + Read/Find/Grep/Bash)을 표시하되 codegraph·serena는 괄호에 도구별 개수. 공정·투명하게.

## 토폴로지
병렬 fan-out(A·B·C 독립) → 오케스트레이터 집계 → HALT(go/no-go: codex 전체 재시도). C는 C→C검증의 2단 미니 파이프라인(nested).
이 런 자체에 비싼 fan-out 없음 → HALT1 cheap-proof 게이트 생략. Phase A 진단이 곧 "전체 재시도" 결정의 cheap-proof.

## Phase 목록 (전부 sonnet, 각각 하위 에이전트)
- **A. codex-tool-exposure-diagnostic** (WORKER/sonnet): 기존 codex `stdout.txt`/`exact_command.json` 먼저 마이닝 → MCP 연결·도구목록 핸드셰이크 증거. (가)노출-미사용=finding / (나)미노출=배선버그. 재실행은 기존 로그 불충분 시 1회만. 부수: codex usage의 input_tokens 캐시포함 여부 1줄.
- **B. opencode-serena-rewire-retry** (WORKER/sonnet): runner XDG config로 opencode-deepseek-serena 1 episode(deno, 빠른 LSP) 실행. serena 전송 부팅·`serena_*` 실호출 여부. 1회만.
- **C. result-metric-correction** (WORKER/sonnet): ①tok_in 런타임별 공식(각 runtime stdout.txt usage에서 캐시포함 여부 **선실증**, claude/opencode=input+cache_read+cache_creation, codex=input as-is(포함 확인 시)). ②tool_call=모든 도구 흔적, codegraph·serena 괄호에 도구별(가능하면 codegraph_explore mode별) 개수. 파이프라인 사본을 `-03/analysis/`에서 수정·node 실행 → 교정 표 산출. `-02` 불변.
- **C-verify. result-metric-verify** (WORKER/sonnet, C 이후): 표본 cell 독립 재계산(raw에서) → `-03` 표와 0 mismatch 확인. 생산자 자가채점 방지.

## 산출물(예정)
- `phases/codex-tool-exposure-diagnostic/findings.{md,json}`
- `phases/opencode-serena-rewire-retry/findings.{md,json}`
- `analysis/mcp_comparison_tables.{json,md}` + `phases/result-metric-correction/correction_notes.md`
- `phases/result-metric-verify/verify.{md,json}`
- `report.md` (집계 + HALT 자료)

## 멈춤
- HALT(go/no-go): codex 전체 재시도 여부 — 사용자 결정. 오케스트레이터는 진단 결과만 제시.
- HALT2(review): 작업3은 규칙 명확한 파생값 교정 + 독립 재계산 검증으로 충분 → 강제 미설정, 산출물 본 뒤 필요시.

## 재버전 규칙
- 작업3이 효율 metric 산출·표기(reporting 기준)를 바꾸므로 새 run-id `-03`. `-02`는 지금 superseded 아님(데이터 재사용). `-03` 교정 표가 `-02` 보고 레이어를 대체하면 그때 기록.
