# 관련 검색어 기능 PoC 3라운드 보고서 (반복 개선 3회)

> **후속:** 평가를 다시 설계한 heldout-5 결과와 최종 결정(블록을 접음)은 [4라운드 보고서](related-terms-feature-poc-round4-report.md)를 본다. 아래 6절에서는 3번(평가 설계 재검토)을 골라 4라운드로 이어졌다.

> **판정: 목표 미달성.** 심층분석과 개선을 세 번 반복하고, 반복마다 새로 만든 held-out 세트로 한 번씩 판정했다. 세 번 모두 불합격이었다.
>
> - **정밀도·하드 제한:** 매번 통과했다. 정밀도는 91.9% → 97.1% → 93.4%였다.
> - **효과:** Grafana는 세 번 모두 기준에 못 미쳤다. 통제군을 넘어선 회복은 매번 1건이었다.
> - **codemap-search:** 마지막 반복에서만 기준을 넘었다.
>
> 반복 상한(3회)에 도달해 여기서 멈춘다.

이 보고서는 [2라운드 보고서](related-terms-feature-poc-report.md)(heldout-1, rtl-2.1)에 이어지는 내용이다.

## 1. 목표와 판정 기준

목표는 그대로다. 에이전트가 `initial_instructions(task_query, terms)`로 검색어를 넘기면 관련 검색어 블록이 돌아온다. 이 블록 덕분에 **기본 검색이 놓친 정답 파일을, 같은 횟수의 원래 검색어 검색보다 더 많이 찾는지** 확인한다.

기준 파일: [`contracts/feature-success-criteria-2.json`](../../apps/codemap-search/experiments/related-terms-poc/contracts/feature-success-criteria-2.json)

| 항목 | 기준 |
|---|---|
| 효과 | 대상별로 중간·복잡 난이도 중 기본 검색이 놓친 사례 수를 M이라 할 때, **통제군을 넘어선 회복 ≥ ⌈M/3⌉** |
| 정밀도 | 블록에 보인 관련어의 **유용 비율 ≥ 75%** (2라운드의 70%에서 사용자 요청으로 상향) |
| 순위 범위 | MCP search 표시 순서로 상위 10개, 후보 수 100 |
| 판정 모드 | 사전+형태 변형(both) |
| 하드 제한 | 블록 2048바이트 이하, 식별자 형태 입력어 확장 0건 |

- **통제군:** 추천 쿼리와 같은 수의 쿼리를 에이전트 자신의 검색어만으로 만든다.
- **2라운드 이후의 짝 구성:** 추천 쿼리 하나마다 짝이 되는 통제 쿼리가 있다. 둘은 관련어 정보만 다르다.

## 2. 반복 요약

| 반복 | 정책 | 핵심 변경 | held-out | Grafana (회복/M, 기준) | codemap-search (회복/M, 기준) | 정밀도 | 판정 |
|---|---|---|---|---|---|---|---|
| (2라운드) | rtl-2.1 | — | heldout-1 | 1/9, 3 ✗ | 측정 불가 (M=0) | 72.9% | 불합격 |
| 1 | rtl-2.2 | 짝 쿼리(H13), 구 안 단수·복수만(H11) | heldout-2 | 1/7, 3 ✗ | 0/2, 1 ✗ | 91.9% | 불합격 |
| 2 | rtl-2.3 | 저장소 식별자 형태(H14), 용어 간 결합 식별자(H15), 단독 검색 짝(H16) | heldout-3 | 1/7, 3 ✗ | 1/4, 2 ✗ | 97.1% | 불합격 |
| 3 | rtl-2.4 | 한 단어 용어의 관련어 단독 검색(H17), 추천 쿼리 4개 | heldout-4 | 1/4, 2 ✗ | **1/3, 1 ✓** | 93.4% | 불합격 |

"회복"은 통제군을 넘어선 회복 건수다. 하드 제한은 매번 통과했다. 블록 최대 크기는 1,252 / 1,314 / 1,308바이트였고, 식별자 형태 입력어 확장은 0건이었다.

held-out 세 개를 합쳐 보면 다음과 같다.

- **Grafana:** 통제군을 넘어선 회복이 3건이다. 같은 기준을 세 번 넘으려면 8건이 필요했다.
- **codemap-search:** 회복이 2건이고, 필요한 건수는 4건이었다.

## 3. 반복별 심층분석과 개선

각 반복은 다음 순서로 진행했다.

