---
title: Configuration Reference
description: Reference for codemap-search config.toml keys, auth.toml credentials, exclusions, Jev filter, and legacy key names.
---

## Files and precedence

Specify only the keys you want to override; settings merge per key as `repo > global > built-in default`.

| Scope | `config.toml` | `auth.toml` (API keys) |
| --- | --- | --- |
| Repo | `<repo>/.codemap/config.toml` | `<repo>/.codemap/auth.toml` |
| Global | `$CODEMAP_HOME/config.toml` (default `~/.codemap/config.toml`) | `$CODEMAP_HOME/auth.toml` (default `~/.codemap/auth.toml`) |

Add `.codemapignore` at the repository root (gitignore syntax) to exclude extra paths from indexing, `find`, and `grep`.

## Example `config.toml`

```toml
[output.search]
max_bytes = "1mb"

[output.redact]
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD"]
sensitive_fields = ["internalCredential"]

[index]
mode = "auto" # "auto", "on", or "off" (restart required)

[index.exclude]
excluded_directories = [".idea", ".vscode", "node_modules", "target", "dist", "build", "vendor"]

[filesystem_permissions]
find = "workspace"
grep = "workspace"
read = "workspace"
```

## Key reference

Byte-size keys accept integer bytes or quoted strings with `b`, `kb`, `mb`, or `gb` (powers of 1024, e.g., `"50mb"` = `52428800`).

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `[global_instructions].enabled` | bool | `false` | Manage a one-line navigation reminder in the client's global instructions file (global config only) |
| `[output].is_redact_enabled` | bool | `true` | Mask credentials and opted-in PII in MCP output |
| `[output].max_bytes` | integer bytes or size string | unset | Fallback MCP response ceiling when a tool-specific `max_bytes` is unset |
| `[output.client].claude_max_result_chars` | integer (chars), 1–500000 | `100000` | Claude Code delivery limit (reconnect MCP after changing) |
| `[output.client].codex_output_token_limit` | positive integer (tokens) | `100000` | Codex output token limit (merge `codemap-search codex-config` into `~/.codex/config.toml`) |
| `[output.client].pi_max_bytes` | integer bytes or size string | `100000` | `pi` (`@buyong/pi-mcp-adapter`) delivery limit (reconnect MCP after changing) |
| `[output.client].opencode_max_bytes` | integer bytes or size string | `100000` | OpenCode delivery limit (match `tool_output.max_bytes` in `opencode.json`, which defaults to `51200`) |
| `[output.search].detail_file_limit` | integer | `24` | Top-ranked files rendered with detailed snippets |
| `[output.search].overview_file_limit` | integer | `80` | Max file headers listed in `search`'s compact tail |
| `[output.search].snippet_max_lines` | integer | `500` | Max snippet lines per symbol in `search` |
| `[output.search].symbol_limit` | integer | `100` | Max symbols rendered per file in `search` |
| `[output.search].max_bytes` | integer bytes or size string | `"1mb"` (`1048576`) | Max response size for one `search` call |
| `[output.search].literal_max_chars` | integer (chars) | `1200` | Max characters per matched string literal before truncation |
| `[output.search].literal_limit` | integer | `60` | Max matched literals shown per file in `search` |
| `[output.search].anchor_snippet_limit` | integer | `20` | Max full snippets per file (remaining symbols show signatures) |
| `[output.read].max_bytes` | integer bytes or size string | `"5mb"` (`5242880`) | Max size for `read` and callable-expanded `grep` |
| `[output.read].is_outside_callers_enabled` | bool | `true` | Append outside callers and importers to `read view="source"` |
| `[output.grep].max_columns` | integer | `0` | Max line width in content-mode `grep` (`0` = unlimited) |
| `[output.grep].max_bytes` | integer bytes or size string | `[output].max_bytes` | Max response size for `grep` |
| `[output.context].is_enabled` | bool | `true` | Default `caller_context` behavior in `search` |
| `[output.context].caller_limit` | integer | `1000` | Max callers or references shown per symbol |
| `[output.context].callee_limit` | integer | `1000` | Max callees shown per symbol |
| `[output.context].max_bytes` | integer bytes or size string | `"128kb"` (`131072`) | Sub-budget for relationship annotations inside `[output.search].max_bytes` |
| `[output.context].common_name_threshold` | integer | `2` | Definition count at which name-based edges are marked ambiguous |
| `[output.context].caller_omit_def_threshold` | integer | `5` | Definition count at which approximate callers switch to a `grep` hint |
| `[output.navigation].is_enabled` | bool | `false` | Resolve call sites structurally and mark confirmed targets `precise` |
| `[output.navigation].callsite_budget` | integer | `1000` | Max call sites checked structurally before name-based fallback |
| `[output.navigation].scan_limit` | integer | `16000` | Shared caller-scan hit cap across symbol names (min 25 per name) |
| `[output.redact].pii_entities` | string array | `[]` | Opt-in PII types to mask; see [PII Redaction](/configuration/pii-redaction/) |
| `[output.redact].sensitive_fields` | string array | `[]` | Additional field names treated as secrets (case/separator normalized) |
| `[output.redact].rules` | inline-table array | `[]` | Custom regex masking rules (`{ id = "custom.*", pattern = "..." }`) |
| `[output.redact].exceptions` | inline-table array | `[]` | Exact `{ rule_id, value }` pairs exempt from masking |
| `[index].mode` | `"auto"`, `"on"`, or `"off"` | `"auto"` | Indexing mode (restart required): `"auto"` (repos up to `auto_max_files`), `"on"` (always), or `"off"` (disabled) |
| `[index].auto_max_files` | integer | `50000` | Max indexable files in `auto` mode (restart required) |
| `[index].path` | string | `".codemap/index"` | Index directory path (restart required) |
| `[index].max_file_bytes` | integer bytes or size string | `"1mb"` (`1048576`) | Skip files larger than this during indexing |
| `[index].store_references` | bool | `false` | Store non-call symbol references in the index |
| `[index.refresh].watch` | bool | `true` | Watch filesystem changes in the background (restart required) |
| `[index.refresh].watch_debounce_ms` | integer (ms) | `500` | Filesystem watcher debounce window (restart required) |
| `[index.refresh].index_staleness_ms` | integer (ms) | `5000` | Staleness interval for request-triggered refreshes when `watch = false` |
| `[index.refresh].indexer_auto_restart` | bool | `true` | Restart the background indexer thread if it stops |
| `[index.language_support].is_document_support_enabled` | bool | `false` | Index `.md` and `.mdx` files |
| `[index.language_support].is_shell_support_enabled` | bool | `false` | Index `.sh`, `.bash`, and `.zsh` scripts |
| `[index.language_support].is_infrastructure_support_enabled` | bool | `false` | Index HCL/Terraform, Dockerfile, and Nix files |
| `[index.language_support].is_interface_support_enabled` | bool | `false` | Index Protocol Buffers and GraphQL files |
| `[index.language_support].is_build_support_enabled` | bool | `false` | Index Make, CMake, and Bazel build files |
| `[index.exclude].excluded_directories` | string array | See [Directory exclusions](#directory-exclusions) | Optional directory exclusion globs |
| `[index.exclude].use_git_exclude` | bool | `true` | Apply `.git/info/exclude` rules |
| `[output.context.exclude].should_include_test_code` | bool | `false` | Include test code in automatic symbol/caller context |
| `[output.context.exclude].test_file_patterns` | string array | See [Test-code context](#test-code-context) | Test file path globs |
| `[output.context.exclude].test_attributes` | language → string array | See [Test-code context](#test-code-context) | Per-language test attribute/annotation names |
| `[output.context.exclude].test_decorators` | language → string array | See [Test-code context](#test-code-context) | Per-language test decorator names |
| `[output.context.exclude].test_calls` | language → string array | See [Test-code context](#test-code-context) | Per-language test call names |
| `[output.jev].enabled` | bool | `false` | Enable the [Jev filter](#jev-filter) |
| `[output.jev].scope` | string array | `["search", "read", "grep"]` | Tools covered when Jev is enabled |
| `[output.jev].model` | string | `"jev-1.14.0"` | Decision model ID (`jev-*` for TypeSafe, `author/slug` for OpenRouter) |
| `[output.jev].timeout_ms` | positive integer (ms), max 7 days | `45000` | Per-call Jev timeout |
| `[output.jev].max_in_flight_requests` | integer, 1–8 | `8` | Max concurrent HTTP requests per Jev call |
| `[output.jev].request_spacing_ms` | integer (ms), >= 100 | `100` | Minimum delay between Jev batch requests |
| `[output.jev].max_batch_bytes` | integer bytes or size string, 1–168000 | `168000` | Max encoded payload size per Jev batch |
| `[output.jev].pool_idle_timeout_ms` | positive integer (ms), max 7 days | `30000` | Idle HTTPS connection timeout |
| `[output.jev].search_filter_min_unrelated_probability` | number, `0.5 < value <= 1.0` | `0.70` | Minimum probability required to omit an unrelated block |
| `[filesystem_permissions].find` | string | `"anywhere"` | Access scope for `find`: `"workspace"`, `"allowed_roots"`, or `"anywhere"` |
| `[filesystem_permissions].grep` | string | `"anywhere"` | Access scope for `grep`: `"workspace"`, `"allowed_roots"`, or `"anywhere"` |
| `[filesystem_permissions].read` | string | `"anywhere"` | Access scope for `read`: `"workspace"`, `"allowed_roots"`, or `"anywhere"` |
| `[filesystem_permissions].allowed_roots` | string array | `[]` | Extra directories allowed when a permission is `"allowed_roots"` |
| `[related_terms].enabled` | bool | `false` | Enable the `synonyms` tool |
| `[related_terms].dataset_path` | string | unset | Local sense dataset file path (skips first-use download) |
| `[update].config_auto_update` | bool | `true` | Create missing repo `config.toml`/`auth.toml` templates on `mcp` startup |

## Section details

### Global instructions

Detected client instruction file paths. Start a new agent session after changing `[global_instructions].enabled`:

| Client | Default file | Override env var |
| --- | --- | --- |
| Codex (`codex-mcp-client`) | `~/.codex/AGENTS.md` (`AGENTS.override.md` if non-empty) | `CODEX_HOME` |
| Claude Code (`claude-code`) | `~/.claude/CLAUDE.md` | `CLAUDE_CONFIG_DIR` |
| `pi` | `~/.pi/agent/AGENTS.md` | `PI_CODING_AGENT_DIR` |
| OpenCode (`opencode`) | `~/.config/opencode/AGENTS.md` | `OPENCODE_CONFIG_DIR` |

### Directory exclusions

Default `[index.exclude].excluded_directories` list: `[".idea", ".vscode", ".vs", "node_modules", ".yarn", "target", "dist", "build", "vendor"]`. Set `[]` to disable these defaults. VCS directories and `.codemap` are always excluded.

| Pattern | Matches |
| --- | --- |
| `build` or `**/build` | `build` at any depth |
| `./build` | Root `build` directory only |
| `apps/web/build` | That directory and everything under it |
| `apps/api/**/__pycache__` | `__pycache__` at any depth under `apps/api` |
| `apps/native/cmake-build-*` | Wildcard directory names under `apps/native` |

### Test-code context

Default test patterns excluded from automatic symbol/caller context when `[output.context.exclude].should_include_test_code = false`:

```toml
[output.context.exclude]
should_include_test_code = false
test_file_patterns = ["**/tests/**", "**/test/**", "**/__tests__/**", "test_*.py", "*_test.*", "*.test.*", "*_spec.*", "*.spec.*", "*Test.java", "*Tests.java", "*IT.java"]
[output.context.exclude.test_attributes]
rust = ["test", "tokio::test", "async_std::test", "rstest", "rstest::rstest", "cfg(test)"]
java = ["Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Nested", "BeforeEach", "AfterEach", "BeforeAll", "AfterAll"]
kotlin = ["Test", "ParameterizedTest", "RepeatedTest", "BeforeTest", "AfterTest", "BeforeEach", "AfterEach"]
csharp = ["Fact", "Theory", "Test", "TestCase", "TestCaseSource", "TestFixture", "SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown"]
swift = ["Test", "Suite"]
php = ["Test"]
[output.context.exclude.test_decorators]
python = ["pytest.fixture", "pytest.mark.*", "unittest.skip", "unittest.skipIf", "unittest.skipUnless", "unittest.expectedFailure"]
[output.context.exclude.test_calls]
javascript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
typescript = ["describe", "describe.*", "it", "it.*", "test", "test.*", "suite", "suite.*"]
dart = ["test", "group", "testWidgets"]
ruby = ["describe", "context", "it", "specify"]
powershell = ["Describe", "Context", "It"]
```

### Credential redaction

Rule IDs in `[output.redact].rules` must start with `custom.`. An optional `(?P<secret>...)` capture group masks only the captured match:

```toml
[output.redact]
pii_entities = ["EMAIL_ADDRESS", "CREDIT_CARD"]
sensitive_fields = ["internalCredential"]
rules = [{ id = "custom.acme", pattern = 'ACME_(?P<secret>[A-Z0-9]+)' }]
exceptions = [{ rule_id = "custom.acme", value = "ACME_EXAMPLE" }]
```

See [PII Redaction](/configuration/pii-redaction/) for all 90 built-in PII entity names.

### Macro expansion and event rules

Configure C/C++/assembly macro expansion (`clang` / `nasm`) and custom event-bus rules (`typescript`, `javascript`, `rust`) as follows:

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

[output.event_navigation]
is_enabled = true
use_builtin_rules = true
rules = [
  { id = "known-on", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "on", role = "subscribe", event_arg = 0, handler_arg = 1, bus = "receiver" },
  { id = "known-emit", language = "typescript", module = "src/known.ts", symbol = "KnownBus", method = "emit", role = "publish", event_arg = 0, bus = "receiver" },
]
```

### Jev filter

Jev uses a decision model to omit irrelevant code blocks from `search`, `read`, and `grep` responses. Store the API key in `auth.toml` (or `TYPESAFE_API_KEY` / `OPENROUTER_API_KEY`) and enable `[output.jev]` in `config.toml`:

```toml
# .codemap/auth.toml
[jev]
api_key = "[REDACTED]"
[openrouter]
api_key = "[REDACTED]"
```

```toml
# .codemap/config.toml
[output.jev]
enabled = true
scope = ["search", "read", "grep"]
model = "jev-1.14.0" # or an OpenRouter ID below
```

#### OpenRouter decision models

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

## Legacy configuration keys

Older keys remain accepted; when both appear in the same file, the current key takes precedence.

| Legacy key | Current key |
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
| `analysis.navigation.*` | `output.navigation.*` |
| `analysis.macro_expansion.*` | `output.macro_expansion.*` |
| `analysis.event_navigation.*` | `output.event_navigation.*` |
