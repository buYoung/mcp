#!/bin/bash
# 에피소드 1개 실행 + 메트릭 추출. 사용: run-one-episode.sh "ITER|REPO|ARM|TASK|REP"
# 프롬프트는 tasks JSON에서 jq로 축어 추출 (한 글자도 재구성 금지 — playbook §2).
# 타임아웃은 perl alarm으로 실집행 (macOS에 timeout 명령 없음, 600s — playbook §4-4).
set -u
HARNESS_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$HARNESS_DIR/config.sh"

SPEC="$1"
ITER=$(echo "$SPEC" | cut -d'|' -f1)
REPO=$(echo "$SPEC" | cut -d'|' -f2)
ARM=$(echo "$SPEC" | cut -d'|' -f3)
TASK=$(echo "$SPEC" | cut -d'|' -f4)
REP=$(echo "$SPEC" | cut -d'|' -f5)

REPO_PATH=$(repo_path "$REPO") || { echo "[error] unknown repo: $REPO" >&2; exit 2; }
TASKS=$(tasks_json "$REPO")
EPISODE_ID="$REPO-$ARM-$TASK-r$REP"
OUT_DIR="$BENCH_ROOT/results/$ITER/$REPO"
mkdir -p "$OUT_DIR"
METRICS="$OUT_DIR/$EPISODE_ID.metrics.json"

# 멱등성: 완료된 에피소드는 재실행하지 않음 (중단 후 재개 안전)
[ -s "$METRICS" ] && { echo "[skip] $EPISODE_ID"; exit 0; }

PROMPT=$(jq -r --arg id "$TASK" '.tasks[] | select(.id==$id) | .prompt' "$TASKS")
EXPECTED_FILE=$(jq -r --arg id "$TASK" '.tasks[] | select(.id==$id) | .expected.file' "$TASKS")
[ -n "$PROMPT" ] && [ "$PROMPT" != "null" ] || { echo "[error] task not found: $TASK" >&2; exit 2; }

case "$ARM" in
  claude-sonnet|claude-sonnet-base) SUFFIX="claude" ;;
  codex-gpt55|codex-gpt55-base)     SUFFIX="codex" ;;
  *) echo "[error] unknown arm: $ARM" >&2; exit 2 ;;
esac

# baseline arm: MCP 유도 접두를 기계 제거한 프롬프트 사용 (재타이핑 금지 원칙 유지)
case "$ARM" in
  *-base) PROMPT="${PROMPT#"$MCP_PROMPT_PREFIX"}" ;;
esac
JSONL="$OUT_DIR/$EPISODE_ID.$SUFFIX.jsonl"
ERRLOG="$OUT_DIR/$EPISODE_ID.$SUFFIX.stderr"

run_contestant() {
  if [ "$ARM" = "claude-sonnet" ]; then
    (cd "$REPO_PATH" && perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
      claude -p --model sonnet --setting-sources "" \
      --mcp-config "$MCP_CONFIG" --strict-mcp-config \
      --allowedTools "$ALLOWED_TOOLS" --disallowedTools "$DISALLOWED_TOOLS" \
      --output-format stream-json --verbose "$PROMPT") < /dev/null > "$JSONL" 2> "$ERRLOG"
  elif [ "$ARM" = "claude-sonnet-base" ]; then
    # baseline: MCP 미설정 + 빈 strict mcp-config로 외부 MCP 차단, 빌트인만 허용
    (cd "$REPO_PATH" && perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
      claude -p --model sonnet --setting-sources "" \
      --strict-mcp-config \
      --allowedTools "$ALLOWED_TOOLS_BASE" --disallowedTools "$DISALLOWED_TOOLS_BASE" \
      --output-format stream-json --verbose "$PROMPT") < /dev/null > "$JSONL" 2> "$ERRLOG"
  elif [ "$ARM" = "codex-gpt55-base" ]; then
    # baseline: mcp_servers 설정 자체를 전달하지 않음
    perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
      codex exec -C "$REPO_PATH" --skip-git-repo-check --ignore-user-config --ephemeral \
      -s read-only -m gpt-5.5 -c model_reasoning_effort="medium" \
      -c approval_policy="never" \
      --json "$PROMPT" < /dev/null > "$JSONL" 2> "$ERRLOG"
  else
    perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
      codex exec -C "$REPO_PATH" --skip-git-repo-check --ignore-user-config --ephemeral \
      -s read-only -m gpt-5.5 -c model_reasoning_effort="medium" \
      -c approval_policy="never" \
      -c "mcp_servers.codemap-search.command=\"$BINARY\"" \
      -c 'mcp_servers.codemap-search.args=["mcp"]' \
      --json "$PROMPT" < /dev/null > "$JSONL" 2> "$ERRLOG"
  fi
}

START=$(date +%s)
run_contestant
RC=$?
DUR=$(( $(date +%s) - START ))

# 하니스 수준 실패(타임아웃 제외)만 1회 재시도 — 오답·타임아웃은 재시도 금지 (playbook §4-5)
if [ "$RC" -ne 0 ] && [ "$RC" -ne 142 ]; then
  echo "[retry] $EPISODE_ID rc=$RC" >&2
  START=$(date +%s)
  run_contestant
  RC=$?
  DUR=$(( $(date +%s) - START ))
fi

HERR=""
if [ "$RC" -eq 142 ]; then HERR="timeout"
elif [ "$RC" -ne 0 ]; then HERR="exit_$RC"
elif ! jq -s 'length' "$JSONL" > /dev/null 2>&1; then HERR="parse_failure"
fi

if [ -z "$HERR" ]; then
  PARTIAL=$("$HARNESS_DIR/extract-metrics.sh" "$ARM" "$JSONL" "$EXPECTED_FILE") || PARTIAL='{}'
else
  PARTIAL='{}'
fi

jq -n \
  --arg episode_id "$EPISODE_ID" --arg repo "$REPO" --arg arm "$ARM" --arg task_id "$TASK" \
  --argjson rep "$REP" --argjson duration "$DUR" --arg herr "$HERR" \
  --argjson partial "$PARTIAL" \
  '{episode_id: $episode_id, repo: $repo, arm: $arm, task_id: $task_id, rep: $rep,
    duration_s: $duration,
    harness_error: (if $herr == "" then null else $herr end),
    auth_variant: (if $arm == "claude-sonnet" then "setting-sources-empty" else "n/a" end)}
   + $partial
   + {score: null, score_rationale: null, notes: ""}' > "$METRICS"

echo "[done] $EPISODE_ID dur=${DUR}s herr=${HERR:-none}"
exit 0
