#!/usr/bin/env python3
"""Index the pinned target workspaces and record index-time statistics evidence (offline).

    python3 workspaces.py check            # pinned commit + clean tree for every checkout
    python3 workspaces.py index [NAME...]  # rtpoc index + verify-stats + population check
                                           # -> results/index-stats.json

Checkouts are cloned once with each target's `clone_command` (network) into the ignored
checkout root. Each target is indexed from its own cwd with a private CODEMAP_HOME.
"""

from __future__ import annotations

import argparse
import collections
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

from rtpoc_common import DATA_ROOT, EXPERIMENT_DIR, REPO_ROOT, RESULTS_DIR, read_json, repo_relative, sha256_file, write_json

MANIFEST_PATH = EXPERIMENT_DIR / "workspaces.json"
BINARY = EXPERIMENT_DIR / "target" / "release" / "rtpoc"
INDEX_STATS_PATH = RESULTS_DIR / "index-stats.json"
RUNS_DIR = DATA_ROOT / "workspaces" / "runs"
PRIMARY_EXTENSIONS = {
    "Python": {".py"},
    "Go": {".go"},
    "Rust": {".rs"},
    "JavaScript": {".js", ".mjs", ".cjs"},
}


def manifest() -> dict:
    return read_json(MANIFEST_PATH)


def checkout_dir(target: dict) -> Path:
    return REPO_ROOT / manifest()["checkout_root"] / target["name"]


def state_dir(target: dict) -> Path:
    return REPO_ROOT / manifest()["state_root"] / target["name"]


def git(checkout: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=checkout, check=True, capture_output=True, text=True).stdout


def check_target(target: dict) -> dict:
    checkout = checkout_dir(target)
    head = git(checkout, "rev-parse", "HEAD").strip()
    dirty = git(checkout, "status", "--porcelain").strip()
    return {"head": head, "matches_pin": head == target["commit"], "clean": not dirty}


def run_measured(command: list[str], cwd: Path, env: dict) -> tuple[subprocess.CompletedProcess, float, int | None]:
    """Run under /usr/bin/time -l; returns the process, wall seconds, and max RSS bytes."""
    started = time.perf_counter()
    process = subprocess.run(["/usr/bin/time", "-l", *command], cwd=cwd, env=env, capture_output=True, text=True)
    wall = time.perf_counter() - started
    match = re.search(r"(\d+)\s+maximum resident set size", process.stderr)
    return process, wall, int(match.group(1)) if match else None


def population(target: dict, indexed: list[str], published: dict) -> dict:
    checkout = checkout_dir(target)
    tracked = git(checkout, "ls-files").splitlines()
    tracked_set = set(tracked)
    indexed_set = set(indexed)
    primary = PRIMARY_EXTENSIONS[target["primary_language"]]
    tracked_primary = [path for path in tracked if Path(path).suffix in primary]
    missing_primary = sorted(path for path in tracked_primary if path not in indexed_set)
    max_bytes = published["workspace"]["effective_config"]["max_file_size"]
    missing_reasons = collections.Counter()
    for path in missing_primary:
        size = (checkout / path).stat().st_size if (checkout / path).exists() else None
        parts = set(Path(path).parts)
        if size is not None and size > max_bytes:
            missing_reasons["over_max_file_size"] += 1
        elif parts & {"vendor", "build", "dist", "target", "node_modules"}:
            missing_reasons["excluded_directory"] += 1
        else:
            missing_reasons["other"] += 1
    return {
        "tracked_files": len(tracked),
        "indexed_files": len(indexed),
        "indexed_not_tracked": len(indexed_set - tracked_set),
        "primary_extensions": sorted(primary),
        "tracked_primary_files": len(tracked_primary),
        "indexed_primary_files": len(tracked_primary) - len(missing_primary),
        "missing_primary_reasons": dict(missing_reasons),
        "missing_primary_examples": missing_primary[:10],
        "indexed_by_extension": dict(
            collections.Counter(Path(path).suffix or Path(path).name for path in indexed).most_common()
        ),
    }


