# scout MCP Code-Search Benchmark Report

A benchmark of **scout** — a code-search MCP server (zoekt trigram index + Universal Ctags) — measured against vanilla coding-agent built-ins on real-world code-navigation tasks. The full procedure is documented below; the harness scripts, task prompts, and ground-truth artifacts are not yet published (see Reproduction notes for availability status).

- **Tool under test:** scout (zoekt + Universal Ctags), managed binary `v0.0.3`
- **Comparison arms:** `default` (built-in tools only), scout (pure / additive), and a reference LSP-based MCP arm (pure / additive)
- **Agent harnesses:** Claude Code (Sonnet 4.6, primary) and a second independent agent harness (Codex / gpt-5.4-mini, supplementary)
- **Target repos:** vscode `64d8ca8`, kubernetes `4ea9058`
- **Dataset:** 12 tasks, 46 essential anchors (anchor = a required `file:line` edit point), independently verified ground truth
- **Date of record:** 2026-06-04

> The author of this report is also the author of scout. The fairness charter (below) documents the conflict-of-interest controls applied. Where scout does not win, that result is reported as plainly as where it does.

---

## TL;DR

On this set of real-world code-navigation tasks over mid-sized repos (15k–31k files):

1. **scout is statistically on par with built-in tools on quality, but costs more.** In the primary Claude Code harness, neither `scout-pure` nor `scout-add` is distinguishable from `default` on the headline F2 metric (a recall-weighted F-score; paired-bootstrap 95% CI includes 0). scout raises turns, latency, and cost without a measurable quality gain.
2. **scout's speed claim is confirmed at the tool layer, but it does not propagate to agent quality at this repo size.** Warm queries land between sub-millisecond and ~51 ms (p90), and a 30k-file cold index finishes in ~4 s (Track A). But at 15k–31k files, ripgrep is already fast enough, so the indexing advantage does not translate into better agent answers.
3. **The single positive significant result is a provisional cross-agent signal.** In the supplementary Codex harness, `scout-add` significantly beat `default` (ΔF2 +0.075, 95% CI excludes 0). This sits on three main uncontrolled confounds (reps=2, a separate rerun batch for scout, a `default` baseline without ripgrep guaranteed on PATH — Limitations lists further environment caveats) and should be read as a provisional signal, not a settled finding.
4. **The reference LSP arm (serena) underperformed.** On vscode it was significantly worse than `default` in the Claude harness (Δ≈−0.10, CI excludes 0), and it did not finish on kubernetes (gopls workspace-symbol timeout). In this configuration, LSP symbol lookup was a poor fit for tasks that require finding *distributed edit points*. (Caveat: serena ran with its 3 navigation tools only — its broader pattern-search tools were not exposed — and its sample is vscode-only, n=6; see Arms.)
5. **Single runs lie.** At reps=1, `scout-pure` looked like the top arm (0.671); at N=3 it fell to 0.646, below `default`. This empirically motivates the N=3 requirement.

---

## What was benchmarked

### Tool under test

**scout** is an MCP server that exposes code search over two engines:

- **zoekt** — a trigram inverted text index (fast substring/regex search at scale)
- **Universal Ctags 6.1.0** — a symbol index (definitions, declarations)

scout exposes four navigation tools per indexed repo: `search_text`, `find_files`, `lookup_symbol`, `read_file`. The value proposition under test is fast indexing and fast search.

### Arms

The primary (Claude Code) harness ran 5 hard-isolated arms; the supplementary (Codex) harness ran 4.

| Arm | Tools available | Purpose |
|---|---|---|
| `default` | Built-ins only: `Read, Glob, Grep, Bash` (no MCP) | Baseline — vanilla agent, no handicap |
| `scout-pure` | scout 4 tools only (no built-in file/grep/Bash) | scout's standalone ceiling |
| `scout-add` | Built-ins + scout 4 tools | scout's marginal benefit (conservative estimate) |
| `serena-pure` | LSP MCP (3 nav tools) + `Read` (no built-in Grep/Glob/Bash) | LSP standalone ceiling |
| `serena-add` | Built-ins + LSP MCP (3 nav tools) | LSP marginal benefit |

The reference LSP arm (**serena**) exposes three navigation tools — `find_symbol`, `find_referencing_symbols`, `get_symbols_overview` — backed by language servers (tsserver for vscode, gopls for kubernetes). In this run, serena's broader tools (`search_for_pattern`, `find_file`, `list_dir`) were not exposed; the whitelist was scoped to what was actually available.

