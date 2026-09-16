# 18개 언어 미탐지 구문 보완 결과

2026-09-16에 앞서 제시한 **미탐지 예시 18개를 모두 저장 지점에서 최종 호출까지 연결했다.** 기존 탐지 예시 18개도 유지됐다. 체크포인트 이후 사례를 단계적으로 추가해 현재 **247사례가 모두 통과**했다. 대상 프로그램을 실행한 결과가 아닌 조건부 소스 관계다. 확장 입력까지 포함한 공개 결과는 호출 후보 31·자료 반환 1·인자 전달 2·미확정 3다. [전체 보고서](../README.ko.md)와 [잔여 경로 보완 결과](../remaining-routes/README.ko.md)에 범위·실패 분석·한계를 기록했다.

## 비교 기준과 결과

질문은 “앞서 언어별로 제시한 미탐지 구문을 해결하면서 다른 저장값을 잘못 연결하지 않는가”다. 독립 Python PoC를 수정했으며 제품 Rust 코드·MCP 계약은 변경하지 않았다. 초기 구문 분석은 Sol·medium이 수행했고, 후속 공개 경로의 실패 분석에는 사용자 지시에 따라 Astra·max를 2회 사용했다. 구현·사례 구성·측정은 메인 스레드에서 수행했고 codemap-search는 사용하지 않았다.

| 구분 | 사례 수 | 결과 |
| --- | ---: | --- |
| 기존 탐지 예시 | 18 | 18개 탐지 유지 |
| 기존 미탐지 예시 | 18 | 18개 최종 호출까지 조건부 탐지 |
| 추가 호출 양성 대조 | 89 | 89개 조건부 호출 탐지 |
| 자료 반환 양성 대조 | 3 | 3개 조건부 반환, 호출과 별도 집계 |
| 자료 소비 양성 대조 | 6 | 객체 쓰기·읽기·조회 키 소비, 호출과 별도 집계 |
| 구조적 오연결 반례 | 105 | 105개 무연결, 같은 종류의 지정 양성 대조도 통과 |
| 미확정 유지 대조 | 8 | 해석 근거가 부족한 경로를 연결하지 않음 |
| 합계 | 247 | 247개 기대 결과 충족, 파싱 오류 0 |

원문은 108파일이며 C·TypeScript·Rust의 여러 파일과 Scala/Java 혼합 소스를 함께 분석하는 사례를 포함해 분석 프로세스는 100개다. 수치는 정의한 소스 쌍의 수다. 여러 관계 후보가 한 쌍에 나와도 사례 하나로 집계한다. `stored_value_argument`나 `stored_value_return`만 있는 경우 최종 호출 탐지로 인정하지 않는다. 자료 반환은 `semantic_kind=data_return`, 객체 쓰기·조회 키 소비는 `semantic_kind=data_consumption`으로 관계 종류와 끝점을 별도로 대조한다. 반례는 연결이 없더라도 지정 양성 대조가 실패하면 통과하지 못한다.

후속 29개는 C++ 생성자·포인터 별칭 4개, C# atomic·getter 2개, Java reflection 2개, PHP 참조 cache·callable 생성 2개, Swift 제네릭 Bag 2개, Rust source-backed Deref·고유 메서드 4개, TypeScript object spread·속성·import·별도 binding 13개다. 기존 111사례와 원본 예시를 유지했고 새 사례만 추가했다.

165사례 단계에서 추가한 25개는 Rust 모듈·데이터 반환 6개, Scala State/copy·Array 6개, TypeScript 모듈·캡처 4개, 사실 한도 2개, 타입 구문 복구 5개, static 분리 2개다. 기존 140사례의 기대값·원문을 변경하지 않았다. [이전 단계 보존본](../remaining-routes/baseline/manifest.json)과 [현재 검증](../remaining-routes/verification.json)에서 대조할 수 있다.

196사례 단계에서는 TypeScript 반환 closure·call/apply·prototype와 모듈 예산·데이터 소비 18개, Scala implicit·Java VarHandle·Set 복사 값 8개, Rust Box·Some pattern·이름 가림 5개로 **31사례**를 더했다. 서로 다른 prototype 생성 환경, final 필드·잘못된 필드 타입, generic Box 가림을 검사한다. Set 복사는 같은 위치의 기존 원소와 새 원소를 구분하고 새 매개변수 값이 최종 관계에 남는지 확인한다. [직전 165사례 보존본](../remaining-routes/history/165-cases/manifest.json)을 그대로 유지했다.

이어서 namespace 안의 배열 타입과 중첩 타입 해석 5사례를 추가했다. 한 파일에 이름이 같은 타입이 있어도 namespace를 구분하고, 여러 객체 타입이 섞인 union에서 하나를 임의 선택하지 않는다. 이 변경 직전의 [196사례 검증 보존본](../remaining-routes/history/196-cases/manifest.json)도 남겼다.

