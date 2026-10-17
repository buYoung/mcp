# 4단계: 조회 검증

작업일: 2026-10-17. 전 판: `rts4-3b9284a3b42c2917`(2단계 결과). 후 판: `rts4-86c5620af1f181ce`(3단계 결과).

3단계에서 더한 매핑이 조회 결과를 나아지게 했는지, 그리고 다른 단어의 조회 결과를 해치지 않았는지 확인했다. 전 판은 평가 세트 224단어의 텍스트 응답이 `rts3-48356e35e1f1ea28`과 모두 같으므로(2단계), 전 판의 점수가 곧 1단계 기준 점수다.

## 1. 확인한 것

| 확인 | 방법 | 결과 파일 |
| --- | --- | --- |
| 평가 세트 점수 | `sense-eval/score_eval_set.py --sense-dataset <판>/dataset.sqlite3` | `sense-eval/results/rts4-86c5620af1f181ce.json` |
| 매핑한 단어의 조회 결과 | `sense-eval/compare_lookups.py --mapped-by <후 판>/mappings.json` | 보존하지 않음 |
| 매핑한 주제에 속한 다른 단어 | `sense-eval/compare_lookups.py --mapped-by <후 판>/mappings.json --affected` | 보존하지 않음 |
| 무작위 표본 | `sense-eval/compare_lookups.py --sample 3000` | 보존하지 않음 |
| 결함 단어와 예시 단어 | `sense-eval/compare_lookups.py --words-file`. [알려진 결함](../known-defects.md), [README.sense.ko.md](../../../apps/codemap-search/experiments/related-terms-poc/README.sense.ko.md), 13단계 재검토에 나오는 20단어 | 보존하지 않음 |
| 크기와 적재 시간 | 파일 크기와 `SenseIndex.load` 시간 | |

`compare_lookups.py`는 단어마다 CLI 기본값으로 두 판을 따로 조회해 비교한다. 이번 단계에서 비교 항목에 뜻 순서와 관련어 위치를 더했다(5절).

## 2. 평가 세트

| 집단 | 단어 | 적중 | 주 분야 적중 | 첫 뜻 적중 | 뜻 수 중앙값 | 분야 수 중앙값 | 관련어 수 중앙값 | 관련어 수 평균 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 전체 | 224 | 80.8% → 84.8% | 71.4% → 75.9% | 64.3% → 64.3% | 2 → 2 | 1 → 1 | 124 → 129 | 209.4 → 215.9 |
| `target` | 113 | 85.8% → 93.8% | 71.7% → 80.5% | 85.8% → 85.8% | 1 → 2 | 1 → 1 | 77 → 93 | 97.6 → 105.5 |
| `multi` | 65 | 86.2% → 86.2% | 83.1% → 83.1% | 43.1% → 43.1% | 9 → 9 | 6 → 6 | 371 → 372 | 436.9 → 443.9 |
| `input` | 46 | 60.9% → 60.9% | 54.3% → 54.3% | 41.3% → 41.3% | 2 → 2 | 1 → 1 | 95 → 95.5 | 162.7 → 164.9 |

- 3단계 대상인 `target`만 적중이 올랐다. `multi`와 `input`은 3단계 대상이 아니어서 적중이 그대로이고, 관련어만 조금 늘었다(매핑한 단어가 그 주제의 "같은 주제" 관련어로 더해진다).
- 첫 뜻 적중은 그대로다. 매핑으로 더한 뜻은 가중치가 0이라 원래 뜻 뒤에 나오기 때문이다(2단계 2절).
- 응답이 바뀌지 않은 단어는 224개 중 73개다.

## 3. 평가 세트 단어별 변화

`target` 113단어는 모두 3단계 대상이었고, 그중 25단어에 매핑을 더했다. 이 25단어는 모두 뜻 수와 분야 수가 늘었다. 나머지 199단어는 분야 수가 그대로다.

| 변화 | 단어 |
| --- | --- |
| 적중을 얻음(9) | `jsonp`, `disposition`, `text transform`, `toolbar item`, `vsetattr`, `listpack`, `search bar`, `amt`, `navigation component` |
| 주 분야 적중을 얻음(10) | `disposition`, `text transform`, `toolbar item`, `vsetattr`, `listpack`, `search bar`, `navigation component`, `escape html`, `aio`, `ajax` |
| 적중·주 분야 적중·첫 뜻 적중을 잃음 | 없음 |

- 적중을 얻은 9단어는 맞는 뜻이 둘째(8단어)나 셋째(`amt`) 뜻이다.
- `escape html`, `aio`, `ajax`는 이미 다른 기대 분야로 적중했고, 이번에 주 분야를 얻었다.

여전히 맞히지 못한 단어는 34개다(1단계 43개).

