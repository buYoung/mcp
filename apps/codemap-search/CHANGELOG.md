# Changelog

All notable changes to codemap-search are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `read view=source` adds a trailer, `[Other files importing this window's declarations (static analysis, approximate)]`, that lists the other files importing the window's exported values, classes and constants, with the functions or lines that use them. The caller trailer finds functions by name, but these declarations reach their users through imports. Each supported programming language follows its own import rule: script modules with their re-exports, renamed exports and the defaults and namespaces they pass on (`export { default as Name } from`, `export * as name from`), Python modules, Go packages, Rust paths, JVM packages, C# and PHP namespaces, Swift modules, Dart libraries, Ruby and PowerShell relative loads, Lua modules, C and C++ includes, assembly `.include`, `#include` and `%include`, shell `source`, Nix `import` and Starlark `load`. Files in the same package, namespace or module count when they use the name. A default import, and a Vue, Svelte or Astro component import, is listed under the imported file's name at its first line. Files in nearer directories come first; each trailer stays within 1,536 bytes and 100 ms, and repeated reads fold it like the caller trailer. A lookup that reaches its time limit before finding any importing file says so instead of showing nothing. Each name is listed once, however often it is assigned, and a use inside a function that binds the same name itself, as a local, a Python or PowerShell assignment or parameter, or a shell `local`, is not counted. Shell names that hold hyphens, as Zsh widget functions do, are listed too. Setting `output.read.is_outside_callers_enabled = false` turns it off as well. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.

### Improved

- The `read` description and initial guidance now name the importer trailer beside the caller trailer, call both static uses, and ask agents to follow the ones the question needs instead of every listed caller. Prompt-text change (tool descriptions / server instructions / initial instructions): +48 / 0 / +89 characters.
- Initial guidance now asks agents to search or grep the requested concepts repo-wide before narrowing, verify the ends, transfer or registration and conditions of requested links in source, and follow missing links before stopping or state the blocker and gap. The repo-wide discovery sentence moves from the monorepo-only guidance into the shared guidance. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / +178 characters, or +72 in monorepo workspaces.
- Search lists files outside the ranked results when one of their docstrings or string literals states a quoted query phrase or two adjacent query words, under `Exact phrase matches outside the ranking`. These rows use only tail rows and output bytes the ranked results leave unused, so ranked files, relations and source excerpts are unchanged. Single identifiers and queries with search syntax add none. Prompt-text change: 0 / 0 / 0 characters.
- Search results share repeated deferred-body notes and callee headers after result selection, preserving source ranges and output budgets. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- `initial_instructions` returns the scope inventory without collecting or displaying statistics. Static prompt-text change: 0 / 0 / 0 characters; the statistics block is removed from the initial result.
- Server instructions and the `search`, `grep` and `read` descriptions now ask agents to keep located notes of their findings and of each distinct relevant behaviour in returned source, to record each requested link with both ends, what passes or matches and any wiring or enabling condition, and to read missing ends. For an unresolved requested link, agents try one lookup past an intermediary for the underlying component's direct users, or past local registration for where the component is installed, enabled or configured. Agents read the enclosing logic when a snippet does not establish the behaviour, and before answering they organize findings by requested part, accounting for each note with a cited finding or an explicit gap. Prompt-text change (tool descriptions / server instructions / initial instructions): +985 / +517 / 0 characters.

### Removed

- **Breaking:** The `overview` tool is removed. Use `find` for directory listings, `read` with `view=definitions` for a file's declaration outline, and `initial_instructions` for the monorepo scope inventory. `search` no longer keeps a scope chosen by an earlier call; pass `workspace_scope` on every call that should stay in one scope. Config schema v33 removes `[output.overview]`, `tool_output.is_overview_stats_enabled` and `output.jev.overview_filter_enabled`, and drops `"overview"` from `output.jev.scope`. If automatic config updates are off, `"overview"` in `output.jev.scope` is ignored and the other listed tools stay selected. Prompt-text change: −1,374 (the `overview` description and parameters −1,336, `search` −26, `grep` −12) / +27 / −53 characters, or −64 in monorepo workspaces.

### Fixed

