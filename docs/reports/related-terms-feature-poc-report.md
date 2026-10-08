# 관련 검색어 기능 PoC 결과 보고서

> **후속:** 이 보고서는 2라운드(heldout-1) 결과다. 이후 심층분석과 개선을 세 번 반복한 결과(heldout-2~4)는 [3라운드 보고서](related-terms-feature-poc-round3-report.md)를, 평가를 다시 설계한 heldout-5와 최종 결정(블록을 접음)은 [4라운드 보고서](related-terms-feature-poc-round4-report.md)를 본다.

> **판정: 불합격.** 미리 정한 성공 기준 가운데 정밀도(72.9%)와 하드 제한은 통과했지만, 효과 기준은 Grafana에서 통과하지 못했고(통제군을 넘어선 회복 1건, 기준 3건) codemap-search에서는 측정할 수 없었다. 평가는 방법을 해시로 고정한 뒤 held-out 사례로 한 번만 실행했다.

## 1. 확인하려던 것

코딩 에이전트가 `initial_instructions(task_query, terms)`로 검색어를 넘기면, 서버가 관련 검색어 블록(사전 관련어와 형태 변형, 추천 쿼리)을 돌려준다. 에이전트가 그 블록을 이용해 검색을 이어 갈 때, **기본 검색이 놓친 정답 파일을 관련어 덕분에 찾는지** 확인하는 것이 목표였다.

- 조회 시점에는 모델·네트워크를 쓰지 않는다. 사전(SQLite)과 인덱싱 때 계산한 저장소 통계만 읽는다.
- 형태 변형은 tantivy의 Snowball 영어 어간 추출기(stemmer)로 인덱싱 시점에 만든 `word_stems` 표를 사용한다.
- 실험 코드는 `apps/codemap-search/experiments/related-terms-poc/` 안에만 있다. 제품 코드(`apps/codemap-search/src/`)는 바꾸지 않았다.

## 2. 성공 기준 (평가 실행 전 사용자 결정)

기준 파일: [`contracts/feature-success-criteria.json`](../../apps/codemap-search/experiments/related-terms-poc/contracts/feature-success-criteria.json)

| 항목 | 기준 |
|---|---|
| 효과 | 대상별로 중간·복잡 난이도 중 기본 검색이 놓친 사례 수를 M이라 할 때, **통제군을 넘어선 회복 ≥ ⌈M/3⌉** |
| 정밀도 | 블록에 실제로 보인 관련어의 **유용 비율 ≥ 70%** (동의어·유사어·별칭·약어·형태 변형) |
| 순위 범위 | MCP search가 보여 주는 순서로 **상위 10개**, 후보 수 100 (제품 기본값) |
| 판정 모드 | 사전+형태 변형(**both**) 모드. 사전만·형태 변형만 모드는 기여도 분석용 |
| 하드 제한 | 모든 모드에서 블록 2048바이트 이하, 식별자 형태 입력어 확장 0건 |

- **기본 검색**: 에이전트 입력어를 모두 이어 붙인 한 번의 검색.
- **처리군**: 기본 검색에 추천 쿼리(최대 5개)를 더한 결과.
- **통제군**: 같은 횟수만큼 **원래 입력어 조합만으로** 검색한 결과. 검색 횟수가 늘어난 효과를 빼고 관련어 자체의 효과만 보기 위해 둔다.

## 3. 결과

### 3.1 효과 (both 모드, 상위 10개)

| 대상 | 중간+복잡 사례 | 기본 검색 적중 | M | 처리군 회복 | 통제군 회복 | 통제군을 넘어선 회복 | 기준 | 판정 |
|---|---|---|---|---|---|---|---|---|
| Grafana | 24 | 15 | 9 | 3 | 4 | **1** | 3 | 불합격 |
| codemap-search | 16 | 16 | 0 | 0 | 0 | 0 | — | 측정 불가 |

- 단순 난이도에서는 두 대상 모두 기본 검색이 정답을 모두 찾았다(Grafana 12/12, codemap-search 4/4).
- 사전만 모드는 회복 0건이다. 형태 변형만 모드는 both와 같았다(회복 3, 통제군을 넘어선 회복 1).
- 사후 탐색으로 상위 5개 기준으로 다시 세어도 결론은 같았다. Grafana는 M=11, 처리군 회복 3건, 통제군 회복 3건, 통제군을 넘어선 회복 1건으로 기준 4건에 못 미쳤다. codemap-search는 M=0이었다. 이 재집계는 판정에 쓰지 않았다.

Grafana에서 기본 검색이 놓친 9건:

| 사례 | 처리군 | 통제군 | 비고 |
|---|---|---|---|
| medium-go-2 | 적중 | 놓침 | **유일하게 통제군을 넘어선 회복.** `org users` → `org user list users`로 정답 2개를 모두 찾음 |
| medium-ts-2 | 적중 | 적중 | 둘 다 찾음 |
| complex-go-4 | 적중 | 적중 | 둘 다 찾음 |
| medium-ts-3 | 놓침 | 적중 | 통제군만 찾음 (`dashboard model export json`) |
| complex-ts-1 | 놓침 | 적중 | 통제군만 찾음 (`permission move button variable` 계열) |
| prep-complex-go, medium-ts-4, complex-ts-2, complex-ts-5 | 놓침 | 놓침 | — |