- **`target` 7개**
  - `logo`(`graphics-media` 기대, 현재 `mobile-desktop`), `component template`(`frontend-ui` 기대, 현재 `databases-storage`), `shutting down`(`concurrency-async`·`os-shell` 기대, 현재 `language-constructs`): 3단계 판정에서 연결하지 않았다. 관련어가 없거나 적어 후보 주제가 비었고(`logo`는 후보 1개), 판정에서 그 분야의 대상이 분명하다고 보지 않았다.
  - `test paths`: 3단계 대상 단어는 `testpaths`(pytest 설정 키)였다. 평가 세트의 코드(gutenberg `ASYNC_TEST_PATHS`)는 사이트 상태 검사의 REST 경로 목록이다.
  - `view result`, `commercial`, `detects`: 기대 분야가 일반어(`general-wording`)다. 이 단계에서 다루지 않는다.
  - 이 단어들은 평가 세트의 답을 보고 고치지 않았다. 고치면 평가 세트가 매핑 판정의 기준이 되어 점수가 실제 개선보다 높게 나온다.
- **`multi` 9개, `input` 18개:** 1단계와 같은 단어다. 3단계 대상이 아니다.

## 4. 전체 조회 비교

| 대상 | 단어 | 같음 | 늘기만 함 | 뜻을 얻음 | 뜻이나 관련어를 잃음 | 관련어 증가(바뀐 단어) |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| 매핑한 단어 | 934 | 0 | 934 | 932 | 0 | 중앙값 +23, 최대 +206 |
| 무작위 표본 | 3,000 | 1,844 | 1,156 | 24 | 0 | 중앙값 +2 |
| 매핑한 주제에 속한 다른 단어 | 62,865 | 487 | 62,378 | 11 | 0 | 중앙값 +2, 최대 +83 |

- **매핑한 단어 중 새 뜻이 없는 2단어:** `Docker layer caching`과 `GitHub Releases`. 별칭이 이미 그 주제의 뜻을 갖고 있어, 매핑한 뜻이 그 뜻과 합쳐졌다(5절). 관련어는 늘었다.
- **매핑한 단어 중 상태가 바뀐 2단어:** `NSKeyedArchiver`와 `mamba`. 전 판에서 뜻이 없다고 답하던 단어가 뜻을 갖게 됐다(상태 `no_sense` → `ok`).
- **무작위 표본에서 뜻을 얻은 24단어:** 22개는 매핑한 단어 자신이고, 2개는 매핑한 단어의 다른 형태다(`disallowed` ← `disallow`, `list formatting` ← `list format`).
- **다른 단어 중 뜻을 얻은 11단어:** 매핑한 단어의 복수형이나 별칭이다. `algebraic data types`, `color scales`, `currency formats`, `currency formatting`, `data warehousing`, `dirnames`, `hash collisions`, `heap dumps`, `ingress controllers`, `passkeys`, `sprites`. 조회할 때 기본형으로 넓혀 찾기 때문이다.
- 세 비교 모두 뜻이나 관련어를 잃은 단어가 없다.

## 5. 뜻 순서와 관련어 위치

매핑은 뜻과 관련어를 더하기만 하지만, 이미 있던 뜻의 순서와 관련어가 놓이는 자리는 바뀔 수 있다. `compare_lookups.py`가 다음을 함께 센다.

- **순서 변화:** 두 판 모두에 있는 뜻의 순서가 다르다.
- **첫 뜻 변화:** 전 판의 첫 뜻이 후 판의 첫 뜻이 아니다.
- **자리를 옮긴 관련어:** 두 판 모두에 있는 관련어가 다른 뜻이나 다른 고리(직접, 가까운, 같은 주제)에 놓인다.

| 대상 | 순서 변화 | 첫 뜻 변화 | 관련어가 자리를 옮긴 단어 | 자리를 옮긴 관련어 |
| --- | ---: | ---: | ---: | ---: |
| 매핑한 단어 | 1 | 1 | 1 | 1 |
| 무작위 표본 | 0 | 0 | 8 | 10 |
| 매핑한 주제에 속한 다른 단어 | 4 | 3 | 453 | 754 |

순서가 바뀌는 원인은 별칭 합치기다. 조회는 입력 단어의 여러 표기(별칭)가 가진 뜻을 분야와 주제가 같으면 하나로 합치고, 가중치를 더한다. 매핑으로 더한 가중치 0의 뜻이 별칭의 뜻과 합쳐지면 가중치가 생겨 앞으로 올라온다.

- `Docker layer caching`: 별칭 `Docker layer cache`의 `t64.1` 뜻과 매핑한 뜻이 합쳐져 첫 뜻이 됐다. 직접 관련어 `build cache`가 이 뜻 아래로 옮겨졌다.
- `Spring profiles`, `spring.profiles`: 첫 뜻이 `t94.1`이 됐다.
- `ansi escape codes`: 첫 뜻이 `os-shell`의 `t415`가 됐다.
- `currying`: 순서만 바뀌고 첫 뜻은 같다.

관련어는 응답에 한 번만 놓인다. 모든 뜻의 직접 관련어를 먼저 놓고, 가까운 관련어, 같은 주제 단어 순으로 놓으며, 이미 놓인 단어는 다시 놓지 않는다. 그래서 자리를 옮긴 관련어는 대부분 매핑한 단어 자신이다.

