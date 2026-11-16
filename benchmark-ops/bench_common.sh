# Common environment of the bench_* scripts. Source it with the cache folder name; it is not run on its own.
# Usage: . bench_common.sh <cache name under /Volumes/Samsung_990_PRO, e.g. wsk10>
BENCH_ROOT=/Volumes/Samsung_990_PRO
# Logs and reports stay outside the repository, in a folder the harness hides from solvers (isolation.hidden_references).
BENCH_OUT=$BENCH_ROOT/codemap-bench-work/bench-out
BENCH_LOGS=$BENCH_OUT/logs
# The harness is the checkout that holds these scripts unless BENCH_HARNESS names another worktree.
BENCH_HARNESS=${BENCH_HARNESS:-$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)}
BENCH_PY=/opt/homebrew/bin/python3
case "${1:-}" in
  ''|*/*|wsv4|wst6b|wst6a1)
    echo "cache name is missing, is a path, or is a protected folder: '${1:-}'" >&2
    return 1 2>/dev/null || exit 1;;
esac
export BENCH_HARNESS
export CODEMAP_BENCH_CACHE=$BENCH_ROOT/$1
export CODEMAP_BENCH_CLAUDE_TOKEN_FILE=/Users/buyong/.config/op-secrets/claude-oauth-token
# CodeGraph 1.6.2 refuses Node 25+; preparation uses nvm Node 24.14.0.
export PATH=/Users/buyong/.nvm/versions/node/v24.14.0/bin:$PATH
mkdir -p "$BENCH_LOGS"
