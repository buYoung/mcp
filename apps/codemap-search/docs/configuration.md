# Configuration

codemap-search configuration is **optional**. If no TOML file exists, compiled-in defaults reproduce the built-in behavior exactly. Add a key only when you want to override one part of that behavior.

한국어 요약: 설정 파일은 없어도 됩니다. 값을 바꾸고 싶은 키만 TOML 파일에 적으면 되고, 나머지는 기본값이나 더 낮은 우선순위 설정에서 그대로 이어받습니다.

## Files and precedence

Config is read from two layers and merged **per key** as `repo > global > default`. Use the repo file for project-specific behavior; use the global file only for defaults you want across repositories.

| Layer | Path |
|---|---|
| Repo | `<repo>/.codemap/config.toml` |
| Global | `$CODEMAP_HOME/config.toml`, else `~/.codemap/config.toml` |

"Per key" means a repo file that sets only `result_threshold` still inherits every other key from the global file (if set there) or the default. Layers are not all-or-nothing.

한국어 참고: 저장소 설정은 그 저장소 안에서만 우선합니다. 예를 들어 저장소 파일에 `result_threshold`만 있으면 다른 키는 전역 설정이나 기본값을 계속 사용합니다.

## Loader behavior

- **Never-exit:** a missing file, parse error, unknown key, or wrong-typed value warns to stderr and falls back for that key. The server does not exit because of config.
- **Auto-generated template:** on `mcp` startup, if `<repo>/.codemap/config.toml` is absent, a commented, no-op template is created. Every key is commented out at its default, so the file changes nothing until you uncomment a line. The file is stamped with a schema-version marker (see below).
- **Incremental sync:** if the file already exists, it is kept in sync with the schema. When a release adds a new key, that key's commented block is appended to your existing file and the version marker is refreshed. The sync is strictly additive: your existing lines (set values and comments alike) are never edited, reordered, or removed, and a file already at the current version is left untouched.
- **Validation:** most numeric keys must be positive integers; `grep_max_columns` also accepts `0` to disable its column cap. `index_path` must be a non-empty string, arrays must contain strings, and filesystem permission policies must be `workspace`, `allowed_roots`, or `anywhere`. An invalid value warns and falls back for that key.

한국어 요약: 설정 오류는 서버 종료로 이어지지 않습니다. 잘못된 키나 값은 stderr 경고를 내고 해당 키만 기본값으로 돌아갑니다. 저장소 설정 파일 자동 동기화는 주석 줄 추가만 수행하며 기존 값은 바꾸지 않습니다.

### Schema version and incremental updates

The first line of a generated config file is a version marker — a comment, not a key, so it never affects parsing:

```toml
# codemap-config-version: 1
```

This marker lets `mcp` keep an existing config file current as the tool evolves. On startup:

- If the file is **absent**, the commented template is written, stamped with the current version.
- If the file's version is **older** than the tool's, each key introduced since that version is appended as a commented block (placed before the `[filesystem_permissions]` table so top-level keys keep their scope), and the marker is bumped. A key you have already added or uncommented is detected and never duplicated.
- If the file is **already current**, it is left byte-for-byte unchanged.
- A file with **no marker** (written before versioning existed) is treated as the baseline version and synced the same way; nothing is duplicated because every key it already carries is detected first.

Because the sync edits your existing file in place, the change is visible in `git status` if you track `.codemap/`. The edits only add commented lines, so behavior is unchanged until you uncomment a key. Editing the marker yourself is unnecessary — the tool manages it; deleting it only causes the file to be re-synced from the baseline (still non-destructive). The global file (`$CODEMAP_HOME/config.toml`) is not auto-generated and is never synced; only the repo-local file is managed.

한국어 참고: 버전 마커는 설정 키가 아니라 주석입니다. 저장소 파일에 새 키가 주석으로 추가되어도, 사용자가 해당 줄을 직접 활성화하기 전까지 동작은 바뀌지 않습니다.

## Key reference

