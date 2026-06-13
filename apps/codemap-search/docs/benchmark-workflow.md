# codemap-search 성능 측정 — 단일 기준 문서

모든 벤치마크 캠페인이 따르는 공통 워크플로우, 운영 상세(격리 플래그·실측 함정·동시성·모델 배치), 메트릭 산출 정의, 고정 실행 명령, 사전 벤치마크(축 A/B) 결과, 그리고 2026-06까지의 캠페인 통합 결과를 담은 — "측정의 모든 것"을 한 곳에 두는 — 단일 기준 문서. 캠페인 인과 서사(관찰→가설→수정→재측정 — 사전 벤치 인과의 프롤로그 포함)·재채점 게이트 판정 기록·제품 개선 백로그는 `benchmark-evolution.md`가 보유한다. 이 문서와 다른 문서가 충돌하면 **이 문서가 우선**한다. (2026-06-12 사용자 확정)

> (2026-06-13) 영문 통합 결과 문서 `BENCHMARKS.md`는 본 문서 §9(캠페인 결과)· §10(재현 가이드)로 병합됐다. 같은 날 운영 플레이북(`agent-benchmark-playbook.md`), 캠페인 2 에피소드 runbook, baseline runbook, 사전 벤치 결과 문서(축 A/B· 스케일링 플랜)도 본 문서로 흡수됐다(흡수된 원본 파일들은 폐기 대상 — 현행 기준 아님). 절차·수치의 단일 출처는 이제 이 문서다.

문서 구성: §1 측정 정의 · §2 단계 구성 · §3 모델 배치 · §4 실행 원칙과 운영 · §5 자산 포인터 · §6 메트릭 산출 정의 · §7 고정 실행 명령 · §8 사전 벤치마크 (축 A/B) · §9 캠페인 결과 · §10 재현 가이드.

## 1. 측정 정의와 목적

- **무엇을**: codemap-search MCP 서버(검색·내비게이션 5종 도구)의 실전 성능.
- **누가(contestant, 고정)**: claude CLI(`--model sonnet`) + codex CLI(gpt-5.5, reasoning medium). 두 CLI 모두 격리 플래그로 사용자 설정·훅을 차단한 pure 구성.
- **왜 (목적 2가지)**:
  1. **codemap-search 성능 개선** — 측정→분석→수정→재측정 루프로 제품 결함과 변별 지표를 찾는다.
  2. **code-agent 토큰 사용량 최적화** — 동일 과제를 더 적은 턴·더 적은 도구 결과 바이트·더 적은 contestant 토큰으로 풀게 만드는 것이 개선의 최종 지표다. contestant 토큰 사용량(claude stream-json usage 필드, codex 토큰 이벤트)을 1급 메트릭으로 추출한다 — 추출 가능 여부는 warmup에서 검증.

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
| **warmup** | 본 세션 전 버그 적발. 파일럿 게이트 6종 기준(정상 종료/파싱, 도구 호출 ≥1, 오염 0, purity 보장, 스키마 추출+채점 가능, lock·연결 에러 0 — 기준 전문은 §7-6)을 본실행과 같은 동시성으로 통과해야 함. **본 세션의 하니스 실패는 허용되지 않는다** — 실패 가능성은 전부 warmup에서 소진한다 | sonnet 러너 + fable 판정 | 본실행 동일 동시성 |
| **run** | 에피소드 실행. 고정 스크립트(run-one-episode.sh, §7) 호출만 허용 — 명령 재구성·프롬프트 수정 금지. 슬라이스 러너가 배치 단위로 분담 | sonnet 러너 (workflow 병렬) | 동시성 8~16, 에피소드 단위 |
| **verify** | 산출물 무결성 기계 검증: metrics 전수 파싱, harness_error, 오염 문자열, purity(비허용 도구 흔적), 토큰 필드 존재 | sonnet verifier | run 완료 후 1회 |
| **채점** | 과제 rubric을 기계 적용해 correct/partial/wrong. 배치 단위(10 에피소드/에이전트), StructuredOutput 강제, 기입은 스크립트(write-scores.sh) | **sonnet** ×배치 | 배치 병렬 |
| **재채점** (반복 — 회차마다) | 어려운 과제 중심 표본 ≥10 + 비-correct 전건. sonnet이 인용 줄 전수 원본 대조·rubric 체크리스트를 기계 수집 → **fable이 판정** (수집·판정 분리로 교차 검증 유지) | sonnet 수집 + **fable** 판정 | 수집 병렬 |
| **집계** | 수치 집계는 jq 스크립트(aggregate-results.sh), 현상 보고는 sonnet — 상위 모델은 기계 작업에 미투입 | 스크립트 + sonnet | — |
| **개선 및 rerun** (반복 세션, 상한 2회) | ① 분석: 실패·비효율 에피소드를 (a)제품 (b)하니스/채점 (c)모델 한계로 분류, transcript 인용 의무 — sonnet. ② root-cause 정정: fix_hypothesis를 라이브 바이너리로 재현 검증 후 확정 — **fable**. ③ 수정: 브리프 기반 코드 수정(DO-NOT-FIX 목록·검증 의무 포함) — **opus**. ④ 추출 변경 시 FORMAT_VERSION 범프 + 재인덱스 → warmup 재실행 → run부터 재진입 | sonnet → fable → opus | 분석 배치 병렬 |

**루프 규율**: 상한 2회. 재진입 조건은 "정답률 100% 미만 **그리고** transcript로 입증된 제품 결함"일 때만. 차기 회차 디렉터리는 현 회차 채점·분석 완료 전 생성 금지 (멱등 재개 × 혼합 바이너리 오염 — §4-5 회차 규율).

## 3. 모델 배치 (확정)

원칙: **물량은 sonnet, 명세된 수정은 opus, 진실 판정은 fable.** 상위 모델은 '생산'이 아니라 '게이트'에 배치한다 — 각 단계의 산출물이 다음 단계의 입력이 되므로, 게이트에서 틀린 것이 통과하면 이후 모든 비용이 오염된다. 아래 배치는 2026-06 C/C++ 캠페인에서 실측 검증됐다(굳어진 경위 — 블로커 적발·오진 정정 — 는 `benchmark-evolution.md` 캠페인 2 절 참조).

| 역할 | 모델 | 근거 (캠페인 실측) | 토큰 규모 |
|---|---|---|---|
| 계획·오케스트레이션·브리프 작성·게이트 판정(warmup/재채점/종료 전 리뷰) | **fable** (메인 루프) | 결정 지점 판단과 브리프 품질이 전체 효율을 결정 | — |
| 측정 전 게이트 검증 | **fable** | opus 적대 리뷰 2회를 통과한 코드에서 블로커(참조 반환 누락) 적발 — 실저장소 인덱싱 스모크 포함 필수 | ~113k/회 |
| 데이터셋 + 루브릭 작성 | **fable** | 함정 보기·전수 검증 설계 → 160 에피소드 채점 분쟁 0건 | 70~94k/repo |
| 러너(run)·verify·파일럿·채점·집계 보고 (실행·추출 자체는 스크립트 — §4-3) | **sonnet** | 에피소드 162회 무손실·스키마 준수 100% — 단, 에피소드별 에이전트 방식은 wall clock 오버헤드 75~83%의 원인이었다(§4-3) | 채점 배치당 ~35k |
| 회차 분석 — 현상 발견 | **sonnet** | 고영향 결함(grep 기본값, read 별칭) 발견은 정확 | 회차 비용 포함 |
| 회차 분석 — root-cause 확정·정정 | **fable** | sonnet 분석에서 사실 오류 3건(근본 원인 오진 포함) 실측 — fix_hypothesis는 수정 라운드 진입 전 라이브 바이너리로 재현 검증 | 정정 패스 1회 |
| 코딩 작업(제품 수정) | **opus** | 4라운드 전부 경고 0·테스트 green·제약 위반 0, 브리프 밖 추가 원인도 자체 발견 | 70~102k/라운드 |
| 종료 전 적대 리뷰·표본 재채점 | **fable** | 재채점 15건(불일치 0) + 오진 정정 + DO-NOT-FIX 선별 | ~153k |
| contestant (측정 대상) | 고정: claude sonnet, codex gpt-5.5 medium | 하니스(§7)에 고정 — 오케스트레이션 모델과 분리 | 외부 비용 |

### 3-1. 모델별 가드레일 (실측된 실패 모드)

- **sonnet**: 제약 이탈이 실측됨(신규 테스트 금지를 어기고 7개 추가 → 제거). 브리프에 금지 사항을 명시하고 산출물을 기계 검증(StructuredOutput 스키마 강제, diff grep 검사)하라. 코드 수준 인과 추론은 시키지 말 것 — transcript 증거가 붙은 현상 보고까지가 적정선이다.
- **opus**: 진단이 정확할 때만 정확하다. 오진된 fix_hypothesis를 그대로 주면 라운드가 헛돈다(iter2 분석의 read 결함 오진이 실례 — fable 정정 없이 진행했다면 최종 라운드는 아무것도 못 고쳤다). 브리프에 이전 분석의 오진 정정을 명시적으로 전달하라.
- **fable**: 가장 비싸므로 게이트 지점에만 — 측정 전 1회, 분석 정정 1회, 종료 전 1회. 생산 루프(에피소드 실행, 집계)에 넣지 말 것.
- **오케스트레이터 자기 점검**: 재채점 게이트를 opus 등 하위 에이전트로 통째로 위임하지 말 것 — opus는 "명세된 수정" 전용이고, 게이트 판정은 fable 본인 몫이다 (django+strapi 캠페인에서 opus 위임 시도가 사용자 정정으로 회수된 실례). 물량이 크면 §4-7의 sonnet 증거 수집 + fable 판정 분업을 쓴다.

### 3-2. 브리프(발주) 규칙

- 수정 라운드: "정확히 이것만 고쳐라" 목록 + **DO-NOT-FIX 목록** + 검증 의무 (빌드·테스트·라이브 재현, 결과 원문 첨부)를 포함한다.
- 검증 라운드: "자가 보고를 믿지 말라" + 원본 대조 의무(저장소 원본 줄, transcript, 라이브 바이너리)를 포함한다.
- 모든 라운드 공통: 커밋 금지, 범위 밖 수정 금지, 최종 메시지는 orchestrator용 원시 데이터(파일:줄, 실측 출력 포함)로 지정한다.

## 4. 실행 원칙과 운영

### 4-1. 핵심 원칙

1. **병렬 우선**: 에피소드 단위 병렬(동일 repo 동시 포함 — 실측 안전, §4-4), 채점·증거 수집은 배치 병렬. 직렬화는 명시적 사유가 있을 때만.
2. **메인 루프(fable)는 contestant CLI를 직접 실행하지 않는다** — 실행은 workflow 서브에이전트(sonnet 러너)가 고정 스크립트를 호출하는 방식만 허용. 메인 루프는 계획·게이트 판정·브리프 발주만.
3. **에이전트는 판단에만, 실행·추출은 스크립트로** (실측 근거는 §4-3) — 러너는 스크립트 호출자이지 명령 작성자가 아니다. 에피소드별 에이전트 스폰 금지 (배치/슬라이스 단위만).
4. **실패 0 원칙**: 본 세션 하니스 실패는 결함이다. warmup에서 전 경로(배선·추출· 채점 가능성·토큰 필드)를 실제로 통과시킨 뒤에만 본 세션 진입. 본 세션 중 하니스 실패 발생 시 즉시 중단하고 원인을 warmup 항목으로 환류한다.
5. **측정 불변 조건**: 프롬프트는 tasks JSON에서 jq 추출(한 글자도 수정 금지; arm별 변환은 결정적 치환만 허용하고 본 문서 §7에 기록), 측정 사본 무수정, 사전 인덱싱 + 포맷 버전 확인, 체크아웃 시 commit SHA 기록.
6. **제품 출하 문자열도 측정 표면이다 — ground truth 기계 대조 의무**: 도구 설명·서버 instructions·예시·에러 메시지 등 contestant에게 노출되는 모든 제품 문자열을 측정 전(warmup 게이트에서) tasks JSON의 expected file/line/symbol과 기계 diff해 정답 단편 등장 0건을 확인한다. 개선 루프에서 제품 텍스트를 수정할 때는 발주 브리프의 예시 문구도 같은 검사를 통과해야 한다 — 진단 transcript의 구체 사례를 예시로 옮겨 쓰는 것 금지. (실측 근거: ds-iter2/3에서 도구 설명 예시가 d7 정답(파일·줄·시그니처)과 일치하는 누출이 사후 적발됐다. 정확도 결론은 무영향이었으나 d7 턴 지표가 오염돼 무힌트 재실행으로 보정했다 — `benchmark-evolution.md` §5.4, 본 문서 §9-8 공개 참조.)

### 4-2. 실측 함정 표 — 전부 실측으로 얻은 것, 건너뛰면 한 회차를 날린다

격리 플래그·호출 형태의 정본은 §7이다. 아래는 그 명령들이 지금 모양이 된 이유 — 2026-06 캠페인들에서 실제로 회차를 위협했던 함정과 대응의 목록이다.

| 함정 | 증상 | 대응 |
|---|---|---|
| `--safe-mode` | 명시적 `--mcp-config`까지 차단 → MCP 도구 0개 | `--setting-sources ""` + `--strict-mcp-config` 조합만 사용 |
| 사용자 훅 오염 | 훅 출력이 contestant 프롬프트에 주입 | 격리 후에도 transcript에서 훅 문자열 grep으로 매회 검증 |
| codex MCP 자동취소 | ToolAnnotations(`readOnlyHint`) 없으면 비대화형에서 전 호출 취소 | MCP 호출 0 + `requires_mcp_tool_approval`이면 회귀로 판정 |
| macOS `timeout` 부재 | 명령 즉시 실패(exit 127) | `perl alarm`으로 실집행(§7-3), wall clock은 `date +%s` |
| 프롬프트 재구성 | 도구 유도 문구 누락 → 측정 오염(v4 사고) | 프롬프트는 tasks JSON에서 jq로 추출, 한 글자도 수정 금지 |
| tantivy writer lock (정정됨) | 충돌을 우려한 "repo 내 순차"가 wall clock을 키움 — 실제 충돌 관측 0건 | 사전 인덱싱된 미변경 repo는 동일 repo 병렬 안전 (§4-4 실측 근거) |
| ToolSearch 집계 포함 | first_answer_turn/turns 왜곡 (0.8→0.4 가짜 회귀 사례) | ToolSearch(하니스 메커니즘)는 도구 호출로 세지 않는다 |
| answer_text 요약 | 메트릭 충실도 저하 (80건 중 55건 요약 사례) | 러너에게 축어 기록 강제, 길면 파일로 저장하고 경로 기록 |
| codex jsonl 무타임스탬프 | duration 산출 불가 | wall clock 측정 필수 |
| 채점 자가 신고 의존 | 점수 인플레이션 위험 | 회차 후 상위 모델이 어려운 과제 중심 표본 재채점 (§4-7) |
| claude `--disallowedTools` 가변 인자 | 플래그 값 바로 뒤에 프롬프트를 두면 프롬프트 단어가 거부 도구 목록으로 파싱 → "Input must be provided" 에러 | 비가변 플래그(`--output-format ...`)를 사이에 두는 검증된 어순 고정 (2026-06-12 실측) |
| 대상 repo의 `AGENTS.md`/`CLAUDE.md` | codex가 repo AGENTS.md를 기본 주입(claude도 cwd CLAUDE.md 로드 가능) → contestant 오염 | 측정 사본에서 사전 격리(이동). 신규 repo 점검 절차에 `ls AGENTS.md CLAUDE.md .cursorrules .claude` 포함 (strapi 실례) |

