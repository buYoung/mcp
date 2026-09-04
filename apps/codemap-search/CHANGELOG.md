# Changelog

All notable changes to codemap-search are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

