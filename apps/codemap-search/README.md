# codemap-search

A **self-contained MCP stdio server** for coding agents (Claude Code, Codex, opencode, …). One
binary gives an agent a hierarchical *codemap*, BM25 keyword search over extracted
symbols, and exact `read` / `find` / `grep` — with **ripgrep, tree-sitter, and tantivy
all compiled in**. No system `rg`, no external runtime binaries.

The intended flow is hierarchical narrowing:

1. **`overview`** — orient: repo root → folder → file symbol details.
2. **`search`** — locate by keyword; returns a codemap overview when many files match,
   per-file details when few.
3. **`read` / `find` / `grep`** — confirm exact content once the target is pinpointed.

> Status: `0.1.0`. This ships the capabilities; it does **not** yet claim to beat an
> agent's built-in Read/Grep — that comparison is a deferred, separate milestone.

## Supported languages

Symbol extraction (tree-sitter) covers: **Rust** (`.rs`), **Python** (`.py`),
**TypeScript/TSX** (`.ts`, `.tsx`), **JavaScript/JSX** (`.js`, `.jsx`), **Go** (`.go`),
**Java** (`.java`), **Kotlin** (`.kt`, `.kts`), **C** (`.c`), **C++** (`.h`, `.cpp`,
`.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx`), **Assembly/GAS** (`.s`, `.S`, `.asm`).
`read`/`find`/`grep` work on any text file.

Per-language flag conventions: Go uses initial-uppercase for exported symbols, `*_test.go`
plus `Test`/`Benchmark`/`Example`/`Fuzz` for tests, and `// Deprecated:` doc paragraphs;
Java uses the `public` modifier, `@Test` / `*Test.java`, and `@Deprecated` / javadoc
`@deprecated`; Kotlin treats symbols as exported unless `private`/`internal`/`protected`,
and reads `@Test` / `@Deprecated` annotations; C/C++ treats a declaration as file-local when
it carries `static` storage class (otherwise exported), and uses C++ access specifiers
(`public`/`private`/`protected`) for class members (struct members default to public, class
members default to private); Assembly exports symbols that appear in a `.globl`/`.global`
directive.

## Install

Five co-equal channels: crates.io, WinGet, Homebrew, the `install.sh` one-liner, and direct
GitHub Release binaries. Per-OS recommendations and per-channel maintainer/publish runbooks
live in [docs/distribution/](./docs/distribution/index.md) (channel guides:
[crates.io](./docs/distribution/crates-io.md), [WinGet](./docs/distribution/winget.md),
[Homebrew](./docs/distribution/homebrew.md), [install.sh](./docs/distribution/curl-installer.md));
the overall strategy is in
[docs/release-distribution-strategy.md](./docs/release-distribution-strategy.md).

### From crates.io

```sh
cargo install codemap-search
```

Builds and installs the published crate into `~/.cargo/bin` (make sure it is on your
`PATH`). Same binary as the prebuilt archives below — pick whichever you prefer.

### From WinGet (Windows)

```powershell
winget install com.livteam.codemap-search
```

Installs the prebuilt Windows binary (x64 or arm64) and puts `codemap-search` on your
`PATH`. Final availability depends on Microsoft's review of the submitted manifest in
`microsoft/winget-pkgs`. Before Microsoft merges it you can also install from the in-repo
manifest with `winget install --manifest apps/codemap-search/packaging/winget`, subject to
two conditions: (a) it only succeeds **after** the first `codemap-v0.1.0` release publishes
the assets and the manifest's placeholder `sha256` are replaced with the real values
(otherwise the download/hash check fails), and (b) the path is relative, so run it **from
the repo root**. The arm64 build is shipped build-only (cross-built on an x64 runner, not
runtime-verified on arm64 hardware).

### From Homebrew (macOS)

```sh
brew install codemap-search
```

Installs the prebuilt darwin binary (Apple Silicon or Intel) and puts
`codemap-search` on your `PATH`. **Availability is pending homebrew-core
acceptance** — homebrew-core has a notability bar (stars/usage), so a fresh
project's new-formula PR is likely held until the project is notable. Until it
is accepted, use `cargo install codemap-search` (above), a direct GitHub Release
download (below), or the `install.sh` one-liner as the macOS fallback. The
formula lives in-repo at
`apps/codemap-search/packaging/homebrew/codemap-search.rb`; its `sha256` values
are filled from the release `.sha256` files once the first `codemap-v0.1.0`
release publishes the darwin tarballs.

### From source

```sh
cargo install --path apps/codemap-search
# or, from a checkout of this repo:
cargo build --release --manifest-path apps/codemap-search/Cargo.toml
# binary at target/release/codemap-search
```

### Prebuilt binaries

