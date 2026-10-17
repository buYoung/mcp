# 유의어 조회하기

뜻 데이터셋 `rts4-301d57d419380d08`에서 단어의 유의어를 조회하는 방법입니다.

## 데이터셋 위치

`apps/codemap-search/experiments/related-terms-poc/datasets/rts4-301d57d419380d08/`

| 파일 | 내용 |
| --- | --- |
| `dataset.sqlite3` | 데이터셋 본체(48.6MB, `rt-sense/4`). 조회 스크립트는 이 파일만 읽습니다 |
| `revisions.json` | 이 판에서 고친 여러 단어 표현의 틀린 뜻 730개(관련어 옮기기 664개, 떼어내기 66개). 항목마다 판정한 단어와 이유가 있습니다 |
| `revision-outcome.json` | 수정을 적용한 결과. 옮기거나 떼어낸 관련어마다 바뀐 소속이 있습니다 |
| `build-report.json` | 만들 때의 기록 |

앞 단계에서 고친 틀린 뜻(관련어 옮기기·떼어내기, 단어 뜻 옮기기)은 `rts4-71cffacbc6f7c0d8`의 `revisions.json`에, 빠진 뜻 연결은 `rts4-912fe1c9f62131b2`의 `mappings.json`에, 주제 재분류(분야 수정, 합치기, 단어 옮기기, 빈 주제 삭제, 새 주제)는 `rts4-0fb3d5bc20c215ba`의 `revisions.json`에 항목마다 이유와 함께 있습니다.

이 판은 앞 판들에 차례로 더해 만들었습니다. 각 단계의 입력은 그 판의 폴더에 있습니다. 저장소에는 이 판, 앞 기본값 `rts4-710c8ba0d79a776a`와 `rts4-e5083ab344e1336d`, `rts3-9e01eaf1eadf960a`의 `dataset.sqlite3`만 들어 있고, 나머지 판은 입력 파일만 들어 있습니다. `rts4` 판들의 과정과 검증은 [작업 이력](../../../../docs/related-terms/sense-mapping-log/README.md)에 있습니다.

| 판 | 더한 것 | 폴더에 있는 입력 |
| --- | --- | --- |
| `rts3-9e01eaf1eadf960a` | 검토 보정 | 보정 계획, 근거로 쓴 12개 코드 저장소의 판(`sources.json`) |
| `rts3-25df1cf41d0d8247` | 분야별 누락 단어(`datas/omission/`) | `entries.json` |
| `rts3-08b1780cc2880c26` | 관련어로만 있던 단어의 분야 연결 | `corrections.json`, `bindings-review.json` |
| `rts3-5fba9ba33d7c7c7f` | 분야 연결에서 맞는 개념이 없던 단어의 새 개념 | `entries.json`, `concepts-review.json` |
| `rts3-48356e35e1f1ea28` | 앞 두 단계에서 빠진 49단어를 설명이 닿는 모든 분야에 연결 | `entries.json`, `rereview.json` |
| `rts4-3b9284a3b42c2917` | 구조 정리. 단어 이름만 붙인 개념을 없애고 단어를 주제에 바로 연결(`rt-sense/4`). 조회 결과는 그대로 | `migration.json` |
| `rts4-86c5620af1f181ce` | 분야가 하나뿐인 단어 934개에 다른 분야의 주제 연결 966개 | `mappings.json`, 판정 원본 |
| `rts4-0fb3d5bc20c215ba` | 주제 재분류. 분야 수정 617개, 합치기 229개, 단어 옮기기 18개, 빈 주제 삭제 15개, 새 주제 8개 | `revisions.json`, `revision-outcome.json` |
| `rts4-e5083ab344e1336d` | 재분류 판에 주제 연결 52개(입력 전용 단어의 연결 2개 포함) | `mappings.json` |
| `rts4-71cffacbc6f7c0d8` | 틀린 뜻 수정. 두 번 따로 판정해 둘 다 틀렸다고 한 뜻만 고침. 관련어 옮기기 877개, 떼어내기 191개, 검토 연결 끊기 8개, 단어 뜻 옮기기 67개 | `revisions.json`, `revision-outcome.json` |
| `rts4-912fe1c9f62131b2` | 수정 판에 빠진 뜻 연결 233개(입력 전용 단어의 연결 19개 포함) | `mappings.json` |
| `rts4-301d57d419380d08` | 남은 여러 단어 표현의 틀린 뜻 수정. 관련어 옮기기 664개, 떼어내기 66개 | `revisions.json`, `revision-outcome.json` |

앞 기본값 `rts4-710c8ba0d79a776a`와 그 앞 판 `rts4-78debdbe7f7bab0b`는 재검증 전의 판정으로 만든 판이라 이 판의 앞 판이 아닙니다.

## 조회 스크립트

`apps/codemap-search/experiments/related-terms-poc/sense_lookup.py`

데이터셋 경로는 스크립트 위치를 기준으로 찾습니다. 그래서 어느 폴더에서 실행해도 됩니다.