**Pure vs additive.** Pure mode *forces* the tool (no fallback) to measure the tool's own ceiling. Additive mode adds the tool on top of a full built-in agent; agents often prefer familiar ripgrep and under-use the MCP, making additive a *conservative* estimate of marginal benefit.

> **Asymmetry note.** `scout-pure` runs with no built-in file access at all, while `serena-pure` is allowed `Read`. This handicaps `scout-pure` relative to `serena-pure` and is a known limitation, not a neutral comparison.

### Two measurement tracks

- **Track A — tool latency (direct):** Cold full index, warm query p50/p90, no-change recheck, single-file re-index, and fingerprint walk, measured against the scout binary directly with no agent in the loop.
- **Track B — agent quality & efficiency:** A coding agent performs each navigation task; we score answer quality (F2), and record tokens, cost, tool-calls, turns, and end-to-end latency. Arms run as hard-isolated custom agent types.

---

## Methodology

### Target repos (pinned SHAs)

| Repo | Pinned SHA | Files | Primary lang | Code LOC (tokei 14.0) | Total LOC | Index (shard) |
|---|---|--:|---|--:|--:|--:|
| vscode | `64d8ca886db19780b1762d54c3a0efd5a4de8c13` | 15,610 | TypeScript | 3,530,767 | 4,454,753 | 428.4 MB |
| kubernetes | `4ea9058d21ea30c4ac101a0a510a7772cd4cf2d1` | 30,689 | Go | 5,355,924 | 6,756,985 | 616.7 MB |

Repos were depth-1 pin-cloned (working tree = tracked, no build artifacts). A hard constraint of the benchmark: **zero changes to scout's code or to the target repos**; all benchmark artifacts live in a separate `$BENCH_ROOT` directory.

### Task design (ground-truth v2)

12 tasks = 2 repos × 3 categories × 2 tasks each (difficulty is fixed per category: fix=low, feat=medium, flow=high), totaling **46 essential anchors**.

- **Categories:** `fix` (localized bug fix), `feat` (feature addition / coordination), `flow` (cross-cutting flow understanding).
- **Design rules:** No symbol or file names in the prompt — tasks describe the symptom (bug) or requirement (feature) in real-world language, forcing the agent to search for itself. Tasks are deliberately **non-greppable**: a single grep does not solve them; symptom→code inference and structural understanding are required.
- **Answer type:** every task is an `anchor-set` — the minimal set of `file:line` points you must edit/understand to do the task correctly. An `optionalContext` field lists helpful-but-not-required locations that are *not scored* (which also blunts over-return precision penalties).

| id | repo | cat | difficulty | anchors | scenario (summary) |
|---|---|---|---|--:|---|
| vsc-fix-1 | vscode | fix | low | 2 | search-and-replace "preserve case" option bug |
| vsc-fix-2 | vscode | fix | low | 2 | whitespace handling when joining multiple lines |
| vsc-feat-1 | vscode | feat | medium | 8 | add a setting that changes line-comment behavior |
| vsc-feat-2 | vscode | feat | medium | 5 | change line alphabetical-sort comparison basis |
| vsc-flow-1 | vscode | flow | high | 4 | flow of save-time auto-cleanup actions |
| vsc-flow-2 | vscode | flow | high | 5 | flow of same-word auto-highlight behavior |
| k8s-fix-1 | kubernetes | fix | low | 2 | memory-pressure eviction threshold branch bug |
| k8s-fix-2 | kubernetes | fix | low | 2 | NotReady/Unreachable taint flow bug |
| k8s-feat-1 | kubernetes | feat | medium | 3 | register a kubelet workload source |
| k8s-feat-2 | kubernetes | feat | medium | 3 | add a scheduler resource-scoring method |
| k8s-flow-1 | kubernetes | flow | high | 4 | API admission check / rejection flow |
| k8s-flow-2 | kubernetes | flow | high | 6 | scheduler node-filtering phase flow |

### Ground-truth curation with independent verification

- **Method B (code reading):** Anchors were found by reading the code directly, not by relying on a single ripgrep sweep.
- **R0 · R2 independent verification:** an initial builder (R0) and a separate verifier (R2) each derived the answer set *independently*. A separate model then compared R0 and R2 and curated the consensus anchor set on code-grounded evidence.
- **Oracle independence:** the answer set was built **without ever using serena/LSP**, so every contestant is independent of the ground truth at the tool layer. Critically, ground truth was **not generated from ripgrep alone** either — generating it from a text-search tool would bias the answer set in favor of text-search arms (circular). Agents were also barred from reading prior results or `answer_locations` fields during the run.

