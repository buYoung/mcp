# codemap-search 개선 기회 분석 보고서 (2026-06-12)

> fable sub-agent의 read-only 탐색 산출물. 근거: 이전 캠페인(clickhouse+ollama) iter1/iter2
> 실전사 160 에피소드 전수 분석, HEAD 소스 대조, HEAD 바이너리 라이브 재현(django/strapi),
> dogfooding 인스턴스 4종 직접 호출. ds 캠페인(django/strapi) 측정→수정 루프의 후보 목록.

## 조사 방법 요약

- 과거 캠페인 원전사 분석: iter1/iter2 160 에피소드 전수 메트릭 집계 + 고비용 에피소드 12건 전사 정밀 판독(`/tmp/benchmark-data/results/iter{1,2}/`). search 호출 281건의 쿼리·응답을 프로그램으로 분류.
- 소스 코드 대조: 각 마찰의 원인 지점을 HEAD 코드에서 확인 (iter2 이후 "최종 라운드" 코어션 수정이 이미 반영된 상태 기준).
- 라이브 검증: HEAD release 바이너리의 CLI search를 ClickHouse/ollama/django/strapi 체크아웃에 읽기 전용으로 실행해 랭킹 실패를 재현. 연결된 dogfooding 인스턴스 4종에서 도구 직접 호출.
- 주의: 연결된 dogfooding MCP 인스턴스(fd/ripgrep/scrapy/hicare)는 구버전 바이너리였고, 5/5 인스턴스에서 search가 인덱스 포맷 비호환으로 전면 다운 상태였다(후보 09의 1차 증거).

---

## 개선 후보 (우선순위순)

### 01. 매치 선택·쿼리 전처리의 토크나이즈 정합화 — `::`/`.`/`/` 붙은 쿼리어 분해

- **가설**: 쿼리 단어를 비식별자 문자(`::`, `.`, `/`, `(` 등)로 추가 분해해 BM25 토크나이저와 매치 선택 로직을 정합시키면, 한정자형 쿼리가 fallback(무스니펫)으로 추락하는 현상과 랭킹 누락이 동시에 사라진다.
- **근거**:
  - 전체 281건 search 중 58건(20.6%)이 구두점 포함 단어를 사용 — `::` 19회, `/` 18회, `.` 13회 (양 캠페인 합산).
  - iter2 `clickhouse-claude-sonnet-c8-r1`: 쿼리 `StorageFactory::get` → 파일은 1위로 랭크됐으나 fallback 목록만 렌더(`get (fn) [L67-247]` 한 줄, 스니펫·caller 없음) → read 추가 턴. 원인: `index.rs:922`의 `query_lower.split_whitespace()`가 `storagefactory::get`을 한 덩어리로 유지해 `term_hits_symbol_name`(index.rs:788)이 절대 발화하지 못함.
  - iter2 `ollama-claude-sonnet-o9-r1/r2`: `/api/chat route registration`, `POST api/chat handler` 모두 routes.go가 detail로 못 올라옴(아래 03과 복합).
  - iter2 `clickhouse-claude-sonnet-c6-r2`: `REGISTER_FUNCTION(Sleep)` → fallback.
  - **라이브 재현(HEAD 바이너리, 진행 중 캠페인 저장소)**: django에서 `QuerySet.__iter__ definition` → 정답 `django/db/models/query.py`가 상위 6위 밖(1위 termcolors.py), 공백 분리형 `QuerySet __iter__ definition` → query.py 1위. strapi에서 `strapi.documents getter` → 정답 `packages/core/core/src/Strapi.ts` 상위 5 밖, 분리형 → 1위. **현재 ds 캠페인의 d7/s6 과제 프롬프트가 정확히 이 표기(`QuerySet.__iter__`, `strapi.documents`)를 쓰므로 본실행에서 재현될 가능성이 높다.**