### 4-3. 매트릭스 운영 — 동시성·스크립트 경제학·타임아웃·재시도·규모

1. 매트릭스는 **에피소드 단위 병렬, 동시성 8~16 — 동일 repo 동시 실행 포함** (안전성 실측 근거는 §4-4). 배정 순서는 arm·과제 무작위 셔플 — 직렬 운영의 arm 교차 배치는 병렬에서는 셔플로 대체된다. 동시성 상한을 정하는 것은 제품이 아니라 contestant API rate limit과 로컬 자원이다. 에피소드당 메트릭 JSON을 디스크에 영속한다(§6 표준 스키마).
2. **스크립트 경제학**: 에피소드 실행은 고정 명령의 결정적 작업이다(§7 — 프롬프트 재구성 금지). 에피소드마다 러너 에이전트를 띄운 2026-06 초기 방식은 wall clock의 75~83%를 오버헤드(스폰·추출·채점 대기)로 만들었다 — 캠페인 wall 7.25h 중 에피소드 순수 실행 합은 2.45h(162개, 중앙값 37s/에피소드), 2레인 임계 경로는 1.37h에 불과했다. 하니스 명령은 동시성 N 스크립트로 실행하고 메트릭 추출도 jq 스크립트로 처리하라. 에이전트는 루브릭 채점·분석에만 배치 단위(예: 10 에피소드/에이전트)로 투입 — 토큰도 ~3M/회차에서 크게 준다.
3. **타임아웃을 실제로 강제하라**: 600s 상한을 선언하고도 캠페인 2 iter1에 1558s 에피소드가 기록됐다(미집행). 병렬에서는 꼬리 에피소드가 임계 경로를 지배하므로 타임아웃 집행이 wall clock에 직결된다. 현 하니스는 `perl alarm`으로 실집행한다(§7-3).
4. **재시도 규칙**: 하니스 수준 실패(프로세스 사망, MCP 연결 실패)만 1회 재시도. 오답·타임아웃은 재시도 금지. API 장애는 score=n/a로 분모에서 제외.
5. **규모 참고** (2-arm × 10문항 × 2rep × 2repo = 80 에피소드/회차): 직렬 운영 실측 wall 4.0h(iter1)·2.4h(iter2), 캠페인 전체 7.25h. **병렬 8 + 스크립트 실행 실측(django+strapi 캠페인): 80 에피소드 wall 약 5분**(에피소드 중앙값 25s, lock 에러 0). 채점+재채점 서브에이전트 토큰도 회차당 ~3M → ~0.45M로 절감. 외부 contestant 비용 별도.

### 4-4. 동일 repo 병렬 안전성 — 실측 근거 (2026-06-12 검증)

캠페인 2까지의 "tantivy writer lock 때문에 repo 내 순차" 규칙은 과잉 보수였다. 코드 경로 전수 분석과 라이브 프로브로 정정한다.

- **코드**: mcp 런타임에서 tantivy IndexWriter를 획득하는 지점은 `index.rs`의 `apply_index_updates` 한 곳뿐이고, 변경 파일이 없으면 writer 획득 전에 조기 반환한다(`index.rs:454-456`). LockFailure는 모든 경로에서 비치명 — warn 후 stale 서빙으로 강등(`index.rs:461-464`). 시작 시 sidecar·config 쓰기도 사전 인덱싱된 repo에서는 발생하지 않는다.
- **프로브**: 신선 인덱스에 MCP 프로세스 8개 동시 → 전원 정상 응답, lock 메시지 0건. stale(파일 touch 직후) 6개 동시 → 전원 정상 응답, `LockFailure: LockBusy` warn 5건(비치명, 1개 프로세스만 재인덱싱 수행), 인덱스 무손상.
- **전제 2가지**: (1) 측정 전 단일 프로세스로 사전 인덱싱을 끝내고 포맷 버전 일치를 확인한다(§7-5). (2) 측정 중 소스 트리를 변경하지 않는다. 전제가 깨져도 corruption은 없지만, 일부 프로세스가 stale 결과를 서빙해 측정이 오염된다.

### 4-5. 회차 이름·잔재 규율

측정→수정 루프는 상한을 미리 정한다(2026-06: 2회; 재진입 조건은 §2 루프 규율). 차기 회차(iterN+1) 실행은 현 회차의 채점·분석·수정이 끝나기 전에 시작하지 않는다. 조기 시작돼 중단된 회차 잔재는 반드시 삭제하거나 이름을 바꿔 보관할 것 — 멱등 재개(metrics 존재 시 skip, §7-1)와 결합하면, 제품 수정 후 같은 이름으로 본실행할 때 수정 전/후 바이너리가 섞인 오염 회차가 된다 (django+strapi 캠페인에서 19/80 잔재를 본실행 전에 적발·삭제한 실례).

### 4-6. 데이터셋 설계 원칙 (`benchmarks/bench-2026-06/tasks-*.json`이 견본)

- 과제 유형 스펙트럼: definition / definition+callers / concept / 다단계 flow / literal·config / error+발생지점. 난이도 easy:medium:hard ≈ 3:4:3.
- **루브릭은 기계 판정 가능해야 한다**: 허용 오차(±0~2줄)를 명시하고, partial/오답 경계와 함정 답안(오답 처리 기준)까지 적는다. 호출 지점 N곳 요구면 N+2곳 이상을 전수 grep으로 확보해 두고 전부 나열한다.
- 정답 확정은 빌트인 Read/Grep만 사용 — **측정 대상 도구(codemap-search MCP)로 ground truth를 만들지 않는다.**
- 과제는 인덱싱되는 파일만 대상(SOURCE_EXTENSIONS — md/CMake/xml/yaml 무효). gitignore된 경로(예: ollama의 llama/vendor)도 무효.
- 프롬프트 고정 프레임: `"codemap-search MCP 도구를 사용해서 <과제>. 정확한 파일 경로와 줄 번호를 인용해서 답해. 파일은 수정하지 마."`
- 함정 보기(distractor)를 의도적으로 심는다(예: `zkutil::ZooKeeper` vs `Coordination::ZooKeeper`) — expected에 함정과 오답 판정 기준을 같이 기록.

### 4-7. 분석·검증 사다리

- 회차 분석(sonnet): 실패 에피소드 원인을 (a) 제품 결함 (b) 하니스/채점 (c) 모델 한계로 분류, transcript 인용 필수. 같은 과제가 rep·arm을 가로질러 실패하면 (a) 우선 조사.
- **상위 모델 정정 패스를 분석 직후에 둔다**: 2026-06에서 sonnet 분석의 근본 원인 오진(별칭 문제로 진단, 실제는 문자열 숫자 파싱)이 fable 리뷰에서야 잡혔다. 오진이 수정 라운드에 들어가면 그 라운드는 헛돈다. 분석의 fix_hypothesis는 수정 전에 라이브 바이너리로 재현 검증할 것.
- 회차 후 재채점: 어려운 과제 중심 표본 ≥10개를 상위 모델이 루브릭 기계 적용 + 저장소 원본 줄 대조로 재채점. 유효한 분업: sonnet이 인용 줄 전수 대조 + rubric 체크리스트를 기계 수집하고, 판정은 fable이 직접 — 수집과 판정을 분리하면 물량을 키우면서도 게이트 품질이 유지된다 (django+strapi 캠페인: 표본 40, 불일치 0).
- 실패 0건(전 arm 100%) 회차는 분석 사다리의 원인 분류·정정 패스가 공집합으로 종결된다 — 대신 효율 지표(턴·바이트·도구 믹스·first_answer_turn) 분석과 포화 여부 판단을 회차 보고에 남길 것.

## 5. 자산 포인터

측정 자산은 세 갈래다: ① 인과·수치·설정의 **서술**은 본 문서 옆 `apps/codemap-search/docs/`의 세 마크다운(이 문서·`benchmark-evolution.md`·`configuration.md`)이 전부 인라인 보유, ② 에이전트 캠페인의 **기계 입력물**(데이터셋·MCP 설정·셸 하니스)은 `apps/codemap-search/benchmarks/`(문서 아님 — "서술 금지" 펜스, `benchmarks/README.md` 참조), ③ 사전 벤치마크(축 A/B)·scout 보고서 자산은 저장소 루트 `docs/benchmarks/`. 절차·함정·결과 수치는 전부 본 문서에 인라인돼 있으므로(§4·§6~§9), 아래는 문서 밖에 실체가 있는 자산만 나열한다. **기계 입력물(②)에서 어떤 주장도 도출하지 않는다 — 수치·인과의 단일 출처는 본 문서와 `benchmark-evolution.md`뿐이다.**

| 자산 | 위치 | 역할 |
|---|---|---|
| 설계 진화 회고 | `benchmark-evolution.md` | 캠페인 통합 회고(한국어) — 인과 서사(프롤로그·캠페인 1~5)·재채점 게이트 판정 기록·제품 개선 백로그의 단일 출처. 수치는 중복 게재하지 않고 본 문서 §8~§9를 참조 |
| 제품 설정 레퍼런스 | `configuration.md` | config 키 정의 — §9의 렌더 수정이 도입한 `caller_omit_def_threshold`·`search_anchor_snippet_limit` 등의 단일 출처 |
| 캠페인 1·2 데이터셋 | `benchmarks/bench-2026-06/tasks-surrealdb.md`(캠페인 1, 산문 — 하니스 이전), `tasks-ollama.json`, `tasks-clickhouse.json` | 과제·rubric·ground truth. 코퍼스·스냅샷에 결속된 정답 키이므로 문서가 아닌 기계 재료로만 보존한다(§7-8) |
| 캠페인 3·4·5 데이터셋 | `benchmarks/bench-2026-06-django-strapi/tasks-django.json`, `tasks-strapi.json` | hold-out 과제 20개 — 함정 답·rubric 포함 |
| claude arm MCP 설정 | `benchmarks/bench-2026-06/mcp-codemap.json`, `benchmarks/bench-2026-06-django-strapi/mcp-codemap.json` | §7-3 `--mcp-config` 인자의 실체 |
| 셸 하니스 | `benchmarks/bench-2026-06-django-strapi/harness/` | §2 고정 스크립트의 실체 — `run-one-episode.sh`·`run-matrix.sh`·`extract-metrics.sh`·`write-scores.sh`·`aggregate-results.sh`·`build-scoring-batches.sh`·`ab-probe.sh`·`config.sh`. 4-arm 토큰 추출 포함. 본 문서 §7의 출처 |
| 사전 벤치 하니스 | `docs/benchmarks/harness/` (저장소 루트) | `bench_axisA.py`(축 A 계측)·`score_axisB.py`(축 B 채점) — §8의 출처 |
| 사전 벤치 원자료 | `docs/benchmarks/artifacts/` | `axisA-results.json`, `axisB-{fd,rg,scrapy}-runs.json`, blind A/B diff·trace 로그 — §8 표의 원천 |
| 사전 벤치 데이터셋 | `docs/benchmarks/codemap-search-{fd,ripgrep,scrapy-context}-dataset.json` | 축 B 앵커·calib_note 포함 |
| scout 보고서 | `docs/benchmarks/scout-mcp-benchmark-report.md`, `scout-mcp-benchmark-reddit-draft.md` | 자매 도구 scout(zoekt+ctags)의 별도 벤치마크 기록(영문 보고서 — 결과·한계·공정성 헌장·후속 계획 포함 — 와 reddit 초안) — codemap-search 측정 자산은 아니나 방법론 선례 |

## 6. 메트릭 산출 정의

(정의는 이 절로 완결된다 — 캠페인 2의 episode runbook에서 표준화돼 본 문서로 흡수됐다. 추출 구현은 `benchmarks/bench-2026-06-django-strapi/harness/extract-metrics.sh`다.)

추출 원천 — 모델 추정이 아니라 CLI의 구조화 이벤트 스트림에 대한 jq 프로그램:

- **codex** `--json`: `item.completed` 이벤트에서 `mcp_tool_call` / `command_execution`을 집계.
- **claude** `stream-json`: `assistant` 메시지의 `tool_use` 블록과 대응 `tool_result`를 집계.

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
- `first_answer_turn`: expected.file(상대경로)이 도구 **결과**에 처음 등장한 호출 순번 (1-indexed, 없으면 null).
- `score`: 해당 task의 `rubric`을 기계적으로 적용.
- `duration_s`: 명령 전후 `date +%s`로 산출한 wall clock (codex jsonl에는 타임스탬프가 없음).

집계 주의사항:

- `first_answer_turn`/`turns` 집계에서 ToolSearch(하니스 메커니즘)는 도구 호출로 세지 않는다.
- `answer_text`는 요약 없이 축어 기록.

토큰 메트릭 (캠페인 4부터 1급 메트릭으로 승격, 4-arm 전부 추출):

- claude: `stream-json` usage 필드. 입력 토큰 = input + cache-read + cache-creation.
- codex: `turn.completed` usage 이벤트 합산. `input_tokens`(cached 포함).
- 두 CLI는 서로 다른 것을 보고한다 — **CLI 내부 비교만, CLI 간 비교 금지**.
- 추출기 변경은 재사용·실행 전에 이전 회차 데이터로 byte-for-byte 회귀 검증 (`del(.tokens)` diff = 0)을 거친다.

오염 검사:

