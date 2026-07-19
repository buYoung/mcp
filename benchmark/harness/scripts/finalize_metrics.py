#!/usr/bin/env python3
"""Idempotently extract, seal, and ledger-bind one published run's automatic metrics."""
from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import sys
import time
import uuid

import protocol
import seal_artifacts
from common import HARNESS_ROOT, load_json, sha256, write_json


def ledger_metrics_status(ledger_path: pathlib.Path, run_id: str) -> str | None:
    if not ledger_path.is_file():
        return None
    return load_json(ledger_path).get("attempts", {}).get(run_id, {}).get("metrics_status")


def record_failure(
    ledger_path: pathlib.Path,
    generation_path: pathlib.Path,
    run_dir: pathlib.Path,
    reason: str,
    extractor_exit_code: int | None,
    expected_exit_code: int,
    metrics_dir: pathlib.Path,
) -> int:
    run_id = run_dir.name
    failure_dir = run_dir.parent / "automatic-metrics-failures" / run_id
    if failure_dir.exists():
        manifest = failure_dir / "artifact-manifest.json"
        is_sealed = manifest.is_file() and not any(
            path.stat().st_mode & 0o222
            for path in [failure_dir, *failure_dir.rglob("*")]
            if not path.is_symlink()
        )
        if is_sealed:
            protocol.metrics_failure([
                str(ledger_path), str(generation_path), run_id, str(run_dir), str(failure_dir),
            ])
            return 79
        archive_partial(failure_dir)
    if not failure_dir.exists():
        failure_dir.mkdir(parents=True)
        stderr_path = metrics_dir / "extractor.stderr.log"
        if stderr_path.is_file():
            shutil.copy2(stderr_path, failure_dir / "extractor.stderr.log")
        write_json(failure_dir / "failure-record.json", {
            "schema_version": 1,
            "run_id": run_id,
            "generation_id": load_json(generation_path)["generation_id"],
            "reason": reason,
            "extractor_exit_code": extractor_exit_code,
            "expected_exit_code": expected_exit_code,
            "metrics_directory": str(metrics_dir),
            "metrics_manifest_sha256": sha256(metrics_dir / "artifact-manifest.json") if (metrics_dir / "artifact-manifest.json").is_file() else None,
            "recorded_at_ns": time.time_ns(),
            "generation_invalid": True,
        })
        seal_artifacts.seal(failure_dir)
    protocol.metrics_failure([
        str(ledger_path), str(generation_path), run_id, str(run_dir), str(failure_dir),
    ])
    return 79


def archive_partial(metrics_dir: pathlib.Path) -> pathlib.Path:
    archive_root = metrics_dir.parent.parent / "automatic-metrics-interrupted"
    archive_root.mkdir(parents=True, exist_ok=True)
    archive = archive_root / f"{metrics_dir.name}-{uuid.uuid4().hex}"
    shutil.move(str(metrics_dir), str(archive))
    seal_artifacts.seal(archive)
    return archive


def finalize(
    ledger_path: pathlib.Path,
    generation_path: pathlib.Path,
    run_dir: pathlib.Path,
    extractor_path: pathlib.Path,
    schema_path: pathlib.Path,
) -> int:
    run_dir = run_dir.resolve(); run_id = run_dir.name
    status = ledger_metrics_status(ledger_path, run_id)
    if status == "sealed":
        return 0
    if status == "failed":
        return 79
    classification = load_json(run_dir / "attempt-classification.json")
    expected_exit_code = 0 if classification["measurement_status"] == "valid" else 3
    metrics_dir = run_dir.parent / "automatic-metrics" / run_id
    metrics_path = metrics_dir / "automatic-run-metrics.json"

    if metrics_dir.exists():
        manifest_path = metrics_dir / "artifact-manifest.json"
        if manifest_path.is_file() and not any(
            path.stat().st_mode & 0o222
            for path in [metrics_dir, *metrics_dir.rglob("*")]
            if not path.is_symlink()
        ):
            try:
                protocol.metrics([
                    str(ledger_path), str(generation_path), run_id, str(run_dir), str(metrics_dir),
                ])
                return 0
            except (SystemExit, OSError, json.JSONDecodeError) as error:
                return record_failure(
                    ledger_path, generation_path, run_dir,
                    f"sealed automatic metrics could not be ledger-bound: {error}",
                    None, expected_exit_code, metrics_dir,
                )
        archive_partial(metrics_dir)

    metrics_dir.mkdir(parents=True)
    stderr_path = metrics_dir / "extractor.stderr.log"
    environment = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}
    with stderr_path.open("wb") as stderr:
        process = subprocess.run(
            [
                sys.executable, str(extractor_path), str(run_dir),
                "--output", str(metrics_path), "--schema", str(schema_path),
                "--require-aggregation-eligible",
            ],
            stdout=subprocess.PIPE,
            stderr=stderr,
            env=environment,
            check=False,
        )
    extraction_ok = process.returncode == expected_exit_code and metrics_path.is_file()
    seal_artifacts.seal(metrics_dir)
    if not extraction_ok:
        return record_failure(
            ledger_path, generation_path, run_dir,
            "automatic metrics extractor exit/output contract failed",
            process.returncode, expected_exit_code, metrics_dir,
        )
    try:
        protocol.metrics([
            str(ledger_path), str(generation_path), run_id, str(run_dir), str(metrics_dir),
        ])
    except (SystemExit, OSError, json.JSONDecodeError) as error:
        return record_failure(
            ledger_path, generation_path, run_dir,
            f"automatic metrics ledger validation failed: {error}",
            process.returncode, expected_exit_code, metrics_dir,
        )
    return 0


def main() -> int:
    if len(sys.argv) != 6:
        raise SystemExit("usage: finalize_metrics.py LEDGER GENERATION RUN_DIR EXTRACTOR SCHEMA")
    return finalize(*map(pathlib.Path, sys.argv[1:]))


if __name__ == "__main__":
    raise SystemExit(main())