- **분류**: 랭킹·색인
- **예상 지표 효과**: claude arm의 first_answer_turn 단축과 turns 감소(한정자 쿼리 재시도 루프 제거), 스니펫 도달율 상승으로 후속 read 감소 → mcp_response_bytes_total 감소. d7/s6/d10류 정답률 방어.
- **구현 스케치·비용 (소)**: ① `index.rs:922` — 쿼리어를 원형 + 비영숫자 분해 서브토큰의 합집합으로 확장(리터럴 exact-value 매치를 위해 원형 유지). ② `index.rs:863` parse 직전, 따옴표 구간 밖의 구두점 결합어를 공백 분해해 tantivy의 인접 phrase 강제(랭킹 누락의 원인)를 회피. 재인덱스 불필요.
- **검증 방법**: 위 4개 글루드/분리형 쿼리 쌍을 fixture로 고정해 수정 전후 랭킹·스니펫 유무를 스냅샷 비교 → ds-iter2 재실행에서 d7/s6 에피소드의 turns·first_answer_turn 대조.

### 02. EXACT_NAME 부스트의 "흔한 이름" 게이트 — `Definition`/`constructor`류 글루 단어 오발사 차단

- **가설**: 정확명 부스트(×3.0)와 정확명 승격을 저장소 내 정의 빈도(또는 doc_freq)로 게이트하면, 글루 단어가 잡동사니 파일을 1위로 끌어올리는 역방향 랭킹이 사라진다.
- **근거**:
  - iter2 `ollama-claude-sonnet-o6-r1/r2`: 쿼리 `NewScheduler constructor function` → 응답 17,276B 전부가 코드젠 TS 파일의 `constructor` 스니펫(같은 이름 정의 28개)이고 정답(InitScheduler, server/sched.go)은 응답에 부재. `constructor`(11자)가 `is_discriminative_name`(index.rs:776-778, "8자 이상이면 식별력 있음")을 통과해 exact_hit(index.rs:949-950) → ×3.0 부스트(index.rs:971-973).
  - iter2 `clickhouse-claude-sonnet-c4-r2`: `zkutil::ZooKeeper class definition` → 1위가 무관한 `ASTCreateWasmFunctionQuery.h`(`Definition` struct 스니펫 렌더). iter1 c7-r2 분석("BM25 랭킹이 ASTCreateWasmFunctionQuery.h를 최상위에")과 동일 기전.
  - iter2 `ollama-claude-sonnet-o9-r1` 쿼리 3(`"api/chat" route register HandleFunc`): 상위 3개 파일이 전부 `Register` 함수(8자, 정의 8개) 스니펫 — 정답 routes.go는 tail 34위.
- **분류**: 랭킹·색인
- **예상 지표 효과**: 오도성 1위 제거로 first_answer_turn 단축, 무관 스니펫 17KB류 낭비 제거로 bytes 감소. "definition/constructor/register"는 claude 계열의 상용 쿼리 어휘라 ds 캠페인 전반에 적용.
- **구현 스케치·비용 (소)**: `index.rs:949`의 exact_hit 판정에 빈도 조건 추가 — `searcher.doc_freq(Term::from_field_text(symbol_field, term))`이 임계(예: 파일 10개) 초과면 부스트·승격 모두 비활성(매치 자체는 유지). 또는 caller 스캔이 이미 쓰는 이름→정의수 맵 재사용. 재인덱스 불필요.
- **검증 방법**: `NewScheduler constructor function`(ollama), `zkutil::ZooKeeper class definition`(ClickHouse) 두 쿼리의 상위 5 파일 스냅샷 비교 + 기존 회귀(정확명 과제 c1~c3, o1 등 80/80 유지) 확인.

### 03. Fallback 파일의 심볼 목록을 줄 순서가 아닌 쿼리어 겹침순으로 정렬·절단

