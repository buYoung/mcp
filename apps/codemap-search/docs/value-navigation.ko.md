# 값 관계 탐색과 함수 요약

`read`, 내용 조회 방식의 `grep`, `search`는 관련 코드에서 인자·반환값·클로저·객체 필드·콜백 사용 관계를 자동으로 보여 준다. 패키지나 이벤트 API 이름이 알려져 있지 않아도 소스에서 확인할 수 있는 단순 전달 함수를 조합한다. 이 문서는 색인 형식 `v33-conditional-outcomes`의 범위와 출력 계약을 설명한다.

| 항목 | 동작 |
| --- | --- |
| `read` / `grep`의 `view=full`, `view=relations` | 반환된 원문 위치를 기준으로 관련 함수 요약을 조회한다. |
| `view=source`, `view=definitions` | 값 관계를 계산하지 않는다. |
| `grep`의 `files_with_matches`, `count` | 파일 또는 개수만 반환한다. |
| `search` | 상위 결과의 실제 일치 함수에서 시작한다. 경로만 일치한 대체 심볼은 분석하지 않는다. 선택한 작업공간 범위를 유지한다. |
| `search.caller_context=false` | 호출 문맥과 값 관계 요약을 생략한다. |
| `include_events=false`, `[event_navigation].is_enabled=false` | 이벤트 지도를 생략한다. 소스의 일반 값 관계는 계속 사용할 수 있다. |
| `debug=true` | 내부 분석 제한과 상세 근거를 표시한다. 시간·메모리 한도나 원문 출력은 변경하지 않는다. |
| `unresolved=count` | 미해결 관계·진단의 이름 목록을 개수로 줄인다. 원문 출력은 유지한다. |

## 파일별 출력과 위치 표기

`read`, 내용 조회 방식의 `grep`, `search`의 상세 결과는 기본적으로 `# codemap-search` 아래에 `## 1. src/user.ts`처럼 파일을 묶는다. 각 파일에 `### fn active`, `### impl ImplementationIndex`처럼 종류와 이름으로 선언별 제목을 만들고, 마지막에 `### results`를 한 번만 배치한다. 메서드는 소속 `impl/class` 아래에 유지한다. `read`는 한 파일을 조회하며, `grep`의 파일 순서와 페이지 범위는 원문 결과 순서를 따른다.

| 항목 | 표시 규칙 |
| --- | --- |
| 함수·메서드 | 각 선언 아래에 해당 선언의 호출자·호출 대상·참조 상수를 붙인다. 반복 일치한 심볼은 한 번 표시한다. |
| `class`·`struct`·`impl` | 부모와 멤버의 계층을 유지한다. 직접 읽거나 검색된 선언에 상세 관계를 우선 배정하고 다른 멤버는 선언만 간결하게 표시한다. |
| 같은 파일의 위치 | `parseUser — L42`, 선언 범위는 `L12-30`처럼 표시한다. |
| 다른 파일의 위치 | `readConfig — src/config.ts:18`처럼 저장소 기준 상대경로를 유지한다. 허용된 저장소 밖 파일은 절대경로를 사용한다. |
| 미해결 위치 | 이름·이유를 표시하되 정의 경로를 추측하지 않는다. 실행 가능한 `read` 제안에는 파일 경로를 유지한다. |
| 파일 간 공유 관계 | 이미 출력한 동일 이벤트 경로·값 관계는 앞선 파일 묶음을 안내한다. |
| 표시 선택 | `source`는 기존 원문만 반환한다. `definitions`는 파일별 선언, `relations`는 같은 선언별 제목 아래에 관계를 반환한다. 파일 목록·개수 조회는 기존 형식을 유지한다. |

원문과 파일 제목 공간을 먼저 확보한 뒤 전체 심볼·관계 예산을 파일들이 나눠 사용한다. 원문 구간을 심볼별로 복제하거나 파일마다 전체 예산을 새로 부여하지 않는다. 색인 문맥을 펼치는 파일은 최대 8개이며, 파일 한도·상세 생략·부분 관계를 명시한다. 값 관계의 시간·연산·소스 확인 한도도 파일 묶음 전체에서 공유한다. `grep`의 `offset`·`head_limit` 의미는 바뀌지 않는다.

