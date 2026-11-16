---
title: 재현 조건과 참고 사항
description: 고정된 저장소·도구 커밋, 사전 구축 색인 스냅샷, 실행 배치·재실행 내역, 환경 민감도와 커밋 자료·비공개 기준 데이터를 정리합니다.
---

이 문서는 Grafana 평가에서 고정한 식별 정보, 실행 배치와 재실행 내역, 공개된 자료와 비공개로 유지하는 기준 데이터의 범위를 정리합니다.

## 고정 저장소·도구·모델·클라이언트 버전

| 구성 요소 | 고정 식별 정보 및 버전 | 커밋 / 패키지 고정 |
| --- | --- | --- |
| 평가 코드베이스 | [grafana/grafana](https://github.com/grafana/grafana) (`24,115`개 추적 파일, TypeScript + Go, `2026-10-26`) | `c0a598f5a49abf2014f67e806075d8ad267c9efb` |
| `codemap-search 0.14.2` | [buYoung/mcp](https://github.com/buYoung/mcp) (`codemap-search`, 선택형 Jev 필터 비활성) | `e727d20c6504948c823cf55398a2f68d45337f7c` (`e727d20c6`) |
| `Graphify 0.9.95` | [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify) (PyPI `graphifyy`, `v0.9.95`) | `92741d162b2094ffc2fed58cfbd8ee823e9da62c` (`92741d16`) |
| `zvec-grep 0.2.2` | [zvec-ai/zvec-grep](https://github.com/zvec-ai/zvec-grep) (npm `@zvec/zvec-grep`, `v0.2.2`, 임베딩 `local/potion-code-16m-v2`) | `b1a9148e26a7bc9bd4a52229ffb7532d3063793d` (`b1a9148e`) |
| `codebase-memory-mcp 0.12.1` | [DeusData/codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) (`v0.12.1`) | `a7158ac2f48ca646b4527341b76f930f2aeff87e` (`a7158ac2`) |
| `rg 15.2.0` (기준군) | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) (`15.2.0`) | `e89fff89ac9af12e8d4ce9d5fd07beb408ca730f` (`e89fff89`) |
| `CodeGraph 1.6.2` | [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) (npm `@colbymchenry/codegraph`, `v1.6.2`) | `6560052a6f856855d3f71eee838fd66ccfa4285d` (`6560052a`) |
| `Serena 2.1.0` (제외) | [oraios/serena](https://github.com/oraios/serena) (PyPI `serena-agent`, `v2.1.0`, 실행 전 제외) | `5e4f5cd173c7ff3eed37ee3f1de6c266b937c5dc` (`5e4f5cd1`) |
| 풀이·블라인드 채점 모델 | `gpt-6.1-sol`(`medium`) 및 `gpt-6-astra`(`high`, 도구·저장소 접근 없음) | 환경·도구·문항마다 독립 3회 실행 |
| `pi` 클라이언트 도구 묶음 | `pi 1.4.0` (`@earendil-works/pi-coding-agent@1.4.0`, `--provider openai-codex --thinking medium`) + `@buyong/pi-mcp-adapter@0.1.2` | `pi direct` 및 `pi codemode` (QuickJS 샌드박스) |
| `Codex` 클라이언트 도구 묶음 | `Codex CLI 0.164.0` (`@openai/codex@0.164.0`, `codex exec --json`, `code_mode_only`) | `Codex code mode` |

## 사전 구축 색인 스냅샷과 실행 배치

- **사전 구축 색인 스냅샷**: 각 색인 도구는 읽기 전용 풀이 실행 전 고정된 Grafana 체크아웃에서 배치당 1회씩 색인을 생성했습니다. 정확한 파일·노드·엣지 수, 디스크 크기와 구축 시간은 [측정 방법](/ko/benchmarks/methodology/)에 있습니다.
- **같은 날 두 배치 실행**: `codemap-search 0.14.2`·`rg 15.2.0`과 비교 MCP 4종은 같은 호스트에서 같은 날 두 배치로 실행했습니다. 배치가 나뉘어 있으므로 실제 경과 시간(wall-clock) 기준 답변 소요 시간은 비교·게재하지 않습니다.
- **`Codex code mode` 재실행 내역**: 두 번째 배치의 `Codex` `scope-annotation-filter` 2회차 4개 슬롯(단일 `도구 × 실행 환경 × 문항 × 회차` 실행 단위)은 호스트 시계 보정으로 하네스 시각 검사에 걸려 검사를 고친 뒤 재실행했으며, [측정 결과](/ko/benchmarks/results/)에 보고한 최종 재실행 점수는 `CodeGraph 1.6.2` `10/12`, `Graphify 0.9.95` `11/12`, `zvec-grep 0.2.2` `10/12`, `codebase-memory-mcp 0.12.1` `8/12`입니다.
- **`pi codemode` 서버 중단**: `pi codemode`의 `zvec-grep 0.2.2`는 `scope-annotation-filter` 2회 실행이 서버 `SIGSEGV`로 종료되어 유효 답변 `7`개(채점 사실 `84`개)로 집계했습니다.

## 하드웨어·캐시·클라이언트 버전 민감도

관측된 토큰 수, 프롬프트 캐시 적중률(`~86–94%`), 색인 구축 시간과 출력 잘림 횟수는 측정 당시 호스트 하드웨어, OS·프롬프트 캐시 상태, 고정 사전 색인 스냅샷, 고정 클라이언트·어댑터 버전(`pi 1.4.0`, `@buyong/pi-mcp-adapter 0.1.2`, `Codex CLI 0.164.0`) 기준입니다. 이 중 어느 조건이라도 바뀌면 토큰 수와 잘림 횟수, 실행 결과가 달라질 수 있습니다.

## 커밋된 자료와 비공개 기준 데이터

- **저장소에 커밋된 자료**:
  - `benchmark/results/gpt-6.1-sol-pi.json`, `benchmark/results/gpt-6.1-sol-codex.json`: 도구별·문항별·회차별 점수, 토큰, 도구 호출 수, 순열 검정 결과의 집계 JSON입니다.
  - `benchmark/pi/`, `benchmark/codex/`: 평가 하네스, 샌드박스·격리 검사, 실행 계약(`grafana-execution-v1.json`)과 고정 비교 잠금·도구 체인 파일(`grafana-comparison-targets-v1/targets.lock.json`, `pi-package.json`, `cli-package.json`)입니다.
- **공개 문서에서 제외한 비공개 기준 데이터**:
  - 향후 모델 평가의 오염을 막기 위해 3문항 벤치마크 데이터셋인 `grafana-routes-v6`의 개별 루브릭 설명, 정답 Grafana 파일 경로, 심볼 이름, 채점 체크리스트 원문은 공개 문서에 싣지 않으며 [측정 결과](/ko/benchmarks/results/)에서는 불투명 사실 ID(`A1–AE5`, `G1–GE5`, `B1–BE5`)만 공개합니다.

## 관련 문서

- [측정 방법](/ko/benchmarks/methodology/) — 도구 역할, 샌드박스 규칙과 색인 규모를 확인합니다.
- [한계와 비주장 범위](/ko/benchmarks/limits/) — 표본 크기, 비주장 범위와 동시 실행 오류 민감도 검증을 확인합니다.
