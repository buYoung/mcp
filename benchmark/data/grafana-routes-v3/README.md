# Grafana 기능 경로 2문항 고정 계약

codemap-search 1.0.0 벤치마크용 `grafana-routes-v3`, revision `3`, 상태 `content_frozen`이다. 기존 `grafana-routes-v2`를 대체한다. 기준 소스는 `grafana/grafana`의 `c6fad8695a96577eb466d425e6ac4a759ca30f47`이다. 내용 고정과 독립 검토 승인·하네스 연결·실행 계약 고정은 구분한다.

| 문항 ID | 언어 | 고정 질문 | 핵심 | 확장 |
| --- | --- | --- | ---: | ---: |
| `alerting-alertmanager` | Go | 제공된 탐색 도구를 활용해서 알림 규칙 평가 구조 중 Alertmanager와 통신하는 구간 다 확인해줘. | 8 | 0 |
| `live-plugin-stream` | TypeScript → Go | 제공된 탐색 도구를 활용해서 Grafana Live 스트리밍 구조 중 브라우저가 plugin stream과 통신하는 구간 다 확인해줘. | 8 | 1 |

순서는 위와 같다. 난도 상태는 `structural_not_empirical`이며 난도·기억 저항·도구 우열을 실측하지 않았다.

## 개정 내용

- revision 2: 독립 검토 `review-opus.md`의 M1·m1–m9·r6와 진행자 결정을 반영해 필수 연결, 허용 범위, 근거 및 하네스 상태를 보완했다. 17개 사실·14개 wrong_claims·풀이 질문은 유지했다.
- revision 3: `review-opus-r2.md`의 n1(a)·n2(required_links 의도)·n3–n6·p1·p3·p4·p6을 반영했다. 17개 사실의 ID·level, 14개 wrong_claims와 풀이 질문은 유지했다.

3문항을 위 2문항으로 교체했다. 알림 평가의 콜백·조직별 구현·외부 송신 큐와 Live의 런타임 등록·언어 간 계약·공용 plugin dispatch·왕복 통신을 위치/연결 사실로 고정했다. Live 발행은 필수이며 구독 정리는 별도 확장 진단이다. **v2와 v3 결과를 같은 점수로 섞지 않는다.**

## 고정 파일

| 파일 | 역할 |
| --- | --- |
| [questions.json](questions.json) | 풀이용 ID·질문 원문만 포함. |
| [dataset.json](dataset.json) | 문항 순서·범위·언어 간 계약·핵심/확장 사실·대체 정답·오해 사례. |
| [evidence.json](evidence.json) | 인용한 39개 파일의 원본 식별자와 148개 발췌 원문·행·해시·고정 커밋 링크. |
| [grading.json](grading.json) | 사실별 정확성/근거 충족과 보류·확장 진단 계약. |
| [manifest.json](manifest.json) | 파일 해시·개수·검증 수치·승인 범위·미고정 실행 상태. |
| [SHA256SUMS](SHA256SUMS) | manifest를 포함한 나머지 6개 파일의 해시. 체크섬 파일 자체는 자기 참조하지 않는다. |

원본 파일 전체를 배포하는 패키지가 아니다. 모든 source_manifest 파일에서 적어도 하나를 발췌했고, 모든 발췌는 사실에서 참조한다. 생성·검증 스크립트는 패키지 밖의 `../build/`에 둔다.

## 문항 범위

- **알림:** Grafana 평가 요청 → 규칙 루틴 → 상태 선택/기록/콜백 → AlertsRouter → 조직별 AM을 연결한다. 로컬은 외부 alerting 라이브러리 인계까지, remote·외부는 큐에서 HTTP 실행까지 필수다. 쿼리/DAG·상태 저장소·HA 조정자·contact point 내부·재시도 수치는 필수 아님.
- **Live:** 브라우저 서비스 → WS 구독 → plugin/ds handler → 공용 client → 비동기 RunStream → 발행자 → TS 수신 체인, 그리고 HTTP/WS publish → PublishStream이 필수다. 구독 정리는 확장 진단이다. TestData 전용으로 제한하지 않는다. 외부 SDK/Centrifuge 내부·개별 plugin 알고리즘·다른 scope·pipeline 내부·렌더러는 필수 아님.

조건부 연결의 존재를 모든 배포의 실행 관측으로 확대하지 않는다. pipeline 가드는 있으나 고정 checkout의 비테스트 초기화/대입 검색은 0건이므로 필수 활성 경로로 고정하지 않았다.

## 채점 계약

`correct`와 `supported`는 각 사실별 독립 차원이며 각각 `true`/`false`/`"indeterminate"`다. 모든 core·extended fact의 `required_links`(2–4개)가 필수 범위이며 모두 정확하면 `correct=true`다. `required_links` 항목의 미언급·잘못된 연결은 `correct=false`다. `accepted_alternatives`의 대체 설명은 해당 `required_links` 항목을 충족한 것으로 보며, 그 안의 제한·주의 문구도 적용한다. statement의 나머지 절은 설명이며 미언급만으로 감점하지 않는다. 답변이 statement의 나머지 절을 틀리게 주장하면 `required_links`가 모두 정확해도 `correct=false`다. 진입점만 언급했다는 실패는 진입점이 아닌 `required_links` 연결이 빠졌을 때 적용한다. 사실에 없는 숫자·조건 값·필드 목록 누락은 감점하지 않는다.

