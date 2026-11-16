---
title: Configuration Reference
description: Complete reference for codemap-search config.toml, auth.toml credentials, client output budgets, exclusions, Jev stages, and event navigation.
---

Configuration is optional. Add only the keys you want to change; everything else falls back to your global config or built-in defaults.

Event navigation (`output.event_navigation`) and native macro expansion (`output.macro_expansion`) are enabled by default. `[analysis].target_os` inherits your global setting when omitted and clears it when set to `""`; without a configured target, Rust target OS stays unknown and is never guessed from the host. `[output.jev]` filtering stays off until explicitly enabled (see [Optional Jev decision stages](#optional-jev-decision-stages)).

## Section layout and output budgets

Behavior settings live in `config.toml`, grouped by responsibility, while API keys live in `auth.toml`. Any active value in a higher layer (including an empty list `[]`) overrides the lower layer, so remove or comment out a key when you want to inherit it.

| Section | Responsibility |
| --- | --- |
| `output` | Shared MCP response byte cap and masking switch |
| `output.client` | Claude character limit, Codex token limit, and pi/opencode byte limits |
| `output.search` | Ranked files, symbols, and snippets |
| `output.read` | Live file reads |
| `output.grep` | Matching rows and callable-expanded responses |
| `output.context` | Caller/callee display limits and relationship sub-budget |
| `output.navigation` | Source-based navigation and caller scanning budgets |
| `output.macro_expansion` | Native C/C++/ASM preprocessing and generated declarations |
| `output.event_navigation` | Indexed event/source routes and navigation output |
| `output.redact` | Additional masking rules, PII entities, and exceptions |
| `output.jev` | Optional `search`/`read`/`grep` relevance filtering (off by default) |
| `output.context.exclude` | Shared test exclusions for relationships, read/grep context, and event/source routes |
| `index`, `index.refresh`, `index.language_support` | Index storage, refresh intervals, and optional language groups |
| `index.exclude` | Shared directory exclusions for indexing, `search`, caller scans, `find`, and `grep` |
| `analysis` | Explicit Rust `target_os` |
| `global_instructions` | User-global toggle for a managed client instruction line (ignored in repo config) |

Search operates on indexed files, while `find` and `grep` share the directory exclusion rules. Direct `read` ignores directory exclusions, though its automatic context still applies `output.context.exclude`.

`output.max_bytes` is optional. Response caps resolve as **repo tool override > repo common > global tool override > global common > built-in tool default**. When all caps are omitted, `search` defaults to `1 MiB` and `read` defaults to `5 MiB`. Callable-expanded `grep` inherits `output.read.max_bytes` within a layer if neither `output.grep.max_bytes` nor `output.max_bytes` is set there. `output.context.max_bytes` is a separate relationship sub-budget further constrained by remaining space in the search response.

When an explicit limit is hit, `search` returns partial results with a truncation footer and `read` asks for a narrower line range. `find`, `initial_instructions`, and unexpanded `grep` fail with a narrowing error instead of clipping output. Byte caps apply to MCP response text only, not JSON-RPC framing, error messages, or ordinary CLI commands.

### Client delivery budgets

`output.client` caps the final result size each coding client passes to its model. Tool `max_bytes` settings govern earlier candidate collection, Jev input, rendering, and pagination, while every final response must fit inside the smallest configured client limit.

Limits are compared in UTF-8 bytes. Claude (`claude_max_result_chars`) treats the character limit as a UTF-8 byte ceiling so multi-byte text stays within Claude's character budget, Codex estimates `floor(tokens × 3.5)` (`codex_output_token_limit`), and `pi` and `opencode` use byte limits directly. All four keys default to `100000` in their respective units, giving a `100000`-byte final delivery ceiling out of the box.

The delivery check runs on final text after Jev omissions, masking, and duplicate folding:

- **`search`** keeps matched and uncertain bodies first, then replaces lower-priority whole bodies with exact read ranges inside `min(output.search.max_bytes, limit - min(512, limit / 8))`, reserving room for relationships, follow-up discovery, and post-render markers.
- **`read` (`ranges`)** packs complete items (including headings and inline errors) within the smaller of `output.read.max_bytes` and the client limit, reserving space for a continuation notice naming the first deferred item. Deferred items are not recorded as delivered.
- **Single `read`, `grep`, and other calls** return a narrowing error (suggesting a smaller window or `head_limit`/`offset`) rather than letting the client clip output.

Client-side limits to keep in mind:

- **Claude Code (`output.client.claude_max_result_chars`, `1–500000`, default `100000`)**: Each tool advertises `_meta["anthropic/maxResultSizeChars"]` in `tools/list`. Reconnect MCP after changes. See the [Claude Code reference](https://code.claude.com/docs/en/mcp#raise-the-limit-for-a-specific-tool).
- **Codex (`output.client.codex_output_token_limit`, default `100000`)**: Run `codemap-search codex-config` (pass `--server-name NAME` if registered under another ID) and merge the printed `mcp_servers.codemap-search.tools.<tool>.output_token_limit` fragment into Codex config. In Codex Code Mode, connections identifying `params.clientInfo.name` as `codex-mcp-client` receive instructions to put `// @exec: {"max_output_tokens": N}` on line 1 (and match it on `wait`). Note that Codex `tool_output_token_limit` and multi-call `exec` cells remain governed by Codex itself. See the [Codex MCP reference](https://learn.chatgpt.com/docs/extend/mcp#other-configuration-options) and [Codex config reference](https://learn.chatgpt.com/docs/config-file/config-reference#configtoml).
- **`pi` (`output.client.pi_max_bytes`, default `100000`)**: Advertised via `_meta["pi/maxOutputBytes"]` in `tools/list`. [@buyong/pi-mcp-adapter](https://www.npmjs.com/package/@buyong/pi-mcp-adapter) honors this ahead of `maxOutputBytes` in `mcp.json`; pi's built-in MCP client ignores tool metadata and cuts larger results at 20 KB. Reconnect MCP after changes.
- **`opencode` (`output.client.opencode_max_bytes`, default `100000`)**: OpenCode ignores server metadata and defaults [`tool_output.max_bytes`](https://opencode.ai/v2/docs/config) to `51200` bytes, saving overflows to disk. Match `opencode`'s `tool_output.max_bytes` to `opencode_max_bytes` manually. OpenCode's line limit (default `2000`) is not enforced by `codemap-search`.

For diagnostics, `MCP client delivery context` logs whether `params._meta.callId` was present (recording the value only for `exec-<uuid>` IDs), response bytes, and the Codex limit without altering output behavior.

## Files and precedence

Settings merge **per key** from two layers: **repo > global > built-in default**. Setting `[output.search].detail_file_limit` in a repo config still inherits every other key from global config or defaults.

| Layer | Path |
| --- | --- |
| Repo | `<repo>/.codemap/config.toml` |
| Global | `$CODEMAP_HOME/config.toml`, else `~/.codemap/config.toml` |

### Global instructions

`[global_instructions].enabled` defaults to `false` and is read **only from global config** (repo values warn and are ignored):

```toml
[global_instructions]
enabled = true
```

On MCP initialization, the server inspects the exact `clientInfo.name` and inherited environment variables to locate the client's global instruction file:

| Client name | Default file | Overrides and precedence |
| --- | --- | --- |
| `codex-mcp-client` | `~/.codex/AGENTS.md` | `CODEX_HOME`; non-empty `AGENTS.override.md` takes precedence |
| `claude-code` | `~/.claude/CLAUDE.md` | `CLAUDE_CONFIG_DIR` |
| `pi` | `~/.pi/agent/AGENTS.md` | `PI_CODING_AGENT_DIR`; first existing file among `AGENTS.override.md`, `AGENTS.md`, `AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD` |
| `opencode` | `$XDG_CONFIG_HOME/opencode/AGENTS.md` (usually `~/.config/opencode/AGENTS.md`) | `OPENCODE_CONFIG_DIR`; v1 falls back to existing `~/.claude/CLAUDE.md` when native `AGENTS.md` is absent and Claude prompts are enabled; v2 uses `AGENTS.md` |

These follow the [Codex](https://learn.chatgpt.com/docs/agent-configuration/agents-md), [Claude Code](https://code.claude.com/docs/en/memory), [pi](https://pi.dev/docs/latest/configuration), and [OpenCode v1](https://dev.opencode.ai/docs/rules/) / [v2](https://opencode.ai/v2/docs/instructions/) conventions (and OpenCode's [global path implementation](https://github.com/anomalyco/opencode/blob/dev/packages/core/src/global.ts)). Unknown client names, as well as OpenCode installations whose version cannot be checked when a Claude fallback exists, are skipped.

When enabled, the server appends or updates one English line regardless of locale:

```md
- Prefer codemap-search for code navigation when available. Call `initial_instructions` first and follow the returned guidance. <!-- codemap-search:managed -->
```

Ownership is tracked in `global-instructions.json` inside the codemap global directory. Setting `enabled = false` removes tracked managed lines while leaving user instructions, file permissions, line endings, and symlinks intact. Unowned markers, edited managed lines, or duplicate lines are left alone with a warning. Updates run on initialization or the next request after a config reload; start a new agent session for the client to read the updated file.

### Credentials (`auth.toml`)

Keep decision-model API keys out of `config.toml` and store them in `<repo>/.codemap/auth.toml` or `$CODEMAP_HOME/auth.toml` (`~/.codemap/auth.toml` by default):

```toml
# .codemap/auth.toml — keep out of version control
[jev]
api_key = "<your TypeSafe key>"

[openrouter]
api_key = "<your OpenRouter key>"
```

Each key resolves independently as **repo auth > global auth > environment variable** (`TYPESAFE_API_KEY` or `OPENROUTER_API_KEY`), using only the provider selected by `[output.jev].model`. Empty or whitespace-only values fall through to the next source, and malformed `auth.toml` entries warn without printing secret values. OpenRouter keys are sent exclusively to OpenRouter's Decisions API (`https://openrouter.ai/api/alpha/decisions`), never to chat endpoints (see [OpenRouter decision models](#openrouter-decision-models)).

When `[update].config_auto_update = true`, MCP startup scaffolds an empty repo `auth.toml` with `0600` permissions on Unix (inherited ACLs on Windows) if missing, without copying environment variables or touching global files. Any file named `auth.toml` is excluded case-insensitively from indexing, `search`, and default `find`/`grep` (though direct `read`/`parse` or `include_ignored: true` can access it). Keep credential files out of Git yourself.

## Loading and automatic writes

Configuration files are optional. Malformed TOML drops that file's layer with a warning on `stderr`, while an unknown key, wrong type, or invalid value warns and falls back per key (`repo > global > default`).

On `mcp` startup, `[update].config_auto_update = true` (the default) creates a missing `<repo>/.codemap/config.toml` and `<repo>/.codemap/auth.toml`, and keeps repo files synced with the current format (`# codemap-config-version: 32`):

```toml
# codemap-config-version: 32
```

- **Generated template**: Initial repo configs populate common directory exclusions plus recursive globs for detected project types, and set active built-in defaults (commenting out optional examples like `output.max_bytes`, `compilation_database`, `index.mode`, `index.auto_max_files`, and `analysis.target_os`). Comment language is Korean on Korean OS locales and English otherwise.
- **Non-destructive sync**: Updating an existing repo config adds new settings as commented examples and refreshes generated section comments without overwriting your explicit values, `[]` lists, or notes. Supported older key aliases are relocated automatically unless conflicts make migration unsafe.
- **User-owned exclusion list**: Once initialized, `[index.exclude].excluded_directories` is never auto-restored or supplemented if you edit, empty (`[]`), or comment it out.
- **Global files stay untouched**: Global `config.toml` and `auth.toml` are never auto-created or rewritten, though older key aliases still work there. Setting `config_auto_update = false` disables all template creation and automatic repo writes while keeping config loading and live reload active. Symlinked config files preserve their symlinks.

### Manual transition when automatic writes are disabled

If you keep `config_auto_update = false` and edit an older repo config manually, note that an explicit `excluded_directories` array replaces the built-in optional fallback (`node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor`) instead of adding to it. Include any of those directories you still want alongside your custom exclusions and update the marker comment to `# codemap-config-version: 32`. Comment out the repo key if you want to inherit global exclusions again.

## Directory exclusions

`[index.exclude].excluded_directories` defines the complete **optional** directory exclusion list. Explicit arrays are not merged with hidden defaults: `[]` disables optional exclusions, while omitting the key inherits global config or the fallback list.

The common initial list is:

```toml
[index.exclude]
excluded_directories = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index"]
```

`.idea`, `.vscode`, and `.vs` are editable defaults. VCS metadata directories (`.git`, `.svn`, `.hg`, `.bzr`, `.jj`, `.sl`), `.codemap`, `.codemap-index`, and the configured index path always stay excluded from directory walks, even if removed from this list or when `include_ignored: true` is passed.

When no `excluded_directories` array is configured anywhere, the fallback list adds `node_modules`, `.yarn`, `target`, `dist`, `build`, and `vendor`. Freshly generated repo configs instead add recursive globs like `**/node_modules`, `**/build`, and `**/target` for detected project types so sibling and nested packages are covered automatically. Replace broad `**` globs with narrower paths if you have source directories with those names.

| Rule | Meaning |
| --- | --- |
| `build` | Directory basename at any depth |
| `**/build` | Matching directories at any depth within the workspace, including root |
| `./build` | Workspace-root directory only |
| `apps/web/build` | That directory and its descendants |
| `apps/api/**/__pycache__` | Cache directories at any depth under `apps/api` |
| `apps/native/cmake-build-*` | Matching CMake output directories under `apps/native` |

Rules are case-sensitive directory globs (not matched against files) resolved relative to the workspace root. Windows separators normalize to `/`, while absolute paths, `..` traversal, empty strings, or invalid globs reject the entire array and fall back to the lower layer. Bare names also apply inside `allowed_roots` external trees; workspace-anchored paths do not.

These rules apply across indexing, codemap, caller scans, file watching, `find`, and `grep` (and cannot be bypassed by searching from inside an excluded subtree). `include_ignored: true` on `find`/`grep` bypasses optional directory exclusions and `.gitignore`/`.codemapignore`, but never mandatory VCS/index paths. Indexing, caller scans, and watchers also skip git linked worktrees inside the workspace (folders whose `.git` file points into a repository's `worktrees` folder), while `find`, `grep`, and codemap still walk them and submodules remain indexed.

### Initial project detection

Project detection runs only when scaffolding a missing repo config (or upgrading a legacy config). It inspects project marker filenames without running builds, evaluating manifests, or following directory symlinks, adding each matched pattern once globally for the workspace:

| Project markers | Recursive globs applied throughout the workspace |
| --- | --- |
| `package.json` | `**/node_modules`, `**/.yarn`, `**/dist`, `**/build`, `**/coverage`, `**/.next`, `**/.nuxt`, `**/.output`, `**/.svelte-kit`, `**/.astro`, `**/.turbo`, `**/.parcel-cache` |
| `pyproject.toml`, `setup.py`, `setup.cfg`, `Pipfile`, `requirements*.txt` | `**/.venv`, `**/venv`, `**/__pycache__`, `**/.pytest_cache`, `**/.mypy_cache`, `**/.ruff_cache`, `**/.tox`, `**/.nox`, `**/build`, `**/dist`, `**/*.egg-info` |
| `Cargo.toml` | `**/target` |
| `go.mod`, `go.work` | `**/vendor` |
| `pom.xml` | `**/target` |
| `build.gradle`, `build.gradle.kts`, `settings.gradle`, `settings.gradle.kts` | `**/.gradle`, `**/build` |
| `build.sbt` | `**/target`, `**/project/target`, `**/.bloop`, `**/.metals` |
| `*.csproj`, `*.sln`, `*.slnx` | `**/bin`, `**/obj` |
| `composer.json` | `**/vendor` |
| `Gemfile`, `*.gemspec` | `**/vendor/bundle` |
| `Package.swift` | `**/.build` |
| `Podfile` / `Cartfile` | `**/Pods` for `Podfile`, `**/Carthage/Build` for `Cartfile` |
| `pubspec.yaml` | `**/.dart_tool`, `**/build` |
| `CMakeLists.txt` | `**/build`, `**/cmake-build-*`, `**/CMakeFiles`, `**/_deps` |
| `MODULE.bazel`, `WORKSPACE`, `WORKSPACE.bazel`, `BUILD`, `BUILD.bazel` | `**/bazel-bin`, `**/bazel-out`, `**/bazel-testlogs` |
| Directory containing `.tf` files | `**/.terraform` |

SQL, Lua, PowerShell, standalone shell/web/config files, and documentation receive no guessed build exclusions, and custom output paths must be added manually. Notice that `gradle` wrapper/config directories are preserved; only `.gradle` cache directories are excluded.

## When changes take effect

MCP watches `config.toml` and `auth.toml` in config directories that exist at startup (independently of `[index.refresh].watch`) and reloads changes after roughly **1000ms** of debouncing. If a directory did not exist at startup, restart the server after creating it. CLI commands read configuration on invocation.

| Settings | Application point |
| --- | --- |
| `[index.exclude]`, `[output.context.exclude]`, all `[index.language_support]` switches | Reload requests a full index refresh; results update when it finishes |
| All `[output.macro_expansion]` settings | Reload requests a full refresh (even when disabling expansion) |
| Output/caller rendering, `output.is_redact_enabled`, `[output.redact]`, `[filesystem_permissions]` | Subsequent tool requests after reload |
| `index.refresh.index_staleness_ms`, `index.refresh.indexer_auto_restart` | Subsequent refresh and recovery decisions |
| `index.max_file_bytes` | Subsequent walks/refreshes (does not trigger a full refresh on its own) |
| `index.store_references` | Subsequent file parsing (unchanged files reuse stored index data) |
| `index.path`, `index.mode`, `index.auto_max_files`, `index.refresh.watch`, `index.refresh.watch_debounce_ms` | Server restart required |
| `config_auto_update` | Next MCP startup |
| `[global_instructions].enabled` (global-only) | MCP initialization or next request after reload; new agent session needed to read updated file |
| `auth.toml` `[jev].api_key`, `[openrouter].api_key` | Subsequent enabled Jev requests after reload (rebuilds shared HTTPS client) |
| `[output.client].claude_max_result_chars`, `pi_max_bytes` | Delivery limit after reload; reconnect MCP to refresh client tool metadata |
| `[output.client].codex_output_token_limit` | Delivery limit after reload; re-export and merge `codex-config` |
| `[output.client].opencode_max_bytes` | Delivery limit after reload; update `opencode.json` to match |

## Key reference

All numeric keys require positive integers (`>= 1`), except `[output.grep].max_columns`, which also accepts `0`. Older top-level aliases remain accepted for compatibility, with sectioned keys taking precedence within the same file.

Byte-size keys accept either integer bytes or quoted positive integers with `b`, `kb`, `mb`, or `gb` (case-insensitive, powers of 1024, e.g. `"50mb"` = `52428800`, spaces allowed like `"50 MB"`). Fractions, zero, negative values, unknown units, or overflows warn and fall back to the lower layer. Remember to quote unit-bearing strings in TOML (`50mb` without quotes is invalid). This applies to `index.max_file_bytes`, `output.max_bytes`, tool-level `output.*.max_bytes`, `output.client.pi_max_bytes`, `output.client.opencode_max_bytes`, `output.macro_expansion.max_output_bytes`, and `output.jev.max_batch_bytes`.

| Key | Type | Default | Summary |
| --- | --- | --- | --- |
| `[global_instructions].enabled` | bool | `false` | Global-only managed client instruction line; `false` removes tracked lines |
| `[output].is_redact_enabled` | bool | `true` | Mask detected credentials and selected PII in MCP output; indexes and matching keep originals |
| `[output].max_bytes` | integer bytes or size string | unset | Common MCP response ceiling; same-layer tool override wins |
| `[output.client].claude_max_result_chars` | integer characters, 1–500000 | `100000` | Claude `tools/list` metadata and delivery limit (as bytes); reconnect required |
| `[output.client].codex_output_token_limit` | positive integer tokens | `100000` | Codex config export and delivery limit (3.5 bytes per token) |
| `[output.client].pi_max_bytes` | integer bytes or size string | `100000` | `pi` `tools/list` metadata and delivery limit; reconnect required |
| `[output.client].opencode_max_bytes` | integer bytes or size string | `100000` | `opencode` final delivery limit |
| `[output.search].detail_file_limit` | integer | `24` | Top-ranked files rendered in detail before the compact tail |
| `[output.search].overview_file_limit` | integer | `80` | Max file headers in `search`'s compact ranked tail |
| `[output.search].snippet_max_lines` | integer | `500` | Per-symbol snippet line cap in `search` detail view |
| `[output.search].symbol_limit` | integer | `100` | Max symbols rendered per file in `search` detail view (unmatched keys in path/docstring-matched JSON/YAML/TOML are counted only) |
| `[output.search].max_bytes` | integer bytes or size string | `"1mb"` (`1048576`) | Total size cap for one `search` response, including the partial-output notice |
| `[output.search].literal_max_chars` | integer (chars) | `1200` | Max length for matched literals before ellipsis truncation |
| `[output.search].literal_limit` | integer | `60` | Max matched literals rendered per file in `search` detail view |
| `[output.search].anchor_snippet_limit` | integer | `20` | Max full snippets per file; remaining symbols show up to 3 signature lines |
| `[output.read].max_bytes` | integer bytes or size string | `"5mb"` (`5242880`) | Ceiling for `read` and callable-expanded `grep`; oversized `read` errors, expanded `grep` paginates |
| `[output.read].is_outside_callers_enabled` | bool | `true` | Append outside callers, same-key uses, and importing files to `read view=source`; `false` omits all three |
| `[output.grep].max_columns` | integer | `0` | Content-mode column cap (`0` disables); wider rows become `[Omitted long matching line]` |
| `[output.grep].max_bytes` | integer bytes or size string | common budget | `grep` response ceiling; callable-expanded mode falls back to `output.read.max_bytes` |
| `[output.context].is_enabled` | bool | `true` | Default `search` caller/callee annotation setting when `caller_context` is omitted |
| `[output.context].caller_limit` | integer | `1000` | Max callers (or non-call references) shown per symbol |
| `[output.context].callee_limit` | integer | `1000` | Max callees shown per symbol |
| `[output.context].max_bytes` | integer bytes or size string | `"128kb"` (`131072`) | Relationship annotation sub-budget inside `output.search.max_bytes` |
| `[output.context].common_name_threshold` | integer | `2` | Definition count at which name-based relationships carry an ambiguity label |
| `[output.context].caller_omit_def_threshold` | integer | `5` | Definition count at which approximate callers switch to a `grep` suggestion (callees unaffected) |
| `[output.navigation].is_enabled` | bool | `false` | Inspect source structure and mark confirmed call targets `precise` |
| `[output.navigation].callsite_budget` | integer | `1000` | Max call sites checked structurally before falling back to name-based scanning |
| `[output.navigation].scan_limit` | integer | `16000` | Total caller-scan hit cap shared across names (min 25/name) |
| `[output.redact].pii_entities` | string array | `[]` | Case-sensitive PII entity names to mask; see [supported types](/configuration/pii-redaction/) |
| `[output.redact].sensitive_fields` | string array | `[]` | Extra sensitive field names, matched exactly after case/separator normalization |
| `[output.redact].rules` | inline-table array | `[]` | Custom regex rules, each with `id` (`custom.*`) and `pattern` |
| `[output.redact].exceptions` | inline-table array | `[]` | Exact `{ rule_id, value }` pairs exempt from masking |
| `[index].mode` | `"auto"`, `"on"`, or `"off"` | `"auto"` | When MCP indexes the workspace; see [When indexing starts](#when-indexing-starts) |
| `[index].auto_max_files` | integer | `50000` | Max indexable files permitted under `auto` mode |
| `[index].path` | string | `".codemap/index"` | Index directory (absolute or workspace-relative) |
| `[index].max_file_bytes` | integer bytes or size string | `"1mb"` (`1048576`) | Files larger than this are skipped before parse/index |
| `[index].store_references` | bool | `false` | Store non-call reference locations in the index |
| `[index.refresh].watch` | bool | `true` | Enable background filesystem watcher |
| `[index.refresh].watch_debounce_ms` | integer (ms) | `500` | Watcher event batching window |
| `[index.refresh].index_staleness_ms` | integer (ms) | `5000` | Debounce for request-triggered fallback refreshes |
| `[index.refresh].indexer_auto_restart` | bool | `true` | Auto-recover if the background indexer thread stops |
| `[index.language_support].is_document_support_enabled` | bool | `false` | Index `.md` and `.mdx` files |
| `[index.language_support].is_shell_support_enabled` | bool | `false` | Index `.sh`, `.bash`, and `.zsh` scripts |
| `[index.language_support].is_infrastructure_support_enabled` | bool | `false` | Index HCL/Terraform, Dockerfile, and Nix files |
| `[index.language_support].is_interface_support_enabled` | bool | `false` | Index Protocol Buffers and GraphQL definitions |
| `[index.language_support].is_build_support_enabled` | bool | `false` | Index Make, CMake, and Starlark/Bazel build files |
| `[index.exclude].excluded_directories` | string array (relative directory globs) | Common + legacy fallback (recursive globs in generated configs) | Complete optional directory exclusion list; see [Directory exclusions](#directory-exclusions) |
| `[index.exclude].use_git_exclude` | bool | `true` | Honor `.git/info/exclude` (independent of `.gitignore`) |
| `[output.context.exclude].should_include_test_code` | bool | `false` | Include test code in automatic symbol/call context |
| `[output.context.exclude].test_file_patterns` | string array | See Test-code context | Test file globs (`[]` disables path detection) |
| `[output.context.exclude].test_attributes` | language → string array | See Test-code context | Per-language attribute/annotation patterns (replaces lower layer per language) |
| `[output.context.exclude].test_decorators` | language → string array | See Test-code context | Per-language decorator patterns (`[]` disables that language) |
| `[output.context.exclude].test_calls` | language → string array | See Test-code context | Per-language test-call patterns (`[]` disables that language) |
| `[output.jev].enabled` | bool | `false` | Master switch for Jev filtering (`false` keeps all tools local) |
| `[output.jev].scope` | string array | `["search", "read", "grep"]` | Complete list of tools filtered when enabled (`[]` selects none) |
| `[output.jev].model` | string | `"jev-1.14.0"` | Pinned provider model (bare version uses TypeSafe; `author/slug` uses [OpenRouter](#openrouter-decision-models); aliases fail) |
| `[output.jev].timeout_ms` | positive integer (ms), max 7 days | `45000` | Absolute per-call deadline including preparation and queue wait |
| `[output.jev].max_in_flight_requests` | integer, 1–8 | `8` | Max concurrent HTTP requests per evaluation |
| `[output.jev].request_spacing_ms` | integer (ms), >= 100 | `100` | Minimum interval between request starts |
| `[output.jev].max_batch_bytes` | integer bytes or size string, 1–168000 | `168000` | Encoded payload ceiling per batch (oversized single questions fail) |
| `[output.jev].pool_idle_timeout_ms` | positive integer (ms), max 7 days | `30000` | Idle HTTPS connection lifetime |
| `[output.jev].search_filter_min_unrelated_probability` | finite number, `0.5 < value <= 1.0` | `0.70` (provisional) | Minimum "no" probability to omit a body (symmetric threshold on "yes"; between stays uncertain) |
| `[filesystem_permissions].find` | string | `"anywhere"` | Path access policy for `find`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].grep` | string | `"anywhere"` | Path access policy for `grep`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].read` | string | `"anywhere"` | Path access policy for `read`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].allowed_roots` | string array | `[]` | External root paths permitted when a tool policy is `allowed_roots` |
| `[related_terms].enabled` | bool | `false` | Opt in to the `synonyms` tool (no download or setup while `false`) |
| `[related_terms].dataset_path` | string | unset | Optional user-owned sense dataset path used instead of downloading |
| `[update].config_auto_update` | bool | `true` | Scaffold missing repo `config.toml`/`auth.toml` and sync comments on `mcp` startup |

### When indexing starts

`[index].mode` controls when MCP indexes the workspace:

| Mode | Behavior |
| --- | --- |
| `"auto"` (default) | Starts indexing on the first `search`, `read`, `grep`, `synonyms`, or monorepo `initial_instructions` call, only if the workspace is inside a repository (`.git`, `.hg`, or `.jj`, excluding one directly in the home directory) and has at most `[index].auto_max_files` indexable files. |
| `"on"` | Starts indexing immediately at server startup without repository or file-count checks. |
| `"off"` | Never indexes the workspace. |

When `auto` mode skips indexing (for example, in a multi-repo parent folder), `search` returns `Index unavailable:` with the reason and up to three discovered repositories one or two levels down, while `read`, `grep`, and `find` continue serving live filesystem results without indexed declarations or callers. Outside a repository, no `.codemap/` directory is created and a temporary empty index is cleaned up on exit. Because the file limit is checked on the first index-backed call, an oversized repository still creates `.codemap/` at startup; set `mode = "on"` and restart to index it.

A newly opened git worktree without an index copies the most recently updated compatible index from another worktree of the same repository without altering the source index. On its first pass, it re-extracts changed files and native macro-expanded files before serving results.

### Indexing and file exclusions

The user's home directory (`~`) itself cannot be an MCP workspace or explicit `index`/`benchmark` target, though projects beneath it work normally.

`auth.toml`, `.txt`, `*.lock`, known package-manager lockfiles, `*.map`, and minified/bundle files are excluded case-insensitively from indexing, codemap, and caller scans (see [supported languages and file exclusions](/usage/languages/)). They are hidden by default in `find`/`grep` unless `include_ignored: true` is set, while direct `read` and `parse` remain available. Files over `index.max_file_bytes` are skipped, and indexing requires valid UTF-8 (invalid UTF-8 clears stale indexed symbols and `read` reports the first invalid byte offset).

The five `[index.language_support]` flags toggle index-backed discovery and refreshes without affecting live `find`, `grep`, `read`, or direct `parse`. `[index.exclude].use_git_exclude` controls only `.git/info/exclude`; `.gitignore`, global gitignore, and `.codemapignore` always apply unless overridden by `include_ignored: true`.

### Native macro expansion

`codemap-search` automatically runs installed Clang and NASM to preprocess C/C++ and CPP-based assembly macros, and uses NASM expansion listings to map generated `.asm` labels back to their invocation lines. If the toolchain is missing, original declarations remain with an unresolved notice. This models [clangd's compile-commands lookup](https://clangd.llvm.org/design/compile-commands) without starting `clangd` or reading `.clangd` files.

```toml
[output.macro_expansion]
is_enabled = true
compilation_database = "build/compile_commands.json"
clang_path = "clang"
nasm_path = "nasm"
clang_flags = ["-Iinclude", "-DFEATURE=1"]
nasm_flags = ["-Iinclude/", "-felf64"]
timeout_ms = 5000
max_output_bytes = "8mb"
```

| Key under `[output.macro_expansion]` | Type / default | Behavior |
| --- | --- | --- |
| `is_enabled` | bool / `true` | Preprocess indexed C/C++/ASM files and direct `parse`/`codemap` calls; set `false` to disable |
| `compilation_database` | nonempty string / omitted | Workspace-relative or absolute path to `compile_commands.json` (or its directory) or `compile_flags.txt` |
| `clang_path`, `nasm_path` | nonempty string / `"clang"`, `"nasm"` | Executable name on `PATH` or explicit path |
| `clang_flags`, `nasm_flags` | string array / `[]` | Extra arguments appended to build flags (repo array replaces global) |
| `timeout_ms` | positive integer / `5000` | Per-process timeout in milliseconds (NASM runs preprocessing plus listing) |
| `max_output_bytes` | integer bytes or size string / `"8mb"` (`8388608`) | Maximum preprocessed output or NASM listing size |

When `compilation_database` is omitted, `codemap-search` walks parent directories up to the workspace root for `compile_commands.json` or `compile_flags.txt`. JSON databases require an exact canonical file match (`arguments` arrays take precedence over `command` strings, and only the configured Clang/NASM binary runs, never compiler wrappers or build commands). Output/dependency flags are stripped while includes, defines, target, standard, `++`, and `-x` flags are kept. Unsupported flags, response files, and compiler plugins produce an unresolved notice.

Successful expansion replaces source declarations with expanded ones tagged `macro expansion` (retaining original macro definitions), while `calls`, constant references, `read`, and `grep` continue reading the original source lines. Failed expansions fall back to original extraction with `Macro expansion unresolved`. NASM expansion requires `-Le -Lm -Lf` (validated on NASM 2.16.03) and omits `..@` macro-local labels; GNU assembler `.macro` expansion is not supported.

While enabled, edits to native sources, headers, or `compile_commands.json`/`compile_flags.txt` trigger a native reconciliation walk, re-preprocessing only files whose recorded inputs or nearest build-setting candidates changed. The watcher also monitors ignored native headers and build databases. Failed expansions retry when the file, build config candidates, or expansion settings change, or when a previously unindexed header appears (touch the file or database to retry after installing external headers). Native execution was validated on macOS arm64; Windows and Linux execution remain unverified.

### Credential redaction

`[output].is_redact_enabled = true` masks credentials and opted-in PII across MCP responses (source views, indexed literals, declaration/constant previews, and errors) using `[REDACTED]`, or asterisks for shorter secrets, while preserving line breaks and never expanding output bytes.

PII detection is off by default. Enable exact case-sensitive types under `[output.redact].pii_entities` (an explicit `[]` clears inherited PII entities while keeping built-in credential rules). See [PII redaction](/configuration/pii-redaction/) for all 90 types and context rules.

Tree-sitter distinguishes literal assignments and default arguments from variable references and types (`password: string = externalValue` stays visible, while `password: string = "hardcoded-value"` is masked), and inspects static parts of concatenated/interpolated strings. Fallback regex/field rules cover unparsed code, ENV/INI values, and the final JSON-RPC text pass.

Built-in rule IDs:

| Rule ID | Detection |
| --- | --- |
| `field.sensitive` | Sensitive fields like `API_KEY`, `access_token`, `client_secret`, and `password` |
| `token.aws-access-key`, `token.github`, `token.openai`, `token.google` | Tokens matching those prefix shapes |
| `token.slack`, `token.jwt` | Slack tokens and JWT shapes |
| `token.stripe`, `token.gitlab`, `token.npm`, `token.sendgrid` | Stripe secret/restricted keys, GitLab, npm, and SendGrid tokens |
| `credential.authorization`, `credential.bearer`, `credential.url-password` | Authorization Bearer/Basic headers, Bearer values, and URL passwords |
| `private-key.pem` | PEM private-key blocks (missing end markers mask through EOF) |

Custom rules and exact exceptions:

```toml
[output.redact]
sensitive_fields = ["internalCredential"]
rules = [{ id = "custom.acme", pattern = 'ACME_[A-Z0-9]+' }]
exceptions = [{ rule_id = "custom.acme", value = "ACME_EXAMPLE" }]
```

- `sensitive_fields` matches exact normalized names (`internalCredential` matches `internal_credential`), while built-in names keep suffix matching.
- Custom regex `id`s must be unique, start with `custom.`, and use ASCII letters, digits, `.`, `_`, or `-`. Patterns use Rust `regex` syntax (no backreferences or look-around); include a `(?P<secret>...)` capture to mask only that group, or flags like `(?s)` for multiline matches.
- `exceptions` require an exact match on both `rule_id` and the **entire unquoted detected value** (without decoding escapes). Exempting `custom.acme` on `ACME_EXAMPLE` does not exempt `ACME_EXAMPLEPLUS` or `password = "ACME_EXAMPLE"` (matched by `field.sensitive`).
- Each list replaces its inherited layer (`repo > global > default`); invalid entries or empty-matching regexes warn without printing secrets/patterns and fall back for that key.
- Source files on disk, local indexes, search ranking, ordinary CLI commands, and stderr logs are never masked. When a search query has no indexed matches, the response omits the query string so unverified secret fragments are not echoed back.

### Output and call relationships

`search` renders detailed matches for top files followed by a compact tail; pages above 32 KiB condense each top file to match reasons, suggested read ranges, matched declarations, up to two matching lines, and connected locations. Generated files and translation resources rank lower on general queries unless targeted by path, symbol, key, or quoted phrase.

- `output.search.max_bytes` includes the partial-output notice (`search` has no `offset`; narrow the query or `read` suggested ranges). Unused tail budget can hold [exact phrase matches outside the ranking](/usage/tools/).
- `output.search.anchor_snippet_limit` caps full snippets per file; later anchors show up to 3 signature lines.
- `output.read.max_bytes` includes line numbers, context, and headings, and caps callable-expanded `grep` (unexpanded whole-file `read` without `limit` also keeps a 256 KiB ceiling).
- Raising `output.read.max_bytes` does not raise search, annotation (`128 KiB`), live read/grep context (`16 KiB` shared ceiling), or callable expansion parse limits (`min(max_file_bytes, 4 MiB)`).

Call relationships in `search` are governed by `[output.context]` (when `caller_context` is omitted) and `[output.navigation]`:

- Approximate by default. With `[output.navigation].is_enabled = true`, structural import/binding checks confirm single targets as `precise` (`callsite_budget` caps how many sites are checked before falling back to name scans).
- `[index].store_references` records non-call references during parsing and is not required for call resolution.
- Confirmed calls format as `name — file:line`. `read` and `grep` also show `references (same-file constants, approximate)` for bare identifier uses of indexed constants (including JS/TS `const` bindings), previewing up to 240 characters of the initializer without evaluating code.
- At `common_name_threshold` definitions, name-based lists carry an ambiguity warning; at `caller_omit_def_threshold` definitions, approximate caller lists are replaced with a `grep` hint (callees and structurally confirmed callers stay visible).

### Test-code context

`[output.context.exclude]` filters test files and test regions out of automatic `read`/`grep` symbol context, `search` caller/callee annotations, definition-count checks, and event/source-route analysis when `should_include_test_code = false` (default). Direct `read`, `grep`, and `search` hits on test files remain fully accessible. Set `should_include_test_code = true` to include test context (directory and ignore exclusions still apply).

For `test_file_patterns` and each language list in `test_attributes`, `test_decorators`, and `test_calls`, an explicit array **replaces** the inherited list per key (or per canonical language name such as `rust`, `python`, `typescript`, `csharp`). Omit a language to inherit it, pass `[]` to disable that category for that language, or pass `{}` to inherit all language entries:

- `test_file_patterns`: case-sensitive workspace-relative globs (bare patterns without `/` match basenames at any depth; absolute paths, `..`, empty strings, and `!` negations are rejected).
- `test_attributes`: attribute/annotation globs without `#[...]` or `@` (Rust `::` paths stay qualified; `cfg(test)` also handles `all`/`any`/`not`, while retaining `cfg(not(test))`).
- `test_decorators`: decorator globs without `@` or arguments.
- `test_calls`: called-expression globs such as `test` or `describe.*` (marks the call and callback body).

Rules combine with OR and apply to nested helpers inside a test region (without following calls out to ordinary functions). Changes take effect on the next request after reload without rebuilding the index.

Example customizing selected patterns while disabling Java attributes and TypeScript call-name rules:

```toml
[output.context.exclude]
should_include_test_code = false
test_file_patterns = ["**/tests/**", "*_test.go", "*.test.ts", "checks/**"]

[output.context.exclude.test_attributes]
rust = ["test", "tokio::test", "cfg(test)", "company::case"]
java = []

[output.context.exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "company_test"]

[output.context.exclude.test_calls]
typescript = []
```

Built-in default lists (unlisted languages have no defaults for that category):

```toml
[output.context.exclude]
test_file_patterns = ["**/tests/**", "**/test/**", "**/__tests__/**", "test_*.py", "*_test.*", "*.test.*", "*_spec.*", "*.spec.*", "*Test.java", "*Tests.java", "*IT.java"]

[output.context.exclude.test_attributes]
rust = ["test", "tokio::test", "async_std::test", "rstest", "rstest::rstest", "cfg(test)"]
java = ["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Nested", "BeforeEach", "AfterEach", "BeforeAll", "AfterAll"]
kotlin = ["Test", "ParameterizedTest", "RepeatedTest", "BeforeTest", "AfterTest", "BeforeEach", "AfterEach"]
csharp = ["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestFixture", "SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown"]
swift = ["Test", "Suite"]
php = ["Test"]

[output.context.exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "unittest.skip", "unittest.skipIf", "unittest.skipUnless", "unittest.expectedFailure"]

[output.context.exclude.test_calls]
javascript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
typescript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
dart = ["test", "group", "testWidgets"]
ruby = ["describe", "context", "it", "specify"]
powershell = ["Describe", "Context", "It"]
```

### Filesystem permissions

`find`, `grep`, and `read` each accept one policy:

- `workspace`: only the current workspace.
- `allowed_roots`: the workspace plus non-empty paths in `allowed_roots` (relative paths resolve from the workspace root; symlinks are resolved against existing components so `..` cannot escape).
- `anywhere`: any path accessible to the server process (default).

`allowed_roots` does not merge external trees into the start folder's index. Searching an external repository with `workspace_scope` creates/uses that repo's own index only if `read` allows the entire external repository directory; that external repo's own `[filesystem_permissions]` never widens the current session's access.

### Index freshness

When `watch = true`, file edits refresh the index in the background using `watch_debounce_ms` (paths hidden by `.gitignore`, `.ignore`, or `.codemapignore` are ignored unless those ignore files themselves change, which triggers a full walk). When watching is off or unavailable, `search` triggers background refreshes at `index_staleness_ms` intervals and returns the latest committed snapshot right away.

With `indexer_auto_restart = true`, the next `search` restarts a dead background indexer up to a per-run cap; when `false`, search results stay frozen until restart while live `read`/`find`/`grep` keep working. Files whose modification time changes without content changes are recorded in `content-mtimes.json` inside the index folder so they are not reparsed.

## Example `config.toml`

Below is a practical `<repo>/.codemap/config.toml` showing common overrides (see [Key reference](#key-reference) above for all 69 keys and defaults):

```toml
# codemap-config-version: 32
# Override only the keys you want to change; omitted keys inherit global settings or defaults.

[output]
is_redact_enabled = true
# max_bytes = "1mb"

[output.client]
claude_max_result_chars = 100000
codex_output_token_limit = 100000
pi_max_bytes = 100000
opencode_max_bytes = 100000

[output.search]
detail_file_limit = 24
max_bytes = "1mb"

[output.read]
max_bytes = "5mb"
is_outside_callers_enabled = true

[output.redact]
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD"]
sensitive_fields = ["internalCredential"]

[output.jev]
enabled = false
scope = ["search", "read", "grep"]
model = "jev-1.14.0"

[index]
# mode = "auto"           # "auto", "on", or "off" (restart required)
max_file_bytes = "1mb"
store_references = false

[index.exclude]
excluded_directories = [".git", ".codemap", ".codemap-index", "**/node_modules", "**/target"]
use_git_exclude = true

[index.language_support]
is_document_support_enabled = false
is_shell_support_enabled = false

[analysis]
# target_os = "linux"     # Set "" to clear an inherited global target_os

[filesystem_permissions]
find = "anywhere"
grep = "anywhere"
read = "anywhere"
allowed_roots = []

[related_terms]
enabled = false
```

### Workspace-only permissions

Confine all three tools to the workspace:

```toml
[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"
allowed_roots = []
```

### Bounded external roots

Let `find` and `grep` inspect shared external source trees while keeping `read` inside the workspace:

```toml
[filesystem_permissions]
find = "allowed_roots"
grep = "allowed_roots"
read = "workspace"
allowed_roots = ["G:/shared/source", "D:/vendor-src"]
```

### Full-disk access for `find` only

Keep `anywhere` for `find` while restricting `grep` and `read` to the workspace:

```toml
[filesystem_permissions]
find = "anywhere"
grep = "workspace"
read = "workspace"
allowed_roots = []
```

## The `.codemap/` directory and ignore files

- `.codemap/index/` and `.codemap/config.toml` share one repo-local `.codemap/` folder. `codemap-search` always excludes `.codemap/` from walks and never edits your Git files, so add `.codemap/` to `.gitignore` or `.git/info/exclude` yourself to keep it out of `git status`.
- A repo-local `.codemapignore` uses **gitignore syntax** to hide paths from indexing, `find`, and `grep` without touching `.gitignore`.

## Live request controls

To fetch multiple known windows in one call, pass `ranges: [{file_path, offset?, limit?}, ...]` (1–20 items) to `read` without top-level `file_path` or its aliases. Each item follows single-read permissions, extension checks, byte caps, and alias precedence (`file_path > path > file > query`; `offset`/`limit > start_line`/`end_line > start`/`end`, including camel/kebab variants and numeric strings). Default `offset` is `1` and default `limit` is EOF, without inheriting top-level `offset`/`limit`.

Top-level `view`, `expand`, `unresolved`, `include_events`, `debug`, and `include_seen` apply across the batch (item-level display options are ignored). Item failures report inline without aborting the batch. Duplicate history is shared with earlier items in the batch and prior calls on the connection (`include_seen=true` skips duplicate folding only). If a batch hits the output ceiling, the continuation notice gives the 1-based range number and 0-based index of the first deferred item.

These are per-request arguments rather than TOML settings (`search.caller_context` remains independent):

| Argument | Default | Result |
| --- | --- | --- |
| `view` | `"full"` | `full`: symbols + source; `source`: live output only without index context or relations; `definitions`: declarations only; `relations`: anchored identities, calls, and constant references without source |
| `unresolved` | `"list"` | `count` prints the unresolved total only; `list` includes bounded unresolved names (applies to `full`/`relations`) |
| `expand` | Content `grep`: `"callable"`; `read`: `"none"` | `callable` expands to the enclosing named callable from the live UTF-8 buffer; `none` keeps exact matching lines or requested read windows |

In `read`, `expand="callable"` uses the effective `offset`/`start` line and overrides `limit`/`end`, returning the original window with a notice if no supported callable spans that line (parsing caps at `min(max_file_bytes, 4 MiB)`). Attached attributes are included, nested callables pick the innermost boundary, and closures resolve to their enclosing named callable. Composite files, syntax errors, bodyless prototypes, and callables sharing boundary lines with other code return an explicit unsupported notice.

In content-mode `grep`, `expand="callable"` is the default across all views, including `source`. It ignores `-A/-B/-C` while honoring pattern, path, glob, case, type, and exclusion filters. Pagination (`offset`, `head_limit`, `next_offset`) counts unique callable or fallback groups rather than source lines, and `head_limit=0` removes the group limit without lifting the byte cap. Pass `expand="none"` for matching rows, line pagination, and `-A/-B/-C` context. `count` and `files_with_matches` modes skip expansion when omitted and reject explicit `expand="callable"`. Oversized callable bodies are never silently split; re-request the displayed range with `expand="none"` and smaller windows.

## Explicit Rust analysis target

```toml
[analysis]
target_os = "macos"
```

`target_os` is never inferred from the host OS. Repo config overrides global config, `""` clears an inherited value, and invalid identifiers warn and fall back. Reloading takes effect on the next request without rebuilding the index (each response uses one consistent config snapshot).

Rust resolution evaluates `target_os = "value"`, `all(...)`, `any(...)`, `not(...)`, and boolean literals using three-valued logic (`not(target_os="macos")` matches every other explicit target, while missing facts, other keys/flags, and `cfg_attr` stay unknown; plain `cfg(test)` delegates to `[output.context.exclude]`). See the [Rust conditional-compilation reference](https://doc.rust-lang.org/reference/conditional-compilation.html).

Conditions on imports, reexports, declarations, call sites, and parent modules are checked before marking targets `precise`. Structurally confirmed callers survive `caller_omit_def_threshold`, while alias discovery (bounded at 16 rounds / 256 names) remains a candidate filter requiring exact source confirmation.

## Optional Jev decision stages

Jev filtering uses a decision model (TypeSafe Jev or OpenRouter) to drop irrelevant function bodies from `search`, `read`, and `grep` responses before delivery:

```toml
[output.jev]
enabled = true
scope = ["search", "read", "grep"]
search_filter_min_unrelated_probability = 0.70
```

Store API keys in [`auth.toml`](#credentials-authtoml), or use environment fallbacks when neither auth file defines a key:

```sh
export TYPESAFE_API_KEY="<your key>"
export OPENROUTER_API_KEY="<your key>"
```

To route through OpenRouter instead of TypeSafe, pass an `author/slug` model ID:

```toml
[output.jev]
enabled = true
model = "openai/gpt-6-luna-decisions"
```

Jev is disabled by default. `enabled = true` filters only tools in `scope` (`search`, `read`, `grep`; duplicates are ignored, unknown entries reject the list and inherit the lower layer, and `[]` filters nothing). `initial_instructions`, `find`, `analyze`, and CLI subcommands never call Jev.

### Task

When `enabled = true` and `scope` is non-empty, `tools/list` adds a required `task` string parameter to `initial_instructions` (reconnecting MCP refreshes tool schemas, though the server accepts `task` either way). Pass the user's complete request verbatim before calling filtered tools, and call `initial_instructions` again whenever the task changes:

```json
{ "task": "Where is the response byte cap applied?" }
```

| Field | Contract |
| --- | --- |
| `task` | Non-empty string, max 65,536 bytes (user's verbatim request) |

`task` is connection-scoped and resets on `initialize`. Calling `initial_instructions` without `task` preserves the current task; changing `task` resets delivery deduplication history, and passing a blank/non-string `task` returns `isError: true` and clears it. When filtering is active, a missing `task` returns an argument error, while missing credentials or provider failures fall back silently to normal unfiltered tool output. Enabled filtered tools set `openWorldHint: true` (while remaining `readOnlyHint: true`).

Reading the exact range from an omission note returns that source unjudged, and reading strictly within a single function returns its rows unjudged; wider windows are judged as usual. `include_seen=true` bypasses common duplicate folding only.

### Question decisions and evidence

Each judged unit receives a fixed built-in yes/no relevance question asking whether the code handles something the task names (including shared infrastructure such as a queue, bus, store, watcher, or registry in that path). All models receive the same fields (file, kind, name, and masked source), formatted by provider family:

| Model | Request layout |
| --- | --- |
| TypeSafe Jev (bare version like `jev-1.14.0`) or OpenRouter `typesafe/*` | Plain-text state holding the task and candidate group (`[b3]`, etc.), packing consecutive candidates into one batch |
| OpenRouter `openai/*` | One request per candidate so a single OpenAI refusal never fails sibling candidates |
| Other OpenRouter decision models | State holds masked `task_query`; each question carries its candidate as JSON |

`search_filter_min_unrelated_probability` (default `0.70`, provisional) sets the minimum probability of "no" needed to omit a body and the symmetric threshold of "yes" needed to seed follow-up expansion; scores in between stay visible as uncertain.

Partial, stale, oversized, masked, or identity-unverified bodies stay visible, along with non-callable declarations, source locations, and read hints. Omission notes never count as delivered source in [reading-activity metrics](/usage/cli-and-analysis/#interpreting-counts-and-bytes).

Batches run concurrently under one call deadline using `max_batch_bytes` and operating estimates of `28,000` tokens (state + longest question) and `56,000` tokens (state + all questions), leaving 12.5% headroom under 32k/64k limits. Any single unit too large to fit stays visible unjudged; failed batches or missing answers preserve the tool's full unfiltered output. Exact `event_key` lookups bypass Jev.

### Selection before rendering

`search` plans ranked candidates under normal budgets before Jev filters them. Matched bodies protect directly connected supporting bodies (and enclosing bodies protect overlapping spans) without propagating across uncertain or small bodies. Evaluation skips isolated bodies no larger than an omission note and bodies already covered by an unconditionally retained parent.

Only each body's own masked source is sent (no relationships, events, or caller code). After successful filtering (when the index is healthy, primary results are not capped, and `caller_context != false`), `search` may append up to 8 follow-up indexed call sites per positively matched function (within a 4 KiB annotation sub-budget and navigation call-site cap), evaluating up to 8 adjacent candidate functions against the task and tagging related ones `(Jev: related)`.

Omission notes include exact `read` coordinates and drop the omitted body's outline and relation notes (files with all bodies omitted retain only their heading and omission notes). Under tight client delivery caps, protected bodies are deferred last and freed space is not backfilled. Before evaluation, `jev candidate evidence` logs masked paths, ranges, sizes, and hashes without logging source or task text.

### Live read/grep selection

In `full`/`source` views for `read` and `source_grouped` for `grep`, Jev evaluates captured units directly from the tool's live buffer:

- Complete returned function/method bodies (in `grep`, the whole body of a matched function).
- Partial function/method slices cut by a `read` window.
- Contiguous runs of top-level or same-declaration rows outside functions (trimmed of leading/trailing blank lines).

Clipped columns, non-contiguous rows, unverified syntax/identity, unjudgable masked spans, and oversized units remain visible. `definitions`/`relations` views and non-content `grep` modes skip Jev.

Omitted bodies leave `_omitted body:` (`read`) or `_omitted matches:` (`grep`) notes; omitted slices or runs leave `_omitted lines: L<start>-<end> (<label>)`. Adjacent omissions in one file merge into one note naming up to three units plus `+N more`. Up to 8 same-container member or callback links provide auxiliary context, and live pagination never refills omitted bytes from the next page or mixes edited disk buffers.

### Data sent to the provider

Enabled filters send the masked task and each unit's masked source, path, kind, and name over HTTPS to TypeSafe or OpenRouter (and its upstream model provider). Redaction runs beforehand, though unflagged custom secret shapes can still slip through. API keys are sent only in HTTPS authorization headers.

### OpenRouter decision models

Setting `[output.jev].model` to `author/slug` routes exclusively to `POST https://openrouter.ai/api/alpha/decisions` using `[openrouter].api_key` (`OPENROUTER_API_KEY` fallback):

- **Decision models only**: Non-decision or unknown models fail pre-flight with HTTP 400 (`http_status` fallback) and return ordinary tool output.
- **Pinned IDs only**: OpenRouter must return the canonical slug (stripping `:variant` suffixes, optionally with a `-YYYYMMDD` release suffix). Alias models (`~author/slug`) are rejected before sending.
- **Context budgets**: Operating defaults expect `>= 64k` context. Requests over ~32 KB run in TypeSafe's slower long-context mode on OpenRouter; lower `max_batch_bytes` if needed.

OpenRouter decision models verified with `>= 64k` context on `2026-10-24`:

| Maker | Model IDs |
| --- | --- |
| OpenAI | `openai/gpt-6-luna-decisions` |
| TypeSafe | `typesafe/jev-1.14`, `typesafe/jev-1.13` |
| Perplexity | `perplexity/pplx-decider-v1.2`, `perplexity/pplx-decider-flash-v1.2` |
| Nace.AI | `nace-ai/drex-max`, `nace-ai/drex-v2.0`, `nace-ai/drex-v1.5` |
| Respan | `respan/span-02` |
| Upstage | `upstage/solar-decide`, `upstage/solar-decide-flash` |
| Cloudflare | `cloudflare/clef`, `cloudflare/clef-omni` |
| Liquid AI | `liquid/d1` |
| Inception | `inception/mercury-decide` |

(`scaledown/classify-1` rejects object `state` fields used by OpenRouter JSON layouts.) Query `GET https://openrouter.ai/api/v1/models?output_modalities=decisions` for the live catalog.

### Transport limits

| Setting or limit | Contract |
| --- | --- |
| `max_in_flight_requests` | 1–8 concurrent requests |
| `request_spacing_ms` | At least 100ms |
| `max_batch_bytes` | 1–168,000 encoded bytes; smaller explicit limits apply |
| Token estimates | Operating defaults: 28,000 state + longest question; 56,000 state + all questions (`ceil(encoded bytes / 3)`) |
| `timeout_ms` | Positive, max 7 days; shared deadline covers preparation and all batches |
| `pool_idle_timeout_ms` | Positive, max 7 days |
| Batch/response limits | Max 8,192 questions and 128 batches per stage; 4 MiB per HTTP response |
| Retries/redirects | None |

Each stage pins its config snapshot at start; config reloads affect later calls without extending an active deadline.

### Outcomes and diagnostics

| Outcome | Meaning |
| --- | --- |
| `applied` | Evaluation completed (retention rules may still keep all bodies) |
| `bypassed` | Skipped (e.g. missing key, no eligible bodies, index warming, or insufficient room) |
| `fallback` | Evaluation failed (deadline expired, HTTP/rate-limit error, invalid answer, or incomplete projection) |

Bypasses, failures, and all-keep evaluations return byte-identical normal output; missing required `task` returns an argument error. Each run logs one `info` `jev stage` line on `stderr` (`codemap_search::mcp::jev`) with counts, token usage, and timings, never task text, source code, credentials, or error bodies.

For direct Rust usage, see [`codemap_search::jev`](https://github.com/buYoung/mcp/blob/main/apps/codemap-search/src/jev/mod.rs) and the [decision example](https://github.com/buYoung/mcp/blob/main/apps/codemap-search/examples/jev_decisions.rs) (`--mock` runs offline; `--live` sends an API request).

## Indexed event navigation

Event navigation is enabled by default and builds an immutable event map alongside each index generation without running handlers or build scripts:

```toml
[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = []
```

`Source routes` (data storage/consumer edges across 18 languages) appear automatically alongside configured events and distinguish callback candidates, argument transfer, and data consumers. Setting `is_enabled = false` or `include_events = false` suppresses both; `use_builtin_rules = false` and `rules = []` disable only rule-based event matching (`event_key` queries the event map only). See the [source-route contract (Korean)](https://github.com/buYoung/mcp/blob/main/apps/codemap-search/docs/source-routes.ko.md) for full language details.

| Request | Meaning |
| --- | --- |
| `read` / content `grep` (`include_events` omitted or `true`) | In `view=full` or `view=relations`, automatically show events touching returned lines or supporting definitions (no empty notice or wasted budget if none exist; skipped in `source`/`definitions`) |
| `search` (`include_events` omitted or `true`) | Automatically append related events for ranked paths, independent of `caller_context` |
| `read` / `grep` / `search` (`include_events: false`) | Suppress automatic event context for that request |
| `search` (`event_key: "saved"`) | Query an exact event key instead of ranked text search (`query` still required; honors `workspace_scope`) |

Event routing relies on indexed API/import evidence rather than plain `on`/`emit` text matching, separating publishers, registrations, and handlers with static registration conditions (not runtime delivery order or counts). Built-in rules cover named ESM imports of Node `EventEmitter` (`events`/`node:events`), Tauri frontend events (`@tauri-apps/api/event`), and Tauri Rust `AppHandle`/`App`/`Window`/`WebviewWindow`/`Webview` (`Emitter`/`Listener`) calls.

Bus identity requires immutable module allocations propagated through supported relative ESM imports/aliases/reexports; mutable locals, parameters, dynamic keys, runtime DI, and external brokers remain unresolved. Tauri frontend buses scope to the nearest indexed `src-tauri/tauri.conf.json` or `package.json`, and targeted operations match exact static targets and channels.

### Custom event rules

Example rule pair for `export class KnownBus` in `src/known.ts`:

```toml
[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = [
  { id = "known-on", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "on", role = "subscribe", event_arg = 0, handler_arg = 1, bus = "receiver" },
  { id = "known-emit", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "emit", role = "publish", event_arg = 0, bus = "receiver" },
]
```

Selectors require exact `language` (`typescript`, `javascript`, or `rust`), `module` (workspace-relative file or supported import module), `symbol`, and optional `method`, overriding any matching built-in rule. Up to 64 rules are accepted (argument indexes `< 16`, identifiers/targets `<= 256` bytes, non-empty keys `<= 256` bytes without newlines/NUL); duplicates, wildcards, or invalid fields reject the whole list.

| Field | Contract |
| --- | --- |
| `role` | `publish`, `subscribe`, or `unsubscribe` |
| `event_arg` / `event_key` | Exactly one: zero-based key argument index or fixed event key string |
| `handler_arg` | Zero-based callback argument index (required for `subscribe`) |
| `bus="receiver"` | Requires `method` and a proven immutable receiver allocation |
| `bus="argument"`, `bus_arg` | Bus allocation read from the specified argument index |
| `bus="fixed"`, `bus_identity` | Explicit shared-bus assumption (labeled as a config assumption, e.g. bridging a Rust wrapper and JS frontend) |
| `bus="framework"` | Restricted to the recognized Tauri frontend API within an indexed app boundary |
| `target_arg` / `target` | Optional static target argument index or fixed qualifier like `any` (mutually exclusive; fixed targets disallowed on Tauri frontend APIs) |
| `channel` | Optional exact channel name (defaults to `default`) |
| `is_once` | Optional boolean marking one-shot registration |

For example, a Rust wrapper `dispatch_event_json(key, payload)` in `src/shared/output/mod.rs` can use `bus="fixed"`, `bus_identity="application-events"`, `target="any"` paired with a matching frontend rule; the connection is explicitly marked as a user-configured bridge assumption.

### Bounds and freshness

Inputs are UTF-8 only and honor index, directory, Git, and test-context exclusions. Changing test-context, rule, target OS, or exclusion settings triggers a generation refresh, hiding stale endpoints on edited proof files immediately.

Extraction and query limits: `512 KiB` per file, `64 MiB` / `4,096` files per snapshot, `256` endpoints per file and `8,192` per snapshot (shared with Source routes), `8 KiB` per serialized endpoint, 32-step JS/TS binding depth (16-step Rust value recursion), and per-query bounds of `512` candidates, `128` rendered endpoints, and `128` verified files (`4 MiB`). Truncation, stale evidence, and exclusions are reported in output; queries never rescan the full workspace.

## Search display compaction and grouped grep source

`search` compacts presentation annotations after Jev evidence capture: non-call references keep file/line locations without previews, unresolved call names share a line, repeated caveats define `[N1]`-style notes once and reference them afterward, and literal previews already visible inside a full body are omitted. Confirmed relationship locations and source bodies stay unchanged.

`grep` with `view="source_grouped"` groups source rows under one heading per file (supporting both `expand="none"` with `42:match`/`43-context` markers and `expand="callable"` while skipping declaration/relationship analysis). The existing path-per-row `view="source"` output stays unchanged; `source_grouped` applies to content-mode `grep` only.

## Legacy configuration names

Older root and section names remain readable. Canonical names win within the same file, and invalid canonical values fall back to lower layers. Only repo configs are auto-migrated; global configs are left untouched.

| Legacy path | Canonical path |
| --- | --- |
| `tool_output.is_redact_enabled` | `output.is_redact_enabled` |
| `search.result_threshold` | `output.search.detail_file_limit` |
| `search.search_overview_file_limit` | `output.search.overview_file_limit` |
| `search.search_detail_snippet_max_lines` | `output.search.snippet_max_lines` |
| `search.search_detail_symbol_limit` | `output.search.symbol_limit` |
| `search.search_detail_byte_cap` | `output.search.max_bytes` |
| `search.search_literal_max_len` | `output.search.literal_max_chars` |
| `search.search_literal_limit` | `output.search.literal_limit` |
| `search.search_anchor_snippet_limit` | `output.search.anchor_snippet_limit` |
| `tool_output.read_output_byte_cap` | `output.read.max_bytes` |
| `tool_output.grep_max_columns` | `output.grep.max_columns` |
| `caller_context.caller_context_default` | `output.context.is_enabled` |
| `caller_context.caller_list_cap` | `output.context.caller_limit` |
| `caller_context.callee_list_cap` | `output.context.callee_limit` |
| `caller_context.annotation_sub_budget` | `output.context.max_bytes` |
| `caller_context.common_name_threshold` | `output.context.common_name_threshold` |
| `caller_context.caller_omit_def_threshold` | `output.context.caller_omit_def_threshold` |
| `index.index_path` | `index.path` |
| `index.max_file_size` | `index.max_file_bytes` |
| `caller_context.navigation_store_references` | `index.store_references` |
| `refresh.watch` | `index.refresh.watch` |
| `refresh.watch_debounce_ms` | `index.refresh.watch_debounce_ms` |
| `refresh.index_staleness_ms` | `index.refresh.index_staleness_ms` |
| `refresh.indexer_auto_restart` | `index.refresh.indexer_auto_restart` |
| `language_support.is_document_support_enabled` | `index.language_support.is_document_support_enabled` |
| `language_support.is_shell_support_enabled` | `index.language_support.is_shell_support_enabled` |
| `language_support.is_infrastructure_support_enabled` | `index.language_support.is_infrastructure_support_enabled` |
| `language_support.is_interface_support_enabled` | `index.language_support.is_interface_support_enabled` |
| `language_support.is_build_support_enabled` | `index.language_support.is_build_support_enabled` |
| `caller_context.navigation_context_default` | `output.navigation.is_enabled` |
| `caller_context.navigation_callsite_budget` | `output.navigation.callsite_budget` |
| `caller_context.scan_cap` | `output.navigation.scan_limit` |
| `redact.*` | `output.redact.*` |
| `macro_expansion.*` | `output.macro_expansion.*` |
| `event_navigation.*` | `output.event_navigation.*` |
| `analysis.navigation.*` (v18) | `output.navigation.*` |
| `analysis.macro_expansion.*` (v18) | `output.macro_expansion.*` |
| `analysis.event_navigation.*` (v18) | `output.event_navigation.*` |
