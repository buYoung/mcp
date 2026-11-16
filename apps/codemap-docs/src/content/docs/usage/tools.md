---
title: MCP Tools
description: Per-tool usage examples and configuration toggle comparisons on grafana/grafana.
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
  - _callers (tree-sitter precise; import/source resolution confirmed):_
    - `pkg/api/dashboard.go:318` (`HTTPServer.PostDashboard`)
  - _calls (depth 1, tree-sitter precise where marked; fallback approximate):_
    - `DashboardServiceImpl.buildSaveDashboardCommand` (`L96`, precise)

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

## Output differences by option (`ON` vs `OFF`)

### 1. Caller and callee context (`[output.context].is_enabled`, `search` argument `caller_context`)

`[output.context].is_enabled` attaches static caller and callee links above `### results` in `search` output.

- **ON (`true`, default)**:
  ```markdown
  ## 1. pkg/services/dashboards/service/dashboard_service.go
  - `method DashboardServiceImpl.SaveDashboard` (L142-178)
    - _callers (tree-sitter precise; import/source resolution confirmed):_
      - `pkg/api/dashboard.go:318` (`HTTPServer.PostDashboard`)
    - _calls (depth 1, tree-sitter precise where marked; fallback approximate):_
      - `DashboardServiceImpl.buildSaveDashboardCommand` (`L96`, precise)
  ```
- **OFF (`false`, or per-call `caller_context: false`)**:
  ```markdown
  ## 1. pkg/services/dashboards/service/dashboard_service.go
  - `method DashboardServiceImpl.SaveDashboard` (L142-178)
  ```

### 2. Outside callers on `read` (`[output.read].is_outside_callers_enabled`)

`[output.read].is_outside_callers_enabled` appends a trailer in `read` (`view="source"`) listing callers outside the requested line range.

- **ON (`true`, default)**:
  ```text
  142→func (dr *DashboardServiceImpl) SaveDashboard(ctx context.Context, dto *dashboards.SaveDashboardDTO, allowUiUpdate bool) (*dashboards.Dashboard, error) {
  ...
  148→}
  [Callers from outside this window (static analysis, approximate)]
  - SaveDashboard L142: pkg/api/dashboard.go:318 (PostDashboard)
  ```
- **OFF (`false`)**:
  ```text
  142→func (dr *DashboardServiceImpl) SaveDashboard(ctx context.Context, dto *dashboards.SaveDashboardDTO, allowUiUpdate bool) (*dashboards.Dashboard, error) {
  ...
  148→}
  ```

### 3. Test callers in context (`[output.context.exclude].should_include_test_code`)

`[output.context.exclude].should_include_test_code` includes `*_test.go` and `*.test.ts` callers in `search` and `read` caller context.

- **ON (`true`)**:
  ```markdown
  - _callers (tree-sitter precise; import/source resolution confirmed):_
    - `pkg/api/dashboard.go:318` (`HTTPServer.PostDashboard`)
    - `pkg/services/dashboards/service/dashboard_service_test.go:88` (`TestSaveDashboard`)
  ```
- **OFF (`false`, default)**:
  ```markdown
  - _callers (tree-sitter precise; import/source resolution confirmed):_
    - `pkg/api/dashboard.go:318` (`HTTPServer.PostDashboard`)
  ```

### 4. Credential and PII redaction (`[output].is_redact_enabled`, `[output.redact].pii_entities`)

`[output].is_redact_enabled` and `[output.redact].pii_entities` mask secrets and selected PII types in MCP output (`conf/defaults.ini` example):

- **ON (`is_redact_enabled = true`, default `pii_entities = []`)**:
  ```ini
  admin_password = [REDACTED]
  from_address = admin@grafana.localhost
  ```
- **ON (`is_redact_enabled = true`, `pii_entities = ["EMAIL_ADDRESS"]`)**:
  ```ini
  admin_password = [REDACTED]
  from_address = [REDACTED]
  ```
- **OFF (`is_redact_enabled = false`)**:
  ```ini
  admin_password = admin
  from_address = admin@grafana.localhost
  ```

### 5. Jev task-relevance filter (`[output.jev].enabled`)

When enabled, `codemap-search` collapses unrelated callable bodies in `search` output into one-line omission notes. Register the user request first via the `task` argument of `initial_instructions` (see [Jev filter](/configuration/#jev-filter)).

- **ON (`true`)**:
  ````markdown
  ### results
  ```
  142→func (dr *DashboardServiceImpl) SaveDashboard(...) (*dashboards.Dashboard, error) {
  ...
  148→}
  ```
  - _omitted body: L210-248 (method DeleteDashboard) Jev found no task match in pkg/services/dashboards/service/dashboard_service.go._
  ````
- **OFF (`false`, default)**:
  ````markdown
  ### results
  ```
  142→func (dr *DashboardServiceImpl) SaveDashboard(...) (*dashboards.Dashboard, error) {
  ...
  148→}
  ```
  ```
  210→func (dr *DashboardServiceImpl) DeleteDashboard(...) error {
  ...
  248→}
  ```
  ````

### 6. Repeat seen lines (tool argument `include_seen`)

By default, `search`, `read`, and `grep` fold unchanged source lines already returned in the same session into `<--removed duplicated-->`.

- **ON (`include_seen: true`)**:
  ```text
  142→func (dr *DashboardServiceImpl) SaveDashboard(...) (*dashboards.Dashboard, error) {
  ...
  148→}
  ```
- **OFF (`include_seen: false`, default)**:
  ```text
  <--removed duplicated-->
  ```
