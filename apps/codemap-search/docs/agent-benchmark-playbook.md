# 에이전트 벤치마크 플레이북 — 재현·확장 가이드

codemap-search 에이전트 벤치마크를 다시 돌리거나(회귀 게이트) 확장(새 저장소·새
arm·난이도 상향)할 때 따르는 절차. 2026-06 두 캠페인(surrealdb 단일 arm,
C/C++ 2-arm — 인과 서사는 `benchmark-evolution.md` 참조)에서 실측으로 굳어진
내용만 담는다.

## 1. 사전 조건

```bash
cd apps/codemap-search && cargo build --release
# 바이너리: apps/codemap-search/target/release/codemap-search
```

- 대상 저장소를 측정 전용 경로(예: /tmp/benchmark-data/<repo>)에 체크아웃. 측정
  중 저장소 파일을 절대 수정하지 않는다. **체크아웃 직후 `git rev-parse HEAD`를
  기록한 뒤** `.git`을 제거할 것 — 캠페인 2·3 연속으로 SHA 미기록이 재발해 줄 단위
  ground truth가 스냅샷에만 결속됐다 (`benchmark-workflow.md` §8 한계 7).
- repo당 1회 사전 인덱싱(에피소드가 인덱싱 시간을 내면 측정 오염):
  `cd <repo> && <binary> index .` → `.codemap/index/codemap.format`이 현재
  `EXTRACTION_FORMAT_VERSION`(2026-06 기준 `v7-owner-tokens-indexed`)인지 확인.
  규모 참고: ollama 약 1.3s/5MB, ClickHouse 전체 약 11.9s/27MB.
- **제품 코드를 수정했다면**: 추출/인덱싱 출력이 바뀌는 변경은 반드시
  `EXTRACTION_FORMAT_VERSION` 범프 → 재인덱스. 도구 레이어(read/grep/overview
  파라미터 처리 등)만 바뀌면 범프·재인덱스 불필요.

## 2. 하니스 — 검증된 호출 형태 (`bench-2026-06/episode-runbook.md`가 원본)

### codex arm
```bash
codex exec -C <REPO> --skip-git-repo-check --ignore-user-config --ephemeral \
  -s read-only -m gpt-5.5 -c model_reasoning_effort="medium" \
  -c approval_policy="never" \
  -c 'mcp_servers.codemap-search.command="<binary>"' \
  -c 'mcp_servers.codemap-search.args=["mcp"]' --json "<PROMPT>"
```

### claude arm (pure MCP)
```bash
cd <REPO> && claude -p --model sonnet --setting-sources "" \
  --mcp-config <mcp-codemap.json> --strict-mcp-config \
  --allowedTools "mcp__codemap-search__search,...(5종 전부)" \
  --disallowedTools "Bash,Read,Glob,Grep,Edit,Write,WebFetch,WebSearch,Task,NotebookEdit,TodoWrite,Workflow,Agent,Skill" \
  --output-format stream-json --verbose "<PROMPT>"
```

### 함정 목록 (전부 실측으로 얻은 것 — 건너뛰면 한 회차를 날린다)

| 함정 | 증상 | 대응 |
|---|---|---|
| `--safe-mode` | 명시적 `--mcp-config`까지 차단 → MCP 도구 0개 | `--setting-sources ""` + `--strict-mcp-config` 조합만 사용 |
| 사용자 훅 오염 | 훅 출력이 contestant 프롬프트에 주입 | 격리 후에도 transcript에서 훅 문자열 grep으로 매회 검증 |
| codex MCP 자동취소 | ToolAnnotations(`readOnlyHint`) 없으면 비대화형에서 전 호출 취소 | MCP 호출 0 + `requires_mcp_tool_approval`이면 회귀로 판정 |
| macOS `timeout` 부재 | 명령 즉시 실패(exit 127) | Bash 도구 타임아웃(600000ms)으로 대체, wall clock은 `date +%s` |
| 프롬프트 재구성 | 도구 유도 문구 누락 → 측정 오염(v4 사고) | 프롬프트는 tasks JSON에서 jq로 추출, 한 글자도 수정 금지 |
| tantivy writer lock (정정됨) | 충돌을 우려한 "repo 내 순차"가 wall clock을 키움 — 실제 충돌 관측 0건 | 사전 인덱싱된 미변경 repo는 동일 repo 병렬 안전 (§4 실측 근거) |
| ToolSearch 집계 포함 | first_answer_turn/turns 왜곡 (0.8→0.4 가짜 회귀 사례) | ToolSearch(하니스 메커니즘)는 도구 호출로 세지 않는다 |
| answer_text 요약 | 메트릭 충실도 저하 (80건 중 55건 요약 사례) | 러너에게 축어 기록 강제, 길면 파일로 저장하고 경로 기록 |
| codex jsonl 무타임스탬프 | duration 산출 불가 | wall clock 측정 필수 |
| 채점 자가 신고 의존 | 점수 인플레이션 위험 | 회차 후 상위 모델이 어려운 과제 중심 표본 재채점 |
| claude `--disallowedTools` 가변 인자 | 플래그 값 바로 뒤에 프롬프트를 두면 프롬프트 단어가 거부 도구 목록으로 파싱 → "Input must be provided" 에러 | 비가변 플래그(`--output-format ...`)를 사이에 두는 검증된 어순 고정 (2026-06-12 실측) |
| 대상 repo의 `AGENTS.md`/`CLAUDE.md` | codex가 repo AGENTS.md를 기본 주입(claude도 cwd CLAUDE.md 로드 가능) → contestant 오염 | 측정 사본에서 사전 격리(이동). 신규 repo 점검 절차에 `ls AGENTS.md CLAUDE.md .cursorrules .claude` 포함 (strapi 실례) |

