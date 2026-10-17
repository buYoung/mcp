# Databases, caching and search 누락 단어

- 분야 id: `databases-storage`
- 원본: [../databases-storage.md](../databases-storage.md)
- 누락 단어 28개 · 사전에 없음 17 · 뜻 미배정 0 · 다른 분야에만 있음 11
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Bulk row insertion (`t487.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `insert into select` | phrase | 사전에 없음 | 대량 적재 SQL과 도구 문서에서 행을 한꺼번에 넣는 구문과 방식을 가리킨다. |

### Database backup restoration (`t512.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `hot backup` | phrase | 사전에 없음 | 백업 종류, 복구 시점 지정, SQL 덤프 파일을 가리키는 말로 백업 문서와 스크립트에 쓰인다. |

### Document databases (`t239`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Cosmos DB` | proper_noun | 다른 분야에만 있음: auth-identity — Azure identity services | Azure의 NoSQL 문서 데이터베이스로 이 분야 범위의 NoSQL 엔진이지만 다른 분야에서는 인증 맥락으로만 다뤄진다. |

### Elasticsearch queries (`t56.4`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `filter context` | phrase | 사전에 없음 | Elasticsearch 쿼리 DSL의 절, 집계, 점수 계산을 가리키는 말로 검색 코드와 문서에 쓰인다. |
| `relevance` | word | 다른 분야에만 있음: ml-ai — Search relevance ranking | 전문 검색 엔진의 관련도 점수를 가리키는 말로 이 분야 범위에 명시되어 있다. |
| `relevance score` | phrase | 다른 분야에만 있음: ml-ai — Search relevance ranking | Elasticsearch가 문서별로 계산하는 관련도 점수를 가리키는 말로 검색 결과 정렬 코드와 문서에 쓰인다. |
| `rescore` | word | 사전에 없음 | Elasticsearch 쿼리의 오타 허용 정도와 재점수 단계를 가리키는 말이다. |

### LevelDB storage (`t150.6`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `BadgerDB` | proper_noun | 사전에 없음 | LevelDB와 같은 임베디드 키값 저장소 라이브러리다. |

### ORM and data access (`t406`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Core Data` | proper_noun | 다른 분야에만 있음: mobile-desktop — Cocoa application APIs | Apple 플랫폼의 객체 영속성 프레임워크로 ORM 용도가 이 분야의 뜻이다. |
| `Ecto` | proper_noun | 다른 분야에만 있음: languages-compilers — BEAM languages | Elixir의 데이터베이스 래퍼와 쿼리 라이브러리로 ORM 용도가 이 분야의 뜻이다. |

### Relational database systems (`t70.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `DB2` | proper_noun | 다른 분야에만 있음: docs-collaboration — Code refactoring | IBM의 관계형 데이터베이스 엔진 이름으로 이 분야의 뜻이 핵심이고 다른 분야에는 우연한 표기로만 있다. |
| `Firebird` | proper_noun | 사전에 없음 | 관계형 및 분산 SQL 데이터베이스 엔진과 서비스 이름이다. |
| `Teradata` | proper_noun | 사전에 없음 | 관계형 및 분산 SQL 데이터베이스 엔진과 서비스 이름이다. |

### Replica failover (`t150.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `read write splitting` | phrase | 사전에 없음 | 복제 방식과 장애 조치 상태를 가리키는 말로 운영 문서에 쓰인다. |

### Schema migration DDL (`t70.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `materialized view` | phrase | 다른 분야에만 있음: data-engineering — OLAP rollups | 저장된 질의 결과를 갱신하는 데이터베이스 객체로 DDL과 마이그레이션 코드에 쓰이며 다른 분야에서는 OLAP 집계 맥락으로만 있다. |
| `unlogged table` | phrase | 사전에 없음 | DDL 구문과 마이그레이션 도구 명령을 가리키는 말로 스키마 변경 코드에 쓰인다. |

### Search indexing (`t56.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Vespa` | proper_noun | 사전에 없음 | 검색 엔진과 전문 검색 라이브러리 이름이다. |

### SQL privilege grants (`t119.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `revoke` | word | 다른 분야에만 있음: auth-identity — Authorization revocation | SQL에서 부여한 권한을 회수하는 REVOKE 문으로 GRANT와 함께 쓰인다. |

### SQL query planning (`t870`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `semi join` | phrase | 사전에 없음 | 실행 계획에 나타나는 스캔과 조인 방식, 계획 조회 구문을 가리키는 말이다. |

### Table keys and indexes (`t70.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `brin` | abbreviation | 사전에 없음 | PostgreSQL의 인덱스 접근 방식 이름이다. |
| `nonclustered index` | phrase | 사전에 없음 | 인덱스 종류와 제약 조건, 인덱스 추가 삭제 구문을 가리키는 말이다. |

## 새 주제가 필요한 단어

### Search cluster and index management

범위: Managing Elasticsearch and OpenSearch clusters, shards, index templates and index lifecycle.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `cluster health` | phrase | 사전에 없음 | 검색 클러스터의 노드, 샤드, 인덱스 템플릿과 운영 설정을 가리키는 말이다. |
| `component template` | phrase | 사전에 없음 | 검색 클러스터의 노드, 샤드, 인덱스 템플릿과 운영 설정을 가리키는 말이다. |
| `coordinating node` | phrase | 사전에 없음 | 검색 클러스터의 노드, 샤드, 인덱스 템플릿과 운영 설정을 가리키는 말이다. |
| `force merge` | phrase | 사전에 없음 | 검색 클러스터의 노드, 샤드, 인덱스 템플릿과 운영 설정을 가리키는 말이다. |

### Time series databases

범위: Time series database engines and extensions for storing timestamped metrics.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `InfluxDB` | proper_noun | 다른 분야에만 있음: observability-debugging — Metrics monitoring | 시계열 데이터를 저장하는 데이터베이스로 이 분야 범위에 명시되어 있고 다른 분야에서는 지표 모니터링 맥락으로만 있다. |
| `time series database` | phrase | 다른 분야에만 있음: data-visualization — Time series plots | 시계열 데이터를 저장하는 데이터베이스 분류를 가리키는 말이다. |
| `TimescaleDB` | proper_noun | 다른 분야에만 있음: observability-debugging — Operational metric monitoring | 시계열 데이터를 저장하는 PostgreSQL 확장으로 다른 분야에서는 지표 모니터링 맥락으로만 있다. |