`supported`는 `required_links`(또는 채택한 대체 설명)의 각 연결을 답변 인용이 입증하는지로 판정한다. 정답 excerpt와 행 범위가 일치할 필요는 없다. 경로와 위치(행 번호나 심볼)가 없는 인용은 불충분한 인용으로 `supported=false`다. A2의 registry·factory 생성·errgroup 기동, A3의 persister/historian 순서, A5의 forked AM 모드별 PutAlerts 위임, A7의 remote sender 재사용, L8의 tenant 접두어 차이는 추가 필수가 아니다. A2는 core를 유지한다. RL1의 생산자 쪽은 runJobFn/Eval 함수명 없이 '스케줄러가 규칙별 evalCh로 평가 요청을 보낸다'는 수준으로 보여도 된다. RL2(Run 루프의 evalCh 수신 → evaluate 호출)는 필요하다. remote 구현 선택은 A5, tenant 계약은 L3에서 채점한다.

조건부 경로를 무조건 경로라고 단정하면 그 절이 틀린 것으로 본다. `required_links`에 없는 조건(예: L2의 Live 활성·orgRole 조건)의 미언급은 감점하지 않는다.

| 상황 | 판정 |
| --- | --- |
| `required_links` 항목의 미언급·잘못된 연결 | 해당 `correct=false`. |
| 답변 인용 누락·잘못되거나 부족한 인용 | 해당 `supported=false`; 보류가 아님. |
| 평가자가 원문을 확인할 수 없고 대체 근거로도 확정 불가 | 해당 차원만 `indeterminate`와 사유. |
| 두 차원 모두 true | `fulfilled`. |
| 하나라도 false | `not_fulfilled`; 다른 차원의 보류는 보존. |
| false 없이 보류가 남음 | 사실 상태 `indeterminate`. |

모든 core가 fulfilled이면 `core_complete=true`, 하나라도 not_fulfilled이면 false, 확정 실패 없이 보류만 있으면 indeterminate다. 확장 LX1의 누락·오류·보류만으로 core를 실패시키지 않는다. core/extended 및 각 차원의 개수·사유를 분리한다. 보류를 0점으로 채우거나 분모에서 빼 확정 평균을 만들지 않는다.

TS↔Go 사실은 양쪽 소스와 URL·채널·payload 대응이 필요하다. 정답 근거가 답변의 인용을 대신하지 않는다. `wrong_claims`와 비채점 참고 설명은 재현 의무가 없으며 문자열 일치나 일괄 감점에 사용하지 않는다. 실제로 제기한 모순만 영향받은 사실에 귀속한다.

풀이 모델에는 `questions.json`과 공통 실행 안내만 제공한다. 나머지 정답 자료를 Grafana 작업 공간에 복사하거나 검색 힌트로 제공하지 않는다.

## 검증과 보존

고정 checkout에서 39개 원본의 Git blob SHA-1·원본 바이트 SHA-256·크기·행 수와 148개 발췌를 대조한다. 발췌는 정확히 `"\n".join(path.read_text().splitlines()[start-1:end])`이며 결합 뒤 LF를 추가하지 않는다. 마지막 선택 행이 빈 행이면 결합 결과 자체가 LF로 끝날 수 있다.

```sh
python3 -B ../build/verify_dataset.py
shasum -a 256 -c SHA256SUMS
```

첫 명령은 이 scratch 배치에서 원본·참조·개수·풀이 입력·패키지 무결성을 검증한다. 둘째는 패키지 바이트만 확인한다. 실제 Grafana 시나리오·모델 풀이/채점·난도 실측은 수행하지 않았다. 의미를 바꾸면 별도 revision과 변경 이유를 남긴다.

## 실행기 연결 상태

운영 상태는 `manifest.json`에만 기록하며 `grading.json`에는 채점 규칙만 둔다. `status_as_of_build`는 revision 3 생성 시점의 스냅샷으로, 후속 결합·승인에 맞춰 다시 쓰지 않는다. 실행 계획은 **2문항 × 7비교군 × 3회 = 42슬롯**이다.

결합 규칙은 `benchmark/route_contract.py DATASET_MANIFEST_SHA256 == sha256(manifest.json)`이다. 내용 변경 시 패키지 사본, `route_contract.py`의 `DATASET_MANIFEST_SHA256`, `targets.lock.json`의 dataset manifest 해시와 해당 `SHA256SUMS`, 이어 `route_contract.py`의 `TARGET_LOCK_SHA256`을 함께 갱신해야 한다. 이 작업에서는 하네스나 저장소를 수정하지 않았다.

`manifest.execution_contract.status=not_frozen`을 유지한다. 42슬롯 계획이나 adapter 결합은 모델 실행 승인이 아니다. revision 1·2 독립 검토 완료, revision 3은 r2 검토 수정안 반영(diff 확인 대상)이다.