## 3. 데이터셋 설계 원칙 (`bench-2026-06/tasks-*.json`이 견본)

- 과제 유형 스펙트럼: definition / definition+callers / concept / 다단계 flow /
  literal·config / error+발생지점. 난이도 easy:medium:hard ≈ 3:4:3.
- **루브릭은 기계 판정 가능해야 한다**: 허용 오차(±0~2줄)를 명시하고, partial/오답
  경계와 함정 답안(오답 처리 기준)까지 적는다. 호출 지점 N곳 요구면 N+2곳 이상을
  전수 grep으로 확보해 두고 전부 나열한다.
- 정답 확정은 빌트인 Read/Grep만 사용 — **측정 대상 도구(codemap-search MCP)로
  ground truth를 만들지 않는다.**
- 과제는 인덱싱되는 파일만 대상(SOURCE_EXTENSIONS — md/CMake/xml/yaml 무효).
  gitignore된 경로(예: ollama의 llama/vendor)도 무효.
- 프롬프트 고정 프레임: `"codemap-search MCP 도구를 사용해서 <과제>. 정확한 파일
  경로와 줄 번호를 인용해서 답해. 파일은 수정하지 마."`
- 함정 보기(distractor)를 의도적으로 심는다(예: `zkutil::ZooKeeper` vs
  `Coordination::ZooKeeper`) — expected에 함정과 오답 판정 기준을 같이 기록.

## 4. 실행 매트릭스·운영

1. **파일럿 게이트 필수**: 본 매트릭스 전 1문항 × 1회 × 전체 arm — 본 매트릭스와
   같은 동시성으로 동시에 돌려 병렬 배선까지 함께 검증한다. 통과 기준 6종
   (정상 종료/파싱, MCP 호출 ≥1, 오염 0, pure 보장, 표준 스키마 추출+채점 가능,
   lock·연결 에러 0). 2026-06 캠페인에서 파일럿 FAIL이 `--safe-mode` 결함을
   본실행 전에 잡았다 — 80 에피소드를 버릴 뻔한 비용을 2 에피소드로 막은 것.
   django+strapi 캠페인(ds-iter1)도 동일 절차 실측: 파일럿 4에피소드로 6종 기준
   전부 통과(`results/dryrun-pilot/`), 추출 회귀는 이전 회차 실데이터 대조로 검증,
   배선 프로브로 두 repo 모두 MCP 5종 노출·빌트인 0·훅 오염 0 확인 후 본실행했다.
2. 매트릭스: **에피소드 단위 병렬, 동시성 8~16 — 동일 repo 동시 실행 포함**
   (안전성 실측 근거는 아래 소절). 배정 순서는 arm·과제 무작위 셔플 — 직렬 운영의
   arm 교차 배치는 병렬에서는 셔플로 대체된다. 동시성 상한을 정하는 것은 제품이
   아니라 contestant API rate limit과 로컬 자원이다. 에피소드당 메트릭 JSON을
   디스크에 영속(runbook §4 표준 스키마).
