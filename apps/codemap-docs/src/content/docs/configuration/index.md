---
title: Configuration Reference
description: Complete reference for codemap-search config.toml, auth.toml credentials, client output budgets, exclusions, Jev stages, and event navigation.
---

Configuration is optional. Add only the keys you want to change; omitted keys inherit global settings or built-in defaults. Behavior settings live in `config.toml` and decision-model API keys live in `auth.toml`.

## Files and precedence

Settings merge **per key** as **repo > global > built-in default**. Any active value in a higher layer (including `[]`) overrides lower layers, so remove or comment out a key to inherit it.

| Layer | Path |
| --- | --- |
| Repo | `<repo>/.codemap/config.toml` |
| Global | `$CODEMAP_HOME/config.toml`, else `~/.codemap/config.toml` |

- **Response byte caps**: Caps resolve as **repo tool override > repo common (`[output].max_bytes`) > global tool override > global common > tool default** (`1 MiB` for `search`, `5 MiB` for `read` and callable-expanded `grep`), covering MCP response text only. Over-limit `search` calls return partial results with a truncation footer, `read` requests a narrower range, and `find`, `initial_instructions`, and unexpanded `grep` return a narrowing error.
- **`.codemap/` and `.codemapignore`**: `.codemap/config.toml` and `.codemap/index/` share `.codemap/` (always excluded by `codemap-search`; add `.codemap/` to `.gitignore` or `.git/info/exclude` yourself). A repo `.codemapignore` uses gitignore syntax to hide extra paths from indexing, `find`, and `grep`.

### Global instructions

`[global_instructions].enabled` defaults to `false` and is read **only from global config** (repo values warn and are ignored):

```toml
[global_instructions]
enabled = true
```

When enabled, MCP initialization writes `- Prefer codemap-search for code navigation when available. Call \`initial_instructions\` first and follow the returned guidance. <!-- codemap-search:managed -->` to the detected client's global instructions file. Setting `enabled = false` removes tracked lines without touching user text; start a new agent session to apply changes:

| Client name | Default file | Env override and precedence |
| --- | --- | --- |
| `codex-mcp-client` | `~/.codex/AGENTS.md` | `CODEX_HOME`; non-empty `AGENTS.override.md` takes precedence |
| `claude-code` | `~/.claude/CLAUDE.md` | `CLAUDE_CONFIG_DIR` |
| `pi` | `~/.pi/agent/AGENTS.md` | `PI_CODING_AGENT_DIR`; first existing of `AGENTS.override.md`, `AGENTS.md`, `AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD` |
| `opencode` | `$XDG_CONFIG_HOME/opencode/AGENTS.md` (usually `~/.config/opencode/AGENTS.md`) | `OPENCODE_CONFIG_DIR`; v1 falls back to `~/.claude/CLAUDE.md` if `AGENTS.md` is absent and Claude prompts are enabled; v2 uses `AGENTS.md` |

### Credentials (`auth.toml`)

Store decision-model API keys in `<repo>/.codemap/auth.toml` or `$CODEMAP_HOME/auth.toml` (`~/.codemap/auth.toml` by default), never in `config.toml`:

```toml
# .codemap/auth.toml — keep out of version control
[jev]
api_key = "<your TypeSafe key>"
[openrouter]
api_key = "<your OpenRouter key>"
```

- **Resolution order**: Each key resolves independently as **repo auth > global auth > environment variable** (`TYPESAFE_API_KEY` or `OPENROUTER_API_KEY`), using only the provider selected by `[output.jev].model`. OpenRouter keys go exclusively to OpenRouter's Decisions API (`https://openrouter.ai/api/alpha/decisions`).
- **Auto-creation and exclusion**: With `[update].config_auto_update = true`, startup creates an empty repo `auth.toml` (`0600` on Unix) if missing. Any `auth.toml` is excluded case-insensitively from indexing, `search`, and default `find`/`grep` (keep it out of Git yourself).

### Client delivery budgets

`[output.client]` caps final MCP response size in UTF-8 bytes after masking, Jev filtering, and duplicate folding. All four limits default to `100000` (`claude_max_result_chars` counts characters as UTF-8 bytes, `codex_output_token_limit` estimates `floor(tokens × 3.5)`, and `pi_max_bytes` and `opencode_max_bytes` use bytes directly). Over-limit `search` responses swap lower-priority bodies for `read` ranges, batch `read` (`ranges`) defers whole items with a notice, and single-range `read` or other tools return a narrowing error.

