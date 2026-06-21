#!/usr/bin/env bash
set -uo pipefail

OUT_DIR="/Users/buyong/workspace/private/buyong-mcp/.agents/orchestration/cms-dataset-hardening-v3-sequential-20260618/phases/angular-main/round-2"
PROMPT_FILE="$OUT_DIR/public_question.md"
PROMPT="$(<"$PROMPT_FILE")"

args=(
  claude -p
  --model sonnet
  --setting-sources ""
  --strict-mcp-config
  --allowedTools Bash,Read,Glob,Grep
  --disallowedTools Edit,Write,WebFetch,WebSearch,Task,NotebookEdit,TodoWrite,Workflow,Agent,Skill
  --output-format stream-json
  --verbose
  "$PROMPT"
)

printf '%q ' "${args[@]}" > "$OUT_DIR/exact_command.txt"
printf '\n' >> "$OUT_DIR/exact_command.txt"

"${args[@]}" > "$OUT_DIR/claude_stdout.jsonl" 2> "$OUT_DIR/claude_stderr.txt"
status=$?
printf '%s\n' "$status" > "$OUT_DIR/exit_code.txt"

if [ -s "$OUT_DIR/claude_stdout.jsonl" ]; then
  jq -r '
    select(.type == "assistant")
    | .message.content[]?
    | select(.type == "text")
    | .text
  ' "$OUT_DIR/claude_stdout.jsonl" > "$OUT_DIR/raw_answer.md"
else
  : > "$OUT_DIR/raw_answer.md"
fi

exit "$status"
