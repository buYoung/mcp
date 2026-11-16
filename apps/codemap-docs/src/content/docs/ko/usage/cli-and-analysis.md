---
title: CLI 및 색인·읽기 분석
description: codemap-search CLI 서브커맨드와 커밋된 색인 용량 및 기록된 MCP 읽기 동작을 점검하는 analyze index / analyze reads 사용법을 설명합니다.
---

## CLI 서브커맨드

`codemap-search`는 MCP 서버와 함께 터미널에서 바로 실행할 수 있는 CLI 서브커맨드를 제공합니다.

```text
codemap-search mcp [--no-call-log]
codemap-search codex-config [--server-name NAME]
codemap-search analyze index [--path DIR] [--sort stored|size|lines|symbols|literals|path] [OPTIONS]
codemap-search analyze reads [--path DIR] [--days 1..30] [--sort reads|bytes|size|last|path] [OPTIONS]
codemap-search parse <file>
codemap-search tokenize <ident>
codemap-search codemap [--path P] [--format F]
codemap-search search <query> [-l N]
codemap-search index [dir]
codemap-search benchmark --queries <json> [--dir D]
```

## 인덱스와 읽기 동작 분석

`analyze index`는 소스를 다시 파싱하거나 색인을 갱신하지 않고 현재 커밋된 색인을 점검합니다. `analyze reads`는 기록된 MCP `read`, `search`, `grep` 호출 통계를 요약합니다. 보고서는 요청 시점에 생성되며, 바이트 수치는 모델 토큰 수나 물리적 디스크 읽기 횟수가 아닌 UTF-8 출력 크기입니다.

분석할 저장소 디렉터리에서 바로 실행하거나 `--path DIR`로 경로를 지정하세요.

```sh
codemap-search analyze index --sort size --limit 10
codemap-search analyze index --path /path/to/repo --language rust --filter src/
codemap-search analyze reads --sort bytes --limit 20
codemap-search analyze reads --days 14 --tool search --filter src/
codemap-search analyze reads --offset 20 --limit 20 --sort bytes
codemap-search analyze reads --view summary --format json
codemap-search analyze index --help
codemap-search analyze reads --help
```

| 옵션 | 적용 대상 | 동작 |
|---|---|---|
| `--path DIR` | 공통 | 분석할 저장소와 해당 색인 설정, 기록 DB를 선택 |
| `--limit N`, `-n N` | 공통 | 페이지당 표시할 파일 행 수(기본 20, `0`이면 전체 출력) |
| `--offset N` | 공통 | 필터와 정렬 후 건너뛸 파일 행 수(안내된 다음 페이지 오프셋과 함께 사용) |
| `--sort KEY`, `-s KEY` | 공통 | 인덱스: `stored`, `size`, `lines`, `symbols`, `literals`, `path`(기본 `stored`); 읽기: `reads`, `bytes`, `size`, `last`, `path`(기본 `reads`) |
| `--order asc\|desc` | 공통 | 기본값은 `path` 오름차순, 나머지 키는 내림차순 |
| `--filter TEXT`, `-f TEXT` | 공통 | 경로에서 대소문자를 구분해 일치시키는 부분 문자열(glob 아님) |
| `--view summary\|files\|full` | 공통 | 요약과 그룹, 요약과 파일 목록, 또는 전체 상세 표(기본 `full`) |
| `--format table\|json` | 공통 | 사람이 읽는 표(기본값) 또는 동일한 열 이름을 공유하는 압축 JSON |
| `--language NAME`, `-l NAME` | `index` | 색인된 언어로 필터링 |
| `--days N`, `-d N` | `reads` | 최근 1~30일 범위 지정(기본 7일) |
| `--tool read\|search\|grep`, `-t NAME` | `reads` | 특정 도구 호출만 집계 |
| `--no-compare` | `reads` | 직전 동일 기간과의 비교 생략(16일 이상 기간은 30일 보존 범위 안에서 비교 불가) |

## MCP `analyze` 도구

MCP `analyze` 도구는 현재 서버 작업공간을 대상으로 동일한 집계를 실행합니다.

```json
{"name":"analyze","arguments":{"target":"reads","sort":"bytes","limit":10}}
```

`target`에는 `index` 또는 `reads`를 지정합니다. 두 대상 모두 `limit`, `offset`, `sort`, `order`, `filter`, `view`를 받으며, `index`는 `language`, `reads`는 `days`, `tool`, `compare`를 추가로 받습니다.

MCP 기본값은 `view=files`, 파일 10행이며 `index`는 `stored`, `reads`는 `bytes` 기준으로 정렬합니다. `limit` 범위는 1~100이며 `page.next_offset`으로 다음 페이지를 조회하고, 전체 표가 필요하면 `view=full`을 요청합니다.

응답은 `summary` 객체와 표별 `columns`, `rows` 배열로 구성된 압축 JSON입니다. 키 이름에 바이트나 시간 단위를 명시하고 숫자와 `null` 자료형을 그대로 유지합니다. 응답 크기는 `8 KiB`(또는 더 작은 `output.max_bytes`) 이하로 제한되며, 초과 시 행 단위로 줄이고 `truncated`와 `omitted_tables`를 표시하되 합계와 `page.next_offset`은 보존합니다. 민감한 문자열은 직렬화 전에 가리며, `analyze` 호출 자체는 읽기 기록에 남기지 않습니다.

## 보고서 내용

