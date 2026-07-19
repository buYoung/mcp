#!/usr/bin/env python3
"""Resolve the run directory across the rename-to-shell-assignment signal window."""
from __future__ import annotations

import os
import pathlib
import sys


def recover(source: pathlib.Path, target: pathlib.Path) -> pathlib.Path:
    source = source.absolute()
    target = target.absolute()
    if source == target and target.is_dir():
        return target
    if source.is_dir() and not target.exists():
        target.parent.mkdir(parents=True, exist_ok=True)
        source_parent = source.parent
        source.rename(target)
        for parent in {source_parent, target.parent}:
            try:
                descriptor = os.open(parent, os.O_RDONLY)
                try:
                    os.fsync(descriptor)
                finally:
                    os.close(descriptor)
            except OSError:
                pass
        return target
    if not source.exists() and target.is_dir():
        return target
    raise SystemExit("run directory rename state is ambiguous or invalid")


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: recover_run_directory.py SOURCE TARGET")
    print(recover(pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
