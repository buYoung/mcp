# 코드 단어 수집과 단어 평가

12개 코드 저장소에서 tree-sitter로 식별자를 모으고, 뜻 조회 결과가 코드에서의 뜻과 맞는지 LLM으로 판정하는 스크립트 모음이다. 같은 스크립트로 데이터셋에 빠진 단어와 뜻을 찾아 보강했다. 스크립트와 판정 지침은 `apps/codemap-search/experiments/related-terms-poc/word-eval/`에 있다.

- **LLM 판정은 스크립트가 하지 않는다.** 스크립트가 판정 묶음과 지침을 만들면, LLM이 결과 파일을 쓰고, 확인 스크립트가 형식을 검사한다.
- **수집 결과, 판정 묶음, 판정 결과는 보존하지 않는다.** 아래 결과 수치는 당시 데이터셋 기준이며, 현재 기본 데이터셋으로는 다시 평가하지 않았다.

## 실행 준비

| 항목 | 내용 |
| --- | --- |
| tree-sitter 환경 | `tree-sitter` 0.26.0과 `tree-sitter-language-pack` 1.21.0. 예: `uv run --no-project --with tree-sitter==0.26.0 --with tree-sitter-language-pack==1.21.0 python <스크립트>`. 수집 스크립트와 `survey_tag_strings.py`, `verify_comment_source.py`, `probe_full_scale.py`, `collect_full.py`, `validate_full.py`, 테스트 2개가 필요로 한다 |
| 작업 폴더 | 환경 변수 `WORD_EVAL_WORK_DIR`로 지정한다. 저장소별 수집 결과(`<작업 폴더>/<저장소>/`)와 집계 결과를 이 아래에 읽고 쓴다. 지정하지 않으면 작업 폴더가 필요한 스크립트는 오류 메시지를 내고 끝난다 |
| 판정 묶음 | `<작업 폴더>/batches/` 아래에 LLM에 줄 입력, 지침, 결과 파일을 둔다 |
| 코드 체크아웃 | `.codemap/poc/related-terms/workspaces/checkouts/<저장소>`. 저장소별 URL·태그·커밋은 [원자료와 근거 저장소](sources.md)에 있다 |
| 판정 지침의 자리표시자 | `{checkouts}`(체크아웃 폴더), `{catalog}`(데이터셋 목록 폴더)는 지침을 만드는 스크립트가 채운다. LLM에 직접 주는 지침(`gap-topic-brief.md`, `unit-brief.md`, `verify-brief.md`, `verify-lang-brief.md`)은 직접 채운다 |

## 1. 식별자 수집

| 스크립트 | 하는 일 | 출력 |
| --- | --- | --- |
| `collect_repo.py <저장소> <출력> [목표 줄 수]` | 고정 시드로 파일을 골라(기본 23,672줄, gin 전체 Go 줄 수) 정의된 이름과 쓰기만 한 이름을 모은다. Go는 `collect_words.py`, 나머지 언어는 `languages.py`의 규칙을 쓴다 | `sample-files.json`, `definitions.jsonl`, `words.json`, `unit-input.json`, `summary.json`. 출력 폴더는 `<작업 폴더>/<저장소>`로 둔다 |
| `collect_tag_values.py` | 태그 표기(Go struct 태그, 속성·애너테이션·데코레이터) 안의 값을 모은다 | `<저장소>/tag-input.json`, `tag-input-summary.json` |
| `survey_tag_strings.py` | 다른 언어의 태그 표기 문자열을 조사한다 | `tag-string-survey.json`, `.md` |
| `verify_comment_source.py` | 판정한 단어가 주석에서 온 것이 아닌지 대조한다 | `comment-source-check.json`, `.md` |
| `compare_verify.py`, `compare_verify_lang.py` | 수집 결과를 LLM이 원문을 읽고 만든 목록과 대조한다(지침 `verify-brief.md`, `verify-lang-brief.md`) | `verify-report.json`, `verify-lang-report.json` |
| `probe_full_scale.py` | 저장소 전체로 넓힐 때의 규모를 미리 잰다 | `full-scale-probe.json` |
| `collect_full.py <출력>`, `validate_full.py` | 표본이 아니라 저장소 전체에서 이름을 모으고 표본 결과와 비교한 뒤 검증한다 | 출력 폴더의 `names.jsonl`, `summary.json` 등 |

