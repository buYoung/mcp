---
title: MCP Tools
description: How to use initial_instructions, search, find, grep, read, analyze, and opt-in synonyms, plus workspace scoping, deduplication, exclusions, and optional Jev filtering.
---

## Navigation tools summary

| Tool | Use | Main arguments |
|---|---|---|
| `initial_instructions` | Load navigation guidance and, when Jev is enabled, register the user's task | `task` (required when Jev is enabled); none otherwise |
| `search` | Find implementations with ranked symbols and snippets | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context` |
| `find` | Find files or directories by glob or basename regex, newest first | `pattern`, `path`, `include_ignored`, `entry_type`, `max_depth`, `pattern_type` |
| `grep` | Search live files with a regex | `pattern`, `path`, `glob`, `type`, `output_mode`, `-i`, `-n`, `-A`, `-B`, `-C`, `multiline`, `head_limit`, `offset`, `include_ignored` |
| `read` | Read live source windows with line numbers, or list declarations with `view=definitions` | `file_path`, `offset`, `limit`, or `ranges`; `view` |
| `analyze` | Inspect index footprint or recorded reading activity as compact JSON | `target`, `limit`, `offset`, `sort`, `filter`, `view`, `days`, `tool` |
| `synonyms` (opt-in) | Look up related terms by field and topic in the local sense dictionary | `terms`: 1–16 selected search words, short operation compounds, or one-concept technical terms |

## Choosing between `search`, `grep`, `find`, and `read`

- **`search`**: Use when you know the behavior or concept you want, not the exact identifier or file.
- **`grep`**: Use for exact identifiers, comments, and recently edited code. Because `grep.pattern` is a regex, escape regex metacharacters when matching literal code (separately from JSON string escaping). Output defaults to numbered `content`, while `files_with_matches` and `count` return paths or match counts.
- **`find`**: Use to locate files or directories by glob or basename regex, sorted newest first.
- **`read`**: Use to inspect live line windows or list declarations with `view=definitions`. In addition to `file_path`, `offset`, and `limit`, `read` accepts `path`, `file`, and `query`, as well as 1-based inclusive `start_line`/`end_line` or `start`/`end` aliases (including camelCase and kebab-case variants).

### Batched `read ranges`

Pass `ranges` to read up to 20 known windows in a single call instead of top-level `file_path` (or its aliases):

```json
{
  "ranges": [
    { "file_path": "src/main.rs", "offset": 10, "limit": 20 },
    { "file_path": "src/lib.rs", "offset": 40, "limit": 15 }
  ],
  "view": "source"
}
```

Each item supports the same path and line-range aliases, while top-level `view`, `expand`, `include_seen`, and other presentation options apply across the batch. Results stay in input order with path and window headings, and an error in one item does not stop the rest. When the combined output hits the read or client byte budget, whole items are deferred with a notice naming the first omitted range. See [live request controls](/configuration/#live-request-controls).

### Compact `grep` source, delivery limits, and relationships

Both `read` and `grep` group results by file using declaration headings followed by a single source section. Resolved callees include their definition file and line, and same-file constant references show their definitions and initializer previews when unambiguous. Indexed declarations and callers can lag behind recent edits.

For compact `grep` output, set `view="source_grouped"` with `expand="none"`. File headings replace repeated per-line paths while keeping every matching row and line number intact. Standard `view="source"` keeps its path-per-row layout (`source_grouped` applies only to content-mode `grep`, not `read`).

In `search`, presentation annotations are compacted (after Jev evidence capture when Jev is enabled), and configured `[output.client]` delivery limits enforce a final byte ceiling that replaces lower-priority bodies with exact read ranges. Relevant results can also include event maps and `Source routes` linking storage sites to callbacks, passed arguments, or data consumers as static candidates, hiding stale dependencies until re-indexed. See [client delivery budgets](/configuration/#client-delivery-budgets), [search display compaction and grouped grep source](/configuration/#search-display-compaction-and-grouped-grep-source), [output and call relationships](/configuration/#output-and-call-relationships), and [indexed event navigation](/configuration/#indexed-event-navigation).

## Monorepo and external `workspace_scope`

In monorepos, `initial_instructions` lists the available workspace scopes. Pass one as `workspace_scope` to restrict `search` to that directory; passing a file selects its parent folder, while omitting it or passing `all` (or `전체`) searches the full repository. If you do not yet know which package owns an implementation, start with a repository-wide search and narrow down from the returned paths. `search` never silently widens a selected scope. Top hits include detailed snippets, or matching lines and read ranges on responses above 32 KiB, followed by a bounded list of remaining files.

`workspace_scope` can also point to a directory outside the server's starting folder (as an absolute or relative path) or to a linked git worktree inside it. In that case, `search` uses the index of the repository containing that directory (the worktree itself or the nearest parent with `.git`, `.hg`, or `.jj`, never treating the user home directory or its ancestors as a repository) and filters results to the target folder. The external index applies that repository's settings while keeping this session's `[filesystem_permissions]`, whose `read` rule must cover the full repository.

On the first external `search`, `codemap-search` creates the index without writing to that repository's config files, returns a warming notice until indexing finishes, and prints absolute paths in results. Subsequent `read` and `grep` calls reuse that external index for declarations and callers if it already exists. Only one external index stays open at a time and closes after 10 minutes of inactivity. If an index there uses another format, `search` reports `Index unavailable:` without modifying it, while `read`, `grep`, and `find` continue serving live disk results.

### Exact phrase matches outside the ranking

Plain `search` queries can list extra matches under `## Exact phrase matches outside the ranking` when a docstring or indexed string literal contains a quoted phrase or two adjacent query words, such as `- src/events/publisher.rs (82 lines) — docstring of EventPublisher [L23-26]`.

