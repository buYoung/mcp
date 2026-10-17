# 오류와 예외 처리 누락 단어

- 분야 id: `error-handling`
- 원본: [../error-handling.md](../error-handling.md)
- 누락 단어 29개 · 사전에 없음 9 · 뜻 미배정 0 · 다른 분야에만 있음 20
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Error cause chains (`t281.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `__context__` | identifier | 다른 분야에만 있음: auth-identity — Spring Security | Python 예외의 암묵적 연쇄 속성으로 __cause__ 와 짝으로 쓰인다. |

### Error types and messages (`t547.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `createError` | identifier | 사전에 없음 | http-errors 등에서 상태 코드가 붙은 오류 객체를 만드는 함수로 쓰인다. |
| `file not found` | phrase | 다른 분야에만 있음: files-io — Filesystem operations errors | 파일 없음 오류 메시지와 오류 이름으로 널리 쓰인다. |
| `LocalizedError` | identifier | 사전에 없음 | Swift에서 사용자에게 보일 오류 설명을 제공하는 프로토콜로 쓰인다. |
| `std::system_error` | identifier | 사전에 없음 | C++에서 운영체제 오류를 담는 표준 예외 클래스로 쓰인다. |

### Error values (`t281.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `failure mode` | phrase | 다른 분야에만 있음: databases-storage — Service failure recovery | 코드와 문서에서 실패가 나타나는 방식을 가리키는 말로 쓰인다. |
| `generic error` | phrase | 사전에 없음 | 구체적 원인 없이 일반적인 실패를 알리는 오류를 가리키는 말로 쓰인다. |

### Exception control flow (`t281.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `croak` | identifier | 사전에 없음 | Perl Carp에서 호출자 관점의 오류를 던지는 함수로 쓰인다. |
| `stack unwinding` | phrase | 다른 분야에만 있음: language-constructs — RAII scope cleanup | 예외나 패닉이 전파되며 스택 프레임을 풀어내는 과정을 가리킨다. |

### Exception reporting boundaries (`t281.6`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Airbrake` | proper_noun | 다른 분야에만 있음: observability-debugging — Crash reporting services | 예외를 수집해 보고하는 오류 추적 서비스 이름이다. |
| `Honeybadger` | proper_noun | 다른 분야에만 있음: observability-debugging — Crash reporting services | 예외를 수집해 보고하는 오류 추적 서비스 이름이다. |
| `Rollbar` | proper_noun | 다른 분야에만 있음: observability-debugging — Crash reporting services | 예외를 수집해 보고하는 오류 추적 서비스 이름이다. |
| `Sentry` | proper_noun | 다른 분야에만 있음: observability-debugging — APM telemetry | 예외를 수집해 보고하는 오류 추적 서비스 이름이다. |

### Exception type hierarchies (`t281.4`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `IOException` | identifier | 다른 분야에만 있음: concurrency-async — Java socket operations | Java와 .NET의 I/O 검사 예외 기본 클래스로 쓰인다. |
| `LogicException` | identifier | 사전에 없음 | PHP 논리 오류 예외 기본 클래스로 쓰인다. |
| `MemoryError` | identifier | 다른 분야에만 있음: algorithms-data-structures — ECC error recovery | Python 메모리 부족 예외 클래스로 쓰인다. |
| `OSError` | identifier | 다른 분야에만 있음: files-io — Filesystem operations errors | Python 운영체제 관련 예외의 기본 클래스(IOError 별칭 포함)로 쓰인다. |
| `OutOfMemoryError` | identifier | 다른 분야에만 있음: observability-debugging — Heap memory diagnostics | Java 힙 부족 시 던져지는 오류 클래스로 쓰인다. |
| `StackOverflowError` | identifier | 다른 분야에만 있음: functional-programming — Recursive control flow | Java 호출 스택 초과 시 던져지는 오류 클래스로 쓰인다. |
| `StandardError` | identifier | 다른 분야에만 있음: math-numerics — Confidence interval estimation | Ruby 일반 예외의 기본 클래스로 쓰이며 표준 오류 스트림과는 다른 뜻이다. |

### Operation outcomes (`t551.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `EACCES` | identifier | 다른 분야에만 있음: auth-identity — Spring Security | errno 권한 거부 코드로 쓰인다. |
| `EEXIST` | identifier | 다른 분야에만 있음: concurrency-async — Shared state races | errno 이미 존재함 코드로 쓰인다. |
| `ENOENT` | identifier | 다른 분야에만 있음: files-io — Filesystem operations errors | errno 파일이나 경로 없음 코드로 쓰인다. |

### Process exit status (`t551.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `sysexits` | identifier | 사전에 없음 | BSD sysexits.h 의 표준 종료 코드 모음 이름으로 쓰인다. |

### Runtime contract validation (`t374.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `DCHECK` | identifier | 사전에 없음 | Chromium 등 C++ 디버그 빌드 단언 매크로로 쓰인다. |

## 새 주제가 필요한 단어

### Null reference errors

범위: Failures caused by dereferencing null or nil pointers and references.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `NPE` | abbreviation | 다른 분야에만 있음: type-systems — Nullable value handling | NullPointerException 의 약어로 이슈와 로그에 쓰인다. |
| `null pointer` | phrase | 다른 분야에만 있음: ml-ai — Context wording | null 포인터 접근 오류를 가리키는 말로 쓰인다. |

### Out-of-bounds and missing-key errors

범위: Errors raised for indexes outside valid bounds and for lookups of missing keys or elements.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `missing key` | phrase | 다른 분야에만 있음: algorithms-data-structures — Dictionary entry operations | 없는 키 접근 오류와 검증 문구로 쓰인다. |
| `OutOfBoundsException` | identifier | 사전에 없음 | PHP 인덱스 범위 밖 예외 클래스로 쓰인다. |
