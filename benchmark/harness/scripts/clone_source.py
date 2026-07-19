#!/usr/bin/env python3
"""Create one run-local APFS copy-on-write source/index clone without fallback."""
from __future__ import annotations

import json
import pathlib
import shutil
import stat
import subprocess
import sys

from common import GOLDEN_CODEMAP, GOLDEN_TREE_SHA256, SOURCE_TREE_SHA256, tree_digest, write_json


def owner_writable(root: pathlib.Path) -> None:
    for path in [root, *root.rglob("*")]:
        if path.is_symlink():
            continue
        path.chmod(path.stat().st_mode | stat.S_IWUSR)


def read_only(root: pathlib.Path) -> None:
    for path in [root, *root.rglob("*")]:
        if path.is_symlink():
            continue
        path.chmod(path.stat().st_mode & ~0o222)


def clone(source: pathlib.Path, destination: pathlib.Path) -> dict:
    if destination.exists():
        raise SystemExit(f"destination already exists: {destination}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    process = subprocess.run(
        ["/bin/cp", "-cRp", str(source), str(destination)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if process.returncode != 0:
        raise SystemExit(f"APFS clonefile copy failed; no byte-copy fallback is allowed: {process.stderr.strip()}")
    source_clone_digest = tree_digest(destination)
    if source_clone_digest != SOURCE_TREE_SHA256:
        raise SystemExit("copy-on-write source clone content digest mismatch")

    existing = destination / ".codemap"
    owner_writable(destination)
    if existing.exists():
        owner_writable(existing)
        shutil.rmtree(existing)
    index_process = subprocess.run(
        ["/bin/cp", "-cRp", str(GOLDEN_CODEMAP), str(existing)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if index_process.returncode != 0:
        raise SystemExit(f"APFS clonefile index copy failed; no byte-copy fallback is allowed: {index_process.stderr.strip()}")
    if tree_digest(existing) != GOLDEN_TREE_SHA256:
        raise SystemExit("copy-on-write golden index clone content digest mismatch")
    read_only(destination)
    if any(path.stat().st_mode & 0o222 for path in [destination, *destination.rglob("*")] if not path.is_symlink()):
        raise SystemExit("run source is not completely read-only")
    return {
        "copy_command": ["/bin/cp", "-cRp"],
        "fallback_used": False,
        "source_tree_sha256_before_index_replacement": source_clone_digest,
        "working_index_tree_sha256": tree_digest(existing),
        "source_read_only": True,
    }


def main() -> int:
    if len(sys.argv) != 4:
        raise SystemExit("usage: clone_source.py SOURCE DESTINATION REPORT")
    report = clone(pathlib.Path(sys.argv[1]).resolve(), pathlib.Path(sys.argv[2]).resolve())
    write_json(pathlib.Path(sys.argv[3]), report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

