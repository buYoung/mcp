# codemap-search

[한국어](./README.ko.md) | English

A self-contained MCP stdio server and CLI for coding agents. Map a repository, search extracted symbols, documentation and literals with BM25, then confirm the source with embedded `read`, `find` and `grep`. Tree-sitter grammars, Tantivy and ripgrep libraries are compiled into one Rust binary; no system `rg`, language server, external runtime, account or API key is required.

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
| `overview` | Inspect repository, folder or file structure | `path`, `format` |
| `search` | Find implementations with ranked symbols and snippets | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context` |
| `find` | Find paths by glob; newest files first | `pattern`, `path`, `include_ignored` |
| `grep` | Search live files with a regex | `pattern`, `path`, `glob`, `type`, `output_mode`, `-i`, `-n`, `-A`, `-B`, `-C`, `multiline`, `head_limit`, `offset`, `include_ignored` |
| `read` | Read live source with line numbers | `file_path`, `offset`, `limit` |

Use `search` for behavior or unknown implementation locations; use `grep` for exact identifiers, comments and just-edited content. `grep.pattern` is a regex, so escape metacharacters for literal code. JSON escaping is a separate layer. `grep` defaults to numbered `content`; `files_with_matches` and `count` return paths or counts. `read` also accepts `path`/`file` and 1-based inclusive `start_line`/`end_line` aliases.

In monorepos, `overview` on a directory selects that exact scope for later `search`; a file selects its parent. Explicit `workspace_scope` overrides it; `all`/`전체` selects the whole repository. If the implementation scope is unknown, start with read-only repo-wide discovery and narrow from returned paths. Search never silently widens a chosen scope. Top matches have detailed snippets; the compact tail is bounded. Narrow the query or follow the supplied read ranges when output is partial.

MCP `read`/`grep` show live source alongside enclosing declarations and call relationships. Resolved callees include their definition file and line. Same-file constant references include definition locations and initializer previews; ambiguous names are omitted. Indexed context can lag recent edits.

Tools are read-only over their configured filesystem scope. The server itself writes its index and, when enabled, repo configuration. No MCP resources or prompts are registered.

## Configure exclusions and output

Settings are read per key from `<repo>/.codemap/config.toml`, then `$CODEMAP_HOME/config.toml` (default `~/.codemap/config.toml`), then built-in defaults. An active repo key overrides its global value; comment it out to inherit instead.

Automatic symbol/call context excludes test regions by default. Set `[exclude].should_include_test_code = true` to include them. `test_file_patterns` and the per-language `test_attributes`, `test_decorators`, and `test_calls` lists let you add custom rules or remove built-ins; each explicit list replaces its inherited value and `[]` disables it. Live `read`/`grep` source is preserved. See [test-code context](./docs/configuration.md#test-code-context) for defaults and examples.

On first MCP startup, a missing repo file is generated with **common exclusions plus recursive globs for detected project types**. Common names include `.git`, `.idea`, `.vscode`, `.vs`, `.codemap`, and other supported VCS internals. A JS/TS project adds `**/node_modules`, `**/dist`, `**/build`, framework outputs and caches; Python, Rust and other build systems add their corresponding globs. Each pattern appears once and applies throughout the workspace, regardless of where the project was detected.

```toml
[exclude]
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

MCP watches existing config directories and reloads after about 1000ms. Manual exclusion or language-support changes request a full index refresh; output limits and filesystem permissions apply to subsequent requests. Restart after changing `index_path`, `watch` or `watch_debounce_ms`, or if config watching was unavailable. See the [full configuration reference](./docs/configuration.md) for every key, common folders, project detection rules, validation, permissions and application timing.

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

Optional groups default to `false` under `[language_support]`:

| Key | Group |
|---|---|
| `is_document_support_enabled` | Markdown `.md`, `.mdx` |
| `is_shell_support_enabled` | `.sh`, `.bash`, `.zsh` |
| `is_infrastructure_support_enabled` | `.hcl`, `.tf`, `.tfvars`, `Dockerfile`, `.nix` |
| `is_interface_support_enabled` | `.proto`, `.graphql`, `.gql` |
| `is_build_support_enabled` | `Makefile`, `.mk`, `CMakeLists.txt`, `.cmake`, `BUILD`, `BUILD.bazel`, `.bzl` |

These switches control index-backed discovery and watcher refreshes. Live `find`, `grep`, `read` and direct CLI `parse` remain available when a group is disabled. See [extraction details](./docs/language-support-checklist.md#extraction-details) for per-language visibility, test/deprecation flags, static relationships and limitations.

## CLI

```text
codemap-search mcp
codemap-search parse <file>
codemap-search tokenize <ident>
codemap-search codemap [--path P] [--format F]
codemap-search search <query> [-l N]
codemap-search index [dir]
codemap-search benchmark --queries <json> [--dir D]
```

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

The MCP server builds/loads its own index in `.codemap/index` by default. A healthy filesystem watcher batches edits for 500ms and refreshes affected paths. Git HEAD changes or large batches trigger a full walk. When watching is off or unavailable, `search`/`overview` use the `index_staleness_ms` fallback. `read`, `find` and `grep` inspect disk directly.

- Files larger than `max_file_size` (default 1 MiB) are skipped by indexing/codemap.
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