Use this table as the source of truth for supported keys, accepted types, and defaults. Detailed notes below group the same keys by operational purpose.

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
| `navigation_context_default` | bool | `false` | Enable tree-sitter precise caller/callee attribution when the structural navigation data confirms exactly one target |
| `navigation_callsite_budget` | integer | `1000` | Max navigation call sites inspected in one annotation pass before falling back to approximate annotations |
| `navigation_store_references` | bool | `false` | Store reference-site observations in `NavigationFile` when reference extraction is enabled |
| `scan_cap` | integer | `500` | Hit budget per caller-annotation scan, split across scanned names (floor 25/name) |
| `caller_list_cap` | integer | `5` | Max callers (or non-call references) rendered per symbol |
| `callee_list_cap` | integer | `5` | Max callees rendered per symbol |
| `annotation_sub_budget` | integer (bytes) | `8192` | Annotation byte budget within `search_detail_byte_cap` |
| `common_name_threshold` | integer | `2` | Defs-per-name count at which caller/callee lists carry an ambiguity label |
| `caller_omit_def_threshold` | integer | `5` | Defs-per-name count at which a matched function's caller list is omitted (attribution unresolvable; a `grep` pointer is emitted instead). Callees unaffected |

### Korean key guide

- 색인 위치와 범위: `index_path`, `max_file_size`, `excluded_directories`, `use_git_exclude`
- 검색 결과 크기: `result_threshold`, `search_overview_file_limit`, `search_detail_snippet_max_lines`, `search_detail_symbol_limit`, `search_detail_byte_cap`, `search_literal_max_len`, `search_literal_limit`, `search_anchor_snippet_limit`
- 실시간 파일 도구 제한: `grep_max_columns`, `read_output_byte_cap`, `[filesystem_permissions]`
- 색인 최신성: `watch`, `watch_debounce_ms`, `index_staleness_ms`, `indexer_auto_restart`
- 호출자/호출 대상 주석: `caller_context_default`, `navigation_context_default`, `navigation_callsite_budget`, `navigation_store_references`, `scan_cap`, `caller_list_cap`, `callee_list_cap`, `annotation_sub_budget`, `common_name_threshold`, `caller_omit_def_threshold`

### Indexing

- **`index_path`** — where the tantivy index lives, relative to the repo root. The default keeps it inside the repo-local `.codemap/` directory. The index location is always excluded from walking and from watcher events, so the index never indexes (or re-triggers) itself, including at a custom location.
- **`max_file_size`** — files larger than this many bytes are silently skipped before read/parse/index. The cap exists to keep minified bundles and generated blobs out of the symbol index; such files remain reachable via `read`/`find`/`grep`.
- **`excluded_directories`** — directory names that are never walked, **added** to the built-ins (`node_modules`, `target`, `dist`, `build`, `vendor`, `.git`, `.codemap`, …). This augments the built-in list; built-ins cannot be removed.

한국어 요약: 색인 설정은 “무엇을 인덱싱할지”를 정합니다. 큰 생성물이나 번들 파일은 `max_file_size`와 `excluded_directories`로 색인에서 빼고, 필요하면 `read`/`find`/`grep`으로 직접 확인합니다.

### Search output

- **`result_threshold`** — number of top-ranked files that `search` renders with per-file symbol details before any remaining matches move to the compact ranked tail.
- **`search_overview_file_limit`** — caps how many file headers `search` emits in that ranked tail. Output-size only — safe to tune.
- **`search_detail_snippet_max_lines`** — per-symbol snippet line cap in the detail view for those top-ranked files. A function body longer than this is truncated with an elision marker. Output-size only — safe to tune (default 80).
- **`search_detail_symbol_limit`** — max symbols rendered per file in the detail view. Symbols beyond the cap are replaced by a "N more not shown" note. Output-size only (default 20).
- **`search_detail_byte_cap`** — hard byte ceiling for one `search` response. The final output, including the partial-output footer, is kept within this cap. Partial output tells the caller to narrow the query or read the listed ranges; `search` does not expose a public page-offset parameter. Output-size only (default 32768 ≈ 32 KiB).
- **`search_literal_max_len`** — matched-literal truncation length in characters. A literal value longer than this is cut with an ellipsis in the detail view. Output-size only (default 200).
- **`search_literal_limit`** — max matched literals rendered per file in the detail view. Output-size only (default 10).
- **`search_anchor_snippet_limit`** — per-file cap on how many anchor symbols (exact-name Tier-1 hits, or the Tier-2 fallback when a file has no Tier-1) receive a full snippet in the detail view. Anchors ranked beyond the cap are demoted to a ≤3-line signature with a `… (N more lines)` marker rather than a one-line stub, so a broad query on a common name (`save`, `send`) can't flood the response with many full snippets. A file whose anchor count is at or below the cap is unaffected. Output-size only (default 3).

