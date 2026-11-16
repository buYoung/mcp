#!/bin/bash
# Create a harness worktree at a given commit and point it at prepared plans of a cache. No model calls, no indexing.
# The pi harness runs only from a checkout whose benchmark/ folder is the pi harness (commit cb89d6886 or ee1441b04).
# The plan pointers and handoff files are copied from the immutable copies inside the cache's plan folders.
# node_modules becomes a link to the main checkout's, which must hold @inquirer/prompts 8.4.2: the harness CUI checks
# import it, and `route_prepare prepare` runs those checks. `verify` and campaign runs do not need it.
# `pnpm install --frozen-lockfile` does not work on these commits (their pnpm-lock.yaml is behind package.json).
# Usage: restore_worktree.sh <worktree name> <branch or commit> <cache name> <GPT plan id> [<Claude plan id>]
# Example: restore_worktree.sh bench-pi-read-tools ee1441b04 wsk12 \
#            c0aebd395e8021313e51635d5dfbf5ea a4119a667964b056713fb2e61af99839
set -euo pipefail
name=${1:?worktree name} ref=${2:?branch or commit} cache=${3:?cache name} gpt=${4:?GPT plan id} claude=${5:-}
repo=/Users/buyong/workspace/private/buyong-mcp
root=/Volumes/Samsung_990_PRO
tree=$root/worktrees/buyong-mcp/$name
plans=$root/$cache/route-preparation/plans
[ -d "$root/worktrees/buyong-mcp" ] || { echo "worktree volume is not mounted: $root" >&2; exit 1; }
[ -f "$plans/$gpt/readiness.json" ] || { echo "missing plan: $plans/$gpt" >&2; exit 1; }
[ -z "$claude" ] || [ -f "$plans/$claude/readiness.json" ] || { echo "missing plan: $plans/$claude" >&2; exit 1; }
want=$(git -C "$repo" rev-parse "$ref^{commit}")
if [ ! -e "$tree" ]; then
  if git -C "$repo" show-ref --verify --quiet "refs/heads/$ref"; then
    git -C "$repo" worktree add "$tree" "$ref"
  else
    git -C "$repo" worktree add --detach "$tree" "$ref"
  fi
fi
have=$(git -C "$tree" rev-parse HEAD)
[ "$have" = "$want" ] || { echo "worktree $tree is at $have, not $want; check out $ref there first" >&2; exit 1; }
if [ ! -e "$tree/node_modules" ]; then
  if grep -q '"version": "8.4.2"' "$repo/node_modules/@inquirer/prompts/package.json" 2>/dev/null; then
    ln -s "$repo/node_modules" "$tree/node_modules"
    echo "node_modules linked to $repo/node_modules"
  else
    echo "the CUI checks (and so prepare) need @inquirer/prompts 8.4.2 under $repo/node_modules" >&2
  fi
fi
evidence=$tree/docs/briefs/evidence/bench-ready
mkdir -p "$evidence"   # the main line does not track this folder
cp "$plans/$gpt/handoffs/"0*.json "$evidence/"
cp "$plans/$gpt/readiness.json" "$evidence/07-readiness.json"
[ -z "$claude" ] || cp "$plans/$claude/readiness.json" "$evidence/07-readiness-claude-opus-5-5.json"
echo "restored $tree at $(git -C "$tree" rev-parse --short=9 HEAD); plans of $cache: gpt $gpt${claude:+ claude $claude}"
