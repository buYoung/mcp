---
title: Detailed Benchmark Results
description: Per-environment token distributions, pairwise p-values, per-question and per-fact rubric tables, and subtool call breakdowns.
---

For test setup, tool versions, and summary charts, see [Benchmarks Summary](/benchmarks/).

## Column guide

- **Fulfilled (`Core` + `Extended`)**:
  - Counts facts out of `12` per answer (`7` `Core` architecture hops + `5` `Extended` branch/config facts) proven by both explanation and cited source lines.
  - `Factually correct` also includes `Correct, weak citation`.
  - `All-core (7/7)` counts answers fulfilling all `7` `Core` facts.
- **Tokens (`k` = `1,000` tokens)**:
  - `Total tokens` includes cached input re-sent across turns.
  - `Uncached total` counts only cache-miss input plus output.
- **Pairwise tests & citations**:
  - Pairwise `p`-values use a question-stratified permutation test (`p < 0.05`; `One-sided p` tests `A > B`).
  - `Cited test/gen` counts answers citing a test or generated file at least once.

## Results by Environment

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
| `codemap-search 0.14.2` | 946k | 967k | 574k–1,255k | 938k | 848k (90.3%) | 91k | 7.2k | 98k | 18.4 (13–25) |
| `Graphify 0.9.95` | 1,008k | 780k | 540k–2,506k | 1,002k | 914k (91.3%) | 88k | 6.0k | 93k | 25.7 (18–45) |
| `zvec-grep 0.2.2` | 784k | 669k | 342k–1,349k | 778k | 707k (90.9%) | 71k | 6.3k | 77k | 23.3 (14–38) |
| `codebase-memory-mcp 0.12.1` | 1,331k | 1,135k | 824k–2,465k | 1,322k | 1,242k (93.9%) | 80k | 9.1k | 89k | 34.6 (23–57) |
| `rg 15.2.0` | 555k | 411k | 321k–1,337k | 549k | 487k (88.7%) | 62k | 5.5k | 68k | 20.9 (14–35) |
| `CodeGraph 1.6.2` | 1,727k | 1,610k | 741k–2,655k | 1,723k | 1,595k (92.6%) | 127k | 4.4k | 132k | 25.4 (16–33) |

| Pairwise comparison | Fulfilled diff / ans | Two-sided `p` | One-sided `p` | Total token ratio | Uncached total ratio |
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
| `codemap-search 0.14.2` | 829k | 761k | 599k–1,361k | 822k | 728k (88.5%) | 95k | 6.5k | 101k | 16.1 (14–24) |
| `zvec-grep 0.2.2` | 608k | 679k | 420k–800k | 602k | 528k (87.8%) | 73k | 5.9k | 79k | 15.9 (13–19) |
| `Graphify 0.9.95` | 746k | 730k | 388k–1,307k | 740k | 653k (88.2%) | 87k | 5.9k | 93k | 17.1 (13–24) |
| `rg 15.2.0` | 505k | 494k | 218k–810k | 499k | 429k (85.9%) | 70k | 5.4k | 76k | 13.9 (11–18) |
| `CodeGraph 1.6.2` | 1,724k | 1,477k | 757k–3,148k | 1,719k | 1,582k (92.0%) | 137k | 4.5k | 142k | 22.6 (15–32) |
| `codebase-memory-mcp 0.12.1` | 1,190k | 1,186k | 774k–1,608k | 1,182k | 1,077k (91.1%) | 105k | 7.7k | 113k | 22.6 (18–26) |

| Pairwise comparison | Fulfilled diff / ans | Two-sided `p` | One-sided `p` | Total token ratio | Uncached total ratio |
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
| `codemap-search 0.14.2` | 1,333k | 1,294k | 877k–1,857k | 1,324k | 1,200k (90.6%) | 124k | 8.5k | 133k | 18.9 (15–23) |
| `Graphify 0.9.95` | 1,183k | 995k | 767k–1,622k | 1,176k | 1,075k (91.4%) | 101k | 7.0k | 108k | 19.9 (16–26) |
| `rg 15.2.0` | 866k | 672k | 580k–1,658k | 859k | 761k (88.6%) | 98k | 6.6k | 104k | 16.4 (13–25) |
| `zvec-grep 0.2.2` | 869k | 895k | 555k–1,124k | 862k | 774k (89.7%) | 89k | 6.9k | 96k | 18.0 (14–21) |
| `CodeGraph 1.6.2` | 3,309k | 3,220k | 1,651k–5,496k | 3,302k | 3,115k (94.3%) | 187k | 6.5k | 194k | 31.3 (21–43) |
| `codebase-memory-mcp 0.12.1` | 1,827k | 1,687k | 1,266k–3,294k | 1,818k | 1,684k (92.6%) | 135k | 9.0k | 144k | 29.9 (25–40) |

