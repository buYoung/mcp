# Grafana 기능 경로 2문항 고정 계약

codemap-search 1.0.0 벤치마크용 `grafana-routes-v4`, revision `3`, 상태 `content_frozen`이다. `grafana-routes-v3`를 대체하며 소스는 `grafana/grafana`의 `c0a598f5a49abf2014f67e806075d8ad267c9efb`이다. 내용 고정·독립 검토·하네스 결합·실행 승인은 구분한다.

| 문항 ID | 언어 | 고정 질문 | 핵심 | 확장 |
| --- | --- | --- | ---: | ---: |
| `git-oauth-authorization` | TypeScript → Go | Git 저장소 연결 인증 구조 중 사용자 지정 OAuth 서버와 통신하는 구간 다 확인해줘. | 6 | 2 |
| `plugin-http-route-bridge` | Go | 앱 플러그인 manifest의 사용자 정의 API 라우팅 구조 중 HTTP로 배포된 백엔드와 통신하는 구간 다 확인해줘. | 6 | 0 |

순서는 위와 같다. 난도는 `structural_not_empirical`이며 실제 탐색 난도·도구 우열은 미측정이다. 기억 측정의 원문·결과·분모는 manifest에 보존한다. 질문 1의 새 문구 결과 출처는 `v4/memory/oauth-reworded-run/summary.json`(부모 세션 측정 결과 파일)이다. 그 측정은 r1 rubric 기준이며 r2/r3 rubric의 모델 재채점 결과는 아니다. `review-opus-r2.md` §5는 답변 직접 대조로 통과 유지와 재측정 불필요를 판단했다. O7은 extended이므로 기억 제외 기준의 core를 바꾸지 않는다. 질문 2는 “manifest” 추가 전 문구로 측정했다.

## 개정 내용

2026-10-26 승인에 따라 최신 소스와 학습 이후 코드 중심의 두 문항으로 교체했다. 모든 비교군에서 앞부분 도구 지시문을 제거했다. revision 1은 이 두 문항의 최초 패키지다. revision 2는 독립 검토 M1·M2·m1–m7·r1과 사용자 결정을 반영한다. 질문 1에서 “브라우저가”를 삭제하여 인가 이동과 서버측 교환을 모두 포함하고, O4를 O4a(provider 선택)/O4b(서버 Exchange)로 분할했다. O5의 교환 결과 저장·authorized 응답은 핵심 유지, O6의 탭 간 통지는 extended로 분리했다. revision 3은 사용자 결정에 따라 서버측 토큰 갱신 O7을 extended로 추가하고 재검토 N2·n3–n5를 반영했다. 핵심은 6+6개 그대로이며 확장은 O6·O7 2개다. r1·r2는 각각 `../snapshots/r1/`·`../snapshots/r2/`에 보존한다. **v2·v3·v4 결과를 같은 점수로 섞지 않는다.** direct tools와 codemode only 결과도 별도로 보고한다.

## 고정 파일

| 파일 | 역할 |
| --- | --- |
| [questions.json](questions.json) | 풀이용 ID·질문 원문만 포함. |
| [dataset.json](dataset.json) | 범위·언어 계약·사실·required_links·대체 설명·오해 사례. |
| [evidence.json](evidence.json) | 23개 원본 식별자, 82개 발췌·행·해시·고정 URL. |
| [grading.json](grading.json) | 정확성/근거 충족 독립 차원과 보류 규칙. 운영 상태 없음. |
| [manifest.json](manifest.json) | 파일 해시·개수·승인·검증·학습 이력/기억 측정 출처·생성 시점 상태. |
| [SHA256SUMS](SHA256SUMS) | manifest 포함 6파일의 해시. 자기 참조 없음. |

모든 발췌는 사실에서 사용한다. 스크립트와 검증 기록은 패키지 밖 `../build/`에 둔다. 소스 전체를 재배포하는 패키지는 아니다.

## 문항 범위

- **Git OAuth:** 브라우저 authURL 이동 → callback/state → namespaced authorize → 사용자 지정 provider 선택 → 서버측 tokenURL 교환 → 교환 토큰 저장·authorized 응답까지 핵심이다. 토큰은 브라우저로 반환하지 않는다. 저장 성공이 인가 완료 응답의 전제라는 연결만 요구하며 저장소/암호화 내부는 제외한다. 원래 탭 BroadcastChannel 완료 통지(O6)와 GenerateConnectionToken → TokenSource → 사용자 지정 tokenURL의 서버측 토큰 갱신(O7)은 extended다. 갱신은 질문 범위 안이지만 core_complete의 필수는 아니다. 로그인 generic_oauth·연결 생성 UI·Git sync·외부 OAuth 내부는 필수가 아니다.
- **HTTP bridge:** app manifest custom route → v3 HTTP client 선택/주입 → route adapter/접근 검사 context → 자격 증명 재구성 → HTTP 요청/redirect 거절 → v3 응답 stream. **plugin.json routes(pluginproxy/dataproxy)와 appsettings proxy subresource(apppluginsHandleProxyRequests)는 범위 밖**이다. 그 경로로 필수 연결을 대체하면 해당 사실 correct=false이며, 별개 경로라고 언급한 것만으로 일괄 감점하지 않는다. discovery/poll/기동·webhook·표준 CRUD·외부 SDK/transport 내부는 요구하지 않는다.

