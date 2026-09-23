# codemap-search

[한국어](./README.ko.md) | English

A self-contained MCP stdio server and CLI for coding agents. Map a repository, search extracted symbols, documentation and literals with BM25, then confirm the source with embedded `read`, `find` and `grep`. Tree-sitter grammars, Tantivy and ripgrep libraries are compiled into one Rust binary; no system `rg`, language server, external runtime, account or API key is required. Only the opt-in [Jev judgments](#optional-jev-judgments) use an API key.

## Install

With Rust/Cargo installed:

```sh
cargo install codemap-search
codemap-search --version
```

Make sure `~/.cargo/bin` is on `PATH`. For a prebuilt macOS/Linux binary, follow the [installer guide](./docs/distribution/curl-installer.md). See [installation channels](./docs/distribution/index.md) for source builds, version selection, and Homebrew/WinGet availability.

## Register with an MCP client

Run `codemap-search mcp` with the **repository to inspect as its working directory**. A user-wide registration can reuse the same binary across projects, but the client must launch it in the intended project. The user home directory itself is refused; projects beneath it, such as `~/work/project`, are valid.

### Claude Code

For your account across projects:

```sh
claude mcp add --scope user codemap-search -- codemap-search mcp
```

For a team-shared project entry in `.mcp.json`, run from that project:

```sh
claude mcp add --scope project codemap-search -- codemap-search mcp
```

Without `--scope`, Claude Code uses `local`: private to you in the current project, stored under its path in `~/.claude.json`. It is not the shared `project` scope. See [Claude Code's MCP scope reference](https://code.claude.com/docs/en/mcp).

### Codex

```sh
codex mcp add codemap-search -- codemap-search mcp
```

Or add the same server entry to `~/.codex/config.toml`:

```toml
[mcp_servers.codemap-search]
command = "codemap-search"
args = ["mcp"]
```

### OpenCode

Use global `~/.config/opencode/opencode.json` or project `opencode.json`:

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

This is the configuration form in the [OpenCode MCP guide](https://opencode.ai/docs/mcp-servers/). Consult the matching client-version documentation when using a different config schema.

## Verify the first connection

1. Ask the client to call `initial_instructions` once. It returns navigation guidance and the root overview; monorepos include selectable scopes and their languages.
2. Confirm that the paths belong to your intended repository. A warming notice means indexing is still in progress; retry `overview` after it completes.
3. Find a known source file with `find`, then `read` its path. Search for a known symbol with `search` and confirm that the same file appears after indexing completes.

If the binary cannot be found, check the client's `PATH`. If the wrong repository appears, correct its working directory. Read stderr diagnostics for startup/config errors; stdout is reserved for MCP JSON-RPC frames.

## Use the navigation tools

| Tool | Use | Main arguments |
|---|---|---|
| `initial_instructions` | Load navigation guidance once | none |
| `overview` | Inspect repository, folder or file structure; repository-root and monorepo workspace-root output include indexed-file language statistics by default | `path`, `format`, `task_query` |
| `search` | Find implementations with ranked symbols and snippets | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context`, `task_query` |
| `find` | Find files or directories by glob or basename regex; newest entries first | `pattern`, `path`, `include_ignored`, `entry_type`, `max_depth`, `pattern_type` |
| `grep` | Search live files with a regex | `pattern`, `path`, `glob`, `type`, `output_mode`, `-i`, `-n`, `-A`, `-B`, `-C`, `multiline`, `head_limit`, `offset`, `include_ignored` |
| `read` | Read live source with line numbers | `file_path`, `offset`, `limit` |
| `analyze` | Inspect index footprint or recorded reading activity as compact JSON | `target`, `limit`, `offset`, `sort`, `filter`, `view`, `days`, `tool` |

Repository-root and monorepo workspace-root `overview` output includes indexed-file language statistics by default. A project root counts only that project's indexed files. Set `[output.overview].is_stats_enabled = false` to omit that section. Unavailable or pending files make the result explicitly partial.

Use `search` for behavior or unknown implementation locations; use `grep` for exact identifiers, comments and just-edited content. `grep.pattern` is a regex, so escape metacharacters for literal code. JSON escaping is a separate layer. `grep` defaults to numbered `content`; `files_with_matches` and `count` return paths or counts. `read` also accepts `path`/`file` and 1-based inclusive `start_line`/`end_line` aliases.

In monorepos, `overview` on a directory selects that exact scope for later `search`; a file selects its parent. Explicit `workspace_scope` overrides it; `all`/`전체` selects the whole repository. If the implementation scope is unknown, start with read-only repo-wide discovery and narrow from returned paths. Search never silently widens a chosen scope. Top matches have detailed snippets; the compact tail is bounded. Narrow the query or follow the supplied read ranges when output is partial.

MCP `read`/`grep` use declaration-kind/name headings within each file, followed by one source section. Folder `overview` retains its file list and adds source-backed signatures and nested fields/methods. Resolved callees include their definition file and line. Same-file constant references include definition locations and initializer previews; ambiguous names are omitted. Indexed context can lag recent edits.

Relevant source wrappers also show bounded argument, return, closure, field and callback-use relationships automatically. Default value context groups repeated paths and passing locations; `debug: true` exposes bounded detailed evidence and diagnostics. Basic conditional paths and native tuples are summarized within the existing budgets. These work independently of event API rules and distinguish source evidence, built-in models and unresolved candidates. Stale dependencies are withheld. Composite queries prioritize term coverage and bounded body evidence while exact identifier queries keep exact-name preference. See the [value-navigation reference and CLI examples](./docs/value-navigation.ko.md) for supported cases, limits and verification commands.

Tools are read-only over their configured filesystem scope. The server itself writes its index, file-content response records and, when enabled, repo configuration. No MCP resources or prompts are registered. `task_query` is used only by the [Jev modes](#optional-jev-judgments); a tool whose mode is off (the default) does not use it and sends nothing over the network.

## Configure exclusions and output

Settings are read per key from `<repo>/.codemap/config.toml`, then `$CODEMAP_HOME/config.toml` (default `~/.codemap/config.toml`), then built-in defaults. An active repo key overrides its global value; comment it out to inherit instead.

MCP responses mask detected API keys, tokens, passwords and private keys by default. `[output].is_redact_enabled = false` disables masking. Matching and ranking still use original data; files and local indexes are unchanged. Detection combines Tree-sitter with pattern rules; `[output.redact]` adds sensitive field names, custom regexes and exact-value exceptions. Unfamiliar formats can still be missed. See [credential redaction](./docs/configuration.md#credential-redaction) for coverage and limits.

Automatic symbol/call context excludes test regions by default. Set `[output.context.exclude].should_include_test_code = true` to include them. `test_file_patterns` and the per-language `test_attributes`, `test_decorators`, and `test_calls` lists let you add custom rules or remove built-ins; each explicit list replaces its inherited value and `[]` disables it. Live `read`/`grep` source is preserved. See [test-code context](./docs/configuration.md#test-code-context) for defaults and examples.

On first MCP startup, a missing repo file is generated with **common exclusions plus recursive globs for detected project types**. Common names include `.git`, `.idea`, `.vscode`, `.vs`, `.codemap`, and other supported VCS internals. A JS/TS project adds `**/node_modules`, `**/dist`, `**/build`, framework outputs and caches; Python, Rust and other build systems add their corresponding globs. Each pattern appears once and applies throughout the workspace, regardless of where the project was detected.

```toml
[index.exclude]
# Example for a mixed repository; keep the entries you need.
excluded_directories = [
    ".git", ".idea", ".vscode", ".vs", ".codemap", ".codemap-index",
    "**/node_modules", "**/dist", "**/build",
    "**/.venv", "**/__pycache__",
    "**/target",
]
```

**Pre-v6 configs migrate once. From `codemap-config-version: 6` onward, this array is never automatically updated. Manage it yourself when new rules are needed.** Deleting an entry does not cause it to return on restart. `config_auto_update` still controls automatic file creation and ordinary schema additions, not this post-v6 array. With automatic writes disabled, follow the [manual transition](./docs/configuration.md#manual-transition-when-automatic-writes-are-disabled).

A bare `build` matches directories at any depth; `./build` means the workspace root; `apps/web/build` scopes it to that project. Explicit arrays replace optional defaults. `[]` clears optional directory rules; omitting the key inherits global/default rules. `.gitignore`, global Git ignores, `.git/info/exclude` and `.codemapignore` still apply. VCS internals, `.codemap`, `.codemap-index` and the actual index location remain excluded from walks regardless of the array. `find`/`grep` can bypass optional exclusions with `include_ignored: true`; direct `read` remains subject to filesystem permissions.

MCP watches existing config directories and reloads after about 1000ms. Manual exclusion or language-support changes request a full index refresh; output limits and filesystem permissions apply to subsequent requests. Restart after changing `index.path`, `index.refresh.watch` or `index.refresh.watch_debounce_ms`, or if config watching was unavailable. See the [full configuration reference](./docs/configuration.md) for every key, common folders, project detection rules, validation, permissions and application timing.

## Optional Jev judgments

Two independent modes can use the TypeSafe Jev API. Both are off by default; nothing is sent until a mode is enabled and a call passes `task_query`.

- **Overview recommendations** (`[analysis.jev].is_overview_enabled`): a repository-root `overview` appends up to 24 recommended indexed files with declarations and `read` windows.
- **Search filtering** (`[analysis.jev].is_search_filter_enabled`): `search` omits displayed bodies judged unrelated to the task. The default threshold, P(unrelated) >= 0.70, is provisional and uncalibrated. File headings, declaration rows and the ranked tail stay, and omitted bodies are listed with ranges to `read`.

Export the key in the environment that starts the MCP server, then enable a mode in the global config (`$CODEMAP_HOME/config.toml`, default `~/.codemap/config.toml`). Only the global file can change the key variable with `api_key_env`.

```bash
export TYPESAFE_API_KEY=...
```

```toml
[analysis.jev]
is_overview_enabled = true
is_search_filter_enabled = true
```

Pass the user's original request, not a reformulated search query:

```json
{"name": "search", "arguments": {"query": "upload retry attempts", "task_query": "Why does the upload retry stop after the third attempt?"}}
```

Enabled calls send `task_query` with bounded index metadata (overview) or displayed declaration bodies (search) to `api.typesafe.ai`, after the same masking as tool output; keep secrets out of `task_query`, whose free text is sent as written unless it matches a credential format. They end with an `applied`, `bypassed` or `fallback` line; applied and fallback lines report requests, input/output tokens and elapsed time separately. Missing `task_query` or key, index warm-up, errors and timeouts keep the regular output. The evaluator can also be called from Rust as `codemap_search::jev`; `cargo run --example jev_decisions -- --mock` shows Score, Choice and Noul questions offline. See [optional Jev judgments](./docs/configuration.md#optional-jev-judgments) for activation rules, transmitted data, limits and every setting.

## Supported languages and formats

| Language | Extensions |
|---|---|
| Rust | `.rs` |
| Python | `.py` |
| TypeScript / TSX | `.ts`, `.tsx`, `.mts`, `.cts` |
| JavaScript / JSX | `.js`, `.jsx`, `.mjs`, `.cjs` |
| Go | `.go` |
| Java | `.java` |
| Kotlin | `.kt`, `.kts` |
| C | `.c` |
| C++ | `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx` |
| C# | `.cs` |
| PHP | `.php` |
| Ruby | `.rb` |
| Lua | `.lua` |
| Assembly / GAS | `.s`, `.S`, `.asm` |
| Swift | `.swift` |
| Dart | `.dart` |
| Scala | `.scala`, `.sc` |
| Groovy / Gradle | `.groovy`, `.gradle` |
| PowerShell | `.ps1`, `.psm1` |
| SQL | `.sql` |

JSON/JSONC, TOML, YAML, HTML/XML derivatives, CSS/Less and Sass are supported, as are Vue, Astro and Svelte components. JSON5 and SCSS are not registered in this version. SQL extracts declarations and literals, without caller/callee relationships.

Optional groups default to `false` under `[index.language_support]`:

| Key | Group |
|---|---|
| `index.language_support.is_document_support_enabled` | Markdown `.md`, `.mdx` |
| `index.language_support.is_shell_support_enabled` | `.sh`, `.bash`, `.zsh` |
| `index.language_support.is_infrastructure_support_enabled` | `.hcl`, `.tf`, `.tfvars`, `Dockerfile`, `.nix` |
| `index.language_support.is_interface_support_enabled` | `.proto`, `.graphql`, `.gql` |
| `index.language_support.is_build_support_enabled` | `Makefile`, `.mk`, `CMakeLists.txt`, `.cmake`, `BUILD`, `BUILD.bazel`, `.bzl` |

These switches control index-backed discovery and watcher refreshes. Live `find`, `grep`, `read` and direct CLI `parse` remain available when a group is disabled. See [extraction details](./docs/language-support-checklist.md#extraction-details) for per-language visibility, test/deprecation flags, static relationships and limitations.

## CLI

```text
codemap-search mcp [--no-call-log]
codemap-search analyze index [--path DIR] [--sort stored|size|lines|symbols|literals|path] [OPTIONS]
codemap-search analyze reads [--path DIR] [--days 1..30] [--sort reads|bytes|size|last|path] [OPTIONS]
codemap-search parse <file>
codemap-search tokenize <ident>
codemap-search codemap [--path P] [--format F]
codemap-search search <query> [-l N]
codemap-search index [dir]
codemap-search benchmark --queries <json> [--dir D]
```

### Analyze the index and recent reading activity

`codemap-search analyze index` inspects an existing committed index immediately. `codemap-search analyze reads` reports the last seven days of reading activity at execution time. Human-readable tables are the default. Activity recording starts after reconnecting MCP with the updated binary; reports are generated on demand.

```sh
codemap-search analyze index --sort size --limit 10
codemap-search analyze index --path /path/to/repo --language rust --filter src/
codemap-search analyze reads --sort bytes --limit 20
codemap-search analyze reads --days 14 --tool search --filter src/
codemap-search analyze reads --offset 20 --limit 20 --sort bytes
codemap-search analyze reads --view summary --format json
codemap-search analyze index --help
codemap-search analyze reads --help
```

| Section | Output | Measurement |
| --- | --- | --- |
| Index footprint | Committed files, segments, deleted documents, disk size, stored JSON size, static call/reference sites | Existing Tantivy snapshot aggregated in an in-memory SQLite database |
| Languages and symbols | Files, lines, symbols, exported symbols, literals and docstrings by language; test/documentation flags by symbol kind | Stored extraction metadata, which may differ from the full repository or current source |
| Files and freshness | Largest stored records with path, size, lines, symbols, literals, largest literal and changed/missing/unavailable state | Current filesystem metadata for sizes and mtime comparison only; no source parsing or index refresh |
| Current/previous window | Calls, errors, content responses, unique files, file reads, response/result volume and changes | Default: rolling 168 hours vs the preceding 168 hours; `n/a` when a baseline is absent |
| Tool and daily activity | Calls, errors, content responses, files, reads, response share, average/maximum processing time by tool; UTC daily trends | Recorded `read`/`search`/`grep` calls; the first and last calendar dates may be partial |
| Returned files | Path, latest recorded size, total/per-tool reads, result volume/share, active dates, last observation and repeat summary | One read per file per successful source-bearing response |

Select the `index` or `reads` subcommand; there is no `--section` option. Default sorting is stored JSON size for index records and read count for activity, with 20 file rows. Each subcommand's `--help` includes examples and its valid options.

| Option | Applies to | Behavior |
| --- | --- | --- |
| `--path DIR` | Both | Select the repository, its index configuration and usage database |
| `--limit N`, `-n N` | Both | File rows per page; default 20, `0` for all |
| `--offset N` | Both | Skip N file rows after filtering/sorting; follow the printed continuation |
| `--sort KEY`, `-s KEY` | Both | Select a sort key listed for the subcommand above |
| `--order asc\|desc` | Both | Default: ascending paths, descending other values |
| `--filter TEXT`, `-f TEXT` | Both | Case-sensitive literal substring of paths, not a glob |
| `--view summary\|files\|full` | Both | Totals/groups, totals/files, or all detailed tables; CLI default `full` |
| `--format table\|json` | Both | Human tables or compact JSON with shared column names |
| `--language NAME`, `-l NAME` | `index` | Filter by indexed language |
| `--days N`, `-d N` | `reads` | Rolling 1–30 days; default 7 |
| `--tool read\|search\|grep`, `-t NAME` | `reads` | Restrict to one tool |
| `--no-compare` | `reads` | Omit the preceding equal-window comparison; windows above 15 days explicitly exceed 30-day retention |

Totals cover all matching files, not just the displayed page. An activity path filter selects calls that returned a matching file: `Response` still measures the whole selected call, while `Results` includes only matching files. Errors without file observations do not match a path filter. Whole-index disk/segment metrics remain global when file filters are applied.

MCP clients can call `analyze` directly, using the same aggregation for the server's current workspace:

```json
{"name":"analyze","arguments":{"target":"reads","sort":"bytes","limit":10}}
```

`target` is `index|reads`. Both accept `limit`, `offset`, `sort`, `order`, `filter` and `view`; index also accepts `language`, and reads accepts `days`, `tool` and `compare`. MCP defaults to `view=files`, 10 file rows, sorting index by `stored` and reads by `bytes`. Request `view=full` for additional breakdowns. `limit` is 1–100; continue with `page.next_offset`.

Compact output uses a `summary` object and per-table `columns`/`rows` arrays, without decorative rules or alignment spaces. Keys identify byte/time units; numbers and `null` remain typed, and short interpretation notes appear once. Output stays within 8 KiB or a smaller `output.max_bytes`, trimming whole rows and marking `truncated`/`omitted_tables` while retaining totals and continuation. Sensitive strings are masked before JSON serialization. Analysis calls do not add their own usage observations.

`Reads` counts files with returned source rows or search excerpts. Path-only and declaration/relation-only responses and failed calls contribute to calls/response volume but not reads. `find`, `overview`, `analyze` and internal indexing reads are not recorded. Repeat counts mean responses after a file's first appearance; different ranges or revisions may be involved, so they are not evidence of wasted tokens.

`File size` is the latest disk size recorded within the window; unknown sizes are `?` and excluded from totals. `Results` measures UTF-8 bytes in each formatted source/excerpt result block before masking, including row/path prefixes and local notices. `Response` measures final masked content text or error messages, including declarations, relations and headers but excluding JSON framing. Timing measures server request processing, excluding SQLite recording and client/network time. Client truncation and actual model consumption are unobservable; these are not token counts or physical disk reads.

Calls and file observations are stored in `.codemap/analysis.sqlite3`, without queries, source or response contents. **Retention is fixed at 30 days and cannot be extended.** Expired calls and their file observations are deleted together at MCP startup, recording, analysis, and every minute while MCP runs. If MCP is stopped, expired rows are removed at the next startup or analysis. `secure_delete` and a deleted rollback journal avoid retaining deleted rows in database free pages or a persistent WAL. This applies to the managed database, not separate backups.

`codemap-search mcp --no-call-log` disables new recording while retaining cleanup of existing records. Recording/cleanup failures warn on stderr without failing MCP responses and retry on subsequent activity. The analysis command reports database access failures as errors. Gaps cannot be reconstructed, and week comparisons do not guarantee continuous collection. Earlier JSONL journals are not imported; the `--log` option is no longer used.

## Development validation

Run from `apps/codemap-search` in the source checkout. `./verify` builds the current release binary and runs small call, exclusion, and constant-context checks for all 25 development languages.

```sh
./verify
./verify --language rust --language typescript --profile structural
./verify test
./verify public --language python --repository django/django
./verify public --language rust --language go --jobs 2
./verify public --dry-run
```

`test` runs the existing `cargo check` and `cargo test`. `public` prepares, qualifies, and validates pinned public repositories; it may download repositories and require independent language parsers. `--jobs` controls concurrent repositories, including Rust/Go, and defaults to 2. Checks within each repository remain sequential. Pass `--binary` to check an existing executable without building. Each run saves its summary and logs under `--cache` or `CODEMAP_VALIDATION_CACHE`, defaulting to `~/.cache/codemap-public-validation`. Failed or unverified checks retain a nonzero exit status. See the [command guide](./docs/development-language-commands.ko.md) for options and result interpretation.

## Indexing, diagnostics and limits

The MCP server builds/loads its own index in `.codemap/index` by default. A healthy filesystem watcher batches edits for 500ms and refreshes affected paths. Git HEAD changes or large batches trigger a full walk. When watching is off or unavailable, `search`/`overview` use the `index.refresh.index_staleness_ms` fallback. `read`, `find` and `grep` inspect disk directly.

- Files larger than `index.max_file_bytes` (default 1 MiB) are skipped by indexing/codemap.
- `.txt`, lockfiles, source maps, minified and bundle files have separate file exclusions. `find`/`grep` can bypass these with `include_ignored`; direct `read`/`parse` remain available.
- Static analysis cannot confirm paths or call targets determined at runtime. Check approximate call relationships in the source.
- This is a single-client sequential stdio server; do not run simultaneous servers against the same index directory.

Diagnostics use stderr. The default log filter is `warn,codemap_search=info`:

```sh
RUST_LOG=debug codemap-search mcp
```

See the [benchmark](../../benchmark/README.md) and [Docker verification guide](./docker/README.md) for measurement and validation methods.

## License

MIT — see [LICENSE](./LICENSE).