Released on GitHub Releases for macOS (arm64/x64), Linux (x64 in two variants — `musl`/`gnu`
— plus arm64 `musl`; see below), and Windows (x64, plus arm64 build-only). Download the
archive for your platform, extract `codemap-search`, and put it on your `PATH`.

Or let `install.sh` do it for you. It detects your OS/arch, downloads the matching
release archive, **verifies its `.sha256` before extracting**, and installs
`codemap-search` to `~/.local/bin`:

```sh
curl -fsSL https://raw.githubusercontent.com/buYoung/mcp/main/apps/codemap-search/install.sh | sh
```

The installer needs only `curl` (or `wget`), `tar`, and `sha256sum`/`shasum` — no extra
runtime. A checksum mismatch aborts with a non-zero exit and installs nothing. macOS and
Linux only (on Windows, use the WinGet install above).

- Pick a different install dir: `INSTALL_DIR=/usr/local/bin curl -fsSL …/install.sh | sh`
  (sudo only if that dir needs it; the default `~/.local/bin` does not).
- Pin a release: pass `--version` through `sh -s --` —
  `curl -fsSL …/install.sh | sh -s -- --version codemap-v0.1.0`.
- Generic Linux gets the static `musl` build by default; set `CODEMAP_LINUX_LIBC=gnu`
  (x86_64 only) to pick the glibc build instead.
- If the install dir is not on your `PATH`, the script prints the `export PATH=…` line
  to add for the current session. To persist it, append that line to your shell profile
  (zsh: `~/.zshrc`, bash: `~/.bashrc`) and restart your shell.

> Note: the one-liner targets the latest GitHub Release. Pin a tag with `--version` for a
> reproducible install.

#### Supported platforms

| Platform | Variant | Support level | Notes |
|---|---|---|---|
| **Linux x86_64** (Ubuntu 22.04 → 26.04) | `musl` (preferred) | Docker-verified (22.04, 24.04, 26.04) | Fully static; no glibc; also runs on Alpine, Debian, RHEL, Amazon Linux, etc. |
| **Linux x86_64** (Ubuntu 22.04+) | `gnu` | Docker-verified (22.04, 24.04, 26.04) | Requires glibc 2.34+; fails on Ubuntu 20.04 and older (glibc < 2.34) |
| **Linux arm64 (aarch64)** | `musl` | Cross-built; not executed on arm64 | Fully static; cross-compiled via cross-rs. Should run on any arm64 Linux (Alpine, Debian, RHEL, Amazon Linux Graviton, etc.); not yet runtime-verified on arm64 hardware |
| **macOS Sequoia (15) or newer** | arm64, x86_64 | Stated baseline (not Docker-verifiable) | Both Apple Silicon and Intel; confirmed on real hardware |
| **Windows 11 or newer** | x86_64 | Stated baseline, best-effort | Confirmed on real hardware |
| **Windows 11 arm64** | arm64 (aarch64) | Build-only; not executed | Cross-built on an x64 runner that cannot run the arm64 binary; ships unverified at runtime |

#### Linux (prebuilt binary)

Download `codemap-search-x86_64-unknown-linux-musl`. It is a **fully static** binary (no
glibc, no dynamic linker) and runs on **Ubuntu 22.04 or newer (Docker-verified)** and any
other x86_64 Linux distribution — Debian, RHEL/CentOS/Rocky, Alpine, Amazon Linux, and
others.

- **Verified range: Ubuntu 22.04 → 26.04** (Docker-verified: `ubuntu:22.04`, `ubuntu:24.04`,
  `ubuntu:26.04` images, exit 0 on `--version` and `parse` smoke test; host arm64, emulated
  amd64 via `--platform linux/amd64`). Because the musl binary has no glibc dependency,
  it also works on musl-only systems (e.g. Alpine) and other distributions at equivalent or
  newer kernel versions.
- **No glibc requirement** — the fully static build works regardless of the host libc.

A glibc build (`codemap-search-x86_64-unknown-linux-gnu`) is also published. It requires
**glibc 2.34+ (Ubuntu 22.04+)** and will not run on Ubuntu 20.04 or older distributions
(`GLIBC_2.32/2.33/2.34 not found`, Docker-verified, exit 1). The gnu build is
Docker-verified on Ubuntu 22.04, 24.04, and 26.04 (exit 0).
**Prefer the `musl` binary unless you have a specific reason to use the glibc build.**

For **arm64 (aarch64)** Linux, download `codemap-search-aarch64-unknown-linux-musl`. It is
also a **fully static** `musl` binary (no glibc) and is the only Linux arm64 variant — there
is no gnu arm64 asset. It is cross-compiled via cross-rs and is **not yet runtime-verified on
arm64 hardware**; it should run on any arm64 Linux (Alpine, Debian, RHEL, Amazon Linux
Graviton, etc.). On Linux, `cargo install codemap-search` is the recommended path.

