#!/usr/bin/env python3
"""Measure pinned supplemental source variants without changing the base corpus."""

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys


DIRECTORY = Path(__file__).resolve().parent
CORPUS = DIRECTORY.parent
sys.path.insert(0, str(CORPUS))
from manage import evaluate


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("variants", nargs="*")
    parser.add_argument("--sources", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=DIRECTORY / "results")
    parser.add_argument("--composites", type=Path, default=Path("/tmp/codemap-language-composite-sources"))
    args = parser.parse_args()
    manifest = json.loads((DIRECTORY / "inputs.json").read_text())
    labels = json.loads((CORPUS / "cases.json").read_text())["cases"]
    if set(args.variants) - manifest["variants"].keys():
        parser.error("unknown variant")
    args.output.mkdir(parents=True, exist_ok=True)
    failures = []
    for name, spec in manifest["variants"].items():
        if args.variants and name not in args.variants:
            continue
        repository_root = args.sources / spec["repository"]
        root = args.composites / spec["composite"] if spec.get("composite") else repository_root
        revision = subprocess.check_output(["git", "-C", str(repository_root), "rev-parse", "HEAD"], text=True).strip()
        if revision != spec["revision"]:
            raise ValueError(name + ": revision mismatch")
        for relative, expected in spec["source_sha256"].items():
            path = root / relative
            if path.is_symlink() or not path.resolve().is_relative_to(root.resolve()) or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
                raise ValueError(name + ": source mismatch " + relative)
        output = args.output / (name + ".json.gz")
        command = [sys.executable, "-X", "faulthandler", str(CORPUS / "analyze_languages.py"),
                   "--root", str(root), "--language", spec["language"], "--output", str(output)]
        for path in spec["paths"]:
            command.extend(["--path", path])
        for language, path in spec["support_paths"]:
            command.extend(["--support-path", language + ":" + path])
        if spec.get("module_bindings"):
            bindings_path = args.output / (name + ".module-bindings.json")
            bindings_path.write_text(json.dumps(spec["module_bindings"], indent=2) + "\n")
            command.extend(["--module-bindings", str(bindings_path)])
        command.extend(["--max-file-bytes", str(spec.get("max_file_bytes", 524288))])
        subprocess.run(command, check=True)
        result = json.loads(gzip.decompress(output.read_bytes()))
        assert result["scope"]["source_sha256"] == spec["source_sha256"], name
        result["revision"] = revision
        result["input_variant"] = name
        output.write_bytes(gzip.compress((json.dumps(result, ensure_ascii=False, separators=(",", ":")) + "\n").encode(), mtime=0))
        cases = [case for case in labels if case["repository"] == spec["repository"] and case["language"] == spec["language"]]
        evaluation = evaluate(cases, {(spec["repository"], spec["language"]): result})
        evaluation["input_variant"] = name
        evaluation["max_file_bytes"] = spec.get("max_file_bytes", 524288)
        evaluation["input_lock_sha256"] = hashlib.sha256((DIRECTORY / "inputs.json").read_bytes()).hexdigest()
        (args.output / (name + ".evaluation.json")).write_text(json.dumps(evaluation, ensure_ascii=False, indent=2) + "\n")
        failures.extend(case["id"] for case in evaluation["cases"] if case["status"] == "incorrect_connection")
        print(json.dumps({"variant": name, "cases": [{"id": case["id"], "status": case["status"]} for case in evaluation["cases"]]}, ensure_ascii=False), flush=True)
    return int(bool(failures))


if __name__ == "__main__":
    raise SystemExit(main())
