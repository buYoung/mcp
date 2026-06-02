# [feat] Add find_files tool (Claude Code Glob mirror)

## Work Type
feat

## Current State (As-Is)
- `apps/code-nav` has no filename-search tool; the Glob capability is absent.
- Verified this session against `/Users/buyonglee/Downloads/claude-code-main/src/utils/glob.ts`: Claude Code's Glob shells out to ripgrep `--files --glob <pat> --sort=modified` (oldest-first, glob.ts:94,104), caps at 100 results (`globLimits?.maxResults ?? 100`), and defaults to `--no-ignore` + `--hidden` (glob.ts:98-99).
- Decision 1 forbids a ripgrep fallback; the user chose a JS glob library (globby) for `find_files` so filename matching does not reintroduce ripgrep.

## Desired Outcome (To-Be)
- A `find_files` tool lists file paths matching a glob pattern, sorted by mtime oldest-first, capped at 100 with a truncation note, with `.gitignore` not respected and hidden files included by default, paths relativized to the repo root.
- Behavior matches Claude Code Glob even though the backend is globby, not ripgrep.

## Scope
### In Scope
- New `find-files` provider backed by globby (picomatch). Input fields: `pattern` (required), `path`.
- mtime ascending (oldest-first) sort, 100-result cap, the truncation message, and the `No files found` empty message.
- Environment toggles `CODE_NAV_GLOB_NO_IGNORE` and `CODE_NAV_GLOB_HIDDEN` (both default true) mirroring Claude Code's flags.
- Exclude `EXCLUDED_DIRECTORY_NAMES` from results so node_modules and vendor dirs never flood output.
- Add `globby` to `apps/code-nav/package.json` dependencies.
### Out of Scope
- [hard] ripgrep backend — decision 1 forbids it; use globby.
- [hard] content search — that is `search_text`.
- [deferred] offset/pagination beyond the 100-result cap.

## Constraints
- Add `globby` as a dependency and justify it in the PR (external dependency policy): zoekt and ripgrep are not filename matchers, and globby gives stable `{}` / `**` semantics plus mtime sort and ignore/hidden control. Pin the workspace-resolved latest; no special version constraint is known.
- Replicate the oldest-first sort and keep-first-100 behavior — the most recently modified files may be dropped on truncation, matching Claude Code.
- `path` is the search base directory (matches Claude Code Glob `getPath`: the given path, else cwd), not a post-filter. The repo root for validation and relativization is `process.cwd()`, consistent with `TextSearchProvider`.
- Map the env toggles to globby options: `CODE_NAV_GLOB_NO_IGNORE=true` → globby `gitignore: false`; `CODE_NAV_GLOB_HIDDEN=true` → globby `dot: true`.
- Pass `EXCLUDED_DIRECTORY_NAMES` to globby as `ignore` patterns (not a post-filter).
- Do not follow symlinks out of the repo root; relativize via a path-containment check so escaping symlinks cannot leak absolute paths.
- Exact output strings (inlined here so this brief is self-sufficient; from DESIGN §4.2): no match → `No files found`; truncation marker appended as the last line → `(Results are truncated. Consider using a more specific path or pattern.)`.

## Related Files / Entry Points
- `apps/code-nav/src/tools/index.ts` — register `find_files` and dispatch here (shared conflict hotspot).
- `apps/code-nav/package.json` — add the `globby` dependency (single-child edit hotspot).
- `apps/code-nav/src/security/path-guard.ts` — validate the base `path` argument within the repo root.
- `apps/code-nav/src/config/defaults.ts` — add `GLOB_RESULT_LIMIT = 100`, truncation/empty messages, and the env toggle keys.
- `apps/code-nav/DESIGN.md` — §4.2 carries the Glob mapping table.
- `apps/code-nav/src/providers/read/find-files.ts` (proposed) — globby execution, mtime sort, truncation, relativization.

## Side Effect Checkpoints
- [ ] `globby` installs cleanly and does not conflict with existing dependencies or the pnpm workspace.
- [ ] node_modules and other excluded dirs are absent from results.
- [ ] Adding `find_files` does not change `search_text` registration behavior.

## Acceptance Criteria
- [ ] `find_files {pattern:"**/*.ts"}` returns repo `.ts` files relativized to the repo root, with node_modules excluded.
- [ ] A pattern matching more than 100 files returns exactly 100 plus the truncation message, and the most recently modified files are the ones dropped (oldest-first).
- [ ] Hidden files are included and `.gitignore` is not respected by default.
- [ ] A pattern with no matches returns `No files found`.

## Open Questions
- None — the exact output strings are inlined in Constraints; backend (globby), sort direction, cap, and env-toggle mapping are all fixed. Optionally re-confirm the strings against the Claude Code GlobTool render path if that source is available, but it is not a prerequisite.
