# Changelog

All notable changes to codemap-search are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

