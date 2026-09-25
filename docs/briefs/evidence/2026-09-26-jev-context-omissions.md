# Jev 문맥 보존 패치 후 남은 근거 생략 분석

R25 단일 실측의 생략 본문 29개를 전부 대조했다. **중요한 본문 일부는 search에서 생략된 뒤 복원됐고, read에서 생략된 WebSocket 응답 계약 함수 5개·89행은 복원되지 않았다.** 현재 패치가 모든 중요한 근거를 보존한다고 결론 내릴 수 없다.

원인은 두 경로로 나뉜다. search는 새 문맥 부족 상태를 계산해 공통 보존 정책에 전달하지 않는다. read/grep의 새 문맥 수집은 같은 파일 안의 근거를 수집하므로, 다른 파일의 호출자가 업무 관련성을 보여 주는 공통 함수에는 여전히 빈틈이 있다. 이번 작업은 이 상태를 분석하고 저장하는 작업이며 추가 제품 동작 수정은 하지 않았다.

## 범위와 방법

- 분석 대상: `20260926-jev-context-25-single`의 최종 답변, 전달 기록, Jev 판정 로그, 동결된 대상 소스 및 측정용 서버 소스.
- 새 메인 모델 실측 0회, 새 Jev API 호출 0회, 제품 재생 0회. 저장 기록을 대상으로 상세 분석 1회를 수행했다.
- Jev 생략 안내의 파일·줄 범위를 모든 MCP 응답과 일반 파일 읽기 출력의 실제 원문 행에 대조했다. 선언 목록이나 파일 경로만 나온 것은 본문 전달로 세지 않았다.
- 판정 로그의 `is_retained=false`와 Jev 생략 안내로 삭제 주체를 확인했다. 공통 중복 제거, 출력 절단, 미요청 범위와 구분했다.
- [기계 판독 증거](./2026-09-26-jev-context-omissions.json)에 29개 목록, 소스 해시, 전달 행 수, 세션 위치, 판정값과 한계를 보존했다. 원본은 로컬 체크포인트 `~/.codex/checkpoints/codemap-search-comparison/runs/20260926-jev-context-25-single/`에 있다. 분석 스크립트·검증 로그·커밋 분리 기록은 인접한 `20260926-jev-context-26-analysis/`에 있다.

## 원인과 관측 결과

| 구분 | 확인된 원인·구조 | 관측 결과 | 확정하지 않은 부분 |
|---|---|---|---|
| search의 문맥 보호 | 후보 생성 시 `has_missing_context=false`; 미해결 호출은 개수만 증가. 보조 근거 확보 실패도 그 상태로 전환하지 않음 | 중요한 발행·구독·잠금 본문도 낮은 판정 뒤 생략 | 각 후보에 어떤 추가 근거가 있었으면 판정이 바뀌는지 |
| read의 파일 밖 호출자 | live 수집은 현재 읽은 버퍼와 같은 파일의 선언·호출 관계에 한정됨 | 관련 호출자가 메인 문맥에 있었어도 `WsMessageFactory` 5개 본문이 생략 | 추가 호출자 근거를 주면 실제 판정·토큰이 얼마나 달라지는지 |
| 판정 소비 | 등록 질문 2개, `match=any`, 임계값 0.70. 두 질문 모두 0.30 이하면 `NoMatch` | formatter 5개의 10개 응답이 0.06–0.11이며 전부 `no_match` 생략 | 제공자가 그 확률을 선택한 내부 이유 |
| 연결 보존 | 직접 연결 상대가 `Matched`일 때 보호. 단순히 보존되거나 `Uncertain`인 상대는 조건을 충족하지 않음 | 불확실 문맥을 모두 연결 보존으로 확장하지 않음 | 이 규칙을 바꿨을 때 품질·출력량 변화 |

### search: 공통 정책과 상태 생산자의 연결이 다름

`FilterEntity`는 [`has_missing_context=false`](../../../apps/codemap-search/src/tools/search/jev.rs)로 만들어진다. search의 호출 해석 실패는 [`unresolved_calls`를 증가](../../../apps/codemap-search/src/tools/search/jev.rs)시키지만 문맥 부족 상태를 켜지 않는다. [보조 본문 확보](../../../apps/codemap-search/src/tools/search/jev/evidence.rs)가 실패하면 해당 본문이 없는 채로 진행한다. 예산 초과는 `is_context_clipped`로 별도 보존하지만 근거 부재와 같은 조건은 아니다.