- claude jsonl에서 `Serena MCP Tool Policy` 문자열(작성자 로컬 훅 설정에서만 등장하는 마커)이 보이면 격리 실패 → 에피소드 무효, 하니스 결함으로 보고. 모든 transcript를 기계 스캔한다.
- baseline 캠페인은 추가로 purity를 에피소드별 기계 검증: `mcp__codemap` 문자열 0, MCP tool-call 이벤트 0, web 사용 0, 거부 도구 시도 0.

## 7. 고정 실행 명령

(출처: `benchmarks/bench-2026-06-django-strapi/harness/run-one-episode.sh`·`config.sh`. 하니스 도입 전인 캠페인 1·2 시기의 직접 호출 원형은 §7-8에 기록으로 보존한다.)

**명령 재구성 금지 원칙**: 러너는 아래 스크립트의 호출자이지 명령 작성자가 아니다. 프롬프트는 tasks JSON의 `prompt` 필드를 `jq -r`로 **글자 그대로** 추출한다(한 글자도 추가/수정 금지 — 캠페인 1의 v4 회차가 정확히 이 실수로 무효화됐다). arm별 변환은 결정적 치환만 허용한다(아래 baseline 접두 제거가 유일한 예).

### 7-1. 호출 계약

```bash
harness/run-one-episode.sh "ITER|REPO|ARM|TASK|REP"
# 예: run-one-episode.sh "ds-iter1|django|claude-sonnet|d3|1"
```

- **멱등성**: `<EPISODE_ID>.metrics.json`이 이미 있으면 skip — 중단된 매트릭스는 안전하게 재개된다.
- 전체 매트릭스: `harness/run-matrix.sh <iteration> [concurrency]` — `ARMS="claude-sonnet-base codex-gpt55-base"`로 baseline arm 선택.
- 산출물(jsonl/stderr/metrics.json)은 `<OUT_DIR> = $BENCH_ROOT/results/<iteration>/<repo>/` 아래 보존.

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

- `--setting-sources ""`: 사용자/프로젝트 설정(훅 포함) 미로드. `--safe-mode`는 사용 금지 — 명시적 `--mcp-config`까지 차단함이 프로브로 확인됐다.
- `ToolSearch`는 차단하지 않는다(하니스 메커니즘 — strict-mcp-config + 차단 목록 하에서 우회 수단이 못 됨).

**claude-sonnet-base (baseline — MCP 미설정 + 빈 strict mcp-config로 외부 MCP 차단, 빌트인만 허용)**:

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

**codex-gpt55-base (baseline — `mcp_servers` 설정 자체를 전달하지 않음; 유일한 도구는 셸)**:

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

- exit code 142 → `harness_error: "timeout"`. 그 외 비0 종료 → `exit_<RC>`. 출력 jsonl이 jq로 파싱 불가 → `parse_failure`.
- 하니스 수준 실패(타임아웃 제외)만 **1회** 재시도 — 오답·타임아웃은 재시도 금지 (§4-3 재시도 규칙).

### 7-5. 사전 인덱싱 + 배선 점검 (repo당 1회, 에피소드 시작 전)

```bash
cd <REPO_PATH> && $BINARY index .
```

- 인덱스 포맷 sidecar(`.codemap/index/codemap.format`)가 현재 포맷(2026-06 기준 `v7-owner-tokens-indexed`)인지 확인. 규모 참고: ollama 약 1.3s/5MB, ClickHouse 전체 약 11.9s/27MB.
- **제품 코드를 수정했다면**: 추출/인덱싱 출력이 바뀌는 변경은 반드시 `EXTRACTION_FORMAT_VERSION` 범프 → 재인덱스. 도구 레이어(read/grep/overview 파라미터 처리 등)만 바뀌면 범프·재인덱스 불필요.
- 사전 인덱싱된 미변경 repo는 writer를 획득하지 않아 **동일 repo 병렬이 안전**하다 (동시 8프로세스 실측 0에러 — §4-4).
- (선택) 배선 사전 점검: claude 명령에서 프롬프트를 "사용 가능한 도구 이름만 나열해. 도구를 호출하지 마."로 바꿔 1회 실행 → 출력에 `mcp__codemap-search__search` 등 5종이 보여야 한다.

### 7-6. warmup(파일럿) 게이트 — 본 매트릭스 진입 조건

구성: 1문항 × 1회 × 전체 arm을 본 매트릭스와 같은 동시성으로 **동시에** 돌려 병렬 배선까지 함께 검증한다. fable이 판정한다(§2). 통과 기준 6종:

1. 두 arm 모두 에피소드 정상 종료 + 출력 파싱 가능
2. 각 transcript에 codemap-search MCP 호출 ≥ 1 (MCP 배선 증명)
3. claude transcript에 훅/사용자 설정 오염 없음 (`Serena MCP Tool Policy` 부재)
4. claude arm에서 빌트인 파일/셸 도구 사용 0 (pure 보장)
5. 메트릭 JSON이 §6 표준 스키마대로 추출되고 rubric 채점 가능
6. tantivy lock 에러 / MCP 연결 에러 0

추가 게이트 항목: 토큰 필드 존재 검증(캠페인 4부터), 제품 출하 문자열의 ground truth 기계 대조(§4-1 원칙 6).

이 게이트의 가치는 실측됐다: 캠페인 2에서 파일럿 FAIL이 `--safe-mode` 결함을 본실행 전에 잡았다 — 80 에피소드를 버릴 뻔한 비용을 2 에피소드로 막은 것. django+strapi 캠페인(ds-iter1)도 동일 절차를 따랐다: 파일럿 4 에피소드로 기준 6종 전부 통과, 추출 회귀는 이전 회차 실데이터 대조로 검증, 배선 프로브로 두 repo 모두 MCP 5종 노출·빌트인 0·훅 오염 0을 확인한 뒤에야 본실행했다.

### 7-7. baseline arm 구성 — 설계 결정·`.codemap` 격리·purity 검증

§7-2·§7-3의 baseline arm(`claude-sonnet-base`·`codex-gpt55-base`)은 캠페인 4(ds-base1, §9-7)에서 "빌트인 도구만"으로 같은 과제를 풀게 해 codemap-search의 부가가치를 직접 재는 구성이다. baseline은 제품 수정의 영향을 받지 않으므로 1회 측정 후 이후 수정 루프의 고정 기준선으로 재사용한다. MCP arm(ds-iter1)과의 의도적 설계 차이는 다음 3가지뿐이다:

1. **프롬프트 변환**: tasks JSON의 프롬프트는 전 과제가 `"codemap-search MCP 도구를 사용해서 "` 접두로 시작한다(20/20 기계 확인). baseline에서는 이 접두를 셸 prefix-strip(`${PROMPT#"$MCP_PROMPT_PREFIX"}`)으로 **기계 제거**만 한다 — 과제 본문·인용 요구·수정 금지 문구는 무변경. 재타이핑 금지 원칙(§4-2)은 변환이 결정적·전수 동일하므로 유지된다.
2. **claude baseline에 Bash 포함**: codex baseline의 유일한 도구가 셸이므로 동등 조건. Edit/Write 등 변경 도구는 차단(과제는 읽기 전용).
3. **`.codemap` 격리(quarantine)**: 측정 사본의 인덱스 디렉터리를 `/tmp/benchmark-data/quarantine-codemap/`로 이동 — baseline 에이전트의 grep이 측정 대상 인덱스 내용을 읽으면 오염이다. **캠페인 종료 후 원위치 복원 필수** (`mv quarantine-codemap/django-main.codemap django-main/.codemap`, strapi 동일; ds-base1은 복원 완료).

**purity 검증** (파일럿·본실행 공통, 에피소드별 기계 적용):

- transcript에 `mcp__codemap` 문자열 등장 0건 (claude 프로브에서 MCP 도구 미노출 확인됨)
- codex transcript에 web 도구 사용 이벤트 0건 — codex exec 기본값에서 web search는 비활성이고 MCP arm 캠페인과 동일 조건이지만, 모델이 도구 목록에 `web.run`을 나열하므로 사용 0건을 사후 기계 검증으로 보증한다
- 오염 문자열(`Serena MCP Tool Policy`) 0건, harness_error 0건

**메트릭 주의** (`extract-metrics.sh`의 baseline 분기):

- `tool_calls` 키가 arm별로 다르다: claude-base `{bash,read,glob,grep}`, codex-base `{shell}`
- `mcp_response_bytes_total`은 필드명을 유지하되 "도구 결과 바이트 총합" 의미
- `denied_builtin_attempts`: 허용 목록 밖 도구 시도(mcp__* 포함 — purity 카운터 겸용)
- `shell_bypass_calls`: baseline에서는 셸이 정규 도구이므로 0 고정
- MCP arm과 비교 가능한 지표: score, turns, duration_s, first_answer_turn, duplicate_calls, 도구 결과 바이트(의미 차이를 주석으로 달아 비교)

### 7-8. 캠페인 1·2 시기 호출 원형 (셸 하니스 이전 — 기록)

§7-1~§7-3의 셸 하니스는 캠페인 3에서 도입됐다. 캠페인 1·2는 아래 명령을 러너 에이전트가 직접 실행했다 — 격리 플래그·도구 목록은 현행과 동일하고, 차이는 타임아웃 집행(`perl alarm` 대신 Bash 도구 타임아웃 600000ms — macOS 기본에 `timeout(1)` 부재 — wall clock은 `date +%s`)과 산출물 명명뿐이다. `--safe-mode` → `--setting-sources ""` 교체와 차단 목록의 `Workflow,Agent,Skill` 추가는 캠페인 2 파일럿 1차 FAIL에서 확정됐다(§4-2 함정 표).

**codex arm** (gpt-5.5, reasoning medium, pure MCP):

```bash
codex exec -C <REPO_PATH> --skip-git-repo-check --ignore-user-config --ephemeral \
  -s read-only -m gpt-5.5 -c model_reasoning_effort="medium" \
  -c approval_policy="never" \
  -c 'mcp_servers.codemap-search.command="<BINARY>"' \
  -c 'mcp_servers.codemap-search.args=["mcp"]' \
  --json "<PROMPT>" > <OUT_DIR>/<EPISODE_ID>.codex.jsonl 2> <OUT_DIR>/<EPISODE_ID>.codex.stderr
```

**claude arm** (sonnet, pure MCP):

```bash
cd <REPO_PATH> && claude -p --model sonnet --setting-sources "" \
  --mcp-config /tmp/benchmark-data/mcp-codemap.json --strict-mcp-config \
  --allowedTools "mcp__codemap-search__search,mcp__codemap-search__overview,mcp__codemap-search__read,mcp__codemap-search__find,mcp__codemap-search__grep" \
  --disallowedTools "Bash,Read,Glob,Grep,Edit,Write,WebFetch,WebSearch,Task,NotebookEdit,TodoWrite,Workflow,Agent,Skill" \
  --output-format stream-json --verbose \
  "<PROMPT>" > <OUT_DIR>/<EPISODE_ID>.claude.jsonl 2> <OUT_DIR>/<EPISODE_ID>.claude.stderr
```

당시에도 산출물은 `<OUT_DIR> = /tmp/benchmark-data/results/<iteration>/<repo>/` 아래 보존했고, 프롬프트 축어 추출·1회 재시도·오염 검사 규칙은 §7-1~§7-4와 같다.

**캠페인 1 데이터셋**: 캠페인 1(surrealdb)의 과제 카탈로그와 ground truth는 다른 캠페인과 동일하게 `apps/codemap-search/benchmarks/`의 기계 입력물로만 보존한다 — 셸 하니스 이전이라 JSON이 아닌 `bench-2026-06/tasks-surrealdb.md` 산문 기록이다(매트릭스·프롬프트 프레임 차이도 그 파일에 기록). **특정 코퍼스·스냅샷의 정답 줄 번호는 본 문서에 옮겨 적지 않는다** — surrealdb·ollama·django 등은 언제든 다시 측정 대상이 될 수 있고, 문서가 그 코퍼스의 낡은 정답 키를 들고 있으면 그 자체가 오염원이 된다(§4-1 원칙 6의 정답-누출 불변 조건과 동형). 본 문서는 코퍼스 비종속이어야 하며, 정답은 매 측정마다 그 시점 코퍼스에서 새로 도출한다.

## 8. 사전 벤치마크 — 축 A(스케일링/성능) · 축 B(에이전트 E2E)

§9의 에이전트 캠페인에 앞서 2026-06-09~10에 두 갈래의 사전 벤치마크를 돌렸다. 축 A는 코드베이스 크기(~10k → ~128k 지원 LOC)에 따른 인덱싱·메모리·도구 지연· 응답 크기를 ground truth 없이 특성화했고, 축 B는 에이전트 E2E 과제 2종(edit-site localization, context-gathering)에서 빌트인 도구 대비 손익을 측정했다. 두 결과 — "진짜 비용은 지연이 아니라 컨텍스트"(축 A)와 "이 규모·이 과제에선 baseline이 Pareto 최적"(축 B) — 가 §9 캠페인 설계의 출발점이다(연결은 §8-5).

이 절의 원자료·하니스·데이터셋 인용 경로는 모두 **저장소 루트의 `docs/benchmarks/` 기준**이다(예: `artifacts/axisA-results.json` = `docs/benchmarks/artifacts/axisA-results.json`) — 본 문서가 있는 `apps/codemap-search/docs/`가 아니다.

### 8-1. 코퍼스 고정 SHA와 티어 — 재클론 복원의 단일 출처

코퍼스는 레포 밖 별도 디렉터리에 **고정 SHA**로 핀해 사용했다 — git 트리를 오염시키지 않기 위함이다. 측정 종료 후 클론 디렉터리는 정리됐으며, 아래 SHA로 재클론하면 누구나 동일 코퍼스를 복원한다. 티어 단위는 **codemap-search가 실제 심볼-인덱싱하는 지원언어(Rust/Python/TS·JS) LOC**다(tokei 14.0, `.gitignore` 준수). 비지원 언어/문서/벤더 파일은 티어 정의에서 제외하되, `read`/`find`/`grep` 대상에는 동일하게 포함되므로 공정성에 영향 없다.

| 티어 | 레포 | SHA | 지원 LOC | 지원 파일 | 주 언어 |
|---|---|---|--:|--:|---|
| ~10k | `sharkdp/fd` | `25461e5` | 6,813 | 23 | Rust |
| ~30k | `BurntSushi/ripgrep` | `82313cf` | 39,070 | 102 | Rust |
| ~50k | `scrapy/scrapy` | `4e956bd` | 63,513 | 439 | Python |
| ~100k | `vuejs/core` | `48ad452` | 128,285 | 519 | TypeScript |