## Register with an MCP client

Run the server with the `mcp` subcommand from the repository you want indexed (the server
operates on its current working directory). A **global** (per-user) registration works the
same way: the client spawns `codemap-search mcp` with the active project as its working
directory, so one global install covers every repo — make sure `codemap-search` is on your
`PATH`.

### Claude Code

Project scope (default — only the current repo):

```sh
claude mcp add codemap-search -- codemap-search mcp
```

Global scope (user — available in every project):

```sh
claude mcp add -s user codemap-search -- codemap-search mcp
```

or edit the config directly — `.mcp.json` for project scope, `~/.claude.json` for user scope:

```json
{
  "mcpServers": {
    "codemap-search": { "command": "codemap-search", "args": ["mcp"] }
  }
}
```

### Codex

`~/.codex/config.toml` is Codex's global config, so this entry applies to every project:

```toml
[mcp_servers.codemap-search]
command = "codemap-search"
args = ["mcp"]
```

or add it via the CLI, which writes the same global config:

```sh
codex mcp add codemap-search -- codemap-search mcp
```

### opencode

Global config lives at `~/.config/opencode/opencode.json` (use a per-project `opencode.json`
at the repo root to scope it to one repo). Register it under the `mcp` key as a `local`
server:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "codemap-search": {
      "type": "local",
      "command": ["codemap-search", "mcp"],
      "enabled": true
    }
  }
}
```

## Tools

| Tool | Purpose | Key arguments |
|---|---|---|
| `overview` | Hierarchical codemap. Empty/omitted `path` → root overview; a folder path narrows; a file path shows that file's symbol details. | `path` (string), `format` (e.g. `"llms-txt"`) |
| `search` | BM25 keyword search over symbols/docstrings/path tokens. ≤ threshold → file details; above → codemap overview. | `query` (string, required) |
| `read` | Read a file with line numbers (`   N→content`). Pages large files. | `file_path` (required), `offset` (1-indexed), `limit` |
| `find` | Locate files by glob (`**/*.rs`), mtime-sorted, capped. | `pattern` (required), `path`, `include_ignored` |
| `grep` | Exact literal/regex over files on disk (sees comments + just-changed files). Mirrors Claude Code's Grep. | `pattern` (required), `path`, `glob`, `type`, `output_mode` (default `content` with line numbers; `files_with_matches`/`count`), `-i`, `-n`, `-A`/`-B`/`-C`, `multiline`, `head_limit`, `offset`, `include_ignored` |
| `read` aliases | `read` also accepts `path`/`file` for `file_path`, and 1-based inclusive `start_line`/`end_line` for `offset`/`limit`. | — |

`find` and `grep` honor `.gitignore`, `.git/info/exclude`, and `.codemapignore` by
default; pass `include_ignored: true` to bypass **all** ignore sources for that call. To
turn off only `.git/info/exclude` (everywhere, while keeping `.gitignore`), use the
`use_git_exclude` config key (see [docs/configuration.md](./docs/configuration.md)).

## CLI

`codemap-search` is also a CLI: `mcp` (server), `parse <file>`, `tokenize <ident>`,
`codemap [--path P] [--format F]`, `search <query> [-l N]`, `index [dir]`,
`benchmark --queries <json> [--dir D]`.

## Configuration

Configuration is **optional** — with no config file, defaults reproduce the built-in
behavior. TOML config is read from a repo layer (`<repo>/.codemap/config.toml`) and a
global layer (`$CODEMAP_HOME/config.toml`, else `~/.codemap/config.toml`), merged per key
as `repo > global > default`. On `mcp` startup, if the repo config is absent, a
commented, no-op template is auto-created for discoverability — every key documented
inline at its default.

All keys, defaults, and the `.codemap/` directory layout are documented in
[docs/configuration.md](./docs/configuration.md), including `[filesystem_permissions]` for
controlling whether `read`, `find`, and `grep` stay workspace-only or may use configured
external roots.

## Logging

Diagnostics go to **stderr only** (stdout is the JSON-RPC stream). By default the log
filter is `warn,codemap_search=info`, so dependency `INFO` noise (e.g. tantivy commit/GC
per search) is suppressed. Raise it with `RUST_LOG`:

```sh
RUST_LOG=debug codemap-search mcp     # full diagnostics
```

## Known limits

- Symbol extraction is bounded by the compiled-in tree-sitter grammars (the languages
  above); other extensions are searchable via `read`/`find`/`grep` but not symbol-indexed.
- `max_file_size` (default 1 MiB) silently skips larger files from indexing/codemap.
- String literals are details-layer only (shown in `overview` file details) and are not
  in the BM25 index; use `grep` for exact string/literal search.
- Single-client, sequential stdio server (no cross-process index locking).

## License

MIT — see [LICENSE](./LICENSE).
