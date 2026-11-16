---
title: Benchmarks
description: Summary charts and benchmark results comparing codemap-search 0.14.2 against rg 15.2.0 and four open-source MCP servers on Grafana.
---

## Test Setup

On a read-only [grafana/grafana](https://github.com/grafana/grafana) TypeScript and Go checkout, `gpt-6.1-sol` (`medium`) answered `3` multi-hop architecture questions `3` times per tool in each of `3` client environments, and a blind grader checked each answer against `12` required facts (`7` `Core` + `5` `Extended`).

### Compared tools and pinned versions

| Tool & version | Upstream project / package | Pinned commit | Tools used |
| --- | --- | --- | --- |
| `codemap-search 0.14.2` | [buYoung/mcp](https://github.com/buYoung/mcp) (`codemap-search`) | `e727d20c6` | `codemap-search` tools only ([Jev filter](/configuration/#jev-filter) off) |
| `Graphify 0.9.95` | [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify) (PyPI `graphifyy`) | `92741d16` | `Graphify` + extra tools (`read`, `grep`, `find`, `ls`, `shell`) |
| `zvec-grep 0.2.2` | [zvec-ai/zvec-grep](https://github.com/zvec-ai/zvec-grep) (npm `@zvec/zvec-grep`) | `b1a9148e` | `zvec-grep` + extra tools (`read`, `grep`, `find`, `ls`, `shell`) |
| `codebase-memory-mcp 0.12.1` | [DeusData/codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) | `a7158ac2` | `codebase-memory-mcp` + extra tools (`read`, `shell`) |
| `rg 15.2.0` (baseline) | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) | `e89fff89` | Built-in tools only (`rg`, `read`, `grep`, `find`, `ls`, `shell`; no MCP) |
| `CodeGraph 1.6.2` | [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) (npm `@colbymchenry/codegraph`) | `6560052a` | `CodeGraph` + extra tools (`grep`, `read`, `find`, `ls`, `shell`) |
| `Serena 2.1.0` (excluded) | [oraios/serena](https://github.com/oraios/serena) (PyPI `serena-agent`) | `5e4f5cd1` | Go/TypeScript LSP MCP (excluded before run) |

`Serena 2.1.0` was excluded before the benchmark runs because indexing Grafana's Go and TypeScript LSP took roughly `34–70` minutes per run.

### Agent clients, models, and execution environments

| Component / Environment | Pinned version & details |
| --- | --- |
| Target codebase | [grafana/grafana](https://github.com/grafana/grafana) commit `c0a598f5a49abf2014f67e806075d8ad267c9efb`, `2026-10-26`, `24,115` tracked TypeScript and Go files |
| Solver model | `gpt-6.1-sol` (`medium`), `3` independent runs per question per tool/environment |
| Blind grader model | `gpt-6-astra` (`high`), no tools or repository access; checks `12` required facts per question |
| `pi direct` | `pi 1.4.0` + `@buyong/pi-mcp-adapter 0.1.2`; turn-by-turn MCP calls plus repo-confined `read`, `grep`, `find`, `ls` (no `bash`) |
| `pi codemode` | `pi 1.4.0` + `@buyong/pi-mcp-adapter 0.1.2` + QuickJS `codemode`; runs JS to batch and filter calls (`10k`-token per-call output cap) |
| `Codex code mode` | `Codex CLI 0.164.0`; runs JS `exec`/`wait` cells calling MCP tools and a repo-confined read-only shell (`10k`-token per-call output cap; `codemap-search 0.14.2` uses `100k`) |

### Grafana index footprint and build time

| Tool | Indexed files / graph entities | Index disk size | Index build time (`pi` envs / `Codex code mode` prep) |
| --- | --- | --- | --- |
| `codemap-search 0.14.2` | `19,757` files | `136.5–138.3 MiB` | `12.96 s` / `16.18 s` |
| `codebase-memory-mcp 0.12.1` | `21,430` tracked files (`525,952` nodes; `1,562,241` `pi` / `1,562,181` `Codex` edges) | `1,915.9–1,916.9 MiB` | `34.35 s` / `56.95 s` |
| `CodeGraph 1.6.2` | `17,094` files (`314,436` nodes, `1,035,428` edges) | `1,194.3 MiB` (`+ 1.9 MiB` state) | `88.82 s` / `101.00 s` |
| `zvec-grep 0.2.2` | `22,642` files (`175,403` entities) | `961.9 MiB` (`+ 32.0 MiB` model cache) | `147.68 s` / `154.22 s` |
| `Graphify 0.9.95` | `19,823` AST-scanned code files (`160,008` nodes, `867,900` edges) | `626.8 MiB` | `311.73 s` / `366.08 s` |
| `rg 15.2.0` (baseline) | None (live disk search) | `0 MiB` | `0 s` |
| `Serena 2.1.0` (excluded) | Go + TypeScript LSP | — | `~34–70 min` (excluded before run) |

## Summary

<div class="bench-charts not-content">
  <div class="bench-chart-card">
    <div class="bench-chart-header">
      <p class="bench-chart-title">Fulfilled rate (%)</p>
      <div class="bench-chart-legend"><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e1"></span>pi direct</span><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e2"></span>pi codemode</span><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e3"></span>Codex code mode</span></div>
    </div>
<svg class="bench-chart-svg" viewBox="0 0 462 314" role="img" aria-label="Fulfilled rate (%)">
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
  <text class="tool-label" x="124" y="217" text-anchor="end">rg (baseline)</text>
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
      <p class="bench-chart-title">Mean total tokens / answer (k)</p>
      <div class="bench-chart-legend"><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e1"></span>pi direct</span><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e2"></span>pi codemode</span><span class="bench-legend-item"><span class="bench-swatch bench-swatch-e3"></span>Codex code mode</span></div>
    </div>
<svg class="bench-chart-svg" viewBox="0 0 462 314" role="img" aria-label="Mean total tokens / answer (k)">
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
  <text class="tool-label" x="124" y="217" text-anchor="end">rg (baseline)</text>
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

| Tool | `pi direct` | `pi codemode` | `Codex code mode` | All-core (7/7) answers (3 envs) | Mean total tokens / answer | Mean uncached total / answer | Mean tool calls / answer | Built-in / fallback calls / answer | Cited test/gen |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `codemap-search 0.14.2` | 96/108 | 97/108 | 98/108 | 5/9 · 6/9 · 6/9 | 946k · 829k · 1,333k | 98k · 101k · 133k | 45.1 · 39.2 · 50.4 | 0.0 · 0.0 · 0.0 | 0 · 0 · 0 |
| `Graphify 0.9.95` | 93/108 | 88/108 | 94/108 | 3/9 · 2/9 · 5/9 | 1,008k · 746k · 1,183k | 93k · 93k · 108k | 55.8 · 66.0 · 70.6 | 16.8 · 22.9 · 11.2 | 8 · 9 · 8 |
| `zvec-grep 0.2.2` | 89/108 | 71/84 (7 ans) | 87/108 | 3/9 · 3/7 · 4/9 | 784k · 608k · 869k | 77k · 79k · 96k | 45.6 · 55.6 · 20.9 | 38.6 · 48.4 · 15.7 | 4 · 2 · 4 |
| `codebase-memory-mcp 0.12.1` | 87/108 | 84/108 | 85/108 | 3/9 · 3/9 · 2/9 | 1,331k · 1,190k · 1,827k | 89k · 113k · 144k | 73.9 · 94.1 · 103.1 | 5.0 · 6.3 · 2.1 | 7 · 9 · 6 |
| `rg 15.2.0` | 86/108 | 87/108 | 89/108 | 3/9 · 1/9 · 5/9 | 555k · 505k · 866k | 68k · 76k · 104k | 63.7 · 66.6 · 20.3 | 63.7 · 66.6 · 20.3 | 6 · 6 · 5 |
| `CodeGraph 1.6.2` | 78/108 | 85/108 | 86/108 | 2/9 · 3/9 · 3/9 | 1,727k · 1,724k · 3,309k | 132k · 142k · 194k | 24.7 · 26.3 · 36.3 | 2.8 · 4.6 · 3.4 | 5 · 5 · 4 |

- **Triple order (`·`)**: Each triple lists `pi direct` · `pi codemode` · `Codex code mode`.
- **Statistical significance (`p < 0.05`, question-stratified permutation test)**:
  - `codemap-search 0.14.2` scored significantly higher than `rg 15.2.0`, `CodeGraph 1.6.2`, and `codebase-memory-mcp 0.12.1` in all `3` environments.
  - It also scored significantly higher than `Graphify 0.9.95` in `pi codemode` and `zvec-grep 0.2.2` in `Codex code mode`.
  - None of the other `4` open-source MCP servers significantly outperformed `rg 15.2.0` in any environment (`p >= 0.05`).
- **Run notes**:
  - `codemap-search 0.14.2` completed every run using its own MCP tools only (`0.0` built-in/fallback calls, `0` answers citing test or generated files).
  - `*` In `pi codemode`, `zvec-grep 0.2.2` crashed twice with `SIGSEGV` on one question, completing `7` of `9` answers (`71/84`, `84.5%`).

See [Detailed Results](/benchmarks/details/) for per-environment `k`-unit token distributions, pairwise `p`-values, per-question/per-fact breakdowns, and subtool call tables.