> 티어 라벨은 명목 버킷이고 **실측 LOC를 그대로 표기**한다(예: "100k"가 아니라 128,285). 스프레드는 Rust(경량 추출) → Python → TypeScript(중량 추출)로 언어별 거동도 함께 관찰한다.

**대안 코퍼스**(재슬라이스용 — SHA 핀 기록, 필요 시 재클론):

| 레포 | SHA | 지원 LOC | 주 언어 |
|---|---|--:|---|
| `pallets/flask` | `36e4a82` | 13,993 | Python |
| `denoland/rusty_v8` | `7e2d4a2` | 36,423 | Rust |
| `tiangolo/fastapi` | `5cdf820` | 94,530 | Python |
| `vitejs/vite` | `689a066` | 79,183 | TypeScript |
| `prettier/prettier` | `15f1320` | 127,839 | JavaScript |
| (폐기·과대) `python/mypy` `e15a6d5` 214,871 Py · `rust-lang/cargo` `0140b9b` 285,414 Rust | | | |

### 8-2. 축 A — 인덱싱·도구 지연·컨텍스트 비용 (2026-06-09)

4티어(~10k→~128k 지원 LOC) 대상 인덱싱·메모리·인덱스 크기·도구 지연 실측. ground truth 불필요. 측정 하니스: `harness/bench_axisA.py` · 원자료: `artifacts/axisA-results.json`.

> ⚠️ **내장 `benchmark` 서브커맨드는 외부 계측을 대체하지 못한다**(제품 CLI에 현존하므로 미래 측정자 주의). 빌드시간·메모리·인덱스 크기를 계측하지 않고, 그 baseline은 rg가 아니라 쿼리마다 전체 파일을 재파싱하는 naive 선형스캔 strawman이라 "인덱스가 baseline보다 N배" 헤드라인은 금지다. `expected[]` 없는 실행은 recall이 baseline·index 모두 무조건 100%라 baseline_set vs index_set divergence%만 유의미하다. 이 절의 수치는 전부 외부 계측(`/usr/bin/time -l` · `bench_axisA.py`)이다.

측정 환경:

- 바이너리: `codemap-search 0.1.0` (`--release`, 16MB), Apple Silicon (darwin 24.6.0).
- 반복: 인덱싱 N=5(median), 도구지연 warmup 3 + 측정 25회(p50/p90/p95). 인덱싱·RSS는 `/usr/bin/time -l`.
- 도구지연은 **상주 MCP 서버**(인덱스 1회 open 후 warm) — 실사용과 동일. CLI 재호출 오버헤드 미포함.
- startup(프로세스 spawn + `--version`) p50 = **2.5ms** (MCP 서버는 이 비용을 1회만 지불).

#### 인덱싱 (cold, N=5 median)

| 코퍼스 | 언어 | 지원 LOC | 색인 파일 | 심볼 | wall median | peak RSS | 인덱스 크기 | LOC/s |
|---|---|--:|--:|--:|--:|--:|--:|--:|
| fd | Rust | 6,813 | 23 | 559 | **0.19s** | 47.9 MB | 0.16 MB | 35.9k |
| ripgrep | Rust | 39,070 | 100 | 4,094 | **0.22s** | 63.1 MB | 0.72 MB | 177.6k |
| scrapy | Python | 63,513 | 439 | 15,398 | **0.35s** | 62.4 MB | 1.24 MB | 181.5k |
| vue-core | TypeScript | 128,285 | 517 | 19,536 | **0.63s** | 69.2 MB | 1.43 MB | 203.6k |

> 색인 파일수 < tokei 파일수(100/102, 517/519)는 일부 파일이 `max_file_size`/추출불가로 스킵된 것. **심볼 컬럼은 raw/indexed(추출 총량) 기준.** 2026-06-09 overview 고도 보정(인과 서사는 `benchmark-evolution.md` 프롤로그 0.2, 결정 수치는 아래 '고도 보정·crossover 결정 수치' 항목) 이후 root overview의 headline `Total Symbols`는 significant(필터 후) 합을 보고하므로(인덱스는 여전히 전량 보유) 이 표의 수치와 의미가 다르다 — `bench_axisA.py:52`로 재집계 시 컬럼을 "significant"로 라벨링할 것.

스케일링 해석:

- **wall: ~0.18s 고정 floor + LOC 선형.** LOC 19×(6.8k→128k)인데 wall은 3.3×(0.19→0.63s). 작은 코퍼스는 고정비가 지배 → fd throughput(35.9k)이 낮게 보이고, 커질수록 **한계 throughput이 ~180–204k LOC/s로 평탄화**. 사실상 선형이며 빠름.
- **peak RSS: 거의 평탄(47.9→69.2 MB, 1.44×).** LOC 19× 증가에도 메모리는 강한 sub-linear — 스케일링 병목 아님.
- **인덱스 크기: 심볼수에 비례.** 심볼 35×(559→19,536) 대비 크기 9×(0.16→1.43 MB). 매우 콤팩트.

#### Warm 도구지연 (상주 MCP, p50 / p95 ms)

| 도구 | fd | ripgrep | scrapy | vue-core | 스케일 요인 |
|---|--:|--:|--:|--:|---|
| read | 0.08 / 0.10 | 0.08 / 0.09 | (에러*) | 0.09 / 0.11 | 파일 크기(코퍼스 무관) |
| find | 0.44 / 0.49 | 1.23 / 1.37 | 2.00 / 2.37 | 2.33 / 2.41 | 파일 수(트리 워크) |
| overview | 0.55 / 0.68 | 1.78 / 1.96 | 4.52 / 4.94 | 6.32 / 6.87 | 심볼 수 |
| grep | 0.70 / 0.77 | 2.56 / 2.70 | 5.52 / 5.71 | 6.80 / 7.79 | 코퍼스 스캔(rg) |
| search (BM25) | 1.46 / 1.56 | 6.94 / 7.36 | 12.95 / 13.66 | 13.56 / 14.08 | 인덱스 크기 |

`*` scrapy는 `setup.py` 부재(`pyproject.toml`만) → 해당 read는 에러경로(무효). 유효 read는 3개 코퍼스 0.05–0.11ms.

- **read는 코퍼스 크기와 무관**하게 <0.12ms — 파일 크기에만 의존(정상).
- **search가 가장 느리나 ~128k에서도 p95 14ms.** BM25는 인덱스/심볼수에 따라 증가(1.5→14ms)하지만 절대값이 작다.
- find/grep/overview는 코퍼스 스캔·심볼수에 선형. 전 도구 p95 ≤ 14ms로 **인덱스가 살아 있으면 모든 탐색이 한 자릿~십 ms대**.

#### 컨텍스트 비용(응답 크기) — root overview 폭증 ⚠️

응답 텍스트 길이(문자):

| 도구 | fd | ripgrep | scrapy | vue-core |
|---|--:|--:|--:|--:|
| overview(root) | 20.7 KB | 138 KB | 578 KB | **761 KB** |
| search | 654 B | 3.7 KB | 24.3 KB | 17.1 KB |

- **128k 코퍼스의 root overview = 761KB 텍스트** → 에이전트 컨텍스트에 통째로 들어가면 토큰비용이 매우 큼. 지연(6.3ms)은 싸지만 **컨텍스트 비용은 비쌈**.
- 실사용 함의: root overview를 통째로 호출하지 말고 **폴더 단위로 좁히기**가 핵심. search는 threshold 초과 시 codemap 폴백이라 응답이 커질 수 있음 (scrapy 24KB).

#### overview 고도 보정 · folder 라인번호 crossover — 결정 수치 (2026-06-09)

root overview 폭증의 대응으로 같은 날 overview 고도 보정(root 슬림화 → folder 라인범위 제거)이 들어갔다 — 인과 서사(관찰→가설→수정→재측정)는 `benchmark-evolution.md` 프롤로그 0.2가 보유하고, 결정을 잠근 수치는 아래가 정본이다.

- **root 슬림화**: fd root overview 20,688 B → **1,265 B**(−94%, per-symbol 라인범위 0개).
- **folder 라인범위 제거의 바이트 절감**: 티어 무관 folder overview의 **25~31%**(fd 10,180→7,294 B · ripgrep 20,238→13,896 B · scrapy 16,752→12,529 B · vue-core 23,698→17,755 B).
- **강제형 에이전트 A/B(n=3) 토큰 Δ%**(라인범위 제거 R2 − 유지 R1): fd(index 0.16 MB) **−0.2%** · ripgrep(0.73 MB) **−3.1%** · vue-core(1.4 MB) **−7.5%** — 3점 선형근사 Δ% ≈ −5.9×index_MB + 0.74, break-even index ≈ 0.13 MB(~7k LOC).
- **결정**: 라인범위가 토큰상 이득인 구간이 index ≲ 0.13 MB뿐이고 그 위로는 크기에 비례해 악화 → **folder 라인범위 항상 숨김(R2)으로 고정**. index 크기 임계 하이브리드는 검토 후 폐기 — tantivy 인덱스는 첫 search 때 lazy 빌드되므로 overview 시점의 인덱스 크기를 신뢰할 수 없고, overview 동작이 search 이력에 의존하면 결정성이 깨진다.

축 A 요약: **성능은 스케일링 병목이 아니다** — 128k LOC도 색인 0.63s·RSS 69MB·인덱스 1.43MB, 전 도구 p95 ≤14ms. **진짜 비용은 지연이 아니라 컨텍스트(응답 크기)** — overview at scale. 이 관찰이 축 B 채점에 반환 토큰/문자수를 컨텍스트 효율 프록시로 포함시킨 이유이고, §9 캠페인의 `mcp_response_bytes_total` 표준 메트릭(§6)으로 이어졌다.

### 8-3. 축 B — edit-site localization (fd·ripgrep, 2026-06-10)

과제: "이 기능을 구현하려면 어디를 고쳐야 하나"를 `file:line`으로 답(edit-site localization). 3 arm × 3 rep × corpus 2 = **18 런**, contestant **sonnet**, 병렬. 헤드라인 = **비용(토큰)+행동**, F2는 정확도 게이트. 데이터셋: `codemap-search-fd-dataset.json`(fd-feat-1), `codemap-search-ripgrep-dataset.json`(rg-feat-1). 원자료: `artifacts/axisB-{fd,rg}-runs.json`. 채점: `harness/score_axisB.py`.

측정 환경:

- contestant 모델 **sonnet**(Agent override). codemap 서버: corpus별 scoped stdio(`codemap-search-fd`/`-ripgrep`, cwd=corpus), release 바이너리, 인덱스 pre-warm.
- corpus: fd@`25461e5`(23파일/6.8k LOC, parity 티어), ripgrep@`82313cf`(100파일/ 39k LOC, 멀티크레이트, 변별 티어) — §8-1 SHA 표와 동일.
- arm 3종: **baseline**(Read/Glob/Grep/Bash) · **pure**(codemap 5도구만, Bash 없음) · **additive**(빌트인+codemap).
- 토큰·tool_calls는 Agent 결과 `<usage>`(subagent_tokens·tool_uses)에서 직접 캡처.
- **동시성 검증**: corpus당 codemap 에이전트 6개가 단일 stdio 서버를 동시 호출 — 응답 garbling 없이 깨끗이 multiplex(9/9 유효 JSON).

#### 결과 (보정 앵커, n=3 평균)

fd (parity 티어, 23파일):

| arm | recall | precision | F2 | over-return | **tokens** | tool_calls |
|---|--:|--:|--:|--:|--:|--:|
| baseline | 1.00 | 0.75 | 0.94 | 1.0 | **19,412** | 10.7 |
| pure | 1.00 | 0.75 | 0.92 | 1.0 | **20,871** (+7.5%) | 15.3 |
| additive | 1.00 | 0.67 | 0.89 | 1.3 | **25,221** (+30%) | 14.3 |

ripgrep (변별 티어, 100파일/39k LOC):

| arm | recall | precision | F2 | over-return | **tokens** | tool_calls |
|---|--:|--:|--:|--:|--:|--:|
| baseline | 1.00 | 1.00 | 1.00 | 0.0 | **34,303** | 21.3 |
| pure | 1.00 | 0.88 | 0.97 | 1.3 | **53,130** (+55%) | 22.7 |
| additive | 0.83 | 0.77 | 0.82 | 1.7 | **52,405** (+53%) | 25.7 |

원자료(tokens): fd-base 16605/25880/15752 · fd-pure 19122/20852/22640 · fd-add 29679/21206/24777 · rg-base 36154/34755/32000 · rg-pure 53786/56919/48685 · rg-add 52124/65606/39486.

raw 앵커(보정 전) — robustness 감사용:

| corpus | arm | recall | F2 |
|---|---|--:|--:|
| fd | baseline / pure / additive | 0.50 / 0.50 / 0.33 | 0.50 / 0.48 / 0.32 |
| ripgrep | baseline / pure / additive | 0.75 / 0.67 / 0.58 | **0.76 / 0.66 / 0.55** |

→ **보정 전 raw에선 baseline 우위가 오히려 더 컸다**(rg F2 0.76 vs pure 0.66). 보정(diff 편집범위=함수/구조체 경계로 확장, arm 동일 적용)은 codemap에 **유리한** 방향이었고, 그럼에도 순위·결론 불변. "데이터 보고 앵커 조작" 혐의를 차단.

ripgrep "stats 강제 활성화"(가장 비자명한 앵커) arm별 적중:

| arm | 적중 | 메커니즘 |
|---|--:|---|
| baseline | **3/3** | grep/read로 Count→stats 비활성 흐름을 추적, 게이트 도달 |
| pure | **3/3** | codemap search/read로 동일 추적(hiargs:1250/1254) |
| additive | **1/3** | 2개 rep은 from_low_args 배선(hiargs:251/280)을 게이트로 오인 — 두 도구셋 병행이 가장 어려운 지점에서 산만 |

#### 실측이 지지하는 결론 (스코프: keyword-greppable edit-site 과제)

> **스코프 경계(헤드라인과 동급):** 이 과제군의 edit-site는 `Count`/`stats`/`FLAGS`/`quiet` 등 **검색가능 심볼·플래그명**으로 특정된다. 결론은 이 조건에 한정되며, codemap의 가설적 강점(키워드를 모를 때의 **구조적 네비게이션**, BM25 심볼/docstring 검색)은 **본 벤치가 시험하지 않았다.**

