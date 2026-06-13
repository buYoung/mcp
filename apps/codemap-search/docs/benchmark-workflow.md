# codemap-search 성능 측정 — 단일 기준 문서

모든 벤치마크 캠페인이 따르는 공통 워크플로우, 메트릭 산출 정의, 고정 실행 명령,
그리고 2026-06까지의 캠페인 통합 결과를 담은 단일 기준 문서. 캠페인별 세부
(저장소·과제·매트릭스 구성)는 각 runbook이, 운영 함정·실측 근거는
`agent-benchmark-playbook.md`가 보유한다. 이 문서와 다른 문서가 충돌하면
**이 문서가 우선**한다. (2026-06-12 사용자 확정)

> (2026-06-13) 영문 통합 결과 문서 `BENCHMARKS.md`는 본 문서 §8(캠페인 결과)·
> §9(재현 가이드)로 병합됐다. 결과 수치의 단일 출처는 이제 이 문서다.

문서 구성: §1 측정 정의 · §2 단계 구성 · §3 모델 배치 · §4 실행 원칙 ·
§5 자산 포인터 · §6 메트릭 산출 정의 · §7 고정 실행 명령 · §8 캠페인 결과 ·
§9 재현 가이드.

## 1. 측정 정의와 목적

- **무엇을**: codemap-search MCP 서버(검색·내비게이션 5종 도구)의 실전 성능.
- **누가(contestant, 고정)**: claude CLI(`--model sonnet`) + codex CLI(gpt-5.5,
  reasoning medium). 두 CLI 모두 격리 플래그로 사용자 설정·훅을 차단한 pure 구성.
- **왜 (목적 2가지)**:
  1. **codemap-search 성능 개선** — 측정→분석→수정→재측정 루프로 제품 결함과
     변별 지표를 찾는다.
  2. **code-agent 토큰 사용량 최적화** — 동일 과제를 더 적은 턴·더 적은 도구 결과
     바이트·더 적은 contestant 토큰으로 풀게 만드는 것이 개선의 최종 지표다.
     contestant 토큰 사용량(claude stream-json usage 필드, codex 토큰 이벤트)을
     1급 메트릭으로 추출한다 — 추출 가능 여부는 warmup에서 검증.

## 2. 단계 구성 (6 phase + warmup)

```
warmup(파일럿 게이트) → run → verify → 채점 → ┐
        ┌──────────────────────────────────────┘
        ├─ 재채점 (회차마다)
        ├─ 집계
        └─ 개선 및 rerun (반복 세션, 상한 2회) → run부터 재진입
```

| phase | 내용 | 모델/주체 | 병렬 |
|---|---|---|---|
| **warmup** | 본 세션 전 버그 적발. playbook §4-1 파일럿 게이트 6종 기준(정상 종료/파싱, 도구 호출 ≥1, 오염 0, purity 보장, 스키마 추출+채점 가능, lock·연결 에러 0 — 기준 전문은 §7-6)을 본실행과 같은 동시성으로 통과해야 함. **본 세션의 하니스 실패는 허용되지 않는다** — 실패 가능성은 전부 warmup에서 소진한다 | sonnet 러너 + fable 판정 | 본실행 동일 동시성 |
| **run** | 에피소드 실행. 고정 스크립트(run-one-episode.sh, §7) 호출만 허용 — 명령 재구성·프롬프트 수정 금지. 슬라이스 러너가 배치 단위로 분담 | sonnet 러너 (workflow 병렬) | 동시성 8~16, 에피소드 단위 |
| **verify** | 산출물 무결성 기계 검증: metrics 전수 파싱, harness_error, 오염 문자열, purity(비허용 도구 흔적), 토큰 필드 존재 | sonnet verifier | run 완료 후 1회 |
| **채점** | 과제 rubric을 기계 적용해 correct/partial/wrong. 배치 단위(10 에피소드/에이전트), StructuredOutput 강제, 기입은 스크립트(write-scores.sh) | **sonnet** ×배치 | 배치 병렬 |
| **재채점** (반복 — 회차마다) | 어려운 과제 중심 표본 ≥10 + 비-correct 전건. sonnet이 인용 줄 전수 원본 대조·rubric 체크리스트를 기계 수집 → **fable이 판정** (수집·판정 분리로 교차 검증 유지) | sonnet 수집 + **fable** 판정 | 수집 병렬 |
| **집계** | 수치 집계는 jq 스크립트(aggregate-results.sh), 현상 보고는 sonnet — 상위 모델은 기계 작업에 미투입 | 스크립트 + sonnet | — |
| **개선 및 rerun** (반복 세션, 상한 2회) | ① 분석: 실패·비효율 에피소드를 (a)제품 (b)하니스/채점 (c)모델 한계로 분류, transcript 인용 의무 — sonnet. ② root-cause 정정: fix_hypothesis를 라이브 바이너리로 재현 검증 후 확정 — **fable**. ③ 수정: 브리프 기반 코드 수정(DO-NOT-FIX 목록·검증 의무 포함) — **opus**. ④ 추출 변경 시 FORMAT_VERSION 범프 + 재인덱스 → warmup 재실행 → run부터 재진입 | sonnet → fable → opus | 분석 배치 병렬 |

**루프 규율**: 상한 2회. 재진입 조건은 "정답률 100% 미만 **그리고** transcript로
입증된 제품 결함"일 때만. 차기 회차 디렉터리는 현 회차 채점·분석 완료 전 생성 금지
(멱등 재개 × 혼합 바이너리 오염 — playbook §4-8).

## 3. 모델 배치 (확정)

| 역할 | 모델 |
|---|---|
| plan·오케스트레이션·게이트 판정(warmup/재채점/종료 전 리뷰) | **fable** (메인 루프) |
| 러너(run)·verify·채점·집계 보고·분석 현상 발견 | **sonnet** |
| root-cause 확정·정정 | **fable** |
| 코딩 작업(제품 수정) | **opus** |
| contestant (측정 대상) | 고정: claude sonnet, codex gpt-5.5 medium |

## 4. 실행 원칙

1. **병렬 우선**: 에피소드 단위 병렬(동일 repo 동시 포함 — 실측 안전), 채점·증거
   수집은 배치 병렬. 직렬화는 명시적 사유가 있을 때만.
2. **메인 루프(fable)는 contestant CLI를 직접 실행하지 않는다** — 실행은 workflow
   서브에이전트(sonnet 러너)가 고정 스크립트를 호출하는 방식만 허용. 메인 루프는
   계획·게이트 판정·브리프 발주만.
3. **에이전트는 판단에만, 실행·추출은 스크립트로** (playbook §4-3) — 러너는
   스크립트 호출자이지 명령 작성자가 아니다. 에피소드별 에이전트 스폰 금지
   (배치/슬라이스 단위만).
4. **실패 0 원칙**: 본 세션 하니스 실패는 결함이다. warmup에서 전 경로(배선·추출·
   채점 가능성·토큰 필드)를 실제로 통과시킨 뒤에만 본 세션 진입. 본 세션 중 하니스
   실패 발생 시 즉시 중단하고 원인을 warmup 항목으로 환류한다.
5. **측정 불변 조건**: 프롬프트는 tasks JSON에서 jq 추출(한 글자도 수정 금지;
   arm별 변환은 결정적 치환만 허용하고 runbook에 기록), 측정 사본 무수정, 사전
   인덱싱 + 포맷 버전 확인, 체크아웃 시 commit SHA 기록.
