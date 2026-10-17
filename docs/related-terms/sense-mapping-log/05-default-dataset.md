# 5단계: 기본 데이터셋 전환

작업일: 2026-10-17. 전 기본값: `rts3-48356e35e1f1ea28`(`rt-sense/3`). 새 기본값: `rts4-86c5620af1f181ce`(`rt-sense/4`).

4단계에서는 기본 데이터셋을 바꾸지 않았다([4단계 7절](04-verification.md#7-기본-데이터셋)). 4단계 결과를 보고 사용자가 기본값을 바꾸기로 정했다. 그래서 기본 데이터셋을 직접 읽는 스크립트가 `rt-sense/4`도 읽도록 고친 뒤 기본값을 바꿨다.

## 1. 바꾼 것

| 파일 | 바꾼 것 |
| --- | --- |
| `sense_lookup.py` | `DEFAULT_SENSE_DATASET`을 `datasets/rts4-86c5620af1f181ce/dataset.sqlite3`로 바꿨다. `persona_rows(connection)`를 더했다. 판의 스키마에 맞는 `PERSONA_QUERIES` 질의로 persona 행(persona, 단어, 분야, 개념, 주제, 가중치)을 읽는다 |
| `word-eval/aggregate_units.py` | `load_dataset`이 `senses` 표 대신 `persona_rows`로 분야를 읽는다 |
| `word-eval/dataset_coverage.py` | `dataset_fields`가 `aggregate_units.load_dataset`을 쓴다. 같은 질의를 따로 두지 않는다 |
| `word-eval/prepare_gap_words.py` | 뜻 자리를 개념 하나가 아니라 (주제, 개념)으로 다룬다. 개념 없이 주제에만 연결된 단어는 개념이 없는 자리로 나온다 |
| `word-eval/build_gap_catalog.py` | `topics.jsonl`의 주제마다, 개념 없이 그 주제에만 연결된 단어를 `words`로 적는다(최대 20개) |

`prepare_inherit_review.py`는 `prepare_gap_words.Dataset`을 쓰므로 함께 고쳐진다. `collect_gap_results.py`는 id와 단어 키만 읽어서 고치지 않았다.

## 2. 확인한 것

| 확인 | 결과 |
| --- | --- |
| 고친 word-eval 함수를 `rts3-48356e35e1f1ea28`에서 고치기 전 코드와 비교 | `load_dataset` 결과가 같다. `current_senses` 3,000단어, `relation_evidence` 2,000단어가 모두 같다. `build_gap_catalog.py`의 `concepts.jsonl`이 바이트 단위로 같고, `topics.jsonl`은 빈 `words`만 더해졌다 |
| 고친 word-eval 함수를 새 기본값에서 실행 | 실패 없이 읽는다. 표제어 15,755개, 입력 뜻 단어 167,531개. 주제 2,781개, 개념 5,158개 |
| 평가 세트(`sense-eval/score_eval_set.py`, 기본값으로 실행) | 요약과 224단어의 응답 SHA-256이 `sense-eval/results/rts4-86c5620af1f181ce.json`과 같다 |
| 기존 검증 스크립트 4개 | `verify_input_bindings.py`, `verify_reviewed_relations.py`, `verify_sense_additions.py`, `verify_spelling_aliases.py` 모두 통과 |
| 조회 README 예시 | `unshare` 65개, `'abort signal' idx` 983개. `README.sense.ko.md`를 이 값으로 고쳤다 |

## 3. 고친 문서

- `README.sense.ko.md`: 데이터셋 이름·위치·파일, 판 표에 `rts4` 두 판, 출력 예시
- `docs/related-terms/pipeline.md`: 데이터셋 표의 기본값, 파이프라인 단계 표의 14·15단계(구조 정리, 분야 매핑 확장)
- `docs/related-terms/known-defects.md`: 뜻 데이터셋 행, U1, 고칠 때 지킬 점
- `docs/related-terms/word-evaluation.md`: 현재 기본 데이터셋 이름, 데이터셋 보강 절의 제약

## 4. 남은 제약

- **쓰기 도구:** `build_sense_dataset.py add-entries`와 `correct_sense_dataset.py`는 `rt-sense/3` 판만 받는다. 그래서 단어 평가의 데이터셋 보강 결과(`gap/entries.json`)는 새 기본값에 그대로 넣을 수 없다. `rt-sense/4` 판을 고치는 도구는 `add_topic_mappings.py`와, 6단계에서 만든 도구다.
- **평가 세트 기준판:** `sense-eval/prepare_eval_set.py`는 평가 단어를 뽑을 때 `rts3-48356e35e1f1ea28`을 기준으로 쓴다. 평가 세트를 만든 판을 고정하려는 것이라 바꾸지 않았다.
