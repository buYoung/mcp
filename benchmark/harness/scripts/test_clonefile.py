#!/usr/bin/env python3
"""Synthetic proof that the required copy-on-write path preserves bytes and metadata isolation."""
from __future__ import annotations

import json
import pathlib
import shutil
import stat
import subprocess
import sys
import uuid

from common import HARNESS_ROOT, sha256, tree_digest, write_json


def main() -> int:
    root = HARNESS_ROOT / "synthetic" / f"clonefile-{uuid.uuid4().hex}"
    source = root / "source"
    clone = root / "clone"
    source.mkdir(parents=True)
    (source / "nested").mkdir()
    (source / "nested/data.bin").write_bytes(bytes(range(256)) * 16)
    (source / "plain.txt").write_text("baseline-clonefile-fixture\n", encoding="utf-8")
    source.chmod(0o555)
    (source / "nested").chmod(0o555)
    for path in source.rglob("*"):
        if path.is_file():
            path.chmod(0o444)
    before_digest = tree_digest(source)
    before_mode = stat.S_IMODE(source.stat().st_mode)
    process = subprocess.run(
        ["/bin/cp", "-cRp", str(source), str(clone)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    checks = {
        "cp_clonefile_exit_zero": process.returncode == 0,
        "tree_digest_equal": clone.is_dir() and tree_digest(clone) == before_digest,
        "root_mode_preserved": clone.is_dir() and stat.S_IMODE(clone.stat().st_mode) == before_mode,
        "different_inodes": clone.is_dir() and (source / "plain.txt").stat().st_ino != (clone / "plain.txt").stat().st_ino,
    }
    if clone.is_dir():
        clone.chmod(0o755)
        cloned_file = clone / "plain.txt"
        cloned_file.chmod(0o644)
        cloned_file.write_text("changed clone only\n", encoding="utf-8")
        checks["clone_content_mutation_isolated"] = (source / "plain.txt").read_text(encoding="utf-8") == "baseline-clonefile-fixture\n"
        checks["clone_mode_mutation_isolated"] = stat.S_IMODE((source / "plain.txt").stat().st_mode) == 0o444
    else:
        checks["clone_content_mutation_isolated"] = False
        checks["clone_mode_mutation_isolated"] = False
    report = {
        "schema_version": 1,
        "external_model_calls": 0,
        "builds": 0,
        "indexing_operations": 0,
        "command": ["/bin/cp", "-cRp"],
        "stderr": process.stderr,
        "checks": checks,
        "passed": all(checks.values()),
    }
    output = HARNESS_ROOT / "reports/clonefile-synthetic.json"
    write_json(output, report)
    shutil.rmtree(root, ignore_errors=True)
    print(json.dumps({"passed": report["passed"], "report": str(output)}, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
