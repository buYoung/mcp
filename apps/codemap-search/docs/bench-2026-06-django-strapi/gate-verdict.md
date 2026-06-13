# ds-iter1 재채점 게이트 판정 (fable, 2026-06-12)

playbook §5 표본 재채점. 1차 채점(sonnet ×8 배치, 전건 correct)에 대한 검증.

## 구성

- 표본: 어려운 과제 8종(d6~d9, s7~s10) + 중간 2종(d10, s5) × 4에피소드 = 40건 (요구 ≥10 초과)
- 절차: sonnet ×10이 인용 줄 전수 원본 대조 + rubric 체크리스트 기계 수집(`evidence.json`),
  fable이 플래그 판정. 별도로 fable이 d7·s9·s10 12건을 sed 원본 대조로 독립 검증.

## 결과: 40/40 correct 유지, overturn 0

인용 대조 총 ~300건 중 불일치 플래그 5건 — 전부 채점 영향 없음으로 판정:

| 에피소드 | 플래그 | 판정 사유 |
|---|---|---|
| django-claude-sonnet-d10-r1 | `query_utils.py:41` 내용 불일치 | 답안 코드블록의 `class Q:` 축약 표기 vs 원본 `class Q(tree.Node):` — 위치 주장은 정확, rubric 요소(41±2 + 재export 61) 충족 |
| django-claude-sonnet-d10-r2 | 동일 | `class Q(...):` 플레이스홀더 표기 — 동일 사유 |
| strapi-claude-sonnet-s5-r1 | `cron.ts:30-32` 키 이름 의역 | 보조 인용(Tasks 타입)의 `taskExpression` vs 원본 `key` — rubric 비요소, add 본문(39~76) 인용 충족 |
| strapi-claude-sonnet-s7-r2 | `entries.ts:137` ±1 | 보조 인용이며 137도 rubric 허용 범위(128~139) 내. 3단계(158→128→events.ts:23) 전부 정확 |
| strapi-claude-sonnet-s8-r1 | 스니펫 줄 라벨 4건 ±1 | contestant의 마크다운 스니펫 전사 오류. 핵심 3단계 인용(13–33, 56–114, 80–102)은 정확. 도구 반환 줄 번호는 정확했음 (제품 결함 아님) |

## 관찰 (회고 반영용)

- claude-sonnet arm 답안의 스니펫 줄 라벨 ±1 전사 오류가 2과제에서 관찰됨 — 도구 출력은
  정확하므로 모델 수준 표기 습관. 채점 영향 0이나, 줄 라벨 정확성을 따지는 rubric을
  설계할 경우 변별 요소가 될 수 있음.
