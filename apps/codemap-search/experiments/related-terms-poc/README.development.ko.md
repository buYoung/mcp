# Development 사전으로 related-terms PoC 조회하기

이 문서는 로컬 checkout의 PoC 개발자·검토자가 **이미 보존된 SQLite로 조회**하고 새 측정 자료를 확인하는 사용 안내입니다. development 선택은 `rtd1-47baa61927c4b090`입니다. 제품·외부 배포가 아니며, **fresh 의미 정확도 검토는 아직 끝나지 않았습니다.**

기존 [README.ko.md](README.ko.md), `results/dataset-handoff.json`, `results/lookup-handoff.json`과 과거 지표는 BROAD baseline(`rtd1-8efd9676107a983d`) 기록으로 그대로 남습니다. development 조회는 별도 [active.json](results/development/active.json)을 사용합니다. 원래 candidate handoff의 과거 `activated:false`도 고치거나 현재 선택 상태로 재해석하지 않습니다.

## 기존 SQLite로 바로 조회하기

시작 조건: 저장소 root에서 Bash를 사용하며 기존 `rtpoc` release binary, SQLite, policy, 네 workspace의 published stats/index metadata가 있어야 합니다. selector를 해석할 `jq`, SHA 확인용 `shasum`, Python 3도 필요합니다. 아래 명령은 설치·다운로드·build·index를 하지 않습니다. 파일이 없거나 hash/generation이 다르면 **중단하고 근거를 검토하십시오. 자동 재구축하거나 frozen builder를 현재 checkout에서 재실행하지 마십시오.**

다음 block은 selector를 한 번 읽고 profile SHA/size를 확인합니다. companion의 `validate`는 읽기 전용이며 실패해도 checkpoint를 쓰지 않습니다. 실제 조회는 기존 binary를 직접 실행합니다.

```sh
set -eu
E=apps/codemap-search/experiments/related-terms-poc
S=$(jq -ce '.' "$E/results/development/active.json")
P=$(printf '%s' "$S" | jq -er '.profile.path')
EXPECTED=$(printf '%s' "$S" | jq -er '.profile.sha256')
BYTES=$(printf '%s' "$S" | jq -er '.profile.bytes')
[ "$(shasum -a 256 "$P" | awk '{print $1}')" = "$EXPECTED" ]
[ "$(wc -c < "$P")" -eq "$BYTES" ]
python3 -B "$E/evaluate_development.py" validate --profile "$P"
B=$(jq -er '.runtime.binary_identity.path' "$P")
DATASET=$(jq -er '.dataset.sqlite.path' "$P")
ID=$(jq -er '.dataset.dataset_id' "$P")
POLICY=$(jq -er '.policy.path' "$P")
STATE=$(jq -er '.targets.tokio.state_dir' "$P")
[ "$(shasum -a 256 "$P" | awk '{print $1}')" = "$EXPECTED" ]
"$B" lookup --dataset "$DATASET" --expect-dataset-id "$ID" \
  --policy "$POLICY" --state "$STATE" \
  start stop delete retry thread "event loop" res
```

출력은 term당 한 줄인 `rt-lookup/1` JSON입니다. `matches`와 `candidates[].evidence`에서 사전 근거를, `stage_counts`·`candidates[].stage/reason`에서 필터를, `final_terms`에서 해당 repository의 최종 표기를 확인합니다. 실행마다 `lookup_us`는 달라질 수 있습니다. 반환 표기는 의미 검토가 완료된 추천이 아닙니다. 조회 중 state가 바뀌어 `repository_not_ready`가 나오면 최종 결과로 사용하지 말고 중단하십시오. 이를 자동 build/index로 복구하지 않습니다.

다른 workspace를 조회하려면 첫 block에서 설정한 `$B`, `$DATASET`, `$ID`, `$POLICY`, `$P`를 사용하고 같은 profile에서 state만 선택하십시오. `flask`, `gin`, `express`, `tokio`가 동결된 네 target입니다.

```sh
STATE=$(jq -er '.targets.flask.state_dir' "$P")
"$B" lookup --dataset "$DATASET" --expect-dataset-id "$ID" \
  --policy "$POLICY" --state "$STATE" \
  start stop delete retry thread "event loop" res
```

