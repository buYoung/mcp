# AGENTS.md

## 1. Overview

This monorepo contains independent local stdio MCP servers for coding-agent workflows. Each app owns its runtime and user-facing tool contract; root tooling only coordinates workspace tasks and shared policy.

## 2. Ownership Map

### Stable Ownership Boundaries

- **codemap-search Rust app**: Start in `apps/codemap-search/src/main.rs`, `apps/codemap-search/src/mcp/mod.rs`, and `apps/codemap-search/src/tools/` when changing the self-contained code-navigation server. It owns the MCP/CLI contract, embedded read/find/grep tools, Tantivy-backed search, codemap views, and configuration docs; verify with `cargo check --manifest-path apps/codemap-search/Cargo.toml` and the relevant e2e area.

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
- Absolute rule for `codemap-search`: actively use `codemap-search` for code exploration and repository navigation. Prefer it over generic Read, Grep, Find, shell search, or broad file-reading workflows whenever it is available and suitable; do not skip this rule for convenience. `codemap-search`는 MCP야. CLI로 쓰지마.
- Absolute rule for benchmark indexing: index the comparison arms and run full benchmark preparation (`route_prepare prepare`) only for a smoke or benchmark measurement the user asks for. Every other task (variant measurement, prompt tuning, regression checks) uses only the codemap arm: reuse an already prepared cache and plan, swap in only the codemap-search binary, and run each slot on an APFS clone of the prepared codemap index. Never re-index the other arms for such work. Even in a smoke or benchmark run the user may ask to measure only codemap and rg; then prepare and run only those two arms and index no others.
