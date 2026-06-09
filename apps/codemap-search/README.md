# codemap-search

A **self-contained MCP stdio server** for coding agents (Claude Code, Codex, …). One
binary gives an agent a hierarchical *codemap*, BM25 keyword search over extracted
symbols, and exact `read` / `find` / `grep` — with **ripgrep, tree-sitter, and tantivy
all compiled in**. No system `rg`, no external runtime binaries.

The intended flow is hierarchical narrowing:

1. **`get_codemap`** — orient: repo root → folder → file symbol details.
2. **`search`** — locate by keyword; returns a codemap overview when many files match,
   per-file details when few.
3. **`read` / `find` / `grep`** — confirm exact content once the target is pinpointed.

> Status: `0.1.0`. This ships the capabilities; it does **not** yet claim to beat an
> agent's built-in Read/Grep — that comparison is a deferred, separate milestone.

## Supported languages

Symbol extraction (tree-sitter) covers: **Rust** (`.rs`), **Python** (`.py`),
**TypeScript/TSX** (`.ts`, `.tsx`), **JavaScript/JSX** (`.js`, `.jsx`). `read`/`find`/`grep`
work on any text file.

## Install

### From source

```sh
cargo install --path apps/codemap-search
# or, from a checkout of this repo:
cargo build --release --manifest-path apps/codemap-search/Cargo.toml
# binary at target/release/codemap-search
```

### Prebuilt binaries

Released on GitHub Releases for macOS (arm64/x64) and Linux (x64); Windows is
best-effort. Download the archive for your platform, extract `codemap-search`, and put it
on your `PATH`.

## Register with an MCP client

Run the server with the `mcp` subcommand from the repository you want indexed (the server
operates on its current working directory).

### Claude Code

```sh
claude mcp add codemap-search -- codemap-search mcp
```

or in `.mcp.json` / your MCP settings:

```json
{
  "mcpServers": {
    "codemap-search": { "command": "codemap-search", "args": ["mcp"] }
  }
}
```

### Codex

In `~/.codex/config.toml`:

```toml
[mcp_servers.codemap-search]
command = "codemap-search"
args = ["mcp"]
```

## Tools

| Tool | Purpose | Key arguments |
|---|---|---|
| `get_codemap` | Hierarchical codemap. Empty/omitted `path` → root overview; a folder path narrows; a file path shows that file's symbol details. | `path` (string), `format` (e.g. `"llms-txt"`) |
| `search` | BM25 keyword search over symbols/docstrings/path tokens. ≤ threshold → file details; above → codemap overview. | `query` (string, required) |
| `read` | Read a file with line numbers (`   N→content`). Pages large files. | `file_path` (required), `offset` (1-indexed), `limit` |
| `find` | Locate files by glob (`**/*.rs`), mtime-sorted, capped. | `pattern` (required), `path`, `include_ignored` |
| `grep` | Exact literal/regex over files on disk (sees comments + just-changed files). Mirrors Claude Code's Grep. | `pattern` (required), `path`, `glob`, `type`, `output_mode` (`content`/`files_with_matches`/`count`), `-i`, `-n`, `-A`/`-B`/`-C`, `multiline`, `head_limit`, `offset`, `include_ignored` |

`find` and `grep` respect `.gitignore`, `.git/info/exclude`, and `.codemapignore` by
default; pass `include_ignored: true` to bypass **all** ignore sources for that call. To
turn off only `.git/info/exclude` (everywhere, while keeping `.gitignore`), use
`respect_git_exclude` below.

## CLI

`codemap-search` is also a CLI: `mcp` (server), `parse <file>`, `tokenize <ident>`,
`codemap [--path P] [--format F]`, `search <query> [-l N]`, `index [dir]`,
`benchmark --queries <json> [--dir D]`.

## Configuration

Configuration is **optional** — with no config file, defaults reproduce the built-in
behavior. TOML config is read from two layers, merged **per key** as
`repo > global > default`:

- **Repo:** `<repo>/.codemap/config.toml`
- **Global:** `$CODEMAP_HOME/config.toml`, else `~/.codemap/config.toml`

The loader is **never-exit**: a missing file, parse error, unknown key, or wrong-typed
value warns to stderr and falls back to the default for that key — it never crashes the
server. No template file is auto-created; copy the example below.

### Example `config.toml`

```toml
# Every key is optional; omitted keys use the default shown.

# Where the tantivy index lives (relative to the repo root).
index_path = ".codemap/index"

# `search` returns file details at or below this many matches, a codemap overview above.
result_threshold = 5

# Files larger than this many bytes are skipped before parse/index (minified/generated blobs).
max_file_size = 1048576   # 1 MiB

# Directory names to exclude, ADDED to the built-ins (node_modules, target, dist, build,
# vendor, .git, …). Built-ins can't be removed — this augments, it does not replace.
excluded_directories = ["__pycache__", ".next", "coverage"]

# Register `.codemap/` in `.git/info/exclude` at startup so the index/config stay out of
# `git status`. Idempotent; silent outside a git repo.
register_git_exclude = true

# Dedicated toggle for `.git/info/exclude` ONLY. Set false to let index/codemap/find/grep
# see files hidden solely by `.git/info/exclude` (e.g. local personal excludes) while
# `.gitignore`, the global gitignore, and `.codemapignore` stay respected.
respect_git_exclude = true
```

### The `.codemap/` directory and ignores

- `.codemap/index/` (the index) and `.codemap/config.toml` live under one repo-local
  `.codemap/` directory, auto-registered in `.git/info/exclude` (toggle with
  `register_git_exclude`).
- A repo-local `.codemapignore` uses **gitignore syntax** to hide paths from indexing,
  `find`, and `grep` — the codemap-search-specific complement to `.gitignore`.

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
- String literals are details-layer only (shown in `get_codemap` file details) and are not
  in the BM25 index; use `grep` for exact string/literal search.
- Single-client, sequential stdio server (no cross-process index locking).

## License

MIT — see [LICENSE](./LICENSE).