3. **실행·추출은 스크립트로, 에이전트는 판단에만**: 에피소드 실행은 고정 명령의
   결정적 작업이다(§2 — 프롬프트 재구성 금지). 에피소드마다 러너 에이전트를 띄운
   2026-06 방식은 wall clock의 75~83%를 오버헤드(스폰·추출·채점 대기)로 만들었다 —
   캠페인 wall 7.25h 중 에피소드 순수 실행 합은 2.45h(162개, 중앙값 37s/에피소드),
   2레인 임계 경로는 1.37h에 불과했다. 하니스 명령은 동시성 N 스크립트로 실행하고
   메트릭 추출도 jq 스크립트로 처리하라. 에이전트는 루브릭 채점·분석에만 배치
   단위(예: 10 에피소드/에이전트)로 투입 — 토큰도 ~3M/회차에서 크게 준다.
4. **타임아웃을 실제로 강제하라**: 600s 상한을 선언하고도 iter1에 1558s 에피소드가
   기록됐다(미집행). 병렬에서는 꼬리 에피소드가 임계 경로를 지배하므로 타임아웃
   집행이 wall clock에 직결된다.
5. 하니스 수준 실패(프로세스 사망, MCP 연결 실패)만 1회 재시도. 오답은 재시도
   금지. API 장애는 score=n/a로 분모에서 제외.
6. 규모 참고(2-arm × 10문항 × 2rep × 2repo = 80 에피소드/회차): 직렬 운영 실측
   wall 4.0h(iter1)·2.4h(iter2), 캠페인 전체 7.25h. **병렬 8 + 스크립트 실행 실측
   (django+strapi 캠페인): 80 에피소드 wall 약 5분** (에피소드 중앙값 25s, lock
   에러 0). 채점+재채점 서브에이전트 토큰도 회차당 ~3M → ~0.45M로 절감. 외부
   contestant 비용 별도.
7. 측정→수정 루프는 상한을 미리 정한다(2026-06: 2회). 재루프 기준: 정답률 100%
   미만 **그리고** transcript로 입증된 제품 결함이 있을 때만.
8. **회차 이름·잔재 규율**: 차기 회차(iterN+1) 실행은 현 회차의 채점·분석·수정이
   끝나기 전에 시작하지 않는다. 조기 시작돼 중단된 회차 잔재는 반드시 삭제하거나
   이름을 바꿔 보관할 것 — 멱등 재개(metrics 존재 시 skip)와 결합하면, 제품 수정
   후 같은 이름으로 본실행할 때 수정 전/후 바이너리가 섞인 오염 회차가 된다
   (django+strapi 캠페인에서 19/80 잔재를 본실행 전에 적발·삭제한 실례).

### 동일 repo 병렬 안전성 — 실측 근거 (2026-06-12 검증)

직전 캠페인의 "tantivy writer lock 때문에 repo 내 순차" 규칙은 과잉 보수였다.
코드 경로 전수 분석과 라이브 프로브로 정정한다.

- **코드**: mcp 런타임에서 tantivy IndexWriter를 획득하는 지점은 `index.rs`의
  `apply_index_updates` 한 곳뿐이고, 변경 파일이 없으면 writer 획득 전에 조기
  반환한다(`index.rs:454-456`). LockFailure는 모든 경로에서 비치명 — warn 후
  stale 서빙으로 강등(`index.rs:461-464`). 시작 시 sidecar·config 쓰기도 사전
  인덱싱된 repo에서는 발생하지 않는다.
- **프로브**: 신선 인덱스에 MCP 프로세스 8개 동시 → 전원 정상 응답, lock 메시지
  0건. stale(파일 touch 직후) 6개 동시 → 전원 정상 응답, `LockFailure: LockBusy`
  warn 5건(비치명, 1개 프로세스만 재인덱싱 수행), 인덱스 무손상.
- **전제 2가지**: (1) 측정 전 단일 프로세스로 사전 인덱싱을 끝내고 포맷 버전
  일치를 확인한다(§1). (2) 측정 중 소스 트리를 변경하지 않는다. 전제가 깨져도
  corruption은 없지만, 일부 프로세스가 stale 결과를 서빙해 측정이 오염된다.

## 5. 분석·검증 사다리

- 회차 분석(sonnet): 실패 에피소드 원인을 (a) 제품 결함 (b) 하니스/채점 (c) 모델
  한계로 분류, transcript 인용 필수. 같은 과제가 rep·arm을 가로질러 실패하면
  (a) 우선 조사.
- **상위 모델 정정 패스를 분석 직후에 둔다**: 2026-06에서 sonnet 분석의 근본 원인
  오진(별칭 문제로 진단, 실제는 문자열 숫자 파싱)이 fable 리뷰에서야 잡혔다.
  오진이 수정 라운드에 들어가면 그 라운드는 헛돈다. 분석의 fix_hypothesis는
  수정 전에 라이브 바이너리로 재현 검증할 것.