### 3.2 정밀도 (both 모드 블록에 보인 관련어 755쌍)

라벨러는 개발 단계와 같은 codex gpt-6.1-sol이고, 같은 라벨 집합을 프롬프트 맥락에서 적용했다. 에이전트 라벨이며 사람이 만든 정답 데이터가 아니다.

| 구분 | 쌍 | 유용 | 비율 |
|---|---|---|---|
| 전체 | 755 | 550 | **72.9%** (합격) |
| Grafana | 501 | 354 | 70.7% |
| codemap-search | 254 | 196 | 77.2% |
| 사전 관련어 | 30 | 25 | 83.3% |
| 형태 변형 (한 단어 용어) | 155 | 137 | 88.4% |
| 형태 변형 (구 안의 단어 교체) | 570 | 388 | 68.1% |

### 3.3 하드 제한과 비용

- 블록 크기: 최대 1,232바이트, 평균 약 857바이트(Grafana 중간·복잡, both 모드). 2048바이트를 넘은 사례는 없다.
- 식별자 형태 입력어가 확장된 사례: 0건.
- 사례당 검색 6회(기본 1회와 추천 5회), 서로 다른 검색 640회, 빈 결과 0건. 실행 시간은 4분 22초였다.

## 4. 불합격 원인 분석 (판정 후 탐색, 판정에는 쓰지 않음)

1. **추천 쿼리가 소수의 용어에 몰린다.** 확장 가능한 입력어는 사례당 평균 5.75개였지만, 추천 쿼리 5개가 실제로 바꾼 입력어는 평균 약 2.5개였다. 쿼리 조립 규칙(H9)이 첫 용어의 대체어를 모두 쓴 뒤에야 다음 용어로 넘어가기 때문이다. 예를 들어 medium-ts-3의 추천 쿼리 5개는 `provisioned dashboard`와 `save dashboard`의 변형뿐이었다. 반면 통제군은 용어마다 한 쿼리씩 돌아가며 써서 입력어를 더 넓게 훑었고, 그 결과 통제군 회복(4건)이 처리군(3건)보다 많았다.
2. **사전이 실제 에이전트 입력어를 거의 덮지 못한다.** 56개 사례 중 사전 관련어가 하나라도 나온 사례는 19개였고, 사전만 모드의 회복은 0건이었다. 개발 세트에서는 사전 품질이 크게 좋아졌지만(4.1절), 에이전트가 실제로 쓰는 구 단위 용어(`provisioned dashboard`, `watch metrics` 등)는 사전에 거의 없다.
3. **구 안의 단어를 굴절형으로 바꾸는 규칙(H11)의 품질이 낮다.** 한 단어 용어의 굴절형은 88%로 개발 세트(89%)와 같았다. 반면 구 안의 단어 교체는 68%였다. `index version`을 `indexed version`으로, `move button`을 `moving button`으로 바꾸는 식으로 명사가 분사가 되어 뜻이 달라진다. 개발 세트 입력은 모두 한 단어였기 때문에 이 규칙은 held-out 전에 품질을 검증받지 못했다.
4. **codemap-search 정답 세트가 너무 쉬웠다.** 중간·복잡 16건 모두 기본 검색이 상위 5개 안에서 정답을 찾아, 효과를 측정할 사례가 없었다.

## 5. 개발 단계에서 확인한 것

### 5.1 사전 v2 (개발 세트 92개 입력 × 저장소 4개)

선별 기준을 새로 정해([`contracts/curation-v2-rubric.md`](../../apps/codemap-search/experiments/related-terms-poc/contracts/curation-v2-rubric.md)) WordNet 동의어 집합 4,335개를 다시 판정했다. 그 결과로 데이터셋 `rtd1-2df12da52eb7a387`(개념 3,208, 표기 6,726, 관계 10,356)을 만들었다.

| 지표 | v1 | v2 |
|---|---|---|
| 최종 후보 쌍 | 97 | 59 |
| 유용 비율 | 59.8% | **84.7%** |
| 틀린 뜻 비율 | 35.1% | **6.8%** |
| 최종 후보가 나온 조회 (368개 중) | 124 | 117 |

v2 조회 정책이 저장소에 없다는 이유로 걸러낸 후보 30쌍은 대부분 유용한 관련어였다. 다만 해당 저장소에 그 단어가 없으므로 검색에는 도움이 되지 않는다.

### 5.2 형태 변형 유형별 품질 (개발 세트 165쌍)

| 유형 | 쌍 | 유용 또는 형태 변형 |
|---|---|---|
| 굴절형 (inflection) | 110 | 98 (89%) |
| 파생형 (derivation) | 20 | 9 (45%) |
| 같은 어근 (same_root, 접두어) | 35 | 6 (17%) |

이 결과에 따라 held-out 전에 블록의 형태 변형을 굴절형으로만 제한했다.

## 6. 평가의 타당성

