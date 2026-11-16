# Common environment of the bench_* scripts. Source it with the cache folder name; it is not run on its own.
# BENCH_CLIENT picks the harness: pi (benchmark/pi) or codex (benchmark/codex).
# Usage: BENCH_CLIENT=<pi|codex> . bench_common.sh <cache name under /Volumes/Samsung_990_PRO, e.g. wsk10>
BENCH_ROOT=/Volumes/Samsung_990_PRO
BENCH_OPS=$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)
# The harness is the one in the checkout that holds these scripts; the harness hides that checkout from solvers.
BENCH_REPO=$(cd "$BENCH_OPS/../.." && pwd)
BENCH_LOGS=$BENCH_OPS/logs
case "${BENCH_CLIENT:-}" in
  pi|codex) ;;
  *)
    echo "set BENCH_CLIENT to pi or codex" >&2
    return 1 2>/dev/null || exit 1;;
esac
BENCH_PACKAGE=benchmark.$BENCH_CLIENT
BENCH_PY=/opt/homebrew/bin/python3
case "${1:-}" in
  ''|*/*|wsv4|wst6b|wst6a1)
    echo "cache name is missing, is a path, or is a protected folder: '${1:-}'" >&2
    return 1 2>/dev/null || exit 1;;
esac
export BENCH_CLIENT
export CODEMAP_BENCH_CACHE=$BENCH_ROOT/$1
export CODEMAP_BENCH_CLAUDE_TOKEN_FILE=/Users/buyong/.config/op-secrets/claude-oauth-token
# CodeGraph 1.6.2 refuses Node 25+; preparation uses nvm Node 24.14.0.
export PATH=/Users/buyong/.nvm/versions/node/v24.14.0/bin:$PATH
mkdir -p "$BENCH_LOGS"