1. **codemap은 빌트인에 추가 정확도를 주지 않는다(중복, redundant).** 보정 후 recall: fd 전 arm 1.00, rg baseline/pure 1.00·additive 0.83. 비자명 핵심 앵커도 baseline 3/3·pure 3/3 동률 적중. **pure는 codemap 단독으로 baseline 정확도를 매칭** — 도구가 부정확한 게 아니라, 이미 강한 grep 위에서 **변별을 못 만든다.**
2. **그 매칭을 토큰 ~1.5×로 산다.** ripgrep pure +55%, additive +53%(34.3k → 53.1k/52.4k). 분산 비중첩(baseline 32–36k vs codemap 48–66k) → n=3에서도 견고. codemap의 구조화 응답(overview·search 결과)이 컨텍스트를 더 먹는다. **즉 "더 비싼 동률".**
3. **additive(빌트인+codemap 동시)는 Pareto 열위 — "열등"이 아니라 "산만".** baseline이 가진 걸 전부 + codemap을 더 줬는데 recall↓(0.83)·over-return↑·tool_calls↑(25.7)·토큰↑. 위 stats-gate 1/3이 메커니즘: 두 도구셋 병행이 **가장 어려운 지점에서 오히려 탐색을 흩뜨린다.**
4. **이 과제군에서 baseline이 Pareto 최적.** 정확도 ≥ codemap이면서 토큰 최소. 결론은 "codemap이 나쁘다"가 아니라 **"greppable edit-site 과제에선 순정 빌트인으로 충분하고, codemap을 얹으면 비용·노이즈만 추가(중복)"**.

#### 한계·해석 경계 (과대해석 금지)

- **과제가 키워드-grep 친화적(스코프 핵심).** 위 헤드라인 스코프 경계 참조. "codemap이 항상 열위/중복"이 아니라 "**greppable edit-site 과제에선** 빌트인으로 충분"이 정확한 진술.
- **비용 프리미엄에 고정 스키마 오버헤드 포함(미분해).** codemap arm 에이전트는 매 컨텍스트에 도구 스키마 10개 + MCP instructions 2벌(fd+ripgrep)을 싣는다(빌트인은 상시 존재라 "공짜"). +55% 중 *per-use 응답 verbosity* vs *고정 도구정의 비용*의 분해는 미측정 — 후자 비중이 크면 더 큰 과제에서 amortize되어 격차가 줄 수 있다. 또 `subagent_tokens`가 output-only인지 input 누적인지 미확정. 단, **어느 쪽이든 codemap이 더 비쌌다는 방향은 불변.** (2라운드의 transcript 실측이 이 항목을 일부 해소했다 — §8-4.)
- **parity 경고(fd):** fd는 23파일이라 도구 무관 동률 예상(데이터셋 명시) — fd 수치는 변별 신호 아님. ripgrep이 본 비교의 무게중심.
- **n=3 분산.** 정확도·precision은 rep 변동 있음(특히 over-return). **비용 격차만 분산을 명확히 초과**(비중첩) → 추가 런 불필요.
- **행동 분해 부재.** Agent 결과는 tool_uses **총수**만 줌(도구 타입별 분해 불가). "pure가 search/grep을 어떻게 섞나"의 정량은 미측정 — tool_calls 총수와 서사적 관찰만.
- **앵커 보정(arm 중립):** 원본 region 범위가 타이트해 정당한 위치를 1~3줄 차이로 놓침. diff 헌크 편집범위=함수/구조체 경계로 확장(전 arm 동일 적용, 데이터셋 `calib_note` 기록). 위 raw 표가 보정 robustness를 감사 가능하게 한다.

### 8-4. 축 B 2라운드 — context-gathering (scrapy)

회사 벤치("X가 어떻게 동작하는지 조사") 재현 시도. edit-site와 **다른 task class**: 정답 = 작업 착수에 필요한 구조적 컨텍스트(호출 체인) 집합. 헤드라인 = **context recall @ 토큰**. corpus: scrapy@`4e956bd`(174 py files/63k LOC). task `scrapy-ctx-1`: "다운로더 미들웨어가 설정 문자열→로드→요청·응답 처리에 끼어들기까지 어떻게 동작하나" — 핵심 위치 수집. 데이터셋: `codemap-search-scrapy-context-dataset.json`(8 essential 컴포넌트 = 실제 호출 체인, 코드로 검증). 원자료: `artifacts/axisB-scrapy-runs.json`. **설계 강화(advisor 반-grep-bias):** 정답을 "tracer가 연 파일"이 아니라 **코드가 실제 거치는 호출 체인**으로 한정(코드가 arbiter). grep/read + codemap 두 뷰 교차 검증. greppability 실측(`DOWNLOADER_MIDDLEWARES`=3파일, 동적 로딩 체인은 그 키워드 미포함).

| arm | recall | precision | F2 | over | **tokens** | tool_calls | uniq_files |
|---|--:|--:|--:|--:|--:|--:|--:|
| baseline | **1.00** | 0.74 | 0.93 | 7.3 | **23,429** | 20.3 | 7.3 |
| pure | 1.00 | 0.68 | 0.91 | 8.7 | 36,511 (+56%) | 21.0 | 8.0 |
| additive | 0.92 | 0.71 | 0.86 | 7.0 | 36,277 (+55%) | 19.3 | 7.0 |

원자료(tokens): base 26664/21951/21671 · pure 38035/35463/36036 · add 34466/44475/29889.

#### 가설 반증 — 이 규모(174파일)에선 baseline이 또 Pareto 최적

1. **가설("indirection은 grep가 못 따라가 codemap이 이긴다")이 틀렸다.** baseline이 동적 로딩 체인(`build_component_list`/ `MiddlewareManager.from_crawler`/`load_object`/`build_from_crawler`)을 **recall 1.00으로 완전 포착**. 메커니즘: 에이전트는 순수 grep이 아니라 **grep→read→import·호출 추적**이라, `core/downloader/middleware.py`를 읽으면 나오는 import를 따라 `middleware.py`·`misc.py`로 자연히 이동. 174파일 규모에선 "읽어서 조립"이 충분히 싸다.
2. **codemap은 또 +56% 토큰에 동률.** pure가 overview·search를 **읽기에 더해** 수행 → 같은 recall에 토큰만 추가(같은 ~7–8파일·~27위치 회수). edit-site 결과와 동일 패턴.
3. **additive 또 Pareto 열위**(recall 0.92, 토큰↑) — 단발 sub-agent의 두 도구셋 juggling 재현.

#### 종합과 미검증 변수

edit-site(fd·rg) + context-gathering(scrapy) 3개 task class 모두에서 baseline grep+read가 recall ≥ codemap이면서 토큰 ~55% 저렴 — **회사 경험("압도적")이 재현되지 않았다.** 남은 미검증 변수:

- **규모(가장 유력):** 전 corpus ≤174파일/≤128k LOC. 이 규모에선 "읽어서 조립"이 싸서 codemap 구조가 중복. 회사 코드는 10–50× 클 가능성 → grep hit·read 비용이 폭증하는 **대형(200k–500k+ LOC, 수천 파일)에서만 교차점**이 날 수 있음. mypy(214k)/cargo(285k) 또는 실제 회사 레포로 검증.
- **측정 아티팩트 — transcript 실측으로 반증됨(중요):** subagent transcript(`subagents/agent-*.jsonl`) 분석 결과, (a) 보고된 "토큰"은 output이 아니라 **context-inclusive 측정**이다 — output_tokens는 arm 무관 거의 동일(baseline 2,608 vs pure 2,425). 차이는 전부 context. (b) "멀티-corpus 스키마 15개 고정비가 비용을 부풀린다"는 가설은 **반증**: codemap 도구는 ToolSearch로 지연 로딩돼 turn1 고정비가 pure 4,614 < baseline 7,528. (c) 비용은 **진짜**: cost-weighted(cache_read×0.1+creation×1.25) baseline 91k vs pure 172k(**1.9×**), context_processed 372k vs 796k(2.1×). 원인 = codemap overview/search **결과 verbosity**가 턴마다 context를 부풀림 + 턴 수↑(29 vs 23). **raw 토큰 지표는 비용을 과소평가했지 과대평가가 아님.**
- **핵심 한계 — 단발 sub-agent ≠ 인터랙티브 main-context:** 본 벤치는 sub-agent의 context 비용을 잰다. 이 워크플로에서 에이전트는 codemap을 **읽기에 더해** 써서 context를 부풀린다. 회사의 "압도적"은 거의 확실히 **인터랙티브 main-context** 경험 — 대형·messy 코드에서 파일 10개를 main에 덤프하는 대신 overview 1회로 **대체**하는 절약 — 이고, 이는 본 단발 sub-agent·file:line recall 지표가 측정하지 않는다. 충실한 재현은 OSS corpus가 아니라 **회사 레포 + 인터랙티브/스텝 지표**.

당시 기록된 다음 수: 대형 corpus(mypy 214k / cargo 285k / 회사 레포)에서 동일 context-gathering 과제, 멀티턴 세션 측정(단발 location-recall이 아닌 후속 질의 누적 비용), 단일-서버 agent 타입으로 고정 스키마 비용 제거 후 재측정.

### 8-5. 사전 벤치가 캠페인 설계에 남긴 것

이 결과들은 "codemap 무용"이 아니라 "**이 규모(≤174파일/≤128k LOC)·이 과제(greppable·단발 sub-agent)에선 중복**"이다 — 그리고 §9의 에이전트 캠페인 설계가 정확히 그 경계 위에 세워졌다.

- **측정 방식의 교체.** 축 B의 단발 sub-agent·`<usage>` 캡처는 context-inclusive 토큰과 도구 타입별 분해 불가라는 한계를 남겼다. §9 캠페인은 에피소드를 신선한 contestant CLI 세션으로 고정하고, 메트릭을 CLI의 구조화 이벤트 스트림에서 jq로 기계 추출하는 방식(§6)으로 갈아탔다.
- **컨텍스트 비용이 핵심 지표가 된 경위.** 축 A의 "지연은 싸고 응답 크기가 비싸다"(root overview 761KB)와 축 B의 "codemap 응답 verbosity가 토큰을 부풀린다"는 관찰이 `mcp_response_bytes_total` 표준 메트릭과 contestant 토큰의 1급 메트릭 승격(§6, 캠페인 4부터)으로 이어졌고, 같은 문제의식이 §9-8 응답 다이어트 캠페인까지 닿는다.
- **baseline 비교의 예고편.** 축 B의 "정확도 동률, 비용만 추가" 패턴은 §9-7 캠페인 4(baseline arm)에서 다른 측정 방식·다른 저장소·다른 과제로 재확인됐다 — 정답률 부가가치 0, 효율 신호는 CLI별로 갈림. 단, 축 B와 §9의 수치는 arm 구성(additive 유무)·채점(F2 vs rubric 3-class)·토큰 정의가 달라 **직접 비교 금지**다.
- **남은 미검증 변수도 그대로 승계됐다.** 축 B가 지목한 "대형 코퍼스 교차점"·"인터랙티브 main-context"는 §9-13의 변별 가능 과제 세트 백로그와 함께 여전히 열린 질문이다.

## 9. 캠페인 결과 (2026-06 통합)

> 최종 갱신: 2026-06-12 · Cargo 패키지 버전 0.1.0 (pre-release; 측정 회차는 릴리스가 아니라 인덱스 포맷 버전 `v3` → `v7-owner-tokens-indexed`로 식별)

**요약.** 코딩 에이전트(Claude Sonnet과 Codex GPT-5.5, 모두 CLI 경유)가 대형 오픈소스 저장소 — SurrealDB, ollama, ClickHouse, 그리고 hold-out으로 Django + Strapi — 의 실제 코드 내비게이션 과제를 codemap-search MCP 서버만으로 풀었다 (CLI 빌트인 파일/셸 도구 차단). transcript에 대한 측정→진단→수정→재측정 루프는 에이전트의 도구 사용 방식을 바꿨다: 약한 `search` 주변의 grep 루프 공회전에서 첫 호출 정답으로. 구체적으로 캠페인 2의 두 iteration 사이에 Claude arm의 중앙값 턴이 43–53% 감소하고 중앙값 응답 바이트가 51–58% 감소했으며(Codex arm은 이미 효율적이라 변화 없음), 정답률은 77/79 → 80/80이 됐다. 캠페인 3은 그 수정들을 미접촉 언어 2종(Python, TypeScript)의 *미접촉·더 어려운* 과제 20개로 재시험했다: 단일 iteration에 80/80 correct, 사실상 모든 에피소드에서 첫 `search` 응답이 정답 파일을 실었다 — 캠페인 2의 효율 프로파일이 전이됐고(Claude 중앙값 3–4턴), 더 어렵게 만든 과제 세트도 어차피 포화했다(한계 절 참조). 캠페인 4는 오래 약속한 **baseline arm** — 같은 CLI를 빌트인 도구로 제한, 같은 20과제, 같은 하니스 — 을 마침내 실행했고 역시 80/80: 이 과제 등급에서 정답률은 MCP 부가가치를 보여주지 못하며, 효율 신호는 CLI별로 갈린다(Claude+MCP: 턴은 줄지만 응답은 무거움; Codex+MCP: 턴 동등, hard 과제의 셸 출력 폭주가 사라짐). 이 절은 무효 회차·폐기 메트릭·이 수치가 주장할 수 있는 한계까지 포함해 지금까지의 모든 결과를 통합한다.

### 9-1. 먼저 읽을 것 — 이 수치가 말하는 것과 말하지 않는 것

