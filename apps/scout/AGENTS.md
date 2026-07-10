# AGENTS.md

## 1. Overview

`@buyong-mcp/scout` is a stdio MCP server that exposes local code-navigation primitives backed by direct filesystem reads, zoekt text search, and Universal Ctags symbol lookup. It is designed to start in degraded mode when external binaries are missing and recover through a managed install tool.

## 2. Ownership Map

### Stable Ownership Boundaries

- **Composition and provider boundary**: Start in `src/index.ts` when changing startup, provider wiring, binary recovery, or shutdown behavior. It owns config loading, `.gitignore` exclude unioning, direct read providers, lazy search/symbol provider resolution, install coalescing, and child-process cleanup; verify through package type-check and the provider-specific tests when present.
- **Tool contract boundary**: Start in `src/tools/index.ts` when changing MCP schemas, descriptions, or tool response behavior. It owns `install_binaries`, `search_text`, `read_file`, `find_files`, and `lookup_symbol` registration with `z.strictObject`, snake_case tool inputs, degraded missing-binary guidance, and text envelopes.
- **Search lifecycle boundary**: Start in `src/providers/text-search/text-search-provider.ts`, `index-lifecycle.ts`, and `zoekt-webserver-lifecycle.ts` when changing `search_text`. These files own working-tree fingerprinting, shard rebuilds, warm webserver reuse, one retry after `WebserverUnreachableError`, and cleanup of the child process.
- **Filesystem safety boundary**: Start in `src/security/path-guard.ts`, `src/security/read-guard.ts`, and `src/providers/read/` when changing path handling. They own root containment, readable-file gates, blocked binary/device checks, glob rejection, and line-numbered read output; preserve consistent user-visible error text.
- **Binary installation boundary**: Start in `src/startup/` when changing managed zoekt/ctags discovery or download. It owns PATH/go-bin/managed-bin resolution, platform asset mapping, checksum verification, atomic replacement, and install guidance.

### Active Change Routes

- **Degraded recovery route**: Within **Composition and provider boundary**, start in `src/index.ts` and `src/startup/ensure-required-binaries.ts` when changing missing-binary behavior. Recent changes cluster around boot guidance, provider reconstruction, and managed install; keep `search_text` and `lookup_symbol` degradation explicit rather than fatal.
- **Strict schema route**: Within **Tool contract boundary**, start in `src/tools/index.ts` when changing any MCP argument. `z.strictObject` is intentional because unknown keys must be rejected instead of stripped; update descriptions and provider input mapping together.

## 3. Core Behaviors & Patterns

- **Provider boundary by dependency**: `read_file` and `find_files` are direct filesystem tools and must work without external binaries or an index. `lookup_symbol` requires only Universal Ctags; `search_text` requires zoekt-index, zoekt-webserver, and ctags.
- **Degraded boot with explicit recovery**: Missing binaries do not abort startup. Startup prints installation guidance, search/symbol tools return guidance when unavailable, and `install_binaries` performs the managed install path after user approval.
- **Install coalescing and provider rebuild**: Concurrent installs share one `installInFlight` promise. Before replacing managed binaries, `index.ts` detaches and shuts down the old `TextSearchProvider`; after install it re-resolves binaries and rebuilds providers.
- **Never-exit config loading**: `loadScoutConfig` creates only the global commented template, reads repo and global layers, warns in Korean for invalid tables/keys/types, and merges each key as `repo > global > default`.
- **Index freshness as optimization**: `IndexLifecycle` uses a working-tree fingerprint, stale-shard cleanup, and a single `buildPromise` to avoid duplicate builds. The staleness window skips unchanged trees without treating the index as authoritative for direct read tools.
- **Webserver recovery**: `TextSearchProvider` starts zoekt-webserver lazily on loopback with an ephemeral port, health-polls before use, keeps it warm across queries, and restarts once after a connection failure.
- **Long-lived symbol cache**: `SymbolProvider` is reused while the ctags path stays the same because it owns a `(scope, language) -> fingerprint/tags` cache. Recreate it only when the resolved ctags executable changes.
- **Deterministic shutdown**: Signal handlers, process exit, transport close, and stdin `end`/`close` all shut down the zoekt-webserver child so MCP clients closing stdio do not leave a process running.

## 4. Conventions

- **Naming**: TypeScript values/functions use `camelCase`, classes/types use `PascalCase`, and constants in `config/defaults.ts` use `UPPER_SNAKE_CASE`. MCP tool names and schema keys stay `snake_case`; provider inputs use `camelCase`.
- **Files and imports**: Source files are kebab-case `.ts` files with ESM `.js` import extensions. Backend-specific logic stays under `providers/read`, `providers/symbol`, or `providers/text-search`.
- **Tool schemas and descriptions**: Register MCP tools with `McpServer.registerTool` and `z.strictObject`. Descriptions are Korean and should guide agent tool choice, including when a tool requires external binaries.
- **Configuration shape**: TOML keys are `snake_case` under `[output]`, `[index]`, and `[limits]`; `ResolvedScoutConfig` exposes normalized `camelCase` fields. Arrays replace per key, while `.gitignore` directory names are unioned after load.
- **Constants over literals**: Timeouts, byte caps, release names, binary names, output modes, and default config values belong in `src/config/defaults.ts`.
- **Provider errors**: Tool boundaries return user-visible text for expected failures. Use named errors only when recovery decisions depend on type, such as `WebserverUnreachableError`.
- **Comments**: Keep concise Korean comments for operational rationale, especially when behavior exists to satisfy `DESIGN.md`, avoid install races, or prevent orphaned child processes.
- **App-local duplication**: Security, binary, and config helpers adapted from other apps remain local. Do not introduce cross-app imports unless the task explicitly changes monorepo boundaries.

## 5. Working Agreements

See root `/AGENTS.md` for common working agreements.

Package-local verification: run `pnpm --filter @buyong-mcp/scout check-types` after TypeScript changes in this package.