반면 read/grep은 [`context::capture`](../../../apps/codemap-search/src/tools/live_symbols/jev.rs)의 결과를 `has_missing_context`에 넣는다. [공통 보존 정책](../../../apps/codemap-search/src/tools/search/jev.rs)은 이 값이 켜져 있으면 보호한다. 즉 정책을 공유한다는 사실만으로 search가 같은 문맥 보호를 받는 것은 아니다. 이 차이는 측정 소스와 이번 커밋 대상에서 모두 확인했다.

search의 20개 생략 후보는 두 질문 모두 0.30 이하였고 최종 결정이 `no_match`였다. 출력 절단 0회, `deferred_bodies=0`이므로 아래 생략을 전달 상한 탓으로 돌릴 수 없다. 검색 질문은 이벤트 흐름뿐 아니라 메시지 계약·실행 조건·등록·실패·생명주기도 포함했다. 단순히 질문에서 이러한 주제를 빼먹은 사례도 아니다.

### 복원된 중요한 search 근거

| 본문 | 소스 범위 | 역할 | 후속 확인 |
|---|---|---|---|
| `RPMRedisService.publishMessage` | L618–626 | `{ action, data }`를 만들어 Redis에 발행 | 9행 전체, 일반 파일 읽기 |
| `publishMessageCompressed` | L628–638 | 같은 계약의 압축 발행 | 11행 전체, 일반 파일 읽기 |
| `MeasureWorkerLock.acquireLock` | L13–16 | worker의 만료·NX 잠금 처리 | 4행 전체, 일반 파일 읽기 |
| `releaseLock` | L18–26 | 잠금 확인·해제 | 9행 전체, 일반 파일 읽기 |
| `SubscriptionWsGateway.handleSubscribe` | L91–131 | 인증 확인 후 측정 로그 구독으로 전달 | 41행 전체, 후속 read |

위 함수들은 초기 search의 세션 기록 25에서 생략됐다. WebSocket 구독 함수는 기록 92에서 복원됐고, 잠금과 공통 발행 함수는 기록 128·130의 일반 파일 읽기로 복원됐다. 두 일반 파일 읽기는 1,564B였으며 출력 35행 중 33행이 앞서 생략된 범위였다. 재조회가 필요했다는 사실과 그만큼의 토큰을 확실히 절약할 수 있다는 주장은 구별한다.

### read: 소켓 응답 계약 5개 함수가 끝까지 복원되지 않음

메인 모델은 세션 기록 102에서 `enrollment-logs-subscription.service.ts:49`의 `WsMessageFactory.normalizeResponse(response)` 호출을 읽었다. 이 호출은 측정 로그 구독 초기화 응답을 변환하는 코드다. 같은 파일의 L58·64·70·76에서도 reload/pause/resume/heartbeat 응답 변환에 사용한다. 직후 기록 103에서 factory 파일 L1–130을 읽었지만 Jev가 다음 본문을 모두 생략했다.

| 본문 | 범위·행 수 | 본문에서 확인되는 응답 계약 | 이후 원문 전달 |
|---|---:|---|---:|
| `success` | L6–23 · 18행 | 성공 응답의 `event`, `statusCode`, `message`, `data` 구성 | 0행 |
| `failure` | L25–43 · 19행 | 오류 상태·메시지·데이터 구성 | 0행 |
| `normalizeResponse` | L45–80 · 36행 | `payload → data`, `status → statusCode` 변환 | 0행 |
| `resolveRequestId` | L82–84 · 3행 | 응답의 요청 식별자와 인자의 우선순위 | 0행 |
| `withRequestId` | L86–98 · 13행 | 요청 식별자가 있을 때 응답에 추가 | 0행 |
| 합계 | **89행** | 측정 로그 구독 응답의 공통 계약 | **0행** |

**메인 모델이 알고 있는 호출 관계가 다음 Jev read 요청의 근거로 공유되지는 않았다.** [`context.rs`](../../../apps/codemap-search/src/tools/live_symbols/jev/context.rs)는 현재 파일의 구문 정보로 같은 파일 호출자·피호출자·참조를 모은다. live의 [FilterInput](../../../apps/codemap-search/src/tools/live_symbols/jev.rs)은 별도 cross-file `supporting_sources`가 빈 상태이며 메인 세션 이력을 받지 않는다.