- **자가 벤치마크.** 도구 작성자가 설계·실행·채점했다. 채점은 과제별 고정 rubric(`correct` / `partial` / `wrong`)을 적용한 LLM 에이전트가 수행했고, 최난도 에피소드들은 더 강한 모델의 별도 적대적 패스에서 저장소 원본과 대조해 재채점 — 역시 같은 작성자가 실행 — 불일치 0(캠페인 2에서 15 에피소드; 캠페인 3에서 40, 표본 답변의 모든 인용을 저장소와 줄 단위 재확인). 제3자 재현은 없다.
- **baseline 비교는 이제 존재한다 — 그리고 도구에 유리하지 않다.** 캠페인 4는 같은 두 CLI를 빌트인 도구만으로(claude: Bash/Read/Glob/Grep; codex: shell) 같은 20과제에 실행했다: 역시 80/80 correct, 함정 포함. 이 과제 등급에서 도구의 부가가치는 정확도가 *아니라* 구조적이고 CLI 의존적이다(캠페인 4 절 참조). codemap-search가 "빌트인 grep/read를 이긴다"는 주장은 여전히 지지되지 않는다.
- **최근 과제 세트 3개 전부 포화.** 캠페인 2 마지막 iteration에서 모든 arm이 모든 과제를 풀었고, 의도적으로 더 어렵게 재구축한(다단계 흐름, alias 간접, 동적 dispatch, 3,500줄 파일) 캠페인 3에서도, 그리고 측정 대상 도구 없이 돈 캠페인 4 baseline arm에서도 다시 그랬다. 이 과제들에서 정답률은 더 이상 arm 간·도구 스택 간 변별을 못 한다; 효율 메트릭(턴·바이트·토큰·도구 믹스)만 한다. 그래서 이 세트들은 유용한 회귀 게이트이자 쓸모없는 자랑 메트릭이다. 변별 가능할 만큼 어려운 과제 세트 구축이 이제 최상위 백로그다.

### 9-2. 측정 대상

codemap-search는 코드 내비게이션용 자립형 Rust MCP 서버(stdio)다: tree-sitter 심볼 추출이 심볼·docstring·문자열 리터럴에 대한 tantivy BM25 인덱스(식별자 분할 토크나이제이션)를 공급한다. 검색 결과는 줄 번호 스니펫 + depth-1 caller/callee 어노테이션을 렌더한다. 도구 5종: `search`, `grep`, `read`, `overview`, `find`. 약 10개 언어 패밀리(Rust, Python, TypeScript/TSX, JavaScript, Go, Java, Kotlin, C, C++, GAS assembly)에 걸친 21개 소스 파일 확장자를 인덱싱하며 1 MiB 초과 파일은 건너뛴다.

"pure MCP"에 대한 한 가지 명확화: 서버 자체가 범용 `grep`·`read` 도구를 출하하므로 CLI 빌트인 차단이 에이전트를 fallback 없이 고립시키는 것은 아니다. 이 벤치마크가 시험하는 것은 *서버 전체*의 충분성이며 — 흥미로운 신호는 도구 믹스와 턴 구조의 이동(grep 복구 루프 감소, 첫 호출 `search` 정답 증가)이지 단순 과제 완수가 아니다.

모든 벤치마크 에피소드는 에이전트 1 · 과제 1 · 신선한 CLI 세션 1이다:

```
"Using the codemap-search MCP tools, <task>. Cite exact file paths and
line numbers in your answer. Do not modify any files."
```

과제는 읽기 전용 코드 내비게이션 질문 — 정의 찾기, call site 열거, 파일 간 흐름 추적, 설정 기본값·에러 출처 찾기 — 이며 각각 사전 검증된 ground truth(`file`, 줄 범위, 증거 인용)를 갖는다. ground truth는 플레인 `rg`/수동 읽기로만 수립하고, 측정 대상 도구로는 절대 수립하지 않는다.

### 9-3. 캠페인 한눈에 보기

| # | Campaign | Repos | Arms | Scale | Headline |
|---|---|---|---|---|---|
| 1 | SurrealDB, rounds v3→v7 | surrealdb (Rust, ~2,700 files) | codex gpt-5.5 | 6 tasks × 2 reps × 5 rounds (1 round invalidated) | answer file in the *first* `search` response: 9/12 → 12/12 |
| 2 | C/C++, iter1→iter2 | ollama (Go + C/C++), ClickHouse (large modern C++) | claude sonnet + codex gpt-5.5 | 2 arms × 10 tasks × 2 reps × 2 repos = 80 episodes/iteration | 77/79 → 80/80 correct; claude median turns −43% / −53% (by repo) |
| 3 | Django + Strapi (hold-out) | django (Python), strapi (TypeScript) | same 2 arms | 80 episodes, harder task mix, single iteration | 80/80 correct; campaign-2 fixes held out-of-sample (claude median 3–4 turns, first-call answers); saturated again |
| 4 | Django + Strapi baseline (ds-base1) | same snapshots | claude + codex, **built-in tools only** (no MCP) | 80 episodes, same tasks/harness as campaign 3 | also 80/80 correct — accuracy shows no MCP added value; efficiency signal splits by CLI |
| 5 | Response-diet loops (ds-iter2 → ds-iter3) | same snapshots | same 2 MCP arms, 6 render fixes then 3 compensation fixes | 2 × 80 episodes, same tasks/harness | iter2: Claude *compensated* the per-response savings away (+16% via whole-file reads). iter3 (signature abbreviation + alias normalization + anchor cap): compensation absorbed — Claude −18%, Codex −14% vs pre-diet, 80/80 held both times |

캠페인 2·3의 버전: claude CLI 2.1.175, `claude-sonnet-4-6` 구동; codex CLI 0.139.0, gpt-5.5 reasoning effort "medium". 캠페인 1은 같은 codex 구성이지만 당시 CLI 버전이 기록되지 않았다. 대상 저장소는 2026-06 스냅샷이다(커밋 핀은 한계 절 참조).

### 9-4. 캠페인 1 — SurrealDB (v3 → v7): `search`가 답을 싣게 만들기

단일 arm(codex gpt-5.5, reasoning medium), 고정 6과제 × 회차당 2 rep, 순차 실행. 측정→수정 5회차. 불량 회차까지 포함한 전체 회차 궤적:

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

¹ 분모 24 = 회차 전체의 detail 렌더 응답 수(에피소드당 2); v3 칸은 기록된 통합 활성화 횟수, v7 칸은 기능별 비율이다.

거의 평탄한 종단 중앙값(9 → 9.5)은 실제 이야기를 가리며, 그 이야기는 궤적 표가 말해준다: 응답을 풍부하게 만들자 중앙값이 먼저 *나빠졌고*(v5–v6: 더 많이 주면 에이전트는 더 많이 검증한다), v7의 인용 수정 — 스니펫 줄 번호 — 이 풍부함의 이득을 전부 유지한 채 도로 끌어내렸다. v3의 턴은 *재검색 공회전*(`search`가 파일명만 반환, 에이전트가 grep/read로 후퇴)이었고 v7의 턴은 *검증 심화*(첫 `search`가 답을 싣고, 남은 턴은 답을 더 완전하게 만들었다 — 예: 한 과제는 예시 call site 1곳 인용에서 KV 백엔드 5종 전부 열거로). 턴 수만 봤다면 캠페인 전체를 무승부로 불렀을 것이다; "몇 번째 턴이 처음 답을 노출했나"가 실제 변화를 보여줬다.

남은 알려진 약점: 다단계 흐름 추적(4-hop 호출 체인)은 여전히 25–33턴이 들었다 — depth-1 caller/callee 맥락으로는 접히지 않는다. 이것이 후속 백로그(더 깊은 call-chain 지원)와 캠페인 3의 어려운 과제 설계를 형성했다.

### 9-5. 캠페인 2 — ollama + ClickHouse (iter1 → iter2): 두 모델, 160 에피소드

두 arm — claude sonnet과 codex gpt-5.5 — 이 각각 repo마다 10과제 × 2 rep을 pure-MCP로 수행. 전체 매트릭스 1회(80 에피소드) → 제품 수정 3건 → 같은 매트릭스 재실행.

**어떤 측정보다 먼저**, ClickHouse 인덱싱 스모크 테스트가 파서 블로커를 적발했다: tree-sitter-cpp가 참조 반환 선언자(`int& f()`, `T& operator=`)를 named field가 아닌 positional children으로 노출해, 해당 심볼 전부가 조용히 미인덱싱됐다 — ClickHouse `src/`에서만 정의 줄 약 3,053개. 같은 게이트에서 파서 버그 2건이 더 나왔다(함수 내부 "vexing parse" 가짜 심볼 — 전체 추출 심볼의 8.5% — 과 in-class private 메서드의 exported 오표시). 이 게이트 없이 측정했다면 파서 구멍을 검색 레이어 탓으로 돌렸을 것이다.

#### 정확도

| Arm × repo (n=20 each) | iter1 | iter2 |
|---|---|---|
| claude · ClickHouse | 18 correct, 2 partial | 20/20 |
| codex · ClickHouse | 19 correct, 1 n/a* | 20/20 |
| claude · ollama | 20/20 | 20/20 |
| codex · ollama | 20/20 | 20/20 |
| **Total** | **77/79** | **80/80** |

\* 에피소드 1건이 provider 측 장애를 만났다 — CLI 이벤트 스트림이 빈 채로 유지되다 1,558 s에 실행이 종료됐다. 정확도 분모와 latency 중앙값에서 제외한다.

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

중앙값은 짝수 크기 표본에 대한 값으로 반올림 표기했고, codex·ClickHouse iter1 중앙값은 n/a 에피소드를 제외한다. 수정들은 Claude arm의 실패 모드를 겨냥했고, Codex 행이 평탄한 이유가 그것이다 — Codex는 애초에 그 실패 모드를 보이지 않았다 (명시적 파라미터를 설정하고 스키마를 더 엄격히 따른다; 교훈 2·4 참조).

대표적인 에피소드 반전 2건 — c7("ClickHouse가 `INFINITE_LOOP`를 던지는 모든 곳 찾기")과 c8("`StorageFactory::get` 찾기") — 의 해부(인과 서사)는 `benchmark-evolution.md` 캠페인 2 절(2.2)이 단독 보유한다.

적대적 검증 패스는 분석 자체도 재검증했고, 분석 모델의 root-cause 서사 2건이 뒤집혔다(점수는 유지됐다; 설명이 틀렸다). 이 문서의 수치는 iteration 분석 보고서에서 복사한 것이 아니라 에피소드별 원시 메트릭에서 재산출했다.

### 9-6. 캠페인 3 — Django + Strapi (ds-iter1): hold-out 시험

캠페인 2의 효율 이득은 in-sample이었다 — 모든 수정이 같은 과제에서 진단되고 재측정됐다. 캠페인 3은 일반화 여부를 시험했다: 같은 두 arm, 같은 하니스 조건, 그러나 미접촉 저장소 2곳·미접촉 언어 2종(Python, TypeScript)의 새 과제 20개, 의도적으로 더 어렵게 구성(repo당 2 easy : 4 medium : 4 hard, 4-hop 흐름 추적, alias 간접 정의, 동적 provider dispatch, 3,561줄 마이그레이션 파일, 과제별 함정 답 포함). 캠페인 사이에 제품은 수정하지 않았다 — 새 수정이 아니라 전이를 측정한다.

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

- **첫 호출 발견.** 양 arm 모두 mean first-answer turn ≈ 1 — owner-token·리터럴 인덱싱·기본 파라미터 수정이 Python/TypeScript 코드로 무수정 전이됐다.
- **효율 프로파일.** Claude의 중앙값 3–4턴·12.8–16.9 KB 응답은 완전히 새로운 재료 위에서 캠페인 2 iter2 수준(4–4.5턴, 16.3–18.2 KB)과 일치한다.
- **강제 변환(coercion) 레이어.** 캠페인 2 iter1은 `read` 파라미터 에러 52건을 기록했다; 캠페인 3은 0건.
- **다단계 흐름이 더는 절벽이 아니다.** 캠페인 1에서 25–33턴이 들던 과제 등급(4-hop 체인)이 여기서는 hard 과제 평균 6.1–9.0턴에 안착했다.

변별하지 못한 것: 정답률. 양 arm 100%, 함정 포함 — 심어 둔 async-variant / raw-queryset / hash-vs-compare / 타입 선언 미끼에 넘어간 에피소드가 없다. 남은 신호는 스타일이다: codex는 같은 답에 도달하면서 `read`를 2.4배(122 vs 50) 쓰고 hard 과제당 약 3턴을 더 썼다 — claude가 `search` 한 번 + 표적 read에 기대는 곳에서 grep+read 검증을 선호했다.

채점은 캠페인 2와 같은 규율의 확장판이다: 채점 배치 8개(sonnet, 기계적 rubric 적용), 이어 **표본 40 에피소드**에 대한 적대적 재채점 게이트(hard 8과제 전부 + medium 2과제, 인용된 모든 file:line을 저장소 원본과 재대조 — 줄 검사 약 300회), 그중 12개는 오케스트레이션 모델이 직접 독립 재검증. overturn 0. 인용 내용 플래그 5건이 표면화됐고 cosmetic으로 판정됐다(답변 측 markdown 줄 라벨 옮김, ±1; 도구가 반환한 줄 번호 자체는 정확).

운영상 이것은 재작성된 하니스의 첫 캠페인이었다: 80-에피소드 풀 매트릭스가 **wall-clock 약 5분**(동시성 8, 에피소드 중앙값 25 s, 동일 repo 병렬 실행, lock 에러 0)에 돌았다 — 캠페인 2의 직렬·에피소드별 에이전트 운영의 iteration당 2.4–4.0 h 대비. 채점 + 재채점은 서브에이전트 토큰 약 0.45M을 소모했다 — 캠페인 2 iteration당 약 3M 대비.

절차 부채, 공개: 캠페인 2 회고는 "캠페인 3부터" 커밋 핀을 약속했다; 측정 스냅샷이 다시 `.git` 없이 출하되어 SHA가 기록되지 않았고 ground truth는 이 스냅샷에만 결속된다(한계 7 유지). 조기 생성된 2차 iteration 디렉터리 1건(루프 규칙 위반)도 본실행 전에 적발·삭제됐다 — 경위와 규범은 §4-5 회차 이름·잔재 규율이 정본이다.

### 9-7. 캠페인 4 — baseline arm (ds-base1): 도구가 실제로 더하는 것은?

한계 항목 1이 요구한 그대로: 같은 스냅샷, 같은 20과제, 캠페인 3과 같은 하니스 조건·동시성 — 단 에이전트는 빌트인 도구만 받는다. `claude-sonnet-base`는 Bash/Read/Glob/Grep 허용에 MCP 차단(`--strict-mcp-config`, MCP 설정 없음); `codex-gpt55-base`는 `mcp_servers`를 아예 받지 않는다 — 유일한 도구는 셸. 캠페인 3과의 의도적 설계 차이 3건은 전부 §7-7에 기록돼 있다: 과제 프롬프트의 MCP 유도 접두를 기계 제거(결정적·일률적 `${PROMPT#prefix}` — 재타이핑 금지 원칙 유지), codex 셸과의 동등 조건으로 claude에 Bash 부여, 측정 사본의 `.codemap` 인덱스 디렉터리 격리(baseline grep이 측정 대상 인덱스를 읽지 못하도록; 캠페인 후 복원). purity는 에피소드별 기계 검증: `mcp__codemap` 문자열 0, MCP tool-call 이벤트 0, web 사용 0, 거부 도구 시도 0.

