# Functional programming 누락 단어

- 분야 id: `functional-programming`
- 원본: [../functional-programming.md](../functional-programming.md)
- 누락 단어 30개 · 사전에 없음 6 · 뜻 미배정 0 · 다른 분야에만 있음 24
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Either result types (`t527`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `None` | identifier | 다른 분야에만 있음: auth-identity — OpenID token validation | Option의 빈 변형으로 Scala, Rust, Python 타입 코드에서 쓰인다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `Option` | identifier | 다른 분야에만 있음: configuration — Preference settings | Maybe와 같은 값 있음/없음 타입으로 fp-ts, Scala, Rust 코드에 쓰인다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `Optional` | identifier | 다른 분야에만 있음: build-release — Optional build features | Java Optional의 map/flatMap 체이닝에 쓰이는 값 있음/없음 타입이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `Result` | identifier | 다른 분야에만 있음: concurrency-async — Background job queues | Ok/Err로 성공과 실패를 담는 결과 타입으로 Rust와 TypeScript 함수형 라이브러리에 쓰인다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `Try` | identifier | 다른 분야에만 있음: distributed-systems — Retry policies | Scala와 Vavr에서 예외를 Success/Failure 값으로 감싸는 모나드 타입이다 (다른 분야와 별개로 이 분야에서도 핵심어). |

### Function composition pipelines (`t301.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `flip` | identifier | 다른 분야에만 있음: algorithms-data-structures — Dictionary entry operations | Haskell과 Ramda에서 두 인자의 순서를 뒤집는 함수이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `partial` | identifier | 다른 분야에만 있음: data-formats — Partial update wording | Ramda, lodash, functools.partial에서 인자를 일부 고정한 함수를 만드는 이름이다 (다른 분야와 별개로 이 분야에서도 핵심어). |

### Functional optics (`t183`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `traversal` | word | 다른 분야에만 있음: algorithms-data-structures — Graph path search | 여러 초점을 갱신하는 옵틱 종류로 렌즈 문서에 쓰인다 (다른 분야와 별개로 이 분야에서도 핵심어). |

### Functional type abstractions (`t96`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `comonad` | word | 사전에 없음 | 모나드의 쌍대로 extract/extend를 갖는 추상이다. |

### Immutable collection updates (`t240.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Array.prototype.with` | identifier | 사전에 없음 | 원본을 바꾸지 않고 한 원소를 교체한 새 배열을 만드는 JavaScript 메서드이다. |

### Lambda combinators (`t721.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Church-Rosser` | proper_noun | 사전에 없음 | 람다 계산의 합류성 정리이다. |

### Predicate sequence filtering (`t218.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `drop` | identifier | 다른 분야에만 있음: databases-storage — Entity associations | 시퀀스 앞쪽 원소를 버리는 함수이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `partition` | identifier | 다른 분야에만 있음: algorithms-data-structures — Data chunk segmentation | 술어로 시퀀스를 둘로 나누는 함수이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `take` | identifier | 다른 분야에만 있음: algorithms-data-structures — Top-N selection | 시퀀스 앞쪽 원소를 가져오는 함수이다 (다른 분야와 별개로 이 분야에서도 핵심어). |

### Pure functional expressions (`t465.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `functools.lru_cache` | identifier | 다른 분야에만 있음: databases-storage — Function memoization | Python 순수 함수 결과를 LRU로 캐시하는 데코레이터이다 (다른 분야와 별개로 이 분야에서도 핵심어). |

### Sequence reduction (`t890`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `fold` | word | 다른 분야에만 있음: data-engineering — Sequence folds | 접기 연산의 기본형으로 map/filter/fold 흐름에 쓰인다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `foldl` | identifier | 다른 분야에만 있음: algorithms-data-structures — Sequence endpoints | Haskell 왼쪽 접기 함수이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `foldRight` | identifier | 다른 분야에만 있음: algorithms-data-structures — List traversal | Scala와 Kotlin 오른쪽 접기 메서드이다 (다른 분야와 별개로 이 분야에서도 핵심어). |

### Sequence transformations (`t40.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `zip` | identifier | 다른 분야에만 있음: algorithms-data-structures — Reactive stream zipping | 두 시퀀스를 짝지어 묶는 함수이다 (다른 분야와 별개로 이 분야에서도 핵심어). |

### Typed functional programming (`t868`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Agda` | proper_noun | 다른 분야에만 있음: type-systems — Dependent types | 의존 타입 증명 지원 함수형 언어이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `Idris` | proper_noun | 다른 분야에만 있음: type-systems — Algebraic data variants | 의존 타입 함수형 언어이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `mappend` | identifier | 다른 분야에만 있음: algorithms-data-structures — Sequence concatenation | Haskell Monoid의 결합 연산이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `mconcat` | identifier | 다른 분야에만 있음: algorithms-data-structures — Sequence concatenation | Haskell Monoid 목록 결합 함수이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `mempty` | identifier | 다른 분야에만 있음: data-engineering — Monoid combination | Haskell Monoid의 항등원이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `monoid` | word | 다른 분야에만 있음: algorithms-data-structures — Sequence concatenation | 결합 연산과 항등원을 갖는 타입 클래스 개념이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `OCaml` | proper_noun | 다른 분야에만 있음: languages-compilers — ML-family languages | ML 계열 타입 함수형 언어이다 (다른 분야와 별개로 이 분야에서도 핵심어). |
| `semigroup` | word | 다른 분야에만 있음: data-engineering — Grouped aggregates | 결합 연산만 갖는 타입 클래스 개념이다 (다른 분야와 별개로 이 분야에서도 핵심어). |

## 새 주제가 필요한 단어

### Recursion schemes and unfolds

범위: Generalized folds and unfolds over recursive data structures through recursion schemes.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `hylomorphism` | word | 사전에 없음 | unfold 뒤 fold를 합친 재귀 도식이다. |
| `paramorphism` | word | 사전에 없음 | 원래 하위 구조에 접근하는 fold 일반화이다. |
| `unfoldr` | identifier | 사전에 없음 | Haskell의 unfold 함수이다. |