### Scoring

- **Headline metric: F2** — recall-weighted harmonic mean (`5PR / (4P + R)`, β=2). The intent is "did it find every essential anchor", with only a weak precision penalty for over-return.
- **Matching tolerance:** `file:line` matched at ±3 lines.
- **Significance test:** paired bootstrap on per-task mean F2 (B=10,000, seed=12345). A contrast is significant when the 95% CI excludes 0.
- **Source of truth:** per-agent workflow transcripts were parsed (not subagent self-reports), mapped via a unique prompt tag `[[RUN:arm|task|run]]`.
- **Scorer parity:** both harnesses share the dataset, ground-truth set, and scorer byte-for-byte (`extractLocations` + `score`, tol ±3 unchanged).
- **Outliers / DNF:** no outlier removal. Run-level failures (empty or unparseable answers) are scored as 0. The one environment-level failure — serena's kubernetes index timeout — was excluded from aggregation and reported separately rather than scored as 0 (see Limitations 8 for why this asymmetry favors serena).
- **Adversarial scoring check:** planned (a separate subagent re-verifying sampled scores against the answer set to catch ±3 boundary and path-normalization bugs), but no execution artifacts were recorded for this run — it is queued as a control for the next measurement, not claimed as a completed check.

### Fairness charter

| Control | Detail |
|---|---|
| Same prompt | Every arm uses the identical neutral prompt; only the available tools differ. |
| No baseline handicap | `default` is full vanilla agent (Read/Glob/Grep/Bash). |
| Hard isolation | Per-arm `agentType` with a hard tool whitelist; prompt-only isolation leaked Bash in testing, so isolation is enforced at the harness level. An unknown agentType throws rather than silently falling back. |
| Transcript audit | Tool isolation is audited from the transcript; out-of-budget calls are flagged/discarded. |
| N=3 repetition | Guards against single-run noise (see the reps=1 reversal below). |
| Paired bootstrap | 95% CI (B=10,000, seed=12345) for each contrast. |
| Honest DNF | Run-level failures (empty answers) score 0. The environment-level serena failure (gopls workspace-symbol timeout on kubernetes) was excluded from aggregation and reported as DNF, not silently hidden — see Limitations 8 for the resulting asymmetry. |
| Tool preference is not a scoring rule | The benchmark does not adopt the tested tool's own preferences (e.g. "exclude indexing time") as scoring rules. |

**Audit result:** across all 144 primary runs, **0 isolation leaks** — no arm called a disallowed MCP, and no pure arm used a built-in. Per-arm: `scout-add` 36/0, `scout-pure` 36/0, `serena-pure` 18/0, `default` 36/0, `serena-add` 18/0.

### Sample sizes

- **Total: 144 primary runs** (48 task-instances × 3 runs) in the Claude harness.
  - `default`, `scout-pure`, `scout-add`: 36 runs/arm (12 tasks × 3).
  - `serena-pure`, `serena-add`: 18 runs/arm (vscode-only, 6 tasks × 3; kubernetes was DNF).
- **Supplementary Codex harness: 24 runs/arm** — reps=2 for `default`, `scout-pure`, `scout-add` (12 tasks × 2); reps=4 for `serena-add` (vscode-only, 6 tasks × 4).
- **Cost envelope (Claude harness only):** $44.06, 64.3M tokens, 2,660 tool-calls across the 144 primary runs. Cost was controlled via a reps=1 scouting pass (48 runs, ~$15) followed by an N=3 increment (+96 runs, ~$29). Codex-harness cost is a derived estimate based on an unconfirmed mini-tier price assumption and is reported only in the Codex efficiency table.

### Measurement environment

- Single machine: 14-core Apple Silicon laptop, macOS 15.7.1.
- scout binary: managed `v0.0.3` (zoekt-index, zoekt-webserver, Universal Ctags 6.1.0).
- Models: Claude Sonnet 4.6 (primary), gpt-5.4-mini (supplementary, run via a separate agent CLI).
- **Track A caveat:** measured under loadavg 7.28 (high load) — absolute milliseconds may be inflated by ~20–30%.

---

## Results

### Track B — quality, Claude Code / Sonnet 4.6 (5-arm, N=3)

Aggregate (per-task mean F2):

