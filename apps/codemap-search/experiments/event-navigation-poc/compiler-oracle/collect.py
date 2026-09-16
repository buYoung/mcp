#!/usr/bin/env python3
"""Audit selected source spans in compiler output; never create navigation edges."""

import argparse
from collections import defaultdict
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess

DIRECTORY = Path(__file__).resolve().parent
SPAN = re.compile(r"\bat ([^\s]+\.rs):(\d+):(\d+): (\d+):(\d+)")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def observe_mir(path, probes):
    selected = defaultdict(list)
    by_source = defaultdict(list)
    for probe in probes:
        by_source[probe["path"]].append(probe)
    function = None
    with path.open() as lines:
        for number, line in enumerate(lines, 1):
            if line.startswith("fn "):
                function = line.rstrip()
            elif line.rstrip() == "}":
                function = None
            if not function:
                continue
            code, marker, comment = line.rpartition("//")
            span = SPAN.search(comment) if marker else None
            if not span or not code.strip():
                continue
            source, begin, column, end, end_column = span.groups()
            begin, end = int(begin), int(end)
            for probe in by_source.get(source, ()):
                if begin <= probe["lines"][1] and end >= probe["lines"][0]:
                    selected[probe["id"]].append({"function": function, "mir_line": number, "code": code.strip(),
                                                   "span": {"path": source, "begin": [begin, int(column)], "end": [end, int(end_column)]}})
                    if len(selected[probe["id"]]) > 2048:
                        raise ValueError("compiler observation span is too broad")
    return selected


def observe_traits(path, probes):
    data = json.loads(path.read_bytes())
    if data["format_version"] != 60:
        raise ValueError("unsupported rustdoc format")
    selected = defaultdict(list)
    for item in data["index"].values():
        implementation = item["inner"].get("impl")
        span = item.get("span")
        if not implementation or not implementation.get("trait") or implementation["is_negative"] or not span or item["crate_id"] != 0:
            continue
        owner = implementation["for"].get("resolved_path", {})
        trait = implementation["trait"]
        for probe in probes:
            if (owner.get("path") == probe["owner"] and trait["path"] == probe["trait"] and span["filename"] == probe["path"]
                    and probe["lines"][0] <= span["begin"][0] <= span["end"][0] <= probe["lines"][1]):
                path_for = lambda entry: data["paths"].get(str(entry["id"]), {}).get("path", [entry["path"]])
                members = [data["index"][str(identifier)] for identifier in implementation["items"]]
                selected[probe["id"]].append({"owner": owner, "owner_path": path_for(owner), "trait": trait,
                                             "trait_path": path_for(trait), "generics": implementation["generics"], "span": span,
                                             "members": [{"name": member["name"], "span": member["span"], "inner": member["inner"]} for member in members]})
    return selected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=DIRECTORY / "results")
    args = parser.parse_args()
    root, output = args.root.resolve(), args.output.resolve()
    manifest = json.loads((DIRECTORY / "inputs.json").read_text())
    build = json.loads((output / "build.json").read_text())
    assert build["inputs_sha256"] == digest(DIRECTORY / "inputs.json")
    assert build["compiler"] == manifest["compiler"]
    assert build["source_revision"] == subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    assert build["cargo_lock_sha256"] == digest(root / "Cargo.lock") == manifest["cargo_lock_sha256"]
    subprocess.run(["git", "diff", "--quiet", "HEAD", "--"], cwd=root, check=True)
    records, traits, hashes, manifest_hashes = {}, {}, {}, {}
    for crate, artifact in build["crates"].items():
        mir = root / artifact["mir"]
        assert mir.resolve().is_relative_to(root) and digest(mir) == artifact["mir_sha256"]
        probes = [probe for probe in manifest["probes"] if probe["crate"] == crate]
        records.update(observe_mir(mir, probes))
        if "rustdoc" in artifact:
            metadata = root / artifact["rustdoc"]
            assert metadata.resolve().is_relative_to(root) and digest(metadata) == artifact["rustdoc_sha256"]
            traits.update(observe_traits(metadata, [probe for probe in manifest["trait_probes"] if probe["crate"] == crate]))
        for path in (root / "Cargo.toml", root / "crates" / crate / "Cargo.toml"):
            manifest_hashes[str(path.relative_to(root))] = digest(path)
    for probe in manifest["probes"] + manifest["trait_probes"]:
        path = root / probe["path"]
        assert path.is_file() and path.resolve().is_relative_to(root)
        hashes[probe["path"]] = digest(path)
    rows = []
    for probe in manifest["probes"]:
        instructions = records.get(probe["id"], [])
        missing = [fragment for fragment in probe["required_fragments"] if not any(fragment in item["code"] for item in instructions)]
        rows.append({"id": probe["id"], "route": probe["route"], "observed": bool(instructions) and not missing,
                     "missing_fragments": missing, "instructions": instructions})
    for probe in manifest["trait_probes"]:
        witnesses = traits.get(probe["id"], [])
        rows.append({"id": probe["id"], "route": probe.get("route", "type_support"), "observed": len(witnesses) == 1, "witnesses": witnesses})
    observations = {"contract": manifest["contract"], "purpose": "compiler_observation_only", "analyzer_input": False,
                    "product_dependency": False, "ast_coverage_credit": False, "end_to_end_connection_proven": False,
                    "concrete_instance_proven": False, "event_classification": "not_inferred", "target_programs_executed": False,
                    "build_sha256": digest(output / "build.json"), "inputs_sha256": digest(DIRECTORY / "inputs.json"),
                    "collector_sha256": digest(Path(__file__)), "source_sha256": hashes, "cargo_manifest_sha256": manifest_hashes,
                    "probes": rows}
    payload = (json.dumps(observations, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
    (output / "observations.json.gz").write_bytes(gzip.compress(payload, mtime=0))
    summary = {key: value for key, value in observations.items() if key not in {"probes", "source_sha256", "cargo_manifest_sha256"}}
    summary.update(observations_sha256=digest(output / "observations.json.gz"), probes=len(rows), observed=sum(row["observed"] for row in rows),
                   failures=[{key: value for key, value in row.items() if key not in {"instructions", "witnesses"}} for row in rows if not row["observed"]])
    (output / "evaluation.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return int(bool(summary["failures"]))


if __name__ == "__main__":
    raise SystemExit(main())
