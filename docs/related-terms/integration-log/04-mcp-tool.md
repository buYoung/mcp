# 04 — 선택형 synonyms MCP 도구

2026-10-17에 조회·다운로드 API를 MCP에 직접 연결했다. 기본 비활성 상태의 도구 정의와 안내문은 이전 바이너리와 같고, 활성 상태에서 실제 rts5 조회와 계약 e2e 5개가 통과했다. 공식 224개 입력의 기대 분야 적중은 상한 적용 전 193개, 실제 MCP 출력에서는 191개다. **모델 에이전트 사용 평가는 아직 실행하지 않았으므로 04의 전체 완료 조건은 충족하지 않았다.**

## 시작 경로와 상태

`McpServer::new`가 설정이 켜진 경우에만 `RelatedTermsRuntime::start`를 호출한다. `related_terms/runtime.rs`의 `codemap-related-terms` OS 스레드가 다음을 실행한다.

1. `fetch_dataset`으로 사용자 파일 또는 고정 다운로드 파일을 구한다.
2. 반환된 `user_owned`를 바꾸지 않고 `prepare_dataset`에 전달한다.
3. `RelatedTermsLookup`의 첫 연결과 잠금 확인도 같은 스레드에서 마친다.
4. 준비된 조회 객체를 채널로 MCP 요청 스레드에 옮긴다.

요청 처리는 `try_recv`만 사용하므로 다운로드·준비의 잠금을 기다리지 않는다. 개별 조회는 준비된 연결·캐시를 사용한다. 다른 프로세스가 내려받는 동안 작업 스레드가 100ms 간격으로 확인하며 MCP 루프는 기다리지 않는다.

| 상태 | 도구 응답 |
| --- | --- |
| 내려받기 또는 다른 다운로드 잠금 대기 | `Related terms: downloading`이다 |
| 복사·인덱스·최초 연결 준비 | `Related terms: preparing local indexes`다 |
| 준비 또는 조회 실패 | `Related terms: unavailable`과 실제 이유를 반환한다. 실패 후 호출은 백그라운드 준비를 재시도한다 |
| 준비됨 | 입력별 해석과 뜻·관련어를 반환한다 |
| 사전 입력 일치 없음 | `Not in the dictionary`다. 준비 실패와 구분한다 |

서버 시작 시 꺼져 있으면 runtime, 스레드, 폴더, 파일·HTTP client를 만들지 않는다. 설정을 다시 읽은 뒤 다음 요청에서 활성 여부·파일 경로를 맞춘다. 비활성화·종료는 다음 준비·재시도를 취소하지만 이미 실행 중인 제한 시간 있는 HTTP 요청이나 SQLite 준비를 강제로 중단하지는 않는다. 이를 기다리는 thread join을 MCP 루프에서 하지 않는다.

## 스키마와 출력

`tools/mod.rs`는 켜진 경우에만 `synonyms`를 목록에 넣는다. handler는 `tools/synonyms.rs`, 설명은 `tools/instructions/tools/synonyms.md`다. 읽기 전용이며 `readOnlyHint: true`, `openWorldHint: false`다. 기존 `_meta.anthropic/maxResultSizeChars` 적용 경로도 사용한다. `initial_instructions`와 초기 안내는 기능을 켠 경우에만 대체 이름 조회 안내를 추가한다.

```json
{
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "terms": {
      "type": "array",
      "minItems": 1,
      "maxItems": 16,
      "items": {"type": "string", "minLength": 1}
    }
  },
  "required": ["terms"]
}
```

빈 배열·공백 입력·문자열이 아닌 입력·16개 초과·추가 인자는 `isError` 결과로 거부한다. 입력은 식별자 또는 여러 단어 표현이며 서버에서 분리한다. 표시에는 입력 제목, 식별자 구성 표현, 실제 조회 입력, 해석된 seed와 단어, 분야 id·이름과 주제, 관련어 그룹을 넣는다.

상한은 처음 정한 값인 뜻 6개, 직접 관련어 20개, 가까운 말 10개, 같은 주제 단어 10개다. 각 목록에 `showing M of N`을 표시한다. 전체 출력은 공통 출력 상한과 client 전달 상한 중 작은 값(명시되지 않으면 100,000 bytes) 안에 맞춘다. 전체 byte 잘림은 UTF-8 경계를 지키고 별도 안내를 남긴다. 01의 syntax 옵션 기본값 `false`를 유지했고 새 설정 키나 공개 CLI를 만들지 않았다.

## 실제 검증

작업 디렉터리는 `/Users/buyong/workspace/private/buyong-mcp`다.

| 검증 | 결과 |
| --- | --- |
| `cargo check --manifest-path apps/codemap-search/Cargo.toml` | exit 0이다 |
| `cargo test --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests related_terms_tool` | 5 passed다 |
| 같은 test 명령의 `mcp` 필터 | 44 passed다 |
| 같은 test 명령의 `search` 필터 | 39 passed, 1 ignored다 |
| 템플릿 상속 수정 뒤 `config` 필터 | 16 passed다 |
| 실제 rts5 MCP 스모크 | 준비 약 0.965초 뒤 3개 입력을 반환했다. 출력은 7,604 bytes다 |
| 기본 설정 전후 비교 | 전체 `tools/list`, initialize 안내, `initial_instructions` 응답이 기존 바이너리와 같다 |
| 원본 불변 | 실제 스모크 및 224개 평가 전후 SHA-256이 같다 |

