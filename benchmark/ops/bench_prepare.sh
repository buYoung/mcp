#!/bin/bash
# Prepare a new neutral cache for a smoke or benchmark run (no model calls): install and index the chosen arms and build
# the plans of both solvers. It refuses a cache that already holds plans, and a harness whose pinned subject is not the
# commit you expect, so an old build cannot be measured by accident. Start it with bench_detach.sh for a long preparation.
# BENCH_CLIENT picks the harness (pi or codex); BENCH_SOLVER_MODEL picks the solver plans to prepare (default: all).
# Usage: BENCH_CLIENT=<pi|codex> bench_prepare.sh <cache name> <arms: all | comma list that includes rg> <full commit the harness must pin>
set -uo pipefail
S=$(cd "$(dirname "$0")" && pwd)
. "$S/bench_common.sh" "${1:-}" || exit 1
arms=${2:?arms: all or a comma list} expected=${3:?full commit the harness must pin}
model=${BENCH_SOLVER_MODEL:-all}
[ -e "$CODEMAP_BENCH_CACHE/route-preparation/plans" ] && { echo "cache already prepared: $CODEMAP_BENCH_CACHE"; exit 1; }
cd "$BENCH_REPO" || exit 1
pinned=$("$BENCH_PY" -c "from $BENCH_PACKAGE import route_contract as r; s = r.arm_contracts()[0]; print(s['git_ref'], s['commit'])") || exit 1
ref=${pinned%% *} commit=${pinned##* }
[ "$commit" = "$expected" ] || { echo "harness pins $ref $commit, expected $expected (move the pin: bench_ops.py pin)"; exit 1; }
[ "$(git rev-parse "$ref^{commit}" 2>/dev/null)" = "$expected" ] || { echo "tag $ref does not resolve to $expected"; exit 1; }
mkdir -p "$CODEMAP_BENCH_CACHE"
if [ ! -e "$CODEMAP_BENCH_CACHE/grafana" ] && [ -e "$BENCH_ROOT/wst6a1/grafana" ]; then
  # Reuse the pinned Grafana checkout of wst6a1 as an APFS clone when it exists; otherwise preparation fetches it.
  # Preparation verifies the checkout either way.
  cp -c -R "$BENCH_ROOT/wst6a1/grafana" "$CODEMAP_BENCH_CACHE/grafana" || exit 1
fi
echo "client $BENCH_CLIENT cache $CODEMAP_BENCH_CACHE arms $arms solver $model subject $ref $commit node $(node --version) harness $(git rev-parse --short HEAD) start $(date '+%F %T') load $(sysctl -n vm.loadavg)"
if [ "$arms" = all ]; then
  "$BENCH_PY" -m "$BENCH_PACKAGE.route_prepare" prepare --solver-model "$model"
else
  "$BENCH_PY" -m "$BENCH_PACKAGE.route_prepare" prepare --solver-model "$model" --arms "$arms"
fi
status=$?
echo "prepare exit $status $(date '+%F %T') load $(sysctl -n vm.loadavg)"
[ "$status" -ne 0 ] && exit "$status"
"$BENCH_PY" -m "$BENCH_PACKAGE.route_prepare" verify --solver-model "$model"
status=$?
echo "verify exit $status $(date '+%F %T')"
exit "$status"