def index_target(target: dict) -> dict:
    checkout = checkout_dir(target)
    state = state_dir(target)
    home = state / "codemap-home"
    home.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "CODEMAP_HOME": str(home)}
    pin = check_target(target)
    if not (pin["matches_pin"] and pin["clean"]):
        raise SystemExit(f"{target['name']}: checkout is not the clean pinned commit: {pin}")
    if (checkout / ".codemap").exists():
        raise SystemExit(f"{target['name']}: unexpected repository .codemap/ (would change effective config)")

    command = [str(BINARY), "index", "--state", str(state)]
    process, wall, rss = run_measured(command, checkout, env)
    if process.returncode != 0:
        raise SystemExit(f"{target['name']}: index failed\n{process.stderr}")
    published = json.loads(process.stdout)
    verify, verify_wall, verify_rss = run_measured([str(BINARY), "verify-stats", "--state", str(state)], checkout, env)
    verification = json.loads(verify.stdout)
    indexed = json.loads((state / "indexed-files.json").read_text(encoding="utf-8"))
    RUNS_DIR.mkdir(parents=True, exist_ok=True)
    (RUNS_DIR / f"{target['name']}-index.json").write_text(
        json.dumps({"published": published, "verify": verification, "stderr": process.stderr}, indent=1),
        encoding="utf-8",
    )
    finalization = published["finalization"]
    return {
        "commit": target["commit"],
        "pin_check": pin,
        "checkout": repo_relative(checkout),
        "state": repo_relative(state),
        "commands": [
            {"cwd": repo_relative(checkout), "env": {"CODEMAP_HOME": repo_relative(home)},
             "command": f"{repo_relative(BINARY)} index --state {repo_relative(state)}", "exit": process.returncode},
            {"cwd": repo_relative(checkout), "command": f"{repo_relative(BINARY)} verify-stats --state {repo_relative(state)}",
             "exit": verify.returncode},
        ],
        "published": {
            "index_generation": published["index_generation"],
            "index_opstamp": published["index_opstamp"],
            "live_files": published["live_files"],
            "vocabulary_size": published["vocabulary_size"],
            "stats_file": published["stats_file"],
            "stats_sha256": published["stats_sha256"],
            "published_json_sha256": sha256_file(state / "published.json"),
        },
        "effective_config": published["workspace"]["effective_config"],
        "costs": {
            "index_command_wall_s": round(wall, 3),
            "index_command_max_rss_bytes": rss,
            "indexing_ms": round(finalization["indexing_ms"], 1),
            "vocabulary_materialize_ms": round(finalization["materialize_ms"], 1),
            "stats_persist_ms": round(finalization["persist_ms"], 1),
            "stats_bytes": finalization["stats_bytes"],
            "per_field_live_postings": finalization["per_field_live_postings"],
            "verify_wall_s": round(verify_wall, 3),
            "verify_max_rss_bytes": verify_rss,
        },
        "verification": {key: value for key, value in verification.items() if not key.endswith("examples")},
        "population": population(target, indexed, published),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("check")
    index = sub.add_parser("index")
    index.add_argument("names", nargs="*")
    args = parser.parse_args()
    targets = manifest()["targets"]
    if args.command == "check":
        results = {target["name"]: check_target(target) for target in targets}
        print(json.dumps(results, indent=2))
        return 0 if all(r["matches_pin"] and r["clean"] for r in results.values()) else 1
    chosen = [target for target in targets if not args.names or target["name"] in args.names]
    existing = read_json(INDEX_STATS_PATH) if INDEX_STATS_PATH.exists() else {"targets": {}}
    for target in chosen:
        existing["targets"][target["name"]] = index_target(target)
        print(f"{target['name']}: {existing['targets'][target['name']]['verification']['status']}")
    write_json(
        INDEX_STATS_PATH,
        {
            "workspace_manifest_path": repo_relative(MANIFEST_PATH),
            "workspace_manifest_sha256": sha256_file(MANIFEST_PATH),
            "binary": {"path": repo_relative(BINARY), "sha256": sha256_file(BINARY), "profile": "release"},
            "stats_policies": {
                "stats_format": "rt-stats/1",
                "field_policy": "rts-fields-1:file_path_parts+symbol+docstring+literal+definition_body+reference+format_text",
                "analyzer_policy": "tantivy-0.26.2/default:simple+remove_long(40)+lowercase",
                "count": "unique live indexed files per analyzed word across all searchable fields",
            },
            "targets": dict(sorted(existing["targets"].items())),
        },
    )
    return 0 if all(t["verification"]["status"] == "pass" for t in existing["targets"].values()) else 1


if __name__ == "__main__":
    sys.exit(main())