1. 직전 held-out을 개발 데이터로 돌려 분석한다.
2. 방법을 바꾸고 독립 검토를 받는다.
3. 사례·검색어·방법을 해시로 고정한 뒤 새 held-out으로 한 번 실행한다.

새 held-out 문항은 매번 두 작성 에이전트가 썼다. 작성 에이전트는 검색 도구를 쓰지 않았고, 이전 세트의 정답 경로와 주제는 피했다. 검색어는 다른 에이전트가 프롬프트만 보고 정했다. 문항은 대상별로 Grafana 32개, codemap-search 24개다.

### 3.1 반복 1 — rtl-2.2 (heldout-2)

**분석 (heldout-1):**
- 놓친 사례는 어휘가 없어서가 아니라 순위 문제였다. 놓친 Grafana 사례 검색어 토큰의 89~91%가 이미 정답 파일에 있었다.
- 구 안의 단어를 `-ed`/`-ing`로 바꾸는 변형은 뜻을 바꿔 정밀도를 떨어뜨렸다.
- H9는 추천 쿼리 5개를 앞쪽 한두 용어에 몰아 썼다.

**개선:**
- 구 안 변형은 단수·복수만 허용했다(H11). 복수형 규칙(`-es`, `-ies`)의 오류도 고쳤다.
- 추천 쿼리 i를 "통제 쿼리 i + 관련어 하나"로 바꿨다(H13, 짝 쿼리).

**결과 (heldout-2):**
- 정밀도는 72.9%에서 91.9%로 올랐다.
- 효과는 Grafana 1/7(기준 3), codemap-search 0/2(기준 1)였다.

### 3.2 반복 2 — rtl-2.3 (heldout-3)

**분석 (heldout-1, heldout-2):**
- 단수·복수 변형을 덧붙여도 상위 10개는 거의 바뀌지 않았다.
- 반면 저장소에 실제로 있는 **식별자 철자**는 제품 검색을 식별자 조회 방식으로 바꿨다. 이 방식은 정의를 우선하고 정확한 이름에 가중을 준다(`QueryTokens::is_identifier_lookup`). 그 결과 정답 순위가 크게 올랐다.

| 사례 | 원래 용어 순위 | 식별자 철자 순위 |
|---|---|---|
| `statPanel` | 12 | 1 |
| `editorMode` | 42 | 3 |
| `queryOptions` | 없음 | 2 |

- 정답 파일은 에이전트의 두 용어를 이은 식별자를 자주 정의하고 있었다(`syncExternalAlertmanager`, `redirectOrgId`). 이런 식별자는 단독으로 검색할 때만 효과가 있었다.
- Rust는 snake_case라서 기존 어휘 통계로 확인할 수 없었다. 분석기가 밑줄에서 단어를 나누기 때문이다. 그런데 `test_file`을 단독으로 검색하면 정답이 5위였고, `test file`은 27위였다.

**개선:** 아래 세 가지 모두 저장소에 실제로 있을 때만 보여 준다.

- **H14:** 구 용어를 camelCase(어휘 통계로 확인)나 snake_case(새 `rt-snake-parts/1` 표로 확인)로 쓴 식별자 형태. `rt-snake-parts/1`은 인덱싱 시점에 만든다.
- **H15:** 두 용어를 이은 식별자.
- **H16:** 짝 구성. 결합 식별자와 snake_case 형태는 단독으로 검색하고, 통제 쿼리는 그 식별자를 만든 원래 단어로 한다. camelCase 형태는 H13처럼 덧붙인다.

**검증:**
- 개발 데이터 결과는 heldout-1 Grafana 2/3, heldout-2 Grafana 3/3, codemap-search 1/1이었다.
- 새 식별자 후보 160개를 표본 라벨링해 모두 유용 판정을 받았다.
- 독립 검토는 두 차례 받았다. 1차의 주요 지적 2건(봉인 입력 혼용 경로, 크기 제한 뒤 표시·실행·라벨링 불일치)과 경미 지적 3건을 고친 뒤 고정했다.

**결과 (heldout-3):**
- 효과는 Grafana 1/7(기준 3), codemap-search 1/4(기준 2)였다.
- 정밀도는 97.1%였고, 식별자 형태는 291개 중 291개가 유용 판정을 받았다.

### 3.3 반복 3 — rtl-2.4 (heldout-4)

**분석 (heldout-1~3):**
- heldout-3에서 놓친 11건 중 6건은 통제 쿼리(용어 + 앵커 단어)도 이미 찾았다. 통제군을 넘을 수 있는 사례는 5건뿐이었다.
- 한 단어 관련어를 통제 쿼리에 덧붙이면 앵커 단어에 묻혔다. 단독으로 검색하면 순위가 크게 달랐다.

