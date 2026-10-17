# Observability, debugging and performance 누락 단어

- 분야 id: `observability-debugging`
- 원본: [../observability-debugging.md](../observability-debugging.md)
- 누락 단어 16개 · 사전에 없음 8 · 뜻 미배정 0 · 다른 분야에만 있음 8
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Debugger sessions (`t439.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `inspect-brk` | identifier | 사전에 없음 | Node.js 실행 첫 줄에서 멈추는 디버깅 옵션이다. |

### Debugger stack locals (`t745`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `call stack` | phrase | 다른 분야에만 있음: editors-ide — Call hierarchy navigation | 다른 분야에서는 호출 계층 탐색으로 다루지만, 이 분야에서는 디버거의 호출 스택 창과 프레임 목록을 가리킨다. |

### Distributed trace instrumentation (`t694.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `span status` | phrase | 사전에 없음 | 스팬의 성공 또는 오류 결과를 기록하는 OpenTelemetry 개념이다. |

### Heap memory diagnostics (`t650.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `memory usage` | phrase | 다른 분야에만 있음: systems-low-level — Heap garbage collection | 프로세스 메모리 사용량으로 이 분야의 범위인 메모리 사용·누수 분석과 모니터링에서 핵심으로 쓴다. |
| `memray` | proper_noun | 다른 분야에만 있음: files-io — Python filesystem APIs | 다른 분야의 뜻은 파일 입출력 문맥이고, 이 분야에서는 파이썬 힙 할당을 추적하는 메모리 프로파일러로 쓴다. |
| `Xmx` | abbreviation | 사전에 없음 | JVM 최대 힙 크기를 정하는 옵션이다. |

### Latency benchmarks (`t84.5`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `cold start` | phrase | 다른 분야에만 있음: cloud-infrastructure — Cloud function orchestration | 이 분야 범위에 명시된 콜드 스타트로, 서버리스 지연 시간 분석과 벤치마크 문서에서 쓴다. |

### Log ingestion pipelines (`t286.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Logz.io` | proper_noun | 사전에 없음 | ELK 기반 관리형 로그 서비스다. |
| `OpenSearch` | proper_noun | 다른 분야에만 있음: databases-storage — Elasticsearch queries | 다른 분야에서는 검색 쿼리로 다루지만, 이 분야에서는 ELK 계열 로그 스택의 저장소로 쓴다. |
| `Sumo Logic` | proper_noun | 사전에 없음 | 클라우드 로그 분석 서비스다. |

### Native crash debugging (`t439.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `strace` | identifier | 다른 분야에만 있음: concurrency-async — Process tracing | 다른 분야에서는 프로세스 추적 일반으로 다루지만, 이 분야에서는 크래시와 멈춤을 진단하는 디버깅 도구로 핵심이다. |

### Operational metric monitoring (`t299.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Cacti` | proper_noun | 사전에 없음 | RRDtool 기반 네트워크 모니터링 도구다. |
| `cAdvisor` | proper_noun | 다른 분야에만 있음: cloud-infrastructure — Linux container isolation | 다른 분야에서는 컨테이너 격리로 다루지만, 이 분야에서는 컨테이너 메트릭을 수집해 Prometheus에 노출하는 모니터링 도구로 쓴다. |
| `Munin` | proper_noun | 사전에 없음 | 그래프 중심 서버 모니터링 도구다. |
| `Sensu` | proper_noun | 사전에 없음 | 이벤트 기반 모니터링 도구다. |

### Runtime profiling (`t401.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Perfetto` | proper_noun | 다른 분야에만 있음: math-numerics — Counter increments | 다른 분야의 뜻은 무관한 문맥이고, 이 분야에서는 Android와 리눅스 성능 트레이스를 수집·분석하는 프로파일링 도구로 쓴다. |