| arm | runs | F2 | F2 SD (across tasks) | per-run F2 | run-to-run SD | recall | precision | over-return |
|---|--:|--:|--:|---|--:|--:|--:|--:|
| default | 36 | 0.663 | 0.300 | [0.653, 0.664, 0.672] | 0.009 | 0.787 | 0.468 | 2.89 |
| scout-add | 36 | 0.655 | 0.321 | [0.641, 0.659, 0.664] | 0.012 | 0.791 | 0.460 | 3.70 |
| scout-pure | 36 | 0.646 | 0.305 | [0.671, 0.618, 0.649] | 0.027 | 0.766 | 0.464 | 3.48 |
| serena-add¹ | 18 | 0.589 | 0.365 | [0.604, 0.571, 0.592] | 0.017 | 0.707 | 0.400 | 2.75 |
| serena-pure¹ | 18 | 0.574 | 0.372 | [0.633, 0.572, 0.518] | 0.058 | 0.671 | 0.390 | 2.06 |

¹ serena arms are vscode-only (n=6 tasks); kubernetes was DNF (gopls workspace-symbol timeout).

Efficiency (mean per run):

| arm | tool-calls | turns | tok in | tok out | cost (USD) | latency (s) | E2E p50 / p90 / p95 (s) |
|---|--:|--:|--:|--:|--:|--:|--:|
| default | 17.64 | 21.28 | 417,638 | 2,080 | 0.277 | 68.26 | 60 / 123 / 154 |
| scout-add | 18.28 | 23.58 | 526,135 | 2,135 | 0.347 | 83.04 | 73 / 129 / 135 |
| scout-pure | 21.08 | 27.92 | 500,992 | 2,278 | 0.356 | 98.66 | 89 / 172 / 181 |
| serena-add¹ | 16.39 | 20.44 | 318,348 | 2,018 | 0.233 | 77.40 | 66 / 130 / 132 |
| serena-pure¹ | 17.39 | 23.78 | 346,532 | 1,943 | 0.255 | 110.79 | 101 / 165 / 212 |

Significance — paired bootstrap (B=10,000, seed=12345):

| contrast | obs Δ | 95% CI | n tasks | verdict | P(treat ≤ base) |
|---|--:|---|--:|---|--:|
| scout-pure − default | −0.017 | [−0.077, +0.028] | 12 | not significant (parity) | 0.718 |
| scout-add − default | −0.009 | [−0.038, +0.022] | 12 | not significant (parity) | 0.722 |
| serena-pure − default | −0.113 | [−0.224, −0.009] | 6 | **significant (worse)** | 0.984 |
| serena-add − default | −0.098 | [−0.212, −0.005] | 6 | **significant (worse)** | 0.984 |

> The serena Δ is computed against `default` on the *same 6 vscode tasks*, not against the 12-task `default` aggregate.

Breakdown by repo:

| arm | kubernetes | vscode |
|---|--:|--:|
| default | 0.640 | 0.687 |
| scout-add | 0.614 | 0.695 |
| scout-pure | 0.590 | 0.703 |
| serena-pure | — (DNF) | 0.574 |
| serena-add | — (DNF) | 0.589 |

Breakdown by category:

| arm | feat (med) | fix (low) | flow (high) |
|---|--:|--:|--:|
| default | 0.609 | 0.954 | 0.427 |
| scout-add | 0.591 | 0.968 | 0.405 |
| scout-pure | 0.557 | 0.962 | 0.420 |
| serena-add | 0.460 | 0.934 | 0.373 |
| serena-pure | 0.463 | 0.901 | 0.358 |

Per-task F2 (N=3 mean):

| task | repo | cat | diff | anchors | default | scout-pure | scout-add | serena-pure | serena-add |
|---|---|---|---|--:|--:|--:|--:|--:|--:|
| k8s-feat-1 | kubernetes | feat | med | 3 | 0.874 | 0.588 | 0.833 | — | — |
| k8s-feat-2 | kubernetes | feat | med | 3 | 0.397 | 0.502 | 0.430 | — | — |
| k8s-fix-1 | kubernetes | fix | low | 2 | 1.000 | 1.000 | 1.000 | — | — |
| k8s-fix-2 | kubernetes | fix | low | 2 | 1.000 | 1.000 | 1.000 | — | — |
| k8s-flow-1 | kubernetes | flow | high | 4 | 0.490 | 0.448 | 0.421 | — | — |
| k8s-flow-2 | kubernetes | flow | high | 6 | 0.076 | 0.000 | 0.000 | — | — |
| vsc-feat-1 | vscode | feat | med | 8 | 0.381 | 0.362 | 0.296 | 0.102 | 0.223 |
| vsc-feat-2 | vscode | feat | med | 5 | 0.785 | 0.776 | 0.804 | 0.824 | 0.697 |
| vsc-fix-1 | vscode | fix | low | 2 | 0.939 | 0.968 | 0.943 | 0.970 | 0.984 |
| vsc-fix-2 | vscode | fix | low | 2 | 0.875 | 0.879 | 0.928 | 0.833 | 0.884 |
| vsc-flow-1 | vscode | flow | high | 4 | 0.423 | 0.490 | 0.519 | 0.148 | 0.069 |
| vsc-flow-2 | vscode | flow | high | 5 | 0.719 | 0.743 | 0.681 | 0.569 | 0.677 |

