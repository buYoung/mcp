---
title: 탐색 도구 사용
description: initial_instructions, search, find, grep, read, analyze, 선택형 synonyms 사용법을 안내합니다.
---

## 도구 요약

| 도구 | 용도 | 주요 인자 |
|---|---|---|
| `initial_instructions` | 탐색 안내 조회 | `task` ([Jev 필터](#선택형-도구와-필터) 활성화 시) |
| `search` | 심볼, 주석 문서, 문자열 리터럴 BM25 검색 | `query`, `workspace_scope`, `language_hint`, `extension_hint`, `caller_context`, `include_seen` |
| `find` | glob 또는 파일명 정규식으로 파일·디렉터리 찾기(최근 수정 순) | `pattern`, `path`, `include_ignored`, `entry_type`, `max_depth`, `pattern_type` |
| `grep` | 실시간 파일 정규식 검색 | `pattern`, `path`, `glob`, `type`, `output_mode`, `view`, `expand`, `-i`, `-n`, `-A`, `-B`, `-C`, `multiline`, `head_limit`, `offset`, `include_ignored`, `include_seen` |
| `read` | 실시간 파일 줄 범위 읽기 또는 선언 목록 조회 | `file_path`, `offset`, `limit`, `ranges`, `view`, `expand`, `include_seen` |
| `analyze` | 색인 용량 및 읽기 이력 JSON 집계 | `target`, `limit`, `offset`, `sort`, `filter`, `view`, `days`, `tool` |
| `synonyms` (선택형) | 로컬 뜻 사전에서 관련 영어 코드 용어 조회 | `terms`(`1–16`개 단어 또는 짧은 기술 구절) |

## 도구 선택 기준

- **`search`**: 정확한 식별자나 파일 경로를 모를 때 동작·개념으로 구현 위치를 찾습니다.
- **`grep`**: 정확한 식별자, 문자열, 주석, 방금 수정한 코드를 정규식으로 찾습니다.
  - `pattern`은 정규식이므로 코드 특수문자는 이스케이프하세요.
  - 파일별로 원문만 간결하게 묶어 보려면 `view="source_grouped"`, `expand="none"`을 함께 넘기세요.
- **`find`**: 파일 내용이 아니라 파일명이나 경로 패턴으로 파일·디렉터리를 찾습니다.
- **`read`**: 지정한 줄 범위(`offset` + `limit` 또는 1부터 시작하는 `start_line` + `end_line`)를 읽거나, `view="definitions"`로 선언 목록만 확인합니다.

## 일괄 읽기 (`ranges`)

여러 구간을 한 번에 읽으려면 `file_path` 대신 `ranges` 배열(1–20개)을 넘기세요.

```json
{
  "ranges": [
    { "file_path": "src/main.rs", "offset": 10, "limit": 20 },
    { "file_path": "src/lib.rs", "offset": 40, "limit": 15 }
  ],
  "view": "source"
}
```

최상위 표시 옵션(`view`, `expand`, `include_seen`)은 `ranges`의 모든 항목에 공통 적용됩니다.

## 검색 범위 제한 (`workspace_scope`)

모노레포에서는 `initial_instructions`가 선택 가능한 패키지 경로를 반환합니다. `search` 호출 시 `workspace_scope`로 특정 디렉터리만 검색할 수 있으며, 생략하거나 `"all"`을 넘기면 저장소 전체를 검색합니다.

`[filesystem_permissions].read`가 허용하는 범위 안에서는 외부 저장소 경로(또는 연결된 git worktree)도 `workspace_scope`로 지정해 검색할 수 있습니다.

## 중복 생략 해제 (`include_seen`)

`search`, `read`, `grep`은 같은 세션에서 이미 반환한 변경 없는 원문을 `<--removed duplicated-->`로 줄입니다. 컨텍스트 압축 등으로 전체 원문을 다시 받아야 할 때는 `include_seen=true`를 넘기세요.

## 선택형 도구와 필터

- **`synonyms`** ([설정 레퍼런스](/ko/configuration/#설정-키)에서 `[related_terms].enabled = true`일 때): `terms`에 복합 식별자는 단어로 쪼개 넣고(`AvatarService` → `["avatar"]`), 단일 개념 구절(`"abort signal"`)은 그대로 넘기세요.
- **Jev 필터**: `.codemap/config.toml`에서 `[output.jev].enabled = true`로 켜면 작업과 무관한 코드 블록을 생략합니다. 활성화한 뒤에는 `search`, `read`, `grep`을 호출하기 전에 `initial_instructions`의 `task` 인자로 사용자 요청을 등록하고, 작업이 바뀌면 다시 호출하세요. API 키와 모델 설정은 [Jev 필터](/ko/configuration/#jev-필터)를 참고하세요.