- Index passes now let the segment merges they start finish before releasing the index writer, and rewrite segments in which more than 10% of the documents are deleted. Before, each pass cancelled its merges, so reindexed files kept their old copies and indexes kept growing (one index had 43 segments with 31% of its documents deleted). After editing and restoring 200 files three times, a scratch buyong-mcp index now stays at one segment and 42 MB, where it had 5 segments, 49 MB and 199 deleted documents. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- TypeScript and JavaScript files now record another module's default or namespace they pass on, `export { default as Name } from` and `export * as name from`, beside their renamed exports. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Assembly files now record the preprocessor's `#include` and NASM's `%include` as imports, as they did `.include`. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Bash function declarations now span their bodies, as Zsh ones do, so declaration ranges and the enclosing-function labels of trailers cover the whole function. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- A Groovy or Scala class, object or value counts as exported unless its own modifiers make it private or protected. A private member in its body, or a private Scala constructor, no longer makes declaration listings label it `not exported` or keeps it out of the importer trailer. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Bash, Zsh and PowerShell scripts that source another script through a computed directory, as in `source "$(dirname "$0")/lib.sh"`, `source ${0:A:h}/lib.zsh` or `. $PSScriptRoot\lib.ps1`, now record that import. The literal rest of the path names the file, beside the sourcing script or as the end of one indexed path. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- A script import of a `.vue`, `.svelte` or `.astro` file now resolves to that component file, so caller resolution follows calls into the named exports of its script. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- A TypeScript type-only import specifier, `import { type Name }`, now records `Name` as the imported declaration instead of a binding called `type`, so caller resolution follows it. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- The caller trailer of `read view=source` no longer stops at its time limit on most reads of large workspaces. While the filesystem watcher keeps the index current, the lookup reads only indexed files whose recorded calls or imports use a looked-up name instead of walking the workspace, and caller resolution reuses the indexed extraction of a file whose content still matches the index instead of parsing it again. The listed callers are the same; they now appear in path and line order. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Exact `event_key` searches with no hits explain the lookup scope and how to try ranked search, without silently falling back to it. Prompt-text change: −3 / 0 / 0 characters.
- `grep` file and count modes accept `view=source` and `view=source_grouped` as presentation-only options, while still rejecting content-only controls. Prompt-text change: 0 / 0 / 0 characters.
- `read` with `view=definitions` no longer reports indexed Sass files as an unsupported format. Prompt-text change: 0 / 0 / 0 characters.

## [0.14.2] - 2026-10-29

### Added