- 회차 후 재채점: 어려운 과제 중심 표본 ≥10개를 상위 모델이 루브릭 기계 적용 +
  저장소 원본 줄 대조로 재채점. 유효한 분업: sonnet이 인용 줄 전수 대조 + rubric
  체크리스트를 기계 수집하고, 판정은 fable이 직접 — 수집과 판정을 분리하면 물량을
  키우면서도 게이트 품질이 유지된다 (django+strapi 캠페인: 표본 40, 불일치 0).
- 실패 0건(전 arm 100%) 회차는 분석 사다리의 원인 분류·정정 패스가 공집합으로
  종결된다 — 대신 효율 지표(턴·바이트·도구 믹스·first_answer_turn) 분석과 포화
  여부 판단을 회차 보고에 남길 것.

## 6. 모델 역할 배치 (멀티 모델 오케스트레이션)

2026-06 C/C++ 캠페인에서 실측 검증된 배치. 원칙: **물량은 sonnet, 명세된 수정은
opus, 진실 판정은 fable.** 상위 모델은 '생산'이 아니라 '게이트'에 배치한다 —
각 단계의 산출물이 다음 단계의 입력이 되므로, 게이트에서 틀린 것이 통과하면
이후 모든 비용이 오염된다. (이 배치가 굳어진 실측 경위 — 블로커 적발·오진
정정 — 는 `benchmark-evolution.md` 캠페인 2 절 참조.)

| 역할 | 모델 | 근거 (캠페인 실측) | 토큰 규모 |
|---|---|---|---|
| 계획·오케스트레이션·브리프 작성 | fable (메인 루프) | 결정 지점 판단과 브리프 품질이 전체 효율을 결정 | — |
| 측정 전 게이트 검증 | fable | opus 적대 리뷰 2회를 통과한 코드에서 블로커(참조 반환 누락) 적발 — 실저장소 인덱싱 스모크 포함 필수 | ~113k/회 |
| 데이터셋 + 루브릭 | fable | 함정 보기·전수 검증 설계 → 160 에피소드 채점 분쟁 0건 | 70~94k/repo |
| 파일럿·채점·집계 (실행·추출은 스크립트 — §4-3) | sonnet | 에피소드 162회 무손실·스키마 준수 100%였으나, 에이전트/에피소드 방식이 wall clock 오버헤드 75~83%의 원인 | 채점 배치당 ~35k |
| 회차 분석 — 현상 발견 | sonnet | 고영향 결함(grep 기본값, read 별칭) 발견은 정확 | 회차 비용 포함 |
| 회차 분석 — root-cause 확정 | **fable 권장 (최소 opus)** | sonnet이 사실 오류 3건(근본 원인 오진 포함) — fix_hypothesis는 수정 라운드 진입 전 라이브 바이너리로 재현 검증 | 정정 패스 1회 |
| 코드 수정 | opus | 4라운드 전부 경고 0·테스트 green·제약 위반 0, 브리프 밖 추가 원인도 자체 발견 | 70~102k/라운드 |
| 종료 전 적대 리뷰·표본 재채점 | fable | 재채점 15건(불일치 0) + 오진 정정 + DO-NOT-FIX 선별 | ~153k |
| contestant (측정 대상) | 고정 (sonnet, gpt-5.5 등) | 하니스 §2 — 오케스트레이션 모델과 분리해 고정 | 외부 비용 |

### 모델별 가드레일 (실측된 실패 모드)

- **sonnet**: 제약 이탈이 실측됨(신규 테스트 금지를 어기고 7개 추가 → 제거).
  브리프에 금지 사항을 명시하고 산출물을 기계 검증(StructuredOutput 스키마 강제,
  diff grep 검사)하라. 코드 수준 인과 추론은 시키지 말 것 — transcript 증거가
  붙은 현상 보고까지가 적정선이다.
- **opus**: 진단이 정확할 때만 정확하다. 오진된 fix_hypothesis를 그대로 주면
  라운드가 헛돈다(iter2 분석의 read 결함 오진이 실례 — fable 정정 없이 진행했다면
  최종 라운드는 아무것도 못 고쳤다). 브리프에 이전 분석의 오진 정정을 명시적으로
  전달하라.
- **fable**: 가장 비싸므로 게이트 지점에만 — 측정 전 1회, 분석 정정 1회, 종료 전
  1회. 생산 루프(에피소드 실행, 집계)에 넣지 말 것.