`Next: read`는 문맥 예산에서 생략된 선언 중 아직 원문을 보여 주지 않은 위치에만 붙이며, 한 응답에서 최대 3개이다. `full`의 원문 범위, `grep` 문맥 행, 이미 제안한 위치는 다시 제안하지 않는다. 긴 줄 제한이나 본문 크기 제한으로 원문 자체가 빠진 위치는 표시된 원문으로 취급하지 않는다. 원문 검색의 페이지 이동과 본문 확장 실패 안내는 별도로 유지한다.

폴더 `overview`는 기존 `## Files` 틀을 유지한다. 함수에는 매개변수 타입·반환 타입을 표시하고, 구조체 필드와 실제 `impl` 메서드는 소속 선언 아래에 묶는다. 시그니처는 색인 해시와 일치하는 소스에서 보강한다. 소스가 달라졌으면 기존 심볼과 오래된 색인 안내를 표시한다. `search`도 선언과 관계를 먼저 보여 주고 파일당 `results`를 한 번만 출력한다. 검색 순위·원문 조각 선택·상위 상세 결과와 나머지 순위 목록의 구분은 유지한다. 정확한 `event_key` 조회는 기존 이벤트 지도 형식을 유지한다.

Rust 공개 범위는 선언마다 구분한다. `struct`와 메서드는 원문의 `pub`, `pub(crate)`, `pub(super)` 등을 표시하며, `impl` 블록에는 공개 여부를 붙이지 않는다. trait 멤버는 trait 계약에서 정하는 멤버로 표시한다. `len`, `clone`, `is_empty`에 대한 이름 기반 제외 정책은 적용하지 않는다.

## 근거 표시

| 출력 | 의미 | 해석의 경계 |
| --- | --- | --- |
| 기본 관계 / 디버그의 `[source]` | 지원하는 소스 구문에서 확인한 전달·사용 위치 | 함수 값 전달은 호출 증거가 아니다. |
| `[model]` | 내장 의미 모델을 적용한 관계 | 원문에서 생성이 확인된 JavaScript `Map`의 `set`·`get`·`delete`, 아래 Rust JSON 객체 가드의 조건부 결과를 다룬다. 같은 메서드 이름의 임의 객체에는 적용하지 않는다. |
| `[candidate]` | 연결 후보 또는 값이 불확실한 구간 | 같은 저장소와 정적 키라도 저장 순서, 덮어쓰기, 실행 시점을 보장하지 않는다. |
| `[unresolved]` | 더 진행하지 못한 이유와 원문 위치 | 외부 구현 부재·지원하지 않는 구문·변경된 소스·분석 제한 등을 구분한다. |

관계가 없으면 빈 `Value relationships` 절을 만들지 않는다. 기본 출력은 같은 파일 경로를 한 번만 표시하고 반복되는 익명 함수 전달의 줄 번호를 합친다. 반환 의미가 불확실한 메서드 체인의 중간 과정과 분석기 내부 사유는 생략한다. `debug: true`를 주면 상세 관계와 `Analysis diagnostics`를 기존 출력 한도 안에서 확인할 수 있다. 실제 원문 잘림, 오래된 색인, 호출 대상 불확실성은 기본 출력에도 남는다. 진단을 숨기는 것은 분석의 완전성을 뜻하지 않는다. 동일 위치·이유의 값 분석 진단은 파일 간에 중복 출력하지 않는다. 공유 시간·연산·값·관계 한도에 도달하면 추가 파일의 값 분석을 시작하지 않고, 응답 상단에 부분 분석 안내를 한 번 표시한다. 미해결 이유를 재조회만으로 해소할 수 있다고 안내하지 않는다. 문자열이나 주석에 함수 이름이 있다는 사실만으로 호출을 연결하지 않는다.

`_calls`는 직접 호출 해석 결과다. `Value relationships`는 인자와 반환 객체를 따라 확인한 문맥별 결과이므로, 직접 조회에서 미해결이던 `.on(...)`의 정의가 이 절에 나타날 수 있다.

## Rust JSON 객체 가드의 조건부 결과

다음 조합은 호출 이름뿐 아니라 수신 타입과 의존성의 정체를 확인한 뒤 연결한다.

```rust
let object = arguments.as_object().ok_or_else(|| {
    (-32602, "Invalid search arguments: expected an object.".into())
})?;
```

함수가 `Result<_, (i64, String)>`를 반환하고 오류 튜플의 타입이 확인되면 기본 출력은 다음과 같다. 줄 범위는 실제 표현식의 위치를 사용한다.

