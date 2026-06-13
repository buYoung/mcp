# codemap-search 벤치마크 Iteration 2 — 최종 분석 보고서

> 작성일: 2026-06-12  
> 측정 대상: iteration 2 (수정 후)  
> 기준선: iteration 1 (수정 전)

---

## 1. 개요

iter1 대비 3건의 수정(grep 기본 output_mode → content, read 별칭 확장, owner 토큰 재인덱싱)을 적용한 뒤, 4개 arm 각 20 에피소드(총 80)를 실행한 결과를 분석한다.

**핵심 결과:** 모든 arm에서 정확도 100%를 달성했으며, 중앙값 턴·소요 시간·응답 바이트가 대폭 감소했다.

---

## 2. iter1 ↔ iter2 비교표

| arm | correct (1→2) | median turns (1→2) | median dur (1→2) | median bytes (1→2) | fa1_rate (1→2) | dup (1→2) |
|-----|:---:|:---:|:---:|:---:|:---:|:---:|
| clickhouse/claude-sonnet | 18→**20** (+2) | 7.0→**4.0** (-43%) | 47→**37**s (-21%) | 42,884→**18,197** (-58%) | 0.60→0.55 (-8pp) | 3→**0** |
| clickhouse/codex-gpt55  | 19→**20** (+1) | 4.0→**3.5** (-13%) | 28→31s (+11%) | 13,094→13,143 (+0%) | 0.65→**0.85** (+31pp) | 0→0 |
| ollama/claude-sonnet     | 20→20 (=) | 9.5→**4.5** (-53%) | 69.5→**38.5**s (-45%) | 33,130→**16,326** (-51%) | 0.80→0.40 (-40pp) | 5→**1** |
| ollama/codex-gpt55       | 20→20 (=) | 4.0→5.0 (+25%) | 41.5→**33.5**s (-19%) | 22,482→**16,591** (-26%) | 1.00→0.90 (-10pp) | 0→0 |

---

## 3. 수정 효과 검증

### 3.1 grep 기본 output_mode → content

**검증 결과: 유효**

- iter2에서 output_mode 명시 없이 호출된 grep 38건 전부 줄번호 포함(`:123:` 패턴) 결과를 반환했다.
- iter1에서 줄번호 없는 grep 결과로 StorageBuffer.cpp:738 미발견(c7-r2 partial 원인)이 iter2에서 3턴, grep 2회로 7개 throw 지점을 완전 열거하며 correct로 전환됐다.
- grep 응답 평균 바이트: iter1 4,095 → iter2 1,690으로 감소(컨텍스트 효율 향상).
- 단, 3건은 여전히 `"output_mode":"files_with_matches"`를 명시 사용했으며, 이는 모델 선택이므로 결함 아님.

### 3.2 read 별칭 수용 (path/file/start_line/end_line)

**검증 결과: 부분 유효** — 핵심 케이스(owner 토큰)는 해결됐으나 잔존 결함 있음.

- iter1에서 -32602 오류 52건 → iter2 5건으로 90% 감소.
- 남은 5건은 모두 `{"query": "src/..."}` 패턴(query 별칭이 여전히 미지원).
  - 발생 에피소드: c8-r1(2건), c10-r1(1건), c10-r2(1건), c3-r2(1건).
  - 전부 재시도 성공, 최종 결과 영향 없음.
- start_line/end_line: file_path 또는 path 별칭과 함께 사용한 22건 모두 줄 범위 올바르게 적용됐다.
- **잔존 결함**: `file` 별칭 + start_line/end_line 동시 사용 시 줄 범위 무시됨(→ 파일 전체 반환, 102400 초과 오류 유발). c7-r1에서 6건 중 4건이 이 방식으로 102400 초과, 86,324 bytes 소모.

### 3.3 owner 토큰 인덱싱 (StorageFactory get)

**검증 결과: 유효**

- iter1 c8-r1: search 10회, 19턴, 95,057 bytes, **partial** (StorageFactory.cpp:67 줄번호 미인용)
- iter2 c8-r1: search 2회, 6턴, 28,722 bytes, **correct** (4지점 모두 ±1줄 정확 제시)
- iter2 c8-r2: search 2회, 4턴, 36,418 bytes, **correct**
- 재인덱싱으로 `StorageFactory::get`을 2회 검색으로 즉시 발견, 실질적 중복 없음.

---

## 4. arm 간 비교

| arm | 강점 | 약점 |
|-----|------|------|
| clickhouse/claude-sonnet | 정확도 완전 달성, 중복 호출 0 | fa1_rate 0.55로 낮음 (도구 호출 후 별도 턴에서 답변) |
| clickhouse/codex-gpt55 | fa1_rate 0.85 최고, 중앙값 턴 3.5 최소 | 소요 시간이 iter2에서 소폭 증가 |
| ollama/claude-sonnet | 중앙값 턴 53% 감소(9.5→4.5) | fa1_rate 0.40으로 가장 낮음, dup 1건 잔존 |
| ollama/codex-gpt55 | 소요 시간 19% 단축, 정확도 100% | 중앙값 턴 소폭 증가(4.0→5.0) |

---

## 5. 과제 유형별 패턴

### grep 활용 과제 (c7: INFINITE_LOOP 정의 + throw 지점)
- iter1 c7-r2: 50턴 한계 도달, context_limit harness_error, partial.
- iter2 c7-r1/r2: 3~8턴, grep 1~2회로 전 지점 발견, correct.
- **grep content 기본화가 가장 직접적인 성능 개선 요인.**