- 사례·용어·프롬프트는 어떤 조회보다 먼저 고정했다([`freeze.json`](../../.codemap/poc/related-terms/eval-targets/freeze.json)).
- 방법 파일 28개도 해시로 고정했다([`method-freeze.json`](../../.codemap/poc/related-terms/eval-targets/method-freeze.json)). 대상은 바이너리, 소스, 정책, 데이터셋, 하네스, 기준, 라벨 기준, 인덱스와 통계 파일이다. 하네스는 실행 전에 모두 대조한다.
- 성공 기준은 데이터셋과 블록을 정한 뒤, held-out을 실행하기 전에 사용자가 골랐다.
- `rtpoc2 search`는 제품 CLI와 같은 순위 함수를 호출하며, 12개 쿼리에서 결과 순서까지 같았다. 표시 순서는 제품의 디렉터리 분산 로직을 그대로 옮겼다.
- 실행 전 독립 코드 검토에서 아래 문제가 지적되었고, 모두 반영한 뒤 고정했다.
  - 형태 변형이 유형 필터보다 먼저 5개로 잘리는 문제.
  - 바이트 상한으로 숨겨진 관련어가 라벨 대상에 섞이는 문제.
  - 인덱스가 조용히 재생성될 위험.
  - 스모크 실행이 held-out 상태를 건드리는 경로.
  - 동결 검증 범위가 좁은 문제.
- **한계**
  - 정밀도 라벨은 에이전트 1회 판정이며 사람이 만든 정답 데이터가 아니다.
  - Grafana의 M=9로 표본이 작아 1~2건 차이로 판정이 바뀔 수 있다.
  - 평가는 에이전트가 실제로 블록을 읽고 쿼리를 고르는 과정을 흉내 내지 않았다. 추천 쿼리를 모두 실행한다고 가정했다.
- 이번 held-out 사례는 결과를 이미 보았으므로, 같은 사례로 설계를 고쳐 다시 판정하면 공정한 평가가 아니다.

## 7. 다음 단계 선택지 (사용자 결정 필요)

1. **여기서 종료**: 관련어 블록은 현재 형태로 검색 효과를 입증하지 못했다고 결론짓는다.
2. **설계 보완 후 새 held-out으로 재평가**: 4절의 원인을 고친다.
   - 추천 쿼리를 용어마다 돌아가며 배분한다.
   - 구 안의 단어 교체는 품사가 바뀌지 않는 굴절(복수형 등)만 허용한다.
   - 대신 **새 평가 사례 세트**가 필요하다.
3. **codemap-search 평가 세트 보강**: 기본 검색이 놓치는 어려운 질문으로 다시 만든다. 2번과 함께 하는 것이 좋다.

## 8. 검토용 파일

| 파일 | 내용 |
|---|---|
| [`final-verdict.json`](../../.codemap/poc/related-terms/evaluation/feature-test/heldout-1/final-verdict.json) | 최종 판정 (효과·정밀도·하드 제한) |
| [`summary.json`](../../.codemap/poc/related-terms/evaluation/feature-test/heldout-1/summary.json) | 대상 × 난이도 × 모드별 집계 |
| [`cases.jsonl`](../../.codemap/poc/related-terms/evaluation/feature-test/heldout-1/cases.jsonl) | 사례·모드별 쿼리, 적중 여부, 블록 크기 |
| [`raw/`](../../.codemap/poc/related-terms/evaluation/feature-test/heldout-1/raw/) | 사례별 블록 JSON, 모든 검색의 원시 결과 (`searches.jsonl`) |
| [`label-queue.jsonl`](../../.codemap/poc/related-terms/evaluation/feature-test/heldout-1/label-queue.jsonl), [`review/labels-combined.jsonl`](../../.codemap/poc/related-terms/evaluation/feature-test/heldout-1/review/labels-combined.jsonl) | 정밀도 라벨 대상과 라벨 |
| [`analysis-exploratory.json`](../../.codemap/poc/related-terms/evaluation/feature-test/heldout-1/analysis-exploratory.json) | 판정 후 탐색 분석 (상위 5개 재집계, 놓친 사례) |
| [`comparison.json`](../../.codemap/poc/related-terms/evaluation/development-v2/v2-run-1/reports/fresh-labels-v2/comparison.json) | 개발 세트 v1/v2 비교 |
| [`lookup-policy-rtl-2.1.json`](../../apps/codemap-search/experiments/related-terms-poc/lookup-v2/contracts/lookup-policy-rtl-2.1.json) | 조회·블록 규칙 (H1–H12) |
| [`lookup-v2/`](../../apps/codemap-search/experiments/related-terms-poc/lookup-v2/) | `rtpoc2` (index, lookup, block, search) |
| [`evaluate_feature.py`](../../apps/codemap-search/experiments/related-terms-poc/evaluate_feature.py), [`judge_feature_precision.py`](../../apps/codemap-search/experiments/related-terms-poc/judge_feature_precision.py) | 평가 하네스와 정밀도 판정 |

큰 데이터는 Git에서 제외된 `.codemap/poc/related-terms/` 아래에 있다. 이번 작업은 커밋·스테이징하지 않았다.
