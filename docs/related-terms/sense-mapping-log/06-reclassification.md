# 6단계: 카테고리 재분류와 추가

작업일: 2026-10-17. 전 판: `rts4-86c5620af1f181ce`(3단계 결과, 5단계부터 기본값). 후 판: `rts4-0fb3d5bc20c215ba`(재분류) → `rts4-e5083ab344e1336d`(재분류 + 매핑).

4단계에서 매핑이 조회를 해치지 않는 것을 확인한 뒤, 주제(카테고리)의 분야와 이름을 다시 매기고, 같은 대상의 주제를 합치고, 빠진 주제를 더했다. 판정은 Claude Opus 5.5가 했다. 검증은 [7단계](07-verification.md)에 있다.

## 1. 입력

- **주제 시트:** `sense-mapping/prepare_topic_sheets.py`로 주제 2,781개를 주 분야 순으로 100개씩 시트 28장에 적었다. 시트마다 주제의 분야(순위순), 이름, 범위, 개념, 단어(가중치순 12개), 단어들이 가진 다른 주제의 주 분야, 형제 주제(점 앞 번호가 같은 주제)를 보인다.
- **중복 시트:** 이름이 같은 주제(대소문자 무시)를 모은 `duplicates.md`.
- **3단계에서 넘긴 것:** [03-mapping.md](03-mapping.md) 6절의 주제가 없어 연결하지 못한 89쌍(단어, 분야)과 7절의 관찰, 13단계에서 원래 제안된 분야에 연결하지 않은 5단어.

## 2. 판정 기준

- 주 분야는 주제의 대상이 속한 분야로 정한다. 주제를 나눌 때 형제 주제에서 물려받은 엉뚱한 분야는 바꾼다. 예: `version-control`의 Maven repositories, `web-styling`의 Biological sequence alignment.
- 그럴듯한 분야는 바꾸지 않는다. 대상이 두 분야에 걸치면 보조 분야를 더한다.
- "X wording" 주제의 주 분야는 `general-wording`이다.
- 단어 하나짜리 주제가 다른 주제와 대상이 같으면 그 주제에 합친다. 대부분은 그 단어가 합칠 주제에 이미 있다.
- 단어가 없는 빈 주제는 지운다.
- 새 주제는 단어 두 개 이상을 담을 때만 만든다. 먼저 기존 주제의 이름과 범위를 넓혀 담을 수 있는지 본다.
- 판정할 때 평가 세트의 답은 보지 않았다.

## 3. 판정 결과

판정은 하나의 계획 파일(`revisions.json`)과 매핑 파일(`mappings.json`)로 모았다. 두 파일은 판 폴더에 복사되어 있고, 항목마다 판정 이유(`reason`)가 있다.

| 연산 | 수 | 내용 |
| --- | ---: | --- |
| 분야(`fields`) | 617 | 주 분야를 바꾼 주제 569개, 보조 분야만 바꾼 주제 48개 |
| 이름(`labels`) | 22 | 합친 뒤 범위가 넓어진 주제, 이름이 단어와 맞지 않던 주제 |
| 합치기(`merges`) | 229 | 대상이 같은 주제를 합쳤다. 이름이 같은 주제 40쌍 포함 |
| 단어 옮기기(`moves`) | 18 | 엉뚱한 주제에 들어간 단어 |
| 지우기(`drops`) | 15 | 단어가 없는 빈 주제 전부 |
| 새 주제(`new_topics`) | 8 | `x307`~`x314` |
| 매핑 | 52 | 단어 51개. persona 50개와 입력 전용 단어의 연결 2개 |

| 지표 | 전 판 | 후 판 |
| --- | ---: | ---: |
| 주제 | 2,781 | 2,545 |
| 분야가 둘 이상인 주제 | 489 | 646 |
| 단어 하나짜리 주제 | 473 | 316 |
| 빈 주제 | 15 | 0 |
| persona | 44,180 | 44,223 |

- 합치기에서 한 주제에 같은 단어의 개념 없는 persona가 둘 생긴 7건은 하나로 합쳤다. 단어를 옮긴 뒤 단어가 남지 않은 개념 `secret rotation` 하나가 지워졌다.
- 단어별로 보면 표제어 15,755개 중 3,297개의 주 분야 집합이 바뀌었다. 2,008개는 분야를 얻었고, 2,868개는 분야를 하나 이상 잃었다. 주 분야를 바꾼 주제 569개의 단어가 새 분야로 따라갔기 때문이다.

### 이름을 바꾼 주제 예

| 주제 | 새 이름 | 이유 |
| --- | --- | --- |
| `t84.3` | Load and stress testing | 부하·스트레스 테스트 새 주제 대신 넓혔다 |
| `t685.2` | String search algorithms | 문자열 탐색 새 주제 대신 넓혔다. `Manacher`를 옮겨 왔다 |
| `t617` | Number theory algorithms | 정수론 새 주제 대신 넓혔다 |
| `t838` | PostgreSQL tools and clients | 단어가 모두 PostgreSQL 도구인데 이름이 Backup and restore였다 |
| `t154` | AWS developer tools | 단어가 AWS CI/CD 서비스인데 이름이 Secrets management였다 |
| `t495.1` | Domain-driven design models | DDD 단어가 함께 있다. `bounded context` 주제를 합쳤다 |
| `t1044.1` | Vim and Neovim | 단어가 Vim·Neovim 전반이다 |

