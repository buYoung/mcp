# [feat] Add bounded skill package read permissions

## Work Type
feat

## Current State (As-Is)
- As of `7446bff` on `refactor/cms-restructure`, `codemap-search` exposes live filesystem access only through `find`, `grep`, and `read`; `search` and `overview` stay index-backed and workspace-scoped.
- `apps/codemap-search/src/config.rs` defines `[filesystem_permissions]` with `workspace`, `allowed_roots`, and `anywhere` policies for `find`, `grep`, and `read`, defaulting all three tools to `workspace`.
- `apps/codemap-search/src/workspace.rs` allows workspace paths unconditionally, then gates external paths through `[filesystem_permissions]`; it has no separate concept for trusted skill package reads.
- `apps/codemap-search/src/config.rs` parses `allowed_roots` as string arrays and canonicalizes them, but `path_from_workspace_input()` only normalizes separators and does not expand `~` or render home-relative display paths.
- `apps/codemap-search/docs/configuration.md` documents `anywhere` as high-risk full-disk access and recommends bounded `allowed_roots`; this is too broad for the user request because skill reads should not widen general `read`, `find`, or `grep`.
- The requested default agent skill locations are `codex`, `claude`, `opencode`, `pi`, and `kilocode`; these should be treated as built-in skill root candidates, not discovered by a global `**/skills/**` permission pattern.

## Desired Outcome (To-Be)
- Add a dedicated `[skill_permissions]` configuration surface that permits reading trusted skill package directories without changing general live filesystem tool permissions.
- Model the permission boundary around verified skill package roots, not around `SKILL.md` files only, so referenced `references/`, `scripts/`, `templates/`, examples, and other package-local files remain readable.
- Include built-in default roots for `codex`, `claude`, `opencode`, `pi`, and `kilocode`, with user-configurable additions or overrides if the existing config layering pattern supports them cleanly.
- Support home-relative config input and user-facing display: macOS/Linux examples use `~/.<agent>/skills`; Windows PowerShell examples use `$env:USERPROFILE\.<agent>\skills`; Windows cmd examples use `%USERPROFILE%\.<agent>\skills`.
- Keep actual authorization checks based on canonical absolute paths after home expansion and symlink resolution; display shortening must never be used as the security check.
- Reject path traversal and symlink escapes from a registered skill package root, even when the original request path appears to be under an allowed skill directory.
- Avoid `**/skills/**` or any other global glob as a permission rule; discovery may enumerate known roots, but authorization must check concrete canonical roots.

## Scope
### In Scope
- Add config structs, defaults, parsing, merge behavior, and template comments for `[skill_permissions]` in `apps/codemap-search/src/config.rs`.
- Add workspace/path helpers needed for home expansion, home-relative display, canonical root matching, and package-root containment in `apps/codemap-search/src/workspace.rs` or a colocated module.
- Add a skill-read authorization path that can answer "may this skill loader read this file?" separately from `FilesystemTool::Read`.
- Update `apps/codemap-search/docs/configuration.md` and `apps/codemap-search/README.md` only where needed to document the new skill permission model and OS-specific home path examples.
- Preserve current `[filesystem_permissions]` behavior for `read`, `find`, and `grep`.

### Out of Scope
- [hard] Do not set `[filesystem_permissions].read = "anywhere"` or otherwise broaden general file reads as the implementation strategy.
- [hard] Do not authorize paths by matching the global pattern `**/skills/**`.
- [hard] Do not limit the feature to `skills/*/SKILL.md`; the permission unit is the verified skill package root.
- [hard] Do not change `search` or `overview` indexing scope.
- [hard] Do not add new test files or test cases unless the owner explicitly asks for tests in a follow-up.
- [deferred] Do not implement a full skill registry UI or installer flow in this brief; this is only the filesystem permission model and documentation.

## Constraints
- Keep the configuration loader's never-exit behavior: unknown keys, bad types, and invalid paths warn to stderr and fall back per key.
- Follow existing config precedence: repo `.codemap/config.toml` wins over global config, which wins over built-in defaults, unless the implementation documents a narrower rule for built-in default skill roots.
- Use `snake_case` TOML keys and Rust field names, matching the existing config style.
- Treat display-path formatting as presentation only; all permission decisions use canonical absolute paths.
- Use OS-specific examples in docs, but keep cross-platform parsing deterministic and covered by code structure rather than shell-specific string expansion at authorization time.

## Related Files / Entry Points
- `apps/codemap-search/src/config.rs` - start here for `ResolvedConfig`, default config template, TOML normalization, merge behavior, and existing filesystem permission parsing.
- `apps/codemap-search/src/workspace.rs` - extend canonicalization and containment helpers; this is where workspace and external filesystem paths are currently authorized.
- `apps/codemap-search/src/tools/read.rs` - reference the existing live `read` path only to avoid accidentally changing its security boundary.
- `apps/codemap-search/src/mcp/mod.rs` - inspect protocol dispatch and tool schema boundaries if skill reads need a public MCP-facing route or shared helper exposure.
- `apps/codemap-search/docs/configuration.md` - document `[skill_permissions]`, defaults, home path display, and the reason global globs are not accepted as permission rules.
- `apps/codemap-search/README.md` - update the short configuration reference if the new section changes user setup instructions.
- `apps/codemap-search/AGENTS.md` - package-local verification guidance for Rust changes; run the listed `cargo check` command after implementation.

## Side Effect Checkpoints
- [ ] Existing `read`, `find`, and `grep` behavior stays workspace-only by default.
- [ ] A configured or built-in skill root allows files under that skill package root, including nested reference assets.
- [ ] A path that escapes the skill package root through `..` or a symlink is rejected after canonicalization.
- [ ] Home-relative path display is shortened for user messages without weakening authorization.
- [ ] Windows examples and parsing do not accidentally treat `$env:USERPROFILE` or `%USERPROFILE%` as literal Unix-style directory names on non-Windows hosts.
- [ ] Documentation distinguishes "discovery under known roots" from "authorization by global glob".

## Acceptance Criteria
- [ ] `[skill_permissions]` is parsed into `ResolvedConfig` with defaults that cover `codex`, `claude`, `opencode`, `pi`, and `kilocode` skill roots.
- [ ] Skill package read authorization is available separately from `resolve_for_filesystem_tool(..., FilesystemTool::Read)`.
- [ ] Authorization accepts a canonical path inside a registered skill package root and rejects a canonical path outside that root.
- [ ] The implementation supports package-local reads beyond `SKILL.md`, including nested references and templates.
- [ ] The config template and `docs/configuration.md` show macOS/Linux `~`, Windows PowerShell `$env:USERPROFILE`, and Windows cmd `%USERPROFILE%` examples.
- [ ] `cargo check --manifest-path apps/codemap-search/Cargo.toml` passes after the Rust changes.

## Open Questions
- None - the owner selected the default agent set and rejected both global `**/skills/**` authorization and `SKILL.md`-only authorization.
