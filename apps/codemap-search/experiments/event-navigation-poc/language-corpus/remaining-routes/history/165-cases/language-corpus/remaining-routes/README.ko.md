# Rust·Scala·TypeScript 잔여 경로 보완 결과

검증일: 2026-09-16. **확장 입력에서 Bevy 메시지의 저장 → iterator 반환 경로를 새로 연결했다.** 이 결과는 `conditional_data_return`이며 콜백 호출 성공으로 세지 않는다. 공개 양성 37개를 기본·확장 입력의 가장 진전된 판정으로 합치면 **호출 후보 30개, 데이터 반환 1개, 인자 전달 2개, 미확정 4개**다. Monix의 JVM·JavaScript × Scala 2·3 네 조합과 LiveStore의 잠긴 Effect 구현도 분석했지만, 이들의 끝까지 이어지는 자동 연결은 아직 미완성이다.

회귀 **165/165**, 공통 사례 **90/90**, 양성 대조를 갖춘 공개 무연결 쌍 **14/14**를 확인했다. 런타임 이벤트 전달, 대상 저장소 빌드·테스트, 모든 후보의 정밀도는 검증하지 않았다. 아래에서 자동 분석 결과와 사람이 원문에서 확인한 소비 경로를 구분한다.

## 범위와 비교 기준

비교 기준은 [직전 140사례 단계의 보존본](baseline/manifest.json)이다. 당시 구현·입력 목록·결과·문서 136파일을 SHA-256과 함께 보존했다. Git 체크포인트 `01aac1515864f7066c368e0081d6fd23836a5474`와 기존 공개 입력 96파일·77사례·입력 잠금 버전 3은 변경하지 않았다. 해당 체크포인트의 111사례, 직전 단계의 140사례, 처음 제시한 원문 예시 36개도 유지했다.

파일 분석은 Sol·medium 하위 에이전트가 맡았고, 구현·사례 추가·측정·종합은 메인 스레드에서 수행했다. codemap-search 도구·CLI는 사용하지 않았다. 앞 단계의 Astra·max 실패 분석 2회에 더해 추가 Astra 분석을 요청하지 않았다.

| 입력 | 파일 수 | 바이트 | 파일당 한도 | 목적 |
| --- | ---: | ---: | ---: | --- |
| `bevy-modules` | 26 | 831,316 | 512 KiB | 기존 22파일에 ECS crate/module·message cursor 선언 4파일 추가 |
| `monix-jvm-scala2` | 15 | 58,048 | 512 KiB | Scala 2 생성 매크로·JVM Atomic·Java 저장 본문 |
| `monix-jvm-scala3` | 13 | 47,076 | 512 KiB | Scala 3 inline 생성·JVM Atomic·Java 저장 본문 |
| `monix-js-scala2` | 8 | 43,047 | 512 KiB | Scala 2 생성 매크로·JavaScript Atomic |
| `monix-js-scala3` | 6 | 32,813 | 512 KiB | Scala 3 inline 생성·JavaScript Atomic |
| `livestore-effect` | 18 | 2,370,935 | **1 MiB** | LiveStore 7파일과 Effect 구현 11파일 |

파일 수는 실행별 수이며 서로 중복된다. Monix 네 조합은 동일 공개 참고 쌍의 변형 입력으로, 공개 양성 분모를 늘리지 않는다. 각 조합은 관련 구현의 선정 소스이며 전체 빌드 의존성을 닫은 입력은 아니다. [inputs.json](inputs.json)에 경로·언어·커밋·해시·모듈 연결을 고정했다. 첫 입력 목록도 [inputs-v1.json](history/inputs-v1.json)에 보존했다.

