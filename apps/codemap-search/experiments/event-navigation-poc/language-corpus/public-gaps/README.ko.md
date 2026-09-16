# 공개 저장소 미탐지 경로 1차 보완 기록

이 문서는 **140사례 단계의 기록**이다. 아래의 “현재”와 “남은 작업”은 해당 단계 기준이다. 최신 165사례 검증과 확장 입력 결과는 [잔여 경로 보완 결과](../remaining-routes/README.ko.md)에 있다. 이 단계의 구현·결과·문서 원본은 [보존본](../remaining-routes/baseline/manifest.json)에 고정했다.

2026-09-16. 체크포인트 `01aac1515` 이후 공개 미확정 양성 12개 중 **7개를 조건부 호출 후보로 복구했다.** 미확정은 Rust 4개와 Scala 1개가 남았고, TypeScript 자료 전달 2개는 계속 인자 전달까지만 확인된다. 새 회귀 사례 29개를 포함한 140개와 기존 공통 사례 90개가 모두 통과했다. 실제 프로그램 실행과 프레임워크 전체 지원은 검증하지 않았다.

## 비교 기준

기준은 `checkpoint/event-navigation-poc-18-languages-20260916`이 가리키는 커밋 `01aac1515864f7066c368e0081d6fd23836a5474`다. 기존 공개 저장소 18개·소스 96파일·공개 사례 77개·입력 잠금 버전 3을 그대로 사용했다. 기존 회귀 111개와 앞서 제시한 원문 예시 36개도 변경하지 않았다. 제품 Rust 코드와 MCP/CLI 계약은 변경하지 않았다.

| 구분 | 체크포인트 | 현재 |
| --- | ---: | ---: |
| 공개 양성의 조건부 호출 후보 | 23 / 37 | 30 / 37 |
| 공개 양성의 인자 전달만 확인 | 2 / 37 | 2 / 37 |
| 공개 양성의 미확정 | 12 / 37 | 5 / 37 |
| 양성 대조를 갖춘 공개 무연결 쌍 | 12 / 14 | 14 / 14 |
| 공개 무연결 쌍의 판정 유보 | 2 / 14 | 0 / 14 |
| 공통 사례 | 90 / 90 | 90 / 90 |
| 회귀 사례 | 111 / 111 | 140 / 140 |
| 실행 경계 시나리오 | 26개 미실행 | 26개 미실행 |

[기준 사실](baseline.json), [1차 비교](../remaining-routes/baseline/language-corpus/public-gaps/comparison.json), [1차 전체 평가](../remaining-routes/baseline/language-corpus/results/evaluation.json), [1차 회귀 평가](../remaining-routes/baseline/language-corpus/regressions/results/evaluation.json)에서 위치·조건·관계 수를 확인할 수 있다. 공개 양성 37개는 선별한 참고 쌍이며, 저장소 전체 재현율의 분모가 아니다. 사례 하나에서 여러 후보가 나와도 한 번만 센다.

## 개선된 경로와 범위

| 대상 | 소스에서 복구한 연결 | 아직 증명하지 않은 부분 |
| --- | --- | --- |
| C++ / EnTT | 범위별 타입 별칭, 생성자 본문 밖 필드 초기화, 포인터 반환, 저장 대상의 접두 별칭을 따라 `sink → sigh.calls → delegate 호출` 후보 생성 | 실제 template 인자에 해당하는 callback 값, 모든 overload·해제 순서·객체 수명 |
| C# / Reactive | 명시된 표준 namespace의 `Volatile.Read`, 조건부 `CompareExchange` 저장, 배열 원소의 wrapper 필드와 expression-bodied getter 연결 | CAS 성공·구독 활성 상태·동시성·이전 배열 snapshot 전체. CAS 반환값은 이전 값으로 남김 |
| Groovy / Grails | import 뒤 개행 처리 수정으로 Closure wrapper의 생성자와 별도 trigger 본문 연결 | 모든 Groovy 동적 dispatch와 해당 파일의 구문 복구 구간 |
| Java / EventBus | `Subscription.subscriberMethod.method`의 필드 흐름과 타입이 확인된 `Method.invoke`의 Method·수신 객체·인자 역할 보존 | 구체 Method 탐색, 수신 객체 호환성, thread mode 및 실행 순서 |
| PHP / EventDispatcher | 실제 foreach AST, 빈 첨자 append, 참조 대입, first-class callable 생성, cache 순회를 따라 직접 callable 경로 연결 | lazy factory의 모든 반환 객체와 분기, 제거·priority 변경 이후의 실행 상태 |
| Swift / RxSwift | 조건부 컴파일 지시문에서 끊기던 파싱 복구, typealias·제네릭 생성자·반환값·호출 인자 저장 투영으로 Bag의 `_value0` 경로 연결 | `_pairs`·dictionary의 모든 원소 경로, 모듈 구성·빌드 분기 선택·값 복사 snapshot |
| TypeScript / Vendure | `.service.ts` 상대 import, 명시·초기화 추론 필드 타입, `getHooks` 반환값, object spread의 method key와 `pre/post` 소비 경로 연결 | DI binding의 실제 객체 동일성, class target 변환 결과와 조회 key의 동일성, 속성 존재·덮어쓰기·호출 순서 |

