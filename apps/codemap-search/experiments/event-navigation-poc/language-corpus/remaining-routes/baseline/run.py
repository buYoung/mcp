#!/usr/bin/env python3
"""Run fixed-revision, isolated static measurements; never execute target code."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--phase", choices=["development", "evaluation", "all"], required=True)
    parser.add_argument("--repo", action="append")
    parser.add_argument("--passes", type=int, choices=range(1, 5))
    parser.add_argument("--compress", action="store_true")
    parser.add_argument("--implementation", type=Path)
    options = parser.parse_args()
    directory = Path(__file__).resolve().parent
    corpus = json.loads((directory / "corpus.json").read_text())
    rows = []
    for name, spec in corpus.items():
        if options.repo and name not in options.repo:
            continue
        root = options.sources.resolve() / name
        revision = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
        if revision != spec["revision"]:
            raise SystemExit(f"{name}: revision mismatch: expected {spec['revision']}, got {revision}")
        dirty = subprocess.check_output(["git", "-C", str(root), "status", "--porcelain", "--untracked-files=no"], text=True)
        if dirty:
            raise SystemExit(f"{name}: tracked sources have local changes")
        paths = spec["development"] + spec["evaluation"] if options.phase == "all" else spec[options.phase]
        output = options.output.resolve() / (name + (".json.gz" if options.compress else ".json"))
        implementation = options.implementation.resolve() if options.implementation else directory / "analyze.py"
        command = [sys.executable, "-X", "faulthandler", str(implementation), "--root", str(root), "--output", str(output)]
        for path in paths:
            command.extend(["--path", path])
        if options.passes is not None:
            command.extend(["--passes", str(options.passes)])
        started = time.perf_counter()
        completed = subprocess.run(command, capture_output=True, text=True)
        wall_seconds = time.perf_counter() - started
        if completed.stderr:
            print(completed.stderr, file=sys.stderr, end="")
        if completed.returncode:
            raise SystemExit(f"{name}: analyzer exited with status {completed.returncode}")
        row = json.loads(completed.stdout)
        row["repository"] = name
        row["subprocess_wall_seconds"] = wall_seconds
        rows.append(row)
        print(json.dumps(row, ensure_ascii=False), flush=True)
    options.output.mkdir(parents=True, exist_ok=True)
    (options.output / "metrics.json").write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
