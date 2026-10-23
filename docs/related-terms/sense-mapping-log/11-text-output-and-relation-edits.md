# 11단계: 텍스트 출력의 빈 뜻과 관련어 단위 수정 연산

작업일: 2026-10-17. 데이터셋: `rts4-e5083ab344e1336d`(기본값, 바꾸지 않음).

사용자가 고른 개선 작업(M3, M5, N1, N4, N5, N6) 가운데 코드만 바꾸는 두 가지다. 텍스트 출력에서 뜻이 통째로 빠지던 문제(M5)를 고쳤다. 그리고 데이터 수정 작업(M3, N4, N6)에 쓸 관련어 단위 연산을 `revise_topics.py`에 더했다. 데이터셋은 이 단계에서 바꾸지 않는다.

## 1. 텍스트 출력에서 빠지던 뜻

| 항목 | 내용 |
| --- | --- |
| 증상 | JSON 출력의 첫 뜻이 텍스트 출력에 나오지 않는다 |
| 원인 | `render_text`는 관련어가 하나도 남지 않은 뜻을 건너뛴다. 같은 단어를 한 번만 쓰려는 규칙인데, 관련어가 처음부터 없는 뜻도 함께 건너뛰었다 |
| 고친 것 | 관련어가 처음부터 하나도 없는 뜻은 제목만 쓴다. 앞에서 이미 나온 단어만 있어서 빈 뜻은 지금처럼 쓰지 않는다. `--help`의 텍스트 출력 설명에도 한 문장을 더했다 |
| 범위 | `sense_lookup.py`의 `render_text`만 바뀐다. JSON 출력과 `--format text-by-input`은 그대로다 |

평가 세트 224단어와 무작위 표본 3,000단어(합쳐서 3,237단어)를 한 단어씩 조회해, JSON의 첫 뜻 제목이 텍스트에 있는지 확인했다.

| 판 | 첫 뜻이 텍스트에 없는 단어 |
| --- | ---: |
| 고치기 전 | 2 (`affine types`, `in memory`) |
| 고친 뒤 | 0 |

두 단어의 첫 뜻은 단어 자신 말고는 단어가 없는 주제다. 고친 뒤에는 이렇게 나온다.

```text
## Type systems — Substructural types (affine types)
## Memory, hardware and low-level systems — In-memory data storage (in memory)
```

`verify_reviewed_relations.py`, `verify_sense_additions.py`, `verify_input_bindings.py`는 고친 뒤에도 통과한다. 이 스크립트들은 텍스트 출력에 특정 단어가 들어 있는지만 확인한다.

## 2. 관련어 단위 수정 연산

### 왜 필요한가

틀린 뜻의 원인은 뜻 전체보다 관련어 한 줄인 경우가 많다. 예를 들어 `ReLU`의 첫 뜻은 License activation이다.

- `ReLU`에는 자기 뜻이 없다. 다른 단어의 뜻을 거쳐 읽는 뜻(input)만 있다.
- `activate`와 `activation`은 관련어 목록의 `ReLU`를 각자의 License activation persona에 붙여 두었다. 그래서 `ReLU`는 License activation 뜻을 두 번 얻는다.
- 맞는 뜻(Neural network layers)은 `activation function` 하나에서만 온다.

주제 단위 연산(`moves`)은 단어의 persona를 통째로 옮긴다. `activation`의 License activation persona를 옮기면 `license key` 같은 맞는 관련어까지 함께 옮겨진다. 그래서 관련어 한 줄만 옮기거나 떼어내는 연산이 필요하다.

틀린 뜻을 고치는 방식은 사용자가 정했다. 관련어를 맞는 뜻으로 옮기고, 맞는 뜻이 없으면 뜻에서 떼어낸다. 지우지는 않는다.

### 연산

`revise_topics.py`의 계획 파일에 세 키를 더했다. 세 연산은 기본 데이터셋의 persona를 기준으로 읽고, `fields` 다음, `moves` 앞에 적용한다.

| 키 | 형식 | 동작 |
| --- | --- | --- |
| `relation_moves` | `{"word", "related", "topic", "into"}` | `word`가 `topic`의 persona에 붙인 관련어 `related`를 같은 단어의 `into` persona로 옮긴다. `into`에 persona가 여럿이면 번호가 가장 작은 것으로 옮긴다 |
| `relation_detaches` | `{"word", "related", "topic"}` | 관련어 `related`를 `word`의 `topic` persona에서 떼어낸다. 관련어는 `word`의 "뜻 미배정 직접 관계어"로 남는다 |
| `input_detaches` | `{"word", "head", "topic"}` | `word`가 `head`의 `topic` persona를 거쳐 읽던 뜻을 끊는다. 관련어 없이 검토로만 만든 입력 연결(기본값에 8,133개)에 쓴다. 관련어가 받치는 연결이면 거부하고 `relation_detaches`를 쓰게 한다 |

- **입력 연결이 따라간다.** `related`가 그 persona를 거쳐 뜻을 읽고 있었다면(`input_senses`), 옮기기에서는 그 연결도 새 persona로 옮긴다. 떼어내기에서는 그 연결을 없앤다.
- **관련어는 지우지 않는다.** 두 연산 모두 `relations` 행은 그대로 두고, 그 행이 가리키는 persona만 바꾼다.
- **거부하는 경우:**
  - 같은 관련어를 두 번 고친다.
  - `word`의 `topic` persona가 `related`를 갖고 있지 않다.
  - 옮길 곳에 `word`의 persona가 없다.
  - `related`가 그 persona를 거쳐 검토된 입력 관계어(`input_relations`)를 갖고 있다.
- 결과 파일(`revision-outcome.json`)의 `revised_relations`에 관련어마다 옮기기 전후의 persona가 남는다.

### 시험

ReLU 사례로 시험 판을 만들어 확인했다(저장소 밖 작업 폴더, 저장소에 넣지 않음).

```json
{"base_dataset": "rts4-e5083ab344e1336d",
 "relation_moves": [{"word": "activation", "related": "ReLU", "topic": "t19.4", "into": "t33"}],
 "relation_detaches": [{"word": "activate", "related": "ReLU", "topic": "t19.4"}]}
```

| 확인 | 결과 |
| --- | --- |
| `ReLU` 조회 | 첫 뜻이 Neural network layers다. 직접 관계어가 `activation`, `activation function`이다. License activation 뜻은 없다 |
| `activate` 조회 | `ReLU`가 "뜻 미배정 직접 관계어"에 남는다 |
| 행 수 | `relations`는 그대로다. `input_senses`는 하나 줄었다. `activation`의 연결은 옮겨졌고, `activate`의 연결은 없어졌다 |
| 검토 연결 끊기 | 같은 계획에 `{"word": "-fsanitize", "head": "compiler flags", "topic": "t62.3"}`를 더했다. `-fsanitize`의 그 뜻만 사라지고, 다른 뜻과 `compiler flags`의 관련어는 그대로다 |
| 잘못된 계획 | 같은 관련어를 두 번 고치는 계획, 없는 관련어를 가리키는 계획, persona가 없는 곳으로 옮기는 계획, 관련어가 받치는 연결을 `input_detaches`로 끊는 계획은 모두 거부됐다 |

## 3. 다음 단계

M3, N4, N6의 판정 결과는 이 연산으로 계획 파일에 적는다. 단어 자신의 persona가 통째로 틀린 경우는 기존 `moves`로 옮긴다.