### Dictionary-only와 repository 필터 결과를 구분하기

첫 block의 profile 검증과 변수 설정을 마친 뒤, 사전 자체의 matches/relations/evidence를 보려면 `--state`를 생략합니다. 이는 사전 진단 모드이지 repository 근거 없이 `final_terms`를 만드는 모드가 아닙니다. 사전 relation이 있어도 `repository.status=not_configured`, `status=repository_not_ready`, `final_terms=[]`가 될 수 있습니다. 사전 entry 자체가 없는 term은 `no_dictionary_entry`입니다.

```sh
"$B" lookup --dataset "$DATASET" --expect-dataset-id "$ID" \
  --policy "$POLICY" start stop delete retry thread "event loop" res
```

repository-filtered 조회는 같은 후보에 published word/file statistics를 적용합니다. 한 token도 없으면 탈락하며, 단일 token의 file fraction이 0.25를 넘으면 commonness 필터로 탈락합니다. deduplication 후 최대 10개를 반환합니다. phrase는 각 token의 존재만 확인하며 phrase의 실제 인접 출현을 확인하지 않습니다. `rtl-1`의 kinds·priority·상한과 `rtn-2` normalization은 바꾸지 않았습니다.

보존된 generation2 직접 consumer 확인에서 `retry`, `thread`, `res`는 네 target 모두 `no_dictionary_entry`였습니다. `delete`에는 사전 relation이 있지만 기존 repository-presence 필터로 `erase`가 탈락했습니다. `start`, `stop`, `event loop` 결과도 새 consumer 기록에 보존했습니다. 이 관측은 semantic label이나 정확도 판정이 아닙니다. 이 과거 관측의 근거: `.codemap/poc/related-terms/development-integration-v1/selector-consumer-v2/`의 term별 raw JSONL과 command/exit 기록.

## Selector와 immutable profile의 역할

[active.json](results/development/active.json)은 현재 development profile의 `{path,sha256,bytes}`만 가리킵니다. dataset/policy/state의 mutable 사본을 중복 저장하지 않습니다. profile의 실제 canonical UTF-8 파일 SHA가 directory key이므로 self-hash 순환이 없습니다.

현재 immutable profile directory는 다음과 같습니다.

```text
apps/codemap-search/experiments/related-terms-poc/results/development/profiles/29b5431cd74b3171244cda1d9919cca6cf2f14cff926934cb81d603193d8b5fd/
```

이 directory의 `profile.json`, `dataset-handoff.json`, `lookup-handoff.json`, `evaluation-protocol.json`이 dataset·curation·policy·reader·code·cases·state/stat/index identities를 연결합니다. 새 companion의 동결 당시 코드도 같은 directory에 보존했습니다. `freeze`와 selector의 명시적 `publish-selection`은 별개 작업입니다. 기존 artifact/run directory를 덮어쓰지 않습니다.

현재 generation은 승인된 원 cases SHA와 재사용 helper의 historical SHA를 helper import 전에 확인하고, 모든 input 검증 후에 profile을 게시합니다. protocol/handoffs/archived code도 측정 전후에 묶습니다. 첫 generation `b5226f6dc63b28d1787dc5769c65bf24bdb5160b640daf5e81ed70af2a077f89`의 실제 입력·측정은 승인본과 일치했지만 이 강화 gate를 소급 주장하지 않습니다. 첫 코드/profile/run/raw/sandbox/blank queue와 selector는 `.codemap/poc/related-terms/development-integration-v1/history/generation-1/manifest.json`에 immutable로 보존했습니다. 같은 후보의 새 측정/queue/보존 gate 통과 후 부모 승인 compare-and-swap으로 NEW selector만 원자적으로 재게시했습니다. BROAD selectors와 첫 generation 산출물은 그대로입니다.

과거 평가를 다시 검증할 때는 selector 대신 immutable profile을 직접 지정합니다. 이후 selector가 달라져도 run의 선택 근거는 달라지지 않습니다. 참조한 실제 코드·SQLite·cases·policy·stats·index가 바뀌면 검증은 실패합니다. stale 상태를 자동 복구하지 않습니다.