- **오케스트레이터 자기 점검**: 재채점 게이트를 opus 등 하위 에이전트로 통째로
  위임하지 말 것 — opus는 "명세된 수정" 전용이고, 게이트 판정은 fable 본인 몫이다
  (django+strapi 캠페인에서 opus 위임 시도가 사용자 정정으로 회수된 실례). 물량이
  크면 위 §5의 sonnet 증거 수집 + fable 판정 분업을 쓴다.

### 브리프(발주) 규칙

- 수정 라운드: "정확히 이것만 고쳐라" 목록 + **DO-NOT-FIX 목록** + 검증 의무
  (빌드·테스트·라이브 재현, 결과 원문 첨부)를 포함한다.
- 검증 라운드: "자가 보고를 믿지 말라" + 원본 대조 의무(저장소 원본 줄, transcript,
  라이브 바이너리)를 포함한다.
- 모든 라운드 공통: 커밋 금지, 범위 밖 수정 금지, 최종 메시지는 orchestrator용
  원시 데이터(파일:줄, 실측 출력 포함)로 지정한다.

## 7. 다음 벤치마크 백로그 (2026-06 캠페인들이 남긴 것)

1. **baseline arm — 최우선으로 격상 (django+strapi 캠페인 반영)**: {claude, codex} ×
   빌트인 도구만 구성을 추가해 "codemap의 부가가치"를 직접 측정 — 세 캠페인 연속
   미답. 난이도를 올려 만든 hold-out 셋(django+strapi)마저 전 arm 100%로 포화한
   지금, 정확도 축에서 남은 유일한 변별 질문이다. baseline은 제품 수정의 영향을
   받지 않으므로 1회만 측정해 루프에서 재사용.
2. **변별력 수단 (난이도 상향 계속)**: 포화 2연속 — C/C++ 셋(전 arm 100%)에 이어
   다단계 흐름·별칭·동적 디스패치·3,500줄+ 파일을 넣은 django+strapi 셋도 포화.
   남은 수단: depth-2 callee 요구, 모호한 자연어 개념 검색, 정답이 여러 후보에
   분산된 과제, 줄 라벨 ±0 정확성 rubric(django+strapi에서 claude 답안의 스니펫
   줄 라벨 ±1 전사 오류 2건 관찰 — 현 rubric으로는 채점 영향 0), 5,000줄+ 파일.
3. **asm 실측**: asm 파일이 실재하는 저장소(musl, openssl, linux 일부) 1종 추가 —
   `.globl` export·라벨 추출의 실전 검증은 아직 픽스처 수준이다.
4. **메트릭 교정**(runbook에 이미 기록): ToolSearch 제외 집계, answer_text 축어,
   codex wall clock — django+strapi 캠페인에서 적용 확인(중복 0, 축어 기록, 오염 0).
5. **first_answer_turn 정의 정밀화**: "도구 결과에 expected.file 등장" 기준은
   넓은 검색 결과에 우연히 실리는 경우를 구분 못 한다 — "정답 줄 번호까지 노출"로
   강화 검토.
6. **read 102KB 한도 관찰**: 코어션 수정 후에도 한도 도달이 잦으면 그때 재논의
   (truncate 전환은 잘린 인용 위험으로 기각된 바 있음). django+strapi 캠페인에서는
   read 파라미터 에러 0건.

## 8. 자산 인덱스

경로는 이 문서가 있는 `apps/codemap-search/docs/` 기준 상대 경로.

| 자산 | 위치 |
|---|---|
| 에피소드 runbook (호출 명령·스키마·파일럿 기준) | `bench-2026-06/episode-runbook.md` |
| 데이터셋 (ollama 10 + ClickHouse 10, 루브릭 포함) | `bench-2026-06/tasks-*.json` |
| 데이터셋 (캠페인 1 — surrealdb t1~t6 과제·ground truth) | `bench-2026-06/tasks-surrealdb.md` |
| claude arm MCP 설정 | `bench-2026-06/mcp-codemap.json` |
| 회차 분석 보고서 | `bench-2026-06/analysis-iter{1,2}.md` |
| 캠페인 통합 회고 — 설계 진화 인과 (관찰→가설→수정→검증, 전 캠페인) | `benchmark-evolution.md` |
| django+strapi 캠페인 (hold-out) 데이터셋·baseline runbook | `bench-2026-06-django-strapi/` |
| 〃 셸 하니스 (매트릭스·채점·집계 스크립트) | `bench-2026-06-django-strapi/harness/` |
| 〃 재채점 게이트 판정 기록 | `bench-2026-06-django-strapi/gate-verdict.md` |
| 캠페인 통합 결과 (수치 표·한계) | `benchmark-workflow.md` §8 |
