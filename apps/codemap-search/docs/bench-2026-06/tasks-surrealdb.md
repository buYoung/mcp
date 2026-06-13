# 캠페인 1 데이터셋 — surrealdb 고정 과제 (v3→v7)

캠페인 1(surrealdb 단일 arm, codex gpt-5.5) 측정에 사용된 과제 정의와 ground
truth. 인과 서사는 `../benchmark-evolution.md` 캠페인 1 절, 수치 상세는
`../benchmark-workflow.md` §8 캠페인 1 절 참조.

- 대상 스냅샷: /tmp/surrealdb-main (Rust, 약 2,700 파일, 2026-06 스냅샷 —
  커밋 SHA 미기록, `../benchmark-workflow.md` §8 한계 7 참조. 줄 단위 ground truth는
  이 스냅샷에만 결속된다)
- 매트릭스: 6과제 × 2rep = 12 에피소드/회차, 측정→수정 5회차(v4 1회 무효)
- 프롬프트 고정 프레임: 모두 "codemap-search MCP 도구를 사용해서 …. 파일은
  수정하지 마." 형식 — 도구 유도 문구가 빠지면 측정이 오염된다(v4 사고).
  캠페인 2부터의 형식("정확한 파일 경로와 줄 번호를 인용해서 답해" 포함)과
  다르다. 기계 판정 rubric·함정 보기는 캠페인 2(`tasks-*.json`)부터 도입됐고
  이 세트에는 없다.

| ID | 과제 | 정답 |
|---|---|---|
| t1 | `Datastore` 구조체 정의와 주 공개 생성자 위치 | core/src/kvs/ds.rs (struct ~L204, new ~L934) |
| t2 | `Transaction`의 `put_tb` 정의 + 호출부 3곳 이상 | core/src/kvs/tx.rs:3049 + 호출부 |
| t3 | full-text search BM25 점수 계산 구조체·함수 | core/src/idx/ft/fulltext.rs (Scorer ~L1099) |
| t4 | RPC `query` 메서드 분기 → 실제 질의 실행 흐름 추적 | core/src/rpc/request.rs(~L87) 포함 체인 |
| t5 | 서버 기본 네트워크 포트 정의 위치 | server/src/cli/start.rs:140 (`default_value = "127.0.0.1:8000"`) |
| t6 | 읽기 전용 트랜잭션 쓰기 오류 정의 + 발생 지점 | core/src/kvs/err.rs:57 (TransactionReadonly) |