201사례 상태를 `33efda1f7`에 먼저 커밋한 후 33개를 추가했다. Rust 포인터 기원·tuple struct·TypeId 키와 이름 가림 20개, TypeScript generator 재개·완료·native yield 위임 11개와 배열 pop 2개다. 새 양성 18개는 `storage_to_invocation`과 필수 조건을 원시 결과에서 대조한다. 다른 객체·Map·타입·generator·배열과 정수 주소, 미해석 타입 인자와 iterator 위임을 구분한다. generator는 직선형 본문과 확인된 native generator 위임을 지원하며, 일반 Effect iterator 실행기나 Bevy component 저장소 전체를 해석하지 않는다.

234사례 검증 상태 `777e8330f` 이후 TypeScript 13사례를 추가했다. 저장된 일반 함수의 receiver와 다른 함수·객체 4개, 모듈 변수 재할당 전후의 live binding 4개, generator의 명시적 callback 호출과 다른 callback·미호출 callback·인자 순서 5개다. 추가 결과는 호출 양성 4·조회 키 소비 양성 2·구조적 무연결 7이다. generator 본문과 반환 메서드의 실행은 여전히 조건이며 런타임 실행으로 판정하지 않는다.

## 언어별로 무엇을 고쳤는가

아래 링크는 수정 전후에 같은 바이트로 분석한 미탐지 예시다. `missed` 디렉터리 이름은 수정 전 분류를 보존한 것이며 현재 기대값은 모두 `connected`다.

| 언어 | 확인한 구문과 보완 |
| --- | --- |
| Assembly | [rbx 간접 호출](examples/assembly/missed/example.asm): 주소 테이블 근거를 레지스터별로 보존한다. 다른 레지스터의 로드와 섞지 않고 덮어쓰기·좁은 폭의 쓰기에서 무효화한다. |
| C | [static 전역 함수 포인터](examples/c/missed/example.c): 소스 파일에 귀속된 전역 슬롯을 수집한다. 지역 변수·매개변수와 다른 파일의 같은 이름 static을 구분한다. |
| C++ | [this 캡처 lambda](examples/cpp/missed/example.cpp): 캡처한 receiver·값을 callable 값에 보존하고 lambda가 호출될 때 본문 사실을 확장한다. 선언만 한 lambda는 확장하지 않는다. |
| C# | [event +=](examples/csharp/missed/example.cs): 필드형 event를 수집해 += 저장과 -= 제거를 구분한다. 호출 후보에는 구독 포함 여부와 제거 조건을 남긴다. |
| Dart | [record 구조 분해](examples/dart/missed/example.dart): 위치별 저장과 패턴 바인딩을 연결하고 `_`는 버린다. |
| Go | [reflect.ValueOf → Call](examples/go/missed/example.go): import 경로가 표준 reflect인지 확인해 감싼 값을 보존한다. import 별칭과 함수 요약을 통과해도 최종 Call이 원래 슬롯을 가리킨다. |
| Groovy | [getter property](examples/groovy/missed/example.groovy): 해당 owner의 유일한 인자 없는 getter 반환값을 property 읽기에 적용한다. |
| Java | [cb::run → relay.run](examples/java/missed/example.java): 수신 객체와 참조한 메서드를 보존한다. Runnable 또는 입력에서 확인한 단일 추상 메서드 인터페이스의 호출일 때만 참조를 확장한다. |
| JavaScript | [Reflect.get](examples/javascript/missed/example.js): 이름이 가려지지 않은 내장 Reflect의 정적 property 조회를 원래 슬롯으로 돌려준다. |
| Kotlin | [property getter](examples/kotlin/missed/example.kt): getter를 owner·property별로 수집해 기존 함수 요약의 반환값을 읽기 경로에 적용한다. |
| Lua | [dot 저장 → bracket 조회](examples/lua/missed/example.lua): 같은 정적 문자열 키를 일치시킨다. 알려진 map의 dot/bracket 접근도 동일하게 정규화한다. |
| PHP | [상수 동적 property](examples/php/missed/example.php): `$name`의 값으로 property를 선택한다. 지역 변수와 같은 이름의 객체 필드도 구분한다. |
| Python | [getattr](examples/python/missed/example.py): 모듈·함수의 이름 가림을 확인한 뒤 정적 속성명을 슬롯으로 해석한다. 호출 뒤에 나오는 지역 선언도 가림 판단에 포함한다. |
| Ruby | [instance_variable_get](examples/ruby/missed/example.rb): 정적 인스턴스 변수명을 보존한다. 입력에 사용자 재정의가 있으면 그 함수의 요약을 사용한다. |
| Rust | [Box&lt;dyn Fn()&gt;.as_ref](examples/rust/missed/example.rs): 표준 Box와 호출 가능한 내부 타입을 확인해 참조 경로를 유지한다. 같은 이름의 사용자 Box에는 적용하지 않는다. |
| Scala | [tuple 구조 분해](examples/scala/missed/example.scala): 튜플 값을 원소별 슬롯에 저장하고 같은 위치의 바인딩으로 전달한다. |
| Swift | [tuple 다중 대입](examples/swift/missed/example.swift): 좌변을 감싼 구문을 풀고 각 원소를 정확한 대상 위치에 저장한다. |
| TypeScript | [getter callback](examples/typescript/missed/example.ts): 생성자 parameter property에 저장한 함수를 getter 반환값과 지역 별칭을 거쳐 호출 지점으로 연결한다. |

