PRE-POST CHECKLIST — this block and the title list below are NOT part of the post. Copy only what's below the POST BODY marker. Do not publish until all items pass:

1. Replace [full report link] with a PUBLIC URL (public repo or gist) hosting the report.
2. Publish or link the harness/ground-truth artifacts the report's reproduction notes reference — or keep the report's "not yet published" wording consistent with reality.
3. Verify the URL exposes no private repo paths or usernames.

Title options (pick one):

- I built a code-search MCP server and benchmarked it against vanilla agent tools. It tied the built-ins on quality — and cost more doing it.
- Honest benchmark: my own code-search MCP (zoekt + ctags) ties built-in grep on quality but costs 25–29% more
- Does a code-search MCP actually beat ripgrep for coding agents? I ran 144 isolated runs to find out (TL;DR: parity)

--- POST BODY (copy from here) ---

I wrote a code-search MCP server (zoekt trigram index + Universal Ctags) and wanted to know if it actually helps a coding agent, so I benchmarked it honestly against the built-in tools. Disclosure up front: I'm the author of the tool, and where it doesn't win I report that just as plainly.

**Headline: on mid-sized repos (15k–31k files), the MCP is statistically on par with built-in grep+read on answer quality, but it raises turns, latency, and cost.** In the primary Claude Code harness (Sonnet 4.6), neither the pure nor the additive arm is distinguishable from the `default` baseline on F2 — a recall-weighted F-score: "did it find every required edit point" (paired-bootstrap 95% CI includes 0).

Track B quality, Claude Code, N=3 (per-task mean F2). `pure` = MCP tools only, no built-ins; `add` = built-ins + MCP; E2E = end-to-end wall-clock per run:

| arm | runs | F2 | cost (USD) | E2E p50 (s) |
|---|--:|--:|--:|--:|
| default | 36 | 0.663 | 0.277 | 60 |
| scout-add | 36 | 0.655 | 0.347 | 73 |
| scout-pure | 36 | 0.646 | 0.356 | 89 |
| serena-add (LSP, vscode-only) | 18 | 0.589 | 0.233 | 66 |
| serena-pure (LSP, vscode-only) | 18 | 0.574 | 0.255 | 101 |

So `scout-pure` costs +29% and adds +31% turns over `default` for zero measurable quality gain. The tool itself is genuinely fast (Track A: warm queries land between sub-millisecond and ~51 ms, a 30k-file cold index finishes in ~4 s), but at this repo size ripgrep is already fast enough, so the speed never propagates to better agent answers.

Two things worth flagging. First, an LSP-based reference arm (serena) was significantly *worse* than `default` on vscode (Δ≈−0.10, CI excludes 0) and did not finish (DNF) on kubernetes (gopls workspace-symbol timeout) — on these tasks and in this configuration, LSP symbol lookup was a poor fit for finding distributed edit points. (Caveat: only serena's 3 nav tools were exposed in this run — its broader pattern-search tools weren't — and the serena sample is vscode-only, n=6, so don't read this as a general verdict on serena.) Second, in a supplementary Codex / gpt-5.4-mini harness, `scout-add` did significantly beat `default` (ΔF2 +0.075, CI excludes 0), hinting the marginal benefit is larger for agents with weaker built-in exploration — but that result sits on three main confounds (reps=2, a separate rerun batch, and a `default` baseline without ripgrep guaranteed on PATH), so I'm calling it provisional, not settled.

Methodology one-liner: 12 non-greppable symptom→code tasks over 2 pinned repos, 46 independently verified anchors (anchor = a required `file:line` edit point; ground truth v2 was built by direct code reading with two independent verification passes — never using LSP, and not derived from ripgrep alone, to avoid tool-circularity bias), hard tool isolation (0 leaks across 144 runs), F2 scored at ±3-line tolerance, N=3 with paired bootstrap.

Limitations: two models only, small samples with wide CIs, and mid-sized repos where grep is already fast — the regimes where an index *should* win (100k+ files, edit loops, grep-absent environments) are exactly the ones I haven't tested yet. "Parity" here means I failed to detect an effect, not proof there's none.

Full report (raw tables, fairness charter, reproduction steps): [full report link]

Happy to take methodology critiques — the next batch adds an rg arm and a same-batch rerun to kill the Codex confounds.
