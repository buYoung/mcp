# 12단계: 틀린 뜻·빠진 뜻·알려진 결함 수정

작업일: 2026-10-17. 데이터셋: `rts4-e5083ab344e1336d`(기본값) → `rts4-78debdbe7f7bab0b`(틀린 뜻 수정) → `rts4-710c8ba0d79a776a`(빠진 뜻 연결). 검증 뒤 사용자 확인을 받아 `rts4-710c8ba0d79a776a`를 기본값으로 바꿨다(7절). [14단계](14-recheck-and-remaining-n6.md)에서 이 판정을 재검증해, 두 판정이 엇갈린 수정 15개를 빼고 다시 만들었다.

사용자가 고른 개선 작업 가운데 데이터를 고치는 네 가지다.

| 작업 | 내용 |
| --- | --- |
| N4 | 알려진 결함 K1, K3~K6의 뜻 |
| M3 | 잘못 만든 뜻이 맨 앞에 오는 단어(사전 전체) |
| N6 | 엉뚱한 분야에 붙은 여러 단어 표현 |
| N1 | 흔한 단어에 빠진 뜻 |

판정은 사용자가 정한 대로 Claude Sonnet 5.5가 했다. 후보 추출, 수정 계획 작성, 검증은 Claude Opus 5.5가 했다. 틀린 뜻은 [11단계](11-text-output-and-relation-edits.md)에서 더한 관련어 단위 연산으로 고쳤다. 관련어는 맞는 뜻으로 옮기고, 맞는 뜻이 없으면 뜻에서 떼어낸다. 지우지는 않는다.

## 1. 판정 방식

