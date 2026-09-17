# Configuration

[한국어](./configuration.ko.md) | English

Configuration is optional. Add only the keys you want to change; other keys use global settings or built-in defaults.

`event_navigation`, `analysis`, and `macro_expansion` are optional sections. Event navigation and native macro expansion are enabled by default. Analysis runs without a target OS override; omitted targets stay unknown rather than inheriting the host OS. Explicit settings, including `is_enabled = false`, still win.

## Files and precedence

Config is read from two layers and merged **per key** as `repo > global > default`. Use the repo file for project-specific behavior; use the global file only for defaults you want across repositories.

| Layer | Path |
|---|---|
| Repo | `<repo>/.codemap/config.toml` |
| Global | `$CODEMAP_HOME/config.toml`, else `~/.codemap/config.toml` |

"Per key" means a repo file that sets only `[search].result_threshold` still inherits every other setting from the global file (if set there) or the default. Layers are not all-or-nothing.

## Loading and automatic writes

The current configuration schema is **13**. The marker is a comment:

```toml
# codemap-config-version: 13
```

- Missing files are optional. Malformed TOML discards that file's layer; an unknown key, wrong type or invalid value warns on stderr and falls back for that key. A valid global value wins over the built-in default when the repo value is invalid.
- On `mcp` startup, `[update].config_auto_update = true` creates a missing repo file. Its directory array contains common folders and recursive globs for detected project types; other active values use their built-in defaults. Active repo values override global settings.
- A pre-v6 repo config, including one without a marker, receives a **one-time directory migration**. Existing user rules, the old effective exclusions, common folders and recommended recursive globs are made explicit in its array. Existing entries and comments are preserved; missing values are appended without duplication. The marker advances to the current schema version.
- **From version 6 onward, `excluded_directories` is never automatically regenerated or supplemented.** Deleting an entry, using `[]`, commenting out the key, or adding another project does not cause the array to be restored. This is separate from reading manual edits at runtime.
- Version 8 moves active test-code settings from the root or `[caller_context]` into `[exclude]`, preserving effective values and user comments. Automatic writes still follow `config_auto_update`; legacy locations remain readable, including in the global file. Invalid or conflicting values that cannot be moved without changing behavior leave the file untouched and produce a warning.
- Version 9 also moves `excluded_directories` and `use_git_exclude` from `[index]` or root-level aliases into `[exclude]`. Existing arrays, explicit `[]`, booleans, and comments are preserved; no directory rules are added by this relocation. Valid `[exclude]` values take precedence within the same file.
- Version 12 introduced the commented `[event_navigation].is_enabled` opt-in, with event indexing disabled by default in that version.
- Version 13 enables event indexing and relevant navigation output by default. Explicit `is_enabled=false` values remain disabled; `include_events=false` suppresses event context for one request.
- Version 11 adds a commented `[analysis].target_os`; omitted or empty remains target-neutral.
- Version 10 introduced the commented `[macro_expansion]` section, initially disabled by default. Native preprocessing is now enabled by default; an existing explicit `is_enabled=false` remains disabled. Migration distinguishes TOML string contents from section headers and version comments.
- Ordinary schema updates still add new settings as commented blocks according to `config_auto_update`; they do not automatically enable those keys. A current file is not rewritten.
- `config_auto_update = false` disables both initial file creation and migration writes. It does not disable reads or config watching. The global file is never generated or migrated.
- Korean OS locale selects Korean generated comments; other/unknown locales use English. Both templates have the same keys and values before project discovery.

If the config changes during migration, contains malformed TOML, cannot be written, or uses an unsupported table layout, the server leaves it untouched and warns. Correct the reported problem and restart. For a dotted/inline index table without an exclusion array, add an explicit array before retrying. A symlinked config keeps its symlink.

### Manual transition when automatic writes are disabled

