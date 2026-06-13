#!/bin/bash
# 채점 배치 입력 생성: 에피소드 answer_text + 과제 rubric/expected 결합 (채점은 sonnet 에이전트 몫)
# 사용: build-scoring-batches.sh <iteration>
# 배치 = repo × arm × rep (각 10 에피소드) → results/<iteration>/scoring/batch-*.json
set -u
HARNESS_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$HARNESS_DIR/config.sh"

ITER="${1:?iteration 이름 필요 (예: ds-iter1)}"
OUT_DIR="$BENCH_ROOT/results/$ITER/scoring"
ARMS="${ARMS:-claude-sonnet codex-gpt55}"
mkdir -p "$OUT_DIR"

for REPO in django strapi; do
  TASKS=$(tasks_json "$REPO")
  for ARM in $ARMS; do
    for REP in 1 2; do
      BATCH="$OUT_DIR/batch-$REPO-$ARM-r$REP.json"
      FILES=""
      for TID in $(jq -r '.tasks[].id' "$TASKS"); do
        M="$BENCH_ROOT/results/$ITER/$REPO/$REPO-$ARM-$TID-r$REP.metrics.json"
        [ -s "$M" ] || { echo "[error] metrics 없음: $M" >&2; exit 2; }
        FILES="$FILES $M"
      done
      # shellcheck disable=SC2086
      jq -s --slurpfile tasks "$TASKS" '
        map(. as $m
          | ($tasks[0].tasks[] | select(.id == $m.task_id)) as $t
          | {episode_id: $m.episode_id, task_id: $m.task_id,
             prompt: $t.prompt, expected: $t.expected, rubric: $t.rubric,
             answer_text: $m.answer_text})' $FILES > "$BATCH"
      echo "[batch] $BATCH ($(jq 'length' "$BATCH") episodes)"
    done
  done
done
