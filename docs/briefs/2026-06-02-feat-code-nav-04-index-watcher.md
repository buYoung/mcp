# [feat] Add proactive index watcher (auto re-index)

## Work Type
feat

## Current State (As-Is)
- `IndexLifecycle.ensureFresh` re-checks the working-tree fingerprint on each query, throttled by `STALENESS_CHECK_TTL_MS`; the first query after an edit pays the re-index latency.
- Automatic incremental indexing already works lazily (query-time), so this is a latency optimization, not a correctness fix.
- There is no background watcher; nothing triggers a re-index before the next query.

## Desired Outcome (To-Be)
- A background watcher detects working-tree changes, debounces them, and triggers a re-index before the next query so the query-time latency is hidden.
- The query-time staleness path remains as the fallback; behavior is unchanged when the watcher is off.

## Scope
### In Scope
- New `index-watcher` using `fs.watch` or polling, with a debounce window and event coalescing, excluding `EXCLUDED_DIRECTORY_NAMES` and the index cache directory.
- An external rebuild trigger entry on `IndexLifecycle` (today only `ensureFresh` is public); the existing build lock is reused so the watcher and query-time paths never double-build.
- Start the watcher lazily (on first search or server start) and stop it on shutdown.
### Out of Scope
- [hard] changing the query-time staleness path — it stays as the correctness fallback.
- [hard] adding a `chokidar` dependency unless `fs.watch`/polling is proven insufficient.
- [deferred] cross-platform recursive-watch tuning beyond the chosen baseline.

## Constraints
- The watcher must not watch the index cache directory or excluded dirs — doing so creates a re-index feedback loop. Resolve the cache directory to exclude via `index-storage`'s `resolveCacheRootDirectory()`, not a hard-coded `~/.cache` string.
- Mass-change events (e.g. `git checkout`) must coalesce into a single re-index via the debounce window.
- A no-op event (unchanged fingerprint) must not invoke `zoekt-index`.
- Default values to add to `config/defaults.ts`: `REINDEX_DEBOUNCE_MS = 400`; if polling is used, a watch poll interval of `1000` ms. These are starting values, tunable.
- "Start lazily" means start on the first `search_text` call (matching the lazy index build), not eagerly at server init.
- Expose the external trigger as a new `IndexLifecycle` method (e.g. `requestBackgroundRebuild()`), distinct from `ensureFresh`, so the watcher path and the query-time path are separable; both funnel through the existing build lock.
- Observe "build generation increments" via `IndexLifecycle`'s internal generation counter (already bumped on each build) — surface it for the acceptance test via provider state or a stderr log line.

## Related Files / Entry Points
- `apps/code-nav/src/providers/text-search/index-lifecycle.ts` — add the external rebuild trigger; reuse the fingerprint walk.
- `apps/code-nav/src/providers/text-search/text-search-provider.ts` — wire the watcher into the provider that owns the index.
- `apps/code-nav/src/index.ts` — start/stop the watcher alongside the existing shutdown hooks.
- `apps/code-nav/src/config/defaults.ts` — add `REINDEX_DEBOUNCE_MS` and a watch poll interval.
- `apps/code-nav/DESIGN.md` — §6.4 carries the watcher design and the "automatic = debounced full re-index + skip-when-unchanged" framing.
- `apps/code-nav/src/providers/text-search/index-watcher.ts` (proposed) — watch start/stop, debounce, trigger.

## Side Effect Checkpoints
- [ ] The watcher and query-time staleness path do not double-build (build lock coalesces both).
- [ ] Shutdown releases watcher handles and timers; the process still exits cleanly on SIGINT/SIGTERM/stdin-close.
- [ ] No feedback loop from cache-directory writes.

## Acceptance Criteria
- [ ] Editing a source file triggers a background re-index within the debounce window (build generation increments) without any query.
- [ ] N rapid edits coalesce into a single re-index.
- [ ] An unchanged touch (same fingerprint) does not invoke `zoekt-index`.
- [ ] With the watcher disabled, the query-time staleness path still keeps results fresh (no regression).

## Open Questions
- Watch mechanism: `fs.watch` recursive (macOS/Windows) versus polling (Linux reliability) versus a single polling implementation for all platforms — the codebase cannot decide this; delegate the choice to implementation.
