# [feat] Add navigate_code intent router (post-v1)

## Work Type
feat

## Current State (As-Is)
- v1 ships primitives only; the calling agent decides definition-versus-call-site routing itself, guided by each tool's description.
- No router exists, so the benchmark's C4 routing rule (M3-W: definitions to ctags, call-sites to text) is not codified in one tool.
- `lookup_symbol`, `read_file`, and `find_files` do not exist yet (children 01–03); this router composes them.

## Desired Outcome (To-Be)
- A `navigate_code` tool classifies query intent (definition → `lookup_symbol`, usage/call-site/cross-language → `search_text`, ambiguous → `search_text` fallback), then chains a `read_file` confirmation of the top candidates, and returns `routed_to` plus a `reason`.
- The router never blindly chains all three tools and never overrides the directly callable primitives.

## Scope
### In Scope
- New router plus an intent classifier that composes the existing providers in-process (no extra child processes).
- Input fields: `query` (required), `intent` (`definition` | `usage` | `auto`), `path`, `read_top_candidates` (default 3).
- Surface `routed_to` and `reason` in the result so mis-routes are debuggable.
### Out of Scope
- [deferred] this entire child is post-v1 — build it only after children 01, 02, and 03 land.
- [hard] spawning extra processes — composition is in-process only.
- [hard] overriding or hiding the primitives — they remain independently callable; the router is additive.

## Constraints
- Ambiguous intent must fall back to `search_text` (zoekt), never to ctags — ctags grabs the declaration instead of the call site and drops M3-W precision.
- The router composes existing providers; it does not duplicate their query, ctags, or read logic.

## Related Files / Entry Points
- `apps/code-nav/src/tools/index.ts` — register `navigate_code` and dispatch here (shared conflict hotspot).
- `apps/code-nav/src/providers/text-search/text-search-provider.ts` — existing search provider the router calls for usage intent.
- `apps/code-nav/DESIGN.md` — §2.0 carries the M3-W routing rationale (definition→ctags, usage/ambiguous→text) for the router-less v1; §2.2 and §2.1 hold the per-tool routing guidance the router should encode.
- `apps/code-nav/src/providers/symbol/symbol-provider.ts` (proposed) — dependency from child 01, called for definition intent.
- `apps/code-nav/src/providers/read/read-file.ts` (proposed) — dependency from child 02, called for the confirmation step.
- `apps/code-nav/src/router/navigate-code.ts` (proposed) — orchestration.
- `apps/code-nav/src/router/classify-navigation-intent.ts` (proposed) — heuristic intent classifier with ambiguous→text fallback.

## Side Effect Checkpoints
- [ ] The three primitives remain independently callable; the router is purely additive.
- [ ] The routing `reason` is surfaced so a mis-route is debuggable from the response alone.

## Acceptance Criteria
- [ ] A "where is X defined" query routes to `lookup_symbol`; a "callers of X" query routes to `search_text`.
- [ ] An ambiguous query routes to `search_text`, never to `lookup_symbol`.
- [ ] The top `read_top_candidates` results are confirmed via `read_file` in the response.
- [ ] The response includes `routed_to` and a non-empty `reason`.

## Open Questions
- The classifier is heuristic; the per-language natural-language cues for definition versus usage intent should be refined after collecting real mis-route cases — delegate tuning to implementation.