이벤트 API 목록이나 저장소 이름에 따른 규칙을 추가하지 않았다. 공통 저장·호출 연결 모델을 유지하면서 언어의 값 해석 단계를 보완했다. 튜플 위치에는 구분되는 정적 키를 사용하며, 다른 위치나 다른 정적 숫자 키를 동적 키처럼 합치지 않는다.

## 반례가 확인하는 경계

다른 필드·event·전역·테이블·튜플 위치·캡처 receiver를 구분했다. Java는 `toString()`을 함수형 인터페이스 호출로 간주하지 않으며, C++는 호출되지 않은 lambda 본문을 연결하지 않는다. Ruby의 사용자 조회 함수가 다른 필드를 반환할 때는 그 반환 경로만 연결했다. JavaScript·Python은 지역/모듈 이름 가림, Go는 import 가림, Rust는 사용자 Box를 대조했다.

미확정 유지 8개는 기존 C 매개변수의 전역 이름 가림, PHP 런타임 property 이름, Python·JavaScript의 뒤늦은 지역 선언 4개와 Rust의 미해석 TypeId 인자, generator의 미확인 iterator 위임·분기 안의 조기 return·throw 4개다. 이 사례들은 무연결이 증명됐다는 통계로 합치지 않는다. C# 제거는 `removal_may_prevent_call` 조건의 보존을 검사하며 실제 구독 해제 이후 호출 여부를 실행 검증한 것으로 주장하지 않는다.

## 여전히 제한되는 범위

- Getter는 입력에서 소유 타입과 적용할 getter를 확인하는 범위다. 상속·동적 descriptor·임의 proxy, setter 호출 전파 전체를 지원한 것은 아니다.
- 내장 조회는 입력에서 확인한 이름 가림과 정적 키를 기준으로 한다. Python의 기본값 인자가 있는 getattr, 임의 동적 속성명, Ruby의 외부 monkey patch, Reflect 자체의 런타임 교체 등은 추가 의미 분석이 필요하다.
- Java의 일반 메서드 참조 타입 추론·상속된 함수형 인터페이스 전체, Go의 임의 reflection 연산, Rust의 임의 AsRef/trait 및 wrapper 체인은 범위 밖이다. Rust Deref는 명시된 표준 trait와 실제 본문의 단일 내부 컨테이너 반환만 보완했다.
- C++는 명시적 this·단순 값 캡처를 처리한다. 참조·기본·초기화·객체 복사 캡처와 수명은 추가 작업이 필요하다. C는 파일별 static을 처리하며 외부 링크 변수의 동일성을 증명하지 않는다.
- 이름 있는 record/tuple, 임의 패턴·가변 개수·모든 중첩 형태, C# 사용자 add/remove accessor와 static event, Assembly의 전체 명령·ABI·제어 흐름은 미지원 또는 미검증이다. 튜플은 최대 8원소다. TypeScript 실제 인자 해석 깊이는 6, Scala/혼합 Java 소스 호출 깊이는 10으로 제한하며 모든 closure·macro를 처리하지 않는다.
- 모든 관계는 `certainty=conditional_source_relation`, `concrete_instance_proven=false`, `event_classification=not_inferred`다. 실제 인스턴스·등록 순서·수명·이벤트 전달·프레임워크 전체 정밀도는 입증하지 않았다. 대상 코드의 컴파일·테스트·실행은 하지 않았다.

이번 예시와 반례는 수정하면서 확인한 회귀 표본이다. 독립 holdout 평가나 언어 전체의 완전성 증거로 사용하지 않는다.

## 재현과 보존본

작업 디렉터리는 상위 `language-corpus`다. 의존성과 Groovy 문법 빌드는 [전체 보고서](../README.ko.md)의 환경 준비 절차를 따른다.

```sh
.venv/bin/python regressions/run.py
.venv/bin/python regressions/run.py python ruby php lua
.venv/bin/python manage.py measure --sources /tmp/codemap-event-language-sources --output /tmp/codemap-event-language-results
```

[cases.json](cases.json)은 입력 해시, 저장·호출 위치, 기대 결과, 양성 대조를 정의한다. 기대값은 평가기에만 전달되며 분석기는 소스와 언어만 받는다. [현재 결과](results/evaluation.json), [원시 분석과 코드 해시](results/analysis/), [무결성 대조](results/verification.json)로 확인할 수 있다.

수정 전 36개 결과는 [baseline/evaluation.json](baseline/evaluation.json)에, 이전 분석기와 공개 측정 결과는 [보완 전 보존본](../history/before-language-gap-fixes/manifest.json)에 있다. 보존본의 원본 상대 경로는 manifest에 기록했다. 원래 36개 예시의 바이트와 기존 공개 입력 잠금은 이번 수정에서 바꾸지 않았다. Python 3.14.5·macOS arm64에서 측정했으며 다른 실행 환경은 검증하지 않았다.
