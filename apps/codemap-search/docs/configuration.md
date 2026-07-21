# Configuration

codemap-search configuration is **optional**. If no TOML file exists, compiled-in defaults reproduce the built-in behavior exactly. Add a setting only when you want to override one part of that behavior.

한국어 요약: 설정 파일은 없어도 됩니다. 값을 바꾸고 싶은 키만 TOML 파일에 적으면 되고, 나머지는 기본값이나 더 낮은 우선순위 설정에서 그대로 이어받습니다.

## Files and precedence

Config is read from two layers and merged **per key** as `repo > global > default`. Use the repo file for project-specific behavior; use the global file only for defaults you want across repositories.

| Layer | Path |
|---|---|
| Repo | `<repo>/.codemap/config.toml` |
| Global | `$CODEMAP_HOME/config.toml`, else `~/.codemap/config.toml` |

"Per key" means a repo file that sets only `[search].result_threshold` still inherits every other setting from the global file (if set there) or the default. Layers are not all-or-nothing.

한국어 참고: 저장소 설정은 그 저장소 안에서만 우선합니다. 예를 들어 저장소 파일에 `[search].result_threshold`만 있으면 다른 설정 항목은 전역 설정이나 기본값을 계속 사용합니다.

## Loader behavior

- **Never-exit:** a missing file, parse error, unknown key, or wrong-typed value warns to stderr and falls back for that key. The server does not exit because of config.
- **Auto-generated template:** on `mcp` startup, if `<repo>/.codemap/config.toml` is absent and `[update].config_auto_update` is true, an explicit-default template is created. The generated file uses TOML sections (`[update]`, `[index]`, `[search]`, etc.) and live default values, so the repo file pins those defaults above the global config until you delete or comment out a setting. The file is stamped with a schema-version marker (see below).
- **Generated comment language:** the generated repo template and future commented schema-sync blocks use Korean comments when the OS preferred locale is Korean (`ko`, `ko-KR`, `ko_KR`, etc.). English is used for every other locale, unknown locale, provider failure, `C`, `POSIX`, and unsupported platforms. Only comments change; TOML keys, values, parsing, precedence, and schema version stay the same.
- **Incremental sync:** if the file already exists and `[update].config_auto_update` is true, it is kept in sync with the schema. When a release adds a new key, that key's commented block is appended to your existing file and the version marker is refreshed. The sync is strictly additive: your existing lines (set values and comments alike) are never edited, reordered, or removed, and a file already at the current version is left untouched.
- **Validation:** most numeric keys must be positive integers; `grep_max_columns` also accepts `0` to disable its column cap. `index_path` must be a non-empty string, arrays must contain strings, and filesystem permission policies must be `workspace`, `allowed_roots`, or `anywhere`. An invalid value warns and falls back for that key.

한국어 요약: 설정 오류는 서버 종료로 이어지지 않습니다. 잘못된 키나 값은 stderr 경고를 내고 해당 키만 기본값으로 돌아갑니다. 저장소 설정 파일 자동 생성/동기화는 `[update].config_auto_update`로 끄고 켤 수 있습니다. 한국어 로케일에서는 생성되는 설명 주석만 한국어가 되며, 설정 키와 값의 의미는 그대로입니다. 자동 동기화가 켜져 있어도 기존 파일에는 새 설정이 주석 블록으로만 추가되어 기존 값은 바꾸지 않습니다.

### Schema version and incremental updates

The first line of a generated config file is a version marker — a comment, not a key, so it never affects parsing:

```toml
# codemap-config-version: 1
```

This marker lets `mcp` keep an existing config file current as the tool evolves. On startup, when `[update].config_auto_update` resolves to `true`:

- If the file is **absent**, the explicit-default template is written, stamped with the current version.
- If the file's version is **older** than the tool's, each key introduced since that version is appended as a commented block at its schema-defined placement, and the marker is bumped. A key you have already added or uncommented is detected and never duplicated.
- If the file is **already current**, it is left byte-for-byte unchanged.
- A file with **no marker** (written before versioning existed) is treated as the baseline version and synced the same way; nothing is duplicated because every key it already carries is detected first.

Because the sync edits your existing file in place, the change is visible in `git status` if you track `.codemap/`. Incremental syncs only add commented lines, so existing files keep their behavior until you uncomment a newly added setting. Editing the marker yourself is unnecessary — the tool manages it; deleting it only causes the file to be re-synced from the baseline (still non-destructive). The global file (`$CODEMAP_HOME/config.toml`) is not auto-generated and is never synced; only the repo-local file is managed.

Set `[update].config_auto_update = false` to prevent codemap-search from creating or rewriting the repo-local config file. Existing repo and global config files are still read; only the automatic write path is disabled.

