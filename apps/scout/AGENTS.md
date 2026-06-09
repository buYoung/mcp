# AGENTS.md

## 1. Overview

`@buyong-mcp/scout` is an MCP stdio server that exposes a zoekt + Universal Ctags code-navigation pipeline as read-only search/read primitives for any coding agent. v1 ships `search_text` (zoekt); `lookup_symbol`, `read_file`, and `find_files` are designed but not yet implemented (see `DESIGN.md` §10).

## 2. Folder Structure

- `DESIGN.md`: authoritative design doc — decisions, tool specs, index lifecycle, and implementation status. Align changes with it.
- `src/index.ts`: entry (`scout` bin → `dist/index.js`) — loads config via `loadScoutConfig(cwd)`, unions the repo `.gitignore` directory names into the excluded set (when `use_gitignore`), resolves required binaries, boots in **degraded mode** when any are missing (no `process.exit`), lazily builds the `McpServer`-backed provider once binaries are present (passing the resolved `config` through), wires the `install_binaries` orchestration (download → re-resolve → build provider, coalesced), and installs shutdown hooks (SIGINT/SIGTERM/exit plus stdin `end`/`close`) so the zoekt-webserver child is always reaped.
- `src/config/`: configuration loading/resolution + tunable constants.
    - `scout-config.ts`: `loadScoutConfig(repositoryRoot)` — the TOML config loader/resolver. Ensures `~/.scout/` and writes a commented-out global template (flag `"wx"`, repo-side config is opt-in/read-only), reads + parses repo and global `config.toml` with `smol-toml`, normalizes/validates each layer (unknown tables/keys and type mismatches warn to stderr and are dropped — never throws), then merges per-key with `repo > global > default` precedence into `ResolvedScoutConfig`. The whole load is wrapped in try/catch so any failure falls back to all-defaults.
    - `gitignore-excludes.ts`: `readGitignoreDirectoryNames(repositoryRoot)` — extracts directory-name-level entries from `<repo>/.gitignore` (skips comments, negations `!`, slash-bearing paths, and glob metacharacters `* ? [ ]`; strips a trailing `/`). Returns `[]` on any read error (never throws); the caller unions this into the excluded set.
    - `defaults.ts`: server identity, binary names/release tag/URL, excluded directories, timeouts/limits, output modes, and the `.scout`/`config.toml` names plus `DEFAULT_*` config fallbacks — the single home for tunable constants.
- `src/startup/`: pre-flight binary checks + managed binary acquisition.
    - `ensure-required-binaries.ts`: `resolveBinaries()` reports `ready`/`missing` **without exiting** (degraded boot, no silent fallback — `search_text` explicitly reports the miss); `prependManagedBinToPath()` puts the managed bin dir on child-process PATH (zoekt-index calls ctags internally); `buildInstallationGuidance()` is the shared Korean guidance (stderr, degraded `search_text`, install failures); ctags resolves under either `ctags` or `universal-ctags`.
    - `binary-availability.ts`: executable resolution over PATH plus Go install dirs (`$GOBIN`, `$GOPATH/bin`, `~/go/bin`) and the managed bin dir.
    - `binary-release.ts`: maps `process.platform`/`arch` → release asset (`zoekt-ctags-<plat>.{tar.gz,zip}`). Covers all 6 platforms shipped since v0.0.3 (linux/macos/windows × amd64/arm64); unmapped combos fall back gracefully on a 404.
    - `managed-bin-storage.ts`: managed (downloaded) bin dir — always `~/.scout/bin/<tag>` (`os.homedir()`-based, fixed, no config/env override) so the installer's `rm -rf` can only ever target a dir it owns.
    - `binary-installer.ts`: `installManagedBinaries()` — streams the pinned-tag asset to a **staging dir** (armed abort timeout across the body, size-capped, SHA-256 hashed incrementally), verifies against the downloaded `.sha256`, extracts via system `tar --strip-components=1` (Windows zip via absolute `System32\tar.exe`), renames `universal-ctags`→`ctags`, chmods, then **atomically swaps** staging into place. Reports an outcome object (never throws); failures leave the existing install untouched.
