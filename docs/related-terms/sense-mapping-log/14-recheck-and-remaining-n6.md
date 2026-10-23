# 14단계: 판정 재검증과 남은 여러 단어 표현 수정

작업일: 2026-10-17. 데이터셋: `rts4-e5083ab344e1336d` → `rts4-71cffacbc6f7c0d8`(재검증을 통과한 수정) → `rts4-912fe1c9f62131b2`(빠진 뜻 연결) → `rts4-301d57d419380d08`(남은 여러 단어 표현 수정). 12단계에서 만든 `rts4-78debdbe7f7bab0b`, `rts4-710c8ba0d79a776a`를 대신한다. 검증 뒤 사용자 확인을 받아 `rts4-301d57d419380d08`을 기본값으로 바꿨다(6절).

사용자가 고른 다음 작업 가운데 데이터를 고치는 두 가지다. 입력 해석 잡음 조사는 [15단계](15-input-inflection-noise.md)에 있다.

| 작업 | 내용 |
| --- | --- |
| 판정 재검증 | 12단계에서 틀렸다고 판정한 뜻 1,012개를 한 번 더 독립 판정한다. 두 판정이 엇갈린 수정은 하지 않는다 |
| N6 나머지 | 12단계에서 판정하지 않은 여러 단어 표현 후보를 같은 방식으로 판정하고 고친다 |

판정은 12단계와 같이 Claude Sonnet 5.5가 했다. 묶음 작성, 계획 작성, 검증은 Claude Opus 5.5가 했다.

## 1. 판정 재검증

재검증 판정자는 12단계 판정자와 같은 지침과 같은 근거 묶음을 받았다. 앞 판정은 보지 않았다. 묶음에는 틀렸다고 판정한 뜻만 판정 대상으로 넣었다. 두 판정이 모두 틀렸다고 한 뜻만 고치고, 재검증이 맞다고 한 뜻은 그대로 둔다.

| 작업 | 틀렸다고 판정한 뜻 | 재검증도 틀렸다고 함 | 엇갈림 |
| --- | ---: | ---: | ---: |
| N4 | 11 | 11 | 0 |
| M3 | 212 | 205 | 7 |
| N6 | 789 | 781 | 8 |
| 합계 | 1,012 | 997 | 15 |

엇갈린 15개는 모두 다른 문맥에서는 맞을 수 있는 경우다. 예를 들어 `gradlew`의 래퍼 뜻(Gradle wrapper는 얇은 래퍼다), `defineConfig`의 정적 사이트 생성 뜻(Astro, VitePress도 이 함수를 쓴다), `fast json parser`의 SIMD 처리 뜻(simdjson), `fourth version`의 IPv4 뜻이 있다.

재검증을 통과한 수정만으로 계획을 다시 만들어 출발 판에 적용했다.

| 연산 | 12단계 | 14단계 |
| --- | ---: | ---: |
| `relation_moves` | 893 | 877 |
| `relation_detaches` | 195 | 191 |
| `input_detaches` | 8 | 8 |
| `moves` | 70 | 67 |

이어서 12단계와 같은 N1 연결 233개를 더해 `rts4-912fe1c9f62131b2`를 만들었다.

### 되돌린 수정의 대가

엇갈린 수정을 하지 않으면 원래 데이터로 돌아간다. 그런데 1차 수정이 더 나았던 경우도 있다.

- `Testcontainers`는 단어 자신의 뜻을 Embedded database tests에서 Container integration tests로 옮기는 수정으로만 Container integration tests 뜻을 얻었다. 되돌리자 그 뜻이 없어졌다.
- `env lookup`은 Environment configuration 대신 Dictionary entry operations만 남았다.
- `gradlew`는 Adapter wrappers가 다시 첫 뜻이 됐다.

두 판정이 모두 틀렸다고 할 때만 고치는 규칙의 대가다. 맞는 뜻은 빠진 뜻 연결로 따로 더할 수 있다.

## 2. 남은 여러 단어 표현

### 후보

12단계와 같은 기준으로 `rts4-912fe1c9f62131b2`에서 다시 뽑았다. 다른 단어를 거쳐서만 읽히는 여러 단어 표현 가운데, head가 붙인 뜻보다 같은 head의 다른 뜻이 단어와 0.3 이상 가까운 것이다. 12단계에서 판정한 단어(N4, M3, N6)는 뺐다. 후보는 1,048개다. 12단계 때 남은 1,009개와 수가 다른 것은 기준 판이 바뀌었기 때문이다.

### 판정

| 항목 | 수 |
| --- | ---: |
| 판정한 단어 | 1,048 |
| 판정한 뜻 | 1,054 |
| 틀린 뜻 | 709 |
| 재검증도 틀렸다고 함 | 706 |
| 엇갈림 | 3 |

틀린 비율은 67%로 12단계 N6(78%)보다 낮다. 후보가 차이가 작은 쪽이기 때문이다. 엇갈린 3개는 `array spread`(호출 인자 전개), `feature matrix`(기능 조합 검증 행렬), `raise event`(테스트의 이벤트 발생 단언)다.

고친 예는 다음과 같다.

| 단어 | 틀린 뜻 | 고친 뒤 |
| --- | --- | --- |
| `stackful coroutine` | Tree structure traversal | Cooperative fiber scheduling |
| `verify email` | License activation | 이 뜻이 빠지고 Account registration만 남음 |
| `pure DI` | 셸 분야 뜻 | Software design 분야 뜻 |
| `prevent injection` | 셸 분야 뜻 | Security 분야 뜻 |

