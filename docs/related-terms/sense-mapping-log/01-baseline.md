# 1단계: 기준 평가 세트와 현재 판 점수

작업일: 2026-10-17. 대상 판: `rts3-48356e35e1f1ea28`(SHA-256 `80df2551b99d43f17db428b6273ed6459a36f4a8d4e8a7e7b0e8dcf695e4f008`).

뒤 단계에서 조회 결과가 나아졌는지 같은 잣대로 재기 위해, 코드에서 뽑은 단어와 그 단어가 코드에서 뜻하는 분야를 평가 세트로 만들고 현재 판의 점수를 냈다.

## 1. 코드 저장소와 식별자 수집

- 저장소: [원자료와 근거 저장소](../sources.md)의 12개. 판은 `datasets/rts3-9e01eaf1eadf960a/sources.json`의 태그·커밋이다.
- 체크아웃이 지워져 있어서 `.codemap/poc/related-terms/workspaces/checkouts/<저장소>`에 태그로 다시 받았다(`git clone --depth 1 --branch <태그>`). 12개 모두 HEAD가 `sources.json`의 커밋과 같다.
- 수집: `word-eval/collect_repo.py`를 기본값(목표 23,672줄, 고정 시드)으로 돌렸다. tree-sitter 0.26.0, tree-sitter-language-pack 1.21.0. 수집 결과는 보존하지 않았다.

| 저장소 | 표본 파일 | 표본 줄 | 정의된 이름 | 쓰기만 한 이름 |
| --- | ---: | ---: | ---: | ---: |
| bitwarden-server | 168 | 23,854 | 2,379 | 1,990 |
| express | 142 | 21,231 | 387 | 244 |
| flask | 83 | 17,889 | 1,692 | 597 |
| gin | 97 | 23,374 | 2,216 | 477 |
| gutenberg | 179 | 23,891 | 1,906 | 1,567 |
| keycloak | 162 | 23,843 | 2,908 | 2,861 |
| redis | 27 | 24,237 | 2,253 | 2,074 |
| sentry | 153 | 23,761 | 2,168 | 2,451 |
| terminal | 87 | 24,708 | 3,531 | 2,263 |
| tokio | 89 | 23,920 | 1,495 | 921 |
| vector | 91 | 23,834 | 2,233 | 1,964 |
| woocommerce | 154 | 24,218 | 2,324 | 1,954 |

## 2. 평가 단어 뽑기

`sense-eval/prepare_eval_set.py`가 뽑았다.

- **단위:** 테스트·생성 파일이 아닌 곳에서 정의된 이름의 이웃한 조각 1~3개(`AuthUserKey` → `auth`, `auth user`, `user key`, …) 가운데, 대상 판에서 분야를 하나 이상 가진 단어다. 분야는 단어 자신의 뜻과 `input_senses`로 받는 뜻을 합쳐 센다.
- **집단:** 3단계에서 바뀔 단어와 그렇지 않은 단어를 따로 재려고 셋으로 나눴다.

| 집단 | 뜻 | 저장소마다 |
| --- | --- | ---: |
| `target` | 표제어이거나 최근 세 단계(25df 누락 단어, 08b1 분야 연결, 5fba 새 개념)에서 들어온 단어 중 분야가 1개인 단어. 3단계 매핑 확장의 대상 | 10 |
| `multi` | 같은 단어 중 분야가 2개 이상인 단어 | 6 |
| `input` | `input_senses`로만 분류된 단어(관련어로만 있는 단어) | 4 |

- 시드 `20261017`로 저장소마다 섞어 뽑았고, 한 단어는 평가 세트에 한 번만 넣었다. 단어마다 정의 하나를 골라 그 줄과 앞뒤 한 줄을 함께 저장했다(함수·클래스 같은 이름을 매개변수·지역 변수보다 먼저 골랐다).
- 결과: 240단어(`target` 120, `multi` 72, `input` 48). `target` 중 최근 세 단계에서 들어온 단어는 5개뿐이고 나머지는 원래 표제어다. 최근 단계 단어는 코드 표본에 드물게 나온다.

## 3. 판정