```sh
# cwd: repository root; read-only profile and fresh-run validation
P=apps/codemap-search/experiments/related-terms-poc/results/development/profiles/29b5431cd74b3171244cda1d9919cca6cf2f14cff926934cb81d603193d8b5fd/profile.json
python3 -B apps/codemap-search/experiments/related-terms-poc/evaluate_development.py \
  validate --profile "$P" --run-id run-1
```

## 실제 수행한 fresh 측정

동결된 92 cases는 control 16개, category probes 46개, random probes 30개입니다. 입력 cohort·순서·intended meaning·policy를 바꾸거나 입력을 재샘플링하지 않았습니다. target마다 fresh process에서 첫 pass를 측정하고 같은 process에서 네 pass를 반복했습니다. OS page cache는 통제하지 않았습니다.

| 검사 | 이번 관측과 범위 |
|---|---|
| Dataset | `rt-dataset/1`, `rtn-2`; 실제 read-only SQLite meta/schema/index/FK/integrity와 counts 확인 |
| Counts | concepts 3,810 / expressions 5,965 / lexicalizations 8,445 / relations 15,578 / sources 5 |
| Native normalization | expressions 5,965 / surfaces 8,445; mismatch 0 |
| Native stats | 네 target 모두 `ready`; published/current generation 일치; live files flask 121 / gin 109 / tokio 820 / express 161 |
| 측정 coverage | target별 raw 92행; 정확히 368개의 고유 `(case_id,target)` observations; whole-object raw equality(`output_bytes` 포함) |
| 반복 | 각 target 5 passes; 내부 lookup 1,840회; 모든 반복 출력 동일. 독립 의미 관측 1,840개라는 뜻은 아님 |
| 실제 비용 | `/usr/bin/time -l` stdout/stderr/exit, process RSS/wall, open 비용과 pass별 timing 보존 |
| Fresh offline | 기존 compatible sandbox에서 network·checkout·index segment 읽기를 deny하고 meta.json만 허용; fresh92×4×2 출력이 native raw와 `lookup_us`만 제외하고 같음 |
| Before/after | actual profile/code/SQLite/cases/policy/state/stat/index SHA와 stat 일치 |
| Build/index/acquisition | `not_run`; 기존 SQLite/binary/statistics 재사용 |
| 기존 Cargo/lifecycle 증거 | exact source/Cargo/binary identities로 bound한 **재사용 증거**; 이번 실행으로 바꿔 쓰지 않음 |

새 sandbox에서 negative index/network 시도나 fixture는 추가하지 않았습니다. 과거의 negative-control 근거와 이번의 실행·출력 동등성 근거는 구분합니다.

이번 첫 pass의 lookup median은 target별 0.031–0.055 ms, repeat median은 0.028–0.049 ms였습니다. process max RSS는 10,829,824–11,763,712 bytes였습니다. RSS는 dataset open과 전체 lookups를 포함하는 process 최대치이며 lookup당 메모리가 아닙니다. 단일 로컬 실행 결과로 일반 성능을 보장하지 않습니다.

기술 관측에서 dictionary coverage는 **unique inputs 53/92**, nonempty final은 **case×target 124/368**입니다. 이들은 entry/반환 유무이지 의미 정확도가 아닙니다. `expected_any`도 동결된 기대 표기일 뿐 gold labels가 아닙니다. 원래 denominator와 cohort별 집계를 유지했고, 반복·final census·control/category의 `ci95`는 `null`입니다.

Compact records: [measurement-handoff.json](results/development/runs/29b5431cd74b3171244cda1d9919cca6cf2f14cff926934cb81d603193d8b5fd/run-1/measurement-handoff.json), [offline-handoff.json](results/development/runs/29b5431cd74b3171244cda1d9919cca6cf2f14cff926934cb81d603193d8b5fd/run-1/offline-handoff.json), [evaluation-handoff.json](results/development/runs/29b5431cd74b3171244cda1d9919cca6cf2f14cff926934cb81d603193d8b5fd/run-1/evaluation-handoff.json), [technical-report.json](results/development/runs/29b5431cd74b3171244cda1d9919cca6cf2f14cff926934cb81d603193d8b5fd/run-1/technical-report.json).

