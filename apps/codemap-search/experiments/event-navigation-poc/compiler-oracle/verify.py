#!/usr/bin/env python3
"""Verify saved observations and run a source case with compiler tools unavailable."""

import argparse
import ast
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

DIRECTORY = Path(__file__).resolve().parent
POC = DIRECTORY.parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_bytes())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DIRECTORY / "results")
    args = parser.parse_args()
    output = args.output
    manifest, build = read(DIRECTORY / "inputs.json"), read(output / "build.json")
    observations = json.loads(gzip.decompress((output / "observations.json.gz").read_bytes()))
    evaluation = read(output / "evaluation.json")
    assert observations["build_sha256"] == digest(output / "build.json")
    assert observations["collector_sha256"] == digest(DIRECTORY / "collect.py")
    assert build["build_script_sha256"] == digest(DIRECTORY / "build.py")
    assert build["inputs_sha256"] == observations["inputs_sha256"] == digest(DIRECTORY / "inputs.json")
    assert hashlib.sha256(gzip.decompress((DIRECTORY / "Cargo.lock.gz").read_bytes())).hexdigest() == build["cargo_lock_sha256"]
    assert evaluation["observations_sha256"] == digest(output / "observations.json.gz")
    assert evaluation["failures"] == [] and evaluation["observed"] == evaluation["probes"] == len(observations["probes"])
    for name, artifact in build["crates"].items():
        for command in ("rustc", "rustdoc"):
            key = command + "_log_sha256"
            if key in artifact:
                assert artifact[key] == digest(output / (name + "." + command + ".txt"))
    for name in ("analyzer_input", "product_dependency", "ast_coverage_credit", "end_to_end_connection_proven", "concrete_instance_proven", "target_programs_executed"):
        assert observations[name] is False
    by_id = {row["id"]: row for row in observations["probes"]}
    assert set(by_id) == {probe["id"] for probe in manifest["probes"] + manifest["trait_probes"]}
    for probe in manifest["probes"]:
        row = by_id[probe["id"]]
        assert row["observed"] and all(any(fragment in item["code"] for item in row["instructions"]) for fragment in probe["required_fragments"])
        assert all(item["span"]["path"] == probe["path"] for item in row["instructions"])
    # Default implementation must not import the optional oracle or its reader.
    implementation = [*POC.glob("*.py"), *(POC / "language-corpus").glob("*.py")]
    forbidden = {"compiler_oracle", "rust_mir", "rust_mir_flow"}
    for path in implementation:
        tree = ast.parse(path.read_bytes())
        for node in ast.walk(tree):
            names = [item.name for item in node.names] if isinstance(node, ast.Import) else [node.module or ""] if isinstance(node, ast.ImportFrom) else []
            assert not any(set(name.split(".")) & forbidden for name in names)
    for path in DIRECTORY.glob("*.py"):
        ast.parse(path.read_bytes())
    # A selected, nontrivial cross-source macro case runs without cargo/rustc.
    with tempfile.TemporaryDirectory(prefix="codemap-no-compiler-") as temporary:
        directory = Path(temporary)
        environment = dict(os.environ, PATH=str(directory / "no-tools"))
        assert shutil.which("rustc", path=environment["PATH"]) is None
        assert shutil.which("cargo", path=environment["PATH"]) is None
        case = "rust-statement-macro-right-to-right"
        command = [sys.executable, str(POC / "language-corpus/regressions/run.py"), "--case", case, "--output", str(directory / "results")]
        subprocess.run(command, env=environment, check=True, capture_output=True, text=True)
        result = read(directory / "results/evaluation.json")
        assert len(result) == 1 and result[0]["passed"] and result[0]["status"] == "conditional_candidate"
        (output / "source-only.evaluation.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    bevy_path = POC / "language-corpus/remaining-routes/results/bevy-modules.evaluation.json"
    bevy = read(bevy_path)
    routes = {probe["route"] for probe in manifest["probes"] if probe["route"] != "pointer_support"}
    statuses = {row["id"]: row["status"] for row in bevy["cases"] if row["id"] in routes}
    assert set(statuses) == routes
    for path in (DIRECTORY / "README.ko.md",):
        for target in re.findall(r"\]\(([^)]+)\)", path.read_text()):
            if "://" not in target and not target.startswith("#"):
                resolved = (path.parent / target.split("#", 1)[0]).resolve()
                if resolved != (output / "verification.json").resolve():
                    assert resolved.exists()
    result = {"compiler_observations_verified": len(by_id), "compiler_tools_available_in_source_check": False,
              "source_check_case": case, "source_check_passed": True, "source_check_sha256": digest(output / "source-only.evaluation.json"),
              "default_implementation_sha256": {str(path.relative_to(POC)): digest(path) for path in implementation},
              "compiler_oracle_sha256": {path.name: digest(path) for path in DIRECTORY.glob("*.py")},
              "observations_sha256": digest(output / "observations.json.gz"), "build_sha256": digest(output / "build.json"),
              "bevy_ast_evaluation_sha256": digest(bevy_path), "bevy_ast_statuses_unchanged_by_oracle": statuses,
              "product_dependency": False, "analyzer_input": False, "ast_coverage_credit": False,
              "end_to_end_connection_proven": False, "target_programs_executed": False}
    (output / "verification.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({key: value for key, value in result.items() if not key.endswith("sha256")}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