Rarer phrases are checked first, adding at most one new file per phrase and skipping phrases that appear in too many files. These rows only use file slots (`output.search.overview_file_limit`) and bytes (`output.search.max_bytes`) left over by ranked results, stay within `workspace_scope`, and never alter ranked matches or snippets. They are skipped for single-identifier queries, Boolean or field query syntax, active Jev filtering, partial output, or an unready index. Because a row only reports a literal phrase hit, read the file to confirm whether it is relevant.

## Shared deduplication (`include_seen`)

`search`, `read`, and `grep` share an optional boolean `include_seen` parameter (default `false`). Unchanged source already returned on the same connection (including short spans, boundary lines, and duplicates within the same response) is replaced once per response, or per `read ranges` window, with `<--removed duplicated-->`. `find` always returns current entries and does not deduplicate.

Pass `include_seen=true` when you need the full un-deduplicated source again, such as after context compaction or loss. Masking, truncation notices, and output byte caps still apply. Deduplication history is connection-local and resets on initialization, task updates, or config reloads, and any edited file automatically returns fresh content. This deduplication runs regardless of whether Jev is enabled.

## Opt-in `synonyms`

Set `related_terms.enabled = true` to enable `synonyms`. It downloads the pinned sense dataset once on first use, or you can point `related_terms.dataset_path` to a local `rt-sense/5` file (copied before indexing; upgrades sharing a dataset ID reuse the cached copy). Before calling `synonyms`, pick `1–16` semantic terms rather than passing raw compound identifiers or glue words. For example, call `{"terms":["avatar"]}` for `AvatarService`. Keep multi-word technical concepts like `abort signal` intact as a single item so the dictionary matches the phrase before splitting into parts.

Dictionary preparation runs in the background without blocking other tools. Each response shows up to `6` display groups (`20` direct, `10` near, and `10` same-topic words per group) with truncation notes, merging identical field/topic pairs within the same evidence tier and splitting the budget across input parts without pushing out supported senses.

Once codebase evidence is indexed, `synonyms` ranks senses and words that appear in code names first. Evidence comes from declarations (symbols plus TypeScript, JavaScript, C, and C++ data members collected for evidence only) as well as imported and called names such as `EventEmitter`:

- Senses whose direct terms contain the input and appear in code rank first (`code uses "abort signal"`), followed by senses with other direct or near code matches; same-topic words alone do not count as evidence.
- A phrase resolved by its parts counts as present when every part appears in code, and each word line reports how many shown words appear in code.
- While evidence is building or when no matches exist in code, senses keep their dictionary order. Evidence refreshes automatically with index updates and branch switches, and readiness status appears before the word lists.
- Only `terms` is accepted as input; fields and topics are response labels, not filters. Diagnostic lookup lines appear only when they add information beyond the input (capped at `8` entries), and non-English misses prompt retrying with English code terms.

