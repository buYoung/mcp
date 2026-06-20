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
