# AGENTS.md

## 1. Overview

`@buyong-mcp/scout` is a stdio MCP server that exposes local code-navigation primitives backed by zoekt, Universal Ctags, and direct filesystem reads.

## 2. Folder Structure

- `DESIGN.md`: authoritative design reference for tool semantics, lifecycle decisions, and accepted divergences from Claude Code primitives.
- `src/index.ts`: composition root; loads config, unions `.gitignore` directory names, builds read/glob providers unconditionally, lazily resolves zoekt/ctags-backed providers, wires install coalescing, and owns shutdown hooks.
- `src/tools/`: high-level `McpServer.registerTool` surface for `install_binaries`, `search_text`, `read_file`, `find_files`, and `lookup_symbol`.
- `src/config/`: settings and constants.
    - `scout-config.ts`: repo/global/default TOML loader with per-key precedence and never-exit warning behavior.
    - `gitignore-excludes.ts`: extracts directory-name entries from `.gitignore` for index exclusions.
    - `defaults.ts`: server identity, binary names, release tag/URL, output defaults, timeouts, and fixed directory names.
- `src/startup/`: binary discovery and managed installation.
    - `ensure-required-binaries.ts`: resolves zoekt and Universal Ctags without exiting and builds shared guidance text.
    - `binary-installer.ts`: downloads, verifies, extracts, and atomically swaps managed binaries.
    - `binary-availability.ts`, `binary-release.ts`, `managed-bin-storage.ts`: PATH/go-bin/managed-bin resolution, platform asset mapping, and managed storage paths.
- `src/providers/`: implementation backends.
    - `read/`: `read_file` and `find_files`, direct filesystem access, line numbering, and read-state dedup.
    - `symbol/`: ctags file collection, fingerprint cache, and symbol rendering.
    - `text-search/`: zoekt index lifecycle, query builder, HTTP client, result renderer, and webserver lifecycle.
- `src/security/`: path normalization, root containment, readable-file gates, and blocked device/binary handling.

## 3. Core Behaviors & Patterns

- **Provider boundary by dependency**: `read_file` and `find_files` are direct filesystem tools and must work before any index or external binary is available. `lookup_symbol` needs only Universal Ctags; `search_text` needs zoekt-index, zoekt-webserver, and Ctags.
- **Degraded boot plus explicit recovery**: missing binaries do not abort startup. The server emits guidance, `search_text`/`lookup_symbol` return guidance when their dependency is missing, and `install_binaries` performs the user-approved managed install path.
- **Install coalescing and provider rebuild**: concurrent `install_binaries` calls share one `installInFlight` promise. Before replacing managed binaries, `index.ts` shuts down the old `TextSearchProvider`; after install it re-resolves binaries and rebuilds the provider.
- **Never-exit config loading**: `loadScoutConfig` creates only the global commented template, reads repo and global TOML layers, drops unknown or mistyped keys with Korean stderr warnings, and merges each key as `repo > global > default`.
- **Index freshness as optimization**: `IndexLifecycle` computes a cheap working-tree fingerprint, clears stale shards before rebuild, coalesces concurrent builds behind `buildPromise`, and skips unchanged trees inside the staleness window.
- **Webserver lifecycle and recovery**: `WebserverLifecycle` starts zoekt-webserver lazily on loopback with an ephemeral port, health-polls before use, keeps it warm across queries, and restarts once after `WebserverUnreachableError`.
- **Symbol lookup cache**: `SymbolProvider` is long-lived because it owns a `(scope, language) -> fingerprint/tags` cache. `index.ts` recreates it only when the resolved ctags path changes.
- **Root containment**: every file/path input normalizes through shared path guards before provider use. `find_files` also rejects dangerous glob patterns before `globby` and rechecks returned absolute paths against the repository root.
- **Deterministic shutdown**: `index.ts` handles signals, process exit, transport close, and stdin `end`/`close` so a zoekt-webserver child is not left running after MCP clients close stdio.

## 4. Conventions

- **Naming**: TypeScript uses `camelCase` values/functions, `PascalCase` types/classes, and `UPPER_SNAKE_CASE` constants in `defaults.ts`. MCP tool names and schema keys are `snake_case`; provider inputs are `camelCase`.
- **Files and imports**: source filenames are kebab-case `.ts` files with ESM `.js` import extensions. Backend-specific code stays under `providers/read`, `providers/symbol`, or `providers/text-search`.
- **Tool schemas**: register tools with `McpServer.registerTool` and `z.strictObject` schemas so unknown keys are rejected instead of stripped. Descriptions are Korean and should guide the calling agent's tool choice.
- **Configuration shape**: TOML keys are `snake_case` under `[output]`, `[index]`, and `[limits]`; `ResolvedScoutConfig` exposes normalized `camelCase` fields. Arrays replace per key, except `.gitignore` directory names are unioned after load.
- **Constants over literals**: timeouts, byte caps, output modes, release names, binary names, and default config values live in `config/defaults.ts`.
- **Error handling**: provider boundaries return user-visible text for tool failures; long-running/lifecycle failures use named errors where recovery decisions depend on type, such as `WebserverUnreachableError`.
- **Comments**: exported or non-obvious code uses concise JSDoc or inline Korean comments explaining why a behavior exists, especially when it mirrors `DESIGN.md`.
- **Copied helpers stay local**: path/binary/config patterns adapted from `mcp-server` are intentionally copied rather than shared; do not introduce cross-app imports unless the task explicitly changes that boundary.

## 5. Working Agreements

See root `/AGENTS.md` for common working agreements.

Package-local verification: run `pnpm --filter @buyong-mcp/scout check-types` after TypeScript changes in this package.
