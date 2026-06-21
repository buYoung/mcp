# Changelog

All notable changes to codemap-search are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

