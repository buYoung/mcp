#!/usr/bin/env python3
"""Hash a .codemap tree including directory metadata for mutation evidence."""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import sys


def digest(path: pathlib.Path) -> str | None:
    if not path.is_file():
        return None
    hasher = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: index_manifest.py TREE OUTPUT")
    tree = pathlib.Path(sys.argv[1]).resolve()
    rows = []
    for directory, names, files in os.walk(tree):
        current = pathlib.Path(directory)
        for name in sorted(names + files):
            path = current / name
            info = path.stat()
            rows.append({
                "path": str(path.relative_to(tree)),
                "kind": "directory" if path.is_dir() else "file",
                "mode": oct(info.st_mode & 0o777),
                "size": info.st_size,
                "mtime_ns": info.st_mtime_ns,
                "sha256": digest(path),
            })
    pathlib.Path(sys.argv[2]).write_text(json.dumps(rows, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