| Pairwise comparison | Fulfilled diff / ans | Two-sided `p` | One-sided `p` | Total token ratio | Uncached total ratio |
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

## Results by Question and Repetition

Each tool and environment ran `3` times per question (`36` facts total).

### `alertmanager-config-autosync`

Traces how the Kubernetes-style API connects to automatic syncing of external Alertmanager configurations.

| Environment | Tool | Fulfilled | Core | Extended | Factually correct | Run 1 · Run 2 · Run 3 (Core+Ext) | Mean total tokens | Mean uncached total | Mean calls |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: |
| `pi direct` | `codemap-search 0.14.2` | 30/36 | 18/21 | 12/15 | 31 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 930k | 94k | 34.7 |
|  | `Graphify 0.9.95` | 30/36 | 18/21 | 12/15 | 31 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 874k | 98k | 57.3 |
|  | `zvec-grep 0.2.2` | 30/36 | 18/21 | 12/15 | 30 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 678k | 66k | 38.0 |
|  | `codebase-memory-mcp 0.12.1` | 30/36 | 18/21 | 12/15 | 31 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 987k | 88k | 71.7 |
|  | `rg 15.2.0` | 28/36 | 17/21 | 11/15 | 31 | 8 (5+3) · 10 (6+4) · 10 (6+4) | 460k | 60k | 55.7 |
|  | `CodeGraph 1.6.2` | 24/36 | 14/21 | 10/15 | 25 | 8 (5+3) · 7 (4+3) · 9 (5+4) | 1,192k | 111k | 19.3 |
| `pi codemode` | `codemap-search 0.14.2` | 30/36 | 18/21 | 12/15 | 32 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 815k | 109k | 34.3 |
|  | `zvec-grep 0.2.2` | 29/36 | 17/21 | 12/15 | 30 | 10 (6+4) · 10 (6+4) · 9 (5+4) | 508k | 76k | 42.7 |
|  | `Graphify 0.9.95` | 28/36 | 16/21 | 12/15 | 33 | 10 (6+4) · 9 (5+4) · 9 (5+4) | 699k | 95k | 59.3 |
|  | `rg 15.2.0` | 30/36 | 18/21 | 12/15 | 33 | 10 (6+4) · 10 (6+4) · 10 (6+4) | 500k | 81k | 64.7 |
|  | `CodeGraph 1.6.2` | 28/36 | 16/21 | 12/15 | 29 | 10 (6+4) · 9 (5+4) · 9 (5+4) | 1,061k | 117k | 19.7 |
|  | `codebase-memory-mcp 0.12.1` | 29/36 | 18/21 | 11/15 | 29 | 9 (6+3) · 10 (6+4) · 10 (6+4) | 1,142k | 110k | 102.3 |
| `Codex code mode` | `codemap-search 0.14.2` | 29/36 | 17/21 | 12/15 | 30 | 9 (5+4) · 10 (6+4) · 10 (6+4) | 1,306k | 133k | 43.0 |
|  | `Graphify 0.9.95` | 29/36 | 17/21 | 12/15 | 30 | 10 (6+4) · 9 (5+4) · 10 (6+4) | 1,149k | 106k | 69.3 |
|  | `rg 15.2.0` | 29/36 | 17/21 | 12/15 | 32 | 9 (5+4) · 10 (6+4) · 10 (6+4) | 891k | 102k | 20.3 |
|  | `zvec-grep 0.2.2` | 29/36 | 17/21 | 12/15 | 32 | 10 (6+4) · 9 (5+4) · 10 (6+4) | 731k | 90k | 18.7 |
|  | `CodeGraph 1.6.2` | 29/36 | 17/21 | 12/15 | 31 | 10 (6+4) · 9 (5+4) · 10 (6+4) | 3,455k | 186k | 32.7 |
|  | `codebase-memory-mcp 0.12.1` | 29/36 | 17/21 | 12/15 | 30 | 10 (6+4) · 9 (5+4) · 10 (6+4) | 1,493k | 110k | 114.3 |

### `git-sync-job-dispatch`

Traces how Git repository sync jobs are dispatched across the message bus to the executing job handler.