### 새 주제

| 주제 | 분야 | 이름 | 단어 |
| --- | --- | --- | --- |
| `x307` | `languages-compilers` | Fortran and COBOL | `Fortran`, `COBOL`(옮김) |
| `x308` | `languages-compilers`, `systems-low-level` | Zig language | `Zig`, `build.zig` |
| `x309` | `cloud-infrastructure`, `data-engineering` | AWS streaming data services | Kinesis 관련 7개 |
| `x310` | `cloud-infrastructure`, `distributed-systems` | AWS messaging services | `Amazon SNS`, `Amazon SQS`, `Amazon MQ` |
| `x311` | `cloud-infrastructure`, `ml-ai` | AWS AI services | `Amazon Polly`, `Amazon Translate` |
| `x312` | `languages-compilers` | Lua language | `Lua`, `LuaJIT` |
| `x313` | `data-visualization`, `web-backend` | Data app frameworks | `Shiny`, `Streamlit`, `Gradio` |
| `x314` | `language-constructs`, `concurrency-async` | Async function syntax | `async await`, `async fn`, `awaitable`, `async function` |

### 옮긴 단어

[03-mapping.md](03-mapping.md) 7절에서 주제가 틀렸다고 본 단어를 옮겼다.

| 단어 | 전 주제 | 후 주제 |
| --- | --- | --- |
| `Manacher` | `t426` Cloudflare platform | `t685.2` String search algorithms |
| `Sorbet` | `t125.3` Python typing | `t918` Ruby development ecosystem |
| `outb`, `inb` | `t22.2` SIMD processing | `t697` Bare-metal hardware access |
| `TiDB` | `t618.3` Cloud deployment locations | `t150.2` Database sharding |
| `Citus` | `x83` Consistency models and replication | `t150.2` Database sharding |
| `AWS CodeBuild` | `t20.6` Secret stores | `t28.1` CI build pipelines |
| `Fortran` | `t717.1` Unsupervised clustering | `x307` |
| `COBOL` | `t244.2` Code refactoring | `x307` |
| `Underscore.js` | `t744.4` Cyclic object graphs | `t809` Functional TypeScript libraries |
| `asyncpg` | `t129.1` Async event loops | `t838` PostgreSQL tools and clients |
| `Kafka`, `Apache Kafka` | `t294.2` 커밋 기록 주제 | `t85.1` Kafka client integration |
| `rotated`, `ansible-vault` | `t154` | `t20.4` API key rotation, `t20.6` Secret stores |
| `status line`, `object result`, `vscode.window` | `t973`(이름은 HTTP status codes, 단어는 편집기 상태 표시줄) | `t412.3`, `t655.3`, `t31.1`. 남은 `status bar`는 `t672.2`에 합쳤다 |

`pgx`, `Tkinter`, `TortoiseSVN`, `Dgraph`는 관계어로만 있고 persona가 없는 단어였다. 각각 `t838`, `x147` Desktop UI toolkits, `t140.9` Subversion revisions, `t977.1` Graph database queries에 매핑했다.

## 4. 주제가 없던 89쌍과 13단계 5단어

94쌍 중 77쌍이 요청한 분야를 가진 주제에 연결됐다(그중 42쌍은 주 분야). 전 판에서는 14쌍이었다. 방법은 세 가지다.

- 단어가 있는 주제의 분야를 고쳤다. 예: `t84.3`의 주 분야를 `testing`으로 해 `soak test`, `JMeter` 등 7단어가 `testing`을 얻었다. `t694.2`에 `distributed-systems`를 더해 `distributed tracing`, `Jaeger`가 연결됐다.
- 새 주제나 넓힌 주제에 매핑했다. 예: Kinesis 단어 → `x309`, `binary exponentiation` → `t617`.
- 13단계 5단어는 원래 제안된 분야의 주제에 연결했다: `data drift` → `t635.1` Data quality checks, `Sublime Merge` → `t984` Sublime Text projects, `EMFILE` → `t236.1` Socket connection failures. `requireNonNull`과 `negative testing`은 표제어가 아니라 입력 전용 단어여서, 그 주제의 persona에 입력 연결을 더했다(`t635.2` Input field validation의 `data validation`, `t251.1` Penetration testing의 `fuzzing`). `t635.2`의 주 분야가 `web-backend`라 `requireNonNull`의 새 뜻은 `web-backend`로 보이고, `data-formats`는 보조 분야다.

대상은 맞지만 요청과 다른 분야의 주제에 매핑한 5쌍:

| 단어 | 요청 분야 | 매핑한 주제 |
| --- | --- | --- |
| `bezier curve` | `math-numerics` | `t108.4` Vector graphics(`graphics-media`) |
| `CalDAV` | `networking` | `t673.1` Calendar data(`time-locale`) |
| `Mathlib` | `math-numerics` | `t173.6` Formal verification(`type-systems`) |
| `MWAA` | `cloud-infrastructure` | `t429.1` Data workflow orchestration(`data-engineering`) |
| `vimscript` | `languages-compilers` | `t1044.1` Vim and Neovim(`editors-ide`) |

연결하지 않은 12쌍. 맞는 주제가 없고, 단어 하나로는 새 주제를 만들지 않았다.

| 단어 | 요청 분야 | 현재 주제와 이유 |
| --- | --- | --- |
| `Samsung Internet` | `mobile-desktop` | `t210` Browser engine identification. 모바일 브라우저 주제가 없다 |
| `lora` | `networking` | 현재 뜻은 ML의 LoRA. LoRa 무선 주제가 없다 |
| `appsettings.json` | `web-backend` | `t99` Application configuration. ASP.NET 설정 주제가 없다 |
| `configuration set` | `cloud-infrastructure` | `t532.3` Email transmission. Amazon SES 주제가 없다 |
| `Crystal` | `languages-compilers` | 현재 뜻은 `x164` Clock and reset circuitry(수정 발진기). Crystal 언어 주제가 없다 |
| `Databricks` | `cloud-infrastructure` | `t67.2` Spark distributed analytics |
| `fraud detection` | `security` | `t170.2` Payment gateway checkout. 사기 탐지 주제가 없다 |
| `gradient descent` | `math-numerics` | `t78.1` Neural training loops. 수치 최적화 주제가 없다 |
| `latch` | `systems-low-level` | 현재 뜻은 동시성 latch. 디지털 회로 주제가 없다 |
| `mib` | `networking` | SNMP 주제가 없다 |
| `Ray` | `graphics-media` | 광선 추적 주제가 없다. 분산 프레임워크 뜻은 `x137`(`distributed-systems` 보조)에 있다 |
| `bpmn` | `business-domains` | `t305.2` Workflow execution. 비즈니스 프로세스 주제가 없다 |

## 5. 도구

| 파일 | 바꾼 것 |
| --- | --- |
| `revise_topics.py` | `drops` 연산을 더했다. 전 판에서 persona도 개념도 없는 주제만 지우고, 같은 주제를 다른 연산이 가리키면 거부한다 |
| `add_topic_mappings.py` | `via`를 단 매핑을 더했다. 표제어가 아닌 입력 전용 단어를, 그 주제에 있는 `via` 단어의 persona에 `input_senses`로 연결한다 |
| `sense-eval/compare_lookups.py` | `--revisions`를 더했다. 전 판의 뜻을 계획대로 바뀐 뜻(합친 주제, 후 판의 주 분야)으로 바꿔 비교한다. `--mapped-by revisions.json --affected`는 합치거나 단어를 옮기거나 분야를 바꾼 주제의 단어를 모두 비교한다 |

판정 원본(시트별 판정 텍스트)은 작업 공간에만 두었고, 그것을 합쳐 충돌을 검사한 결과가 `revisions.json`이다. 합칠 때 확인한 것: 한 주제를 두 곳에 합치지 않는다, 합쳐질 주제의 분야나 이름을 고치지 않는다, 합친 주제로 다시 합치는 사슬은 끝 주제로 잇는다(1건), 합쳐질 주제로 단어를 옮기지 않는다. 시트 판정 중에 생긴 충돌 6건(같은 주제의 분야를 두 시트에서 다르게 적거나, 합쳐질 주제의 분야를 적은 것)은 정리했다.

## 6. 만든 판

```sh
cd apps/codemap-search/experiments/related-terms-poc
python3 revise_topics.py --base datasets/rts4-86c5620af1f181ce --plan revisions.json --output-root datasets
python3 add_topic_mappings.py --base datasets/rts4-0fb3d5bc20c215ba --mappings mappings.json --output-root datasets
```

| 판 | 내용 | 크기 |
| --- | --- | ---: |
| `rts4-0fb3d5bc20c215ba` | 재분류. `revisions.json`, `revision-outcome.json`, `build-report.json` | 48,611,328바이트 |
| `rts4-e5083ab344e1336d` | 재분류 판에 매핑 52개. `mappings.json`, `build-report.json` | 48,611,328바이트 |

## 7. 한계

- 판정은 시트에 보이는 단어 12개와 개념 이름으로 했다. 단어가 많은 주제는 보이지 않는 단어가 다른 대상일 수 있다.
- 뜻의 분야는 주제의 주 분야 하나로만 보인다. 보조 분야를 더해도 조회 결과의 분야는 바뀌지 않는다.
- 새 주제는 3단계에서 넘긴 단어를 담는 데 필요한 것만 만들었다. 시트 판정에서 주제가 없다고 본 대상(SNMP, 광선 추적 등)은 단어가 하나뿐이라 만들지 않았다.