### Track B — quality, supplementary Codex / gpt-5.4-mini (4-arm, reps=2; serena-add reps=4)

> Cross-agent caveat: **absolute numbers across harnesses are not directly comparable.** Only the direction and significance of `arm − default` *within* a harness are valid comparisons. The two harnesses use different models, schedulers, and turn definitions.

Aggregate (per-task mean F2):

| arm | runs | F2 | F2 SD (across tasks) | per-run F2 | run-to-run SD | recall | precision | over-return |
|---|--:|--:|--:|---|--:|--:|--:|--:|
| scout-add | 24 | 0.556 | 0.258 | [0.541, 0.570] | 0.021 | 0.600 | 0.525 | 1.46 |
| default | 24 | 0.481 | 0.244 | [0.450, 0.511] | 0.043 | 0.526 | 0.461 | 1.56 |
| serena-add¹ | 24 | 0.464 | 0.213 | [0.527, 0.558, 0.431, 0.341] | 0.098 | 0.511 | 0.427 | 1.47 |
| scout-pure | 24 | 0.453 | 0.250 | [0.433, 0.473] | 0.029 | 0.491 | 0.441 | 1.50 |

¹ serena-add is vscode-only (n=6 tasks), reps=4. Its run-to-run SD of 0.098 indicates instability.

Efficiency (mean per run):

| arm | tool-calls | turns² | tok in | tok out | cost (USD)³ | latency (s) | E2E p50 / p90 / p95 (s) |
|---|--:|--:|--:|--:|--:|--:|--:|
| default | 25.25 | 7.58 | 829,005 | 25,843 | 0.089 | 275.39 | 288 / 388 / 474 |
| scout-pure | 28.25 | 5.33 | 1,028,876 | 25,273 | 0.092 | 370.48 | 342 / 549 / 614 |
| scout-add | 33.42 | 7.50 | 979,161 | 26,103 | 0.096 | 363.49 | 375 / 484 / 528 |
| serena-add | 37.33 | 6.29 | 1,039,993 | 23,122 | 0.082 | 294.70 | 303 / 484 / 491 |

² Codex "turns" = model-utterance count, which differs in meaning from Claude turns; not directly comparable.
³ gpt-5.4-mini pricing is an unconfirmed mini-tier approximation (in $0.25 / cached-in $0.025 / out $2.00 per MTok).

Significance — paired bootstrap (Codex):

| contrast | obs Δ | 95% CI | n tasks | verdict | P(treat ≤ base) |
|---|--:|---|--:|---|--:|
| scout-pure − default | −0.028 | [−0.099, +0.036] | 12 | not significant (parity) | 0.788 |
| scout-add − default | +0.075 | [+0.004, +0.136] | 12 | **significant (better than default)** | 0.021 |
| serena-add − default | −0.008 | [−0.121, +0.105] | 6 | not significant | 0.534 |

> ⚠️ This single positive significant result is provisional. It rests on three main uncontrolled confounds at once: reps=2 (below the N≥3 we found necessary), a *separate rerun batch* for scout (see integrity note), and an rg-absent `default` baseline — ripgrep was not guaranteed on PATH in the Codex environment, so `default` explored via plain shell, which plausibly weakens the baseline relative to a true rg baseline (a dedicated rg arm with rg guaranteed on PATH is planned; see Planned next measurements). Limitations adds environment-freeze and task-isolation caveats on top. Treat as a signal pending re-confirmation under controlled conditions.

Per-task F2 (Codex):

