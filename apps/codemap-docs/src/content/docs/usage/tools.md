---
title: MCP Tools
description: How to use initial_instructions, search, find, grep, read, analyze, and opt-in synonyms, plus workspace scoping, deduplication, exclusions, and optional Jev filtering.
---

## Navigation tools summary

| Tool | Use | Main arguments |
|---|---|---|
| `initial_instructions` | Load navigation guidance and, when the optional Jev relevance filter is enabled, register the user's task | `task` (required when Jev is enabled); none otherwise |
| `search` | Find implementations with ranked symbols and snippets | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context` |
| `find` | Find files or directories by glob or basename regex, newest first | `pattern`, `path`, `include_ignored`, `entry_type`, `max_depth`, `pattern_type` |
| `grep` | Search live files with a regex | `pattern`, `path`, `glob`, `type`, `output_mode`, `-i`, `-n`, `-A`, `-B`, `-C`, `multiline`, `head_limit`, `offset`, `include_ignored` |
| `read` | Read live source windows with line numbers, or list declarations with `view=definitions` | `file_path`, `offset`, `limit`, or `ranges`; `view` |
| `analyze` | Inspect index footprint or recorded reading activity as compact JSON | `target`, `limit`, `offset`, `sort`, `filter`, `view`, `days`, `tool` |
| `synonyms` (opt-in) | Look up related terms by field and topic in the local sense dictionary | `terms`: 1–16 search words, short compounds, or single-concept technical terms |

## Choosing between `search`, `grep`, `find`, and `read`

- `search`: Use when you know the behavior or concept you want, not the exact identifier or file.
- `grep`: Use for exact identifiers, comments, and recently edited code. Because `grep.pattern` is a regex, escape regex metacharacters for literal code separately from JSON string escaping. Output defaults to numbered `content`, while `files_with_matches` and `count` return paths or match counts.
- `find`: Use when you know the filename or path pattern rather than the file contents (sorted newest first).
- `read`: Use to inspect live line windows or list declarations with `view=definitions`. In addition to `file_path`, `offset`, and `limit`, `read` accepts `path`, `file`, `query`, and 1-based inclusive `start_line`/`end_line` or `start`/`end` aliases (including camelCase and kebab-case variants).

### Batched `read ranges`

Pass `ranges` to read 1–20 known windows in one call instead of top-level `file_path` (or its aliases):

```json
{
  "ranges": [
    { "file_path": "src/main.rs", "offset": 10, "limit": 20 },
    { "file_path": "src/lib.rs", "offset": 40, "limit": 15 }
  ],
  "view": "source"
}
```

Each item accepts the same path and line-range aliases, while top-level options such as `view`, `expand`, and `include_seen` apply to every item. Results return in input order with path and window headings, and an error in one item does not stop later items. If the combined response hits the read or client byte budget, remaining items are omitted from the response with a notice naming the first skipped range. See [live request controls](/configuration/#live-request-controls).

### Compact `grep` source, delivery limits, and relationships

Both `read` and `grep` group results by file under declaration headings with a single source section. Resolved callees show their definition file and line, and unambiguous same-file constant references show their definitions and initializer previews. Indexed declarations and callers can lag behind recent edits.

For compact `grep` output, set `view="source_grouped"` with `expand="none"`. File headings replace repeated per-line paths while keeping every matching row and line number (`source_grouped` applies only to content-mode `grep`, not `read`; `view="source"` keeps the path-per-row layout).

- `search` compacts duplicate presentation annotations and enforces `[output.client]` delivery byte ceilings by replacing lower-priority bodies with exact read ranges.
- Relevant `search` results can also include event maps and `Source routes` (static candidate links from storage sites to callbacks, arguments, or consumers, with stale links hidden until re-indexed).
- See [client delivery budgets](/configuration/#client-delivery-budgets), [search display compaction and grouped grep source](/configuration/#search-display-compaction-and-grouped-grep-source), [output and call relationships](/configuration/#output-and-call-relationships), and [indexed event navigation](/configuration/#indexed-event-navigation).

## Monorepo and external `workspace_scope`

In monorepos, `initial_instructions` lists available workspace scopes:

- Pass a scope as `workspace_scope` to restrict `search` to that directory (passing a file selects its parent folder).
- Omit `workspace_scope` or pass `all` (or `전체`) to search the full repository. Start repository-wide when you do not know which package owns an implementation.
- `search` never widens a selected scope silently. Top hits include detailed snippets (or matching lines and read ranges on responses over 32 KiB), followed by a bounded list of remaining files.

`workspace_scope` can also point to a directory outside the server's starting folder (absolute or relative) or to a linked git worktree inside it:

- `search` uses the index of the repository containing that directory (the worktree itself or the nearest parent with `.git`, `.hg`, or `.jj`, never the user home directory or its ancestors) and filters results to the target directory.
- That external index uses the target repository's config while keeping the current session's `[filesystem_permissions]`, whose `read` rule must cover the full target repository.
- The first external `search` builds the index, returns a warming notice until indexing finishes, and prints absolute paths. Subsequent `read` and `grep` calls reuse that index for declarations and callers once it exists.
- Only one external index stays open at a time and closes after 10 minutes of inactivity. If an index there uses an incompatible format, `search` reports `Index unavailable:` without modifying it, while `read`, `grep`, and `find` continue returning live disk results.

### Exact phrase matches outside the ranking

Plain `search` queries can append matches under `## Exact phrase matches outside the ranking` when a docstring or indexed string literal contains a quoted phrase or two adjacent query words, such as ``- src/events/publisher.rs (82 lines) — docstring of EventPublisher [L23-26]``.

