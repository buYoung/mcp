#!/bin/bash
# bench django+strapi 캠페인 하니스 설정 (playbook §2 검증된 호출 형태 준수)
BINARY=/Users/buyong/workspace/private/buyong-mcp/apps/codemap-search/target/release/codemap-search
BENCH_ROOT=/tmp/benchmark-data
MCP_CONFIG=$BENCH_ROOT/mcp-codemap.json
TIMEOUT_S=600
ALLOWED_TOOLS="mcp__codemap-search__search,mcp__codemap-search__overview,mcp__codemap-search__read,mcp__codemap-search__find,mcp__codemap-search__grep"
DISALLOWED_TOOLS="Bash,Read,Glob,Grep,Edit,Write,WebFetch,WebSearch,Task,NotebookEdit,TodoWrite,Workflow,Agent,Skill"

# baseline arm (빌트인 전용, MCP 미설정) — claude는 codex 셸과의 동등 조건으로 Bash 포함
ALLOWED_TOOLS_BASE="Bash,Read,Glob,Grep"
DISALLOWED_TOOLS_BASE="Edit,Write,WebFetch,WebSearch,Task,NotebookEdit,TodoWrite,Workflow,Agent,Skill"
# baseline 프롬프트 변환: tasks JSON prompt에서 이 접두만 기계 제거 (전 과제 동일 접두 — 검증됨)
MCP_PROMPT_PREFIX="codemap-search MCP 도구를 사용해서 "

repo_path() {
  case "$1" in
    django) echo "$BENCH_ROOT/django-main" ;;
    strapi) echo "$BENCH_ROOT/strapi-develop" ;;
    *) return 1 ;;
  esac
}

tasks_json() {
  case "$1" in
    django) echo "$BENCH_ROOT/tasks-django.json" ;;
    strapi) echo "$BENCH_ROOT/tasks-strapi.json" ;;
    *) return 1 ;;
  esac
}