| task | repo | cat | anchors | default | scout-pure | scout-add | serena-add |
|---|---|---|--:|--:|--:|--:|--:|
| k8s-feat-1 | kubernetes | feat | 3 | 0.705 | 0.598 | 0.788 | — |
| k8s-feat-2 | kubernetes | feat | 3 | 0.466 | 0.435 | 0.456 | — |
| k8s-fix-1 | kubernetes | fix | 2 | 0.556 | 0.556 | 0.556 | — |
| k8s-fix-2 | kubernetes | fix | 2 | 0.778 | 0.778 | 1.000 | — |
| k8s-flow-1 | kubernetes | flow | 4 | 0.289 | 0.322 | 0.426 | — |
| k8s-flow-2 | kubernetes | flow | 6 | 0.139 | 0.074 | 0.216 | — |
| vsc-feat-1 | vscode | feat | 8 | 0.124 | 0.125 | 0.130 | 0.148 |
| vsc-feat-2 | vscode | feat | 5 | 0.435 | 0.508 | 0.615 | 0.519 |
| vsc-fix-1 | vscode | fix | 2 | 0.778 | 0.778 | 0.556 | 0.556 |
| vsc-fix-2 | vscode | fix | 2 | 0.500 | 0.705 | 0.705 | 0.705 |
| vsc-flow-1 | vscode | flow | 4 | 0.239 | 0.119 | 0.357 | 0.265 |
| vsc-flow-2 | vscode | flow | 5 | 0.762 | 0.441 | 0.863 | 0.594 |

> Integrity note: in the Codex harness, scout MCP calls failed at ~100% in the first run (`"user cancelled MCP tool call"`). Only the two scout arms were fixed and rerun; success rates after the fix were `scout-pure` 675/678 (99.6%) and `scout-add` 724/736 (98.4%). Consequently `default`/`serena-add` come from the initial batch and the scout arms from a separate rerun batch. This batch separation can perturb latency/cost (quality F2/recall is unaffected by it) and is the root of the provisional-signal caveat above.

### Track A — tool latency (direct measurement)

Measured against the scout binary with no agent in the loop. Absolute values may be ~20–30% inflated (loadavg 7.28).

| metric | vscode (15,610 files, shard 428.4 MB) | kubernetes (30,689 files, shard 616.7 MB) |
|---|--:|--:|
| cold full index (median) | 2,848 ms | 4,071.4 ms |
| cold index min / max | 2,806.8 / 3,126.2 ms | 3,920.2 / 5,631.7 ms |
| cold first query | 3,510.5 ms | 4,097 ms |
| warm query p50 — common | 18.7 ms | 50.2 ms |
| warm query p90 — common | 19.6 ms | 51.3 ms |
| warm query p50 — rare | 1.2 ms | 0.6 ms |
| warm query p90 — rare | 1.9 ms | 0.8 ms |
| warm query p50 — regex | 35.1 ms | 2.2 ms |
| warm query p90 — regex | 35.5 ms | 4.0 ms |
| warm query p50 — langFilter | 2.0 ms | 0.4 ms |
| warm query p90 — langFilter | 2.2 ms | 0.7 ms |
| warm query overall p50 (n=28) | 10.2 ms | 1.4 ms |
| warm query overall p90 (n=28) | 35.2 ms | 50.3 ms |
| no-change recheck p50 | 334.9 ms | 467.8 ms |
| 1-file touch re-index p50 | 3,417.8 ms | 4,960.4 ms |
| fingerprint walk (median) | 307.7 ms | 519.6 ms |

Raw samples (where recorded):

- vscode cold index: [3126.2, 2846.3, 2863.5, 2848.0, 2806.8] ms · kubernetes cold index: [5631.7, 4195.0, 4071.4, 3936.6, 3920.2] ms
- vscode no-change recheck: [357.8, 334.9, 326.1] ms · kubernetes: [462.2, 475.1, 467.8] ms
- vscode 1-file re-index: [3420.3, 3408.6, 3417.8] ms · kubernetes: [5543.9, 4960.4, 4822.7] ms
- fingerprint walk: vscode median 307.7 ms (range 293–351.7) · kubernetes median 519.6 ms (range 503.7–560.6)

**Read of Track A:** a 30k-file index finishes in ~4 s, and warm-query p50s span 0.4–50.2 ms with p90s at or below 51.3 ms. However, a single-file change triggers a *full* re-index (~3–5 s) — there is no incremental indexing. For this read-only benchmark with a persistent index, the re-index cost does not affect Track B latency or F2, but it is a real caveat for edit-loop workflows.

---

## Analysis & failure modes

### Cross-agent direction agreement

| contrast | Claude | Codex | both harnesses |
|---|---|---|---|
| scout-pure vs default | parity (Δ−0.017, n.s.) | parity (Δ−0.028, n.s.) | both parity — strong agreement |
| scout-add vs default | parity (Δ−0.009, n.s.) | better (Δ+0.075, sig.) | both ≥0 — neutral→positive |
| serena-add vs default | worse (Δ−0.098, sig.) | below (Δ−0.008, n.s.) | both ≤0 — no serena advantage |

