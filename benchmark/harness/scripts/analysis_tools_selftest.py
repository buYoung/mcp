#!/usr/bin/env python3
"""Run and bind the model-free extractor and aggregator unit suites."""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys

from common import HARNESS_ROOT, QUALITY_ROOT, sha256, write_json
from selftest_contract import AGGREGATOR_REQUIRED_TESTS, EXTRACTOR_REQUIRED_TESTS


def run_suite(pattern: str, required_tests: frozenset[str]) -> dict:
    command = [
        sys.executable, "-m", "unittest", "discover",
        "-s", str(QUALITY_ROOT / "analysis-tools/tests"), "-p", pattern, "-v",
    ]
    process = subprocess.run(
        command, cwd=QUALITY_ROOT, env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, text=True,
    )
    combined = process.stdout + process.stderr
    match = re.search(r"Ran (\d+) tests?", combined)
    result_lines = re.findall(r"^(test_[A-Za-z0-9_]+) \(.*\) \.\.\. (.+)$", combined, re.MULTILINE)
    executed = {name for name, _status in result_lines}
    passed = {name for name, status in result_lines if status.strip() == "ok"}
    return {
        "command": command,
        "returncode": process.returncode,
        "test_count": int(match.group(1)) if match else None,
        "required_test_names": sorted(required_tests),
        "executed_test_names": sorted(executed),
        "passed_test_names": sorted(passed),
        "missing_required_test_names": sorted(required_tests - passed),
        "output_sha256": hashlib.sha256(combined.encode("utf-8")).hexdigest(),
        "passed_marker": "OK" in combined,
    }


def main() -> int:
    extractor = run_suite("test_extract_run_metrics.py", EXTRACTOR_REQUIRED_TESTS)
    aggregator = run_suite("test_aggregate_baseline_metrics.py", AGGREGATOR_REQUIRED_TESTS)
    checks = {
        "extractor-required-test-names-all-pass": (
            extractor["returncode"] == 0
            and extractor["passed_marker"]
            and extractor["missing_required_test_names"] == []
            and extractor["test_count"] == len(extractor["executed_test_names"])
        ),
        "aggregator-required-test-names-all-pass": (
            aggregator["returncode"] == 0
            and aggregator["passed_marker"]
            and aggregator["missing_required_test_names"] == []
            and aggregator["test_count"] == len(aggregator["executed_test_names"])
        ),
    }
    tested_paths = sorted({
        *[path for path in (QUALITY_ROOT / "analysis-tools").rglob("*") if path.is_file() and "__pycache__" not in path.parts and path.suffix != ".pyc"],
        HARNESS_ROOT / "schemas/automatic-run-metrics.schema.json",
        HARNESS_ROOT / "schemas/analysis-input-seal.schema.json",
        HARNESS_ROOT / "scripts/analysis_tools_selftest.py",
        HARNESS_ROOT / "scripts/selftest_contract.py",
    })
    report = {
        "schema_version": 1,
        "external_model_calls": 0,
        "builds": 0,
        "indexing_operations": 0,
        "suites": {"extractor": extractor, "aggregator": aggregator},
        "tested_file_sha256": {
            os.path.relpath(path, HARNESS_ROOT): sha256(path) for path in tested_paths
        },
        "required_check_names": sorted(checks),
        "checks": checks,
        "passed": all(checks.values()),
    }
    output = HARNESS_ROOT / "reports/analysis-tools-selftest.json"
    write_json(output, report)
    print(json.dumps({"passed": report["passed"], "checks": checks, "suites": report["suites"], "report": str(output)}, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
