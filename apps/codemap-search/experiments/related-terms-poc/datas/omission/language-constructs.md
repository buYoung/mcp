# Core language constructs 누락 단어

- 분야 id: `language-constructs`
- 원본: [../language-constructs.md](../language-constructs.md)
- 누락 단어 23개 · 사전에 없음 8 · 뜻 미배정 1 · 다른 분야에만 있음 14
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Callable parameters (`t105.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `spread argument` | phrase | 사전에 없음 | Kotlin과 JavaScript에서 배열을 가변 인자로 펼쳐 넘기는 호출 구문을 가리키는 말이다. |
| `trailing closure` | phrase | 사전에 없음 | Swift에서 마지막 클로저 인자를 괄호 밖에 쓰는 호출 구문을 가리키는 말이다. |

### Instance creation (`t960`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `object instantiation` | phrase | 사전에 없음 | 클래스로부터 객체를 만드는 과정을 가리키는 말이다. |

### Instance method receivers (`t224.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `infix function` | phrase | 사전에 없음 | Kotlin과 Haskell에서 중위 표기로 호출하는 함수를 가리키는 말이다. |

### Iterator traversal (`t131.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `iterator adapter` | phrase | 사전에 없음 | Rust에서 map, filter처럼 이터레이터를 감싸 새 이터레이터를 만드는 메서드를 가리키는 말이다. |

### Lexical variable bindings (`t215.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `multiple assignment` | phrase | 뜻 미배정 | 여러 변수에 한 번에 대입하는 구문을 가리키는 말이다. |

### Loop iteration control (`t600.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `enhanced for loop` | phrase | 사전에 없음 | Java에서 컬렉션을 순회하는 for 구문을 가리키는 말이다. |

### Operator overloading (`t135.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `__str__` | identifier | 다른 분야에만 있음: data-formats — Object display strings | Python 객체의 사람이 읽는 문자열 표현을 정의하는 특수 메서드다. |
| `toString` | identifier | 다른 분야에만 있음: data-formats — Object display strings | 객체의 문자열 표현을 정의하는 메서드로 Java와 JavaScript에서 널리 쓴다. |

## 새 주제가 필요한 단어

### Boolean values

범위: Defining and testing boolean values, truthiness, and logical operators.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `bool` | word | 다른 분야에만 있음: functional-programming — Boolean expressions | Python, C++, Go 등에서 불리언 타입을 가리키는 이름이다. |
| `boolean` | word | 다른 분야에만 있음: functional-programming — Boolean expressions | 불리언 값과 타입을 가리키는 말로 코드와 문서에서 널리 쓴다. |
| `false` | word | 다른 분야에만 있음: functional-programming — Boolean expressions | 거짓을 나타내는 불리언 리터럴이다. |
| `true` | word | 다른 분야에만 있음: functional-programming — Boolean expressions | 참을 나타내는 불리언 리터럴이다. |

### Decorators and wrappers

범위: Wrapping functions and classes with decorators, decorator factories, and wrapper types.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `decorator factory` | phrase | 사전에 없음 | 인자를 받아 데코레이터를 만들어 돌려주는 함수를 가리키는 말이다. |
| `function decorator` | phrase | 사전에 없음 | Python에서 함수를 감싸 동작을 덧붙이는 데코레이터를 가리키는 말이다. |
| `functools.wraps` | identifier | 다른 분야에만 있음: functional-programming — Callable function values | Python 데코레이터가 원본 함수의 메타데이터를 보존하게 하는 함수다. |
| `wrapper` | word | 다른 분야에만 있음: software-design — Function decorators | 원본 함수나 객체를 감싸 동작을 덧붙이는 코드를 가리키는 말이다. |

### Macros and preprocessor

범위: Defining and expanding macros and preprocessor directives at compile time.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `macro` | word | 다른 분야에만 있음: languages-compilers — Source macro expansion | 컴파일 전에 코드로 확장되는 정의를 가리키는 말이다. |
| `macro_rules` | identifier | 다른 분야에만 있음: languages-compilers — Source macro expansion | Rust에서 선언형 매크로를 정의하는 매크로 이름이다. |
| `preprocessor` | word | 다른 분야에만 있음: languages-compilers — Source macro expansion | 컴파일 전에 지시어와 매크로를 처리하는 단계를 가리키는 말이다. |

### Program entry points

범위: Locating the function, file, or script where program execution begins.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `entry point` | phrase | 다른 분야에만 있음: frontend-ui — Startup initialization | 프로그램이나 패키지의 실행이 시작되는 지점을 가리키는 말이다. |
| `main` | word | 다른 분야에만 있음: os-shell — CLI argument handling | 프로그램 실행이 시작되는 함수와 파일 이름으로 널리 쓴다. |
| `main function` | phrase | 다른 분야에만 있음: os-shell — CLI argument handling | C, Go, Rust에서 프로그램이 시작되는 main 함수를 가리키는 말이다. |