| 쿼리 | 정답 순위 |
|---|---|
| `pref`를 통제 쿼리에 덧붙임 | 39 |
| `pref` 단독 | 1 |
| `preference` 단독 | 5 |
| 에이전트 용어 `preferences` 단독 | 60 |

- 저장된 블록으로 짝 설계를 모의 실험했다. 모의 결과는 실제 rtl-2.3 결과와 정확히 일치했다.
- 어떤 설계도 모든 개발 대상·세트 조합에서 기준을 넘지 못했다. heldout-1 Grafana와 heldout-3 codemap-search는 어떤 설계로도 이길 수 있는 사례 수가 기준보다 적었다.

**개선:**
- **H17:** 한 단어 용어의 관련어는 단독으로 검색하고, 통제 쿼리는 그 용어 단독으로 한다. 양쪽 모두 한 단어라 같은 검색 방식이 된다. 다만 사전 관련어가 여러 단어인 짝 20개는 예외다.
- **추천 쿼리 수:** 5개에서 4개로 줄였다.

**검증:**
- 개발 데이터 결과는 heldout-1 Grafana 2/3, heldout-2 3/3·1/1, heldout-3 3/3·1/2로, 모든 조합에서 rtl-2.3 이상이었다.
- 독립 검토는 차단 문제 없음으로 끝났다. 정책 설명 1건을 고쳤다.
- 다만 쿼리 수 4가 5보다 나은 근거는 사례 하나(c3-23)에 의존한다. 개발 데이터로 고른 값이라 과적합일 수 있다.

**결과 (heldout-4):**
- 효과는 Grafana 1/4(기준 2), codemap-search 1/3(기준 1, 통과)였다.
- 정밀도는 93.4%였다.

## 4. 알게 된 것

1. **정밀도 문제는 해결됐다.** 구 안 변형을 단수·복수로 제한하고 저장소에서 확인된 식별자만 보여 주자, 정밀도가 72.9%에서 92~97%로 올랐다. 식별자 형태는 held-out에서도 모두 유용 판정을 받았다(heldout-3 291/291, heldout-4 301/301).
2. **효과는 개발 데이터에서 보인 개선이 held-out에서 재현되지 않았다.** 반복 2와 3은 개발 데이터에서 Grafana 3/3을 냈다. 그러나 새 세트에서는 매번 1건에 그쳤다. 개발 데이터로 설계를 고른 만큼 낙관 편향(평균 회귀)이 있었다고 본다.
3. **구조적인 한계가 크다.**
   - 통제 쿼리가 강하다. 앵커 단어를 붙인 용어 조합이나 용어 단독 검색이, 기본 검색이 놓친 사례의 절반 안팎을 이미 찾았다.
   - 기본 검색이 놓치는 사례(M)가 대상별로 3~9건뿐이다. 그래서 기준을 넘으려면 통제군이 놓친 사례를 거의 모두 이겨야 한다.
4. **식별자 철자 효과는 실제로 있다.** 식별자 형태를 넣은 반복 2·3에서 held-out으로 이긴 4건은 모두 식별자 형태 덕분이었다. 반복 1에서 이긴 1건은 단수·복수 변형이었다.

| held-out | 이긴 관련어 | 통제 쿼리 순위 → 추천 쿼리 순위 |
|---|---|---|
| heldout-2 | `sparklines` (단수·복수 변형) | 13 → 6 |
| heldout-3 | `barChart` | 36 → 4 |
| heldout-3 | `symbol_context` | 36 → 3 |
| heldout-4 | `calculateHeatmap` | 28 → 3 |
| heldout-4 | `thread_local` | 20 → 1 |

   다만 이런 사례가 한 세트에 1~2건이라, 대상별 ⌈M/3⌉을 매번 채우기에는 모자랐다.

## 5. 평가의 타당성과 한계

