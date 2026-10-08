# 완료 workflow receipt 기반 writer 검증 revision v3

분류 계약·의미 기준·`decision/2`·`projection/1`은 frozen `development-curation-readonly-batches.md` 그대로다. Companion `verify_development_curation_receipts.py`는 **새 publication 증거 취득 경로**이며 기존 helper를 속이는 adapter가 아니다. 모듈/status monkeypatch, 소멸한 result 합성, 기존 출력 복사·교체, 새 분류/재라벨링을 하지 않는다.

## 불변 입력과 별도 revision

새 ignored root는 `.codemap/poc/related-terms/development-curation/durable-verification-v3/`이다. 앞선 verifier revision v1은 launch inventory를 호출 정책과 혼동해 실제 pilot을 거부했다. v2는 이를 수정했지만 공식 structured_output의 별도 acceptanceReport wrapper를 지원하지 못해 같은 pilot을 거부했다. 두 revision의 manifest/실패/binding과 당시 source/계약 bytes 및 SHA를 각각 `durable-verification-v{1,2}/{sources/,supersession.json}`에 보존한다. v3는 실제 공식 wrapper 호환성 수정이며 분류 입력 revision v2와 별개의 번호다. `manifest.json`에 verifier/본 계약 SHA와 frozen v2 input manifest/producer/원래 분류 계약 SHA를 **구분해** 고정한다. 공개 runtime source의 정확한 파일·SHA·증거 의미, 이미 존재하는 readonly-batches publication/history/results의 원천 파일 SHA도 고정한다. 기존 v2의 helper/계약/manifest/142 assignments/schema/prefix 150파일과 pilot JSON 및 이전 partial/failed 기록은 변경하지 않는다. 이후 verifier 수정은 이미 발행한 revision을 덮어쓰지 않고 새 revision 승인으로 진행한다. 분류자가 본 계약을 새로 읽을 필요는 없다. 필수 read 입력은 frozen assignment가 지정한 원래 계약/assignment/schema/페이지 그대로다.

기존 `plan(False)`, `inputs()`, `convert_result`, `confirmed_read`, `canonical_json` 및 공식 `project_development_curation.py validate-decisions`를 재사용한다. v2 검증·prefix 1,839개 byte 보존·잔여 5,308개/총 7,147개·17,830표기의 전체 배정 요건은 완화하지 않는다.

## 공식 publication 원천

호출자가 `--bindings`의 절대경로 JSON을 새 revision 안에 제공한다:

```json
{
  "trace_root": "<공식 traceRoot 절대경로>",
  "runs": [{
    "batch_id": "<v2 batch id>",
    "run_id": "<원래 child run UUID>",
    "workflow_run_id": "<완료된 분류 workflow UUID>",
    "workflow_key": "<분류 workflow의 exact child key>",
    "receipt_path": "<trace_root>/<workflow_run_id>/workflow-receipt.json"
  }]
}
```

User-home/uid는 코드에 하드코딩하지 않는다. 모든 경로는 절대/non-symlink이며 receipt는 exact 공식 parent directory 안에 있어야 한다. 진행 중인 parent receipt는 사용하지 않는다. 별도 writer workflow의 receipt를 분류 workflow receipt 대신 사용할 수 없다.

공개 runtime source의 증거 사슬:

- `single-output.js`의 실제 write 성공 또는 changed file read 성공에서만 `savedPath`가 나온다. 요청 경로만으로 savedPath를 만들지 않는다.
- runner는 exit0에서 schema JSON을 저장하고 이 savedPath를 `savedOutputPath`로 전달한다. 저장 실패면 reference가 없다.
- `chain-root-attachment.js`의 imported publication은 savedOutputPath를 전달한다. status-only fallback은 success=false이며 savedOutputPath가 없다.
- `importWorkflowAwaitedChildResult`는 원본 publication을 import한 뒤 transient 파일/index/pending을 소비한다. `claimWorkflowAwaitedResult`도 rename 경로를 갖는다. 따라서 standalone 파일의 사후 부재는 실패 자체가 아니다.
- `workflowChildResult`는 실제 child savedOutputPath/outputReference.path에서 workflow outputReference를 만든다. receipt writer가 해당 reference/latestRunId를 영구 기록한다. child inventory에는 completed 상태와 exact run/key/model이 들어간다.