6. **제품 출하 문자열도 측정 표면이다 — ground truth 기계 대조 의무**: 도구
   설명·서버 instructions·예시·에러 메시지 등 contestant에게 노출되는 모든 제품
   문자열을 측정 전(warmup 게이트에서) tasks JSON의 expected file/line/symbol과
   기계 diff해 정답 단편 등장 0건을 확인한다. 개선 루프에서 제품 텍스트를 수정할
   때는 발주 브리프의 예시 문구도 같은 검사를 통과해야 한다 — 진단 transcript의
   구체 사례를 예시로 옮겨 쓰는 것 금지. (실측 근거: ds-iter2/3에서 도구 설명
   예시가 d7 정답 \`compiler.py:1595 execute_sql\`과 일치하는 누출이 사후
   적발됐다. 정확도 결론은 무영향이었으나 d7 턴 지표가 오염돼 무힌트 재실행으로
   보정했다 — `benchmark-evolution.md` §5.4, 본 문서 §8-8 공개 참조.)

## 5. 자산 포인터

캠페인 자산은 전부 이 저장소의 본 문서 옆(`docs/`)에 있다. 위치와 역할:

| 자산 | 위치 | 역할 |
|---|---|---|
| 운영 플레이북 | `agent-benchmark-playbook.md` | 운영 상세 — 격리 플래그, 실측 함정 표(가변 CLI 플래그·repo 컨텍스트 주입·인덱스 lock 의미론), 동시성 근거, 모델 가드레일, 스케일링 규칙 |
| 설계 진화 회고 | `benchmark-evolution.md` | 캠페인 통합 회고(한국어) — §8 모든 수치 뒤의 관찰→가설→수정→재측정 인과. 수치는 중복 게재하지 않고 본 문서 §8을 참조 |
| 캠페인별 runbook | `bench-*/...runbook.md` | 캠페인별 호출 명령·매트릭스·게이트 기록 |
| 캠페인 1·2 자산 | `bench-2026-06/` | 데이터셋(캠페인 1 `tasks-surrealdb.md`, 캠페인 2 `tasks-ollama.json`·`tasks-clickhouse.json`), `episode-runbook.md`(메트릭 표준 스키마 §4·파일럿 통과 기준 §5의 원본 — 본 문서 §6·§7에 인라인), 회차 분석 보고서 `analysis-iter{1,2}.md`, claude arm MCP 설정 `mcp-codemap.json` |
| 캠페인 3·4·5 자산 | `bench-2026-06-django-strapi/` | hold-out 캠페인 데이터셋. `baseline-runbook.md`는 baseline arm(빌트인 전용) 전용 — 측정 사본 `.codemap` 격리(quarantine)·복원 절차 포함. 재채점 게이트 판정 기록: `gate-verdict.md`, `gate-verdict-ds-base1.md`, `gate-verdict-ds-iter2.md`, `gate-verdict-ds-iter3.md`. 진행 기록은 `benchmark-evolution.md` 캠페인 3 절로 흡수 |
| 셸 하니스 | `bench-2026-06-django-strapi/harness/` | §2 고정 스크립트의 실체 — `run-one-episode.sh`·`run-matrix.sh`·`extract-metrics.sh`·`write-scores.sh`·`aggregate-results.sh`·`config.sh`. 4-arm 토큰 추출 포함. 본 문서 §7의 출처 |
| 후속 백로그 | `agent-benchmark-followups.md` | 잔여 제품·측정 개선 항목 |
| 개선 후보 분석 | `improvement-candidates-2026-06-12.md` | 추가 개선 후보 분석 |

## 6. 메트릭 산출 정의

(출처: `bench-2026-06/episode-runbook.md` §4. 정의는 아래로 완결되며, 추출 구현은
`harness/extract-metrics.sh`다.)

추출 원천 — 모델 추정이 아니라 CLI의 구조화 이벤트 스트림에 대한 jq 프로그램:

- **codex** `--json`: `item.completed` 이벤트에서 `mcp_tool_call` /
  `command_execution`을 집계.
- **claude** `stream-json`: `assistant` 메시지의 `tool_use` 블록과 대응
  `tool_result`를 집계.

에피소드당 1 JSON — 단일 표준 스키마:

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

필드 의미:

- `turns` = 도구 호출 총수 (두 하니스 동일 정의).
- `shell_bypass_calls`: codex의 `command_execution` 수 (claude는 0 고정 — Bash 차단됨).
- `denied_builtin_attempts`: claude에서 거부된 비허용 도구 시도 수 (codex는 0 고정).
- `duplicate_calls`: 동일 도구+동일 인자 반복 호출 수.
- `mcp_response_bytes_total`: MCP tool_result 텍스트 바이트 합 (산출 불가 시 null).
- `first_answer_turn`: expected.file(상대경로)이 도구 **결과**에 처음 등장한 호출
  순번 (1-indexed, 없으면 null).
- `score`: 해당 task의 `rubric`을 기계적으로 적용.
- `duration_s`: 명령 전후 `date +%s`로 산출한 wall clock (codex jsonl에는
  타임스탬프가 없음).

집계 주의사항:

- `first_answer_turn`/`turns` 집계에서 ToolSearch(하니스 메커니즘)는 도구 호출로
  세지 않는다.
- `answer_text`는 요약 없이 축어 기록.

토큰 메트릭 (캠페인 4부터 1급 메트릭으로 승격, 4-arm 전부 추출):

- claude: `stream-json` usage 필드. 입력 토큰 = input + cache-read + cache-creation.
- codex: `turn.completed` usage 이벤트 합산. `input_tokens`(cached 포함).
- 두 CLI는 서로 다른 것을 보고한다 — **CLI 내부 비교만, CLI 간 비교 금지**.
- 추출기 변경은 재사용·실행 전에 이전 회차 데이터로 byte-for-byte 회귀 검증
  (`del(.tokens)` diff = 0)을 거친다.

오염 검사:

- claude jsonl에서 `Serena MCP Tool Policy` 문자열(작성자 로컬 훅 설정에서만
  등장하는 마커)이 보이면 격리 실패 → 에피소드 무효, 하니스 결함으로 보고.
  모든 transcript를 기계 스캔한다.
- baseline 캠페인은 추가로 purity를 에피소드별 기계 검증: `mcp__codemap` 문자열 0,
  MCP tool-call 이벤트 0, web 사용 0, 거부 도구 시도 0.

## 7. 고정 실행 명령

(출처: `bench-2026-06-django-strapi/harness/run-one-episode.sh`·`config.sh`.
원형 정의는 `bench-2026-06/episode-runbook.md` §0~§3.)

**명령 재구성 금지 원칙**: 러너는 아래 스크립트의 호출자이지 명령 작성자가 아니다.
프롬프트는 tasks JSON의 `prompt` 필드를 `jq -r`로 **글자 그대로** 추출한다(한 글자도
추가/수정 금지 — 캠페인 1의 v4 회차가 정확히 이 실수로 무효화됐다). arm별 변환은
결정적 치환만 허용한다(아래 baseline 접두 제거가 유일한 예).

### 7-1. 호출 계약

```bash
harness/run-one-episode.sh "ITER|REPO|ARM|TASK|REP"
# 예: run-one-episode.sh "ds-iter1|django|claude-sonnet|d3|1"
```

- **멱등성**: `<EPISODE_ID>.metrics.json`이 이미 있으면 skip — 중단된 매트릭스는
  안전하게 재개된다.
- 전체 매트릭스: `harness/run-matrix.sh <iteration> [concurrency]` —
  `ARMS="claude-sonnet-base codex-gpt55-base"`로 baseline arm 선택.
- 산출물(jsonl/stderr/metrics.json)은
  `<OUT_DIR> = $BENCH_ROOT/results/<iteration>/<repo>/` 아래 보존.

### 7-2. config.sh 고정값

```bash
BINARY=/Users/buyong/workspace/private/buyong-mcp/apps/codemap-search/target/release/codemap-search
BENCH_ROOT=/tmp/benchmark-data
MCP_CONFIG=$BENCH_ROOT/mcp-codemap.json
TIMEOUT_S=600
ALLOWED_TOOLS="mcp__codemap-search__search,mcp__codemap-search__overview,mcp__codemap-search__read,mcp__codemap-search__find,mcp__codemap-search__grep"
DISALLOWED_TOOLS="Bash,Read,Glob,Grep,Edit,Write,WebFetch,WebSearch,Task,NotebookEdit,TodoWrite,Workflow,Agent,Skill"

# baseline arm (빌트인 전용, MCP 미설정) — claude는 codex 셸과의 동등 조건으로 Bash 포함
ALLOWED_TOOLS_BASE="Bash,Read,Glob,Grep"
DISALLOWED_TOOLS_BASE="Edit,Write,WebFetch,WebSearch,Task,NotebookEdit,TodoWrite,Workflow,Agent,Skill"
# baseline 프롬프트 변환: tasks JSON prompt에서 이 접두만 기계 제거 (전 과제 동일 접두 — 검증됨)
MCP_PROMPT_PREFIX="codemap-search MCP 도구를 사용해서 "
```

### 7-3. arm별 고정 명령

타임아웃은 `perl alarm`으로 실집행한다 — macOS 기본에 `timeout(1)`이 없다.

**claude-sonnet (pure MCP)**:

```bash
(cd "$REPO_PATH" && perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
  claude -p --model sonnet --setting-sources "" \
  --mcp-config "$MCP_CONFIG" --strict-mcp-config \
  --allowedTools "$ALLOWED_TOOLS" --disallowedTools "$DISALLOWED_TOOLS" \
  --output-format stream-json --verbose "$PROMPT") < /dev/null > "$JSONL" 2> "$ERRLOG"
```

- `--setting-sources ""`: 사용자/프로젝트 설정(훅 포함) 미로드. `--safe-mode`는
  사용 금지 — 명시적 `--mcp-config`까지 차단함이 프로브로 확인됐다.
- `ToolSearch`는 차단하지 않는다(하니스 메커니즘 — strict-mcp-config + 차단 목록
  하에서 우회 수단이 못 됨).

**claude-sonnet-base (baseline — MCP 미설정 + 빈 strict mcp-config로 외부 MCP 차단,
빌트인만 허용)**:

```bash
(cd "$REPO_PATH" && perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
  claude -p --model sonnet --setting-sources "" \
  --strict-mcp-config \
  --allowedTools "$ALLOWED_TOOLS_BASE" --disallowedTools "$DISALLOWED_TOOLS_BASE" \
  --output-format stream-json --verbose "$PROMPT") < /dev/null > "$JSONL" 2> "$ERRLOG"
```

**codex-gpt55 (pure MCP)**:

```bash
perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
  codex exec -C "$REPO_PATH" --skip-git-repo-check --ignore-user-config --ephemeral \
  -s read-only -m gpt-5.5 -c model_reasoning_effort="medium" \
  -c approval_policy="never" \
  -c "mcp_servers.codemap-search.command=\"$BINARY\"" \
  -c 'mcp_servers.codemap-search.args=["mcp"]' \
  --json "$PROMPT" < /dev/null > "$JSONL" 2> "$ERRLOG"
```

**codex-gpt55-base (baseline — `mcp_servers` 설정 자체를 전달하지 않음; 유일한
도구는 셸)**:

```bash
perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
  codex exec -C "$REPO_PATH" --skip-git-repo-check --ignore-user-config --ephemeral \
  -s read-only -m gpt-5.5 -c model_reasoning_effort="medium" \
  -c approval_policy="never" \
  --json "$PROMPT" < /dev/null > "$JSONL" 2> "$ERRLOG"
```

**baseline 프롬프트 변환** (MCP 유도 접두의 기계 제거 — 재타이핑 금지 원칙 유지):

```bash
case "$ARM" in
  *-base) PROMPT="${PROMPT#"$MCP_PROMPT_PREFIX"}" ;;
