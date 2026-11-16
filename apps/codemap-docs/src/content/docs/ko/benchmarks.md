---
title: 벤치마크
description: pi direct, pi codemode, Codex code mode 세 환경에서 codemap-search 0.14.2, rg 15.2.0, 오픈소스 MCP 서버 4종을 Grafana에서 비교한 요약 결과입니다.
---

## 측정 조건

읽기 전용 [grafana/grafana](https://github.com/grafana/grafana) TypeScript·Go 저장소에서 `gpt-6.1-sol`(`medium`)으로 다단계 아키텍처 질문 `3`개를 세 클라이언트 환경마다 도구별로 `3`회씩 풀고, 블라인드 채점기가 답변당 필수 사실 `12`개(`Core` `7` + `Extended` `5`) 충족 여부를 채점했습니다.

### 비교 도구와 고정 버전

| 도구·버전 | 공식 프로젝트·패키지 | 고정 커밋 | 사용 도구 |
| --- | --- | --- | --- |
| `codemap-search 0.14.2` | [buYoung/mcp](https://github.com/buYoung/mcp) (`codemap-search`) | `e727d20c6` | `codemap-search` 도구만 사용 ([Jev 필터](/ko/configuration/#jev-필터) 비활성) |
| `Graphify 0.9.95` | [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify) (PyPI `graphifyy`) | `92741d16` | `Graphify` + 추가 도구(`read`, `grep`, `find`, `ls`, `shell`) 사용 |
| `zvec-grep 0.2.2` | [zvec-ai/zvec-grep](https://github.com/zvec-ai/zvec-grep) (npm `@zvec/zvec-grep`) | `b1a9148e` | `zvec-grep` + 추가 도구(`read`, `grep`, `find`, `ls`, `shell`) 사용 |
| `codebase-memory-mcp 0.12.1` | [DeusData/codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) | `a7158ac2` | `codebase-memory-mcp` + 추가 도구(`read`, `shell`) 사용 |
| `rg 15.2.0` (기준군) | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) | `e89fff89` | 기본 도구(`rg`, `read`, `grep`, `find`, `ls`, `shell`)만 사용 (MCP 없음) |
| `CodeGraph 1.6.2` | [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) (npm `@colbymchenry/codegraph`) | `6560052a` | `CodeGraph` + 추가 도구(`grep`, `read`, `find`, `ls`, `shell`) 사용 |
| `Serena 2.1.0` (제외) | [oraios/serena](https://github.com/oraios/serena) (PyPI `serena-agent`) | `5e4f5cd1` | Go/TypeScript LSP MCP (사전 제외) |

`Serena 2.1.0`은 Grafana의 Go·TypeScript LSP 초기 색인에 1회당 약 `34–70`분이 걸려 본 측정에서 제외했습니다.

### 에이전트 클라이언트, 모델, 실행 환경

| 구성 요소 / 실행 환경 | 고정 버전 및 조건 |
| --- | --- |
| 평가 코드베이스 | [grafana/grafana](https://github.com/grafana/grafana) 커밋 `c0a598f5a49abf2014f67e806075d8ad267c9efb`, `2026-10-26`, TypeScript/Go 추적 파일 `24,115`개 |
| 풀이 모델 | `gpt-6.1-sol` (`medium`), 환경·도구·문항당 독립 `3`회 실행 |
| 블라인드 채점 모델 | `gpt-6-astra` (`high`), 도구·저장소 접근 없이 문항당 필수 사실 `12`개 채점 |
| `pi direct` | `pi 1.4.0` + `@buyong/pi-mcp-adapter 0.1.2`; 턴마다 MCP 도구를 직접 호출하며 저장소 내 `read`, `grep`, `find`, `ls`만 병행(`bash` 없음) |
| `pi codemode` | `pi 1.4.0` + `@buyong/pi-mcp-adapter 0.1.2` + QuickJS `codemode`; JS로 호출을 묶고 출력을 추려 반환(호출당 출력 상한 `10k`토큰) |
| `Codex code mode` | `Codex CLI 0.164.0`; JS `exec`·`wait` 셀에서 MCP 도구와 저장소 내 읽기 전용 셸 실행(호출당 출력 상한 기본 `10k`토큰, `codemap-search 0.14.2`는 `100k`토큰) |

### Grafana 색인 규모와 구축 시간

| 도구 | 색인 파일·그래프 엔티티 수 | 색인 디스크 크기 | 색인 구축 시간 (`pi` 두 환경 / `Codex code mode` 준비) |
| --- | --- | --- | --- |
| `codemap-search 0.14.2` | `19,757`개 파일 | `136.5–138.3 MiB` | `12.96 s` / `16.18 s` |
| `codebase-memory-mcp 0.12.1` | `21,430`개 추적 파일 (`525,952`개 노드; `pi` `1,562,241` / `Codex` `1,562,181`개 엣지) | `1,915.9–1,916.9 MiB` | `34.35 s` / `56.95 s` |
| `CodeGraph 1.6.2` | `17,094`개 파일 (`314,436`개 노드, `1,035,428`개 엣지) | `1,194.3 MiB` (`+ 1.9 MiB` 상태) | `88.82 s` / `101.00 s` |
| `zvec-grep 0.2.2` | `22,642`개 파일 (`175,403`개 엔티티) | `961.9 MiB` (`+ 32.0 MiB` 모델 캐시) | `147.68 s` / `154.22 s` |
| `Graphify 0.9.95` | AST 스캔 코드 `19,823`개 파일 (`160,008`개 노드, `867,900`개 엣지) | `626.8 MiB` | `311.73 s` / `366.08 s` |
| `rg 15.2.0` (기준군) | 없음 (실시간 디스크 검색) | `0 MiB` | `0 s` |
| `Serena 2.1.0` (제외) | Go + TypeScript LSP | — | `~34–70 min` (사전 제외) |

## 요약 결과

<div class="bench-charts not-content">
  <div class="bench-chart-card">
    <div class="bench-chart-header">
      <p class="bench-chart-title">정답 충족률 (%)</p>
      <div class="bench-chart-legend"><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e1"></span>pi direct</span><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e2"></span>pi codemode</span><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e3"></span>Codex code mode</span></div>
    </div>
<svg class="bench-chart-svg" viewBox="0 0 462 314" role="img" aria-label="정답 충족률 (%)">
  <line class="axis-line" x1="132.0" y1="6" x2="132.0" y2="286" />
  <text class="axis-label" x="132.0" y="301" text-anchor="middle">0%</text>
  <line class="grid-line" x1="268.0" y1="6" x2="268.0" y2="286" />
  <text class="axis-label" x="268.0" y="301" text-anchor="middle">50%</text>
  <line class="grid-line" x1="336.0" y1="6" x2="336.0" y2="286" />
  <text class="axis-label" x="336.0" y="301" text-anchor="middle">75%</text>
  <line class="grid-line" x1="404.0" y1="6" x2="404.0" y2="286" />
  <text class="axis-label" x="404.0" y="301" text-anchor="middle">100%</text>
  <text class="tool-label tool-label-primary" x="124" y="33" text-anchor="end">codemap-search</text>
  <rect class="bar-e1" x="132" y="14" width="241.8" height="9" rx="1.5" />
  <text class="val-label" x="378.8" y="22">88.9%</text>
  <rect class="bar-e2" x="132" y="25" width="244.3" height="9" rx="1.5" />
  <text class="val-label" x="381.3" y="33">89.8%</text>
  <rect class="bar-e3" x="132" y="36" width="246.7" height="9" rx="1.5" />
  <text class="val-label" x="383.7" y="44">90.7%</text>
  <text class="tool-label" x="124" y="79" text-anchor="end">Graphify</text>
  <rect class="bar-e1" x="132" y="60" width="234.2" height="9" rx="1.5" />
  <text class="val-label" x="371.2" y="68">86.1%</text>
  <rect class="bar-e2" x="132" y="71" width="221.7" height="9" rx="1.5" />
  <text class="val-label" x="358.7" y="79">81.5%</text>
  <rect class="bar-e3" x="132" y="82" width="236.6" height="9" rx="1.5" />
  <text class="val-label" x="373.6" y="90">87.0%</text>
  <text class="tool-label" x="124" y="125" text-anchor="end">zvec-grep</text>
  <rect class="bar-e1" x="132" y="106" width="224.1" height="9" rx="1.5" />
  <text class="val-label" x="361.1" y="114">82.4%</text>
  <rect class="bar-e2" x="132" y="117" width="229.8" height="9" rx="1.5" />
  <text class="val-label" x="366.8" y="125">84.5%*</text>
  <rect class="bar-e3" x="132" y="128" width="219.2" height="9" rx="1.5" />
  <text class="val-label" x="356.2" y="136">80.6%</text>
  <text class="tool-label" x="124" y="171" text-anchor="end">codebase-memory</text>
  <rect class="bar-e1" x="132" y="152" width="219.2" height="9" rx="1.5" />
  <text class="val-label" x="356.2" y="160">80.6%</text>
  <rect class="bar-e2" x="132" y="163" width="211.6" height="9" rx="1.5" />
  <text class="val-label" x="348.6" y="171">77.8%</text>
  <rect class="bar-e3" x="132" y="174" width="214.1" height="9" rx="1.5" />
  <text class="val-label" x="351.1" y="182">78.7%</text>
  <text class="tool-label" x="124" y="217" text-anchor="end">rg (기준군)</text>
  <rect class="bar-e1" x="132" y="198" width="216.5" height="9" rx="1.5" />
  <text class="val-label" x="353.5" y="206">79.6%</text>
  <rect class="bar-e2" x="132" y="209" width="219.2" height="9" rx="1.5" />
  <text class="val-label" x="356.2" y="217">80.6%</text>
  <rect class="bar-e3" x="132" y="220" width="224.1" height="9" rx="1.5" />
  <text class="val-label" x="361.1" y="228">82.4%</text>
  <text class="tool-label" x="124" y="263" text-anchor="end">CodeGraph</text>
  <rect class="bar-e1" x="132" y="244" width="196.4" height="9" rx="1.5" />
  <text class="val-label" x="333.4" y="252">72.2%</text>
  <rect class="bar-e2" x="132" y="255" width="214.1" height="9" rx="1.5" />
  <text class="val-label" x="351.1" y="263">78.7%</text>
  <rect class="bar-e3" x="132" y="266" width="216.5" height="9" rx="1.5" />
  <text class="val-label" x="353.5" y="274">79.6%</text>
</svg>
  </div>
  <div class="bench-chart-card">
    <div class="bench-chart-header">
      <p class="bench-chart-title">답변당 평균 총 토큰 (k)</p>
      <div class="bench-chart-legend"><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e1"></span>pi direct</span><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e2"></span>pi codemode</span><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e3"></span>Codex code mode</span></div>
    </div>
<svg class="bench-chart-svg" viewBox="0 0 462 314" role="img" aria-label="답변당 평균 총 토큰 (k)">
  <line class="axis-line" x1="132.0" y1="6" x2="132.0" y2="286" />
  <text class="axis-label" x="132.0" y="301" text-anchor="middle">0</text>
  <line class="grid-line" x1="209.7" y1="6" x2="209.7" y2="286" />
  <text class="axis-label" x="209.7" y="301" text-anchor="middle">1,000k</text>
  <line class="grid-line" x1="287.4" y1="6" x2="287.4" y2="286" />
  <text class="axis-label" x="287.4" y="301" text-anchor="middle">2,000k</text>
  <line class="grid-line" x1="404.0" y1="6" x2="404.0" y2="286" />
  <text class="axis-label" x="404.0" y="301" text-anchor="middle">3,500k</text>
  <text class="tool-label tool-label-primary" x="124" y="33" text-anchor="end">codemap-search</text>
  <rect class="bar-e1" x="132" y="14" width="73.5" height="9" rx="1.5" />
  <text class="val-label" x="210.5" y="22">946k</text>
  <rect class="bar-e2" x="132" y="25" width="64.4" height="9" rx="1.5" />
  <text class="val-label" x="201.4" y="33">829k</text>
  <rect class="bar-e3" x="132" y="36" width="103.6" height="9" rx="1.5" />
  <text class="val-label" x="240.6" y="44">1,333k</text>
  <text class="tool-label" x="124" y="79" text-anchor="end">Graphify</text>
  <rect class="bar-e1" x="132" y="60" width="78.3" height="9" rx="1.5" />
  <text class="val-label" x="215.3" y="68">1,008k</text>
  <rect class="bar-e2" x="132" y="71" width="58.0" height="9" rx="1.5" />
  <text class="val-label" x="195.0" y="79">746k</text>
  <rect class="bar-e3" x="132" y="82" width="91.9" height="9" rx="1.5" />
  <text class="val-label" x="228.9" y="90">1,183k</text>
  <text class="tool-label" x="124" y="125" text-anchor="end">zvec-grep</text>
  <rect class="bar-e1" x="132" y="106" width="60.9" height="9" rx="1.5" />
  <text class="val-label" x="197.9" y="114">784k</text>
  <rect class="bar-e2" x="132" y="117" width="47.3" height="9" rx="1.5" />
  <text class="val-label" x="184.3" y="125">608k</text>
  <rect class="bar-e3" x="132" y="128" width="67.5" height="9" rx="1.5" />
  <text class="val-label" x="204.5" y="136">869k</text>
  <text class="tool-label" x="124" y="171" text-anchor="end">codebase-memory</text>
  <rect class="bar-e1" x="132" y="152" width="103.4" height="9" rx="1.5" />
  <text class="val-label" x="240.4" y="160">1,331k</text>
  <rect class="bar-e2" x="132" y="163" width="92.5" height="9" rx="1.5" />
  <text class="val-label" x="229.5" y="171">1,190k</text>
  <rect class="bar-e3" x="132" y="174" width="142.0" height="9" rx="1.5" />
  <text class="val-label" x="279.0" y="182">1,827k</text>
  <text class="tool-label" x="124" y="217" text-anchor="end">rg (기준군)</text>
  <rect class="bar-e1" x="132" y="198" width="43.1" height="9" rx="1.5" />
  <text class="val-label" x="180.1" y="206">555k</text>
  <rect class="bar-e2" x="132" y="209" width="39.2" height="9" rx="1.5" />
  <text class="val-label" x="176.2" y="217">505k</text>
  <rect class="bar-e3" x="132" y="220" width="67.3" height="9" rx="1.5" />
  <text class="val-label" x="204.3" y="228">866k</text>
  <text class="tool-label" x="124" y="263" text-anchor="end">CodeGraph</text>
  <rect class="bar-e1" x="132" y="244" width="134.2" height="9" rx="1.5" />
  <text class="val-label" x="271.2" y="252">1,727k</text>
  <rect class="bar-e2" x="132" y="255" width="134.0" height="9" rx="1.5" />
  <text class="val-label" x="271.0" y="263">1,724k</text>
  <rect class="bar-e3" x="132" y="266" width="257.2" height="9" rx="1.5" />
  <text class="val-label" x="394.2" y="274">3,309k</text>
</svg>
  </div>
</div>

| 도구 | `pi direct` | `pi codemode` | `Codex code mode` | Core 완결 (7/7) 답변 (3개 환경) | 답변당 평균 총 토큰 | 답변당 평균 비캐시 합계 | 답변당 평균 호출 | 답변당 기본·보조 도구 호출 | 테스트·생성 인용 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `codemap-search 0.14.2` | 96/108 | 97/108 | 98/108 | 5/9 · 6/9 · 6/9 | 946k · 829k · 1,333k | 98k · 101k · 133k | 45.1 · 39.2 · 50.4 | 0.0 · 0.0 · 0.0 | 0 · 0 · 0 |
| `Graphify 0.9.95` | 93/108 | 88/108 | 94/108 | 3/9 · 2/9 · 5/9 | 1,008k · 746k · 1,183k | 93k · 93k · 108k | 55.8 · 66.0 · 70.6 | 16.8 · 22.9 · 11.2 | 8 · 9 · 8 |
| `zvec-grep 0.2.2` | 89/108 | 71/84 (답변 7개) | 87/108 | 3/9 · 3/7 · 4/9 | 784k · 608k · 869k | 77k · 79k · 96k | 45.6 · 55.6 · 20.9 | 38.6 · 48.4 · 15.7 | 4 · 2 · 4 |
| `codebase-memory-mcp 0.12.1` | 87/108 | 84/108 | 85/108 | 3/9 · 3/9 · 2/9 | 1,331k · 1,190k · 1,827k | 89k · 113k · 144k | 73.9 · 94.1 · 103.1 | 5.0 · 6.3 · 2.1 | 7 · 9 · 6 |
| `rg 15.2.0` | 86/108 | 87/108 | 89/108 | 3/9 · 1/9 · 5/9 | 555k · 505k · 866k | 68k · 76k · 104k | 63.7 · 66.6 · 20.3 | 63.7 · 66.6 · 20.3 | 6 · 6 · 5 |
| `CodeGraph 1.6.2` | 78/108 | 85/108 | 86/108 | 2/9 · 3/9 · 3/9 | 1,727k · 1,724k · 3,309k | 132k · 142k · 194k | 24.7 · 26.3 · 36.3 | 2.8 · 4.6 · 3.4 | 5 · 5 · 4 |

- **`·` 구분 순서**: 세 값은 `pi direct` · `pi codemode` · `Codex code mode` 순서입니다.
- **통계적 유의성 (`p < 0.05`, 문항 층화 순열 검정)**:
  - `codemap-search 0.14.2`는 `3`개 환경 모두에서 `rg 15.2.0`, `CodeGraph 1.6.2`, `codebase-memory-mcp 0.12.1`보다 충족 건수가 유의하게 높았습니다.
  - `Graphify 0.9.95` 대비로는 `pi codemode`에서, `zvec-grep 0.2.2` 대비로는 `Codex code mode`에서 유의하게 높았습니다.
  - 나머지 오픈소스 MCP 서버 `4`종은 어느 환경에서도 `rg 15.2.0`을 유의하게 앞서지 못했습니다(`p >= 0.05`).
- **실행 특이사항**:
  - `codemap-search 0.14.2`만 모든 환경에서 기본·보조 도구 호출(`0.0`)과 테스트·생성 파일 인용 답변(`0`) 없이 자체 MCP 도구만으로 응답했습니다.
  - `*` `pi codemode`의 `zvec-grep 0.2.2`는 한 문항에서 서버 `SIGSEGV` 중단이 두 번 발생해 `9`회 중 `7`회(`71/84`, `84.5%`)만 완료했습니다.

환경별 토큰 분포(중앙값·최소–최대), 쌍별 `p` 값, 문항·세부 항목별 채점표, 하위 도구 호출 상세는 [상세 결과](/ko/benchmarks/details/)에서 확인할 수 있습니다.
