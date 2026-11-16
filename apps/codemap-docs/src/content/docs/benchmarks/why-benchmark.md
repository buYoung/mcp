---
title: Why Benchmark
description: Why codemap-search was evaluated on multi-hop end-to-end Grafana questions requiring line-range citations instead of isolated lookup micro-benchmarks.
---

Code-navigation tools for coding agents are often checked with isolated lookup micro-benchmarks: finding a single symbol definition or matching a known identifier. Those checks miss the failure modes that happen when an agent investigates an unfamiliar repository end to end across multiple files, layers, and languages.

## Why multi-hop questions with line-range citations

On a large codebase, answering an architecture question requires discovering an entry point, tracing delegation across packages or language boundaries, checking configuration guards or secondary branches, and verifying the implementation in source rather than guessing from names or tests.

To measure end-to-end navigation behavior rather than single-hit recall, this evaluation used multi-hop architecture questions where every claimed fact must be backed by `path:start-end` line citations. Answers were graded by a blind grader model (`gpt-6-astra`, reasoning effort `high`) with no tools or repository access. A fact only counts as fulfilled when the explanation is both accurate and established by the cited line ranges; describing the right behavior without supporting line ranges counts as a weak citation, not a fulfilled fact.

## What was tested

Under fixed read-only conditions on **Grafana** (`24,115` tracked files, TypeScript + Go, pinned commit `c0a598f5a49abf2014f67e806075d8ad267c9efb`, `2026-10-26`), `codemap-search 0.14.2` was measured alongside a built-in-tools baseline (`rg 15.2.0`) and four open-source MCP code-navigation servers (`Graphify 0.9.95`, `zvec-grep 0.2.2`, `codebase-memory-mcp 0.12.1`, and `CodeGraph 1.6.2`; `Serena 2.1.0` was excluded before running because full LSP indexing took ~34–70 min per run).

- **Solver and grader**: `gpt-6.1-sol` (reasoning effort `medium`) answered `3` multi-hop questions `3` times each (`9` answers / `108` graded facts per tool and environment, except `zvec-grep 0.2.2` in `pi codemode` with `7` answers / `84` facts after two server crashes). Each question contains `12` graded facts (`7` Core end-to-end hops and `5` Extended secondary branches or guards).
- **Total evaluated corpus**: `160` validated answers and `1,920` graded facts across `3` execution environments (`pi direct`, `pi codemode`, and `Codex code mode`), which are reported side-by-side and never pooled.

## How benchmarks are used in `codemap-search`

These measurements are used to identify navigation and delivery bottlenecks and to compare before/after behavior under fixed conditions—not to make universal performance claims. Accordingly, these docs make no claims of universal speed rankings, generalized accuracy across arbitrary codebases, or unqualified superiority over built-in file-reading and search tools. Every figure is tied to the tested repository, question set, solver model, pinned tool versions, and client configurations described in [Methodology](/benchmarks/methodology/).

## Benchmark sections

- [Methodology](/benchmarks/methodology/) — Compared tools, pinned commits, three execution environments, sandbox isolation rules, and pre-built Grafana index footprints.
- [Metrics](/benchmarks/metrics/) — Exact definitions of accuracy scores, token accounting, call outcomes, citation counts, client output caps, and permutation tests.
- [Results](/benchmarks/results/) — Cross-environment summary, detailed tables, per-question scores, 36-fact matrices (`A1–AE5`, `G1–GE5`, and `B1–BE5`), and subtool call/truncation records.
- [What Changed Because of Benchmarks](/benchmarks/what-changed/) — Product and instruction changes informed by observed bottlenecks, plus the open GPT token tradeoff.
- [Limits](/benchmarks/limits/) — Sample size, unadjusted `p`-values, single-repo scope, tool-condition asymmetries, crashes, and non-claims.
- [Reproduction Notes](/benchmarks/reproduction-notes/) — Pinned commits, same-day batches, reruns, hardware/cache sensitivity, and committed versus withheld artifacts.
