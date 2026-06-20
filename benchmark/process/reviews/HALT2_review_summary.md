# HALT2 — 검토 findings 종합 (사용자 accept/reject 판단용)

> 오케스트레이터는 findings를 생성·정리만 한다. accept/reject는 사용자가 정한다.
> 원본: `reviews/constructive.md`(10건), `reviews/adversarial.md`(7건). 둘 다 scored_episodes.180.json 독립 재계산 기반.

## 바텀라인
- **치명 오류 1건 있음** (적대 #3/#4 = 건설 F1로 수렴): 재작성 보고서가 **자기 정의한 동률 밴드 규칙을 codex 셀에 위반**해, 밴드 안 tie를 "win"으로 오분류했다. 이게 "2-모델 교차복제" 헤드라인의 유일한 근거였다.
- 기본 집계(180/166/24/codex exercised 27)·tok_in·backend_off 전환은 적대 검증 **통과(holds/refuted=정확)**.
- 나머지는 분모 표기·반올림·비표준 라벨 등 정직성/정밀도 항목.

## 주요(major)

1. **codex win→tie 오분류 (치명)** — codex CH serena Δ+0.167<밴드0.25=**tie**, codex deno codemap Δ+0.042<밴드0.125=**tie**. 재계산 결과 **codex에 실제 win backend 없음**(deno codegraph/serena는 loss). 보고서가 밴드 규칙을 codex에만 미적용.
   → 헤드라인 "2-모델 교차복제(둘 다 codemap/serena win)"를 **"방향(순위) 일치이나 codex는 밴드 내 tie — claude는 실제 win, codex는 MCP 효과 구별 안 됨"**으로 약화. 원인 가설(inferred): codex의 강한 no-mcp baseline(sandbox)이 MCP 한계효용을 낮춤.
2. **opencode-serena "27 평균 0.151" 분모 미명시** — 0.151은 **valid n=22** 기준(timeout 3 제외). 전수 27 기준은 0.1227. "27 episode 평균"이라 쓰면 오독. → "valid 22개 평균(timeout 3 제외)"로 분모 명시 + 코드베이스별 n 병기(CH8/deno6/angular8).
3. **backend_off 24 분해 누락(report §2)** — runtime 합계만 있고 "carryover 12(opencode non-serena) + 신규 opencode-serena 12" 분해가 없음. → §2에 분해 추가 또는 limitations §5 교차참조.

## 경미(minor)
- 반올림 0.625→0.63 3셀(claude CH codegraph, codex angular codemap/codegraph) → 0.625로.
- 비표준 라벨 "win(밴드 경계)"·"tie-high" 제거, 밴드 규칙 일관 적용(tie(+0.13) 등).
- n=3 descriptive 한계를 헤드라인 근처로 선제 고지(강도 비대칭 명시).
- detailed_report §1: "codex behavioral null"에 **"사용자가 -02 HALT2에서 수락한"** 명시(파일 간 일관성).

## 참고(nit)
- 풀링표 제목 "5 model 풀링"→"claude 기준 clean 기준선".
- key_facts_digest §E 분모(valid 22) 명시(서술 에이전트 재오류 방지).
- limitations §11에 angular 앵커 variance 구분·텔레메트리 버그 class(§4-4) 교차참조.

## holds (수정 불요 — 적대 검증 통과)
- executed 180 / valid 166 / timed_out 10 / backend_off 24 / codex exercised 27 전수 일치. 153→180 backend_off 전환 정확. deno LOSS 수치 정확(단 헤드라인 배치 framing은 major#1과 함께 교정).

## 오케스트레이터 권고 (참고 — 결정은 사용자)
- **주요 3건은 수용 강력 권장**: 특히 #1은 보고서 자기모순(밴드 규칙 위반)이라 정직성·정확성 모두 요구. 수용 시 헤드라인이 "교차복제"→"방향 일치/강도 비대칭, MCP는 약한 baseline(claude)에서만 실효"로 바뀐다(이게 데이터가 실제로 말하는 것).
- 경미·참고는 정밀도/일관성 향상(선택).
- 수치 결론(180/166/24/codex usable)은 불변 — 바뀌는 건 codex의 win/tie 해석과 헤드라인 강도뿐.