In schema 6, an explicitly configured array replaces the optional default list. Old arrays were additions to built-ins. If you keep automatic writes disabled, include the old optional names (`node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor`) yourself when you want to retain that behavior, then add the common/project rules you need and set the marker to 6. Other settings need no conversion. Back up your config before manually editing it.

Migration can materialize inherited global exclusions into the repo array. Comment out the repo key afterward if you want subsequent global changes to be inherited again. A global array is also a complete list in schema 6; it is never automatically rewritten.

## Directory exclusions

`[exclude].excluded_directories` is the complete **optional** directory-rule list. Explicit arrays are not unioned with hidden built-ins. `[]` disables these optional rules; omitting the key inherits the global list or the default. Existing ignore files and mandatory exclusions are independent.

The common initial list is:

```toml
[exclude]
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

MCP watches the repo/global config directories that exist at startup, independently of `[refresh].watch`. It batches config events for about **1000ms**, then reloads the settings. If a directory did not exist or watching could not start, restart the server after creating/editing the config. CLI commands load configuration when invoked.

| Settings | Application point |
|---|---|
| `excluded_directories`, all `[language_support]` switches | Reload requests a full index refresh; results reflect the change when it finishes |
| All `[macro_expansion]` settings | Reload requests a full refresh, including when expansion is disabled |
| Search output, caller annotation options, tool output limits, filesystem permissions | Subsequent tool requests after reload |
| `index_staleness_ms`, `indexer_auto_restart` | Subsequent refresh/recovery decisions |
| `max_file_size`, `use_git_exclude` | Subsequent walks/refreshes; changing them alone does not request a full refresh |
| `navigation_store_references` | Subsequent parsing; unchanged files can be reused from the index even after restart |
| `index_path`, `watch`, `watch_debounce_ms` | Restart required |
| `config_auto_update` | Automatic writes at the next MCP startup |

Manual exclusion changes update file filters and request a full index refresh. Removed files disappear from results and newly included files become searchable when the refresh finishes. If indexing is unavailable, recover or restart the server before checking the results.

## Key reference

This table summarizes supported keys, accepted types, and defaults. Numeric keys require positive integers except `grep_max_columns`, which also accepts `0`. The generated template uses the sectioned form shown here. Legacy top-level keys (for example `result_threshold = 5`) are still accepted for compatibility; when both forms appear in one file, the sectioned value wins.

Byte-size keys accept either an integer byte count or a quoted positive integer with `b`, `kb`, `mb`, or `gb`. Units are case-insensitive and use powers of 1024: `"50mb"` = `52428800` bytes. Surrounding whitespace and a space before the unit are accepted (`"50 MB"`). Fractions, zero, negative values, unknown units, and values outside the destination integer range warn and inherit the lower layer. TOML requires quotes around unit-bearing values; bare `50mb` is invalid TOML. This applies to `max_file_size`, `read_output_byte_cap`, `search_detail_byte_cap`, `annotation_sub_budget`, and `macro_expansion.max_output_bytes`; counts and milliseconds still require integers.

| Key | Type | Default | Summary |
|---|---|---|---|
| `[update].config_auto_update` | bool | `true` | Create missing repo config and append commented schema-sync blocks on `mcp` startup |
| `[index].index_path` | string | `".codemap/index"` | Index directory; absolute or relative to the workspace root |
| `[index].max_file_size` | integer bytes or size string | `"1mb"` (`1048576`) | Files larger than this are skipped before parse/index |
| `[exclude].excluded_directories` | string array (relative directory globs) | Common + legacy fallback names; generated files use recursive globs | Complete optional list; see [Directory exclusions](#directory-exclusions) |
| `[exclude].use_git_exclude` | bool | `true` | Whether walkers honor `.git/info/exclude` (that source only) |
| `[language_support].is_document_support_enabled` | bool | `false` | Include `.md`/`.mdx` in index, search, overview, codemap, and watcher refreshes |
| `[language_support].is_shell_support_enabled` | bool | `false` | Include `.sh`, `.bash`, and `.zsh` in index-backed discovery |
| `[language_support].is_infrastructure_support_enabled` | bool | `false` | Include HCL/Terraform, Dockerfile, and Nix definitions |
| `[language_support].is_interface_support_enabled` | bool | `false` | Include Protocol Buffers and GraphQL definitions |
| `[language_support].is_build_support_enabled` | bool | `false` | Include Make, CMake, and Starlark/Bazel build definitions |
| `[refresh].watch` | bool | `true` | Filesystem watcher (autonomous background index refresh) |
| `[refresh].watch_debounce_ms` | integer (ms) | `500` | Batching window for watcher events |
| `[refresh].index_staleness_ms` | integer (ms) | `5000` | Debounce for the request-triggered fallback refresh |
| `[refresh].indexer_auto_restart` | bool | `true` | Auto-recovery when the background indexer thread dies |
| `[search].result_threshold` | integer | `5` | Number of top-ranked files `search` renders as details before the ranked tail |
| `[search].search_overview_file_limit` | integer | `12` | Max file headers in `search`'s compact ranked tail |
| `[search].search_detail_snippet_max_lines` | integer | `80` | Per-symbol snippet line cap in `search` detail view; bodies longer than this are truncated |
| `[search].search_detail_symbol_limit` | integer | `20` | Max symbols rendered per file in `search` detail view; overflow becomes a summary note |
| `[search].search_detail_byte_cap` | integer bytes or size string | `"32kb"` (`32768`) | Output size limit in bytes for one `search` response, including the partial-output footer |
| `[search].search_literal_max_len` | integer (chars) | `200` | Matched-literal truncation length; longer literals are cut with an ellipsis |
| `[search].search_literal_limit` | integer | `10` | Max matched literals rendered per file in `search` detail view |
| `[search].search_anchor_snippet_limit` | integer | `3` | Maximum full snippets per file; further matches use signatures of up to three lines |
| `[tool_output].grep_max_columns` | integer | `0` | `grep` content-mode column cap; matched lines wider than a positive cap are replaced with `[Omitted long matching line]`; `0` disables |
| `[tool_output].read_output_byte_cap` | integer bytes or size string | `"50mb"` (`52428800`) | `read` and callable-expanded `grep` output ceiling; oversized reads return a narrowing error, expanded grep paginates |
| `[filesystem_permissions].find` | string | `"workspace"` | Path policy for `find`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].grep` | string | `"workspace"` | Path policy for `grep`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].read` | string | `"workspace"` | Path policy for `read`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].allowed_roots` | string array | `[]` | External roots available to tools set to `allowed_roots` |
| `[caller_context].caller_context_default` | bool | `true` | `search` caller/callee annotation default when the per-call parameter is omitted |
| `[exclude].should_include_test_code` | bool | `false` | Include tests in automatic symbol/call context |
| `[exclude].test_file_patterns` | string array | See test-code context | Test file globs; [] disables path detection |
| `[exclude].test_attributes` | language → string array | See test-code context | Attribute/annotation patterns; each language list replaces its inherited list |
| `[exclude].test_decorators` | language → string array | See test-code context | Decorator patterns; [] disables one language’s list |
| `[exclude].test_calls` | language → string array | See test-code context | Test-call patterns; [] disables one language’s list |
| `[caller_context].navigation_context_default` | bool | `false` | Check source structure and mark confirmed call targets `precise` |
| `[caller_context].navigation_callsite_budget` | integer | `1000` | Maximum call sites checked before using approximate name-based scanning |
| `[caller_context].navigation_store_references` | bool | `false` | Store reference locations other than function calls |
| `[caller_context].scan_cap` | integer | `500` | Caller-scan hit limit, shared across names (minimum 25/name) |
| `[caller_context].caller_list_cap` | integer | `1000` | Max callers (or non-call references) rendered per symbol |
| `[caller_context].callee_list_cap` | integer | `1000` | Max callees rendered per symbol |
| `[caller_context].annotation_sub_budget` | integer bytes or size string | `"8kb"` (`8192`) | Call-relationship output size within `search_detail_byte_cap` |
| `[caller_context].common_name_threshold` | integer | `2` | Defs-per-name count at which caller/callee lists carry an ambiguity label |
| `[caller_context].caller_omit_def_threshold` | integer | `5` | Definition count for the same name at which the approximate caller list is replaced with a `grep` suggestion; callees unaffected |

