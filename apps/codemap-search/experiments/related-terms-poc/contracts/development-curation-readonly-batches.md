# 독립 read-only batch 계약 v1

기존 [의미 기준](development-curation.md#3-에이전트-직접-판정-기준), `decision/2`, `projection/1`, 고정 packet/manifest는 변경하지 않는다. 새 경로는 **입력 분할 → read-only JSON 분류자 → 별도 writer의 검증·기계적 변환**이다. 기존 셸 요청을 숨기는 renderer가 아니다. active handoff/README/evaluator/builder와 연결하지 않으며 데이터셋 생성·활성화는 별도 승인 대상이다. 현재 단계는 준비·기존 자료 검증만 한다.

## 입력과 배정

Helper: `apps/codemap-search/experiments/related-terms-poc/batch_development_curation.py`.
새 ignored 루트: `.codemap/poc/related-terms/development-curation/readonly-batches-v2/`. 이전 v1 publication은 bytes 그대로 보존하며 당시 helper/계약은 별도 `readonly-batches-history/v1-sources/`에 고정 SHA로 보관한다. v2는 이벤트 검증 결함 수정 publication이고 의미 기준·결과 schema·논리적 배정은 동일하다.

- `manifest.json`: 호출자가 `plan --prefix-root`로 전달한 기존 prefix 절대경로, 고정 입력·helper·본 계약 해시, 7필드 shard 목록, 전체 분모·artifact 경로. `verify-plan` 이후에는 이 고정 바인딩을 다시 사용한다.
- `verified-plan.json`, `execution-handoff.json`: 실제 검증 결과·총량·보존 증거와 작은 실행 인계. 준비 완료는 분류/병합 완료가 아니다.
- `prefix/*.jsonl`: 기존 operations 236 / technical 678 / catalogs 261 / abbreviations 664 = **1,839**개의 원본 bytes 그대로 스냅샷. 각 원본 절대경로·고정 SHA·개수는 manifest에 있다. 저장 안 된 판단은 포함하지 않는다. 원본 bytes를 저장했다는 사실은 직접 판독 증거가 아니다.
- `assignments/*.json`: 절대 page 경로·page ID·SHA·바이트, packet/projection/source identity, 완결 concept별 input/projection hash·원본 line·모든 lexicalization ID·범위·연결 페이지. 의미 필드·원래 표기는 **기존 고정 페이지를 직접 read**한다. 페이지의 `context_only_prefix_concept_ids`는 판정 대상이 아니다.
- 남은 **5,308**개 concept을 정확히 한 번 배정한다. prefix와 합해 **7,147 concept / 17,830 lexicalization**이다. 각 concept의 모든 oversized 조각과 연결 페이지는 같은 connected group이다. 보통 최대 6페이지 / 48concept / 160표기를 목표로 연속 group을 묶는다. 완결 group 자체가 목표를 넘으면 단독 batch로 보존한다. 목표는 자료 제외 기준이 아니다.
- `results/<batch_id>.json`: 런타임의 공식 output 바인딩이 저장하는 충돌 없는 절대 경로. 분류자는 파일을 쓰지 않는다. 기존 runtime 결과를 복사·교체하지 않는다.
- `result-schema.json`: 사용자 지정 JSON schema 그대로. 모든 object에서 extra field를 금지한다. 분류자는 이 schema도 read한다.

## 분류자 (모든 자식 `gpt-6.1-sol`)

assignment·본 계약·result schema·모든 source page를 **read 도구만**으로 전범위 읽는다. shell/renderer/파일변경/재위임은 금지한다. 부모는 판단·오케스트레이션만 한다. 정상 read 한 번으로 전체가 반환되면 그대로 읽는다. 잘렸거나 limit로 끝났으면 실제 반환 범위 이후 offset로 계속 읽는다. 한 concept의 모든 fragment 확인 전에는 그 판정을 내보내지 않는다.

개발 직접 관련성은 원문 sense/context와 연결해 설명한다. keyword/domain/source/저장소 출현은 최종 포함 근거가 아니다. 모든 **원래** lexicalization에 대해 `should_keep`와 비어 있지 않은 개별 근거를 판단한다. 표기·관계·의미를 생성하거나 전이하지 않는다. include는 유지 표기가 적어도 하나, exclude/uncertain은 전부 false다.

유일한 새 의미 입력은 runtime `structured_output`의 JSON value:

- `status`: `complete | partial | blocked`
- `batch_id`, `assignment_sha256`: 해당 shard와 정확 일치
- `reviewed_page_ids`: 읽은 고정 페이지 ID, assignment 순서·중복 없음
- `decisions`: concept마다 `concept_id`, `decision`(`include | exclude | uncertain`), `development_rationale`, `lexicalization_decisions`(모든 원래 ID의 `lexicalization_id`, boolean `should_keep`, `rationale`)
- `remaining_concept_ids`: 아직 완결 판정하지 않은 배정 ID, 중복 없음
- `blocker`: null 또는 구체적 막힘 이유. blocked에는 비어 있지 않은 이유 필수

complete는 모든 배정 concept/표기·페이지 완료, remaining=[], blocker=null이다. partial/blocked는 남은 ID를 빠짐없이 보고한다. 중간 판단을 억지로 complete로 만들지 않는다. 근거 문자열과 boolean은 writer가 수정하지 않는다.

## writer · read 증거

`runs-binding.json` 입력은 `{ "trace_root": "<공식 traceRoot 절대경로>", "runs": [{ "batch_id": "...", "run_id": "...", "runtime_result_path": "<선택: 공식 완료 result 파일 절대경로>" }] }`다. 부모는 공식 runs 결과의 ID/binding만 전달하며 변환 코드나 파일 생성 코드를 실행하지 않는다. writer가 helper를 실행한다. 기본 result 위치는 `<trace_root>/<run_id>/workflow-result.json`(공개 runner의 workflow-await seam)이다. 다른 공식 위치는 명시한다(공개 runner의 형제 `async-subagent-results/<run_id>.json`도 지원). traceRoot와 run ID는 호출자의 공식 runtime 인계값만 사용하며 helper·본 계약에 사용자 홈/uid 경로를 하드코딩하지 않는다. 추정 경로를 만들어 증거로 쓰지 않는다.

Extractor는 각 **완료 single native run**의 `status.json`, 공식 result, `events.jsonl`에서 필요한 metadata만 선택한다. status/run ID·cwd·complete/exit0·공식 model (`provider/gpt-6.1-sol:thinking` 허용)·공식 `savedOutputPath`·structured value와 지정 output 실제 JSON/SHA를 대조한다. 실제 원본 결과는 results에 그대로 남는다.

Native 이벤트는 `tool_execution_start`의 `toolCallId/toolName/args/subagentRunId`와 같은 call ID의 `tool_execution_end` `result/isError`를 연결한다. 모든 end는 앞선 동일 call ID의 등록된 start·동일 도구·성공 결과를 요구한다. 시작 없는 end와 read/structured_output 외 도구, 배정 외 read, 미완료/중복 호출, 실패 read는 거부한다. structured_output **start 전에** 이전 read가 모두 성공 종료되고 필수 입력 전범위 coverage가 충족되어야 한다. 그 이후 새 read/추가 tool start를 거부하되 해당 structured_output의 정상 end와 비도구 runtime 종료 이벤트는 처리한다. `read.result.content`의 실제 반환 text를 고정 파일의 해당 line 문자열과 대조한다. 요청 offset/limit만으로 판독 성공을 인정하지 않는다. 알려진 read continuation suffix만 허용하며 잘린 경우 실제 반환 range marker 또는 native truncation outputLines와 **고정 본문 전체 일치**를 요구한다. 일반 read의 content/continuation을 사용하며 제거된 guard 전용 필드에 의존하지 않는다. 형식이 불명확하면 fail-closed한다. 여러 read의 **성공한 실제 반환 line 범위 합집합**이 assignment/계약/schema/각 source page 전범위를 덮어야 한다. 선언한 page ID만 믿지 않는다.

`evidence/<batch_id>-<run_id>.json`에는 read call ID·도구·허용된 절대경로·고정 파일 SHA/총 line·실제 반환 범위·truncated·반환 text SHA·run/model/result binding과 read start/end 및 structured_output start의 원본 event line 위치만 기록한다. 이 위치로 판독 완료가 출력 제출보다 앞서는지 원본과 대조할 수 있다. SHA는 trace가 직접 제공한 척하지 않고 **고정 입력과 반환 본문을 비교해 계산한 값**이라고 표시한다. 원본 trace는 읽기 전용이며 전체 trace/추론문/환경/자격증명/비관련 본문을 출력하거나 복제하지 않는다. 독립 reviewer는 ledger를 그대로 믿지 않고 extractor와 원본 공식 runtime metadata를 대조해야 한다. 이는 **텍스트 전달 증거이지 의미 판단 정확성 증명은 아니다**. 지원하지 않는 wire/누락 metadata/정책·권한 실패는 partial 증거를 남기고 중단한다.

writer는 누락/중복/미등록 concept·표기·page, binding 오류, 빈 근거, include/keep 불일치를 거부한다. decision/2의 metadata/hash/review basis·keep ID·근거 map만 기계적으로 복원한다. 기존 prefix의 의미와 bytes는 절대 수정하지 않는다. incomplete 결과는 검증할 수 있지만 merge 완료로 선언하지 않는다.

## 명령과 최종 소비 경계

모든 cwd는 실제 repository root(`execution-handoff.json.cwd`). 실행 전 정확한 명령/cwd와 한계를 고지한다. `-B`로 pycache 쓰기를 피한다. 의존성/새 테스트파일/사례/설정/guard/세션을 만들거나 바꾸지 않는다.

```bash
# 이번 준비 단계에서만 실행: 새 영역 생성(동일 bytes만 재사용), 재검증
python3 -B apps/codemap-search/experiments/related-terms-poc/batch_development_curation.py plan --prefix-root <호출자가_전달한_기존_prefix_절대경로>
python3 -B apps/codemap-search/experiments/related-terms-poc/batch_development_curation.py verify-plan
# 후속 writer 전용; 이번 단계에서는 실행하지 않음
python3 -B apps/codemap-search/experiments/related-terms-poc/batch_development_curation.py extract-evidence --bindings <절대 bindings.json>
python3 -B apps/codemap-search/experiments/related-terms-poc/batch_development_curation.py validate-results --bindings <절대 bindings.json>
python3 -B apps/codemap-search/experiments/related-terms-poc/batch_development_curation.py merge --bindings <절대 bindings.json>
```

plan/verify-plan은 기존 공식 projection·packet·source archive/dataset 해시/원표기·페이지/조각과 prefix의 정확한 14필드·기존 판정 계약, HEAD/staging/보존 baseline을 검증한다. 생성된 plan을 다시 계산해 artifact bytes까지 대조한다. 자동으로 의미를 판정하지 않는다.

extract-evidence는 원본결과/공식 run을 재검증하고 최소 ledger만 새 ignored evidence에 저장한다. validate-results는 쓰지 않는다. merge는 plan·고정 manifest·prefix·모든 결과·원본 run read 증거를 다시 검증하고 새로운 `canonical/<packet>.jsonl`에 prefix bytes를 그대로 선두로 보존한 뒤 새 판단을 canonical 순서로 덧붙인다. 네 packet 각각 **기존 공식 `project_development_curation.py validate-decisions <path> --packet <name>` exit0**를 요구한다. 실패 보고를 남기고 완료를 선언하지 않는다. 모두 성공하면 `canonical/decisions.jsonl`과 `canonical/validation.json`만 별도 생성한다. 기존 runtime 판정 파일·active dataset은 교체하지 않는다. 기존 builder/lookup-handoff는 현재 고정 dataset/handoff만 소비하므로 새 canonical 판정을 자동 소비하지 않는다. 서빙 데이터셋 연결·빌드·평가는 이 계약 밖이다.