검증은 원래 child의 status와 parent/key, parent status의 complete/workflow/cwd/receipt 경로, version1/state complete receipt, inventoryComplete=true/workflowState=completed, exact childId/runId/model/agent, entry.key/latestRunId/continuation/outputReference를 모두 연결한다. complete 부모에서도 selected child가 failed/paused/stopped/running이면 거부한다. partial `terminalOutcome`, acceptance recovery, 외부 adapter 및 기록된 오류/중단/시간초과는 거부한다.

원래 child는 single/complete/cwd/gpt-6.1-sol/한 step complete exit0이어야 한다. 별도 `process-terminal.json`의 observed proof가 status와 전체 일치하고, 같은 runner instance의 실제 close exit0/signal=null이어야 한다. recovery descriptor의 model/cwd/정식 reviewer agent/launch digest/등록 도구 inventory/정확 schema/지정 output을 status와 대조한다. 부모가 공식 capabilities로 확인한 builtin reviewer의 exact 등록 inventory는 `read, grep, find, ls, watchdog_diff, contact_supervisor`이며 schema-bound structured_output은 runtime이 주입한다. manifest에 이 승인 기준을 기록하고 descriptor의 실제 list와 정확 대조한다. 새 도구권한이나 profile 변경은 하지 않는다. launch 등록 inventory와 실제 호출 정책은 다르므로 미사용 도구의 등록을 호출 위반으로 취급하지 않는다. bash/edit/write/외부실행 등 예상 밖 inventory는 거부하며, 실제 호출은 아래대로 read/structured_output만 허용한다. runtime captured JSON·공식 captured schema·status structured value·실제 지정 JSON이 전체 일치해야 한다. 각각 실제 파일 SHA를 계산한다. native 실행은 schema-bound launch + 정확한 native read/structured_output 이벤트의 양성 증거로 입증한다. 공개 preflight는 external CLI/job + outputSchema를 거부하고 external capability는 trusted native tool events/structured output을 제공하지 못한다. external flag 부재만으로 native를 인정하지 않는다.

## 보존되지 않은 내부 flag와 검증한 속성 구분

Standalone result의 모든 내부 flag가 receipt/status에 남지는 않는다. 다음 동등 양성 검증은 부모의 명시 승인에 따른다. 누락 flag를 `false`라 합성하거나 runtime이 false라고 보고했다고 쓰지 않는다. Ledger는 `unretained_raw_result_flags=not-recorded`와 실제 기록된 flags, 검사한 속성 및 원천을 구분한다.

- 소비하는 결과는 raw prose/stream이 아니라 구조화 JSON이다. complete/exit0 + captured/status/지정 JSON 전체 일치 + 정확 schema/모든 concept·원표기 ID/remaining 없음/개별 근거·keep 관계 검증으로 **구조화 결과 완전성**을 입증한다. raw prose outputPartial은 보존 안 됨으로 남긴다.
- 실제 저장 reference의 출처 사슬 + 정확 parent/run/key/model + 실제 파일/captured JSON 일치/SHA로 **필수 판정 JSON 저장 성공**을 입증한다. 기록된 반대 저장 실패는 거부한다. 별개 보조 debug artifact 오류 유무가 보존되지 않은 것은 한계이지 판정 JSON 실패로 합성하지 않는다.
- 완료 receipt/selected child, child status/step exit0, 정상 process-terminal, 기록된 오류·중단·timeout 부재의 적용 범위를 모두 대조한다. 존재하는 부정적 증거를 무시하지 않는다.

이는 publication/텍스트 전달/구조 증거이지 **의미 정답 보장**이 아니다. 원천이 불명확하거나 핵심 속성을 입증하지 못하면 fail-closed한다.

## read ledger와 순서