| Environment | Tool | Fulfilled | Core | Extended | Factually correct | Run 1 · Run 2 · Run 3 (Core+Ext) | Mean total tokens | Mean uncached total | Mean calls |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: |
| `pi direct` | `codemap-search 0.14.2` | 33/36 | 20/21 | 13/15 | 34 | 12 (7+5) · 11 (7+4) · 10 (6+4) | 1,027k | 107k | 52.3 |
|  | `Graphify 0.9.95` | 32/36 | 19/21 | 13/15 | 34 | 12 (7+5) · 11 (7+4) · 9 (5+4) | 1,553k | 100k | 59.3 |
|  | `zvec-grep 0.2.2` | 33/36 | 21/21 | 12/15 | 33 | 12 (7+5) · 10 (7+3) · 11 (7+4) | 1,138k | 101k | 56.0 |
|  | `codebase-memory-mcp 0.12.1` | 30/36 | 21/21 | 9/15 | 30 | 9 (7+2) · 10 (7+3) · 11 (7+4) | 2,075k | 94k | 86.7 |
|  | `rg 15.2.0` | 32/36 | 21/21 | 11/15 | 32 | 10 (7+3) · 11 (7+4) · 11 (7+4) | 826k | 88k | 74.7 |
|  | `CodeGraph 1.6.2` | 30/36 | 20/21 | 10/15 | 30 | 10 (7+3) · 11 (7+4) · 9 (6+3) | 2,332k | 159k | 29.0 |
| `pi codemode` | `codemap-search 0.14.2` | 35/36 | 21/21 | 14/15 | 35 | 11 (7+4) · 12 (7+5) · 12 (7+5) | 982k | 107k | 45.7 |
|  | `zvec-grep 0.2.2` | 32/36 | 21/21 | 11/15 | 32 | 9 (7+2) · 11 (7+4) · 12 (7+5) | 643k | 79k | 63.3 |
|  | `Graphify 0.9.95` | 32/36 | 20/21 | 12/15 | 34 | 10 (7+3) · 10 (6+4) · 12 (7+5) | 1,064k | 115k | 83.7 |
|  | `rg 15.2.0` | 32/36 | 19/21 | 13/15 | 34 | 11 (7+4) · 10 (6+4) · 11 (6+5) | 705k | 86k | 73.0 |
|  | `CodeGraph 1.6.2` | 35/36 | 21/21 | 14/15 | 35 | 12 (7+5) · 11 (7+4) · 12 (7+5) | 2,722k | 179k | 30.3 |
|  | `codebase-memory-mcp 0.12.1` | 29/36 | 21/21 | 8/15 | 31 | 10 (7+3) · 9 (7+2) · 10 (7+3) | 1,427k | 126k | 104.3 |
| `Codex code mode` | `codemap-search 0.14.2` | 36/36 | 21/21 | 15/15 | 36 | 12 (7+5) · 12 (7+5) · 12 (7+5) | 1,560k | 144k | 56.0 |
|  | `Graphify 0.9.95` | 33/36 | 21/21 | 12/15 | 33 | 11 (7+4) · 10 (7+3) · 12 (7+5) | 1,560k | 125k | 81.7 |
|  | `rg 15.2.0` | 29/36 | 21/21 | 8/15 | 29 | 10 (7+3) · 9 (7+2) · 10 (7+3) | 1,097k | 109k | 23.7 |
|  | `zvec-grep 0.2.2` | 31/36 | 21/21 | 10/15 | 31 | 11 (7+4) · 10 (7+3) · 10 (7+3) | 1,047k | 107k | 22.0 |
|  | `CodeGraph 1.6.2` | 31/36 | 21/21 | 10/15 | 31 | 11 (7+4) · 9 (7+2) · 11 (7+4) | 4,158k | 237k | 41.7 |
|  | `codebase-memory-mcp 0.12.1` | 29/36 | 20/21 | 9/15 | 30 | 10 (7+3) · 9 (6+3) · 10 (7+3) | 2,437k | 149k | 112.0 |

### `scope-annotation-filter`

Traces how dashboard scope filters connect to annotation storage and querying.