- These rows only use file slots (`output.search.overview_file_limit`) and bytes (`output.search.max_bytes`) left over by ranked results, stay within `workspace_scope`, and never alter ranked matches or snippets.
- They are skipped for single-identifier queries, Boolean or field query syntax, active Jev filtering, partial output, or an unready index.
- Each row reports a literal phrase match only; read the file to confirm whether it is relevant.

## Shared deduplication (`include_seen`)

`search`, `read`, and `grep` accept an optional `include_seen` boolean (default `false`).

- Unchanged source already returned on the same connection (including short spans, boundary lines, and duplicates within the same response) is replaced once per response, or per `read ranges` window, with `<--removed duplicated-->`.
- Pass `include_seen=true` to return full un-deduplicated source again, such as after context loss. Masking, truncation notices, and output byte caps still apply.
- Deduplication history is connection-local, clears on initialization, task updates, or config reloads, and automatically returns fresh content when a file changes.

## Opt-in `synonyms`

Enable `synonyms` with `related_terms.enabled = true`. It downloads the pinned sense dataset once on first use, or you can set `related_terms.dataset_path` to a local `rt-sense/5` file.

- **Input terms**: Pass `1–16` semantic terms in `terms` (avoid compound identifiers or glue words; pass `{"terms":["avatar"]}` for `AvatarService`), keeping single-concept technical phrases like `abort signal` intact. Fields and topics in responses are labels, not input filters.
- **Preparation and output format**: Dictionary preparation runs in the background without blocking other tools. Responses report readiness status before word lists and show up to `6` display groups (`20` direct, `10` near, and `10` same-topic words per group) with truncation notices.
- **Codebase evidence ranking**: Once codebase evidence is indexed, senses and words matching repository declarations (including TypeScript, JavaScript, C, and C++ data members), imports, and calls rank first (`code uses "abort signal"` for direct hits; same-topic words alone do not count). Phrases resolved by parts count when every part appears in code, and word lines report how many shown words appear in code.
- **Fallback and refresh**: While evidence is building or when no code matches exist, senses keep dictionary order. Evidence updates with index refreshes and branch switches.
- **Lookup diagnostics**: Lookup notes appear (up to `8` entries) only when they add information beyond the input, and non-English misses prompt retrying with English terms.

## Configure exclusions and output

Settings merge per key in order: `<repo>/.codemap/config.toml` → `$CODEMAP_HOME/config.toml` (default `~/.codemap/config.toml`) → built-in defaults. Comment out a repository key to inherit the global or default value.

- **Credential masking**: Detected API keys, tokens, passwords, and private keys are masked by default (`output.is_redact_enabled = false` disables masking). Matching and ranking still use original text. See [credential redaction](/configuration/#credential-redaction).
- **Test-code context**: Automatic symbol and call context excludes test regions by default (`output.context.exclude.should_include_test_code = true` includes them); direct `read` and `grep` source remains unaffected. See [test-code context](/configuration/#test-code-context).
- **Directory exclusions**: `index.exclude.excluded_directories` combines with `.gitignore`, `.git/info/exclude`, and `.codemapignore` to skip build and cache directories. `find` and `grep` can bypass optional exclusions with `include_ignored: true`, while `read` follows `[filesystem_permissions]`. See [directory exclusions](/configuration/#directory-exclusions).
- **Reloading changes**: Edits inside existing config directories apply automatically after about 1 second. Exclusion or language-support changes queue a full index refresh, while `index.path` and watch settings require a server restart. See the [configuration reference](/configuration/) for all keys, defaults, and migration details.

## Optional Jev decision stages

Jev is an optional relevance filter (off by default) that filters returned code against the user's task. `search` evaluates complete function bodies, while `read` and `grep` also evaluate cut function windows and lines outside functions.

- **Enable and authenticate**: Set `output.jev.enabled = true` and choose covered tools in `output.jev.scope` (default `["search", "read", "grep"]`; `[]` selects none). Store a TypeSafe key in `.codemap/auth.toml` under `[jev].api_key`, in `$CODEMAP_HOME/auth.toml` (`~/.codemap/auth.toml`), or in `TYPESAFE_API_KEY` (checked in that order). To use an OpenRouter decision model such as `openai/gpt-6-luna-decisions`, set `output.jev.model` (default `jev-1.14.0`) and provide `[openrouter].api_key` or `OPENROUTER_API_KEY`; non-decision models are rejected. See [OpenRouter decision models](/configuration/#openrouter-decision-models).
- **Register the task**: Before calling an enabled filter, pass the user's complete request once as `task` to `initial_instructions`, and call it again whenever the task changes. A blank or invalid `task` returns an error and clears the previous task; search queries never replace `task`.
- **Apply filtering and fallbacks**: Enabled `search`, `read`, and `grep` calls send the masked `task` and masked candidate code blocks to the provider and omit code marked unrelated, while keeping uncertain blocks and original file locations so reading an exact omitted range returns the code directly. Provider errors fall back to ordinary unfiltered output, and client delivery limits still apply.
- **Bypass Jev on wide reads**: To read wider windows without Jev filtering, remove `"read"` from `output.jev.scope` or disable Jev (`include_seen=true` only bypasses deduplication). See [optional Jev decision stages](/configuration/#optional-jev-decision-stages).
