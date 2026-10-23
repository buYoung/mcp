# 3단계: 매핑 확장 — 분야가 하나뿐인 단어에 주제 더하기

작업일: 2026-10-17. 대상 판: `rts4-3b9284a3b42c2917`. 결과 판: `rts4-86c5620af1f181ce`(`rt-sense/4`, SHA-256 `a5bf7d58099012cb07154c319181a7624992079a4e3700278b473e9ec10ed46e`, 48,631,808바이트).

분야가 하나뿐인 단어를 한 단어씩 보고, 설명에 다른 분야의 대상이 들어 있으면 그 분야의 주제를 더했다. 판정은 Claude Opus 5.5가 했다.

## 1. 대상

- **단어:** 분야가 정확히 하나인 단어 5,947개. 분야는 단어 자신의 뜻과 `input_senses`로 받는 뜻을 합쳐 센다(`sense_word_groups.word_fields`).
- **범위:** 표제어이거나, 최근 세 단계(`rts3-25df1cf41d0d8247` 누락 단어, `rts3-08b1780cc2880c26` 분야 연결, `rts3-5fba9ba33d7c7c7f` 새 개념)에서 들어온 단어다. 최근 단계 단어 1,919개, 원래 표제어 4,028개.
- 관련어로만 있는 단어(`input_senses`로만 분류된 단어)는 대상이 아니다.

## 2. 판정 기준

사용자와 정한 기준이다.

- 단어 설명에 그 분야의 대상이 들어 있으면 연결한다.
- 다른 분야가 더 주된 분야라는 것은 빼는 이유가 아니다.
- 한 단어를 여러 분야에 연결할 수 있다.

이를 적용한 방식은 다음과 같다.

- 설명에 그 분야의 대상이 분명히 있을 때만 더했다. 숫자, 식별자 조각, 기능어, 뜻이 넓은 일반어는 건너뛰었다.
- 단어가 이미 가진 분야의 다른 주제는 더하지 않았다. 이 단계는 단어를 다른 분야로 넓히는 일이다.
- 동음이의어도 연결했다. 예: `bst`(British Summer Time) → 시간대, `simplified`(간체 중국어) → 로캘, `Polly`(LLVM Polly) → 컴파일러 최적화, `config set`(Redis `CONFIG SET`) → Redis 명령.
- 같은 종류의 단어는 같은 주제로 모았다. 예: Azure 서비스 → `t109` Microsoft Azure, HTML 요소(`th`, `textarea`) → `t508.2` HTML markup elements, 웹 메시지 헤더(`X-RateLimit-Limit`, `WWW-Authenticate`, `ua`) → `t88.3` HTTP headers, 기능 플래그 도구(LaunchDarkly, Unleash) → `t21.2` Feature flags, TLS 관련 단어(`trust store`, `X.509`) → `t65.4` TLS handshake, Redis 벡터 집합 명령(`vadd`, `vsim` 등 11개) → `t665` Redis commands.

## 3. 도구

| 스크립트 | 하는 일 |
| --- | --- |
| `sense-mapping/prepare_mapping_sheets.py` | 대상 단어마다 현재 주제, 관련어, 그 단어를 관련어로 둔 단어, 후보 주제를 적은 시트를 만든다(시트당 100단어). 후보 주제는 관련어와 그 단어를 관련어로 둔 단어가 속한 주제, 이름에 그 단어가 든 주제에서 점수를 매기고, 단어가 이미 가진 분야의 주제는 뺀다 |
| `sense-mapping/collect_mappings.py topics` | 판정 A에서 분야만 고른 단어를 분야별 시트로 모은다. 분야의 모든 주제를 함께 적는다 |
| `sense-mapping/collect_mappings.py combine` | 판정 A·B를 `mappings.json`으로 합친다. 단어가 이미 가진 주제나 다른 분야의 주제를 고르면 거부한다 |
| `add_topic_mappings.py` | `mappings.json`의 매핑마다 persona(단어, 주제, 개념 없음, 가중치 0)를 더한 새 판을 만든다. 판 id는 대상 판과 매핑 내용에서 정해진다. 이미 있는 (단어, 주제)와 대상 판이 다른 매핑은 거부한다 |

