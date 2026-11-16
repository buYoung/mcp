---
title: 측정 방법
description: 평가 설계, 비교 도구와 고정 버전, 세 가지 실행 환경, 샌드박스 격리 규칙, Grafana 사전 색인 규모를 정리합니다.
---

이 문서는 평가 구성 방식, 고정한 도구와 버전, 세 가지 에이전트 실행 환경의 차이, 실행 격리 규칙, 그리고 사전 구축한 Grafana 색인 규모를 차례대로 설명합니다.

## 평가 설계

모든 측정은 풀이 모델 학습 기준일 이후 커밋을 포함한 [grafana/grafana](https://github.com/grafana/grafana) 고정 읽기 전용 체크아웃(`c0a598f5a49abf2014f67e806075d8ad267c9efb`, `2026-10-26`, 추적 파일 `24,115`개, TypeScript + Go)에서 수행했습니다.

- **문항과 채점**: 다단계 아키텍처 문항 `3`개(`alertmanager-config-autosync`, `git-sync-job-dispatch`, `scope-annotation-filter`)를 문항당 필수 사실 `12`개(`Core` 종단간 필수 연결 `7`개 + `Extended` 부가 분기·설정 조건 `5`개)로 채점했습니다.
- **반복 횟수**: 환경·도구·문항마다 독립 `3`회씩 실행했습니다(셀당 답변 `9`개·채점 사실 `108`개, 단 `pi codemode`의 `zvec-grep 0.2.2`는 서버 중단 2건으로 답변 `7`개·사실 `84`개).
- **공통 지시문**: `rg 15.2.0`을 포함한 모든 비교군에 동일한 `path:시작-끝` 행 범위 인용 지시를 제공했습니다. 모든 MCP 비교군에는 MCP 도구를 우선 사용하고 MCP 서버가 처리할 수 없는 작업에만 내장 도구나 셸을 쓰도록 같은 지시를 주었습니다.

## 비교 도구와 고정 버전

각 도구는 색인 준비와 풀이 실행 전에 공식 업스트림 커밋 또는 릴리스로 고정했습니다.

| 도구·버전 | 공식 프로젝트·패키지 | 고정 커밋 | 탐색 방식과 노출 도구 |
| --- | --- | --- | --- |
| `codemap-search 0.14.2` | [buYoung/mcp](https://github.com/buYoung/mcp) (`codemap-search`) | `e727d20c6` | 단일 Rust 바이너리(노출 MCP 도구 6종): Tree-sitter 심볼·호출 추출 + Tantivy(BM25 전문 검색 엔진) `search` + 내장 `read`, `find`, `grep`, `initial_instructions`, `analyze`(작업공간 색인·읽기 이력 분석 도구, 풀이 미사용); 선택형 Jev LLM 관련성 필터(응답 전달 전 반환 코드의 작업 관련성을 판정하는 선택 기능) 비활성 |
| `Graphify 0.9.95` | [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify) (PyPI `graphifyy`, `v0.9.95`) | `92741d16` | Python 코드 그래프 MCP(그래프 도구 7종); 그래프 도구가 원문 소스 본문 없이 엔티티 메타데이터와 위치만 반환하므로 읽기 전용 MCP 보조 `read` 도구를 함께 연결 |
| `zvec-grep 0.2.2` | [zvec-ai/zvec-grep](https://github.com/zvec-ai/zvec-grep) (npm `@zvec/zvec-grep`, `v0.2.2`) | `b1a9148e` | Node.js 로컬 벡터·시맨틱 검색 MCP(`zvec_grep_search`, 임베딩 모델 `local/potion-code-16m-v2`); `zvec_grep_search`가 제한된 원문 발췌를 함께 반환하므로 보조 `read`는 붙이지 않음 |
| `codebase-memory-mcp 0.12.1` | [DeusData/codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) (`v0.12.1`) | `a7158ac2` | 코드베이스 그래프·코드 검색·조각 조회 MCP(도구 13종) |
| `rg 15.2.0` (기준군) | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) (`15.2.0`) | `e89fff89` | MCP 서버 없이 에이전트 내장 파일·검색 도구 또는 읽기 전용 셸만 사용 |
| `CodeGraph 1.6.2` | [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) (npm `@colbymchenry/codegraph`, `v1.6.2`) | `6560052a` | Node.js 코드 그래프 탐색 MCP(`codegraph_explore`) |
| `Serena 2.1.0` (제외) | [oraios/serena](https://github.com/oraios/serena) (PyPI `serena-agent`, `v2.1.0`) | `5e4f5cd1` | Go·TypeScript LSP 기반 MCP; Grafana 전체 LSP 색인이 1회 약 34~70분 걸려 측정 전 비교 대상에서 제외 |

`Graphify 0.9.95`는 자체 그래프 도구 7종(`query_graph`, `get_neighbors`, `get_node`, `graph_stats` 등)이 원문 본문 없이 엔티티 메타데이터와 위치만 돌려주므로, 하네스가 MCP 경로로 원문을 읽을 수 있게 읽기 전용 MCP 보조 `read`(`helper read`)를 연결했습니다. 반면 `zvec-grep 0.2.2`는 `zvec_grep_search`가 제한된 원문 발췌를 함께 반환하므로 보조 `read`를 붙이지 않았고, 더 넓은 파일 범위를 볼 때는 클라이언트 내장 도구나 읽기 전용 셸을 사용했습니다.

## 에이전트 클라이언트·모델·실행 환경

| 구성 요소 / 실행 환경 | 고정 버전 및 조건 |
| --- | --- |
| 평가 코드베이스 | [grafana/grafana](https://github.com/grafana/grafana) 커밋 `c0a598f5a49abf2014f67e806075d8ad267c9efb` (`2026-10-26`, 추적 파일 `24,115`개, TypeScript + Go, 풀이 모델 학습 기준일 이후 커밋 포함) |
| 풀이 모델 | `gpt-6.1-sol` (추론 강도 `medium`, 환경·도구·문항마다 독립 3회 실행) |
| 블라인드 채점 모델 | `gpt-6-astra` (추론 강도 `high`, 도구·저장소 접근 없음; 문항당 필수 사실 `12`개 대조 채점) |
| `pi direct` | `pi 1.4.0` (`--provider openai-codex --thinking medium`) + `@buyong/pi-mcp-adapter 0.1.2`. 일반 도구 호출 모드: 매 턴 MCP 도구를 직접 부르며, 저장소로 제한된 읽기 전용 내장 `read`, `grep`, `find`, `ls`(`bash` 없음)를 함께 사용 |
| `pi codemode` | 위 `pi 1.4.0` + `@buyong/pi-mcp-adapter 0.1.2` + `pi` 내장 `codemode` 확장(`@earendil-works/pi-coding-agent 1.4.0`, QuickJS 샌드박스). 스크립트 호출 모드: 모델이 짧은 JavaScript로 MCP·내장 도구를 묶어 부르고 출력을 추려 컨텍스트로 반환(기본 스크립트 출력 상한 `10,000`토큰) |
| `Codex code mode` | `Codex CLI 0.164.0` (`codex exec --json`, 기본 `code_mode_only`). JavaScript `exec`/`wait` 셀(Codex 코드 실행·비동기 대기 단위) 안에서 MCP 도구와 저장소로 제한된 읽기 전용 셸(`exec_command`)을 실행해 한 턴에 여러 호출을 묶어 처리(`exec` 기본 출력 상한 `10,000`토큰; `codemap-search`는 `// @exec: {"max_output_tokens": 100000}`을 안내) |

### 실행 환경을 합산하지 않는 이유

세 에이전트 환경(`pi direct`, `pi codemode`, `Codex code mode`)은 다음 세 축이 서로 다르므로 나란히 보고하되 합산하지 않습니다.

1. **기본 보조 도구**: `pi`는 `bash` 없이 저장소로 제한된 `read`, `grep`, `find`, `ls`를 제공하는 반면, `Codex code mode`는 `exec` JavaScript 셀 안에서 호출할 수 있는 읽기 전용 `exec_command` 셸을 제공합니다.
2. **출력 잘림 상한**: `pi` 내장 도구는 줄·바이트 한도(`read` `2,000`줄/`50 KiB`, `grep` `100`건, `find` `1,000`건, `ls` `500`건)를 적용하고, `@buyong/pi-mcp-adapter 0.1.2`는 `_meta["pi/maxOutputBytes"]` 지정이 없으면 기본 `20 KiB`에서 자르며, `pi codemode`는 스크립트 출력을 기본 `10,000`토큰에서, `Codex code mode`는 별도 지정이 없으면 `exec` 셀 출력을 기본 `10,000`토큰에서 자릅니다.
3. **프롬프트와 토큰 집계 방식**: `Codex code mode`는 매 턴 더 큰 개발자·코드 모드 프롬프트(`codemap-search` 첫 턴 입력 토큰 `9,748` 대 `pi direct` `5,118`, `pi codemode` `4,338`)를 포함해 대부분 캐시 상태로 다시 전달하므로 총 캐시 입력 토큰이 더 높게 집계됩니다.

### 샌드박스·격리 규칙

모든 실행은 고정된 Grafana 스냅샷의 읽기 전용 격리 체크아웃에서 외부 네트워크를 차단한 채 진행했습니다(하위 에이전트 없음, 소스 수정 금지). 두 클라이언트 모두 `.git`과 사전 구축된 도구 색인·상태 폴더(`.codemap`, `.codegraph`, `.zvec-grep`, `.codebase-memory`, `graphify-out`, `.pi`, `.codex`) 접근을 막았습니다.

- **`pi`**: 실행 전 `tool_call` 가드가 내장 `read`, `grep`, `find`, `ls`(직접 호출과 `codemode` 스크립트 내부 모두)의 경로 경계를 강제했고, `grep`과 `find` 바이너리는 macOS Seatbelt(macOS 운영체제 수준 프로세스 샌드박스) 안에서 실행했습니다.
- **`Codex CLI`**: `permissions.local_read`로 작업공간 밖 읽기와 차단 폴더 접근을 막고, MCP 서버는 로컬 macOS Seatbelt 격리 아래에서 실행했습니다.

## Grafana 사전 색인 규모와 구축 시간

고정 Grafana(`24,115`개 추적 파일)에서 읽기 전용 풀이를 실행하기 전 배치별로 1회 색인할 때 관측된 크기와 시간입니다. 구축 시간은 다섯 색인 도구 모두 동일하게 `timings.index_build_seconds`(`pi`·`Codex` 준비 과정의 사전·사후 상태 및 준비 확인 포함 전체 시간)로 통일했습니다.

| 도구 | 색인 파일·그래프 엔티티 수 | 색인 디스크 크기 | 색인 구축 시간 (`pi` / `Codex` 준비) |
| --- | --- | --- | --- |
| `codemap-search 0.14.2` | `19,757`개 파일 | `136.5–138.3 MiB` | `12.96 s` / `16.18 s` |
| `codebase-memory-mcp 0.12.1` | `21,430`개 추적 파일 (`525,952`개 노드; `pi` `1,562,241` / `Codex` `1,562,181`개 엣지) | `1,915.9–1,916.9 MiB` | `34.35 s` / `56.95 s` |
| `CodeGraph 1.6.2` | `17,094`개 파일 (`314,436`개 노드, `1,035,428`개 엣지) | `1,194.3 MiB` (`+ 1.9 MiB` 상태) | `88.82 s` / `101.00 s` |
| `zvec-grep 0.2.2` | `22,642`개 파일 (`175,403`개 엔티티) | `961.9 MiB` (`+ 32.0 MiB` 모델 캐시) | `147.68 s` / `154.22 s` |
| `Graphify 0.9.95` | AST 스캔 코드 `19,823`개 파일 (`160,008`개 노드, `867,900`개 엣지) | `626.8 MiB` | `311.73 s` / `366.08 s` |
| `rg 15.2.0` (기준군) | 없음 (실시간 디스크 검색) | `0 MiB` | `0 s` |
| `Serena 2.1.0` (제외) | Go + TypeScript LSP | — | `~34–70 min` (사전 제외) |

사전·사후 상태·준비 확인 명령(`0.2–8.9 s`)을 제외한 주 색인 CLI 단계만의 시간은 `codemap-search 0.14.2` `12.7 s` / `15.9 s`, `codebase-memory-mcp 0.12.1` `31.9 s` / `55.8 s`, `CodeGraph 1.6.2` `86.8 s` / `99.0 s`, `zvec-grep 0.2.2` `138.8 s` / `145.4 s`, `Graphify 0.9.95` `311.7 s` / `366.1 s`입니다.

## 관련 문서

- [측정 지표](/ko/benchmarks/metrics/) — 결과표 모든 열의 정의를 확인합니다.
- [측정 결과](/ko/benchmarks/results/) — 환경별 전체 수치표와 세부 채점 행렬을 봅니다.
- [재현 조건과 참고 사항](/ko/benchmarks/reproduction-notes/) — 실행 배치, 재실행 내역과 민감도를 확인합니다.
