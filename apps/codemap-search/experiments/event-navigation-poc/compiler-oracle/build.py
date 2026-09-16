#!/usr/bin/env python3
"""Build optional, pinned library-only compiler observations in a separate worktree.

This command is never imported or invoked by the source analyzer or the product.
Build scripts and proc macros run as compiler inputs; target apps do not run.
"""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

DIRECTORY = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=DIRECTORY / "results")
    args = parser.parse_args()
    root, output = args.root.resolve(), args.output.resolve()
    manifest = json.loads((DIRECTORY / "inputs.json").read_text())
    if root == DIRECTORY or DIRECTORY.is_relative_to(root):
        raise ValueError("use an isolated target-repository worktree")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    if revision != manifest["source_revision"]:
        raise ValueError("source revision mismatch")
    subprocess.run(["git", "diff", "--quiet", "HEAD", "--"], cwd=root, check=True)
    compiler = subprocess.check_output(["rustc", "--version", "--verbose"], text=True).strip()
    if compiler != manifest["compiler"]:
        raise ValueError("compiler version mismatch: MIR is an unstable interface")
    lock = gzip.decompress((DIRECTORY / "Cargo.lock.gz").read_bytes())
    if hashlib.sha256(lock).hexdigest() != manifest["cargo_lock_sha256"]:
        raise ValueError("preserved Cargo.lock mismatch")
    if not (root / "Cargo.lock").exists():
        (root / "Cargo.lock").write_bytes(lock)
    if digest(root / "Cargo.lock") != manifest["cargo_lock_sha256"]:
        raise ValueError("worktree Cargo.lock differs; it was not overwritten")
    output.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, RUSTC_BOOTSTRAP="1")
    # User-local flags cannot silently change the recorded configuration.
    for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET"):
        environment.pop(key, None)
    results = {}
    for crate, spec in manifest["crates"].items():
        common = ["--locked", "-p", crate, "--lib", *spec["cargo_options"]]
        command = ["cargo", "rustc", "--message-format=json", *common, "--", "--emit=mir", "-Zmir-include-spans=yes"]
        print(json.dumps({"cwd": str(root), "command": command}), flush=True)
        process = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True)
        (output / (crate + ".rustc.txt")).write_text(process.stderr)
        if process.returncode:
            print(process.stderr, file=sys.stderr)
            raise subprocess.CalledProcessError(process.returncode, command)
        artifacts = [json.loads(line) for line in process.stdout.splitlines() if line.startswith("{")]
        artifacts = [item for item in artifacts if item.get("reason") == "compiler-artifact" and item["target"]["name"] == crate]
        if len(artifacts) != 1:
            raise ValueError("ambiguous compiler artifact")
        artifact = artifacts[0]
        mirrors = {str(Path(name).with_name(Path(name).stem.removeprefix("lib") + ".mir"))
                   for name in artifact["filenames"] if Path(name).suffix in {".rlib", ".rmeta"}}
        mirrors = [Path(path) for path in mirrors if Path(path).is_file()]
        if len(mirrors) != 1:
            raise ValueError("MIR artifact was not identified from cargo output")
        record = {"commands": [command], "features": artifact["features"], "mir": str(mirrors[0].relative_to(root)),
                  "mir_sha256": digest(mirrors[0]), "rustc_log_sha256": digest(output / (crate + ".rustc.txt"))}
        if spec["rustdoc"]:
            command = ["cargo", "rustdoc", *common, "--", "-Z", "unstable-options", "--output-format", "json", "--document-private-items"]
            print(json.dumps({"cwd": str(root), "command": command}), flush=True)
            process = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True)
            (output / (crate + ".rustdoc.txt")).write_text(process.stdout + process.stderr)
            process.check_returncode()
            metadata = root / "target/doc" / (crate + ".json")
            record.update(rustdoc=str(metadata.relative_to(root)), rustdoc_sha256=digest(metadata),
                          rustdoc_log_sha256=digest(output / (crate + ".rustdoc.txt")))
            record["commands"].append(command)
        results[crate] = record
    result = {"contract": manifest["contract"], "compiler": compiler, "source_revision": revision,
              "cargo": subprocess.check_output(["cargo", "--version"], text=True).strip(),
              "environment_override": {"RUSTC_BOOTSTRAP": "1"}, "crates": results,
              "cargo_lock_sha256": digest(root / "Cargo.lock"), "inputs_sha256": digest(DIRECTORY / "inputs.json"),
              "build_script_sha256": digest(Path(__file__)), "build_scripts_and_proc_macros_executed": True,
              "target_programs_executed": False, "product_dependency": False, "analyzer_input": False}
    (output / "build.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"built_crates": list(results), "target_programs_executed": False}), flush=True)


if __name__ == "__main__":
    main()