계약 테스트는 `tests/e2e/related_terms_tool.rs`에 있다.

- `test_related_terms_tool_disabled_has_no_side_effects`: 목록·안내에 없고 호출은 unknown tool이며 전역 데이터 폴더를 만들지 않는다.
- `test_related_terms_tool_schema_groups_and_input_cap`: schema·모든 그룹 상한·미일치·입력 오류를 실제 MCP에서 확인하고, 사용자 원본 불변과 `local/tool-fixture/ready.json`을 확인한다.
- `test_related_terms_tool_utf8_output_obeys_client_cap`: 512-byte client 한도와 잘림 안내를 확인한다.
- `test_related_terms_tool_downloading_does_not_block_search`: 다운로드 잠금을 잡아 외부 요청 없이 downloading 상태를 유지하고 검색이 준비를 기다리지 않는지 확인한다.
- `test_related_terms_tool_unavailable_retries_local_preparation`: 없는 사용자 파일로 실패한 뒤 파일이 생기면 후속 호출에서 준비가 성공한다.

실제 스모크는 `appVersion`, `abort signal`, `token`을 한 번에 호출했다. `appVersion`에는 `app version`과 구성 단어가 보이며 준비된 파일은 전역 다운로드 폴더가 아니라 `related-terms/local/rts5-75e02862daa29308/`에 생겼다.

### 설정 상속에서 찾은 결함

224개 평가의 최초 실행은 전역 설정을 켰는데 도구가 노출되지 않아 실패했다. 자동 생성한 저장소 템플릿의 활성 `enabled = false`가 전역 true를 덮어쓴 것이 원인이다. 양언어 템플릿에서 이 값을 주석으로 바꿔 기본 false는 유지하고 전역 상속을 허용했다. 수정 뒤 새 작업공간에서 전역 설정만으로 아래 평가가 성공했다. 이 실패를 모델 호출 실패나 데이터셋 결함으로 분류하지 않는다.

## 출력 상한 전후의 224개 평가

모델·에이전트를 호출하지 않는 `sense-eval/mcp_tool_eval.py`를 추가했다. 01에서 저장한 동일 224개 입력의 uncapped JSONL과 브랜치에서 빌드한 실제 MCP 응답의 분야를 비교한다. 원본 데이터셋 hash도 검사한다.

```sh
python3 apps/codemap-search/experiments/related-terms-poc/sense-eval/mcp_tool_eval.py \
  --binary apps/codemap-search/target/debug/codemap-search \
  --uncapped-jsonl /Users/buyong/tmp/rterms-01-5_i16l71/eval-rust.jsonl \
  --output-dir /Users/buyong/tmp/rterms-04-direct/cap-eval-global
```

| 항목 | 실측 |
| --- | ---: |
| 입력 | 224 |
| 상한 전 기대 분야 적중 | 193 |
| 상한 후 기대 분야 적중 | 191 |
| 적중이 사라진 입력 | `base`, `editor` |
| 최종 출력 합계 | 349,405 bytes / 347,997 characters |
| 단건 최대 출력 | 3,607 bytes |

두 입력은 기대 분야가 처음 6개 뜻 밖에 있어서 상한으로 잘렸다. 각 입력의 기대 분야·전후 분야와 실제 text는 `cap-eval-global/report.json`·`responses.jsonl`에 있다. 상한을 바꾸지는 않았다. 위 출력 합계는 모델 평가의 실행당 평균 출력과 다른 집계이므로 서로 직접 비교하지 않는다.

## 모델 평가 — 미실행

기존 shell `usage-short.md` 기준은 `docs/related-terms/agent-usage-eval.md`의 기록이다.

| 모델 | 경로 | 실행 수 | 호출률 | 답에 쓴 출력 비율 | 도구 출력 문자 기록 | 프롬프트 토큰 기록 |
| --- | --- | ---: | --- | --- | --- | --- |
| haiku | 기존 shell | 12 | 83% | 0.39 | 16,781 | 23,406 |
| luna | 기존 shell | 12 | 92% | 0.30 | 46,414 | 8,426 |
| haiku | 이번 MCP | 미실행 | 미확인 | 미확인 | 미확인 | 미확인 |
| luna | 이번 MCP | 미실행 | 미확인 | 미확인 | 미확인 | 미확인 |

사용자가 2026-10-17에 평가용 모델 실행도 보류하기로 명시했다. 구현 위임뿐 아니라 Haiku·Luna 사용 평가와 이후 제거 표본의 독립 판정도 실행하지 않고, 로컬 구현·자동 검증을 계속한다. 기존 `agent_usage_eval.py`를 MCP 방식으로 바꾸고 두 모델의 호출률이 42%를 넘는지 확인하는 작업은 남아 있다. 이 표는 평가 완료나 해당 기준 통과를 뜻하지 않는다.

## 근거 파일과 남은 범위

실행 로그·비활성 baseline·실제 스모크·224개 응답은 `/Users/buyong/tmp/rterms-04-direct/`에 보존했다. `README.md`·`README.ko.md`에는 도구·설정·상한과 아직 게시되지 않은 데이터 릴리스 대신 로컬 경로를 지정해야 한다는 조건을 적었다. 공개 데이터 릴리스 게시와 공개 URL smoke는 사용자 선택에 따라 보류 중이다. 코드베이스 근거·갱신·제거 판정은 05 범위이며 아직 추가하지 않았다.