Effect는 LiveStore의 잠금 파일에 적힌 **4.0.0-rc.113**을 사용했다. [공식 배포 메타데이터](https://registry.npmjs.org/effect/4.0.0-rc.113)와 배포 archive의 SHA-512 무결성·SHA-256, 선정 소스/라이선스/패키지 파일 474개의 SHA-256을 [effect-source.lock.json](effect-source.lock.json)에 기록했다. 제공되지 않은 Git 커밋은 추정하지 않았다. 패키지를 설치하거나 실행하지 않았다.

`Stream.ts`는 **615,757바이트**다. 사용자가 선택한 대로 `livestore-effect` 실행에만 `--max-file-bytes 1048576`을 적용했다. 기본 공개·공통 입력은 모두 524,288바이트 한도를 유지한다.

## 자동 연결 결과

| 공개 양성 판정 | 직전 기본 입력 | 현재 기본 입력 | 현재 확장 입력까지 반영 |
| --- | ---: | ---: | ---: |
| 조건부 호출 후보 | 30 | 30 | 30 |
| 조건부 데이터 반환 | 0 | 0 | **1** |
| 인자 전달만 확인 | 2 | 2 | 2 |
| 미확정 | 5 | 5 | **4** |
| 합계 | 37 | 37 | 37 |

기본 96파일만으로 얻은 판정은 직전과 같다. 마지막 열은 추가 소스가 포함된 실행을 합친 결과이므로 동일 입력에서 코드 변경만으로 얻은 개선율로 해석하면 안 된다. [comparison.json](comparison.json)에 사례별 결과와 집계 기준이 있다.

Bevy에서는 `messages.rs:140`의 `MessageInstance.message` 저장과 `iterators.rs:97`의 `Some(item)` 반환이 같은 `Messages.messages_b.messages[*].message` 경로로 연결됐다. `message_id`도 별도 필드로 보존했다. 명시적인 `mod`·`use`·재노출, `Self`·associated 함수, 소스에 있는 Deref, slice 조회·iterator chain·Option 반환을 따라간 결과다. [저장 소스](https://github.com/bevyengine/bevy/blob/29fe519f32a14503c8c0fab0baacafe76842bb76/crates/bevy_ecs/src/message/messages.rs#L138), [반환 소스](https://github.com/bevyengine/bevy/blob/29fe519f32a14503c8c0fab0baacafe76842bb76/crates/bevy_ecs/src/message/iterators.rs#L87), [평가](results/bevy-modules.evaluation.json).

동일한 Messages 인스턴스, cursor 범위 포함, 비어 있지 않은 iterator, 등록·읽기 순서 등의 조건은 남는다. 반환 관계에는 `returned_value_only_not_callback_execution`이 붙는다. 표준 연산의 의미는 [Iterator](https://doc.rust-lang.org/std/iter/trait.Iterator.html), [Option](https://doc.rust-lang.org/std/option/enum.Option.html), [slice](https://doc.rust-lang.org/std/primitive.slice.html) 계약을 대조했으며 런타임 검증과 구분한다.

glTF의 `loader/mod.rs:1754` 최종 메서드 호출 사실도 이제 추출된다. 함수별 192사실 한도 안에서 함수 자신의 저장·호출을 확장된 하위 호출과 단순 인자 전달보다 우선하도록 바꿨다. 다만 호출 대상은 여전히 `unresolved:...:extension`이므로 **등록과 호출은 연결되지 않았다**.

## 범용 구문 개선과 회귀

저장소 이름이나 이벤트 API 이름에 따른 특수 규칙은 추가하지 않았다. 공통 관계 모델과 언어별 값 해석을 보완했다.

| 변경 | 확인한 동작 | 추가 사례 |
| --- | --- | ---: |
| Rust 모듈·자료 반환 | 선택한 crate의 명시된 모듈/import, wrapper 저장, slice·iterator·Option 반환, 같은 매개변수와 다른 매개변수 구분 | 6 |
| Scala State·배열 | case class 생성·기본값·명명 인자·copy, 매개변수 없는 메서드, Array 적용 구문, ref 저장/조회 | 6 |
| TypeScript 모듈·캡처 | namespace import·재노출과 실제 객체 메서드의 캡처 환경, 서로 다른 factory 할당 구분 | 4 |
| TypeScript 사실 한도 | 앞선 무관한 호출이 많아도 자기 함수의 최종 저장값 호출 보존, 다른 필드 분리 | 2 |
| TypeScript 타입 구문 | `in/out`와 명시적 타입 선언 복구, 실행 본문·문자열·정규식 보존 | 5 |
| TypeScript static | 클래스의 정적 필드 저장과 인스턴스 필드 분리 | 2 |
| 합계 | 기존 140사례를 유지하며 추가 | **25** |

현재 회귀는 85소스 파일·78분석 실행·165사례다. 호출 양성 91, 데이터 반환 양성 3, 구조적 무연결 67, 미확정 유지 4로 집계한다. 데이터 반환 양성의 반례는 같은 종류의 양성 대조가 통과해야 인정한다. [사례](../regressions/cases.json), [결과](../regressions/results/evaluation.json).

Scala의 배열 원소 적용을 객체의 멤버 메서드 호출로 잘못 다루던 오연결도 제거했다. Array 원소에는 원소 호출 사실만 만들고, `member_invoke`는 정적 멤버 key일 때만 만든다. `State.copy`는 새 객체를 유지하며 변경하지 않은 필드만 전달한다. [Scala case class 계약](https://docs.scala-lang.org/tour/case-classes.html).

Effect 입력 9파일은 사용 중인 Tree-sitter 문법이 타입 구문을 온전히 읽지 못했다. TypeScript에서만 타입 매개변수 modifier와 줄 시작의 명시적 타입 객체 본문을 바이트·개행 위치를 유지해 가린 대체 트리를 사용한다. 대체 트리 전체에 오류가 없을 때만 채택하고 원문은 보존한다. 최종 선정 입력의 파싱 오류는 0이지만, 복구 파일의 사실에는 `generic_type_constraints_unproven`을 남겼다. 이는 타입 검사·컴파일 성공이 아니다. [타입 매개변수 variance 구문](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-4-7.html), 구현 [ts_syntax.py](../../ts_syntax.py).

중간 실패도 보존했다. [Rust 첫 측정](rust-round1/evaluation.json)은 match arm의 최종 반환 위치 누락으로 14/15, [Scala 첫 측정](scala-round1/evaluation.json)은 배열 원소의 멤버 호출 오인으로 11/12였다. 원인을 수정한 뒤 전체 165사례를 다시 측정했다. 중간 결과의 구현 해시는 당시 코드에 해당하며 최종 코드 해시와 다를 수 있다.

## 소스에서 확인했지만 자동 연결이 남은 경로

| 경로 | 원문에서 확인한 구현 | 자동 분석의 남은 경계 |
| --- | --- | --- |
| Bevy observer | 기본 runner와 사용자 system의 wrapper·World/Entity 경로 | trait·component 조회에서 정확한 World와 observer Entity 유지. 임의 custom runner를 기본 runner와 합칠 수 없음 |
| Bevy SystemId | Entity에 등록된 boxed system의 조회·take·재삽입 경로 | Entity 식별과 실제 trait 구현·이동된 system의 동일성 |
| Bevy glTF | 공유 registry의 Arc와 load별 Vec/Box 복제가 다름 | 복제된 handler 객체의 별도 기원과 trait dispatch. 최종 호출 사실 추출만으로 해결되지 않음 |
| Monix JVM·Scala 2 | Atomic 매크로 생성, AtomicAny → BoxedObject → VarHandle get/CAS 본문 | 매크로 확장·implicit builder 선택·상속 구현 및 VarHandle 저장 대상 |
| Monix JVM·Scala 3 | inline 생성과 JVM의 동일한 저장 구현 | implicit builder·익명 구현 선택과 JVM 필드 경로 |
| Monix JavaScript·Scala 2 | 매크로 생성, AtomicAny.ref 읽기·비교·교체 | 매크로 생성 객체와 State 저장의 연결 |
| Monix JavaScript·Scala 3 | inline 생성, AtomicAny.ref 읽기·비교·교체 | implicit builder와 익명 구현의 정확한 선택 |
| LiveStore Queue | 아래의 MutableList 저장·Stream 소비 본문 | Effect generator/yield, 반환 closure 환경, 내부 thunk·고차 함수 인자 연결 |
| LiveStore RPC | 아래의 clientId Map·실제 응답 callback 본문 | Protocol 생성·Effect 실행을 거친 함수 인자·캡처 환경 연결 |

Monix 네 조합은 모두 구문 오류 없이 선정 소스를 분석했지만 `scala-state-subscriber`는 네 실행에서 모두 `unresolved`다. Scala 2·3 컴파일러나 JVM·JavaScript 런타임을 실행하지 않았다. Atomic 연산의 성공·실제 동시성·Future/Ack도 검증하지 않았다. [JVM/2](results/monix-jvm-scala2.evaluation.json), [JVM/3](results/monix-jvm-scala3.evaluation.json), [JavaScript/2](results/monix-js-scala2.evaluation.json), [JavaScript/3](results/monix-js-scala3.evaluation.json).

LiveStore의 두 경로는 이제 외부 구현 원문까지 확보했으므로 “외부 본문이 없다”는 상태와 구분한다. 다음은 소스 검토로 확인한 흐름이며 자동 연결 성공으로 집계하지 않는다.

- **Queue**: `node.ts:608–609`의 queue를 Map에 보관하고 `229–232`에서 `Queue.offer`로 전달한다. Effect `Queue.ts:646–670`은 `self.messages`에 append하고, `MutableList.ts:190–199`는 tail 배열에 push한다. `node.ts:625–628`의 `Stream.fromQueue`는 Effect `Stream.ts:1137–1138` → `Channel.ts:1242–1244` → `Queue.takeAll/takeBetween` → `MutableList.takeN`의 자료 반환으로 이어진다.
- **RPC**: Effect `RpcClient.ts:868–891`의 `Protocol.make`는 `withRunClient`를 사용한다. `unstable/rpc/Utils.ts:68–120`은 `clientWrites` Map을 만들고 실제 write 함수를 주입한다. `clientId`는 `clientWrites.get`의 키이며 조회된 handler를 호출한다. 아직 등록되지 않은 client는 별도 buffer에 쌓았다가 등록 시 비운다. request별 엔트리를 찾는 `requestId`는 이 client 선택과 별개다.

관련 경로·바이트는 [Effect 잠금](effect-source.lock.json), 실제 제공 입력은 [확장 목록](inputs.json)에 있다. 두 참고 쌍의 자동 판정은 여전히 `argument_transfer_only`다. [Effect 포함 평가](results/livestore-effect.evaluation.json).

## 검증과 재현

작업 디렉터리는 상위 `language-corpus`다. Python 3.14.5·macOS arm64와 기존 문법 의존성을 사용했다. 환경 준비는 [전체 보고서](../README.ko.md)를 따른다.

```sh
.venv/bin/python remaining-routes/prepare.py --sources /tmp/codemap-language-repro-sources
.venv/bin/python regressions/run.py
.venv/bin/python manage.py measure --sources /tmp/codemap-language-repro-sources
.venv/bin/python remaining-routes/run.py --sources /tmp/codemap-language-repro-sources
.venv/bin/python remaining-routes/verify.py --sources /tmp/codemap-language-repro-sources
```

[prepare.py](prepare.py)는 고정 커밋과 archive 무결성을 확인해 입력만 준비하며 설치·빌드 스크립트를 실행하지 않는다. [run.py](run.py)는 입력 해시와 한도를 확인하고 여섯 변형을 별도 실행한다. [verify.py](verify.py)는 기존 검증기에 더해 이전 단계 보존본, 추가 입력·의존성, 현재 구현 해시, 데이터 반환 분류와 선택한 한도를 대조한다.

실측은 `/tmp/codemap-event-poc-venv/bin/python`으로 수행했다. 현재 기본·공통 분석 37개, 회귀 분석 78개, 확장 분석 6개로 원시 결과는 총 **121개**다. 기본 167개 대조 판정은 직전 단계와 같고 회귀 165개가 통과했다. 소스·구현 해시와 결과의 대조는 [verification.json](verification.json), 기존 사례·체크포인트의 검증은 [기본 검증 결과](../regressions/results/verification.json)에 있다.

모든 관계는 `conditional_source_relation`, `concrete_instance_proven=false`, `event_classification=not_inferred`다. 기본 공개 입력 일부의 구문 복구 한계는 [전체 보고서](../README.ko.md)에 유지했다. 확장 입력도 요약 4회, 함수·관계·값 대안의 기존 한도를 유지하며 Bevy에서는 함수 사실 상한, Effect에서는 반환 closure 환경 미확정 진단이 남는다. 26개 공개 실행 시나리오는 미실행이고, 이미 보며 보완한 표본이므로 독립 평가나 언어 전체의 완전성 근거가 아니다.

남은 구현 대상은 **Rust 세 경로, Monix 한 경로의 네 변형, LiveStore 두 최종 소비 경로**다. 그 뒤에도 별도 저장소 평가, 전체 후보의 정밀도, 런타임 인스턴스·수명·동시성 검증과 제품 통합은 별도로 필요하다. 이번 변경은 독립 PoC 안에 한정했으며 Rust 제품·MCP/CLI는 변경하지 않았다.