Frozen `extract_run`과 동일한 native event state machine을 명시적으로 사용하고 원래 `confirmed_read`를 호출한다. 전체 trace/추론/환경/자격증명/비관련 본문은 출력·복제하지 않는다. 비도구 이벤트는 streaming 단계에서 즉시 버린다.

read와 structured_output만 허용하고, assignment 외 경로·orphan end·중복/미완료 call·tool identity/run 불일치·isError·알 수 없는 wire를 거부한다. 실제 성공 read 반환 text는 고정 본문의 line·SHA와 대조한다. offset/limit/page 선언은 단독 증거가 아니다. 잘린 경우 원래 confirmed_read의 반환 range/continuation 규칙을 유지한다. assignment/원래 계약/schema/모든 source page 전범위의 성공 반환 합집합과 모든 read 종료가 structured_output **start 전에** 충족되어야 한다. 이후 새 tool start를 거부하고 유일한 정상 output end만 허용한다. output args.value도 실제 지정 JSON과 같다. 공개 `createStructuredOutputToolParameters`가 허용하는 outer args는 `{value}` 또는 `{value, acceptanceReport}`만 지원한다. 추가 wrapper 필드는 거부한다. acceptanceReport는 object여야 하고 공식 captured schema와 같은 runtime directory의 `acceptance-report.json`이 args의 report와 전체 일치해야 한다. 이 report는 collector가 별도로 저장하는 runtime metadata이며 classification value/schema에 합치지 않는다. ledger에는 wrapper 필드명과 report 원천 경로/SHA만 기록하고 본문은 복제하지 않는다. classification JSON의 extra field 금지·정확 value equality·판독 전범위/순서는 그대로다.

Ledger는 새 revision `evidence/`에만 기록한다. read call ID·반환 범위·고정/반환 SHA·원본 event line·output start line, receipt/status/terminal/launch/captured/output의 path/SHA와 property 검증 결과만 남긴다. 검사 후 원천 SHA를 다시 대조한다. 오류 report는 새 revision에 보존하고 중단한다. 기존 partial/failed 기록을 덮어쓰지 않는다.

## writer CLI와 최종 경계

모든 cwd는 repository root. 실행 전 정확한 명령/cwd를 고지하고 `python3 -B`를 사용한다. 설치소스는 읽기 전용이며 의존성/설정/테스트/fixture/린트/커밋을 추가하지 않는다.

```bash
python3 -B apps/codemap-search/experiments/related-terms-poc/verify_development_curation_receipts.py prepare --runtime-source-root <공개SDK-src-절대경로>
python3 -B apps/codemap-search/experiments/related-terms-poc/verify_development_curation_receipts.py verify-batch --bindings <새revision-절대JSON> --batch-id <한batch>
# 모든 분류 workflow 완료 후 별도 승인된 writer workflow 전용:
python3 -B apps/codemap-search/experiments/related-terms-poc/verify_development_curation_receipts.py validate-results --bindings <새revision-전체142binding-절대JSON>
python3 -B apps/codemap-search/experiments/related-terms-poc/verify_development_curation_receipts.py merge --bindings <동일전체binding>
```

verify-batch는 exact 한 실제 batch binding만 받아 부분검증 report를 낸다. final validation/merge는 142개 전부의 unique batch/run/parent-key와 completed receipts, 모든 complete 결과·read ledger 및 잔여 concept 집합=5,308을 요구한다. 일부 결과로 merge할 수 없다.

merge는 전체검증을 다시 수행하고 새 revision `canonical/` 안에 원본 prefix bytes를 그대로 앞에 보존한 뒤 같은 canonical ID 순서로 mechanical decision/2만 덧붙인다. boolean/rationale는 수정하지 않는다. 네 packet 모두 기존 공식 validate-decisions exit0를 요구한다. 중간실패는 incomplete report를 남기고 완료를 선언하지 않는다. 모두 통과해야 총 7,147행 통합 canonical/validation을 만든다. 기존 v2 결과·canonical·active dataset은 변경하지 않는다. 현재 승인에서는 **실제 pilot 하나만** 검증하며 나머지 분류/전체검증·merge·dataset build/activation은 실행하지 않는다.
