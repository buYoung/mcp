---
title: 탐색 도구 사용
description: grafana/grafana 저장소 기준 도구별 실행 예시와 설정 옵션별 출력 차이를 안내합니다.
---

## 도구 요약

| 도구 | 용도 | 주요 인자 |
|---|---|---|
| `initial_instructions` | 탐색 안내 및 모노레포 범위 목록 조회 | `task` ([Jev 필터](/ko/configuration/#jev-필터) 활성화 시) |
| `search` | 심볼, 문서 주석, 문자열 리터럴 BM25 검색 | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context`, `include_seen` |
| `find` | glob 또는 파일명 정규식으로 파일·디렉터리 찾기(최근 수정 순) | `pattern`, `path`, `include_ignored`, `entry_type`, `max_depth`, `pattern_type` |
| `grep` | 실시간 파일 정규식 검색 | `pattern`, `path`, `glob`, `output_mode`, `view`, `expand` |
| `read` | 실시간 파일 줄 범위 읽기 또는 선언 목록 조회 | `file_path`, `offset`, `limit`, `ranges`, `view`, `expand`, `include_seen` |
| `analyze` | 색인 용량 및 읽기 이력 JSON 집계 | `target`, `limit`, `offset`, `sort`, `filter`, `view`, `days`, `tool` |
| `synonyms` (선택형) | 관련 영어 코드 용어 및 유의어 조회 | `terms`(`1–16`개 단어 또는 짧은 기술 구절) |

## 도구별 사용 예시 (`grafana/grafana`)

### `initial_instructions`

세션 시작 시 호출하면 탐색 규칙과 모노레포에서 선택 가능한 `workspace_scope` 목록을 반환합니다.

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

심볼이나 동작 개념을 BM25로 검색하며, `workspace_scope`로 검색 대상 패키지를 좁힐 수 있습니다.

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

정확한 식별자, 오류 상수, 방금 수정한 코드를 정규식으로 찾습니다. `pattern`에 정규식 특수문자가 들어가면 이스케이프하세요.

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

파일 내용이 아니라 파일명·경로 패턴으로 파일이나 디렉터리를 찾습니다.

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

단일 줄 범위(`offset` + `limit` 또는 `start_line` + `end_line`)를 읽거나, `ranges` 배열(최대 20개)로 여러 구간을 한 번에 읽습니다.

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

색인 규모(`target: "index"`)나 기록된 조회 통계(`target: "reads"`)를 JSON으로 확인합니다.

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

`.codemap/config.toml`에서 `[related_terms].enabled = true`로 활성화한 뒤 사용합니다. 저장소 코드에 등장하는 관련 영어 용어와 유의어를 찾습니다. `DashboardQuota` → `["dashboard", "quota"]`처럼 복합어는 단어별로 쪼개고, `"rate limit"` 같은 단일 개념 구절은 그대로 넣으세요.

```json
{ "name": "synonyms", "arguments": { "terms": ["quota"] } }
```

```markdown
### Resource Allocation ("quota")
Direct (showing 3 of 3; 3 in code): "allowance", "limit", "threshold"
```

## 설정 옵션에 따른 출력 차이 (`켰을 때` vs `껐을 때`)

### 1. 호출 관계 문맥 (`[output.context].is_enabled` / `caller_context`)

`search` 결과의 심볼 선언 아래에 상위 호출자(`_callers_`)와 하위 호출 대상(`_calls_`)을 표시합니다.

- **켰을 때 (`true`, 기본값)**:
  ```markdown
  ## 1. pkg/services/dashboards/service/dashboard_service.go
  - `method DashboardServiceImpl.SaveDashboard` (L142-178)
    - _callers (tree-sitter precise; import/source resolution confirmed):_
      - `pkg/api/dashboard.go:318` (`HTTPServer.PostDashboard`)
    - _calls (depth 1, tree-sitter precise where marked; fallback approximate):_
      - `DashboardServiceImpl.buildSaveDashboardCommand` (`L96`, precise)
  ```
- **껐을 때 (`false`, 또는 호출 시 `caller_context: false`)**:
  ```markdown
  ## 1. pkg/services/dashboards/service/dashboard_service.go
  - `method DashboardServiceImpl.SaveDashboard` (L142-178)
  ```

### 2. `read` 범위 밖 호출자 표시 (`[output.read].is_outside_callers_enabled`)

`read` (`view="source"`) 응답 끝에 조회한 줄 범위 바깥에서 해당 함수를 호출하는 위치를 덧붙입니다.

- **켰을 때 (`true`, 기본값)**:
  ```text
  142→func (dr *DashboardServiceImpl) SaveDashboard(ctx context.Context, dto *dashboards.SaveDashboardDTO, allowUiUpdate bool) (*dashboards.Dashboard, error) {
  ...
  148→}
  [Callers from outside this window (static analysis, approximate)]
  - SaveDashboard L142: pkg/api/dashboard.go:318 (PostDashboard)
  ```
- **껐을 때 (`false`)**:
  ```text
  142→func (dr *DashboardServiceImpl) SaveDashboard(ctx context.Context, dto *dashboards.SaveDashboardDTO, allowUiUpdate bool) (*dashboards.Dashboard, error) {
  ...
  148→}
  ```

### 3. 호출 관계에 테스트 코드 포함 (`[output.context.exclude].should_include_test_code`)

`search`와 `read`의 호출자 문맥에 `*_test.go`, `*.test.ts` 같은 테스트 파일을 함께 표시합니다.

- **켰을 때 (`true`)**:
  ```markdown
  - _callers (tree-sitter precise; import/source resolution confirmed):_
    - `pkg/api/dashboard.go:318` (`HTTPServer.PostDashboard`)
    - `pkg/services/dashboards/service/dashboard_service_test.go:88` (`TestSaveDashboard`)
  ```
- **껐을 때 (`false`, 기본값)**:
  ```markdown
  - _callers (tree-sitter precise; import/source resolution confirmed):_
    - `pkg/api/dashboard.go:318` (`HTTPServer.PostDashboard`)
  ```

### 4. 인증정보·PII 마스킹 (`[output].is_redact_enabled` / `pii_entities`)

MCP 출력에서 비밀값과 선택한 PII 유형을 `[REDACTED]`로 가립니다(`conf/defaults.ini` 조회 예시).

- **켰을 때 (`is_redact_enabled = true`, 기본값 `pii_entities = []`)**:
  ```ini
  admin_password = [REDACTED]
  from_address = admin@grafana.localhost
  ```
- **켰을 때 (`is_redact_enabled = true`, `pii_entities = ["EMAIL_ADDRESS"]`)**:
  ```ini
  admin_password = [REDACTED]
  from_address = [REDACTED]
  ```
- **껐을 때 (`is_redact_enabled = false`)**:
  ```ini
  admin_password = admin
  from_address = admin@grafana.localhost
  ```

### 5. Jev 작업 관련성 필터 (`[output.jev].enabled`)

활성화하면 `search` 결과에서 작업과 무관한 함수 본문을 한 줄 안내로 접습니다. 사용 전에 `initial_instructions`의 `task` 인자로 현재 요청을 등록하세요([Jev 필터](/ko/configuration/#jev-필터) 참고).

- **켰을 때 (`true`)**:
  ````markdown
  ### results
  ```
  142→func (dr *DashboardServiceImpl) SaveDashboard(...) (*dashboards.Dashboard, error) {
  ...
  148→}
  ```
  - _omitted body: L210-248 (method DeleteDashboard) Jev found no task match in pkg/services/dashboards/service/dashboard_service.go._
  ````
- **껐을 때 (`false`, 기본값)**:
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

### 6. 중복 구간 재출력 (`include_seen`)

`search`, `read`, `grep`은 기본적으로 같은 세션에서 이미 전달한 변경 없는 줄을 `<--removed duplicated-->`로 접습니다.

- **켰을 때 (`include_seen: true`)**:
  ```text
  142→func (dr *DashboardServiceImpl) SaveDashboard(...) (*dashboards.Dashboard, error) {
  ...
  148→}
  ```
- **껐을 때 (`include_seen: false`, 기본값)**:
  ```text
  <--removed duplicated-->
  ```