`measurement-handoff.json`의 `offline:pending`은 native measurement를 게시한 시점의 immutable 기록입니다. 뒤에 실행한 fresh offline 결과는 별도 `offline-handoff.json`을 함께 확인하십시오. 과거 파일의 상태를 나중에 덮어쓰지 않습니다.

## Fresh labels를 준비하되 아직 판단하지 않기

새 queue는 다음 ignored run 아래에 있습니다.

```text
.codemap/poc/related-terms/evaluation/development/29b5431cd74b3171244cda1d9919cca6cf2f14cff926934cb81d603193d8b5fd/run-1/review/
```

- `blank-review.tsv`: 기존 review columns 그대로, final unique pairs 97개와 loss sample 62개, 총 **159행의 label/rationale가 모두 빈 값**입니다.
- `queue-manifest.json`: profile/run/raw identities, ordered F/L pair keys, population/sample/seed, 정확한 assignments와 packet identities.
- `rubric.json`: 기존 8 labels의 정의와 intended meaning + recorded evidence 판정 방법. label을 지금 제공하지 않습니다.
- `submission-schema.json`: 별도 새 `submissions/*.jsonl`의 필수 identity/key/label/rationale fields와 허용 labels.
- `assignments/batch-001/manifest.json`부터 **23개 byte-aware batch**: 원래159 review-ID 순서의 contiguous greedy 구간입니다. 각 manifest는 정확한 pair 범위/queue binding과 required file path/SHA/size/line count를 담습니다.
- `batch-output.schema.json`: Draft2020-12 formal JSON Schema입니다. read+structured_output-only labeler의 transport wrapper를 정의하고 원래9-field records/8 labels를 그대로 보존합니다.
- `pairs/<review_id>/packet-index.json` 및 `observations/<target>/<case_id>/packet-index.json`: 전체 source/license/gloss/concept/lexicalization evidence와 per-target 결과를 lossless로 읽는 자료.

F는 어느 target에서든 final인 pair 전수입니다. L은 어느 target에서든 normalization을 지나갔지만 어느 target에서도 final이 아닌 pair population입니다. 원래 방법 그대로 sorted population에서 seed `20261007`, 최대 150개의 uniform sample을 뽑고 정렬했습니다. 이번 population은 62개여서 sample도 62개입니다. 이는 입력 cohort 재샘플링이나 의미 판단이 아닙니다. 이번 62/62는 전수이므로 표본오차 CI를 붙이지 않습니다. queue의 `interval_condition`은 `census`/`null`이며, future semantic rates에도 `ci95=null`을 적용합니다.

**배정받은 labeler의 읽기 순서:** 부모가 제공하는 repo-relative assignment manifest identity(path/SHA/bytes)와 wrapper schema reference를 사용하십시오. manifest를 offset1/limit120부터 +120씩 EOF까지 모두 읽고 `required_files`의 모든 파일도 각각 EOF까지 읽습니다. 각 packet-index의 ordered parts를 전부 합쳐 전체 JSON을 복원합니다. 같은 observation은 assignment 안에서 한 번만 읽습니다. TSV preview만으로 판단하거나 observation 안의 다른 부분을 생략하지 마십시오.

각 batch의 **실제 deduplicated mandatory file bytes는327,680 bytes(320KiB) 이하**입니다. 자체 assignment manifest, pair/observation indexes와 모든 parts, rubric, submission schema와 formal wrapper schema를 포함합니다. 이번 최대는322,540 bytes입니다. prompt/tool envelope/output 여유는 별도로 둡니다. singleton 하나가 예산을 넘으면 중단하고 부모에게 보고하며 truncate하지 않습니다. 이는 파일 전달 예산이지 실제 model overflow 관측이나 token-limit 보장은 아닙니다.