- `src/tools/`: the MCP tool surface.
    - `index.ts`: `registerTools(server: McpServer, deps)` — registers `install_binaries` and `search_text` via high-level `server.registerTool(name, config, cb)` with a **zod raw shape** `inputSchema` (the SDK validates args and auto-derives the ListTools JSON Schema; no hand-written `readArguments`). Maps the validated `snake_case` args → the provider's `camelCase` input, with `resolveContextLines` collapsing `-A/-B/-C` to the max, and wraps output in the `textResult` envelope.
- `src/providers/text-search/`: the zoekt-backed `search_text` pipeline.
    - `text-search-provider.ts`: orchestrates index-fresh → webserver-warm → query → render.
    - `index-lifecycle.ts`: working-tree fingerprint, full re-index, build coalescing, no-change skip, stale-shard cleanup.
    - `zoekt-webserver-lifecycle.ts`: lazy child start on a loopback random port, health polling, restart-once on crash.
    - `zoekt-query-builder.ts` / `zoekt-search-client.ts` / `zoekt-result-renderer.ts`: arg→zoekt-query mapping, HTTP JSON query, Grep-style output rendering.
    - `index-storage.ts`: in-repo index directory, fixed at `<repo>/.scout/zoekt/` (`shards/` + `meta.json`); the old global cache + repo-path SHA hash dir is gone.
    - `http-get.ts`: minimal HTTP GET helper for webserver queries.
- `src/security/path-guard.ts`: path normalization (`expandPath`) and repo-root containment (`resolveRelativePathWithinRoot`).

## 3. Core Behaviors & Patterns

- **Strict provider boundary**: only `search_text` depends on the zoekt index + webserver. The planned read/glob primitives are filesystem-direct and must keep working even before an index exists or if the webserver is down (`DESIGN.md` §3).
- **Degraded boot + on-demand install, no silent fallback**: `resolveBinaries` runs before the transport connects but never exits. If binaries are missing or incompatible (e.g. a non-Universal `ctags`, detected via `ctags --version`), the server still boots: `search_text` returns install guidance instead of wrong results, and the agent can call the `install_binaries` tool to fetch the pinned-tag pre-built binaries (SHA-256 verified) from `buYoung/zoetk-ctags-release`. This is the chosen trigger model (stdio servers cannot interactively prompt); startup does **not** auto-download. The old hard-exit (`process.exit(1)`) is gone — "no silent fallback" now means the miss is reported explicitly, not that the process dies.
- **Config precedence (per-key `repo > global > default`)**: `loadScoutConfig` reads `<repo>/.scout/config.toml` and `~/.scout/config.toml`, and for each key takes the highest-priority layer that **declared** a value (arrays are per-key replace, not append), falling back to the `defaults.ts` constant. Paths (in-repo index, `~/.scout/bin/<tag>`, the `.scout`/`config.toml` names) and release/server identity are not configurable; env is not a config channel (only OS execution-model vars like `PATH`/`GOBIN`/`GOPATH`/`PATHEXT` stay, for binary discovery).
- **never-exit config parsing**: malformed/broken config never crashes the server. A missing file → empty layer; a TOML parse failure, unknown table/key, or type mismatch on a known key → a Korean stderr warning and that value is dropped (the offending key alone falls through to default), and the whole load is try/catch-wrapped to an all-defaults result. This is deliberately **unlike the sibling app's throw-on-bad-config** — scout favors degraded-but-running.
- **`.gitignore` directory-name union**: when `use_gitignore` is on, `index.ts` reads the repo `.gitignore` (directory-name-level entries only — see `gitignore-excludes.ts`) and **unions** those names into `config.index.excludedDirectories` (a separate source, deduped — not a replace), passing the merged set to the provider so the index/lifecycle stay synchronous.
- **Index freshness as a transparent optimization**: `zoekt-index` has no incremental mode, so `IndexLifecycle` re-indexes the whole working tree — but a cheap fingerprint (file count + max mtime, throttled by `STALENESS_CHECK_TTL_MS`) skips unchanged trees, and a single `buildPromise` coalesces concurrent/duplicate builds. The shard directory is cleared before each rebuild so a shrunk corpus never leaves stale shards.
- **Webserver lifecycle & crash recovery**: the webserver starts lazily, binds a loopback random port, is health-polled until ready, and is kept warm across queries. A `WebserverUnreachableError` triggers exactly one restart-and-retry (`markUnhealthy` → `ensureRunning`); a new build generation invalidates a stale warm server.
- **Deterministic shutdown**: `index.ts` installs an idempotent `shutdown` on process signals and, critically, on stdin `end`/`close` — `StdioServerTransport` does not fire `onclose` on EOF, so listening for stdin close ourselves prevents an orphaned webserver child.
- **Path containment**: every path input is normalized via `expandPath` (trim, `~` expansion, cwd-relative resolve) and asserted within the repo root; the relative prefix is emitted in POSIX form for zoekt `file:` filters.
- **Claude Code parity**: tool semantics and output formatting (output modes, `DEFAULT_HEAD_LIMIT` = 250, truncation footers, symmetric `-A/-B/-C` collapse) faithfully mirror Claude Code's Grep/Read/Glob, with the engine swapped to zoekt.

