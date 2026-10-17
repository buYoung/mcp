# Configuration and environments 누락 단어

- 분야 id: `configuration`
- 원본: [../configuration.md](../configuration.md)
- 누락 단어 20개 · 사전에 없음 3 · 뜻 미배정 0 · 다른 분야에만 있음 17
- 기준: 데이터셋에 없거나, 분야 없이 있거나, 다른 분야에만 있는 단어만 남겼습니다. 다른 단어의 관련어로 이미 들어 있는 단어는 뺐습니다.

## 기존 주제에 넣을 단어

### Application configuration (`t99`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `application.properties` | extension | 다른 분야에만 있음: web-backend — Spring Boot configuration | Spring Boot 설정 파일로 이 분야 범위에 명시된 properties 설정의 대표 이름입니다. |
| `application.yml` | identifier | 다른 분야에만 있음: web-backend — Spring Boot configuration | Spring Boot 설정 파일로 이 분야 범위에 명시된 application.yml의 대표 이름입니다. |

### Configuration loading (`t289.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `.yaml` | extension | 다른 분야에만 있음: data-formats — YAML parsing | 설정 파일 확장자로 쓰이며 설정 파일 검색에서 자주 지정됩니다. |
| `.yml` | extension | 다른 분야에만 있음: data-formats — YAML parsing | 설정 파일 확장자로 쓰이며 설정 파일 검색에서 자주 지정됩니다. |

### Configuration value access (`t366`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `dotted key` | phrase | 다른 분야에만 있음: data-formats — JSON patch paths | spring.datasource.url처럼 점으로 중첩 설정 키를 가리키는 표기입니다. |

### Default configuration (`t364`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `default value` | phrase | 다른 분야에만 있음: type-systems — Default value initialization | 설정 키의 기본값을 가리키는 말로 설정 문서와 코드에 쓰입니다. |

### Deployment environment configuration (`t458.1`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `debug` | word | 다른 분야에만 있음: languages-compilers — Debug build symbols | debug 대 release 모드 구분의 한 쪽으로 설정과 빌드 옵션에 쓰입니다. |
| `debug build` | phrase | 다른 분야에만 있음: languages-compilers — Debug build symbols | debug 모드로 만든 빌드를 가리켜 환경별 설정 분기에 쓰입니다. |
| `profile` | word | 다른 분야에만 있음: auth-identity — User profiles | dev, prod 같은 환경 프로필을 가리키며 설정 파일과 빌드 도구에 쓰입니다. |
| `twelve-factor app` | phrase | 사전에 없음 | 설정을 환경 변수로 분리하자는 twelve-factor 방법론을 가리킵니다. |

### Remote configuration stores (`t557`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `Azure App Configuration` | proper_noun | 다른 분야에만 있음: algorithms-data-structures — Key-value stores | Azure의 중앙 설정과 기능 플래그 저장 서비스입니다. |
| `Consul` | proper_noun | 다른 분야에만 있음: distributed-systems — Service discovery | Consul KV를 원격 설정 저장소로 쓰는 경우가 많습니다. |
| `Nacos` | proper_noun | 다른 분야에만 있음: distributed-systems — Service discovery | 이 분야 범위에 명시된 중앙 설정 서비스입니다. |

### Runtime configuration reload (`t908`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `hot reload` | phrase | 다른 분야에만 있음: files-io — File watching/hot reload | 재시작 없이 바뀐 설정을 반영하는 동작에도 쓰입니다. |

### User configuration directories (`t661`)

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `home dir` | phrase | 다른 분야에만 있음: files-io — Workspace paths | 사용자 홈 디렉터리 아래 설정 파일 위치를 찾을 때 쓰입니다. |
| `Library/Application Support` | identifier | 사전에 없음 | macOS에서 사용자 앱 설정을 두는 표준 디렉터리 경로입니다. |
| `ProjectDirs` | identifier | 사전에 없음 | Rust directories 크레이트에서 앱별 설정 경로를 구하는 타입입니다. |

## 새 주제가 필요한 단어

### Masked configuration values

범위: Masking or redacting secret and sensitive configuration values in output and logs.

| 단어 | 종류 | 현재 상태 | 이유 |
| --- | --- | --- | --- |
| `masked` | word | 다른 분야에만 있음: security — Secret redaction | 설정 값을 별표 등으로 가려 보여 주는 상태를 가리킵니다. |
| `redact` | word | 다른 분야에만 있음: security — Secret redaction | 설정 덤프나 로그에서 비밀 값을 지우는 동작을 가리킵니다. |
| `sensitive` | word | 다른 분야에만 있음: security — Secret redaction | 비밀 설정 값에 붙는 속성 이름으로 Terraform 변수 등에 쓰입니다. |
