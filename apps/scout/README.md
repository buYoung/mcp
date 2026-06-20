# @buyong-mcp/scout

Local **code-navigation MCP server** backed by [zoekt](https://github.com/sourcegraph/zoekt)
(indexed regex search) and [Universal Ctags](https://github.com/universal-ctags/ctags)
(symbol definitions), plus direct filesystem reads. It gives a coding agent Claude-Code-style
`search_text` / `lookup_symbol` / `read_file` / `find_files` primitives over the current
working-tree repository, and manages its own zoekt/ctags binaries.

[DESIGN.md](./DESIGN.md) is the authoritative reference for tool semantics and lifecycle.

## Prerequisites

- Node.js ≥ 18.
- `search_text` and `lookup_symbol` need the `zoekt` and Universal Ctags binaries. Call the
  **`install_binaries`** tool once to download and install prebuilt releases (verified by
  SHA-256). Until then those two tools return setup guidance instead of results; `read_file`
  and `find_files` work without any binaries.

## Installation

Not published to npm — build locally, then register the built entry point with your MCP host.

```bash
# from the monorepo root
pnpm install
pnpm --filter @buyong-mcp/scout build   # emits apps/scout/dist/index.js
```

Register with Claude Code (`.mcp.json` for project scope, `~/.claude.json` for user scope).
The server operates on its current working directory, so run it from the repo you want indexed:

```json
{
  "mcpServers": {
    "scout": {
      "command": "node",
      "args": ["/absolute/path/to/apps/scout/dist/index.js"]
    }
  }
}
```

For local debugging with the MCP Inspector: `pnpm --filter @buyong-mcp/scout inspect`.

## Tools

| Tool | Purpose | Key arguments |
|---|---|---|
| `search_text` | Indexed regex content search (zoekt). Broad candidates, call-sites, cross-language scans. Mirrors Claude Code's Grep. | `pattern` (RE2, required), `path`, `glob`, `type`, `output_mode`, `case_insensitive`, `context_lines`, `head_limit`, `offset` |
| `lookup_symbol` | Symbol **definition** lookup (ctags). Declarations only — use `search_text` for call-sites. | `symbol_name` (required), `kind`, `path`, `language`, `is_prefix_match`, `head_limit` |
| `read_file` | Read a file in `cat -n` form (line number + content). Mirrors Claude Code's Read. | `file_path` (required), `offset` (1-based), `limit` |
| `find_files` | Locate files by glob, mtime-sorted, capped at 100. Mirrors Claude Code's Glob. | `pattern` (required), `path` |
| `install_binaries` | Download + install zoekt and Universal Ctags (SHA-256 verified). Runs network downloads — confirm with the user first. | — |

`find_files` ignores `.gitignore` while including hidden files — matching Claude Code's
defaults. Index exclusions for `search_text` are config-driven (see below).

## Configuration

Optional TOML config is read from two layers, merged **per key as repo > global > default**:

- global: `~/.scout/config.toml` (auto-created as a commented template if missing)
- repo: `<repo>/.scout/config.toml` (opt-in — you create it)

Broken or unknown keys log a Korean stderr warning and fall back to the default for that key —
the server never exits on bad config.

| Table | Key | Type | Default | Purpose |
|---|---|---|---|---|
| `[output]` | `mode` | `content` / `files_with_matches` / `count` | `files_with_matches` | Search output format |
| `[output]` | `head_limit` | int ≥ 0 | `250` | Result cap (0 = unlimited) |
| `[output]` | `context_lines` | int ≥ 0 | `0` | Context lines around each match |
| `[output]` | `show_line_numbers` | bool | `true` | Show line numbers in `content` mode |
| `[index]` | `excluded_directories` | string[] | built-in (`.git`, `node_modules`, `dist`, …) | Directory names excluded from indexing (replace) |
| `[index]` | `staleness_check_ms` | int > 0 | `2000` | Index-freshness recheck throttle (ms) |
| `[index]` | `use_gitignore` | bool | `true` | Union repo `.gitignore` directory names into the exclude set |
| `[limits]` | `search_request_timeout_ms` | int > 0 | `15000` | Single search-request timeout (ms) |
| `[limits]` | `index_build_timeout_ms` | int > 0 | `600000` | Index-build timeout (ms) |

```toml
[output]
mode = "files_with_matches"
head_limit = 250
context_lines = 0
show_line_numbers = true

[index]
excluded_directories = [".git", "node_modules", "dist"]
staleness_check_ms = 2000
use_gitignore = true

[limits]
search_request_timeout_ms = 15000
index_build_timeout_ms = 600000
```

The server writes its index to `<repo>/.scout/zoekt/` and shares managed binaries from
`~/.scout/bin/`. `<repo>/.scout/` lands inside the repo but scout never edits your git-tracked
files — add `.scout/` to the repo's `.gitignore` (or `.git/info/exclude`) to keep it out of
`git status`.