```text
[model] JSON 객체가 아닌 입력 → Err((-32602, …)) 조기 반환 · L6–L11
```

이는 조건부 계약이다. 실제 입력이 객체가 아니었다거나 클로저가 실행됐다는 관측 결과가 아니다. 변경되지 않는 지역 변수의 `Value::Object(...)` 생성이 확인되면 오류 관계를 만들지 않는다. 튜플의 첫 정수는 역할을 추정해 “오류 코드”라고 부르지 않는다.

- `serde_json::Value`의 직접 타입·참조 타입, 명시적인 가져오기 별칭과 지역 타입 별칭을 확인한다. Cargo의 의존성 이름 변경과 workspace 의존성도 확인한다.
- 저장소 내부 `Cargo.toml`과 `Cargo.lock`에서 crates.io의 `serde_json` 1.x를 확인한다. 경로·Git 의존성, 패치·소스 대체, 선택적·대상별 의존성의 불확실성은 모델 적용을 중단한다. 로컬 모듈·타입·일반 타입 매개변수로 이름을 가린 경우와 알 수 없는 가져오기·트레이트도 보수적으로 제외한다.
- 기존 분기·튜플 요약기로 오류 생성 클로저를 별도로 분석한다. 그 분석 중의 호출을 실제 호출자의 실행 사실로 합치지 않는다. 분기 결과가 다르면 공통으로 확인한 값만 남긴다.
- `?`의 오류 변환이 동일 타입임을 확인한 경우에만 튜플 값을 표시한다. 동적 튜플 요소나 사용자 오류 타입의 `From` 변환은 `Err(…)` 또는 `반환값 미확정`으로 남긴다. 클로저 반환 자체가 미확정이거나 `panic!()`처럼 반환 근거가 없으면 조건부 반환 모델을 보류한다. 현재 동일 타입 확인은 명시적인 표준 튜플 타입과 지원하는 튜플 생성 구문을 중심으로 한다.
- 조건·결과 행이 실제 출력됐을 때만 같은 익명 함수의 단순 전달 행을 기본 출력에서 줄인다. 다른 이름 있는 오류 생성 함수로 이어지는 관계는 조건부 후보로 유지한다. `debug:true`에서 전달 과정과 의존성·타입·호출 위치의 근거를 확인할 수 있다.

조건, 반환값, 조기 반환 여부, 위치, 확실성은 공통 분석 자료에 저장하고 출력 템플릿이 문장으로 바꾼다. 현재 이 API 조합의 의미 모델은 Rust만 지원한다. 임의의 `Option`·`Result` 체인, `match`·복잡한 패턴·프로시저 매크로·전체 Rust 타입 추론으로 확대해서 해석하면 안 된다. 일반 `?` 식의 성공 경로까지 모두 실행 분석하는 기능도 아니다. Cargo 근거 파일 읽기와 클로저 요약은 기존 100ms·파일·소스 바이트·연산·값·관계 한도를 함께 사용한다.

