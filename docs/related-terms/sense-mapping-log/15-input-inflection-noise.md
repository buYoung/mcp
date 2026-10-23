# 15단계: 입력 해석의 기본형 잡음 조사

작업일: 2026-10-17. 조사만 했고 코드와 데이터셋은 바꾸지 않았다. 데이터셋: `rts4-710c8ba0d79a776a`(조사 당시 기본값).

[12단계](12-sense-data-fixes.md)에서 틀린 뜻으로 판정했지만 데이터로 고칠 수 없어 그대로 둔 뜻이 있다. 기호가 붙은 입력이 다른 단어의 기본형으로 읽혀서 생긴 뜻이다(`--seed` → `see`, `:checked` → `check`). 이 단계에서는 원인을 찾고, 조회 규칙을 바꾸는 안 세 가지의 영향을 쟀다.

## 1. 원인

입력은 두 단계를 거쳐 사전 단어로 풀린다.

| 단계 | 코드 | 하는 일 |
| --- | --- | --- |
| 기본형 후보 | `lookup-v3/src/morphology.rs`의 `inflection_bases`, `lookup-v3/src/related.rs`의 `find_phrase` | 입력의 마지막 단어에서 `-s`, `-es`, `-ies`, `-ing`, `-ed`를 떼어 낸 후보를 만들고, 사전에 있는 후보를 모두 `lemma` 일치로 돌려준다. `-ing`·`-ed`는 e를 붙인 형태, 뗀 형태, 겹자음을 하나 지운 형태를 차례로 시도한다 |
| 입력 단어 고르기 | `category_lookup.py`의 `same_word`, `select_inflections` | 일치 가운데 입력 자신으로 볼 것을 고른다 |

`same_word`는 입력과 키가 다른 일치를 "기호가 남은 다른 토큰"으로 보고 버린다. 그런데 기본형 일치는 기본형이 평범한 영어 단어이기만 하면 받아들인다. 이 비대칭 때문에 다음 일이 생긴다.

- `--seed`의 키는 `--seed`다(`word_key`는 앞뒤 기호를 남긴다). 같은 철자 단어 `seed`는 키가 달라 버리고, `seed`에서 `-ed`를 떼고 e를 붙인 `see`는 받아들인다. 그래서 `--seed`는 `seed`의 뜻은 받지 못하고 `see`의 뜻(문서 상호 참조, 시각 지각 등)을 받는다.
- 자기 뜻이 있는 단어는 기본형으로 넓히지 않는 규칙(exact-first)이 있다. 그러나 `--seed`처럼 다른 단어를 거쳐서만 읽히는 입력에는 적용되지 않는다.
- 근거 검사(`InflectionEvidence.rank`)는 영문자로만 된 입력만 본다. 기호가 붙은 입력은 근거 점수가 0이 되고, 다른 후보가 없으면 근거 없이도 기본형이 남는다.

12단계에서 그대로 둔 틀린 뜻 37개는 다음과 같이 나뉜다.

| 종류 | 뜻 | 예 |
| --- | ---: | --- |
| 기호가 붙은 입력이 기본형으로 읽힘 | 32 | `--seed`, `--seed=`, `:checked`, `.includes(`, `__extends`, `${string}`(→ `str`), `--profiling`, `--cpus` |
| 판정 묶음에서만 생긴 기본형 | 2 | `seed` → `see` |
| 평범한 단어가 기본형으로 읽힘 | 1 | `yours` → `your` |
| 단어 자신의 뜻 | 2 | `ch`, `group id` |

`seed`는 자기 뜻이 있는 단어여서 CLI에서는 `see`로 넓혀지지 않는다. 판정 묶음을 만든 스크립트가 exact-first 없이 조회했기 때문에 이 뜻이 묶음에 들어갔다. 2절에 그 영향을 적었다.

## 2. 판정 묶음의 exact-first 누락

12단계의 M3 후보를 뽑은 스크립트와 N4 묶음 스크립트는 `sense_lookup.lookup`을 기본 인자로 불렀다. 이 함수의 `is_exact_first` 기본값은 `False`라서, CLI와 달리 자기 뜻이 있는 단어도 기본형의 뜻까지 함께 판정 대상이 됐다. N6 묶음은 조회를 거치지 않고 입력 연결에서 바로 만들었으므로 해당하지 않는다.

이 때문에 판정한 단어가 아니라 그 기본형 단어의 데이터를 고친 수정 항목이 있는지 점검했다.

| 항목 | 수 |
| --- | ---: |
| 고친 단어가 판정한 단어와 다른 수정 | 58 |
| 그 가운데 고친 단어가 CLI 응답에도 나오는 것 | 22 |
| 그 가운데 고친 단어가 CLI 응답에 나오지 않는 것(exact-first가 감추는 기본형) | 36 |

36개를 모두 읽었다. 대부분은 `circular imports` → `circular import`, `finalizers` → `finalizer`처럼 같은 용어의 단수·복수 쌍이어서, 복수형에서 틀린 뜻은 단수형에서도 틀리다. 기본형이 다른 단어인 경우는 두 가지다.

- `marked` 판정으로 `mark` 자신의 뜻을 Stateful stream processing에서 Source text positions로 옮겼다. 이 뜻의 관련어가 `bookmark`, `vim mark` 같은 편집기 위치 표시라 `mark`에게도 맞는 수정이다.
- `partitioned` 판정으로 `split`, `unzip` 등이 Bundle code splitting 뜻에 붙인 관련어 `partition`을 떼어냈다. 일반 동사가 붙인 약한 연결이라 `partition`에게도 맞는 수정이다.

