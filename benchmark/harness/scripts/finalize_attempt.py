#!/usr/bin/env python3
"""Post-run invariant evidence and immutable artifact manifest."""
from __future__ import annotations

import json
import pathlib
import sys

from common import SOURCE_TREE_SHA256, load_json, sha256, tree_digest, write_json
from generation import verify as verify_generation


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: finalize_attempt.py RUN_DIR GENERATION")
    run_dir, generation = pathlib.Path(sys.argv[1]).resolve(), pathlib.Path(sys.argv[2]).resolve()
    try:
        verify_generation(generation)
        generation_current = True
        generation_error = None
    except (SystemExit, OSError, json.JSONDecodeError) as error:
        generation_current = False
        generation_error = str(error)
    source_after = tree_digest(run_dir / "source")
    before = load_json(run_dir / "index.before.json")
    after = load_json(run_dir / "index.after.json")
    invariants = {
        "generation_current": generation_current,
        "generation_error": generation_error,
        "source_before_sha256": SOURCE_TREE_SHA256,
        "source_after_sha256": source_after,
        "source_unchanged": source_after == SOURCE_TREE_SHA256,
        "index_unchanged": before == after,
    }
    write_json(run_dir / "invariants.after.json", invariants)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

