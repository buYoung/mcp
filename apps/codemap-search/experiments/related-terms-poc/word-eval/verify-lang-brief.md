# 코드에서 정의된 이름 목록 만들기 (언어별)

배정된 소스 파일을 직접 읽고, 그 파일 안에서 **새 이름이 선언(정의)되는 위치**를 빠짐없이 나열해 주세요.
이 목록은 다른 방법으로 만든 목록과 대조할 기준으로 씁니다. 정확성과 빠짐없음이 가장 중요합니다.

## 작업 규칙
- 배정된 파일만 읽습니다. 다른 파일이나 디렉터리는 열지 않습니다.
- 원문을 직접 읽고 목록을 작성합니다. 코드를 파싱하거나 이름을 자동 추출하는 프로그램·스크립트(AST 모듈, 언어 서버, ctags, tree-sitter, 정규식 추출 스크립트 등)는 쓰지 않습니다.
  파일 보기 명령(cat, sed -n, nl)이나 위치를 찾는 검색(grep -n)은 써도 되지만, 목록의 각 항목은 원문에서 직접 확인해야 합니다.
- 파일을 수정하지 않습니다. 결과 파일 하나만 씁니다.

## 공통 규칙
- 이미 있는 이름을 쓰는 곳(호출, 참조, 속성·필드 접근, 이미 선언된 변수에 대입 — 아래 언어별 예외 제외)은 세지 않습니다.
- 빈 식별자 `_`와 `this`는 세지 않습니다. 이름 앞의 `#`, `@`, `@@`, `$` 기호는 떼고 적습니다(`#priv` → `priv`, `$value` → `value`).
- 주석·문자열 안의 이름, 별칭 없는 import/using/use, 어노테이션·데코레이터·속성(attribute) 이름은 세지 않습니다.
- 한 줄에 이름이 여럿이면 항목도 여럿입니다. `line`은 그 이름이 적힌 줄입니다.
- 매크로 등으로 코드가 깨져 보이는 곳도 읽히는 대로 판단하고, 애매하면 `note`에 적습니다.

## 언어별 규칙 (kind 값)

### Python
- `class` → `class`. 클래스 본문 바로 안의 `def` → `method`, 그 밖의 `def` → `function`
- 매개변수(`self`·`cls`, `*args`, `**kwargs`, 기본값 있는 것, lambda 매개변수 포함) → `parameter`
- 대입문 왼쪽의 이름(튜플·리스트 풀기의 각 이름 포함): 클래스 본문 바로 안이면 `field`, 모듈 최상위의 전부 대문자 이름이면 `constant`, 나머지는 `variable`.
  이미 있던 이름에 다시 대입해도 셉니다. 타입만 적은 `x: int`도 셉니다.
- `self.x = ...`·`cls.x = ...`의 `x` → `field`
- `for` 대상, 컴프리헨션 `for` 대상, `with ... as x`, `except E as x`, `:=` → `variable`
- `import a as b`, `from a import b as c`의 별칭 → `import_alias`
- 세지 않음: `+=` 같은 증강 대입, `global`/`nonlocal`, 호출의 키워드 인자 이름, `obj.attr = ...`(self/cls 제외), `a[i] = ...`

### JavaScript / TypeScript / TSX
- 이름 있는 함수 선언·함수 표현식(generator, TS 오버로드 선언 포함) → `function`
- 클래스(추상 클래스, 이름 있는 클래스 표현식 포함) → `class`, 클래스 메서드(constructor, getter/setter 포함) → `method`, 클래스 필드(`#x` 포함) → `field`
- `interface` → `interface`, `type X =` → `type_alias`, `enum` → `enum`, enum 멤버 → `enum_member`, `namespace`/`module` → `namespace`
- interface·객체 타입 안의 속성 시그니처 → `field`, 메서드 시그니처 → interface 안이면 `interface_method`, 그 밖은 `method`
- 타입 매개변수 `<T>` → `type_parameter`
- `const`/`let`/`var` 선언의 이름(구조 분해의 각 이름, 이름을 바꾼 대상, 기본값·나머지 포함) → `variable`
- 함수·메서드·화살표 함수 매개변수(구조 분해의 각 이름, 기본값·나머지, 함수 타입 시그니처의 매개변수 포함) → `parameter`
- `catch (e)`, 선언이 있는 `for (const x of ...)`/`for (let k in ...)` → `variable`
- `import { a as b }`의 `b`, `import * as ns`의 `ns` → `import_alias`. 레이블 → `label`
- 세지 않음: 기본 import와 별칭 없는 named import, 객체 리터럴의 키, JSX 속성 이름, 이미 있는 변수에 대입

