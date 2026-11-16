---
title: Reproduction Notes
description: Pinned repository and tool commits, pre-built index snapshots, execution batches, reruns, sensitivity notes, and committed versus withheld benchmark artifacts.
---

This page records the pinned identities, execution batch notes, reruns, and artifact boundaries for the Grafana evaluation.

## Pinned codebase, tools, models, and clients

| Component | Exact identifier / pinned version | Commit / package pin |
| --- | --- | --- |
| Target codebase | [grafana/grafana](https://github.com/grafana/grafana) (`24,115` tracked files, TypeScript + Go, `2026-10-26`) | `c0a598f5a49abf2014f67e806075d8ad267c9efb` |
| `codemap-search 0.14.2` | [buYoung/mcp](https://github.com/buYoung/mcp) (`codemap-search`, optional Jev filter off) | `e727d20c6504948c823cf55398a2f68d45337f7c` (`e727d20c6`) |
| `Graphify 0.9.95` | [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify) (PyPI `graphifyy`, `v0.9.95`) | `92741d162b2094ffc2fed58cfbd8ee823e9da62c` (`92741d16`) |
| `zvec-grep 0.2.2` | [zvec-ai/zvec-grep](https://github.com/zvec-ai/zvec-grep) (npm `@zvec/zvec-grep`, `v0.2.2`, embedding `local/potion-code-16m-v2`) | `b1a9148e26a7bc9bd4a52229ffb7532d3063793d` (`b1a9148e`) |
| `codebase-memory-mcp 0.12.1` | [DeusData/codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) (`v0.12.1`) | `a7158ac2f48ca646b4527341b76f930f2aeff87e` (`a7158ac2`) |
| `rg 15.2.0` (baseline) | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) (`15.2.0`) | `e89fff89ac9af12e8d4ce9d5fd07beb408ca730f` (`e89fff89`) |
| `CodeGraph 1.6.2` | [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) (npm `@colbymchenry/codegraph`, `v1.6.2`) | `6560052a6f856855d3f71eee838fd66ccfa4285d` (`6560052a`) |
| `Serena 2.1.0` (excluded) | [oraios/serena](https://github.com/oraios/serena) (PyPI `serena-agent`, `v2.1.0`, excluded pre-run) | `5e4f5cd173c7ff3eed37ee3f1de6c266b937c5dc` (`5e4f5cd1`) |
| Solver & blind grader | `gpt-6.1-sol` (`medium`) & `gpt-6-astra` (`high`, no tools or repo access) | 3 independent runs per question per tool/env |
| `pi` client toolchain | `pi 1.4.0` (`@earendil-works/pi-coding-agent@1.4.0`, `--provider openai-codex --thinking medium`) + `@buyong/pi-mcp-adapter@0.1.2` | `pi direct` & `pi codemode` (QuickJS sandbox) |
| `Codex` client toolchain | `Codex CLI 0.164.0` (`@openai/codex@0.164.0`, `codex exec --json`, `code_mode_only`) | `Codex code mode` |

## Pre-built index snapshots and execution batches

- **Pre-built index snapshots**: Each indexed tool built its Grafana index once per batch on the pinned read-only checkout prior to solver runs; exact file/node/edge counts, disk sizes, and build times are listed in [Methodology](/benchmarks/methodology/).
- **Two same-day batches**: `codemap-search 0.14.2` + `rg 15.2.0` ran in one batch and the four external MCP servers ran in a second batch on the same host and day. Because the runs were split across two batches, wall-clock answer durations are omitted.
- **Reruns in `Codex code mode`**: Four `Codex` `scope-annotation-filter` run-2 slots (single `tool × environment × question × run` executions) in the second batch were rerun after fixing a harness wall-clock timestamp check triggered by a host clock adjustment; the final rerun scores reported in [Results](/benchmarks/results/) are `CodeGraph 1.6.2` `10/12`, `Graphify 0.9.95` `11/12`, `zvec-grep 0.2.2` `10/12`, and `codebase-memory-mcp 0.12.1` `8/12`.
- **Server crashes in `pi codemode`**: Two `pi codemode` `zvec-grep 0.2.2` runs on `scope-annotation-filter` terminated on server `SIGSEGV`, leaving `7` validated answers (`84` graded facts) in that cell.

## Hardware, cache, and client sensitivity

Observed token counts, prompt cache hit rates (`~86–94%`), build times, and output truncation counts depend on the specific host hardware, OS and prompt cache state, pre-built index snapshots, and pinned client/adapter versions (`pi 1.4.0`, `@buyong/pi-mcp-adapter 0.1.2`, `Codex CLI 0.164.0`). Changing any of those conditions can shift token counts, truncation rates, and execution behavior.

## Committed artifacts vs private ground truth

- **Committed in the repository**:
  - `benchmark/results/gpt-6.1-sol-pi.json` and `benchmark/results/gpt-6.1-sol-codex.json`: aggregated per-tool, per-question, repetition, token, tool-call, and permutation-test summaries.
  - `benchmark/pi/` and `benchmark/codex/`: evaluation harnesses, sandbox/isolation checks, execution contracts (`grafana-execution-v1.json`), and pinned comparison lockfiles/toolchains (`grafana-comparison-targets-v1/targets.lock.json`, `pi-package.json`, `cli-package.json`).
- **Withheld from public benchmark documentation**:
  - Individual rubric item text, expected Grafana file paths, symbol names, and grading checklists from the 3-question `grafana-routes-v6` benchmark dataset remain non-public and are represented only by opaque fact IDs (`A1–AE5`, `G1–GE5`, `B1–BE5`) in [Results](/benchmarks/results/) to prevent contaminating future model runs.

## Related pages

- [Methodology](/benchmarks/methodology/) — Tool roles, sandbox rules, and index footprints.
- [Limits](/benchmarks/limits/) — Sample size, non-claims, and concurrent-cleanup sensitivity checks.
