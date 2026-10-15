# 관련어 시소러스: 남은 결함 처리 결정 기록 (4단계)

[R1~R3 처리](r1-decisions.md) 뒤에 남은 항목을 판단해 새 데이터셋에 반영한 기록이다. 대상은 R1 단어의 이견 조치, R2 보류, R1~R3 처리 때문에 생긴 조회 변화, [미반영 결함 목록](unresolved-defects.md)의 A·B·C·D·E와 처리 여부를 확인하지 못한 보류다. 판단은 이 작업 세션이 원 비교 파일의 검토 근거, 데이터셋, 고정 체크아웃 코드, 이전 후속 분석(후속389)을 직접 확인해 내렸다. 한쪽 검토자의 제안만으로 채택하지 않았다.

| 항목 | 내용 |
| --- | --- |
| 작업일 | 2026-10-16 |
| 시작 데이터셋 | `rts3-b5fb0c137c71a08f` (R1~R3 처리 결과, 이전 기본값) |
| 결과 데이터셋 | `rts3-9e01eaf1eadf960a` (새 기본값) |
| 결과 SHA-256 | `ab8f1aea8a60625fb4f4795e02ffdfa9b3947ce61cc7120c84a03dc61646d192` |
| 작업 파일 | `~/.cache/related-terms-usage-1/word-eval/remaining-001/` (결정 `r1d_decisions.py`·`other_decisions.py`·`decisions/stage4.json`, 계획 `plans/`, 확인 결과 `checks/`, 단계별 데이터셋 `datasets/`) |
| 사용자 결정 | 독립 검토는 작업 마지막에 읽기 전용 하위 에이전트로 한다. 커밋은 related-terms 관련 파일만 한다. `docs/handoffs/`에는 정정 문구를 추가한다 |
| 독립 검토 | 읽기 전용 하위 에이전트 1회. 발견 9건과 처리는 [독립 검토](#독립-검토) 참고 |

## 결과 요약

| 범위 | 대상 | 채택 | 기각 | 보류 |
| --- | ---: | ---: | ---: | ---: |
| R1 단어의 이견 조치 | 325 | 226 | 72 | 27 |
| R2 보류 재판단 | 31 | 10 | 17 | 4 |
| 조회 변화 보완 | 7 | 7 | 0 | 0 |
| A (오연결 미수정) | 3 | 3 | 0 | 0 |
| B (뜻 미확정 연결) | 35 | 14 | 19 | 2 |
| C (추가하지 않은 후보) | 12 | 6 | 5 | 1 |
| 처리 여부 미확인 보류 | 9 | 9 | 0 | 0 |
| R1 이견의 의존 정리 | 2 | 2 | 0 | 0 |

- R1 이견의 의존 정리 2건은 R1 이견 표의 `invalidations`·`margins` 행과 같은 조치다. 위 표에서는 실제 적용 단위로 따로 셌다.
- B의 "기각"은 현행 연결을 유지한다는 결정이다. B17 `sections`·B18 `updaters`는 이전 사용자 결정(기존 상태만 보존)을 그대로 따랐다.

## 판단 기준

- **굴절형:** 복수형·3인칭·과거분사·-ing 형은 기본형 뜻을 그대로 갖는다. 기본형의 뜻을 상속하는 제안은 채택했다. 기본형 연결 자체가 맞는지는 이 판단과 별개로 두었다.
- **고정 이름:** 기본형 연결이 제품명·명령·함수·타입처럼 굴절하지 않는 고정 이름이면 상속하지 않았다. 예: `Blink` 엔진, Windows `PAUSE` 명령, `%matplotlib inline`, SQL `LAG`, unit 타입, Rx `sample` 연산자.
- **품사가 바뀌는 파생:** -able·-er 형과 명사→동사 파생은 뜻이 달라진다. 기본형의 객체·동작 뜻을 그대로 옮긴 연결은 제거를 채택했다. 예: `bufferable`, `creatable`, `configurer`, `exporters`.
- **일관성:** 같은 뜻의 기본형이 같은 개념에 남아 있는데 파생형만 빼자는 제안은 기각했다. 기본형 소속까지 의심스러우면 보류했다.
- **새 개념:** 양쪽 검토자가 모두 필요하다고 본 개념은 채택했다(이름이 다르면 하나로 합쳤다). 한쪽 제안은 기존 개념으로 표현할 수 없고 코드로 확인될 때만 채택했다. 복수형만 담는 새 개념, 한 저장소의 내부 기능을 나누는 개념은 보류하거나 기각했다.
- **표제어:** 12개 저장소 수집 단어가 아닌 표제어는 새로 만들지 않았다. `fusedmultiplyadd`, `membername`, `megabyte`(단수) 추가는 이 기준으로 기각했다. 예외로, input 재연결(`input_bindings`)은 이미 input 단어로 데이터에 있던 단어를 API 동작에 따라 표제어로 등록한다. 이것은 새 own 구성원이나 새 단어를 더하는 것이 아니며, 이전 통합에서도 같은 방식으로 등록했다(시작 전 기본값 `rts3-c4a3b0fe9eb74bc2`에 corrected 표제어 105개).
- **own과 input:** input을 own으로 승격하지 않았다. 같은 개념에 이미 input이 있는데 own을 더한 4건(`hot keys`, `inherited`, `slaves`, `sources`의 6007)은 직접 코드 근거가 따로 있는 경우다.

## 적용 단계

| 단계 | 데이터셋 | 내용 | 저장 확인 | 소비 확인 |
| --- | --- | --- | --- | --- |
| 4a | `rts3-7b926399d21b0f85` | 새 주제 1(x52 Bookmarks and favorites), 새 개념 8(7341~7348), persona 15 | 일치, FK 0 | 9단어에 새 개념 반영 |
| 4b | `rts3-641a078ef551bfc0` | 이동 9, own 제거 32, input 제거 77, own 추가 61, 상속 51항목(개념 72) | 일치, FK 0 | 140단어 확인. 표시 2건은 아래 참고 |
| 4c | `rts3-9e01eaf1eadf960a` | 의존 정리 own 제거 5, 관계 재귀속 24, 관계 제거 1, input 해제 12, input 재연결 9 | 일치, FK 0, 새 표제어 표기 = 이전 조회 표시 | 10단어에서 잘못된 개념 사라짐, 옮긴 관계가 맞는 뜻 아래 표시, 표시 표기 변화 0 |

- 4b를 처음 적용한 출력 `rts3-cf39a85c388e8b35`는 쓰지 않았다. `megabyte`(단수) own 추가가 수집 단어가 아닌 표제어를 만든다는 것을 저장 확인에서 발견했다. 그래서 그 조치를 기각으로 바꾸고 4b를 다시 만들었다.
- 4b 소비 확인에서 표시된 2건은 의도한 변화다. `slaves`는 기본형 `slave`의 잘못된 30을 지워 조회에서 빠졌다. `unshared`는 own이 생겨 기본형 `unshare`(Linux namespace) 확장이 멈추며 XML namespace 개념 3060이 빠졌는데, 이 뜻은 `unshared`(Redis 단독 보유 메모리)에 성립하지 않는다.
- 4c의 input 재연결은 정해진 API 동작대로 재연결한 단어 9개를 표제어(`terms`, kinds `["corrected"]`)로 등록한다. 처음 만든 4c 출력 `rts3-d10ac73c3aee85cb`에서는 생성기가 원래 표기 대신 `word_key`를 넘겼다. 그 결과 `mx-auto`가 `mxauto`로, `margin-left`가 `marginleft`로 표시되는 등 7개 단어의 조회 표시가 바뀌었다. 독립 검토가 이를 찾았고, 이전 조회 표시와 같은 표기(`mx-auto`, `margin-left`, `ping timeout` 등)를 넘기도록 고쳐 4c를 다시 만들었다. 다시 만든 결과는 표시 표기 변화가 0이다. `rts3-d10ac73c3aee85cb`는 게시 폴더에 수정 없이 남아 있지만 기본값이 아니며 쓰지 않는다.

## 확인한 것

- **최종본 정합성:** FK 오류 0, 고아 sense 0, persona 분야·단어 불일치 0, input·관계의 누락 persona 0. 같은 단어·개념 중복 1,804건은 이전 기본값과 같다.
- **시작 DB와 최종 DB 직접 비교:** 모든 테이블의 증감이 4a·4b·4c 적용량의 합과 같다.
- **빈 주제:** 개념이 없는 주제가 시작 데이터셋(`rts3-b5fb0c137c71a08f`) 14개에서 17개로 늘었다. 4단계에서 새로 빈 주제는 t141.2·t766.2·t896.4다. 각각 5637·5342·5400을 맞는 주제로 옮긴 결과다. 조사 대상 데이터셋(`rts3-c4a3b0fe9eb74bc2`)의 10개와 비교하면 t213.4·t344.2·t706.4·t89.5 4개가 더 있는데, 이 넷은 R1~R3에서 비었다. 개념이 없는 주제는 조회에 쓰이지 않는다.
- **D3:** `rt-sense/2` 기준선 `rts2-309e9bbaa19f7eb5`에서 `add_entries`를 실제로 실행했다. 출력 스키마가 `rt-sense/2`로 유지되고 FK 0, 조회에 새 개념이 반영되며 기준선 해시가 바뀌지 않았다. 임시 출력은 지웠다.
- **E1:** 281~305 단계 폴더의 `corrections.json`이 모두 출력 DB `sense_meta`의 `correction_plan_sha256`과 일치한다. 실행 기록에 계획 해시가 없어도 적용된 계획을 추적할 수 있다.
- **E2:** 281·285·291·292·293·295·296·304·305 단계를 지금의 저장 확인으로 다시 확인했다. 확인 항목은 정확한 삭제·추가 집합, sense와 분야 일치, 고아 sense 정리, 나머지 테이블 불변이며 모두 문제 0, FK 0이다.
- **E3:** E235의 기준선·항목 파일 해시가 기록과 같고 저장 확인이 통과했다. 같은 입력으로 현재 코드를 다시 실행한 결과가 모든 테이블과 `dataset_id`까지 같다.

## 독립 검토

작업 마지막에 읽기 전용 하위 에이전트가 결정 파일·계획·확인 결과·스크립트·체크아웃 코드를 대조했다. 검토자 환경에는 셸·SQL 실행 도구가 없었다. 그래서 DB로 확인해야 할 항목은 이 작업 세션이 직접 질의해 확인했다. 검토 의견은 수락이 아니며, 항목마다 다시 판단했다.

| 발견 | 심각도 | 처리 |
| --- | --- | --- |
| 4c input 재연결이 붙여 쓴 표기의 표제어를 만들어 조회 표시가 바뀜 | 주요 | 사실 확인 후 수정. 생성기를 고쳐 4c를 다시 만들었다(`rts3-9e01eaf1eadf960a`). 확인 도구에 새 표제어 표기 검사를 추가했고, 이 검사는 폐기한 `d10a`의 7건을 잡아낸다 |
| A1 execution 관계의 수신자 11311이 맞는 뜻이라는 근거가 기록되지 않음, 후속389와 차이 | 주요 | 수신자 유지. execution의 네 persona를 비교한 근거와 한계(주제가 Server route handlers라 web-backend 뜻 아래 표시됨), 389와의 차이를 결정 이유에 적었다 |
| B23 `multiple` 결정 누락 | 경미 | `multiple` 소속 유지 결정과, 두 단어 persona의 관계 혼합 전수 확인 보류 1건을 추가 |
| 빈 주제 수의 비교 기준이 섞임 | 경미 | DB로 확인(b5fb 14, 최종 17)하고 문구를 고쳤다 |
| 표제어 기준과 input 재연결의 충돌 | 경미 | 판단 기준에 예외와 이전 통합의 같은 방식(105개)을 적었다 |
| `remap` 새 개념 채택과 `transforms` 새 개념 기각의 차이 | 참고 | `remap` 이유에 더 좁은 연산이라는 차이를 적었다 |
| 결정 파일의 `hold-dependency` 라벨 | 참고 | `adopt`로 바꾸고 이유에 4c 정밀 편집으로 정리했다고 적었다. 4a·4b 계획은 그대로임을 확인했다 |
| 기본형 `unshare`의 3060 연결 | 참고 | DB 확인 결과 `isolate`·`namespace` persona를 거친다. 개념 3060이 XML namespace와 격리 용어를 섞은 문제라서 고치지 않고 결함 목록에 새 관찰로 올렸다 |
| B1이 후속389 제거 권고를 다루지 않음 | 참고 | 389 권고와 유지 이유를 결정 이유에 적었다 |

## R1 이견 조치별 결정

R1 296단어에 합의 조치와 함께 남아 있던 한쪽 검토자 조치 325건이다. "제안"은 그 조치를 낸 검토자다.

| 단어 | 제안 | 조치 | 결정 | 이유 | 데이터셋 |
| --- | --- | --- | --- | --- | --- |
| `averaged` | crosscheck | own 제거 2890 | 기각 | 2890 구성원에 average·mean·median·moving average가 있어 averaged의 평균 뜻이 같은 구성으로 성립. 개념 이름(model.train)의 혼합은 이 단어 처분 범위 밖 | 미적용 |
| `backends` | crosscheck | input 제거 63 | 채택 | 63은 의존성 포함·요구 표현(adding·needed·optionally·versions) 주제. backends는 구현 대상 명사로 그 표현이 아니며 implementations의 동의어 연결만으로 들어옴 | `rts3-641a078ef551bfc0` |
| `blinking` | analyst | blink에서 3254 상속 | 기각 | 기본형 blink의 3254 persona(3111)는 Blink 렌더링 엔진(dom·layout·htmlparser·stylerecalculation 관계). 제품명 뜻은 blinking에 성립하지 않음 | 미적용 |
| `bubbled` | analyst | own 제거 3435 | 채택 | tooltip/popover 말풍선 명사 bubble은 bubbled로 굴절해 popup 객체가 되지 않음. 반대쪽도 성립 근거 부족(uncertain)으로 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `bufferable` | analyst | 개념 5876 이동 t160.3→t201.1 | 채택 | Avro binary serialization이 Text encoding conversion 주제에 있음. t201.1 Binary serialization(data-formats)이 직접 범위. 반대쪽은 input 유지만 다룸 | `rts3-641a078ef551bfc0` |
| `bufferable` | crosscheck | 새 개념 "bufferability"(t241.1) 단어 bufferable | 기각 | 같은 Buffered stream I/O 주제의 500(backpressure)·501(buffered writer)을 유지하므로 저장 가능 항목 뜻이 기존 개념으로 표현됨 | 미적용 |
| `bufferable` | crosscheck | own 추가 새 개념 | 기각 | 새 개념 기각에 따름 | 미적용 |
| `bufferable` | crosscheck | own 제거 160 | 채택 | append(배열·버퍼 길이 확장)는 버퍼 객체·API 개념. bufferable(파이프라인 버퍼에 담을 수 있는 항목의 성질)은 그 구성원으로 쓰이지 않음. -able 파생이라 buffer의 객체 뜻이 이어지지 않음. 의존 0 | `rts3-641a078ef551bfc0` |
| `bufferable` | crosscheck | own 제거 1738 | 채택 | vao(정점 버퍼 그리기 제출)는 버퍼 객체·API 개념. bufferable(파이프라인 버퍼에 담을 수 있는 항목의 성질)은 그 구성원으로 쓰이지 않음. -able 파생이라 buffer의 객체 뜻이 이어지지 않음. 의존 0 | `rts3-641a078ef551bfc0` |
| `bufferable` | crosscheck | own 제거 1873 | 채택 | ArrayBuffer(바이트 배열 저장 객체)는 버퍼 객체·API 개념. bufferable(파이프라인 버퍼에 담을 수 있는 항목의 성질)은 그 구성원으로 쓰이지 않음. -able 파생이라 buffer의 객체 뜻이 이어지지 않음. 의존 0 | `rts3-641a078ef551bfc0` |
| `bufferable` | crosscheck | own 제거 3744 | 채택 | recv(소켓 송수신 API)는 버퍼 객체·API 개념. bufferable(파이프라인 버퍼에 담을 수 있는 항목의 성질)은 그 구성원으로 쓰이지 않음. -able 파생이라 buffer의 객체 뜻이 이어지지 않음. 의존 0 | `rts3-641a078ef551bfc0` |
| `bufferable` | crosscheck | own 제거 4293 | 채택 | java.nio(NIO 버퍼·채널 API)는 버퍼 객체·API 개념. bufferable(파이프라인 버퍼에 담을 수 있는 항목의 성질)은 그 구성원으로 쓰이지 않음. -able 파생이라 buffer의 객체 뜻이 이어지지 않음. 의존 0 | `rts3-641a078ef551bfc0` |
| `bufferable` | crosscheck | own 제거 5886 | 채택 | Byte buffers(메모리 복사)는 버퍼 객체·API 개념. bufferable(파이프라인 버퍼에 담을 수 있는 항목의 성질)은 그 구성원으로 쓰이지 않음. -able 파생이라 buffer의 객체 뜻이 이어지지 않음. 의존 0 | `rts3-641a078ef551bfc0` |
| `bufferable` | crosscheck | own 제거 500 | 기각 | backpressure·buffered·stream의 버퍼링 흐름 개념에 버퍼링 가능한 항목 뜻이 성립(Vector Bufferable은 backpressure가 있는 토폴로지 버퍼 항목) | 미적용 |
| `bufferable` | crosscheck | own 제거 501 | 기각 | buffered reader/writer·flush 개념에 버퍼링 가능한 항목 뜻이 성립(Vector Bufferable은 backpressure가 있는 토폴로지 버퍼 항목) | 미적용 |
| `canonicalizes` | crosscheck | canonicalize에서 3036 상속 | 채택 | 3인칭 굴절이라 기본형 canonicalize의 정규화 뜻이 그대로 성립. 기본형 연결 자체의 적합성은 이 상속 판단과 별개(출처 input 유지) | `rts3-641a078ef551bfc0` |
| `channels` | crosscheck | channel에서 3386 상속 | 채택 | 복수형이라 기본형 channel의 stream conduit 뜻이 그대로 성립(own 유지). 기본형 소속 자체의 적합성은 별개 | `rts3-641a078ef551bfc0` |
| `clearable` | crosscheck | own 추가 3375 | 채택 | sentry core/select/select.tsx:418-431,657 clearable 속성이 isClearable로 전달되는 선택 컨트롤 성질. 3375에 selectable·deselect가 있음 | `rts3-641a078ef551bfc0` |
| `cohorts` | analyst | own 추가 2709 | 채택 | sentry workflow_engine/processors/schedule.py:86-130 ProjectChooser가 프로젝트를 hash로 num_cohorts개 집단에 나눠 실행마다 처리. 2709의 group·partition 처리 집단과 같은 구성 | `rts3-641a078ef551bfc0` |
| `collaborators` | analyst | own 추가 1204 | 채택 | keycloak SsfTransmitterContext.java:13, woocommerce RenewalEngine.php:112·RenewalDispatcher.php:100에서 생성자로 받는 의존 객체를 collaborators로 부름. 1204 deps·coupling과 같은 구성 | `rts3-641a078ef551bfc0` |
| `cols` | analyst | 개념 6740 이동 t415→t944.2 | 채택 | t415 범위는 escape sequence·제어 코드·색상이라 cell grid(cells, cols)가 맞지 않음. t944.2 Screen dimensions에 같은 terminal column(6703)이 있음. 분야(os-shell) 동일 | `rts3-641a078ef551bfc0` |
| `columns` | analyst | 개념 5877 이동 t135.1→t153 | 기각 | 이동 필요에는 동의하나 AWK text processing 전체는 $0 레코드 참조(t153)보다 넓음. t326.3 채택 | 미적용 |
| `columns` | crosscheck | 개념 5877 이동 t135.1→t326.3 | 채택 | AWK text processing이 Object property operations 주제에 있음. t326.3 범위가 sed·awk·grep 명령줄 텍스트 처리를 직접 명시. 양쪽 모두 이동 필요에 동의 | `rts3-641a078ef551bfc0` |
| `components` | crosscheck | own 추가 3313 | 채택 | vector vdev/src/utils/paths.rs:52, sinks/file/mod.rs:677의 path.components() 경로 구성요소 순회. 3313 path 구성(parent directory·relative·infix)과 같은 대상 | `rts3-641a078ef551bfc0` |
| `configurer` | crosscheck | own 제거 2269 | 채택 | 2269는 설치·초기 설정 안내 활동 개념. 행위자 명사 configurer(설정 콜백·권한 사용자)는 그 안내의 구성원이 아님. 반대쪽도 소속 미확정으로만 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `configurer` | crosscheck | own 제거 619 | 채택 | 619의 configure는 ./configure 빌드 스크립트·컴파일러 플래그. 행위자 명사 configurer에 그 뜻이 이어지지 않음. 의존 0 | `rts3-641a078ef551bfc0` |
| `consoles` | analyst | 새 개념 "account management console"(t896.1) 단어 consoles | 보류 | 기본형 console이 같은 5869·72에 소속돼 있어 계정 관리 콘솔 개념 신설은 console과 함께 정해야 함. 한쪽 제안 | 미적용 |
| `consoles` | analyst | own 제거 5869 | 기각 | 기본형 console이 같은 5869 own. 복수형만 빼면 불일치 | 미적용 |
| `consoles` | analyst | own 제거 72 | 기각 | 기본형 console과 dashboard가 같은 72 own. 관리 콘솔 복수 뜻이 성립 | 미적용 |
| `consts` | analyst | constant에서 2603 상속 | 채택 (같은 행의 다른 조치로 충족) | 양쪽 모두 consts를 2603 Literal constants에 두자는 결론(방식만 상속/own 추가로 다름). constants의 널리 쓰는 축약. own 추가로 함께 충족 | `rts3-641a078ef551bfc0` |
| `consts` | crosscheck | own 추가 2603 | 채택 | vector의 std::env::consts 플랫폼 상수와 SinkBatchSettings associated const 값. 2603 constant·literal·config value와 같은 구성 | `rts3-641a078ef551bfc0` |
| `consts` | crosscheck | own 제거 3766 | 기각 | 기본형 const(타입 한정자)가 같은 3766 own. const 한정자의 복수 용법도 성립 | 미적용 |
| `contractless` | analyst | 새 개념 "contractless object serialization"(t460.1) 단어 contractless | 기각 | 2739 serializer에 resolvers·type adapter가 이미 있어 ContractlessStandardResolver 선택을 기존 개념으로 표현 가능 | 미적용 |
| `contractless` | crosscheck | own 추가 2739 | 채택 | bitwarden-server Notifications/Startup.cs:49-54 MessagePack ContractlessStandardResolver 지정. 2739의 resolvers·custom serializer와 같은 직렬화 구성 | `rts3-641a078ef551bfc0` |
| `contractless` | analyst | input 제거 4144 | 채택 | 직렬화 멤버 계약 생략은 SRP·ISP 책임 분리 원칙이 아님. interface segregation/isp의 관계로만 들어옴. 반대쪽도 미확정으로만 봄 | `rts3-641a078ef551bfc0` |
| `contributes` | crosscheck | input 제거 1607 | 채택 | contributes는 VS Code 확장 manifest 키. Chrome/Firefox Manifest V3에는 없는 키라 extend의 identifier 연결이 맞지 않음 | `rts3-641a078ef551bfc0` |
| `coordinates` | analyst | coords에서 3811 상속 | 채택 | coords는 coordinates의 축약. CSS 요소 위치(getBoundingClientRect·top·inset) 뜻이 coordinates에 그대로 성립(input 유지). sentry에서 coordinates[key]를 coords로 씀 | `rts3-641a078ef551bfc0` |
| `coordinates` | analyst | input 제거 4163 | 채택 | 4163은 HTTP 응답 처리(status·error page·response body). manages의 일반 조율 동사 동의어로 들어온 연결로 좌표·조율 어느 뜻도 성립하지 않음. 반대쪽도 근거 부족으로 봄 | `rts3-641a078ef551bfc0` |
| `creat` | crosscheck | 개념 5400 이동 t896.4→t922.1 | 채택 | O_CREAT·creat()는 open 모드 플래그. 현재 주제 t896.4의 첫 분야가 auth-identity라 인증 분야로 분류됨. t922.1 File opening modes(files-io)에 readonly(4018)가 있음. 반대쪽은 own 유지만 다룸 | `rts3-641a078ef551bfc0` |
| `creatable` | analyst | own 제거 5400 | 채택 | 생성 가능성(-able)은 파일 생성 모드 값이 아님. 실제 CREATABLE은 WP REST HTTP 메서드 상수. 의존 0 | `rts3-641a078ef551bfc0` |
| `custom fields` | crosscheck | input 제거 1792 | 채택 | 1792는 FAQ·문제 해결 주제. YouTrack 제품 persona에서 기능 이름으로 들어온 연결로 사용자 정의 필드 뜻과 무관. 반대쪽도 보류로만 봄 | `rts3-641a078ef551bfc0` |
| `custom fields` | crosscheck | input 제거 1838 | 채택 | 1838은 문서 위키 게시 주제. Redmine 제품 persona에서 기능 이름으로 들어온 연결로 사용자 정의 필드 뜻과 무관 | `rts3-641a078ef551bfc0` |
| `data sources` | analyst | 새 개념 "endpoint routing data source"(t89.1) 단어 datasources | 기각 | EndpointDataSource는 인터페이스로 데이터를 공급하는 provider. 6848 data provider own 추가로 기존 개념에 표현됨 | 미적용 |
| `data sources` | crosscheck | own 추가 6631 | 채택 | gutenberg lib/class-wp-theme-json-resolver-gutenberg.php:10 core/blocks/theme/user 설정 data sources를 우선순위로 합침. 6631 configuration source(설정 로딩·병합)와 같은 대상 | `rts3-641a078ef551bfc0` |
| `data sources` | crosscheck | own 추가 6848 | 채택 | bitwarden-server HttpExtensions/StandaloneEndpointDataSource.cs:35,49 EndpointDataSource provider 집합, vector Table provider. 6848 data provider와 같은 공급 객체 역할 | `rts3-641a078ef551bfc0` |
| `datasets` | analyst | dataset에서 1953 상속 | 채택 | 복수형이라 기본형 dataset의 학습 입력 집합 뜻이 그대로 성립(input 유지). 기본형 연결 자체의 적합성은 별개 | `rts3-641a078ef551bfc0` |
| `datasets` | analyst | 새 개념 "telemetry query dataset"(t694.1) 단어 datasets | 보류 | 기본형 dataset도 같은 질의 대상 데이터 집합 뜻이라 복수형만 담는 개념 신설은 불일치. 기본형과 함께 정해야 함. 한쪽 제안 | 미적용 |
| `definitions` | crosscheck | definition에서 5859 상속 | 채택 | 복수형이라 기본형 definition의 XML namespace 정의 뜻이 그대로 성립(own 유지) | `rts3-641a078ef551bfc0` |
| `deletes` | analyst | delete에서 3618 상속 | 채택 | 3인칭 굴절이라 기본형 delete의 뜻이 그대로 성립(input 유지, 원천 knex·returning). 기본형 연결 자체의 적합성은 별개 | `rts3-641a078ef551bfc0` |
| `deletes` | analyst | delete에서 6012 상속 | 채택 | 3인칭 굴절이라 기본형 delete의 뜻이 그대로 성립(input 유지, 원천 tbl). 기본형 연결 자체의 적합성은 별개 | `rts3-641a078ef551bfc0` |
| `deletes` | crosscheck | delete에서 3838 상속 | 채택 | 3인칭 굴절이라 기본형 delete의 뜻이 그대로 성립(input 유지, 원천 binarysearchtree). 기본형 연결 자체의 적합성은 별개 | `rts3-641a078ef551bfc0` |
| `descendent` | analyst | own 추가 1340 | 채택 | 양쪽 모두 유효로 봄. woocommerce 블록이 상위 블록의 하위 노드인지 검사(getBlockParentsByBlockName). 1340 child·component tree·dom tree와 같은 트리 구성 | `rts3-641a078ef551bfc0` |
| `detectors` | analyst | detector에서 1070 상속 | 채택 | 복수형이라 기본형 detector의 data pipeline sensor 뜻이 그대로 성립(input 유지, 원천 sensor) | `rts3-641a078ef551bfc0` |
| `folders` | analyst | own 추가 6522 (대상 folder) | 채택 | 양쪽 모두 menu folder 뜻을 인정(반대쪽은 기본형 보완을 이 단어 범위 밖으로만 봄). terminal TerminalPage.cpp:1099-1102 단수 FolderEntry가 메뉴 항목을 묶음 | `rts3-641a078ef551bfc0` |
| `folders` | analyst | own 추가 6523 (대상 folder) | 채택 | 양쪽 모두 vault folder 뜻을 인정. bitwarden-server ImportCiphersCommand.cs:237-241 cipher를 사용자별 folder에 연결하는 단수 folder 객체 | `rts3-641a078ef551bfc0` |
| `fragments` | crosscheck | fragment에서 5154 상속 | 채택 | 복수형이라 기본형 fragment의 GPU 셰이더 조각 뜻이 그대로 성립(own 유지). 래스터 fragment들의 복수 용법 | `rts3-641a078ef551bfc0` |
| `fragments` | crosscheck | input 제거 447 | 채택 | 447은 소프트웨어 BOM 구성요소 목록 주제. parts의 일반 조각 동의어로만 들어온 연결로 설정 문서·문자열 조각 어느 뜻도 SBOM 구성요소가 아님 | `rts3-641a078ef551bfc0` |
| `framer` | analyst | input 제거 1889 | 채택 | gif의 이미지 프레임 뜻은 바이트 경계를 붙이는 framer(Vector Framer)나 다른 확인된 framer 뜻에 성립하지 않음. 반대쪽도 별도 뜻을 입증해야 한다고만 봄 | `rts3-641a078ef551bfc0` |
| `framer` | analyst | input 제거 192 | 채택 | sprite·texture atlas의 애니메이션 프레임 뜻은 바이트 경계를 붙이는 framer(Vector Framer)나 다른 확인된 framer 뜻에 성립하지 않음. 반대쪽도 별도 뜻을 입증해야 한다고만 봄 | `rts3-641a078ef551bfc0` |
| `framer` | analyst | input 제거 3254 | 채택 | draw·paint·rendering의 화면 표시 뜻은 바이트 경계를 붙이는 framer(Vector Framer)나 다른 확인된 framer 뜻에 성립하지 않음. 반대쪽도 별도 뜻을 입증해야 한다고만 봄 | `rts3-641a078ef551bfc0` |
| `framer` | analyst | input 제거 461 | 채택 | bounds의 이미지 경계 뜻은 바이트 경계를 붙이는 framer(Vector Framer)나 다른 확인된 framer 뜻에 성립하지 않음. 반대쪽도 별도 뜻을 입증해야 한다고만 봄 | `rts3-641a078ef551bfc0` |
| `framer` | analyst | input 제거 490 | 채택 | BrowserWindow·window의 창 뜻은 바이트 경계를 붙이는 framer(Vector Framer)나 다른 확인된 framer 뜻에 성립하지 않음. 반대쪽도 별도 뜻을 입증해야 한다고만 봄 | `rts3-641a078ef551bfc0` |
| `framer` | analyst | own 제거 1785 | 채택 | iframe·video frame·keyframe 표시 객체 개념에 바이트 framing 처리기가 속하지 않음. 반대쪽도 성립 근거를 제시하지 않음. 의존 0. 1781·5099 소속은 유지 | `rts3-641a078ef551bfc0` |
| `gateways` | analyst | own 제거 2296 | 기각 | 기본형 gateway가 같은 2296 own(facade·api와 함께). 복수형만 빼면 불일치. 기본형 소속 자체의 적합성은 별개 | 미적용 |
| `grantee` | analyst | 새 개념 "access grant recipient"(t15.1) 단어 grantee | 보류 | 102에 grantor·grantees가 함께 있어 권한 수령 주체 개념 신설은 같은 역할어들과 함께 정해야 함. 한쪽 제안 | 미적용 |
| `grantee` | analyst | own 제거 102 | 기각 | 102에 grantor·grantees가 같은 권한 부여 관계의 역할어로 있음. grantee만 빼면 불일치 | 미적용 |
| `grouping` | crosscheck | group에서 1761 상속 | 기각 | 기본형 group의 1761 뜻은 process group 명사. 묶기 동작·결과인 grouping에 그 명사 뜻이 성립한다는 용례가 없음(tokio 근거는 process_group 설정) | 미적용 |
| `grouping` | crosscheck | group에서 5050 상속 | 기각 | 기본형 group의 5050 뜻은 RBAC 사용자 그룹 명사. grouping(키별 묶기·이슈 그룹화)에 그 명사 뜻이 성립하지 않음 | 미적용 |
| `grouping` | crosscheck | own 추가 6704 | 채택 | 6704 issue grouping fingerprint 개념 이름 그대로. sentry grouping/component.py:30-35 contributes 값으로 이슈 그룹화 hash 계산. 6704에 contributes가 있음 | `rts3-641a078ef551bfc0` |
| `guarded` | crosscheck | own 추가 2794 | 채택 | redis jemalloc pac.h:102 guarded extents, pages.c:373 mprotect(PROT_NONE) guard page. 2794 memory protection과 같은 접근 제한 구성 | `rts3-641a078ef551bfc0` |
| `guards` | crosscheck | guard에서 5514 상속 | 채택 | 복수형이라 기본형 guard의 코드 경로 보호 뜻이 그대로 성립(own 유지). 기본형 개념의 분야 적합성은 별개 | `rts3-641a078ef551bfc0` |
| `highlighted` | crosscheck | highlight에서 1833 상속 | 채택 | t616.1 범위가 highlighted selection states를 명시. 강조된 선택 상태 뜻이 highlighted에 성립 | `rts3-641a078ef551bfc0` |
| `highlighted` | crosscheck | own 추가 새 개념 | 채택 | 양쪽 모두 t739 Syntax highlighting 새 개념을 제안. sentry utils/marked/marked.tsx:119-129 highlighted 구문 강조 문자열 | `rts3-7b926399d21b0f85` |
| `highlighter` | analyst | 새 개념 "Syntax highlighting"(t739) 단어 highlight, highlighted, highlighter, highlights | 채택 | 양쪽 모두 같은 이름·주제(t739, 범위에 syntax highlighting 명시)로 제안. keycloak CliJsonOutputHighlighter, sentry marked.tsx:122 highlight callback. 기본형 highlight도 코드에서 같은 뜻으로 확인 | `rts3-7b926399d21b0f85` |
| `highlighter` | crosscheck | 새 개념 "Syntax highlighting"(t739) 단어 highlighter | 채택 | 위 새 개념과 같은 개념으로 함께 적용 | `rts3-7b926399d21b0f85` |
| `highlighter` | analyst | own 제거 1833 | 채택 | 1833은 선택·강조 상태 용어. 강조 처리기(행위자) highlighter는 상태가 아님. 반대쪽도 근거 부족으로만 봄. 처리기 뜻은 새 Syntax highlighting 개념으로 보존. 의존 0 | `rts3-641a078ef551bfc0` |
| `highlights` | analyst | input 제거 4635 | 채택 | 4635는 annotation·attribute·reflection 메타데이터. marks의 근접 관계로 들어온 연결로 검색 강조 표식·주요 정보 요약은 메타데이터 태그가 아님(태그를 표시하는 소비는 소속 근거가 아님) | `rts3-641a078ef551bfc0` |
| `highlights` | crosscheck | highlight에서 1833 상속 | 채택 | 복수형이라 기본형 highlight의 강조 선택 상태 뜻이 그대로 성립(t616.1 범위 명시) | `rts3-641a078ef551bfc0` |
| `highlights` | crosscheck | own 추가 새 개념 | 채택 | 새 Syntax highlighting 개념(양쪽 제안)의 구성원 | `rts3-7b926399d21b0f85` |
| `hot keys` | analyst | own 추가 2458 | 채택 | terminal WindowEmperor.cpp:1369 RegisterHotKey, ActionMap.h:128 _GlobalHotkeysCache 전역 단축키 등록. input 승격이 아니라 같은 대상이라는 직접 코드 근거로 own 추가(input 3건 유지) | `rts3-641a078ef551bfc0` |
| `identifiers` | analyst | identifier에서 3036 상속 | 채택 | 복수형이라 기본형 identifier의 Unicode 식별자 정규화 뜻이 그대로 성립(own 유지). 기본형 소속 자체의 적합성은 별개 | `rts3-641a078ef551bfc0` |
| `identifiers` | crosscheck | own 추가 4251 | 기각 | identifiers는 어디에나 쓰이는 일반 명사. COSE 알고리즘 숫자 식별자가 서명 설정에 쓰인다는 공존만으로 digital signature 구성원이 되지 않음 | 미적용 |
| `identities` | analyst | own 추가 4263 | 채택 | keycloak UserBrokerLinkFederatedStorage의 federated identities는 IdP 브로커링으로 연결된 사용자 identity. 4263 범위(identity provider·identity management integration)와 구성원(idp·Keycloak·accounts)에 성립 | `rts3-641a078ef551bfc0` |
| `identities` | crosscheck | own 추가 6043 | 채택 | keycloak federated identity link 관리(getFederatedIdentitiesStream)는 외부 계정 연결 레코드. 6043 account linking(unlinked)과 같은 대상 | `rts3-641a078ef551bfc0` |
| `idxs` | crosscheck | own 제거 3409 | 보류 | 기본형 idx도 같은 3409 own(nth·ordinal·day와 함께). 소속 자체가 의심스럽지만 기본형과 함께 정해야 함(한쪽 제거, 한쪽 불확실) | 미적용 |
| `images` | analyst | own 추가 1132 (대상 image) | 채택 | 양쪽 모두 1132(executable·object file·pdb·symbolicate)의 debug image 뜻을 인정. 복수형 images가 이미 구성원이고 기본형 image를 같은 뜻으로 보완 | `rts3-641a078ef551bfc0` |
| `images` | analyst | own 추가 907 (대상 image) | 채택 | container image·docker image·image tag를 다루는 개념에 기본형 image가 빠져 있음. vector docker list_images·pull_image. 복수형 images는 이미 구성원 | `rts3-641a078ef551bfc0` |
| `imported` | analyst | own 추가 1430 (대상 import) | 채택 | 1430 ETL에 importing·imported가 있고 기본형 import만 빠짐. 데이터 반입(sentry relocation import, woocommerce migrator) 뜻이 기본형에 성립 | `rts3-641a078ef551bfc0` |
| `importing` | analyst | import에서 3877 상속 | 채택 | 동사 import의 -ing 형이라 모듈 불러오기 뜻이 그대로 성립(own 유지). 같은 개념에 imported도 있음 | `rts3-641a078ef551bfc0` |
| `importing` | analyst | own 추가 1430 (대상 import) | 채택 | imported 행의 같은 조치와 동일. 함께 적용 | `rts3-641a078ef551bfc0` |
| `incl` | crosscheck | 새 개념 "tax-inclusive price display"(t261.2) 단어 incl, inclusive | 채택 | woocommerce src/Enums/TaxDisplayMode.php:8-20 INCLUSIVE='incl'로 세금 포함 가격 표시 옵션 값. 공개 제품 설정값이며 기존 범위 경계 포함(5532)·allowlist와 다른 대상. t261.2 가격·세금 계산 주제 | `rts3-7b926399d21b0f85` |
| `infos` | crosscheck | info에서 3042 상속 | 채택 | 복수형이라 기본형 info의 정보성 안내(note·tip) 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `inherited` | analyst | own 추가 4635 | 채택 | keycloak testsuite AuthServerContainerExclude.java:32 등 Java @Inherited 메타 애너테이션. input 승격이 아니라 같은 대상이라는 직접 코드 근거로 own 추가(input 유지). 반대쪽도 이 뜻을 유효로 봄 | `rts3-641a078ef551bfc0` |
| `inherited` | crosscheck | input 제거 1308 | 채택 | Delphi의 inherited(조상 메서드 호출 키워드)는 Windows 실행 파일·DLL 통합 주제가 아님. 언어 상속 뜻은 다른 개념에 남음 | `rts3-641a078ef551bfc0` |
| `inlined` | analyst | inline에서 1670 상속 | 기각 | 기본형 inline의 1670 뜻은 %matplotlib inline 백엔드 이름. 고정 이름이라 inlined로 굴절하지 않음 | 미적용 |
| `inlined` | analyst | 새 개념 "CSS style inlining"(t796.1) 단어 inline, inlined | 채택 | woocommerce lib/packages/Pelago/Emogrifier/CssInliner.php:180-208 CSS 선언을 HTML style 속성으로 복사(이메일 CSS 인라이닝). 기존 Flex·style·CSS-in-JS와 다른 연산. t796.1 스타일시트 처리 주제 | `rts3-7b926399d21b0f85` |
| `inlined` | analyst | 새 개념 "function inlining"(t808) 단어 inline, inlined | 채택 | 양쪽 모두 컴파일러 함수 인라이닝 뜻을 코드에서 확인(redis t_list.c, gin). 기존 6624는 강제 인라이닝(always inline)만 담음. t808 Compiler optimization | `rts3-7b926399d21b0f85` |
| `inlined` | crosscheck | 새 개념 "inline web resource content"(t586) 단어 inline, inlined | 보류 | t586은 resx 등 리소스 파일 조직 주제라 문서 안 자원 본문 내장 뜻과 맞지 않음. 뜻 자체는 가능하나 주제와 기존 6124 inline script·6295와의 관계 미정. 한쪽 제안 | 미적용 |
| `insights` | analyst | 새 개념 "application performance analytics"(t694.1) 단어 insight, insights | 보류 | 반대쪽은 같은 성능 분석 뜻을 SLI input으로 이미 표현된다고 봄. 기존 2281 observability와의 관계 미정. 한쪽 제안 | 미적용 |
| `insights` | analyst | 새 개념 "vault security risk analysis"(t15.9) 단어 insight, insights | 채택 | 양쪽 모두 Bitwarden Risk Insights 새 개념 필요에 동의(주제만 다름). bitwarden-server Dirt/Controllers/ReportsController.cs:105-129. 기존 주제 t15.9 Security operations로 충분해 새 주제는 만들지 않음 | `rts3-7b926399d21b0f85` |
| `insights` | crosscheck | insights에서 1520 상속 (대상 insight) | 채택 | 단수·복수 같은 뜻. 반대쪽도 insights의 오류 추적 분석 input을 유효로 봄(input 유지) | `rts3-641a078ef551bfc0` |
| `insights` | crosscheck | insights에서 6321 상속 (대상 insight) | 채택 | 단수·복수 같은 뜻. 반대쪽도 issue analytics 연결을 유효로 봄(own 유지) | `rts3-641a078ef551bfc0` |
| `insights` | crosscheck | 새 개념 "application package size diagnostics"(t564) 단어 insight, insights | 기각 | 기존 6137 build size insight(insight·insights 구성원)가 앱 크기 진단 뜻을 이미 표현 | 미적용 |
| `insights` | crosscheck | 새 개념 "credential security analytics"(new:credential-security-analytics) 단어 insight, insights | 기각 | 같은 뜻의 vault security risk analysis를 기존 주제 t15.9로 채택 | 미적용 |
| `insights` | crosscheck | 새 주제 "Credential security analytics" | 기각 | 기존 주제 t15.9로 충분해 새 주제 불필요 | 미적용 |
| `invalidations` | analyst | own 제거 1779 (대상 invalidation) | 채택 | 양쪽 모두 기본형 invalidation의 1779(재사용 helper 추출) 소속을 오분류로 봄. persona 16853의 native 관계 8개를 같은 단어의 캐시 무효화 persona로 옮긴 뒤 4c 정밀 편집으로 제거 | `rts3-9e01eaf1eadf960a` |
| `keyed` | analyst | own 제거 4092 | 채택 | 4092는 API key·client secret 발급·회전 자격 증명. -ed 형 keyed(키로 색인·선택됨)에 자격 증명 명사 뜻이 이어지지 않음. 반대쪽도 근거 부족으로만 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `keyed` | crosscheck | input 제거 374 | 채택 | attention의 key는 Q/K/V key 벡터 명사. keyed에 그 뜻이 성립하지 않음. 반대쪽도 근거 부족으로만 봄 | `rts3-641a078ef551bfc0` |
| `keyed` | crosscheck | input 제거 708 | 채택 | 708은 클릭·탭·롱프레스. ev/evt의 key 공존 관계로 들어온 연결로 keyed에 탭 상호작용 뜻이 없음 | `rts3-641a078ef551bfc0` |
| `kinesis streams` | analyst | 개념 1392 이동 t277.2→new:aws-service-clients | 기각 | 이동 필요에는 동의하나 기존 t282.3 Service SDK integration이 패키지된 API client 범위를 이미 담음 | 미적용 |
| `kinesis streams` | analyst | 새 주제 "AWS service clients" | 기각 | 기존 주제 t282.3으로 충분해 새 주제 불필요 | 미적용 |
| `kinesis streams` | crosscheck | 개념 1392 이동 t277.2→t282.3 | 채택 | 1392 AWS SDK(DynamoDBClient·Kinesis 포함)가 DynamoDB 항목 접근 전용 주제에 있음. 양쪽 모두 이동 필요에 동의. t282.3 범위가 외부 서비스용 패키지 API client. DynamoDB 자체 개념은 t277.2에 남음 | `rts3-641a078ef551bfc0` |
| `kinesis streams` | crosscheck | 새 개념 "Amazon Kinesis Data Streams"(t488) 단어 kinesisstreams | 보류 | 같은 1392에 Kinesis·Amazon Kinesis가 남아 있어 Kinesis 제품 개념 신설은 이 단어들과 함께 정해야 함. 한쪽 제안 | 미적용 |
| `kinesis streams` | crosscheck | own 제거 1392 | 기각 | 같은 1392에 Kinesis·Amazon Kinesis가 서비스 연동 대상으로 남음. kinesis streams만 빼면 불일치 | 미적용 |
| `latencies` | analyst | input 제거 4205 | 채택 | 4205는 high·medium·low 수준 표현 어휘. latencies는 측정값이지 수준 표현이 아니며 high latency 수식 공존만으로 들어옴. 지연 측정 뜻은 다른 개념에 남음 | `rts3-641a078ef551bfc0` |
| `latencies` | crosscheck | input 제거 332 | 채택 | 332는 앱 생성·일시정지·재개 수명주기. experience/feel의 UX 지연 공존으로 들어온 연결로 수명주기 뜻이 아님. 반대쪽도 근거 부족으로만 봄 | `rts3-641a078ef551bfc0` |
| `loaders` | analyst | loader에서 4413 상속 | 채택 | 복수형이라 기본형 loader의 native 로더·링커 이웃 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `margins` | analyst | own 제거 2849 (대상 margin) | 채택 | 양쪽 모두 기본형 margin(가장자리·여백)을 중간 위치 어휘 2849에 둔 것이 오분류라고 봄. persona 19880의 native 관계 5개·inbound input 4개를 CSS 여백 persona로 옮긴 뒤 4c 정밀 편집으로 제거 | `rts3-9e01eaf1eadf960a` |
| `markers` | crosscheck | input 제거 3383 | 채택 | 3383은 Arduino·GPIO 전기 핀. pins의 지도 pin/marker 근접 관계가 전기 핀 persona로 옮겨진 분류. 반대쪽도 GPIO marker 뜻을 확정하지 못함 | `rts3-641a078ef551bfc0` |
| `measured` | analyst | measure에서 1381 상속 | 기각 | 기본형 measure의 이 연결은 BI·OLAP의 집계 지표 명사(measure). 동사 과거분사 measured(측정된)에 그 명사 뜻이 성립하지 않음 | 미적용 |
| `measured` | analyst | measure에서 4901 상속 | 기각 | 기본형 measure의 이 연결은 BI·OLAP의 집계 지표 명사(measure). 동사 과거분사 measured(측정된)에 그 명사 뜻이 성립하지 않음 | 미적용 |
| `measured` | analyst | measure에서 5463 상속 | 기각 | 기본형 measure의 이 연결은 BI·OLAP의 집계 지표 명사(measure). 동사 과거분사 measured(측정된)에 그 명사 뜻이 성립하지 않음 | 미적용 |
| `megabytes` | crosscheck | own 추가 6879 (대상 megabyte) | 기각 | megabyte(단수)는 표제어가 없고 12개 저장소 수집 단어도 아님(수집된 것은 megabytes). megabytes는 이미 6879 own. 새 표제어를 만들지 않음 | 미적용 |
| `menus` | analyst | 새 주제 "Food-service menu planning" | 채택(기존 개념으로 대체) | 양쪽 모두 음식 메뉴 뜻을 제안. 같은 범위의 기존 주제 x49 Food-service menus와 개념 7315 meal menu가 이미 있어 새 주제 대신 menus가 기본형 menu의 7315를 상속(own) | `rts3-641a078ef551bfc0` |
| `menus` | crosscheck | 새 주제 "Food service menus" | 채택(기존 개념으로 대체) | 위와 같은 제안. 기존 x49/7315로 함께 충족 | `rts3-641a078ef551bfc0` |
| `menus` | analyst | own 추가 7281 | 채택 | gutenberg wp_navigation 메뉴 엔터티(useEntityRecords NAVIGATION_POST_TYPE). 7281 WordPress navigation menus의 navigation menu와 같은 대상. 반대쪽도 같은 뜻을 기술 | `rts3-641a078ef551bfc0` |
| `messaging` | analyst | 새 개념 "team messaging"(t56.12) 단어 messaging | 보류 | 한쪽 제안이며 대상 주제 t56.12의 분야가 databases-storage로 잘못돼 있어 분야 검토가 먼저 필요(제안자도 적용 전 분야 검토 필요로 표시) | 미적용 |
| `messaging` | crosscheck | message에서 2811 상속 | 채택 | 2811은 Azure Service Bus·message bus·request reply 메시징 구성. messaging은 이 메시징 체계 활동 자체(bitwarden Azure.Messaging.ServiceBus)로 성립 | `rts3-641a078ef551bfc0` |
| `messaging` | crosscheck | message에서 4354 상속 | 채택 | 4354는 SignalR·Socket.IO·WebSocket 실시간 push. 서버 push 메시징 뜻이 messaging에 성립 | `rts3-641a078ef551bfc0` |
| `muted` | analyst | own 추가 333 | 기각 | 낮은 강조 색상 역할 뜻은 4819 design tokens(accent·secondary·semantic color와 함께)에 이미 있음. 소비 위치(border 색상)만으로 333 구성원을 더하지 않음 | 미적용 |
| `namespaced` | crosscheck | namespace에서 566 상속 | 기각 | 566은 식별자 명명 스타일·대소문자 변환. 이름공간 접두가 붙은 상태(namespaced)는 casing 스타일이 아님 | 미적용 |
| `nonspace` | analyst | input 제거 5016 | 채택 | 5016은 URL percent 인코딩. space의 URL 인코딩 이웃 관계가 부정형 nonspace로 복사된 연결. 반대쪽도 성립 근거를 제시하지 못함 | `rts3-641a078ef551bfc0` |
| `normalized` | analyst | 새 개념 "structured data normalization"(t135.4) 단어 normalized | 채택 | 양쪽 모두 같은 주제 t135.4·같은 근거(gutenberg dataviews field-types/index.tsx:65-124, woocommerce MetaDataUtil.php:39-63)로 레코드 표현 정규화 새 개념을 제안(이름만 다름) | `rts3-7b926399d21b0f85` |
| `normalized` | crosscheck | 새 개념 "record representation normalization"(t135.4) 단어 normalized (같은 개념 "structured data normalization"으로 합침) | 채택 | 위 새 개념과 같은 개념으로 함께 적용 | `rts3-7b926399d21b0f85` |
| `normalized` | crosscheck | normalize에서 759 상속 | 채택 | 동사 normalize의 과거분사라 String.normalize 결과 문자열의 코드 포인트 이웃 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `not allowed` | analyst | input 제거 38 | 기각 | disabled persona의 관계 표기가 not-allowed(비활성 컨트롤에 쓰는 CSS cursor 값). 비활성 상태의 실제 이웃이라 활성 상태 주제의 input이 성립 | 미적용 |
| `not allowed` | analyst | input 제거 4975 | 채택 | 4975는 미구현·미지원 연산(NotImplementedError·stub). 금지(405·정책 거절)는 구현 부재가 아니며 cannot/not 일반 부정 공존으로만 들어옴 | `rts3-641a078ef551bfc0` |
| `notifying` | analyst | 새 개념 "security event subject inclusion"(x4) 단어 notifying | 기각 | keycloak의 ssf.notify 설정 판정이라는 프로젝트 내부 상태. 통지 뜻을 내부 기능 단위로 나누는 개념이라 기준(내부 기능 이름 제외)에 맞지 않음 | 미적용 |
| `notifying` | analyst | input 제거 4163 | 채택 | 4163은 HTTP 응답 처리. sending의 근접 관계로 들어온 연결로 통지 동작은 HTTP 응답 처리가 아님. 반대쪽도 근거 부족으로만 봄 | `rts3-641a078ef551bfc0` |
| `notifying` | analyst | own 제거 4553 | 기각 | t285.3 범위가 publishing notifications를 명시하고 notified도 같은 4553 구성원. 통지 동작 뜻이 성립 | 미적용 |
| `orphaned` | crosscheck | 새 개념 "orphaned billing object"(t111.1) 단어 orphaned | 기각 | orphaned의 일반 뜻(정상 연결·소유 관계 없이 남은 객체)을 한 저장소의 billing 문맥에 적용한 것. 3197(orphan·orphaned)과 별도로 문맥별 개념을 나누지 않음 | 미적용 |
| `packages` | analyst | 새 개념 "Windows application package"(t271) 단어 packages | 보류 | 한쪽 제안. 같은 주제 t271의 573(appx)과 기본형 package의 뜻을 함께 정해야 하며 복수형만 담는 개념은 불일치 | 미적용 |
| `packets` | crosscheck | input 제거 2925 | 채택 | 2925는 commit·log 메시지 문구 이름. messages/msgs의 통신 근접 관계로 들어온 연결로 바이트 덩어리 packets는 메시지 문구가 아님. 통신 뜻은 500·1781 등에 남음 | `rts3-641a078ef551bfc0` |
| `paged` | crosscheck | input 제거 1838 | 채택 | Confluence·wiki의 page는 문서 페이지 명사. 페이지 구획 상태 paged에 위키 문서 뜻이 성립하지 않음. 반대쪽도 판단을 유보만 함 | `rts3-641a078ef551bfc0` |
| `paged` | crosscheck | input 제거 5208 | 채택 | UWP·WinUI의 Page는 UI 컨테이너 객체 이름. paged에 그 객체 뜻이 성립하지 않음 | `rts3-641a078ef551bfc0` |
| `patching` | analyst | patch에서 4376 상속 | 기각 | 기본형 patch의 4376 연결은 staging의 hunk 단위 patch 명사(git add -p). 패치 적용·monkey patch 동작 patching에 Git unstaging 뜻이 성립한다는 근거 없음 | 미적용 |
| `paused` | crosscheck | pause에서 739 상속 | 기각 | 기본형 pause의 739 연결은 Windows batch의 PAUSE 명령 이름. 고정 명령 이름이라 paused로 굴절하지 않음 | 미적용 |
| `payment gateways` | analyst | own 제거 586 | 기각 | 같은 586에 payment gateway(단수)·payment methods가 있음. 복수형만 빼면 불일치 | 미적용 |
| `popularity` | analyst | own 제거 4381 | 채택 | 4381은 typical·normal·standard 전형성 서술어. 인기도 명사는 전형성을 서술하지 않음. 반대쪽도 명사형 적용을 미확정으로만 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `popularity` | analyst | own 제거 4647 | 기각 | 4647에 popular·best·most·highest가 있고 popularity는 상위 항목 선택의 순위 기준(woocommerce orderby=popularity → total_sales 내림차순)으로 성립 | 미적용 |
| `positioner` | analyst | input 제거 2212 | 채택 | watermark·marks의 스트림 진행 위치는 위치 값·주소 뜻. 요소를 배치하는 행위자 positioner(Floating UI Popover.Positioner)에 성립하지 않음. 반대쪽도 이 결과들이 positioning agent가 아니라고 기술 | `rts3-641a078ef551bfc0` |
| `positioner` | analyst | input 제거 2219 | 채택 | iloc·nthe의 인덱스 접근는 위치 값·주소 뜻. 요소를 배치하는 행위자 positioner(Floating UI Popover.Positioner)에 성립하지 않음. 반대쪽도 이 결과들이 positioning agent가 아니라고 기술 | `rts3-641a078ef551bfc0` |
| `positioner` | analyst | input 제거 2222 | 채택 | findIndex·indexOf의 검색 결과 위치는 위치 값·주소 뜻. 요소를 배치하는 행위자 positioner(Floating UI Popover.Positioner)에 성립하지 않음. 반대쪽도 이 결과들이 positioning agent가 아니라고 기술 | `rts3-641a078ef551bfc0` |
| `positioner` | analyst | input 제거 2499 | 채택 | LSP 요청의 소스 좌표는 위치 값·주소 뜻. 요소를 배치하는 행위자 positioner(Floating UI Popover.Positioner)에 성립하지 않음. 반대쪽도 이 결과들이 positioning agent가 아니라고 기술 | `rts3-641a078ef551bfc0` |
| `positioner` | analyst | input 제거 3338 | 채택 | eof·token stream의 스캔 위치는 위치 값·주소 뜻. 요소를 배치하는 행위자 positioner(Floating UI Popover.Positioner)에 성립하지 않음. 반대쪽도 이 결과들이 positioning agent가 아니라고 기술 | `rts3-641a078ef551bfc0` |
| `positioner` | analyst | input 제거 3769 | 채택 | ReferenceProvider의 심볼 위치는 위치 값·주소 뜻. 요소를 배치하는 행위자 positioner(Floating UI Popover.Positioner)에 성립하지 않음. 반대쪽도 이 결과들이 positioning agent가 아니라고 기술 | `rts3-641a078ef551bfc0` |
| `positioner` | analyst | input 제거 3796 | 채택 | registerDefinitionProvider의 정의 위치는 위치 값·주소 뜻. 요소를 배치하는 행위자 positioner(Floating UI Popover.Positioner)에 성립하지 않음. 반대쪽도 이 결과들이 positioning agent가 아니라고 기술 | `rts3-641a078ef551bfc0` |
| `positioner` | analyst | input 제거 4517 | 채택 | startsWith·lastIndexOf의 문자열 오프셋는 위치 값·주소 뜻. 요소를 배치하는 행위자 positioner(Floating UI Popover.Positioner)에 성립하지 않음. 반대쪽도 이 결과들이 positioning agent가 아니라고 기술 | `rts3-641a078ef551bfc0` |
| `positioner` | analyst | input 제거 4721 | 채택 | DefinitionProvider·LocationLink의 탐색 위치는 위치 값·주소 뜻. 요소를 배치하는 행위자 positioner(Floating UI Popover.Positioner)에 성립하지 않음. 반대쪽도 이 결과들이 positioning agent가 아니라고 기술 | `rts3-641a078ef551bfc0` |
| `positioner` | analyst | input 제거 509 | 채택 | ByteBuffer의 버퍼 위치 값는 위치 값·주소 뜻. 요소를 배치하는 행위자 positioner(Floating UI Popover.Positioner)에 성립하지 않음. 반대쪽도 이 결과들이 positioning agent가 아니라고 기술 | `rts3-641a078ef551bfc0` |
| `positioner` | analyst | own 제거 3443 | 채택 | 3443은 소스 줄·열·커서 위치. 배치 행위자는 소스 텍스트 주소가 아님. 반대쪽도 미확정으로만 봄. 의존 0. 배치 뜻은 3811에 남음 | `rts3-641a078ef551bfc0` |
| `positioner` | analyst | own 제거 3899 | 채택 | 3899는 GPS·지리 위치. 배치 행위자는 위치 측정 장치가 아님. 의존 0 | `rts3-641a078ef551bfc0` |
| `prelogin` | analyst | own 추가 3345 | 기각 | prelogin은 로그인 전 단계 이름. KDF 매개변수·salt 조회는 bitwarden 한 저장소의 단계 내용이며 prelogin이 KDF 계산·비밀번호 해시 구성원은 아님 | 미적용 |
| `prelogin` | crosscheck | 새 개념 "SQL Server connection prelogin"(t25.4) 단어 prelogin | 보류 | MS-TDS PRELOGIN 단계는 공개 프로토콜이지만 근거가 keycloak 테스트의 오류 문자열뿐. 한쪽 제안 | 미적용 |
| `prelogin` | crosscheck | own 제거 2353 | 기각 | t141.1 범위가 로그인·로그아웃 흐름 구현을 명시. 로그인 준비 단계 prelogin이 로그인 흐름 구성으로 성립 | 미적용 |
| `preprocessed` | analyst | preprocess에서 2098 상속 | 채택 | 동사 preprocess의 과거분사라 기본형의 Redis 명령 전처리 뜻이 그대로 성립(own 유지). 기본형 개념의 혼합은 별개 | `rts3-641a078ef551bfc0` |
| `preprocessing` | analyst | preprocess에서 2098 상속 | 채택 | 동명사라 기본형 preprocess의 Redis 명령 전처리 뜻이 그대로 성립(own 유지) | `rts3-641a078ef551bfc0` |
| `preprocessing` | crosscheck | own 추가 3481 | 채택 | C/C++ preprocessing token과 매크로 ## token pasting(terminal). 3481 preprocessor·macro·directive와 같은 소스 전처리 구성 | `rts3-641a078ef551bfc0` |
| `preprocessing` | crosscheck | own 추가 6395 | 채택 | 6395 redis command preprocessing 개념 이름 그대로(preprocessCommand). 반대쪽도 6395 소속을 인정 | `rts3-641a078ef551bfc0` |
| `presort` | analyst | own 추가 4307 | 채택 | terminal presorted_static_map이 미리 정렬된 입력을 전제로 partition_point 검색. 4307 sorted·stable sort 정렬 상태 구성 | `rts3-641a078ef551bfc0` |
| `presort` | analyst | input 제거 4690 | 채택 | 4690은 검색어·구문 지정. Meilisearch·search results의 정렬 관계로 들어온 연결로 사전 정렬은 검색어 지정이 아님. 반대쪽도 범위 미확정으로만 봄 | `rts3-641a078ef551bfc0` |
| `presort` | analyst | own 제거 4304 | 기각 | 기본형 sort가 같은 4304 own. 접두 파생 presort/presorted에 정렬 뜻이 그대로 있음. 불일치 | 미적용 |
| `presorted` | analyst | own 추가 4307 | 채택 | terminal presorted_static_map이 미리 정렬된 입력을 전제로 partition_point 검색. 4307 sorted·stable sort 정렬 상태 구성 | `rts3-641a078ef551bfc0` |
| `presorted` | analyst | input 제거 4690 | 채택 | 4690은 검색어·구문 지정. Meilisearch·search results의 정렬 관계로 들어온 연결로 사전 정렬은 검색어 지정이 아님. 반대쪽도 범위 미확정으로만 봄 | `rts3-641a078ef551bfc0` |
| `presorted` | analyst | own 제거 4304 | 기각 | 기본형 sort가 같은 4304 own. 접두 파생 presort/presorted에 정렬 뜻이 그대로 있음. 불일치 | 미적용 |
| `previews` | analyst | 새 개념 "invoice preview"(t170.3) 단어 previews | 보류 | 한쪽 제안. 복수형만 담는 개념이라 기본형 preview와 함께 정해야 함(Stripe invoice preview) | 미적용 |
| `previews` | analyst | own 추가 6434 | 채택 | gutenberg rich link previews(hasRichPreviews가 URL metadata 조회). 6434 link preview(unfurl)와 같은 대상 | `rts3-641a078ef551bfc0` |
| `prioritized` | crosscheck | own 추가 5528 | 채택 | 5528 Priority wording(중요 표시 어휘, important). gutenberg가 지정 블록을 우선 배치한 상태. 우선성 부여라는 일반 뜻이 성립 | `rts3-641a078ef551bfc0` |
| `processors` | analyst | processor에서 710 상속 | 채택 | 복수형이라 기본형 processor의 메시지 처리 구성 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `processors` | crosscheck | processor에서 2442 상속 | 채택 | 복수형이라 기본형 processor의 Spring Cloud Stream 처리 바인딩 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `processors` | analyst | 새 개념 "Software input processing components"(software_input_processing_components) 단어 processors | 보류 | 양쪽 모두 새 개념·새 주제를 제안하나 범위가 다름(입력 처리 구성요소 일반 vs 규칙 평가). 맞는 기존 주제가 없어 새 주제 범위를 정해야 함 | 미적용 |
| `processors` | analyst | 새 주제 "Software input processing components" | 보류 | 위 새 개념과 함께 보류 | 미적용 |
| `processors` | analyst | own 추가 새 개념 | 보류 | 위 새 개념과 함께 보류 | 미적용 |
| `processors` | crosscheck | 새 개념 "rule evaluation processors"(new:configured-rule-evaluation) 단어 processors | 보류 | 위 analyst 제안과 범위가 달라 함께 보류 | 미적용 |
| `processors` | crosscheck | 새 주제 "Configured rule evaluation" | 보류 | 위 새 개념과 함께 보류 | 미적용 |
| `product types` | crosscheck | own 제거 7044 | 기각 | 양쪽 모두 commerce product type 뜻을 유효로 봄. 6187과의 중복은 뜻 오류가 아니며 제거하면 7044가 빈 개념이 됨 | 미적용 |
| `projected` | analyst | own 추가 6939 | 채택 | terminal cascadia/inc/cppwinrt_utils.h:125-126 handler is a projected type, not an implementation. 6939 Windows Runtime language projections(proj)와 같은 대상 | `rts3-641a078ef551bfc0` |
| `projected` | crosscheck | own 추가 1343 | 채택 | redis modules/vector-sets/vset.c:693 projected = applyProjection(vec, proj_matrix). 1343 matrix·vector·pca 행렬 연산의 결과 벡터 | `rts3-641a078ef551bfc0` |
| `proofs` | crosscheck | proof에서 884 상속 | 채택 | 복수형이라 기본형 proof의 불변식 증명 뜻이 그대로 성립(input 유지, 원천 invariants) | `rts3-641a078ef551bfc0` |
| `rates` | analyst | 새 개념 "shipping rate"(t261.3) 단어 rates | 보류 | 한쪽 제안. 복수형만 담는 개념이라 기본형 rate와 함께 정해야 함(WC_Shipping_Rate) | 미적용 |
| `ratings` | analyst | rating에서 815 상속 | 채택 | 복수형이라 기본형 rating의 유용성 평가 뜻이 그대로 성립(input 유지, 원천 helpful). 기본형 연결 자체의 적합성은 별개 | `rts3-641a078ef551bfc0` |
| `react testing library` | analyst | own 추가 1494 | 채택 | 1494에 act·component test·mount·unmount가 있음. React Testing Library는 컴포넌트를 렌더링·마운트해 검사하는 React 생태계 라이브러리 | `rts3-641a078ef551bfc0` |
| `rects` | crosscheck | 개념 5932 이동 t649.1→t649.2 | 채택 | Rectangle drawing(fillRect)이 좌표 변환 주제에 있음. t649.2는 2D 도형 그리기를 직접 다룸 | `rts3-641a078ef551bfc0` |
| `redisplay` | analyst | input 제거 1026 | 채택 | getComputedStyle의 CSS display 속성 뜻. 다시 그리기 동작 redisplay에 그 명사 뜻이 성립하지 않음. 반대쪽도 미확정으로만 봄 | `rts3-641a078ef551bfc0` |
| `redisplay` | analyst | input 제거 2174 | 채택 | box model·hd의 display 속성·화면 장치 뜻. 다시 그리기 동작 redisplay에 그 명사 뜻이 성립하지 않음. 반대쪽도 미확정으로만 봄 | `rts3-641a078ef551bfc0` |
| `redisplay` | analyst | input 제거 5088 | 채택 | css의 display 속성 뜻. 다시 그리기 동작 redisplay에 그 명사 뜻이 성립하지 않음. 반대쪽도 미확정으로만 봄 | `rts3-641a078ef551bfc0` |
| `redisplay` | analyst | input 제거 551 | 채택 | PWA manifest의 display 모드 뜻. 다시 그리기 동작 redisplay에 그 명사 뜻이 성립하지 않음. 반대쪽도 미확정으로만 봄 | `rts3-641a078ef551bfc0` |
| `reflectors` | crosscheck | 개념 6242 이동 t361.2→x40 | 채택 | kube reflector는 list/watch로 로컬 캐시를 유지(vector kubernetes/reflector.rs). x40 범위가 list/watch consistency를 명시. t361.2는 CRD·operator 구현 | `rts3-641a078ef551bfc0` |
| `registered users` | crosscheck | own 추가 5035 | 채택 | bitwarden onlyRegisteredUsers가 실제 User 계정이 있는 구성원만 선택. 5035 user·user account·users와 같은 계정 객체 | `rts3-641a078ef551bfc0` |
| `remap` | analyst | 새 개념 "event field remapping"(t286.1) 단어 remap | 채택 | vector transforms/remap.rs:54-66,587-622 공개 문서화된 remap transform이 VRL로 이벤트 필드를 재작성. 반대쪽도 이 뜻을 기술. 기존 6306(설정 참조)·2731(source map)과 다른 대상. transforms의 새 개념은 일반 처리 단계(3387)로 표현돼 기각했지만, remap은 필드 재작성이라는 더 좁은 연산이라 별도 개념으로 둠 | `rts3-7b926399d21b0f85` |
| `remap` | analyst | input 제거 4069 | 채택 | MessagePack의 map은 인코딩된 키-값 컨테이너 타입. 재대응 동작 remap에 그 명사 뜻이 성립하지 않음 | `rts3-641a078ef551bfc0` |
| `remap` | analyst | input 제거 408 | 채택 | CBOR의 map 자료형 명사. remap에 성립하지 않음 | `rts3-641a078ef551bfc0` |
| `remap` | analyst | own 제거 2729 | 채택 | 기본형 map의 dict 컨테이너 명사 뜻을 상속한 연결. 동사 re-map에 사전 컨테이너 뜻이 이어지지 않음. 의존 0 | `rts3-641a078ef551bfc0` |
| `remappings` | crosscheck | own 추가 6306 | 채택 | terminal 설정 병합 시 충돌한 색 구성표 이름의 참조 재매핑 목록. 6306 reference remapping(remap)과 같은 대상 | `rts3-641a078ef551bfc0` |
| `resolving` | analyst | resolve에서 3645 상속 | 채택 | 동사 resolve의 -ing 형이라 LSP code action resolve 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `resolving` | analyst | 개념 6863 이동 t849.2→t679.2 | 기각 | data resolver는 서버 데이터 조회. t849.2 Server state queries에 맞고 t679.2(로딩 표시)는 조회 동작이 아님 | 미적용 |
| `rewire` | crosscheck | own 제거 3147 | 기각 | GPIO·bus 핀 배선을 다시 연결하는 하드웨어 뜻이 성립. 같은 3147에 wire가 있음 | 미적용 |
| `rewrites` | analyst | rewrite에서 151 상속 | 채택 | 복수형·3인칭이라 기본형 rewrite의 API 재작성 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `rewritten` | analyst | own 추가 2731 | 기각 | rewritten은 일반 과거분사. 한 코드에서 source map으로 함수명을 바꿨다는 쓰임만으로 source map 개념 구성원이 되지 않음 | 미적용 |
| `rlim` | crosscheck | own 추가 4952 | 채택 | t310.5 범위가 resource limits를 명시하고 cap·limit·quota가 구성원. POSIX rlim_t·rlim_cur/rlim_max 자원 한도와 같은 대상 | `rts3-641a078ef551bfc0` |
| `ru` | analyst | input 제거 3902 | 채택 | 3902는 JS 번들 disk footprint. resource utilization 동의어로 들어온 연결로 rusage의 CPU 시간 사용 통계가 아님 | `rts3-641a078ef551bfc0` |
| `samples` | crosscheck | sample에서 2806 상속 | 기각 | 기본형 sample의 이 연결은 Rx sample 연산자 이름(throttleTime·frp의 관계). 고정 연산자 이름이라 복수 명사 samples로 굴절하지 않음 | 미적용 |
| `samples` | crosscheck | sample에서 3700 상속 | 기각 | 기본형 sample의 이 연결은 Rx sample 연산자 이름(throttleTime·frp의 관계). 고정 연산자 이름이라 복수 명사 samples로 굴절하지 않음 | 미적용 |
| `scalars` | analyst | scalar에서 458 상속 | 채택 | 복수형이라 기본형 scalar의 0차원 tensor 값 뜻이 그대로 성립(input 유지, 원천 tensor) | `rts3-641a078ef551bfc0` |
| `scalars` | analyst | own 추가 6357 (대상 scalar) | 채택 | 개념 이름이 scalar value이고 복수형만 구성원. 양쪽 모두 비컨테이너 값 뜻을 인정. 기본형 scalar 보완 | `rts3-641a078ef551bfc0` |
| `scalars` | crosscheck | scalar에서 4716 상속 | 채택 | 복수형이라 기본형 scalar의 문자열 primitive 값 뜻이 그대로 성립(input 유지, 원천 primitive) | `rts3-641a078ef551bfc0` |
| `services` | analyst | service에서 3439 상속 | 채택 | 복수형이라 기본형 service의 뜻이 그대로 성립(input 유지, 원천 onStart). 기본형 연결 자체의 적합성은 별개 | `rts3-641a078ef551bfc0` |
| `services` | crosscheck | service에서 5122 상속 | 채택 | 복수형이라 기본형 service의 수명주기 이웃 뜻이 그대로 성립(input 유지, 원천 onDestroy·stopped) | `rts3-641a078ef551bfc0` |
| `services` | crosscheck | 새 개념 "Business service offerings"(t261.1) 단어 services | 보류 | 한쪽 제안이고 근거가 marketplace UI 분류 문자열 하나. 복수형만 담는 개념이라 기본형 service와 함께 정해야 함 | 미적용 |
| `short date string` | crosscheck | datestr에서 1746 상속 | 보류 | 형태상 굴절 관계가 아니고, 원천 str persona의 dateStr 관계가 format specifier·보간 뜻인지 일반 문자열 이름인지 확정 근거 부족(한쪽 상속, 한쪽 불확실) | 미적용 |
| `short date string` | crosscheck | datestring에서 1032 상속 | 채택 | 1032 범위가 날짜 parsing·formatting·serializing이고 strftime·ctime이 구성원. ToShortDateString의 짧은 날짜 문자열은 날짜 서식화 결과로 성립(input 유지, 원천 ctime) | `rts3-641a078ef551bfc0` |
| `signed cookies` | analyst | input 제거 3489 | 채택 | 3489는 S3 SDK 객체 전송·서명 URL. pre signed url persona에서 signed 철자로 들어온 연결로 쿠키 서명·검증과 다른 기능. 반대쪽도 연계를 입증하지 못함 | `rts3-641a078ef551bfc0` |
| `sizer` | crosscheck | 새 개념 "batch item size estimator"(t85.4) 단어 sizer | 보류 | 한쪽 제안. 근거가 vector 내부 StatsdBatchSizer 하나라 일반 sizer(크기 계산기) 뜻과 별도 개념이 필요한지 미정 | 미적용 |
| `sizer` | crosscheck | own 제거 4269 | 채택 | 4269는 파일 크기 메타데이터 값. 행위자 명사 sizer(크기 계산기)에 파일 크기 값 뜻이 이어지지 않음. 반대쪽도 미확정으로만 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `sizer` | crosscheck | own 제거 4270 | 기각 | UI 레이아웃 sizer(위젯 치수를 정하는 구성요소, AutoSizer 등)가 반응형 치수 주제에 성립 | 미적용 |
| `sizes` | crosscheck | size에서 2252 상속 | 채택 | 복수형이라 기본형 size의 입력 길이·크기 제약 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `sizes` | crosscheck | 새 개념 "image size variants"(t223) 단어 sizes | 보류 | 한쪽 제안. 복수형만 담는 개념이라 기본형 size와 함께 정해야 함(WordPress 등록 이미지 크기) | 미적용 |
| `slaves` | analyst | own 추가 3499 | 채택 | redis replication의 slaves는 replica 서버 연결(역사적 명칭). 3499 replica·secondary·master·primary와 같은 대상이라는 직접 코드 근거로 own 추가(input 3건 유지) | `rts3-641a078ef551bfc0` |
| `snippets` | analyst | own 추가 3156 | 기각 | 재사용 텍스트 조각 뜻은 815(snippet·snippet expansion·live template)에 이미 있음. Terminal snippet이 SendInput 명령으로 구현된다는 내부 방식만으로 command action 구성원이 되지 않음 | 미적용 |
| `snippets` | crosscheck | 새 개념 "Reusable terminal input snippets"(t326.4) 단어 snippets | 기각 | 재사용 입력 조각 뜻은 815에 이미 있어 터미널 적용을 별도 개념으로 나누지 않음 | 미적용 |
| `sourced` | analyst | 새 개념 "pipeline event production"(t241.3) 단어 sourced | 기각 | source에서 공급된 이벤트 뜻은 sourced의 2442(수집 source) 소속으로 표현됨. 테스트 집계 문맥별 개념을 따로 만들지 않음 | 미적용 |
| `sourced` | analyst | own 제거 3671 | 채택 | PRNG·entropy source 명사 뜻을 과거분사 sourced에 own으로 복사한 연결. 공급·획득 완료 상태에 그 명사 뜻이 성립하지 않음. 반대쪽도 미확정으로만 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `sourced` | analyst | own 제거 4308 | 채택 | taint source 명사 뜻을 과거분사 sourced에 own으로 복사한 연결. 공급·획득 완료 상태에 그 명사 뜻이 성립하지 않음. 반대쪽도 미확정으로만 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `sourced` | analyst | own 제거 4309 | 채택 | compiler emitter의 출력원 명사 뜻을 과거분사 sourced에 own으로 복사한 연결. 공급·획득 완료 상태에 그 명사 뜻이 성립하지 않음. 반대쪽도 미확정으로만 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `sourced` | analyst | own 제거 4353 | 채택 | imgix 이미지 source 설정 명사 뜻을 과거분사 sourced에 own으로 복사한 연결. 공급·획득 완료 상태에 그 명사 뜻이 성립하지 않음. 반대쪽도 미확정으로만 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `sourced` | analyst | own 제거 809 | 보류 | 셸 source 명령으로 실행된(sourced) 스크립트 뜻이 해석기 주제에 성립할 수 있음(반대쪽 근거: Tcl source). 소스 코드 명사 연결과 구별할 근거 부족 | 미적용 |
| `sourced` | crosscheck | input 제거 1369 | 채택 | 1369는 목적지·sink 어휘. dest·destination의 반대 개념으로 들어온 연결로 sourced는 목적지 표현이 아님 | `rts3-641a078ef551bfc0` |
| `sourced` | crosscheck | input 제거 943 | 채택 | memcpy·strcpy의 source operand 명사. 과거분사 sourced에 성립하지 않음 | `rts3-641a078ef551bfc0` |
| `sources` | analyst | source에서 4309 상속 | 채택 | 복수형이라 기본형 source의 event source·producer 뜻이 그대로 성립(own 유지). 기본형 개념의 주제 적합성은 별개 | `rts3-641a078ef551bfc0` |
| `sources` | analyst | own 추가 6667 | 채택 | 6667 source(입출력 흐름의 공급원, src). vector pipeline upstream sources 등 일반 공급원 복수 | `rts3-641a078ef551bfc0` |
| `sources` | analyst | own 추가 6852 | 채택 | terminal DisabledProfileSources의 sources는 동적 profile generator namespace(generator.GetNamespace 비교). 6852 terminal profile generator(generators)와 같은 대상 | `rts3-641a078ef551bfc0` |
| `sources` | crosscheck | source에서 4644 상속 | 채택 | 복수형이라 기본형 source의 Tailwind content source(스캔 대상 파일) 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `sources` | crosscheck | 개념 6303 이동 t303.1→t289.1 | 기각 | 6303 profile generator의 DisabledProfileSources는 사용자 preference 설정이라 현재 주제도 성립. sources를 6852에 추가해 이웃 관계를 보완 | 미적용 |
| `sources` | crosscheck | 새 개념 "extension installation source"(t42.1) 단어 sources | 기각 | gutenberg wp-env 설정 파서의 출처 descriptor. source의 일반 출처 뜻을 한 저장소 자료 구조 단위로 나누는 개념 | 미적용 |
| `sources` | crosscheck | 새 개념 "style value provenance"(t164.1) 단어 sources | 기각 | global styles의 core/theme/user 기여 출처는 6631 configuration source(설정 층 출처)로 표현됨 | 미적용 |
| `sources` | crosscheck | own 추가 6007 | 채택 | gutenberg pluginSources·themeSources가 mount할 원본 소스 디렉터리. 6007 Source directories(src)와 같은 대상이라는 직접 근거로 own 추가(input 유지) | `rts3-641a078ef551bfc0` |
| `sources` | crosscheck | own 추가 6631 | 채택 | gutenberg global styles의 sources가 path별 기여 층(core/theme/user)을 기록. 6631 configuration source와 같은 설정 출처 대상(data sources 행과 같은 근거) | `rts3-641a078ef551bfc0` |
| `specs` | analyst | spec에서 4701 상속 | 채택 | 복수형이라 기본형 spec의 mock 인터페이스 명세 뜻이 그대로 성립(input 유지, 원천 MagicMock 등) | `rts3-641a078ef551bfc0` |
| `specs` | crosscheck | 새 개념 "inbound event filter specifications"(inbound-event-filtering) 단어 specs | 기각 | sentry 내부 _FilterSpec 메타데이터. spec의 일반 명세 뜻을 한 저장소 기능 단위로 나누는 개념 | 미적용 |
| `specs` | crosscheck | 새 주제 "Inbound event filtering" | 기각 | 위 새 개념 기각에 따름 | 미적용 |
| `specs` | crosscheck | own 제거 1079 | 보류 | 1079(로그 수집 파이프라인) 소속 근거가 불명확하나 반대쪽은 잠정 유지. 대체 개념 기각으로 제거 시 근거 없이 뜻이 사라짐 | 미적용 |
| `spis` | analyst | serviceproviderinterface에서 2377 상속 | 채택 | SPI(Service Provider Interface)의 널리 쓰는 약어 복수. 원래 말 serviceproviderinterface의 Java ServiceLoader 뜻에 연결(input 유지, 원천 implements) | `rts3-641a078ef551bfc0` |
| `stars` | analyst | own 추가 5342 | 채택 | sentry prebuilt stars가 DashboardFavoriteUser favorited 항목. 5342의 starred·favorited·favorite과 같은 대상이라는 직접 코드 근거로 own 추가 | `rts3-641a078ef551bfc0` |
| `stars` | analyst | own 추가 6225 | 채택 | woocommerce 상품평 1~5 별점 선택 UI. 6225 product rating(ratings)과 같은 대상 | `rts3-641a078ef551bfc0` |
| `stars` | analyst | own 추가 6913 | 채택 | gin tree.go가 경로의 * 문자 수를 셈. 6913 asterisk character와 같은 문자 대상 | `rts3-641a078ef551bfc0` |
| `stars` | crosscheck | star에서 5342 상속 | 채택 (같은 행의 다른 조치로 충족) | 즐겨찾기 표시 뜻은 위 own 추가로 같은 5342에 반영. 별도 input은 중복이라 적용하지 않음 | `rts3-641a078ef551bfc0` |
| `stashing` | analyst | stash에서 6041 상속 | 채택 | vector line_agg가 두 번째 줄을 임시 슬롯에 보관하는 동작. 6041 temporary stream buffer(stash)에 보관 동작이 성립 | `rts3-641a078ef551bfc0` |
| `stashing` | crosscheck | stash에서 5211 상속 | 채택 | 동사 stash의 -ing 형이라 WIP 변경 임시 보관 뜻이 그대로 성립(input 유지, 원천 wip) | `rts3-641a078ef551bfc0` |
| `string values` | crosscheck | stringvalue에서 4231 상속 | 채택 | 복수형이라 기본형 string value의 설정값 이웃 뜻이 그대로 성립(input 유지, 원천 config value). 기본형 연결 자체의 적합성은 별개 | `rts3-641a078ef551bfc0` |
| `subkeys` | analyst | 새 개념 "Redis hash subkeys"(t665) 단어 subkeys | 보류 | 한쪽 제안. 복수형만 담는 Redis 전용 개념이라 기본형 subkey·hash field 표현과 함께 정해야 함 | 미적용 |
| `subtotals` | analyst | input 제거 4473 | 채택 | 4473은 카운터·루프·진행 수 증감. waterfall chart의 근접 관계로 들어온 연결로 소계 집계는 카운터 감소가 아님. 반대쪽도 범위 미확정으로만 봄 | `rts3-641a078ef551bfc0` |
| `subvalue` | analyst | own 추가 3391 | 채택 | sentry locale.tsx TemplateSubvalue는 파싱된 템플릿의 literal 조각·중첩 group 참조. 3391의 literal·placeholder·template variable과 같은 템플릿 구성 요소 | `rts3-641a078ef551bfc0` |
| `subvalue` | crosscheck | 새 개념 "parsed template fragment"(t595.2) 단어 subvalue | 기각 | 양쪽 모두 같은 주제 t595.2로 봄. 기존 3391 template 구성 요소로 표현 가능해 새 개념 불필요 | 미적용 |
| `subvalue` | crosscheck | input 제거 163 | 채택 | Spring bean 수명주기 뜻은 구조 내부의 하위 값 subvalue에 성립하지 않음. 반대쪽도 연결을 입증하지 못함 | `rts3-641a078ef551bfc0` |
| `subvalue` | crosscheck | input 제거 258 | 채택 | async iterator 완료 프로토콜 뜻은 구조 내부의 하위 값 subvalue에 성립하지 않음. 반대쪽도 연결을 입증하지 못함 | `rts3-641a078ef551bfc0` |
| `subvalue` | crosscheck | input 제거 3552 | 채택 | progress bar 진행 값 뜻은 구조 내부의 하위 값 subvalue에 성립하지 않음. 반대쪽도 연결을 입증하지 못함 | `rts3-641a078ef551bfc0` |
| `suggestions` | analyst | own 추가 2292 | 채택 | terminal 명령줄 후보·ghost-text 미리보기·WinGet quick fix 제안. 2292 readline의 completion·history·prompt 구성. 반대쪽도 같은 주제 소속을 제안 | `rts3-641a078ef551bfc0` |
| `suggestions` | analyst | own 추가 7136 | 채택 | gutenberg 검색 entity 후보 배열이 listbox로 표시. 7136 suggestion list와 같은 후보 목록 | `rts3-641a078ef551bfc0` |
| `suggestions` | crosscheck | own 추가 3109 | 채택 | terminal commandline completion 후보. 3109 completion·shell completion과 같은 대상 | `rts3-641a078ef551bfc0` |
| `tokenable` | analyst | own 제거 3058 | 기각 | bitwarden SsoEmail2faSessionTokenable 등이 보호 토큰으로 직렬화돼 익명 endpoint 인증에 쓰임. 요청 인증 자격 증명의 데이터 구성 요소로 성립 | 미적용 |
| `tokenable` | crosscheck | own 제거 4184 | 채택 | 4184는 HTTP cookie 세션 식별자·세션 공격. 인증 payload 모델이 cookie 세션 ID는 아님. 반대쪽도 경계를 미확정으로만 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `trace logging` | analyst | tracelog에서 4850 상속 | 채택 | Windows TraceLogging은 ETW 기반 구조화 이벤트 추적. 4850 범위의 tracing 도구(etl·perf·strace)와 같은 기록 문맥(input 유지, 원천 etl·trace·tracing) | `rts3-641a078ef551bfc0` |
| `trace logging` | crosscheck | own 추가 812 | 채택 | vector trace! 이벤트·keycloak의 TRACE 수준 진단 로깅. 812 log의 trace·debug·logger 구성과 같은 일반 로깅 뜻 | `rts3-641a078ef551bfc0` |
| `traits` | crosscheck | trait에서 1222 상속 | 채택 | 복수형이라 기본형 trait의 class 계층 구성 이웃 뜻이 그대로 성립(input 유지, 원천 derived) | `rts3-641a078ef551bfc0` |
| `traits` | crosscheck | trait에서 3356 상속 | 채택 | 복수형이라 기본형 trait의 지원 특성·능력 뜻이 그대로 성립(own 유지) | `rts3-641a078ef551bfc0` |
| `traits` | crosscheck | input 제거 3881 | 채택 | 3881은 must·shall·spec 규범 의무 어휘. Protocol persona의 언어 trait 관계가 규범 persona에 붙은 연결. 단수 trait는 같은 원 관계에서 structural typing(1376)에 있음 | `rts3-641a078ef551bfc0` |
| `transforms` | analyst | own 추가 3387 | 채택 | 3387에 Beam ParDo·GroupByKey 처리 단계(PTransform)가 있음. vector transforms(filter·remap)도 파이프라인 처리 단계로 같은 구성 | `rts3-641a078ef551bfc0` |
| `transforms` | crosscheck | 새 개념 "event-processing transform"(t241.3) 단어 transforms | 기각 | 양쪽 모두 같은 주제 t241.3. 기존 3387로 표현 가능해 새 개념 불필요 | 미적용 |
| `translators` | analyst | own 추가 2298 | 채택 | woocommerce 벤더 Symfony CssSelector XPath/Translator.php가 selector AST를 XPath로 변환. 2298의 transpiler·translation·codegen과 같은 구성 | `rts3-641a078ef551bfc0` |
| `translators` | crosscheck | 새 개념 "selector syntax translator"(t161.1) 단어 translators | 기각 | 양쪽 모두 같은 주제 t161.1. 기존 2298로 표현 가능해 새 개념 불필요 | 미적용 |
| `translators` | crosscheck | translator에서 2630 상속 | 채택 | 복수형이라 기본형 translator의 번역 도구 뜻이 그대로 성립(input 유지, 원천 DeepL) | `rts3-641a078ef551bfc0` |
| `trimmed` | analyst | own 제거 4684 | 기각 | 기본형 trim이 같은 4684 own. 과거분사 trimmed만 빼면 불일치. 기본형 소속 자체의 적합성은 별개 | 미적용 |
| `trimming` | crosscheck | own 추가 1547 | 채택 | redis 슬롯 trimming·LTRIM/XTRIM은 원소 제거 동작. 1547 remove·drop·evict 제거 구성 | `rts3-641a078ef551bfc0` |
| `typing` | crosscheck | type에서 1372 상속 | 채택 | t509 범위가 type 기반 분류와 dependent typing을 명시. 타입 부여 typing이 그대로 성립(own 유지) | `rts3-641a078ef551bfc0` |
| `unassigned` | analyst | 새 개념 "provider organization assignment"(x11) 단어 unassigned | 기각 | bitwarden ProviderOrganizations 연결 부재라는 한 저장소 업무 관계. unassigned의 일반 뜻(배정되지 않음)을 기능 단위 개념으로 나누지 않음 | 미적용 |
| `unassigned` | analyst | own 추가 3394 | 채택 | bitwarden 공급자 구독의 미배정 청구 좌석(SeatMinimum − AssignedSeats). 3394의 seat·occupied·reserved와 같은 좌석 상태 구성(SaaS 좌석 과금 용어) | `rts3-641a078ef551bfc0` |
| `units` | analyst | unit에서 1438 상속 | 기각 | 기본형 unit의 이 연결은 unit 타입(값 없는 반환·()) 이름. 고정 타입 이름이라 복수 units(측정 단위·텍스트 단위)로 굴절하지 않음 | 미적용 |
| `units` | analyst | unit에서 3928 상속 | 기각 | 기본형 unit의 이 연결은 unit 타입(값 없는 반환·()) 이름. 고정 타입 이름이라 복수 units(측정 단위·텍스트 단위)로 굴절하지 않음 | 미적용 |
| `units` | crosscheck | own 추가 6118 | 채택 | terminal UIA TextUnit(문자·단어·줄·문서) 이동 단위. 6118 accessibility text units(document)와 같은 대상군 | `rts3-641a078ef551bfc0` |
| `unparsed` | crosscheck | unparse에서 5978 상속 | 채택 | 동사 unparse의 과거분사라 pretty printer의 구문→텍스트 복원 뜻이 그대로 성립(input 유지). 미파싱 형용사 뜻과 함께 유효 | `rts3-641a078ef551bfc0` |
| `unshared` | analyst | own 추가 3768 | 채택 | redis reply의 refcount==1 단독 보유 메모리(server.c:6586, networking.c:1991-2035). 3768의 shared·shared reference·holds·own과 같은 참조 계수 소유 구성 | `rts3-641a078ef551bfc0` |
| `unshared` | crosscheck | 새 개념 "exclusively owned memory"(t307.1) 단어 unshared | 기각 | 양쪽 모두 같은 주제 t307.1. 기존 3768로 표현 가능해 새 개념 불필요 | 미적용 |
| `unslash` | analyst | own 추가 5448 | 채택 | WordPress wp_unslash가 escaping backslash를 되돌림(woocommerce GET·term 처리). 5448 문자 이스케이프(escape·escaper)와 같은 체계의 동작 | `rts3-641a078ef551bfc0` |
| `unsubscribed` | analyst | unsubscribe에서 3468 상속 | 채택 | 동사 unsubscribe의 과거분사라 서비스 정리 시 구독 해제 뜻이 그대로 성립(input 유지, 원천 destroy·teardown) | `rts3-641a078ef551bfc0` |
| `unsubscribed` | crosscheck | 새 개념 "issue notification subscription state"(x12) 단어 unsubscribed | 보류 | 한쪽 제안. 알림 구독 상태 개념은 subscribe·subscribed 등과 함께 정해야 하며 근거가 sentry 하나 | 미적용 |
| `untag` | crosscheck | input 제거 653 | 채택 | 653은 차트 범주·시리즈·범례 라벨. categories·tsdb의 태그 관계로 들어온 연결로 태그 해제(tagged JSON 복원) 뜻이 아님 | `rts3-641a078ef551bfc0` |
| `untrash` | analyst | 새 개념 "Soft-deleted record restoration"(t453.2) 단어 untrash | 채택 | 양쪽 모두 같은 주제 t453.2로 휴지통 복원 새 개념을 제안(이름만 다름). woocommerce ProductVersionStringInvalidator.php:58-64 untrashed_post, wp_untrash_post | `rts3-7b926399d21b0f85` |
| `untrash` | crosscheck | 새 개념 "trash restoration"(t453.2) 단어 untrash (같은 개념 "Soft-deleted record restoration"으로 합침) | 채택 | 위 새 개념과 같은 개념으로 함께 적용 | `rts3-7b926399d21b0f85` |
| `untrash` | crosscheck | input 제거 1188 | 채택 | 1188은 파일·레코드 삭제. recursive delete persona로 들어온 연결로 휴지통 복원은 삭제가 아님. 복원 뜻은 새 개념으로 보존 | `rts3-641a078ef551bfc0` |
| `value changed` | analyst | input 제거 1528 | 기각 | 1528은 ETag·CAS·optimistic lock의 갱신 표지. 저장 값이 바뀌었는지 비교하는 것이 CAS의 핵심이라 value changed의 근접 뜻이 성립 | 미적용 |
| `valued` | analyst | 새 개념 "attribute value multiplicity"(t135.1) 단어 valued | 보류 | 한쪽 제안. single-/multi-valued 속성의 값 개수 뜻이 valued 단독 조각에 성립하는지 미정 | 미적용 |
| `valued` | analyst | input 제거 163 | 채택 | Spring bean 수명주기 뜻은 값을 갖는다는 형용사 valued에 성립하지 않음. 반대쪽도 연결을 입증하지 못함 | `rts3-641a078ef551bfc0` |
| `valued` | analyst | input 제거 258 | 채택 | async iterator 완료 프로토콜 뜻은 값을 갖는다는 형용사 valued에 성립하지 않음. 반대쪽도 연결을 입증하지 못함 | `rts3-641a078ef551bfc0` |
| `valued` | analyst | input 제거 3552 | 채택 | progress bar 진행 값 뜻은 값을 갖는다는 형용사 valued에 성립하지 않음. 반대쪽도 연결을 입증하지 못함 | `rts3-641a078ef551bfc0` |
| `valued` | analyst | own 제거 2603 | 채택 | 2603은 상수·리터럴·고정 설정값. 값을 갖는다는 valued는 고정 상수가 아님. 반대쪽도 미확정으로만 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `variants` | analyst | 새 개념 "event grouping variants"(t273.3) 단어 variants | 기각 | 양쪽 모두 같은 주제 t273.3. 기존 6704 issue grouping fingerprint로 표현 가능 | 미적용 |
| `variants` | crosscheck | own 추가 6704 | 채택 | sentry grouping/api.py:329-459 grouping variants가 component tree에서 grouping hash를 만들고 contributes 상태를 가짐. 6704와 같은 이슈 그룹화 구성 | `rts3-641a078ef551bfc0` |
| `variants` | crosscheck | input 제거 2851 | 채택 | 2851은 버전 업그레이드·마이그레이션. versions의 동의어로 들어온 연결로 대안 변형 variants는 업그레이드 전환이 아님 | `rts3-641a078ef551bfc0` |
| `viewers` | analyst | 새 개념 "content viewer"(t824) 단어 viewers | 보류 | 한쪽 제안. 복수형만 담는 개념이라 기본형 viewer와 함께 정해야 함 | 미적용 |
| `virtualize` | analyst | input 제거 2263 | 채택 | 2263은 문서 DB CRUD. populate의 virtual 공존 관계로 들어온 연결로 목록 가상화 동사가 아님. 반대쪽도 미확정으로만 봄 | `rts3-641a078ef551bfc0` |
| `waitable` | analyst | own 제거 636 | 채택 | 636은 CSP 채널 송수신. -able 형 waitable(대기 가능한 동기화 객체)에 wait 동작 뜻이 이어지지 않으며 그 뜻은 새 개념 7335(waitable)에 있음. 반대쪽도 미확정으로만 봄. 의존 0 | `rts3-641a078ef551bfc0` |
| `waitable` | crosscheck | input 제거 91 | 채택 | 91은 반복 trigger 지연(debounce). 대기 가능 성질이 debounce 뜻이 아님 | `rts3-641a078ef551bfc0` |
| `waiters` | analyst | waiter에서 1224 상속 | 채택 | 복수형이라 기본형 waiter의 AWS 리소스 상태 waiter 뜻이 그대로 성립(input 유지, 원천 RunInstances) | `rts3-641a078ef551bfc0` |
| `waiters` | analyst | waiter에서 4024 상속 | 채택 | 복수형이라 기본형 waiter의 SDK waiter 뜻이 그대로 성립(input 유지, 원천 boto3.client) | `rts3-641a078ef551bfc0` |
| `walks` | crosscheck | walk에서 3032 상속 | 채택 | 복수형·3인칭이라 기본형 walk의 파일 시스템 순회 이웃 뜻이 그대로 성립(input 유지, 원천 os) | `rts3-641a078ef551bfc0` |
| `walks` | crosscheck | 개념 6045 이동 t354.1→t837.1 | 기각 | random walk는 인접 정점을 따라가는 그래프 순회라 현재 주제(인접 정점 탐색)에 성립. 무작위 표본 선택은 한 용도 | 미적용 |
| `wcs` | analyst | own 추가 6229 | 기각 | wcs를 WooCommerce Shipping 제품 약칭으로 쓰는 것은 플러그인 내부 변수 접두이고 다른 제품(WooCommerce Subscriptions)과도 겹침. 널리 쓰는 약어 기준에 맞지 않음 | 미적용 |
| `weighted` | analyst | own 추가 4382 | 채택 | vector test_util/stats.rs WeightedSum·TimeWeightedSum 가중 평균. 4382 average·mean·variance와 같은 통계 구성 | `rts3-641a078ef551bfc0` |
| `weighted` | crosscheck | weight에서 1163 상속 | 채택 | 신경망의 가중 입력·가중합 뜻이 weighted에 성립(own 유지). 같은 개념에 weight·model weights가 있음 | `rts3-641a078ef551bfc0` |
| `weighted` | crosscheck | weight에서 6046 상속 | 채택 | 가중 무작위 표본 선택 뜻이 weighted에 성립(own 유지). 반대쪽도 가중 표본 선택 자체는 유효로 봄 | `rts3-641a078ef551bfc0` |
| `weighted` | crosscheck | 새 개념 "weighted sum"(t81.2) 단어 weighted | 기각 | 4382 own 추가로 같은 주제에서 가중 평균 뜻이 표현됨 | 미적용 |
| `windowing` | analyst | window에서 2945 상속 | 기각 | 기본형 window의 2945 연결은 WPF·WinUI 창 객체 명사. 창 관리 정책 windowing이 MVVM 바인딩 뜻이 아니며 설정이 바인딩된다는 사실은 소속 근거가 아님 | 미적용 |
| `windowing` | crosscheck | window에서 3679 상속 | 채택 | 기본형 window의 sliding window(시간 창) 뜻에서 windowing(시간 창 나누기)이 그대로 성립(input 유지, 원천 rate limiter) | `rts3-641a078ef551bfc0` |
| `windowing` | crosscheck | window에서 3876 상속 | 채택 | 기본형 window의 sliding window(시간 창) 뜻에서 windowing(시간 창 나누기)이 그대로 성립(input 유지, 원천 request throttling) | `rts3-641a078ef551bfc0` |
| `windowing` | crosscheck | window에서 5265 상속 | 채택 | 기본형 window의 sliding window(시간 창) 뜻에서 windowing(시간 창 나누기)이 그대로 성립(input 유지, 원천 X-RateLimit 헤더) | `rts3-641a078ef551bfc0` |

## R2 보류 재판단

R1~R3 처리에서 보류한 R2 31건을 위 기준으로 다시 판단했다.

| 단어 | 조치 | 결정 | 이유 | 데이터셋 |
| --- | --- | --- | --- | --- |
| `cacher` | own 제거 1530 | 기각 | 1530 범위에 read through·write through 소프트웨어 캐시가 있고 기본형 cache도 같은 own. 캐시 구성요소 cacher가 성립 | 미적용 |
| `completed task` | completed에서 1574 상속 | 기각 | 상속 규칙은 파생어·활용형 대상. completed task는 .NET Task.CompletedTask 이름을 보존한 문구이고 completed는 그 수식어라 파생 관계가 아님 | 미적용 |
| `completed task` | completed에서 3219 상속 | 기각 | 상속 규칙은 파생어·활용형 대상. completed task는 .NET Task.CompletedTask 이름을 보존한 문구이고 completed는 그 수식어라 파생 관계가 아님 | 미적용 |
| `completed task` | completed에서 3535 상속 | 기각 | 상속 규칙은 파생어·활용형 대상. completed task는 .NET Task.CompletedTask 이름을 보존한 문구이고 completed는 그 수식어라 파생 관계가 아님 | 미적용 |
| `completed task` | completed에서 3551 상속 | 기각 | 상속 규칙은 파생어·활용형 대상. completed task는 .NET Task.CompletedTask 이름을 보존한 문구이고 completed는 그 수식어라 파생 관계가 아님 | 미적용 |
| `completed task` | completed에서 361 상속 | 기각 | 상속 규칙은 파생어·활용형 대상. completed task는 .NET Task.CompletedTask 이름을 보존한 문구이고 completed는 그 수식어라 파생 관계가 아님 | 미적용 |
| `completed task` | completed에서 4199 상속 | 기각 | 상속 규칙은 파생어·활용형 대상. completed task는 .NET Task.CompletedTask 이름을 보존한 문구이고 completed는 그 수식어라 파생 관계가 아님 | 미적용 |
| `completed task` | completed에서 4419 상속 | 기각 | 상속 규칙은 파생어·활용형 대상. completed task는 .NET Task.CompletedTask 이름을 보존한 문구이고 completed는 그 수식어라 파생 관계가 아님 | 미적용 |
| `completed task` | completed에서 5168 상속 | 기각 | 상속 규칙은 파생어·활용형 대상. completed task는 .NET Task.CompletedTask 이름을 보존한 문구이고 completed는 그 수식어라 파생 관계가 아님 | 미적용 |
| `crashing` | own 제거 2004 | 기각 | -ing 활용형이고 기본형 crash가 같은 2004 own. 불일치 | 미적용 |
| `cvt` | own 제거 3966 | 기각 | cvt는 convert의 널리 쓰는 약어이고 convert가 같은 3966 own(ecvt/fcvt도 10진 변환 함수). 원래 말과 같은 뜻에 연결 | 미적용 |
| `cvt` | own 제거 4863 | 기각 | convert가 같은 4863 own. 약어는 원래 말과 같은 뜻에 연결 | 미적용 |
| `dismissal` | own 추가 5637 | 채택 | 뜻은 맞고(대화상자 닫힘), 막고 있던 A3(주제 분야 오류)를 5637을 t773으로 옮겨 해결 | `rts3-641a078ef551bfc0` |
| `dismissals` | own 추가 5637 | 채택 | dismissal과 같음 | `rts3-641a078ef551bfc0` |
| `fmadd` | own 추가 6675 (대상 fusedmultiplyadd) | 기각 | fusedmultiplyadd는 표제어가 없고 12개 저장소 수집 단어도 아님(수집된 것은 fmadd·_mm256_fmadd_ps). fmadd는 이미 6675 own | 미적용 |
| `invalidator` | own 제거 5241 (대상 invalidate) | 채택 | 5241은 dirty 값을 저장소에 다시 쓰는 write-behind. 무효화는 쓰기 지연 정책이 아님. 관계 dirty·markdirty·dirtyflag를 invalidate의 캐시 무효화 persona(16844)로 옮기고 markdirty input도 재연결한 뒤 제거(정밀 편집) | `rts3-9e01eaf1eadf960a` |
| `lagging` | own 제거 2519 | 채택 | 기본형 lag의 2519 뜻은 SQL LAG 함수 이름. 고정 함수 이름이라 lagging(뒤처짐)으로 굴절하지 않음. 의존 0 | `rts3-641a078ef551bfc0` |
| `lockouts` | own 제거 3920 | 기각 | 복수형이고 기본형 lockout이 같은 3920 own. 불일치 | 미적용 |
| `member names` | own 추가 2774 (대상 membername) | 기각 | membername은 표제어가 없고 수집 단어도 아님(수집된 것은 member names 문구). member names는 이미 2774 own | 미적용 |
| `member names` | own 추가 7151 (대상 membername) | 기각 | membername 표제어 없음. member names는 이미 7151 own | 미적용 |
| `paginating` | own 제거 2709 | 기각 | -ing 활용형이고 기본형 paginate가 같은 2709 own. 불일치 | 미적용 |
| `pointer moved` | pointermove에서 3381 상속 | 채택 | XAML PointerMoved와 DOM pointermove는 같은 포인터 이동 이벤트의 플랫폼별 이름. 기본형의 pan·pointing·pointer input 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `pointer moved` | pointermove에서 5665 상속 | 채택 | XAML PointerMoved와 DOM pointermove는 같은 포인터 이동 이벤트의 플랫폼별 이름. 기본형의 pan·pointing·pointer input 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `pointer moved` | pointermove에서 708 상속 | 채택 | XAML PointerMoved와 DOM pointermove는 같은 포인터 이동 이벤트의 플랫폼별 이름. 기본형의 pan·pointing·pointer input 뜻이 그대로 성립(input 유지) | `rts3-641a078ef551bfc0` |
| `revokable` | own 제거 1188 | 채택 | 1188은 파일·레코드 삭제 동작. -able 형 revokable(회수 가능)에 삭제 동작 뜻이 이어지지 않음(기본형 revoke의 1188 연결은 revokeObjectURL). 의존 0 | `rts3-641a078ef551bfc0` |
| `rxs` | 새 개념 "channel receiver"(t297.1) 단어 rxs | 보류 | 수신 endpoint 개념은 rx·receiver와 함께 정해야 함. 한쪽 제안 | 미적용 |
| `savings` | 새 개념 "execution cost savings"(t401.4) 단어 savings | 보류 | 한쪽 제안의 신규 개념. 기존 6681(앱 크기 절감) 외 뜻별 분할 필요성 미확정 | 미적용 |
| `savings` | 새 개념 "memory size savings"(t234.4) 단어 savings | 보류 | 한쪽 제안의 신규 개념. 기존 6681(앱 크기 절감) 외 뜻별 분할 필요성 미확정 | 미적용 |
| `savings` | 새 개념 "monetary cost savings"(t111.1) 단어 savings | 보류 | 한쪽 제안의 신규 개념. 기존 6681(앱 크기 절감) 외 뜻별 분할 필요성 미확정 | 미적용 |
| `subcomponents` | own 추가 823 | 채택 | gutenberg ui/src/*/stories/index.story.tsx의 Storybook Meta subcomponents 키(공개 Storybook API). 823 Storybook 개념 구성 | `rts3-641a078ef551bfc0` |
| `unfocused` | focused에서 455 상속 | 채택 | 455는 시각적 focus 장식. terminal Theme.idl UnfocusedFrame·UnfocusedTabOpacity 등 비포커스 상태의 표시 장식이 같은 범위(input 유지, 원천 focus indicator) | `rts3-641a078ef551bfc0` |

## 조회 변화 보완

조회 도구는 own이 없는 단어를 기본형으로 넓힌다. R1~R3에서 own이 생기며 빠진 뜻과 기본형에 남은 잘못된 연결을 처리했다.

| 단어 | 조치 | 결정 | 이유 | 데이터셋 |
| --- | --- | --- | --- | --- |
| `investigations` | investigation에서 4081 상속 | 채택 | own이 생겨 조회가 기본형으로 넓어지지 않으면서 빠진 뜻. 기본형 investigation의 agile spike(조사 작업) input이 복수형에 성립 | `rts3-641a078ef551bfc0` |
| `preds` | pred에서 2222 상속 | 채택 | own이 생겨 기본형 확장이 멈추면서 빠진 뜻. 기본형 pred의 predicate 함수 인자(find·every·any) 뜻이 복수형 preds에 성립(own 유지) | `rts3-641a078ef551bfc0` |
| `preds` | pred에서 4693 상속 | 채택 | own이 생겨 기본형 확장이 멈추면서 빠진 뜻. 기본형 pred의 테스트 predicate matcher 뜻이 복수형 preds에 성립(own 유지) | `rts3-641a078ef551bfc0` |
| `preds` | pred에서 1365 상속 | 채택 | own이 생겨 기본형 확장이 멈추면서 빠진 뜻. 기본형 pred의 takewhile·dropwhile 조건 뜻이 복수형 preds에 성립(own 유지) | `rts3-641a078ef551bfc0` |
| `preds` | pred에서 16 상속 | 채택 | own이 생겨 기본형 확장이 멈추면서 빠진 뜻. 기본형 pred의 predecessor·previous 약어 뜻이 복수형 preds에 성립(own 유지) | `rts3-641a078ef551bfc0` |
| `slave` | input 제거 30 | 채택 | 파생형 slaves의 같은 연결을 1단계에서 두 검토자 합의로 제거함. CORS origin whitelist persona의 master/slave 관계라 기본형에도 CORS 뜻이 성립하지 않음 | `rts3-641a078ef551bfc0` |
| `negation` | input 제거 2865 | 채택 | 파생형 negations의 같은 연결을 합의로 제거함. non persona로 들어온 연결이며 부정 연산은 엔터티 부재(not found) 응답이 아님 | `rts3-641a078ef551bfc0` |

## A. 오연결 미수정 항목

| 단어 | 조치 | 결정 | 이유 | 데이터셋 |
| --- | --- | --- | --- | --- |
| `execution` | own 제거 812 (대상 execution, 관계·input 재귀속 포함) | 채택 | 부모가 제거 취지를 지지한 오소속. 관계 executionorder·executionflow·executiontrace·trace를 execution의 persona 11311로 옮김. execution의 네 persona(5220 worker, 3908 runtime, 1036 invocation, 6060 인증 단계) 중 실행 단위 관계(run·executioncontext·executionresult·jobrun)를 가진 11311이 가장 가까움. 단 그 개념 1036의 주제가 Server route handlers(web-backend)라 이 관계들이 web-backend 뜻 아래 표시되는 한계가 있음. 후속389는 이 persona들을 자동 수신자로 지정하지 말라고 했고, 이번에는 비교 근거를 두고 고름. input executionflow·executionorder는 해제(389 권고와 같음), executiontrace는 logging 입력으로 812의 trace persona(35220)에 다시 연결(389 권고와 같은 방향) | `rts3-9e01eaf1eadf960a` |
| `disconnect` | own 제거 5000 (대상 disconnect, 관계·input 재귀속 포함) | 채택 | 실제 뜻은 소켓 연결 끊김. 관계 leave·socket.disconnect·reconnection·pingtimeout을 disconnect의 연결 수명주기 persona 9185(4872)로 옮기고 input pingtimeout·reconnection·socket.disconnect도 9185로 다시 연결 | `rts3-9e01eaf1eadf960a` |
| `onclose` | 개념 5637 이동 t141.2→t773 | 채택 | 대화상자 닫기 콜백이 auth-identity 단일 분야 주제에 있음. t773 Popup UI widgets(frontend-ui, 범위에 dialogs·modals 명시)로 이동 | `rts3-641a078ef551bfc0` |

## R1 이견의 의존 정리

양쪽 검토자 모두 오분류로 본 own에 관계가 걸려 있던 2건이다.

| 단어 | 조치 | 결정 | 이유 | 데이터셋 |
| --- | --- | --- | --- | --- |
| `invalidations` | own 제거 1779 (대상 invalidation, 관계·input 재귀속 포함) | 채택 | 양쪽 모두 오분류로 봄. 관계 8개(tag·version·dirtyflag·recompute 등)를 invalidation의 캐시 무효화 persona 16851(1594)로 옮긴 뒤 제거 | `rts3-9e01eaf1eadf960a` |
| `margins` | own 제거 2849 (대상 margin, 관계·input 재귀속 포함) | 채택 | 양쪽 모두 오분류로 봄. 관계 marginright·marginleft·mxauto·marginauto·auto를 margin의 CSS 여백 persona 19876(465)로 옮기고 input 4개도 19876으로 다시 연결한 뒤 제거 | `rts3-9e01eaf1eadf960a` |

## B. 뜻 미확정 연결

"기각"은 현행 연결 유지를 뜻한다.

| 단어 | 조치 | 결정 | 이유 | 데이터셋 |
| --- | --- | --- | --- | --- |
| `execution` | own 제거 3908 | 기각 | B1. 후속389는 실행 사건과 소요 시간을 구별해 제거 방향을 권고했다. 그러나 3908 범위가 measuring elapsed execution을 명시해 실행 시간 측정 문맥의 구성 요소로 성립하고, 관계 12개·input 4개를 옮길 맞는 수신자도 없어 유지 | 미적용 |
| `dsig` | own 제거 1265 | 기각 | B2. dsig는 digital signature 약칭(XMLDSig). 1265·2438에 digital signature가 함께 있어 유지 | 미적용 |
| `encodable` | own 제거 2253 | 채택 | B3. 2253은 주입 공격 방지(encode·escape). -able 형 encodable(인코딩 가능)에 출력 인코딩 방어 동작 뜻이 이어지지 않음 | `rts3-641a078ef551bfc0` |
| `encodable` | input 제거 1442 | 채택 | B3. Vega-Lite의 encoding은 시각 채널 매핑 명사. encodable에 그 뜻이 성립하지 않음(altair·vega·grammar of graphics 원천) | `rts3-641a078ef551bfc0` |
| `engines` | input 제거 2083 | 기각 | B4. JS 엔진은 코드를 실행하는 런타임 호스트라 2083(runtime·extension host)에 성립. 유지 | 미적용 |
| `ent` | 현행 유지 검토: own 8개(997·1334·2407·2954·3196·3503·3871·4164)·input 10개 | 기각 | B5. ent는 entity 약칭이자 공개 Go ORM ent. own 8개(JPA·ddd·record 등)가 entity 뜻으로 성립해 유지 | 미적용 |
| `exchange` | own 제거 3935 | 기각 | B6. OAuth authorization code·token exchange가 3935(OAuth grant·token) 범위에 성립. 유지 | 미적용 |
| `exchanged` | input 제거 1084 | 기각 | B7. 1084 범위가 exchanging columnar data를 명시. 유지 | 미적용 |
| `exchanged` | input 제거 3652 | 기각 | B7. RabbitMQ exchange로 주고받는 메시지 문맥. 유지 | 미적용 |
| `exporters` | own 제거 1167 | 채택 | B8. 기본형 export의 모듈 export 문 뜻을 행위자 명사 exporters(OpenTelemetry·CSV exporter 등)에 own으로 복사한 연결. 의존 0 | `rts3-641a078ef551bfc0` |
| `exporters` | own 제거 1308 | 채택 | B8. 기본형 export의 DLL export 뜻을 행위자 명사 exporters(OpenTelemetry·CSV exporter 등)에 own으로 복사한 연결. 의존 0 | `rts3-641a078ef551bfc0` |
| `exporters` | own 제거 3592 | 채택 | B8. 기본형 export의 공개 API visibility export 뜻을 행위자 명사 exporters(OpenTelemetry·CSV exporter 등)에 own으로 복사한 연결. 의존 0 | `rts3-641a078ef551bfc0` |
| `exporters` | input 제거 3591 | 채택 | B8. access modifier·mod의 모듈 로딩 문맥. exporters에 성립하지 않음 | `rts3-641a078ef551bfc0` |
| `exporters` | input 제거 2296 | 채택 | B8. public method·resi의 인터페이스 계약 문맥. exporters에 성립하지 않음 | `rts3-641a078ef551bfc0` |
| `exporters` | input 제거 1377 | 기각 | B8. DB dump·backup을 내보내는 exporter 문맥으로 성립. 유지 | 미적용 |
| `expression` | own 제거 555 | 기각 | B9. 식을 평가해 결과를 계산하는 뜻이 555(calculate·evaluate) 범위에 성립. 유지 | 미적용 |
| `favorite` | input 제거 3673 | 채택 | B10. 3673은 편집기 소스 위치·선택 범위(줄 bookmark). 즐겨찾기 favorite은 편집기 줄 위치가 아님 | `rts3-641a078ef551bfc0` |
| `file type` | own 제거 2953 | 기각 | B11. stat의 파일 종류(S_IFIFO)가 named pipe를 구별. 유지 | 미적용 |
| `filled` | input 제거 110 | 채택 | B12. 110은 DOM 준비·로드 이벤트. populated 동의어로 들어온 연결로 채워짐(filled)은 페이지 초기화가 아님 | `rts3-641a078ef551bfc0` |
| `finalized` | input 제거 5923 | 채택 | B13. 5923은 Eiffel contracts. Eiffel의 finalize는 컴파일 모드라 계약 개념과 무관 | `rts3-641a078ef551bfc0` |
| `finalized` | input 제거 2194 | 기각 | B13. finished 원천의 완료·구현됨 어휘에 finalized가 성립. 유지 | 미적용 |
| `fingerprint` | own 제거 2105 | 기각 | B14. canvas fingerprinting(브라우저 지문)이 2105 canvas 그리기와 실제로 함께 쓰임. 유지 | 미적용 |
| `fingerprinter` | own 제거 1716 | 채택 | B14. 1716은 GPG 키 서명. 키 fingerprint 명사 뜻을 행위자 명사 fingerprinter에 own으로 복사한 연결 | `rts3-641a078ef551bfc0` |
| `mixed` | input 제거 4412 | 채택 | B15. 4412는 GoF 설계 패턴. combined 동의어로 들어온 연결로 mixed(섞인)는 패턴 이름이 아님 | `rts3-641a078ef551bfc0` |
| `multinode` | 현행 유지 검토: own 1962·2246·3956·4890 등 구조 노드 연결 | 보류 | B16. 코드 용례는 vector의 MULTINODE(여러 Kubernetes 노드) 하나. 트리·DOM·vertex 구조 노드 뜻(ProseMirror 다중 노드 선택 등)의 성립 여부를 확정할 근거 부족 | 미적용 |
| `sections` | 현행 유지 검토: input 1932·4230·4988 | 기각 | B17. 사용자 결정(기존 상태만 보존, followup107)을 유지 | 미적용 |
| `updaters` | 현행 유지 검토: own 3427·input 354·2020·2260 | 기각 | B18. 사용자 결정(기존 상태만 보존, followup107)을 유지 | 미적용 |
| `drifted` | own 제거 1237 | 기각 | B19. 1237에 drift·detection이 있어 drift 감지 뜻이 성립. 유지 | 미적용 |
| `editable` | own 제거 625 | 기각 | B19. 625의 form control·edit 변경 처리에 편집 가능 상태가 성립. 유지 | 미적용 |
| `downloadable` | input 제거 4798 | 기각 | B20. 후속389 권고대로 보존. POEditor·Smartling의 번역 메모리 내려받기 문맥이 가능하고 반대 근거가 없음 | 미적용 |
| `lead` | lead의 901 persona 관계 leadtime 제거 | 채택 | B21. lead_time은 Kanban·흐름 지표의 소요 시간. CRM 잠재 고객(lead) persona의 관계·input으로 둘 근거가 없어 관계를 제거하고 leadtime의 이 input만 해제(leadtime은 kanban·workinprogress input 유지) | `rts3-9e01eaf1eadf960a` |
| `scene` | input 제거 1298 | 채택 | B22. 1298은 화면 치수·픽셀 밀도. screen persona로 들어온 연결로 장면(scene)은 화면 치수가 아님 | `rts3-641a078ef551bfc0` |
| `common` | 현행 유지 검토: own 3877·3973 | 기각 | B23. 모노레포 공통 패키지·CommonJS 문맥이 가능하고 제거 근거가 부족. 유지 | 미적용 |
| `multiple` | 현행 유지 검토: own 2838·1360·1804 | 기각 | B23. 다중 상속(mro)·중복(duplicate)·다중 반환값(tuple) 뜻이 성립해 소속 유지 | 미적용 |
| `common·multiple` | 현행 유지 검토: 남은 관계 혼합 payload 전수 확인 | 보류 | B23. 소속은 유지했지만 두 단어 persona에 남은 관계가 섞여 있는지 전수 확인은 하지 않음 | 미적용 |

## C. 추가하지 않은 후보

| 단어 | 조치 | 결정 | 이유 | 데이터셋 |
| --- | --- | --- | --- | --- |
| `exited` | exit에서 469 상속 | 채택 | C1. CFG에서 블록·영역을 빠져나간 경로(후속389 권고). 원천 cfg·control flow graph input | `rts3-641a078ef551bfc0` |
| `exited` | exit에서 1231 상속 | 채택 | C1. scope 이탈 뒤 소멸·해제 문맥(후속389 권고). 원천 del·destructor·dtor input | `rts3-641a078ef551bfc0` |
| `exited` | exit에서 2499 상속 | 채택 | C1. language server가 종료된 lifecycle(후속389 권고). 원천 LSP·lsp input | `rts3-641a078ef551bfc0` |
| `exited` | exit에서 1717 상속 | 기각 | C1. 스트림 close·drain 완료와 프로세스·제어 영역 종료는 다른 완료(후속389 권고) | 미적용 |
| `embedded` | input 추가 391 (원천 bidirectional) | 기각 | C2. 양방향 스트리밍에 embedded(내장됨) 뜻이 성립한다는 근거 없음 | 미적용 |
| `embedded` | input 추가 2070 (원천 wrap) | 기각 | C2. 고차 컴포넌트 wrap에 embedded 파생 뜻 근거 부족 | 미적용 |
| `missed` | miss에서 2865 상속 | 채택 | C3. 동사 miss의 과거분사. cache miss·조회 실패(not found·negative caching) 뜻이 missed에 그대로 성립(own) | `rts3-641a078ef551bfc0` |
| `download` | own 추가 3237 | 채택 | C4. 패키지 레지스트리에서 패키지를 내려받는 뜻. 파생형 downloadable은 이미 3237 own인데 기본형만 빠져 있음 | `rts3-641a078ef551bfc0` |
| `rax` | own 추가 3956 | 채택 | C5. redis/src/rax.c "Rax -- A radix tree implementation"(antirez/rax로 공개 배포된 라이브러리). 3956 tree 구조 개념 | `rts3-641a078ef551bfc0` |
| `scenes` | scene에서 1298 상속 | 기각 | C6. scene의 같은 연결(B22)을 제거하므로 복수형에 넓히지 않음 | 미적용 |
| `committed` | 현행 유지 검토: CommitInfo 관련어 추가 | 보류 | C7. CommitInfo 관련어의 정확한 외부 제공자를 확인하지 못함 | 미적용 |
| `favorited` | favorite에서 3673 상속 | 기각 | C8. favorite의 3673 연결(B10)을 제거하므로 넓히지 않음 | 미적용 |

## 처리 여부를 확인하지 못한 보류

| 단어 | 조치 | 결정 | 이유 | 데이터셋 |
| --- | --- | --- | --- | --- |
| `searchable` | input 제거 1706 | 채택 | location·where의 클라이언트 라우팅 위치 뜻. location.search 같은 고정 속성 이름이나 라우팅 객체가 searchable/searched(검색 가능/검색됨)로 굴절하지 않음 | `rts3-641a078ef551bfc0` |
| `searchable` | input 제거 5034 | 채택 | useLocation의 React Router 뜻. location.search 같은 고정 속성 이름이나 라우팅 객체가 searchable/searched(검색 가능/검색됨)로 굴절하지 않음 | `rts3-641a078ef551bfc0` |
| `searchable` | input 제거 3633 | 채택 | URLSearchParams의 URL 쿼리 문자열 뜻. location.search 같은 고정 속성 이름이나 라우팅 객체가 searchable/searched(검색 가능/검색됨)로 굴절하지 않음 | `rts3-641a078ef551bfc0` |
| `searched` | input 제거 1706 | 채택 | location·where의 클라이언트 라우팅 위치 뜻. location.search 같은 고정 속성 이름이나 라우팅 객체가 searchable/searched(검색 가능/검색됨)로 굴절하지 않음 | `rts3-641a078ef551bfc0` |
| `searched` | input 제거 5034 | 채택 | useLocation의 React Router 뜻. location.search 같은 고정 속성 이름이나 라우팅 객체가 searchable/searched(검색 가능/검색됨)로 굴절하지 않음 | `rts3-641a078ef551bfc0` |
| `searched` | input 제거 3633 | 채택 | URLSearchParams의 URL 쿼리 문자열 뜻. location.search 같은 고정 속성 이름이나 라우팅 객체가 searchable/searched(검색 가능/검색됨)로 굴절하지 않음 | `rts3-641a078ef551bfc0` |
| `alignments` | input 제거 3443 | 채택 | 3443은 소스 줄·커서 위치. position persona로 들어온 연결로 메모리·서열 정렬이 아님 | `rts3-641a078ef551bfc0` |
| `bookmarks` | 새 주제 "Bookmarks and favorites" | 채택 | Bookmarks 주제 t766.2의 분야가 language-constructs뿐임. 맞는 기존 주제가 없어 새 주제를 만들고 5342를 옮김 | `rts3-7b926399d21b0f85` |
| `bookmarks` | 개념 5342 이동 t766.2→new:bookmarks-favorites | 채택 | 위 새 주제로 이동 | `rts3-641a078ef551bfc0` |

## D·E·F 처리

| ID | 처리 | 근거 |
| --- | --- | --- |
| D1 | 결함 아님(의도된 표기 통합)으로 확인. 코드 미수정 | 조회 도구는 같은 `word_key`에 표기를 하나만 쓰고 표제어(`terms`) 표기를 우선한다(`sense_lookup.py`의 `surfaces`). `checkin`과 `check-in`, `TransactionManager`와 `transaction manager`는 같은 `word_key`다. 저장 값은 보존된다 |
| D2 | 제약으로 유지. 코드 미수정 | 고정된 거부 조건(fail-closed)이며 계획을 나눠 적용하면 된다. 이번 4c는 보내는 쪽과 받는 쪽 persona가 겹치지 않아 해당하지 않았다 |
| D3 | 해결(실행 확인) | 위 "확인한 것" 참고 |
| D4(새로 확인) | 기록만 함 | `input_bindings`는 넘겨받은 표기를 그대로 표제어로 저장한다. 시작 전 기본값에 이미 조회 표시가 바뀐 corrected 표제어 24개가 있다(예: `COCO`→`coco`, `look and feel`→`lookandfeel`). 표기만 고치는 조작이 없어 고치지 않았다 |
| E1~E3 | 해결(재확인) | 위 "확인한 것" 참고. 권한 파일 경로는 여전히 일부 적용 스크립트에만 있다 |
| E4 | 부분 보완 | 이번 소비 확인은 선택한 값 외에 대상 단어의 다른 개념 변화도 표시한다(4b의 2건). 대상 밖 단어의 전수 의미·순위는 보지 않았다 |
| E5·F | 처리하지 않음 | 회귀검증·평가셋4판·최종 전후 비교는 사용자가 범위에서 제외했다. F의 재측정은 최종 전후 비교에 해당한다 |

## 남은 보류

- `consoles` (R1 이견) — 새 개념 "account management console"(t896.1) 단어 consoles: 기본형 console이 같은 5869·72에 소속돼 있어 계정 관리 콘솔 개념 신설은 console과 함께 정해야 함. 한쪽 제안
- `datasets` (R1 이견) — 새 개념 "telemetry query dataset"(t694.1) 단어 datasets: 기본형 dataset도 같은 질의 대상 데이터 집합 뜻이라 복수형만 담는 개념 신설은 불일치. 기본형과 함께 정해야 함. 한쪽 제안
- `grantee` (R1 이견) — 새 개념 "access grant recipient"(t15.1) 단어 grantee: 102에 grantor·grantees가 함께 있어 권한 수령 주체 개념 신설은 같은 역할어들과 함께 정해야 함. 한쪽 제안
- `idxs` (R1 이견) — own 제거 3409: 기본형 idx도 같은 3409 own(nth·ordinal·day와 함께). 소속 자체가 의심스럽지만 기본형과 함께 정해야 함(한쪽 제거, 한쪽 불확실)
- `inlined` (R1 이견) — 새 개념 "inline web resource content"(t586) 단어 inline, inlined: t586은 resx 등 리소스 파일 조직 주제라 문서 안 자원 본문 내장 뜻과 맞지 않음. 뜻 자체는 가능하나 주제와 기존 6124 inline script·6295와의 관계 미정. 한쪽 제안
- `insights` (R1 이견) — 새 개념 "application performance analytics"(t694.1) 단어 insight, insights: 반대쪽은 같은 성능 분석 뜻을 SLI input으로 이미 표현된다고 봄. 기존 2281 observability와의 관계 미정. 한쪽 제안
- `kinesis streams` (R1 이견) — 새 개념 "Amazon Kinesis Data Streams"(t488) 단어 kinesisstreams: 같은 1392에 Kinesis·Amazon Kinesis가 남아 있어 Kinesis 제품 개념 신설은 이 단어들과 함께 정해야 함. 한쪽 제안
- `messaging` (R1 이견) — 새 개념 "team messaging"(t56.12) 단어 messaging: 한쪽 제안이며 대상 주제 t56.12의 분야가 databases-storage로 잘못돼 있어 분야 검토가 먼저 필요(제안자도 적용 전 분야 검토 필요로 표시)
- `packages` (R1 이견) — 새 개념 "Windows application package"(t271) 단어 packages: 한쪽 제안. 같은 주제 t271의 573(appx)과 기본형 package의 뜻을 함께 정해야 하며 복수형만 담는 개념은 불일치
- `prelogin` (R1 이견) — 새 개념 "SQL Server connection prelogin"(t25.4) 단어 prelogin: MS-TDS PRELOGIN 단계는 공개 프로토콜이지만 근거가 keycloak 테스트의 오류 문자열뿐. 한쪽 제안
- `previews` (R1 이견) — 새 개념 "invoice preview"(t170.3) 단어 previews: 한쪽 제안. 복수형만 담는 개념이라 기본형 preview와 함께 정해야 함(Stripe invoice preview)
- `processors` (R1 이견) — 새 개념 "Software input processing components"(software_input_processing_components) 단어 processors; 새 주제 "Software input processing components"; own 추가 새 개념; 새 개념 "rule evaluation processors"(new:configured-rule-evaluation) 단어 processors; 새 주제 "Configured rule evaluation": 양쪽 모두 새 개념·새 주제를 제안하나 범위가 다름(입력 처리 구성요소 일반 vs 규칙 평가). 맞는 기존 주제가 없어 새 주제 범위를 정해야 함
- `rates` (R1 이견) — 새 개념 "shipping rate"(t261.3) 단어 rates: 한쪽 제안. 복수형만 담는 개념이라 기본형 rate와 함께 정해야 함(WC_Shipping_Rate)
- `services` (R1 이견) — 새 개념 "Business service offerings"(t261.1) 단어 services: 한쪽 제안이고 근거가 marketplace UI 분류 문자열 하나. 복수형만 담는 개념이라 기본형 service와 함께 정해야 함
- `short date string` (R1 이견) — datestr에서 1746 상속: 형태상 굴절 관계가 아니고, 원천 str persona의 dateStr 관계가 format specifier·보간 뜻인지 일반 문자열 이름인지 확정 근거 부족(한쪽 상속, 한쪽 불확실)
- `sizer` (R1 이견) — 새 개념 "batch item size estimator"(t85.4) 단어 sizer: 한쪽 제안. 근거가 vector 내부 StatsdBatchSizer 하나라 일반 sizer(크기 계산기) 뜻과 별도 개념이 필요한지 미정
- `sizes` (R1 이견) — 새 개념 "image size variants"(t223) 단어 sizes: 한쪽 제안. 복수형만 담는 개념이라 기본형 size와 함께 정해야 함(WordPress 등록 이미지 크기)
- `sourced` (R1 이견) — own 제거 809: 셸 source 명령으로 실행된(sourced) 스크립트 뜻이 해석기 주제에 성립할 수 있음(반대쪽 근거: Tcl source). 소스 코드 명사 연결과 구별할 근거 부족
- `specs` (R1 이견) — own 제거 1079: 1079(로그 수집 파이프라인) 소속 근거가 불명확하나 반대쪽은 잠정 유지. 대체 개념 기각으로 제거 시 근거 없이 뜻이 사라짐
- `subkeys` (R1 이견) — 새 개념 "Redis hash subkeys"(t665) 단어 subkeys: 한쪽 제안. 복수형만 담는 Redis 전용 개념이라 기본형 subkey·hash field 표현과 함께 정해야 함
- `unsubscribed` (R1 이견) — 새 개념 "issue notification subscription state"(x12) 단어 unsubscribed: 한쪽 제안. 알림 구독 상태 개념은 subscribe·subscribed 등과 함께 정해야 하며 근거가 sentry 하나
- `valued` (R1 이견) — 새 개념 "attribute value multiplicity"(t135.1) 단어 valued: 한쪽 제안. single-/multi-valued 속성의 값 개수 뜻이 valued 단독 조각에 성립하는지 미정
- `viewers` (R1 이견) — 새 개념 "content viewer"(t824) 단어 viewers: 한쪽 제안. 복수형만 담는 개념이라 기본형 viewer와 함께 정해야 함
- `rxs` (R2) — 새 개념 "channel receiver"(t297.1) 단어 rxs: 수신 endpoint 개념은 rx·receiver와 함께 정해야 함. 한쪽 제안
- `savings` (R2) — 새 개념 "execution cost savings"(t401.4) 단어 savings; 새 개념 "memory size savings"(t234.4) 단어 savings; 새 개념 "monetary cost savings"(t111.1) 단어 savings: 한쪽 제안의 신규 개념. 기존 6681(앱 크기 절감) 외 뜻별 분할 필요성 미확정
- `multinode` (B) — 현행 유지 검토: own 1962·2246·3956·4890 등 구조 노드 연결: B16. 코드 용례는 vector의 MULTINODE(여러 Kubernetes 노드) 하나. 트리·DOM·vertex 구조 노드 뜻(ProseMirror 다중 노드 선택 등)의 성립 여부를 확정할 근거 부족
- `common·multiple` (B) — 현행 유지 검토: 남은 관계 혼합 payload 전수 확인: B23. 소속은 유지했지만 두 단어 persona에 남은 관계가 섞여 있는지 전수 확인은 하지 않음
- `committed` (C) — 현행 유지 검토: CommitInfo 관련어 추가: C7. CommitInfo 관련어의 정확한 외부 제공자를 확인하지 못함