| Environment | Tool | Fulfilled | Core | Extended | Factually correct | Run 1 · Run 2 · Run 3 (Core+Ext) | Mean total tokens | Mean uncached total | Mean calls |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: |
| `pi direct` | `codemap-search 0.14.2` | 33/36 | 21/21 | 12/15 | 33 | 11 (7+4) · 11 (7+4) · 11 (7+4) | 880k | 94k | 48.3 |
|  | `Graphify 0.9.95` | 31/36 | 19/21 | 12/15 | 31 | 10 (6+4) · 11 (7+4) · 10 (6+4) | 596k | 82k | 50.7 |
|  | `zvec-grep 0.2.2` | 26/36 | 16/21 | 10/15 | 27 | 8 (5+3) · 10 (6+4) · 8 (5+3) | 537k | 63k | 42.7 |
|  | `codebase-memory-mcp 0.12.1` | 27/36 | 15/21 | 12/15 | 27 | 9 (5+4) · 9 (5+4) · 9 (5+4) | 932k | 86k | 63.3 |
|  | `rg 15.2.0` | 26/36 | 15/21 | 11/15 | 27 | 8 (5+3) · 9 (5+4) · 9 (5+4) | 377k | 55k | 60.7 |
|  | `CodeGraph 1.6.2` | 24/36 | 16/21 | 8/15 | 25 | 8 (6+2) · 8 (5+3) · 8 (5+3) | 1,658k | 125k | 25.7 |
| `pi codemode` | `codemap-search 0.14.2` | 32/36 | 21/21 | 11/15 | 33 | 11 (7+4) · 10 (7+3) · 11 (7+4) | 690k | 88k | 37.7 |
|  | `zvec-grep 0.2.2` | 10/12 | 6/7 | 4/5 | 10 | — · — · 10 (6+4) | 800k | 89k | 71.0 |
|  | `Graphify 0.9.95` | 28/36 | 18/21 | 10/15 | 29 | 9 (6+3) · 9 (6+3) · 10 (6+4) | 474k | 69k | 55.0 |
|  | `rg 15.2.0` | 25/36 | 16/21 | 9/15 | 26 | 10 (6+4) · 8 (5+3) · 7 (5+2) | 310k | 60k | 62.0 |
|  | `CodeGraph 1.6.2` | 22/36 | 14/21 | 8/15 | 23 | 8 (5+3) · 7 (5+2) · 7 (4+3) | 1,389k | 130k | 29.0 |
|  | `codebase-memory-mcp 0.12.1` | 26/36 | 16/21 | 10/15 | 27 | 9 (5+4) · 8 (5+3) · 9 (6+3) | 999k | 103k | 75.7 |
| `Codex code mode` | `codemap-search 0.14.2` | 33/36 | 21/21 | 12/15 | 33 | 11 (7+4) · 11 (7+4) · 11 (7+4) | 1,133k | 121k | 52.3 |
|  | `Graphify 0.9.95` | 32/36 | 20/21 | 12/15 | 32 | 10 (6+4) · 11 (7+4) · 11 (7+4) | 840k | 94k | 60.7 |
|  | `rg 15.2.0` | 31/36 | 20/21 | 11/15 | 32 | 10 (6+4) · 11 (7+4) · 10 (7+3) | 609k | 102k | 17.0 |
|  | `zvec-grep 0.2.2` | 27/36 | 17/21 | 10/15 | 28 | 8 (5+3) · 10 (7+3) · 9 (5+4) | 830k | 91k | 22.0 |
|  | `CodeGraph 1.6.2` | 26/36 | 16/21 | 10/15 | 27 | 8 (5+3) · 10 (6+4) · 8 (5+3) | 2,314k | 158k | 34.7 |
|  | `codebase-memory-mcp 0.12.1` | 27/36 | 16/21 | 11/15 | 28 | 10 (6+4) · 8 (5+3) · 9 (5+4) | 1,552k | 172k | 83.0 |

### Per-fact fulfillment (`36` rubric items)

Each cell shows fulfilled runs for `pi direct · pi codemode · Codex code mode` (`sum/9` total). For `zvec-grep 0.2.2` on `B1–BE5`, `pi codemode` completed `1` run (`1/1` or `0/1`, `sum/7` total).

#### `alertmanager-config-autosync` (`A1–AE5`)

| Fact ID | Level | `codemap-search 0.14.2` | `Graphify 0.9.95` | `zvec-grep 0.2.2` | `codebase-memory-mcp 0.12.1` | `rg 15.2.0` | `CodeGraph 1.6.2` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `A1` | Core | 3·3·2 (8/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·2 (8/9) | 2·3·3 (8/9) | 0·1·2 (3/9) |
| `A2` | Core | 3·3·3 (9/9) | 3·1·2 (6/9) | 3·2·2 (7/9) | 3·3·3 (9/9) | 3·3·2 (8/9) | 2·3·3 (8/9) |
| `A3` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `A4` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `A5` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `A6` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `A7` | Core | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) |
| `AE1` | Extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `AE2` | Extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·2·3 (8/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `AE3` | Extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 2·3·3 (8/9) | 1·3·3 (7/9) |
| `AE4` | Extended | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) |
| `AE5` | Extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |

