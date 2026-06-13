#!/bin/bash
# 회차 집계: arm×repo 정답률 + 효율 지표, 실패 에피소드 목록. 사용: aggregate-results.sh <iteration>
set -u
HARNESS_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$HARNESS_DIR/config.sh"

ITER="${1:?iteration 이름 필요}"
ALL=$(find "$BENCH_ROOT/results/$ITER" -name '*.metrics.json' -print0 | xargs -0 cat | jq -s '.')

echo "=== arm × repo 정답률 ==="
echo "$ALL" | jq -r '
  group_by(.arm + "|" + .repo)[] |
  {key: (.[0].arm + " × " + .[0].repo), n: length,
   correct: ([.[] | select(.score == "correct")] | length),
   partial: ([.[] | select(.score == "partial")] | length),
   wrong: ([.[] | select(.score == "wrong")] | length)} |
  "\(.key): \(.correct)/\(.n) correct (partial \(.partial), wrong \(.wrong)) — 정답률 \((.correct * 100 / .n | floor))%"'

echo; echo "=== arm 전체 ==="
echo "$ALL" | jq -r '
  group_by(.arm)[] |
  {arm: .[0].arm, n: length,
   correct: ([.[] | select(.score == "correct")] | length),
   avg_turns: (([.[] | .turns] | add) / length * 10 | floor / 10),
   avg_dur: (([.[] | .duration_s] | add) / length * 10 | floor / 10),
   fat_null: ([.[] | select(.first_answer_turn == null)] | length)} |
  "\(.arm): 정답률 \((.correct * 100 / .n | floor))% (\(.correct)/\(.n)), avg_turns \(.avg_turns), avg_dur \(.avg_dur)s, first_answer 미도달 \(.fat_null)"'

echo; echo "=== 비-correct 에피소드 (분석 대상) ==="
echo "$ALL" | jq -r '[.[] | select(.score != "correct")] | sort_by(.task_id)[] |
  "\(.episode_id): \(.score) — \(.score_rationale)"'

echo; echo "=== 과제별 correct 수 (4 = 전 arm·rep 통과) ==="
echo "$ALL" | jq -r '
  group_by(.repo + "-" + .task_id)[] |
  "\(.[0].repo) \(.[0].task_id): \([.[] | select(.score == "correct")] | length)/4"' | sort -t' ' -k1,1 -k2,2V