한국어 요약: 이 그룹은 검색 품질 자체보다 응답 크기와 읽기 쉬운 정도를 조절합니다. 넓은 질의가 너무 많은 상세 스니펫을 내보낼 때 낮추고, 충분한 문맥이 필요할 때 조심해서 올립니다.

### Tool output limits

- **`read_output_byte_cap`** — always-applied output ceiling for `read` in bytes. Even with `offset`/`limit` set, a `read` whose rendered output (including line-number prefixes) would exceed this throws rather than emitting an unbounded blob. Oversized errors include a concrete narrower `offset`/`limit` suggestion. This approximates Claude Code's ~25,000-token cap and is distinct from the 256 KiB whole-file cap that applies only when `limit` is omitted (default 102400 ≈ 100 KiB).
- **`grep_max_columns`** — column cap for `grep` content-mode output. A matched line wider than this many characters is replaced with `[Omitted long matching line]`, matching Claude Code's `--max-columns 500` default. Partial `grep` pages include the shown range, total count, and `next_offset`. Set `0` to disable the column cap (default 500).

한국어 요약: `read`와 `grep`은 실제 파일 시스템을 읽으므로 출력 상한이 안전장치입니다. 너무 큰 결과는 한 번에 내보내지 말고 줄 범위나 다음 페이지로 나누어 읽는 흐름을 권장합니다.

### Filesystem permissions

`read`, `find`, and `grep` are live filesystem tools. By default, each tool is confined to the current workspace root and rejects `..` traversal or absolute paths that resolve outside the workspace. The `[filesystem_permissions]` table lets you widen access per tool:

- **`workspace`** — workspace only. This is the default for `find`, `grep`, and `read`.
- **`allowed_roots`** — workspace plus the canonicalized paths listed in `allowed_roots`.
- **`anywhere`** — full-disk access for that tool. Use this only when the MCP server already runs in an environment where broad local file access is acceptable.

`allowed_roots` entries are canonicalized before matching, so symlink and `..` resolution cannot broaden access beyond the configured roots. `search` and `overview` are index-backed and keep their workspace indexing scope; this permission table applies only to `read`, `find`, and `grep`.

한국어 요약: 기본값은 작업공간 안으로 제한됩니다. 외부 소스 트리를 읽어야 하면 `anywhere`보다 `allowed_roots`를 먼저 쓰고, 전체 디스크 접근은 MCP 서버 실행 환경이 이미 격리되어 있을 때만 선택하세요.

### Caller/callee context

- **`caller_context_default`** — whether `search`'s detail view annotates each matched function with its depth-1 callers and callees when the per-call `caller_context` parameter is omitted. Default `true`; an explicit per-call parameter always wins over this key.
- **`navigation_context_default`** — whether caller/callee annotations may use tree-sitter navigation observations for precise attribution. Default `false`: structural calls may still feed conservative approximate output, but `(precise)` labels and import/source/receiver narrowing stay disabled unless this is enabled.
- **`navigation_callsite_budget`** — max navigation call sites inspected in one annotation pass before the navigation path falls back to the existing approximate scan path. Default `1000`.
- **`navigation_store_references`** — whether extracted `NavigationFile` values store non-call reference observations. Default `false`; the first navigation layer uses calls/imports/local bindings for attribution.
- **`scan_cap`** — overall hit-collection budget for the single combined-regex workspace scan behind one annotation pass, distributed across the scanned names (per-name cap = `scan_cap / names`, floored at 25) so one hot name cannot starve the others; a name exhausting its share marks its own caller list as truncated.
- **`caller_list_cap` / `callee_list_cap`** — per-symbol render caps; overflow becomes a "… N more not shown" note. Output-size only — safe to tune.
- **`annotation_sub_budget`** — byte budget for all annotations in one response, counted WITHIN `search_detail_byte_cap` (snippets keep priority). An annotation that cannot fit degrades to a one-line omission marker rather than disappearing silently.
- **`common_name_threshold`** — a function name with at least this many definitions in the index gets its caller list and callee entries labeled attribution-ambiguous (a name-match scan cannot tell which definition a site targets); the lists are still rendered, never suppressed.
- **`caller_omit_def_threshold`** — stricter than `common_name_threshold`: a matched function name with at least this many definitions has its caller list *suppressed*, not just labeled — a name-match scan cannot attribute call sites among that many same-named definitions, so a one-line omission note (with the def count and a `grep "name("` pointer) replaces it. Callees are unaffected; the scan itself is unchanged.