### Java
- class/interface/enum/record/`@interface` → `class`/`interface`/`enum`/`record`/`interface`
- 메서드 → `method`(interface 안이면 `interface_method`), 생성자 → `constructor`
- 필드(선언자 하나당) → `field`, interface 상수 → `constant`, 지역 변수 → `variable`
- 매개변수(가변 인자, 람다 매개변수 포함) → `parameter`, record 구성 요소 → `field`
- catch 변수, try-with-resources 변수, 향상된 for 변수, `instanceof` 패턴 변수 → `variable`
- enum 상수 → `enum_member`, 타입 매개변수 → `type_parameter`, 레이블 → `label`

### C#
- class/struct/interface/enum/record/delegate → `class`/`struct`/`interface`/`enum`/`record`/`type`
- namespace 이름(점으로 나뉜 각 부분) → `namespace`
- 메서드 → `method`(interface 안이면 `interface_method`), 생성자 → `constructor`, 지역 함수 → `function`, 속성(property) → `property`
- 필드·이벤트 필드(선언자 하나당) → `field`(`const`면 `constant`), 지역 변수(분해 `var (a, b)`의 각 이름 포함) → `variable`
- 매개변수(`params` 배열, 람다 매개변수 포함) → `parameter`
- catch 변수, foreach 변수, 패턴 변수(`is Foo foo`), `out var x` → `variable`
- enum 멤버 → `enum_member`, 타입 매개변수 → `type_parameter`, `using A = ...`의 `A` → `import_alias`, 레이블 → `label`

### C / C++
- 함수 정의 → `function`(`Class::name`처럼 범위가 붙었거나 클래스 본문 안에서 정의되면 `method`), 함수 선언(프로토타입) → `function`(클래스 본문 안이면 `method`)
- 선언의 변수(전역·지역, 선언자 하나당, 포인터·배열 포함) → `variable`, struct/class/union 멤버 변수 → `field`
- 매개변수(함수 포인터 매개변수, 프로토타입의 매개변수, 람다 매개변수, 템플릿 값 매개변수 포함) → `parameter`, `#define` 함수형 매크로의 매개변수 → `parameter`
- 본문이 있는 이름 있는 struct/class/union/enum → `struct`/`class`/`union`/`enum`, 열거자 → `enum_member`
- `typedef` 이름 → `type`, `using X = ...` → `type_alias`, `#define` 이름 → `macro`, namespace → `namespace`, 템플릿 타입 매개변수 → `type_parameter`
- 범위 기반 for 변수, 구조화된 바인딩의 각 이름 → `variable`, 레이블 → `label`
- 세지 않음: 소멸자, 연산자 오버로드 이름, 본문 없는 `struct node` 참조

### Rust
- `fn` → `function`(impl·trait 안이면 `method`), struct/enum/union/trait/`type`/`mod`/`macro_rules!`/`const`/`static` → `struct`/`enum`/`union`/`trait`/`type_alias`/`module`/`macro`/`constant`/`constant`
- enum 변형 → `enum_member`, 이름 있는 구조체 필드 → `field`, 타입 매개변수·const 제네릭 → `type_parameter`
- 함수 매개변수 패턴의 이름(`self` 제외), 클로저 매개변수 → `parameter`
- `let`, `for`, `if let`/`while let`, `match` 갈래 패턴에서 묶이는 소문자 이름 → `variable`(대문자로 시작하는 `None`·변형·상수 이름은 세지 않음, match 가드는 세지 않음)
- 세지 않음: impl 대상 타입, 라이프타임, `use`, 매크로 호출

### PHP
- `function` → `function`, 메서드 → `method`(interface 안이면 `interface_method`), class/interface/trait/enum → `class`/`interface`/`trait`/`enum`, namespace 각 부분 → `namespace`
- 속성(property) → `field`, 생성자 승격 매개변수 → `field`, `const`(전역·클래스) → `constant`
- 매개변수(가변 인자, 익명 함수·화살표 함수 매개변수 포함) → `parameter`
- `$변수`에 대입(`[$a, $b] = ...`·`list()` 분해의 각 이름 포함, 다시 대입해도 셈) → `variable`. `$this->x = ...`는 세지 않음
- foreach의 키·값 변수, catch 변수 → `variable`, enum case → `enum_member`, `use X as Y`의 `Y` → `import_alias`
- 세지 않음: 익명 함수의 `use ($x)`, 함수 호출, 속성 접근

## 결과 형식
JSON 배열 하나를 결과 파일에 씁니다. 항목마다:
`{"file": "<배정 목록에 적힌 경로>", "line": <줄 번호, 1부터>, "name": "<이름>", "kind": "<위 kind 값>"}`
판단이 애매했던 항목은 `"note"`에 이유를 적습니다. 마지막 답변에는 파일별 항목 수와 애매했던 경우만 짧게 적습니다.
