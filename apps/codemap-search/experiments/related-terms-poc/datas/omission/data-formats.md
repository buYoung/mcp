# 데이터 형식과 직렬화 누락 단어

- 분야 id: `data-formats`
- 원본: [../data-formats.md](../data-formats.md)
- 누락 단어 57개 · 사전에 없음 21 · 뜻 미배정 0 · 다른 분야에만 있음 36
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Binary serialization (`t201.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `bencode` | word | 사전에 없음 | BitTorrent에서 쓰는 단순 직렬화 형식이다. |
| `capnp` | proper_noun | 다른 분야에만 있음: software-design — Data record definitions | Cap'n Proto의 약칭이자 컴파일러 명령 이름이다. |
| `DER` | abbreviation | 다른 분야에만 있음: security — Certificate chains | ASN.1 구별 인코딩 규칙으로 이진 인코딩 문서에서 쓰인다(분야 범위의 ASN.1 인코딩 의미). |
| `field number` | phrase | 다른 분야에만 있음: web-backend — RPC service protocols | Protobuf 스키마에서 필드를 식별하는 번호로 하위 호환 문서에서 자주 쓰인다. |
| `.h5` | extension | 다른 분야에만 있음: data-engineering — Dataframe transformations | HDF5 파일 확장자로 Keras 모델 저장에도 쓰인다. |
| `HDF5` | proper_noun | 다른 분야에만 있음: data-engineering — Dataframe transformations | 분야 범위에 명시된 계층형 이진 데이터 형식이다. |
| `IDL` | abbreviation | 다른 분야에만 있음: languages-compilers — Source code generators | 분야 범위의 Protobuf/IDL에 해당하는 인터페이스 정의 언어 약어다. |
| `.proto` | extension | 다른 분야에만 있음: language-constructs — Prototype inheritance | Protobuf 스키마 정의 파일 확장자로 IDL 파일을 찾을 때 쓴다. |
| `Protocol Buffers` | proper_noun | 다른 분야에만 있음: web-backend — RPC service protocols | Protobuf의 정식 이름으로 문서와 검색어에서 풀네임으로 쓰인다. |
| `Protostuff` | proper_noun | 사전에 없음 | Java 스키마 기반 직렬화 라이브러리다. |
| `textproto` | word | 다른 분야에만 있음: networking — Network terminology | Protobuf 텍스트 형식 또는 그 파일을 가리키는 이름이다. |
| `Thrift` | proper_noun | 다른 분야에만 있음: web-backend — RPC service protocols | Apache Thrift 직렬화·IDL로 이진 형식 문서와 코드에 나온다. |
| `UBJSON` | proper_noun | 사전에 없음 | 이진 JSON 형식 중 하나로 CBOR, BSON과 함께 언급된다. |
| `varint` | word | 다른 분야에만 있음: math-numerics — Integer bit significance | Protobuf 등 이진 형식의 가변 길이 정수 인코딩으로 직렬화 코드에서 쓰인다. |
| `wire type` | phrase | 다른 분야에만 있음: language-constructs — Object property operations | Protobuf 인코딩에서 필드 값의 바이트 표현 종류를 가리키는 핵심 용어다. |
| `Zarr` | proper_noun | 사전에 없음 | 청크 단위 배열 저장 형식으로 HDF5 대안으로 쓰인다. |

### Constrained model decoding (`t460.4`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `jsonformer` | proper_noun | 사전에 없음 | 스키마에 맞춰 JSON 출력을 강제하는 라이브러리다. |

### Data export (`t926`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `.ckpt` | extension | 다른 분야에만 있음: databases-storage — Database backup restoration | 체크포인트 모델 파일 확장자다. |
| `gguf` | proper_noun | 다른 분야에만 있음: ml-ai — GPU compute kernels | llama.cpp 계열 모델 파일 형식이다. |
| `.pth` | extension | 다른 분야에만 있음: ml-ai — Pretrained model loading | PyTorch 모델 가중치 저장 확장자다. |
| `safetensors` | proper_noun | 다른 분야에만 있음: databases-storage — Sharded model checkpoints | 모델 가중치 저장 형식으로 save_pretrained 출력에 나온다. |

### Data quality checks (`t635.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Deequ` | proper_noun | 사전에 없음 | Spark용 데이터 품질 검증 라이브러리다. |
| `Pandera` | proper_noun | 사전에 없음 | DataFrame 스키마 검증 라이브러리다. |

### Declarative validation schemas (`t635.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `datamodel-codegen` | proper_noun | 사전에 없음 | 스키마에서 Pydantic 모델을 생성하는 도구다. |
| `discriminated union` | phrase | 다른 분야에만 있음: type-systems — Tagged union narrowing | 판별자로 대안을 구분하는 스키마 패턴으로 주제 범위의 discriminated alternatives에 해당한다. |

### HTTP media types (`t12.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `application/yaml` | identifier | 사전에 없음 | YAML 본문에 쓰는 미디어 타입이다. |
| `charset` | word | 다른 분야에만 있음: text-processing — Text encoding conversion | Content-Type의 문자 인코딩 매개변수다. |
| `multipart/form-data` | identifier | 다른 분야에만 있음: browser-platform — Blob file downloads | 파일 업로드 폼 미디어 타입이다. |

### Input field validation (`t635.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `superstruct` | proper_noun | 사전에 없음 | JavaScript 구조 검증 라이브러리다. |

### JSON format parsing (`t460.2`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `json lines` | phrase | 다른 분야에만 있음: files-io — JSON record formats | 줄마다 JSON 한 개를 담는 형식이다. |
| `JsonCpp` | proper_noun | 사전에 없음 | C++ JSON 라이브러리로 JSON 파싱 코드와 빌드 설정에 나온다. |
| `jsonl` | word | 다른 분야에만 있음: files-io — JSON record formats | JSON Lines 형식과 확장자다. |
| `ndjson` | word | 다른 분야에만 있음: files-io — JSON record formats | 줄 단위 JSON 형식이다. |
| `org.json` | proper_noun | 사전에 없음 | Java JSONObject를 제공하는 패키지다. |

### NumPy array files (`t800`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `savez_compressed` | identifier | 사전에 없음 | 압축된 .npz 저장 함수다. |

### Object serialization (`t460.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Moshi` | proper_noun | 사전에 없음 | Kotlin/Java JSON 라이브러리다. |

### Python object serialization (`t744.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `__reduce__` | identifier | 다른 분야에만 있음: data-engineering — Sequence folds | pickle 재구성 훅이다. |
| `__setstate__` | identifier | 다른 분야에만 있음: frontend-ui — Flutter widget state | pickle 복원 훅이다. |

### Schema registry serialization (`t635.6`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Confluent` | proper_noun | 다른 분야에만 있음: distributed-systems — Kafka client integration | Schema Registry 제공사다. |

### Tabular data exports (`t12.3`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Arrow` | proper_noun | 다른 분야에만 있음: data-engineering — Columnar formats | Apache Arrow다. |
| `csv.reader` | identifier | 다른 분야에만 있음: text-processing — CSV record I/O | Python csv 읽기 함수다. |
| `csv.writer` | identifier | 다른 분야에만 있음: text-processing — CSV record I/O | Python csv 쓰기 함수다. |
| `DictReader` | identifier | 다른 분야에만 있음: algorithms-data-structures — Dictionary entry operations | csv 딕셔너리 읽기 클래스다. |
| `Feather` | proper_noun | 다른 분야에만 있음: data-engineering — Columnar formats | Arrow IPC 파일 형식이다. |
| `read_csv` | identifier | 다른 분야에만 있음: data-engineering — Dataframe table operations | pandas CSV 읽기 함수다. |
| `.tsv` | extension | 다른 분야에만 있음: text-processing — CSV record I/O | 탭 구분 파일이다. |
| `.xlsx` | extension | 다른 분야에만 있음: frontend-ui — Table cells | Excel 확장자다. |

### Typed data adapters (`t635.4`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `BaseModel` | identifier | 다른 분야에만 있음: language-constructs — Model base classes | Pydantic 모델 기반 클래스다. |
| `parse_obj_as` | identifier | 사전에 없음 | Pydantic v1 어댑터 함수다. |

### XML parsing (`t508.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `JAXP` | abbreviation | 사전에 없음 | Java XML 처리 API다. |
| `RelaxNG` | proper_noun | 사전에 없음 | XML 스키마 언어다. |
| `Schematron` | proper_noun | 사전에 없음 | XML 규칙 검증 언어다. |
| `Xerces` | proper_noun | 사전에 없음 | XML 파서다. |

## 새 주제가 필요한 단어

### Native object archiving

범위: Archiving objects with platform-native serialization frameworks.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `NSKeyedArchiver` | identifier | 사전에 없음 | Apple 객체 아카이버다. |

### Schema evolution

범위: Evolving serialization schemas while preserving compatibility between readers and writers.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `backward compatibility` | phrase | 다른 분야에만 있음: build-release — API compatibility | 옛 스키마 호환성이다. |
| `schema evolution` | phrase | 다른 분야에만 있음: build-release — API compatibility | 스키마 변경 호환성이다. |
| `tolerant reader` | phrase | 사전에 없음 | 알 수 없는 필드를 무시하는 읽기 패턴이다. |
