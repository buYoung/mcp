#!/bin/bash
# Start a script of this folder detached from the terminal session, so it keeps running when the session ends, and keep
# the Mac awake while it runs. Its output goes to detached-<script>-<time>.out in the logs folder of bench_common.sh.
# Usage: bench_detach.sh <script name in this folder> [arguments ...]
set -uo pipefail
S=$(cd "$(dirname "$0")" && pwd)
script=$S/${1:?script name}
shift
[ -f "$script" ] || { echo "no such script: $script"; exit 1; }
logs=/Volumes/Samsung_990_PRO/codemap-bench-work/bench-out/logs
mkdir -p "$logs"
out=$logs/detached-$(basename "$script" .sh)-$(date +%Y%m%dT%H%M%S).out
nohup /opt/homebrew/bin/python3 -c 'import os, sys; os.setsid(); os.execvp("caffeinate", ["caffeinate", "-i", "/bin/bash", *sys.argv[1:]])' \
  "$script" "$@" > "$out" 2>&1 < /dev/null &
echo "started pid $! output $out"