`scout-pure` lands at parity with `default` in *both* harnesses — the strongest, most consistent finding: zoekt+ctags search alone matches built-in grep+read on quality, with no quality loss in environments that have no built-ins.

### Why scout-add helped Codex but not Claude (hypothesis)

- **Claude (parity):** Sonnet's built-in grep+read strategy is already strong, so scout's marginal benefit is small — it only adds turns and latency.
- **Codex (better):** gpt-5.4-mini's shell-only default exploration was weaker, and scout's structured search compensated for the gap. This is consistent with the hypothesis that *scout's marginal benefit is larger for agents with weaker built-in exploration*. It remains a hypothesis given the confounds above.

### Cost trade-off

scout consistently raises turns, latency, and cost — the increase comes from MCP round-trips and the agent running more exploration rounds, **not** from the tool being slow (Track A shows the tool itself answering warm queries in sub-millisecond to ~51 ms):

- Claude: `scout-pure` vs `default` — turns +31%, E2E p50 +48% (89 s vs 60 s), cost +29%, with 0 quality gain.
- Codex: `scout-add` vs `default` — tool-calls 25.3→33.4, tok in 829k→979k, E2E p50 288→375 s, cost $0.089→$0.096, for a +15.6% quality gain (provisional).

### Notable failures

**k8s-flow-2 (6 anchors, flow/high) — all Claude arms ≤0.08.** A very hard cross-flow task (kubernetes scheduler node-filtering phase). `default` 0.076, `scout-pure` 0, `scout-add` 0. Codex did somewhat better (`default` 0.139, `scout-add` 0.216).

**vsc-flow-1 serena collapse (4 anchors).** `default` 0.423, `scout-pure` 0.490, `scout-add` 0.519, but `serena-pure` 0.148 and `serena-add` 0.069. A clear illustration that LSP is very weak at locating *distributed edit points*.

**vsc-feat-1 (8 anchors) — all arms struggled.** `default` 0.381, `scout-pure` 0.362, `scout-add` 0.296, `serena-pure` 0.102, `serena-add` 0.223. The highest-anchor-count task was the hardest to fully recall.

**serena kubernetes DNF.** Probe evidence: `get_symbols_overview(pkg/scheduler/util/utils.go)` returned OK (file-scoped document-symbol works), but `find_symbol('NewMainKubelet', unscoped)` raised TimeoutError — an unscoped workspace-symbol query explodes on kubernetes (~17k Go files). `serena-add` could fall back to Grep, but only after wasting the 240 s serena timeout, so it was unified as DNF.

### The reps=1 false signal

Single runs gave a misleading ranking that N=3 corrected:

| arm | reps=1 F2 | N=3 F2 | Δ |
|---|--:|--:|--:|
| default | 0.653 | 0.663 | +0.010 |
| scout-pure | 0.671 (1st) | 0.646 (3rd) | −0.025 |
| scout-add | 0.641 | 0.655 | +0.014 |
| serena-pure | 0.633 | 0.574 | −0.059 |
| serena-add | 0.604 | 0.589 | −0.015 |

At reps=1, `scout-pure` looked like the winner; at N=3 it dropped to third, below `default`. This is the empirical basis for requiring N≥3.

---

## Limitations

State plainly:

1. **Few models.** Two models total (Sonnet 4.6, gpt-5.4-mini). Results can change sign on stronger or different models.
2. **Small samples.** scout n=12 tasks; serena n=6 (vscode). Bootstrap CIs are wide. "Parity" is *not* proof of no effect — only failure to detect one.
3. **Mid-sized repos.** At 15k–31k files, ripgrep is already fast enough. Very large repos (100k+ files), slow/remote filesystems, and grep-absent environments are untested — these are exactly where scout's index could plausibly win, and they are the next validation target.
4. **scout-pure handicap.** `scout-pure` runs with no built-in file access; `serena-pure` is allowed `Read`. The pure-arm comparison is asymmetric.
5. **kubernetes serena DNF.** serena conclusions are vscode-only.
6. **Headline-metric bias.** F2 (β=2) weights recall 4× and only weakly penalizes over-return; F1 / Fβ-sweep / anchor-pooled micro-average should be reported alongside it.
7. **Codex confounds.** reps=2 (below N≥3), a separate scout rerun batch, an rg-absent `default` baseline, unfrozen environment (codex-cli/Node versions, reasoning effort not recorded), and unspecified per-task isolation. The single positive significant result inherits all of these.
8. **DNF scoring asymmetry.** An empty answer scores 0, but the serena kubernetes index failure was excluded from scoring — effectively exempting serena from the hardest repo.
9. **Single machine.** One laptop; absolute Track A values may be ~20–30% inflated (loadavg 7.28). Index-state manifest was not recorded.
10. **Scoring method.** F2 with ±3-line tolerance against a curated anchor set is one defensible scoring choice among several; a different tolerance or metric could shift rankings.