이 호출에서는 6개 본문 중 5개가 판정 대상이 됐고 5개 모두 생략됐다. 10개 확률은 0.06–0.11, `uncertain=0`, `linked=0`이었다. 낮은 점수가 보존 규칙에 막히지 않고 실제 본문 제거로 이어졌다는 것은 로그로 확인된다. 파일 밖 관련 호출자의 부재는 코드로 확인되는 문맥 범위의 한계다. 제공자의 내부 추론은 저장되지 않았으므로 확률이 낮아진 유일한 원인이라고 단정하지 않는다.

이 5개 함수를 단순히 무관한 유틸리티라고 분류할 수 없다. 측정 로그 구독·제어 응답이 실제로 이 함수를 소비한다. 다만 최종 답변의 잘못된 특정 문장이나 기존 11개 항목의 실패가 이 제거 때문에 생겼다는 인과까지 입증한 것은 아니다.

## 생략 본문 전체 대조

총 search 20개·read 9개·grep 0개다. 최종 문맥에 **전체 본문이 들어온 것은 5개, 일부 행만 들어온 것은 5개, 본문 행이 전혀 들어오지 않은 것은 19개**다. 선언 행이나 동일 호출의 다른 구획에서 일부 소스 행이 보이는 경우도 있어, 생략 안내 하나를 전체 소스 소실과 동일시하지 않았다.

| # | 도구 | 본문 | 범위 | 최종 확보 행/본문 행 |
|---:|---|---|---|---:|
| 1 | search | `MeasureWorkerRunnerService.constructor` | 12–15 | 0/4 |
| 2 | search | `MeasureWorkerQueueConsumerService.constructor` | 17–23 | 2/7 |
| 3 | search | `MeasureWorkerMeasureRepository.createMeasure` | 22–56 | 2/35 |
| 4 | search | `saveMeasureDetails` | 62–64 | 0/3 |
| 5 | search | `saveMemo` | 66–81 | 0/16 |
| 6 | search | `MeasureWorkerPersistenceService.constructor` | 26–33 | 2/8 |
| 7 | search | `createPhotoEntity` | 221–242 | 0/22 |
| 8 | search | `MeasureWorkerLookupRepository.findDeviceCodes` | 61–70 | 0/10 |
| 9 | search | `findLoginUser` | 72–81 | 0/10 |
| 10 | search | `RPMRedisService.publishMessage` | 618–626 | 9/9 |
| 11 | search | `publishMessageCompressed` | 628–638 | 11/11 |
| 12 | search | `MeasureWorkerLock.acquireLock` | 13–16 | 4/4 |
| 13 | search | `releaseLock` | 18–26 | 9/9 |
| 14 | search | `MeasureWorkerPhotoStorage.constructor` | 17–22 | 0/6 |
| 15 | search | `uploadPhotoFile` | 24–54 | 0/31 |
| 16 | search | `SubscriptionWsGateway.handleSubscribe` | 91–131 | 41/41 |
| 17 | search | `buildSubscribeFrame` | 16–22 | 0/7 |
| 18 | search | `DatagridSyncService.handleSubscribe` | 126–161 | 0/36 |
| 19 | search | `MeetingController.addSubscribeInfo` | 51–53 | 0/3 |
| 20 | search | `emitAuthAuditEvent` | 105–137 | 0/33 |
| 21 | read | `EnrollmentLogsSocketService.constructor` | 33–39 | 1/7 |
| 22 | read | `RPMRedisService.consumeOneTimeValue` | 106–113 | 0/8 |
| 23 | read | `SubscriptionWsGateway.constructor` | 31–38 | 1/8 |
| 24 | read | `WsMessageFactory.success` | 6–23 | 0/18 |
| 25 | read | `failure` | 25–43 | 0/19 |
| 26 | read | `normalizeResponse` | 45–80 | 0/36 |
| 27 | read | `resolveRequestId` | 82–84 | 0/3 |
| 28 | read | `withRequestId` | 86–98 | 0/13 |
| 29 | read | `AppModule.configure` | 202–206 | 0/5 |

0행이라는 사실만으로 모두 필요한 근거였다고 판정하지 않았다. 나머지 생성자, 저장·파일·조회·별도 이벤트 구현은 이 표로 소실 범위를 확인했으며, 전체 본문을 유지했을 때 답변이 달라지는지는 미검증이다.

## 품질 점수와 토큰 결과의 해석