판정은 두 번에 나눠 했다.

- **판정 A:** 단어마다 더할 주제 id를 적는다. 후보에 맞는 주제가 없으면 분야 id를 적는다. 형식은 `<단어 id> <주제 또는 분야> ...`.
- **판정 B:** 분야만 고른 단어마다 그 분야의 전체 주제 목록에서 하나를 고른다. 맞는 주제가 없으면 `-`로 적고, 이 쌍은 연결하지 않고 `mappings.json`의 `fields_without_topic`에 남긴다. 형식은 `<단어 id> <분야> <주제 또는 ->`.

## 4. 시험 판정

전체를 판정하기 전에 고정 시드로 뽑은 200단어(최근 단계 100, 원래 표제어 100)로 절차를 시험했다.

- 78단어에 87개를 매핑했다. 맞는 주제가 없는 분야 선택이 8건이었다(`SNTP`·`lora` → `networking`, `Samsung Internet` → `mobile-desktop`, `soak test` → `testing`, `reg multi sz` → `os-shell`, `kinesis firehose` → `cloud-infrastructure`, `escaped path` → `text-processing`, `appsettings.json` → `web-backend`).
- `sense-eval/compare_lookups.py`로 시험 판을 대상 판과 비교했다. 매핑한 78단어는 모두 관련어가 늘기만 했다(중앙값 +18.5). 매핑한 주제에 속한 다른 단어 14,729개 중 뜻이나 관련어를 잃은 단어는 없었다.
- 시험 판정은 전체 판정의 앞 200단어(`m00001`~`m00200`)로 그대로 썼다.

## 5. 전체 판정 결과

| 항목 | 수 |
| --- | ---: |
| 판정한 단어 | 5,947 |
| 하나 이상 더하기로 한 단어 | 1,015 (17.1%) |
| 판정 A에서 고른 항목 | 1,055 (후보 주제 588, 분야 467) |
| 판정 B에서 주제를 고른 분야 선택 | 378 |
| 맞는 주제가 없어 연결하지 않은 분야 선택 | 89 |
| 최종 매핑 | 966 (단어 934개) |
| 그중 최근 단계 단어 / 원래 표제어 | 303 / 663 |

분야별 매핑 수(많은 순): `build-release` 71, `web-backend` 69, `languages-compilers` 64, `databases-storage` 61, `networking`·`cloud-infrastructure` 48, `mobile-desktop`·`browser-platform`·`os-shell` 46, `security` 39, `concurrency-async` 35, 그 밖의 28개 분야가 4~28.

결과 판의 persona는 43,214개에서 44,180개가 됐다. 판 폴더에 다음을 남겼다.

| 파일 | 내용 |
| --- | --- |
| `mappings.json` | 매핑 966개(단어, 주제, 분야, 주제 이름, 집단, 판정 A 후보인지 B 분야 선택인지)와 `fields_without_topic` 89건 |
| `mapping-decisions-a.txt` | 판정 A 원본. 시트마다 `# sheet-NNN` 주석이 있다 |
| `mapping-decisions-b.txt` | 판정 B 원본. 분야마다 고른 기준을 주석으로 적었다 |

단어 id(`m00001`~`m05947`)는 `prepare_mapping_sheets.py`를 대상 판에 다시 돌리면 같은 순서로 나온다.

## 6. 맞는 주제가 없던 분야 선택

설명에는 그 분야의 대상이 있지만, 그 분야에 맞는 주제가 없어 연결하지 못한 89건이다. 6단계에서 주제를 새로 만들 후보다.

