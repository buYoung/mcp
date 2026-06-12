# AGENTS.md

## 1. Overview

A monorepo of standalone local MCP servers and shared TypeScript configuration for coding-agent workflows. The apps are independent products: `acp-bridge` pairs agents over ACP, `scout` exposes zoekt/ctags navigation primitives, and `codemap-search` ships a self-contained Rust code-navigation server.

## 2. Folder Structure

- `apps/`: deployable stdio MCP servers; each app owns its runtime, docs, and package-local `AGENTS.md`.
    - `mcp-server` (`@buyong-mcp/acp-bridge`): TypeScript MCP server that spawns Claude Code, Codex, and Gemini CLI as read-only ACP pair-review child processes.
    - `scout` (`@buyong-mcp/scout`): TypeScript MCP server that exposes `search_text`, `lookup_symbol`, `read_file`, `find_files`, and managed zoekt/Universal Ctags installation.
    - `codemap-search`: Rust MCP/CLI binary with embedded tree-sitter parsing, Tantivy BM25 search, caller/callee annotations, and read/find/grep tools.
- `packages/`: shared, non-deployable workspace packages.
    - `typescript-config` (`@repo/typescript-config`): shared `tsconfig` bases (`base.json`, `node.json`) consumed by TypeScript apps.
- `docs/briefs/`: dated task briefs; align feature work with the relevant brief when one exists.
- Root tooling: `pnpm-workspace.yaml` declares `apps/*` and `packages/*`; `turbo.json` wires pnpm workspace tasks; `biome.json`, `lint-staged.config.mjs`, and `.husky/` define repository-wide formatting and pre-commit behavior.

## 3. Working Agreements

- Respond in the user's preferred language; if unspecified, infer from the codebase (Korean appears in README/design docs, comments, and MCP `description` strings; never translate fenced code blocks).
- Ask the user before introducing tests, lint, or formatter setups; add them only on explicit request.
- Build context by reviewing related usages, flows, patterns, and likely impact before editing.
- Fix the underlying cause, not only the visible symptom; inspect affected flows and apply the narrowest complete change that resolves the root issue.
- Check side effects across callers, shared abstractions, and behavior/API boundaries; report relevant impact and compatibility risks. Treat the apps as independent unless the task explicitly spans them.
- Ask actively when user decisions are needed for scope, behavior, or tradeoffs.
- Run type-check after TypeScript code changes: `pnpm check-types`. Package-level `AGENTS.md` files may add package-local verification for non-pnpm code.
- In monorepos, put package-only tests/type-check/verification guidance in the package-level `AGENTS.md`, not this root document.
- New functions: single-purpose, colocated with related code.
- External dependencies: only when necessary, explain why.

## User custom rules
- codemap-search 를 적극 활용하세요. Read, Grep, Find 대체가능
