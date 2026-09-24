# 인덱스·읽기 동작 분석

한국어 | [English](./analysis.md)

`analyze index`는 소스를 다시 파싱하거나 색인을 갱신하지 않고 커밋된 색인을 분석합니다. `analyze reads`는 기록된 MCP `read`·`search`·`grep` 동작을 분석합니다. 보고서는 요청할 때 생성하며, 바이트 수는 토큰 수나 물리적 디스크 읽기 횟수가 아닙니다.

## CLI

분석할 저장소에서 실행하거나 `--path DIR`로 저장소를 지정합니다.

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
| --- | --- | --- |
| `--path DIR` | 공통 | 분석할 저장소와 해당 저장소의 색인 설정·기록 DB 선택 |
| `--limit N`, `-n N` | 공통 | 파일 표시 수; 기본 20, `0`이면 전부 |
| `--offset N` | 공통 | 필터·정렬 이후 파일 행을 N개 건너뛰기; 출력의 다음 페이지 안내 사용 |
| `--sort KEY`, `-s KEY` | 공통 | 인덱스: `stored`, `size`, `lines`, `symbols`, `literals`, `path`; 읽기: `reads`, `bytes`, `size`, `last`, `path`. 기본은 각각 `stored`, `reads` |
| `--order asc\|desc` | 공통 | 기본은 경로 오름차순, 나머지는 내림차순 |
| `--filter TEXT`, `-f TEXT` | 공통 | 경로에 포함된 대소문자 구분 문자열; glob 아님 |
| `--view summary\|files\|full` | 공통 | 요약·그룹, 요약·파일, 전체 상세 보기; 기본 `full` |
| `--format table\|json` | 공통 | 사람이 보는 표(기본값) 또는 열 이름을 공유하는 압축 JSON |
| `--language NAME`, `-l NAME` | `index` | 색인된 언어로 필터링 |
| `--days N`, `-d N` | `reads` | 최근 1~30일; 기본 7일 |
| `--tool read\|search\|grep`, `-t NAME` | `reads` | 특정 도구만 집계 |
| `--no-compare` | `reads` | 직전 동일 길이 기간 비교 생략; 16일 이상은 30일 보존 범위 안에서 비교 불가 |

## MCP

`analyze` 도구는 같은 집계 방식으로 현재 서버의 작업공간을 분석합니다.

```json
{"name":"analyze","arguments":{"target":"reads","sort":"bytes","limit":10}}
```

`target`은 `index|reads`입니다. 공통으로 `limit`, `offset`, `sort`, `order`, `filter`, `view`를 받으며, `index`는 `language`, `reads`는 `days`, `tool`, `compare`도 받습니다.

MCP 기본은 `view=files`, 파일 10개이며 인덱스는 `stored`, 읽기는 `bytes` 순입니다. `limit`은 1~100이고 `page.next_offset`으로 이어서 조회합니다. 더 넓은 분석은 `view=full`로 요청합니다.

응답은 `summary`와 표별 `columns`·`rows` 배열을 사용합니다. 바이트·시간 단위를 키에 표시하고 숫자·`null`을 유지합니다. 최대 8 KiB 또는 더 작은 `output.max_bytes` 안에서 완전한 행 단위로 줄입니다. 잘림은 `truncated`, 생략한 표는 `omitted_tables`로 알리며 합계와 다음 페이지 위치는 유지합니다. 민감정보는 JSON 직렬화 전에 가립니다. 분석 호출 자체는 읽기 기록에 추가되지 않습니다.

## 보고서 내용

| 구분 | 출력 내용 | 집계 기준 |
| --- | --- | --- |
| 인덱스 용량 | 커밋된 파일·세그먼트·삭제 문서 수, 디스크 용량, 저장 JSON 크기, 정적 호출·참조 지점 수 | 기존 Tantivy 스냅샷을 읽어 메모리 SQLite에서 집계 |
| 언어·심볼 | 언어별 파일·행·심볼·공개 심볼·리터럴·문서 문자열 수, 심볼 종류별 테스트·문서화 플래그 | 저장된 추출 결과; 전체 저장소 파일이나 최신 소스와 다를 수 있음 |
| 파일·최신성 | 큰 저장 레코드의 경로·파일 크기·행·심볼·리터럴·최대 리터럴 크기, 변경·삭제·접근 불가 상태 | 크기와 변경 여부만 현재 파일 메타데이터로 확인; 소스 재파싱·색인 갱신 없음 |
| 현재/직전 기간 | 호출·오류·내용 반환 응답·고유 파일·읽은 횟수·응답량과 증감률 | 기본 최근 168시간과 직전 168시간; 기준값이 없으면 `n/a` |
| 도구·일별 | 도구별 호출·오류·내용 반환·고유 파일·읽은 횟수·응답량 비중·평균/최대 처리 시간, UTC 날짜별 추이 | 기록된 `read`·`search`·`grep` 호출; 날짜 표의 양끝은 하루 일부일 수 있음 |
| 파일별 읽기 | 경로·최신 기록 크기·전체/도구별 읽은 횟수·결과 반환량·비중·활동일·마지막 시각, 반복 조회 요약 | 한 성공 응답에서 같은 파일은 한 번만 계산 |

