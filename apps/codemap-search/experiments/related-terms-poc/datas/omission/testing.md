# Testing 누락 단어

- 분야 id: `testing`
- 원본: [../testing.md](../testing.md)
- 누락 단어 23개 · 사전에 없음 19 · 뜻 미배정 0 · 다른 분야에만 있음 4
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Assertion matchers (`t13.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `assertArrayEquals` | identifier | 사전에 없음 | JUnit 테스트에서 배열 내용이 같은지 비교하는 단언 메서드로 쓰입니다. |
| `assertNotEquals` | identifier | 사전에 없음 | JUnit 테스트에서 두 값이 달라야 함을 검증하는 단언 메서드로 쓰입니다. |

### Browser end-to-end tests (`t237.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `auto wait` | phrase | 사전에 없음 | Playwright 등 브라우저 테스트에서 요소가 준비될 때까지 자동으로 기다리는 동작을 가리킵니다. |
| `.cy.ts` | extension | 사전에 없음 | Cypress 테스트 파일에 쓰이는 확장자입니다. |
| `data-cy` | identifier | 사전에 없음 | Cypress 테스트에서 요소를 안정적으로 찾으려고 붙이는 속성입니다. |
| `Maestro` | proper_noun | 사전에 없음 | 모바일 앱 UI 흐름을 YAML로 자동화하는 종단 간 테스트 도구입니다. |
| `Watir` | proper_noun | 사전에 없음 | Ruby 기반 브라우저 자동화 테스트 라이브러리입니다. |

### Component testing queries (`t45.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `getBy` | identifier | 사전에 없음 | Testing Library에서 요소를 동기로 찾는 질의 접두사입니다. |

### Coverage threshold reporting (`t400.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `coverage gap` | phrase | 사전에 없음 | 테스트가 닿지 않는 코드 영역을 가리키는 커버리지 보고서 용어입니다. |

### Flaky/transient failures (`t871`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `test order dependency` | phrase | 사전에 없음 | 테스트 실행 순서에 따라 결과가 달라지는 결함을 가리킵니다. |

### Local service mocks (`t943`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Betamax` | proper_noun | 사전에 없음 | HTTP 상호작용을 녹화해 재생하는 테스트용 라이브러리입니다. |
| `miragejs` | proper_noun | 사전에 없음 | 프런트엔드 개발과 테스트에서 가짜 API 서버를 정의하는 라이브러리입니다. |
| `vcrpy` | proper_noun | 사전에 없음 | Python 테스트에서 HTTP 응답을 카세트로 녹화해 재생하는 라이브러리입니다. |

### Mock interaction tests (`t5.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `fake timers` | phrase | 다른 분야에만 있음: concurrency-async — Fake timer testing | 테스트에서 setTimeout 같은 시간 함수를 가짜로 대체해 시간을 제어하는 기능입니다. |

### Test case planning (`t13.4`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `exploratory test` | phrase | 사전에 없음 | 테스트 케이스 없이 탐색하며 결함을 찾는 수동 테스트를 가리킵니다. |

### Test fixture helpers (`t45.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `conftest.py` | extension | 다른 분야에만 있음: language-constructs — pytest fixture scopes; languages-compilers — Pytest configuration | pytest 공용 픽스처와 훅을 두는 파일 이름입니다. |
| `Testcontainers` | proper_noun | 다른 분야에만 있음: databases-storage — Embedded database tests | 테스트에서 도커 컨테이너로 데이터베이스와 서비스를 띄우는 라이브러리입니다. |

### Test result reports (`t400.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `ReportPortal` | proper_noun | 사전에 없음 | 테스트 결과를 모아 분석하는 보고 플랫폼입니다. |

### Test suite execution (`t45.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Robolectric` | proper_noun | 사전에 없음 | Android 단위 테스트를 JVM에서 실행하게 하는 프레임워크입니다. |
| `runInBand` | identifier | 사전에 없음 | Jest 테스트를 하나의 프로세스에서 순차 실행하는 옵션입니다. |
| `test parallelization` | phrase | 사전에 없음 | 테스트 스위트를 병렬로 나눠 실행해 시간을 줄이는 설정입니다. |
| `test splitting` | phrase | 사전에 없음 | CI에서 테스트 파일을 실행 시간 기준으로 나누는 방식입니다. |
| `tox` | proper_noun | 다른 분야에만 있음: build-release — CI workflow execution; languages-compilers — Python package configuration | Python 테스트를 여러 환경에서 자동 실행하는 도구입니다. |
