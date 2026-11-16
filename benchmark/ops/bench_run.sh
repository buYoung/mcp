#!/bin/bash
# Run the approved slots of a prepared cache for each solver model, one campaign after another (the harness allows one
# campaign per host), then approve the known review flags and resume so every answer is graded.
# It can be started again at any time: a finished campaign is skipped, a campaign waiting for review is reviewed and
# resumed, and a campaign that was cut off is resumed with its unstarted slots (that last path has not been exercised).
# Logs get a timestamp in the name, so nothing is overwritten. Start it with bench_detach.sh so that it keeps running
# when the terminal session ends.
# Usage: BENCH_CLIENT=<pi|codex> bench_run.sh <cache name> <tag> [model ...]   (default models: gpt-6.1-sol claude-opus-5-5)
set -uo pipefail
S=$(cd "$(dirname "$0")" && pwd)
. "$S/bench_common.sh" "${1:-}" || exit 1
cache=$1 tag=${2:?tag given to bench_ops.py inputs}
shift 2
[ $# -gt 0 ] || set -- gpt-6.1-sol claude-opus-5-5
IN=$CODEMAP_BENCH_CACHE/operator-input
cd "$BENCH_REPO" || exit 1
stamp() { date +%Y%m%dT%H%M%S; }
note() { echo "$(date '+%F %T') $* load $(sysctl -n vm.loadavg)"; }
state() { "$BENCH_PY" -I "$S/bench_ops.py" state "$cache" "$tag" "$1"; }
resume() {  # <model> <campaign folder> <extra JSON fields>
  local at file log
  at=$(stamp)
  file=$IN/resume-$tag-$1-$at.json
  log=$BENCH_LOGS/bench-$cache-$tag-resume-$1-$at.jsonl
  printf '{"run_root": "%s", "confirmed": true%s}\n' "$CODEMAP_BENCH_CACHE/route-preparation/campaigns/$2" "$3" > "$file"
  note "resume $1 $2 start"
  "$BENCH_PY" -m "$BENCH_PACKAGE.cli" resume < "$file" > "$log" 2> "${log%.jsonl}.stderr"
  note "resume $1 exit $? log $log"
}
failed=0
for model in "$@"; do
  input=$IN/run-$tag-$model.json
  [ -f "$input" ] || { echo "missing $input (write it with: bench_ops.py inputs)"; exit 1; }
  read -r kind campaign <<< "$(state "$model")"
  if [ "$kind" = none ]; then
    log=$BENCH_LOGS/bench-$cache-$tag-run-$model-$(stamp).jsonl
    note "run $model start"
    "$BENCH_PY" -m "$BENCH_PACKAGE.cli" run < "$input" > "$log" 2> "${log%.jsonl}.stderr"
    note "run $model exit $? log $log"
    read -r kind campaign <<< "$(state "$model")"
  elif [ "$kind" = unfinished ]; then
    resume "$model" "$campaign" ', "retry_unstarted": true'
    read -r kind campaign <<< "$(state "$model")"
  fi
  if [ "$kind" = review ]; then
    if reviews=$("$BENCH_PY" -I "$S/bench_ops.py" review "$cache" "$campaign" "$tag"); then
      while IFS= read -r review; do
        [ -n "$review" ] || continue
        name=$(basename "$review" .json)
        "$BENCH_PY" -m "$BENCH_PACKAGE.cli" approve-review < "$review" > "$BENCH_LOGS/bench-$cache-$name.jsonl" 2> "$BENCH_LOGS/bench-$cache-$name.stderr" \
          || { echo "approve-review failed: $name"; failed=1; }
      done <<< "$reviews"
      resume "$model" "$campaign" ''
      read -r kind campaign <<< "$(state "$model")"
    else
      echo "$model: a flag needs a person; campaign $campaign waits for review"
    fi
  fi
  note "$model ${kind:-unknown} ${campaign:-}"
  [ "$kind" = completed ] || failed=1
done
exit "$failed"
