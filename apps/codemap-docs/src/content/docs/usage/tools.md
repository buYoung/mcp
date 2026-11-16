---
title: MCP Tools
description: Per-tool usage examples on grafana/grafana.
---

## Tools summary

| Tool | Use | Main arguments |
|---|---|---|
| `initial_instructions` | Load navigation guidance and monorepo scopes | `task` (when [Jev filter](/configuration/#jev-filter) is enabled) |
| `search` | Search symbols, docstrings, and literals with BM25 | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context`, `include_seen` |
| `find` | Find files or directories by glob or basename regex (newest first) | `pattern`, `path`, `include_ignored`, `entry_type`, `max_depth`, `pattern_type` |
| `grep` | Search live files with a regex | `pattern`, `path`, `glob`, `output_mode`, `view`, `expand` |
| `read` | Read live file windows or list declarations | `file_path`, `offset`, `limit`, `ranges`, `view`, `expand`, `include_seen` |
| `analyze` | Inspect index size or recorded reading activity as JSON | `target`, `limit`, `offset`, `sort`, `filter`, `view`, `days`, `tool` |
| `synonyms` (opt-in) | Look up related English code terms and synonyms | `terms` (`1–16` words or short technical phrases) |

See [Output differences by option](/configuration/#output-differences-by-option) and the [Configuration Playground](/configuration/playground/) to inspect how `config.toml` settings change tool responses.

## Tool examples (`grafana/grafana`)

### `initial_instructions`

Call once at session start:

```json
{ "name": "initial_instructions", "arguments": {} }
```

```text
The scope inventory below lists selectable scopes; call initial_instructions again only if it reports a warming indexer.

Selectable workspace_scope values:
- pkg (11,420 files; Go)
- public (9,810 files; TypeScript, SCSS)
- packages/grafana-data (640 files; TypeScript)
- packages/grafana-ui (1,510 files; TypeScript)
```

### `search`

```json
{
  "name": "search",
  "arguments": {
    "query": "SaveDashboard",
    "workspace_scope": "pkg"
  }
}
```

````markdown
_Workspace scope: `pkg`. Pass `workspace_scope: "all"` for repo-wide search._

# codemap-search

## 1. pkg/services/dashboards/service/dashboard_service.go
- `method DashboardServiceImpl.SaveDashboard` (L142-178)
  - _callers (approximate name scan):_
    - `pkg/api/dashboard.go:318` (`HTTPServer.PostDashboard`)
  - _calls (depth 1, approximate):_
    - `buildSaveDashboardCommand` — `pkg/services/dashboards/service/dashboard_service.go:96`

### results
```
142→func (dr *DashboardServiceImpl) SaveDashboard(ctx context.Context, dto *dashboards.SaveDashboardDTO, allowUiUpdate bool) (*dashboards.Dashboard, error) {
143→    cmd, err := dr.buildSaveDashboardCommand(ctx, dto, allowUiUpdate)
144→    if err != nil {
145→        return nil, err
146→    }
147→    return dr.store.SaveDashboard(ctx, *cmd)
148→}
```
````

### `grep`

Match exact identifiers, error names, or unsaved edits with a regex. Escape literal regex characters in `pattern`:

```json
{
  "name": "grep",
  "arguments": {
    "pattern": "ErrDashboardVersionMismatch",
    "path": "pkg",
    "view": "source_grouped",
    "expand": "none"
  }
}
```

```markdown
# codemap-search

## 1. pkg/services/dashboards/models.go
48:var ErrDashboardVersionMismatch = errors.New("the dashboard has been changed by someone else")

## 2. pkg/api/dashboard.go
334:    if errors.Is(err, dashboards.ErrDashboardVersionMismatch) {
```

### `find`

```json
{
  "name": "find",
  "arguments": {
    "pattern": "*dashboard*service*.go",
    "path": "pkg/services"
  }
}
```

```text
pkg/services/dashboards/service/dashboard_service.go
pkg/services/dashboards/service/dashboard_service_test.go
```

### `read`

Read a single window (`offset` + `limit`, or 1-based inclusive `start_line` + `end_line`), or batch up to 20 windows via `ranges`:

```json
{
  "name": "read",
  "arguments": {
    "ranges": [
      { "file_path": "pkg/api/dashboard.go", "offset": 315, "limit": 8 },
      { "file_path": "pkg/services/dashboards/models.go", "offset": 46, "limit": 5 }
    ],
    "view": "source"
  }
}
```

### `analyze`

Pass `target: "index"` for index size or `target: "reads"` for recorded reading activity:

```json
{
  "name": "analyze",
  "arguments": { "target": "index", "language": "go", "sort": "size", "limit": 2 }
}
```

```json
{
  "target": "index",
  "status": "ok",
  "summary": { "files": 11420, "symbols": 198430 },
  "columns": ["path", "language", "stored", "size", "lines", "symbols"],
  "rows": [
    ["pkg/services/sqlstore/migrations/migrations.go", "go", 98210, 84210, 2140, 68],
    ["pkg/api/api.go", "go", 61400, 52380, 1180, 12]
  ]
}
```

### `synonyms`

`synonyms` requires `[related_terms].enabled = true` in `.codemap/config.toml`. Pass single words or single-concept phrases (`"rate limit"`), and split compound identifiers into core words (`DashboardQuota` → `["dashboard", "quota"]`):

```json
{ "name": "synonyms", "arguments": { "terms": ["quota"] } }
```

```markdown
### Resource Allocation ("quota")
Direct (showing 3 of 3; 3 in code): "allowance", "limit", "threshold"
```
