#!/bin/bash
# 채점 결과를 metrics.json에 기입. 사용: write-scores.sh <iteration> <scores.json>
# scores.json: [{episode_id, score, score_rationale}, ...] (전 배치 평탄화)
set -u
HARNESS_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$HARNESS_DIR/config.sh"

ITER="${1:?iteration 이름 필요}"
SCORES="${2:?scores JSON 경로 필요}"

N=$(jq 'length' "$SCORES")
WRITTEN=0
for i in $(seq 0 $((N - 1))); do
  EID=$(jq -r ".[$i].episode_id" "$SCORES")
  REPO=$(echo "$EID" | cut -d'-' -f1)
  M="$BENCH_ROOT/results/$ITER/$REPO/$EID.metrics.json"
  [ -s "$M" ] || { echo "[error] metrics 없음: $M" >&2; exit 2; }
  jq --argjson s "$(jq ".[$i]" "$SCORES")" \
    '.score = $s.score | .score_rationale = $s.score_rationale' "$M" > "$M.tmp" && mv "$M.tmp" "$M"
  WRITTEN=$((WRITTEN + 1))
done
echo "[write-scores] $WRITTEN/$N 기입 완료"
REMAIN=$(find "$BENCH_ROOT/results/$ITER" -name '*.metrics.json' -exec jq -r '.score' {} \; | grep -c null || true)
echo "[write-scores] score null 잔여: $REMAIN"
