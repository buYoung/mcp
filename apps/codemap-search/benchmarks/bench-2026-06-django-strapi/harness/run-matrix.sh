#!/bin/bash
# 매트릭스 실행: 에피소드 단위 병렬, 동일 repo 포함 (안전성 실측 — playbook §4 소절).
# 사용: run-matrix.sh <iteration> [concurrency=8] [--pilot|--dry-run]
#   --pilot:   repo당 1과제 × 1rep × 전체 arm = 4 에피소드 (게이트, playbook §4-1)
#   --dry-run: 에피소드 목록만 생성·출력하고 실행하지 않음
#   ARMS 환경변수로 arm 목록 재정의 가능 (기본: MCP 2-arm; baseline 캠페인은
#   ARMS="claude-sonnet-base codex-gpt55-base")
set -u
HARNESS_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$HARNESS_DIR/config.sh"

ITER="${1:?iteration 이름 필요 (예: pilot, iter1)}"
CONC="${2:-8}"
MODE="${3:-full}"
ARMS="${ARMS:-claude-sonnet codex-gpt55}"

LIST="$BENCH_ROOT/results/$ITER.episodes.txt"
mkdir -p "$BENCH_ROOT/results"
: > "$LIST"

for REPO in django strapi; do
  TASKS=$(tasks_json "$REPO")
  [ -s "$TASKS" ] || { echo "[error] tasks JSON 없음: $TASKS" >&2; exit 2; }
  if [ "$MODE" = "--pilot" ]; then
    IDS=$(jq -r '.tasks[0].id' "$TASKS"); REPS="1"
  else
    IDS=$(jq -r '.tasks[].id' "$TASKS"); REPS="1 2"
  fi
  for ARM in $ARMS; do
    for TASK in $IDS; do
      for REP in $REPS; do
        echo "$ITER|$REPO|$ARM|$TASK|$REP" >> "$LIST"
      done
    done
  done
done

# 무작위 셔플 — 직렬 운영의 arm 교차 배치를 대체 (playbook §4-2)
perl -MList::Util=shuffle -e 'srand(42); print shuffle(<>)' "$LIST" > "$LIST.shuffled"

TOTAL=$(wc -l < "$LIST.shuffled" | tr -d ' ')
if [ "$MODE" = "--dry-run" ]; then
  echo "[dry-run] $TOTAL episodes, concurrency=$CONC"
  cat "$LIST.shuffled"
  exit 0
fi

echo "[matrix] $TOTAL episodes, concurrency=$CONC, iteration=$ITER"
WALL_START=$(date +%s)
xargs -n1 -P "$CONC" "$HARNESS_DIR/run-one-episode.sh" < "$LIST.shuffled"
WALL_END=$(date +%s)

DONE=$(find "$BENCH_ROOT/results/$ITER" -name '*.metrics.json' | wc -l | tr -d ' ')
echo "[matrix] 완료: $DONE/$TOTAL metrics, wall $((WALL_END - WALL_START))s → $BENCH_ROOT/results/$ITER/"
