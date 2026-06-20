# [fix] Block unsafe workspace roots from indexing

## Work Type
fix

## Current State (As-Is)
- Baseline: as of `7446bff` on branch `refactor/cms-restructure`.
- `apps/codemap-search/src/main.rs` starts MCP mode by calling `config::ensure_repo_template(&cwd)`, opening `TantivySearchEngine`, spawning `spawn_indexer(engine)`, and then optionally spawning the watcher.
- `apps/codemap-search/src/index/indexer.rs` runs an initial background refresh immediately after `spawn_indexer`, and that refresh calls `engine.index_files_changed(&["."])`.
- `apps/codemap-search/src/mcp/mod.rs` calls `EngineSupervisor::ensure_alive()` and `EngineSupervisor::trigger_refresh()` before both `search` and `overview`, so later requests can also enqueue full-tree refreshes.
- `apps/codemap-search/src/index/supervisor.rs` can auto-restart a dead indexer through `ensure_alive()`, and the restart path also calls `spawn_indexer(engine)` and can reattach a watcher rooted at the current directory.
- `apps/codemap-search/src/workspace.rs` centralizes path containment and live filesystem tool permission policy, but it does not currently decide whether the current directory is safe to use as an indexable workspace root.
- `apps/codemap-search/src/workspace.rs` already has a separate `resolve_for_filesystem_tool` path for `read`, `find`, and `grep`; those live filesystem tools should not be treated as equivalent to persistent indexing.

## Reproduction
- Steps: start `codemap-search mcp` with the process current directory set to the user's home directory.
- Observed: MCP startup creates or reuses `.codemap/config.toml`, constructs the Tantivy engine, spawns the background indexer, and the initial pass indexes from `"."`.
- Expected: user home must not be indexed, watched, or scaffolded with repo-local `.codemap/config.toml`; `search` and `overview` must report that indexing is disabled for the unsafe root.
- Steps: start `codemap-search mcp` with the process current directory set to a filesystem root or broad public/system directory for the current platform.
- Observed: the same startup path can attempt a broad walk from `"."` unless existing ignore rules happen to suppress parts of the tree.
- Expected: root-level and broad public/system directories must be rejected before any background indexer or watcher starts.
- Frequency: always when MCP is launched from an unsafe directory and the current code reaches `Commands::Mcp`.

## Desired Outcome (To-Be)
- `codemap-search` has one shared `validate_indexable_workspace_root(root)` policy that decides whether persistent indexing is allowed for the current workspace root.
- The validation rejects the user home directory on every platform.
- The validation delegates platform-specific broad-root checks to separate macOS, Linux, and Windows helper functions.
- The validation rejects filesystem roots, platform system directories, and broad public directories even if they contain project-like files.
- The validation requires a real project signal for non-dangerous directories, and `.codemap/config.toml` is not accepted as a project signal because MCP mode creates it automatically.
- MCP mode does not spawn the background indexer, start the watcher, trigger refreshes, or create repo-local `.codemap/config.toml` when the current directory is rejected.
- `search` and `overview` return a clear JSON-RPC tool error or disabled-index message when indexing is disabled for the root.
- `read`, `find`, `grep`, `tools/list`, and `initial_instructions` continue to work under their existing live-filesystem permission model when possible.

## Scope
### In Scope
- Add indexable-workspace validation to `apps/codemap-search/src/workspace.rs`.
- Add platform-specific unsafe-root helpers for macOS, Linux, and Windows behind `cfg` gates.
- Add a project-signal check that accepts real repository/build markers such as `.git`, `package.json`, `Cargo.toml`, `go.mod`, `pyproject.toml`, `pom.xml`, `build.gradle`, `settings.gradle`, `pnpm-workspace.yaml`, `deno.json`, or `tsconfig.json`.
- Wire the validation into MCP startup before `.codemap/config.toml` scaffolding, `TantivySearchEngine` creation, `spawn_indexer`, and watcher startup.
- Wire the disabled state into `search` and `overview` dispatch so those tools do not call `ensure_alive()` or `trigger_refresh()` on rejected roots.
- Add focused coverage for unsafe roots, user home, project-signal acceptance, and `.codemap/config.toml` non-acceptance.
- Preserve the existing `read`, `find`, and `grep` filesystem-permission behavior unless a direct conflict appears during implementation.

