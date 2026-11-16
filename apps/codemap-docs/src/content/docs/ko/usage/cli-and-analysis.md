---
title: CLI 및 색인·읽기 분석
description: CLI 서브커맨드와 analyze index / analyze reads 사용법을 정리합니다.
---

## CLI 서브커맨드

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

## 색인·읽기 분석 (`analyze`)

`analyze index`는 재색인 없이 현재 커밋된 색인 용량과 심볼 구성을 점검하고, `analyze reads`는 기록된 MCP `read`·`search`·`grep` 호출 통계를 요약합니다. 분석할 저장소에서 실행하거나 `--path DIR`를 지정하세요.

```sh
codemap-search analyze index --sort size --limit 10
codemap-search analyze index --path /path/to/repo --language rust --filter src/
codemap-search analyze reads --sort bytes --limit 20
codemap-search analyze reads --days 14 --tool search --filter src/
codemap-search analyze reads --view summary --format json
```

| 옵션 | 대상 | 설명 |
|---|---|---|
| `--path DIR` | 공통 | 분석할 저장소 디렉터리 |
| `--limit N`, `-n N` | 공통 | 페이지당 파일 행 수(기본 `20`, `0`이면 전체) |
| `--offset N` | 공통 | 페이지 이동 시 건너뛸 파일 행 수 |
| `--sort KEY`, `-s KEY` | 공통 | `index`: `stored`(색인 저장 크기, 기본), `size`(원본 크기), `lines`, `symbols`, `literals`, `path`<br>`reads`: `reads`(조회 수, 기본), `bytes`(반환된 원문·발췌 바이트), `size`(원본 크기), `last`(최근 조회 시각), `path` |
| `--order asc\|desc` | 공통 | 기본값은 `path` 오름차순, 나머지 키 내림차순 |
| `--filter TEXT`, `-f TEXT` | 공통 | 대소문자 구분 경로 부분 문자열 필터(glob 아님) |
| `--view summary\|files\|full` | 공통 | `summary`(합계와 언어·도구별 집계), `files`(합계와 파일 목록), `full`(전체 섹션, 기본) 선택 |
| `--format table\|json` | 공통 | 표(기본) 또는 압축 JSON 출력 |
| `--language NAME`, `-l NAME` | `index` | 색인된 언어로 필터링 |
| `--days N`, `-d N` | `reads` | 최근 `1–30`일 조회 기간(기본 `7`일) |
| `--tool read\|search\|grep`, `-t NAME` | `reads` | 특정 도구 호출만 집계 |
| `--no-compare` | `reads` | 직전 동일 기간과의 증감 비교 생략 |

## MCP `analyze` 도구

MCP `analyze` 도구는 현재 작업공간에 대해 같은 집계를 실행합니다. 기본값은 `view: "files"`, `limit: 10`(최대 `100`)이며, 다음 페이지를 조회하려면 응답의 `page.next_offset` 값을 `offset` 인자에 넘기세요.

```json
{"name":"analyze","arguments":{"target":"reads","sort":"bytes","limit":10}}
```

## 보고서 항목과 지표

| 섹션 | 표시 내용 |
|---|---|
| 색인 용량 (`index`) | 커밋된 파일·세그먼트 수, 디스크 크기, 색인 저장 크기, 호출·참조 수 |
| 언어와 심볼 (`index`) | 언어별 파일, 줄, 심볼, 공개 심볼, 리터럴, 문서 문자열 수 |
| 파일과 최신성 (`index`) | 색인된 파일의 경로, 원본 파일 크기, 심볼·리터럴 수, 수정·삭제 여부 |
| 현재/직전 기간 (`reads`) | 호출·오류 수, 고유 파일, `Reads`, `Response`/`Results` 바이트, 직전 기간 대비 증감 |
| 도구와 일별 추이 (`reads`) | 도구별 호출·오류·처리 시간과 UTC 일별 추이 |
| 파일별 읽기 (`reads`) | 파일별 `Reads`, `Results` 바이트, 마지막 조회 시각, 재조회(`Reads` 2회 이상) 횟수 |

- **`Reads`**: 원문 줄이나 검색 발췌를 반환한 성공 응답마다 파일당 1회로 셉니다.
- **`Results`와 `Response`**: `Results`는 전달된 파일별 원문·발췌의 UTF-8 바이트 수이고, `Response`는 제목·선언·생략 안내까지 포함한 전체 응답 텍스트의 UTF-8 바이트 수입니다.

호출 메타데이터는 질의어와 소스 원문을 제외하고 `.codemap/analysis.sqlite3`에 30일간 보관됩니다. 기록을 끄려면 `codemap-search mcp --no-call-log`로 실행하세요.