**global queue의 전체 읽기는 labeler에게 명시적으로 면제**됐습니다. 전체 integrity/159 coverage/중복·누락 검증은 부모·독립 audit·collector가 소유합니다. labeler는 자신의 완전한 assigned-pair/관련 observation graph, rubric/schema와 scope manifest만 읽습니다. full profile/measurement/global queue 및 whole target raw 파일은 labeler의 필수 payload가 아닙니다. 관련 raw observation의 전체 객체는 required observation packets에 그대로 있습니다.

부모 launch owner는 `.codemap/poc/related-terms/development-integration-review-fixes-v1/labeling-launch-plan.json`에서 current profile/run/queue/schema와23개 assignment identity·count·bytes를 찾습니다. 각 항목은 required-file manifest를 가리킵니다. 부모/collector는 compact `results/development/runs/29b5431cd74b3171244cda1d9919cca6cf2f14cff926934cb81d603193d8b5fd/run-1/labeling-plan.json`과 global queue를 완독·검증해야 합니다.

향후 native labeler는 read+structured_output만 사용하고 파일을 쓰거나 blank table/queue/packets를 수정하지 않습니다. formal wrapper의 `status`, profile/run/queue/assignment identity, `records`, `remaining_review_ids`, `blocker`를 반환합니다. `complete`는 assigned scope 전수이고 `blocked`는 partial records와 남은 IDs/명시적 blocker입니다. collector는 launch-plan의 실제 assignment SHA/size/path와 각 pair key/coverage를 대조한 뒤 원래9 fields를 가진 records만 losslessly accepted JSONL로 옮깁니다. wrapper 자체를 `labels-check`에 전달하지 않습니다. historical labels 읽기/복사와 curation 재개는 금지합니다. 이번 worker는 labels를 만들거나 classifier를 실행하지 않았습니다.

**실제 EOF 예:** current `pairs/L127/part-0004.json.txt`는120줄/5,223 bytes이며 offset1/limit120과 offset121/limit120(EOF)으로 읽었습니다. 최대 `assignments/batch-011/manifest.json`은747줄/38,333 bytes이며 offsets1/401에 limit400으로 모두 전달되고748은 EOF였습니다. 일반 labeler는 manifest/required-file의 line count를 보고120줄씩 마지막까지 읽으십시오. 모든23개 graph의 실제 EOF/SHA/bytes와 dedup 합계는 `current-graph-verification.json`에 있습니다.

`labels-check`는 explicit 새 submissions 파일의 profile/run/queue hash, pair key, F/L population, allowed label, nonempty rationale, 정확한 final census와 seeded loss sample coverage를 확인합니다. blank·missing·duplicate·extra·unbound labels가 있으면 실패합니다. `aggregate`도 이 gate와 fresh offline gate 전에는 report를 쓰지 않습니다. missing labels를 0이나 자동 정답으로 취급하지 않습니다. 이번에는 worker가 labels를 만들거나 semantic aggregation을 실행하지 않았습니다.

향후 fresh labels가 있어도 agent annotation은 human gold가 아닙니다. 전수 loss(이번 62/62)의 비율은 `ci95=null`입니다. Wilson95는 실제 uniform 부분 표본에만 기존 caveat와 함께 적용하며 FPC를 추가하지 않습니다. uncertain은 별도로 유지합니다. 독립 implementation/measurement review와 fresh semantic labeling이 다음 gate입니다.

## Source provenance와 builder 재현 한계

사전은 보존된 broad source의 concept-qualified lexicalization/relation 부분집합입니다. five sources는 Open English WordNet 2025, pinned GitHub Explore, abbrcode, software-engineering abbreviations, selected Wikidata입니다. source revisions/archive hashes는 [sources.lock.json](sources.lock.json), 라이선스·attribution은 [ATTRIBUTION.md](ATTRIBUTION.md)와 dataset의 `LICENSES/`에 있습니다. 일반 영어 sense·catalog aliases·약어를 담는 source가 있으므로 개발 사전 선택만으로 문맥 정확성을 보장하지 않습니다.

SQLite SHA-256:

```text
b69cac9ca0d64c58d675f2e2d4d5f3a300cba271204ea628f07cbcee6c14678d
```

content ID와 별도로 curation/recipe/content를 묶는 derivation SHA는 `63f31c637266b5f9de5be2b3d39a1fff6a59c5f54a6e36b679b595b1d086ab21`입니다. frozen curation decisions와 builder·원래 handoff/archives는 수정하지 않았습니다.

