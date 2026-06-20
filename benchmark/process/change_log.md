# change_log — cms-official-benchmark-20260619-03

## 2026-06-20 — 런 개시 (`-02` 후속/수정)

- 기반: `cms-official-benchmark-20260619-02` (정식 벤치마크, 완료). episode raw·`scored_episodes.json` 읽기전용 재사용.
- 새 run-id 사유(orchestration 재버전 규칙): 작업3이 효율 metric `tok_in` 산출식과 `tool_call` 표기 정의(reporting/evaluation 기준)를 변경. 점수·fact는 불변.
- `-02` 처분: **superseded 아님**. 데이터-of-record로 유지. `-03`이 교정 표를 산출하면 보고 레이어만 대체.
- 범위: 진단 2(codex 노출 / opencode-serena 배선) + 표 교정 1. 153 episode 재실행 없음.

## 2026-06-20 — advisor 검토 반영(spawn 전)

- **차단 교정(Phase C)**: `tok_in`을 런타임 단일 공식으로 합산하면 codex 이중계산 위험(codex `input_tokens`가 캐시 포함으로 추정). → 각 runtime의 `stdout.txt` usage 객체에서 "input_tokens 캐시포함 여부"를 **먼저 실증**하고 런타임별 공식 고정 + 표 각주에 근거 명시. 단일 공식 금지.
- **Phase A 강화**: 재실행 전에 기존 codex `stdout.txt`/`exact_command.json`을 먼저 마이닝(핸드셰이크/도구목록 증거). 부재 시에만 1회 재실행. "도구 0회"가 아니라 "도구 노출 양성 증거 + `-c mcp_servers` 형식 적법성"을 요구.
- **Phase C 검증 강화**: 자가검증만 신뢰 금지(`-02`에서 codemap 6→5 산술오류가 자가검증 통과). 독립 재계산 단계(C-verify) 추가.
- **codegraph 괄호 granularity**: `tool_events.json`의 `codegraph_explore` mode 인자가 있으면 도구명이 아니라 operation(mode)별로 분해(안 그러면 "codegraph 3 vs serena 9"가 여전히 오도). codemap 도구도 all-traces에 열거.

## 2026-06-20 — 사용자 결정(HALT) + 범위 확장 → `-03`이 `-02`를 supersede

- 발견 통지: runner `extractCodexOutput` 텔레메트리 버그로 `-02`의 "codex 비교 무의미"가 거짓임을 확정. 사용자에게 `-02 HALT2 수락 결론이 뒤집혔음`을 명시 통지.
- 사용자 결정 3건:
  1. **codex 전체 재실행 = 안 함**(권장 채택). 로그 재파싱으로 도구호출 복구 완료, 점수 유효.
  2. **opencode-serena 27 episode = 재실행함**. serena 동시성 한도(전역 3 / codebase당 1) + 메모리 가드 적용. (executed N 153→180)
  3. **`-02` 서술 보고서 = 재작성함**. report.md·detailed_report.md·limitations를 codex-usable로 정정.
- 범위 확장 결과: `-03`은 더 이상 "표 교정"에 그치지 않고 (a) 새 episode 27개 추가(item set 153→180) (b) `-02` 서술 대체. 이는 run-condition 변경이므로 `-03`이 `-02`를 **superseded**로 만든다(traceability: 관련 교정·확장 산출물을 한 run에 모음).
- 실행 안전: runner가 episode backend/runtime에서 동시성 자동 유도 → opencode-serena만 먹이면 serena cap 3/codebase1 + 메모리 가드 자동. ClickHouse(clangd) 무거우나 per-codebase 1 + 가드로 보호. 강제종료 위험 통제.
- 잔여(미요청): runner `extractCodexOutput` 한 줄 버그는 이번 run에 영향 없음(codex 재실행 안 함, 복구는 분석측 stdout 재파싱). 향후 codex run 대비 패치 제안은 유지.

## 2026-06-20 — harness 패치: runner extractCodexOutput 텔레메트리 버그 수정

- 대상: `cms-official-benchmark-20260619-02/runner/runner.mjs` `extractCodexOutput()`.
- 버그: `toolEvents:[]`를 하드코딩 반환 → codex의 MCP/쉘 도구 호출이 전수 0으로 오기록(이번 벤치마크 -02 헤드라인 오염의 근본원인).
- 수정: codex stdout JSONL의 `item.started`/`item.completed`(item.type=`mcp_tool_call`|`command_execution`)를 claude/opencode 파서와 동일한 call/result 이벤트로 기록. 네이밍 `mcp__<server>__<tool>`(쉘은 `command_execution`).
- 검증: 실제 codex stdout 3종 — codemap deno r1=40 calls(grep19/read16/…), codegraph CH r1=13(search7/explore4/node2), no-mcp CH r1=17 command_execution. 문법검사 통과.
- 영향: 기존 run 데이터 불변(-03 분석은 이미 stdout 재파싱으로 복구). 패치는 **향후 codex 런** 재발 방지용. 잔여 이슈(runner 미패치)는 본 수정으로 해소.
