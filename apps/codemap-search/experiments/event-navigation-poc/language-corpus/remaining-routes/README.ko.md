# Rust·Scala·TypeScript 잔여 경로 보완 결과

검증일: 2026-09-16. **Monix의 JVM·JavaScript × Scala 2·3 네 구현에서 새 구독자 저장 → 최종 호출 후보를 연결했다.** Bevy 메시지의 자료 반환도 유지했고, LiveStore Queue는 추가 소비 지점인 Effect 내부 배열 쓰기와 한 bucket의 배열 반환까지 조건부로 연결했다. RPC의 clientId → 실제 clientWrites 조회 키 전달도 조건부 연결했다. 회귀 **247/247**, 공통 사례 **90/90**, 기존 공개 무연결 쌍 **14/14**가 통과했다.

**모든 잔여 경로를 해결한 것은 아니다.** Bevy observer·SystemId·glTF 세 경로가 남는다. Queue의 여러 bucket 순회와 실제 payload 전달은 검증하지 않았다. 아래 결과는 대상 프로그램 실행이 아닌 조건부 정적 분석이다.

201사례 검증 상태는 `33efda1f7`에 먼저 커밋했다. 이후 Rust 포인터·타입 키와 TypeScript generator 재개 처리 및 33개 대조를 추가했다. 전체 잔여 경로 해결과 기본 의미 처리의 회귀 통과를 구분한다.

## 범위와 비교 기준

질문은 “추가 소스와 범용 값 추적을 통해 저장값을 실제 소비 본문까지 이어갈 수 있는가”다. 독립 PoC만 수정했다. 파일 분석은 Sol·medium 하위 에이전트, 구현·실행·종합은 메인 스레드가 맡았다. codemap-search 도구·CLI는 사용하지 않았다. 앞 단계의 Astra·max 실패 분석 2회 이후 추가 Astra 분석은 요청하지 않았다.

Git 체크포인트 `01aac1515864f7066c368e0081d6fd23836a5474`와 기본 공개 입력 96파일·77사례·입력 잠금 버전 3을 보존했다. [140사례 단계](baseline/manifest.json)의 136파일과 [직전 165사례 단계](history/165-cases/manifest.json)의 165파일을 해시로 대조한다. 기존 165사례의 기대값과 최초 예시 36개도 유지했다. Queue 타입 보완 전 [196사례 검증본](history/196-cases/manifest.json)의 183파일도 추가 보존했다.

| 확장 입력 | 파일 수 | 바이트 | 파일당 한도 | 선정 범위 |
| --- | ---: | ---: | ---: | --- |
| `bevy-modules` | 62 | 1,711,338 | 512 KiB | ECS·App·World·Bundle·저장소·포인터·derive 소스와 crate 재노출 |
| `monix-jvm-scala2` | 15 | 58,048 | 512 KiB | Scala 2 생성 매크로·JVM Atomic·Java 필드 |
| `monix-jvm-scala3` | 13 | 47,076 | 512 KiB | Scala 3 inline 생성·JVM Atomic·Java 필드 |
| `monix-js-scala2` | 8 | 43,047 | 512 KiB | Scala 2 생성 매크로·JavaScript Atomic |
| `monix-js-scala3` | 6 | 32,813 | 512 KiB | Scala 3 inline 생성·JavaScript Atomic |
| `livestore-effect` | 23 | 2,420,495 | **1 MiB** | LiveStore·Effect 내부 소비·생성자·iterator·Effectable 구현 |

파일 수는 실행별 수로 서로 중복된다. Monix 네 조합은 동일 공개 사례의 입력 변형이며 공개 양성 분모를 늘리지 않는다. 입력은 [inputs.json](inputs.json)의 버전 7에 고정했고, 이전 목록은 [버전 1](history/inputs-v1.json)부터 [버전 6](history/inputs-v6.json)까지 보존했다. Bevy crate 이름과 소스 연결은 실제 Cargo manifest 8개의 해시를 기록한 [crate-sources.lock.json](crate-sources.lock.json)으로 검증한다. [이전 5개 crate 목록](history/crate-sources-v1.lock.json)도 보존했다. 이 표는 소스 AST 분석용 입력이다. 별도의 분리 worktree에서 Bevy ECS·glTF·포인터 라이브러리 컴파일과 MIR 수집을 진행했으나, 그 출력을 아래 연결 수치에 반영하지 않았다.