- **Claude Code (`[output.client].claude_max_result_chars`, `1–500000`)**: Sent via tool metadata; reconnect MCP after changing ([Claude Code docs](https://code.claude.com/docs/en/mcp#raise-the-limit-for-a-specific-tool)).
- **Codex (`[output.client].codex_output_token_limit`)**: Run `codemap-search codex-config` (`--server-name NAME` if needed) and merge the output into `~/.codex/config.toml`. Code Mode clients also receive instructions to put `// @exec: {"max_output_tokens": N}` on line 1 and match it on `wait`; Codex's own `tool_output_token_limit` still applies ([Codex MCP docs](https://learn.chatgpt.com/docs/extend/mcp#other-configuration-options)).
- **`pi` (`[output.client].pi_max_bytes`)**: Honored by [@buyong/pi-mcp-adapter](https://www.npmjs.com/package/@buyong/pi-mcp-adapter) after MCP reconnect; `pi`'s built-in MCP client ignores metadata and clips at 20 KB.
- **`opencode` (`[output.client].opencode_max_bytes`)**: OpenCode ignores server metadata and defaults [`tool_output.max_bytes`](https://opencode.ai/v2/docs/config) to `51200`; update `opencode.json` to match `opencode_max_bytes`.

## Loading and automatic writes

Malformed TOML drops that file layer with a `stderr` warning; invalid keys warn and fall back per key (`repo > global > default`). With `[update].config_auto_update = true` (default), `mcp` startup creates missing repo `config.toml`/`auth.toml`, populates detected project excludes on first creation, upgrades repo files to `# codemap-config-version: 32`, and adds new keys as comments without overwriting explicit values, `[]` lists, or user-edited `excluded_directories`.

Global files are never auto-created or rewritten. Setting `[update].config_auto_update = false` stops template creation and automatic writes while keeping loading and hot reload active. When upgrading an older config manually with `config_auto_update = false`, include any fallback exclusions you want (`node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor`) in `[index.exclude].excluded_directories` and update `# codemap-config-version: 32`.

## Directory exclusions

`[index.exclude].excluded_directories` defines the complete **optional** directory exclusion list. An explicit array replaces defaults instead of merging with them, `[]` disables optional exclusions, and omitting the key inherits global config or the built-in fallback list (`.idea`, `.vscode`, `.vs`, `node_modules`, `.yarn`, `target`, `dist`, `build`, `vendor`).

```toml
# Editable defaults shown; VCS and .codemap directories remain excluded regardless
[index.exclude]
excluded_directories = [".idea", ".vscode", ".vs", "node_modules", ".yarn", "target", "dist", "build", "vendor"]
```

- **Mandatory exclusions**: VCS folders (`.git`, `.svn`, `.hg`, `.bzr`, `.jj`, `.sl`), `.codemap`, `.codemap-index`, and `[index].path` are always excluded even if removed from this list or when `include_ignored: true` is passed.
- **Initial generation**: When creating a missing repo `config.toml`, `codemap-search` inspects project marker files to add matching recursive globs such as `**/node_modules`, `**/target`, or `**/.venv`. Replace broad `**` globs if you have source directories with those names, and add custom build paths manually.

| Rule | Meaning |
| --- | --- |
| `build` | Directory basename at any depth (also applies inside `[filesystem_permissions].allowed_roots`) |
| `**/build` | Matching directories at any depth within the workspace, including root |
| `./build` | Workspace-root directory only |
| `apps/web/build` | That directory and its descendants |
| `apps/api/**/__pycache__` | Cache directories at any depth under `apps/api` |
| `apps/native/cmake-build-*` | Matching CMake output directories under `apps/native` |

Rules are case-sensitive relative directory globs (Windows separators normalize to `/`; `..`, empty strings, and absolute paths reject the entire array). They apply across indexing, CLI `codemap`, caller scans, watchers, `find`, and `grep`; direct `read` skips directory exclusions while applying `[output.context.exclude]`.

- **Nested worktrees and submodules**: Indexing and watchers skip nested git linked worktrees but index submodules; `find`, `grep`, and CLI `codemap` walk both.

## When changes take effect

MCP watches existing config directories and reloads `config.toml` and `auth.toml` after ~1000ms (CLI reads config per command).

- **Full index refresh**: `[index.exclude]`, `[output.context.exclude]`, `[index.language_support].*`, `[output.macro_expansion].*`.
- **Next request or refresh cycle**: `[output].*`, `[output.redact].*`, `[filesystem_permissions].*`, `[analysis].target_os`, `[index].max_file_bytes`, `[index.refresh].index_staleness_ms`, `[index.refresh].indexer_auto_restart`, `auth.toml`, and `[index].store_references` (new parses only).
- **Restart, reconnect, or new session**: `[index].path`, `[index].mode`, `[index].auto_max_files`, `[index.refresh].watch`, `[index.refresh].watch_debounce_ms`, and newly created config directories require server restart; `[global_instructions].enabled` takes effect in a new agent session; `[output.client].*` requires MCP reconnect, `codex-config`, or `opencode.json` sync; `[update].config_auto_update` applies at next MCP startup.

## Key reference

Numeric keys require integers `>= 1` (`[output.grep].max_columns` also accepts `0`). Byte-size keys accept integer bytes or quoted strings with `b`, `kb`, `mb`, or `gb` (powers of 1024, e.g., `"50mb"` = `52428800`; bare `50mb` is invalid TOML).

| Key | Type | Default | Summary |
| --- | --- | --- | --- |
| `[global_instructions].enabled` | bool | `false` | Global-only managed client instruction line; `false` removes tracked lines |
| `[output].is_redact_enabled` | bool | `true` | Mask detected credentials and selected PII in MCP output; indexes and matching keep originals |
| `[output].max_bytes` | integer bytes or size string | unset | Common MCP response ceiling; same-layer tool override wins |
| `[output.client].claude_max_result_chars` | integer characters, 1–500000 | `100000` | Claude `tools/list` metadata and delivery limit (as bytes); reconnect required |
| `[output.client].codex_output_token_limit` | positive integer tokens | `100000` | Codex config export and delivery limit (3.5 bytes per token) |
| `[output.client].pi_max_bytes` | integer bytes or size string | `100000` | `pi` `tools/list` metadata and delivery limit; reconnect required |
| `[output.client].opencode_max_bytes` | integer bytes or size string | `100000` | `opencode` final delivery limit |
| `[output.search].detail_file_limit` | integer | `24` | Top-ranked files rendered in detail before the compact tail |
| `[output.search].overview_file_limit` | integer | `80` | Max file headers in `search`'s compact ranked tail |
| `[output.search].snippet_max_lines` | integer | `500` | Per-symbol snippet line cap in `search` detail view |
| `[output.search].symbol_limit` | integer | `100` | Max symbols rendered per file in `search` detail view (unmatched keys in path/docstring-matched JSON/YAML/TOML are counted only) |
| `[output.search].max_bytes` | integer bytes or size string | `"1mb"` (`1048576`) | Total size cap for one `search` response, including the partial-output notice |
| `[output.search].literal_max_chars` | integer (chars) | `1200` | Max length for matched literals before ellipsis truncation |
| `[output.search].literal_limit` | integer | `60` | Max matched literals rendered per file in `search` detail view |
| `[output.search].anchor_snippet_limit` | integer | `20` | Max full snippets per file; remaining symbols show up to 3 signature lines |
| `[output.read].max_bytes` | integer bytes or size string | `"5mb"` (`5242880`) | Ceiling for `read` and callable-expanded `grep`; oversized `read` errors, expanded `grep` paginates |
| `[output.read].is_outside_callers_enabled` | bool | `true` | Append outside callers, same-key uses, and importing files to `read view=source`; `false` omits all three |
| `[output.grep].max_columns` | integer | `0` | Content-mode column cap (`0` disables); wider rows become `[Omitted long matching line]` |
| `[output.grep].max_bytes` | integer bytes or size string | `[output].max_bytes` | `grep` response ceiling; callable-expanded mode falls back to `[output.read].max_bytes` |
| `[output.context].is_enabled` | bool | `true` | Default `search` caller/callee annotation setting when `caller_context` is omitted |
| `[output.context].caller_limit` | integer | `1000` | Max callers (or non-call references) shown per symbol |
| `[output.context].callee_limit` | integer | `1000` | Max callees shown per symbol |
| `[output.context].max_bytes` | integer bytes or size string | `"128kb"` (`131072`) | Relationship annotation sub-budget inside `[output.search].max_bytes` |
| `[output.context].common_name_threshold` | integer | `2` | Definition count at which name-based relationships carry an ambiguity label |
| `[output.context].caller_omit_def_threshold` | integer | `5` | Definition count at which approximate callers switch to a `grep` suggestion (callees unaffected) |
| `[output.navigation].is_enabled` | bool | `false` | Inspect source structure and mark confirmed call targets `precise` |
| `[output.navigation].callsite_budget` | integer | `1000` | Max call sites checked structurally before falling back to name-based scanning |
| `[output.navigation].scan_limit` | integer | `16000` | Total caller-scan hit cap shared across names (min 25/name) |
| `[output.redact].pii_entities` | string array | `[]` | Case-sensitive PII entity names to mask; see [supported types](/configuration/pii-redaction/) |
| `[output.redact].sensitive_fields` | string array | `[]` | Extra sensitive field names, matched exactly after case/separator normalization |
| `[output.redact].rules` | inline-table array | `[]` | Custom regex rules, each with `id` (`custom.*`) and `pattern` |
| `[output.redact].exceptions` | inline-table array | `[]` | Exact `{ rule_id, value }` pairs exempt from masking |
| `[index].mode` | `"auto"`, `"on"`, or `"off"` | `"auto"` | When MCP indexes the workspace; see [When indexing starts](#when-indexing-starts) |
| `[index].auto_max_files` | integer | `50000` | Max indexable files permitted under `auto` mode |
| `[index].path` | string | `".codemap/index"` | Index directory (absolute or workspace-relative) |
| `[index].max_file_bytes` | integer bytes or size string | `"1mb"` (`1048576`) | Files larger than this are skipped before parse/index |
| `[index].store_references` | bool | `false` | Store non-call reference locations in the index |
| `[index.refresh].watch` | bool | `true` | Enable background filesystem watcher |
| `[index.refresh].watch_debounce_ms` | integer (ms) | `500` | Watcher event batching window |
| `[index.refresh].index_staleness_ms` | integer (ms) | `5000` | Debounce for request-triggered fallback refreshes |
| `[index.refresh].indexer_auto_restart` | bool | `true` | Auto-recover if the background indexer thread stops |
| `[index.language_support].is_document_support_enabled` | bool | `false` | Index `.md` and `.mdx` files |
| `[index.language_support].is_shell_support_enabled` | bool | `false` | Index `.sh`, `.bash`, and `.zsh` scripts |
| `[index.language_support].is_infrastructure_support_enabled` | bool | `false` | Index HCL/Terraform, Dockerfile, and Nix files |
| `[index.language_support].is_interface_support_enabled` | bool | `false` | Index Protocol Buffers and GraphQL definitions |
| `[index.language_support].is_build_support_enabled` | bool | `false` | Index Make, CMake, and Starlark/Bazel build files |
| `[index.exclude].excluded_directories` | string array (relative directory globs) | Fallback list; see [Directory exclusions](#directory-exclusions) | Complete optional directory exclusion list |
| `[index.exclude].use_git_exclude` | bool | `true` | Honor `.git/info/exclude` (independent of `.gitignore`) |
| `[output.context.exclude].should_include_test_code` | bool | `false` | Include test code in automatic symbol/call context |
| `[output.context.exclude].test_file_patterns` | string array | See [Test-code context](#test-code-context) | Test file globs (`[]` disables path detection) |
| `[output.context.exclude].test_attributes` | language → string array | See [Test-code context](#test-code-context) | Per-language attribute/annotation patterns (replaces lower layer per language) |
| `[output.context.exclude].test_decorators` | language → string array | See [Test-code context](#test-code-context) | Per-language decorator patterns (`[]` disables that language) |
| `[output.context.exclude].test_calls` | language → string array | See [Test-code context](#test-code-context) | Per-language test-call patterns (`[]` disables that language) |
| `[output.jev].enabled` | bool | `false` | Master switch for Jev filtering (`false` keeps all tools local) |
| `[output.jev].scope` | string array | `["search", "read", "grep"]` | Complete list of tools filtered when enabled (`[]` selects none) |
| `[output.jev].model` | string | `"jev-1.14.0"` | Pinned provider model (bare version uses TypeSafe; `author/slug` uses [OpenRouter](#openrouter-decision-models); aliases fail) |
| `[output.jev].timeout_ms` | positive integer (ms), max 7 days | `45000` | Absolute per-call deadline including preparation and queue wait |
| `[output.jev].max_in_flight_requests` | integer, 1–8 | `8` | Max concurrent HTTP requests per evaluation |
| `[output.jev].request_spacing_ms` | integer (ms), >= 100 | `100` | Minimum interval between request starts |
| `[output.jev].max_batch_bytes` | integer bytes or size string, 1–168000 | `168000` | Encoded payload ceiling per batch (oversized single questions fail) |
| `[output.jev].pool_idle_timeout_ms` | positive integer (ms), max 7 days | `30000` | Idle HTTPS connection lifetime |
| `[output.jev].search_filter_min_unrelated_probability` | finite number, `0.5 < value <= 1.0` | `0.70` (provisional) | Minimum "unrelated" ("no") probability to omit a body (symmetric threshold for "related"; scores between stay visible) |
| `[filesystem_permissions].find` | string | `"anywhere"` | Path access policy for `find`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].grep` | string | `"anywhere"` | Path access policy for `grep`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].read` | string | `"anywhere"` | Path access policy for `read`: `workspace`, `allowed_roots`, or `anywhere` |
| `[filesystem_permissions].allowed_roots` | string array | `[]` | External root paths permitted when a tool policy is `allowed_roots` |
| `[related_terms].enabled` | bool | `false` | Opt in to the `synonyms` tool (no download or setup while `false`) |
| `[related_terms].dataset_path` | string | unset | Optional user-owned sense dataset path used instead of downloading |
| `[update].config_auto_update` | bool | `true` | Scaffold missing repo `config.toml`/`auth.toml` and sync comments on `mcp` startup |

### When indexing starts

- **`"auto"` (default)**: Indexes on the first `search`, `read`, `grep`, `synonyms`, or monorepo `initial_instructions` call inside a repository (`.git`, `.hg`, or `.jj`) with at most `[index].auto_max_files` indexable files. Multi-repo folders and oversized repos return `Index unavailable:` on `search` while `read`, `grep`, and `find` work live; set `mode = "on"` and restart to index oversized repos.
- **`"on"` / `"off"`**: `"on"` indexes at startup; `"off"` disables indexing. A new git worktree without an index copies the most recent compatible index from another worktree of that repo before re-extracting changed files.
- **Workspace root limit**: `~` itself cannot be a workspace.
- **Skipped and ignored files**: `auth.toml`, `.txt`, lockfiles, `*.map`, minified/bundle files ([supported languages](/usage/languages/)), non-UTF-8 files, files over `[index].max_file_bytes`, and paths matched by `.gitignore`, global gitignore, or `.codemapignore` are skipped by indexing (`find`/`grep` show ignored files with `include_ignored: true`, and `[index.exclude].use_git_exclude = false` disables `.git/info/exclude` only). `[index.language_support]` switches affect indexing and CLI `codemap` only.
- **Background refresh**: Edits refresh in the background when `[index.refresh].watch = true`, or every `[index.refresh].index_staleness_ms` on `search` requests when `false`.

### Native macro expansion

Enabled by default (`[output.macro_expansion].is_enabled = true`) for C, C++, and assembly via installed `clang` or `nasm`. When `compilation_database` is omitted, parent directories are searched up to the workspace root for `compile_commands.json` or `compile_flags.txt`; missing toolchains or failed expansions keep original declarations (`read` and `grep` always show original source):

```toml
[output.macro_expansion]
is_enabled = true
compilation_database = "build/compile_commands.json"
clang_path = "clang"
nasm_path = "nasm"
clang_flags = ["-Iinclude", "-DFEATURE=1"]
nasm_flags = ["-Iinclude/", "-felf64"]
timeout_ms = 5000
max_output_bytes = "8mb"
```

### Credential redaction

`[output].is_redact_enabled = true` replaces detected secrets and opted-in PII in MCP responses with `[REDACTED]` (or asterisks for short tokens) without touching disk files, local indexes, search ranking, or CLI output. Built-in rules cover `field.sensitive` (`API_KEY`, `access_token`, `client_secret`, `password`), `token.*` (`aws-access-key`, `github`, `openai`, `google`, `slack`, `jwt`, `stripe`, `gitlab`, `npm`, `sendgrid`), `credential.*` (`authorization`, `bearer`, `url-password`), and `private-key.pem`. PII is off by default; see [PII redaction](/configuration/pii-redaction/) for all 90 types.

```toml
[output.redact]
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD"]
sensitive_fields = ["internalCredential"] # normalized exact match (includes internal_credential)
rules = [{ id = "custom.acme", pattern = 'ACME_(?P<secret>[A-Z0-9]+)' }]
exceptions = [{ rule_id = "custom.acme", value = "ACME_EXAMPLE" }]
```

- Custom rule IDs must start with `custom.` and use Rust `regex` syntax (`(?P<secret>...)` masks only that capture group).
- `exceptions` require an exact match on `rule_id` and the unquoted detected value; each list replaces its inherited layer (`[]` clears it).
- Zero-match `search` responses omit the query string to avoid echoing unverified secrets.

### Output and call relationships

- When a `search` response exceeds 32 KiB, each top-ranked file condenses to match reasons, `read` ranges, matched declarations, up to two matching source lines, and connected locations. Generated and translation files rank lower unless targeted directly.
- Whole-file `read` without `limit` or `expand="callable"` keeps a 256 KiB ceiling; live `read`/`grep` context shares a 16 KiB ceiling.
- Call annotations default to approximate matching; `[output.navigation].is_enabled = true` marks confirmed single targets `precise` (`name — file:line`). In `read` and `grep`, `references (same-file constants, approximate)` previews (up to 240 chars) appear even when `[index].store_references = false`.
- `[output.context].common_name_threshold` adds ambiguity warnings to approximate lists; `[output.context].caller_omit_def_threshold` switches approximate callers to a `grep` hint while keeping callees and `precise` callers.

### Test-code context

When `[output.context.exclude].should_include_test_code = false` (default), test files and test regions are excluded from automatic context, caller annotations, definition counts, and event analysis while remaining directly searchable and readable. Rules combine with OR into nested helpers. For `test_file_patterns` and each language table below, an explicit array **replaces** the inherited list (`[]` disables that language/category; omit or use `{}` to inherit). Built-in defaults:

```toml
[output.context.exclude]
should_include_test_code = false
test_file_patterns = ["**/tests/**", "**/test/**", "**/__tests__/**", "test_*.py", "*_test.*", "*.test.*", "*_spec.*", "*.spec.*", "*Test.java", "*Tests.java", "*IT.java"]
[output.context.exclude.test_attributes] # omit #[...] and @
rust = ["test", "tokio::test", "async_std::test", "rstest", "rstest::rstest", "cfg(test)"]
java = ["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Nested", "BeforeEach", "AfterEach", "BeforeAll", "AfterAll"]
kotlin = ["Test", "ParameterizedTest", "RepeatedTest", "BeforeTest", "AfterTest", "BeforeEach", "AfterEach"]
csharp = ["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestFixture", "SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown"]
swift = ["Test", "Suite"]
php = ["Test"]
[output.context.exclude.test_decorators] # omit @
python = ["pytest.fixture", "pytest.mark.*", "unittest.skip", "unittest.skipIf", "unittest.skipUnless", "unittest.expectedFailure"]
[output.context.exclude.test_calls]
javascript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
typescript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
dart = ["test", "group", "testWidgets"]
ruby = ["describe", "context", "it", "specify"]
powershell = ["Describe", "Context", "It"]
```

### Filesystem permissions

`find`, `grep`, and `read` each accept `"workspace"`, `"allowed_roots"` (workspace plus non-empty `[filesystem_permissions].allowed_roots`, relative to workspace root with symlinks resolved), or `"anywhere"` (default). `allowed_roots` does not merge external trees into the main index; `search` with `workspace_scope` indexes an external repo only when `read` allows that full repo directory.

## Example `config.toml`

Practical `<repo>/.codemap/config.toml` overrides:

```toml
# codemap-config-version: 32
[output.search]
max_bytes = "1mb"

[output.redact]
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD"]
sensitive_fields = ["internalCredential"]

[output.jev]
enabled = false
scope = ["search", "read", "grep"]
model = "jev-1.14.0"

[index]
mode = "auto" # "auto", "on", or "off" (restart required)

[index.exclude]
excluded_directories = [".git", ".codemap", ".codemap-index", "**/node_modules", "**/target"]
```

Common `[filesystem_permissions]` presets (`find`, `grep`, `read` default to `"anywhere"`):

```toml
# 1. Workspace only
[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"

# 2. Bounded external roots for find/grep, workspace only for read
[filesystem_permissions]
find = "allowed_roots"
grep = "allowed_roots"
read = "workspace"
allowed_roots = ["/shared/source", "../vendor-src"]

# 3. Full disk for find only
[filesystem_permissions]
find = "anywhere"
grep = "workspace"
read = "workspace"
```

## Live request controls

To read multiple windows in one call, pass `ranges: [{file_path, offset?, limit?}, ...]` (1–20 items) to `read` without top-level `file_path`. Top-level `view`, `expand`, `unresolved`, `include_events`, `debug`, and `include_seen` apply across the batch (`offset` defaults to `1`, `limit` to EOF); item errors appear inline, and hitting a byte cap defers remaining items with a notice.

| Argument | Default | Result |
| --- | --- | --- |
| `view` | `"full"` | `full`: symbols + source; `source`: live text only; `source_grouped`: content-mode `grep` only ([grouped by file](#search-display-compaction-and-grouped-grep-source)); `definitions`: declarations only; `relations`: identities, calls, and constant references without source |
| `unresolved` | `"list"` | `count` prints the unresolved total only; `list` includes bounded names (`full`/`relations`) |
| `expand` | Content `grep`: `"callable"`; `read`: `"none"` | `callable` expands to the enclosing named callable (within `min(max_file_bytes, 4 MiB)`); `none` keeps exact lines or windows |

Content-mode `grep` defaults to `expand="callable"` across all views and paginates by unique callable group (`head_limit=0` lifts the group count cap only). Pass `expand="none"` for matching lines, line pagination, and `-A/-B/-C` context, or to narrow oversized callables.

## Explicit Rust analysis target

```toml
[analysis]
target_os = "macos"
```

`target_os` is never inferred from the host OS (`""` clears an inherited global value). Rust resolution evaluates `#[cfg(...)]` expressions with `target_os`, `all(...)`, `any(...)`, and `not(...)` (`cfg(test)` delegates to `[output.context.exclude]`), applying on the next request without re-indexing.

## Optional Jev decision stages

Jev uses a decision model (TypeSafe Jev or OpenRouter) to drop irrelevant function bodies from `search`, `read`, and `grep` responses before delivery:

```toml
[output.jev]
enabled = true
scope = ["search", "read", "grep"]
model = "jev-1.14.0"
search_filter_min_unrelated_probability = 0.70
```

Configure API keys in [`auth.toml`](#credentials-authtoml) or via `TYPESAFE_API_KEY` / `OPENROUTER_API_KEY`.

- **Scope and `task`**: `[output.jev].enabled = true` filters only tools in `[output.jev].scope` (`search`, `read`, `grep`; `[]` filters none), keeping `initial_instructions`, `find`, `analyze`, and CLI local. When `scope` is non-empty, pass the user's request verbatim as `task` (max 65,536 bytes) in `initial_instructions` and update it whenever the task changes (missing `task` errors).
- **Visibility, privacy, and fallback**: Masked `task` and code snippets go over HTTPS. Uncertain, partial, or oversized code stays visible; omitted blocks show `_omitted body:`, `_omitted matches:`, or `_omitted lines:` with exact `read` ranges (reading that exact range or strictly inside one function bypasses re-filtering). Omission notes never count as delivered source in [reading-activity metrics](/usage/cli-and-analysis/#interpreting-counts-and-bytes), and missing keys or provider errors fall back silently to unfiltered output.

### OpenRouter decision models

Setting `[output.jev].model` to an `author/slug` ID routes requests exclusively to `POST https://openrouter.ai/api/alpha/decisions` using `[openrouter].api_key` (`OPENROUTER_API_KEY` fallback), such as `model = "openai/gpt-6-luna-decisions"`.

Only pinned decision IDs work (`~author/slug` aliases and non-decision models fail with HTTP 400 and fall back). Requests above ~32 KB use slower long-context mode on OpenRouter; lower `[output.jev].max_batch_bytes` if needed.

Verified `>= 64k`-context OpenRouter decision models (`2026-10-24`; live list via `GET https://openrouter.ai/api/v1/models?output_modalities=decisions`):

| Maker | Model IDs |
| --- | --- |
| OpenAI | `openai/gpt-6-luna-decisions` |
| TypeSafe | `typesafe/jev-1.14`, `typesafe/jev-1.13` |
| Perplexity | `perplexity/pplx-decider-v1.2`, `perplexity/pplx-decider-flash-v1.2` |
| Nace.AI | `nace-ai/drex-max`, `nace-ai/drex-v2.0`, `nace-ai/drex-v1.5` |
| Respan | `respan/span-02` |
| Upstage | `upstage/solar-decide`, `upstage/solar-decide-flash` |
| Cloudflare | `cloudflare/clef`, `cloudflare/clef-omni` |
| Liquid AI | `liquid/d1` |
| Inception | `inception/mercury-decide` |

## Indexed event navigation

Event navigation is enabled by default (`[output.event_navigation].is_enabled = true`, `use_builtin_rules = true`), surfacing `Source routes` across 18 languages alongside Node `EventEmitter` and Tauri event maps. Pass `include_events: false` to suppress it on a request, or `event_key: "saved"` on `search` to query an exact event key.

Custom rules override matching built-in selectors (up to 64 rules; argument indexes `< 16`, identifiers/keys `<= 256` bytes):

```toml
[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = [
  { id = "known-on", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "on", role = "subscribe", event_arg = 0, handler_arg = 1, bus = "receiver" },
  { id = "known-emit", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "emit", role = "publish", event_arg = 0, bus = "receiver" },
]
```

- **Selector and role**: Exact `language` (`typescript`, `javascript`, or `rust`), `module`, `symbol`, optional `method`, and `role` (`publish`, `subscribe`, or `unsubscribe`).
- **Event and handler**: One of `event_arg` (0-based index) or `event_key` (fixed string), plus `handler_arg` for `subscribe`.
- **Bus and qualifiers**: `bus` accepts `"receiver"` (requires `method`), `"argument"` (`bus_arg`), `"fixed"` (`bus_identity`, e.g., Rust-to-JS bridge), or `"framework"` (Tauri frontend only); optional `target_arg`/`target`, `channel` (`"default"`), and `is_once`.

## Search display compaction and grouped grep source

`search` keeps confirmed relationship locations and source bodies intact while compacting non-call references (file/line only), merging repeated caveats into `[N1]` notes, and omitting literal previews already visible inside a full body.

`grep` with `view="source_grouped"` groups matching lines under one heading per file (content mode only; supports `expand="none"` with `42:match`/`43-context` markers and `expand="callable"`).

## Legacy configuration names

Older keys remain readable; canonical names win within the same file, and auto-migration rewrites repo configs only.

| Legacy path | Canonical path |
| --- | --- |
| `tool_output.is_redact_enabled` | `output.is_redact_enabled` |
| `search.result_threshold` | `output.search.detail_file_limit` |
| `search.search_overview_file_limit` | `output.search.overview_file_limit` |
| `search.search_detail_snippet_max_lines` | `output.search.snippet_max_lines` |
| `search.search_detail_symbol_limit` | `output.search.symbol_limit` |
| `search.search_detail_byte_cap` | `output.search.max_bytes` |
| `search.search_literal_max_len` | `output.search.literal_max_chars` |
| `search.search_literal_limit` | `output.search.literal_limit` |
| `search.search_anchor_snippet_limit` | `output.search.anchor_snippet_limit` |
| `tool_output.read_output_byte_cap` | `output.read.max_bytes` |
| `tool_output.grep_max_columns` | `output.grep.max_columns` |
| `caller_context.caller_context_default` | `output.context.is_enabled` |
| `caller_context.caller_list_cap` | `output.context.caller_limit` |
| `caller_context.callee_list_cap` | `output.context.callee_limit` |
| `caller_context.annotation_sub_budget` | `output.context.max_bytes` |
| `caller_context.common_name_threshold` | `output.context.common_name_threshold` |
| `caller_context.caller_omit_def_threshold` | `output.context.caller_omit_def_threshold` |
| `index.index_path` | `index.path` |
| `index.max_file_size` | `index.max_file_bytes` |
| `caller_context.navigation_store_references` | `index.store_references` |
| `refresh.watch` | `index.refresh.watch` |
| `refresh.watch_debounce_ms` | `index.refresh.watch_debounce_ms` |
| `refresh.index_staleness_ms` | `index.refresh.index_staleness_ms` |
| `refresh.indexer_auto_restart` | `index.refresh.indexer_auto_restart` |
| `language_support.is_document_support_enabled` | `index.language_support.is_document_support_enabled` |
| `language_support.is_shell_support_enabled` | `index.language_support.is_shell_support_enabled` |
| `language_support.is_infrastructure_support_enabled` | `index.language_support.is_infrastructure_support_enabled` |
| `language_support.is_interface_support_enabled` | `index.language_support.is_interface_support_enabled` |
| `language_support.is_build_support_enabled` | `index.language_support.is_build_support_enabled` |
| `caller_context.navigation_context_default` | `output.navigation.is_enabled` |
| `caller_context.navigation_callsite_budget` | `output.navigation.callsite_budget` |
| `caller_context.scan_cap` | `output.navigation.scan_limit` |
| `redact.*` | `output.redact.*` |
| `macro_expansion.*` | `output.macro_expansion.*` |
| `event_navigation.*` | `output.event_navigation.*` |
| `analysis.navigation.*` (v18) | `output.navigation.*` |
| `analysis.macro_expansion.*` (v18) | `output.macro_expansion.*` |
| `analysis.event_navigation.*` (v18) | `output.event_navigation.*` |