| 분야 | 건수 | 단어 |
| --- | ---: | --- |
| `cloud-infrastructure` | 16 | AWS 개별 서비스: `Amazon SNS`, `Amazon MQ`, `Amazon Polly`, `Amazon Translate`, `AWS CodeBuild`, `AWS CodeCommit`, `CloudFront`, `key management service`, `MWAA`, Kinesis 4개, `firehose`, `configuration set`, `Databricks` |
| `testing` | 9 | 부하·스트레스·soak 테스트(`load test`, `stress test`, `Gatling`, `JMeter` 등 7개), `Espresso`(모바일 UI 테스트), `dast` |
| `algorithms-data-structures` | 9 | 문자열 탐색(`Boyer-Moore`, `Knuth-Morris-Pratt`, `Manacher`, `string search algorithm`), 정수론(`greatest common divisor`, `extended Euclidean`, `binary exponentiation`), 근사 최근접 이웃(`ann`, `approximate nearest neighbor`) |
| `languages-compilers` | 9 | 자기 주제가 없는 언어: `COBOL`, `Crystal`, `Fortran`, `Lua`, `Zig`, `build.zig`, `vimscript`, `plsql`, `glsl` |
| `networking` | 8 | 시간 동기화(`SNTP`, `NTP server`, `chronyd`), 메일·일정 프로토콜(`imap`, `CalDAV`), `mib`(SNMP), `lora`, `adaptive bitrate` |
| `distributed-systems` | 5 | 분산 추적(`distributed tracing`, `Jaeger`), 분산 실행 프레임워크(`Ray`, `Orleans`), `bff` |
| 그 밖 | 33 | `browser-platform` 4(`atob`, `btoa`, `web worker`, `Web Workers`), `text-processing` 4(`nlp` 등), `language-constructs` 3(`async await`, `async fn`, `awaitable`), `business-domains`·`math-numerics` 각 3, 나머지 16개 분야 1~2 |

## 7. 6단계에 넘길 관찰

판정하면서 본, 주제나 분류가 틀린 사례다. 이번 단계에서는 고치지 않았다.

- **엉뚱한 분야에 들어간 주제:** `algorithms-data-structures`의 Bundle code splitting·Git repository cloning, `auth-identity`의 Square payment APIs·Gregorian calendars, `text-processing`의 Hardware control registers·Cron expressions·Google Pay payments, `web-styling`의 Biological sequence alignment(`t58.2`), `data-visualization`의 Linux kernel, `time-locale`의 Candlestick charts·Pseudorandom generation, `concurrency-async`의 Kafka consumer offsets, `docs-collaboration`의 Test doubles·Read locks, `files-io`의 Database read replicas, `networking`의 Liveness probes, `editors-ide`의 AMQP exchange declaration, `security`의 Recurring payment mandates·Maximum flow algorithms, `cloud-infrastructure`의 Debian package management·Browser vendor prefixes.
- **같은 이름의 주제가 두 개:** Greedy graph algorithms(`t479`, `t399`), Helm charts(`t361.4`, `t652`), MPI message passing(`x73`, `t497.3`), Route auth guards(`t309`, `t60`), Filesystem internals(`t480`, `t570`).
- **단어의 현재 주제가 틀림:** `Manacher` → Cloudflare platform, `Sorbet` → Python typing, `outb` → SIMD, `pgx` → Backup and restore, `Tkinter` → Flutter widget rebuilds, `TiDB` → Cloud deployment locations, `TortoiseSVN` → History rewriting. 이 밖에 `AWS CodeBuild`, `Fortran`, `COBOL`, `Citus`, `Dgraph`, `Underscore.js`, `asyncpg`.
- **주제 이름이 한 제품에 묶임:** `t56.10` Milvus vector storage에 `Qdrant`, `pgvector`를 넣었다. 벡터 저장소 일반 주제가 없어서다.

## 8. 한계

- 판정자는 모델 하나이고, 사람의 판정과 대조하지 않았다. 평가 세트(1단계)를 판정한 모델과 같은 모델이다.
- 평가 세트의 `target` 113단어는 모두 이번 대상에 들었고, 그중 25단어에 매핑을 더했다. 매핑 판정에는 평가 세트를 보지 않았다.
- 평가 세트에 최근 단계 단어는 5개뿐이라, 최근 단계 단어의 개선은 평가 세트로 거의 재지 못한다.
- 후보 주제는 관련어에서 뽑으므로, 관련어가 적은 단어는 후보가 비어 있다. 이때는 판정 A에서 분야를 직접 골랐다.
- 13단계에서 원래 제안된 분야에 연결하지 않은 5단어(`data drift`, `requireNonNull`, `Sublime Merge`, `EMFILE`, `negative testing`)는 이번에도 더하지 않았다. `Sublime Merge`만 이번 대상이었다. 원래 제안대로 바꿀지는 정하지 않았다.