이 캠페인은 또한 **contestant 토큰 사용량을 1급 메트릭으로 승격**했다(claude `stream-json` usage 필드; codex `turn.completed` usage 이벤트 합산) — 4개 arm 전부에서 추출. 추출기 변경은 어떤 실행 전에도 이전 iteration 데이터와 byte-for-byte 회귀 검증(`del(.tokens)` diff = 0)을 거쳤고, 캠페인 3 에피소드는 아래 토큰 컬럼을 위해 보존 JSONL에서 재추출했다.

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

¹ claude: input + cache-read + cache-creation; codex: `input_tokens`(cached 포함). 두 CLI는 서로 다른 것을 보고한다 — CLI 내부 비교만, CLI 간 비교 금지.

#### 정직하게 읽기

- **정답률 부가가치: 이 세트에서는 0.** 두 baseline 모두 심어 둔 함정 포함 전부 풀었다. pure-MCP 캠페인 2회 + baseline 캠페인 1회의 일치된 결론: 이 등급의 작성자 설계 내비게이션 과제는 도구가 정확도에 도움이 되는지 보여줄 수 없다.
- **Claude + MCP: 더 싼 검색 구조, 더 무거운 응답.** 턴 −11%(4.6→4.1), hard 과제 턴 −16%(7.4→6.2), first-answer 1.56→1.15. 그러나 도구 결과 바이트 +73%(13.7→23.8 KB), 에피소드 duration +28%, 출력 토큰 +21%; 입력 토큰만 개선(−9%). 한 방 `search`가 grep→read 캐스케이드를 대체하고, 그 값을 응답 무게로 치른다.
- **Codex + MCP: 동일한 노력, 꼬리 억제.** 턴·호출 수는 완전히 동등하다(양쪽 6.1 / 약 244 호출). 평균 바이트 −45%(51.6→28.6 KB)는 전적으로 꼬리 효과 — baseline 중앙값이 오히려 *낮지만*, hard 과제가 셸을 통해 폭주한다(s7 104 KB, s8 85 KB, s10 69 KB 평균 도구 출력); MCP 응답은 유계로 유지된다. 출력 토큰 −20%, 입력 토큰 +15%(역방향).
- **토큰 최적화 목표에서 일관된 승리 없음.** 입력/출력 델타가 CLI별로 반대 방향을 가리킨다. "codemap-search가 에이전트 토큰 사용량을 줄인다"는 이 데이터로 지지되지 않는다; 응답 크기 다이어트(스니펫·caller 컨텍스트 슬리밍)가 그 결과로 나온 제품 백로그 항목이다.
- **메트릭 경고 하나 표면화**: baseline 에피소드 6개(전부 correct)에서 expected 파일 문자열이 어느 도구 결과에도 나타나지 않았다(`first_answer_turn` null) — 셸 파이프라인과 `Glob` 결과는 경로를 반드시 에코하지 않는다. baseline first-answer 수치는 보수적으로 취급할 것.

채점·검증은 캠페인 3과 동일했다: sonnet 8-배치 기계 rubric 채점, 같은 40-에피소드 표본 구성의 적대적 재채점 게이트 — rubric 요건 실패 0, 3개 에피소드의 인용 불일치 12건 전부 cosmetic 판정(±1 보조 인용, 스니펫 라벨 드리프트), **overturn 0**. 매트릭스 전에 warmup 게이트(4 에피소드, 기준 6종 전부 + 토큰 필드 존재) 통과; 80-에피소드 실행은 wall-clock 314 s, 하니스 실패 0. baseline arm은 제품 변경의 영향을 받지 않으므로 ds-base1은 이후 수정 루프의 고정 기준선이다.

### 9-8. 캠페인 5 — 응답 다이어트 (ds-iter2 → ds-iter3): 에이전트는 보상한다

baseline 캠페인의 가장 분명한 제품 신호는 응답 무게였고, 렌더 레이어 변경 6건이 들어갔다(인덱스 포맷 무변경; 스캔·랭킹·coercion 무변경): exact-identifier(tier-1) 우선의 쿼리 매칭 심볼 앵커링과 2–3줄 컨테이너 요약, symbol-overflow 파일에서 쿼리 매칭 심볼의 스니펫 보장, 중복 caller 블록 dedup("same as above"), 정의 ≥5개 이름의 caller에 대한 정직한 한 줄 생략(신규 `caller_omit_def_threshold`), ranked-tail 트림 50→12, `.yarn`/minified-bundle grep 제외, 그리고 재읽기 확인을 막는 도구 설명 예시.

매트릭스 **전에** 보존된 변경 전 바이너리와의 라이브 A/B 프로빙이 결함 2건을 잡았다: 토큰 중첩 매칭이 너무 느슨했고(`select` 서브토큰이 `get_select` 등에 풀 스니펫을 부여 — 전체 식별자 tier-1 우선으로 교체), caller dedup이 렌더 순서가 아닌 스캔 순서를 키로 잡아 원본이 렌더되지 않은 "same as above" 참조를 만들었다(렌더 타임·렌더 순서·emit-committed dedup으로 이동). 세 번째 운영 교훈: 인덱서가 첫 스냅샷을 발행하기 전의 호출은 빈 caller 인덱스를 받는다(라벨 없음, "top-level/unindexed" 귀속) — 프로브는 warm-up을 기다려야 한다; 매트릭스 80 에피소드 전부 warming 응답이 없음을 기계 검증했다.

동일 쿼리 A/B 절감은 실재했다: −34%(클래스 preamble 케이스), −53%(반복 caller 케이스), −27%, −22%, fallback 케이스는 설계상 +8%(이전에 없던 답 스니펫). 매트릭스는 다른 이야기를 했다:

| Metric (n=40/arm) | claude iter1 | claude iter2 | codex iter1 | codex iter2 |
|---|---|---|---|---|
| Correct (re-score gate) | 40/40 | **40/40 (0 overturns)** | 40/40 | **40/40 (0 overturns)** |
| Mean turns | 4.1 | 4.2 | 6.1 | **5.8** |
| Mean tool-result bytes | 23.8 KB | **27.6 KB (+16%)** | 28.6 KB | **23.5 KB (−18%)** |
| Mean total input tokens | 55.1k | 60.2k (+9%) | 75.9k | **68.6k (−10%)** |
| s10 (hardest) turns / bytes | 8 / 30.7 KB | 9.5 / 19.6 KB | 25.5 / 104.5 KB | **19.5 / 54.5 KB** |

도구별 분해가 그 갈림을 설명한다. Claude의 `search` 합계는 거의 움직이지 않았지만(73→80 호출에 745→780 KB — 호출당 바이트는 오히려 감소), `read` 합계가 53%(120→184 KB), `grep`이 74%(53→92 KB) 올랐다: 앵커링이 한 줄 스텁으로 강등한 맥락을 Claude가 도로 가져왔다 — 최대 s7 도구 결과가 9.0 KB search 응답에서 30.4 KB 통파일 read로 교체됐다. 이미 read 중심이던 Codex는 보상하지 않고 절감을 전부 챙겼다 — 최악 케이스 과제의 바이트 −48%·턴 −24% 포함. 정답률은 어디서나 유지(함정 포함)됐으므로 회귀는 순수하게 경제적이다.

**교훈 8, 이 캠페인의 기여: 에이전트는 보상한다.** 캠페인 1은 더 풍부한 응답이 처음에 턴을 늘린다는 것을 배웠다(v5–v6); 이것은 그 거울상이다 — 더 빈약해진 응답은 *read*를 늘릴 수 있고, 응답 크기 최적화는 응답 단위 크기가 아니라 에이전트의 보상 행동을 포함한 에피소드 총 바이트로 판정해야 한다.

#### 두 번째 루프 (ds-iter3): 보상의 흡수

transcript 재진단이 iter2 회귀를 3개 기전으로 분해했고, 각각 두 번째(이자 2-루프 상한상 마지막) 수정 루프에서 고쳤다:

1. **시그니처 축약** (승인된 가설): 강등된 심볼을 맨 한 줄 스텁 대신 시그니처 + 최대 3줄(tier-2) 또는 elision 마커가 붙은 시그니처 한 줄(비매칭)로 렌더 — 그 부재가 Claude의 재읽기를 유발한 바로 그 맥락이다.
2. **`read` alias 키 정규화** (입증된 결함, 교훈 4의 미완 과제): Claude가 보낸 `startLine`(camelCase)을 alias 맵이 조용히 무시했다 — snake_case였다면 3.3 KB 범위 read였을 곳에서 한 에피소드에 30.4 KB 통파일 렌더 2회. alias 매칭은 이제 변형 열거 대신 키 정규화(소문자화, `_`/`-` 제거)를 쓴다.
3. **파일당 anchor-snippet 상한** (`search_anchor_snippet_limit`, 기본 3): `save`·`send` 같은 흔한 쿼리 단어는 *많은* 심볼의 exact-match 이름이다 — 첫 search 응답 하나가 풀 스니펫 25개(29.1 KB, rep 간 결정적)를 실었다. 초과 앵커는 이제 스텁이 아니라 축약형으로 강등된다. 라이브 A/B 패스가 symbol-overflow fallback 분기에 이 전부가 빠진 자체 렌더 경로가 있음을 잡았다(두 분기가 렌더러 하나를 공유할 때까지 d9 프로브가 움직이지 않았다: 30.1→14.1 KB).

결과, 같은 80 에피소드: Claude 에피소드 총 바이트 23.8→27.6→**19.5 KB**(다이어트 전 대비 −18%), `read` 바이트는 기준선 복귀(193→135 KB — 보상이 흡수됨), 입력 토큰 55.4k 복귀; iter2의 모든 폭주 과제가 회복됐다(s7 92.4→39.1 KB, s8 62.2→40.4 KB·13.5→10턴, d9 49.7→27.5 KB). Codex는 다이어트 전 대비 −14% 유지. 정답률은 다시 80/80 — 다만 이번 회차 재채점 게이트가 시리즈 최초의 overturn 2건을 냈는데, 둘 다 엄격 방향의 *채점자* 오류였다(명백히 존재하는 절을 "누락"으로; rubric "no-penalty" 조항 미적용) — 전문 재판정으로 correct로 정정. 잘못된 방향으로 움직인 유일한 메트릭: Claude의 mean first-answer turn이 1.15→1.70으로 드리프트(모든 에피소드가 여전히 도달은 함) — 차기 캠페인 플래그.

**공개된 오염, 사후 적발·교정.** 도구 설명을 강화한 캠페인 5 루프 1 변경이 예시 문자열을 추가했는데, 이는 *문자 그대로 과제 d7의 step-4 ground truth(파일·줄·시그니처)와 일치*했다. 오케스트레이터가 진단 중이던 바로 그 에피소드에서 수정 브리프로 복사했고; iter2·iter3 내내 모든 search 호출의 도구 설명에 들어 있었다. 세 iteration의 d7 에피소드 12개 전부에 대한 forensic: 줄 번호는 어느 답이 인용하기 전에 항상 도구 *결과*에 (2–5회) 먼저 나타났다 — 블라인드 복사 0. 예시는 중화됐고(렌더 출력 byte-identical; 대체 문자열을 전 과제 ground truth와 대조), d7은 최종 바이너리에서 힌트 없이 재실행됐다: 여전히 4/4 correct, 함정 답 0 — 정답률 결론은 무영향. 그러나 Claude의 힌트 없는 d7 턴은 11–12(no-hint iter1 범위)로 돌아왔다, 힌트가 있을 때의 약 7 대비: **외견상의 d7 턴 개선은 렌더 작업이 아니라 누출이었다.** iter3의 Claude 집계를 힌트 없는 값으로 보정하면: mean turns 4.05→**4.25**(iter1과 parity, 개선 아님 — 그 주장은 철회), mean bytes 19.5→**18.4 KB**(바이트 절감은 −22.5%로 오히려 *커진다*). Codex는 힌트 무관(내내 6–8턴). 오염 체크리스트를 위한 교훈: 제품이 contestant에게 출하하는 모든 문자열 — 도구 설명, 에러 메시지, 예시 — 은 측정 표면이다; 측정 전에 과제의 expected file/line/symbol과 기계적으로 diff하라.

### 9-9. 실제로 효과를 낸 것 — 전이 가능한 MCP 설계 교훈 7가지

이 교훈들은 이 도구 너머로 일반화된다; 이 절이 이 결과 기록이 존재하는 주된 이유다.

1. **ToolAnnotations에 `readOnlyHint`를 설정하라 — 아니면 codex가 전부 조용히 취소한다.** 비대화형 실행에서 codex는 승인이 필요한 MCP 호출을 자동 취소한다. 이 한 줄 어노테이션이 들어가기 전에는 다른 어떤 개선도 측정조차 불가능했다.

2. **기본 파라미터 값이 가장 뜨거운 코드 경로다.** claude grep 호출 122건 중 116건(95.1%)이 기본 `output_mode`를 썼는데, 이는 줄 번호 없는 파일명을 반환했고 — 에이전트는 그것을 복구하러 search/read로 되돌아갔다(c7 해부는 `benchmark-evolution.md` 캠페인 2 절 참조). 기본값을 줄 번호 포함 content로 뒤집은 것이 캠페인 2의 단일 최대 승리였다. 에이전트는 압도적으로 최소 인자로 도구를 호출한다; 문서를 읽는 인간이 아니라 에이전트를 위해 기본값을 조율하라.

3. **렌더하는 모든 스니펫에 줄 번호를 넣어라.** 과제가 "정확한 줄 번호 인용"을 요구하는데 스니펫이 그것을 보여주지 않으면, 에이전트는 이미 본 범위를 번호만 얻으려고 재읽기한다. 캠페인 1에서 풍부해진 search 응답은 중앙값을 12.5턴까지 *밀어올렸다*(궤적 표의 v6); 스니펫을 `read`와 같은 `  1234→ …` 형식으로 렌더하자 풍부한 응답을 유지한 채 9.5로 돌아왔다 — 그 캠페인에서 단일 변경이 만든 최대 효과.

