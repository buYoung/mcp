# Configuration

codemap-search is configured with an **optional** TOML file. With no config file present, the compiled-in defaults reproduce the built-in behavior exactly — you only ever write a key to change something.

## Files and precedence

Config is read from two layers and merged **per key** as `repo > global > default`:

| Layer | Path |
|---|---|
| Repo | `<repo>/.codemap/config.toml` |
| Global | `$CODEMAP_HOME/config.toml`, else `~/.codemap/config.toml` |

"Per key" means a repo file that sets only `result_threshold` still inherits every other key from the global file (if set there) or the default — layers are not all-or-nothing.

## Loader behavior

- **Never-exit:** a missing file, parse error, unknown key, or wrong-typed value warns to stderr and falls back to the default for that key. The server never crashes over config.
- **Auto-generated template:** on `mcp` startup, if `<repo>/.codemap/config.toml` is absent, a commented, no-op template is created — every key commented out at its default, so the file changes nothing until you uncomment a line. An existing file is never overwritten.
- **Validation:** numeric keys must be positive integers, `index_path` must be a non-empty string, arrays must contain strings, and filesystem permission policies must be `workspace`, `allowed_roots`, or `anywhere`. An invalid value warns and falls back to the default for that key.

## Key reference

| Key | Type | Default | Summary |
|---|---|---|---|
| `index_path` | string | `".codemap/index"` | Where the tantivy index lives (relative to the repo root) |
| `result_threshold` | integer | `5` | Number of top-ranked files `search` renders as details before the ranked tail |
| `max_file_size` | integer (bytes) | `1048576` (1 MiB) | Files larger than this are skipped before parse/index |
| `excluded_directories` | string array | `[]` | Directory names excluded in addition to the built-ins |
| `use_git_exclude` | bool | `true` | Whether walkers honor `.git/info/exclude` (that source only) |
| `index_staleness_ms` | integer (ms) | `5000` | Debounce for the request-triggered fallback refresh |
| `search_overview_file_limit` | integer | `12` | Max file headers in `search`'s compact ranked tail |
| `watch` | bool | `true` | Filesystem watcher (autonomous background index refresh) |
| `watch_debounce_ms` | integer (ms) | `500` | Batching window for watcher events |
| `indexer_auto_restart` | bool | `true` | Auto-recovery when the background indexer thread dies |
| `[filesystem_permissions].find` | string | `"workspace"` | Path policy for `find`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].grep` | string | `"workspace"` | Path policy for `grep`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].read` | string | `"workspace"` | Path policy for `read`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].allowed_roots` | string array | `[]` | Canonicalized external roots available to tools set to `allowed_roots` |
| `grep_max_columns` | integer | `500` | `grep` content-mode column cap; matched lines wider than this are replaced with `[Omitted long matching line]`; `0` disables |
| `read_output_byte_cap` | integer (bytes) | `102400` | `read` always-applied output ceiling; a rendered output exceeding this throws instead of emitting an unbounded blob |
| `search_detail_snippet_max_lines` | integer | `80` | Per-symbol snippet line cap in `search` detail view; bodies longer than this are truncated |
| `search_detail_symbol_limit` | integer | `20` | Max symbols rendered per file in `search` detail view; overflow becomes a summary note |
| `search_detail_byte_cap` | integer (bytes) | `32768` | Hard byte ceiling for one `search` response, including the partial-output footer |
| `search_literal_max_len` | integer (chars) | `200` | Matched-literal truncation length; longer literals are cut with an ellipsis |
| `search_literal_limit` | integer | `10` | Max matched literals rendered per file in `search` detail view |
| `search_anchor_snippet_limit` | integer | `3` | Max anchor symbols given a full snippet per file in `search` detail view; further (lower-ranked) anchors degrade to a ≤3-line signature |
| `caller_context_default` | bool | `true` | `search` caller/callee annotation default when the per-call parameter is omitted |
| `scan_cap` | integer | `500` | Hit budget per caller-annotation scan, split across scanned names (floor 25/name) |
| `caller_list_cap` | integer | `5` | Max callers (or non-call references) rendered per symbol |
| `callee_list_cap` | integer | `5` | Max callees rendered per symbol |
| `annotation_sub_budget` | integer (bytes) | `8192` | Annotation byte budget within `search_detail_byte_cap` |
| `common_name_threshold` | integer | `2` | Defs-per-name count at which caller/callee lists carry an ambiguity label |
| `caller_omit_def_threshold` | integer | `5` | Defs-per-name count at which a matched function's caller list is omitted (attribution unresolvable; a `grep` pointer is emitted instead). Callees unaffected |

### Indexing

- **`index_path`** — where the tantivy index lives, relative to the repo root. The default keeps it inside the repo-local `.codemap/` directory. The index location is always excluded from walking and from watcher events, so the index never indexes (or re-triggers) itself, including at a custom location.
- **`max_file_size`** — files larger than this many bytes are silently skipped before read/parse/index. The cap exists to keep minified bundles and generated blobs out of the symbol index; such files remain reachable via `read`/`find`/`grep`.
- **`excluded_directories`** — directory names that are never walked, **added** to the built-ins (`node_modules`, `target`, `dist`, `build`, `vendor`, `.git`, `.codemap`, …). This augments the built-in list; built-ins cannot be removed.

### Search output

- **`result_threshold`** — number of top-ranked files that `search` renders with per-file symbol details before any remaining matches move to the compact ranked tail.
- **`search_overview_file_limit`** — caps how many file headers `search` emits in that ranked tail. Output-size only — safe to tune.
- **`search_detail_snippet_max_lines`** — per-symbol snippet line cap in the detail view for those top-ranked files. A function body longer than this is truncated with an elision marker. Output-size only — safe to tune (default 80).
- **`search_detail_symbol_limit`** — max symbols rendered per file in the detail view. Symbols beyond the cap are replaced by a "N more not shown" note. Output-size only (default 20).
- **`search_detail_byte_cap`** — hard byte ceiling for one `search` response. The final output, including the partial-output footer, is kept within this cap. Partial output tells the caller to narrow the query or read the listed ranges; `search` does not expose a public page-offset parameter. Output-size only (default 32768 ≈ 32 KiB).
- **`search_literal_max_len`** — matched-literal truncation length in characters. A literal value longer than this is cut with an ellipsis in the detail view. Output-size only (default 200).
- **`search_literal_limit`** — max matched literals rendered per file in the detail view. Output-size only (default 10).
- **`search_anchor_snippet_limit`** — per-file cap on how many anchor symbols (exact-name Tier-1 hits, or the Tier-2 fallback when a file has no Tier-1) receive a full snippet in the detail view. Anchors ranked beyond the cap are demoted to a ≤3-line signature with a `… (N more lines)` marker rather than a one-line stub, so a broad query on a common name (`save`, `send`) can't flood the response with many full snippets. A file whose anchor count is at or below the cap is unaffected. Output-size only (default 3).

### Tool output limits

- **`read_output_byte_cap`** — always-applied output ceiling for `read` in bytes. Even with `offset`/`limit` set, a `read` whose rendered output (including line-number prefixes) would exceed this throws rather than emitting an unbounded blob. Oversized errors include a concrete narrower `offset`/`limit` suggestion. This approximates Claude Code's ~25,000-token cap and is distinct from the 256 KiB whole-file cap that applies only when `limit` is omitted (default 102400 ≈ 100 KiB).
- **`grep_max_columns`** — column cap for `grep` content-mode output. A matched line wider than this many characters is replaced with `[Omitted long matching line]`, matching Claude Code's `--max-columns 500` default. Partial `grep` pages include the shown range, total count, and `next_offset`. Set `0` to disable the column cap (default 500).

### Filesystem permissions

`read`, `find`, and `grep` are live filesystem tools. By default, each tool is confined to the current workspace root and rejects `..` traversal or absolute paths that resolve outside the workspace. The `[filesystem_permissions]` table lets you widen access per tool:

- **`workspace`** — workspace only. This is the default for `find`, `grep`, and `read`.
- **`allowed_roots`** — workspace plus the canonicalized paths listed in `allowed_roots`.
- **`anywhere`** — full-disk access for that tool. Use this only when the MCP server already runs in an environment where broad local file access is acceptable.

`allowed_roots` entries are canonicalized before matching, so symlink and `..` resolution cannot broaden access beyond the configured roots. `search` and `overview` are index-backed and keep their workspace indexing scope; this permission table applies only to `read`, `find`, and `grep`.

### Caller/callee context

- **`caller_context_default`** — whether `search`'s detail view annotates each matched function with its depth-1 callers and callees (approximate, name-match only) when the per-call `caller_context` parameter is omitted. Default `true`; an explicit per-call parameter always wins over this key.
- **`scan_cap`** — overall hit-collection budget for the single combined-regex workspace scan behind one annotation pass, distributed across the scanned names (per-name cap = `scan_cap / names`, floored at 25) so one hot name cannot starve the others; a name exhausting its share marks its own caller list as truncated.
- **`caller_list_cap` / `callee_list_cap`** — per-symbol render caps; overflow becomes a "… N more not shown" note. Output-size only — safe to tune.
- **`annotation_sub_budget`** — byte budget for all annotations in one response, counted WITHIN `search_detail_byte_cap` (snippets keep priority). An annotation that cannot fit degrades to a one-line omission marker rather than disappearing silently.
- **`common_name_threshold`** — a function name with at least this many definitions in the index gets its caller list and callee entries labeled attribution-ambiguous (a name-match scan cannot tell which definition a site targets); the lists are still rendered, never suppressed.
- **`caller_omit_def_threshold`** — stricter than `common_name_threshold`: a matched function name with at least this many definitions has its caller list *suppressed*, not just labeled — a name-match scan cannot attribute call sites among that many same-named definitions, so a one-line omission note (with the def count and a `grep "name("` pointer) replaces it. Callees are unaffected; the scan itself is unchanged.

### Ignore handling

- **`use_git_exclude`** — dedicated toggle for **`.git/info/exclude` only**. Set `false` to let index/codemap/`find`/`grep` see files hidden solely by `.git/info/exclude` (e.g. local personal excludes) while `.gitignore`, the global gitignore, and `.codemapignore` stay honored. The per-call `include_ignored` argument on `find`/`grep` is the broader override that bypasses every ignore source for that call.

### Index freshness

- **`watch`** — when `true` (the default), a filesystem watcher refreshes the index in the background on its own: ordinary edits become path-scoped incremental updates, and `search`/`overview` never trigger a tree walk. When `false` — or when the watcher fails to start or dies — the server falls back to the request-triggered lazy refresh below.
- **`watch_debounce_ms`** — events arriving within this window are batched into one incremental refresh, so a save-burst (formatter, branch switch) costs one pass instead of one per file.
- **`index_staleness_ms`** — debounce for the **fallback** refresh path, active only when the watcher is off or unavailable: within this window, repeated `search`/`overview` calls enqueue at most one background refresh, and every call answers immediately from the last committed snapshot. `read`/`find`/`grep` always read live disk, so brief search staleness is corrected by the follow-up read.
- **`indexer_auto_restart`** — when `true` (the default) and the background indexer thread dies, the next `search`/`overview` rebuilds the index engine, respawns the indexer, and re-attaches the watcher. Restarts are capped per server run so a deterministic crash cannot respawn-loop. Set `false` to instead serve results frozen at the last commit until the server is restarted.

## Example `config.toml`

```toml
# Every key is optional; omitted keys use the defaults above.

index_path = ".codemap/index"
result_threshold = 5
max_file_size = 1048576   # 1 MiB
excluded_directories = ["__pycache__", ".next", "coverage"]
use_git_exclude = true
index_staleness_ms = 5000
search_overview_file_limit = 12
watch = true
watch_debounce_ms = 500
indexer_auto_restart = true
grep_max_columns = 500
read_output_byte_cap = 102400             # 100 KiB
search_detail_snippet_max_lines = 80
search_detail_symbol_limit = 20
search_detail_byte_cap = 32768            # 32 KiB
search_literal_max_len = 200
search_literal_limit = 10
search_anchor_snippet_limit = 3

[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"
allowed_roots = []
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