- Each tool now advertises `output.client.pi_max_bytes` as `_meta["pi/maxOutputBytes"]` in `tools/list`, so [@buyong/pi-mcp-adapter](https://www.npmjs.com/package/@buyong/pi-mcp-adapter) applies codemap-search's delivery limit without a matching `mcp.json` setting. pi's built-in MCP support ignores tool metadata and still cuts results at 20 KB.

## [0.14.1] - 2026-10-28

### Improved

- Server instructions now ask agents to send independent tool calls together in one response as separate calls and to begin with several narrow lookups instead of one broad search. The paragraph appears twice on purpose.

## [0.14.0] - 2026-10-26

### Added

- Added `read ranges` for 1–20 known file windows in one MCP call, with single-read aliases, shared presentation options, per-item errors, connection-local deduplication and whole-item continuation under the existing byte budgets. Either `file_path` or `ranges` is required, but not both; this is checked at runtime without top-level schema combinators.
- Added OpenRouter decision-model support with provider-specific credentials and strict response validation.

### Changed

- Updated the default TypeSafe decision model to `jev-1.14.0`.

## [0.13.0] - 2026-10-23

### Added

- Added up to three workspace path suggestions when `read`, `grep`, or `find` targets a missing path.
- Added cross-file matches for string keys and uppercase constants used as call arguments in `read view=source`; indexes rebuild once.
- Added `auto`/`on`/`off` indexing modes; `auto` starts on demand and skips non-repository folders or workspaces over a configurable file-count limit.

### Improved

- Linked C/C++ header declarations to unique definitions and their callers, including calls through indirect includes.
- Resolved C++ member calls through `this`, typed variables, and class names, excluding overloads and inheritance.
- Limited C/C++ macro reprocessing to files whose inputs changed.
- Reduced analysis time during initial indexing of large repositories.
- Explained omitted caller summaries with index status, file status, or timeout details.
- Reduced repeated warnings and backed off failed background index updates to retry at intervals of up to 30 seconds.

### Fixed

- Fixed `initial_instructions` errors from older clients by ignoring legacy arguments.
- Fixed obsolete index files accumulating on disk after updates.
- Fixed missing caller summaries and false stale-result warnings while another process writes to the shared index.
- Restored caller information in C/C++ files with macro expansion enabled.
- Fixed unnecessary index updates from ignored file changes, while preserving updates needed for C/C++ macro expansion.
- Fixed nested repositories losing indexed files because of a parent repository's ignore rules.
- Fixed outdated generated config comments during upgrades while preserving their language and existing settings.
- Fixed search errors for queries such as `symbol:*` by falling back to plain-term search.

### Changed

- Changed default `read`, `grep`, and `find` access from `workspace` to `anywhere`, leaving existing settings unchanged.
- Hid `register_task` from the tool list when all Jev filters are disabled; direct calls still work.

## [0.12.0] - 2026-10-21

### Added

- Added an opt-in `synonyms` tool with a local dictionary that ranks meanings by how terms are used in your codebase.
- Added caller summaries beyond the displayed range in `read view=source`, with `[output.read].is_outside_callers_enabled` to turn them off.
- Added client-wide code navigation instructions management, with injected guidance in English.

### Improved

- Improved caller and callee detection for typed method calls and for functions imported from other files.
- Followed re-exports in TypeScript, JavaScript, Python, and Dart to resolve calls to their original declarations.
- Resolved JS/TS workspace package imports to source entry points, including `exports` subpaths.
- Limited caller summary lookups to 250ms per read, keeping callers found so far and noting when the limit is reached.
- Reduced data-file search output by showing counts for unmatched keys and limiting source text.

### Fixed

- Fixed long `else if` chains preventing an entire repository's index from updating.
- Fixed secret masking hiding code after operators and missing values assigned in short variable declarations.

### Changed

- Moved task registration from `initial_instructions` to the new `register_task` tool; `initial_instructions` now takes no arguments.
- Triggered a one-time automatic rebuild of existing indexes for expanded caller detection and codebase evidence.

## [0.11.0] - 2026-09-28

### Added

- Added Jev file recommendations and automatic filters for `search`, `read`, `grep`, and non-root overviews using registered task questions.
- Added repository and global Jev credential files with automatic reload and environment-variable fallback.
- Added output limits for pi and opencode, with source ranges for oversized searches and guidance to narrow other results.

### Improved

- Reduced repeated source text across tools, with an option to request the original text again.
- Preserved source bodies when Jev lacks enough context to judge relevance.
- Reduced processing overhead for large MCP inputs, sensitive-data masking, and source relationship analysis.

### Fixed

- Fixed tool execution errors being reported as JSON-RPC errors by returning error-marked tool results.
- Fixed Codex-specific output-budget guidance appearing for other clients.
- Fixed Windows build failures caused by incompatible build-tool versions.

### Changed

- Enabled client output limits by default and unified their default values at `100000`.
- Simplified Jev filtering settings to an on/off switch and a tool list, with automatic migration of existing settings.

### Removed

- Removed configurable environment-variable names for Jev authentication.

## [0.10.0] - 2026-09-21

### Added

- Added opt-in masking for 9 common PII types and 81 national identifier types across 18 countries.
- Added `fd` file discovery and per-root `tokei` code statistics.
- Added shared and per-tool output limits.
- Added Codex configuration output.
- Added CLI and MCP analytics for index status and read activity, with pagination and 30-day history with automatic cleanup.

### Improved

- Reduced repeated relationship analysis by reusing cached results and avoiding duplicate work across concurrent runs.
- Reduced release archive size from 33.4MiB to 3.4MiB.

### Fixed

- Fixed reindexing that could expand beyond caller-specified paths.
- Fixed index readiness and retries when multiple processes compete for the same index.
- Fixed search output budgeting to preserve later results and flag partial output.

### Changed

- Reorganized output, indexing, and exclusion settings while preserving values, inheritance, comments, and support for legacy keys.

## [0.9.1] - 2026-09-17

### Added

- Added custom field and regex masking rules, with exact-value exceptions.

### Security

- Masked sensitive values in MCP output while preserving variable references and safe literals.
- Stopped echoing search queries in empty results when masking context is unavailable.

`## [0.9.0] - 2026-09-17

### Added

- Added free-function scope, callee locations, and same-file constant initializers to `read` and `grep` context.
- Added automatic links between abstract declarations and their implementations.
- Added event relationship navigation and source-path tracing across 18 languages.
- Added conditional value-flow and error-return summaries.
- Added output controls and size-unit support.

### Improved

- Organized `read` and `grep` results by file, with clearer declaration sections and hierarchies.
- Expanded default `grep` context to include function bodies.
- Improved call lookup efficiency by avoiding unnecessary parsing and repeated analysis.

### Fixed

- Fixed recursive exclusion rules and exclusions for files passed directly to `grep`.
- Fixed false call links from strings, comments, cross-language matches, and object members.
- Fixed parsing errors in large repositories.
- Fixed `grep` modes `files_with_matches` and `count` returning unwanted symbol context.
- Fixed search output losing source text or showing relationships outside the displayed range.
- Fixed `cm find` command routing.
- Fixed inconsistent readiness checks during initial indexing.

### Changed

- Enabled event analysis and related context by default.
- Excluded test code from automatic context by default, with per-language rules that can be replaced or disabled.
- Consolidated exclusion settings under `exclude`, preserving values, comments, and support for legacy configuration keys.
- Raised default read output to 5MiB and search output to 1MiB, with larger caller/callee limits and relationship budgets.

## [0.8.1] - 2026-09-12

### Added

- Added per-project exclusion rules and configuration switching.

## [0.8.0] - 2026-09-11

### Added

- Added file members and caller/callee context under `# symbols` in `read` and `grep`, keeping source text under `# results`.

### Improved

- Improved scoped search by applying scope filters before collecting candidates.
- Improved search relevance for generated and translated files and implementation candidates.
- Clarified regex escaping and next steps for searches with no results.

### Fixed

- Fixed searches losing their subfolder scope.

## [0.7.0] - 2026-09-04

### Improved
- Improved `grep` no-result messages with active filters and next-step guidance.
- Clarified `output_mode` options and updated `search` and `grep` guidance for more efficient navigation.
- Improved `overview` and `read` handling of unsupported file types.

## [0.6.0] - 2026-08-28

### Added
- Added navigation for Vue, Astro, and Svelte components across markup, scripts, styles, and original source locations.
- Added C#, PHP, Ruby, Lua, Swift, Dart, Scala, Groovy/Gradle, PowerShell, and Nix language support.
- Added Bash, CMake, Containerfile, CSS, GraphQL, HCL, HTML, JSON, TOML, YAML, XML, Less, Dockerfile, and Makefile support.
- Added optional Markdown and MDX indexing for headings, links, code blocks, and full text via `is_document_support_enabled`.
- Added optional language groups for shell, infrastructure, interface, and build formats.

### Improved
- Improved search relevance by skipping text, lock, source map, minified, and bundle files by default.
- Improved caller/callee navigation for receiver calls, relative and dynamic imports, local shadowing, and indirection.
- Improved live indexing for file creation, changes, deletion, and runtime language-setting updates.
- Improved workspace safety by refusing to index a user's home directory as the workspace root.

### Fixed
- Fixed standalone SCSS files being indexed as a supported language.

### Changed
- Changed shell, infrastructure, interface, and build groups to opt-in; `find`, `grep`, `read`, and direct parsing remain available.
- Changed `EXTRACTION_FORMAT_VERSION` to `v18` for the expanded language and symbol data.

### Internal
- Replaced legacy benchmark data with a fixed Directus search-quality harness and 14 tasks.

## [0.5.0] - 2026-07-20

### Added
- Added static collection read/write tracking for TypeScript, JavaScript, Python, Java, Kotlin, Go, Rust, C, and C++.
- Added code relationship extraction for Vue, Astro, and Svelte components, plus SQL syntax support.

### Improved
- Improved monorepo navigation with workspace scopes and source roots included in `initial_instructions`.
- Improved workspace selection by rejecting duplicate or ambiguous monorepo paths.
- Improved relationship extraction for multilingual code searches.

### Internal
- Expanded benchmark fixtures, validation, attempt records, episode claims, terminal event checks, and cache reuse.

## [0.4.0] - 2026-07-01

### Added
- Added live `config.toml` watching so config changes sync without restarting.
- Added Korean and English `config.toml` comments selected from the OS locale.
- Added permission-aware filesystem tool descriptions that show the active access policy.

### Improved
- Improved config syncing with debounce handling for rapid `config.toml` edits.
- Improved filesystem permission docs with clearer policy and auto-sync guidance.

## [0.3.0] - 2026-06-30

### Added
- Added workspace-scoped MCP search so requests stay within the active workspace range.
- Added monorepo workspace detection for search and file exploration.
- Added `[update].config_auto_update` to control automatic `config.toml` creation and sync.

### Improved
- Improved file discovery with expanded workspace filters and better parent-child path handling.
- Improved search results and summaries by showing the active workspace scope.
- Improved `config.toml` docs for monorepo and workspace scope settings.

### Fixed
- Fixed repository config templates overriding global settings during config generation or sync.

### Internal
- Improved crates.io publishing reliability with retry and error handling.

## [0.2.0] - 2026-06-26

### Added
- Added Tree-sitter based caller and callee tracking for more precise code navigation.
- Added navigation context settings, including `navigation_context_default` and `navigation_callsite_budget`.

### Improved
- Improved search ranking with language and file extension hints.
- Improved definition and reference tagging through Tree-sitter `tags.scm` queries.
- Improved `codemap-search` README install guidance for crates.io, local checkout, and Claude Code user-scope setup.

## [0.1.6] - 2026-06-22

- No user-facing changes in this release.

## [0.1.5] - 2026-06-22

- No user-facing changes in this release.

## [0.1.4] - 2026-06-21

- No user-facing changes in this release.

## [0.1.3] - 2026-06-21

- No user-facing changes in this release.

## [0.1.2] - 2026-06-21

- No user-facing changes in this release.

## [0.1.1] - 2026-06-21

- No user-facing changes in this release.

## [0.1.0] - 2026-06-21

### Added
- Added `codemap-search`, a local MCP server and CLI for code navigation.
- Added BM25 search across symbols, docstrings, string literals, and error messages.
- Added `overview`, `search`, `read`, `grep`, and `find` tools for repository exploration.
- Added caller and callee context in detailed `search` results.
- Added automatic background indexing and file watching for code changes.
- Added repo-local `.codemap/config.toml` creation and schema migration.
- Added support for Rust, Python, TypeScript, JavaScript, Go, Java, Kotlin, C, C++, and Assembly.
- Added Linux, macOS, and Windows release artifacts with sha256 checksums.
- Added POSIX install script, Homebrew formula, WinGet manifests, and crates.io publishing.

### Improved
- Improved `overview` output with directory-focused summaries and folded tree formatting.
- Improved `search` output with line-numbered snippets and shorter high-signal result details.
- Improved `grep` and `find` glob handling with ripgrep-style include and exclude patterns.
- Improved `read` and `grep` inputs with aliases like `path`, `file`, `query`, `start_line`, and `end_line`.
- Improved cold indexing feedback with a visible `warming up` state.
- Improved Linux compatibility so GNU and musl builds support Ubuntu 22.04+.

### Fixed
- Fixed `serverInfo.version` so MCP clients see the Cargo package version.
- Fixed Windows release checksum formatting and shell variable expansion in release builds.
- Fixed benchmark-answer text in tool examples by replacing it with neutral sample paths.

### Changed
- Changed `get_codemap` to `overview` for a clearer tool name.
- Changed default `grep` output to include matching lines as `file:line:text`.
- Changed git ignore handling so `codemap-search` no longer edits `.git/info/exclude`.
- Changed `register_git_exclude` and `respect_git_exclude` into the unified `use_git_ignore` setting.

### Security
- Added repo-confined filesystem permission controls for `find`, `grep`, and `read`.
- Added stronger path boundary checks for absolute paths and workspace-relative input.
- Hardened the install script with atomic installs, required tool checks, and symlink rejection.

