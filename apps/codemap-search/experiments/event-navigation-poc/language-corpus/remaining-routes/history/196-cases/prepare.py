#!/usr/bin/env python3
"""Fetch the pinned public sources into a separate directory; run no target code."""

import argparse
import json
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", required=True, type=Path)
    options = parser.parse_args()
    options.sources.mkdir(parents=True, exist_ok=True)
    corpus = json.loads((Path(__file__).parent / "corpus.json").read_text())
    for name, spec in corpus.items():
        target = options.sources.resolve() / name
        if target.exists():
            revision = subprocess.check_output(["git", "-C", str(target), "rev-parse", "HEAD"], text=True).strip()
            if revision != spec["revision"]:
                raise SystemExit(f"Refusing to replace existing {target}: expected {spec['revision']}, got {revision}")
        else:
            with tempfile.TemporaryDirectory(prefix=f".{name}-fetch-", dir=options.sources) as temporary:
                checkout = Path(temporary) / "checkout"
                subprocess.run(["git", "init", "--quiet", str(checkout)], check=True)
                subprocess.run(["git", "-C", str(checkout), "remote", "add", "origin", spec["url"]], check=True)
                subprocess.run(["git", "-C", str(checkout), "fetch", "--depth", "1", "--quiet", "origin", spec["revision"]], check=True)
                subprocess.run(["git", "-C", str(checkout), "checkout", "--quiet", "--detach", "FETCH_HEAD"], check=True)
                checkout.rename(target)
        print(f"{name}: {spec['revision']}", flush=True)


if __name__ == "__main__":
    main()