의미 모델의 근거는 [`Value::as_object`](https://docs.rs/serde_json/latest/serde_json/value/enum.Value.html#method.as_object), [`Option::ok_or_else`](https://doc.rust-lang.org/std/option/enum.Option.html#method.ok_or_else), [`?`의 오류 전파와 `From` 변환](https://doc.rust-lang.org/reference/expressions/operator-expr.html#the-question-mark-operator)이다.

현재 저장소의 앱 디렉터리에서 확인한다.

```sh
cd apps/codemap-search
cm search '{"query":"validate +arguments.rs","workspace_scope":"codemap-search"}'
cm read '{"file_path":"src/tools/search/arguments.rs","offset":1,"limit":43}'
cm grep '{"path":"src/tools/search/arguments.rs","pattern":"ok_or_else","-B":1,"-A":6}'
cm read '{"file_path":"src/tools/search/arguments.rs","offset":1,"limit":43,"debug":true}'
```

## 요약 범위

단순한 위치 기반 인자 전달, 반환값의 변수 바인딩, 연속된 소스 함수 호출, 변경되지 않는 클로저 캡처, 명시적 객체 필드 생성·대입·읽기를 다룬다. TypeScript/JavaScript의 반환 객체 메서드와 명시적 인스턴스 필드 초기화도 이 표현을 사용한다. 서로 다른 생성 지점의 인스턴스는 같은 타입·필드 이름만으로 합치지 않는다. 서로 다른 호출 문맥의 필드 쓰기·읽기는 순서가 확인되지 않으면 후보로 남긴다.

기본 `if/else`와 조건식은 확정된 불리언이면 선택된 경로만 따라간다. 불확정 조건은 분기 상태를 분리해 분석하며, 조건부 호출 대상은 `[candidate]`로 표시한다. 조기 반환 뒤의 원문을 실행 경로로 계속 조합하지 않는다. `&&/||` 등 단락 조건은 필요한 우변만 평가한다. 분기마다 다른 객체 변경은 확정된 필드 값으로 합치지 않으며, 같은 조건의 서로 배타적인 분기에서 발생한 컬렉션 저장·조회는 연결하지 않는다.

네이티브 튜플 생성·반환·정적 요소 조회와 지역 변수 분해를 지원한다. Rust 튜플 필드 대입은 해당 값의 새 버전으로 반영한다. 배열·객체 구조 분해를 튜플로 간주하지 않는다. 전개 패턴, 분해 선언을 실행하지 않은 전역 바인딩, 값 표현식 내부의 비지역 `return`, 초기화·패턴 바인딩이 있는 조건문 등은 여전히 제한되며 디버그 진단에서 확인한다. 분기 깊이와 후보는 각각 최대 8, 튜플 요소는 최대 32이며 기존 질의 시간·연산·값 한도도 계속 적용된다.

클래스 인스턴스 생성·필드 요약은 JavaScript/TypeScript의 확인된 구문으로 제한한다. 다른 언어의 생성자·프로퍼티·디스크립터·복사 의미는 미해결로 남긴다. Python 사전의 문자열 키 조회와 객체 속성 조회도 구분하며, 사전 키를 메서드로 연결하지 않는다. 프로토타입이나 클래스 멤버가 변경되면 해당 인스턴스 해석을 중단한다.

단순 전달 함수 검증 대상은 TypeScript, JavaScript, Rust, Go, Python, C, C++, Java, C#, Kotlin, Swift, Dart, Scala, Groovy, PHP, Ruby, Lua이다. Vue·Svelte·Astro의 스크립트는 원본 파일의 줄 번호와 해시를 유지하며 공통 요약을 사용한다. 이는 각 언어의 모든 구문이나 객체 모델을 해석한다는 뜻이 아니다.

파일 간 연결은 유일한 상대 ESM 이름 가져오기·명시적 재내보내기, 기존 소스 해석기가 확인한 Rust 함수, 같은 Go 패키지의 유일한 일반 함수에 한정한다. 외부 패키지, 경로 별칭, 동적 import, 모호한 내보내기 등은 정의를 추측하지 않는다.

다음 경계에서는 의미를 확정하지 않는다.

- 외부 함수에 전달한 값이 반환값으로 나온다는 가정, 콜백 인자를 즉시 호출한다는 가정.
- 재귀·깊은 호출, 반복·예외 제어, 가변 캡처, 중복 선언·오버로드, 전개·이름 지정 인자, 해석할 수 없는 기본 인자.
- 비동기·생성기·데코레이터·접근자, 상속·메타클래스 등 추가 실행 의미가 필요한 경우.
- C/C++·Go·Rust의 집합체·인스턴스 복사 및 참조 의미. 이 언어의 단순 값·함수 전달 요약을 객체 동일성의 증거로 확대하지 않는다.
- 셸과 PowerShell의 파이프라인·위치 인자 의미, Go의 빌드 조건, C/C++의 조건부 전처리·확장된 매크로 본문. 기존 선언·호출·매크로 탐색 기능과는 별도의 요약 한계이다.
- SQL·ASM의 고수준 함수 값 흐름. 기존 언어 색인과 원문 탐색은 유지된다.

객체 필드별 계약 비교와 자동 결함 판정은 이 기능에 포함하지 않는다.

## 진단·검색 순위·출력 제한

빈 선언 문맥은 구간에 선언이 없는 경우와 색인 제외·미지원 형식·파일 크기 제한·색인 후 변경을 구분한다. 비 UTF-8 입력은 계속 색인에서 제외하고, `read`의 원문 제공 동작은 유지한다. 이벤트 입력의 파일별 실패 사유는 최대 64개를 보존한다.

단일 식별자 또는 식별자 표기가 있는 질의는 기존 정확 일치 우선순위를 유지한다. 일반 복합 질의는 여러 질의 요소를 충족하는 결과에 더 높은 가중치를 주며, 긴 함수에서는 실제 근거가 있는 본문 창을 고를 수 있다. 일부 본문을 생략하면 실제 줄 범위와 다음 읽기 위치를 표시한다. 일반 읽기 제안은 최대 40줄이다.

MCP `search`의 미지원 인자는 검색 전에 `-32602` 오류로 반환한다. `path`, `limit`는 검색 범위·개수 제한으로 적용되지 않는다. 범위는 루트 `overview`에 나온 `workspace_scope`를 사용한다. 기존 `scope` 별칭과 `workspaceScope`·`includeEvents`·`eventKey`처럼 기존에 지원하던 정규화 표기는 유지한다. `query`, `caller_context`, `language_hint`, `extension_hint`는 문서에 나온 정확한 키를 사용한다. CLI의 일반 문자열 검색 옵션은 별도 계약이다.

| 제한 | 기본 상한 |
| --- | --- |
| 파일당 요약 표현식·바인딩 | 각각 4,096개 |
| 파일당 함수 요약 | 모듈 포함 256개 |
| 질의에서 확인하는 파일 | 24개 |
| 질의의 소스 신선도 확인 | 합계 8 MiB 및 기존 `max_file_size` |
| 질의 연산·값 | 각각 4,096개 |
| 호출 깊이 | 8단계 |
| 질의 경과 시간 | 100ms, 분석 단계 사이에서 확인 |
| 표시 관계 | 최대 128개, 기존 도구의 바이트 제한 안에서 추가 제한 |
| 재사용 요약 캐시 | 최대 32개 파일, 표현식·바인딩 합계 32,768개 |

요약은 색인 시 저장하고 요청한 파일만 지연 로드한다. 새 색인 세대는 별도 캐시를 사용한다. 요청 중에는 참조한 정의와 재내보내기 파일의 현재 해시를 확인한다. 파일이 변경·제외·삭제됐으면 이전 관계를 붙이지 않는다. 제한에 도달한 결과는 부분 결과이며, 부재의 증거가 아니다. 부하에 따라 추가 문맥의 양이 달라질 수 있다.

## 확인 예시

현재 저장소 루트에서 기존 검증 체계를 실행한다.

```sh
cargo test --locked --manifest-path apps/codemap-search/Cargo.toml --lib flow::
cargo test --locked --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests test_generic_value_relationships_persist_and_follow_live_views
cargo test --locked --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests test_composite_query_preserves_coverage_body_evidence_and_continuation
cargo test --locked --manifest-path apps/codemap-search/Cargo.toml --test e2e_tests test_live_diagnostics_distinguish_empty_excluded_and_stale_source
```

다음과 같은 사용자 정의 코드를 `bus.ts`에 두고 해당 디렉터리에서 조회할 수 있다.

```ts
const routes = new Map();
function connect(key) { return { on(handler) { routes.set(key, handler); } }; }
function notify(payload) {}
function setup() { const handle = connect('changed'); handle.on(notify); }
function dispatch() { const callback = routes.get('changed'); callback(1); }
```

```sh
cm read '{"file_path":"bus.ts","offset":4,"limit":1}'
cm read '{"file_path":"bus.ts","offset":4,"limit":1,"debug":true}'
cm grep '{"path":"bus.ts","pattern":"handle\\.on","view":"relations"}'
cm read '{"file_path":"bus.ts","offset":4,"limit":1,"view":"source"}'
cm read '{"file_path":"bus.ts","offset":4,"limit":1,"view":"relations","unresolved":"count"}'
```

기본 출력은 `notify · L3`을 포함한 콜백 호출 **후보**, 캡처·반환·저장 관계를 제공한다. `view=source`는 원문만 제공한다. 다른 `Map` 인스턴스나 다른 키로 조회하면 해당 콜백 연결은 생기지 않는다.

구현 근거: [공통 요약과 제한](../src/flow/mod.rs), [소스 추출](../src/flow/extract.rs), [요약 조합](../src/flow/evaluate.rs), [지연 로드와 캐시](../src/flow/index.rs), [회귀 검증](../src/flow/tests.rs).
