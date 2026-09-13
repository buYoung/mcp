#!/usr/bin/env python3
"""Replay navigation cases against a new binary and unchanged qualified worktrees."""

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import shutil

from public_validation import (
    APP, DATA, Checks, McpClient, command, load_specs, result_has_issues,
    run_regressions, save_json, sha256, validate_test_configuration,
)


def recheck_probes(checks, language):
    is_rust = language == "Rust"
    path = f"_codemap_validation_probe/probe.{'rs' if is_rust else 'go'}"
    names = (
        ["cm_validation_target", "cm_validation_caller", "cm_validation_imported_constant", "cm_validation_test", "cm_validation_custom_test"]
        if is_rust else
        ["CmValidationTarget", "CmValidationCaller", "CmValidationImportedConstant", "TestCmValidation", "CmValidationCustomTest"]
    )
    source = (checks.root / path).read_text().splitlines()
    def line_of(name):
        return next(line for line, text in enumerate(source, 1) if re.search(r"(?:fn|func) " + name + r"\(", text))
    target, caller, imported, test, custom = names
    target_line, caller_line, imported_line = (line_of(name) for name in names[:3])
    checks.query("probe:callers-and-constant", "read", {"file_path": path, "offset": target_line, "limit": 1},
        contains=[f"{caller} ({path}:{caller_line})", f"CM_VALIDATION_LIMIT — {path}:{1 if is_rust else 2}"],
        excludes=[f"{test} ("], category="callers_and_references", section=(path, target_line))
    checks.query("probe:callee", "read", {"file_path": path, "offset": caller_line, "limit": 1},
        contains=[f"{target} — {path}:{target_line}"], category="callees", section=(path, caller_line))
    checks.query("probe:imported-constant", "read", {"file_path": path, "offset": imported_line, "limit": 1},
        contains=[f"CM_VALIDATION_IMPORTED — _codemap_validation_probe/constants.{Path(path).suffix[1:]}:{1 if is_rust else 2}"],
        category="reference_coverage", section=(path, imported_line))
    validate_test_configuration(checks, {"file": path, "target_line": target_line, "test": test, "custom_test": custom}, language)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--previous-run", required=True)
    parser.add_argument("--run-id", required=True)
    args = parser.parse_args()
    for name in (args.previous_run, args.run_id):
        if not re.fullmatch(r"[A-Za-z0-9_-]+", name):
            parser.error("run IDs must contain only letters, digits, underscores and hyphens")
    args.cache, args.binary = args.cache.expanduser().resolve(), args.binary.expanduser().resolve()
    args.profile = None
    previous = args.cache / "runs" / args.previous_run / "summary.json"
    baseline = json.loads(previous.read_text())
    previous_specs = json.loads((previous.parent / "repositories.json").read_text())["repositories"]
    specs = {spec["name"]: spec for spec in load_specs(None)}
    for spec in previous_specs:
        if any(spec[key] != specs[spec["name"]][key] for key in ("sha", "url", "language", "files")):
            raise RuntimeError("Repository qualification or sample files differ from the previous run")
    output = args.cache / "runs" / args.run_id
    output.mkdir(parents=True, exist_ok=False)
    shutil.copy(Path(__file__), output / "recheck.py")
    shutil.copy(DATA / "repositories.json", output / "repositories.json")
    metadata = {"started_utc": datetime.now(timezone.utc).isoformat(), "binary_sha256": sha256(args.binary),
        "binary": str(args.binary), "previous_run": args.previous_run, "previous_summary_sha256": sha256(previous),
        "runner_sha256": sha256(Path(__file__)), "source_commit": command(["git", "rev-parse", "HEAD"], APP),
        "manifest_sha256": sha256(DATA / "repositories.json"), "previous_manifest_sha256": baseline["manifest_sha256"],
        "case_changes": [{"repository": spec["name"], "before": spec["cases"], "after": specs[spec["name"]]["cases"]} for spec in previous_specs if spec["cases"] != specs[spec["name"]]["cases"]], "results": []}
    for result in baseline["results"]:
        spec = specs[result["repository"]]
        root = Path(result["worktree"]).resolve()
        if not root.is_relative_to(args.cache) or command(["git", "rev-parse", "HEAD"], root) != spec["sha"]:
            raise RuntimeError(f"Unexpected worktree or revision: {root}")
        if command(["git", "status", "--porcelain", "--untracked-files=no"], root):
            raise RuntimeError(f"Tracked source changed: {root}")
        if sha256(root / ".codemap/config.toml") != result["config_sha256"]:
            raise RuntimeError(f"Configuration changed: {root}")
        label = f"{spec['name']}-{result['profile']}"
        with McpClient(args.binary, root, output / label) as client:
            client.ready(spec["files"][0], timeout=600)
            checks = Checks(client, root)
            for case in spec["cases"]:
                checks.case(case)
            recheck_probes(checks, spec["language"])
        record = {"repository": spec["name"], "sha": spec["sha"], "profile": result["profile"], "worktree": str(root),
            "config_sha256": result["config_sha256"], "checks": checks.results,
            "tracked_changes": command(["git", "status", "--porcelain", "--untracked-files=no"], root)}
        record["counts"] = {status: sum(check["status"] == status for check in checks.results) for status in ("pass", "fail", "unverified")}
        metadata["results"].append(record)
        save_json(output / label / "results.json", record)
        save_json(output / "summary.json", metadata)
        print(label, record["counts"], flush=True)
    metadata["regressions"] = run_regressions(args, output)
    metadata["finished_utc"] = datetime.now(timezone.utc).isoformat()
    save_json(output / "summary.json", metadata)
    print(output / "summary.json", flush=True)
    return int(any(result_has_issues(result) for result in metadata["results"] + metadata["regressions"]))


if __name__ == "__main__":
    raise SystemExit(main())