- **가설**: fallback 렌더가 "줄 순서 상위 20개"(mcp.rs:647, config 기본 `search_detail_symbol_limit=20`) 대신 쿼리어 부분 겹침이 큰 심볼부터 보여주면, 정답 심볼이 절단선 아래로 숨는 일이 없어진다.
- **근거**: iter2 `ollama-claude-sonnet-o5-r1` 첫 search(`tensor promotion F16 F32 load op transform`) 응답에서 정답 파일 `llama-ollama-compat-util.cpp`가 3위로 랭크됐지만, fallback 목록이 L23~L224의 20개 심볼에서 끊기고 `_… 9 more symbols not shown_` — 정답 3심볼(`register_load_op` L263, `take_load_op` L268, `promote_tensor_to_f32` L312)이 정확히 그 숨겨진 9개 안에 있었다. 에이전트는 이후 overview+grep 3회+read 3회를 추가로 소모(에피소드 16턴, 73,724B). 동일 기전이 surrealdb 캠페인의 "L3049의 `put_tb`는 목록에 보이지도 않았다"(`benchmark-evolution.md` §1.1)에서 이미 한 번 관측됐고, 그때는 매치 승격으로 우회했지만 fallback 자체의 정렬은 그대로다(index.rs:986-988에서 `matched_symbols = all_symbols` 줄 순서 그대로).
- **분류**: 출력 포맷/UX
- **예상 지표 효과**: concept/flow 과제(ds의 d5·d9·s5·s7·s8)에서 후속 overview/read 턴 감소 → turns·bytes 감소. iter2 기준 detail 응답 103건 중 14건(14%)이 전면 fallback이었으므로 적용 빈도도 실재.
- **구현 스케치·비용 (소)**: `index.rs:986-989`에서 fallback 대입 전에 `all_symbols`를 심볼별 `symbol_matches_term` 겹침 수 내림차순(동률은 줄 순서)으로 정렬. 겹침 수>0인 심볼에 `(~2/7 terms)`류 표기를 붙이면 인용 신뢰도도 상승. 재인덱스 불필요.
- **검증 방법**: o5-r1의 4개 쿼리를 fixture로 — 수정 후 첫 응답의 fallback 목록 상단에 정답 3심볼이 노출되는지 확인 → ds-iter2에서 flow 과제 turns 중앙값 비교.

### 04. 긴 자연어 쿼리의 부분 일치 임계 상한 — `div_ceil(n/2)`를 3으로 캡

- **가설**: 부분 커버리지 승격 임계(index.rs:814-816, 954-956)가 쿼리 길이에 비례해 올라가 6단어 이상 자연어 쿼리에서 사실상 승격이 불가능해지므로, 임계를 `min(div_ceil(n,2), 3)`로 캡하면(이름 증거 게이트는 유지) 긴 개념 쿼리도 스니펫에 도달한다.
- **근거**: iter2 무스니펫 search 14건 중 9건이 5단어 이상 쿼리. `ollama-claude-sonnet-o3-r2`의 `promote f16 f32 load tensor conversion helper function`(8단어, 임계 4): 정답 심볼 `promote_tensor_to_f32`는 promote·tensor·f32 3개를 맞추고도 승격 탈락 → 3연속 search 실패 후 grep 4회. `o5-r1/r2`도 동일(6~8단어 쿼리 연쇄 fallback). 대조: codex가 같은 과제를 1콜에 푼 쿼리는 `LoadOp`라는 정확명을 포함했기 때문(`ollama-codex-gpt55-o5-r2` 첫 search 5,825B에 즉시 스니펫). 7단어+ 쿼리의 스니펫율은 claude 67%/codex 77%로 다른 구간(85~100%) 대비 명확히 낮다.
- **분류**: 랭킹·색인
- **예상 지표 효과**: 개념형 과제의 search 재시도 턴 감소. 03과 결합 시 o3/o5류가 "search 1~2콜 + read 1콜" 패턴으로 수렴 예상.
- **구현 스케치·비용 (소)**: `index.rs:814-816` 한 줄 수정. 과거 v5→v6에서 잡았던 "독스트링 스침 32KB 비대" 회귀는 이름 증거 게이트(954-956)가 그대로 막아주지만, 노이즈 상승 리스크가 있어 측정 동반 필수.
- **검증 방법**: o3/o5의 실패 쿼리 6개 fixture로 스니펫 도달 확인 + t6류 바이트 비대 회귀 감시(v6 게이트 테스트 유지) → ds-iter2 concept 과제 비교.

### 05. 동일 이름 다중 정의의 caller 주석 중복·모호 노이즈 압축

