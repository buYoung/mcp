# Data engineering and analytics 누락 단어

- 분야 id: `data-engineering`
- 원본: [../data-engineering.md](../data-engineering.md)
- 누락 단어 55개 · 사전에 없음 43 · 뜻 미배정 0 · 다른 분야에만 있음 12
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Change capture connectors (`t293.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Apache Sqoop` | proper_noun | 사전에 없음 | 관계형 DB와 HDFS 사이의 일괄 수집 도구로 Hadoop 문서와 스크립트에 쓰입니다. |
| `data replication` | phrase | 사전에 없음 | 원본 DB를 분석 저장소로 복제하는 수집 파이프라인 문서에 쓰입니다. |
| `log-based CDC` | phrase | 사전에 없음 | 쿼리 방식과 구분해 로그 기반 변경 캡처를 가리키는 설계 문서 표현입니다. |
| `Meltano` | proper_noun | 사전에 없음 | Singer 기반 ELT 수집 파이프라인 설정에 쓰이는 도구입니다. |
| `NiFi` | proper_noun | 다른 분야에만 있음: distributed-systems — Client connection timeouts | NiFi 프로세서와 흐름 설정에서 이 분야의 수집 도구 이름으로 쓰입니다. |
| `SeaTunnel` | proper_noun | 사전에 없음 | 여러 소스와 싱크를 잇는 데이터 통합 커넥터 설정에 쓰입니다. |

### Columnar formats (`t74.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Apache Paimon` | proper_noun | 사전에 없음 | Flink 생태계의 스트리밍 레이크하우스 테이블 형식 이름입니다. |
| `compaction` | word | 다른 분야에만 있음: algorithms-data-structures — Merge algorithms | Delta, Iceberg, Hudi에서 작은 파일을 합치는 유지 작업을 가리키는 핵심어입니다. |
| `copy on write` | phrase | 다른 분야에만 있음: functional-programming — Copy-on-write storage | Hudi와 Iceberg 테이블 쓰기 모드 설정에 쓰이는 이 분야 고유 뜻이 있습니다. |

### DAG workflows (`t104.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Apache Oozie` | proper_noun | 사전에 없음 | Hadoop 워크플로 스케줄러의 공식 이름입니다. |
| `Azkaban` | proper_noun | 사전에 없음 | 배치 작업 스케줄러로 Hadoop 작업 흐름에 쓰입니다. |
| `Cloud Composer` | proper_noun | 사전에 없음 | Google Cloud의 관리형 Airflow 서비스 이름입니다. |
| `depends_on_past` | identifier | 사전에 없음 | Airflow 태스크가 이전 실행 성공에 의존하게 하는 인자입니다. |
| `DolphinScheduler` | proper_noun | 사전에 없음 | Apache DolphinScheduler 워크플로 스케줄러 이름입니다. |
| `dynamic task mapping` | phrase | 사전에 없음 | Airflow에서 런타임에 태스크 수를 늘리는 기능 이름입니다. |
| `ExternalTaskSensor` | identifier | 사전에 없음 | Airflow에서 다른 DAG의 태스크 완료를 기다리는 센서입니다. |
| `FileSensor` | identifier | 사전에 없음 | Airflow에서 파일 도착을 기다리는 센서입니다. |
| `Kestra` | proper_noun | 사전에 없음 | YAML로 정의하는 워크플로 오케스트레이션 도구 이름입니다. |
| `KubernetesPodOperator` | identifier | 사전에 없음 | Airflow에서 쿠버네티스 파드로 태스크를 실행하는 연산자입니다. |
| `MWAA` | abbreviation | 사전에 없음 | AWS 관리형 Airflow 서비스의 약어입니다. |

### DataFrame missing values (`t504`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `interpolate` | identifier | 다른 분야에만 있음: graphics-media — Image resampling | Pandas에서 결측값을 보간으로 채우는 메서드로 이 주제 범위에 직접 있습니다. |
| `notnull` | identifier | 다른 분야에만 있음: data-formats — Input field validation | Pandas에서 결측이 아닌 값을 판별하는 메서드입니다. |

### Dataframe table operations (`t67.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Modin` | proper_noun | 사전에 없음 | Pandas API를 병렬로 실행하는 데이터프레임 라이브러리입니다. |
| `to_numpy` | identifier | 사전에 없음 | Pandas에서 NumPy 배열로 변환하는 메서드입니다. |
| `to_spark` | identifier | 사전에 없음 | Pandas-on-Spark에서 Spark 데이터프레임으로 변환하는 메서드입니다. |
| `Vaex` | proper_noun | 사전에 없음 | 메모리보다 큰 표 데이터를 다루는 데이터프레임 라이브러리입니다. |

### Dataframe transformations (`t431.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `resample` | identifier | 다른 분야에만 있음: data-visualization — Time series plots | Pandas에서 시계열을 다른 주기로 집계하는 메서드로 이 분야에서 집계 뜻이 있습니다. |

### Distributed partition balancing (`t431.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `shuffle join` | phrase | 사전에 없음 | 파티션을 재분배하는 조인 방식으로 skew 진단 문서에 쓰입니다. |

### ETL transformations (`t293.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Azure Data Factory` | proper_noun | 사전에 없음 | Azure 관리형 ETL 서비스 이름입니다. |
| `dbt model` | phrase | 사전에 없음 | dbt 프로젝트에서 SQL 변환 단위를 가리킵니다. |
| `incremental extraction` | phrase | 사전에 없음 | 변경된 행만 추출하는 ETL 단계를 가리킵니다. |
| `upsert` | word | 다른 분야에만 있음: databases-storage — Document databases | 증분 적재에서 갱신과 삽입을 함께 수행하는 핵심 연산입니다. |

### Hive query metadata (`t74.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Hive partition` | phrase | 사전에 없음 | Hive 테이블 파티션을 가리키는 표현으로 파티션 프루닝 문서에 쓰입니다. |
| `Spark Thrift Server` | proper_noun | 사전에 없음 | Spark SQL을 JDBC로 노출하는 서버 이름입니다. |

### MapReduce output (`t756`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Apache Pig` | proper_noun | 사전에 없음 | MapReduce 위의 데이터 처리 언어 이름입니다. |
| `YARN` | proper_noun | 다른 분야에만 있음: build-release — Dependency graph resolution | Hadoop 자원 관리자로 Spark 배포 설정에 쓰이는 이 분야 고유 뜻이 있습니다. |

### OLAP rollups (`t50.5`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `metrics layer` | phrase | 사전에 없음 | 지표 정의를 중앙화하는 분석 계층을 가리킵니다. |

### Spark distributed analytics (`t67.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Catalyst` | proper_noun | 다른 분야에만 있음: databases-storage — SQL query planning | Spark SQL 쿼리 최적화기 이름으로 이 분야 고유 뜻이 있습니다. |
| `udf` | abbreviation | 다른 분야에만 있음: databases-storage — SQL routine invocation | Spark SQL에 등록하는 사용자 정의 함수의 약어로 이 분야에서 쓰입니다. |
| `Zeppelin` | proper_noun | 사전에 없음 | Spark 노트북 도구 이름입니다. |

### Stateful stream processing (`t429.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Apache Samza` | proper_noun | 사전에 없음 | Kafka 기반 스트림 처리 프레임워크 이름입니다. |
| `Dataflow` | proper_noun | 다른 분야에만 있음: files-io — Data processing pipelines | Google Cloud의 Beam 실행 서비스로 이 분야에서 쓰입니다. |
| `RisingWave` | proper_noun | 사전에 없음 | 스트리밍 SQL 데이터베이스 이름입니다. |

### Warehouse analytics (`t293.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Apache Doris` | proper_noun | 사전에 없음 | OLAP 분석 엔진 공식 이름입니다. |
| `Apache Drill` | proper_noun | 사전에 없음 | 스키마 없는 SQL 쿼리 엔진 이름입니다. |
| `BigQuery` | proper_noun | 다른 분야에만 있음: cloud-infrastructure — Google Cloud integration | Google 클라우드 웨어하우스로 이 분야의 핵심 서비스입니다. |
| `data vault` | phrase | 사전에 없음 | 허브, 링크, 새틀라이트로 구성하는 웨어하우스 모델링 기법입니다. |
| `Dremio` | proper_noun | 사전에 없음 | 레이크하우스 쿼리 엔진 이름입니다. |
| `StarRocks` | proper_noun | 사전에 없음 | OLAP 분석 엔진 이름입니다. |

## 새 주제가 필요한 단어

### Data catalog and governance

범위: Cataloging, governing, and monitoring datasets with metadata catalogs, data contracts, and data observability tools.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Collibra` | proper_noun | 사전에 없음 | 데이터 거버넌스 플랫폼 이름입니다. |
| `data fabric` | phrase | 사전에 없음 | 분산 데이터를 통합 관리하는 아키텍처 용어입니다. |
| `data mesh` | phrase | 사전에 없음 | 도메인별 데이터 제품 아키텍처 용어입니다. |
| `data observability` | phrase | 사전에 없음 | 데이터 품질과 신선도를 감시하는 도구 설명에 쓰입니다. |
| `data product` | phrase | 사전에 없음 | data mesh에서 도메인이 제공하는 데이터셋 단위를 가리킵니다. |
| `Deequ` | proper_noun | 사전에 없음 | Spark 위의 데이터 품질 검증 라이브러리 이름입니다. |