합계는 표시 페이지가 아닌 필터에 맞는 전체 파일을 포함합니다. 읽기 경로 필터는 해당 파일을 반환한 호출을 선택합니다. 이때 `Response`는 선택된 호출의 전체 응답량이며 `Results`는 일치한 파일의 반환량입니다. 파일 관측이 없는 오류는 경로 필터 결과에 포함되지 않습니다. 인덱스의 전체 디스크 용량·세그먼트 지표는 파일 필터와 무관한 전체 인덱스 값입니다.

## 횟수와 바이트 해석

- **`Reads`**는 성공 응답에 원문 행이나 검색 발췌가 포함된 파일마다 1회입니다. 경로 목록·선언/관계 전용 보기·실패 호출은 호출/응답량에는 포함되지만 읽은 횟수에는 포함되지 않습니다. Jev 생략 안내만 남은 파일도 원문을 읽은 것으로 세지 않습니다.
- **반복 조회**는 같은 파일의 첫 반환 이후 횟수입니다. 다른 구간이나 변경된 내용일 수 있으므로 낭비된 토큰으로 해석하지 않습니다.
- **`File size`**는 분석 기간 안에서 마지막으로 기록한 디스크 크기입니다. 알 수 없으면 `?`로 표시하고 크기 합계에서 제외합니다.
- **`Results`**는 실제 전달한 파일별 원문/발췌 블록의 UTF-8 바이트 수로 행 번호·경로 접두사를 포함합니다. 소스 단계 마스킹과 search의 Jev 렌더링 전 선택 이후, 응답 전체 마스킹 이전에 계산합니다. 생략한 본문과 Jev 생략 안내는 전달한 원문에 포함하지 않습니다. Read와 grep은 원본 소스 경로를 사용하며 Jev로 필터링하지 않습니다.
- **`Response`**는 마스킹 후 최종 응답 본문 또는 오류 메시지의 UTF-8 바이트 수입니다. 선언·관계·제목·생략 안내를 포함하고 JSON 포장은 제외합니다.
- **처리 시간**은 서버의 요청 처리 시간이며 SQLite 기록과 클라이언트/네트워크 시간은 제외합니다.

`find`·`overview`·`analyze`와 내부 색인 읽기는 기록하지 않습니다. 클라이언트의 추가 잘림과 모델의 실제 소비량은 관측하지 못합니다. 반환 바이트를 모델 토큰 수로 환산하거나 반환된 모든 줄을 모델이 사용했다고 가정하지 마세요.

## 기록·보존·실패 처리

MCP는 호출과 파일별 관측을 `.codemap/analysis.sqlite3`에 저장하며 질의·소스·응답 내용은 저장하지 않습니다. **보존 기간은 30일 고정이며 연장할 수 없습니다.** MCP 시작·새 기록·분석 시, 그리고 MCP 실행 중 매분 만료 호출과 연결된 파일 기록을 함께 삭제합니다. MCP가 꺼져 있는 동안 만료된 기록은 다음 실행 또는 분석 시 삭제됩니다.

`secure_delete`와 삭제형 rollback journal을 사용해 삭제된 행을 DB 여유 페이지나 상시 WAL에 남기지 않습니다. 이 정책은 관리 대상 DB에 적용되며 별도 백업까지 삭제하지는 않습니다.

`codemap-search mcp --no-call-log`는 새 기록만 끄며 기존 DB의 만료 정리는 계속합니다. 저장/정리 실패는 stderr에 알리고 MCP 응답은 유지하며 다음 작업에서 재시도합니다. 분석 명령은 DB에 접근하지 못하면 오류를 보고합니다. 기록 중단 기간은 복원할 수 없고 기간 비교도 연속 수집을 보장하지 않습니다.

출력 한도는 [설정](./configuration.ko.md), 원문 생략 조건은 [Jev 필터](./configuration.ko.md#선택적-jev-판단-단계)를 참고하세요.
