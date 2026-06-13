# baseline arm 캠페인 runbook — django + strapi (2026-06-12)

> 상위 기준 문서: `../benchmark-workflow.md` — 단계·모델 배치·실행 원칙은 그 문서가
> 우선한다. 이 runbook은 캠페인 세부(arm 구성·설계 결정·재개 절차)만 보유한다.

## 현재 상태 — **캠페인 완료 (2026-06-12)**

ds-base1 본실행·채점·재채점·집계·비교 분석까지 종결. 결과 요약:

- **80/80 correct (양 baseline arm 100%)** — 재채점 게이트 overturn 0
  (`gate-verdict-ds-base1.md`). 정답률 축에서 MCP 부가가치 변별 실패(3연속 포화).
- 효율 신호는 CLI별 반대 방향 — claude: MCP가 턴 −11%·hard 턴 −16%·first_answer
  단축, 대신 도구 결과 바이트 +73%·duration +28%. codex: 턴 동등, MCP가 hard 과제
  셸 출력 폭주를 억제(평균 바이트 −45%), 입력 토큰은 +15%. 토큰 일관 절감 증거
  없음. 상세 수치·판정: `../benchmark-workflow.md` §8 캠페인 4 절, 인과 서사:
  `../benchmark-evolution.md` 캠페인 4 절.
- 토큰 메트릭(4-arm)이 `extract-metrics.sh`에 추가됨(기존 필드 diff 0 회귀 검증,
  보존본 동기화 완료). ds-iter1 토큰은 보존 jsonl에서 재추출해
  `/tmp/benchmark-data/results/ds-base1/comparison/ds-iter1-tokens.json`에 별도 보관.
- warmup(ds-base-warmup) 게이트 6종 + 토큰 필드 통과 기록, 본실행 wall 314s
  (동시성 8, 하니스 실패 0). 배선 프로브 통과 기록은 `harness/probe-base-*.out`.
- **`.codemap` 원위치 복원 완료** (quarantine-codemap → 측정 사본). baseline은
  제품 수정의 영향을 받지 않으므로 ds-base1 결과는 향후 루프의 고정 기준선으로
  재사용한다 — 재측정 불필요.
- 원시 산출물(비커밋, 측정 머신): `/tmp/benchmark-data/results/ds-base1/`
  (metrics 80 + scoring + rescore 증거 + comparison).

### 재개 절차 (완료 — 기록용, 순서 고정)

1. **자산 확인 (최우선)**: `/tmp/benchmark-data/{django-main,strapi-develop,tasks-django.json,tasks-strapi.json,harness,results/ds-iter1}`
   존재 확인. **저장소 스냅샷이 사라졌으면 중단하고 사용자에게 보고** — 스냅샷은
   commit 핀이 없어 새 체크아웃과 줄 번호 ground truth가 어긋나므로 그대로 재개할
   수 없다.
2. **토큰 메트릭 보강 (opus 코딩 발주)**: `extract-metrics.sh`의 4-arm 전부에
   contestant 토큰 필드 추가 (claude stream-json의 usage, codex jsonl의 토큰 이벤트 —
   실제 필드명은 기존 ds-iter1/ds-base-pilot jsonl에서 확인). 기존 필드·값 무변경
   (추출 회귀 금지 — 기존 jsonl 1건씩 재추출해 diff로 검증).
3. **잔재 삭제**: 위 ds-base1·ds-base-pilot 잔재 제거.
4. **warmup**: `ARMS="claude-sonnet-base codex-gpt55-base" run-matrix.sh ds-base-warmup 8 --pilot`
   → workflow 문서 게이트 6종 + 토큰 필드 존재까지 검증. fable이 판정.
5. **본실행**: `ARMS="claude-sonnet-base codex-gpt55-base" run-matrix.sh ds-base1 8`
   — sonnet 슬라이스 러너(workflow 병렬)가 고정 스크립트 호출, 메인 루프는 CLI 직접
   실행 금지 (workflow 문서 §4).
6. **verify → 채점 → 재채점 → 집계**: workflow 문서 단계·모델 배치대로.
   채점 배치 생성은 `ARMS=... build-scoring-batches.sh ds-base1`.
7. **비교 분석·정리**: ds-iter1 대비 정답률·턴·duration·바이트·토큰 비교 →
   `../benchmark-workflow.md` §8(캠페인 결과)·회고 갱신 → `.codemap` 복원.