- 매 반복에서 사례·검색어·프롬프트를 방법 결정 전에 고정했다(`eval-targets/heldout-N/freeze.json`). 방법 파일 33개도 실행 전에 고정했다(`method-freeze.json`). 대상은 바이너리, 소스, 정책, 데이터셋, 하네스, 기준, 라벨 기준, 인덱스·통계·snake 표다. 하네스는 실행 전에 이를 모두 대조한다.
- held-out은 반복마다 한 번만 실행했다. 실행에 쓴 방법은 `evaluation/feature-test/heldout-N/method-snapshot/`에 보관했다.
- 새 바이너리가 이전 정책의 출력을 바이트 단위로 재현하는지 매번 확인했다. rtl-2.2는 heldout-2 168개, rtl-2.3은 heldout-3 168개 블록이다.
- **한계**
  - 정밀도 라벨은 codex gpt-6.1-sol의 1회 판정이다. 사람이 만든 정답 데이터가 아니다.
  - M이 작아 1건 차이로 판정이 바뀐다.
  - 에이전트가 블록을 읽고 쿼리를 고르는 과정은 흉내 내지 않았다. 추천 쿼리를 모두 실행한다고 가정했다.
  - H16/H17의 효과에는 관련어 자체뿐 아니라, 식별자 철자로 검색 방식이 바뀌는 효과도 들어 있다. 독립 검토도 이 점을 지적했다.

## 6. 다음 단계 선택지 (사용자 결정 필요)

1. **여기서 종료:** 관련 검색어 블록은 현재 기준으로 검색 효과를 입증하지 못했다고 결론짓는다.
2. **식별자 형태만 따로 검토:** 정밀도는 거의 100%이고 작동 원리도 분명하다. 다만 효과는 세트당 1~2건이다. 제품에 넣을지는 효과 기준을 다시 정한 뒤 판단해야 한다.
3. **평가 설계 재검토:** 이번 결과로 기준을 바꾸는 것이므로 새 held-out이 필요하다. 예를 들면 다음과 같다.
   - 판정 단위를 세트 합산으로 바꾼다.
   - 기본 검색이 놓치는 사례를 늘린다.
   - 에이전트가 블록을 직접 읽고 쿼리를 고르는 평가를 한다.

## 7. 검토용 파일

| 파일 | 내용 |
|---|---|
| `.codemap/poc/related-terms/evaluation/feature-test/heldout-{2,3,4}/final-verdict.json` | 반복별 최종 판정 (효과·정밀도·하드 제한) |
| `.codemap/poc/related-terms/evaluation/feature-test/heldout-{2,3,4}/{summary.json,cases.jsonl,raw/}` | 집계, 사례·모드별 쿼리와 적중, 블록과 검색 원본 |
| `.codemap/poc/related-terms/evaluation/feature-test/heldout-{2,3,4}/review/` | 정밀도 라벨 배정과 라벨 |
| `.codemap/poc/related-terms/evaluation/feature-test/heldout-2/analysis-iteration2/`, `heldout-3/analysis-iteration3/` | 반복 2·3 심층분석 요약과 스크립트 |
| `.codemap/poc/related-terms/evaluation/development-v3/` | rtl-2.3/2.4 개발 실행, 식별자 형태 개발 라벨 |
| `.codemap/poc/related-terms/eval-targets/heldout-{2,3,4}/` | 사례 원본, 프롬프트, 입력·방법 고정 기록 |
| [`lookup-policy-rtl-2.2.json`](../../apps/codemap-search/experiments/related-terms-poc/lookup-v2/contracts/lookup-policy-rtl-2.2.json), [`rtl-2.3`](../../apps/codemap-search/experiments/related-terms-poc/lookup-v2/contracts/lookup-policy-rtl-2.3.json), [`rtl-2.4`](../../apps/codemap-search/experiments/related-terms-poc/lookup-v2/contracts/lookup-policy-rtl-2.4.json) | 반복별 정책 (H13–H17 규칙과 근거) |
| [`lookup-v2/src/block.rs`](../../apps/codemap-search/experiments/related-terms-poc/lookup-v2/src/block.rs), [`snake_parts.rs`](../../apps/codemap-search/experiments/related-terms-poc/lookup-v2/src/snake_parts.rs) | 블록·짝 쿼리, snake_case 부분 어휘 |
| [`evaluate_feature.py`](../../apps/codemap-search/experiments/related-terms-poc/evaluate_feature.py), [`judge_feature_precision.py`](../../apps/codemap-search/experiments/related-terms-poc/judge_feature_precision.py), [`feature-precision-rubric-2.json`](../../apps/codemap-search/experiments/related-terms-poc/contracts/feature-precision-rubric-2.json) | 평가 하네스, 정밀도 판정, 라벨 기준 |

큰 데이터는 Git에서 제외된 `.codemap/poc/related-terms/` 아래에 있다. 제품 코드(`apps/codemap-search/src/`)는 바꾸지 않았고, 커밋·스테이징도 하지 않았다.