---

## Reproduction notes

Prerequisites: check out the pinned SHAs (`vscode 64d8ca8`, `kubernetes 4ea9058`), pin scout managed `v0.0.3` on PATH, and prepare the v2 ground-truth file. Set `$BENCH_ROOT` to your benchmark working directory.

> **Artifact availability:** the harness scripts (`harness/perf-run.sh`, `harness/collect-codex.mjs`, `harness/score-trackB-multi.mjs`, the generated workflow runners), the 12 task prompts, and the v2 ground-truth file are **not yet published**. Until they are, this section documents the procedure rather than providing a turnkey reproduction; artifacts are available on request and are queued for release alongside this report.

```bash
cd $BENCH_ROOT

# 1) Track A — scout tool latency (cold index / warm query / re-index)
bash harness/perf-run.sh                 # -> results/perf-native.json

# 2) Track B (Claude) — in-session workflow, 5-arm x 12 tasks x N=3
#    Manual step: run the self-contained workflow scripts
#    harness/generated/bench-run-native-sonnet-12t-*.js from inside a
#    Claude Code session -> results/trackB-native-v2-n3.jsonl

# 3) Track B (Codex) — codex exec, 4-arm x 12 tasks x reps=2
#    Toggle scout/serena MCP per arm in the codex config; run the job runner.
#    NOTE: run the two scout arms only after confirming MCP integration works.

# 4) Collect Codex answers
node harness/collect-codex.mjs           # -> results/trackB-codex-n3.jsonl (96 records)

# 5) Score (Claude and Codex separately; identical scoring logic)
node harness/score-trackB-multi.mjs results/trackB-native-v2-n3.jsonl
node harness/score-trackB-multi.mjs results/trackB-codex-n3.jsonl --out=scores-trackB-codex-n3
```

Sanity checks: `wc -l results/trackB-codex-n3.jsonl` = 96; Codex no-result count = 0; scout MCP success rate ≥ 98%.

Record schema: `{env, taskId, repo, category, contestant, run, locations:[{file,line}], tools_used, tokens:{in,out}, turns, costUsd, error}`. Transcript→arm mapping uses the embedded `[[RUN:arm|task|run]]` tag.

### Planned next measurements (fairness guards)

| priority | item | fairness guard |
|---|---|---|
| P0 | add an rg arm (Codex) | rg guaranteed on PATH; arm-neutral prompt |
| P0 | same-batch rerun for all arms + freeze env/version manifest | removes batch confound |
| P0 | split cold/warm latency + pre-warm index + record state | does not hide indexing cost |
| P1 | Codex N≥3 + one session per task | isolation symmetry with Claude |
| P1 | Claude tool-call success / serena breakdown at Codex granularity | reporting symmetry |
| P2 | add a Rust multi-crate monorepo (pre-registered) | language/structure diversity, no cherry-pick |
| P2 | report F1 / Fβ-sweep / anchor-pooled micro-average | metric does not dictate the conclusion |

---

## Conclusion

On this real-world code-navigation benchmark over mid-sized repos, **scout is statistically on par with built-in tools on quality, but more expensive** (turns, latency, and cost all rise with no measurable quality gain). scout's value proposition — fast indexing and search — is confirmed directly in Track A (warm queries sub-millisecond to ~51 ms, ~4 s to index 30k files), but at this repo size ripgrep is already fast enough, so the index advantage does not propagate to agent answer quality. The single positive significant result (Codex `scout-add`, Δ+0.075) is a provisional cross-agent signal weighed down by three main confounds and awaits controlled re-confirmation. Separately, the reference LSP arm (serena), in the nav-tools-only configuration tested here, was a poor fit for tasks that require finding distributed edit points — significantly so on vscode (its only completed repo). The regimes where scout could plausibly beat built-ins — very large repos, incremental edit loops, grep-absent environments — are exactly the ones this benchmark does not yet cover.