Groovy의 reply와 subscriber를 섞는 반례, Java의 `methodString`을 reflection Method로 취급하는 반례는 각각 양성 대조가 복구되어 판정 유보에서 검증된 무연결로 바뀌었다.

Rust는 공개 양성의 최종 판정은 바뀌지 않았지만, `core/std::ops::Deref/DerefMut`의 명시된 import와 실제 구현의 반환 필드를 따라 내부 표준 컨테이너로 접근하는 기능을 추가했다. Bevy의 `messages.rs:143`은 이제 `Messages.messages_b.messages[*]`의 저장 사실로 나온다. source body가 없거나 같은 이름의 자체 메서드가 있으면 자동 역참조 규칙으로 대신하지 않는다. 제네릭 Bag 저장·조회, 다른 Bag, 고유 `push` 메서드를 구분한 회귀 4개가 통과했다.

표준 연산의 의미는 공식 자료를 대조했다. C#의 읽기와 CAS 반환·교체 의미는 [Volatile.Read](https://learn.microsoft.com/en-us/dotnet/api/system.threading.volatile.read?view=net-9.0)와 [Interlocked.CompareExchange](https://learn.microsoft.com/en-us/dotnet/api/system.threading.interlocked.compareexchange?view=net-9.0), reflection의 역할은 [Java Method.invoke](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/reflect/Method.html), Rust의 공유·가변 역참조 구분은 [Deref](https://doc.rust-lang.org/std/ops/trait.Deref.html)와 [DerefMut](https://doc.rust-lang.org/std/ops/trait.DerefMut.html)에 근거한다. 이 계약 확인은 대상 저장소를 실행한 검증과 별개다.

## 오연결을 막기 위한 변경

저장·호출 연결의 소유자, 정적 키, 튜플 위치, 경로 깊이 조건은 유지했다. 생성자와 명시적 저장 별칭을 따라갈 때는 조건과 원문 위치를 함께 남긴다. 저장 대상의 접두 별칭을 투영해도 좌변 전체를 이전 값으로 바꾸지 않아 참조 재대입을 이전 객체의 필드 쓰기로 오해하지 않는다.

TypeScript object spread는 실제 소스의 복사 구문에서만 파생한다. 호출에서 요구한 속성 경로를 투영한 사실에는 구조적 `consumer`를 넣어 **그 위치·대상·호출 종류·인자 위치에만** 소비를 허용한다. 동적 키 호출을 위해 만든 저장 후보를 다른 `missing` 또는 `blocked` 호출에 다시 쓰지 않는다. 알려진 literal의 명시된 필드 쓰기를 포함해 키를 확인하며, 중첩 속성 부재와 명시적 덮어쓰기·뒤쪽 spread의 확인된 덮어쓰기를 차단한다. computed key, 외부 mutation·escape·prototype·Proxy 전체는 지원하지 않는다.

외부 DI 반환값에는 실제 allocation을 발명하지 않는다. 명시된 opaque 필드 대입 한 건을 근거로 원본 receiver-schema 저장만 해당 **정확한 필드 경로로 한 번** 투영한다. 원래 binding의 조건·위치를 보존하고, 다른 필드에서 파생한 저장이나 알려진 allocation을 타입만으로 재투영하지 않는다. 결과에는 `opaque_receiver_binding_required`, `declared_receiver_schema_only`, `method_dispatch_unproven`이 남는다.

공통 `Fact`에 선택적 소비 제한 정보가 추가됐지만 결과의 확실성 계약은 유지했다. 모든 관계는 `conditional_source_relation`, `concrete_instance_proven=false`, `event_classification=not_inferred`다. 문자열 조건은 참으로 증명된 사실이 아니라 연결이 성립하려면 필요한 의무다.

## 실패 분석과 재검증

일반 파일 분석을 요청한 Sol·medium 하위 에이전트 4개가 모두 모델 용량 부족으로 실패했다. 사용자 지시에 따라 Astra·max 상세 분석을 **총 2회** 사용했다. 첫 번째는 기존 14개 경로의 단절과 안전한 변경 경계를 분석했고, 두 번째는 실제 새 회귀 실패와 투영 규칙을 상세 분석했다. 구현·사례 추가·측정·문서 작성은 메인 스레드에서 수행했다. codemap-search는 사용하지 않았다.

