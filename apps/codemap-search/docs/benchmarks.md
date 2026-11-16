# Code navigation benchmark results

[한국어](./benchmarks.ko.md) | English

To measure how code-navigation tools affect coding-agent answer accuracy and token cost on a large real-world codebase, we tested `codemap-search 0.14.2` against a built-in-tools baseline (`rg 15.2.0`) and four open-source MCP code-navigation servers on **Grafana** (`24,115` tracked files, TypeScript + Go). Using `gpt-6.1-sol` (`medium`) as the solver and `gpt-6-astra` (`high`) as a blind grader with no tools or repository access, each tool answered 3 multi-hop architecture questions 3 times (`9` answers, `108` line-cited facts) in each of 3 agent environments.

## Summary

The three agent environments (`pi direct`, `pi codemode`, `Codex code mode`) are reported side-by-side and never pooled because each client differs in built-in fallback tools, output truncation caps, and prompt/token accounting (for example, Codex includes a larger per-turn developer and code-mode preamble, driving higher total cached input tokens; `zvec-grep 0.2.2` in `pi codemode` has `7` answers / `84` facts due to two server crashes).

| Tool | `pi direct` | `pi codemode` | `Codex code mode` | All-core (7/7) answers (3 envs) | Mean total tokens / answer | Mean uncached tokens / answer | Mean tool calls / answer | Built-in / fallback calls / answer | Answers citing test/generated files |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `codemap-search 0.14.2` | 96/108 | 97/108 | 98/108 | 5/9 · 6/9 · 6/9 | 946k · 829k · 1.33M | 98k · 101k · 133k | 45.1 · 39.2 · 50.4 | 0.0 · 0.0 · 0.0 | 0 · 0 · 0 |
| `Graphify 0.9.95` | 93/108 | 88/108 | 94/108 | 3/9 · 2/9 · 5/9 | 1.01M · 746k · 1.18M | 93k · 93k · 108k | 55.8 · 66.0 · 70.6 | 16.8 · 22.9 · 11.2 | 8 · 9 · 8 |
| `zvec-grep 0.2.2` | 89/108 | 71/84 (7 ans) | 87/108 | 3/9 · 3/7 · 4/9 | 784k · 608k · 869k | 77k · 79k · 96k | 45.6 · 55.6 · 20.9 | 38.6 · 48.4 · 15.7 | 4 · 2 · 4 |
| `codebase-memory-mcp 0.12.1` | 87/108 | 84/108 | 85/108 | 3/9 · 3/9 · 2/9 | 1.33M · 1.19M · 1.83M | 89k · 113k · 144k | 73.9 · 94.1 · 103.1 | 5.0 · 6.3 · 2.1 | 7 · 9 · 6 |
| `rg 15.2.0` | 86/108 | 87/108 | 89/108 | 3/9 · 1/9 · 5/9 | 555k · 505k · 866k | 68k · 76k · 104k | 63.7 · 66.6 · 20.3 | 63.7 · 66.6 · 20.3 | 6 · 6 · 5 |
| `CodeGraph 1.6.2` | 78/108 | 85/108 | 86/108 | 2/9 · 3/9 · 3/9 | 1.73M · 1.72M · 3.31M | 132k · 142k · 194k | 24.7 · 26.3 · 36.3 | 2.8 · 4.6 · 3.4 | 5 · 5 · 4 |

- **vs `rg 15.2.0` (built-in-tools baseline)**: `codemap-search 0.14.2` fulfilled `+1.00` to `+1.11` more cited facts per answer across all three environments (`p = 0.0250`, `0.0300`, `0.0280`, unadjusted), at `1.54×`–`1.71×` total tokens (`1.27×`–`1.45×` uncached tokens).
- **vs `CodeGraph 1.6.2` and `codebase-memory-mcp 0.12.1`**: `codemap-search 0.14.2` fulfilled `+1.00` to `+2.00` more facts per answer across all three environments (all 6 comparisons `p < 0.05`, unadjusted) while using `0.40×`–`0.73×` total tokens (`0.68×`–`1.10×` uncached; in `pi direct`, `codemap-search` used `1.10×` uncached tokens vs `codebase-memory-mcp 0.12.1`, `98,081` vs `89,291`).
- **vs `Graphify 0.9.95` and `zvec-grep 0.2.2`**: `codemap-search 0.14.2` scored `+0.33` to `+1.22` higher in all three environments, reaching `p < 0.05` in one environment each (`pi codemode` `p = 0.0180` vs `Graphify 0.9.95`; `Codex` `p = 0.0100` vs `zvec-grep 0.2.2`; treated as inconclusive at `n = 9`). On raw `Factually correct` counts without citation checks, `Graphify 0.9.95` scored `96` vs `codemap-search 0.14.2`'s `98` in `pi direct`, `96` vs `100` in `pi codemode`, and `95` vs `99` in `Codex code mode`.
- **Self-contained execution & comparison MCPs vs `rg 15.2.0`**: `codemap-search 0.14.2` made `0` built-in/shell fallback calls (bundling its own `read`, `grep`, and `find` tools inside the MCP server) and `0` test/generated-file citations across all `27` answers; none of the other four MCP servers outperformed `rg 15.2.0` at `p < 0.05` in any environment despite using `1.00×`–`3.82×` total tokens (`0.92×`–`1.94×` uncached).

## Compared tools, versions, and environments

### Compared tools and pinned versions