### Out of Scope
- [hard] Do not use `.codemap/config.toml` as an allow signal for indexing.
- [hard] Do not broaden `read`, `find`, or `grep` permissions while fixing index safety.
- [hard] Do not index, watch, or scaffold `.codemap/config.toml` in rejected roots.
- [deferred] Do not redesign the entire config system or filesystem permission model.
- [deferred] Do not add a user-facing opt-in override for unsafe roots unless a separate product decision requests it.
- [deferred] Do not change `scout` or `acp-bridge`; this brief is scoped to `apps/codemap-search`.

## Constraints
- Keep the root-safety policy in one place so MCP startup, restart, `search`, and `overview` cannot drift.
- Keep platform-specific forbidden directory lists isolated in platform-specific helpers called by `validate_indexable_workspace_root`.
- Treat dangerous-root rejection as stronger than project-signal detection: a marker file inside a forbidden root must not allow indexing.
- Avoid path-string-only checks when canonical path comparison is available; resolve symlinks leniently enough to match the existing `workspace.rs` path style.
- Keep error text actionable: name the rejected path and tell the user to start from a project directory with a real project marker.

## Related Files / Entry Points
- `apps/codemap-search/src/workspace.rs` - add `validate_indexable_workspace_root`, common home-directory rejection, platform-specific broad-root helpers, project-signal detection, and unit tests.
- `apps/codemap-search/src/main.rs` - gate MCP startup before repo-template creation, engine creation, background indexer spawn, and watcher spawn.
- `apps/codemap-search/src/index/supervisor.rs` - prevent auto-restart and refresh paths from re-enabling indexing after a root has been rejected.
- `apps/codemap-search/src/mcp/mod.rs` - return disabled-index behavior for `search` and `overview` while leaving live filesystem tools routed normally.
- `apps/codemap-search/src/index/indexer.rs` - confirm no initial `index_files_changed(&["."])` pass can run for rejected roots.
- `apps/codemap-search/src/config.rs` - keep `ensure_repo_template` from being used as an allow signal and avoid calling it for rejected roots.
- `apps/codemap-search/tests/e2e/helpers.rs` - reuse MCP process spawning patterns for integration coverage of unsafe-root behavior.
- `apps/codemap-search/tests/e2e/mcp.rs` - add or colocate MCP behavior tests for disabled `search` and `overview` responses.
- `apps/codemap-search/Cargo.toml` - existing dev dependencies already include `tempfile` and `assert_cmd`, which are sufficient for focused tests.

## Side Effect Checkpoints
- [ ] Existing safe project directories still start MCP mode, build an index, and answer `search` and `overview`.
- [ ] `ensure_alive()` cannot auto-restart an indexer in a rejected root after the initial startup gate disables indexing.
- [ ] `trigger_refresh()` cannot enqueue a full-tree refresh when indexing is disabled for the root.
- [ ] Watcher startup is skipped for rejected roots.
- [ ] `.codemap/config.toml` is not created in rejected roots.
- [ ] `read`, `find`, and `grep` still follow the existing `filesystem_permissions` behavior.
- [ ] Existing config parsing and default config template behavior still work for accepted project roots.
- [ ] Existing tests that spawn MCP from temp project directories still pass after the new project-signal requirement is satisfied in fixtures.

## Acceptance Criteria
- [ ] Starting MCP from the user home directory does not create `.codemap/config.toml`, does not spawn the indexer, and does not start the watcher.
- [ ] Starting MCP from a platform filesystem root or configured broad public/system directory does not index, watch, or scaffold repo-local config.
- [ ] Calling `search` in a rejected root returns a clear disabled-index response instead of searching stale or newly indexed content.
- [ ] Calling `overview` in a rejected root returns a clear disabled-index response instead of rendering a codemap snapshot.
- [ ] A normal project root with a real project marker still indexes and serves `search` and `overview`.
- [ ] A directory whose only marker is `.codemap/config.toml` is rejected as missing a real project signal.
- [ ] Unit tests cover macOS, Linux, and Windows forbidden-root helper behavior in a way that can be compiled on non-target platforms where practical.
- [ ] Run `cargo test --manifest-path apps/codemap-search/Cargo.toml` and report the result.

## Open Questions
- None - user decisions are resolved for this brief: `.codemap/config.toml` is excluded as an allow signal, platform-specific root checks are required, and live filesystem tools stay outside the persistent-indexing gate.
