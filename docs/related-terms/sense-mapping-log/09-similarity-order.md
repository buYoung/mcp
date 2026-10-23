# 9단계: 질의 유사도 순 뜻 정렬

작업일: 2026-10-17. 데이터셋: `rts4-e5083ab344e1336d`(기본값, 바꾸지 않음).

사용자가 "분류는 질의와 가장 비슷한 순서로 정렬"하도록 정했다. 그래서 조회 결과의 뜻 순서를 생성 횟수(persona 가중치)가 아니라 질의와 뜻의 유사도로 정한다. 데이터셋은 바꾸지 않았다.

[10단계](10-sense-order-rule.md)에서 기본 뜻 순서를 규칙 A(생성 횟수 순, 같으면 질의 유사도 순)로 바꿨다. 아래 결함 중 S1(생성 횟수 무시)과 S2(검토로만 연결한 뜻이 맨 앞)는 A에서 생기지 않는다. 유사도 계산(1절)은 A에서도 그대로 쓴다.

## 1. 유사도를 재는 방법

관련어 데이터셋 `rtr1-930791c83bea6132`에는 단어마다 임베딩이 저장되어 있다(`jinaai/jina-embeddings-v2-base-code`, 768차원, int8). 조회할 때 모델을 돌리지 않고 이 벡터만 읽는다. 입력 해석도 이미 이 데이터셋을 쓴다.

- **질의 벡터:** 입력 표기의 벡터. 없으면 입력이 풀린 단어들의 벡터 평균.
- **뜻 벡터:** 그 뜻의 주제에 속한 단어들의 벡터 평균. 질의 자신과 그 표기들은 뺀다. 빼지 않으면 질의만 든 작은 주제가 유사도 1에 가까워진다. 주제에 다른 단어가 없으면 그 뜻의 직접 관계어로 대신한다.
- **유사도:** 두 벡터의 코사인.
- **순서:** 유사도가 큰 뜻부터. 유사도를 잴 수 없는 뜻은 뒤로 가고, 같으면 8단계 규칙(`sense_order`)을 따른다. 질의 벡터가 없으면 8단계 규칙 그대로다.

시제품으로 네 방법을 평가 세트에서 비교하고 가장 나은 것을 골랐다. 순서만 바뀌므로 적중과 주 분야 적중은 모두 같다.

| 방법 | 첫 뜻 적중 |
| --- | ---: |
| 8단계 규칙(생성 횟수, 같으면 주제 단어 수) | 68.8% |
| **주제 단어 평균 벡터(직접 관계어는 주제가 비었을 때만), 채택** | 69.6% |
| 가장 비슷한 단어 3개의 평균 | 68.8% |
| 주제 단어와 직접 관계어를 함께 평균 | 67.4% |
| 가장 비슷한 단어 하나 | 66.5% |

## 2. 바꾼 것

| 파일 | 바꾼 것 |
| --- | --- |
| `sense_lookup.py` | `TermVectors`(관련어 데이터셋의 벡터를 정확 키로 읽어 캐시), `sense_similarity`, `query_vector`를 더했다. `seed_senses`가 벡터를 받으면 유사도 순으로 정렬한다. `lookup`에 `is_similarity_ordered`(기본 켬)와 `vector_dataset`(기본은 관련어 데이터셋)을 더했다. CLI에 `--no-similarity-order`를 더했다(8단계 순서). 주제 벡터의 합은 색인에 캐시한다 |
| `verify_spelling_aliases.py` | CLI 검사에 `--no-similarity-order`를 더했다. 이 검사는 입력 경로가 가짜(`chosen-lexical.sqlite3`)라 벡터를 읽을 수 없다. 비교하는 것이 관련어 집합이라 순서와 관계없다 |

뜻을 주제로 나누지 않는 `--no-split-senses-by-topic`은 이전 순서 그대로다.

## 3. 평가 세트

결과 파일: `sense-eval/results/rts4-e5083ab344e1336d-similarity-order.json`.

| 집단 | 첫 뜻 적중(7단계 → 8단계 → 9단계) |
| --- | --- |
| 전체 | 67.9% → 68.8% → 69.6% |
| `target` | 87.6% → 87.6% → 88.5% |
| `multi` | 50.8% → 52.3% → 50.8% |
| `input` | 43.5% → 45.7% → 50.0% |

- 8단계 대비 첫 뜻 적중을 얻은 단어 16개, 잃은 단어 14개다.
  - 얻음: `prefix`, `jsonp`, `role`, `session`, `base`, `core`, `text transform`, `set selected`, `offsets`, `export`, `operators`, `end index`, `weak`, `condition`, `bytes`, `image url`
  - 잃음: `method override`, `attribute`, `loaders`, `am`, `handler`, `breakpoints`, `tone`, `public`, `option`, `query params`, `filter`, `tokens`, `fragment`, `keys`
- 맞힌 단어에서 맞는 뜻이 놓인 평균 순위는 1.47(7단계) → 1.48(8단계) → 1.56(9단계)이다. 첫 뜻을 틀린 단어에서는 맞는 뜻이 전보다 뒤에 놓인다.

## 4. 조회 영향

같은 판에서 8단계 순서와 유사도 순서로 무작위 표본 3,000단어를 조회했다.