- **가설**: 한 응답 안에서 같은 스캔 이름의 caller 목록이 정의마다 동일하게 반복 렌더되는 것을 1회 렌더+참조(또는 모호 시 1줄 요약)로 바꾸면, 응답 바이트가 의미 손실 없이 크게 준다.
- **근거**: iter2 search 응답 103건(총 947KB) 정량 분석 — 62건에 모호 귀속 caller 목록 포함, **50건에서 같은 이름의 동일 caller 목록이 한 응답에 2회 이상 반복**, annotation성 줄이 전체 search 바이트의 약 17%. 대표: `ollama-claude-sonnet-o9-r1` 쿼리 3 응답에서 `Register` 정의 3개가 각각 동일한 `Server.chat (app/ui/ui.go:867…)` 5줄 + `_… 39 more not shown._`을 반복 — 게다가 그 귀속은 사실상 오류(다른 Register들의 호출처)다. 렌더 지점: `callers.rs:638-651`(모호 라벨 달린 목록), `config.rs:248`(`caller_list_cap=5`).
- **분류**: 출력 포맷/UX
- **예상 지표 효과**: mcp_response_bytes_total 5~10% 직접 감소(중복분), 오귀속을 따라가는 헛 read 감소. c9류 callers 과제의 가치(주석이 곧 정답, iter2 c9 2턴/4,397B)는 비모호 케이스라 영향 없음.
- **구현 스케치·비용 (소~중)**: `callers.rs:532` `render_symbol_annotation`에서 응답 단위로 스캔 이름별 렌더 캐시를 두고, 2회째부터는 `- _callers: see `Register` above_` 1줄. 추가로 `own_def_count >= common_name_threshold`(callers.rs:558)인 이름은 목록 대신 `N call sites in M files (attribution ambiguous; grep 'Register(' for exact list)` 1줄 요약 옵션 검토.
- **검증 방법**: o9-r1 쿼리 3 fixture의 응답 바이트 전후 비교(약 40줄 감소 예상) + c9 과제 회귀 확인 → ds-iter2 bytes 중앙값 대조.

### 06. grep에도 read와 같은 관용 파라미터 코어션 레이어 적용 (path·컨텍스트 별칭)

- **가설**: grep의 `path`에 `file_path`/`file`/`query` 별칭, `-C`/`-B`/`-A`에 `context_lines`/`C`/`B`/`A`/`n` 별칭을 수용하면, 별칭 무시로 인한 전역 grep·무컨텍스트 재시도가 사라진다.
- **근거**: 양 캠페인 grep 276건 파라미터 실측 — `file_path` 14, `file` 5, `query` 2, `paths` 1건이 조용히 무시되어 의도치 않은 저장소 전역 검색이 됐고, `context_lines` 9, `C` 6, `n` 16건도 무시. 현재 코드(grep.rs:127)는 `path` 단일 키만 읽고, 별칭은 glob에만 구현됨(grep.rs:131-133). read는 동일 문제를 별칭+코어션 계층(read.rs:42-91)으로 이미 해결 — C/C++ 캠페인(`benchmark-evolution.md` §2.3)이 "별칭 두더지잡기가 아니라 코어션 레이어가 올바른 설계"라고 결론낸 그 패턴의 미적용 잔여분이다.
- **분류**: 도구 설명(description)/입력 UX
- **예상 지표 효과**: 파일 한정 의도가 살아나 grep 응답 바이트 감소, 무시된 파라미터로 인한 재호출(duplicate_calls 일부) 감소. claude arm 전용 효과(codex는 스키마 준수율 100%).
- **구현 스케치·비용 (소)**: `grep.rs:127` path 해석을 read의 `resolve_file_path_arg` 패턴으로 교체, `grep.rs:146-151`에 컨텍스트 별칭 추가. 스키마 description에 별칭 명기(mcp.rs:494 부근).
- **검증 방법**: 실관측 별칭 조합 6종의 유닛 fixture → ds-iter2에서 grep 평균 응답 바이트와 grep 연쇄 재시도 횟수 비교.

### 07. 리터럴 exact-value 승격의 글루 단어 게이트

