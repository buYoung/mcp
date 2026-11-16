---
title: Methodology
description: Evaluation design, compared tools and pinned versions, three execution environments, sandbox rules, and pre-built Grafana index footprints.
---

This page walks through how the evaluation was structured, which tools and versions were pinned, how the three agent environments operate, how runs were isolated, and how pre-built Grafana indexes compare.

## Evaluation design

All measurements ran against a pinned read-only checkout of [grafana/grafana](https://github.com/grafana/grafana) (`c0a598f5a49abf2014f67e806075d8ad267c9efb`, `2026-10-26`, `24,115` tracked files, TypeScript + Go), which includes commits after the solver model's training cutoff.

- **Questions and grading**: `3` multi-hop architecture questions (`alertmanager-config-autosync`, `git-sync-job-dispatch`, and `scope-annotation-filter`), each evaluated against `12` required facts (`7` Core end-to-end hops + `5` Extended branches, guards, or conditions).
- **Repetitions**: `3` independent runs per question per tool and environment (`9` answers / `108` graded facts per cell, except `zvec-grep 0.2.2` in `pi codemode` with `7` answers / `84` facts due to two server crashes).
- **Shared instructions**: All tools—including `rg 15.2.0`—received the identical `path:start-end` line-range citation instruction. All MCP arms received the same instruction to prefer MCP tools and use built-in tools or the shell only when the MCP server could not perform the task.

## Compared tools and pinned versions

Each tool was pinned to an exact upstream commit or release prior to preparing indexes and running solver sessions.

| Tool & version | Upstream project / package | Pinned commit | Navigation approach & exposed tools |
| --- | --- | --- | --- |
| `codemap-search 0.14.2` | [buYoung/mcp](https://github.com/buYoung/mcp) (`codemap-search`) | `e727d20c6` | Single Rust binary (6 exposed MCP tools): Tree-sitter symbol/caller extraction + Tantivy (BM25 full-text search engine) `search` + embedded `read`, `find`, `grep`, `initial_instructions`, and `analyze` (workspace index and read-activity log analyzer, unused by solvers); optional Jev LLM relevance filter (an opt-in stage that scores returned snippets for task relevance before delivery) disabled |
| `Graphify 0.9.95` | [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify) (PyPI `graphifyy`, `v0.9.95`) | `92741d16` | Python code-graph MCP (7 graph tools); paired with a read-only MCP `read` helper because Graphify's graph tools return entity metadata and locations without raw source bodies |
| `zvec-grep 0.2.2` | [zvec-ai/zvec-grep](https://github.com/zvec-ai/zvec-grep) (npm `@zvec/zvec-grep`, `v0.2.2`) | `b1a9148e` | Node.js local vector/semantic search MCP (`zvec_grep_search`, embedding model `local/potion-code-16m-v2`); no `read` helper attached because `zvec_grep_search` returns bounded source snippets alongside matches |
| `codebase-memory-mcp 0.12.1` | [DeusData/codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) (`v0.12.1`) | `a7158ac2` | Codebase graph, code search, and snippet-retrieval MCP (13 tools) |
| `rg 15.2.0` (baseline) | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) (`15.2.0`) | `e89fff89` | No MCP server attached; the agent relies solely on its built-in file/search or read-only shell tools |
| `CodeGraph 1.6.2` | [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) (npm `@colbymchenry/codegraph`, `v1.6.2`) | `6560052a` | Node.js code-graph exploration MCP (`codegraph_explore`) |
| `Serena 2.1.0` (excluded) | [oraios/serena](https://github.com/oraios/serena) (PyPI `serena-agent`, `v2.1.0`) | `5e4f5cd1` | Go + TypeScript LSP-backed MCP; excluded prior to running because full LSP indexing of Grafana took ~34–70 min per run |

Because `Graphify 0.9.95`'s 7 native graph tools (`query_graph`, `get_neighbors`, `get_node`, `graph_stats`, etc.) only return graph entities and locations without raw source lines, the harness attached a read-only MCP `read` helper (`helper read`) so the agent could inspect files directly through MCP. `zvec-grep 0.2.2` did not receive a `read` helper because `zvec_grep_search` already includes bounded source snippets; when it needed broader file reads, it used the client's built-in `read` tool or read-only shell.

## Agent clients, models, and execution environments

| Component / Environment | Pinned version & details |
| --- | --- |
| Target codebase | [grafana/grafana](https://github.com/grafana/grafana) commit `c0a598f5a49abf2014f67e806075d8ad267c9efb` (`2026-10-26`, `24,115` tracked files, TypeScript + Go; includes commits after the solver's training cutoff) |
| Solver model | `gpt-6.1-sol` (reasoning effort `medium`, `3` independent runs per question per tool/environment) |
| Blind grader model | `gpt-6-astra` (reasoning effort `high`, no tools or repository access; checks each answer against `12` required facts per question) |
| `pi direct` | `pi 1.4.0` (`--provider openai-codex --thinking medium`) + `@buyong/pi-mcp-adapter 0.1.2`. Standard tool-calling mode: calls MCP tools directly turn by turn, alongside read-only built-in `read`, `grep`, `find`, `ls` (no `bash`) confined to the repository |
| `pi codemode` | Same `pi 1.4.0` + `@buyong/pi-mcp-adapter 0.1.2` + `pi`'s built-in `codemode` extension (`@earendil-works/pi-coding-agent 1.4.0`, QuickJS sandbox). Scripted mode: the model writes JavaScript that batches/chains MCP or built-in calls and filters outputs before returning to context (default `10,000`-token script output cap) |
| `Codex code mode` | `Codex CLI 0.164.0` (`codex exec --json`, default `code_mode_only`). Executes JavaScript `exec`/`wait` cells (Codex code-execution and async-wait primitives) that invoke MCP tools and a read-only shell (`exec_command`) confined to the repository, batching multiple tool or shell calls per turn (`10,000`-token default `exec` output cap unless overridden; `codemap-search` instructs `// @exec: {"max_output_tokens": 100000}`) |

### Why environments are never pooled

The three agent environments (`pi direct`, `pi codemode`, `Codex code mode`) are reported side-by-side and never pooled into a combined score because each client differs along three axes:

1. **Built-in fallback tools**: `pi` gives every arm read-only `read`, `grep`, `find`, and `ls` (no `bash`), whereas `Codex code mode` provides a read-only `exec_command` shell callable inside `exec` JavaScript cells.
2. **Output truncation caps**: `pi` built-ins cap outputs by line/byte limits (`read` `2,000` lines / `50 KiB`, `grep` `100` matches, `find` `1,000` entries, `ls` `500` entries), `@buyong/pi-mcp-adapter 0.1.2` defaults to `20 KiB` unless overridden via `_meta["pi/maxOutputBytes"]`, `pi codemode` caps QuickJS script output at `10,000` tokens, and `Codex code mode` defaults `exec` cells to `10,000` tokens unless overridden.
3. **Prompt and token accounting**: `Codex code mode` includes a larger per-turn developer and code-mode preamble (`9,748` first-turn input tokens on `codemap-search` vs `5,118` in `pi direct` and `4,338` in `pi codemode`) that is re-sent and mostly cached every turn, driving higher total cached input tokens.

### Sandbox and isolation rules

Every run operated on an isolated read-only checkout of the pinned Grafana snapshot with external network access disabled (`no subagents`, no source edits). Both clients blocked access to `.git` and pre-built tool index/state folders (`.codemap`, `.codegraph`, `.zvec-grep`, `.codebase-memory`, `graphify-out`, `.pi`, `.codex`).

- **In `pi`**: A pre-execution `tool_call` guard enforced those path boundaries on built-in `read`, `grep`, `find`, and `ls` (both direct and inside `codemode` scripts), while `grep` and `find` binaries ran inside macOS Seatbelt (Apple's OS-level process sandbox).
- **In `Codex CLI`**: `permissions.local_read` denied out-of-workspace reads and blocked folders, while MCP servers ran under local macOS Seatbelt isolation.

## Pre-built Grafana index footprint and build time

Measured once per batch when indexing the pinned `24,115`-file Grafana checkout prior to read-only solver runs. Build times report `timings.index_build_seconds` uniformly across all five indexed tools (covering the full index-preparation step in `pi` and `Codex` setup, including pre/post status or ready checks).

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

## Related pages

- [Metrics](/benchmarks/metrics/) — Exact definitions for every column in the result tables.
- [Results](/benchmarks/results/) — Full numerical tables and per-fact matrix.
- [Reproduction Notes](/benchmarks/reproduction-notes/) — Batch details, reruns, and sensitivity notes.
