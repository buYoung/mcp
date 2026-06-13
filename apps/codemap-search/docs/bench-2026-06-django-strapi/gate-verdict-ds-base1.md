# ds-base1 재채점 게이트 판정 (fable, 2026-06-12)

playbook §5 표본 재채점. 1차 채점(sonnet ×8 배치, 전건 correct)에 대한 검증.
비-correct 에피소드는 0건이므로 표본 재채점만 해당.

## 구성

- 표본: ds-iter1 재채점과 동일 — 어려운 과제 8종(d6~d9, s7~s10) + 중간 2종(d10, s5)
  × 4에피소드(claude-sonnet-base/codex-gpt55-base × r1/r2) = 40건 (요구 ≥10 초과)
- 절차: sonnet ×10이 인용 줄 전수 원본 대조 + rubric 체크리스트 기계 수집(`evidence.json`),
  fable이 플래그 판정. 별도로 fable이 s7(codex-r1)·s8(claude-r1, codex-r2) 3건을
  sed 원본 대조로 독립 검증.

## 결과: 40/40 correct 유지, overturn 0

- rubric 요건 미충족: **0건** (40 에피소드 × 전 체크리스트 항목 satisfied=true)
- 인용 대조 불일치(match=false): 3 에피소드 12건 — 전부 채점 영향 없음 판정:

| 에피소드 | 플래그 | 판정 사유 |
|---|---|---|
| strapi-codex-gpt55-base-s7-r1 | `repository.ts:599` 호출 지점 ±1 | 호출 표현식(`async.map(...)`)이 599에서 시작, `entries.publish` 리터럴은 600. 보조 인용이며 rubric 3요소(541/128/23)는 전부 정확 — fable이 sed로 직접 확인 |
| strapi-claude-sonnet-base-s8-r1 | 스니펫 줄 라벨 8건 ±1~3 | 코드블록 내부 라벨이 빈 줄 생략으로 드리프트. rubric 요소(provider.ts:13, register.ts:56, uploadStream:80/upload:103)는 전부 정확. ds-iter1 s8-r1 동일 패턴의 비영향 판정과 일치 |
| strapi-codex-gpt55-base-s8-r2 | 범위 인용 3건 (348, 87-100, 110-121) | upload.ts:348은 실제 checkFileSize 호출로 서술과 정합(수집기 과민 플래그). 87-100/110-121은 rubric 허용 본문 범위(80~102/103~123) 내 보조 범위 서술. rubric 3요소 전부 정확 |

- 기타 플래그 28에피소드: ① codex arm의 절대 경로 마크다운 링크 표기(상대 경로 정보
  병기, 위치 정확 — 형식 문제), ② rubric이 명시적으로 허용한 추가 언급/별도 줄 인용
  (d8 931/933, s10 129/130, 호출부 추가 언급), ③ 설명 텍스트 의역(d8-r1
  `fetch_mode.fetch`, s10-r1 toLower 생략) — 전부 rubric 비요소. 채점 영향 없음.

## 토큰 필드 (이번 회차 신규)

- 80/80 에피소드 tokens 비-null, arm별 요구 키 전부 존재 (verify 단계에서 전수 확인).

## 결론

1차 채점 80/80 correct **유지**. baseline 2-arm도 과제 20종 전부에서 정답 도달 —
정답률 축에서는 MCP arm(ds-iter1 80/80)과 변별 없음. 변별은 효율 지표(턴·duration·
바이트·토큰)에서 비교할 것 (집계 단계).