- **가설**: 단일 쿼리어가 리터럴 전체와 일치하면 무조건 표시하는 규칙(index.rs:1009)에 식별력 조건(숫자/기호 포함 또는 다단어)을 달면, 리터럴 줄의 78%를 차지하는 무가치 노이즈가 사라진다.
- **근거**: iter2 search 응답의 리터럴 줄 275건 중 215건(78%)이 8자 이하 단일 영단어. 실례: `ollama-claude-sonnet-o9-r1` 응답 1의 `- Literal: "route" [L78] [L113] [L120]`(inference_request_log.go — 과제와 무관), `ollama-codex-gpt55-o7-r2`의 `"port"`, `"default"` 3줄(정작 필요한 기본값은 스니펫 미달로 read 필요). 이 기능의 본래 표적("8000", 에러 메시지)은 게이트를 통과한다.
- **분류**: 출력 포맷/UX
- **예상 지표 효과**: bytes 소폭 감소, 리터럴 신호의 정밀도 상승(d2/s2류 literal 과제에서 표시되는 리터럴이 곧 정답이 되도록).
- **구현 스케치·비용 (소)**: `index.rs:996-1012`의 exact-value 분기(`t == lit_lower`)에 `리터럴에 숫자·비영문자가 있거나 다단어이거나 쿼리가 2어 이하` 조건 추가.
- **검증 방법**: o9-r1/o7-r2 fixture에서 글루 리터럴 소거 + d2(django "8000") 과제에서 `- Literal: "8000" [L32]`류가 유지되는지 회귀 확인.

### 08. 식별력 있는 숫자 리터럴의 선별 인덱싱

- **가설**: 3자리 이상 숫자 리터럴을 문자열 리터럴과 같은 최저 부스트로 인덱싱하면, 설정 기본값류 과제에서 숫자 앵커 쿼리가 함정 대신 정답 파일을 띄운다.
- **근거**: **진행 중 캠페인의 s2 과제 정답이 정확히 이 사각지대다** — 정답은 `packages/core/core/src/configuration/index.ts:23`의 `port: Number(process.env.PORT) || 1337`(숫자 리터럴, 미인덱스). HEAD 바이너리 라이브 검증: strapi 인덱스에 `search "1337"` → 상위 8개 전부가 과제 루브릭이 "함정"으로 지정한 테스트/URL 문자열 파일이고 정답 파일 부재. (현재는 `default server port 1337`처럼 운 좋게 `defaultServerConfig` 심볼 토큰을 스치면 5위로 걸리는 정도.) 문자열인 django d2("8000")는 1위로 잘 잡힌다 — 차이는 오직 따옴표 유무. 현재 파서는 `literal.string`만 수집(parser.rs:2109), 백로그 01이 숫자를 명시적으로 Out of Scope 처리했었다.
- **분류**: 랭킹·색인 (신규 능력에 가까움)
- **예상 지표 효과**: s2류 literal 과제의 first_answer_turn·정답률 직접 개선. ds-iter1 결과에서 s2 마찰이 실측되면 우선순위 상향.
- **구현 스케치·비용 (중)**: parser.rs 리터럴 캡처에 숫자 노드 추가(언어별 쿼리 보강) + 3자리 미만/0·1류 제외 필터, `EXTRACTION_FORMAT_VERSION` 범프(재인덱스 1회). index.rs:130 literal 필드 재사용이라 스키마 변경 없음.
- **검증 방법**: ds-iter1의 s2 에피소드 전사로 실패 양상 확인(채점 후) → 수정·재인덱스 후 `search "1337"` 1위 검증 + 인덱스 크기/색인 시간 증가율 측정(허용선 합의).

### 09. 인덱스 비호환·교체에 대한 런타임 자기복구와 에이전트 친화 오류

- **가설**: search 경로에서 지속성 인덱스 열기 오류(IncompatibleIndex 등)를 감지해 엔진 재생성으로 자기복구하고, 복구 불가 시 에이전트가 행동할 수 있는 오류 문구(grep/overview 우회 안내)를 주면, 장수 서버의 검색 전면 다운이 사라진다.
- **근거**: **라이브 dogfooding에서 연결된 5개 인스턴스 전부**의 search가 `MCP error -32603: Indexing error: Failed to open file for read: 'IncompatibleIndex(Library version: 6, index version: 7 …)'`로 영구 실패(상주 구버전 서버 밑에서 신버전 바이너리가 인덱스를 재생성한 시나리오 — 바이너리 업그레이드 시 실사용에서 반복될 패턴). grep/read/overview는 정상이지만 오류 문구는 Rust 개발자용 안내라 에이전트가 다음 행동을 정할 수 없다. 코드상 복구는 기동 시에만 존재(index.rs:162-186), 런타임 search 오류는 -32603로 그대로 매핑(mcp.rs:548-549)되고 자동 재생성은 "인덱서 스레드 사망" 시에만 발화(mcp.rs:257-264) — 리더 stale은 인덱서를 죽이지 않으므로 영원히 미발화.
- **분류**: 신규 능력(신뢰성)
- **예상 지표 효과**: 벤치 지표 직접 효과는 없음(에피소드는 매번 새 프로세스). 실사용 신뢰성 결함이며, dogfooding 채널 복구 자체가 향후 측정의 전제 조건.
- **구현 스케치·비용 (중)**: mcp.rs:548 search 오류에서 영구성 오류 패턴 감지 → `restart_indexer_if_dead`와 같은 엔진 재구축 경로 재사용(재시도 1회 + 백오프, 실패 시 "index unavailable; grep/overview still work" 문구). 다운그레이드(신 인덱스+구 바이너리)는 wipe 재생성으로 동일 처리.
- **검증 방법**: 통합 테스트 — 서버 기동 후 인덱스 디렉터리를 외부에서 교체/손상시키고 search 1회 실패 → 자동 복구 후 2회째 성공을 단정. 라이브 인스턴스 재기동 후 재발 모니터링.