### 복합 추적 과제 (c8: StorageFactory get, o9: ChatHandler 흐름)
- owner 토큰 인덱싱으로 c8이 search 10→2회, 19→4~6턴으로 대폭 단축.
- o9-r1(claude-sonnet): overview에서 routes.go 전체 읽기 시도 → 102400 초과 오류 2회 → grep으로 재시도, 15턴 소모.

### 심볼 정의 위치 과제 (c1~c6, o1~o8)
- 단순 심볼 검색은 1~3턴으로 해결.
- 복잡한 다단계 흐름(o9) 또는 대형 파일(o5: llama-compat 3000+줄)에서 read 크기 초과 오류 발생.

---

## 6. fa1_rate 변화 해석

claude-sonnet의 fa1_rate 감소(0.80→0.40 for ollama)는 제품 결함이 아닌 **모델 행동 변화**다.

- iter1: "먼저 도구를 호출하겠습니다"라는 텍스트를 도구 호출 이전 같은 턴에 출력 → fa1_rate 높음.
- iter2: 도구 호출을 먼저 수행하고, 결과를 받은 후 답변 텍스트 출력 → fa1_rate 낮음.
- 실제 턴 수는 감소(9.5→4.5), 불필요한 예비 텍스트가 없어져 효율 개선.

codex-gpt55의 ollama fa1_rate 감소(1.00→0.90)도 동일 패턴.

---

## 7. 남은 제품 결함

### 결함 1 — read 'query' 파라미터 별칭 미지원 (minor)

- **증거**: iter2에서 `{"query": "src/Storages/StorageFactory.cpp", "start_line": "67", ...}` → `MCP error -32602: Missing required 'file_path' parameter (aliases: 'path', 'file')` 5건 (c8-r1 ×2, c10-r1, c10-r2, c3-r2).
- **분류**: 제품 결함 — iter1 수정에서 path/file 별칭만 추가, query 누락.
- **수정 방향**: read 도구 별칭 목록에 `query` 추가.
- **현재 영향**: 전부 재시도로 복구, 결과 영향 없음.

### 결함 2 — file 별칭 + start_line/end_line 동시 사용 시 줄 범위 무시 (major)

- **증거**: `{"file": "src/Common/ErrorCodes.cpp", "start_line": "228", "end_line": "232"}` 호출 → 파일 1→부터 전체 반환. c7-r1에서 6건 중 4건이 102400 바이트 초과 오류 유발(StorageDistributed.cpp 109,553 bytes, StorageBuffer.cpp 112,191 bytes), 86,324 bytes 총 소모.
- **분류**: 제품 결함 — file 별칭이 file_path로만 매핑되고, 함께 전달된 start_line/end_line이 offset/limit으로 변환되지 않음.
- **수정 방향**: read 별칭 처리 로직에서 `file` → `file_path` 매핑 시 `start_line` → `offset`, `end_line` → `limit` 변환도 함께 수행.
- **현재 영향**: 불필요한 대량 데이터 반환, 102400 초과 오류 유발, 추가 재시도 필요.

### 결함 3 — start/end 숫자형 별칭 미지원 (minor)

- **증거**: `{"path": "llama/compat/llama-ollama-compat.cpp", "start": "3330", "end": "3408"}` → 파일 전체(196,826 bytes) 반환 → 102400 초과 오류. o5-r1에서 3회 반복.
- **분류**: 제품 결함 — start/end가 offset/limit 별칭으로 미처리.
- **수정 방향**: start → offset, end → (end - start + 1) → limit 변환 추가.
- **현재 영향**: 재시도로 복구, 결과 영향 없음.

### 결함 4 — overview에서 대형 파일 102400 bytes 초과 오류 (minor)

- **증거**: o9-r1 notes: "overview 시 read output exceeds 102400 bytes 오류 2회 발생(server/routes.go 등)". o5-r1에서도 동일.
- **분류**: 제품 결함 — overview가 대형 파일 개요 반환 시 100KB 한도 초과.
- **수정 방향**: overview 결과에 심볼 목록만 반환하고 원시 코드 반환을 제한하거나, 한도를 상향하거나, 자동 페이지네이션 지원.
- **현재 영향**: grep 재시도로 복구, 결과 영향 없음.

---

## 8. 권장사항

1. **우선순위 1 (major)**: read 도구에서 `file` 별칭 + `start_line`/`end_line` 동시 처리 — 줄 범위 변환 로직 추가. [결함 2]
2. **우선순위 2 (minor)**: read 별칭에 `query` 및 `start`/`end` 추가. [결함 1, 3]
3. **우선순위 3 (minor)**: overview 도구 대형 파일 처리 개선 (심볼 목록 전용 응답 또는 페이지네이션). [결함 4]

---

## 9. 결론

iter2는 iter1 대비 세 가지 수정 모두 유의미한 성능 개선을 가져왔다. 정확도는 4개 arm 모두 100%로 향상됐으며, 중앙값 턴은 최대 53%(ollama/claude-sonnet), 소요 시간은 최대 45%, 응답 바이트는 최대 58% 감소했다. 중복 호출도 8건(iter1 합계)에서 1건으로 줄었다.

남은 결함 4건은 모두 재시도로 복구 가능한 수준이며 최종 정확도에 영향을 주지 않았다. 그러나 결함 2(file 별칭 + start_line 범위 무시)는 대형 파일에서 불필요한 102400 초과 오류를 유발하고 턴 수를 늘리는 효율 문제이므로 우선 수정을 권장한다.