- 판정: Claude Opus 5.5가 저장된 코드 줄만 보고, 그 단어가 그 코드에서 뜻하는 분야를 39개 분야에서 골랐다. 데이터셋의 뜻은 판정 시트에 넣지 않았다.
- 기대 분야는 여러 개일 수 있고 첫 번째가 주 분야다. 예: express `methodOverride`의 `method override`는 HTTP 메서드 덮어쓰기라서 `web-backend`만, gutenberg `getViewportBreakpoints`의 `breakpoints`는 반응형 레이아웃이라서 `web-styling`만, woocommerce `build_tax_query`의 `tax`는 WordPress 분류(taxonomy)라서 `business-domains`·`databases-storage`.
- 제외 16개: 그 자리에서 따로 뜻을 갖지 않는 단위다. 한 글자(`u`, `b`, `p`), 기능어(`as`, `without`, `are`, `your`, `did`, `we`), 더 긴 이름의 조각(`PayPal`의 `pay`, `FlyIO`의 `io`, `byte_code`의 `code`, `VSET_DEFAULT_C_EF`의 `c ef`, `non_empty`의 `non`, `_addUserProfileParent`의 `add user`), 지역 변수 약자(`en`).
- 평가 세트: `sense-eval/eval-set.json`(`rt-sense-eval/1`). 판정한 224단어(`target` 113, `multi` 65, `input` 46).

## 4. 채점 방법

`sense-eval/score_eval_set.py`가 단어마다 CLI 기본값으로 하나씩 조회한다(`word-eval/lookup_words.lookup_all`과 같다).

| 지표 | 뜻 |
| --- | --- |
| 적중 | 뜻 중 하나의 분야가 기대 분야에 든다 |
| 주 분야 적중 | 뜻 중 하나의 분야가 주 분야다 |
| 첫 뜻 적중 | 첫 번째 뜻의 분야가 기대 분야에 든다 |
| 뜻 수, 분야 수, 관련어 수 | 응답의 크기. 매핑을 넓히면 늘어난다 |

결과 파일에는 단어마다 텍스트 응답의 SHA-256도 남겨, 두 판의 응답을 단어별로 비교할 수 있다.

## 5. 결과

`sense-eval/results/rts3-48356e35e1f1ea28.json`

| 집단 | 단어 | 적중 | 주 분야 적중 | 첫 뜻 적중 | 뜻 수 중앙값 | 분야 수 중앙값 | 관련어 수 중앙값 | 관련어 수 평균 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 전체 | 224 | 80.8% | 71.4% | 64.3% | 2 | 1 | 124 | 209.4 |
| `target` | 113 | 85.8% | 71.7% | 85.8% | 1 | 1 | 77 | 97.6 |
| `multi` | 65 | 86.2% | 83.1% | 43.1% | 9 | 6 | 371 | 436.9 |
| `input` | 46 | 60.9% | 54.3% | 41.3% | 2 | 1 | 95 | 162.7 |

응답이 없는 단어는 없다.

## 6. 맞히지 못한 단어 43개

- **`target` 16개:** 하나뿐인 분야가 코드의 뜻과 다르다. 3단계에서 빠진 주제를 더하면 풀리는 유형이다.
  - `jsonp` → `security`만, `disposition` → `browser-platform`만, `text transform` → `text-processing`만(CSS 속성), `logo`·`toolbar item`·`navigation component` → `mobile-desktop`만, `search bar`·`component template` → `databases-storage`만, `listpack` → `data-formats`만, `vsetattr` → `ml-ai`만, `test paths` → `languages-compilers`만, `shutting down` → `language-constructs`만.
  - 일반어가 기술 분야 하나에만 있는 것: `view result`, `commercial`, `amt`, `detects`.
- **`multi` 9개:** 분야는 많지만 코드의 뜻이 없다. `prefix`(문자열 접두사 없음), `define`(언어 구성 요소 없음), `link preview`, `skip`, `fill`, `lean`(`WIN32_LEAN_AND_MEAN`), `group`(벤치마크 묶음), `second`·`third`(서수 뜻이 없고 시간 단위 등의 뜻만 있다).
- **`input` 18개:** 관련어로만 분류된 단어가 엉뚱한 분야를 받는다. `tcp state` → `files-io`, `thread stack size` → `functional-programming`, `delta t` → `web-styling`, `first byte` → `browser-platform`, `node value` → `browser-platform` 등. 3단계 대상이 아니다.

## 7. 한계

- 판정자는 모델 하나이고, 사람의 판정과 대조하지 않았다.
- 단위는 식별자 조각이라, 그 자리의 의미 단위와 경계가 다를 수 있다. 그런 단위는 제외했다.
- 분야 단위로만 맞았는지 본다. 같은 분야 안에서 주제가 맞는지는 보지 않는다.
- `target` 집단은 대부분 원래 표제어다. 최근 세 단계 단어가 매핑 확장으로 나아지는지는 3단계의 시험 표본에서 따로 본다.
