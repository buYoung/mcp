# Configuration

codemap-search is configured with an **optional** TOML file. With no config file present,
the compiled-in defaults reproduce the built-in behavior exactly — you only ever write a
key to change something.

## Files and precedence

Config is read from two layers and merged **per key** as `repo > global > default`:

| Layer | Path |
|---|---|
| Repo | `<repo>/.codemap/config.toml` |
| Global | `$CODEMAP_HOME/config.toml`, else `~/.codemap/config.toml` |

"Per key" means a repo file that sets only `result_threshold` still inherits every other
key from the global file (if set there) or the default — layers are not all-or-nothing.

## Loader behavior

- **Never-exit:** a missing file, parse error, unknown key, or wrong-typed value warns to
  stderr and falls back to the default for that key. The server never crashes over config.
- **Auto-generated template:** on `mcp` startup, if `<repo>/.codemap/config.toml` is
  absent, a commented, no-op template is created — every key commented out at its default,
  so the file changes nothing until you uncomment a line. An existing file is never
  overwritten.
- **Validation:** numeric keys must be positive integers, `index_path` must be a
  non-empty string, `excluded_directories` must be an array of strings. An invalid value
  warns and falls back to the default for that key.

## Key reference

| Key | Type | Default | Summary |
|---|---|---|---|
| `index_path` | string | `".codemap/index"` | Where the tantivy index lives (relative to the repo root) |
| `result_threshold` | integer | `5` | `search` detail-vs-overview branch threshold |
| `max_file_size` | integer (bytes) | `1048576` (1 MiB) | Files larger than this are skipped before parse/index |
| `excluded_directories` | string array | `[]` | Directory names excluded in addition to the built-ins |
| `use_git_exclude` | bool | `true` | Whether walkers honor `.git/info/exclude` (that source only) |
| `index_staleness_ms` | integer (ms) | `5000` | Debounce for the request-triggered fallback refresh |
| `search_overview_file_limit` | integer | `50` | Max file headers in `search`'s codemap-overview branch |
| `watch` | bool | `true` | Filesystem watcher (autonomous background index refresh) |
| `watch_debounce_ms` | integer (ms) | `500` | Batching window for watcher events |
| `indexer_auto_restart` | bool | `true` | Auto-recovery when the background indexer thread dies |

### Indexing

- **`index_path`** — where the tantivy index lives, relative to the repo root. The
  default keeps it inside the repo-local `.codemap/` directory. The index location is
  always excluded from walking and from watcher events, so the index never indexes (or
  re-triggers) itself, including at a custom location.
- **`max_file_size`** — files larger than this many bytes are silently skipped before
  read/parse/index. The cap exists to keep minified bundles and generated blobs out of
  the symbol index; such files remain reachable via `read`/`find`/`grep`.
- **`excluded_directories`** — directory names that are never walked, **added** to the
  built-ins (`node_modules`, `target`, `dist`, `build`, `vendor`, `.git`, `.codemap`, …).
  This augments the built-in list; built-ins cannot be removed.

### Search output

- **`result_threshold`** — `search` returns per-file symbol details when the match count
  is at or below this value, and a codemap overview above it.
- **`search_overview_file_limit`** — caps how many file headers `search` emits in its
  codemap-overview branch (the branch taken when matches exceed `result_threshold`).
  Output-size only — safe to tune.

### Ignore handling

- **`use_git_exclude`** — dedicated toggle for **`.git/info/exclude` only**. Set `false`
  to let index/codemap/`find`/`grep` see files hidden solely by `.git/info/exclude`
  (e.g. local personal excludes) while `.gitignore`, the global gitignore, and
  `.codemapignore` stay honored. The per-call `include_ignored` argument on `find`/`grep`
  is the broader override that bypasses every ignore source for that call.

### Index freshness

- **`watch`** — when `true` (the default), a filesystem watcher refreshes the index in
  the background on its own: ordinary edits become path-scoped incremental updates, and
  `search`/`overview` never trigger a tree walk. When `false` — or when the watcher fails
  to start or dies — the server falls back to the request-triggered lazy refresh below.
- **`watch_debounce_ms`** — events arriving within this window are batched into one
  incremental refresh, so a save-burst (formatter, branch switch) costs one pass instead
  of one per file.
- **`index_staleness_ms`** — debounce for the **fallback** refresh path, active only when
  the watcher is off or unavailable: within this window, repeated `search`/`overview`
  calls enqueue at most one background refresh, and every call answers immediately from
  the last committed snapshot. `read`/`find`/`grep` always read live disk, so brief
  search staleness is corrected by the follow-up read.
- **`indexer_auto_restart`** — when `true` (the default) and the background indexer
  thread dies, the next `search`/`overview` rebuilds the index engine, respawns the
  indexer, and re-attaches the watcher. Restarts are capped per server run so a
  deterministic crash cannot respawn-loop. Set `false` to instead serve results frozen at
  the last commit until the server is restarted.

## Example `config.toml`

```toml
# Every key is optional; omitted keys use the defaults above.

index_path = ".codemap/index"
result_threshold = 5
max_file_size = 1048576   # 1 MiB
excluded_directories = ["__pycache__", ".next", "coverage"]
use_git_exclude = true
index_staleness_ms = 5000
search_overview_file_limit = 50
watch = true
watch_debounce_ms = 500
indexer_auto_restart = true
```

## The `.codemap/` directory and ignore files

- `.codemap/index/` (the index) and `.codemap/config.toml` live under one repo-local
  `.codemap/` directory. codemap-search never walks `.codemap/` (it is a built-in
  exclude), so it is never indexed — but to keep it out of `git status`, add `.codemap/`
  to your repo's `.gitignore` (or `.git/info/exclude` for a local-only, uncommitted
  ignore). The tool does not write to your git files.
- A repo-local `.codemapignore` uses **gitignore syntax** to hide paths from indexing,
  `find`, and `grep` — the codemap-search-specific complement to `.gitignore`.
