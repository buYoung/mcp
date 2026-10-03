# Configuration

[한국어](./configuration.ko.md) | English

Configuration is optional. Add only the keys you want to change; other keys use global settings or built-in defaults.

`output.event_navigation`, `analysis`, and `output.macro_expansion` are optional sections. Event navigation and native macro expansion are enabled by default. Omitting the target OS inherits the global setting; an empty string clears it. Without a configured target, analysis leaves it unknown and never infers the host OS. Explicit settings, including `is_enabled = false`, still win. `output.jev` holds the optional Jev output filters; all stages stay off unless enabled explicitly (see [Optional Jev decision stages](#optional-jev-decision-stages)).

## Section layout and output budgets

Keep behavior settings in `config.toml`, grouped by responsibility, and credentials in `auth.toml`. Generated files describe each setting, its units, inheritance and application point. Settings with concrete built-in defaults are active, including search/read limits, all four client limits, preprocessing, event rules and custom masking lists. Common/grep limit examples, the compilation database path and the target-clearing example remain commented. Active values, including empty lists, override global settings; remove or comment out a key to inherit it.

| Section | Responsibility |
|---|---|
| `output` | Common MCP response byte ceiling and masking switch |
| `output.client` | Claude character limit, Codex per-tool token limit and pi/opencode byte limits |
| `output.overview` | Root statistics and overview responses |
| `output.search` | Ranked files, symbols and snippets |
| `output.read` | Live file reads |
| `output.grep` | Matching rows and expanded callable responses |
| `output.context` | Caller/callee display limits and relationship output budget |
| `output.navigation` | Source-based navigation and caller scanning budgets |
| `output.macro_expansion` | Native preprocessing and generated declaration results |
| `output.event_navigation` | Indexed event/source routes and navigation results |
| `output.redact` | Additional masking rules and exceptions |
| `output.jev` | Optional search/read/grep/non-root overview output filtering; off by default |
| `output.context.exclude` | Shared test exclusions for search relationships, read/grep context and event/source-route analysis |
| `index`, `index.refresh`, `index.language_support` | Storage, refresh and indexed languages |
| `index.exclude` | Shared directory exclusions for indexing, overview, search, caller scans and find/grep |
| `analysis` | Explicit Rust target OS |
| `global_instructions` | User-global opt-in for a managed client instruction line; ignored in repo config |

Only the configuration location changes. Overview and search use indexed files, while find and grep share the directory rules. Direct read does not apply directory exclusions; its automatic context uses `output.context.exclude`.

`output.max_bytes` is optional. Response ceilings resolve as **repo tool override > repo common > global tool override > global common > existing tool default**. With all limits omitted, search remains 1 MiB and read/expanded grep remain 5 MiB. Expanded grep inherits the read ceiling within a layer when neither grep nor common is configured there. `output.context.max_bytes` is a separate relationship sub-budget, additionally bounded by space left in the search response.

Search returns explicit partial output; read requests a narrower range. Overview, find, initial_instructions and unexpanded grep reject responses exceeding an explicitly configured ceiling with a narrowing error. Budgets cover MCP body text, not the JSON envelope or error messages. They do not truncate ordinary CLI parse/index output.

### Client delivery budgets

`output.client` sets the largest result each coding agent passes to its model: codemap-search's final context size. `max_bytes` budgets apply earlier, to candidate capture, Jev input, rendering and pagination; however large they are, every final response must fit the smallest configured client limit. Limits are compared in UTF-8 bytes: Claude characters count as bytes, which never undercounts them; Codex tokens use `floor(tokens × 3.5)`, a headroom estimate rather than tokenization; pi and opencode values are already bytes. All four keys default to `100000` in their respective units, giving a final delivery limit of `100000` bytes with the defaults. Omitted keys inherit global settings or these built-in defaults.

The check runs on the final text after Jev omissions, masking and duplicate folding, for every tool. Ranked search fits before the check: after judgment it keeps matched/uncertain bodies first and replaces lower-priority whole bodies with exact read ranges within `min(output.search.max_bytes, limit − min(512, limit / 8))`, with space reserved for relationships and discovery. The reserve absorbs masking and duplicate markers added after rendering. Candidate/Jev input budgets stay unchanged. Other tools return a narrowing error instead of a result the client would clip: read suggests a narrower window, grep a smaller `head_limit` with `offset` for later pages. Claude metadata and multi-tool Codex exec cell limits remain separate; arbitrary batches can still exceed a client limit.

`output.client.claude_max_result_chars` accepts 1–500000 characters and defaults to `100000`. Each tool advertises `_meta["anthropic/maxResultSizeChars"]` in `tools/list`. Reconnect MCP so the client reloads tool metadata. [Claude Code reference](https://code.claude.com/docs/en/mcp#raise-the-limit-for-a-specific-tool)

`output.client.codex_output_token_limit` accepts a positive token count and defaults to `100000`. `codemap-search codex-config` prints TOML for all `mcp_servers.codemap-search.tools.<tool>.output_token_limit` entries. Use `--server-name NAME` if the registered server has a different ID. Merge the printed fragment into Codex configuration to apply it; the command never writes client files. [Codex reference](https://learn.chatgpt.com/docs/extend/mcp#other-configuration-options)

Codex Code Mode has a separate output budget for each `exec` call. Only connections whose `initialize` request identifies `params.clientInfo.name` as `codex-mcp-client` receive the Codex-specific instructions; other or unidentified clients receive only the shared instructions. The Codex instructions ask the agent to set first-line `// @exec: {"max_output_tokens": N}` using `codex_output_token_limit` as `N` (`100000` by default), and to use the same output budget on `wait`. The budget covers the combined printed results, so larger batches must be split. A lower client `tool_output_token_limit` can still truncate history; the server neither reads nor changes that setting. Reconnect MCP after changing the configured value to refresh the instructions. This guidance applies with Jev on or off, but the agent's compliance is not guaranteed. [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference#configtoml)

The `MCP client delivery context` diagnostic records whether `params._meta.callId` was present, its value only when it matches `exec-` followed by a hyphenated UUID, response bytes and the configured Codex limit. This internal ID pattern is only a Code Mode hint; it does not reveal the exec budget or confirm delivery to model context. It does not change filtering, output limits or duplicate history.

`output.client.pi_max_bytes` and `output.client.opencode_max_bytes` accept integer bytes or size strings and both default to `100000` bytes. pi ([pi-mcp-adapter](https://github.com/nicobailon/pi-mcp-adapter#output-guard) `settings.outputGuard.maxBytes`) and opencode ([`tool_output.max_bytes`](https://opencode.ai/v2/docs/config)) read no server metadata; by default they keep only the leading 51200 bytes of a larger text result and save the rest to a file. Set the client and server to matching values; using the server's `100000`-byte default requires raising the client's limit too. codemap-search never changes client settings. Their line limit (default 2000) is not enforced.

## Files and precedence

Config is read from two layers and merged **per key** as `repo > global > default`. Use the repo file for project-specific behavior; use the global file only for defaults you want across repositories.

| Layer | Path |
|---|---|
| Repo | `<repo>/.codemap/config.toml` |
| Global | `$CODEMAP_HOME/config.toml`, else `~/.codemap/config.toml` |

"Per key" means a repo file that sets only `[output.search].detail_file_limit` still inherits every other setting from the global file (if set there) or the default. Layers are not all-or-nothing.

### Global instructions

`[global_instructions].enabled` is a boolean, defaults to `false`, and is read **only from the global config**. Repo values warn and are ignored. Add the following to `~/.codemap/config.toml` or `$CODEMAP_HOME/config.toml`; no `clients` list is required:

```toml
[global_instructions]
enabled = true
```

On MCP initialization, the server matches the exact `clientInfo.name` and chooses the client's global instruction file using environment variables inherited by the server:

| Client name | Default file | Overrides and existing-file precedence |
|---|---|---|
| `codex-mcp-client` | `~/.codex/AGENTS.md` | `CODEX_HOME`; a non-empty `AGENTS.override.md` takes precedence |
| `claude-code` | `~/.claude/CLAUDE.md` | `CLAUDE_CONFIG_DIR` |
| `pi` | `~/.pi/agent/AGENTS.md` | `PI_CODING_AGENT_DIR`; the first existing file among `AGENTS.override.md`, `AGENTS.md`, `AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD` wins |
| `opencode` | `$XDG_CONFIG_HOME/opencode/AGENTS.md`, normally `~/.config/opencode/AGENTS.md` | `OPENCODE_CONFIG_DIR`; v1 uses an existing `~/.claude/CLAUDE.md` fallback when native AGENTS.md is absent and Claude prompts are enabled; v2 uses AGENTS.md |

These paths follow the [Codex](https://learn.chatgpt.com/docs/agent-configuration/agents-md), [Claude Code](https://code.claude.com/docs/en/memory), [pi](https://pi.dev/docs/latest/configuration), and [OpenCode v1](https://dev.opencode.ai/docs/rules/) / [v2](https://opencode.ai/v2/docs/instructions/) instruction conventions. OpenCode's config-directory override follows its [global path implementation](https://github.com/anomalyco/opencode/blob/dev/packages/core/src/global.ts). Unknown names are skipped. If an OpenCode fallback exists but its version is unavailable, selection is skipped rather than hiding that fallback.

The server appends or updates exactly one line, always in English regardless of locale:

```md
- Prefer codemap-search for code navigation when available. Call `initial_instructions` first and follow the returned guidance. <!-- codemap-search:managed -->
```

`global-instructions.json` in the codemap global directory records the actual paths and owned lines. `enabled = false` removes previously recorded lines across clients, without deleting the files or unmarked user instructions. Existing line endings, file permissions and symlinks are preserved. Existing managed markers without ownership records, edited managed lines and ambiguous duplicates are left unchanged with a warning. Concurrent codemap processes share a registry lock; file updates use atomic replacement and check for intervening edits. Read, write and lock failures warn on stderr and leave MCP available; reconnect to retry after resolving the cause.

Synchronization runs on initialization and the next request after the config watcher reloads a changed toggle. Create a new agent session to load the updated instructions; modifying the file does not guarantee that the current session reloads it or that the agent calls the tool. Repo config templates, migrations and version markers do not manage this global-only preference.

### Credentials (`auth.toml`)

Jev credentials are separate from behavior settings. Put the API key in `[jev].api_key` in `<repo>/.codemap/auth.toml` or `$CODEMAP_HOME/auth.toml` (otherwise `~/.codemap/auth.toml`). Resolution is **repo auth > global auth > the fixed `TYPESAFE_API_KEY` environment variable**. Missing, empty or whitespace-only keys inherit the next source. Read failures, malformed TOML and invalid types warn and fall back without printing credential values or parser source excerpts. Unknown sections/keys are ignored with a value-free warning.

```toml
# .codemap/auth.toml — keep out of version control
[jev]
api_key = "<your key>"
```

With `config_auto_update = true`, MCP startup creates a missing repo `auth.toml` as an empty, localized template; existing files are never overwritten and environment keys are never copied into it. New files use owner-only `0600` permissions on Unix; Windows uses inherited filesystem ACLs. No global auth file is generated. Model, enable flags and request limits remain in `config.toml` under `[output.jev]`.

Every file named `auth.toml` is excluded case-insensitively from indexing/search/overview and default `find`/`grep`. This is not an access-control boundary: direct `read`/`parse` remains available, and `include_ignored: true` bypasses the filename exclusion for `find`/`grep`. Mandatory `.codemap` directory exclusions still apply to walks. Keep credential files out of Git yourself; codemap-search never edits Git ignore files.

## Loading and automatic writes

The current **repository** configuration schema is **29**. This version controls repo templates and migrations only; global config is never stamped or migrated automatically. The marker is a comment:

```toml
# codemap-config-version: 29
```

- Missing files are optional. Malformed TOML discards that file's layer; an unknown key, wrong type or invalid value warns on stderr and falls back for that key. A valid global value wins over the built-in default when the repo value is invalid.
- On `mcp` startup, `[update].config_auto_update = true` creates a missing repo file. Its directory array contains common folders and recursive globs for detected project types; other active values use their built-in defaults. Active repo values override global settings.
- A pre-v6 repo config, including one without a marker, receives a **one-time directory migration**. Existing user rules, the old effective exclusions, common folders and recommended recursive globs are made explicit in its array. Existing entries and comments are preserved; missing values are appended without duplication. The marker advances to the current schema version.
- **From version 6 onward, `excluded_directories` is never automatically regenerated or supplemented.** Deleting an entry, using `[]`, commenting out the key, or adding another project does not cause the array to be restored. This is separate from reading manual edits at runtime.
- Schema updates relocate supported older key names to the current layout while preserving effective values, inheritance, explicit `[]` lists and user comments. Older aliases remain readable, including in the global file. Conflicting or invalid values that cannot be moved safely leave the file unchanged and produce a warning.
- New settings are added as commented examples, not active assignments. Omitted keys use their inherited or built-in defaults; explicit values such as `is_enabled=false` remain effective. Each Jev stage stays off unless its own flag is enabled.
- Schema 28 moves `[analysis.jev]` to `[output.jev]`, preserving filter settings; the new section wins per key when both exist. The old section remains readable for global files or when automatic writes are disabled. The retired `api_key_env` option and its examples are removed during migration; it is no longer read. Move keys from custom environment variables to `auth.toml` or `TYPESAFE_API_KEY`.
- Schema 29 replaces the four `*_filter_enabled` switches with `enabled` and `scope`. An existing repo's active switches are converted to its currently effective selection, including inherited per-tool choices. This one-time conversion makes that selection explicit; afterward, the scope list replaces the global list. Old switches remain readable only for compatibility with unmigrated files; same-layer new controls take precedence. Commented switch examples are removed. Omitted controls in existing files remain inherited; fresh templates write every Jev default as an active assignment, with `enabled=false`.
- Generated descriptions and section placement may be refreshed, but other inactive assignments and user notes are preserved. A current file is not rewritten.
- `config_auto_update = false` disables initial config/auth template creation and config migration writes. It does not disable reads or config watching. The global file is never generated or migrated.
- Korean OS locale selects Korean generated comments; other/unknown locales use English. Both templates have the same keys and values before project discovery.

If the config changes during migration, contains malformed TOML, cannot be written, or uses an unsupported table layout, the server leaves it untouched and warns. Correct the reported problem and restart. For a dotted/inline index table without an exclusion array, add an explicit array before retrying. A symlinked config keeps its symlink.

### Manual transition when automatic writes are disabled

In schema 6, an explicitly configured array replaces the optional default list. Old arrays were additions to built-ins. If you keep automatic writes disabled, include the old optional names (`node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor`) yourself when you want to retain that behavior, then add the common/project rules you need and set the marker to 6. Other settings need no conversion. Back up your config before manually editing it.

Migration can materialize inherited global exclusions into the repo array. Comment out the repo key afterward if you want subsequent global changes to be inherited again. A global array is also a complete list in schema 6; it is never automatically rewritten.

## Directory exclusions

`[index.exclude].excluded_directories` is the complete **optional** directory-rule list. Explicit arrays are not unioned with hidden built-ins. `[]` disables these optional rules; omitting the key inherits the global list or the default. Existing ignore files and mandatory exclusions are independent.

The common initial list is:

```toml
[index.exclude]
excluded_directories = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index"]
```

`.idea`, `.vscode` and `.vs` are editable defaults. VCS internals (`.git`, `.svn`, `.hg`, `.bzr`, `.jj`, `.sl`), `.codemap`, `.codemap-index`, and the actual configured index directory remain excluded from walks even if removed from the list or `include_ignored` is enabled. Direct `read` still follows its filesystem permission policy.

With no configured array, the fallback list is the common list plus `node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor`. Initial generated configs contain common names and **recursive globs** for detected project types, such as `**/node_modules`, `**/build` and `**/target`. Each glob appears once and matches at any depth in the workspace, including sibling projects. To keep a source directory with a matching name, replace the broad glob with narrower rules.

Configs generated by 0.8.1 may contain paths such as `apps/web/node_modules`. Replace such entries with `**/node_modules` when recursive matching is wanted. Existing schema-6 arrays remain user-managed and are not automatically rewritten.

| Rule | Meaning |
|---|---|
| `build` | Directory basename at any depth |
| `**/build` | Matching directories at any depth within the workspace, including its root |
| `./build` | Only the workspace-root directory |
| `apps/web/build` | Only that project directory and its descendants |
| `apps/api/**/__pycache__` | Cache directories at any depth in that project |
| `apps/native/cmake-build-*` | Matching CMake output directories in that project |

Rules are case-sensitive globs matched against directories, not files with the same name. Relative paths resolve from the current workspace, not the directory passed to `find`/`grep`. Windows separators are normalized to `/`; absolute paths, parent traversal, empty strings and invalid globs reject the array with a warning and lower-layer fallback. Bare names also apply to allowed external source trees; anchored workspace paths do not.

The same rules apply to indexing, codemap, caller scans, watcher updates, `find` and `grep`. A direct search inside an excluded subtree does not bypass its ancestors. `include_ignored: true` on `find`/`grep` bypasses optional directory and ordinary file-ignore rules, but not mandatory internal/index directories. Removing a rule does not override `.gitignore` or `.codemapignore`.

### Initial project detection

Common folders and project recommendations are generated only for a missing repo config or its pre-v6 transition. Detection reads names of project files and does not execute a build, evaluate a manifest or follow directory symlinks. It respects ignore files, skips common/generated folders and orphan dependency/cache trees, and supports nested projects. It records recommended outputs as recursive globs even if those folders do not exist yet. Discovered project paths are not recorded, and repeated project types share the same patterns. User-customized build output paths must be added manually.

| Project markers | Recursive globs applied throughout the workspace |
|---|---|
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
| `Podfile` / `Cartfile` | `**/Pods` / `**/Carthage/Build`, respectively |
| `pubspec.yaml` | `**/.dart_tool`, `**/build` |
| `CMakeLists.txt` | `**/build`, `**/cmake-build-*`, `**/CMakeFiles`, `**/_deps` |
| `MODULE.bazel`, `WORKSPACE`, `WORKSPACE.bazel`, `BUILD`, `BUILD.bazel` | `**/bazel-bin`, `**/bazel-out`, `**/bazel-testlogs` |
| Directory containing `.tf` files | `**/.terraform` |

SQL, Lua, PowerShell, standalone shell/web/configuration files and documents do not receive guessed output paths. A detected build system adds its patterns throughout the workspace. If no project markers are detected, only common recommendations are generated. The `gradle` source/configuration directory is retained; only `.gradle` is a cache exclusion.

## When changes take effect

MCP watches `config.toml` and `auth.toml` in the repo/global config directories that exist at startup, independently of `[index.refresh].watch`. It batches config events for about **1000ms**, then reloads the settings. If a directory did not exist or watching could not start, restart the server after creating/editing the config. CLI commands load configuration when invoked.

| Settings | Application point |
|---|---|
| `[index.exclude]`, `[output.context.exclude]`, all `[index.language_support]` switches | Reload requests a full index refresh; results reflect the change when it finishes |
| All `[output.macro_expansion]` settings | Reload requests a full refresh, including when expansion is disabled |
| Output and caller rendering/budgets except `output.client`, `output.is_redact_enabled`, `[output.redact]`, filesystem permissions | Subsequent tool requests after reload |
| `index.refresh.index_staleness_ms`, `index.refresh.indexer_auto_restart` | Subsequent refresh/recovery decisions |
| `index.max_file_bytes` | Subsequent walks/refreshes; changing it alone does not request a full refresh |
| `index.store_references` | Subsequent parsing; unchanged files can be reused from the index even after restart |
| `index.path`, `index.refresh.watch`, `index.refresh.watch_debounce_ms` | Restart required |
| `config_auto_update` | Automatic writes at the next MCP startup |
| `[global_instructions].enabled` (global-only) | MCP initialization or the next request after a reloaded toggle; new agent session to load the updated file |
| `auth.toml` `[jev].api_key` | Subsequent enabled Jev requests after reload; a changed key rebuilds the shared HTTPS evaluator |
| `[output.client].claude_max_result_chars` | Final delivery limit after reload; reconnect MCP to refresh client metadata |
| `[output.client].codex_output_token_limit` | Final delivery limit after reload; re-export/merge codex-config for the client |
| `[output.client].pi_max_bytes`, `[output.client].opencode_max_bytes` | Final delivery limit after reload; no client change |

Manual exclusion changes update file filters and request a full index refresh. Removed files disappear from results and newly included files become searchable when the refresh finishes. If indexing is unavailable, recover or restart the server before checking the results.

## Key reference

This table summarizes supported keys, accepted types, and defaults. Numeric keys require positive integers except `output.grep.max_columns`, which also accepts `0`. The generated template uses the sectioned form shown here. Legacy top-level keys (for example `result_threshold = 5`) are still accepted for compatibility; when both forms appear in one file, the sectioned value wins.

Byte-size keys accept either an integer byte count or a quoted positive integer with `b`, `kb`, `mb`, or `gb`. Units are case-insensitive and use powers of 1024: `"50mb"` = `52428800` bytes. Surrounding whitespace and a space before the unit are accepted (`"50 MB"`). Fractions, zero, negative values, unknown units, and values outside the destination integer range warn and inherit the lower layer. TOML requires quotes around unit-bearing values; bare `50mb` is invalid TOML. This applies to `index.max_file_bytes`, `output.read.max_bytes`, `output.search.max_bytes`, `output.context.max_bytes`, `output.client.pi_max_bytes`, `output.client.opencode_max_bytes`, and `output.macro_expansion.max_output_bytes`; counts and milliseconds still require integers.

| Key | Type | Default | Summary |
|---|---|---|---|
| `[global_instructions].enabled` | bool | `false` | Global-only managed instruction line for the connected client; false removes previously recorded lines |
| `[output].is_redact_enabled` | bool | `true` | Mask detected credentials and selected PII in MCP responses; matching and local indexes retain original data |
| `[output].max_bytes` | integer bytes or size string | unset | Common MCP response ceiling; a same-layer tool override wins |
| `[output.client].claude_max_result_chars` | integer characters, 1–500000 | `100000` | Claude tools/list metadata and final delivery limit (as bytes); reconnect required |
| `[output.client].codex_output_token_limit` | positive integer tokens | `100000` | Codex export and final delivery limit (3.5 bytes per token) |
| `[output.client].pi_max_bytes` | integer bytes or size string | `100000` | pi final delivery limit |
| `[output.client].opencode_max_bytes` | integer bytes or size string | `100000` | opencode final delivery limit |
| `[output.overview].is_stats_enabled` | bool | `true` | Include indexed-file language statistics in repository-root and monorepo project-root `overview` output; `false` omits the section |
| `[output.overview].max_bytes` | integer bytes or size string | common budget | Overview ceiling; no extra cap if common is unset |
| `[output.search].detail_file_limit` | integer | `24` | Number of top-ranked files `search` renders as details before the ranked tail |
| `[output.search].overview_file_limit` | integer | `80` | Max file headers in `search`'s compact ranked tail |
| `[output.search].snippet_max_lines` | integer | `500` | Per-symbol snippet line cap in `search` detail view; bodies longer than this are truncated |
| `[output.search].symbol_limit` | integer | `100` | Max symbols rendered per file in `search` detail view; overflow becomes a summary note |
| `[output.search].max_bytes` | integer bytes or size string | `"1mb"` (`1048576`) | Output size limit in bytes for one `search` response, including the partial-output footer |
| `[output.search].literal_max_chars` | integer (chars) | `1200` | Matched-literal truncation length; longer literals are cut with an ellipsis |
| `[output.search].literal_limit` | integer | `60` | Max matched literals rendered per file in `search` detail view |
| `[output.search].anchor_snippet_limit` | integer | `20` | Maximum full snippets per file; further matches use signatures of up to three lines |
| `[output.read].max_bytes` | integer bytes or size string | `"5mb"` (`5242880`) | `read` and callable-expanded `grep` output ceiling; oversized reads return a narrowing error, expanded grep paginates |
| `[output.grep].max_columns` | integer | `0` | `grep` content-mode column cap; matched lines wider than a positive cap are replaced with `[Omitted long matching line]`; `0` disables |
| `[output.grep].max_bytes` | integer bytes or size string | common budget | Grep response ceiling; expanded bodies can also fall back to read |
| `[output.context].is_enabled` | bool | `true` | `search` caller/callee annotation default when the per-call parameter is omitted |
| `[output.context].caller_limit` | integer | `1000` | Max callers (or non-call references) rendered per symbol |
| `[output.context].callee_limit` | integer | `1000` | Max callees rendered per symbol |
| `[output.context].max_bytes` | integer bytes or size string | `"128kb"` (`131072`) | Call-relationship output size within `output.search.max_bytes` |
| `[output.context].common_name_threshold` | integer | `2` | Defs-per-name count at which caller/callee lists carry an ambiguity label |
| `[output.context].caller_omit_def_threshold` | integer | `5` | Definition count for the same name at which the approximate caller list is replaced with a `grep` suggestion; callees unaffected |
| `[output.navigation].is_enabled` | bool | `false` | Check source structure and mark confirmed call targets `precise` |
| `[output.navigation].callsite_budget` | integer | `1000` | Maximum call sites checked before using approximate name-based scanning |
| `[output.navigation].scan_limit` | integer | `16000` | Caller-scan hit limit, shared across names (minimum 25/name) |
| `[output.redact].pii_entities` | string array | `[]` | Exact PII entity types to enable; see [supported types](./pii-redaction.md) |
| `[output.redact].sensitive_fields` | string array | `[]` | Additional sensitive field names, matched exactly after case/separator normalization |
| `[output.redact].rules` | inline-table array | `[]` | Additional regex rules, each with `id` and `pattern` |
| `[output.redact].exceptions` | inline-table array | `[]` | Exact pairs of `rule_id` and detected `value` to exempt |
| `[index].path` | string | `".codemap/index"` | Index directory; absolute or relative to the workspace root |
| `[index].max_file_bytes` | integer bytes or size string | `"1mb"` (`1048576`) | Files larger than this are skipped before parse/index |
| `[index].store_references` | bool | `false` | Store reference locations other than function calls |
| `[index.refresh].watch` | bool | `true` | Filesystem watcher (autonomous background index refresh) |
| `[index.refresh].watch_debounce_ms` | integer (ms) | `500` | Batching window for watcher events |
| `[index.refresh].index_staleness_ms` | integer (ms) | `5000` | Debounce for the request-triggered fallback refresh |
| `[index.refresh].indexer_auto_restart` | bool | `true` | Auto-recovery when the background indexer thread dies |
| `[index.language_support].is_document_support_enabled` | bool | `false` | Include `.md`/`.mdx` in index, search, overview, codemap, and watcher refreshes |
| `[index.language_support].is_shell_support_enabled` | bool | `false` | Include `.sh`, `.bash`, and `.zsh` in index-backed discovery |
| `[index.language_support].is_infrastructure_support_enabled` | bool | `false` | Include HCL/Terraform, Dockerfile, and Nix definitions |
| `[index.language_support].is_interface_support_enabled` | bool | `false` | Include Protocol Buffers and GraphQL definitions |
| `[index.language_support].is_build_support_enabled` | bool | `false` | Include Make, CMake, and Starlark/Bazel build definitions |
| `[index.exclude].excluded_directories` | string array (relative directory globs) | Common + legacy fallback names; generated files use recursive globs | Complete optional list; see [Directory exclusions](#directory-exclusions) |
| `[index.exclude].use_git_exclude` | bool | `true` | Whether walkers honor `.git/info/exclude` (that source only) |
| `[output.context.exclude].should_include_test_code` | bool | `false` | Include tests in automatic symbol/call context |
| `[output.context.exclude].test_file_patterns` | string array | See test-code context | Test file globs; [] disables path detection |
| `[output.context.exclude].test_attributes` | language → string array | See test-code context | Attribute/annotation patterns; each language list replaces its inherited list |
| `[output.context.exclude].test_decorators` | language → string array | See test-code context | Decorator patterns; [] disables one language’s list |
| `[output.context.exclude].test_calls` | language → string array | See test-code context | Test-call patterns; [] disables one language’s list |
| `[output.jev].enabled` | bool | `false` | Master switch; false prevents all Jev output filtering regardless of scope |
| `[output.jev].scope` | string array | `["overview", "search", "read", "grep"]` | Complete tool selection when enabled; [] selects none, repo list replaces global |
| `[output.jev].model` | string | `"jev-1.13.0"` | Concrete provider model validated against every response; alias names fail validation |
| `[output.jev].timeout_ms` | positive integer (ms), at most 7 days | `45000` | One absolute deadline per tool call, counted from the start of the stage's preparation and including queue time |
| `[output.jev].max_in_flight_requests` | integer, 1 to 3 | `3` | HTTP requests in flight at once (3 is the runtime ceiling) |
| `[output.jev].request_spacing_ms` | integer (ms), at least 300 | `300` | Minimum spacing between request starts (300 is the runtime floor) |
| `[output.jev].max_batch_bytes` | integer bytes or size string, 1 to 168000 | `168000` | Encoded request bytes per batch (168000 is the runtime ceiling); a question that does not fit is an explicit failure |
| `[output.jev].pool_idle_timeout_ms` | positive integer (ms), at most 7 days | `30000` | Idle HTTPS connection lifetime |
| `[output.jev].search_filter_min_unrelated_probability` | finite number, `0.5 < value <= 1.0` | `0.70` (provisional) | Per-criterion false-probability threshold; composed with all/any |
| `[filesystem_permissions].find` | string | `"workspace"` | Path policy for `find`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].grep` | string | `"workspace"` | Path policy for `grep`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].read` | string | `"workspace"` | Path policy for `read`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].allowed_roots` | string array | `[]` | External roots available to tools set to `allowed_roots` |
| `[update].config_auto_update` | bool | `true` | Create missing repo config/auth templates and sync config schema on `mcp` startup |

### Indexing and file exclusions

The user home directory itself cannot be an MCP workspace or an explicit `index`/`benchmark` target; a project beneath it is valid. If neither `HOME` nor `USERPROFILE` is available, the server warns and continues.

`auth.toml`, `.txt`, `*.lock`, known package-manager lockfiles, `*.map`, and minified/bundle files are excluded case-insensitively from indexing, codemap, and caller scans. `find`/`grep` hide them by default but accept `include_ignored: true`; direct `read`/`parse` remains available. See [supported languages and file exclusions](./language-support-checklist.md) for the full list. Files larger than `index.max_file_bytes` are also skipped by indexing. Indexing accepts UTF-8 source only. Invalid UTF-8 removes any stale indexed symbols; `overview` and `read` explain the exclusion with the first invalid byte offset. `read` keeps its replacement-character display and never rewrites the source.

The five `[index.language_support]` switches control indexing, search, overview, codemap, and file-change refreshes. They do not disable live `find`/`grep`/`read` or direct `parse`.

`use_git_exclude` controls only `.git/info/exclude`. Setting it to `false` leaves `.gitignore`, global Git ignores, and `.codemapignore` in effect.

### Native macro expansion

Native expansion automatically attempts C/C++ preprocessor macros and CPP-based assembly macros with installed Clang. NASM `.asm` files use NASM expansion listings to map generated labels to the invocation line. No configuration section is required; use `is_enabled = false` to disable native processes. Missing tools retain original declarations with an unresolved notice. It follows [clangd's compilation-context model](https://clangd.llvm.org/design/compile-commands); it does not start clangd or load `.clangd` configuration.

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
| `is_enabled` | bool / `true` | Automatically attempts preprocessing for indexed C/C++/ASM files and direct `parse`/`codemap`; explicit `false` disables it |
| `compilation_database` | nonempty string / omitted | Workspace-relative or absolute JSON file/directory; a `compile_flags.txt` file is also accepted |
| `clang_path`, `nasm_path` | nonempty string / `"clang"`, `"nasm"` | Installed executable name or path |
| `clang_flags`, `nasm_flags` | string array / `[]` | Arguments appended to the selected build settings; repo arrays replace global arrays |
| `timeout_ms` | positive integer / `5000` | Maximum milliseconds for each native process; NASM runs preprocessing and a listing pass |
| `max_output_bytes` | integer bytes or size string / `"8mb"` (`8388608`) | Maximum bytes for expanded output or NASM listing |

Without an explicit database, source-parent directories are searched up to the workspace root for `compile_commands.json` or `compile_flags.txt`. A JSON database requires an exact canonical file entry; the first matching entry wins. Header commands are not inferred from similarly named files. If no database exists, configured flags and the installed tool's defaults are used. With a database but no matching entry, add an entry or select a `compile_flags.txt` file explicitly.

The [JSON database](https://clang.llvm.org/docs/JSONCompilationDatabase.html) supplies arguments, include paths, definitions and the working directory. Argument arrays take precedence over command strings. Command strings are tokenized without shell evaluation. Only the configured Clang/NASM executable runs; recorded compiler wrappers and project build commands do not. The recorded `++` compiler name and explicit `-x` determine C/C++ parsing. Includes, definitions, target and standard options are retained; output/dependency flags are removed. Unsupported flags, response files, compiler plugins and unsupported language overrides yield an explicit unresolved notice. Toolchain-specific system includes/targets must be supplied explicitly; query-driver discovery and all compiler-option compatibility are not implemented.

Successful expansion replaces source declarations with the active expanded declarations, while retaining macro definitions extracted from the original source. Generated declarations carry `macro expansion` and original file/line ranges. Header declarations are not attributed to the including file. Expanded token columns, calls and constant-reference attribution remain unresolved; these files do not emit guessed definition links or `precise` call labels. `read`/`grep` still show the original source.

Missing tools/headers, timeout, output limits, unsupported expanded syntax and explicit `#line`/`%line` remapping retain original declarations with the reason `Macro expansion unresolved`. NASM requires a version supporting `-Le -Lm -Lf`; validation used 2.16.03. Its listing pass assembles into the null device without running the resulting program. Validated listing labels/globals are projected into declaration input; instruction operands are not reinterpreted by the generic ASM parser. Generated `..@` macro-local labels are omitted. A pinned FFmpeg `x86inc.asm` macro was also checked with explicit architecture/format flags; this is not a full FFmpeg build. GNU assembler `.macro` expansion, other assembler dialects, and every repository's build configuration are not established by these checks.

While enabled, workspace changes reconcile native files in a full refresh. Recorded external headers/build settings are checked on tool requests at most once per second. After failed preprocessing, a newly supplied external dependency may require a refresh or restart. Enabling this feature adds native-process work per eligible file; budget settings are per process, not per repository. macOS arm64 was validated; Windows/Linux execution remains unverified.

### Credential redaction

`[output].is_redact_enabled = true` masks detected credentials and selected PII in MCP tool responses, including source views, indexed literals, declaration/constant previews and error messages. The replacement is `[REDACTED]`, or asterisks for values shorter than that marker. Replacements preserve source line breaks and never increase output size. Disable it with `false`; changes apply after config reload without rebuilding the index.

PII rules are disabled by default. Set `[output.redact].pii_entities = ["CREDIT_CARD", "EMAIL_ADDRESS", "IBAN_CODE"]` to enable those types. Names are exact and case-sensitive; an unsupported name or non-string entry rejects this entire key and falls back to the lower layer. An explicit `[]` disables inherited PII selections while preserving credential rules. See [PII redaction](./pii-redaction.md) for all supported types, exact exceptions, and detection limits.

Tree-sitter distinguishes literal values from references and types in recognized assignments, fields and default arguments. For example, `password: string = externalValue` is preserved, while the value in `password: string = "hardcoded-value"` is masked. Static parts of concatenated and interpolated strings are inspected too. Unsupported syntax, malformed regions and plain text retain name/pattern fallback. Unquoted ENV/INI values include semicolons and spaces.

Detections retain original UTF-8 byte ranges, rule IDs and kinds, without copying secret values into metadata. Masking precedes source windows, snippets and literal truncation, including multiline strings, YAML blocks and PEM interiors. Indexed literals are matched to current source string nodes and locations: only detected ranges are hidden, leaving safe literals on the same line visible. Unverifiable or changed literals are withheld. Grep matching, counts and column limits use original bytes; changed or unavailable reread source is withheld.

Built-in rule IDs identify token shapes, not verified live credentials:

| Rule ID | Detection |
|---|---|
| `field.sensitive` | Values of sensitive fields such as `API_KEY`, `access_token`, `client_secret` and `password` |
| `token.aws-access-key`, `token.github`, `token.openai`, `token.google` | Tokens with the corresponding prefix shapes |
| `token.slack`, `token.jwt` | Slack tokens and JWT shapes |
| `token.stripe`, `token.gitlab`, `token.npm`, `token.sendgrid` | Stripe secret/restricted keys and GitLab/npm/SendGrid tokens |
| `credential.authorization`, `credential.bearer`, `credential.url-password` | Authorization Bearer/Basic values, Bearer values and URL passwords |
| `private-key.pem` | PEM private-key blocks; a missing end marker hides the remaining source |

Custom rules:

```toml
[output.redact]
sensitive_fields = ["internalCredential"]
rules = [{ id = "custom.acme", pattern = 'ACME_[A-Z0-9]+' }]
exceptions = [{ rule_id = "custom.acme", value = "ACME_EXAMPLE" }]
```

`internalCredential` also matches `internal_credential`. Additional names use exact normalized matches; built-in names retain suffix matching. Regex IDs must be unique, start with `custom.`, and contain only ASCII letters, digits, dots, underscores or hyphens. Patterns use Rust `regex` syntax, without backreferences or look-around. A participating `(?P<secret>...)` capture selects the masked range; otherwise the entire match is masked. Specify flags such as `(?s)` for multiline matching.

An exception exempts only its rule and **entire detected source value**, both matched exactly. It does not exempt `ACME_EXAMPLEPLUS`; `password = "ACME_EXAMPLE"` remains covered by the independent `field.sensitive` rule. There are no file/path-wide or substring exceptions. Quoted values are compared without their outer quotes, retaining source escapes without decoding them.

Each list follows repo → global → default precedence; an explicit list replaces its inherited list. `[]` clears the selected list while preserving built-in credential rules; `pii_entities = []` disables the additional PII types. Invalid lists, duplicate custom-rule IDs, invalid regexes and patterns matching the empty string fall back for the entire config key. Warnings omit patterns, exception values and regex parser diagnostics. Schema 15 scaffolds the three customization lists and schema 16 adds the PII selection; comment out a repo key to inherit its global list.

Literal labels in match reasons, anchor maps and ranked tails are also masked before rendering or truncation. When there are no indexed matches, the response omits the input query to avoid echoing a secret fragment whose source context cannot be verified.

The final JSON-RPC text pass reapplies token, credential, selected PII that does not require label context, and custom regex rules. Label-dependent PII and contextual source decisions run on complete originals before formatting. Weak rules are not reapplied to line numbers or references newly placed next to labels by formatting. Protocol IDs, object keys, numeric/boolean values and parent object/array treatment retain their existing contracts.

No additional scanning byte, candidate-count or time limits are introduced. The existing Tree-sitter parse deadline (5000ms) and tool input/output limits remain in effect; incomplete parsing falls back to text detection. Full-context inspection reads matched files into memory, with work and memory usage increasing with file size.

Unrecognized names/formats, encoded values and values assembled through calls can escape detection; ordinary examples may be masked. Source files, persisted indexes, matching/ranking, ordinary CLI commands such as `parse`, and stderr logs are outside masking scope. JSON-RPC responses from the CLI `mcp` command are covered. Other file-reading tools are outside this feature's scope.

### Output and call relationships

Repository-root and monorepo project-root `overview` output includes indexed-file language statistics by default; set `[output.overview].is_stats_enabled = false` to omit the section. For example, `overview(path="apps/api")` counts only that selectable project's indexed physical files. Other folders and file views omit statistics. Missing or changed sources are reported as unavailable; collection beyond 256 uncached files or 64 MiB per request is reported as pending, and subsequent requests reuse completed counts.

`search` shows detailed top matches followed by a compact list. Its size settings limit output; they do not change which files are excluded. General queries rank generated files and translation resources lower, while exact paths, symbols, resource keys, or quoted text can target those files directly.

`output.search.max_bytes` includes the partial-output notice. If results are cut off, narrow the query or read the suggested ranges; `search` has no page-offset parameter. `output.search.anchor_snippet_limit` limits full snippets per file; further matches show signatures of up to three lines.

`output.read.max_bytes` includes line numbers, context, and headings, and also bounds callable-expanded `grep`. Oversized `read` output returns an error with a narrower `offset`/`limit` suggestion. Expanded grep reports oversized bodies or pagination instead of silently splitting a callable. A separate 256 KiB whole-file limit applies to `read` when `limit` is omitted and callable expansion is off. `grep_max_columns = 0` disables the long-line limit; otherwise long matches become `[Omitted long matching line]`. Partial `grep` pages report `next_offset`.

Raising the read cap does not raise the independent search, annotation, or parsing limits. Search defaults to a 1 MiB cap, with a 128 KiB annotation sub-budget. Caller scans use `scan_cap = 16000`, shared across scanned names; 1000-entry caller/callee lists remain upper bounds, not guaranteed output counts. Live `read`/`grep` context also has a fixed shared 16 KiB ceiling. Callable parsing accepts at most `min(max_file_size, 4 MiB)`, which is 1 MiB by default. Larger caps permit larger responses and more rendering work; they do not establish any consuming client's response limit.

Existing explicit repo/global values are preserved when built-in defaults change. Remove, comment out, or update an old override to use the new value; restarting alone does not replace it.

`output.context.is_enabled` applies only when a `search` call omits `caller_context`. An explicit argument wins. Call relationships are approximate by default. With `navigation_context_default = true`, source structure, imports, and local bindings can confirm a single target and mark it `precise`. Calls that cannot be confirmed use approximate results. `output.navigation.callsite_budget` limits the number of call sites checked before using name-based scanning.

`index.store_references` stores reference locations other than function calls; it is not required to confirm call targets. Some structured formats always store references. The setting applies during parsing, so restarting alone may reuse unchanged files without reparsing them.

Resolved `calls` entries include the definition as `name — file:line`, in both approximate and `precise` modes. Ambiguous targets keep their bare names. MCP `read`/`grep` also show `references (same-file constants, approximate)` for direct bare identifiers in the displayed functions. This works with `navigation_store_references = false`: the source tree is checked against indexed constant declarations (including JavaScript/TypeScript `const` bindings). Locations and initializer text are shown without evaluating code; previews longer than 240 characters are shortened. Comments, strings, qualified/imported references, macro token trees, duplicate names, and names with local bindings are omitted. Test exclusions and context byte budgets also apply to these references, with a notice when the budget omits entries.

`output.navigation.scan_limit` is shared across scanned names, with a minimum of 25 hits per name. `output.context.caller_limit` and `output.context.callee_limit` limit each symbol's displayed relationships. `output.context.max_bytes` limits their total output in bytes within `output.search.max_bytes`; source snippets take priority, and omitted relationships are noted.

At `output.context.common_name_threshold` definitions of the same name, approximate relationships carry an ambiguity label. At `output.context.caller_omit_def_threshold`, the approximate caller list is replaced by a note and a `grep` suggestion. This does not suppress callees or prevent a confirmed target from being shown.

### Test-code context

Manage `excluded_directories` and `use_git_exclude` under `[index.exclude]`, and the test-context keys `should_include_test_code`, `test_file_patterns`, `test_attributes`, `test_decorators`, and `test_calls` under `[output.context.exclude]`. Within one file, valid new locations take precedence over `[exclude]` and older `[caller_context]`, `[index]`, and root aliases; language tables inherit missing language entries. Repo → global → built-in precedence still applies between files. The directory rules remain shared by indexing, overview, search, caller scans, and `find`/`grep`; changing either requests a full index refresh after config reload. Direct `read` does not apply directory exclusions.

`should_include_test_code = false` excludes configured test regions from automatic `read`/`grep` symbol context and `search` caller/callee annotations. The filter runs before definition counts, navigation lookup, and caller-scan budgets. Direct `read`/`grep` source, search hits, and the stored index remain available. Set it to `true` to include test context; directory/ignore exclusions still apply independently.

All four rule lists are editable. An explicit list **replaces** its inherited list; it is not added to a hidden built-in list. For the language tables, precedence is per language: repo → global → built-in. Omit a language to inherit, set it to `[]` to disable that category, or copy its default list and add/remove individual patterns. `{}` inherits all language entries. Invalid lists warn and inherit; unknown language names warn and are ignored. Language names use the registered canonical names such as `rust`, `python`, `typescript`, and `csharp`.

- `test_file_patterns`: case-sensitive workspace-relative globs. A pattern without `/` matches the basename at any depth. Absolute paths, parent traversal, empty patterns, and `!` negation are invalid.
- `test_attributes`: attribute/annotation name globs without `#[...]` or `@`. Qualified names and their final annotation/decorator component are checked; Rust `::` paths stay qualified. The Rust entry `cfg(test)` enables conditional-expression checks that prove a region requires test mode, including `all`/`any`/`not`; `cfg(not(test))` is retained.
- `test_decorators`: decorator name globs, without `@` or argument values.
- `test_calls`: called-expression name globs such as `test` or `describe.*`. The matched expression and its callback bodies form a test region.

Rules combine with OR: disabling one marker does not include a file still matched by a path rule or code inside another test region. Test classification extends through the matched region, including nested helpers, but not into ordinary functions called from that region. Rules inspect source syntax; they do not resolve imports/aliases or expand custom macros. Register alias spellings explicitly. Unknown source shapes or unreadable/oversized source files retain their unclassified context.

Changes apply to subsequent requests after config reload without rebuilding the search index. A bounded cache is invalidated by source metadata or changed rules. Existing configured lists are never automatically supplemented with newly introduced defaults.

The following example keeps selected built-ins, adds custom markers, and disables Java attribute and TypeScript call-name detection. Other active rules still apply.

```toml
[output.context.exclude]
should_include_test_code = false
# Replace the complete path list with the patterns you want.
test_file_patterns = ["**/tests/**", "*_test.go", "*.test.ts", "checks/**"]

[output.context.exclude.test_attributes]
rust = ["test", "tokio::test", "cfg(test)", "company::case"]
java = []

[output.context.exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "company_test"]

[output.context.exclude.test_calls]
typescript = []
```

Default lists (languages not listed have no built-in entries for that category):

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

`read`, `find`, and `grep` each accept these policies:

- `workspace`: access only the current workspace (default).
- `allowed_roots`: access the workspace and the listed external roots.
- `anywhere`: access any path available to the server process.

`allowed_roots` entries must be non-empty strings. Relative paths start at the workspace root; absolute paths are allowed. Existing path components are resolved, including symlinks, while nonexistent suffixes are retained. Path traversal cannot extend access beyond the configured roots. These permissions do not widen the indexing scope of `search` or `overview`.

### Index freshness

With `watch = true`, file changes refresh the index in the background. `index.refresh.watch_debounce_ms` combines nearby changes into one refresh. When file watching is off or unavailable, `search`/`overview` request background refreshes at intervals controlled by `index.refresh.index_staleness_ms` and return the last available results immediately.

With `indexer_auto_restart = true`, the next `search`/`overview` attempts recovery if background indexing stops. Recovery attempts are capped per server run. With it disabled, results remain frozen until restart. Live `read`/`find`/`grep` remains available in either case.

## Example `config.toml`

The example below is intentionally explicit. In a real file, you can keep only the settings you want to override.

```toml
# codemap-config-version: 29
# codemap-search settings for this repository. Values here override global settings.
# Delete or comment out a key to use the global setting or the default.
# A list here replaces the global list instead of merging with it. [] empties it.
# Counts, times, and sizes must be integers of 1 or more. Only output.grep.max_columns accepts 0.
# Byte sizes accept b/kb/mb/gb, e.g. "50mb" (1kb = 1024 bytes).
# The server reloads this file when you save it. Restart the server if a change does not apply.

[output]
# Mask credentials found in MCP responses. false turns off all masking.
is_redact_enabled = true

# Response size limit for every tool (default: no limit). A per-tool max_bytes takes priority.
# max_bytes = "1mb"

[output.client]
# Maximum characters in a Claude Code result (1–500000, codemap-search default 100000).
# Reconnect MCP in Claude Code after changing this.
claude_max_result_chars = 100000

# Maximum tokens in a Codex tool result (codemap-search default 100000).
# Set the same value in Codex; `codemap-search codex-config` prints the lines for ~/.codex/config.toml.
codex_output_token_limit = 100000

# Maximum bytes in an MCP result in pi (codemap-search default 100000).
# Set settings.outputGuard.maxBytes in pi-mcp-adapter to the same value.
pi_max_bytes = 100000

# Maximum bytes in a tool result in opencode (codemap-search default 100000).
# Set tool_output.max_bytes in opencode.json to the same value.
opencode_max_bytes = 100000

[output.overview]
# Show per-language file statistics when overview opens the repository root or a subproject root.
is_stats_enabled = true

[output.search]
# Maximum number of top-ranked files shown in detail. Remaining results use a compact list.
detail_file_limit = 24

# Maximum files in the compact result list.
overview_file_limit = 80

# Maximum lines of code shown per matched definition (function, class, etc.).
snippet_max_lines = 500

# Maximum definitions shown per file in detailed results.
symbol_limit = 100

# Maximum characters shown for a string value from the code that matches the query.
literal_max_chars = 1200

# Maximum string values shown per file.
literal_limit = 60

# Maximum definitions per file shown with their full code. The rest show at most 3 lines of their declaration.
anchor_snippet_limit = 20

# Search response limit.
max_bytes = "1mb"

[output.read]
# Read response limit.
max_bytes = "5mb"

[output.grep]
# Maximum length of a line in grep results, in bytes. 0 disables the limit.
# Longer lines are replaced with an omission marker such as `[Omitted long matching line]`.
max_columns = 0

# grep response limit (default: no limit).
# grep with function bodies uses output.read.max_bytes when neither this nor output.max_bytes is set.
# max_bytes = "5mb"

[output.context]
# Show callers and callees in search results. A caller_context argument passed to search takes priority.
is_enabled = true

# Maximum callers shown per definition, including non-call references.
caller_limit = 1000

# Maximum callees shown per definition.
callee_limit = 1000

# Space in the search response for call relationships. Code goes in first; relationships use the space left.
max_bytes = "128kb"

# When a name has at least this many definitions, name-based relationships are marked ambiguous.
common_name_threshold = 2

# When a name has at least this many definitions, suggest grep instead of listing estimated callers.
caller_omit_def_threshold = 5

[output.navigation]
# Analyze imports and code structure to find actual call targets. Found targets are marked `precise`.
# false uses names only.
is_enabled = false

# After checking this many call sites, fall back to name-based estimates.
callsite_budget = 1000

# Maximum search hits checked when finding callers.
scan_limit = 16000

[output.macro_expansion]
# Expand macros in C/C++/assembly code with the installed Clang/NASM.
# If expansion fails, the original declarations are shown.
is_enabled = true

# Path to compile_commands.json (or its folder) or compile_flags.txt, relative to the workspace root.
# If not set, folders from the source file up to the workspace root are searched.
# compilation_database = "build/compile_commands.json"

# Clang executable name (found on PATH) or path.
clang_path = "clang"

# NASM executable name (found on PATH) or path.
nasm_path = "nasm"

# Additional Clang arguments appended after build settings.
clang_flags = []

# Additional NASM arguments appended after build settings.
nasm_flags = []

# Time limit for one preprocessing run, in milliseconds.
timeout_ms = 5000

# Maximum size of preprocessed output.
max_output_bytes = "8mb"

[output.event_navigation]
# Analyze code that sends and receives events and how values flow, and show it in navigation results.
is_enabled = true

# Use built-in event rules such as EventEmitter on/emit. Your own rules still apply when false.
use_builtin_rules = true

# Your own event rules. See docs/configuration.md for the format and examples.
rules = []

[output.redact]
# Additional personal data types to mask, e.g. ["EMAIL_ADDRESS", "CREDIT_CARD"]
# See docs/pii-redaction.md for supported types.
pii_entities = []

# Additional key or variable names whose values are masked, e.g. "internalCredential". Case and symbols such as _ and - are ignored.
sensitive_fields = []

# Your own regex rules. Each entry needs an id and a pattern.
# The id must start with custom. and must not repeat another rule's id. See docs/configuration.md for examples.
rules = []

# Exceptions to masking. Each applies only when both rule_id and value match exactly.
exceptions = []

[output.jev]
# Use TypeSafe Jev to remove code bodies unrelated to the task from results (default: off).
# Register the task first with initial_instructions(task_query, questions).
# Store the TypeSafe API key in auth.toml under [jev].api_key, not here.
# If neither auth file supplies a key, TYPESAFE_API_KEY is used.

# Master switch for all Jev output filters. false keeps every tool local.
enabled = false

# Tools to filter when enabled. [] selects none; this list replaces the global scope.
# Root overview maps remain local. If Jev is unavailable, ordinary output is retained.
scope = ["overview", "search", "read", "grep"]

# Jev model version (default jev-1.13.0). Aliases such as jev-latest are not accepted.
model = "jev-1.13.0"

# Maximum time Jev may use per tool call, including waiting, in milliseconds (at most 7 days, default 45000).
timeout_ms = 45000

# Maximum concurrent requests (1–3, default 3) and minimum gap between requests in milliseconds (300 or more, default 300).
max_in_flight_requests = 3
request_spacing_ms = 300

# Maximum size of one question batch in bytes (1–168000, default 168000). A single larger question fails.
max_batch_bytes = 168000

# How long an idle HTTPS connection is kept, in milliseconds (at most 7 days, default 30000).
pool_idle_timeout_ms = 30000

# Minimum probability for judging a question's answer as "no" (above 0.5, at most 1.0, default 0.70).
# Higher values remove bodies only when Jev is more certain.
search_filter_min_unrelated_probability = 0.70

[output.context.exclude]
# Include test code in call relationships and automatically added context.
# Test code still appears in search/overview declarations and read/grep source when false.
should_include_test_code = false

# Path patterns for test files (* and ** allowed), relative to the workspace. A pattern without / matches file names.
test_file_patterns = ["**/tests/**", "**/test/**", "**/__tests__/**", "test_*.py", "*_test.*", "*.test.*", "*_spec.*", "*.spec.*", "*Test.java", "*Tests.java", "*IT.java"]

[output.context.exclude.test_attributes]
# Markers such as #[test] or @Test that mark test code, per language. Omit #[...] and @; * is allowed.
# Languages not listed use the default list. cfg(test) also covers all/any/not combinations.
rust = ["test", "tokio::test", "async_std::test", "rstest", "rstest::rstest", "cfg(test)"]

java = ["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Nested", "BeforeEach", "AfterEach", "BeforeAll", "AfterAll"]

kotlin = ["Test", "ParameterizedTest", "RepeatedTest", "BeforeTest", "AfterTest", "BeforeEach", "AfterEach"]

csharp = ["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestFixture", "SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown"]

swift = ["Test", "Suite"]

php = ["Test"]

[output.context.exclude.test_decorators]
# Python markers such as @pytest.fixture that mark test code. Omit @.
python = ["pytest.fixture", "pytest.mark.*", "unittest.skip", "unittest.skipIf", "unittest.skipUnless", "unittest.expectedFailure"]

[output.context.exclude.test_calls]
# Function names such as describe(...) or it(...) that define tests.
javascript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]

typescript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]

dart = ["test", "group", "testWidgets"]

ruby = ["describe", "context", "it", "specify"]

powershell = ["Describe", "Context", "It"]

[index]
# Where to store the index. Relative paths use the workspace root. Restart the server after changing it.
path = ".codemap/index"

# Maximum size of an indexed file. Larger files are still available to read/find/grep.
# Raising it can increase CPU, memory, and disk use.
max_file_bytes = "1mb"

# Also store reference locations other than function calls.
# Files already indexed pick up the change only when their content changes.
store_references = false

[index.exclude]
# Folders to exclude from indexing and find/grep. Files opened directly with read are not affected.
# "build" or "**/build" excludes build at any depth; "./build" excludes only the one at the workspace root.
# Workspace-relative paths such as "apps/web/build" work; absolute paths and .. do not.
# The first list includes folders for detected project types; after that, maintain it yourself.
# Version control folders such as .git, .codemap, and the index folder are always excluded.
excluded_directories = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index"]

# Also exclude paths listed in `.git/info/exclude`. `.gitignore`, global gitignore, and `.codemapignore` always apply.
use_git_exclude = true

[index.refresh]
# Update the index automatically when files change. If false, search/overview refresh it when called.
# Restart the server after changing this.
watch = true

# Time to batch file changes, in milliseconds. Longer means fewer refreshes but slower updates.
# Restart the server after changing this.
watch_debounce_ms = 500

# Minimum interval between refreshes when file watching is not used, in milliseconds.
index_staleness_ms = 5000

# Restart indexing on the next search/overview if it stops (a limited number of times).
# If false, search/overview results stop updating until the server restarts.
indexer_auto_restart = true

[index.language_support]
# Choose which file types to index. read/find/grep still work on files that are not indexed.
# Markdown: `.md`, `.mdx`
is_document_support_enabled = false

# Shell scripts: `.sh`, `.bash`, `.zsh`
is_shell_support_enabled = false

# Infrastructure: HCL/Terraform (`.hcl`, `.tf`, `.tfvars`), Dockerfile, Nix (`.nix`)
is_infrastructure_support_enabled = false

# Interface definitions: Protocol Buffers (`.proto`), GraphQL (`.graphql`, `.gql`)
is_interface_support_enabled = false

# Build files: Makefile, `.mk`, CMakeLists.txt, `.cmake`, BUILD, BUILD.bazel, `.bzl`
is_build_support_enabled = false

[analysis]
# OS used for cfg(target_os) conditions in Rust code (default: not set), e.g. "linux", "macos", "windows"
# "" ignores the value from the global settings.
# target_os = ""

[filesystem_permissions]
# Where find, grep, and read may access files.
# "workspace" allows the workspace only, "allowed_roots" adds the allowed_roots paths, and "anywhere" allows any path.
find = "workspace"

grep = "workspace"

read = "workspace"

# Extra paths for tools set to "allowed_roots". Relative paths use the workspace root.
# Symbolic links are checked by their real path.
allowed_roots = []

[update]
# On server start, create this file if missing and add new settings as comments.
# Values and comments you changed are kept.
config_auto_update = true
```

### Default workspace-only permissions

```toml
[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"
allowed_roots = []
```

### Bounded external roots

This example lets `find` and `grep` inspect a shared source tree while keeping `read` confined to the workspace:

```toml
[filesystem_permissions]
find = "allowed_roots"
grep = "allowed_roots"
read = "workspace"
allowed_roots = ["G:/shared/source", "D:/vendor-src"]
```

### High-risk broad access

This intentionally gives one tool full-disk access. Prefer `allowed_roots` unless the MCP server is already isolated by the surrounding environment:

```toml
[filesystem_permissions]
find = "anywhere"
grep = "workspace"
read = "workspace"
allowed_roots = []
```

## The `.codemap/` directory and ignore files

- `.codemap/index/` (the index) and `.codemap/config.toml` live under one repo-local `.codemap/` directory. codemap-search never walks `.codemap/` (it is a built-in exclude), so it is never indexed — but to keep it out of `git status`, add `.codemap/` to your repo's `.gitignore` (or `.git/info/exclude` for a local-only, uncommitted ignore). The tool does not write to your git files.
- A repo-local `.codemapignore` uses **gitignore syntax** to hide paths from indexing, `find`, and `grep` — the codemap-search-specific complement to `.gitignore`.

한국어 요약: `.codemap/`에는 저장소별 색인과 설정이 함께 있습니다. 도구가 git ignore 파일을 직접 수정하지는 않으므로, `git status`에서 숨기려면 사용자가 `.gitignore`나 `.git/info/exclude`에 `.codemap/`을 추가해야 합니다.

## Live request controls

These are request arguments, not persistent TOML settings. Content-mode `grep` defaults to callable expansion; `read` keeps its requested line window by default. `search.caller_context` retains its independent meaning.

| Argument | Default | Result |
| --- | --- | --- |
| `view` | `"full"` | `full`: symbols plus source; `source`: live output only, bypassing indexed context and relation preparation; `definitions`: declarations only; `relations`: anchored target/owner identities with calls and constant references, without source. |
| `unresolved` | `"list"` | `count` keeps the same unresolved total without formatting individual names; `list` includes the bounded names. Applies to full/relations. |
| `expand` | Content `grep`: `"callable"`; `read`: `"none"` | `callable` resolves a supported named callable from the live UTF-8 buffer; no stale indexed bounds or guessed body. Explicit `none` preserves matching rows or the requested read window. |

In `read`, callable expansion uses the effective offset/start alias and overrides limit/end. Outside a supported callable, the original window is returned with an unavailable notice. Expansion parsing is limited to `min(max_file_size, 4 MiB)`; too-large input is refused before parsing. Attached attributes are included, nested named callables select the innermost boundary, and anonymous closures use their enclosing named callable. Composite files, unparseable bodies, prototypes without bodies and callables sharing boundary lines with other code receive an explicit unsupported notice.

In content-mode `grep`, callable expansion is the default for every view, including `source`. It ignores `-A/-B/-C` while preserving pattern, path, glob, case, type and exclusion behavior. Matching, parsing and rendering use one buffer per file. Output is ordered by path/range; `offset`, `head_limit`, and `next_offset` count unique callable groups (or matched-line fallback groups), not source rows. `head_limit=0` removes the group-count limit, not the byte cap. Set `expand="none"` for matching rows, line-based pagination and `-A/-B/-C` context. `count` and `files_with_matches` do not expand when the option is omitted; explicit `expand="callable"` is rejected in those modes.

Read and expanded grep retain `output.read.max_bytes`; annotations retain their existing budgets. Oversized callable bodies are never silently split: use the displayed source range with `expand=none` and line windows. Grep column omissions are marked as incomplete. Macro/encoding/test-context notices remain in applicable context views; `source` contains only live filesystem output and operational expansion notices. Eligible event relationships appear automatically in full/relations views.

## Explicit Rust analysis target

```toml
[analysis]
target_os = "macos"
```

`target_os` is an optional OS identifier, never inferred from the running host. Repo settings override global settings; an empty string explicitly clears an inherited value. Invalid types/identifiers warn and fall back to the lower layer. Config reload takes effect in the next request's fresh source/condition resolver; it does not require a new source index.

Rust lookup evaluates `target_os = "value"`, `all(...)`, `any(...)`, `not(...)`, and boolean literals with three-valued logic. Thus `not(target_os="macos")` applies to every explicitly non-macOS target, not only Windows. Missing OS facts, other keys/flags, `cfg_attr`, and unsupported string/token forms remain unknown. Plain `cfg(test)` is delegated to the existing test-context filter; enabling test context does not claim a Cargo test build. See the [Rust conditional-compilation reference](https://doc.rust-lang.org/reference/conditional-compilation.html).

Conditions on imports, reexports, declarations, call sites and discoverable parent modules are checked before confirming a definition. Module membership outside the bounded source/path model remains unknown. Source-confirmed callers survive the name-count threshold; ambiguous name-only callers remain suppressed. Callsite and alias-name limits retain already-proven entries and report incomplete resolution. Alias discovery is a bounded candidate filter (16 rounds, 256 names), not proof: each displayed link still needs exact source identity. An explicit target is shown in relation output; this is static navigation, not a build/runtime guarantee.

One response uses one configuration snapshot; a concurrent reload applies to subsequent requests.

## Optional Jev decision stages

```toml
[output.jev]
enabled = true
scope = ["overview", "search", "read", "grep"]
search_filter_min_unrelated_probability = 0.70
```

Store the key separately in [`auth.toml`](#credentials-authtoml). Existing environment-based setups still work when neither auth file supplies a key:

```sh
export TYPESAFE_API_KEY="<your key>"   # request-time fallback; never copied into auth.toml
```

Jev is off by default. `enabled=true` permits filtering only for tools in `scope`; `enabled=false` disables it for every tool, regardless of the list. `scope` accepts `overview`, `search`, `read` and `grep`, defaults to all four, and replaces the inherited list rather than merging with it. `[]` selects none. Duplicate names are ignored; unknown names or non-string entries reject the whole scope value and inherit the lower layer. `enabled` and `scope` independently follow repo > global > default precedence. Root `overview`, `find`, `analyze`, task registration and the CLI do not invoke Jev.

### Task registration

When any Jev filter is enabled, the main agent derives focused yes/no questions from the user's task and registers them once through `initial_instructions`. A valid registration requires `task_query` and `questions`; legacy text-only registration returns an error result (`isError: true`). Register again when the task changes. `search.query` remains the retrieval query and never replaces task intent. With Jev disabled, `initial_instructions({})` remains valid.

```json
{
  "task_query": "Where is the response byte cap applied?",
  "questions": [{
    "question": "Does this function implement or concretely support the response byte cap?",
    "when_true": "The supplied source computes, reserves, enforces or passes this response byte budget.",
    "when_false": "The supplied facts establish a separate behavior with no concrete role in that budget; missing cross-file evidence alone is uncertain."
  }],
  "match": "all"
}
```

| Field | Contract |
| --- | --- |
| `task_query` | Nonempty goal; bounded by the whole registration |
| `questions` | 1–8 independent yes/no criteria; preserve the requested target, direction and coverage |
| `id` | Optional legacy label, ignored; omit it. Routing IDs are generated by the server |
| `question`, `when_true`, `when_false` | Required nonempty strings; no separate per-field byte cap. True means the criterion matches |
| `match` | `all` (default) or `any`; no nested expressions |
| Whole registration | At most 65,536 encoded JSON bytes; question fields do not accept nested values |

Registration is atomic and connection-local. Invalid replacement clears the previous task, and `initialize` resets it. No separate question-generation service is used. Missing registration is an argument error, while unavailable credentials or evaluation failures preserve the tool's ordinary output. Common delivery deduplication still applies afterward.

Each enabled search/read/grep/overview filter advertises `openWorldHint: true`; every tool remains read-only. Root overview, find and analyze do not invoke Jev. `enabled` and `scope` govern these hints as well as runtime filtering. The retired `overview_enabled` key still warns and is ignored. An omission's read location does not bypass the enabled read filter. `include_seen=true` bypasses only common delivery deduplication. Find is not deduplicated.

### Question decisions and evidence

Each eligible function is evaluated against every registered criterion with Noul. The function body appears once in a group state, with identity, source range, completeness, masking status and bounded static call evidence. Missing cross-file or channel evidence is explicit and must not be treated as proof of irrelevance. Questions refer to their named candidate and the same flow; two unrelated true properties do not prove a connection.

For a relevance criterion, one qualifying branch or callback can establish contribution even in a mixed-purpose function. A wrapper or setup function can contribute through a source-backed connection without implementing the whole flow or naming the target. The shared policy separates the user's task from retrieval arguments and asks Jev to inspect the candidate's body first, then its connected support. Each question independently states the requested target, relationship and scope. Registration guidance asks for one coherent relationship per question, merges equivalent questions and groups spelling variants. Independently useful roles should not be packed into a long checklist merely to reduce the count. The 1–8 question limit is a capacity, not a target count.

`search_filter_min_unrelated_probability` retains its negative meaning: default `0.70` is the minimum probability of a criterion being false for a no decision. A yes requires the same threshold on the positive answer; values between the two are uncertain. `all` fails on any decisive no and matches only if every leaf is yes; `any` matches on any decisive yes and fails only if every leaf is no. All other combinations are uncertain. These are discrete decisions, not calibrated joint probabilities. The threshold remains provisional.

Partial, stale, oversized, masked or identity-unverified bodies remain visible, as do uncertain judgments. Non-callable declarations, source locations and read hints stay. Omission notes never count as delivered source in [reading-activity metrics](./analysis.md#interpreting-counts-and-bytes).

Evidence is split before dispatch into groups of at most eight candidates. The same preflight and serializer as the evaluator enforce `max_batch_bytes` and operating estimates of 28,000 tokens for state plus the longest question and 56,000 tokens for state plus all questions. These leave 12.5% headroom below the provider's 32k/64k limits; the byte-based estimate is not the provider tokenizer. Questions split before exceeding either budget. A single candidate that cannot fit stays visible while other candidates are evaluated. Bodies and direct caller/callee source occur once per group. At most 128 groups share one absolute deadline, caller cancellation and the evaluator pool. A missing answer or failed required group preserves the entire base response of that tool. Exact `event_key` maps bypass selection.

### Selection before rendering

Search first plans the same ranked candidates and source windows under the ordinary output budgets. Jev selects from these source blocks before the final fenced bodies and large result strings are assembled. Disabled search remains byte-identical for matched inputs, and a failed evaluation returns the complete ordinary plan.

Uncertain, partial and identity-unverified source stays. A matched function can protect directly connected supporting bodies, and overlapping retained source keeps its enclosing body. Retention does not spread transitively from uncertain, small or already protected functions through an entire connected component.

Evaluation is skipped only when neither that candidate's own body nor a neighboring candidate's support decision can benefit. This includes isolated bodies no larger than their omission note and bodies already covered by an unconditionally retained parent. A small function can still be evaluated when its answer could protect another body. Bypass reasons and eligible/omitted/protected counts are recorded.

Each candidate receives its prepared annotations and scoped event evidence independently, without a search-wide 16 KiB prefix quota or 1 KiB per-part caps. Direct caller/callee source uses ordinary read permissions and full-file redaction, checks the declaration identity and complete returned range, and is deduplicated in `state.supporting_sources`. Named references connect each candidate to this evidence without claiming verified runtime dispatch. Shared evidence policy is stored once, and question instructions explicitly reference it and their candidate. If a complete encoded candidate request cannot fit, that candidate remains unjudged and visible. Bounded supplementary annotations or event context may be omitted with `is_context_clipped` set; that coverage flag is passed to Jev and does not itself force retention. The body-only preliminary bound is derived from the whole state-plus-question budget; final eligibility uses complete encoded requests. `caller_context=false` disables extra caller/callee reads and caller event anchors; `include_events=false` disables event evidence. Warming, stopped or failed snapshots bypass evaluation.

After successful filtering, search may append indexed call-name candidates outside the displayed source, seeded only by functions with a positive composed task match. Declarations retained solely for missing or uncertain evidence do not seed expansion. These are discovery hints, not verified edges or delivered source. They use only the remaining output budget, up to 4 KiB within the annotation budget, at most eight sites per name within the caller limit, and the configured navigation call-site budget. Scope and context exclusions apply; capped lists explicitly direct the caller to grep. Disabled, failed, stale, capped-primary and `caller_context=false` searches do not add them.

Compact omission notes preserve source locations and state that a read location does not bypass Jev. Where omissions free enough room, the summary also identifies retained uncertainty. No space is reserved for score tables. Diagnostic byte counts distinguish planned primary output, returned text and delivered source; omission notes are not source observations. Before evaluation, `jev candidate evidence` maps each group/candidate index to its masked path and source range, body/context sizes and hashes, and coverage flags. It does not log source text or task text, and is not part of the main tool response.

### Live read/grep selection

Read and grep capture callable bounds and returned source rows from the exact buffer used by the tool. Complete returned function/method bodies are eligible in full/source views and grep source_grouped. Partial windows, clipped columns, unverified syntax/identity and oversized bodies remain visible. Declaration/relation views and grep file/count modes are not evaluated.

Body completeness and supporting-context coverage are separate. Live selection captures value/type references from the same parsed buffer independently of the optional persisted reference index. Same-file callers, callees, referenced declarations and enclosing member contracts provide masked evidence. Capture starts with the candidate's own bindings and contracts, then traverses support breadth-first within the existing budgets so a caller's dependency chain cannot consume them before the candidate is visited. Imports, unresolved calls, ambiguous declarations, extraction gaps and exhausted support budgets are described in the Jev input through supporting notes, `has_missing_context` and `is_context_clipped`. These flags neither skip evaluation nor force retention. Jev assesses whether each gap matters to the registered criterion; an unrelated gap does not preclude a decisive answer.

Before classification, bounded indexed candidates supply possible cross-file callers, callees and imported declarations. The candidate's explicit call/import dependencies are captured before possible callers share the remaining budgets. Existing captured buffers are reused; at most eight other files are read within ordinary read permissions and size limits. Current calls or declarations must match the indexed locations. Context exclusions, scan/link/byte budgets and the caller's absolute deadline and cancellation apply. Excerpts include source bodies or explicitly partial declaration contracts. Referenced enclosing members take priority over other member contracts. Supporting declarations include import bindings referenced by the supplied excerpts; glob and unnamed imports remain because a local spelling cannot establish their scope. Name/import hints remain unverified target candidates; they are not automatic evidence of task relevance. These Jev-only excerpts do not expand the main response's source window.

Complete, identifiable bodies proceed to the ordinary Noul judgment when the encoded request fits. A decisive non-match can omit a body even when supporting evidence has gaps; the model must assess the supplied facts and relevance of those gaps. Existing protections for partial/unidentified bodies, oversized requests, uncertain model judgments and overlapping or directly linked retained source remain separate. Diagnostics count actual evaluated candidates and distinguish `matched`, `no_match`, `judged_uncertain` and `unjudged_bodies` from the final `protected`/`linked` outcomes. These are integration and policy contracts, not a measured improvement in classification accuracy or token use.

The registered task, all/any decisions, uncertainty retention, linked-body protection, request budgets and absolute deadline are shared with search. Live rendering and pagination finish before evaluation. Omissions only replace captured spans: no reread after await, no mixing changed file versions, and no refill from the next page. Independent common deduplication follows Jev and excludes find.

### Data sent to TypeSafe

Enabled search/read/grep/non-root overview filters send the masked registered goal and questions, tool arguments, eligible source bodies and their bounded supporting evidence. Redaction runs before transmission; it cannot detect every sensitive format. The API key is confined to HTTPS authorization. The default provider model remains `jev-1.13.0`.

### Transport limits

| Setting or limit | Contract |
| --- | --- |
| `max_in_flight_requests` | 1–3 concurrent requests |
| `request_spacing_ms` | At least 300ms |
| `max_batch_bytes` | 1–168,000 encoded bytes; a smaller explicit value remains effective |
| Token estimates | Operating defaults: 28,000 state + longest question; 56,000 state + all questions. `ceil(encoded bytes / 3)` is an estimate, not exact usage |
| `timeout_ms` | Positive, at most seven days; one deadline includes preparation and all batches |
| `pool_idle_timeout_ms` | Positive, at most seven days |
| Batch/response limits | At most 8,192 questions and 128 batches per evaluation; 4 MiB per response |
| Retries/redirects | None |

Invalid configured values warn and fall back to the lower configuration layer or default. Each stage pins its configuration at the start; changes affect later requests and do not extend a running deadline.

The runtime estimates tokens at one per three bytes against the provider's 64k state-plus-all-questions and 32k state-plus-longest-question limits. This is not a tokenizer guarantee. Provider rejection preserves the base output instead of silently truncating evidence.

### Outcomes and diagnostics

| Outcome | Meaning |
| --- | --- |
| `applied` | Evaluation completed; retention rules may still keep every body |
| `bypassed` | No evaluation, for example missing credentials, no complete bodies, index warming or insufficient output room |
| `fallback` | Evaluation failed, for example deadline expiry, rate limiting, invalid/incomplete answers or incomplete projection |

Bypasses, failures and all-keep filters preserve the base output byte for byte. Missing required task registration instead returns an MCP argument error. Automatic filtering does not guarantee an HTTP request when no eligible evidence exists.

Each executed stage records a `jev stage` line on stderr at `info` level (`codemap_search::mcp::jev`). It includes tool, outcome/reason, model and evidence versions, coverage/judgment/omission counts, known token usage, responses without usage, attempted requests and elapsed/HTTP/queue times. It does not log the task, source evidence, credentials or provider error bodies. Disabled stages log nothing.

For direct Rust integration, see [`codemap_search::jev`](../src/jev/mod.rs) and the [decision example](../examples/jev_decisions.rs). The example's `--mock` mode is offline; `--live` uses an API key and sends a real request.

### Jev on non-root overview

With `enabled=true` and `"overview"` in `scope`, Jev selects declaration rows in file and subfolder views. The repository root, its aliases and its absolute path return the ordinary local map without registration or external calls. A child workspace in a monorepo is a non-root path. Folder views judge declarations in up to eight immediate files; subdirectory maps, file entries and indexed statistics remain unchanged.

Private classification source follows read permissions, redaction and test-code exclusions, with its digest checked against the committed index. Files are not reread after inference. Unavailable or over-budget evidence stays unjudged, which is not counted as successful classification. Uncertain declarations remain; only unrelated declarations are removed. Retaining a parent does not retain all children, while a retained child preserves its enclosing declaration structure.

Classification bodies are neither returned by overview nor recorded in common source delivery history, so later reads do not fold them as already delivered. Supporting evidence for read, grep and overview includes up to eight same-container member assignment/use or callback-reference links. These are evidence for Jev, not automatic retention rules or proof of runtime dispatch.

## Indexed event navigation

Event navigation is enabled by default and needs no extra request option. It stores bounded source inputs with the symbol index and builds a separate immutable map before publishing that generation. It does not execute handlers or build scripts. The following configuration shows the defaults; it is not required to enable the feature.

```toml
[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = []
```

Source-derived storage/consumer relations for 18 programming languages appear separately as `Source routes`. They work without built-in or custom event rules and distinguish callback candidates, argument transfer and data consumers. `include_events=false` and `is_enabled=false` suppress both maps; `use_builtin_rules=false` and `rules=[]` do not disable Source routes. `event_key` queries only the configured event map. See the [source-route contract (Korean)](source-routes.ko.md) for language coverage, conditions and limits.

`is_enabled` and `use_builtin_rules` both default to `true`. Explicit `is_enabled=false` disables event collection and automatic output; existing values are preserved. Repo keys override global keys. The entire `rules` list replaces the lower layer; `[]` removes inherited custom rules. Set `use_builtin_rules=false` to disable the built-in catalog. Invalid lists warn and use the lower layer. Rule, analysis-target and exclusion changes trigger a generation refresh. Until it is ready, queries hide stale event links. Changes to imported keys, bus bindings and handlers rebuild their dependent routes. Original application files are never rewritten.

| Request | Meaning |
| --- | --- |
| `read` / content `grep`: omitted `include_events` or `true` | In `view=full` or `view=relations`, automatically show eligible events overlapping the returned lines or supporting definitions. No related event means no event section, empty-result notice or reserved event output space. `source` and `definitions` skip event lookup. |
| `search`: omitted `include_events` or `true` | Automatically append related events for ranked result paths; independent of `caller_context`. No eligible events means no event section or reduction of the normal output budget. |
| `read` / `grep` / `search`: `include_events: false` | Suppress automatic event context for this request. Non-content grep keeps its existing result shape; explicit `true` still requires content mode. |
| `search`: `event_key: "saved"` | Query that exact event key instead of ranked text search. `query` remains required. Existing workspace selection and `workspace_scope` apply. |

Automatic routing uses the indexed API and source-location evidence, not matching `on`/`emit` text. Recognized unresolved endpoints still show their reasons. Excluded or stale anchors cannot display a route merely because another endpoint remains eligible. Explicit `event_key` queries retain empty/disabled/not-ready diagnostics for investigating coverage; automatic context omits them.

The map separates publishers, subscription registrations and handler definitions, and lists their original file/line, API rule, bus/key evidence and conditions. A route means static registration evidence. It does not establish delivery, order, active subscription count or removal timing. `unresolved=list/count` continues to govern direct callee names; event uncertainty is reported separately.

The initial built-in catalog covers named ESM imports of Node `EventEmitter` from `events`/`node:events` (`on`, `addListener`, `once`, `emit`, `off`, `removeListener`) and Tauri frontend `listen`, `once`, `emit`, `emitTo` from `@tauri-apps/api/event`. Tauri Rust `AppHandle`, `App`, `Window`, `WebviewWindow`, `Webview` calls require type/import evidence plus `Emitter`/`Listener`; their opaque application handles do not prove a shared instance. See [Node events](https://nodejs.org/api/events.html), [Tauri frontend events](https://v2.tauri.app/reference/javascript/api/namespaceevent/) and [Tauri Emitter](https://docs.rs/tauri/latest/tauri/trait.Emitter.html) for API semantics.

Node/custom receiver routes require the same immutable module allocation, propagated through supported relative ESM imports, aliases and reexports. Distinct allocations stay separate. Mutable, shadowed, function-local, parameter and factory instances cannot be merged by type or variable spelling. Keys support unescaped literals, immutable constant aliases and explicit TypeScript string enum initializers; Rust literal constants use the existing source resolver and explicit `[analysis].target_os` policy. Dynamic keys, unsupported module aliases/escapes, unknown handlers and unknown qualifiers retain uncertainty. Arbitrary wrappers, runtime DI, external brokers, Flow syntax and cross-language runtime delivery are outside this static subset.

Tauri frontend scope is the nearest indexed application boundary (`src-tauri/tauri.conf.json` or `package.json`), not proof that separate runtime processes share a bus. Target labels must be statically known. Exact target and channel identities stay separate: the map does not infer compatibility between unrestricted and targeted operations. Custom fixed bus/key/target values are explicitly labeled configuration assumptions.

### Custom event rules

For `export class KnownBus` in `src/known.ts`, this rule pair recognizes the explicit shared-allocation example:

```toml
[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = [
  { id = "known-on", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "on", role = "subscribe", event_arg = 0, handler_arg = 1, bus = "receiver" },
  { id = "known-emit", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "emit", role = "publish", event_arg = 0, bus = "receiver" },
]
```

Selectors are exact `language` + `module` + `symbol` + optional `method`; method spelling alone never activates a rule. `module` is a workspace-relative definition file or a supported external import module. Language is `typescript`, `javascript` or `rust`. A custom selector overrides the corresponding built-in selector. Duplicate IDs/selectors, wildcard selectors, unknown fields, unsupported roles or invalid argument indexes reject the custom list.

| Field | Contract |
| --- | --- |
| `role` | `publish`, `subscribe` or `unsubscribe` |
| `event_arg` / `event_key` | Exactly one: zero-based key argument or a fixed key declared by the API rule |
| `handler_arg` | Zero-based callback argument, required for `subscribe`; unknown definitions remain unresolved |
| `bus="receiver"` | Requires `method` and a proven immutable allocation |
| `bus="argument"`, `bus_arg` | Bus allocation comes from that argument |
| `bus="fixed"`, `bus_identity` | Explicit shared-bus assumption, suitable for an inspected wrapper; never inferred from event strings |
| `bus="framework"` | Only the recognized Tauri frontend module; indexed application scope required |
| `target_arg` / `target` | Optional static label argument or fixed qualifier such as `any`; mutually exclusive. Fixed target overrides are not accepted for the known Tauri frontend API. |
| `channel` | Optional exact channel qualifier; defaults to `default` |
| `is_once` | Optional boolean; records once-only registration without simulating execution |

Argument indexes must be below 16. At most 64 custom rules are accepted. Rule identities/qualifiers are bounded to 256 bytes; event keys are non-empty, at most 256 bytes and cannot contain newline/NUL.

For an inspected Rust wrapper `dispatch_event_json(key, payload)` defined in `src/shared/output/mod.rs`, a rule can use `language="rust"`, that exact module path, `symbol="dispatch_event_json"`, `role="publish"`, `event_arg=0`, `bus="fixed"`, `bus_identity="application-events"`, `target="any"`. A frontend rule can explicitly declare the same bus identity. This represents the user's configured bridge assumption, not an automatically proven Rust-to-webview delivery path.

### Bounds and freshness

Inputs are UTF-8 only and follow index/Git/directory exclusions. Current test-context rules apply when rendering both endpoints and their proof/handler locations. Source-route analysis also masks excluded test regions before indexing; test-inclusion setting changes request a new generation. A changed proof file suppresses the old endpoint until a successful refresh.

Limits are 512 KiB per source file, 64 MiB and 4,096 source files per snapshot, 256 endpoints per file and 8,192 per snapshot, shared by configured events and Source routes. Binding resolution has a 32-step JS/TS budget (Rust value recursion: 16); each serialized configured-event endpoint is capped at 8 KiB. Each query inspects at most 512 indexed candidates, renders at most 128 endpoints, and verifies at most 128 source files / 4 MiB. Output stays within existing read/search byte budgets. Unavailable inputs, extraction/query omissions, stale evidence and output truncation are reported; the result is not an exhaustive runtime map. No request performs a full-workspace event relation scan.

## Legacy configuration names

Legacy root keys and sections remain readable. A canonical spelling in the same file wins; invalid canonical values fall back to a lower file layer. Automatic relocation changes only repository files, never the global file.

| Legacy path | Canonical path |
|---|---|
| `tool_output.is_redact_enabled` | `output.is_redact_enabled` |
| `tool_output.is_overview_stats_enabled` | `output.overview.is_stats_enabled` |
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

### Search display compaction and grouped grep source

Search compacts presentation annotations after capturing Jev evidence. Non-call references retain every returned file/line location while omitting source previews; unresolved call names share a line. Repeated analysis caveats are defined at their first `[N1]`-style label and referenced at later applicable locations. Source bodies are unchanged. Confirmed relationship locations retain their original format. Duplicate literal previews are omitted only when their complete enclosing body is already visible. Bodies deferred by the delivery budget are distinct from relevance judgments.

`grep` with `view="source_grouped"` places source rows beneath a heading for every returned file. With `expand="none"`, `42:match` and `43-context` remain distinct; callable expansion is also supported. File/line/page order, masking, column limits and response caps remain effective, without declaration/relationship analysis. The old path-per-row `view="source"` format is unchanged. This view is content-mode-only and is not a read option.