## Configure exclusions and output

Settings merge per key in order: `<repo>/.codemap/config.toml`, then `$CODEMAP_HOME/config.toml` (default `~/.codemap/config.toml`), then built-in defaults. Comment out a repository key to inherit the global or default value.

- **Credential masking**: Detected API keys, tokens, passwords, and private keys are masked by default (`[output].is_redact_enabled = false` disables masking). Matching and ranking still use original text, and local files and indexes are never modified. You can add custom field names, regexes, and exact exceptions under `[output.redact]`, though unfamiliar secret shapes can still be missed. See [credential redaction](/configuration/#credential-redaction).
- **Test-code context**: Automatic symbol and call context excludes test regions by default (`[output.context.exclude].should_include_test_code = true` includes them). Custom `test_file_patterns` and per-language `test_attributes`, `test_decorators`, and `test_calls` lists replace inherited values (`[]` disables a category), while direct `read` and `grep` source remains intact. See [test-code context](/configuration/#test-code-context).
- **Directory exclusions**: On first startup, `codemap-search` generates a repository config with common VCS/editor exclusions and recursive globs matching detected project types:

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

After generation, maintain this list directly (`config_auto_update` handles file creation and schema sync, not re-adding deleted array items; see [loading and automatic writes](/configuration/#loading-and-automatic-writes) when migrating older configs). A bare name like `build` matches at any depth, `./build` targets the workspace root, and `apps/web/build` targets one project. Explicit lists replace optional defaults (`[]` clears them, omitting inherits). `.gitignore`, global Git ignores, `.git/info/exclude`, and `.codemapignore` apply independently, and VCS internals, `.codemap`, `.codemap-index`, and the active index directory are always excluded. `find` and `grep` bypass optional exclusions with `include_ignored: true`, while `read` follows `[filesystem_permissions]`.

Changes to existing config directories reload automatically after about 1000ms. Editing exclusions or language switches queues a full index refresh, while output limits and permissions apply on the next request. Restart the server after changing `index.path`, `index.refresh.watch`, or `index.refresh.watch_debounce_ms`, or if config watching was unavailable. See the [configuration reference](/configuration/) for all keys and defaults.

## Optional Jev decision stages

Jev is an optional filtering stage (off by default) that scores returned code against the user's task. In `search`, Jev evaluates complete function bodies; in `read` and `grep`, it also evaluates cut function windows and lines outside functions.

Enable it with `output.jev.enabled = true` and choose covered tools in `scope` (default `["search", "read", "grep"]`; `[]` selects none). Store your TypeSafe key in `.codemap/auth.toml` under `[jev].api_key`, in `$CODEMAP_HOME/auth.toml` (`~/.codemap/auth.toml`), or in `TYPESAFE_API_KEY` (resolved in that order). `auth.toml` files are excluded from indexing and default `find`/`grep`. To use an OpenRouter decision model such as `openai/gpt-6-luna-decisions`, set `output.jev.model` and store your OpenRouter key under `[openrouter].api_key` or `OPENROUTER_API_KEY`; the key goes only to OpenRouter's Decisions API, which rejects non-decision models. The default model is `jev-1.14.0`. See [OpenRouter decision models](/configuration/#openrouter-decision-models).

Before calling any enabled filter, pass the user's complete request once as `task` to `initial_instructions`, and call it again whenever the task changes. A blank or invalid `task` returns an error and clears the previous task, and search queries never replace `task`. The server generates one fixed yes/no question per candidate unit; the agent does not write prompts.

Enabled `search`, `read`, and `grep` calls send the masked `task` and masked unit source to the configured provider and omit code marked unrelated. Uncertain units stay visible, and omitted blocks retain their file location so reading that exact range returns the code directly. `initial_instructions` and `find` stay local. To read wider windows without Jev omission, remove `"read"` from `output.jev.scope` or set `output.jev.enabled = false` (`include_seen=true` only bypasses deduplication). Provider errors fall back cleanly to ordinary unfiltered output, while client delivery limits still apply. See [optional Jev decision stages](/configuration/#optional-jev-decision-stages) for bounds and diagnostics.