4. **파라미터는 강제 변환하라; alias 두더지잡기 금지.** claude 계열 에이전트는 습관적으로 `path`, `file`, `start_line`/`end_line`을 보냈다(iter1의 28개 에피소드에서 hard 에러 52건 + 조용히 무시된 범위 파라미터 48건; codex: 0 — 스키마 규율은 모델마다 다르다). 숫자를 JSON 문자열(`"228"`)로도 보내는데, 엄격한 `as_u64()`는 이를 조용히 "범위 없음"으로 만든다 — 통파일 렌더, 그다음 크기 제한 에러. 항구적 수정은 alias를 하나씩 쫓는 것이 아니라 관용적 coercion 레이어(문자열→숫자, alias 맵)였다.

5. **BM25 단독으로는 짧고 흔한 멤버 이름을 랭킹하지 못한다 — owner 토큰을 인덱싱하라.** `get`은 변별력이 없어 "StorageFactory get" 쿼리조차 실패했다. 각 멤버의 소유 타입 이름(과 그 분할 토큰)을 멤버 문서에 인덱싱하자 그 과제가 실패 검색 10회에서 2번째 호출 발견이 됐다.

6. **에이전트가 실제로 인용하는 것을 인덱싱하라: 문자열 리터럴과 enum variant.** "기본 포트는 어디 정의되나"·"read-only 쓰기에 어떤 에러가 발생하나" 같은 과제는 인용 값 조회다. enum variant는 심볼이 아니었고 리터럴은 미인덱싱이라 각각 grep 우회 4–5턴이 들었다; 둘 다 인덱싱(리터럴 256자 상한, 심볼·docstring 보다 낮게 부스팅)하자 정의 파일이 최상위 검색 히트가 됐다.

7. **이름당 비싼 스캔을 유계화하고, 에이전트가 신뢰할 수 있는 절단 라벨을 써라.** 전역 caller-scan 예산은 한 핫 이름이 나머지를 굶기게 했다 — 실제 call site 19곳인 심볼이 절단 마커도 없이 "no direct caller observed"로 렌더됐다. 이름당 예산(하한 25) + 이름당 절단 플래그가 고쳤다. 관련해서, caller 라벨 "(approximate, name-match only)"는 사실은 정확한 위치를 에이전트가 재검증하게 만들었다; "(file:line positions exact; name-match attribution approximate)"로 고쳐 쓰자 불필요한 재읽기가 멈췄다. 에이전트는 라벨을 문자 그대로 읽는다 — 표현의 정밀성은 성능 기능이다.

### 9-10. 측정 통제

통제 규칙 자체는 본 문서 앞부분이 정본이다 — pure-MCP 격리 플래그·고정 명령은 §7(함정의 경위는 §4-2; repo `AGENTS.md`/`CLAUDE.md` 격리 포함, baseline의 `.codemap` 격리는 §7-7), 축어 프롬프트와 제품 출하 문자열의 ground-truth 기계 대조는 §4-1 원칙 5·6(누출 실례의 공개는 §9-8), 기계 추출 메트릭 정의는 §6, 채점·재채점 사다리는 §4-6·§4-7, 타임아웃 실집행·멱등 재개는 §4-3·§7-1. 이 절은 그 규칙들이 캠페인 전반에서 실제로 검증된 집계 결과만 기록한다.

- **오염 0건.** 모든 transcript를 작성자 로컬 훅 설정에서만 등장하는 마커 문자열로 기계 스캔 — 보존된 캠페인 2/3/4/5 에피소드 496개 전부 0건(160 + 파일럿 4 + 매트릭스 80 + baseline warmup 4 + baseline 매트릭스 80 + 다이어트 루프 2회 각 warmup 4 + 매트릭스 80). 캠페인 1 transcript는 이 기계 검사 도입 이전이다.
- **재채점 불일치 0 (초기 2패스).** 최난도 에피소드를 더 강한 모델의 적대적 패스로 저장소 원본과 대조해 재채점한 초기 두 패스에서 점수 변경 0(캠페인 2에서 15 에피소드, 캠페인 3에서 40). 이후 회차를 포함한 게이트 4회의 판정 기록은 `benchmark-evolution.md` 채점 신뢰성 절이 단일 출처다(ds-iter3의 시리즈 최초 overturn 2건은 둘 다 채점자 측 오류).
- **공개된 측정 아티팩트.** 에피소드 duration은 provider API 지연을 포함한다. "first-answer-turn rate"는 모델 답변 스타일에 민감함이 입증됐고 — 한 arm의 외견상 하락은 보조 하니스 도구의 호출이 턴으로 집계된 것이지 제품 회귀가 아니었다 — 회차 간 불안정 지표로 취급한다(baseline first_answer null 6건은 §9-7 메트릭 경고 참조). iter2 에피소드 80개 중 55개의 자유 텍스트 답변 요약이 축어 인용이 아닌 paraphrase였다; 표본 점검(전수 감사 아님)에서 인용 위치의 왜곡은 발견되지 않았다.

### 9-11. 캠페인 비용

C/C++ 캠페인은 오케스트레이션 측 LLM 토큰 약 7.2M(계획·데이터셋 구축·채점·분석· 적대적 리뷰 — 측정 iteration당 약 3.0M과 약 2.9M)에 contestant CLI 에피소드 약 165회를 소모했다. 초기 캠페인의 wall-clock 오버헤드 구조(에피소드별 에이전트 운영)와 하니스 재작성(스크립트 구동 병렬 실행) 후의 실측 절감 수치는 §4-3 항목 2·5가 정본이고, 그 재작성을 검증한 캠페인 3의 운영 실측은 §9-6에 있다. 여기의 비용·wall-clock 수치는 실행 로그에서 재구성한 근사치다 — 기계 추출되는 에피소드별 메트릭과 달리.

### 9-12. 한계

1. **baseline arm은 이제 존재하고, "빌트인 grep/read보다 낫다"는 여전히 주장하지 않는다.** 캠페인 4가 측정했다: 정답률은 동일(모든 곳 100%, 세트 포화)하고, 효율 델타는 실재하지만 혼합적·CLI 특이적이다 — claude는 적은 턴을 무거운 응답과 교환하고, codex는 꼬리 억제 외에 가시적인 교환이 없다. 포화된 세트에서 baseline 캠페인이 보여줄 *수 없는* 것은 빌트인이 실패하는 곳에서 도구가 돕는지다 — baseline이 100% 아래로 떨어질 만큼 어려운 과제 세트가 필요하다.
2. **자가 벤치마크, LLM 채점.** 작성자 설계 과제, 에이전트 적용 rubric, 적대적 표본 검증 — 같은 작성자에 의해. 독립 재현을 환영한다; 데이터셋·rubric·하니스 스크립트는 이 저장소에 있다(§10 참조).
3. **캠페인 2 수정은 in-sample이었다; 캠페인 3이 hold-out이었고, 유지됐다.** 모든 캠페인 2 수정은 iter1 transcript에서 진단되고 *같은* 과제로 재측정됐다(owner-token 수정은 과제 c8에서 동기를 얻어 과제 c8로 검증). 캠페인 3은 수정된 제품을 미접촉 언어 2종의 미접촉·더 어려운 과제 20개로 재시험했다: 효율 프로파일과 첫 호출 발견이 전이됐다(캠페인 3 절 참조). 포화된 세트에서의 hold-out 성공이 보여줄 수 없는 것은 headroom이다 — 그것엔 더 어려운 세트나 baseline arm이 필요하다.
4. **소표본.** repo당·arm당 10과제 × 2 rep; partial 1건이 arm 정답률을 5%p 흔든다. arm별 델타는 방향성 지표로 취급할 것.
5. **포화, 두 번.** 캠페인 2 iter2가 80/80을 쳤고, 캠페인 3 세트는 의도적으로 더 어렵게 재구축(2 easy : 4 medium : 4 hard, 다단계 흐름, alias 간접, 동적 dispatch, 함정 답)됐는데 — 그래도 80/80으로 포화했다. 이 등급의 작성자 설계 내비게이션 과제에서 정답률은 더 이상 변별하지 못한다; 향후 변별은 baseline 비교, depth-2 call-chain 요구, 모호한 자연어 개념 쿼리, exact-line-label rubric에서 와야 한다.
6. **실행 간 분산은 완전히 분리되지 않는다.** iter1 vs iter2는 같은 과제를 재사용한다; 델타 일부는 모델 비결정성이다(ClickHouse의 codex는 더 정확해지며 11% *느려졌고*; ollama의 codex는 턴이 하나 늘었다).
7. **대상 repo 커밋이 핀되지 않았다 — 캠페인 3 포함.** 측정 저장소는 git 메타데이터 없는 2026-06 스냅샷이었고 커밋 SHA가 기록되지 않았다. 캠페인 2 회고는 "캠페인 3부터" 핀을 약속했다; 캠페인 3의 스냅샷이 다시 `.git` 없이 출하되어 약속은 미이행됐고 여기에 공개한다. 따라서 줄 번호 ground truth는 그 스냅샷들에 결속되며, 독자가 오늘 `git checkout`할 수 있는 무엇에도 결속되지 않는다. SHA 기록은 이제 §10 재현 가이드 절차의 체크리스트 항목이다.
8. **읽기 전용 내비게이션 과제만.** 편집·리팩터링·빌드 과제는 측정하지 않았다.
9. **회차 1개가 무효화됐고**(v4, 프롬프트 실수) **에피소드 1개가 제외됐다**(provider 장애). 어느 쪽도 보고 수치에 포함되지 않으며, 둘 다 여기 공개한다.
10. **에피소드별 transcript는 아카이브되지 않는다.** 이 수치 뒤의 JSONL transcript·메트릭은 측정 머신에 존재하지만 repo에 커밋되지 않았고 장기 보존 되지 않을 수 있다. 위의 무결성 주장(오염 0, 재채점 불일치 0)은 오늘 검증 가능하지, 무기한은 아니다.

### 9-13. 다음 과제 — 변별 가능한 과제 세트

캠페인 4는 이 과제 세트가 허용하는 한도까지 baseline 질문을 닫았고, 캠페인 5의 수정 루프 2회는 응답 다이어트 질문을 닫았다: 보상을 흡수한 두 번째 루프 후 양 arm 모두 다이어트 전 자신을 바이트에서 이겼고(Claude 힌트 보정 −22.5%, Codex −14%) 정답률 100%·턴 parity를 유지했으며, Claude-vs-빌트인 바이트 격차는 +73%에서 약 +34%로 줄고 Codex-vs-shell은 −53%로 벌어졌다. 루프 상한은 소진됐다. 최상위 *측정* 백로그 항목은 여전히 **변별**이다: 빌트인 전용 arm이 실제로 100% 아래로 떨어지는 과제 등급 — depth-2 callee 요구, 모호한 자연어 개념 쿼리, 다수 후보에 흩어진 답, exact-line-label(±0) rubric — 그래야 baseline 비교가 스타일이 아니라 정확도에 대해 말할 수 있다. 더 작은 후속: anchor 중심 렌더링 하의 Claude first-answer-turn 드리프트(1.15→1.70), 채점자 근거의 rubric 조항 인용(이번 캠페인에서 시리즈 최초의 재채점 overturn이 나왔고, 둘 다 채점자 측), 커밋 핀 스냅샷(`.git` 제거 전 `git rev-parse HEAD`), 아직 fixture 수준인 assembly 추출을 위한 asm 실측 저장소(asm 파일이 실재하는 musl·openssl·linux 일부 등 1종 추가), first_answer_turn 정의 정밀화("도구 결과에 expected.file 등장" 기준은 넓은 검색 결과에 우연히 실리는 경우를 구분 못 한다 — "정답 줄 번호까지 노출"로 강화 검토), read 102KB 한도 관찰(코어션 수정 후에도 한도 도달이 잦으면 그때 재논의 — truncate 전환은 잘린 인용 위험으로 기각된 바 있음; django+strapi 캠페인에서는 read 파라미터 에러 0건), 그리고 folder overview 무게 cap — 대형 deep 폴더에서 significant 심볼 목록 자체가 비대한지는 2026-06-09 고도 보정에서 분리된 대형 티어 measure-then-decide 트랙(§8-2 결정 수치 항목)으로 여전히 미종결이며, 착수 전에 캠페인 5 응답 다이어트 이후의 실효성부터 재평가한다. ds-base1 baseline 수치는 제품 독립적이며 고정 기준선으로 유지된다.

제품 개선 백로그(렌더·랭킹·코어션 등 제품 측 후속 항목)는 `benchmark-evolution.md` 백로그 절이 단일 출처다.

## 10. 재현 가이드

캠페인 자산의 위치·역할은 §5 자산 포인터가 단일 출처다. 재현 절차:

1. release 바이너리를 빌드한다.
2. 대상 저장소 스냅샷을 직접 핀한다 — **`git rev-parse HEAD`를 기록한 뒤** `.git`을 제거(§9-12 한계 7의 교훈) — 하고 측정 전용 경로에 체크아웃한다.
3. repo당 1회 사전 인덱싱 후 포맷 sidecar를 확인한다 (§7-5).
4. 하니스 설정(`harness/config.sh`)에 바이너리·repo·tasks 경로를 지정한다.
5. tasks JSON을 작성하거나 재검증한다 — ground truth는 측정 대상 도구가 *아닌* 도구(플레인 `rg`/수동 읽기)로만 수립한다.
6. 4-에피소드 파일럿(warmup) 게이트를 통과한다 (§7-6).
7. 매트릭스를 실행한다: `harness/run-matrix.sh <iteration> [concurrency]` — `ARMS="claude-sonnet-base codex-gpt55-base"`로 baseline arm 선택. 에피소드는 멱등·재개 가능하며, 채점·집계 스크립트와 4-arm 토큰 추출이 포함된다.
8. verify → 채점 → 재채점 → 집계는 §2 단계 구성과 §3 모델 배치대로 진행한다.

"재현"에 대한 정직한 단서 하나: 측정 스냅샷이 커밋 핀되지 않았으므로(§9-12 한계 7) 캠페인 1·2 데이터셋의 줄 단위 ground truth는 신선한 클론과 어긋난다. 저장소 자산이 가능하게 하는 것은 직접 핀한 스냅샷 위에서 *같은 절차*를 도는 것이다. 대조적으로 사전 벤치마크(축 A/B)의 코퍼스는 §8-1의 고정 SHA 표로 재클론 복원이 가능하다 — 그 표가 재클론 복원의 단일 출처다.

과제 데이터셋은 대상 저장소(SurrealDB, ollama, ClickHouse, Django, Strapi)의 짧은 증거 인용을 포함한다; 해당 발췌는 원 저장소의 라이선스를 따른다.
