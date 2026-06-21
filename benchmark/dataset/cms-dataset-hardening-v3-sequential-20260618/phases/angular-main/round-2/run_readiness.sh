#!/usr/bin/env bash
set -uo pipefail

OUT_DIR="/Users/buyong/workspace/private/buyong-mcp/.agents/orchestration/cms-dataset-hardening-v3-sequential-20260618/phases/angular-main/round-2"

args=(
  claude -p
  --model sonnet
  --setting-sources ""
  --strict-mcp-config
  --allowedTools Bash,Read,Glob,Grep
  --disallowedTools Edit,Write,WebFetch,WebSearch,Task,NotebookEdit,TodoWrite,Workflow,Agent,Skill
  --output-format stream-json
  --verbose
  "Reply with exactly OK."
)

printf '%q ' "${args[@]}" > "$OUT_DIR/readiness_exact_command.txt"
printf '\n' >> "$OUT_DIR/readiness_exact_command.txt"

"${args[@]}" > "$OUT_DIR/readiness_stdout.jsonl" 2> "$OUT_DIR/readiness_stderr.txt"
status=$?
printf '%s\n' "$status" > "$OUT_DIR/readiness_exit_code.txt"

exit "$status"