### 10. (재평가) 다단계 흐름 추적 depth-2 — 01·03·04 적용 후 재측정으로 후순위 유지

- **가설(재평가)**: flow 과제의 잔여 비용은 callee 깊이 부족보다 "진입점 심볼이 detail로 노출되지 않는 것"이 지배적이므로, 위 01~04를 먼저 적용하면 depth-2 없이 상당분이 흡수된다.
- **근거**: iter2 `ollama-claude-sonnet-o9-r1`(15턴)의 턴 구성 — search 실패 3턴(01·02·03 표적) + routes.go 내부 grep 루프 6턴(진입점 줄을 못 받았기 때문) + read 2턴. `o5-r1`(16턴)도 fallback 절단(03)과 임계(04)가 원인의 대부분. 반면 체인 전개 자체가 병목인 턴은 소수였다. `design-callchain-tracing.md`의 (a)안(디스패치형 depth-2)은 유효하지만, 같은 과제군을 더 싸게 줄이는 선행 수단이 생겼으므로 ds-iter2에서 01~04 적용 후의 flow 과제(d6~d8, s6~s8 — 20문항 중 6개) 턴 분포를 본 뒤 결정하는 것이 측정 경제상 옳다.
- **분류**: 신규 능력
- **예상 지표 효과**: (선행 수정 후 잔여가 크면) flow 과제 turns 추가 감소.
- **구현 스케치·비용**: 설계서 (a)안 그대로(`callers.rs:407` discover_callees 조건부 재귀, 중) — 단 착수 조건부.
- **검증 방법**: ds-iter2에서 flow 과제 턴이 여전히 중앙값 2배 초과면 착수, 아니면 종결.

---

## 기존 백로그(agent-benchmark-followups.md) 대비 신규 / 재평가 요약

백로그 5항목 중 **01(리터럴 줄 번호), 02(Java/Kotlin enum variant), 05(find 글롭 설명)은 HEAD에 구현 완료**를 코드로 확인했고(parser.rs:46/2109의 `ExtractedLiteral`+mcp.rs:764의 `[L{n}]`, parser.rs:255·284의 `enum_constant`/`enum_entry`, mcp.rs:472의 basename 캐비엇), 03(타 언어 검증)은 진행 중인 django/strapi 캠페인 그 자체다. 04(callchain)는 이번 실측으로 "진입점 미노출이 선행 병목"이라는 근거가 추가되어 조건부 후순위로 재평가했다(후보 10). 이번 보고서의 **후보 01~09는 전부 백로그에 없던 신규 항목**이며, 그중 01(토크나이즈 정합)·02(EXACT 글루 게이트)·03(fallback 정렬)은 iter2가 "수정 완료"로 닫은 영역(랭킹·detail 렌더) 아래에서 전사 정밀 판독과 HEAD 라이브 재현으로 새로 적발한 결함이고, 07은 백로그 01이 구현되며 생긴 부작용(글루 리터럴 노이즈 78%)의 후속이며, 08은 백로그 01이 명시적으로 제외했던 숫자 리터럴을 진행 중 캠페인의 s2 정답이 정확히 요구한다는 실측으로 재개봉한 것이다. 06은 C/C++ 캠페인의 설계 교훈("코어션 레이어")을 grep에 마저 적용하는 잔여 작업, 09는 dogfooding에서만 드러난 운영 신뢰성 결함이다.
