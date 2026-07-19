#!/usr/bin/env python3
"""Hash every run artifact, then remove write bits without deleting failed attempts."""
from __future__ import annotations

import os
import pathlib
import sys

from common import load_json, sha256, write_json


def artifact_entry(run_dir: pathlib.Path, path: pathlib.Path) -> dict[str, int | str]:
    if not path.is_symlink():
        return {"sha256": sha256(path), "bytes": path.stat().st_size}

    target = os.readlink(path)
    if pathlib.Path(target).is_absolute():
        raise SystemExit(f"artifact symlink target must be relative: {path.relative_to(run_dir)}")
    try:
        resolved_target = path.resolve(strict=True)
        resolved_target.relative_to(run_dir)
    except (FileNotFoundError, OSError, RuntimeError, ValueError):
        raise SystemExit(f"artifact symlink must resolve inside the run directory: {path.relative_to(run_dir)}") from None
    if not resolved_target.is_file():
        raise SystemExit(f"artifact symlink target must be a regular file: {path.relative_to(run_dir)}")
    return {
        "sha256": sha256(path),
        "bytes": path.stat().st_size,
        "symlink_target": target,
    }


def artifact_evidence(run_dir: pathlib.Path) -> dict[str, dict[str, int | str]]:
    run_dir = run_dir.resolve()
    manifest_path = run_dir / "artifact-manifest.json"
    artifacts: dict[str, dict[str, int | str]] = {}
    for path in sorted(run_dir.rglob("*")):
        if path == manifest_path:
            if path.is_symlink():
                raise SystemExit("artifact manifest path may not be a symlink")
            continue
        if path.is_symlink():
            artifacts[str(path.relative_to(run_dir))] = artifact_entry(run_dir, path)
        elif path.is_dir():
            continue
        elif path.is_file():
            artifacts[str(path.relative_to(run_dir))] = artifact_entry(run_dir, path)
        else:
            raise SystemExit(f"artifact tree contains a non-regular entry: {path.relative_to(run_dir)}")
    return artifacts


def verify_manifest_contents(run_dir: pathlib.Path, manifest: dict) -> None:
    if set(manifest) != {"schema_version", "artifacts"} or manifest.get("schema_version") != 1 or not isinstance(manifest.get("artifacts"), dict):
        raise SystemExit("existing artifact manifest has an invalid contract")
    if manifest["artifacts"] != artifact_evidence(run_dir):
        raise SystemExit("existing artifact manifest does not match the complete file set")


def seal(run_dir: pathlib.Path) -> dict:
    run_dir = run_dir.resolve()
    manifest_path = run_dir / "artifact-manifest.json"
    if manifest_path.is_file():
        manifest = load_json(manifest_path)
        verify_manifest_contents(run_dir, manifest)
    else:
        manifest = {"schema_version": 1, "artifacts": artifact_evidence(run_dir)}
        write_json(manifest_path, manifest)
    for path in sorted(run_dir.rglob("*"), reverse=True):
        if not path.is_symlink():
            path.chmod(path.stat().st_mode & ~0o222)
    run_dir.chmod(run_dir.stat().st_mode & ~0o222)
    verify_manifest_contents(run_dir, manifest)
    writable = [
        str(path.relative_to(run_dir)) if path != run_dir else "."
        for path in [run_dir, *run_dir.rglob("*")]
        if not path.is_symlink() and path.stat().st_mode & 0o222
    ]
    if writable:
        raise SystemExit(f"artifact seal remains writable: {writable[:3]}")
    if os.environ.get("HARNESS_SYNTHETIC_CRASH_AFTER_ARTIFACT_SEAL_COMMIT") == "1":
        if os.environ.get("HARNESS_SYNTHETIC_TEST") != "1":
            raise SystemExit("artifact seal crash hook is restricted to synthetic tests")
        raise SystemExit("synthetic crash after artifact seal commit")
    return manifest


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: seal_artifacts.py RUN_DIR")
    seal(pathlib.Path(sys.argv[1]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