| 항목 | 단어 |
| --- | ---: |
| 뜻이 둘 이상인 단어 | 823 |
| 뜻과 관련어 집합이 같은 단어 | 3,000 |
| 뜻 순서가 바뀐 단어 | 552 |
| 첫 뜻이 바뀐 단어 | 438 |
| 관련어가 다른 뜻 아래로 옮겨 간 단어 | 246(관련어 673개) |

뜻이 둘 이상인 단어의 절반 넘게 첫 뜻이 바뀌었다. 잃거나 얻은 뜻과 관련어는 없다. 관련어는 앞선 뜻에 먼저 놓이므로, 순서가 바뀌면 같은 관련어가 다른 뜻 아래에 나온다.

| 단어 | 8단계 첫 뜻 | 9단계 첫 뜻 |
| --- | --- | --- |
| `tf` | Terraform infrastructure state | TensorFlow training |
| `LuaJIT` | Load and stress testing | Lua language |
| `automatic` | RAII ownership | Automatic behavior wording |
| `AWS CodePipeline` | CI build pipelines | AWS developer tools |
| `__builtin_ctz` | Bit manipulation | Compiler builtin intrinsics |
| `aria-label` | Accessible widget semantics | Form accessibility labels |
| `#fff` | Color channels and spaces | White terminology |
| `Arrays.copyOfRange` | C buffer operations | C buffer operations |

벡터가 있는 범위: 표본 1,000단어 중 질의 벡터가 없는 입력이 15개(1.5%), 뜻 1,716개 중 유사도를 잴 수 없는 뜻이 1개, 직접 관계어로 대신한 뜻이 2개였다. 5개 입력을 한 번에 조회하는 데 걸린 시간은 약 2.4초다(유사도 정렬을 끄면 약 2.0초).

## 5. 결함

| ID | 결함 | 규모와 예 |
| --- | --- | --- |
| S1 | 생성 횟수를 무시한다 | 뜻이 둘 이상인 823단어 중 226단어(27%)는 첫 뜻이 생성 횟수가 가장 많은 뜻이 아니다. `handler`는 13번 생성된 Event emission listeners가 넷째, `attribute`는 11번 생성된 Reflective metadata annotations가 셋째다 |
| S2 | 검토로만 연결한 뜻이 맨 앞에 올 수 있다 | 823단어 중 7단어. `am`의 첫 뜻이 Android app components(가중치 0)이고, 평가 세트의 기대는 `time-locale`이다. 그 밖에 `alpn`, `AsyncWriteExt`, `patch file`, `private subnet`, `Shiny`, `thenCompose`, `twos complement` |
| S3 | 문맥을 보지 못한다 | 질의 표기 하나의 벡터만 보므로, 임베딩이 그 표기에서 주로 떠올리는 뜻이 앞에 온다. `breakpoints`는 디버거 뜻이 앞서고, 평가 세트의 기대는 CSS 반응형 기준점(`web-styling`)이다 |
| S4 | 표기가 닮은 주제와 작은 주제가 앞선다 | 코드용 임베딩이라 같은 글자가 든 단어끼리 가깝다. `tokens`는 Lexical token streams, Design token tooling이 앞서고 인증 토큰은 뒤다. `#fff`는 단어가 하나 더 있는 White terminology가 Color channels and spaces(85개)보다 앞선다 |
| S5 | 근소한 차이로 순서가 갈린다 | 첫째와 둘째 뜻의 유사도 차이는 중앙값 0.076이고, 814건 중 117건(14%)이 0.02 미만이다. `handler`는 0.855, 0.849, 0.823 |
| S6 | 첫 뜻을 틀리면 맞는 뜻이 더 뒤에 놓인다 | 맞는 뜻의 평균 순위 1.48 → 1.56(3절) |
| S7 | 질의 벡터가 없는 입력은 다른 기준으로 정렬된다 | 입력의 약 1.5%. 이 입력은 8단계 순서(생성 횟수)를 따른다 |
| S8 | 저장소 밖의 관련어 데이터셋에 기대는 범위가 넓어졌다 | 입력 해석에 쓰던 `rtr1`(약 430MiB, 로컬에만 있음)을 이제 뜻 정렬에도 쓴다. 없으면 조회가 실패하는 것은 전과 같다 |
| S9 | 텍스트 출력에서 첫 뜻이 보이지 않을 수 있다 | 텍스트 출력은 관련어가 남지 않은 뜻을 쓰지 않는다. 관련어는 모든 뜻의 직접 관계어부터 놓이므로, 첫 뜻의 단어가 다른 뜻에 먼저 놓이면 첫 뜻이 빠진다. 823단어 중 1단어(`patch file`)이고, 8단계 순서에서도 2단어였다. JSON 출력에는 그대로 첫째로 나온다 |

## 6. 고칠 수 있는 방향(시험하지 않음)

- 생성 횟수와 유사도를 함께 쓴다. 예: 생성 횟수가 같은 뜻끼리만 유사도로 정렬(S1, S2 해소, 순서 변화는 줄어듦), 또는 두 값을 섞은 점수.
- 검토로만 연결한 뜻(가중치 0)은 계속 뒤에 둔다(S2).
- 유사도 차이가 작으면(예: 0.02 미만) 생성 횟수로 정한다(S5).
- 문맥(S3)은 질의 표기만으로는 풀 수 없다. 조회에 주변 코드나 다른 입력어를 함께 넘겨야 한다.
