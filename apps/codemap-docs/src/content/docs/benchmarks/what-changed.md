---
title: What Changed Because of Benchmarks
description: Product and instruction changes informed by the Grafana benchmark, observed outcomes under fixed conditions, and the open GPT token tradeoff.
---

This benchmark was used to find delivery, truncation, and citation bottlenecks and to compare before/after behavior under fixed conditions. Below are the product and instruction changes reflected in `codemap-search 0.14.2`, what was observed on **Grafana** (`24,115` tracked files, TypeScript + Go) with `gpt-6.1-sol` (`medium`), and the token tradeoff that remains open.

## Compact ranked `search` output near client limits (`e727d20c6`)

- **Change**: Ranked `search` pages render one compact entry per listed file with matching-line previews and replace lower-priority whole function bodies with exact `read` ranges near configured client output budgets instead of truncating mid-response (`e727d20c6`).
- **Observed on Grafana**: `0` built-in/CLI cap hits and `0` adapter output-cap hits across all `27` `codemap-search 0.14.2` answers across all three environments.
- **Script-level cap nuance**: In `pi codemode`, `codemap-search 0.14.2` still recorded `4` script-level truncations when combined JS script outputs hit `codemode`'s `10,000`-token default script cap (compared with `4` across `7` answers for `zvec-grep 0.2.2` and `5–11` across the other comparison arms).

## `_meta["pi/maxOutputBytes"]` delivery metadata (`0.14.2`)

- **Change**: Every MCP tool advertises `output.client.pi_max_bytes` (`100,000` bytes by default) via `_meta["pi/maxOutputBytes"]` in `tools/list` (`0.14.2`).
- **Why it mattered**: `@buyong/pi-mcp-adapter 0.1.2` (which enforces a `20 KiB` default cap per MCP response unless overridden in `mcp.json`) reads that metadata and applies `codemap-search`'s `100,000`-byte delivery budget ahead of its `20 KiB` default without manual `mcp.json` configuration (`pi`'s built-in MCP client ignores tool metadata and truncates at `20 KB`).
- **Observed on Grafana**: `0` adapter truncations across all `18` `pi` `codemap-search 0.14.2` answers (`9` in `pi direct` and `9` in `pi codemode`).

## Line-range citation and hand-written implementation guidance (`a2f81c087`, `87e36d849`, `f33203c88`, `a7c083ae6`)

- **Change**: Server instructions ask agents to cite evidence as `path:start-end` line ranges covering the supporting lines (`a2f81c087`, `f33203c88`) and to avoid reading or citing test or generated files unless the request is about tests or generated code, tracing and citing the hand-written implementation instead (`87e36d849`, `f33203c88`, `a7c083ae6`).
- **Observed on Grafana**: `0/27` `codemap-search 0.14.2` answers cited test or generated files (versus `2/7` for `zvec-grep 0.2.2` in `pi codemode` and `4–9/9` across all remaining comparison cells), and single-line citations stayed at `1.7–2.0` per answer (`2.2%–2.7%` of citations: `15/678` in `pi direct`, `17/632` in `pi codemode`, and `18/764` in `Codex code mode`).
- **Unisolated causality**: In this benchmark run, the causal contribution of server instructions alone versus tool design (indexed symbols, BM25 ranking, and embedded `read`, `grep`, and `find`) was not isolated.

## Open tradeoff observed on `gpt-6.1-sol`

On `gpt-6.1-sol` (`medium`) on `Grafana`, fulfilling `+1.00` to `+1.11` more cited facts per answer than `rg 15.2.0` across the three environments (`p = 0.0250`, `0.0300`, `0.0280`, unadjusted) still came with `1.54×`–`1.71×` total tokens (`1.27×`–`1.45×` uncached tokens). Reducing GPT token overhead relative to `rg 15.2.0` remains an open tradeoff.

## Related pages

- [Results](/benchmarks/results/) — Detailed token, truncation, and citation tables across all 18 environment/tool cells.
- [Limits](/benchmarks/limits/) — Non-claims, sample size, and tool-boundary caveats.