| Tool & version | Upstream project / package | Pinned commit | Navigation approach & exposed tools |
| --- | --- | --- | --- |
| `codemap-search 0.14.2` | [buYoung/mcp](https://github.com/buYoung/mcp) (`codemap-search`) | `e727d20c6` | Single Rust binary (6 exposed MCP tools): Tree-sitter symbol/caller extraction + Tantivy BM25 `search` + embedded `read`, `find`, `grep`, `initial_instructions`, and `analyze` (unused by solvers); optional Jev LLM relevance filter off (see [Optional Jev decision stages](./configuration.md#optional-jev-decision-stages)) |
| `Graphify 0.9.95` | [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify) (PyPI `graphifyy`, `v0.9.95`) | `92741d16` | Python code-graph MCP (7 graph tools); paired with a read-only MCP `read` helper because Graphify's graph tools return entity metadata and locations without raw source bodies |
| `zvec-grep 0.2.2` | [zvec-ai/zvec-grep](https://github.com/zvec-ai/zvec-grep) (npm `@zvec/zvec-grep`, `v0.2.2`) | `b1a9148e` | Node.js local vector/semantic search MCP (`zvec_grep_search`, embedding model `local/potion-code-16m-v2`); no `read` helper attached because `zvec_grep_search` returns bounded source snippets alongside matches |
| `codebase-memory-mcp 0.12.1` | [DeusData/codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) (`v0.12.1`) | `a7158ac2` | Codebase graph, code search, and snippet-retrieval MCP (13 tools) |
| `rg 15.2.0` (baseline) | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) (`15.2.0`) | `e89fff89` | No MCP server attached; the agent relies solely on its built-in file/search or read-only shell tools |
| `CodeGraph 1.6.2` | [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) (npm `@colbymchenry/codegraph`, `v1.6.2`) | `6560052a` | Node.js code-graph exploration MCP (`codegraph_explore`) |
| `Serena 2.1.0` (excluded) | [oraios/serena](https://github.com/oraios/serena) (PyPI `serena-agent`, `v2.1.0`) | `5e4f5cd1` | Go + TypeScript LSP-backed MCP; excluded prior to running because full LSP indexing of Grafana took ~34–70 min per run |

### Agent clients, models, and execution environments

| Component / Environment | Pinned version & details |
| --- | --- |
| Target codebase | [grafana/grafana](https://github.com/grafana/grafana) commit `c0a598f5a49abf2014f67e806075d8ad267c9efb` (`2026-10-26`, `24,115` tracked files, TypeScript + Go; includes commits after the solver's training cutoff) |
| Solver model | `gpt-6.1-sol` (reasoning effort `medium`, `3` independent runs per question per tool/environment) |
| Blind grader model | `gpt-6-astra` (reasoning effort `high`, no tools or repository access; checks each answer against `12` required facts per question) |
| `pi direct` | `pi 1.4.0` (`--provider openai-codex --thinking medium`) + `@buyong/pi-mcp-adapter 0.1.2`. Standard tool-calling mode: calls MCP tools directly turn by turn, alongside read-only built-in `read`, `grep`, `find`, `ls` (no `bash`) confined to the repository |
| `pi codemode` | Same `pi 1.4.0` + `@buyong/pi-mcp-adapter 0.1.2` + `pi`'s built-in `codemode` extension (`@earendil-works/pi-coding-agent 1.4.0`, QuickJS sandbox). Scripted mode: the model writes JavaScript that batches/chains MCP or built-in calls and filters outputs before returning to context (default `10,000`-token script output cap) |
| `Codex code mode` | `Codex CLI 0.164.0` (`codex exec --json`, default `code_mode_only`). Executes JavaScript `exec`/`wait` cells that invoke MCP tools and a read-only shell (`exec_command`) confined to the repository, batching multiple tool or shell calls per turn (`10,000`-token default `exec` output cap unless overridden; `codemap-search` instructs `// @exec: {"max_output_tokens": 100000}`) |

**Sandbox & isolation rules**: Every run operated on an isolated read-only checkout of the pinned Grafana snapshot with external network access disabled (`no subagents`, no source edits). Both clients blocked access to `.git` and pre-built tool index/state folders (`.codemap`, `.codegraph`, `.zvec-grep`, `.codebase-memory`, `graphify-out`, `.pi`, `.codex`). In `pi`, a pre-execution `tool_call` guard enforced those path boundaries on built-in `read`, `grep`, `find`, and `ls` (both direct and inside `codemode` scripts) while `grep`/`find` binaries ran inside macOS Seatbelt; in `Codex CLI`, `permissions.local_read` denied out-of-workspace reads and blocked folders while MCP servers ran under local macOS Seatbelt isolation.

### How to read the metrics

1. **Fulfilled (`correct` + `supported`, primary accuracy score)**: Each answer is graded against `12` required facts (`108` across `9` answers). A fact counts as **Fulfilled** only if the explanation is accurate (`correct=true`) **and** proven by the `path:start-end` line ranges cited in the answer (`supported=true`). All tools—including `rg 15.2.0`—received the identical line-range citation instruction.
2. **Core (`63`) vs Extended (`45`) & All-core (7/7)**: Each question has `7` **Core** facts (essential end-to-end hops from entry point to handler/storage) and `5` **Extended** facts (secondary branches, configuration guards, or conditions). **All-core (7/7)** counts answers that fulfilled every Core fact without missing a link.
3. **Factually correct, Correct, weak citation, & Incorrect / missed**: **Correct, weak citation** counts facts where the model described the right behavior, but the cited line ranges failed to establish that claim (`correct=true, supported=false`). **Incorrect / missed** equals `total graded facts − Factually correct` (`108 − Factually correct`, or `84 − Factually correct` at `n = 7`), so `Fulfilled + Correct, weak citation + Incorrect / missed = total graded facts` (e.g., `96 + 2 + 10 = 108`).
4. **Total tokens vs Uncached tokens**: Multi-turn agents re-send prior context on every turn (`~86–94%` served from prompt cache). **Total tokens** is all input (cached + uncached) plus output tokens per answer; **Uncached tokens** is cache-miss input plus output tokens. Codex totals run `~1.4–1.9×` higher than `pi` largely because Codex includes a larger per-turn system/tool/code-mode preamble (e.g., `9,748` first-turn input tokens on `codemap-search` vs `5,118` in `pi direct` and `4,338` in `pi codemode`) that is re-sent and mostly cached every turn.
5. **Built-in / fallback calls**: All MCP arms received the same instruction to prefer MCP tools and use built-in tools or the shell only when the MCP server could not perform the task. `0.0` means every run completed using only the MCP server's own tools (`codemap-search 0.14.2` bundles `read`, `grep`, and `find` inside the MCP server). For `rg 15.2.0` (which has no MCP server), this column equals all built-in file/search or read-only shell calls.
6. **Statistical test (`p`) & Token ratios**: Pairwise `p`-values come from an exact question-stratified permutation test (`p < 0.05` threshold, unadjusted across `24` external-MCP comparisons and `27` total rows). **Two-sided `p`** tests whether the two tools differ in either direction; **One-sided `p`** tests whether the first tool (`A` in `A − B`) fulfilled strictly more facts per answer than the second (`B`), which is why negative differences yield high one-sided `p`-values near `1.0`. At `n = 9` answers per cell, differences of roughly `1.0+` facts/answer are separable, whereas `0.3–0.8` gaps lie within run-to-run noise. Token ratios (`×1.71`, `×0.55`) divide the first tool's mean tokens by the second's.

## Index footprint and build time on Grafana

Measured once per batch when indexing the pinned `24,115`-file Grafana checkout prior to read-only solver runs. Build times report `timings.index_build_seconds` uniformly across all five indexed tools (covering the full index-preparation step in `pi` / `Codex` setup, including pre/post status or ready checks).

| Tool | Indexed files / graph entities | Index disk size | Index build time (`pi` / `Codex` prep) |
| --- | --- | --- | --- |
| `codemap-search 0.14.2` | `19,757` files | `136.5–138.3 MiB` | `12.96 s` / `16.18 s` |
| `codebase-memory-mcp 0.12.1` | `21,430` tracked files (`525,952` nodes; `1,562,241` `pi` / `1,562,181` `Codex` edges) | `1,915.9–1,916.9 MiB` | `34.35 s` / `56.95 s` |
| `CodeGraph 1.6.2` | `17,094` files (`314,436` nodes, `1,035,428` edges) | `1,194.3 MiB` (`+ 1.9 MiB` state) | `88.82 s` / `101.00 s` |
| `zvec-grep 0.2.2` | `22,642` files (`175,403` entities) | `961.9 MiB` (`+ 32.0 MiB` model cache) | `147.68 s` / `154.22 s` |
| `Graphify 0.9.95` | `19,823` AST-scanned code files (`160,008` nodes, `867,900` edges) | `626.8 MiB` | `311.73 s` / `366.08 s` |
| `rg 15.2.0` (baseline) | None (live disk search) | `0 MiB` | `0 s` |
| `Serena 2.1.0` (excluded) | Go + TypeScript LSP | — | `~34–70 min` (excluded before run) |

Main indexing CLI step alone (excluding pre/post scaffold, status, and ready-check commands of `0.2–8.9 s`): `codemap-search 0.14.2` `12.7 s` / `15.9 s`, `codebase-memory-mcp 0.12.1` `31.9 s` / `55.8 s`, `CodeGraph 1.6.2` `86.8 s` / `99.0 s`, `zvec-grep 0.2.2` `138.8 s` / `145.4 s`, `Graphify 0.9.95` `311.7 s` / `366.1 s`.

## Detailed results by environment

Ordered within each environment by fulfilled rate (`9` answers per tool except `zvec-grep 0.2.2` in `pi codemode`).

### `pi direct`

| Tool | Answers | Fulfilled | Fulfilled % | Core | Extended | All-core (7/7) | Factually correct | Correct, weak citation | Incorrect / missed | Cited test/gen |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `codemap-search 0.14.2` | 9 | 96/108 | 88.9% | 59/63 | 37/45 | 5/9 | 98 | 2 | 10 | 0 |
| `Graphify 0.9.95` | 9 | 93/108 | 86.1% | 56/63 | 37/45 | 3/9 | 96 | 3 | 12 | 8 |
| `zvec-grep 0.2.2` | 9 | 89/108 | 82.4% | 55/63 | 34/45 | 3/9 | 90 | 1 | 18 | 4 |
| `codebase-memory-mcp 0.12.1` | 9 | 87/108 | 80.6% | 54/63 | 33/45 | 3/9 | 88 | 1 | 20 | 7 |
| `rg 15.2.0` | 9 | 86/108 | 79.6% | 53/63 | 33/45 | 3/9 | 90 | 4 | 18 | 6 |
| `CodeGraph 1.6.2` | 9 | 78/108 | 72.2% | 50/63 | 28/45 | 2/9 | 80 | 2 | 28 | 5 |

| Tool | Total tokens (mean) | Median | Min–Max | Input tokens (mean) | Cached input (mean, %) | Uncached input (mean) | Output tokens (mean) | Uncached total (mean) | Turns (mean, min–max) |
| --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `codemap-search 0.14.2` | 945,598 | 967,031 | 573,594–1,254,646 | 938,400 | 847,516 (90.3%) | 90,884 | 7,197 | 98,081 | 18.4 (13–25) |
| `Graphify 0.9.95` | 1,007,660 | 779,617 | 540,216–2,505,552 | 1,001,707 | 914,190 (91.3%) | 87,517 | 5,953 | 93,470 | 25.7 (18–45) |
| `zvec-grep 0.2.2` | 784,052 | 669,022 | 342,436–1,349,405 | 777,753 | 707,214 (90.9%) | 70,539 | 6,299 | 76,838 | 23.3 (14–38) |
| `codebase-memory-mcp 0.12.1` | 1,331,261 | 1,134,777 | 823,755–2,465,320 | 1,322,159 | 1,241,970 (93.9%) | 80,189 | 9,102 | 89,291 | 34.6 (23–57) |
| `rg 15.2.0` | 554,578 | 410,872 | 320,828–1,337,174 | 549,030 | 486,827 (88.7%) | 62,204 | 5,547 | 67,751 | 20.9 (14–35) |
| `CodeGraph 1.6.2` | 1,727,142 | 1,609,990 | 741,011–2,654,991 | 1,722,726 | 1,595,378 (92.6%) | 127,348 | 4,416 | 131,764 | 25.4 (16–33) |

| Pairwise comparison | Fulfilled diff / ans | Two-sided `p` | One-sided `p` | Total token ratio | Uncached token ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| `codemap-search 0.14.2` − `Graphify 0.9.95` | +0.33 | 0.5400 | 0.2700 | ×0.94 | ×1.05 |
| `codemap-search 0.14.2` − `zvec-grep 0.2.2` | +0.78 | 0.1600 | 0.0800 | ×1.21 | ×1.28 |
| `codemap-search 0.14.2` − `codebase-memory-mcp 0.12.1` | +1.00 | 0.0250 | 0.0125 | ×0.71 | ×1.10 |
| `codemap-search 0.14.2` − `rg 15.2.0` | +1.11 | 0.0250 | 0.0125 | ×1.71 | ×1.45 |
| `codemap-search 0.14.2` − `CodeGraph 1.6.2` | +2.00 | 0.0020 | 0.0010 | ×0.55 | ×0.74 |
| `Graphify 0.9.95` − `rg 15.2.0` | +0.78 | 0.1475 | 0.0737 | ×1.82 | ×1.38 |
| `zvec-grep 0.2.2` − `rg 15.2.0` | +0.33 | 0.5700 | 0.2850 | ×1.41 | ×1.13 |
| `codebase-memory-mcp 0.12.1` − `rg 15.2.0` | +0.11 | 1.0000 | 0.5000 | ×2.40 | ×1.32 |
| `CodeGraph 1.6.2` − `rg 15.2.0` | -0.89 | 0.0600 | 0.9940 | ×3.11 | ×1.94 |

### `pi codemode`

| Tool | Answers | Fulfilled | Fulfilled % | Core | Extended | All-core (7/7) | Factually correct | Correct, weak citation | Incorrect / missed | Cited test/gen |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `codemap-search 0.14.2` | 9 | 97/108 | 89.8% | 60/63 | 37/45 | 6/9 | 100 | 3 | 8 | 0 |
| `zvec-grep 0.2.2` | 7 | 71/84 | 84.5% | 44/49 | 27/35 | 3/7 | 72 | 1 | 12 | 2 |
| `Graphify 0.9.95` | 9 | 88/108 | 81.5% | 54/63 | 34/45 | 2/9 | 96 | 8 | 12 | 9 |
| `rg 15.2.0` | 9 | 87/108 | 80.6% | 53/63 | 34/45 | 1/9 | 93 | 6 | 15 | 6 |
| `CodeGraph 1.6.2` | 9 | 85/108 | 78.7% | 51/63 | 34/45 | 3/9 | 87 | 2 | 21 | 5 |
| `codebase-memory-mcp 0.12.1` | 9 | 84/108 | 77.8% | 55/63 | 29/45 | 3/9 | 87 | 3 | 21 | 9 |

| Tool | Total tokens (mean) | Median | Min–Max | Input tokens (mean) | Cached input (mean, %) | Uncached input (mean) | Output tokens (mean) | Uncached total (mean) | Turns (mean, min–max) |
| --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `codemap-search 0.14.2` | 828,914 | 761,345 | 599,493–1,361,409 | 822,432 | 727,666 (88.5%) | 94,766 | 6,482 | 101,248 | 16.1 (14–24) |
| `zvec-grep 0.2.2` | 607,776 | 679,388 | 419,837–800,319 | 601,837 | 528,402 (87.8%) | 73,435 | 5,939 | 79,374 | 15.9 (13–19) |
| `Graphify 0.9.95` | 745,621 | 730,311 | 388,492–1,307,038 | 739,715 | 652,658 (88.2%) | 87,057 | 5,906 | 92,963 | 17.1 (13–24) |
| `rg 15.2.0` | 504,871 | 493,749 | 218,026–809,725 | 499,493 | 429,056 (85.9%) | 70,437 | 5,379 | 75,815 | 13.9 (11–18) |
| `CodeGraph 1.6.2` | 1,724,007 | 1,476,736 | 756,656–3,147,915 | 1,719,458 | 1,582,037 (92.0%) | 137,421 | 4,549 | 141,970 | 22.6 (15–32) |
| `codebase-memory-mcp 0.12.1` | 1,189,558 | 1,186,285 | 773,994–1,608,332 | 1,181,898 | 1,076,580 (91.1%) | 105,319 | 7,660 | 112,979 | 22.6 (18–26) |

| Pairwise comparison | Fulfilled diff / ans | Two-sided `p` | One-sided `p` | Total token ratio | Uncached token ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| `codemap-search 0.14.2` − `zvec-grep 0.2.2` | +0.63 | 0.2250 | 0.1125 | ×1.36 | ×1.28 |
| `codemap-search 0.14.2` − `Graphify 0.9.95` | +1.00 | 0.0180 | 0.0090 | ×1.11 | ×1.09 |
| `codemap-search 0.14.2` − `rg 15.2.0` | +1.11 | 0.0300 | 0.0150 | ×1.64 | ×1.34 |
| `codemap-search 0.14.2` − `CodeGraph 1.6.2` | +1.33 | 0.0280 | 0.0140 | ×0.48 | ×0.71 |
| `codemap-search 0.14.2` − `codebase-memory-mcp 0.12.1` | +1.44 | 0.0025 | 0.0013 | ×0.70 | ×0.90 |
| `zvec-grep 0.2.2` − `rg 15.2.0` | +0.48 | 0.5375 | 0.4562 | ×1.20 | ×1.05 |
| `Graphify 0.9.95` − `rg 15.2.0` | +0.11 | 1.0000 | 0.5000 | ×1.48 | ×1.23 |
| `CodeGraph 1.6.2` − `rg 15.2.0` | -0.22 | 0.8080 | 0.7765 | ×3.41 | ×1.87 |
| `codebase-memory-mcp 0.12.1` − `rg 15.2.0` | -0.33 | 0.5625 | 0.8825 | ×2.36 | ×1.49 |

In `pi codemode`, two `zvec-grep 0.2.2` runs on `scope-annotation-filter` terminated when the `zvec-grep` server crashed with `SIGSEGV` (`7` answers / `84` facts total).

### `Codex code mode`

| Tool | Answers | Fulfilled | Fulfilled % | Core | Extended | All-core (7/7) | Factually correct | Correct, weak citation | Incorrect / missed | Cited test/gen |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `codemap-search 0.14.2` | 9 | 98/108 | 90.7% | 59/63 | 39/45 | 6/9 | 99 | 1 | 9 | 0 |
| `Graphify 0.9.95` | 9 | 94/108 | 87.0% | 58/63 | 36/45 | 5/9 | 95 | 1 | 13 | 8 |
| `rg 15.2.0` | 9 | 89/108 | 82.4% | 58/63 | 31/45 | 5/9 | 93 | 4 | 15 | 5 |
| `zvec-grep 0.2.2` | 9 | 87/108 | 80.6% | 55/63 | 32/45 | 4/9 | 91 | 4 | 17 | 4 |
| `CodeGraph 1.6.2` | 9 | 86/108 | 79.6% | 54/63 | 32/45 | 3/9 | 89 | 3 | 19 | 4 |
| `codebase-memory-mcp 0.12.1` | 9 | 85/108 | 78.7% | 53/63 | 32/45 | 2/9 | 88 | 3 | 20 | 6 |

| Tool | Total tokens (mean) | Median | Min–Max | Input tokens (mean) | Cached input (mean, %) | Uncached input (mean) | Output tokens (mean) | Uncached total (mean) | Turns (mean, min–max) |
| --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `codemap-search 0.14.2` | 1,333,026 | 1,294,144 | 876,991–1,856,795 | 1,324,497 | 1,200,412 (90.6%) | 124,085 | 8,529 | 132,614 | 18.9 (15–23) |
| `Graphify 0.9.95` | 1,183,030 | 995,489 | 766,935–1,622,245 | 1,176,013 | 1,074,702 (91.4%) | 101,311 | 7,017 | 108,328 | 19.9 (16–26) |
| `rg 15.2.0` | 865,511 | 671,568 | 579,926–1,657,964 | 858,947 | 761,017 (88.6%) | 97,931 | 6,564 | 104,494 | 16.4 (13–25) |
| `zvec-grep 0.2.2` | 869,358 | 894,667 | 555,350–1,123,862 | 862,418 | 773,504 (89.7%) | 88,914 | 6,940 | 95,854 | 18.0 (14–21) |
| `CodeGraph 1.6.2` | 3,308,912 | 3,219,962 | 1,650,687–5,495,578 | 3,302,442 | 3,115,236 (94.3%) | 187,206 | 6,470 | 193,677 | 31.3 (21–43) |
| `codebase-memory-mcp 0.12.1` | 1,827,391 | 1,686,510 | 1,266,007–3,294,229 | 1,818,397 | 1,683,598 (92.6%) | 134,799 | 8,994 | 143,793 | 29.9 (25–40) |

| Pairwise comparison | Fulfilled diff / ans | Two-sided `p` | One-sided `p` | Total token ratio | Uncached token ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| `codemap-search 0.14.2` − `Graphify 0.9.95` | +0.44 | 0.2600 | 0.1300 | ×1.13 | ×1.22 |
| `codemap-search 0.14.2` − `rg 15.2.0` | +1.00 | 0.0280 | 0.0140 | ×1.54 | ×1.27 |
| `codemap-search 0.14.2` − `zvec-grep 0.2.2` | +1.22 | 0.0100 | 0.0050 | ×1.53 | ×1.38 |
| `codemap-search 0.14.2` − `CodeGraph 1.6.2` | +1.33 | 0.0130 | 0.0065 | ×0.40 | ×0.68 |
| `codemap-search 0.14.2` − `codebase-memory-mcp 0.12.1` | +1.44 | 0.0070 | 0.0035 | ×0.73 | ×0.92 |
| `Graphify 0.9.95` − `rg 15.2.0` | +0.56 | 0.2080 | 0.1040 | ×1.37 | ×1.04 |
| `zvec-grep 0.2.2` − `rg 15.2.0` | -0.22 | 0.7700 | 0.8160 | ×1.00 | ×0.92 |
| `CodeGraph 1.6.2` − `rg 15.2.0` | -0.33 | 0.6080 | 0.8420 | ×3.82 | ×1.85 |
| `codebase-memory-mcp 0.12.1` − `rg 15.2.0` | -0.44 | 0.3520 | 0.9500 | ×2.11 | ×1.38 |

## Results by question and repetition

Each question has `12` required facts per answer (`7` Core + `5` Extended; `36` facts across `3` runs per tool/environment).

### `alertmanager-config-autosync`

Traces how the Kubernetes-style API connects to automatic syncing of external Alertmanager configurations.

| Environment | Tool | Fulfilled | Core | Extended | Factually correct | Run 1 · Run 2 · Run 3 (Core+Ext) | Mean total tokens | Mean uncached tokens | Mean calls |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: |
| `pi direct` | `codemap-search 0.14.2` | 30/36 | 18/21 | 12/15 | 31 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 929,699 | 93,902 | 34.7 |
|  | `Graphify 0.9.95` | 30/36 | 18/21 | 12/15 | 31 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 873,702 | 97,638 | 57.3 |
|  | `zvec-grep 0.2.2` | 30/36 | 18/21 | 12/15 | 30 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 677,822 | 66,152 | 38.0 |
|  | `codebase-memory-mcp 0.12.1` | 30/36 | 18/21 | 12/15 | 31 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 987,358 | 87,774 | 71.7 |
|  | `rg 15.2.0` | 28/36 | 17/21 | 11/15 | 31 | 8 (5+3) · 10 (6+4) · 10 (6+4) | 460,328 | 60,115 | 55.7 |
|  | `CodeGraph 1.6.2` | 24/36 | 14/21 | 10/15 | 25 | 8 (5+3) · 7 (4+3) · 9 (5+4) | 1,191,822 | 110,648 | 19.3 |
| `pi codemode` | `codemap-search 0.14.2` | 30/36 | 18/21 | 12/15 | 32 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 814,705 | 109,083 | 34.3 |
|  | `zvec-grep 0.2.2` | 29/36 | 17/21 | 12/15 | 30 | 10 (6+4) · 10 (6+4) · 9 (5+4) | 508,009 | 76,308 | 42.7 |
|  | `Graphify 0.9.95` | 28/36 | 16/21 | 12/15 | 33 | 10 (6+4) · 9 (5+4) · 9 (5+4) | 699,135 | 94,719 | 59.3 |
|  | `rg 15.2.0` | 30/36 | 18/21 | 12/15 | 33 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 500,342 | 81,185 | 64.7 |
|  | `CodeGraph 1.6.2` | 28/36 | 16/21 | 12/15 | 29 | 10 (6+4) · 9 (5+4) · 9 (5+4) | 1,061,231 | 117,274 | 19.7 |
|  | `codebase-memory-mcp 0.12.1` | 29/36 | 18/21 | 11/15 | 29 | 9 (6+3) · 10 (6+4) · 10 (6+4) | 1,142,482 | 110,247 | 102.3 |
| `Codex code mode` | `codemap-search 0.14.2` | 29/36 | 17/21 | 12/15 | 30 | 9 (5+4) · 10 (6+4) · 10 (6+4) | 1,306,111 | 133,417 | 43.0 |
|  | `Graphify 0.9.95` | 29/36 | 17/21 | 12/15 | 30 | 10 (6+4) · 9 (5+4) · 10 (6+4) | 1,148,878 | 106,019 | 69.3 |
|  | `rg 15.2.0` | 29/36 | 17/21 | 12/15 | 32 | 9 (5+4) · 10 (6+4) · 10 (6+4) | 891,082 | 102,389 | 20.3 |
|  | `zvec-grep 0.2.2` | 29/36 | 17/21 | 12/15 | 32 | 10 (6+4) · 9 (5+4) · 10 (6+4) | 730,925 | 90,114 | 18.7 |
|  | `CodeGraph 1.6.2` | 29/36 | 17/21 | 12/15 | 31 | 10 (6+4) · 9 (5+4) · 10 (6+4) | 3,455,409 | 185,820 | 32.7 |
|  | `codebase-memory-mcp 0.12.1` | 29/36 | 17/21 | 12/15 | 30 | 10 (6+4) · 9 (5+4) · 10 (6+4) | 1,493,129 | 109,535 | 114.3 |

### `git-sync-job-dispatch`

Traces how Git repository sync jobs are dispatched across the message bus to the executing job handler.

| Environment | Tool | Fulfilled | Core | Extended | Factually correct | Run 1 · Run 2 · Run 3 (Core+Ext) | Mean total tokens | Mean uncached tokens | Mean calls |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: |
| `pi direct` | `codemap-search 0.14.2` | 33/36 | 20/21 | 13/15 | 34 | 12 (7+5) · 11 (7+4) · 10 (6+4) | 1,027,200 | 106,795 | 52.3 |
|  | `Graphify 0.9.95` | 32/36 | 19/21 | 13/15 | 34 | 12 (7+5) · 11 (7+4) · 9 (5+4) | 1,552,796 | 100,422 | 59.3 |
|  | `zvec-grep 0.2.2` | 33/36 | 21/21 | 12/15 | 33 | 12 (7+5) · 10 (7+3) · 11 (7+4) | 1,137,786 | 101,242 | 56.0 |
|  | `codebase-memory-mcp 0.12.1` | 30/36 | 21/21 | 9/15 | 30 | 9 (7+2) · 10 (7+3) · 11 (7+4) | 2,074,625 | 94,422 | 86.7 |
|  | `rg 15.2.0` | 32/36 | 21/21 | 11/15 | 32 | 10 (7+3) · 11 (7+4) · 11 (7+4) | 826,434 | 87,874 | 74.7 |
|  | `CodeGraph 1.6.2` | 30/36 | 20/21 | 10/15 | 30 | 10 (7+3) · 11 (7+4) · 9 (6+3) | 2,331,725 | 159,479 | 29.0 |
| `pi codemode` | `codemap-search 0.14.2` | 35/36 | 21/21 | 14/15 | 35 | 11 (7+4) · 12 (7+5) · 12 (7+5) | 982,222 | 107,043 | 45.7 |
|  | `zvec-grep 0.2.2` | 32/36 | 21/21 | 11/15 | 32 | 9 (7+2) · 11 (7+4) · 12 (7+5) | 643,362 | 79,394 | 63.3 |
|  | `Graphify 0.9.95` | 32/36 | 20/21 | 12/15 | 34 | 10 (7+3) · 10 (6+4) · 12 (7+5) | 1,064,101 | 115,066 | 83.7 |
|  | `rg 15.2.0` | 32/36 | 19/21 | 13/15 | 34 | 11 (7+4) · 10 (6+4) · 11 (6+5) | 704,707 | 85,827 | 73.0 |
|  | `CodeGraph 1.6.2` | 35/36 | 21/21 | 14/15 | 35 | 12 (7+5) · 11 (7+4) · 12 (7+5) | 2,722,023 | 179,047 | 30.3 |
|  | `codebase-memory-mcp 0.12.1` | 29/36 | 21/21 | 8/15 | 31 | 10 (7+3) · 9 (7+2) · 10 (7+3) | 1,426,901 | 126,080 | 104.3 |
| `Codex code mode` | `codemap-search 0.14.2` | 36/36 | 21/21 | 15/15 | 36 | 12 (7+5) · 12 (7+5) · 12 (7+5) | 1,559,975 | 143,655 | 56.0 |
|  | `Graphify 0.9.95` | 33/36 | 21/21 | 12/15 | 33 | 11 (7+4) · 10 (7+3) · 12 (7+5) | 1,560,296 | 125,416 | 81.7 |
|  | `rg 15.2.0` | 29/36 | 21/21 | 8/15 | 29 | 10 (7+3) · 9 (7+2) · 10 (7+3) | 1,096,712 | 109,192 | 23.7 |
|  | `zvec-grep 0.2.2` | 31/36 | 21/21 | 10/15 | 31 | 11 (7+4) · 10 (7+3) · 10 (7+3) | 1,046,671 | 106,938 | 22.0 |
|  | `CodeGraph 1.6.2` | 31/36 | 21/21 | 10/15 | 31 | 11 (7+4) · 9 (7+2) · 11 (7+4) | 4,157,775 | 237,434 | 41.7 |
|  | `codebase-memory-mcp 0.12.1` | 29/36 | 20/21 | 9/15 | 30 | 10 (7+3) · 9 (6+3) · 10 (7+3) | 2,437,177 | 149,390 | 112.0 |

### `scope-annotation-filter`

Traces how dashboard scope filters connect to annotation storage and querying.

| Environment | Tool | Fulfilled | Core | Extended | Factually correct | Run 1 · Run 2 · Run 3 (Core+Ext) | Mean total tokens | Mean uncached tokens | Mean calls |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: |
| `pi direct` | `codemap-search 0.14.2` | 33/36 | 21/21 | 12/15 | 33 | 11 (7+4) · 11 (7+4) · 11 (7+4) | 879,894 | 93,547 | 48.3 |
|  | `Graphify 0.9.95` | 31/36 | 19/21 | 12/15 | 31 | 10 (6+4) · 11 (7+4) · 10 (6+4) | 596,484 | 82,350 | 50.7 |
|  | `zvec-grep 0.2.2` | 26/36 | 16/21 | 10/15 | 27 | 8 (5+3) · 10 (6+4) · 8 (5+3) | 536,548 | 63,118 | 42.7 |
|  | `codebase-memory-mcp 0.12.1` | 27/36 | 15/21 | 12/15 | 27 | 9 (5+4) · 9 (5+4) · 9 (5+4) | 931,799 | 85,677 | 63.3 |
|  | `rg 15.2.0` | 26/36 | 15/21 | 11/15 | 27 | 8 (5+3) · 9 (5+4) · 9 (5+4) | 376,971 | 55,264 | 60.7 |
|  | `CodeGraph 1.6.2` | 24/36 | 16/21 | 8/15 | 25 | 8 (6+2) · 8 (5+3) · 8 (5+3) | 1,657,880 | 125,165 | 25.7 |
| `pi codemode` | `codemap-search 0.14.2` | 32/36 | 21/21 | 11/15 | 33 | 11 (7+4) · 10 (7+3) · 11 (7+4) | 689,815 | 87,617 | 37.7 |
|  | `zvec-grep 0.2.2` | 10/12 | 6/7 | 4/5 | 10 | — · — · 10 (6+4) | 800,319 | 88,511 | 71.0 |
|  | `Graphify 0.9.95` | 28/36 | 18/21 | 10/15 | 29 | 9 (6+3) · 9 (6+3) · 10 (6+4) | 473,627 | 69,104 | 55.0 |
|  | `rg 15.2.0` | 25/36 | 16/21 | 9/15 | 26 | 10 (6+4) · 8 (5+3) · 7 (5+2) | 309,565 | 60,435 | 62.0 |
|  | `CodeGraph 1.6.2` | 22/36 | 14/21 | 8/15 | 23 | 8 (5+3) · 7 (5+2) · 7 (4+3) | 1,388,768 | 129,589 | 29.0 |
|  | `codebase-memory-mcp 0.12.1` | 26/36 | 16/21 | 10/15 | 27 | 9 (5+4) · 8 (5+3) · 9 (6+3) | 999,291 | 102,609 | 75.7 |
| `Codex code mode` | `codemap-search 0.14.2` | 33/36 | 21/21 | 12/15 | 33 | 11 (7+4) · 11 (7+4) · 11 (7+4) | 1,132,993 | 120,769 | 52.3 |
|  | `Graphify 0.9.95` | 32/36 | 20/21 | 12/15 | 32 | 10 (6+4) · 11 (7+4) · 11 (7+4) | 839,915 | 93,547 | 60.7 |
|  | `rg 15.2.0` | 31/36 | 20/21 | 11/15 | 32 | 10 (6+4) · 11 (7+4) · 10 (7+3) | 608,739 | 101,902 | 17.0 |
|  | `zvec-grep 0.2.2` | 27/36 | 17/21 | 10/15 | 28 | 8 (5+3) · 10 (7+3) · 9 (5+4) | 830,478 | 90,510 | 22.0 |
|  | `CodeGraph 1.6.2` | 26/36 | 16/21 | 10/15 | 27 | 8 (5+3) · 10 (6+4) · 8 (5+3) | 2,313,552 | 157,776 | 34.7 |
|  | `codebase-memory-mcp 0.12.1` | 27/36 | 16/21 | 11/15 | 28 | 10 (6+4) · 8 (5+3) · 9 (5+4) | 1,551,868 | 172,455 | 83.0 |

### Per-fact fulfillment across all 36 rubric items

Individual rubric text and expected file paths remain private to avoid contaminating future model runs. Even with opaque fact IDs (`A1–AE5`, `G1–GE5`, `B1–BE5`), this matrix shows where tools converged or diverged: `A7`, `AE4`, and `BE5` scored `0` across every run of every tested tool (`0/9` per tool, or `0/7` for `zvec-grep 0.2.2` on `BE5`), indicating uniformly unfulfilled hops across all arms, whereas on `scope-annotation-filter` core hop `B6`, `codemap-search 0.14.2` succeeded `9/9` times versus `0–3/9` for the other tools (`1/7` for `zvec-grep 0.2.2`).

Cell format: fulfilled runs in `pi direct · pi codemode · Codex code mode` (each out of `3` runs unless shown as `v/d`, as in `1/1` or `0/1` for `zvec-grep 0.2.2` on `B*`/`BE*` where `2` of `3` `pi codemode` runs crashed), followed by total across environments (`sum/9`, or `sum/7` for `zvec-grep 0.2.2` on `B*`/`BE*`).

#### `alertmanager-config-autosync` (`A1–AE5`)

| Fact ID | Level | `codemap-search 0.14.2` | `Graphify 0.9.95` | `zvec-grep 0.2.2` | `codebase-memory-mcp 0.12.1` | `rg 15.2.0` | `CodeGraph 1.6.2` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `A1` | core | 3·3·2 (8/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·2 (8/9) | 2·3·3 (8/9) | 0·1·2 (3/9) |
| `A2` | core | 3·3·3 (9/9) | 3·1·2 (6/9) | 3·2·2 (7/9) | 3·3·3 (9/9) | 3·3·2 (8/9) | 2·3·3 (8/9) |
| `A3` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `A4` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `A5` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `A6` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `A7` | core | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) |
| `AE1` | extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `AE2` | extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·2·3 (8/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `AE3` | extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 2·3·3 (8/9) | 1·3·3 (7/9) |
| `AE4` | extended | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) |
| `AE5` | extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |

#### `git-sync-job-dispatch` (`G1–GE5`)

| Fact ID | Level | `codemap-search 0.14.2` | `Graphify 0.9.95` | `zvec-grep 0.2.2` | `codebase-memory-mcp 0.12.1` | `rg 15.2.0` | `CodeGraph 1.6.2` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `G1` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `G2` | core | 3·3·3 (9/9) | 2·3·3 (8/9) | 3·3·3 (9/9) | 3·3·2 (8/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `G3` | core | 2·3·3 (8/9) | 3·2·3 (8/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `G4` | core | 3·3·3 (9/9) | 2·3·3 (8/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·2·3 (8/9) | 2·3·3 (8/9) |
| `G5` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `G6` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·2·3 (8/9) | 3·3·3 (9/9) |
| `G7` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `GE1` | extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `GE2` | extended | 3·3·3 (9/9) | 2·1·3 (6/9) | 2·1·2 (5/9) | 2·1·2 (5/9) | 0·2·1 (3/9) | 3·2·2 (7/9) |
| `GE3` | extended | 1·3·3 (7/9) | 2·2·1 (5/9) | 1·2·0 (3/9) | 1·1·2 (4/9) | 3·2·1 (6/9) | 0·3·1 (4/9) |
| `GE4` | extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 0·0·0 (0/9) | 3·3·3 (9/9) | 2·3·2 (7/9) |
| `GE5` | extended | 3·2·3 (8/9) | 3·3·2 (8/9) | 3·2·2 (7/9) | 3·3·2 (8/9) | 2·3·0 (5/9) | 2·3·2 (7/9) |

#### `scope-annotation-filter` (`B1–BE5`)

| Fact ID | Level | `codemap-search 0.14.2` | `Graphify 0.9.95` | `zvec-grep 0.2.2` | `codebase-memory-mcp 0.12.1` | `rg 15.2.0` | `CodeGraph 1.6.2` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `B1` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 2·3·3 (8/9) |
| `B2` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `B3` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 1·1/1·1 (3/7) | 0·1·0 (1/9) | 0·0·3 (3/9) | 2·0·1 (3/9) |
| `B4` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `B5` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `B6` | core | 3·3·3 (9/9) | 1·0·2 (3/9) | 0·0/1·1 (1/7) | 0·0·1 (1/9) | 0·1·2 (3/9) | 0·0·0 (0/9) |
| `B7` | core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·2·3 (8/9) |
| `BE1` | extended | 3·2·3 (8/9) | 3·2·3 (8/9) | 2·1/1·2 (5/7) | 3·2·2 (7/9) | 2·2·2 (6/9) | 0·2·2 (4/9) |
| `BE2` | extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `BE3` | extended | 3·3·3 (9/9) | 3·2·3 (8/9) | 2·1/1·2 (5/7) | 3·2·3 (8/9) | 3·1·3 (7/9) | 2·0·2 (4/9) |
| `BE4` | extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `BE5` | extended | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0/1·0 (0/7) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) |

## Tool-call breakdown, citations, and output truncations

- **Dispatched, Rejected, and Errors**: `Total calls = Dispatched + Rejected` (`Total calls / ans` and `Dispatched / ans` are per-answer means; `Rejected (sum)` and `Errors (sum)` are totals across all answers in that cell). `Rejected` counts calls blocked before execution by the benchmark's read-only path sandbox (such as targeting `.git` or a path outside the source checkout). `Errors ⊂ Dispatched` counts dispatched calls that returned an error (such as a nonexistent path or failing query).
- **Helper `read` (`Graphify 0.9.95`) vs `zvec-grep 0.2.2`**: `Graphify`'s 7 native graph tools return graph entities and locations without raw source bodies, so the harness attached a read-only MCP `read` helper. `zvec-grep 0.2.2` did not receive a `read` helper because `zvec_grep_search` already returns bounded source snippets alongside matches; it used built-in tools or the shell when reading broader files.
- **Client output-cap hits, script truncations, and adapter truncations**: `Built-in/CLI cap hits (answers)` counts how often outputs hit client caps (in `pi`: built-in `read` `2,000` lines / `50 KiB`, `grep` `100` matches, `find` `1,000` entries, `ls` `500` entries; in `Codex code mode`: `Codex CLI 0.164.0` `Warning: truncated output` across both shell `exec_command` calls and `exec` code-mode cells, which default to a `10,000`-token output cap unless overridden via `// @exec: {"max_output_tokens": ...}`). `Script truncations` (`pi codemode`) counts `codemode` script responses clipped at the extension's `10,000`-token default. `Adapter truncations` (`pi`) counts direct MCP responses clipped by `@buyong/pi-mcp-adapter 0.1.2` (`20 KiB` default unless overridden by `_meta["pi/maxOutputBytes"]`, which `codemap-search 0.14.2` sets to `100,000` bytes).
- **Citation counts**: Recomputed directly from the normalized review citation index across all `160` eligible slots. `Range citations` counts multi-line `path:start-end` citations (`end > start`), `Single-line` counts 1-line citations (`path:line` or `start == end`), and `Cited test/gen` counts answers citing at least one test or generated file.

### Call outcomes and per-subtool calls (per answer)

| Environment | Tool | Total calls / ans | Dispatched / ans | Rejected (sum) | Errors (sum) | Built-in / shell / ans | Per-subtool calls / ans |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `pi direct` | `codemap-search 0.14.2` | 45.1 | 44.6 | 5 | 0 | 0.0 | `read` 20.7, `grep` 17.6, `find` 3.6, `search` 1.8, `initial_instructions` 1.0 |
|  | `Graphify 0.9.95` | 55.8 | 54.2 | 14 | 7 | 16.8 | helper `read` 30.6, grep (built-in) 14.9, `query_graph` 4.7, find (built-in) 1.4, `get_neighbors` 1.1, `get_node` 1.0, ls (built-in) 0.4, `graph_stats` 0.1 |
|  | `zvec-grep 0.2.2` | 45.6 | 45.6 | 0 | 2 | 38.6 | read (built-in) 28.2, grep (built-in) 9.8, `zvec_grep_search` 7.0, find (built-in) 0.3, ls (built-in) 0.2 |
|  | `codebase-memory-mcp 0.12.1` | 73.9 | 73.1 | 7 | 12 | 5.0 | `get_code_snippet` 32.6, `search_code` 19.1, `search_graph` 8.1, read (built-in) 5.0, `get_file_outline` 4.1, `trace_path` 1.7, `check_index_coverage` 1.6, `list_projects` 1.0 |
|  | `rg 15.2.0` | 63.7 | 63.7 | 0 | 11 | 63.7 | `read` 36.7, `grep` 18.8, `ls` 4.2, `find` 4.0 |
|  | `CodeGraph 1.6.2` | 24.7 | 24.7 | 0 | 0 | 2.8 | `codegraph_explore` 21.9, grep (built-in) 1.0, find (built-in) 0.8, ls (built-in) 0.6, read (built-in) 0.4 |
| `pi codemode` | `codemap-search 0.14.2` | 39.2 | 38.6 | 6 | 0 | 0.0 | `grep` 17.9, `read` 13.6, `find` 4.4, `search` 1.7, `initial_instructions` 1.0 |
|  | `zvec-grep 0.2.2` | 55.6 | 55.6 | 0 | 3 | 48.4 | read (built-in) 33.4, grep (built-in) 13.6, `zvec_grep_search` 7.1, find (built-in) 0.7, ls (built-in) 0.7 |
|  | `Graphify 0.9.95` | 66.0 | 65.8 | 2 | 7 | 22.9 | helper `read` 36.1, grep (built-in) 19.0, `query_graph` 5.6, find (built-in) 2.3, ls (built-in) 1.6, `get_neighbors` 0.7, `get_node` 0.6 |
|  | `rg 15.2.0` | 66.6 | 66.6 | 0 | 12 | 66.6 | `read` 39.8, `grep` 19.4, `ls` 4.1, `find` 3.2 |
|  | `CodeGraph 1.6.2` | 26.3 | 26.3 | 0 | 0 | 4.6 | `codegraph_explore` 21.8, grep (built-in) 2.6, ls (built-in) 0.8, read (built-in) 0.8, find (built-in) 0.4 |
|  | `codebase-memory-mcp 0.12.1` | 94.1 | 92.3 | 16 | 23 | 6.3 | `get_code_snippet` 40.2, `search_code` 22.1, `search_graph` 10.6, `get_file_outline` 7.8, read (built-in) 6.3, `check_index_coverage` 2.3, `trace_path` 1.7, `list_projects` 1.0, `get_architecture` 0.3 |
| `Codex code mode` | `codemap-search 0.14.2` | 50.4 | 49.9 | 5 | 5 | 0.0 | `grep` 24.8, `read` 17.3, `find` 5.8, `initial_instructions` 1.0, `search` 1.0 |
|  | `Graphify 0.9.95` | 70.6 | 70.0 | 5 | 20 | 11.2 | helper `read` 51.7, shell (built-in) 11.2, `query_graph` 6.7, `get_neighbors` 0.4 |
|  | `rg 15.2.0` | 20.3 | 20.3 | 0 | 14 | 20.3 | shell (built-in) 20.3 |
|  | `zvec-grep 0.2.2` | 20.9 | 20.9 | 0 | 18 | 15.7 | shell (built-in) 15.7, `zvec_grep_search` 5.2 |
|  | `CodeGraph 1.6.2` | 36.3 | 36.3 | 0 | 3 | 3.4 | `codegraph_explore` 32.9, shell (built-in) 3.4 |
|  | `codebase-memory-mcp 0.12.1` | 103.1 | 100.3 | 25 | 24 | 2.1 | `get_code_snippet` 48.9, `search_code` 22.1, `search_graph` 18.9, `get_file_outline` 2.7, shell (built-in) 2.1, `trace_path` 1.9, `check_index_coverage` 1.2, `list_projects` 1.0, `query_graph` 0.8, `get_architecture` 0.4, `get_graph_schema` 0.3 |

### Citations, answer length, and client output-cap hits

| Environment | Tool | Range citations / ans (sum) | Mean lines / range | Single-line / ans (sum) | Mean answer chars | Cited test/gen | Built-in/CLI cap hits (answers) | Script truncations | Adapter truncations |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `pi direct` | `codemap-search 0.14.2` | 73.7 (663) | 19.1 | 1.7 (15) | 10,109 | 0 | 0 (0) | — | 0 |
|  | `Graphify 0.9.95` | 58.3 (525) | 19.1 | 1.1 (10) | 8,821 | 8 | 18 (8) | — | 0 |
|  | `zvec-grep 0.2.2` | 53.7 (483) | 20.6 | 0.7 (6) | 8,543 | 4 | 24 (9) | — | 0 |
|  | `codebase-memory-mcp 0.12.1` | 51.8 (466) | 19.8 | 1.1 (10) | 8,266 | 7 | 0 (0) | — | 0 |
|  | `rg 15.2.0` | 55.0 (495) | 18.1 | 1.1 (10) | 8,370 | 6 | 32 (9) | — | 0 |
|  | `CodeGraph 1.6.2` | 51.8 (466) | 17.6 | 0.2 (2) | 8,078 | 5 | 6 (3) | — | 0 |
| `pi codemode` | `codemap-search 0.14.2` | 68.3 (615) | 18.1 | 1.9 (17) | 9,598 | 0 | 0 (0) | 4 | 0 |
|  | `zvec-grep 0.2.2` | 52.6 (368) | 18.6 | 1.1 (8) | 8,761 | 2 | 15 (6) | 4 | 0 |
|  | `Graphify 0.9.95` | 60.6 (545) | 17.7 | 1.2 (11) | 9,137 | 9 | 32 (8) | 5 | 0 |
|  | `rg 15.2.0` | 58.2 (524) | 19.4 | 0.9 (8) | 8,895 | 6 | 36 (9) | 11 | 0 |
|  | `CodeGraph 1.6.2` | 51.7 (465) | 16.4 | 0.8 (7) | 8,196 | 5 | 5 (4) | 6 | 0 |
|  | `codebase-memory-mcp 0.12.1` | 55.1 (496) | 19.9 | 1.6 (14) | 8,454 | 9 | 0 (0) | 5 | 0 |
| `Codex code mode` | `codemap-search 0.14.2` | 82.9 (746) | 18.3 | 2.0 (18) | 10,729 | 0 | 0 (0) | — | — |
|  | `Graphify 0.9.95` | 67.0 (603) | 18.8 | 1.3 (12) | 9,524 | 8 | 27 (9) | — | — |
|  | `rg 15.2.0` | 69.9 (629) | 15.9 | 1.9 (17) | 9,686 | 5 | 25 (9) | — | — |
|  | `zvec-grep 0.2.2` | 63.4 (571) | 15.9 | 1.3 (12) | 9,314 | 4 | 17 (5) | — | — |
|  | `CodeGraph 1.6.2` | 61.0 (549) | 16.9 | 0.8 (7) | 9,338 | 4 | 17 (9) | — | — |
|  | `codebase-memory-mcp 0.12.1` | 54.9 (494) | 19.4 | 1.9 (17) | 8,366 | 6 | 5 (5) | — | — |

## Product changes informed by this benchmark

- **Compact ranked `search` output near client limits (`e727d20c6`)**: Ranked `search` pages render one compact entry per listed file with matching-line previews and replace lower-priority whole function bodies with exact `read` ranges near configured client output budgets instead of truncating mid-response (`0` client/built-in/adapter output-cap hits across all `27` `codemap-search` answers).
- **Line-range citation and hand-written implementation guidance (`87e36d849`, `f33203c88`, `a7c083ae6`)**: Server instructions ask agents to cite evidence as `path:start-end` line ranges covering the supporting lines and to avoid reading or citing test or generated files unless requested, tracing hand-written implementations instead (`0/27` answers citing test or generated files vs `2–9/9` across comparison tools; causal contribution of the guidance alone was not isolated in this run).
- **`_meta["pi/maxOutputBytes"]` delivery metadata (`0.14.2`)**: Every tool advertises `output.client.pi_max_bytes` (`100,000` bytes by default) via `_meta["pi/maxOutputBytes"]` in `tools/list`, allowing `@buyong/pi-mcp-adapter` to apply `codemap-search`'s delivery budget ahead of its `20 KiB` default without manual `mcp.json` configuration (`0` adapter truncations).

On `gpt-6.1-sol`, higher accuracy over `rg 15.2.0` (`+1.00` to `+1.11` facts/answer) still came with `1.54×`–`1.71×` total tokens (`1.27×`–`1.45×` uncached); reducing GPT token overhead relative to `rg` remains an open tradeoff.

## Scope and reproducibility notes

- **Sample size & unadjusted `p`-values**: With `9` answers per tool and environment (`7` for `zvec-grep 0.2.2` in `pi codemode`), exact permutation tests reliably separate differences of roughly `1` fact/answer or larger; `0.3–0.8` gaps are inconclusive. Reported `p`-values are unadjusted across `24` external-MCP comparisons (`27` total pairwise rows).
- **Single repository and 3-question / single-question-driven limit**: Only one repository (`Grafana`) and `3` questions were tested; `alertmanager-config-autosync` scored `28–30/36` across almost all tools (`CodeGraph 1.6.2` `pi direct` `24/36`), so most tool separation came from `scope-annotation-filter`.
- **Single solver model**: Only `gpt-6.1-sol` (`medium`) was evaluated here; token ratios and scores can differ on other model families.
- **Hardware, cache, index state, and client-version sensitivity**: Measurements reflect the specific host hardware, cache state, pinned pre-built index snapshots, and pinned client/adapter versions (`pi 1.4.0`, `@buyong/pi-mcp-adapter 0.1.2`, `Codex CLI 0.164.0`); changing any of these can shift token counts and truncation rates.
- **Two same-day batches**: `codemap-search 0.14.2` + `rg 15.2.0` and the four external MCP servers ran in two same-day batches on the same host, so wall-clock answer durations are omitted.
- **Reruns and crashes**: Four `Codex` `scope-annotation-filter` run-2 slots in the second batch were rerun after fixing a harness wall-clock timestamp check triggered by a host clock adjustment (`CodeGraph 1.6.2` `10/12`, `Graphify 0.9.95` `11/12`, `zvec-grep 0.2.2` `10/12`, `codebase-memory-mcp 0.12.1` `8/12`). Two `pi codemode` `zvec-grep 0.2.2` runs ended on server `SIGSEGV`.
- **Transient concurrent-cleanup errors**: Two `pi` `codebase-memory-mcp 0.12.1` answers (`9/12`, `10/12`) hit one tool failure each from shared temporary state cleanup from another concurrent run; excluding both gives `78/96` (`+0.92`, `p = 0.0350`) in `pi direct` and `74/96` (`+1.53`, `p = 0.0025`) in `pi codemode`, with unchanged conclusions.