| 구분 | 출력 내용 | 집계 기준 |
|---|---|---|
| 인덱스 용량 | 커밋된 파일, 세그먼트, 삭제 문서 수, 디스크 용량, 저장 JSON 크기, 정적 호출 및 참조 지점 수 | 기존 Tantivy 스냅샷을 메모리 SQLite에서 집계 |
| 언어와 심볼 | 언어별 파일, 줄, 심볼, 공개 심볼, 리터럴, 문서 문자열 수 및 심볼 종류별 테스트와 문서화 플래그 | 저장된 추출 메타데이터(미반영 편집이나 색인 제외 파일과 다를 수 있음) |
| 파일과 최신성 | 큰 저장 레코드의 경로, 파일 크기, 줄, 심볼, 리터럴, 최대 리터럴 크기와 변경, 삭제, 접근 불가 상태 | 파일시스템 크기와 수정 시각(mtime)만 대조(재파싱 및 색인 갱신 없음) |
| 현재/직전 기간 | 호출, 오류, 내용 반환 응답, 고유 파일, 읽은 횟수, 응답 및 결과량과 증감률 | 기본 최근 168시간과 직전 168시간 비교(기준 데이터 없으면 `n/a`) |
| 도구와 일별 추이 | 도구별 호출, 오류, 내용 반환, 고유 파일, 읽은 횟수, 응답 비중, 평균 및 최대 처리 시간과 UTC 일별 추이 | 기록된 `read`, `search`, `grep` 호출(양끝 날짜는 하루 일부만 포함될 수 있음) |
| 파일별 읽기 | 경로, 최신 기록 크기, 전체 및 도구별 읽은 횟수, 결과 반환량과 비중, 활동일, 마지막 시각, 반복 조회 요약 | 성공 응답 1건당 파일별 1회 읽기로 계산 |

합계는 현재 페이지가 아니라 조건에 맞는 전체 파일을 기준으로 계산합니다. `reads`에 경로 필터를 걸면 해당 파일을 반환한 호출이 선택되며, 이때 `Response`는 그 호출의 전체 응답 크기를, `Results`는 일치한 파일의 반환 크기만 합산합니다(파일 반환이 없는 오류 응답은 경로 필터에 걸리지 않습니다). 인덱스 용량과 세그먼트 지표는 파일 필터와 무관하게 전체 인덱스 기준으로 표시됩니다.

## 횟수와 바이트 해석

- **`Reads`**: 원문 줄이나 검색 발췌를 담아 성공적으로 반환한 응답마다 파일당 1회씩 셉니다. 경로 목록만 반환했거나 선언·관계만 본 호출, Jev 생략 안내만 남은 파일, 실패한 호출은 호출 수와 응답량에는 포함되지만 `Reads`에는 포함되지 않습니다.
- **반복 조회**: 분석 기간 안에서 같은 파일이 두 번째 이상 반환된 횟수입니다. 다른 줄 범위를 읽었거나 편집 후 다시 확인한 경우일 수 있으므로 낭비된 토큰을 뜻하지 않습니다.
- **`File size`**: 분석 기간 내 마지막으로 기록된 디스크 크기입니다. 크기를 알 수 없으면 `?`로 표시하고 합계에서 제외합니다.
- **`Results`**: 실제로 전달된 파일별 원문 또는 발췌 블록(줄 번호와 경로 접두사 포함)의 UTF-8 바이트 수입니다. 생략된 본문, Jev 생략 안내, 공통 중복 제거로 빠진 줄은 제외합니다(`find`는 중복 제거하지 않습니다).
- **`Response`**: 제목, 선언, 관계, 생략 안내까지 포함해 마스킹을 마친 최종 응답 본문 또는 오류 메시지의 UTF-8 바이트 수입니다(JSON-RPC 포장은 제외).
- **처리 시간**: 서버가 요청을 처리한 시간이며, SQLite 기록과 클라이언트·네트워크 시간은 제외합니다.

`find`, `analyze`, 내부 색인 작업은 기록하지 않습니다. 서버는 클라이언트 측 추가 잘림이나 실제 모델 토큰 소모량을 알 수 없으므로, 반환 바이트를 토큰 수로 환산하거나 반환된 모든 줄이 모델 컨텍스트에 들어갔다고 해석하지 마세요.

## 기록, 보존 및 실패 처리

MCP 서버는 질의어나 소스 원문, 응답 내용은 빼고 호출과 파일 관측 정보만 `.codemap/analysis.sqlite3`에 기록합니다. **보존 기간은 30일로 고정**되어 있어 연장할 수 없으며, 만료된 호출과 파일 기록은 서버 시작 시, 새 기록 추가 시, 분석 시, 그리고 서버 실행 중 매분마다 함께 삭제됩니다(서버가 꺼져 있었다면 다음 시작이나 분석 시 정리됩니다).

SQLite 데이터베이스는 `secure_delete`와 롤백 저널 삭제 방식을 사용해 삭제한 행을 여유 페이지나 상시 WAL 파일에 남기지 않습니다.

`codemap-search mcp --no-call-log`로 실행하면 새 호출 기록은 남기지 않으면서 만료 데이터 정리는 계속 수행합니다. 기록이나 정리에 실패하면 `stderr`에 경고를 남기고 MCP 응답은 정상 유지하며, `analyze` 명령은 DB 접근에 실패하면 오류를 보고합니다.

출력 한도는 [설정 레퍼런스](/ko/configuration/), 원문 생략 조건은 [선택적 Jev 판단 단계](/ko/configuration/#선택적-jev-판단-단계)를 참고하세요.