| 지표 | R23 | R25 | 해석 |
|---|---:|---:|---|
| 메인 토큰 | 1,122,724 | 1,029,007 | -8.35%, 단일 과거 실행과 비교 |
| Jev 토큰 | 187,057 | 113,907 | -39.11%, 메인과 별도 |
| 모델 요청 | 16 | 15 | 한 요청 적음 |
| 핵심 / 확장 | 6/6 · 2/5 | 6/6 · 3/5 | custom 연결 설명이 추가됨 |
| 총 품질 항목 | 8/11 | 9/11 | 전체 답변 정확도나 근거 보존율이 아님 |

기존 실패 2개는 그대로 구분한다. ServiceCandidate의 실제 호출자 행은 탐색하지 않았고, 수동 입력의 `extra` 소비 조건은 이미 읽었지만 답변에 연결하지 않았다. 이번에 발견한 factory의 응답 필드·호환 변환은 고정 11개 기준의 독립 평가 항목이 아니다. 그래서 9/11을 유지하면서도 이 중요한 근거 소실이 함께 존재할 수 있다.

수동 입력 함수는 이번 read가 L790–904로 끝나 기존 부분 본문 보호를 탔다. 이 함수 전체를 새 `missing_context` 규칙으로 보호하는 동작을 이번 실측에서 직접 재현한 것은 아니다. R25의 read 19회·grep 1회에서 새 보호가 작동한 사실을 모든 본문의 의미적 안전성으로 확대하지 않는다.

시간은 평가하지 않았다. 토큰 차이는 탐색 경로·등록 질문·요청 수가 달라지는 단일 실행의 관측이며 패치만의 인과 효과가 아니다. Jev HTTP 요청 전문은 저장되지 않았고 이번 분석에서 재생하지 않았다. source hash와 request hash가 있는 것만으로 전문을 확인했다고 주장하지 않는다.

## 커밋 범위와 실제 검증

이번 커밋은 기존 Jev search/read/grep 연결, 공통 중복 제거, 문맥 부족 보존, 관련 설정·문서·기존 테스트 조정을 저장한다. 분석 중 발견한 search 상태 생산 공백이나 파일 밖 호출자 보강을 새로 구현하지 않았다. 따라서 이 커밋을 모든 잘못된 생략의 해결 커밋으로 표시하지 않는다.

병행 작업의 pi/opencode·최종 클라이언트 출력 한도 변경은 커밋에서 분리하고 작업 트리에 보존했다. R24 작업 전 diff와 해당 작업 전용 patch로 커밋 대상을 구성했고, R24가 소유한 8개 파일의 결과 해시가 일치하는지 확인했다. 실제 검증은 이 분리된 소스에서 실행했다.

| 기존 검증 | 결과 |
|---|---:|
| `cargo check --manifest-path apps/codemap-search/Cargo.toml --locked --offline` | 통과 |
| `cargo test … --lib tools::search::jev::tests` | 26개 통과 |
| `cargo test … --test e2e_tests e2e::jev` | 13개 통과 |
| `cargo test … --test e2e_tests e2e::tools` | 35개 통과, 기존 1개 무시 |
| `cargo test … --test e2e_tests e2e::mcp` | 30개 통과 |

표의 `…`는 첫 행과 같은 `--manifest-path apps/codemap-search/Cargo.toml --locked --offline`이다. 작업 디렉터리는 R26의 `commit-source`이며 빌드 캐시는 기존 target 디렉터리를 사용했다. 합계 **104개 통과·기존 무시 1개**다. 새 테스트나 단언 변경은 추가하지 않았다. 커밋에 포함된 테스트의 동작 변경은 앞선 기능 작업의 기존 변경이며, 이번에는 저장소 커밋 훅이 요구한 Rust 서식 정리만 추가했다.

이 결과는 기존 동작·실패 복귀·프로토콜 계약에 대한 검증이다. 실제 관련성을 판단하는 Jev가 모든 중요한 근거를 보존한다는 검증은 아니며, 위 두 미해결 범위는 그대로 남는다.

커밋 훅의 첫 시도는 기존 서식 검사와 `clippy::unnecessary_map_or` 1개 때문에 실패했다. 커밋 대상 Rust 파일 14개에 기존 `cargo fmt` 서식을 적용하고, 설정 이력 초기화 조건의 `map_or(true, predicate)`를 같은 의미의 `is_none_or(predicate)`로 바꿨다. 프로젝트가 이미 사용하는 API이며 lint 설정·테스트 단언·Jev 판정 동작은 바꾸지 않았다. 위 104개 기능 검증은 이 서식 정리 전의 분리 소스에서 수행했고, 최종 커밋에서는 저장소의 fmt·Clippy 훅을 다시 실행한다. 병행 출력 한도 변경은 별도로 보존했다.