Effect는 LiveStore 잠금 파일의 **4.0.0-rc.113**을 사용했다. [공식 배포 메타데이터](https://registry.npmjs.org/effect/4.0.0-rc.113), archive의 SHA-512·SHA-256, 소스·라이선스·패키지 파일 474개의 SHA-256은 [effect-source.lock.json](effect-source.lock.json)에 있다. 설치·빌드 스크립트는 실행하지 않았다. `Stream.ts` **615,757바이트**를 포함하도록 사용자 선택에 따라 이 확장 실행에만 `--max-file-bytes 1048576`을 적용했다. 기본 입력 한도는 524,288바이트다.

## 실제 자동 분석 결과

| 원래 공개 양성 37쌍의 판정 | 직전 기본 입력 | 현재 기본 입력 | 현재 확장 입력까지 반영 |
| --- | ---: | ---: | ---: |
| 조건부 호출 후보 | 30 | 30 | **31** |
| 조건부 데이터 반환 | 0 | 0 | **1** |
| 인자 전달만 확인 | 2 | 2 | 2 |
| 미확정 | 5 | 5 | **3** |
| 합계 | 37 | 37 | 37 |

기본 입력의 판정은 직전과 같다. 확장 열은 선정 소스를 더한 실행의 가장 진전된 판정을 사례별로 집계한 값이므로 동일 입력에서 코드만 바꾼 개선율이 아니다. [comparison.json](comparison.json)에 집계 기준과 사례별 결과가 있다.

### Monix 네 구현

`PublishSubject.scala:76`에서 추가된 **`subscriber` 매개변수 값 자체**가 `:124`의 `onNext` 호출까지 연결된다. 기존 원소를 복사한 사실만으로 통과시키지 않도록, 같은 위치의 서로 다른 저장값을 결과 중복 제거에서 구분했다. 검증기는 네 출력에서 새 구독자 값과 아래 조건이 최종 관계에 남는지도 확인한다.

| 구현 | 확인한 소스 경로 | 남겨 둔 주요 조건 |
| --- | --- | --- |
| JVM·Scala 2 | companion builder → 반환되는 reify 본문 → AtomicAny → Java 상속·VarHandle 필드 → CAS·State | implicit 선택, 매크로 본문 보존·컴파일러 typing 미확정, CAS 성공 |
| JVM·Scala 3 | inline 생성 본문 → builder·익명 구현 → 같은 Java 필드 경로 | implicit 선택, CAS 성공, cast 호환성 |
| JavaScript·Scala 2 | 반환되는 reify 본문 → builder → AtomicAny.ref 읽기·교체 | implicit 선택, 매크로 본문 보존·컴파일러 typing 미확정, 분기 선택 |
| JavaScript·Scala 3 | inline 생성 본문 → builder → AtomicAny.ref 읽기·교체 | implicit 선택, 분기 선택 |

이는 컴파일러의 전체 implicit 탐색·매크로 확장·타입 검사를 구현했다는 뜻이 아니다. 입력에서 유일하게 선택되는 companion의 매개변수 없는 provider, 제한된 타입 조건, 실제 익명 클래스·상속 본문을 처리한다. Scala 2는 매크로를 실행하지 않고 반환 트리에 연결된 `reify` 본문을 읽는다. 컴파일러가 그 본문을 보존하고 typing에 성공한다는 조건을 숨기지 않는다. [JVM/2](results/monix-jvm-scala2.evaluation.json), [JVM/3](results/monix-jvm-scala3.evaluation.json), [JavaScript/2](results/monix-js-scala2.evaluation.json), [JavaScript/3](results/monix-js-scala3.evaluation.json).

JVM 처리는 `java.lang.invoke.MethodHandles`와 `VarHandle`, 실제 선언 필드와 요청한 필드 타입을 확인한다. final 필드 쓰기, 다른 필드 타입, 다른 receiver의 저장을 양성으로 만들지 않는 반례를 포함했다. 클래스 초기화·접근 권한·메모리 모델·실제 동시성은 실행 검증하지 않았다. 표준 계약은 [VarHandle](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/invoke/VarHandle.html), [Lookup.findVarHandle](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/invoke/MethodHandles.Lookup.html#findVarHandle(java.lang.Class,java.lang.String,java.lang.Class))을 대조했다.

### Bevy 메시지와 저장소 앞단

`messages.rs:140`의 payload 저장과 `iterators.rs:97`의 반환은 같은 `Messages.messages_b.messages[*].message` 경로로 연결된다. `message_id`는 별도 필드다. 명시적 모듈·import·재노출, `Self`, source-backed Deref, slice·iterator·Option을 따라간 결과이며 `returned_value_only_not_callback_execution`을 유지한다. [저장 소스](https://github.com/bevyengine/bevy/blob/29fe519f32a14503c8c0fab0baacafe76842bb76/crates/bevy_ecs/src/message/messages.rs#L138), [반환 소스](https://github.com/bevyengine/bevy/blob/29fe519f32a14503c8c0fab0baacafe76842bb76/crates/bevy_ecs/src/message/iterators.rs#L87), [평가](results/bevy-modules.evaluation.json).

201사례 단계에서는 `Some` 패턴의 constructor 이름을 지역 변수로 오인하던 문제, 표준 import가 확인된 `Box::new`의 별도 allocation과 pointee 저장, `&mut Vec<T>`의 컨테이너 타입 처리를 보완했다. 다른 Box, generic 이름 가림, 사용자 `Some`을 대조했다. 표준 prelude의 모든 Box 사용이나 임의의 wrapper·trait를 해석한 것은 아니다. [Iterator](https://doc.rust-lang.org/std/iter/trait.Iterator.html), [Option](https://doc.rust-lang.org/std/option/enum.Option.html), [slice](https://doc.rust-lang.org/std/primitive.slice.html) 계약과 구체 구현 범위를 구분한다.

이후 [rust_values.py](../../rust_values.py)에 표준 경로가 확인된 `NonNull::from_ref/from_mut`, 참조에서 유래한 raw pointer, `as_ref/as_mut/as_ptr`, cast·clone의 기원 보존을 추가했다. 숫자 주소는 pointee를 만들지 않는다. tuple struct의 위치 필드와 `unsafe` 블록 반환값도 보존한다. `TypeId::of`는 제네릭 인자까지 구체 타입을 확인할 때만 정적 키를 만들며, 미해석 타입 인자는 `opaque_key`로 남겨 임의의 구체 타입과 연결하지 않는다. 기본 타입 이름을 가린 사용자 타입도 구분한다. pointer 수명·정렬·aliasing과 cast의 layout 호환성은 조건이며 검증 완료가 아니다. [NonNull 계약](https://doc.rust-lang.org/std/ptr/struct.NonNull.html), [TypeId 계약](https://doc.rust-lang.org/std/any/struct.TypeId.html).

Rust Map 조회는 `Option`으로 감싸고 `unwrap`·`?`에서 payload를 꺼내도록 했다. 새 호출 양성은 중간 `unwrap` 메서드를 최종 호출로 세지 않도록 정확한 `storage_to_invocation` 관계를 요구한다. 이 기본 처리만으로 Bevy의 generic component 슬롯이나 trait 구현 선택까지 해결된 것은 아니다.

`loader/mod.rs:1754`의 glTF 최종 호출 사실은 추출하지만 registry 등록과 연결되지 않는다. `World`·Entity·component 조회, trait 변환, Arc 공유와 load별 Box 복제의 의미가 남아 있다.

### Effect 추가 소비 지점

원래 `ls_mesh_queue`와 `ls_rpc_client`는 각각 `Queue.offer`와 `writeResponse`의 인자 위치가 끝점이다. 이 위치의 판정을 바꾸거나 소비를 콜백 실행으로 세지 않고, [consumers.json](consumers.json)에 별도 소비 지점을 고정했다. 추가 6쌍은 위 공개 양성 37쌍에 합산하지 않는다.

| 추가 대조 | 자동 결과 | 해석 |
| --- | --- | --- |
| `node.ts:609` Queue 저장 → `MutableList.ts:197` | **조건부 객체 쓰기** | 저장한 Queue의 내부 tail 배열 쓰기. payload 반환·최종 전달 성공은 아님 |
| 같은 Queue 저장 → `MutableList.ts:432` 반환 | **조건부 객체 읽기** | `takeN`의 단일 bucket 배열 직접 반환. 모든 bucket이나 최종 Stream 전달을 뜻하지 않음 |
| `requestClientMap` 저장 → `Utils.ts:90` | **조건부 조회 키 소비** | 생성 시 바인딩한 callback의 명시적 호출에서 clientId 인자를 실제 조회 본문으로 전달. generator·반환 메서드 실행은 조건 |
| `Utils.ts:101` handler 저장 → `:92` 호출 | **조건부 호출 후보** | Effect 내부 구간의 양성 대조. LiveStore clientId 전달 전체의 증거는 아님 |
| Queue → RPC 조회 / clientId → Queue 쓰기 | **2쌍 무연결** | Queue 쓰기 양성 대조와 함께 확인 |

[추가 소비 평가](results/consumers.evaluation.json)는 원시 분석의 해시·선정 소스 해시와 관계 종류를 확인한다. Queue 쓰기에는 불투명 initializer, 선언된 인자 타입, 지연 메서드 실행과 동적 키 일치 등의 조건이 남는다. `stored_object_write`·`stored_object_read`·`stored_key_lookup`은 콜백 관계와 분리한다.

RPC는 `body.apply(this, arguments)`가 생성한 generator의 callback 인자를 보존하고, `writeResponse(clientId, response)`의 명시적 호출에 실제 callback 본문을 바인딩해 연결했다. 함수 생성과 실행을 구분하며 `source_callable_parameter_binding`, `generator_execution_required`, `enclosing_function_schema_only`, `enclosing_callable_execution_unproven`을 검사한다. factory의 실제 할당을 다른 호출에 투영하지 않고, callback 인자 위치를 따로 보존한다. 사용자 정의 iterator·fiber 전체를 실행한 증거는 아니다.

Queue 반환의 추가 개선은 `MutableList.Bucket` namespace 소유 범위를 보존한 결과다. `head/next`의 선언된 타입을 따라 `array`가 실제 배열임을 확인하며, nullish 멤버만 제외한 유일 타입일 때 해석한다. 서로 다른 namespace의 같은 이름 타입이나 복수 객체 union을 합치지 않는다. 현재 반환 근거는 `MutableList.ts:429` 조건을 만족하는 `:432` 분기다. 동적 index 복사·덮어쓰기와 임의 길이 while 순회를 모두 모델링한 것은 아니다.

TypeScript에서는 반환 closure의 캡처, `call/apply/arguments`·rest 인자, 상수 조건식, prototype 생성자·수신 객체·호출 문맥을 보존했다. 큰 모듈의 앞선 초기화가 뒤쪽 prototype 대입을 지우지 않도록 선언별 사실 한도를 적용했다. prototype 본문은 실제 저장된 callable 근거가 있을 때 조건부로 해석하며, 생성만으로 실행을 증명하지 않는다. 앞단 입력·파싱과 지연 실행 내부 연결은 진전됐지만 임의 Effect 프로그램의 실행기를 완성한 것은 아니다.

[js_generators.py](../../js_generators.py)는 generator 생성을 지연된 값으로 보존하고, 명시적인 `.next()`에서 직선형 본문을 재개한다. `yield` 출력, 다음 `.next(value)` 입력, 완료 후 `undefined`, 확인된 동기 native generator 사이의 `yield*`를 구분한다. 미확인 iterator 위임, 지원하지 않는 분기·반복 안의 yield, 중첩 제어문 안의 return과 throw는 경계로 남는다. 마지막 두 경우에는 뒤의 yield로 진행하지 않도록 미확정 대조를 추가했다. 배열 `pop`은 같은 배열의 원소 기원을 보존하되 비어 있지 않음과 실제 선택·순서 조건을 남긴다. async generator와 사용자 정의 iterator·fiber continuation 전체는 해석하지 않는다. [ECMAScript generator 계약](https://tc39.es/ecma262/multipage/control-abstraction-objects.html#sec-generator-objects).

## 범용성·회귀·보존

이벤트 API나 저장소 이름별 연결 규칙을 추가하지 않았다. 같은 [공통 관계 모델](../../model.py)과 언어별 값 해석을 사용한다. 명시적인 표준 라이브러리 계약과 선정 의존성 모듈 연결은 구분해 다룬다.

165→201사례 단계의 36사례를 유지하고, 포인터·타입 키 20개와 generator·배열 pop 13개를 추가했다. 현재 **108소스 파일·100분석 실행·247사례**이며, 호출 양성 125·데이터 반환 양성 3·데이터 소비 양성 6·구조적 무연결 105·미확정 유지 8으로 구분한다. 추가 33개는 호출 양성 18·구조적 무연결 11·미확정 유지 4다. 이후 `777e8330f`의 234사례를 유지하고 TypeScript 3파일·13사례를 더했다. 저장된 함수의 receiver, 모듈 live binding, 명시적인 generator callback 호출과 인자 순서를 대조한다. 추가 13개는 호출 양성 4·조회 키 소비 양성 2·구조적 무연결 7이다. 반례는 지정 양성 대조가 함께 통과해야 인정하며, 새 양성은 정확한 호출 관계와 필수 조건도 검사한다. [사례](../regressions/cases.json), [결과](../regressions/results/evaluation.json).

공개 경로에서 찾은 결함을 사용해 개선한 회귀 표본이므로 독립 평가 표본이 아니다. [Scala case class 계약](https://docs.scala-lang.org/tour/case-classes.html)과 Set 복사에서는 기존 원소와 새 원소의 provenance를 따로 유지한다. 한 저장 위치의 서로 다른 값도 출력에서 사라지지 않도록 대조한다.

Effect 선정 입력 중 13파일은 Tree-sitter 문법의 타입 구문 한계를 바이트·개행 위치를 유지한 대체 트리로 복구한다. 실행 본문과 원문을 보존하며 대체 트리에 구문 오류가 없을 때만 채택한다. `generic_type_constraints_unproven`을 유지하므로 파싱 오류 0은 타입 검사 성공이 아니다. [TypeScript 타입 매개변수 구문](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-4-7.html), [구현](../../ts_syntax.py).

중간 측정은 `final-*-round*` 디렉터리에 남아 있다. 생성자/prototype 문맥을 구분하기 전의 일부 연결, 실패한 회귀, 당시 코드 해시를 최종 근거로 사용하지 않는다. 현재 결과의 소유 경로는 이 디렉터리의 `results`와 상위 `results`, `regressions/results`다.

## 남은 작업과 SCM의 역할

| 미완료 경로 | 이번에 확인한 앞단 | 아직 필요한 의미 분석 |
| --- | --- | --- |
| Bevy observer | source-backed pointer·tuple wrapper의 값 기원, derive·Bundle 원문 확보 | `IntoObserverSystem` 구현 선택, derive 생성 trait 근거, 같은 World·observer Entity·component 타입 키를 유지한 물리 저장 위치 해석 |
| Bevy SystemId | 구체 TypeId 키와 다른 Map·타입의 분리 | generic 타입 인자의 호출 간 치환, Entity별 column/sparse 저장 슬롯과 take·재삽입, 실제 system trait 구현 연결 |
| Bevy glTF | 최종 hook 호출 추출, 포인터·resource·저장소 원문 확보 | 같은 World의 resource 조회, Arc 공유와 load별 Vec/Box 복제 기원을 구분한 trait dispatch |

이 세 경로는 단순히 테스트를 더 실행하면 완료되는 항목이 아니라 분석기 기능의 미구현 부분이다. Bevy 출력에는 generic `TypeId` 인자와 pointer 기원을 해석하지 못한 진단이 남고, Effect 출력에는 source-call 깊이·prototype 문맥 한도가 기록된다. 입력을 늘리거나 한도만 높였다는 이유로 연결 성공을 인정하지 않았다.

**이 경로들이 원리적으로 연결 불가능하다는 결론은 아니다.** Tree-sitter `.scm` 쿼리를 개선하면 호출·제네릭·필드·패턴 등 원문에 있는 구조를 더 잘 추출할 수 있다. 다만 raw pointer의 가리키는 대상이나 World/Entity 동일성은 별도 값·타입 분석이 필요하고, proc-macro가 생성한 본문은 원본 AST에 존재하지 않을 수 있다. 컴파일러의 확장·타입 정보를 사용하거나, 근거를 명시한 외부 저장 효과 요약을 도입하는 방법이 있다. [Tree-sitter 쿼리](https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html), [Rust procedural macro](https://doc.rust-lang.org/reference/procedural-macros.html).

현재 PoC는 `.scm` 파일을 읽지 않고 Tree-sitter AST를 직접 해석하므로 제품의 `.scm`만 수정해도 이 PoC가 바뀌지는 않는다. 수동 Bevy 저장 효과 계약은 도입하지 않았다. 분리 worktree의 Rust 1.98.1 컴파일은 통과했지만 컴파일러 입력의 일반 포인터·제네릭 저장소 분석은 아직 실험 중이며, 위 정적 분석의 자동 연결 근거로 사용하지 않았다. 소스에서 확인한 표준 포인터·타입 키·generator 동작을 보완했으나, 자동 연결에 필요한 generic component 슬롯의 물리 저장 위치 해석은 아직 구현 중이다. RPC의 이번 조회 키 연결은 iterator·continuation 전체 실행을 요구하지 않는 명시적 callback 인자 바인딩으로 확인했다.

## 검증과 재현

작업 디렉터리는 상위 `language-corpus`다. Python 3.14.5·macOS arm64와 기존 문법 의존성을 사용했다. 환경 준비는 [전체 보고서](../README.ko.md)를 따른다.

```sh
.venv/bin/python remaining-routes/prepare.py --sources /tmp/codemap-language-repro-sources
.venv/bin/python regressions/run.py
.venv/bin/python manage.py measure --sources /tmp/codemap-language-repro-sources
.venv/bin/python remaining-routes/run.py --sources /tmp/codemap-language-repro-sources
.venv/bin/python remaining-routes/consumers.py
.venv/bin/python remaining-routes/verify.py --sources /tmp/codemap-language-repro-sources
```

[prepare.py](prepare.py)는 고정 소스를 준비하고, [run.py](run.py)는 여섯 변형의 입력 해시·한도를 확인한다. [consumers.py](consumers.py)는 추가 소비 끝점만 평가한다. [verify.py](verify.py)는 원시 결과 **143개**의 구현·소스 해시, 이전 보존본, 원래 사례와 새 구독자 값·필수 조건, 표준 호출/데이터 분류, 추가 소비 대조를 검증한다. 실제 실행은 `/tmp/codemap-event-poc-venv/bin/python`으로 수행했다. [무결성 결과](verification.json), [기본 검증](../regressions/results/verification.json).

모든 관계는 `conditional_source_relation`, `concrete_instance_proven=false`, `event_classification=not_inferred`다. 원시 결과 수는 기본·공통 37개 + 회귀 100개 + 확장 6개다. 분석기는 사례의 정답을 입력받지 않는다.

확장 입력의 구문 오류는 0이지만 기본 공개 입력의 일부 문법 복구 한계는 [전체 보고서](../README.ko.md)에 유지했다. 함수 요약 4회·전체 40,000사실, 기존 엔진 함수당 192사실·관계 2,048개, 추가 언어 엔진 함수당 256사실·관계 4,096개 등 한도가 있다. TypeScript 실제 인자 해석 깊이 6, Scala/혼합 Java 소스 호출 깊이 10, prototype 문맥 최대 256개·6회 확장도 완전성을 제한한다. Native generator는 32개 캡처·64단계·위임 깊이 6의 한도를 사용하며 일반적인 분기·반복·예외 제어 흐름은 구현하지 않았다. 관계는 종류·소비 파일별로 예산을 나누며 일부 후보가 생략되면 notices에 기록한다.

대상 저장소의 컴파일·테스트·런타임, 실제 인스턴스·수명·등록 순서·동시성, 공개 실행 시나리오 26개와 모든 후보의 정밀도는 검증하지 않았다. 제품 Rust·MCP/CLI 통합도 수행하지 않았다. 현재 검증 완료 범위와 위 네 미완료 경로를 구분해 사용해야 한다.