한국어 참고: 버전 마커는 설정 키가 아니라 주석입니다. 저장소 파일에 새 키가 주석으로 추가되어도, 사용자가 해당 줄을 직접 활성화하기 전까지 동작은 바뀌지 않습니다. `[update].config_auto_update = false`로 설정하면 저장소 설정 파일 자동 생성과 자동 동기화 쓰기만 멈추고, 기존 설정 파일 읽기는 계속 동작합니다.

## Key reference

Use this table as the source of truth for supported keys, accepted types, and defaults. The generated template uses the sectioned form shown here. Legacy top-level keys (for example `result_threshold = 5`) are still accepted for compatibility; when both forms appear in one file, the sectioned value wins.

| Key | Type | Default | Summary |
|---|---|---|---|
| `[update].config_auto_update` | bool | `true` | Create missing repo config and append commented schema-sync blocks on `mcp` startup |
| `[index].index_path` | string | `".codemap/index"` | Where the tantivy index lives (relative to the repo root) |
| `[index].max_file_size` | integer (bytes) | `1048576` (1 MiB) | Files larger than this are skipped before parse/index |
| `[index].excluded_directories` | string array | `[]` | Directory names excluded in addition to the built-ins |
| `[index].use_git_exclude` | bool | `true` | Whether walkers honor `.git/info/exclude` (that source only) |
| `[refresh].watch` | bool | `true` | Filesystem watcher (autonomous background index refresh) |
| `[refresh].watch_debounce_ms` | integer (ms) | `500` | Batching window for watcher events |
| `[refresh].index_staleness_ms` | integer (ms) | `5000` | Debounce for the request-triggered fallback refresh |
| `[refresh].indexer_auto_restart` | bool | `true` | Auto-recovery when the background indexer thread dies |
| `[search].result_threshold` | integer | `5` | Number of top-ranked files `search` renders as details before the ranked tail |
| `[search].search_overview_file_limit` | integer | `12` | Max file headers in `search`'s compact ranked tail |
| `[search].search_detail_snippet_max_lines` | integer | `80` | Per-symbol snippet line cap in `search` detail view; bodies longer than this are truncated |
| `[search].search_detail_symbol_limit` | integer | `20` | Max symbols rendered per file in `search` detail view; overflow becomes a summary note |
| `[search].search_detail_byte_cap` | integer (bytes) | `32768` | Hard byte ceiling for one `search` response, including the partial-output footer |
| `[search].search_literal_max_len` | integer (chars) | `200` | Matched-literal truncation length; longer literals are cut with an ellipsis |
| `[search].search_literal_limit` | integer | `10` | Max matched literals rendered per file in `search` detail view |
| `[search].search_anchor_snippet_limit` | integer | `3` | Max anchor symbols given a full snippet per file in `search` detail view; further (lower-ranked) anchors degrade to a ≤3-line signature |
| `[tool_output].grep_max_columns` | integer | `500` | `grep` content-mode column cap; matched lines wider than this are replaced with `[Omitted long matching line]`; `0` disables |
| `[tool_output].read_output_byte_cap` | integer (bytes) | `102400` | `read` always-applied output ceiling; a rendered output exceeding this throws instead of emitting an unbounded blob |
| `[filesystem_permissions].find` | string | `"workspace"` | Path policy for `find`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].grep` | string | `"workspace"` | Path policy for `grep`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].read` | string | `"workspace"` | Path policy for `read`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].allowed_roots` | string array | `[]` | Canonicalized external roots available to tools set to `allowed_roots` |
| `[caller_context].caller_context_default` | bool | `true` | `search` caller/callee annotation default when the per-call parameter is omitted |
| `[caller_context].navigation_context_default` | bool | `false` | Enable tree-sitter precise caller/callee attribution when the structural navigation data confirms exactly one target |
| `[caller_context].navigation_callsite_budget` | integer | `1000` | Max navigation call sites inspected in one annotation pass before falling back to approximate annotations |
| `[caller_context].navigation_store_references` | bool | `false` | Store reference-site observations in `NavigationFile` when reference extraction is enabled |
| `[caller_context].scan_cap` | integer | `500` | Hit budget per caller-annotation scan, split across scanned names (floor 25/name) |
| `[caller_context].caller_list_cap` | integer | `5` | Max callers (or non-call references) rendered per symbol |
| `[caller_context].callee_list_cap` | integer | `5` | Max callees rendered per symbol |
| `[caller_context].annotation_sub_budget` | integer (bytes) | `8192` | Annotation byte budget within `search_detail_byte_cap` |
| `[caller_context].common_name_threshold` | integer | `2` | Defs-per-name count at which caller/callee lists carry an ambiguity label |
| `[caller_context].caller_omit_def_threshold` | integer | `5` | Defs-per-name count at which a matched function's caller list is omitted (attribution unresolvable; a `grep` pointer is emitted instead). Callees unaffected |

### Korean key guide

- 색인 위치와 범위: `[index]`
- 설정 파일 자동 생성/동기화: `[update]`
- 색인 최신성: `[refresh]`
- 검색 결과 크기: `[search]`
- 실시간 파일 도구 제한: `[tool_output]`, `[filesystem_permissions]`
- 호출자/호출 대상 주석: `[caller_context]`

### Config file updates

- **`config_auto_update`** — controls only automatic writes to the repo-local `.codemap/config.toml` on `mcp` startup. When `true`, a missing repo config is created from the explicit-default template, and an older repo config receives newly introduced settings as commented blocks. When `false`, codemap-search does not create or rewrite the repo config; existing repo/global settings are still read normally.

한국어 요약: 자동 업데이트가 켜져 있으면 기존 설정 파일에는 새 키가 주석으로 추가됩니다. 바로 활성화되지는 않으며, 사용자가 주석을 해제해야 동작이 바뀝니다. 끄면 자동 생성과 자동 동기화 쓰기만 멈춥니다.

### Indexing

- **`index_path`** — where the tantivy index lives, relative to the repo root. The default keeps it inside the repo-local `.codemap/` directory. The index location is always excluded from walking and from watcher events, so the index never indexes (or re-triggers) itself, including at a custom location.
- **Workspace safety** — the user home directory itself cannot be an MCP workspace or an explicit `index`/`benchmark` target. Descendant project directories remain valid. The guard runs before repo config, index, or watcher state is created. If both `HOME` and `USERPROFILE` are unavailable, startup warns on stderr and continues.
- **Built-in file exclusions** — `.md`, `.mdx`, `.txt`, `*.lock`, known package-manager lockfile names, `*.map`, and minified/bundle suffixes are excluded case-insensitively from index, codemap, and caller scans. They are also hidden by default from `find`/`grep`; `include_ignored: true` restores explicit live-tool access, while direct `read`/`parse` remains available. Repo-specific additions belong in `.codemapignore`; the built-in semantic-index exclusions cannot be removed.
- **`max_file_size`** — files larger than this many bytes are silently skipped before read/parse/index. The cap remains a second guard against generated blobs; such files remain reachable via direct live filesystem tools.
- **`excluded_directories`** — directory names that are never walked, **added** to the built-ins (`node_modules`, `target`, `dist`, `build`, `vendor`, `.git`, `.codemap`, …). This augments the built-in list; built-ins cannot be removed.

한국어 요약: 사용자 홈 자체는 인덱싱 루트로 사용할 수 없지만 홈 아래 프로젝트는 허용합니다. 문서·잠금·source map·minified·bundle 파일은 의미 기반 인덱스에서 항상 제외됩니다. `find`/`grep`은 기본적으로 같은 제외를 따르며 `include_ignored: true`로 명시적 접근할 수 있고, 직접 `read`/`parse`도 유지됩니다. 큰 생성물은 `max_file_size`와 `excluded_directories`로 추가 차단합니다.

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

The example below is intentionally explicit. In a real file, you can keep only the settings you want to override.

한국어 참고: 실제 설정 파일에는 바꿀 키만 남겨도 됩니다. 생략한 키는 전역 설정이나 기본값을 사용합니다.

```toml
# Every setting is optional; omitted settings use the defaults above.

[update]
config_auto_update = true

[index]
index_path = ".codemap/index"
max_file_size = 1048576   # 1 MiB
excluded_directories = ["__pycache__", ".next", "coverage"]
use_git_exclude = true

[refresh]
watch = true
watch_debounce_ms = 500
index_staleness_ms = 5000
indexer_auto_restart = true

[search]
result_threshold = 5
search_overview_file_limit = 12
search_detail_snippet_max_lines = 80
search_detail_symbol_limit = 20
search_detail_byte_cap = 32768            # 32 KiB
search_literal_max_len = 200
search_literal_limit = 10
search_anchor_snippet_limit = 3

[tool_output]
grep_max_columns = 500
read_output_byte_cap = 102400             # 100 KiB

[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"
allowed_roots = []

[caller_context]
caller_context_default = true
navigation_context_default = false
navigation_callsite_budget = 1000
navigation_store_references = false
scan_cap = 500
caller_list_cap = 5
callee_list_cap = 5
annotation_sub_budget = 8192
common_name_threshold = 2
caller_omit_def_threshold = 5
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
