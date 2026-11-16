---
title: MCP Tools
description: How to use initial_instructions, search, find, grep, read, analyze, and opt-in synonyms.
---

## Tools summary

| Tool | Use | Main arguments |
|---|---|---|
| `initial_instructions` | Load navigation guidance | `task` (when [Jev filter](#optional-tools-and-filters) is enabled) |
| `search` | Search symbols, docstrings, and literals with BM25 | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context`, `include_seen` |
| `find` | Find files or directories by glob or basename regex (newest first) | `pattern`, `path`, `include_ignored`, `entry_type`, `max_depth`, `pattern_type` |
| `grep` | Search live files with a regex | `pattern`, `path`, `glob`, `type`, `output_mode`, `view`, `expand`, `-i`, `-n`, `-A`, `-B`, `-C`, `multiline`, `head_limit`, `offset`, `include_ignored`, `include_seen` |
| `read` | Read live file windows or list declarations | `file_path`, `offset`, `limit`, `ranges`, `view`, `expand`, `include_seen` |
| `analyze` | Inspect index size or recorded reading activity as JSON | `target`, `limit`, `offset`, `sort`, `filter`, `view`, `days`, `tool` |
| `synonyms` (opt-in) | Look up related English code terms in the local sense dictionary | `terms` (`1–16` words or short technical phrases) |

## Choosing a tool

- **`search`**: Find an implementation by behavior or concept when you do not know the exact identifier or file path.
- **`grep`**: Match exact identifiers, strings, comments, or unsaved edits with a regex.
  - Escape literal regex characters in `pattern`.
  - Pass `view="source_grouped"` with `expand="none"` for compact output grouped under one heading per file.
- **`find`**: Locate files or directories by name or path pattern rather than file contents.
- **`read`**: Inspect known line ranges (`offset` + `limit`, or 1-based inclusive `start_line` + `end_line`), or pass `view="definitions"` to list declarations only.

## Batch reading (`ranges`)

Pass `ranges` (1–20 items) instead of `file_path` to read multiple windows in one call:

```json
{
  "ranges": [
    { "file_path": "src/main.rs", "offset": 10, "limit": 20 },
    { "file_path": "src/lib.rs", "offset": 40, "limit": 15 }
  ],
  "view": "source"
}
```

Top-level display options (`view`, `expand`, `include_seen`) apply to every item in `ranges`.

## Scoping search (`workspace_scope`)

In monorepos, `initial_instructions` lists available package paths. Pass one as `workspace_scope` in `search` to restrict results to that directory, or omit it (or pass `"all"`) to search the whole repository.

`workspace_scope` also accepts an external repository directory or linked git worktree when `[filesystem_permissions].read` permits that repository.

## Deduplication (`include_seen`)

`search`, `read`, and `grep` fold unchanged lines already returned in the same session into `<--removed duplicated-->`. Pass `include_seen=true` to receive the full text again (for example, after context compaction).

## Optional tools and filters

- **`synonyms`** (when `[related_terms].enabled = true` in `.codemap/config.toml`; see [Configuration](/configuration/#key-reference)): In `terms`, split compound identifiers into core words (`AvatarService` → `["avatar"]`) and keep single-concept phrases intact (`"abort signal"`).
- **Jev filter**: When `[output.jev].enabled = true` in `.codemap/config.toml` (see [Jev filter](/configuration/#jev-filter)), `codemap-search` omits unrelated code blocks. Pass the user's request as `task` to `initial_instructions` before calling `search`, `read`, or `grep`, and update `task` when the request changes.