- **수집 대상:** Go, Python, JavaScript(`.js` `.mjs` `.cjs` `.jsx`), TypeScript(`.ts` `.mts` `.cts`), TSX, Java, C#, C(`.c` `.h`), C++(`.cc` `.cpp` `.cxx` `.hpp` `.hh` `.hxx`), Rust, PHP, Ruby
- **제외:** `node_modules`, `vendor`, `third_party`, `dist`, `generated` 같은 외부·생성 코드 폴더, 생성 표시가 있는 파일, 한 줄이 1,000자를 넘는 압축 파일. 테스트 경로의 파일은 표시해서 구분한다.
- **`collect_full.py`의 제약**
  - 이름의 등장만 모으고, 정의 여부는 기록하지 않는다.
  - 저장소 커밋이 표본 수집 때(`<저장소>/summary.json`)와 다르면 수집 전에 멈춘다.
  - 출력 폴더는 현재 작업 디렉터리 아래여야 한다.
  - 당시 전체 수집 결과는 데이터셋에 쓰이지 않았다.
- **테스트:** `test_collect_full.py`, `test_whitespace_regression.py`. 코드 안의 고정 예제만 쓰므로 작업 폴더 없이 실행된다.

## 2. 개념 단위 판정

식별자를 의미 단위로 나누고, 각 단위가 개발 분야의 개념으로 분류 가능한지 LLM이 판정한다(지침 `unit-brief-repo.md`).

1. `prepare_units.py <저장소>`: 판정 묶음(`batches/units-input/<저장소>/uNN.json`)과 지침을 만든다.
2. LLM이 `batches/units/<저장소>/uNN.json`에 결과를 쓴다.
3. `check_units.py`: 결과가 입력과 같은 수·순서인지, 단위가 식별자 전체를 덮는지 확인한다.
4. `aggregate_units.py`: 단위별로 모으고 데이터셋의 현재 상태와 맞춘다.

태그 값은 `prepare_tag_units.py`와 `merge_tag_units.py prepare|combine`으로 같은 절차를 거친다.

## 3. 사전 조회와 응답 판정

1. `lookup_words.py`: 평가 단어를 하나씩 `sense_lookup.py`로 조회해 응답을 저장한다.
2. `prepare_judge.py <저장소>`: 응답과 코드에서의 뜻을 묶어 판정 묶음과 지침(`judge-brief-repo.md`)을 만든다.
3. LLM이 `batches/repo-judge/<저장소>/jNN.json`에 결과를 쓴다. 당시 판정자는 `gpt-6-luna`(추론 medium)였다.
4. `check_judge.py`로 결과를 확인하고, `summarize_judge.py`로 지표를 낸다.
5. `write_judge_report.py`(저장소별)와 `cross_repo_summary.py [v2]`(12개 저장소)로 집계 문서를 쓴다.

판정 등급은 다음과 같다.

| 등급 | 뜻 |
| --- | --- |
| `match` | 코드에서의 뜻과 같은 뜻의 묶음이 응답에 있다 |
| `partial` | 같은 뜻의 묶음은 없지만, 가까운 뜻(더 넓거나 좁은 뜻, 인접 개념)의 묶음이 있거나, 뜻이 배정되지 않은 관계어에만 맞는 단어가 있다 |
| `none` | 관련어는 나왔지만 코드에서의 뜻과 맞는 묶음이 없다 |
| `no_result` | 관련어가 0개다 |

## 4. 데이터셋 보강

