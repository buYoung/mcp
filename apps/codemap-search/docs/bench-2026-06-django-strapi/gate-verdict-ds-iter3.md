# ds-iter3 재채점 게이트 판정 (fable, 2026-06-13)

playbook §5 표본 재채점 + 비-correct 전건 검증. ds-iter3는 루프 2(Tier-2 시그니처
축약 + read 별칭 정규화 + 앵커 캡) 적용 후 본실행 — 정답률 회귀 게이트 겸용.

## 구성

- 표본: 직전 회차들과 동일 10과제 × 4에피소드 = 40건. 1차 채점 비-correct
  2건(전부 표본 내 — s9-r2, s10-r1 claude)은 answer_text 전문 포함 정밀 수집.
- 절차: sonnet ×10 수집 + fable 판정. 비-correct 2건은 fable이 answer_text
  전문·rubric 원문·sed 원본 대조로 직접 판정.

## 결과: **overturn 2건 (partial→correct) — 최종 80/80 correct**

| 에피소드 | 1차 채점 | 판정 | 사유 |
|---|---|---|---|
| strapi-claude-sonnet-s9-r2 | partial ("(c) 3553 누락") | **correct** | **1차 채점 오류** — answer_text 섹션 3에 "L3553 — export const discardDocumentDrafts" + 코드 스니펫(3553~3556) 명시. 수집기 인용 대조 전부 일치, rubric (a)(b)(c) 전부 충족 |
| strapi-claude-sonnet-s10-r1 | partial ("user.js:130 경유 언급, 호출부 오분류") | **correct** | 결론 문장이 "bcrypt.compare 직접 호출은 auth.ts:23, user.js:130, documentation.ts:163 세 곳"으로 rubric 비교 지점 3개를 정확 식별. 해시 생성 함정 미분류, 호출부 추가 언급은 rubric "감점 없음" 조항. 본문 5곳 동등 나열은 구조 모호성이나 rubric 요건 위반 아님 — ds-base1 동형 사례(claude-base s10-r1 "4곳" 표기) correct 판정과 일관 기준 |

- 그 외 38건: rubric 미충족 0, 인용 불일치 1건(d7-r1 codex 보조 인용)은 채점 비영향.
- 정정 기입: `write-scores.sh ds-iter3 rescore/overturn-scores.json` (2/2).

## 채점 신뢰성 기록 (중요)

이 시리즈에서 **재채점이 1차 채점을 뒤집은 첫 사례** (직전까지 누적 불일치 0).
두 건 모두 "채점자가 더 엄격" 방향 — s9-r2는 명백한 사실 오인(존재하는 섹션을
누락으로 판정), s10-r1은 rubric의 "추가 언급 감점 없음" 조항 미적용. 모델 답안의
회귀가 아니라 채점 변동성이며, 같은 답안 스타일이 이전 회차들에서 correct였다.
교훈: 1차 채점 rationale에 rubric 조항 인용을 의무화하면 이런 편차를 줄일 수 있다.

## 개선 회차(루프 2) 특이 확인

- 함정 답안 제시 0건, dangling "same as" 0건, warming 응답 0건 (verify 전수).
- 축약 마커("more lines)")가 실전 에피소드에서 정상 발화 — 렌더 무결성 유지.

## 결론

루프 2 바이너리에서 정답률 100% 유지 확정 (재채점 정정 후). 효율 비교는 4자
집계(iter1/iter2/iter3/base1)에서 판정.