기본형 단어를 망가뜨린 수정은 찾지 못했다. 이후 판정 묶음은 CLI와 같은 조건(exact-first 켬)으로 만들어야 한다.

## 3. 조회 규칙을 바꾸는 안

PoC 코드는 바꾸지 않고, 같은 프로세스에서 `category_lookup`의 함수만 바꿔 끼워 비교했다. 대상은 평가 세트 224단어, 무작위 표본 3,000단어, 12단계에서 판정한 2,649단어다.

| 안 | 내용 |
| --- | --- |
| `plain` | 입력이 영문자와 공백만으로 된 경우에만 기본형 일치를 받는다 |
| `symbol-key` | 입력의 키(`word_key`)에 기호가 남는 경우에만 기본형 일치를 버린다. 키에서 구분자가 지워지는 `virtual_hosts`, `end-to-end testing`은 지금처럼 기본형을 받는다 |
| `evidence` | 입력 자신이 뜻이 있는 단어이면, 형태 연결이나 관계 같은 데이터 근거가 있는 기본형만 받는다 |

| 안 | 평가 세트 적중·첫 뜻 적중 | 표본에서 바뀐 단어 | 판정 단어에서 바뀐 단어 | 답을 잃은 단어 |
| --- | --- | ---: | ---: | ---: |
| `plain` | 그대로(86.2%, 71.0%) | 14 | 46 | 0 |
| `symbol-key` | 그대로 | 9 | 37 | 0 |
| `evidence` | 그대로 | 72 | 173 | 0 |

- `plain`은 맞는 기본형까지 버린다. `end-to-end testing` → `end to end test`, `key-value pairs` → `key value pair`, `virtual_hosts` → `virtual host`가 그 예다.
- `evidence`는 더 많이 버린다. `configuration files` → `configuration file`, `relationships` → `relationship`, `conducted` → `conduct`처럼 데이터에 형태 연결이 없는 맞는 복수형·과거형이 많다.
- `symbol-key`는 12단계에서 그대로 둔 기호 입력 32개를 모두 고친다. 예를 들어 `--seed`의 첫 뜻은 Reproducible builds, `:checked`는 Checkable controls, `/posts`는 Content URL identifiers가 된다.

### `symbol-key`의 사전 전체 영향

사전 단어 17만 6천 개 가운데 키에 기호가 남는 단어는 38,129개다. 이 가운데 534개(사전의 0.3%)의 입력 해석이 바뀌고, 295개는 맨 위 뜻이 바뀐다. 답을 잃는 단어는 없다.

맨 위 뜻이 바뀐 295단어를 Claude Sonnet 5.5가 블라인드로 비교했다. 어느 쪽이 지금 규칙인지 감추고, 두 맨 위 뜻 가운데 코드에서 그 철자의 뜻에 더 맞는 쪽을 골랐다.

| 결과 | 단어 |
| --- | ---: |
| `symbol-key`가 나음 | 132 |
| 지금 규칙이 나음 | 115 |
| 차이 없음 | 48 |

- 좋아진 예: Structurizr 지시문 `!adrs`·`!docs`(기계어 → 아키텍처 다이어그램), `#[cached]`(HTTP 캐시 검증 → 메모이제이션), `${string}`(문자열 보간 → TypeScript 타입 변환), `%files`(경로 → RPM 배포판)
- 나빠진 예: `--extras`(선택적 빌드 기능 → 심볼 탐색), `--compressed`(압축 → 사용 예), `--jobs`(작업 큐 → PostgreSQL 도구), `--lines`(줄 처리 → 커버리지 기준)

기호가 붙은 입력은 자기 뜻의 품질이 단어마다 다르다. 그래서 기본형의 뜻이 더 맞는 단어도 비슷하게 많다.

### 평범한 단어의 잘못된 기본형

기호가 없는 단어도 굴절이 아닌데 접미사가 떨어지는 경우가 있다. 자기 뜻이 없어서 기본형으로 넓혀지는 영문 소문자 단어는 6,527개다. 대부분은 맞는 기본형이다(`abbreviations` → `abbreviation`, `activating` → `activate`). 틀린 예는 `cstring` → `cstr`, `fmtstring` → `fmtstr`(`string`의 `-ing`), `yours` → `your`다. 모음 없는 기본형 같은 단순한 규칙으로는 가려지지 않는다. 모음 없는 기본형 42개는 `cmds` → `cmd`, `ctxs` → `ctx`처럼 대부분 맞다.

## 4. 결론

- 원인은 `same_word`가 같은 철자 단어는 버리고 기본형은 받는 비대칭이다.
- 세 안 가운데 `symbol-key`가 가장 좁고, 12단계에서 남은 기호 입력 잡음을 모두 고친다. 그러나 사전 전체에서는 좋아지는 단어와 나빠지는 단어가 비슷하다(132 대 115). 그래서 이 단계에서는 코드를 바꾸지 않았다.
- 판정 묶음을 만드는 스크립트는 CLI와 같은 조건으로 조회해야 한다. 이번 수정 가운데 이 때문에 기본형 단어를 망가뜨린 것은 없다.

## 5. 파일

시험 스크립트(`trial_inflection_rules.py`), 비교 결과, 블라인드 비교 묶음과 판정은 저장소 밖 작업 폴더에 있고 저장소에 넣지 않았다.
