---
title: Limits
description: Scope boundaries, statistical limits, tool-condition differences, server crashes, transient errors, and what this benchmark does not claim.
---

:::note[What this benchmark does not claim]
- It does **not** claim universal speed rankings or that `codemap-search` outperforms comparison tools across all repositories or models.
- It does **not** claim generalized accuracy superiority across repositories or unqualified superiority over built-in `Read`/`Grep` or `rg`.
- It does **not** claim unconditional reproducibility independent of host hardware, cache state, pre-built index state, or pinned client/adapter versions.
- It does **not** isolate or establish the causal contribution of server instructions alone versus tool design in this run.

All reported figures are observations on **Grafana** (`24,115` tracked files, TypeScript + Go), `3` multi-hop questions, `gpt-6.1-sol` (`medium`), `gpt-6-astra` (`high`), and the pinned tool and client versions described in [Methodology](/benchmarks/methodology/).
:::

## Sample size and unadjusted `p`-values

- **Small sample resolution (`n = 9`, `n = 7` for `zvec-grep 0.2.2` in `pi codemode`)**: With `9` answers per tool and environment, question-stratified exact permutation tests reliably separate differences of roughly `1` fulfilled fact per answer or larger. Smaller gaps of `0.3–0.8` facts per answer fall within run-to-run noise and are treated as inconclusive. For instance, `codemap-search 0.14.2` scored `+0.33` to `+1.22` higher than `Graphify 0.9.95` and `zvec-grep 0.2.2`, reaching `p < 0.05` in only one environment each (`pi codemode` `p = 0.0180` vs `Graphify 0.9.95`; `Codex code mode` `p = 0.0100` vs `zvec-grep 0.2.2`), which is treated as inconclusive at `n = 9`.
- **Citation checks vs raw factual accuracy**: When counting raw `Factually correct` facts without citation verification, `Graphify 0.9.95` scored `96` versus `codemap-search 0.14.2`'s `98` in `pi direct`, `96` versus `100` in `pi codemode`, and `95` versus `99` in `Codex code mode`.
- **Unadjusted multiple comparisons**: Reported `p`-values are unadjusted across `24` external-MCP comparisons (`27` total pairwise comparison rows across the three environments).

## Single repository and `scope-annotation-filter`-driven separation

Only one repository (**Grafana**, `24,115` tracked files, TypeScript + Go) and `3` questions were tested. Most separation between tools came from a single question (`scope-annotation-filter`), where `codemap-search 0.14.2` fulfilled `33/36`, `32/36`, and `33/36` across the three environments (for example, `9/9` on core hop `B6` vs `0–3/9` for peers and `1/7` for `zvec-grep 0.2.2`). By contrast, `alertmanager-config-autosync` converged at `28–30/36` across almost every tool and environment (only `CodeGraph 1.6.2` in `pi direct` scored `24/36`).

## Single solver model

Only `gpt-6.1-sol` (reasoning effort `medium`) was evaluated as the solver model. Token overhead ratios, prompt cache rates, citation formatting habits, and fulfillment scores can differ substantially on other model families or reasoning levels.

## Tool-condition differences and pre-run exclusion

- **`Graphify 0.9.95` helper `read` vs `zvec-grep 0.2.2` fallback**: Because `Graphify`'s 7 native graph tools return entity metadata and locations without raw source bodies, the harness paired it with a read-only MCP `read` helper (`30.6–51.7` calls per answer). `zvec-grep 0.2.2` did not receive a `read` helper because `zvec_grep_search` returns bounded source snippets alongside matches, so it relied on client built-ins or the read-only shell (`15.7–48.4` calls per answer) when reading wider file ranges.
- **Bundled tools and client caps**: `codemap-search 0.14.2` bundles `read`, `grep`, and `find` inside the MCP server (`0.0` built-in/fallback calls, with ~95% of its calls being embedded `grep`/`read`/`find` and `1.0–1.8` calls/answer being `search`). Comparisons against `rg 15.2.0` and built-in fallbacks also reflect client truncation boundaries (`15–36` built-in/CLI cap hits across `zvec-grep 0.2.2` [`15` across `7` `pi codemode` answers and `17–24` elsewhere], `Graphify 0.9.95` [`18–32`], and `rg 15.2.0` [`25–36`] vs `0` for `codemap-search 0.14.2`).
- **`Serena 2.1.0` excluded pre-run**: `Serena 2.1.0` was excluded before running solver sessions because full Go + TypeScript LSP indexing of Grafana took ~34–70 min per run.

## Server crashes and transient concurrent-cleanup errors

- **`zvec-grep 0.2.2` crashes (`pi codemode`)**: Two `pi codemode` runs on `scope-annotation-filter` terminated when the `zvec-grep` server crashed with `SIGSEGV`, leaving `7` validated answers (`84` graded facts) for that cell.
- **Transient `codebase-memory-mcp 0.12.1` concurrent-cleanup errors**: Two `pi` `codebase-memory-mcp 0.12.1` answers (`9/12` in `pi direct` and `10/12` in `pi codemode`) hit one tool failure each caused by shared temporary-state cleanup from another concurrent run. Excluding both affected slots (single `tool × environment × question × run` executions) in a sensitivity check of `codemap-search 0.14.2` versus `codebase-memory-mcp 0.12.1` leaves `78/96` (`+0.92`, `p = 0.0350`) in `pi direct` and `74/96` (`+1.53`, `p = 0.0025`) in `pi codemode`, with unchanged conclusions.

## Non-pooled environments and execution sensitivity

`pi direct`, `pi codemode`, and `Codex code mode` differ in fallback tools, output caps, and prompt/token accounting and are never pooled. Measurements also reflect the specific host hardware, cache state, pre-built index snapshots, and pinned client/adapter versions (`pi 1.4.0`, `@buyong/pi-mcp-adapter 0.1.2`, `Codex CLI 0.164.0`); see [Reproduction Notes](/benchmarks/reproduction-notes/).
