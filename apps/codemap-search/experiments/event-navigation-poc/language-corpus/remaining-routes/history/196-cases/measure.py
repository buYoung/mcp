#!/usr/bin/env python3
"""Freeze, run both modes sequentially, then evaluate the saved results together."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import sys

from evaluate import load_result


def fingerprint(directory):
    paths = sorted(directory.glob("*.py")) + [directory / name for name in ("corpus.json", "cases.json", "requirements.txt")]
    return {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in paths}


def check_initial_snapshot(directory):
    initial = directory / "history/initial"
    frozen = json.loads((initial / "results/freeze.json").read_text())["sha256"]
    for name, digest in frozen.items():
        if hashlib.sha256((initial / name).read_bytes()).hexdigest() != digest:
            raise SystemExit(f"Historical implementation changed: {name}")
    return frozen


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    directory = Path(__file__).resolve().parent
    options.output.mkdir(parents=True, exist_ok=True)
    frozen = fingerprint(directory)
    initial_frozen = check_initial_snapshot(directory)
    (options.output / "freeze.json").write_text(json.dumps({"started_at_utc": datetime.now(timezone.utc).isoformat(),
                                                           "sha256": frozen, "initial_sha256": initial_frozen}, indent=2) + "\n")
    modes = (("baseline", 4), ("local", 1), ("summaries", 4))
    for mode, passes in modes:
        command = [sys.executable, str(directory / "run.py"), "--sources", str(options.sources),
                   "--output", str(options.output / mode), "--phase", "all", "--passes", str(passes), "--compress"]
        if mode == "baseline":
            command.extend(["--implementation", str(directory / "history/initial/analyze.py")])
        subprocess.run(command, check=True)
    if fingerprint(directory) != frozen:
        raise SystemExit("Implementation or labels changed during measurement")
    check_initial_snapshot(directory)
    corpus = json.loads((directory / "corpus.json").read_text())
    for name in corpus:
        local = load_result(options.output / "local", name)
        summaries = load_result(options.output / "summaries", name)
        baseline = load_result(options.output / "baseline", name)
        if local["scope"] != summaries["scope"] or local["revision"] != summaries["revision"]:
            raise SystemExit(f"{name}: comparison input mismatch")
        if baseline["scope"] != summaries["scope"] or baseline["revision"] != summaries["revision"]:
            raise SystemExit(f"{name}: historical implementation input mismatch")
        if local["implementation_sha256"] != summaries["implementation_sha256"]:
            raise SystemExit(f"{name}: comparison implementation mismatch")
    # Scoring happens only after every repository has completed both modes.
    for mode, _ in modes:
        subprocess.run([sys.executable, str(directory / "evaluate.py"), "--results", str(options.output / mode),
                        "--output", str(options.output / (mode + "-evaluation.json"))], check=True)
    print("Completed three modes: identical sources, frozen old/new implementations, then joint evaluation.")


if __name__ == "__main__":
    main()
