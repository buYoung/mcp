# Distributed systems and messaging 누락 단어

- 분야 id: `distributed-systems`
- 원본: [../distributed-systems.md](../distributed-systems.md)
- 누락 단어 13개 · 사전에 없음 3 · 뜻 미배정 0 · 다른 분야에만 있음 10
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Client connection timeouts (`t46.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `deadline propagation` | phrase | 사전에 없음 | gRPC 등에서 호출 마감 시한을 하위 서비스로 전달하는 방식을 가리킵니다. |

### Data stream processing (`t488`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Flink` | proper_noun | 다른 분야에만 있음: data-engineering — Stateful stream processing | 주제 범위에 명시된 스트림 처리 엔진으로 Kafka 연동 코드에 자주 나옵니다. |
| `Kinesis` | proper_noun | 다른 분야에만 있음: data-engineering — Stateful stream processing | 분야 범위에 명시된 AWS 스트리밍 서비스로 SDK·설정 코드에 쓰입니다. |

### Distributed saga coordination (`t305.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `change data capture` | phrase | 다른 분야에만 있음: data-engineering — Change capture connectors | 트랜잭션 로그를 읽어 outbox 이벤트를 발행하는 방식 문서에 쓰입니다. |

### Message delivery ordering (`t812`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `partition key` | phrase | 다른 분야에만 있음: data-engineering — Distributed partition balancing | Kafka·Kinesis에서 같은 키의 메시지를 같은 파티션으로 보내 순서를 지키는 키입니다. 다른 분야에서는 DB 파티션 키로 쓰입니다. |

### Service discovery (`t940`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `client side discovery` | phrase | 사전에 없음 | 클라이언트가 레지스트리에서 인스턴스를 직접 골라 호출하는 방식을 가리킵니다. |
| `server side discovery` | phrase | 사전에 없음 | 로드 밸런서가 인스턴스를 골라 호출을 전달하는 방식을 가리킵니다. |

### Workflow execution (`t305.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Argo Workflows` | proper_noun | 다른 분야에만 있음: cloud-infrastructure — Argo delivery workflows | 범위에 명시된 Argo 기반 Kubernetes 워크플로 엔진 이름입니다. |
| `DAG` | abbreviation | 다른 분야에만 있음: algorithms-data-structures — Graph path search | 범위에 명시된 워크플로 의존 그래프를 가리키는 약어입니다. |

## 새 주제가 필요한 단어

### Consistency models and replication

범위: Replicating data across nodes with consistency levels, conflict resolution and convergent data types.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `conflict resolution` | phrase | 다른 분야에만 있음: databases-storage — Consistency models | 복제 쓰기 충돌을 병합하는 정책을 가리킵니다. 버전 관리의 병합 충돌과 구별되는 분산 복제 뜻이 있습니다. |
| `CRDT` | abbreviation | 다른 분야에만 있음: databases-storage — Consistency models | 범위에 명시된 충돌 없는 복제 데이터 타입을 가리키는 약어입니다. |
| `strong consistency` | phrase | 다른 분야에만 있음: databases-storage — Consistency models | 범위에 명시된 일관성 모델 중 선형적 읽기를 보장하는 수준을 가리킵니다. |

### Distributed locks and leases

범위: Coordinating exclusive access across nodes through distributed locks, leases and fencing tokens.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `lease` | word | 다른 분야에만 있음: auth-identity — Access grants | 만료 시간이 있는 락·리더 권한을 가리키며 분산 락 구현에 쓰입니다. |
