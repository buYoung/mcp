# 타입 시스템 누락 단어

- 분야 id: `type-systems`
- 원본: [../type-systems.md](../type-systems.md)
- 누락 단어 11개 · 사전에 없음 2 · 뜻 미배정 0 · 다른 분야에만 있음 9
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Dependent types (`t509`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Coq` | proper_noun | 다른 분야에만 있음: general-wording — Minimality wording | 의존 타입을 쓰는 대표적인 증명 보조기로 형식 검증 코드와 문서에 쓰인다. |
| `Lean` | proper_noun | 다른 분야에만 있음: data-formats — Object deserialization; general-wording — Minimality wording | 의존 타입 기반 증명 보조기이자 프로그래밍 언어로 Mathlib와 함께 쓰인다. |

### Generic type declarations (`t596.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `higher kinded type` | phrase | 다른 분야에만 있음: functional-programming — Functional type abstractions | 타입 생성자를 매개변수로 받는 타입으로 Scala와 Haskell 함수형 라이브러리에 쓰인다. |

### Parameter direction variance (`t816`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `contravariant` | word | 다른 분야에만 있음: language-constructs — Subtype compatibility | 공변과 대비되는 반공변 타입 매개변수를 가리키는 형용사다. |
| `invariant` | word | 다른 분야에만 있음: error-handling — Runtime contract validation; language-constructs — Subtype compatibility | 공변도 반공변도 아닌 불변 타입 매개변수를 가리키는 형용사다. |

### Python typing (`t125.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Pydantic` | proper_noun | 다른 분야에만 있음: data-formats — Input field validation; data-formats — Typed data adapters; software-design — Data record definitions; web-backend — Typed Python APIs | Python 타입 힌트로 데이터 검증과 직렬화를 하는 라이브러리다. |
| `Pyrefly` | proper_noun | 사전에 없음 | Meta의 Rust 기반 Python 타입 검사기이다. |

### Tuple values (`t608`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `namedtuple` | identifier | 다른 분야에만 있음: algorithms-data-structures — Collection helpers; software-design — Data record definitions | Python의 이름 붙은 필드를 가진 튜플 타입이다. |

### TypeScript type tooling (`t511.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `type-only import` | phrase | 사전에 없음 | 타입만 가져오는 import 구문이다. |

## 새 주제가 필요한 단어

### Interfaces and traits

범위: Declaring behavior contracts through interfaces, traits and protocols, including trait objects, default methods and structural compatibility.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `interface` | word | 다른 분야에만 있음: concurrency-async — UI control wording; language-constructs — Interface implementation; networking — Interface addressing; software-design — Interface dispatch contracts; software-design — Ports and adapters | 구현을 강제하는 타입 계약이다. |
| `structural typing` | phrase | 다른 분야에만 있음: language-constructs — Interface implementation | 멤버 구조로 호환성을 판단하는 타입 방식이다. |