- 매핑한 주제에 속한 다른 단어에서 자리를 옮긴 754개 중 752개가 매핑한 단어다. 매핑한 단어가 조회한 단어의 앞쪽 뜻과 같은 주제가 되어, 그 뜻의 같은 주제 단어로 옮겨 왔다. 예: `X509Certificate` 34단어, `self signed` 31단어, `trust store` 30단어(모두 `t65.4` TLS handshake에 매핑).
- 나머지 2개(`Spring Boot`, `active profile`)는 `Spring profiles`의 첫 뜻이 바뀌면서 옮겨졌다.
- 무작위 표본의 10개는 모두 매핑한 단어다.
- 어느 경우에도 응답 전체로는 관련어를 잃지 않았다.

## 6. 결함 단어와 예시 단어

[알려진 결함](../known-defects.md), [README.sense.ko.md](../../../apps/codemap-search/experiments/related-terms-poc/README.sense.ko.md), 13단계 재검토에 나오는 20단어를 비교했다: `unshare`, `execution`, `idx`, `idxs`, `multinode`, `sourced`, `specs`, `bypass`, `messaging`, `abort signal`, `consoles`, `datasets`, `kinesis streams`, `link preview`, `svelte`, `requireNonNull`, `negative testing`, `data drift`, `Sublime Merge`, `EMFILE`.

- 18단어는 관련어가 늘기만 했다(중앙값 +3.5, 최대 `specs` +22). `bypass`와 `svelte`는 같다. 뜻을 얻은 단어, 뜻 순서가 바뀐 단어, 관련어가 자리를 옮긴 단어는 없다.
- 그래서 결함 문서의 증상은 그대로다. 예를 들어 `unshare`는 관련어가 4개 늘었지만 뜻과 그 순서는 같다.
- `README.sense.ko.md`의 예시 수치는 기본 데이터셋(`rts3-48356e35e1f1ea28`) 기준이라 그대로 둔다. 후 판으로 조회하면 `unshare` 61 → 65, `'abort signal' idx` 972 → 983이다.
- 13단계에서 원래 제안된 분야에 연결하지 않은 5단어(`data drift`, `requireNonNull`, `Sublime Merge`, `EMFILE`, `negative testing`)도 관련어가 1~2개 늘었을 뿐, 뜻은 그대로다.

## 7. 기본 데이터셋

`sense_lookup.py`의 기본 데이터셋(`DEFAULT_SENSE_DATASET`)은 `rts3-48356e35e1f1ea28` 그대로 두었다. 후 판은 `--sense-dataset`로 지정해 쓴다. 이 절은 4단계 당시의 판단이다. 아래 스크립트를 고치고 기본값을 바꾼 것은 [5단계](05-default-dataset.md)다.

```sh
python3 apps/codemap-search/experiments/related-terms-poc/sense_lookup.py \
  --sense-dataset apps/codemap-search/experiments/related-terms-poc/datasets/rts4-86c5620af1f181ce/dataset.sqlite3 token retry
```

바꾸지 않은 이유는, 기본 데이터셋을 직접 읽는 word-eval 스크립트가 `rt-sense/4` 판을 읽지 못하기 때문이다.

| 스크립트 | 문제 |
| --- | --- |
| `word-eval/aggregate_units.py`(`load_dataset`) | `senses` 표를 조인한다. `merge_tag_units.py`, `prepare_gap_words.py`도 이 함수를 쓴다 |
| `word-eval/dataset_coverage.py`(`dataset_fields`) | `senses` 표를 조인한다 |
| `word-eval/prepare_gap_words.py`(`sense_rows`) | persona마다 개념이 있다고 가정한다. 개념이 없는 persona에서 실패할 수 있다 |

이 스크립트들은 단어 평가 결과로 빠진 단어를 찾아 `add-entries`로 넣는 흐름에 쓰이고, `add-entries`는 어차피 `rt-sense/3` 판만 받는다. 기본값을 바꾸려면 이 스크립트들이 `sense_lookup.py`의 `PERSONA_QUERIES`처럼 스키마에 맞는 질의를 쓰도록 고쳐야 한다.

## 8. 크기와 적재 시간

| 판 | 크기 | 적재 시간 |
| --- | ---: | ---: |
| `rts4-3b9284a3b42c2917` | 48,566,272바이트 | 약 1.34~1.44초 |
| `rts4-86c5620af1f181ce` | 48,631,808바이트 | 약 1.34~1.44초 |

같은 때 같은 장비에서 `SenseIndex.load`를 여러 번 잰 범위다. 2단계의 적재 시간(2.86초, 3.37초)과는 잰 때의 장비 부하가 달라 직접 비교하지 않는다.

## 9. 한계

- 평가 세트의 `target`에 최근 단계 단어는 5개뿐이다. 최근 단계 단어(매핑 303개)의 개선은 평가 세트로 거의 재지 못하고, 4절의 "늘기만 함"으로만 확인했다.
- 평가 세트는 분야 단위로만 맞았는지 본다. 매핑한 주제가 그 분야 안에서 맞는 주제인지는 재지 않는다.
- 조회 비교는 뜻과 관련어를 잃지 않았는지 보는 것이다. 늘어난 관련어가 쓸모 있는지는 보지 않았다.