한국어 요약: 호출자/호출 대상 주석은 기본적으로 보수적 근사치입니다. `navigation_context_default`를 켜면 tree-sitter 구조, import/source 해석, receiver 힌트가 단일 대상만 확정할 때만 정밀 표시를 붙입니다. 같은 이름의 정의가 많을수록 귀속이 모호해지므로, 임계값은 “더 많이 보여줄지”보다 “혼동될 정보를 얼마나 줄일지”를 정하는 설정입니다.

### Ignore handling

- **`use_git_exclude`** — dedicated toggle for **`.git/info/exclude` only**. Set `false` to let index/codemap/`find`/`grep` see files hidden solely by `.git/info/exclude` (e.g. local personal excludes) while `.gitignore`, the global gitignore, and `.codemapignore` stay honored. The per-call `include_ignored` argument on `find`/`grep` is the broader override that bypasses every ignore source for that call.

한국어 요약: `use_git_exclude`는 `.git/info/exclude`만 대상으로 합니다. `.gitignore`, 전역 gitignore, `.codemapignore`까지 무시하고 싶다면 설정 키가 아니라 `find`/`grep` 호출의 `include_ignored` 인자를 사용합니다.

### Index freshness

- **`watch`** — when `true` (the default), a filesystem watcher refreshes the index in the background on its own: ordinary edits become path-scoped incremental updates, and `search`/`overview` never trigger a tree walk. When `false` — or when the watcher fails to start or dies — the server falls back to the request-triggered lazy refresh below.
- **`watch_debounce_ms`** — events arriving within this window are batched into one incremental refresh, so a save-burst (formatter, branch switch) costs one pass instead of one per file.
- **`index_staleness_ms`** — debounce for the **fallback** refresh path, active only when the watcher is off or unavailable: within this window, repeated `search`/`overview` calls enqueue at most one background refresh, and every call answers immediately from the last committed snapshot. `read`/`find`/`grep` always read live disk, so brief search staleness is corrected by the follow-up read.
- **`indexer_auto_restart`** — when `true` (the default) and the background indexer thread dies, the next `search`/`overview` rebuilds the index engine, respawns the indexer, and re-attaches the watcher. Restarts are capped per server run so a deterministic crash cannot respawn-loop. Set `false` to instead serve results frozen at the last commit until the server is restarted.

한국어 요약: watcher가 정상일 때는 변경 사항을 백그라운드에서 반영합니다. watcher를 끄거나 사용할 수 없으면 `index_staleness_ms`가 요청 기반 갱신의 디바운스 역할을 합니다. 방금 수정한 파일을 즉시 확인해야 하면 `read`/`find`/`grep`이 항상 실제 디스크를 읽습니다.

## Example `config.toml`

The example below is intentionally explicit. In a real file, you can keep only the keys you want to override.

한국어 참고: 실제 설정 파일에는 바꿀 키만 남겨도 됩니다. 생략한 키는 전역 설정이나 기본값을 사용합니다.

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
caller_context_default = true
navigation_context_default = false
navigation_callsite_budget = 1000
navigation_store_references = false

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

한국어 요약: `.codemap/`에는 저장소별 색인과 설정이 함께 있습니다. 도구가 git ignore 파일을 직접 수정하지는 않으므로, `git status`에서 숨기려면 사용자가 `.gitignore`나 `.git/info/exclude`에 `.codemap/`을 추가해야 합니다.