ds-iter1(MCP 2-arm, 80/80 correct)과 동일 과제·저장소·하니스 조건에서 "빌트인 도구만"
arm 2종을 측정해 codemap-search의 부가가치를 직접 비교하는 캠페인. playbook §7-1
(baseline arm 최우선) 이행. baseline은 제품 수정의 영향을 받지 않으므로 1회 측정 후
향후 루프에서 재사용한다.

## 매트릭스

- arm 2종: `claude-sonnet-base`(빌트인 `Bash,Read,Glob,Grep` 허용, MCP 미설정 +
  `--strict-mcp-config`), `codex-gpt55-base`(mcp_servers 설정 자체를 미전달, 기본 셸만)
- repo 2종 × 10과제 × 2rep = 80 에피소드, 회차 이름 `ds-base1`, 동시성 8
- CLI·모델·타임아웃(600s)·동시성·셔플 — ds-iter1과 동일 조건

## 설계 결정 (ds-iter1과의 차이는 이 3가지뿐)

1. **프롬프트 변환**: tasks JSON의 프롬프트는 전 과제가
   `"codemap-search MCP 도구를 사용해서 "` 접두로 시작한다(20/20 기계 확인).
   baseline에서는 이 접두를 셸 prefix-strip(`${PROMPT#"$MCP_PROMPT_PREFIX"}`)으로
   **기계 제거**만 한다 — 과제 본문·인용 요구·수정 금지 문구는 무변경. 재타이핑 금지
   원칙(playbook §2)은 변환이 결정적·전수 동일하므로 유지된다.
2. **claude baseline에 Bash 포함**: codex의 유일한 도구가 셸이므로 동등 조건.
   Edit/Write 등 변경 도구는 차단(과제는 읽기 전용).
3. **`.codemap` 격리**: 측정 사본의 인덱스 디렉터리를
   `/tmp/benchmark-data/quarantine-codemap/`로 이동 — baseline 에이전트의 grep이
   인덱스 내용을 읽으면 오염이다. **캠페인 종료 후 원위치 복원 필수**
   (`mv quarantine-codemap/django-main.codemap django-main/.codemap`, strapi 동일).

## purity 검증 (파일럿·본실행 공통, 기계 적용)

- transcript에 `mcp__codemap` 문자열 등장 0건 (claude 프로브에서 MCP 도구 미노출 확인됨)
- codex transcript에 web 도구 사용 이벤트 0건 — codex exec 기본값에서 web search는
  비활성이고 MCP arm 캠페인과 동일 조건이지만, 모델이 도구 목록에 `web.run`을
  나열하므로 사용 0건을 사후 기계 검증으로 보증한다
- 오염 문자열(`Serena MCP Tool Policy`) 0건, harness_error 0건

## 실행·역할 배치 (playbook §4-3·§6)

- 에피소드 실행: 스크립트(`ARMS="claude-sonnet-base codex-gpt55-base" run-matrix.sh
  ds-base1 8`) — 멱등 재개, perl alarm 600s
- 스크립트 실행·기계 추출 감독: **sonnet 러너 (Workflow 서브에이전트)** — 메인 루프
  (fable)는 contestant CLI를 직접 실행하지 않는다. 게이트 판정·비교 분석만 fable.
- 채점: ds-iter1과 동일 rubric을 sonnet 8배치(`ARMS=... build-scoring-batches.sh
  ds-base1`)로 기계 적용 — rubric은 도구 불문이므로 무수정 재사용
- 재채점 게이트: sonnet 증거 수집 + fable 판정 (§5 분업)

## 메트릭 주의 (extract-metrics.sh baseline 분기)

- `tool_calls` 키가 arm별로 다름: claude-base `{bash,read,glob,grep}`, codex-base `{shell}`
- `mcp_response_bytes_total`은 필드명을 유지하되 "도구 결과 바이트 총합" 의미
- `denied_builtin_attempts`: 허용 목록 밖 도구 시도(mcp__* 포함 — purity 카운터 겸용)
- `shell_bypass_calls`: baseline에서는 셸이 정규 도구이므로 0 고정
- ds-iter1과의 비교 가능 지표: score, turns, duration_s, first_answer_turn,
  duplicate_calls, 도구 결과 바이트(의미 주석 달아 비교)

## 비교 분석 계획 (본실행 후)

- 1차 지표: 정답률 (MCP 100% 대비 baseline ?%) — 과제·난이도별 분해
- 2차 지표: 턴/duration/바이트 — 동일 정답 도달 비용 비교 (특히 hard 과제)
- first_answer_turn: baseline은 "정답 파일이 도구 결과에 처음 등장한 턴" 정의 동일 적용
- 타임아웃(600s 초과) 발생 시 harness_error="timeout"으로 분모 유지, 별도 보고
