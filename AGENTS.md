# AGENTS.md

## 1. Overview

This monorepo contains independent local stdio MCP servers for coding-agent workflows plus shared TypeScript compiler presets. Each app owns its runtime and user-facing tool contract; root tooling only coordinates workspace tasks and shared policy.

## 2. Ownership Map

### Stable Ownership Boundaries

- **ACP pair-review app**: Start in `apps/mcp-server/src/index.ts` and `apps/mcp-server/src/tools/index.ts` when changing read-only pair consultation behavior. The app owns the MCP tools that launch ACP child agents, preserve consent and `main_agent_position` guardrails, and return the shared JSON text envelope; verify through the package type-check and the existing pair/session tests.
- **Scout navigation app**: Start in `apps/scout/src/index.ts` and `apps/scout/src/tools/index.ts` when changing zoekt/ctags navigation behavior. The app owns degraded startup, managed binary recovery, direct filesystem reads, and strict MCP schemas; preserve the tool semantics documented in `apps/scout/DESIGN.md` and verify through its package type-check.
- **codemap-search Rust app**: Start in `apps/codemap-search/src/main.rs`, `apps/codemap-search/src/mcp/mod.rs`, and `apps/codemap-search/src/tools/` when changing the self-contained code-navigation server. It owns the MCP/CLI contract, embedded read/find/grep tools, Tantivy-backed search, codemap views, and configuration docs; verify with `cargo check --manifest-path apps/codemap-search/Cargo.toml` and the relevant e2e area.
- **Shared TypeScript presets**: Start in `packages/typescript-config/base.json` or `node.json` when changing TypeScript compiler defaults. These presets are consumed by both TypeScript apps through `@repo/typescript-config/node.json`; preserve broad NodeNext/strict defaults and verify by running the root type-check.

### Active Change Routes

- **codemap-search release/config route**: Within **codemap-search Rust app**, start in `apps/codemap-search/src/config.rs`, `apps/codemap-search/docs/configuration.md`, and `apps/codemap-search/Cargo.toml` when changing config keys, defaults, or release metadata. Recent churn is concentrated there, and code/docs must stay aligned because generated config templates, schema-sync behavior, and the published README all expose the same contract.
- **codemap-search tool-description route**: Within **codemap-search Rust app**, start in `apps/codemap-search/src/tools/mod.rs` and `apps/codemap-search/src/tools/search/` when changing navigation guidance or search output. Preserve read-only tool annotations, monorepo-aware descriptions, output caps, and caller/callee budget behavior because clients rely on these descriptions to choose tools safely.

## 3. Working Agreements

- Respond in the user's preferred language; if unspecified, infer from codebase (keep tech terms in English, never translate code blocks).
- Ask the user before introducing tests, lint, or formatter setups; add them only on explicit request.
- Build context by reviewing related usages, flows, patterns, and likely impact before editing.
- Fix the underlying cause, not only the visible symptom; inspect affected flows and apply the narrowest complete change that resolves the root issue.
- Check side effects across callers, shared abstractions, and behavior/API boundaries; report relevant impact and compatibility risks.
- Ask actively when user decisions are needed for scope, behavior, or tradeoffs.
- Run type-check after TypeScript code changes: `pnpm check-types`.
- Put package-only tests/type-check/verification guidance in the package-level AGENTS.md, not the root document.
- New functions: single-purpose, colocated with related code.
- External dependencies: only when necessary, explain why.

## User custom rules
- Absolute rule for `fable5.md`: for any work involving `fable5.md`, read `fable5.md` first and treat its current contents as the source of truth. Do not skip this rule for convenience.
- Absolute rule for `codemap-search`: actively use `codemap-search` for code exploration and repository navigation. Prefer it over generic Read, Grep, Find, shell search, or broad file-reading workflows whenever it is available and suitable; do not skip this rule for convenience.
