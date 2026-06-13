# [feat] codemap-search 에이전트 벤치마크 후속 개선

## Work Type
feat

## 배경
2026-06 codex(gpt-5.5) 벤치마크 4회 반복(v3→v7)으로 search detail의 스니펫·caller
주석 활성화(4%→75~87%), 정답 첫 응답 포함 12/12, 중앙값 턴 수 v3 동등(9.5)을
달성했다. 수정 인과와 측정 경위는 같은 폴더의 `benchmark-evolution.md`(캠페인 1
절) 참조.
이 문서는 그 측정에서 식별된 잔여 개선 항목을 우선순위 순으로 정의하는 백로그다.

---

## 01. 리터럴 줄 번호 (우선 진행)

### Current State (As-Is)
- 리터럴은 텍스트만 저장된다: `ExtractedFile.literals: Vec<String>`
  (`apps/codemap-search/src/parser.rs:51`). tree-sitter 캡처 시점에 노드 위치를
  버린다 (`parser.rs` extract의 `literal.string` 분기 — `literals.push(stripped)`).
- search detail은 `- Literal: "…"` 형태로 줄 번호 없이 렌더링한다
  (`apps/codemap-search/src/mcp.rs:762` 부근, `truncate_literal` 경유).
- 벤치마크 증거: t5(기본 포트)·t6(에러 메시지)류에서 리터럴 히트가 정답 신호인데
  줄 번호가 없어 에이전트가 위치 확정을 위해 read/grep 턴을 추가로 쓴다. v7에서
  스니펫 줄 번호가 확인성 read를 줄인 것과 같은 메커니즘이 리터럴에는 빠져 있다.

### Desired Outcome (To-Be)
- 리터럴이 1-based 시작 줄 번호를 갖고, detail 뷰가 `- Literal: "…" [L140]`처럼
  렌더링한다. 줄 번호는 read와 일치하는 정확 값(인용 가능).
- 인덱싱(BM25 `literal` 필드)과 매치 선택(`matched_literals`의 완화 규칙: 전단어 /
  정확값 / 3단어+ 절반 커버리지)은 의미 변화 없음 — 표시만 풍부해진다.

### Scope
#### In Scope
- `ExtractedSymbol`처럼 줄 번호를 담는 리터럴 구조(예: `ExtractedLiteral { text, line }`)
  도입, 직렬화 형태 변경에 따라 `EXTRACTION_FORMAT_VERSION` 범프(예: `v4-literal-line`).
- 소비처 일괄 갱신: `parser.rs`(추출), `index.rs`(L516 인덱싱 — text만 사용,
  `SearchResult.matched_literals` 타입), `mcp.rs`(렌더), `benchmark.rs:144`,
  `codemap.rs`(detail 뷰가 리터럴을 다루는 경로), `callers.rs` 테스트 픽스처.
- 기존 테스트 유지보수 (예: `test_raw_string_literal_quote_stripping`의 단정 형태).
#### Out of Scope
- 리터럴 종료 줄/범위(멀티라인 리터럴의 end_line) — 시작 줄로 충분, 필요 시 후속.
- 숫자/불리언 리터럴 인덱싱(여전히 문자열만).

---

## 02. Java/Kotlin enum 상수 추출

### Current State (As-Is)
- variant 추출은 Rust만 구현 (`parser.rs` RUST_QUERY_STR의 `enum_variant` 캡처,
  kind `variant`, owner = 소속 enum). t6류("에러/상태 이름으로 정의 찾기")는 Go는
  `var_spec`, Python은 class-attr assignment로 이미 커버되지만 Java(`enum_constant`),
  Kotlin(`enum_entry`)은 심볼이 추출되지 않는다.

### Desired Outcome (To-Be)
- Java `enum_constant`, Kotlin `enum_entry`가 Rust와 동일하게 kind `variant`로
  추출되고 owner(소속 enum/class)가 달린다. 같은 개념에 같은 어휘(`variant`) 유지.

### Scope
#### In Scope
- JAVA_QUERY_STR / KOTLIN_QUERY_STR에 캡처 추가, kind 매핑(`symbol.variant` 재사용),
  owner 해석 확인 (Java는 `enum_declaration`이 이미 type-container,
  `enum_body`/`enum_body_declarations` passthrough; Kotlin은 `enum_class_body`
  passthrough 기존재 — 부족분만 보강). `EXTRACTION_FORMAT_VERSION` 범프(01과 합쳐
  1회로).
#### Out of Scope
- TS enum 멤버(드묾; 관용적 variant는 문자열 유니언 타입 → 리터럴 인덱싱이 커버).

---

## 03. 타 언어 저장소 벤치마크 검증

### Current State (As-Is)
- 측정은 Rust(surrealdb)에서만 수행. 개선 대부분은 언어 무관 계층이지만 검증이 없다.
  caller 스캔(`name(` 텍스트 매치)은 Python/JS 동적 디스패치, Go/Java 인터페이스
  디스패치에서 정밀도가 낮다(라벨로 정직하게 표시 중이나 실효성 미측정).

### Desired Outcome (To-Be)
- Python(예: scrapy) 또는 TS(예: vscode 일부) 저장소에서 동일 방법론(6태스크 ×
  2rep, codex gpt-5.5 medium, 순차 실행)으로 측정해 언어별 효과/한계를 수치화.
  측정 절차는 `benchmark-workflow.md`와 playbook을 그대로 재사용하고, 태스크
  프롬프트는 파일로 고정한다.

---

## 04. t4류 다단계 흐름 추적 (별도 설계 필요)

### Current State (As-Is)
- "RPC 분기 → 실행" 같은 4단계 체인 추적은 전 회차에서 25~33턴. depth-1 callee
  주석으로는 체인이 접히지 않고, 에이전트가 단계마다 read로 따라간다.

### Desired Outcome (To-Be)
- 방향 후보: (a) 디스패치형 함수에 한해 depth-2 callee 확장, (b) 호출 체인 요약
  응답(시작 심볼 → 끝 심볼 경로), (c) 현행 유지(복잡 과제 예외로 인정). 비용·노이즈
  통제가 핵심 쟁점 — 상세 분석과 권장안은 같은 폴더의 `design-callchain-tracing.md` 참조.

---

## 05. 소소한 부채

- `find`의 슬래시 없는 글롭이 basename 매치라는 점을 에이전트가 오해(`*rpc*` 무결과,
  v4 데이터 6회 중 3회 빈약) — 도구 설명 보강 또는 경로 매치 폴백 검토.
- `docs/configuration.md`에 구형 키(`search_detail_*`, `grep_max_columns` 등) 미기재.

## Open Questions
- 01: 동일 텍스트 리터럴이 여러 줄에 반복될 때 모두 별도 항목으로 둘지(현행 push
  방식 유지 = 모두 유지, 표시는 `search_literal_limit` 캡) — 기본은 유지.
- 02와 01의 포맷 버전 범프는 한 번에 합쳐 재인덱스를 1회로 만든다.