| 기록 | 관측 결과와 후속 조치 |
| --- | --- |
| [회귀 round4](regression-round4/evaluation.json) | 127개 중 124개 통과. TS 동적 키 후보 재사용 오연결, Swift 제네릭 생성자 누락과 그 양성 대조 의존 반례가 실패 |
| [회귀 round5](regression-round5/evaluation.json) | TS·Swift 19개 통과. 소비 지점 제한·literal 키 대조와 `constructor_expression` 처리로 수정 |
| [공개 round5](round5/evaluation.json) | dotted basename import와 필드 경로를 보존한 타입 스키마 투영 후 Vendure hook 후보 확인 |
| [회귀 round6](regression-round6/evaluation.json) | 29개 중 27개 통과. import된 클래스의 초기화 추론 필드 타입이 외부 접근에 전달되지 않는 추가 누락 확인 |
| [1차 최종 회귀](../remaining-routes/baseline/language-corpus/regressions/results/evaluation.json) | 타입 추론 전달 수정 후 140개 모두 통과. 양성 81, 구조적 반례 55, 미확정 유지 4 |

중간 디렉터리는 당시 실패와 관측을 보존한 자료다. 각 원시 결과의 코드 해시는 해당 측정 당시 구현을 가리키며 현재 코드 해시와 다를 수 있다. 이 단계의 최종 검증 결과는 위 보존본의 `results`와 `regressions/results`다. 상위의 같은 경로에는 후속 단계 결과가 저장된다.

## 남은 작업

| 경로 | 현재 단절 | 필요한 추가 작업 |
| --- | --- | --- |
| Rust `b_boxed_observer` | `Self`·associated 생성과 wrapper에서 trait object를 거쳐 World/Entity에서 회수한 system까지 값·소유자 추적이 끊김 | Rust 모듈·re-export·제네릭 owner, `Some/Box`와 실제 변환 본문, component lookup의 Entity 식별 근거를 연결. custom runner와 분리 |
| Rust `b_message_buffer` | 버퍼 저장은 복구됐지만 slice·iterator chain의 메시지 참조 반환은 최종 관계로 표현하지 못함 | 자료 읽기·반환 관계를 callback과 다른 종류로 모델링하고 평가기를 함께 확장. cursor·buffer rotation·World 경계 보존 |
| Rust `b_system_id` | associated 생성자·component 저장/회수·`take()` 경로가 미완성 | 등록된 system의 Entity ID를 유지하고 조회·제거·재삽입의 실제 소스 본문을 연결 |
| Rust `b_gltf_handler` | resource/wrapper 해석과 함수 사실 상한에서 최종 호출 사실이 끊김 | 소유자 해석과 관련 사실 선택 개선. 공유 Arc clone과 load별 Vec/Box clone의 별도 객체 provenance 보존 |
| Scala `scala-state-subscriber` | case-class State·copy·cache 배열과 외부 Monix Atomic 구현 경계 | case-class·매개변수 없는 메서드·Array 적용 구문 보완 후 JVM/JS 및 Scala 버전을 명시해 Atomic 입력을 고정하고 실제 본문 연결 |
| TypeScript `ls_mesh_queue` | Map 조회 queue가 `Queue.offer` arg0에 도달 | re-export 및 잠긴 Effect 버전의 실제 Queue 구현을 추가해 enqueue·소비 경로 확인. 현재 저장값은 일부 opaque |
| TypeScript `ls_rpc_client` | Map 조회 clientId가 `writeResponse` arg0에 도달 | Protocol이 제공하는 실제 함수 구현과 transport 경계를 연결. clientId를 callable로 취급하지 않음 |

따라서 미확정 5개와 인자 전달 2개는 완료로 집계하지 않는다. 남은 입력 확장에는 고정 커밋의 원문·의존성 버전과 입력 잠금 이력을 함께 추가해야 한다. 라이브러리 메서드 이름만으로 외부 동작을 가정하면 해결한 것으로 볼 수 없다.

그 밖에 모든 후보의 정밀도 검수, 보완하면서 보지 않은 별도 저장소 평가, 이름 변경·중첩 조합, 런타임 인스턴스·수명·동시성·비동기 전달 검증, Rust 제품 통합이 남는다. 공개 26개 실행 시나리오를 실행한 적은 없으며 이번 작업에서도 대상 저장소의 빌드·테스트·서버를 실행하지 않았다.

## 재현과 무결성 확인

작업 디렉터리는 상위 `language-corpus`다. 기존 환경 준비는 [전체 보고서](../README.ko.md)의 재현 절차를 따른다.

```sh
.venv/bin/python regressions/run.py
.venv/bin/python manage.py measure --sources /tmp/codemap-language-repro-sources
.venv/bin/python public-gaps/verify.py --sources /tmp/codemap-language-repro-sources
```

140사례 단계의 실측은 `/tmp/codemap-event-poc-venv/bin/python`의 Python 3.14.5·macOS arm64에서 수행했다. 당시 검증기는 원시 결과 108개, 구현·소스·커밋 해시, 입력 잠금 유지, 기존 111사례·원문 36개·보완 전 보존본 74파일 유지, 소비 제한 계약, 문서의 로컬 링크를 대조했다. 당시 실행 결과는 [verification.json](../remaining-routes/baseline/language-corpus/regressions/results/verification.json)에 있다. 다른 실행 환경과 처리 성능 향상은 확인하지 않았다.
