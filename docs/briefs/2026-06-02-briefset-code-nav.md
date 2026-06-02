# Brief Set: code-nav remaining tools

## Purpose
- Coordinate the work remaining on `@buyong-mcp/code-nav` after `search_text` shipped: secure acquisition of the external binaries, the rest of the C4 pipeline (ctags definitions, read confirmation, filename glob), proactive indexing, and the optional intent router.
- Keep each remaining tool an independently executable unit while making the shared registration surface, the binary trust anchor, and execution ordering explicit.

## Child Briefs
- [ ] `docs/briefs/2026-06-02-feat-code-nav-01-lookup-symbol.md` — lookup_symbol (ctags definitions); exists because symbol-definition lookup is a distinct ctags-backed tool with its own performance model (`-L` file list + `--kinds` + fingerprint cache).
- [ ] `docs/briefs/2026-06-02-feat-code-nav-02-read-file.md` — read_file (Claude Code Read mirror); exists because the read-confirmation step is a distinct provider mirroring exact Read constants.
- [ ] `docs/briefs/2026-06-02-feat-code-nav-03-find-files.md` — find_files (Claude Code Glob mirror); exists because filename glob is a distinct globby-backed capability that adds a new dependency.
- [ ] `docs/briefs/2026-06-02-feat-code-nav-04-index-watcher.md` — proactive index watcher; exists because it is an optional latency optimization independent of the tool surface.
- [ ] `docs/briefs/2026-06-02-feat-code-nav-05-navigate-code.md` — navigate_code intent router; exists because it composes 01/02/03 and is post-v1 work behind a dependency.
- [ ] `docs/briefs/2026-06-02-feat-code-nav-06-fetch-binaries.md` — download + verify zoekt/ctags from a pinned GitHub release; exists because secure binary acquisition is a distinct, security-critical foundation that reworks startup and feeds `ResolvedBinaries`.

## Execution Order
- Wave 1: `2026-06-02-feat-code-nav-01-lookup-symbol.md`, `2026-06-02-feat-code-nav-02-read-file.md`, `2026-06-02-feat-code-nav-03-find-files.md`, `2026-06-02-feat-code-nav-04-index-watcher.md`, `2026-06-02-feat-code-nav-06-fetch-binaries.md` — independent, run in parallel. 06 is foundational (it reworks how binaries resolve) but decoupled via the `ResolvedBinaries` interface, so it does not block the others' implementation.
- Wave 2: `2026-06-02-feat-code-nav-05-navigate-code.md` — starts after 01, 02, and 03 are complete.
- Shared-file landing order within Wave 1: the single coordinating executor applies edits to `tools/index.ts`, `config/defaults.ts`, and the startup files in numeric child order (01 → 02 → 03 → 04 → 06), running `check-types` after each, so "passes after each child lands" holds even though the per-provider implementation work is parallel.

## Dependencies
- `2026-06-02-feat-code-nav-05-navigate-code.md` depends on `2026-06-02-feat-code-nav-01-lookup-symbol.md`, `2026-06-02-feat-code-nav-02-read-file.md`, and `2026-06-02-feat-code-nav-03-find-files.md` because the router composes lookup_symbol, read_file, and search_text.
- `2026-06-02-feat-code-nav-04-index-watcher.md` has no child dependencies — it builds on the existing `IndexLifecycle` — and it blocks nothing: `2026-06-02-feat-code-nav-05-navigate-code.md` does not require the watcher, and the v1 tools work without it (it is a latency optimization only).
- `2026-06-02-feat-code-nav-01-lookup-symbol.md`, `2026-06-02-feat-code-nav-02-read-file.md`, and `2026-06-02-feat-code-nav-03-find-files.md` are mutually independent.
- `2026-06-02-feat-code-nav-06-fetch-binaries.md` has no child dependencies — it reworks startup behind the `ResolvedBinaries` interface that the binary-consuming tools already use, so implementation order is decoupled; only the startup files and `config/defaults.ts` are shared edit surfaces.

## Parallelization
- 01, 02, 03, and 04 can run in parallel — each lives under a separate provider directory.
- 01, 02, 03, and 05 must not edit `apps/code-nav/src/tools/index.ts` or `apps/code-nav/src/config/defaults.ts` simultaneously — serialize edits to those shared files even when the child work runs in parallel.
- Serialization is owned by a **single coordinating executor** that applies the shared-file edits one child at a time, in numeric order (01 → 02 → 03 → 04) — no CI gate or merge queue is assumed. Provider directories are independent; only the two shared files plus `package.json` need ordered edits.

## Conflict Hotspots
- `apps/code-nav/src/tools/index.ts` — every tool registers here; only one child edits it at a time.
- `apps/code-nav/src/config/defaults.ts` — shared constants (06 adds pinned release tag, asset names, and digests here); serialize edits.
- `apps/code-nav/package.json` — only child 03 (find_files) edits it, to add globby.
- `apps/code-nav/src/startup/ensure-required-binaries.ts` — only child 06 reworks it (PATH/go-bin resolution → verified download); no other child edits it.

## Shared Constraints
- snake_case tool names, full-word identifiers (no abbreviations), biome 4-space / double-quote, ESM NodeNext, TypeScript strict.
- All new code lives under `apps/code-nav/src/**`; do not modify the sibling `acp-bridge` app.
- The `textResult` helper in `apps/code-nav/src/tools/index.ts` is currently a private module function — export it (or extract a shared helper) before children reuse it, rather than duplicating it per tool.
- Do not add tests or lint configuration unless explicitly requested.

## Global Acceptance Criteria
- [ ] All four v1 tools (`search_text` plus 01/02/03) are registered and appear in an MCP `tools/list` response.
- [ ] Whole-repo `check-types` and `build` pass after each child lands.
- [ ] The `search_text` and `lookup_symbol` descriptions cross-reference each other with no dangling "symbol-definition tool" reference.
- [ ] An MCP stdio handshake lists the new tools and each returns results on a smoke query.
- [ ] zoekt and ctags are obtained only via verified download against an MCP-pinned digest; a tampered or unverified binary never executes (fail closed), and the cached binary is re-verified on every startup.

## Open Questions
- None — all set-level decisions are resolved: decomposition, find_files backend = globby, watcher type = feat, repo-root location (Stage 4), and shared-file serialization = a single coordinating executor applying edits in numeric child order (Execution Order / Parallelization). Per-child residuals live in each child's Open Questions.