### Indexing and file exclusions

The user home directory itself cannot be an MCP workspace or an explicit `index`/`benchmark` target; a project beneath it is valid. If neither `HOME` nor `USERPROFILE` is available, the server warns and continues.

`.txt`, `*.lock`, known package-manager lockfiles, `*.map`, and minified/bundle files are excluded case-insensitively from indexing, codemap, and caller scans. `find`/`grep` hide them by default but accept `include_ignored: true`; direct `read`/`parse` remains available. See [supported languages and file exclusions](./language-support-checklist.md) for the full list. Files larger than `max_file_size` are also skipped by indexing. Indexing accepts UTF-8 source only. Invalid UTF-8 removes any stale indexed symbols; `overview` and `read` explain the exclusion with the first invalid byte offset. `read` keeps its replacement-character display and never rewrites the source.

The five `[language_support]` switches control indexing, search, overview, codemap, and file-change refreshes. They do not disable live `find`/`grep`/`read` or direct `parse`.

`use_git_exclude` controls only `.git/info/exclude`. Setting it to `false` leaves `.gitignore`, global Git ignores, and `.codemapignore` in effect.

### Native macro expansion

Native expansion automatically attempts C/C++ preprocessor macros and CPP-based assembly macros with installed Clang. NASM `.asm` files use NASM expansion listings to map generated labels to the invocation line. No configuration section is required; use `is_enabled = false` to disable native processes. Missing tools retain original declarations with an unresolved notice. It follows [clangd's compilation-context model](https://clangd.llvm.org/design/compile-commands); it does not start clangd or load `.clangd` configuration.

```toml
[macro_expansion]
is_enabled = true
compilation_database = "build/compile_commands.json"
clang_path = "clang"
nasm_path = "nasm"
clang_flags = ["-Iinclude", "-DFEATURE=1"]
nasm_flags = ["-Iinclude/", "-felf64"]
timeout_ms = 5000
max_output_bytes = "8mb"
```

| Key under `[macro_expansion]` | Type / default | Behavior |
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

### Output and call relationships

`search` shows detailed top matches followed by a compact list. Its size settings limit output; they do not change which files are excluded. General queries rank generated files and translation resources lower, while exact paths, symbols, resource keys, or quoted text can target those files directly.

`search_detail_byte_cap` includes the partial-output notice. If results are cut off, narrow the query or read the suggested ranges; `search` has no page-offset parameter. `search_anchor_snippet_limit` limits full snippets per file; further matches show signatures of up to three lines.

`read_output_byte_cap` includes line numbers, context, and headings, and also bounds callable-expanded `grep`. Oversized `read` output returns an error with a narrower `offset`/`limit` suggestion. Expanded grep reports oversized bodies or pagination instead of silently splitting a callable. A separate 256 KiB whole-file limit applies to `read` when `limit` is omitted and callable expansion is off. `grep_max_columns = 0` disables the long-line limit; otherwise long matches become `[Omitted long matching line]`. Partial `grep` pages report `next_offset`.

Raising the read cap does not raise the independent search, annotation, or parsing limits. Search remains capped at 32 KiB, with an 8 KiB annotation sub-budget. Caller scans still use `scan_cap = 500`, so 1000-entry caller/callee lists are upper bounds, not guaranteed output counts. Live `read`/`grep` context also has a fixed shared 16 KiB ceiling. Callable parsing accepts at most `min(max_file_size, 4 MiB)`, which is 1 MiB by default. Larger caps permit larger responses and more rendering work; they do not establish any consuming client's response limit.

Existing explicit repo/global values are preserved when built-in defaults change. Remove, comment out, or update an old override to use the new value; restarting alone does not replace it.

`caller_context_default` applies only when a `search` call omits `caller_context`. An explicit argument wins. Call relationships are approximate by default. With `navigation_context_default = true`, source structure, imports, and local bindings can confirm a single target and mark it `precise`. Calls that cannot be confirmed use approximate results. `navigation_callsite_budget` limits the number of call sites checked before using name-based scanning.

`navigation_store_references` stores reference locations other than function calls; it is not required to confirm call targets. Some structured formats always store references. The setting applies during parsing, so restarting alone may reuse unchanged files without reparsing them.

Resolved `calls` entries include the definition as `name — file:line`, in both approximate and `precise` modes. Ambiguous targets keep their bare names. MCP `read`/`grep` also show `references (same-file constants, approximate)` for direct bare identifiers in the displayed functions. This works with `navigation_store_references = false`: the source tree is checked against indexed constant declarations (including JavaScript/TypeScript `const` bindings). Locations and initializer text are shown without evaluating code; previews longer than 240 characters are shortened. Comments, strings, qualified/imported references, macro token trees, duplicate names, and names with local bindings are omitted. Test exclusions and context byte budgets also apply to these references, with a notice when the budget omits entries.

`scan_cap` is shared across scanned names, with a minimum of 25 hits per name. `caller_list_cap` and `callee_list_cap` limit each symbol's displayed relationships. `annotation_sub_budget` limits their total output in bytes within `search_detail_byte_cap`; source snippets take priority, and omitted relationships are noted.

At `common_name_threshold` definitions of the same name, approximate relationships carry an ambiguity label. At `caller_omit_def_threshold`, the approximate caller list is replaced by a note and a `grep` suggestion. This does not suppress callees or prevent a confirmed target from being shown.

### Test-code context

Manage `should_include_test_code`, `test_file_patterns`, `test_attributes`, `test_decorators`, and `test_calls` under `[exclude]`, alongside `excluded_directories` and `use_git_exclude`. Within one file, valid `[exclude]` values take precedence over legacy `[caller_context]`, `[index]`, and root-level aliases; language tables inherit missing language entries. Repo → global → built-in precedence still applies between files. The two workspace exclusion settings apply to indexing, codemap, caller scans, and `find`/`grep`; changing either requests a full index refresh after config reload.

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
[exclude]
should_include_test_code = false
# Replace the complete path list with the patterns you want.
test_file_patterns = ["**/tests/**", "*_test.go", "*.test.ts", "checks/**"]

[exclude.test_attributes]
rust = ["test", "tokio::test", "cfg(test)", "company::case"]
java = []

[exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "company_test"]

[exclude.test_calls]
typescript = []
```

Default lists (languages not listed have no built-in entries for that category):

```toml
[exclude]
test_file_patterns = ["**/tests/**", "**/test/**", "**/__tests__/**", "test_*.py", "*_test.*", "*.test.*", "*_spec.*", "*.spec.*", "*Test.java", "*Tests.java", "*IT.java"]

[exclude.test_attributes]
rust = ["test", "tokio::test", "async_std::test", "rstest", "rstest::rstest", "cfg(test)"]
java = ["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Nested", "BeforeEach", "AfterEach", "BeforeAll", "AfterAll"]
kotlin = ["Test", "ParameterizedTest", "RepeatedTest", "BeforeTest", "AfterTest", "BeforeEach", "AfterEach"]
csharp = ["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestFixture", "SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown"]
swift = ["Test", "Suite"]
php = ["Test"]

[exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "unittest.skip", "unittest.skipIf", "unittest.skipUnless", "unittest.expectedFailure"]

[exclude.test_calls]
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

With `watch = true`, file changes refresh the index in the background. `watch_debounce_ms` combines nearby changes into one refresh. When file watching is off or unavailable, `search`/`overview` request background refreshes at intervals controlled by `index_staleness_ms` and return the last available results immediately.

With `indexer_auto_restart = true`, the next `search`/`overview` attempts recovery if background indexing stops. Recovery attempts are capped per server run. With it disabled, results remain frozen until restart. Live `read`/`find`/`grep` remains available in either case.

## Example `config.toml`

The example below is intentionally explicit. In a real file, you can keep only the settings you want to override.

```toml
# Every setting is optional; omitted settings use the defaults above.

[update]
config_auto_update = true

[index]
index_path = ".codemap/index"
max_file_size = "1mb"

[language_support]
is_document_support_enabled = false
is_shell_support_enabled = false
is_infrastructure_support_enabled = false
is_interface_support_enabled = false
is_build_support_enabled = false

[refresh]
watch = true
watch_debounce_ms = 500
index_staleness_ms = 5000
indexer_auto_restart = true

[search]
result_threshold = 5
search_overview_file_limit = 12
search_detail_snippet_max_lines = 80
search_detail_symbol_limit = 20
search_detail_byte_cap = "32kb"
search_literal_max_len = 200
search_literal_limit = 10
search_anchor_snippet_limit = 3

[tool_output]
grep_max_columns = 0
read_output_byte_cap = "50mb"             # 52428800 bytes

[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"
allowed_roots = []

[exclude]
excluded_directories = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index"]
# Initial generation also adds recursive globs for detected project types; edit them manually from v6 onward.
use_git_exclude = true
should_include_test_code = false

[caller_context]
caller_context_default = true
navigation_context_default = false
navigation_callsite_budget = 1000
navigation_store_references = false
scan_cap = 500
caller_list_cap = 1000
callee_list_cap = 1000
annotation_sub_budget = "8kb"
common_name_threshold = 2
caller_omit_def_threshold = 5
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

Read and expanded grep retain `read_output_byte_cap`; annotations retain their existing budgets. Oversized callable bodies are never silently split: use the displayed source range with `expand=none` and line windows. Grep column omissions are marked as incomplete. Macro/encoding/test-context notices remain in applicable context views; `source` contains only live filesystem output and operational expansion notices. Eligible event relationships appear automatically in full/relations views.

## Explicit Rust analysis target

```toml
[analysis]
target_os = "macos"
```

`target_os` is an optional OS identifier, never inferred from the running host. Repo settings override global settings; an empty string explicitly clears an inherited value. Invalid types/identifiers warn and fall back to the lower layer. Config reload takes effect in the next request's fresh source/condition resolver; it does not require a new source index.

Rust lookup evaluates `target_os = "value"`, `all(...)`, `any(...)`, `not(...)`, and boolean literals with three-valued logic. Thus `not(target_os="macos")` applies to every explicitly non-macOS target, not only Windows. Missing OS facts, other keys/flags, `cfg_attr`, and unsupported string/token forms remain unknown. Plain `cfg(test)` is delegated to the existing test-context filter; enabling test context does not claim a Cargo test build. See the [Rust conditional-compilation reference](https://doc.rust-lang.org/reference/conditional-compilation.html).

Conditions on imports, reexports, declarations, call sites and discoverable parent modules are checked before confirming a definition. Module membership outside the bounded source/path model remains unknown. Source-confirmed callers survive the name-count threshold; ambiguous name-only callers remain suppressed. Callsite and alias-name limits retain already-proven entries and report incomplete resolution. Alias discovery is a bounded candidate filter (16 rounds, 256 names), not proof: each displayed link still needs exact source identity. An explicit target is shown in relation output; this is static navigation, not a build/runtime guarantee.

One response uses one configuration snapshot; a concurrent reload applies to subsequent requests.


## Indexed event navigation

Event navigation is enabled by default and needs no extra request option. It stores bounded source inputs with the symbol index and builds a separate immutable map before publishing that generation. It does not execute handlers or build scripts. The following configuration shows the defaults; it is not required to enable the feature.

```toml
[event_navigation]
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
[event_navigation]
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