판정 기준은 이 작업 이력의 [정한 기준](README.md#정한-기준)과 같다. 단어가 실제 코드 문맥에서 그 대상을 가리키면 맞는 뜻이다. 다른 분야가 더 주된 분야라는 것은 틀린 이유가 아니다. 애매하면 맞는 뜻으로 둔다.

판정자는 묶음 파일(75~80단어)과 지침만 읽는다. 묶음에는 뜻마다 다음 근거가 들어 있다.

- 단어 자신이 그 뜻으로 생성됐는지와 그때의 관련어
- 그 뜻을 준 head 단어, head가 그 뜻에 함께 둔 관련어, head의 다른 뜻 가운데 단어와 가까운 8개
- 생성 횟수와, 단어와 그 뜻 주제 단어의 유사도

틀린 뜻은 근거마다 고칠 방법을 하나씩 고른다.

| 근거 | 고치는 방법 |
| --- | --- |
| head가 관련어로 붙인 뜻 | head의 다른 뜻 가운데 맞는 것이 있으면 관련어를 그 뜻으로 옮긴다(`relation_moves`). 없으면 떼어낸다(`relation_detaches`) |
| 관련어 없이 검토로만 만든 연결 | 연결을 끊는다(`input_detaches`) |
| 단어 자신의 뜻 | 관련어가 단어의 다른 뜻에 속하면 그 뜻으로 옮긴다(`moves`). 아니면 그대로 둔다(`keep`) |

## 2. 후보와 판정 결과

| 작업 | 후보 | 판정한 단어 | 판정한 뜻 | 틀린 뜻 |
| --- | --- | ---: | ---: | ---: |
| N4 | K1·K3~K6에 적힌 9단어 | 9 | 13 | 11 |
| M3 | 사전의 모든 단어(17만 6천 개) 가운데, 뜻 순서 규칙 A가 앞에 둔 뜻이 뒤에 둔 어떤 뜻보다 단어와의 유사도가 0.2 이상 낮은 단어. 그 뒤 뜻보다 앞에 있는 뜻을 6개까지 판정했다 | 1,441 | 1,888 | 212 |
| N6 | 다른 단어를 거쳐서만 읽히는 여러 단어 표현 가운데, head가 붙인 뜻보다 같은 head의 다른 뜻이 단어와 0.3 이상 가까운 것 2,009개. 차이가 큰 순으로 1,000개를 판정했다. 평가 세트 단어는 차이와 관계없이 넣었다 | 1,000 | 1,016 | 789 |

N6은 후보를 "head가 다른 뜻에 붙였다"는 오류 모양으로 골랐기 때문에 틀린 비율(78%)이 높다. Claude Opus 5.5가 N6 판정 18개와 M3 첫 묶음을 표본으로 읽었고, 판정은 타당했다.

고친 예는 다음과 같다.

| 단어 | 틀린 뜻 | 고친 것 |
| --- | --- | --- |
| `ReLU`, `sigmoid` | License activation | `activation`이 붙인 관련어를 Neural network layers로 옮기고, `activate`가 붙인 관련어는 떼어냈다 |
| `HTTP/2 push` | Repository transport(Git 푸시) | `push`가 붙인 관련어를 HTTP/2 frames로 옮겼다 |
| `encrypted chat` | Browser end-to-end tests | `end to end encryption`이 붙인 관련어를 Encryption operations로 옮겼다 |
| `HTTP header` | Request rate limits, CDN delivery 등 5개 | `X-RateLimit-Limit`, `xff` 같은 head가 붙인 관련어를 각 head의 HTTP headers 뜻으로 옮겼다. `HTTP headers`의 HTTP headers 뜻에 이 head들이 모인다 |
| `pseudo element` | CSS selector specificity | 단어 자신의 뜻(관련어 `::before`, `::after` 등)을 CSS pseudo-elements로 옮겼다 |

### N1: 빠진 뜻

| 항목 | 내용 |
| --- | --- |
| 후보 | 다른 단어의 관련어로 가장 많이 나오는 2,000단어 가운데, 뜻이 없는 주제가 가장 가까운 기존 뜻과 거의 같거나(차이 0.03 이내) 더 가까운 단어와, 지정한 9단어(`second`, `detects` 등)를 합친 656단어다. 단어마다 뜻이 없는 주제를 가까운 순으로 5개까지 보였다 |
| 판정 | 단어가 실제 코드 문맥에서 그 주제의 대상을 가리키면 연결한다. 이름만 비슷하거나 기존 뜻과 겹치는 주제는 연결하지 않는다. 애매하면 연결하지 않는다 |
| 결과 | 206단어에 연결 233개. 이 가운데 19개는 사전에 표제어가 없는 단어여서, 그 주제의 가장 가까운 단어를 거쳐 연결했다(`via`). 예: `async`, `lambda`, `mkdir`, `detects`(Detection wording), `second`·`third`(순서 위치) |

검토로 연결한 뜻은 생성 횟수가 0이라 뜻 순서 규칙 A에서 그 단어의 뜻 가운데 맨 뒤에 놓인다.

## 3. 수정 계획과 적용

판정을 계획 파일 하나로 모았다. 항목마다 판정한 단어(`judged_word`), 판정 이유(`reason`), 판정 항목 id(`entry`)를 남겼다.

| 연산 | 항목 |
| --- | ---: |
| `relation_moves` | 893 |
| `relation_detaches` | 195 |
| `input_detaches` | 8 |
| `moves` | 70 |

- 같은 관련어를 여러 판정이 고치면 먼저 나온 판정을 따랐다(N4 → M3 → N6 순). 실제로 겹친 것은 `replicated` 판정 하나다. head가 붙인 관련어가 `replicated`가 아니라 `replicate`여서, 4건 가운데 2건은 계획 검사가 거부했고 2건은 앞의 수정과 같은 관련어라 뺐다.
- 틀린 뜻 가운데 37건은 그대로 두었다(`keep`).
  - 34건은 입력 해석이 기호가 붙은 입력을 다른 단어로 읽어서 생긴 뜻이다. 예: `--seed` → `see`, `:checked` → `check`, `.includes(` → `include`, `__extends` → `extend`. 이 뜻은 `see`, `check` 같은 단어 자신의 뜻이어서 데이터를 고치면 그 단어가 망가진다. 입력 해석의 문제라 이 단계에서 고치지 않았다.
  - 3건은 단어가 직접 만든 뜻인데 옮길 다른 뜻이 없다. `ch`의 Replica failover, `group id`의 Allocator heap profiles, `yours`의 Account provisioning이다.

`revise_topics.py`로 기본값에 적용한 결과는 다음과 같다.

| 항목 | 기본값 | `rts4-78debdbe7f7bab0b` |
| --- | ---: | ---: |
| `relations` | 592,793 | 592,793 |
| `personas` | 44,223 | 44,210 |
| `input_senses` | 334,444 | 334,232 |

- 관련어와 입력 연결 1,096개가 가리키는 persona가 바뀌었다. 관련어 행은 하나도 지우지 않았다.
- `moves`로 옮긴 persona는 73개다. 이 가운데 13개는 옮겨 간 주제에 같은 단어의 뜻이 이미 있어 합쳐졌다.
- 이어서 `add_topic_mappings.py`로 N1 연결 233개를 더해 `rts4-710c8ba0d79a776a`를 만들었다.

## 4. 검증

### 평가 세트 224단어

| 지표 | 기본값(규칙 A) | `rts4-710c8ba0d79a776a` |
| --- | ---: | ---: |
| 적중 | 85.3% | 86.2% |
| 주 분야 적중 | 78.1% | 79.0% |
| 첫 뜻 적중 | 70.5% | 71.0% |
| 맞는 뜻의 평균 순위 | 1.36 | 1.36 |

- 적중을 잃은 단어는 없다.
- 새로 적중한 단어는 2개다. `detects`는 N1 연결로 Detection wording(`general-wording`) 뜻이 생겼다. `tcp state`는 N6 수정으로 Buffered stream I/O 대신 Connection lifecycle(`networking`)이 첫 뜻이 됐다.
- 응답 글자가 바뀐 단어는 105개다. 대부분 옮긴 관련어가 다른 단어의 관련어 목록에서 빠지거나 더해진 것이다.

### 판정한 단어 2,649개

판정하거나 연결한 단어를 기본값과 새 판으로 한 단어씩 조회해 비교했다(`sense-eval/compare_lookups.py`).

| 항목 | 단어 |
| --- | ---: |
| 응답이 같음 | 908 |
| 더해지기만 함 | 373 |
| 뜻을 잃음 | 968 |
| 맨 위 뜻이 바뀜 | 784 |

잃은 뜻은 996개다.

- 978개는 틀렸다고 판정한 뜻이다.
- 맞다고 판정했는데 잃은 뜻은 없다.
- 나머지 18개는 다른 판정의 수정을 따라간 것이다. 판정한 단어와 같은 단어의 다른 형태이거나(`strided` → `stride`, `reduced` → `reduce`, `HTTP header` → `HTTP headers`), 옮긴 persona를 거쳐 뜻을 읽던 단어다(`execution`을 옮기자 `execution context`도 따라감).

떼어낸 관련어는 head 단어의 "뜻 미배정 직접 관계어"에 남는다. 예를 들어 `activate`를 조회하면 `ReLU`, `sigmoid`, `GELU`가 그 줄에 나온다.

### 무작위 표본 3,000단어

| 항목 | 단어 |
| --- | ---: |
| 응답이 같음 | 2,454 |
| 더해지기만 함 | 251 |
| 뜻을 잃음 | 27 |
| 맨 위 뜻이 바뀜 | 26 |
| 관련어를 잃음 | 295 |

- 뜻을 잃은 27단어는 모두 고친 결과다. 예: `bytes/sec`는 인증 분야 뜻 대신 시간 분야 뜻을, `church booleans`는 수학 뜻 대신 함수형 프로그래밍 뜻을, `OAuth code`는 언어 도구 뜻 대신 인증 뜻을 얻었다.
- 관련어를 잃은 295단어는, 틀린 주제에서 빠진 단어가 그 주제의 다른 단어 목록에서도 빠진 것이다. 예를 들어 `/Applications`의 Server route handlers 뜻에서 `execution`이 빠졌다.

### 알려진 결함

| ID | 새 판의 상태 |
| --- | --- |
| K1 | `unshare`, `isolate`, `tenant isolation`에서 XML parsing 뜻이 빠졌다. `unshare`의 두 번째 뜻은 Linux container isolation이다. `namespace`의 XML parsing 뜻은 XML 이름공간이라 그대로다 |
| K3 | `execution`의 일반 실행 관련어 16개가 Execution wording(`general-wording`) 뜻 아래 나온다. `web-backend` 뜻은 없다 |
| K4 | `idx`, `idxs`에서 Plural form selection 뜻이 빠졌다. `idx`의 첫 뜻은 Indexed element access다 |
| K5 | `multinode`의 단어 자신의 뜻 4개를 Runtime host nodes로 옮겼다. 첫 뜻은 Runtime host nodes이고, 뜻은 19개에서 15개로 줄었다. 다른 단어를 거쳐 오는 구조 뜻(Linked list traversal, AST visitors 등)은 판정 대상이 아니어서 남아 있다 |
| K6 | `sourced`의 두 뜻은 모두 맞다고 판정해 그대로다. `specs`의 Log ingestion pipelines 뜻은 Technical specifications로 옮겼다 |

K9(`role`, `tokens`의 뜻 순서)는 뜻 순서 규칙의 문제라 그대로다.

## 5. 남은 것

- N6 후보 가운데 차이가 작은 1,009개는 판정하지 않았다.
- M3는 앞의 뜻이 뒤의 뜻보다 유사도가 0.2 이상 낮은 단어만 봤다. 차이가 작은 단어의 틀린 뜻은 찾지 않았다.
- 입력 해석이 기호 붙은 입력을 다른 단어로 읽는 문제(34건)는 남아 있다.
- 판정은 Claude Sonnet 5.5가 한 번 했다. 표본만 검토했고, 독립 판정을 한 번 더 하지는 않았다.

## 6. 파일

| 위치 | 내용 |
| --- | --- |
| `datasets/rts4-78debdbe7f7bab0b/` | 수정 계획 `revisions.json`, 적용 결과 `revision-outcome.json`, `build-report.json` |
| `datasets/rts4-710c8ba0d79a776a/` | N1 연결 `mappings.json`(연결마다 이유), `build-report.json`, `dataset.sqlite3` |
| `sense-eval/results/rts4-710c8ba0d79a776a.json` | 평가 세트 채점 결과 |

판정 묶음과 결과, 후보 추출 스크립트는 저장소 밖 작업 폴더에 있고 저장소에 넣지 않았다.

## 7. 기본값 전환

- `sense_lookup.py`의 `DEFAULT_SENSE_DATASET`을 `rts4-710c8ba0d79a776a`로 바꿨다. 이 판의 `dataset.sqlite3`(48.6MB)를 저장소에 넣었다. 중간 판 `rts4-78debdbe7f7bab0b`는 입력 파일만 넣었다.
- 이전 기본값 `rts4-e5083ab344e1336d`의 `dataset.sqlite3`는 이미 저장소 기록에 있으므로 그대로 두었다.
- 바꾼 뒤 `verify_input_bindings.py`, `verify_reviewed_relations.py`, `verify_sense_additions.py`, `verify_spelling_aliases.py`가 모두 통과했다.
- `README.sense.ko.md`의 판 설명과 `unshare` 출력 예시를 새 기본값으로 고쳤다. `unshare`의 결과는 XML parsing 뜻이 빠져 관련어 65개에서 24개로 줄었다.
