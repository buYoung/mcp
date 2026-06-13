# Iteration 1 벤치마크 분석 보고서

**작성일**: 2026-06-12  
**분석 대상**: 4개 arm × 20개 에피소드 = 총 80개 에피소드

---

## 1. 집계 표

| Arm | n | correct | partial | wrong | n/a | harness_error | median_turns | median_duration_s | median_response_bytes | dup_calls |
|-----|---|---------|---------|-------|-----|---------------|--------------|-------------------|-----------------------|-----------|
| clickhouse/claude-sonnet | 20 | 18 | 2 | 0 | 0 | 1 | 7.0 | 47.0 | 42,884 | 3 |
| clickhouse/codex-gpt55 | 20 | 19 | 0 | 0 | 1 | 1 | 4.0 | 28.0 | 13,094 | 0 |
| ollama/claude-sonnet | 20 | 20 | 0 | 0 | 0 | 0 | 9.5 | 69.5 | 33,130 | 5 |
| ollama/codex-gpt55 | 20 | 20 | 0 | 0 | 0 | 0 | 4.0 | 41.5 | 22,482 | 0 |

---

## 2. Arm 간 비교

### 2.1 정확도

- **ollama arm**: 양쪽 모델 모두 100% 정답.
- **clickhouse/claude-sonnet**: 2건 partial (c7-r2, c8-r1), 1건 harness_error (c7-r2). 실질 정답률 90%.
- **clickhouse/codex-gpt55**: 1건 n/a (c3-r1, API 타임아웃). 나머지 19건 correct. 실질 정답률 95%.

### 2.2 효율성

- codex-gpt55는 두 repo 모두에서 claude-sonnet 대비 턴 수 약 50%, 응답 바이트 30~57% 수준.
- claude-sonnet은 grep 기본 모드 문제로 파일 목록만 받고 재시도하는 루프가 반복되어 불필요한 컨텍스트 소비.

### 2.3 first_answer_turn1_rate

- ollama/codex-gpt55: 1.0 (20/20 에피소드가 1턴에 답변)
- ollama/claude-sonnet: 0.8
- clickhouse/codex-gpt55: 0.65
- clickhouse/claude-sonnet: 0.6 (최저)

---

## 3. 실패 에피소드 상세 분석

### 3.1 clickhouse-claude-sonnet-c7-r2 (partial + context_limit)

**태스크**: INFINITE_LOOP 에러 코드 정의 위치와 모든 throw 지점(7곳) 조회.

**경위**:
1. grep(pattern="INFINITE_LOOP") 9회 호출 — 모두 output_mode 미지정으로 기본값 files_with_matches 적용. 줄 번호 없이 파일 목록만 반환.
2. 줄 번호를 못 얻자 search를 25회 반복. BM25 랭킹이 ASTCreateWasmFunctionQuery.h 등 관련 없는 파일을 최상위에 올림.
3. read(file_path="DatabaseCatalog.cpp", start_line=…) 호출 시 start_line/end_line이 offset/limit으로 인식되지 않아 전체 파일(109,553 bytes) 읽기 시도 → -32602 오류 19회.
4. 50턴 한계 도달 후 "Prompt is too long"으로 최종 답변 미완성.

**비교 (c7-r1, correct)**: grep(output_mode="content", glob="*.h,*.cpp", -n=true) 1회 호출로 모든 줄 번호 즉시 획득 → 2턴에 완료.

### 3.2 clickhouse-claude-sonnet-c8-r1 (partial)

**태스크**: Memory 스토리지 엔진 등록·생성 흐름 4개 지점 조회 ((1)~(3)은 정상, (4) StorageFactory::get 구현 StorageFactory.cpp:67 미인용).

**경위**:
- search("StorageFactory::get definition") 등 10회 시도했으나 L67의 get 함수 스니펫 대신 L249의 instance() 함수나 .h 파일 선언만 반환.
- read(path="src/Storages/registerStorages.cpp", …) 호출 시 path 키가 file_path로 인식되지 않아 -32602 오류 2회.
- 최종 답변에서 StorageFactory.h:94(선언)만 인용, StorageFactory.cpp:67 줄 번호 미제시.

**비교 (c8-r2, correct)**: 같은 read 오류 2회 발생했으나 이후 find + read(file_path="StorageFactory.cpp") 직접 읽기로 L67 발견.

### 3.3 clickhouse-codex-gpt55-c3-r1 (n/a, timeout)

gpt-5.5 API 무응답으로 타임아웃(1558초). c3-r2는 정상 correct. API 가용성 간헐적 장애로 추정.

---

## 4. 교차 에피소드 패턴

### 4.1 grep 기본 output_mode 문제 (116/122건 = 95.1%)

전체 grep 호출 122건 중 116건이 output_mode 미지정 → files_with_matches(파일 목록만 반환). 에이전트들이 include_line_numbers, with_line_numbers 같은 존재하지 않는 파라미터를 시도하는 패턴도 다수 관찰됨.

### 4.2 read 파라미터 불일치 (28개 에피소드, 총 101건)

- path/file 대신 file_path 사용 실패: 52건 (-32602 오류)
- start_line/end_line 대신 offset/limit 사용: 48건 (오류 없이 무시되지만 전체 파일 읽기 야기, 19건 output too large)
- claude-sonnet 에이전트가 Claude Code 내장 Read 도구의 파라미터 컨벤션(path, start_line, end_line)을 그대로 사용하는 습관.
- codex-gpt55는 이 오류 0건.

### 4.3 StorageFactory::get BM25 미노출 (c8)

get 함수(L67)는 다양한 쿼리에서 인라인 스니펫으로 반환되지 않음. instance()(L249)는 반환됨. get처럼 짧고 범용적인 함수명은 BM25 IDF가 낮아 순위 하락하는 것으로 추정.

---

## 5. 결함 목록 및 수정 가설

### 결함 1: grep 기본 output_mode = files_with_matches [주요]

- 증거: 116/122 grep 호출이 파일 목록만 반환. c7-r2 context_limit의 직접 원인.
- 수정: apps/codemap-search/src/tools/grep.rs L132-133: 기본값을 "content"로 변경.

### 결함 2: read 파라미터명 불일치 (file_path/offset/limit) [주요]

- 증거: 28개 에피소드 52건 -32602 오류. c8-r1 partial의 기여 원인.
- 수정: apps/codemap-search/src/tools/read.rs L39: path/file을 file_path 별칭으로 수용. start_line/end_line을 offset/limit 별칭으로 변환.

### 결함 3: StorageFactory::get BM25 랭킹 실패 [경미]

- 증거: c8-r1에서 10회 search 모두 L67 미반환.
- 수정: apps/codemap-search/src/index.rs, parser.rs — 짧은 함수명에 클래스명 접두어 조합 토큰 추가 인덱싱.

### 결함 4: codex-gpt55 API 가용성 불안정 [경미]

- 증거: c3-r1 timeout(1558s), turns=0, 빈 JSONL.
- 수정: 하니스에 API 헬스체크 및 exponential backoff 재시도 로직 추가.

---

## 6. 재루프 판단

reloop_needed = true. 이유: 100% 미만 정답률(clickhouse arm) + (a)류 제품 결함 2건(grep 기본 output_mode, read 파라미터 불일치)이 명확히 확인됨. 두 결함 수정 후 재실행 시 c7-r2와 c8-r1이 correct로 전환될 가능성 높음.