**Full-builder relocation은 지원하지 않습니다.** full reproduction에는 동결 근거에 기록된 **원래 절대 저장소 root를 유지하는 격리 환경**, 모든 frozen prerequisites, 그리고 owned candidate AREA/content-ID outputs가 없는 상태가 필요합니다. root의 근거는 ignored `.codemap/poc/related-terms/development-curation/semantic-adjudication-v2/final-decisions-handoff.json`의 `cwd`·`material[].path`, 같은 directory의 `preservation-before.json`의 `[].path`입니다. 기존 locators를 새 root로 고치지 마십시오. portable serving SQLite/JSONL과 cross-root builder 지원은 다른 문제입니다. cross-platform SQLite byte 재현이나 다른 platform sandbox 실행은 이번에 검증하지 않았습니다.

이전 integration 단계의 보존 근거는14,660 current SHA와17 first-phase SHA였습니다. 이번 fix 보존 snapshot은3,884 SHA(개정 live4는 archived 원본에 bind),14,648 bounded stat-only 및18 sensitive stat-only로 scope를 구분합니다. 이는 범위 밖 inventory를 새로 해시했다는 뜻이 아닙니다. 설정·자격 증명·환경 경로 18개는 bytes를 읽지 않고 stat-only로 유지합니다. 과거 SHA는 historical/prior-only이며 해당 18개에 current byte-identity 검증을 주장하지 않습니다. 모든 old metrics·label/protocol/history·indexes/statistics는 보존 대상입니다. 전체 인계 근거는 `.codemap/poc/related-terms/development-integration-v1/integration-handoff.json`을 확인하십시오.

## 두 독립 review finding에 대한 좁은 후속 수정

현재 generation3은 보존 generation2 `c32d80981993c6c4d81065d347c20476b6bc7b7aac506a57e0e91e6b0ce5d1d1`의 같은 dataset/cases/policy/binary/stats/index로 새92×4×5와 sandbox92×4×2를 실제 수행했습니다. raw 비교에서는 native `lookup_us`만 volatile timing으로 제외했고 모든 다른 객체 값과 F97/L62/seed/order는 동일합니다. current observations는 current raw에서 whole-object로 정확히 재구성하며 `output_bytes`도 현재 UTF-8 line bytes와 일치합니다.

NEW writer는 trusted repository anchor 아래 lexical output namespace를 고정한 뒤 각 ancestor/leaf symlink를 첫 mkdir/open/native output 이전에 거부합니다. raw/compact/profiles/offline/review/reports/selector/temp/failure evidence의 공통 guard입니다. exclusive collision과 explicit selection behavior는 유지합니다. sole-writer pre-write gate이며 concurrent attacker sandbox 기능은 추가하지 않았습니다. symlink/새 test fixture는 만들지 않았고 rejection은 정적으로 확인했습니다. validate의 정상/실패는 실제로 비쓰기 확인했습니다.

첫 두 generations는 유효한 historical 실행이며 이번 수정은 과거 output 오염 증거가 아닙니다. oversized assignment는 transport 계약 문제였고 native model overflow는 관측되지 않았습니다. 과거 immutable reports/profiles/packets/indexes는 고치지 않습니다. 개정 live 코드/README/NEW selector의 원본은 `.codemap/poc/related-terms/development-integration-review-fixes-v1/archive/generation-2/`에 보존했으므로 old artifact index의 해당4 entries를 current live와 같다고 주장하지 않습니다. old code pins를 완화한 replay도 하지 않습니다.

NEW selector는 새 측정/offline/readability/preservation gate 이후 승인 old SHA compare-and-swap으로 같은 후보의 generation3에 연결했습니다. 현재 source/graph의 독립 재검토는 아직 필요하며 fresh159 labels와 semantic aggregation은 미완료입니다. 전체 후속 인계는 `.codemap/poc/related-terms/development-integration-review-fixes-v1/fix-handoff.json`을 확인하십시오. same-original-root full-builder limitation은 그대로이며 relocation 기능은 없습니다.
