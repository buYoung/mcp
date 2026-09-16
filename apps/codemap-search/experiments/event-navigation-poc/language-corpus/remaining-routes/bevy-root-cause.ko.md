# Bevy 연결 실패 원인과 다음 작업의 완료 기준

분석 기준: `02426ebdbd66e6f90833dbd9b50b42d51ea1fe47`, 2026-09-16. Bevy는 `29fe519f32a14503c8c0fab0baacafe76842bb76`의 선정 입력 62파일·1,711,338바이트다.

이 문서는 수정 전 분석 기록이다. 이후 구현과 현재 71파일 입력의 결과는 [잔여 경로 보완 결과](README.ko.md#bevy-원인-분석-후-적용한-수정)에 구분해 기록한다. 아래의 한도 진단은 보존한 62파일 입력·당시 분석기 해시에 관한 결과이며 현재 코드의 측정값이 아니다.

**세 경로의 실패는 출력 한도만의 문제가 아니다. 현재 분석기가 호출·타입·저장 위치의 연결 정보를 여러 지점에서 잃고, 필요한 중간 소스도 일부 빠져 있다.** 전체·함수별 사실 한도를 확대한 진단에서도 세 저장→호출 관계는 모두 0개였다. 이와 별도로 요약 깊이·대안 수 등의 제한은 남아 있어 무제한 분석의 결과를 주장하지 않는다.

이번에는 제품·PoC 분석기 구현과 기존 측정 결과를 수정하지 않았다. 메인 스레드에서 코드·Bevy 원문·평가 계약을 직접 대조하고, 별도 프로세스의 진단 실행 3개와 추출 단계 질의 1개를 수행했다. 초기 위임 자료를 결론으로 채택하지 않고 아래 재현 근거와 직접 읽은 구현을 판단 기준으로 사용했다. 이후 원인 파악·설계·판단은 하위 에이전트에 위임하지 않는다.

핵심 결론은 다음과 같다.

1. 원래 목표는 **조건부 소스 관계**다. 런타임의 동일 객체·실제 이벤트 실행을 완전히 증명해야 한다는 이전 설명은 완료 조건을 과도하게 높였다.
2. `Some`, `Box`, 호출 식별자, 제네릭 변수 처리에도 아직 구체적인 누락·결함이 있다. 문제를 모두 “복잡한 ECS/raw pointer”로 설명한 것은 부정확했다.
3. 표준 래퍼 처리를 고쳐도 trait 선택·연관 타입·저장 슬롯·복제 기원 분석은 별도로 필요하다. 한 군데를 고치면 세 경로가 해결된다는 근거는 없다.
4. `290/290`은 구문·반례 회귀의 성공이다. Bevy 세 경로를 직접 통과한 결과가 아니다. `13/13` 컴파일러 대조도 값의 연결 검증이 아니다.
5. 다음 구현은 **SystemId의 실제 공개 쌍 하나를 끝까지 연결하는 단계**부터 수행한다. 이 단계가 통과하기 전에 Observer·glTF로 구현 범위를 넓히지 않는다.

## 1. 원래 요구한 성공은 무엇인가

[cases.json](../cases.json)의 세 사례는 모두 `positive / endpoint_pair / callback`이며, 조건은 “조건부 소스 관계이며 런타임 전달을 입증하지 않음”이다. 저장과 호출 위치를 대조하는 [평가기](../manage.py)는 `storage_to_invocation` 또는 `stored_object_method_candidate`를 호출 후보로 인정한다.

| 사례 | 저장 범위 | 호출 범위 | 기준 결과 |
| --- | --- | --- | --- |
| `b_boxed_observer` | `observer/distributed_storage.rs:224–235` | `observer/runner.rs:79–123` | 미확정, 관계 0 |
| `b_system_id` | `system/system_registry.rs:456–478` | 같은 파일 `700–734` | 미확정, 관계 0 |
| `b_gltf_handler` | `examples/gltf/gltf_extension_mesh_2d.rs:67–85` | `bevy_gltf/src/loader/mod.rs:1751–1762` | 미확정, 관계 0 |

소스 경로의 `observer/`, `system/` 접두사는 `crates/bevy_ecs/src/` 아래다. 기준 결과는 [Bevy 평가](results/bevy-modules.evaluation.json)에 있다. 이미 연결된 `b_message_buffer`는 자료 반환 사례이며 위 세 호출 사례와 구분한다.

필요한 것은 저장값이 최종 호출 대상일 수 있는 **일관된 소스 경로와 조건**이다. `concrete_instance_proven=false`여도 성공할 수 있다. 다만 타입 이름이 같다는 이유로 다른 World·Entity·객체를 합치거나, 관계 근거 없이 조건 문자열만 붙이는 것은 허용하지 않는다.

Observer의 원래 사용자 closure 본문까지 실행됐다는 증명, 특정 asset이 실제 loader를 선택했다는 증명, 런타임 수명·동시성의 완전한 증명은 원래 완료 조건이 아니다. 추가 정밀도 검증과 원래 양성 쌍의 완료 조건을 분리한다.

## 2. 메인에서 직접 실행한 한도 분리 진단

같은 원문 해시·같은 62파일·같은 모듈 연결을 사용했다. 변경한 값은 진단 프로세스 안의 사실 한도뿐이며 기본 설정 파일은 바꾸지 않았다. 함수 요약 반복은 4회로 유지했다.

| 진단 | 전체 사실 한도 | 함수별 사실 한도 | 최종 사실 수 | 전체 절단 / 함수 cap 진단 | 세 경로의 관계 수 |
| --- | ---: | ---: | ---: | --- | --- |
| 기본 재현 | 40,000 | 192 | 40,000 | 3회 / 16건 | 0 / 0 / 0 |
| 전체 한도 확대 | 160,000 | 192 | 41,674 | 0회 / 16건 | 0 / 0 / 0 |
| 함수 한도도 확대 | 160,000 | 768 | 42,879 | 0회 / 0건 | 0 / 0 / 0 |

전체 한도 확대 시 네 차례의 `cap_facts` 호출에서 제거 수가 모두 0이었다. 함수 한도도 늘린 실행에서는 `function_fact_cap` 진단도 사라졌다. 따라서 **현재 입력에서 이 두 예산을 늘리는 것만으로 해결된다**는 가설은 이번 실행으로 반증됐다. 함수 cap 진단 0건을 모든 내부 제한·생략이 없다는 뜻으로 확대하지 않는다.

최종 출력 경쟁도 별도로 분리했다. 분석을 마친 facts에서 각 사례의 저장·호출 범위에 해당하는 사실만 골라 기존 연결 함수를 다시 적용했다. 사례 라벨은 이 대조 단계에만 사용했으며 추출기나 타입 해석기에 전달하지 않았다.

| 진단 | Observer 대상 facts | SystemId 대상 facts | glTF 대상 facts | 집중 대조의 관계/생략 |
| --- | ---: | ---: | ---: | --- |
| 기본 | 171 | 94 | 7 | 세 사례 모두 0 / 0 |
| 전체 한도 확대 | 173 | 97 | 10 | 세 사례 모두 0 / 0 |
| 함수 한도도 확대 | 173 | 99 | 10 | 세 사례 모두 0 / 0 |

집중 대조는 출력 한도의 영향을 가려내는 진단이며 공식 성능·정밀도 평가가 아니다. 중간 facts를 새로 만들거나 연결 규칙을 느슨하게 하지 않았다. 생략이 없는데도 관계가 없으므로 **이미 만들어진 올바른 세 관계가 최종 출력에서만 사라졌다**고 설명할 수 없다.

남는 제한은 요약 반복 4회, 호출 해석·heap 조회 깊이, 값 대안 수, 필드 투영 횟수 등이다. 이 제한들의 영향을 모두 분리한 것은 아니다. 다만 아래의 코드상 손실은 한도와 별도로 직접 확인됐다.

근거: [기본](diagnosis-02426ebdb/baseline.json), [전체 한도 확대](diagnosis-02426ebdb/global-budget.json), [함수 한도 확대](diagnosis-02426ebdb/function-budget.json), [진단 코드](diagnosis-02426ebdb/diagnose.py).

## 3. 추출 단계에서 직접 재현한 결함

### 3.1 Observer의 `Some(system)`에서 이미 Box 기원이 끊긴다

`distributed_storage.rs:225`의 `Box::new(...)`는 allocation으로 만들어진다. 그러나 같은 함수 `:227`의 `Some(system)`은 Option wrapper가 아니라 불투명한 `result`가 된다.

현재 [engine.py](../../engine.py)의 `rust_standard_path`(`1323–1344`)는 별도 import가 없는 prelude 이름에 대해 파일에 wildcard `use`가 하나라도 있으면 해석을 거부한다. 이 파일에는 `crate::prelude::*`가 있다. 직접 질의한 결과는 다음과 같다.

| 질의 | 결과 |
| --- | --- |
| 실제 `Some`의 local/module 이름 가림 | 둘 다 없음 |
| unqualified `Some`를 표준 Option으로 인정 | false |
| 같은 환경의 `core::option::Option::Some` | true |

이것은 일반적인 trait·포인터 분석보다 앞에 있는 누락이다. 원문의 import를 억지로 바꿔 통과시키는 대신, 실제 이름 해석 결과를 사용해야 한다. wildcard가 외부에서 같은 이름을 가져올 가능성을 무시하고 모든 `Some`를 표준으로 취급해서도 안 된다.

### 3.2 glTF 예제의 prelude `Box::new`도 지원 범위에서 빠져 있다

`gltf_extension_mesh_2d.rs:73–85`는 별도의 `alloc::boxed::Box` import 없이 `Box::new(...)`를 사용한다. 현재 Box 생성 분기는 명시적 canonical 경로·import만 확인하며 Box의 prelude 해석을 제공하지 않는다(`engine.py:1501–1506`).

직접 질의에서 예제의 `Box::new`는 false, `alloc::boxed::Box::new`는 true였다. 따라서 resource/lock 문제와 별도로 Box allocation·pointee 사실도 만들어지지 않는다. 이 질의는 AST 처리 범위를 확인한 것이며, 예제 바이너리를 컴파일한 검증은 아니다.

### 3.3 연쇄 호출이 같은 불투명 값 식별자를 사용한다

`system_registry.rs:476`의 두 AST 노드는 다음과 같다.

| 식 | start byte | end byte | 원문 위치 |
| --- | ---: | ---: | --- |
| `self.spawn(RegisteredSystem::new(system))` | 16647 | 16688 | 476:22 |
| `self.spawn(RegisteredSystem::new(system)).id()` | 16647 | 16693 | 476:22 |

현재 fallback 결과는 `path:line:column`으로만 만들어진다(`engine.py:1617,1626`). 실제 호출 추적에서도 두 식 모두 `result:.../system_registry.rs:476:22`였다. 별도 호출·반환값인데 시작 위치가 같아 식별자가 충돌한다.

이 문제는 직전 수정의 “매크로 내부 상대 위치” 구분으로 해결되지 않는다. 일반 연쇄 호출은 같은 start byte도 공유하므로 **끝 위치까지 포함한 AST 호출 식별자와 호출 문맥**이 필요하다. 화면에 보여 주는 원문 위치는 유지할 수 있다. 이 충돌 자체가 세 경로의 유일한 실패 원인이라고 주장하지는 않는다.

### 3.4 미확정 impl 타입 변수 `T`가 구체 타입 식별 경로로 들어간다

실제 원문은 다음 generic impl이다.

```rust
impl<E: EventPattern, M, T: IntoObserverSystem<E, M>> IntoObserver<(E, M)> for T
```

현재 collector는 이 메서드의 owner를 `.../distributed_storage.rs::T`로 만든다. 이 owner는 `Program.types`에 선언된 명목 타입도 아니고 `generic_types`에 기록된 generic 명목 타입도 아니다. 그런데 `Observer::new(self)`의 인자 추론은 `I`를 이 문자열로 바인딩한다. 이어 `concrete_type_identity`에 실제 formal `I`를 전달하면 같은 문자열을 구체 타입 식별값으로 반환한다.

즉 **타입 변수와 실제 선언 타입을 분리하지 못하는 경로가 재현됐다.** 이것이 현재 Bevy에서 특정 오연결을 발생시켰다고 측정한 것은 아니다. 그러나 타입 동일성의 근거가 될 수 없는 owner 문자열을 구체 타입 키로 사용할 위험이 있으므로, 연결을 늘리기 전에 고쳐야 한다. 서로 다른 scope의 `T`, 연관 타입, 실제 `RegisteredSystem<I,O>`를 문자열 하나로 다루어서는 안 된다.

네 항목의 직접 재현: [frontend.json](diagnosis-02426ebdb/frontend.json), [probe_frontend.py](diagnosis-02426ebdb/probe_frontend.py). 초기 함수 선택에서 impl 시작 줄과 함수 시작 줄을 혼동한 진단 스크립트 오류를 수정한 뒤 저장한 결과다. 분석기 코드는 변경하지 않았다.

## 4. 작은 구문 보완으로 해결되지 않는 구조적 손실

### 호출이 해석되지 않을 때 정보가 너무 일찍 사라진다

[model.py](../../model.py)의 `Fact`는 전체 호출 operand 목록을 보존하지 않는다. 현재 `engine.py:1576–1582`는 인자가 `slot`이거나 특정 callable parameter일 때만 인자 사실을 남긴다. allocation·wrapper·tuple·직접 함수 값 등은 unresolved call의 전체 인자로 기록되지 않는다.

target이 없거나 여러 개면 callee 호출 사실과 불투명한 `result`만 남는다. 선언된 반환 타입, 원래 인자 목록, 제네릭 치환, 선택하지 못한 후보와 그 이유가 이 결과에 함께 남지 않는다. 원본 AST를 다시 해석할 수는 있지만, 저장된 facts·summary만으로 잃은 연결을 복원할 수는 없다.

연결 함수의 거절은 명확하다. `match_storage`(`model.py:156–199`)는 같은 추상 root와 호환되는 경로를 요구하며 `unknown/result/unresolved` root를 저장소 root로 인정하지 않는다. 기준 산출물에서:

- Observer의 최종 수신자는 `unknown:[key:run_unsafe]`다.
- SystemId의 최종 수신자는 `result:...:704:26[key:run_without_applying_deferred]`다.
- glTF의 최종 수신자는 `unresolved:...:extension[key:on_spawn_mesh_and_material]`이며 등록 범위의 store 자체가 0개다.

따라서 매칭 조건을 느슨하게 만드는 것은 원인 해결이 아니다. 먼저 이 수신자를 원래 저장값의 기원과 연결해야 한다.

### 제네릭·trait·함수 포인터 인스턴스 정보가 부족하다

- [rust_types.py](../../rust_types.py)의 타입 바인딩은 함수 자체의 type parameter와 제한된 직접 인자만 처리한다. impl의 type parameter, `Vec<T>` 같은 중첩 추론, 연관 타입, trait bound 선택을 함께 해석하지 않는다.
- `Program.methods`는 주로 `(owner, method name)`으로 후보를 찾는다. 임의 trait→impl 선택과 연관 타입을 표현하는 별도 구조가 없다. 실제 `IntoObserverSystem` blanket impl은 입력에 있어도 그 관계를 활용하지 못한다.
- `generic_function` 표현은 안쪽 function 값으로 축약된다(`engine.py:1147` 부근). 저장된 `observer_system_runner::<E,I::System>`은 함수 위치만 남고 `E`, `S=I::System`의 인스턴스 정보가 사라진다.
- runner의 `S`는 runtime 함수 인자에서 직접 추론할 수 있는 값이 아니다. 생성 시 boxed system의 타입과 함수 포인터에 바인딩된 타입을 함께 보존해야 한다.
- Any downcast는 현재 구체적인 비제네릭 target만 받아들인다. 미확정 `S`나 복합 generic target은 `UNKNOWN`을 반환한다([rust_values.py](../../rust_values.py):68–97).

### 래퍼·저장 효과·복제의 지원도 부분적이다

현재 Option 처리는 제한된 `Some/unwrap/expect/map` 등이다. SystemId의 `Option::take`, Observer의 `as_deref_mut`, Result의 `map_err/ok_or/?`를 한 경로로 전달하는 처리까지 갖춘 것은 아니다. 특히 `take`는 이전 payload를 반환하면서 원래 슬롯을 비우므로 단순한 getter로 취급하면 안 된다.

pointer 처리는 알려진 pointee 기원을 보존하는 범위다. byte offset·row·layout·`ptr::copy`의 저장 효과는 표현하지 않는다. 또한 standard `Arc` 공유 clone, Vec의 원소별 clone, source-defined Box clone의 새 객체 기원을 따로 표현하는 기능이 없다.

이 목록은 직접 읽은 구현의 지원 경계다. 각각을 고친 뒤 세 경로가 얼마나 개선되는지는 아직 측정하지 않았다.

## 5. SystemId: 처음 끝까지 연결할 대상

이번 첫 대상은 **World의 `register_boxed_system → run_system_with` 내부 경로**다. 원래 공개 쌍의 저장 범위에는 `register_system`의 Box 생성도 포함되므로 그 원래 저장 위치와의 기원 연결은 보존한다. Commands queue나 예제 Query의 실제 실행까지 첫 단계에 섞지 않는다.

소스는 다음 논리 연결의 근거를 제공한다. `(W,E,T)`는 아래 설명용 기호이며 Bevy API 이름을 분석기에 하드코딩하겠다는 뜻이 아니다.

```text
등록 Box
  → RegisteredSystem<I,O>.system
  → Bundle의 component pointer
  → 동일 W의 TypeId<T> → ComponentId
  → 선택된 column[row]에 payload 저장

spawn이 반환한 E → SystemId.entity
  → 동일 W에서 E의 EntityLocation 조회
  → 같은 T의 ComponentId + table_row로 column[row] 조회
  → Mut<T>.value → system.take()의 이전 payload
  → run_without_applying_deferred
```

| 단계 | 직접 확인한 원문 | 현재 부족한 연결 |
| --- | --- | --- |
| Box 보관 | `system_registry.rs:33–38,456–478` | Box와 `RegisteredSystem<I,O>`의 타입·값 관계를 spawn 인자에 보존 |
| spawn/ID 반환 | `world/mod.rs:1245–1257`, `world_mut.rs:179–184` | `B` 치환, 반환 `EntityWorldMut.entity`와 `.id()` 값 연결 |
| component ID | `bundle/impls.rs:16–25`, `component/register.rs:163–169`, `component/info.rs:594–620` | `Bundle for C:Component` 선택, 같은 Components의 TypeId→ComponentId 왕복 |
| row/location | `bundle/spawner.rs:108–153`, `archetype.rs:600–614` | entity→table row→EntityLocation의 기원 연결 |
| payload 쓰기 | `bundle/impls.rs:43–51`, `bundle/info.rs:257–277`, `blob_array.rs:328–334` | generic callback의 component pointer를 column/row 저장 효과로 전달 |
| entity 조회 | `entity/mod.rs:846–862,957–964` | 저장한 location 조회와 Entity generation 검사 보존 |
| typed component 읽기 | `unsafe_world_cell.rs:1048–1075` | 같은 TypeId/ComponentId·EntityLocation에서 pointer와 `Mut<T>.value` 복원 |
| take/호출/복원 | `system_registry.rs:704–734` | 이전 Box payload 반환, 슬롯 비움, 호출, 같은 슬롯 재삽입을 구분 |

`World` 관련 파일은 `crates/bevy_ecs/src/world/`, 나머지 ECS 경로는 `crates/bevy_ecs/src/` 아래다. `world_mut.rs`는 `world/entity_access/world_mut.rs`다.

물리 주소를 전부 계산해야만 조건부 관계를 만들 수 있는 것은 아니다. `core::ptr::copy::<u8>(value.as_ptr(), dst.as_ptr(), size)`와 destination의 row 계산으로부터 **검증 가능한 논리 저장 효과**를 유도하는 접근을 검토할 수 있다. 다만 실제 source body에서 base·index·layout·payload 관계를 얻어야 하며, `spawn/get_mut`라는 이름만 보고 저장·조회를 가정해서는 안 된다.

`RegisteredSystem`의 `Component`는 derive다. 현재 원문 생성기와 compiler metadata에는 Table/Mutable 근거가 있지만 분석기에 구체 impl로 반영되지 않는다. 반면 Observer는 아래와 같이 명시적인 Component impl이 있다. 두 경우를 같은 매크로 문제로 묶지 않는다.

## 6. Observer와 glTF는 어떤 추가 문제가 있는가

### Observer

1. `observer_system.rs:28–46`의 trait·blanket impl·`type System=S::System`은 **이미 입력에 있다**. 입력 추가만으로 해결될 문제가 아니다.
2. 기본 생성자는 `Box<I::System>`과 `observer_system_runner::<E,I::System>`을 같은 Observer에 보관한다(`distributed_storage.rs:224–235`). 이 타입 관계를 저장된 함수 값에도 유지해야 한다.
3. `Some(system)`의 prelude 누락 때문에 그보다 앞에서 Box의 연결이 한 번 더 끊긴다.
4. `Observer: Component`는 `distributed_storage.rs:361`의 명시적 impl이며 `SparseSet`을 선택한다. 이 부분에는 proc-macro 확장이 필요하지 않다.
5. cache는 `observer_entity → runner`를 저장하고(`observer/mod.rs:321–374`), 순회에서 나온 key를 runner의 entity 인자로 전달한다(`:171–182`). key와 value를 독립적으로 섞으면 다른 observer/runner를 잘못 연결한다.
6. runner는 같은 entity로 Observer를 읽고, `Any`에서 `S`로 downcast한 후 호출한다(`runner.rs:42–47,90–110`). 저장소 조회뿐 아니라 generic 함수 포인터 바인딩과 Any/trait view도 필요하다.
7. custom runner 생성자는 호출자 runner와 별도 dummy system을 보관한다(`distributed_storage.rs:238–273`). Observer 타입이라는 이유만으로 default runner의 system 호출과 합치면 안 된다. 모든 임의 custom runner가 system과 무관하다고 단정하는 것도 잘못이다. 선택된 runner 본문을 기준으로 판단한다.

원래 양성 쌍은 boxed system→runner 호출의 조건부 관계다. 원래 사용자가 전달한 모든 closure/combinator의 최종 본문까지 해석하는 것은 더 넓은 범위다.

### glTF

등록 경로 `resource_mut::<GltfExtensionHandlers>().0.write[_blocking]().push(...)`는 현재 store가 아니라 불투명 수신자에 대한 member call로만 남는다. prelude Box 누락도 동시에 존재한다. 최종 `:1754` hook은 추출되므로 endpoint를 찾지 못한 문제와 구분한다.

| 구간 | 원문 | 보존해야 할 의미 |
| --- | --- | --- |
| resource/등록 | 예제 `70–85`, extensions `31–34` | 같은 World의 typed resource, tuple field, lock 보호값, Vec membership |
| loader에 전달 | `bevy_gltf/src/lib.rs:302–310` | Arc handle clone은 같은 보호값 allocation을 공유 |
| load별 복제 | `loader/mod.rs:259–266` | read guard의 Vec를 새 Vec로 복제하고 원소별 clone을 추적 |
| Box clone | `loader/extensions/mod.rs:401–404,569–572`, 예제 `95–97` | 로컬 `dyn_clone` 구현이 반환한 새 Box와 원본의 방향성 있는 기원 관계 |
| hook 호출 | `loader/mod.rs:1751–1762`, extensions `502–519` | slice/iterator 원소→Box pointee→erased trait→해당 구현의 호출 대상 |

`dyn_clone`은 별도의 외부 `dyn-clone` crate가 아니다. 이 경로의 trait와 Box Clone 구현은 Bevy 소스에 있다. 외부 lock은 `async_lock::RwLock`이다.

Arc 공유, Vec 복제, Box 복제를 하나의 “clone은 동일 객체” 규칙으로 처리할 수 없다. 복제 전후 객체는 구분하고, 원본에서 유래했다는 관계만 보존해야 한다. 서로 다른 load의 clone을 같은 인스턴스로 판정하지 않는다.

## 7. 입력이 실제로 빠진 곳

직접 현재 [입력 목록](inputs.json)과 원문을 대조했다.

| 빠진 입력 | 관련성 |
| --- | --- |
| `bevy_ecs/src/entity/mod.rs` | Entity 선언, allocator, location 저장·조회, generation 검사 |
| `bevy_ecs/src/archetype.rs` | table row를 포함한 EntityLocation 생성 |
| `bevy_ecs/src/entity/hash_map.rs` | Observer cache wrapper의 Deref·IntoIterator와 key/value 전달 |
| `bevy_platform/src/collections/hash_map.rs` 및 해당 모듈 연결 | wrapper가 실제 내부 map에 연산을 전달하는 근거 |
| `async-lock` 소스 | read/write future·guard에서 보호값으로 돌아가는 경로 |

앞 네 경로는 Bevy의 `crates/` 아래다. source-only 분석에서 필요한 구현이 없는 경우와, 구현이 들어 있는데 분석기가 활용하지 못하는 경우를 별도 진단해야 한다. 파일 수를 늘리는 것만으로 trait·저장 효과가 자동 구현되는 것은 아니다.

Bevy 원 revision에는 Cargo.lock이 없다. glTF Cargo manifest의 요구는 `async-lock = "3.0"`이고, 별도 compiler oracle의 보존 lock은 `3.4.2`를 선택했다. 3.4.2를 원 revision의 유일한 공식 고정 버전으로 설명해서는 안 된다. 추가 입력으로 채택하면 그 선택·출처·해시를 명시한다.

`bevy_asset`의 loader 등록·회수 소스도 현재 입력 밖이다. 다만 **그 전체 구현을 읽는 것이 기존 조건부 endpoint 계약의 명시적 완료 요건은 아니다.** 실제 특정 loader의 선택을 추적하거나 여러 loader를 분리하는 정밀도를 높일 때 필요한 추가 범위로 구분한다. 이 경계를 이유로 첫 SystemId 구현을 미루지 않는다.

## 8. 이전 검증 숫자가 놓친 것

### 회귀 290개

전체 290개 중 Rust 회귀는 83개다: 연결 38, 구조적 무연결 40, 미확정 유지 5. 이들은 별도 예제 경로에 있으며 실제 Bevy 세 endpoint를 직접 대조하는 회귀는 아니다.

TypeId·다른 key/type/container·Any 반례는 있다. 그러나 World·Entity·generation·custom runner를 직접 구분하는 대조는 없었다. clone 관련 Rust 회귀도 pointer clone 기원 유지 사례이며 Vec/Box의 복제 객체 분리 검증이 아니다.

공개 Bevy 사례 6개는 양성 4개와 실행하지 않은 시나리오 2개다. **공개 Bevy의 자동 negative endpoint pair는 0개**다. “공개 반례 14개 통과”를 Bevy의 안전성 검증으로 해석하면 안 된다. custom runner·clone 시나리오는 `source_scenario_not_executed`이며 자동 오연결 방지 검증 성공이 아니다.

### 컴파일러 대조 13개

12개는 지정 source span 안의 MIR 명령에 요구 문자열이 존재하는지 검사하고, 1개는 rustdoc의 `RegisteredSystem: Component` 구현을 검사한다. 요구 문자열들이 같은 명령·basic block·연속된 def-use 경로에 있어야 한다는 조건은 없다([collect.py](../../compiler-oracle/collect.py):103–113).

따라서 이 검증은 타입 표현·변환·호출·impl이 컴파일러 출력에 존재한다는 근거다. 저장된 값이 해당 호출의 operand까지 흘렀다는 검증이 아니다. 예제 glTF 바이너리는 해당 library-only 빌드에 포함되지 않았다. generic MIR의 `<I as Trait>::method`나 dyn call도 모두 구체 구현 하나로 해소된 것은 아니다.

또한 사용자의 조건은 **제품에 컴파일러 보조 입력을 필수로 넣지 말라**는 것이었다. 독립 PoC에서도 정보를 읽어 연결을 실험하지 말라는 뜻은 아니었다. 이전에 보조 작업을 대조기 수준으로만 마무리한 것은 내가 범위를 좁혀 처리한 부분이다.

앞으로 compiler 자료는 소스 해석의 타입·변환 결과를 비교하거나 별도 연결 실험에 활용할 수 있다. 다만 컴파일러가 있어야만 나온 결과는 source-only 제품 후보의 성공 수에 합산하지 않는다. 제품 적용 후보는 대상 저장소의 컴파일 없이도 필요한 근거를 만들어야 한다.

## 9. 다음 구현 순서와 단계별 통과 조건

이번 보고서는 구현 전 판단 자료다. 아래는 아직 수행하지 않은 작업이며, 완료했다고 보고한 내용이 아니다.

### 단계 A — 손실을 먼저 막고 원인을 관측 가능하게 만든다

대상은 `syntax.py`, `model.py`, `engine.py`, `rust_types.py`의 관련 부분이다.

- 호출마다 AST 시작·끝, 매크로 확장 식별자, 호출 문맥을 포함한 고유 식별자를 둔다. 표시용 원문 위치와 내부 식별자를 분리한다.
- 해석 못한 호출도 callee, receiver, 전체 실제 인자, 제네릭 인자, 선언된 반환 타입, 미해석 이유를 보존한다. `result` 하나로만 끝내지 않는다.
- 명목 타입·타입 변수·연관 타입·타입 적용을 구분한다. 함수와 impl의 제네릭 scope를 보존하고 미확정 변수를 구체 TypeId로 만들지 않는다.
- 표준 prelude와 source import의 해석, 실제 경로에 필요한 Option/Result·Box view를 보완한다. 이름이 같은 사용자 정의 타입·메서드에는 표준 효과를 적용하지 않는다.

통과 조건은 이번에 재현한 네 결함의 값 추적이 정확해지는 것이다. 이 단계만 통과했다고 Bevy 해결로 세지 않는다. 지원하지 못하는 표현은 원래 operand를 보존한 미확정으로 남긴다.

### 단계 B — SystemId에 필요한 소스 효과만 유도한다

- Entity/location, archetype, map wrapper의 누락 입력을 호출 경로에 필요한 범위로 추가하고 해시를 고정한다. 기존 62파일과 원래 endpoint 기대값은 보존한다.
- `RegisteredSystem<I,O>`의 타입 인자, Component/Bundle 선택, storage type, callback의 pointer payload를 유지한다.
- 원문이 실제로 수행하는 key 선택·field 접근·row 접근·복사로부터 저장/조회 효과를 만든다. `spawn/get_mut/register_system` 이름에 대응표를 붙이지 않는다.
- 같은 receiver, Entity 전체 값, component type/ID의 관계를 추상 슬롯에 보존한다. ComponentId는 해당 World의 Components에 종속되므로 전역 숫자 키로 취급하지 않는다.
- `take`는 이전 payload의 독립된 반환과 원래 슬롯의 비움 효과를 분리한다. 이후 재삽입도 같은 슬롯에 대한 별도 효과다.
- derive는 선언→실제 생성기/출력 템플릿의 지원 가능한 부분을 근거로 처리한다. source-only로 해석하지 못하면 그 지점에서 미확정으로 남기며 compiler 결과를 숨겨진 정답으로 주입하지 않는다.

**통과 조건:** 원래 `b_system_id`의 위치와 호출 관계 종류를 유지한 채 조건부 관계가 실제로 생성되어야 한다. 중간 인자 전달, 예제 이름 탐지, `via` 어디엔가 끝점이 등장하는 것만으로 통과시키지 않는다. 등록 위치와 내부 저장 근거를 모두 보존한다.

### 단계 C — SystemId의 오연결 대조를 같은 기능 위에 둔다

같은 타입의 여러 객체가 섞이지 않는지 실제 Bevy 경로를 사용하는 대조가 필요하다.

| 구분 | 통과 기준 |
| --- | --- |
| 같은 World·해당 ID·해당 component type | 양성 대조가 최종 호출까지 연결 |
| 다른 World 또는 다른 명시적 Entity | 그 저장값과 호출 쌍은 무연결 |
| 다른 component type 또는 서로 다른 SystemId I/O | 구별 가능한 타입을 합치지 않음 |
| 같은 index·다른 generation | generation을 버리지 않음. 해석이 부족하면 안전성 통과가 아니라 미확정 |
| take 뒤 비어 있는 슬롯·제거·교체 | 이전 payload 반환과 이후 슬롯 상태를 혼합하지 않음. 상태 해석 범위 밖은 별도 미확정 |

음성 대조는 같은 경로의 양성 대조가 통과해야 인정한다. 모든 결과가 0인 상태를 안전하다고 보고하지 않는다. 라이브러리 API 이름을 바꾼 작은 동형 저장소에서도 같은 소스 효과가 유도되는지 확인해 이름 의존 규칙을 막는다.

**단계 B·C가 통과하기 전에는 Observer·glTF 구현으로 넘어가지 않는다.**

### 단계 D — Observer에 저장소 모델과 generic 함수 값을 적용한다

SparseSet 저장소, EntityHashMap의 entry key/value, 저장된 generic runner 인스턴스, boxed type과 downcast target의 관계를 추가한다. default constructor의 pairing과 선택된 custom runner 본문을 구분한다.

통과 조건은 원래 `b_boxed_observer`의 양성 관계와 다른 observer/default-custom runner 대조다. 임의 custom runner가 항상 dummy system과 무관하다고 단정하지 않는다. 원문이 확인되는 구체 반례와 미확정 시나리오를 구분한다.

### 단계 E — glTF의 공유와 복제를 구분한다

typed resource→lock 보호값→등록 Vec의 값을 먼저 연결한다. 그 다음 Arc 공유, load별 Vec 복제, source-defined Box/dyn_clone, iterator item과 trait method를 연결한다.

통과 조건은 원래 `b_gltf_handler`의 저장→hook 관계와 다른 resource·다른 등록 값·다른 clone 기원의 대조다. 복제 전후의 객체를 동일 인스턴스로 표시하지 않는다. 실제 asset loading의 실행 성공은 이 정적 관계와 분리한다.

### 단계 F — 기본 실행으로 최종 검증한다

원래 공개 Bevy 3쌍, 이미 연결된 메시지 반환, 기존 290회귀와 새 대조, 공통 90사례, 다른 언어의 공개 결과를 확인한다. 실제 구현·입력 해시와 출력 근거를 저장한다.

기본 한도에서 endpoint가 출력되지 않으면 “내부적으로 있을 것”이라고 완료 처리하지 않는다. 원인을 사실 생성과 출력 선택으로 분리한다. compiler 보조가 없는 source-only 실행 결과와 compiler-assisted 실험 결과도 별도 집계한다.

## 10. 이번 방식이 이전과 달라지는 지점

이전에는 실제 공개 경로에서 사라진 값의 단계를 고정하지 않은 채 개별 구문 지원과 회귀 수를 늘렸다. 필수 source body의 포함 여부, 실제 Bevy 양성 대조와 연결된 음성 대조, 컴파일러 관측과 연결 계산의 차이를 충분히 검증하지 않았다.

다음 진행 보고의 단위는 테스트 개수가 아니라 다음 세 가지다.

1. 원래 공개 경로의 어느 값이 어느 소비 지점까지 이어졌는가.
2. 같은 모델이 구별해야 할 다른 World·Entity·타입·복제본을 구별하는가.
3. 아직 불투명한 단계는 정확히 어디이며, 입력 누락·추출 누락·의미 미구현·한도 중 무엇인가.

한 단계가 실패하면 해당 단계의 원문과 추상 값 차이를 수정한다. 다른 언어 보완이나 관련 없는 회귀 추가를 그 단계의 진척도로 보고하지 않는다. 현재 증거로 세 경로의 완성을 미리 보장할 수는 없다. 다만 이번에는 **실제 SystemId 양성·오연결 대조를 통과하기 전에는 다음 경로를 진행하지 않는 명확한 판정 기준**을 갖는다.

## 재현 근거와 미확인 범위

[증거 manifest](diagnosis-02426ebdb/manifest.json)에 진단 코드·결과의 해시를 고정했다. 호출 관측 파일은 각 source callsite의 서로 다른 관측을 최대 24개 저장한다. 런타임 trace가 아니라 분석기의 중간 symbolic 값 관측이며 모든 호출 문맥의 완전한 목록은 아니다.

작업 디렉터리는 `apps/codemap-search`다.

아래 명령을 수정 전 결과의 재현에 사용하려면 `--poc` 경로에 분석 기준 커밋의 코드와 버전 7 입력 목록을 준비하고, 이 문서와 함께 보존한 진단 스크립트를 사용해야 한다. 현재 작업 트리의 코드·확대 입력으로 실행한 값은 당시 기준 결과와 구분해야 한다.

```sh
/tmp/codemap-event-poc-venv/bin/python experiments/event-navigation-poc/language-corpus/remaining-routes/diagnosis-02426ebdb/diagnose.py --poc experiments/event-navigation-poc --sources /tmp/codemap-language-repro-sources/bevy --variant baseline --output /tmp/codemap-bevy-diagnosis-results
/tmp/codemap-event-poc-venv/bin/python experiments/event-navigation-poc/language-corpus/remaining-routes/diagnosis-02426ebdb/diagnose.py --poc experiments/event-navigation-poc --sources /tmp/codemap-language-repro-sources/bevy --variant global-budget --output /tmp/codemap-bevy-diagnosis-results
/tmp/codemap-event-poc-venv/bin/python experiments/event-navigation-poc/language-corpus/remaining-routes/diagnosis-02426ebdb/diagnose.py --poc experiments/event-navigation-poc --sources /tmp/codemap-language-repro-sources/bevy --variant function-budget --output /tmp/codemap-bevy-diagnosis-results
/tmp/codemap-event-poc-venv/bin/python experiments/event-navigation-poc/language-corpus/remaining-routes/diagnosis-02426ebdb/probe_frontend.py --poc experiments/event-navigation-poc --sources /tmp/codemap-language-repro-sources/bevy --output /tmp/codemap-bevy-diagnosis-results/frontend.json
```

각 분석 진단은 120초 제한이며 실제 세 실행은 모두 완료됐다. 이 시간·RSS는 진단 관측을 포함하므로 성능 비교 지표로 사용하지 않는다. PoC 기본 설정, 기존 149개 원시 분석 결과, compiler oracle 결과는 이번에 변경하지 않았다.

아직 실행하지 않은 것은 누락 입력을 추가한 재분석, 위 결함을 수정한 뒤의 양성·반례 검증, source-only derive/trait/저장 효과 구현, 대상 애플리케이션·테스트·런타임 실행이다. 표준/외부 라이브러리의 모든 버전이나 임의 proc-macro를 지원한다는 결론도 내리지 않는다.