#### `git-sync-job-dispatch` (`G1–GE5`)

| Fact ID | Level | `codemap-search 0.14.2` | `Graphify 0.9.95` | `zvec-grep 0.2.2` | `codebase-memory-mcp 0.12.1` | `rg 15.2.0` | `CodeGraph 1.6.2` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `G1` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `G2` | Core | 3·3·3 (9/9) | 2·3·3 (8/9) | 3·3·3 (9/9) | 3·3·2 (8/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `G3` | Core | 2·3·3 (8/9) | 3·2·3 (8/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `G4` | Core | 3·3·3 (9/9) | 2·3·3 (8/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·2·3 (8/9) | 2·3·3 (8/9) |
| `G5` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `G6` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·2·3 (8/9) | 3·3·3 (9/9) |
| `G7` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `GE1` | Extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `GE2` | Extended | 3·3·3 (9/9) | 2·1·3 (6/9) | 2·1·2 (5/9) | 2·1·2 (5/9) | 0·2·1 (3/9) | 3·2·2 (7/9) |
| `GE3` | Extended | 1·3·3 (7/9) | 2·2·1 (5/9) | 1·2·0 (3/9) | 1·1·2 (4/9) | 3·2·1 (6/9) | 0·3·1 (4/9) |
| `GE4` | Extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) | 0·0·0 (0/9) | 3·3·3 (9/9) | 2·3·2 (7/9) |
| `GE5` | Extended | 3·2·3 (8/9) | 3·3·2 (8/9) | 3·2·2 (7/9) | 3·3·2 (8/9) | 2·3·0 (5/9) | 2·3·2 (7/9) |

#### `scope-annotation-filter` (`B1–BE5`)

| Fact ID | Level | `codemap-search 0.14.2` | `Graphify 0.9.95` | `zvec-grep 0.2.2` | `codebase-memory-mcp 0.12.1` | `rg 15.2.0` | `CodeGraph 1.6.2` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `B1` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 2·3·3 (8/9) |
| `B2` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `B3` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 1·1/1·1 (3/7) | 0·1·0 (1/9) | 0·0·3 (3/9) | 2·0·1 (3/9) |
| `B4` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `B5` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `B6` | Core | 3·3·3 (9/9) | 1·0·2 (3/9) | 0·0/1·1 (1/7) | 0·0·1 (1/9) | 0·1·2 (3/9) | 0·0·0 (0/9) |
| `B7` | Core | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·2·3 (8/9) |
| `BE1` | Extended | 3·2·3 (8/9) | 3·2·3 (8/9) | 2·1/1·2 (5/7) | 3·2·2 (7/9) | 2·2·2 (6/9) | 0·2·2 (4/9) |
| `BE2` | Extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `BE3` | Extended | 3·3·3 (9/9) | 3·2·3 (8/9) | 2·1/1·2 (5/7) | 3·2·3 (8/9) | 3·1·3 (7/9) | 2·0·2 (4/9) |
| `BE4` | Extended | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·1/1·3 (7/7) | 3·3·3 (9/9) | 3·3·3 (9/9) | 3·3·3 (9/9) |
| `BE5` | Extended | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0/1·0 (0/7) | 0·0·0 (0/9) | 0·0·0 (0/9) | 0·0·0 (0/9) |

## Tool Calls, Citations, and Output Caps

- **Call outcomes**:
  - `Rejected` counts calls blocked before execution by the read-only path sandbox; `Errors` counts failed calls after dispatch.
  - `Built-in / fallback calls` counts calls per answer outside the MCP server (`0.0` = MCP tools only).
- **Citations & output caps**:
  - `Range citations` counts multi-line source citations; `Single-line` counts single-line citations.
  - `Built-in/CLI cap hits`, `Script truncations`, and `Adapter truncations` count outputs clipped at each client layer.

### Call outcomes and per-subtool calls (per answer)

| Environment | Tool | Total calls / ans | Dispatched / ans | Rejected (sum) | Errors (sum) | Built-in / fallback calls / ans | Per-subtool calls / ans |
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