## 4. Conventions

- **Naming**: `camelCase` variables/functions, `PascalCase` classes/types, `UPPER_SNAKE_CASE` constants centralized in `config/defaults.ts`. MCP tool names and JSON-schema keys are `snake_case`; provider input interfaces use `camelCase`, and the tool handler maps `snake_case` args → the camelCase `SearchTextInput`.
- **Files & modules**: kebab-case `.ts` filenames, one cohesive concern per file; provider directories group a pipeline by backend (`text-search/`). ESM with `.js` import extensions (NodeNext).
- **zod tool validation via `McpServer.registerTool`**: tools are registered on the high-level `McpServer` with a zod raw-shape `inputSchema` (`z.string().min(1)`, `z.enum([...OUTPUT_MODES])`, `z.number().int().min(0)`, …, each with a Korean `.describe(...)`). The SDK validates incoming args and auto-generates the ListTools JSON Schema — there is no hand-rolled `readArguments`/`readRequiredString` layer (that file was removed).
- **TOML configuration**: config files are TOML, parsed with `smol-toml`'s `parse`. The `[output]`/`[index]`/`[limits]` tables and their keys are `snake_case` in the file and normalized to `camelCase` in `ResolvedScoutConfig`; only the keys in §2's schema are tunable, everything else stays a `defaults.ts` constant.
- **Constants over literals**: timeouts, head-limit defaults, excluded directories, binary names, and the `DEFAULT_*` config fallbacks are named constants in `defaults.ts`, never inlined.
- **Doc comments**: JSDoc `/** */` on exported classes/functions explains the "why" and cites `DESIGN §`/measured findings. Tool `description` strings are written in Korean and steer the calling agent (e.g. definition vs call-site routing).
- **Custom errors**: provider failures use named error classes (e.g. `WebserverUnreachableError`) detected with `instanceof`; argument/path failures throw plain `Error` with a descriptive message.
- **Copied, not shared**: helpers mirrored from `@buyong-mcp/acp-bridge` (path containment, binary availability, and the config loader pattern in `scout-config.ts` adapted from `acp-bridge-config.ts`) are intentionally copied — the sibling app is not modified or imported, so do not refactor these into a shared package (`DESIGN.md` §9).

## 5. Working Agreements

See root `/AGENTS.md` for common working agreements.

Package-local verification: run `pnpm --filter @buyong-mcp/scout check-types` after changes in this package (this package has no test suite).