평가 단어 중 데이터셋에 없거나 분류되지 않은 것을 찾아 추가할 항목을 만든다. 결과 `gap/entries.json`은 [데이터셋 생성 과정](pipeline.md#4-뜻-데이터셋-rts3)의 8단계 입력이다.

1. `dataset_coverage.py`: 평가 단어를 데이터셋에서 찾는다. 표제어, 입력 뜻, 관계 파일에만 있음, 없음으로 나눈다.
2. `prepare_gap_phrases.py`: 데이터셋에 없는 문구를 하나로 볼지 나눌지 판정할 묶음을 만든다(지침 `gap-phrase-brief.md`). 결과를 `check_gap_batch.py pNN`으로 확인하고, `collect_gap_results.py phrases`로 모은다.
3. `build_gap_catalog.py`: LLM이 찾아볼 데이터셋 목록(분야·주제·개념)을 `batches/gap/catalog/`에 쓴다.
4. `prepare_gap_words.py base|phrases`: 단어를 넣을지와 넣을 자리를 판정할 묶음을 만든다(지침 `gap-word-brief.md`).
5. `prepare_inherit_review.py`: 파생어·약어가 기본형의 뜻을 이어받아도 되는지 다시 판정할 묶음을 만든다(지침 `gap-inherit-brief.md`). 새 주제 정리 판정은 지침 `gap-topic-brief.md`를 쓴다.
6. `collect_gap_results.py words`: 판정을 모아 `gap/entries.json`을 쓴다.

이 중 기본 데이터셋을 직접 읽는 스크립트(`dataset_coverage.py`, `build_gap_catalog.py`, `prepare_gap_words.py`, `prepare_inherit_review.py`, `collect_gap_results.py`)는 `rt-sense/3`과 `rt-sense/4`를 모두 읽는다. 그러나 `entries.json`을 넣는 `add-entries`는 `rt-sense/3` 판만 받으므로, 현재 기본 데이터셋(`rt-sense/4`)에는 이 결과를 그대로 넣을 수 없다.

## 5. 보강 후 재판정

보강한 데이터셋으로 평가 단어를 다시 조회하고, 응답이 바뀐 단어를 다시 판정한다.

- `prepare_v3.py prepare <새 데이터셋> <저장소…>`: 재판정 묶음을 만든다.
- `check_v3_judge.py`: 결과를 확인한다.
- `prepare_v3.py combine`: 판정을 합친다.
- `summarize_v3_all.py`: 12개 저장소를 집계한다.
- `count_v3_downgrades.py`, `count_v3_kept_changes.py`: 판정이 내려간 단어와, 이전 판정을 그대로 썼지만 응답이 바뀐 단어를 유형별로 센다.
- `verify_v3_47.py`: 관련어가 크게 줄어든 단어 47개를 단어별 원자료로 다시 확인한다.

## 당시 결과

보강 전 데이터셋은 `rts2-32469ce3ab670de2`, 보강 후는 `rts2-309e9bbaa19f7eb5`다(2026-10-11). 보강 전후의 단어 집합이 달라 합계를 그대로 빼서 비교할 수는 없다. 두 판 모두에 있는 단어 17,156개만 비교하면 `match`는 63.7% → 72.9%, `match`+`partial`은 89.9% → 96.9%였다.

| 판 | 단어 | `match` | `match`+`partial` | `none` | `no_result` |
| --- | ---: | ---: | ---: | ---: | ---: |
| 보강 전(2판, 일부 저장소는 1판) | 19,058 | 60.6% | 90.5% | 909 | 911 |
| 보강 후(3판) | 17,746 | 71.5% | 96.5% | 391 | 238 |

보강 후 저장소별 결과는 다음과 같다.

| 저장소 | 단어 | `match` | `match`+`partial` |
| --- | ---: | ---: | ---: |
| express | 431 | 81.0% | 97.9% |
| sentry | 1,919 | 76.8% | 97.4% |
| gutenberg | 1,661 | 75.9% | 98.4% |
| vector | 1,737 | 74.4% | 96.9% |
| keycloak | 1,937 | 72.6% | 97.2% |
| woocommerce | 1,690 | 71.5% | 97.3% |
| flask | 994 | 71.0% | 97.0% |
| gin | 915 | 70.6% | 93.7% |
| bitwarden-server | 1,579 | 69.2% | 96.4% |
| redis | 1,646 | 68.2% | 92.6% |
| tokio | 993 | 67.9% | 97.2% |
| terminal | 2,244 | 64.8% | 95.7% |

- 판정자는 LLM 하나이며, 사람의 판정과 대조하지 않았다.
- 이후 검토 보정으로 만든 `rts3-9e01eaf1eadf960a`는 다시 평가하지 않았다. 여기에 분야별 누락 단어를 더한 `rts3-25df1cf41d0d8247`, 관련어로만 있던 단어의 분야 연결을 더한 `rts3-08b1780cc2880c26`, 연결하지 못한 단어의 새 개념을 더한 `rts3-5fba9ba33d7c7c7f`, 빠진 단어를 재검토한 `rts3-48356e35e1f1ea28`, 이를 `rt-sense/4`로 옮기고 분야 매핑을 더한 `rts4-86c5620af1f181ce`, 주제를 재분류한 `rts4-e5083ab344e1336d`, 틀린 뜻을 고치고 빠진 뜻을 연결한 `rts4-710c8ba0d79a776a`, 그 판정을 재검증하고 남은 여러 단어 표현을 고친 `rts4-301d57d419380d08`, 이를 저장 방식만 바꾼 현재 기본 데이터셋 `rts5-75e02862daa29308`도 마찬가지다([알려진 결함](known-defects.md)의 U1).

## 이 폴더로 옮기며 확인한 것

스크립트는 원래 특정 컴퓨터의 절대 경로와 `/tmp`를 썼다. 지금은 `word_eval_paths.py`가 저장소 위치를 계산하고, 작업 폴더는 `WORD_EVAL_WORK_DIR`로 받는다. 이 변경 뒤 2026-10-16에 다음을 확인했다.

- 모든 모듈이 작업 폴더를 지정한 상태에서 import된다. 테스트 9개는 작업 폴더 없이 통과한다.
- 같은 체크아웃으로 `collect_repo.py`를 다시 실행하니, flask·express의 출력 파일 5개씩이 이전 결과와 바이트 단위로 같았다.
- `summarize_v3_all.py`와 `cross_repo_summary.py`(기본, `v2`)를 같은 데이터로 실행하니, 출력이 경로를 고치기 전 스크립트의 출력과 같았다.
