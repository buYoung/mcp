# Concurrency and async 누락 단어

- 분야 id: `concurrency-async`
- 원본: [../concurrency-async.md](../concurrency-async.md)
- 누락 단어 26개 · 사전에 없음 18 · 뜻 미배정 0 · 다른 분야에만 있음 8
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### .NET async tasks (`t128.5`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Task.Factory` | identifier | 사전에 없음 | TaskCreationOptions와 함께 Task를 만드는 .NET 팩토리 진입점입니다. |

### Actor mailboxes (`t297.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Ractor` | proper_noun | 사전에 없음 | Ruby의 행위자 기반 병렬 실행 기능입니다. |

### Async event loops (`t129.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `ReactPHP` | proper_noun | 사전에 없음 | PHP의 이벤트 루프 기반 비동기 라이브러리입니다. |
| `Swoole` | proper_noun | 사전에 없음 | PHP의 코루틴·이벤트 루프 기반 비동기 확장입니다. |

### Async task coordination (`t128.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Task.await` | identifier | 사전에 없음 | Elixir에서 Task.async로 시작한 작업의 결과를 기다리는 호출입니다. |

### Background job queues (`t165.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `DispatchQueue.sync` | identifier | 사전에 없음 | Swift에서 큐에 작업을 동기로 올리고 기다리는 호출입니다. |
| `Sidekiq` | proper_noun | 다른 분야에만 있음: languages-compilers — Ruby development ecosystem | Ruby의 대표적인 백그라운드 작업 처리 프레임워크입니다. 동시성 분야에서도 핵심이라 함께 넣습니다. |

### Concurrency barriers (`t110`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `asyncio.Barrier` | identifier | 사전에 없음 | asyncio에서 여러 작업이 한 지점에서 만나게 하는 장벽입니다. |

### Concurrent channels (`t297.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `MVar` | identifier | 사전에 없음 | Haskell에서 스레드 간 값 전달과 상호 배제에 쓰는 동기화 변수입니다. |
| `nil channel` | phrase | 사전에 없음 | Go에서 nil 채널은 송수신이 영원히 막히는 점을 다룰 때 쓰이는 말입니다. |

### Condition-variable waiting (`t132.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `asyncio.Condition` | identifier | 사전에 없음 | asyncio에서 조건 대기와 알림을 제공하는 조건 변수입니다. |

### Context propagation (`t1061`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `MDC` | abbreviation | 다른 분야에만 있음: observability-debugging — JVM logging facades | 로깅에서 스레드나 요청별 진단 컨텍스트를 붙이는 Mapped Diagnostic Context입니다. 동시성 분야에서도 핵심이라 함께 넣습니다. |
| `thread local storage` | phrase | 다른 분야에만 있음: web-backend — Execution context propagation | 스레드마다 따로 값을 두는 저장소를 가리키는 말입니다. 동시성 분야에서도 핵심이라 함께 넣습니다. |

### Fork-join algorithms (`t358.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `parallel_invoke` | identifier | 사전에 없음 | TBB와 PPL에서 여러 함수를 병렬로 호출하는 함수입니다. |
| `pragma omp` | phrase | 사전에 없음 | C와 C++에서 OpenMP 지시문을 시작하는 표기입니다. |

### JavaScript task limits (`t851`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `p-debounce` | proper_noun | 사전에 없음 | 비동기 함수를 디바운스하는 JavaScript 패키지입니다. |

### Kotlin coroutines (`t718`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `GlobalScope` | identifier | 다른 분야에만 있음: functional-programming — Callable function values | Kotlin에서 앱 수명의 전역 코루틴 스코프입니다. 동시성 분야에서도 핵심이라 함께 넣습니다. |
| `SharedFlow` | identifier | 다른 분야에만 있음: distributed-systems — Event bus communication | Kotlin에서 여러 수집자에게 값을 내보내는 핫 Flow입니다. 동시성 분야에서도 핵심이라 함께 넣습니다. |
| `StateFlow` | identifier | 다른 분야에만 있음: language-constructs — Data model objects | Kotlin에서 최신 상태 값을 유지하는 핫 Flow입니다. 동시성 분야에서도 핵심이라 함께 넣습니다. |

### Mutex critical sections (`t576.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `navigator.locks` | identifier | 사전에 없음 | 브라우저 Web Locks API의 진입점입니다. |

### Observable stream operators (`t190.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `bufferTime` | identifier | 사전에 없음 | RxJS에서 시간 단위로 값을 묶는 연산자입니다. |

### Parallel promise aggregation (`t213.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `FuturesUnordered` | identifier | 사전에 없음 | Rust futures에서 완료 순서대로 결과를 받는 Future 모음입니다. |

### Task executor pools (`t358.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `ScheduledExecutorService` | identifier | 다른 분야에만 있음: time-locale — Interval timers | Java에서 지연·주기 작업을 실행하는 실행기 인터페이스입니다. 동시성 분야에서도 핵심이라 함께 넣습니다. |

### Trio task nurseries (`t269.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `create_task_group` | identifier | 사전에 없음 | anyio에서 작업 그룹을 만드는 함수입니다. |

## 새 주제가 필요한 단어

### MPI message passing

범위: Running message-passing parallel programs across ranks with MPI libraries and launchers.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `collective communication` | phrase | 사전에 없음 | MPI에서 전체 프로세스가 참여하는 집합 통신을 가리킵니다. |
| `MPI` | abbreviation | 다른 분야에만 있음: ml-ai — MPI message passing | HPC 병렬 프로그램에서 프로세스 간 메시지를 주고받는 표준이며 범위에 명시됩니다. 동시성 분야에서도 핵심이라 함께 넣습니다. |
