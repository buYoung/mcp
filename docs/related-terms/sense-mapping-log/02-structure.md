# 2단계: 구조 정리 — 단어 이름 개념을 단어 → 주제 매핑으로

작업일: 2026-10-17. 대상 판: `rts3-48356e35e1f1ea28`(`rt-sense/3`). 결과 판: `rts4-3b9284a3b42c2917`(`rt-sense/4`, SHA-256 `7621a8bec1365698f87795462b45c85534800ca4d0938f161000b344174ad04f`, 48,566,272바이트).

3단계에서 단어에 주제를 더하기 전에, 같은 내용을 여러 번 적던 부분을 한 번만 적도록 데이터셋 구조를 바꿨다. 이 단계는 조회 결과를 바꾸지 않는 것이 목표다.

## 1. 바꾼 이유

`rt-sense/3`에서 단어를 주제에 넣으려면 개념이 하나 필요했다. 그래서 단어 하나만 담고 그 단어 이름을 붙인 개념(`nvidia-smi` 개념의 구성원이 `nvidia-smi` 하나)이 많았다. 같은 단어가 같은 주제·같은 개념에 persona로 여러 번 들어간 경우도 있었다. 이 상태로 매핑을 넓히면 단어마다 개념을 또 만들어야 하고, 같은 단어가 더 많이 겹친다.

## 2. `rt-sense/4` 구조

정의는 `apps/codemap-search/experiments/related-terms-poc/sense_mapping_schema.py`에 있다.

| 항목 | `rt-sense/3` | `rt-sense/4` |
| --- | --- | --- |
| persona | 단어, 뜻(`senses`), 개념 | 단어, 주제, 개념(없어도 된다). 단어·주제·개념 조합마다 한 행 |
| 개념 없는 연결 | 없음 | 개념이 NULL이면 단어를 주제에만 연결한다 |
| 뜻 순서 가중치 | persona 행 수 | `source_persona_count`. 합친 persona 수를 적는다. 검토로만 연결한 주제는 0 |
| 분야 | `senses` 표에 저장 | 저장하지 않는다. 주제의 첫 분야(`topic_fields.rank = 0`)다. 주제를 다른 분야로 옮기면 그 주제의 단어가 따라간다 |
| `relations`, `input_senses`, `input_relations` | | 그대로. persona를 가리킨다 |

`source_persona_count`가 0인 뜻은 생성된 뜻과 연결된 뜻 뒤에 나온다. 또 그런 뜻만 가진 단어는 표제어로 치지 않아서, 조회할 때 기본형으로 넓혀 찾는 동작이 그대로다.

## 3. 옮기기

`migrate_sense_dataset.py`가 `rt-sense/3` 판을 읽어 새 판을 만든다.

- 단어 이름 개념 2,673개를 지우고, 그 단어의 persona는 개념 없이 주제에 연결했다.
- 어떤 persona도 쓰지 않는 개념 1개를 지웠다.
- 같은 단어·주제·개념을 되풀이한 persona 1,937개를 가장 작은 persona id로 합쳤다. 합쳐진 persona의 관련어, `input_senses`, `input_relations`는 남은 persona로 옮겼다.
- 지운 개념과 합친 persona 목록은 판 폴더의 `migration.json`에 있다.

| 항목 | 전 | 후 |
| --- | ---: | ---: |
| 단어(`terms`) | 18,566 | 18,566 |
| 개념 | 7,832 | 5,158 |
| persona | 45,151 | 43,214 |
| 그중 개념 없는 persona | 0 | 2,673 |
| 관련어(`relations`) | 592,793 | 592,793 |
| `input_senses` | 334,442 | 334,442 |
| `input_relations` | 9 | 9 |

## 4. 조회 코드

`sense_lookup.py`가 `rt-sense/4`를 읽도록 고쳤다. 기본 데이터셋은 바꾸지 않았다.

- 읽을 수 있는 스키마에 `rt-sense/4`를 더했다(`READABLE_SCHEMAS`).
- persona를 읽는 질의를 스키마별로 나눴다(`PERSONA_QUERIES`). `rt-sense/4`는 분야를 주제의 첫 분야에서 읽고, 가중치를 `source_persona_count`에서 읽는다.
- 개념이 NULL인 persona는 개념 없이 주제만 가진 뜻이 된다.
- 표제어 판정(`generated_headwords`)과 뜻 순서(`sense_order`)에서 가중치 0인 뜻을 따로 다룬다.

## 5. 확인한 것

| 확인 | 방법 | 결과 |
| --- | --- | --- |
| 모든 단어의 조회 결과 | `verify_sense_migration.py`. 전 판이 분류하는 176,416단어를 CLI 기본값으로 한 단어씩 두 판에서 조회했다 | 같음 172,848, 개념 이름만 다름 3,564, 달라짐 4 |
| 무작위 표본 | 같은 스크립트, 3,000단어 | 같음 2,934, 개념 이름만 다름 66 |
| 평가 세트 | `sense-eval/score_eval_set.py`, 224단어 | 224단어 모두 텍스트 응답의 SHA-256이 같다(`sense-eval/results/rts4-3b9284a3b42c2917.json`) |
| 옛 코드와 새 코드 | 고치기 전 `sense_lookup.py`로 `rts3`을, 고친 코드로 `rts4`를 조회했다. 2,000단어 표본 | 개념 이름 말고 모두 같다 |
| 기존 검증 스크립트 | `verify_input_bindings.py`, `verify_reviewed_relations.py`, `verify_sense_additions.py`, `verify_spelling_aliases.py` | 4개 모두 통과 |
| 크기와 적재 시간 | 파일 크기와 `SenseIndex.load` 시간 | 49.2MB → 48.6MB. 적재 2.86초 → 3.37초(같은 때 같은 장비에서 잰 값) |

- **개념 이름만 다름:** 텍스트 응답은 같고, JSON 응답의 개념 이름만 다르다. 단어 이름 개념을 지웠기 때문이다.
- **달라짐 4단어**(`evictionHard`, `nested list`, `nested lists`, `repoURL`): persona를 합치면서 `input_senses`가 가리키는 persona 순서가 바뀌었다. 그래서 직접 관계어의 순서만 다르고, 단어 집합·개수·뜻은 같다. 받아들였다.

## 6. 영향 범위

- **쓰기 도구:** `build_sense_dataset.py add-entries`와 `correct_sense_dataset.py`는 `rt-sense/4` 판을 거부한다. 의도한 동작이다. `rt-sense/4` 판에 매핑을 더하는 도구는 3단계의 `add_topic_mappings.py`다.
- **기본값을 직접 읽는 word-eval 스크립트:** `word-eval/aggregate_units.py`(`load_dataset`)와 `word-eval/dataset_coverage.py`는 `senses` 표를 직접 조인한다. `word-eval/prepare_gap_words.py`는 persona마다 개념이 있다고 가정한다. 기본 데이터셋을 `rt-sense/4` 판으로 바꾸면 이 스크립트들이 실패한다. 그래서 기본값은 아직 바꾸지 않았다([4단계](04-verification.md#7-기본-데이터셋)).
- **조회 결과를 읽는 스크립트:** `SenseIndex`를 거치는 스크립트(`word-eval/lookup_words.py`, `sense-eval/`, `sense-mapping/`)는 두 스키마를 모두 읽는다.