esac
```

### 7-4. 실패 처리

- exit code 142 → `harness_error: "timeout"`. 그 외 비0 종료 → `exit_<RC>`.
  출력 jsonl이 jq로 파싱 불가 → `parse_failure`.
- 하니스 수준 실패(타임아웃 제외)만 **1회** 재시도 — 오답·타임아웃은 재시도 금지
  (playbook §4-5).

### 7-5. 사전 인덱싱 + 배선 점검 (repo당 1회, 에피소드 시작 전)

```bash
cd <REPO_PATH> && $BINARY index .
```

- 인덱스 포맷 sidecar(`.codemap/index/codemap.format`)가 현재 포맷(2026-06 기준
  `v7-owner-tokens-indexed`)인지 확인.
- 사전 인덱싱된 미변경 repo는 writer를 획득하지 않아 **동일 repo 병렬이 안전**하다
  (동시 8프로세스 실측 0에러 — playbook §4).
- (선택) 배선 사전 점검: claude 명령에서 프롬프트를 "사용 가능한 도구 이름만
  나열해. 도구를 호출하지 마."로 바꿔 1회 실행 → 출력에
  `mcp__codemap-search__search` 등 5종이 보여야 한다.

### 7-6. warmup(파일럿) 게이트 — 본 매트릭스 진입 조건

(출처: `bench-2026-06/episode-runbook.md` §5. 본실행과 같은 동시성으로 통과해야
하며, fable이 판정한다 — §2.)

1. 두 arm 모두 에피소드 정상 종료 + 출력 파싱 가능
2. 각 transcript에 codemap-search MCP 호출 ≥ 1 (MCP 배선 증명)
3. claude transcript에 훅/사용자 설정 오염 없음 (`Serena MCP Tool Policy` 부재)
4. claude arm에서 빌트인 파일/셸 도구 사용 0 (pure 보장)
5. 메트릭 JSON이 §6 표준 스키마대로 추출되고 rubric 채점 가능
6. tantivy lock 에러 / MCP 연결 에러 0

추가 게이트 항목: 토큰 필드 존재 검증(캠페인 4부터), 제품 출하 문자열의 ground
truth 기계 대조(§4 원칙 6).

## 8. 캠페인 결과 (2026-06 통합)

> 최종 갱신: 2026-06-12 · Cargo 패키지 버전 0.1.0 (pre-release; 측정 회차는
> 릴리스가 아니라 인덱스 포맷 버전 `v3` → `v7-owner-tokens-indexed`로 식별)

**요약.** 코딩 에이전트(Claude Sonnet과 Codex GPT-5.5, 모두 CLI 경유)가 대형
오픈소스 저장소 — SurrealDB, ollama, ClickHouse, 그리고 hold-out으로 Django +
Strapi — 의 실제 코드 내비게이션 과제를 codemap-search MCP 서버만으로 풀었다
(CLI 빌트인 파일/셸 도구 차단). transcript에 대한 측정→진단→수정→재측정 루프는
에이전트의 도구 사용 방식을 바꿨다: 약한 `search` 주변의 grep 루프 공회전에서 첫
호출 정답으로. 구체적으로 캠페인 2의 두 iteration 사이에 Claude arm의 중앙값 턴이
43–53% 감소하고 중앙값 응답 바이트가 51–58% 감소했으며(Codex arm은 이미 효율적이라
변화 없음), 정답률은 77/79 → 80/80이 됐다. 캠페인 3은 그 수정들을 미접촉 언어
2종(Python, TypeScript)의 *미접촉·더 어려운* 과제 20개로 재시험했다: 단일
iteration에 80/80 correct, 사실상 모든 에피소드에서 첫 `search` 응답이 정답 파일을
실었다 — 캠페인 2의 효율 프로파일이 전이됐고(Claude 중앙값 3–4턴), 더 어렵게 만든
과제 세트도 어차피 포화했다(한계 절 참조). 캠페인 4는 오래 약속한 **baseline
arm** — 같은 CLI를 빌트인 도구로 제한, 같은 20과제, 같은 하니스 — 을 마침내
실행했고 역시 80/80: 이 과제 등급에서 정답률은 MCP 부가가치를 보여주지 못하며,
효율 신호는 CLI별로 갈린다(Claude+MCP: 턴은 줄지만 응답은 무거움; Codex+MCP: 턴
동등, hard 과제의 셸 출력 폭주가 사라짐). 이 절은 무효 회차·폐기 메트릭·이 수치가
주장할 수 있는 한계까지 포함해 지금까지의 모든 결과를 통합한다.

### 8-1. 먼저 읽을 것 — 이 수치가 말하는 것과 말하지 않는 것

- **자가 벤치마크.** 도구 작성자가 설계·실행·채점했다. 채점은 과제별 고정
  rubric(`correct` / `partial` / `wrong`)을 적용한 LLM 에이전트가 수행했고, 최난도
  에피소드들은 더 강한 모델의 별도 적대적 패스에서 저장소 원본과 대조해 재채점
  — 역시 같은 작성자가 실행 — 불일치 0(캠페인 2에서 15 에피소드; 캠페인 3에서
  40, 표본 답변의 모든 인용을 저장소와 줄 단위 재확인). 제3자 재현은 없다.
- **baseline 비교는 이제 존재한다 — 그리고 도구에 유리하지 않다.** 캠페인 4는
  같은 두 CLI를 빌트인 도구만으로(claude: Bash/Read/Glob/Grep; codex: shell) 같은
  20과제에 실행했다: 역시 80/80 correct, 함정 포함. 이 과제 등급에서 도구의
  부가가치는 정확도가 *아니라* 구조적이고 CLI 의존적이다(캠페인 4 절 참조).
  codemap-search가 "빌트인 grep/read를 이긴다"는 주장은 여전히 지지되지 않는다.
- **최근 과제 세트 3개 전부 포화.** 캠페인 2 마지막 iteration에서 모든 arm이 모든
  과제를 풀었고, 의도적으로 더 어렵게 재구축한(다단계 흐름, alias 간접, 동적
  dispatch, 3,500줄 파일) 캠페인 3에서도, 그리고 측정 대상 도구 없이 돈 캠페인 4
  baseline arm에서도 다시 그랬다. 이 과제들에서 정답률은 더 이상 arm 간·도구 스택
  간 변별을 못 한다; 효율 메트릭(턴·바이트·토큰·도구 믹스)만 한다. 그래서 이
  세트들은 유용한 회귀 게이트이자 쓸모없는 자랑 메트릭이다. 변별 가능할 만큼
  어려운 과제 세트 구축이 이제 최상위 백로그다.

### 8-2. 측정 대상

codemap-search는 코드 내비게이션용 자립형 Rust MCP 서버(stdio)다: tree-sitter 심볼
추출이 심볼·docstring·문자열 리터럴에 대한 tantivy BM25 인덱스(식별자 분할
토크나이제이션)를 공급한다. 검색 결과는 줄 번호 스니펫 + depth-1 caller/callee
어노테이션을 렌더한다. 도구 5종: `search`, `grep`, `read`, `overview`, `find`.
약 10개 언어 패밀리(Rust, Python, TypeScript/TSX, JavaScript, Go, Java, Kotlin,
C, C++, GAS assembly)에 걸친 21개 소스 파일 확장자를 인덱싱하며 1 MiB 초과 파일은
건너뛴다.

"pure MCP"에 대한 한 가지 명확화: 서버 자체가 범용 `grep`·`read` 도구를 출하하므로
CLI 빌트인 차단이 에이전트를 fallback 없이 고립시키는 것은 아니다. 이 벤치마크가
시험하는 것은 *서버 전체*의 충분성이며 — 흥미로운 신호는 도구 믹스와 턴 구조의
이동(grep 복구 루프 감소, 첫 호출 `search` 정답 증가)이지 단순 과제 완수가 아니다.

모든 벤치마크 에피소드는 에이전트 1 · 과제 1 · 신선한 CLI 세션 1이다:

```
"Using the codemap-search MCP tools, <task>. Cite exact file paths and
line numbers in your answer. Do not modify any files."
```

과제는 읽기 전용 코드 내비게이션 질문 — 정의 찾기, call site 열거, 파일 간 흐름
추적, 설정 기본값·에러 출처 찾기 — 이며 각각 사전 검증된 ground truth(`file`, 줄
범위, 증거 인용)를 갖는다. ground truth는 플레인 `rg`/수동 읽기로만 수립하고,
측정 대상 도구로는 절대 수립하지 않는다.

### 8-3. 캠페인 한눈에 보기

| # | Campaign | Repos | Arms | Scale | Headline |
|---|---|---|---|---|---|
| 1 | SurrealDB, rounds v3→v7 | surrealdb (Rust, ~2,700 files) | codex gpt-5.5 | 6 tasks × 2 reps × 5 rounds (1 round invalidated) | answer file in the *first* `search` response: 9/12 → 12/12 |
| 2 | C/C++, iter1→iter2 | ollama (Go + C/C++), ClickHouse (large modern C++) | claude sonnet + codex gpt-5.5 | 2 arms × 10 tasks × 2 reps × 2 repos = 80 episodes/iteration | 77/79 → 80/80 correct; claude median turns −43% / −53% (by repo) |
| 3 | Django + Strapi (hold-out) | django (Python), strapi (TypeScript) | same 2 arms | 80 episodes, harder task mix, single iteration | 80/80 correct; campaign-2 fixes held out-of-sample (claude median 3–4 turns, first-call answers); saturated again |
| 4 | Django + Strapi baseline (ds-base1) | same snapshots | claude + codex, **built-in tools only** (no MCP) | 80 episodes, same tasks/harness as campaign 3 | also 80/80 correct — accuracy shows no MCP added value; efficiency signal splits by CLI |
| 5 | Response-diet loops (ds-iter2 → ds-iter3) | same snapshots | same 2 MCP arms, 6 render fixes then 3 compensation fixes | 2 × 80 episodes, same tasks/harness | iter2: Claude *compensated* the per-response savings away (+16% via whole-file reads). iter3 (signature abbreviation + alias normalization + anchor cap): compensation absorbed — Claude −18%, Codex −14% vs pre-diet, 80/80 held both times |

캠페인 2·3의 버전: claude CLI 2.1.175, `claude-sonnet-4-6` 구동; codex CLI
0.139.0, gpt-5.5 reasoning effort "medium". 캠페인 1은 같은 codex 구성이지만 당시
CLI 버전이 기록되지 않았다. 대상 저장소는 2026-06 스냅샷이다(커밋 핀은 한계 절
참조).

### 8-4. 캠페인 1 — SurrealDB (v3 → v7): `search`가 답을 싣게 만들기

단일 arm(codex gpt-5.5, reasoning medium), 고정 6과제 × 회차당 2 rep, 순차 실행.
측정→수정 5회차. 불량 회차까지 포함한 전체 회차 궤적:

| Round | Change under test | Median turns (12 episodes) | Note |
|---|---|---|---|
| v3 | baseline (after `readOnlyHint` fix) | 9 | snippets/annotations active in 1/24 detail responses |
| v4 | symbol-matching + indexing fixes | — | **invalidated**: a prompt-reconstruction mistake dropped the tool-steering phrase; `search` was used in 3 of 182 calls; the round measured nothing |
| v5 | same code, prompts restored | 12 | annotations now active in 89.5% of detail responses — richer responses initially *cost* turns |
| v6 | name-evidence gate on partial matches | 12.5 | one task's response shrank 33.2 KB → 15.6 KB; answer exposed on turn 1 in 12/12 episodes |
| v7 | line numbers in snippets + label rewording | 9.5 | the citation bottleneck, not discovery, was eating the turns |

| Metric (12 episodes/round) | v3 (before) | v7 (after) |
|---|---|---|
| Correct answers | not scored | 12/12 |
| Answer file present in first `search` response | 9/12 (mostly as a bare one-line tail entry) | 12/12 (with snippet + line numbers) |
| Detail responses with snippets / caller annotations¹ | 1/24 (4%) | 87.5% / 75% |
| Median turns | 9 | 9.5 |
| Turns on simple/medium tasks (t1 / t2 / t5, rep1·rep2) | 11·7 / 9·9 / 8·9 | 5·5 / 7·7 / 7·8 |
| Errors / duplicate calls / shell bypasses | 1 / 0 / 0 | 0 / 0 / 0 |

¹ 분모 24 = 회차 전체의 detail 렌더 응답 수(에피소드당 2); v3 칸은 기록된 통합
활성화 횟수, v7 칸은 기능별 비율이다.

거의 평탄한 종단 중앙값(9 → 9.5)은 실제 이야기를 가리며, 그 이야기는 궤적 표가
말해준다: 응답을 풍부하게 만들자 중앙값이 먼저 *나빠졌고*(v5–v6: 더 많이 주면
에이전트는 더 많이 검증한다), v7의 인용 수정 — 스니펫 줄 번호 — 이 풍부함의
이득을 전부 유지한 채 도로 끌어내렸다. v3의 턴은 *재검색 공회전*(`search`가
파일명만 반환, 에이전트가 grep/read로 후퇴)이었고 v7의 턴은 *검증 심화*(첫
`search`가 답을 싣고, 남은 턴은 답을 더 완전하게 만들었다 — 예: 한 과제는 예시
call site 1곳 인용에서 KV 백엔드 5종 전부 열거로). 턴 수만 봤다면 캠페인 전체를
무승부로 불렀을 것이다; "몇 번째 턴이 처음 답을 노출했나"가 실제 변화를 보여줬다.

남은 알려진 약점: 다단계 흐름 추적(4-hop 호출 체인)은 여전히 25–33턴이 들었다 —
depth-1 caller/callee 맥락으로는 접히지 않는다. 이것이 후속 백로그(더 깊은
call-chain 지원)와 캠페인 3의 어려운 과제 설계를 형성했다.

### 8-5. 캠페인 2 — ollama + ClickHouse (iter1 → iter2): 두 모델, 160 에피소드

두 arm — claude sonnet과 codex gpt-5.5 — 이 각각 repo마다 10과제 × 2 rep을
pure-MCP로 수행. 전체 매트릭스 1회(80 에피소드) → 제품 수정 3건 → 같은 매트릭스
재실행.

**어떤 측정보다 먼저**, ClickHouse 인덱싱 스모크 테스트가 파서 블로커를 적발했다:
tree-sitter-cpp가 참조 반환 선언자(`int& f()`, `T& operator=`)를 named field가
아닌 positional children으로 노출해, 해당 심볼 전부가 조용히 미인덱싱됐다 —
ClickHouse `src/`에서만 정의 줄 약 3,053개. 같은 게이트에서 파서 버그 2건이 더
나왔다(함수 내부 "vexing parse" 가짜 심볼 — 전체 추출 심볼의 8.5% — 과 in-class
private 메서드의 exported 오표시). 이 게이트 없이 측정했다면 파서 구멍을 검색
레이어 탓으로 돌렸을 것이다.

#### 정확도

| Arm × repo (n=20 each) | iter1 | iter2 |
|---|---|---|
| claude · ClickHouse | 18 correct, 2 partial | 20/20 |
| codex · ClickHouse | 19 correct, 1 n/a* | 20/20 |
| claude · ollama | 20/20 | 20/20 |
| codex · ollama | 20/20 | 20/20 |
| **Total** | **77/79** | **80/80** |

\* 에피소드 1건이 provider 측 장애를 만났다 — CLI 이벤트 스트림이 빈 채로 유지되다
1,558 s에 실행이 종료됐다. 정확도 분모와 latency 중앙값에서 제외한다.

#### 효율 (같은 과제, 수정 3건 적용 후)

| Metric | iter1 | iter2 | Δ |
|---|---|---|---|
| Median turns, claude (ollama / ClickHouse) | 9.5 / 7 | 4.5 / 4 | −53% / −43% |
| Median turns, codex (ollama / ClickHouse) | 4 / 4 | 5 / 3.5 | flat (n=2 reps/task; not distinguishable from noise) |
| Median MCP response bytes, claude (ollama / CH) | 33,130 / 42,884 | 16,326 / 18,197 | −51% / −58% |
| Median MCP response bytes, codex (ollama / CH) | 22,482 / 13,094 | 16,591 / 13,143 | −26% / flat |
| Median episode duration, claude (ollama / CH) | 69.5 s / 47 s | 38.5 s / 37 s | −45% / −21% |
| Median episode duration, codex (ollama / CH) | 41.5 s / 28 s | 33.5 s / 31 s | −19% / +11% |
| Total tool calls, claude (ollama / CH) | 218 / 239 | 104 / 66 | −52% / −72% |
| Total tool calls, codex (ollama / CH) | 110 / 79 | 108 / 80 | flat |
| `read` parameter errors (whole iteration) | 52 | 5 | −90%; the remaining alias was fixed right after the campaign |
| Duplicate (identical) tool calls | 8 | 1 | |
| Shell bypasses / context contamination / blocked-builtin attempts | 0 / 0 / 0 | 0 / 0 / 0 | |

중앙값은 짝수 크기 표본에 대한 값으로 반올림 표기했고, codex·ClickHouse iter1
중앙값은 n/a 에피소드를 제외한다. 수정들은 Claude arm의 실패 모드를 겨냥했고,
Codex 행이 평탄한 이유가 그것이다 — Codex는 애초에 그 실패 모드를 보이지 않았다
(명시적 파라미터를 설정하고 스키마를 더 엄격히 따른다; 교훈 2·4 참조).

대표적인 에피소드 반전:

- **c7 — "ClickHouse가 `INFINITE_LOOP`를 던지는 모든 곳 찾기".** iter1: 한 rep이
  `search` 25회·`grep` 9회(도구 호출 49회, 319 s)를 쓰고 러너 컨텍스트 한도에
  걸려 partial — 모든 grep이 맨 파일 목록만 반환했기 때문. iter2: 두 rep 모두
  3–8턴에 throw 지점 7곳 전부 열거.
- **c8 — "`StorageFactory::get` 찾기".** iter1: 실패 검색 10회, 19턴, partial.
  iter2: 2번째 `search`에서 발견, 6턴, correct.

적대적 검증 패스는 분석 자체도 재검증했고, 분석 모델의 root-cause 서사 2건이
뒤집혔다(점수는 유지됐다; 설명이 틀렸다). 이 문서의 수치는 iteration 분석
보고서에서 복사한 것이 아니라 에피소드별 원시 메트릭에서 재산출했다.

### 8-6. 캠페인 3 — Django + Strapi (ds-iter1): hold-out 시험

캠페인 2의 효율 이득은 in-sample이었다 — 모든 수정이 같은 과제에서 진단되고
재측정됐다. 캠페인 3은 일반화 여부를 시험했다: 같은 두 arm, 같은 하니스 조건,
그러나 미접촉 저장소 2곳·미접촉 언어 2종(Python, TypeScript)의 새 과제 20개,
의도적으로 더 어렵게 구성(repo당 2 easy : 4 medium : 4 hard, 4-hop 흐름 추적,
alias 간접 정의, 동적 provider dispatch, 3,561줄 마이그레이션 파일, 과제별 함정
답 포함). 캠페인 사이에 제품은 수정하지 않았다 — 새 수정이 아니라 전이를
측정한다.

#### 결과 (단일 iteration, 80 에피소드)

| Metric | claude-sonnet | codex-gpt55 |
|---|---|---|
| Correct | 40/40 | 40/40 |
| Mean turns (all / hard tasks) | 4.1 / 6.1 | 6.1 / 9.0 |
| Median turns (django / strapi) | 4 / 3 | 4.5 / 6 |
| Median episode duration (django / strapi) | 22 s / 28 s | 23 s / 29.5 s |
| Median MCP response bytes (django / strapi) | 12.8 KB / 16.9 KB | 19.8 KB / 22.7 KB |
| Mean first-answer turn | 1.1 | 1.0 |
| Tool mix (search/grep/read/overview/find) | 73/38/50/3/0 | 48/70/122/2/3 |
| `read` parameter errors / duplicates / bypasses / contamination | 0 / 0 / 0 / 0 | 0 / 0 / 0 / 0 |

표본 밖에서 유지된 것:

- **첫 호출 발견.** 양 arm 모두 mean first-answer turn ≈ 1 — owner-token·리터럴
  인덱싱·기본 파라미터 수정이 Python/TypeScript 코드로 무수정 전이됐다.
- **효율 프로파일.** Claude의 중앙값 3–4턴·12.8–16.9 KB 응답은 완전히 새로운 재료
  위에서 캠페인 2 iter2 수준(4–4.5턴, 16.3–18.2 KB)과 일치한다.
- **강제 변환(coercion) 레이어.** 캠페인 2 iter1은 `read` 파라미터 에러 52건을
  기록했다; 캠페인 3은 0건.
- **다단계 흐름이 더는 절벽이 아니다.** 캠페인 1에서 25–33턴이 들던 과제
  등급(4-hop 체인)이 여기서는 hard 과제 평균 6.1–9.0턴에 안착했다.

변별하지 못한 것: 정답률. 양 arm 100%, 함정 포함 — 심어 둔 async-variant /
raw-queryset / hash-vs-compare / 타입 선언 미끼에 넘어간 에피소드가 없다. 남은
신호는 스타일이다: codex는 같은 답에 도달하면서 `read`를 2.4배(122 vs 50) 쓰고
hard 과제당 약 3턴을 더 썼다 — claude가 `search` 한 번 + 표적 read에 기대는
곳에서 grep+read 검증을 선호했다.

채점은 캠페인 2와 같은 규율의 확장판이다: 채점 배치 8개(sonnet, 기계적 rubric
적용), 이어 **표본 40 에피소드**에 대한 적대적 재채점 게이트(hard 8과제 전부 +
medium 2과제, 인용된 모든 file:line을 저장소 원본과 재대조 — 줄 검사 약 300회),
그중 12개는 오케스트레이션 모델이 직접 독립 재검증. overturn 0. 인용 내용 플래그
5건이 표면화됐고 cosmetic으로 판정됐다(답변 측 markdown 줄 라벨 옮김, ±1; 도구가
반환한 줄 번호 자체는 정확).

운영상 이것은 재작성된 하니스의 첫 캠페인이었다: 80-에피소드 풀 매트릭스가
**wall-clock 약 5분**(동시성 8, 에피소드 중앙값 25 s, 동일 repo 병렬 실행, lock
에러 0)에 돌았다 — 캠페인 2의 직렬·에피소드별 에이전트 운영의 iteration당
2.4–4.0 h 대비. 채점 + 재채점은 서브에이전트 토큰 약 0.45M을 소모했다 — 캠페인 2
iteration당 약 3M 대비.

절차 부채, 공개: 캠페인 2 회고는 "캠페인 3부터" 커밋 핀을 약속했다; 측정 스냅샷이
다시 `.git` 없이 출하되어 SHA가 기록되지 않았고 ground truth는 이 스냅샷에만
결속된다(한계 7 유지). 채점 전에 조기 생성된 2차 iteration 디렉터리(루프 규칙
위반)는 하니스의 멱등 재개 아래 바이너리가 섞이기 전에 적발·삭제됐다; 그것이
동기가 된 iteration 명명 규칙은 이제 playbook에 있다.

### 8-7. 캠페인 4 — baseline arm (ds-base1): 도구가 실제로 더하는 것은?

한계 항목 1이 요구한 그대로: 같은 스냅샷, 같은 20과제, 캠페인 3과 같은 하니스
조건·동시성 — 단 에이전트는 빌트인 도구만 받는다. `claude-sonnet-base`는
Bash/Read/Glob/Grep 허용에 MCP 차단(`--strict-mcp-config`, MCP 설정 없음);
`codex-gpt55-base`는 `mcp_servers`를 아예 받지 않는다 — 유일한 도구는 셸. 캠페인
3과의 의도적 설계 차이 3건은 전부 runbook에 기록됐다: 과제 프롬프트의 MCP 유도
접두를 기계 제거(결정적·일률적 `${PROMPT#prefix}` — 재타이핑 금지 원칙 유지),
codex 셸과의 동등 조건으로 claude에 Bash 부여, 측정 사본의 `.codemap` 인덱스
디렉터리 격리(baseline grep이 측정 대상 인덱스를 읽지 못하도록; 캠페인 후 복원).
purity는 에피소드별 기계 검증: `mcp__codemap` 문자열 0, MCP tool-call 이벤트 0,
web 사용 0, 거부 도구 시도 0.

이 캠페인은 또한 **contestant 토큰 사용량을 1급 메트릭으로 승격**했다(claude
`stream-json` usage 필드; codex `turn.completed` usage 이벤트 합산) — 4개 arm
전부에서 추출. 추출기 변경은 어떤 실행 전에도 이전 iteration 데이터와
byte-for-byte 회귀 검증(`del(.tokens)` diff = 0)을 거쳤고, 캠페인 3 에피소드는
아래 토큰 컬럼을 위해 보존 JSONL에서 재추출했다.

#### 결과 (80 에피소드, 단일 패스; 캠페인 3 MCP 수치 병기)

| Metric (n=40/arm) | claude MCP | claude base | codex MCP | codex base |
|---|---|---|---|---|
| Correct | 40/40 | 40/40 | 40/40 | 40/40 |
| Mean turns (all / hard) | 4.1 / 6.2 | 4.6 / 7.4 | 6.1 / 9.1 | 6.1 / 8.9 |
| Median turns (django / strapi) | 4 / 3 | 2 / 5 | 4.5 / 6 | 5.5 / 5 |
| Median duration (django / strapi) | 22 s / 28 s | 13.5 s / 25 s | 23 s / 29.5 s | 28 s / 32.5 s |
| Mean tool-result bytes | 23.8 KB | 13.7 KB | 28.6 KB | 51.6 KB |
| Mean first-answer turn (null) | 1.15 (0) | 1.56 (1) | 1.0 (0) | 1.37 (5) |
| Mean output tokens | 1,483 | 1,221 | 855 | 1,075 |
| Mean total input tokens¹ | 55.1k | 60.5k | 75.9k | 66.1k |
| Total tool calls (mix) | 164 (search 73) | 185 (grep 90, read 72) | 245 (read 122) | 243 (shell) |
| Duplicates / contamination / purity violations | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |

¹ claude: input + cache-read + cache-creation; codex: `input_tokens`(cached 포함).
두 CLI는 서로 다른 것을 보고한다 — CLI 내부 비교만, CLI 간 비교 금지.

#### 정직하게 읽기

- **정답률 부가가치: 이 세트에서는 0.** 두 baseline 모두 심어 둔 함정 포함 전부
  풀었다. pure-MCP 캠페인 2회 + baseline 캠페인 1회의 일치된 결론: 이 등급의
  작성자 설계 내비게이션 과제는 도구가 정확도에 도움이 되는지 보여줄 수 없다.
- **Claude + MCP: 더 싼 검색 구조, 더 무거운 응답.** 턴 −11%(4.6→4.1), hard 과제
  턴 −16%(7.4→6.2), first-answer 1.56→1.15. 그러나 도구 결과 바이트
  +73%(13.7→23.8 KB), 에피소드 duration +28%, 출력 토큰 +21%; 입력 토큰만
  개선(−9%). 한 방 `search`가 grep→read 캐스케이드를 대체하고, 그 값을 응답
  무게로 치른다.
- **Codex + MCP: 동일한 노력, 꼬리 억제.** 턴·호출 수는 완전히 동등하다(양쪽
  6.1 / 약 244 호출). 평균 바이트 −45%(51.6→28.6 KB)는 전적으로 꼬리 효과 —
  baseline 중앙값이 오히려 *낮지만*, hard 과제가 셸을 통해 폭주한다(s7 104 KB,
  s8 85 KB, s10 69 KB 평균 도구 출력); MCP 응답은 유계로 유지된다. 출력 토큰
  −20%, 입력 토큰 +15%(역방향).
- **토큰 최적화 목표에서 일관된 승리 없음.** 입력/출력 델타가 CLI별로 반대
  방향을 가리킨다. "codemap-search가 에이전트 토큰 사용량을 줄인다"는 이 데이터로
  지지되지 않는다; 응답 크기 다이어트(스니펫·caller 컨텍스트 슬리밍)가 그 결과로
  나온 제품 백로그 항목이다.
- **메트릭 경고 하나 표면화**: baseline 에피소드 6개(전부 correct)에서 expected
  파일 문자열이 어느 도구 결과에도 나타나지 않았다(`first_answer_turn` null) —
  셸 파이프라인과 `Glob` 결과는 경로를 반드시 에코하지 않는다. baseline
  first-answer 수치는 보수적으로 취급할 것.

채점·검증은 캠페인 3과 동일했다: sonnet 8-배치 기계 rubric 채점, 같은 40-에피소드
표본 구성의 적대적 재채점 게이트 — rubric 요건 실패 0, 3개 에피소드의 인용 불일치
12건 전부 cosmetic 판정(±1 보조 인용, 스니펫 라벨 드리프트), **overturn 0**.
매트릭스 전에 warmup 게이트(4 에피소드, 기준 6종 전부 + 토큰 필드 존재) 통과;
80-에피소드 실행은 wall-clock 314 s, 하니스 실패 0. baseline arm은 제품 변경의
영향을 받지 않으므로 ds-base1은 이후 수정 루프의 고정 기준선이다.

### 8-8. 캠페인 5 — 응답 다이어트 (ds-iter2 → ds-iter3): 에이전트는 보상한다

baseline 캠페인의 가장 분명한 제품 신호는 응답 무게였고, 렌더 레이어 변경 6건이
들어갔다(인덱스 포맷 무변경; 스캔·랭킹·coercion 무변경): exact-identifier(tier-1)
우선의 쿼리 매칭 심볼 앵커링과 2–3줄 컨테이너 요약, symbol-overflow 파일에서 쿼리
매칭 심볼의 스니펫 보장, 중복 caller 블록 dedup("same as above"), 정의 ≥5개
이름의 caller에 대한 정직한 한 줄 생략(신규 `caller_omit_def_threshold`),
ranked-tail 트림 50→12, `.yarn`/minified-bundle grep 제외, 그리고 재읽기 확인을
막는 도구 설명 예시.

매트릭스 **전에** 보존된 변경 전 바이너리와의 라이브 A/B 프로빙이 결함 2건을
잡았다: 토큰 중첩 매칭이 너무 느슨했고(`select` 서브토큰이 `get_select` 등에 풀
스니펫을 부여 — 전체 식별자 tier-1 우선으로 교체), caller dedup이 렌더 순서가
아닌 스캔 순서를 키로 잡아 원본이 렌더되지 않은 "same as above" 참조를
만들었다(렌더 타임·렌더 순서·emit-committed dedup으로 이동). 세 번째 운영 교훈:
인덱서가 첫 스냅샷을 발행하기 전의 호출은 빈 caller 인덱스를 받는다(라벨 없음,
"top-level/unindexed" 귀속) — 프로브는 warm-up을 기다려야 한다; 매트릭스 80
에피소드 전부 warming 응답이 없음을 기계 검증했다.

동일 쿼리 A/B 절감은 실재했다: −34%(클래스 preamble 케이스), −53%(반복 caller
케이스), −27%, −22%, fallback 케이스는 설계상 +8%(이전에 없던 답 스니펫).
매트릭스는 다른 이야기를 했다:

| Metric (n=40/arm) | claude iter1 | claude iter2 | codex iter1 | codex iter2 |
|---|---|---|---|---|
| Correct (re-score gate) | 40/40 | **40/40 (0 overturns)** | 40/40 | **40/40 (0 overturns)** |
| Mean turns | 4.1 | 4.2 | 6.1 | **5.8** |
| Mean tool-result bytes | 23.8 KB | **27.6 KB (+16%)** | 28.6 KB | **23.5 KB (−18%)** |
| Mean total input tokens | 55.1k | 60.2k (+9%) | 75.9k | **68.6k (−10%)** |
| s10 (hardest) turns / bytes | 8 / 30.7 KB | 9.5 / 19.6 KB | 25.5 / 104.5 KB | **19.5 / 54.5 KB** |

도구별 분해가 그 갈림을 설명한다. Claude의 `search` 합계는 거의 움직이지
않았지만(73→80 호출에 745→780 KB — 호출당 바이트는 오히려 감소), `read` 합계가
53%(120→184 KB), `grep`이 74%(53→92 KB) 올랐다: 앵커링이 한 줄 스텁으로 강등한
맥락을 Claude가 도로 가져왔다 — 최대 s7 도구 결과가 9.0 KB search 응답에서
30.4 KB 통파일 read로 교체됐다. 이미 read 중심이던 Codex는 보상하지 않고 절감을
전부 챙겼다 — 최악 케이스 과제의 바이트 −48%·턴 −24% 포함. 정답률은 어디서나
유지(함정 포함)됐으므로 회귀는 순수하게 경제적이다.

**교훈 8, 이 캠페인의 기여: 에이전트는 보상한다.** 캠페인 1은 더 풍부한 응답이
처음에 턴을 늘린다는 것을 배웠다(v5–v6); 이것은 그 거울상이다 — 더 빈약해진
응답은 *read*를 늘릴 수 있고, 응답 크기 최적화는 응답 단위 크기가 아니라
에이전트의 보상 행동을 포함한 에피소드 총 바이트로 판정해야 한다.

#### 두 번째 루프 (ds-iter3): 보상의 흡수

transcript 재진단이 iter2 회귀를 3개 기전으로 분해했고, 각각 두 번째(이자 2-루프
상한상 마지막) 수정 루프에서 고쳤다:

1. **시그니처 축약** (승인된 가설): 강등된 심볼을 맨 한 줄 스텁 대신 시그니처 +
   최대 3줄(tier-2) 또는 elision 마커가 붙은 시그니처 한 줄(비매칭)로 렌더 —
   그 부재가 Claude의 재읽기를 유발한 바로 그 맥락이다.
2. **`read` alias 키 정규화** (입증된 결함, 교훈 4의 미완 과제): Claude가 보낸
   `startLine`(camelCase)을 alias 맵이 조용히 무시했다 — snake_case였다면 3.3 KB
   범위 read였을 곳에서 한 에피소드에 30.4 KB 통파일 렌더 2회. alias 매칭은 이제
   변형 열거 대신 키 정규화(소문자화, `_`/`-` 제거)를 쓴다.
3. **파일당 anchor-snippet 상한** (`search_anchor_snippet_limit`, 기본 3):
   `save`·`send` 같은 흔한 쿼리 단어는 *많은* 심볼의 exact-match 이름이다 — 첫
   search 응답 하나가 풀 스니펫 25개(29.1 KB, rep 간 결정적)를 실었다. 초과
   앵커는 이제 스텁이 아니라 축약형으로 강등된다. 라이브 A/B 패스가
   symbol-overflow fallback 분기에 이 전부가 빠진 자체 렌더 경로가 있음을
   잡았다(두 분기가 렌더러 하나를 공유할 때까지 d9 프로브가 움직이지 않았다:
   30.1→14.1 KB).

결과, 같은 80 에피소드: Claude 에피소드 총 바이트 23.8→27.6→**19.5 KB**(다이어트
전 대비 −18%), `read` 바이트는 기준선 복귀(193→135 KB — 보상이 흡수됨), 입력
토큰 55.4k 복귀; iter2의 모든 폭주 과제가 회복됐다(s7 92.4→39.1 KB,
s8 62.2→40.4 KB·13.5→10턴, d9 49.7→27.5 KB). Codex는 다이어트 전 대비 −14%
유지. 정답률은 다시 80/80 — 다만 이번 회차 재채점 게이트가 시리즈 최초의
overturn 2건을 냈는데, 둘 다 엄격 방향의 *채점자* 오류였다(명백히 존재하는 절을
"누락"으로; rubric "no-penalty" 조항 미적용) — 전문 재판정으로 correct로 정정.
잘못된 방향으로 움직인 유일한 메트릭: Claude의 mean first-answer turn이
1.15→1.70으로 드리프트(모든 에피소드가 여전히 도달은 함) — 차기 캠페인 플래그.

**공개된 오염, 사후 적발·교정.** 도구 설명을 강화한 캠페인 5 루프 1 변경이 예시
문자열 — `compiler.py:1595→ def execute_sql(...)` — 을 추가했는데, 이는 *문자
그대로 과제 d7의 step-4 ground truth*다. 오케스트레이터가 진단 중이던 바로 그
에피소드에서 수정 브리프로 복사했고; iter2·iter3 내내 모든 search 호출의 도구
설명에 들어 있었다. 세 iteration의 d7 에피소드 12개 전부에 대한 forensic: 줄
번호는 어느 답이 인용하기 전에 항상 도구 *결과*에 (2–5회) 먼저 나타났다 —
블라인드 복사 0. 예시는 중화됐고(렌더 출력 byte-identical; 대체 문자열을 전 과제
ground truth와 대조), d7은 최종 바이너리에서 힌트 없이 재실행됐다: 여전히 4/4
correct, 함정 답 0 — 정답률 결론은 무영향. 그러나 Claude의 힌트 없는 d7 턴은
11–12(no-hint iter1 범위)로 돌아왔다, 힌트가 있을 때의 약 7 대비: **외견상의 d7
턴 개선은 렌더 작업이 아니라 누출이었다.** iter3의 Claude 집계를 힌트 없는 값으로
보정하면: mean turns 4.05→**4.25**(iter1과 parity, 개선 아님 — 그 주장은 철회),
mean bytes 19.5→**18.4 KB**(바이트 절감은 −22.5%로 오히려 *커진다*). Codex는
힌트 무관(내내 6–8턴). 오염 체크리스트를 위한 교훈: 제품이 contestant에게
출하하는 모든 문자열 — 도구 설명, 에러 메시지, 예시 — 은 측정 표면이다; 측정
전에 과제의 expected file/line/symbol과 기계적으로 diff하라.

### 8-9. 실제로 효과를 낸 것 — 전이 가능한 MCP 설계 교훈 7가지

이 교훈들은 이 도구 너머로 일반화된다; 이 절이 이 결과 기록이 존재하는 주된
이유다.

1. **ToolAnnotations에 `readOnlyHint`를 설정하라 — 아니면 codex가 전부 조용히
   취소한다.** 비대화형 실행에서 codex는 승인이 필요한 MCP 호출을 자동 취소한다.
   이 한 줄 어노테이션이 들어가기 전에는 다른 어떤 개선도 측정조차 불가능했다.

2. **기본 파라미터 값이 가장 뜨거운 코드 경로다.** claude grep 호출 122건 중
   116건(95.1%)이 기본 `output_mode`를 썼는데, 이는 줄 번호 없는 파일명을
   반환했고 — 에이전트는 그것을 복구하러 search/read로 되돌아갔다(위 c7 참조).
   기본값을 줄 번호 포함 content로 뒤집은 것이 캠페인 2의 단일 최대 승리였다.
   에이전트는 압도적으로 최소 인자로 도구를 호출한다; 문서를 읽는 인간이 아니라
   에이전트를 위해 기본값을 조율하라.

3. **렌더하는 모든 스니펫에 줄 번호를 넣어라.** 과제가 "정확한 줄 번호 인용"을
   요구하는데 스니펫이 그것을 보여주지 않으면, 에이전트는 이미 본 범위를 번호만
   얻으려고 재읽기한다. 캠페인 1에서 풍부해진 search 응답은 중앙값을 12.5턴까지
   *밀어올렸다*(궤적 표의 v6); 스니펫을 `read`와 같은 `  3049→ …` 형식으로
   렌더하자 풍부한 응답을 유지한 채 9.5로 돌아왔다 — 그 캠페인에서 단일 변경이
   만든 최대 효과.

4. **파라미터는 강제 변환하라; alias 두더지잡기 금지.** claude 계열 에이전트는
   습관적으로 `path`, `file`, `start_line`/`end_line`을 보냈다(iter1의 28개
   에피소드에서 hard 에러 52건 + 조용히 무시된 범위 파라미터 48건; codex: 0 —
   스키마 규율은 모델마다 다르다). 숫자를 JSON 문자열(`"228"`)로도 보내는데,
   엄격한 `as_u64()`는 이를 조용히 "범위 없음"으로 만든다 — 통파일 렌더, 그다음
   크기 제한 에러. 항구적 수정은 alias를 하나씩 쫓는 것이 아니라 관용적 coercion
   레이어(문자열→숫자, alias 맵)였다.

5. **BM25 단독으로는 짧고 흔한 멤버 이름을 랭킹하지 못한다 — owner 토큰을
   인덱싱하라.** `get`은 변별력이 없어 "StorageFactory get" 쿼리조차 실패했다.
   각 멤버의 소유 타입 이름(과 그 분할 토큰)을 멤버 문서에 인덱싱하자 그 과제가
   실패 검색 10회에서 2번째 호출 발견이 됐다.

6. **에이전트가 실제로 인용하는 것을 인덱싱하라: 문자열 리터럴과 enum variant.**
   "기본 포트는 어디 정의되나"·"read-only 쓰기에 어떤 에러가 발생하나" 같은
   과제는 인용 값 조회다. enum variant는 심볼이 아니었고 리터럴은 미인덱싱이라
   각각 grep 우회 4–5턴이 들었다; 둘 다 인덱싱(리터럴 256자 상한, 심볼·docstring
   보다 낮게 부스팅)하자 정의 파일이 최상위 검색 히트가 됐다.

7. **이름당 비싼 스캔을 유계화하고, 에이전트가 신뢰할 수 있는 절단 라벨을
   써라.** 전역 caller-scan 예산은 한 핫 이름이 나머지를 굶기게 했다 — 실제 call
   site 19곳인 심볼이 절단 마커도 없이 "no direct caller observed"로 렌더됐다.
   이름당 예산(하한 25) + 이름당 절단 플래그가 고쳤다. 관련해서, caller 라벨
   "(approximate, name-match only)"는 사실은 정확한 위치를 에이전트가 재검증하게
   만들었다; "(file:line positions exact; name-match attribution approximate)"로
   고쳐 쓰자 불필요한 재읽기가 멈췄다. 에이전트는 라벨을 문자 그대로 읽는다 —
   표현의 정밀성은 성능 기능이다.

### 8-10. 측정 통제

- **Pure-MCP 격리.** claude: `--setting-sources ""`(사용자 설정/훅 미로드),
  `--strict-mcp-config`, codemap-search 5종 도구만의 allowlist, 빌트인
  파일/셸/웹 도구 전체 denylist. codex: `--ignore-user-config --ephemeral
  -s read-only`에 codemap-search를 유일한 MCP 서버로. (고정 명령 전문은 §7-3.)
- **오염 검사는 기계로.** 모든 transcript를 작성자 로컬 훅 설정에서만 등장하는
  마커 문자열로 스캔하고, 발견 시 에피소드를 무효화한다. 보존된 캠페인 2/3/4/5
  에피소드 496개 전부 0건(160 + 파일럿 4 + 매트릭스 80 + baseline warmup 4 +
  baseline 매트릭스 80 + 다이어트 루프 2회 각 warmup 4 + 매트릭스 80); 캠페인 1
  transcript는 이 기계 검사 도입 이전이다. 대상 repo 자체의
  `AGENTS.md`/`CLAUDE.md`는 측정 사본에서 격리한다 — codex는 기본으로 repo
  AGENTS.md를 주입해, 두지 않으면 contestant를 코치하게 된다. baseline 캠페인은
  추가로 측정 사본의 `.codemap` 인덱스 디렉터리를 격리해 빌트인 grep이 측정 대상
  인덱스를 읽지 못하게 했다.
- **축어 프롬프트.** 과제 프롬프트는 JSON에 저장하고 `jq -r`로 추출한다 — 절대
  재타이핑하지 않는다. 이 규칙은 v4 회차가 정확히 그 실수로 무효화됐기 때문에
  존재한다. (§4 원칙 5, §7.)
- **기계 추출 메트릭.** 턴·도구별 호출 수·응답 바이트·first-answer turn·중복
  호출·우회 시도는 CLI의 구조화 이벤트 스트림(claude `stream-json`, codex
  `--json`)에 대한 `jq` 프로그램으로 산출한다 — 모델 추정이 아니다. 추출기는
  재사용 전에 이전 iteration 데이터로 회귀 검증했다. (스키마는 §6.)
- **채점 규율.** rubric은 실행 전에 과제별로 작성한다(줄 번호 허용 오차, 요구
  call-site 수, `wrong` 처리되는 함정 답 명시). 3-class 채점; 하니스 실패는 1회
  재시도, 오답은 재시도 없음. 최난도 에피소드는 더 강한 모델의 적대적 패스로
  저장소 원본과 대조해 재채점한다: 지금까지 수행된 두 패스에서 점수 변경
  0(캠페인 2에서 15 에피소드, 캠페인 3에서 40).
- **멱등·타임아웃 강제 하니스.** 에피소드당 600 s를 `perl alarm`으로
  강제한다(macOS 기본에 `timeout(1)` 없음); 완료 에피소드는 재실행 시 skip되어
  중단된 매트릭스가 안전하게 재개된다. 캠페인 2 초기엔 타임아웃이 선언만 되고
  강제되지 않았고 — stuck provider 호출이 1,558 s를 달린 경위가 그것이다 — 이후
  실행에서 그 강제 공백을 닫았다.
- **제품 출하 문자열도 측정 표면이다.** 캠페인 5는 진단 transcript에서 복사한
  설명용 예시를 통해 한 과제의 ground-truth file:line을 search 도구 설명에
  누출시켰다(캠페인 5의 공개 참조). 사후 적발·중화·forensic 한정(블라인드 인용 0;
  정답률 무영향; 한 과제의 턴 메트릭 정정)됐고, 오염 체크리스트에 도구
  설명/instructions를 전 과제 expected file/line/symbol과 diff하는 항목이
  추가됐다. (§4 원칙 6으로 규범화.)
- **공개된 측정 아티팩트.** 에피소드 duration은 provider API 지연을 포함한다.
  "first-answer-turn rate"는 모델 답변 스타일에 민감함이 입증됐고 — 한 arm의
  외견상 하락은 보조 하니스 도구의 호출이 턴으로 집계된 것이지 제품 회귀가
  아니었다 — 회차 간 불안정 지표로 취급한다. iter2 에피소드 80개 중 55개의 자유
  텍스트 답변 요약이 축어 인용이 아닌 paraphrase였다; 표본 점검(전수 감사
  아님)에서 인용 위치의 왜곡은 발견되지 않았다.

### 8-11. 캠페인 비용

C/C++ 캠페인은 오케스트레이션 측 LLM 토큰 약 7.2M(계획·데이터셋 구축·채점·분석·
적대적 리뷰 — 측정 iteration당 약 3.0M과 약 2.9M)에 contestant CLI 에피소드 약
165회를 소모했다. 초기 캠페인의 wall-clock은 약 75–83%가 에피소드 자체가 아니라
에피소드별 에이전트 오케스트레이션 오버헤드였다(순수 실행: 전체 중 약 2.45 h);
이후 하니스를 스크립트 구동 병렬 실행(동시성 8, 에피소드 단위 셔플)으로
재작성했다. 캠페인 3이 그 재작성을 검증했다: 80-에피소드 매트릭스가 wall-clock 약
5분에 돌았고, 채점 + 40-에피소드 적대적 재채점에 서브에이전트 토큰 약 0.45M이
들었다. 여기의 비용·wall-clock 수치는 실행 로그에서 재구성한 근사치다 — 기계
추출되는 에피소드별 메트릭과 달리.

### 8-12. 한계

1. **baseline arm은 이제 존재하고, "빌트인 grep/read보다 낫다"는 여전히 주장하지
   않는다.** 캠페인 4가 측정했다: 정답률은 동일(모든 곳 100%, 세트 포화)하고,
   효율 델타는 실재하지만 혼합적·CLI 특이적이다 — claude는 적은 턴을 무거운
   응답과 교환하고, codex는 꼬리 억제 외에 가시적인 교환이 없다. 포화된 세트에서
   baseline 캠페인이 보여줄 *수 없는* 것은 빌트인이 실패하는 곳에서 도구가
   돕는지다 — baseline이 100% 아래로 떨어질 만큼 어려운 과제 세트가 필요하다.
2. **자가 벤치마크, LLM 채점.** 작성자 설계 과제, 에이전트 적용 rubric, 적대적
   표본 검증 — 같은 작성자에 의해. 독립 재현을 환영한다; 데이터셋·rubric·하니스
   스크립트는 이 저장소에 있다(§9 참조).
3. **캠페인 2 수정은 in-sample이었다; 캠페인 3이 hold-out이었고, 유지됐다.** 모든
   캠페인 2 수정은 iter1 transcript에서 진단되고 *같은* 과제로
   재측정됐다(owner-token 수정은 과제 c8에서 동기를 얻어 과제 c8로 검증).
   캠페인 3은 수정된 제품을 미접촉 언어 2종의 미접촉·더 어려운 과제 20개로
   재시험했다: 효율 프로파일과 첫 호출 발견이 전이됐다(캠페인 3 절 참조). 포화된
   세트에서의 hold-out 성공이 보여줄 수 없는 것은 headroom이다 — 그것엔 더 어려운
   세트나 baseline arm이 필요하다.
4. **소표본.** repo당·arm당 10과제 × 2 rep; partial 1건이 arm 정답률을 5%p
   흔든다. arm별 델타는 방향성 지표로 취급할 것.
5. **포화, 두 번.** 캠페인 2 iter2가 80/80을 쳤고, 캠페인 3 세트는 의도적으로 더
   어렵게 재구축(2 easy : 4 medium : 4 hard, 다단계 흐름, alias 간접, 동적
   dispatch, 함정 답)됐는데 — 그래도 80/80으로 포화했다. 이 등급의 작성자 설계
   내비게이션 과제에서 정답률은 더 이상 변별하지 못한다; 향후 변별은 baseline
   비교, depth-2 call-chain 요구, 모호한 자연어 개념 쿼리, exact-line-label
   rubric에서 와야 한다.
6. **실행 간 분산은 완전히 분리되지 않는다.** iter1 vs iter2는 같은 과제를
   재사용한다; 델타 일부는 모델 비결정성이다(ClickHouse의 codex는 더 정확해지며
   11% *느려졌고*; ollama의 codex는 턴이 하나 늘었다).
7. **대상 repo 커밋이 핀되지 않았다 — 캠페인 3 포함.** 측정 저장소는 git
   메타데이터 없는 2026-06 스냅샷이었고 커밋 SHA가 기록되지 않았다. 캠페인 2
   회고는 "캠페인 3부터" 핀을 약속했다; 캠페인 3의 스냅샷이 다시 `.git` 없이
   출하되어 약속은 미이행됐고 여기에 공개한다. 따라서 줄 번호 ground truth는 그
   스냅샷들에 결속되며, 독자가 오늘 `git checkout`할 수 있는 무엇에도 결속되지
   않는다. SHA 기록은 이제 playbook 사전 측정 절차의 체크리스트 항목이다.
8. **읽기 전용 내비게이션 과제만.** 편집·리팩터링·빌드 과제는 측정하지 않았다.
9. **회차 1개가 무효화됐고**(v4, 프롬프트 실수) **에피소드 1개가
   제외됐다**(provider 장애). 어느 쪽도 보고 수치에 포함되지 않으며, 둘 다 여기
   공개한다.
10. **에피소드별 transcript는 아카이브되지 않는다.** 이 수치 뒤의 JSONL
    transcript·메트릭은 측정 머신에 존재하지만 repo에 커밋되지 않았고 장기 보존
    되지 않을 수 있다. 위의 무결성 주장(오염 0, 재채점 불일치 0)은 오늘 검증
    가능하지, 무기한은 아니다.

### 8-13. 다음 과제 — 변별 가능한 과제 세트

캠페인 4는 이 과제 세트가 허용하는 한도까지 baseline 질문을 닫았고, 캠페인 5의
수정 루프 2회는 응답 다이어트 질문을 닫았다: 보상을 흡수한 두 번째 루프 후 양 arm
모두 다이어트 전 자신을 바이트에서 이겼고(Claude 힌트 보정 −22.5%, Codex −14%)
정답률 100%·턴 parity를 유지했으며, Claude-vs-빌트인 바이트 격차는 +73%에서 약
+34%로 줄고 Codex-vs-shell은 −53%로 벌어졌다. 루프 상한은 소진됐다. 최상위
*측정* 백로그 항목은 여전히 **변별**이다: 빌트인 전용 arm이 실제로 100% 아래로
떨어지는 과제 등급 — depth-2 callee 요구, 모호한 자연어 개념 쿼리, 다수 후보에
흩어진 답, exact-line-label(±0) rubric — 그래야 baseline 비교가 스타일이 아니라
정확도에 대해 말할 수 있다. 더 작은 후속: anchor 중심 렌더링 하의 Claude
first-answer-turn 드리프트(1.15→1.70), 채점자 근거의 rubric 조항 인용(이번
캠페인에서 시리즈 최초의 재채점 overturn이 나왔고, 둘 다 채점자 측), 커밋 핀
스냅샷(`.git` 제거 전 `git rev-parse HEAD`), 그리고 아직 fixture 수준인 assembly
추출을 위한 asm 실측 저장소. ds-base1 baseline 수치는 제품 독립적이며 고정
기준선으로 유지된다.

## 9. 재현 가이드

캠페인 자산의 위치·역할은 §5 자산 포인터가 단일 출처다. 재현 절차:

1. release 바이너리를 빌드한다.
2. 대상 저장소 스냅샷을 직접 핀한다 — **`git rev-parse HEAD`를 기록한 뒤**
   `.git`을 제거(§8 한계 7의 교훈) — 하고 측정 전용 경로에 체크아웃한다.
3. repo당 1회 사전 인덱싱 후 포맷 sidecar를 확인한다 (§7-5).
4. 하니스 설정(`harness/config.sh`)에 바이너리·repo·tasks 경로를 지정한다.
5. tasks JSON을 작성하거나 재검증한다 — ground truth는 측정 대상 도구가 *아닌*
   도구(플레인 `rg`/수동 읽기)로만 수립한다.
6. 4-에피소드 파일럿(warmup) 게이트를 통과한다 (§7-6).
7. 매트릭스를 실행한다: `harness/run-matrix.sh <iteration> [concurrency]` —
   `ARMS="claude-sonnet-base codex-gpt55-base"`로 baseline arm 선택. 에피소드는
   멱등·재개 가능하며, 채점·집계 스크립트와 4-arm 토큰 추출이 포함된다.
8. verify → 채점 → 재채점 → 집계는 §2 단계 구성과 §3 모델 배치대로 진행한다.

"재현"에 대한 정직한 단서 하나: 측정 스냅샷이 커밋 핀되지 않았으므로(§8 한계 7)
캠페인 1·2 데이터셋의 줄 단위 ground truth는 신선한 클론과 어긋난다. 저장소
자산이 가능하게 하는 것은 직접 핀한 스냅샷 위에서 *같은 절차*를 도는 것이다.

과제 데이터셋은 대상 저장소(SurrealDB, ollama, ClickHouse, Django, Strapi)의 짧은
증거 인용을 포함한다; 해당 발췌는 원 저장소의 라이선스를 따른다.
