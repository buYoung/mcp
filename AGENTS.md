# AGENTS.md

## 1. Overview

A pnpm + Turborepo monorepo of standalone stdio MCP (Model Context Protocol) servers that extend coding agents: an ACP pair-review bridge to other coding agents, and a zoekt + Universal Ctags code-navigation toolset. Each app is independent; treat `apps/mcp-server` and `apps/scout` as sibling apps that are not modified together.

## 2. Folder Structure

- `apps/`: deployable stdio MCP servers. Each is its own workspace package with a `bin` entry, ESM output to `dist/`, and a `src/index.ts` entry that wires an MCP `Server` to a `StdioServerTransport`.
    - `mcp-server` (`@buyong-mcp/acp-bridge`): MCP server that spawns other coding agents (Claude Code, Codex, Gemini CLI) as ACP child processes for read-only pair review. See its `AGENTS.md`.
    - `scout` (`@buyong-mcp/scout`): MCP server exposing zoekt + ctags search/read primitives. `DESIGN.md` is its authoritative design doc; see its `AGENTS.md`.
- `packages/`: shared, non-deployable workspace packages.
    - `typescript-config` (`@repo/typescript-config`): shared `tsconfig` bases (`base.json`, `node.json`) that apps `extends`. No source code.
- `docs/briefs/`: dated, Conventional-Commit-typed task briefs that drive feature work; align changes with the relevant brief.
- Root tooling: `turbo.json` (task graph for `build`/`dev`/`check-types`/`test`), `biome.json` (lint + format), `pnpm-workspace.yaml` (workspace globs), `lint-staged.config.mjs` + `.husky/` (pre-commit). New apps go under `apps/*`, shared libraries under `packages/*`.

## 3. Working Agreements

- Respond in the user's preferred language; if unspecified, infer from the codebase (Korean here — comments, docs, and `description` strings are Korean). Keep technical terms in English and never translate code blocks.
- Ask the user before introducing tests, lint, or formatter setups; add them only on explicit request.
- Build context by reviewing related usages, flows, patterns, and likely impact before editing.
- Fix the underlying cause, not only the visible symptom; inspect affected flows and apply the narrowest complete change that resolves the root issue.
- Check side effects across callers, shared abstractions, and behavior/API boundaries; report relevant impact and compatibility risks. The two apps are independent — do not let a change to one modify the other (e.g. `scout` copies, rather than imports, helpers from `mcp-server`).
- Ask actively when user decisions are needed for scope, behavior, or tradeoffs.
- Run type-check after code changes: `pnpm check-types` (Turborepo runs `tsc -p tsconfig.json --noEmit` per package).
- In monorepos, put package-only test/verification guidance in the package-level `AGENTS.md`, not this root document.
- New functions: single-purpose, colocated with related code.
- External dependencies: only when necessary, and explain why.

## User custom rules
- codemap-search 를 적극 활용하세요. Read, Grep, Find 대체가능