조건부 경로의 존재를 모든 배포의 실행 관측으로 확대하지 않는다. gitOAuth는 connection_types에 포함될 때만 Build되며 기본값 github에서는 비활성이다. callback 라우트의 provisioningEnabled·checkRequiredFeatures도 추가 필수가 아니다. 외부 경계는 OAuth의 Config.Exchange 및 확장 O7의 Config.TokenSource(...).Token() 호출과 app-sdk의 HandlerFunc(ClientV3)/CallRoute 반환 계약까지다. HTTP shim은 deprecated transition path이며 routes.go:395-397의 token exchange 주석은 gRPC 경로 기준이다. 이 경계·활성 조건은 packet의 context_notes에도 복제했다. chunk 크기·전체 필드/헤더 목록·상태 코드 번호·서버 기동은 추가 필수가 아니다.

## 채점 계약

각 사실의 `correct`와 `supported`는 독립적으로 true/false/indeterminate다. `required_links`(2–3개)가 필수 범위다. 모두 정확하면 correct=true이고, 미언급·잘못된 연결은 해당 correct=false다. `accepted_alternatives`의 `RLn 대체:`는 required_links의 1부터 시작하는 n번째 항목만 대체하며 다른 항목을 면제하지 않는다. 제한·주의도 적용한다. 같은 연결의 등록 또는 조회 증명을 모두 허용하며 둘 다 요구하지 않는다. statement의 나머지 절은 설명: 누락만으로 감점하지 않지만 틀리게 주장하면 correct=false다.

supported는 required_links(또는 채택한 대체 설명)의 각 연결을 **답변 인용**이 입증하는지 판정한다. 경로와 위치(행 번호 또는 심볼)가 없거나 인용이 부족하면 supported=false다. 정답 구간과 행 범위가 같을 필요는 없다. TS↔Go 계약은 양쪽 근거와 URL/payload 대응이 필요하다. 정답 evidence는 답변 인용을 대신하지 않는다.

조건부 경로를 무조건 경로라고 단정하면 그 절이 틀린 것이다. required_links에 없는 조건의 미언급은 감점하지 않는다. wrong_claims/context_notes의 재현 의무는 없으며 실제 모순만 영향받는 사실에 귀속한다.

| 상황 | 판정 |
| --- | --- |
| 필수 연결 누락/오류 | 해당 correct=false |
| 인용 누락/부족/잘못된 위치 | 해당 supported=false; 보류 아님 |
| 평가자가 원문 접근 불능이며 대체 근거로도 확인 불가 | 해당 차원만 indeterminate와 사유 |
| 두 차원 모두 true | fulfilled |
| 하나라도 false | not_fulfilled; 다른 차원의 보류는 보존 |
| false 없이 보류가 남음 | 사실 상태 indeterminate |

모든 core가 fulfilled이면 core_complete=true, 하나라도 not_fulfilled이면 false, 확정 실패 없이 보류만 있으면 indeterminate다. core/extended 및 각 차원의 수·사유를 분리한다. 보류를 0점이나 분모 제외로 처리한 확정 평균은 만들지 않는다. 확장은 O6·O7 2개이며 이 확장만의 누락·오류·보류로 core_complete를 실패시키지 않는다.

풀이 모델에는 `questions.json`과 공통 실행 안내만 제공한다. 나머지 정답 자료를 Grafana workspace에 복사하거나 검색 힌트로 노출하지 않는다.

## 검증과 보존

원본 Git blob SHA-1·바이트 SHA-256·크기·행 수와 발췌를 고정 checkout에서 검증한다. text는 정확히 `"\n".join(path.read_text().splitlines()[start-1:end])`다. 결합 뒤 LF를 더하지 않으며 마지막 선택 행이 비어 있으면 결합 결과 자체가 LF로 끝날 수 있다. 각 본문 발췌의 소속 함수 헤더를 같은 사실 근거에 포함한다.

```sh
python3 -B ../build/build_dataset.py
python3 -B ../build/verify_dataset.py
python3 -B ../build/test_dataset.py
python3 -B ../build/measure_packets.py
shasum -a 256 -c SHA256SUMS
```

모델 요청·실제 외부 통신·Go/TS 실행은 하지 않았다. 학습 기준일은 2026-07-01 UTC다. manifest는 최종 인용 파일/지정 정의별 비율을 별도로 보존한다. 날짜와 기억 측정 통과가 학습 데이터 부재를 보장하지 않으며 PR 사전 공개일도 미확인이다. 의미 변경은 별도 revision으로 남긴다.

## 실행기 연결 상태

운영 상태는 manifest의 `status_as_of_build` 스냅샷으로만 보존하며 후속 승인·결합에 맞춰 다시 쓰지 않는다. 결합 규칙은 `benchmark/route_contract.py DATASET_MANIFEST_SHA256 == sha256(manifest.json)`이다. 이 작업에서는 하네스와 소스를 수정하지 않았다. `sync_into_repo.py`도 실행하지 않는다.

계획은 모델당 **2문항 × 7비교군 × 3회 × 2모드 = 84슬롯**(모드별 42)이다. 계획은 실행 승인이 아니다. `execution_contract.status=not_frozen`이며 독립 내용 리뷰와 하네스 source commit·question ID·manifest/target-lock 결합은 후속 게이트다.