### 기본 사용

```sh
# 단어 하나
python3 apps/codemap-search/experiments/related-terms-poc/sense_lookup.py unshare

# 여러 개: 인자 하나가 입력 하나. 여러 단어로 된 표현은 따옴표로 묶음
python3 apps/codemap-search/experiments/related-terms-poc/sense_lookup.py 'abort signal' idx

# 기호가 붙은 입력은 작은따옴표로 감싸 셸이 바꾸지 않게 함
python3 apps/codemap-search/experiments/related-terms-poc/sense_lookup.py '$eventLoop' .net 'c++'
```

### 출력 읽는 법

`unshare`를 조회한 결과입니다.

```text
# 관련어 24개 · 입력 1개
입력 해석: unshare
## Operating systems, processes and shell — Namespace and root isolation
  같은 주제: AppArmor, chroot, mount namespace, namespaces, pivot_root, setns
## Cloud, containers and infrastructure — Linux container isolation
  직접 관계어: isolate, namespace
  가까운 연관어: cAdvisor, cgroup, container, crun, Debian, ext4, ...
  같은 주제: systemd-nspawn, ulimit
```

- **첫 줄:** 결과에 나온 단어 수와 입력 수입니다.
- **입력 해석:** 입력이 사전의 어떤 단어로 풀렸는지 보여 줍니다. 괄호 안은 실제로 맞춰진 사전 단어입니다.
- **`## 분야 — 주제`:** 그 단어의 뜻 하나입니다. 그 단어가 그 뜻으로 생성된 횟수가 많은 뜻이 먼저 나오고, 횟수가 같으면 입력과 비슷한 뜻이 먼저 나옵니다. 비슷한 정도는 관련어 데이터셋에 저장된 임베딩으로, 입력의 벡터와 그 주제 단어들의 평균 벡터를 비교해 잽니다. 검토로만 연결한 뜻은 맨 뒤에, 단어 자신의 뜻은 다른 단어를 거쳐 읽는 뜻보다 앞에 놓입니다. 이 규칙을 고른 과정은 [작업 이력 10단계](../../../../docs/related-terms/sense-mapping-log/10-sense-order-rule.md)에 있습니다. 뜻마다 아래 세 묶음이 가까운 순서로 나옵니다.
  - **직접 관계어:** 그 뜻으로 함께 만든 관련어
  - **가까운 연관어:** 같은 개념에 속한 단어
  - **같은 주제:** 같은 주제의 다른 단어
- **`## 뜻 미배정 직접 관계어`:** 뜻이 정해지지 않은 관련어입니다. 주로 같은 단어의 다른 표기이고, 틀린 뜻에서 떼어낸 관련어도 여기 남습니다(예: `activate`의 `ReLU`).
- 한 번 실행한 결과 안에서 같은 단어는 처음 나온 곳에만 씁니다.
- 결과는 자르지 않습니다. 단어에 따라 수백 개가 나옵니다. 예를 들어 `'abort signal' idx`는 988개입니다.

### 자주 쓰는 옵션

| 옵션 | 하는 일 |
| --- | --- |
| `--terms-file 파일` | 한 줄에 입력 하나씩 적은 파일로 조회 |
| `--format json` | 입력마다 JSON 한 줄로 출력 |
| `--format text-by-input` | 입력마다 결과를 따로 나눠서 출력 |
| `--output 파일` | 화면 대신 파일로 저장 |
| `--sense-dataset 경로` | 다른 rts 데이터셋 파일로 조회 |
| `--skip-syntax-parts` | 입력 중 코드 문법 조각(`fn`, `.go` 등)은 찾지 않음 |
| `--reverse` | 입력을 관련어로 가진 단어도 함께 출력(확인용) |
| `--no-exact-first` | 뜻이 있는 입력도 기본형까지 넓혀 찾음(예: `blocking` → `block`도 포함) |
| `--no-split-senses-by-topic` | 뜻을 주제가 아니라 분야 단위로 묶음 |
| `--no-similarity-order` | 생성 횟수가 같은 뜻을 입력과의 유사도 대신 주제 단어 수 순으로 정렬 |

### JSON 출력 구조

입력 하나에 한 줄이 나옵니다.

| 필드 | 내용 |
| --- | --- |
| `status` | `ok`, `no_sense`, `no_match` 중 하나 |
| `resolution` | 입력 해석 결과 |
| `senses` | 뜻 목록. 뜻마다 `field`, `topics`, `concepts`, `direct`, `near`, `same_topic`이 있음 |
| `unsorted_direct` | 뜻이 정해지지 않은 관련어 |
| `candidate_count` | 결과 단어 수 |

## 주의

지금은 이 데이터셋을 만든 컴퓨터에서만 동작합니다. 입력 해석에 `lookup-v3/target/release/rtpoc3` 실행 파일과 저장소 밖 `.codemap/` 아래 파일을 함께 쓰기 때문입니다. 새로 받은 저장소에서는 실행되지 않습니다.
