# codemap-search 축 A 스케일링/성능 — 실측 결과 (2026-06-09)

> 4티어(~10k→~128k 지원 LOC) 대상 인덱싱·메모리·인덱스크기·도구지연 실측. ground-truth 불필요.
> 측정 하베스: `harness/bench_axisA.py` · 원자료: `artifacts/axisA-results.json` (본 문서 기준 상대 경로).

## 측정 환경
- 바이너리: `codemap-search 0.1.0` (`--release`, 16MB), Apple Silicon(darwin 24.6.0).
- 반복: 인덱싱 N=5(median), 도구지연 warmup 3 + 측정 25회(p50/p90/p95). 인덱싱·RSS는 `/usr/bin/time -l`.
- 도구지연은 **상주 MCP 서버**(인덱스 1회 open 후 warm) — 실사용과 동일. CLI 재호출 오버헤드 미포함.
- startup(프로세스 spawn + `--version`) p50 = **2.5ms** (MCP 서버는 이 비용을 1회만 지불).

## 1. 인덱싱 (cold, N=5 median)

| 코퍼스 | 언어 | 지원 LOC | 색인 파일 | 심볼 | wall median | peak RSS | 인덱스 크기 | LOC/s |
|---|---|--:|--:|--:|--:|--:|--:|--:|
| fd | Rust | 6,813 | 23 | 559 | **0.19s** | 47.9 MB | 0.16 MB | 35.9k |
| ripgrep | Rust | 39,070 | 100 | 4,094 | **0.22s** | 63.1 MB | 0.72 MB | 177.6k |
| scrapy | Python | 63,513 | 439 | 15,398 | **0.35s** | 62.4 MB | 1.24 MB | 181.5k |
| vue-core | TypeScript | 128,285 | 517 | 19,536 | **0.63s** | 69.2 MB | 1.43 MB | 203.6k |

> 색인 파일수 < tokei 파일수(100/102, 517/519)는 일부 파일이 `max_file_size`/추출불가로 스킵된 것.
> **심볼 컬럼은 raw/indexed(추출 총량) 기준.** 2026-06-09 overview 고도 보정 이후 root overview의 headline `Total Symbols`는 significant(필터 후) 합을 보고하므로(인덱스는 여전히 전량 보유) 이 표의 수치와 의미가 다르다 — `bench_axisA.py:52`로 재집계 시 컬럼을 "significant"로 라벨링할 것. 상세: `codemap-search-overview-altitude-results.md`.

### 스케일링 해석
- **wall: ~0.18s 고정 floor + LOC 선형.** LOC 19×(6.8k→128k)인데 wall은 3.3×(0.19→0.63s). 작은 코퍼스는 고정비가 지배 → fd throughput(35.9k)이 낮게 보이고, 커질수록 **한계 throughput이 ~180–204k LOC/s로 평탄화**. 사실상 선형이며 빠름.
- **peak RSS: 거의 평탄(47.9→69.2 MB, 1.44×).** LOC 19× 증가에도 메모리는 강한 sub-linear — 스케일링 병목 아님.
- **인덱스 크기: 심볼수에 비례.** 심볼 35×(559→19,536) 대비 크기 9×(0.16→1.43 MB). 매우 콤팩트.

## 2. Warm 도구지연 (상주 MCP, p50 / p95 ms)

| 도구 | fd | ripgrep | scrapy | vue-core | 스케일 요인 |
|---|--:|--:|--:|--:|---|
| read | 0.08 / 0.10 | 0.08 / 0.09 | (에러*) | 0.09 / 0.11 | 파일 크기(코퍼스 무관) |
| find | 0.44 / 0.49 | 1.23 / 1.37 | 2.00 / 2.37 | 2.33 / 2.41 | 파일 수(트리 워크) |
| overview | 0.55 / 0.68 | 1.78 / 1.96 | 4.52 / 4.94 | 6.32 / 6.87 | 심볼 수 |
| grep | 0.70 / 0.77 | 2.56 / 2.70 | 5.52 / 5.71 | 6.80 / 7.79 | 코퍼스 스캔(rg) |
| search (BM25) | 1.46 / 1.56 | 6.94 / 7.36 | 12.95 / 13.66 | 13.56 / 14.08 | 인덱스 크기 |

`*` scrapy는 `setup.py` 부재(`pyproject.toml`만) → 해당 read는 에러경로(무효). 유효 read는 3개 코퍼스 0.05–0.11ms.

### 해석
- **read는 코퍼스 크기와 무관**하게 <0.12ms — 파일 크기에만 의존(정상).
- **search가 가장 느리나 ~128k에서도 p95 14ms.** BM25는 인덱스/심볼수에 따라 증가(1.5→14ms)하지만 절대값이 작다.
- find/grep/overview는 코퍼스 스캔·심볼수에 선형. 전 도구 p95 ≤ 14ms로 **인덱스가 살아 있으면 모든 탐색이 한 자릿~십 ms대**.

## 3. 컨텍스트 비용(응답 크기) — root overview 폭증 ⚠️
응답 텍스트 길이(문자):

| 도구 | fd | ripgrep | scrapy | vue-core |
|---|--:|--:|--:|--:|
| overview(root) | 20.7 KB | 138 KB | 578 KB | **761 KB** |
| search | 654 B | 3.7 KB | 24.3 KB | 17.1 KB |

- **128k 코퍼스의 root overview = 761KB 텍스트** → 에이전트 컨텍스트에 통째로 들어가면 토큰비용이 매우 큼. 지연(6.3ms)은 싸지만 **컨텍스트 비용은 비쌈**.
- 실사용 함의: root overview를 통째로 호출하지 말고 **폴더 단위로 좁히기**가 핵심. search는 threshold 초과 시 codemap 폴백이라 응답이 커질 수 있음(scrapy 24KB).
- → 축 B(품질) 채점에 **반환 토큰/문자수**를 컨텍스트 효율 프록시로 반드시 포함해야 함.

## 4. 요약
- **성능은 스케일링 병목이 아님:** 128k LOC도 색인 0.63s·RSS 69MB·인덱스 1.43MB, 전 도구 p95 ≤14ms.
- **진짜 비용은 지연이 아니라 컨텍스트(응답 크기)** — overview at scale. 이게 축 B에서 빌트인 도구 대비 codemap-search의 손익을 가를 변수.
- 다음: ripgrep 티어부터 축 B(품질 F2) 측정 시 도구지연(여기 수치)·반환토큰을 함께 기록.