계획은 관련어 옮기기 664개와 떼어내기 66개다. `rts4-912fe1c9f62131b2`에 적용해 `rts4-301d57d419380d08`을 만들었다. 관련어 행은 592,793개로 그대로다.

## 3. 검증

### 평가 세트 224단어

| 판 | 적중 | 주 분야 적중 | 첫 뜻 적중 | 맞는 뜻의 평균 순위 |
| --- | ---: | ---: | ---: | ---: |
| `rts4-e5083ab344e1336d`(출발 판) | 85.3% | 78.1% | 70.5% | 1.36 |
| `rts4-710c8ba0d79a776a`(12단계) | 86.2% | 79.0% | 71.0% | 1.36 |
| `rts4-301d57d419380d08`(14단계) | 86.2% | 79.0% | 71.0% | 1.36 |

12단계 판과 비교해 적중이 바뀐 단어는 없다. 응답 글자가 바뀐 단어는 9개다.

### 12단계 판과의 비교

| 대상 | 단어 | 응답이 같음 | 뜻을 잃음 | 맨 위 뜻이 바뀜 |
| --- | ---: | ---: | ---: | ---: |
| 무작위 표본 | 3,000 | 2,965 | 15 | 12 |
| 남은 여러 단어 표현과 되돌린 단어 | 1,063 | 344 | 708 | 534 |

- 표본에서 뜻을 잃은 15단어는 모두 고친 결과다. 예: `pure DI`, `prevent injection`, `systemctl disable`(편집기 → 설정), `golang ide`(관측 도구 → 편집기)
- 두 번째 대상이 잃은 뜻은 714개다. 700개는 틀렸다고 판정한 뜻이고, 맞다고 판정한 뜻은 없다. 나머지 14개는 1절에서 되돌린 수정 때문이다. `Testcontainers`, `env lookup` 같은 단어와, `defineConfig`를 거쳐 읽히던 `conditional config`, `config autocomplete`, `satisfies UserConfig`가 여기에 해당한다.

### 출발 판과의 비교(무작위 표본 3,000단어)

| 항목 | 단어 |
| --- | ---: |
| 응답이 같음 | 2,444 |
| 더해지기만 함 | 258 |
| 뜻을 잃음 | 37 |
| 맨 위 뜻이 바뀜 | 34 |

### 알려진 결함과 예시 단어

12단계에서 고친 결함(K1, K3~K5, K6의 `specs`)은 그대로 고쳐져 있다. `ReLU`, `HTTP/2 push`, `tcp state`, `detects`의 응답도 12단계 판과 같다.

## 4. 남은 것

- M3는 앞의 뜻이 뒤의 뜻보다 유사도가 0.2 이상 낮은 단어만, N6은 0.3 이상 가까운 다른 뜻이 있는 표현만 봤다. 차이가 더 작은 단어의 틀린 뜻은 찾지 않았다.
- 1절에서 되돌린 수정 가운데 맞는 뜻을 잃은 단어(`Testcontainers`, `env lookup` 등)는 빠진 뜻 연결로 따로 더할 수 있다.
- 입력 해석의 기본형 잡음은 [15단계](15-input-inflection-noise.md)에서 조사만 했다.

## 5. 파일

| 위치 | 내용 |
| --- | --- |
| `datasets/rts4-71cffacbc6f7c0d8/` | 재검증을 통과한 수정 계획 `revisions.json`(항목마다 판정한 단어와 이유), `revision-outcome.json`, `build-report.json` |
| `datasets/rts4-912fe1c9f62131b2/` | N1 연결 `mappings.json`, `build-report.json` |
| `datasets/rts4-301d57d419380d08/` | 남은 여러 단어 표현의 수정 계획 `revisions.json`, `revision-outcome.json`, `build-report.json`, `dataset.sqlite3` |
| `sense-eval/results/rts4-301d57d419380d08.json` | 평가 세트 채점 결과 |

판정 묶음, 판정과 재검증 결과, 엇갈린 판정 목록은 저장소 밖 작업 폴더에 있고 저장소에 넣지 않았다.

## 6. 기본값 전환

- `sense_lookup.py`의 `DEFAULT_SENSE_DATASET`을 `rts4-301d57d419380d08`로 바꿨다. 이 판의 `dataset.sqlite3`(48.6MB, SHA-256 `cc7aab0b255845023c473b81414d8ad3eb04849a24616fc86a746ecaa73a1690`)를 저장소에 넣었다. 중간 판 `rts4-71cffacbc6f7c0d8`, `rts4-912fe1c9f62131b2`는 입력 파일만 넣었다.
- 이전 기본값 `rts4-710c8ba0d79a776a`의 `dataset.sqlite3`는 이미 저장소 기록에 있으므로 그대로 두었다.
- 바꾼 뒤 `verify_input_bindings.py`, `verify_reviewed_relations.py`, `verify_sense_additions.py`, `verify_spelling_aliases.py`가 모두 통과했다.
- `README.sense.ko.md`의 예시(`unshare` 관련어 24개, `'abort signal' idx` 988개, `activate`의 뜻 미배정 관련어 `ReLU`)는 새 기본값에서도 그대로다. 판 설명과 판 표만 고쳤다.
- 알려진 결함 단어 `multinode`(첫 뜻 Runtime host nodes, 뜻 15개), `execution`, `idx`, `specs`, `unshare`의 첫 뜻과 뜻 수가 12단계 판과 같다.
- [데이터셋 생성 과정](../pipeline.md)에서 17단계 결과를 새 판으로 바꾸고 남은 여러 단어 표현 수정을 18단계로 더했다. [알려진 결함](../known-defects.md)과 [단어 평가](../word-evaluation.md)의 현재 기본값 표기도 고쳤다.
