#!/usr/bin/env python3
"""Build a development dataset from the broad dataset and an explicit decisions file (offline).

  build --decisions FILE --output-root DIR   filter the broad dataset into DIR/<content-id>/ plus DIR/build-report.json
  convert-v1 --output FILE                   convert the frozen v1 effective decisions into the decision format below

Decision file (JSON Lines, one record per candidate concept):
  {"concept_id", "decision": "include"|"exclude", "keep_lexicalization_ids": [...], "basis", "reason"}
Concepts absent from the file are excluded (default non-serving). The filter is the v1
development candidate recipe (build_development_dataset.py) with only the decision source
replaced. Outputs are new files only: staging is renamed into place, existing directories
and reports are never overwritten, and failed staging is retained for inspection.
"""
from __future__ import annotations

import argparse
import collections
import json
import platform
import shutil
import sqlite3
import sys
import tempfile
from pathlib import Path

import build_dataset as storage
from rtpoc_common import (
    DATA_ROOT, DATASETS_DIR, NORMALIZATION_VERSION, REPO_ROOT, SCHEMA_VERSION, SOURCES_LOCK_PATH,
    canonical_json, read_json, sha256_bytes, sha256_file,
)

BROAD_ID = "rtd1-8efd9676107a983d"
BROAD = DATASETS_DIR / BROAD_ID
V1_DECISIONS = DATA_ROOT / "development-curation/semantic-adjudication-v2/effective/decisions.jsonl"
V1_DECISIONS_SHA256 = "465395457b362446a179e7af86896c37613f91d5992217ca3f406f83c2679611"
KINDS = storage.JSONL_FILES
KEYS = storage.PRIMARY_KEYS
ALLOWED = frozenset(("synonym", "alias", "abbreviation_expansion", "abbreviation_of"))
DECISION_FIELDS = frozenset(("concept_id", "decision", "keep_lexicalization_ids", "basis", "reason"))
REPORT_NAME = "build-report.json"
RECIPE = {
    "version": "development-dataset/2",
    "decisions": "explicit decisions file; concepts absent from it are excluded (default non-serving)",
    "concepts": "include decisions with at least one kept original lexicalization",
    "lexicalizations": "exactly the kept IDs, each owned by its decision's concept",
    "relations": "original allowed-kind records only; both endpoint lexicalizations kept; no transitive or new relations",
    "relation_kinds": sorted(ALLOWED),
    "expressions": "original records with a retained surface; surfaces = sorted unique retained surfaces",
    "sources": "original records of contributing sources; original attribution/license bytes",
    "content_id": "unchanged rt-dataset/1 JSONL-only formula",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def display_path(path: Path) -> str:
    resolved = path.resolve()
    return resolved.relative_to(REPO_ROOT).as_posix() if resolved.is_relative_to(REPO_ROOT) else str(resolved)


def new_json(path: Path, value: object) -> None:
    # Same layout as the v1 builder, so equal content yields equal manifest bytes.
    with path.open("x", encoding="utf-8") as handle:
        handle.write(json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n")


def jsonl_rows(payload: bytes, label: str) -> list[dict]:
    # Split on "\n" only: unescaped U+2028/U+2029 inside JSON strings are not line breaks.
    lines = payload.decode("utf-8").split("\n")
    if lines[-1] == "":
        lines.pop()
    rows = []
    for line_number, line in enumerate(lines, 1):
        try:
            rows.append(json.loads(line))
        except ValueError as error:
            raise ValueError(f"{label}:{line_number}: invalid JSON line") from error
    return rows


def read_decisions(payload: bytes, label: str) -> list[dict]:
    decisions = jsonl_rows(payload, label)
    for line_number, row in enumerate(decisions, 1):
        where = f"{label}:{line_number}"
        require(isinstance(row, dict) and set(row) == DECISION_FIELDS,
                f"{where}: decision fields must be exactly {sorted(DECISION_FIELDS)}")
        require(isinstance(row["concept_id"], str) and row["concept_id"], f"{where}: invalid concept_id")
        require(row["decision"] in ("include", "exclude"), f"{where}: decision must be include or exclude")
        keep = row["keep_lexicalization_ids"]
        require(isinstance(keep, list) and all(isinstance(lid, str) for lid in keep) and len(set(keep)) == len(keep),
                f"{where}: keep_lexicalization_ids must be unique strings")
        require(row["decision"] == "include" or not keep, f"{where}: exclude decision keeps lexicalizations")
        require(isinstance(row["basis"], str) and row["basis"] and isinstance(row["reason"], str),
                f"{where}: basis must be a nonempty string and reason a string")
    require(decisions, f"{label}: no decisions")
    require(len({row["concept_id"] for row in decisions}) == len(decisions), f"{label}: duplicate concept_id")
    return decisions


def verify_broad() -> dict:
    manifest = read_json(BROAD / "manifest.json")
    hashes = {name: sha256_file(BROAD / f"{name}.jsonl") for name in KINDS}
    require((manifest["dataset_id"], manifest["schema_version"], manifest["normalization_version"]) ==
            (BROAD_ID, SCHEMA_VERSION, NORMALIZATION_VERSION), "unexpected broad dataset contract")
    require(hashes == manifest["jsonl_sha256"] and storage.dataset_id_from_hashes(hashes) == BROAD_ID,
            "broad dataset JSONL differs from its manifest/content ID")
    return manifest


def broad_records(name: str):
    # Streamed: the broad relations file is too large to hold as parsed records.
    with (BROAD / f"{name}.jsonl").open(encoding="utf-8") as handle:
        for line in handle:
            yield json.loads(line)


def filtered_records(decisions: list[dict], broad_manifest: dict) -> tuple[dict, dict]:
    requested = {row["concept_id"]: row for row in decisions}
    owners = {lid: row["concept_id"] for row in decisions for lid in row["keep_lexicalization_ids"]}
    require(len(owners) == sum(len(row["keep_lexicalization_ids"]) for row in decisions),
            "lexicalization kept by more than one decision")
    concepts = {r["concept_id"]: r for r in broad_records("concepts") if r["concept_id"] in requested}
    require(len(concepts) == len(requested),
            f"decisions for unknown concepts: {sorted(set(requested) - set(concepts))[:5]}")
    included = sorted(cid for cid, row in requested.items()
                      if row["decision"] == "include" and row["keep_lexicalization_ids"])

    lexicalizations = []
    for lex in broad_records("lexicalizations"):
        owner = owners.get(lex["lexicalization_id"])
        if owner is not None:
            require(lex["concept_id"] == owner,
                    f"kept lexicalization {lex['lexicalization_id']} does not belong to {owner}")
            lexicalizations.append(lex)
    found = {lex["lexicalization_id"] for lex in lexicalizations}
    require(len(found) == len(owners), f"unknown kept lexicalizations: {sorted(set(owners) - found)[:5]}")

    surfaces = collections.defaultdict(set)
    for lex in lexicalizations:
        surfaces[lex["expression_id"]].add(lex["surface"])
    expressions = []
    for original in broad_records("expressions"):
        retained = surfaces.get(original["expression_id"])
        if retained:
            require(retained <= set(original["surfaces"]), f"new surface for {original['expression_id']}")
            expressions.append({**original, "surfaces": sorted(retained)})
    require(len(expressions) == len(surfaces), "kept lexicalization without an original expression")

    relations = []
    dropped = collections.Counter()
    for relation in broad_records("relations"):
        if relation["relation_kind"] not in ALLOWED:
            dropped["prohibited_kind"] += 1
        elif not all(relation[end + "_lexicalization_id"] in owners for end in ("from", "to")):
            dropped["unkept_concept_qualified_endpoint"] += 1
        else:
            relations.append(relation)

    used_sources = {concepts[cid]["source_id"] for cid in included}
    sources = [r for r in storage.read_jsonl(BROAD, "sources") if r["source_id"] in used_sources]
    require(len(sources) == len(used_sources), "concept from an unknown source")
    records = {"sources": sources, "concepts": [concepts[cid] for cid in included], "expressions": expressions,
               "lexicalizations": lexicalizations, "relations": relations}
    for name, rows in records.items():
        require(rows == sorted(rows, key=lambda r: r[KEYS[name]]), f"unsorted filtered {name}")
    summary = {
        "candidate_concepts": len(decisions),
        "default_excluded_non_serving": broad_manifest["record_counts"]["concepts"] - len(decisions),
        "decision_counts": dict(sorted(collections.Counter(row["decision"] for row in decisions).items())),
        "decision_basis_counts": dict(sorted(collections.Counter(row["basis"] for row in decisions).items())),
        "include_without_kept_lexicalizations": sum(1 for row in decisions
                                                    if row["decision"] == "include" and not row["keep_lexicalization_ids"]),
        "retained_counts": {name: len(records[name]) for name in KINDS},
        "dropped_relations": dict(sorted(dropped.items())),
    }
    return records, summary


def copy_notices(directory: Path, source_ids: set[str]) -> None:
    shutil.copyfile(BROAD / "ATTRIBUTION.md", directory / "ATTRIBUTION.md")
    (directory / "LICENSES").mkdir()
    for source in read_json(SOURCES_LOCK_PATH)["sources"]:
        if source["source_id"] in source_ids:
            for filename in source["license_files"]:
                leaf = source["source_id"] + "-" + Path(filename).name
                shutil.copyfile(BROAD / "LICENSES" / leaf, directory / "LICENSES" / leaf)


def build(decisions_path: Path, output_root: Path) -> dict:
    output_root = output_root.resolve()
    require(not output_root.is_relative_to(DATASETS_DIR.resolve()),
            "output root must be outside the published datasets directory")
    report_path = output_root / REPORT_NAME
    require(not report_path.exists() and not report_path.is_symlink(),
            f"refusing to overwrite existing report: {display_path(report_path)}")
    payload = decisions_path.read_bytes()
    decisions = read_decisions(payload, display_path(decisions_path))
    broad_manifest = verify_broad()
    records, summary = filtered_records(decisions, broad_manifest)

    output_root.mkdir(parents=True, exist_ok=True)
    staging_root = Path(tempfile.mkdtemp(prefix=".staging-", dir=output_root))
    try:
        staging = staging_root / "canonical"
        staging.mkdir()
        hashes = storage.write_jsonl(staging, records)
        dataset_id = storage.dataset_id_from_hashes(hashes)
        require(dataset_id != BROAD_ID, "broad identity reused")
        destination = output_root / dataset_id
        require(not destination.exists() and not destination.is_symlink(),
                f"refusing to overwrite existing directory: {display_path(destination)}")
        manifest = {"dataset_id": dataset_id, "schema_version": SCHEMA_VERSION,
                    "normalization_version": NORMALIZATION_VERSION, "jsonl_sha256": hashes,
                    "record_counts": {name: len(records[name]) for name in KINDS}}
        new_json(staging / "manifest.json", manifest)
        copy_notices(staging, {r["source_id"] for r in records["sources"]})
        # verify_dataset requires the directory name to equal the content ID.
        candidate = staging.rename(staging_root / dataset_id)
        storage.build_sqlite(candidate)
        verification = storage.verify_dataset(candidate)
        # A directory rename may replace an empty destination, so check again right before it.
        require(not destination.exists() and not destination.is_symlink(),
                f"refusing to overwrite existing directory: {display_path(destination)}")
        candidate.rename(destination)
        staging_root.rmdir()
    except BaseException:
        print(f"build failed; staging retained: {display_path(staging_root)}", file=sys.stderr)
        raise
    report = {
        "status": "pass",
        "dataset_id": dataset_id,
        "dataset_dir": display_path(destination),
        "source_dataset_id": BROAD_ID,
        "decisions": {"path": display_path(decisions_path), "sha256": sha256_bytes(payload), "bytes": len(payload)},
        "builder": {"path": display_path(Path(__file__)), "sha256": sha256_file(Path(__file__))},
        "recipe": RECIPE,
        "manifest": manifest,
        "filter": summary,
        "verification": verification,
        "environment": {"python": platform.python_version(), "sqlite": sqlite3.sqlite_version},
    }
    new_json(report_path, report)
    return report


def convert_v1(output: Path) -> dict:
    require(not output.exists() and not output.is_symlink(), f"refusing to overwrite existing file: {display_path(output)}")
    payload = V1_DECISIONS.read_bytes()
    require(sha256_bytes(payload) == V1_DECISIONS_SHA256, "frozen v1 decisions changed")
    converted = []
    for row in jsonl_rows(payload, display_path(V1_DECISIONS)):
        require(row["decision"] in ("include", "exclude", "uncertain"), f"unknown v1 decision: {row['concept_id']}")
        # Uncertain never served in v1, so it is carried as exclude.
        is_included = row["decision"] == "include"
        converted.append({"concept_id": row["concept_id"], "decision": "include" if is_included else "exclude",
                          "keep_lexicalization_ids": sorted(row["keep_lexicalization_ids"]) if is_included else [],
                          "basis": "v1", "reason": row["development_rationale"]})
    converted.sort(key=lambda row: row["concept_id"])
    content = "".join(canonical_json(row) + "\n" for row in converted).encode("utf-8")
    read_decisions(content, "converted decisions")
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("xb") as handle:
        handle.write(content)
    return {"status": "pass", "source": {"path": display_path(V1_DECISIONS), "sha256": V1_DECISIONS_SHA256},
            "output": {"path": display_path(output), "sha256": sha256_bytes(content), "rows": len(converted)},
            "decision_counts": dict(sorted(collections.Counter(row["decision"] for row in converted).items())),
            "kept_lexicalizations": sum(len(row["keep_lexicalization_ids"]) for row in converted)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    build_parser = commands.add_parser("build")
    build_parser.add_argument("--decisions", type=Path, required=True)
    build_parser.add_argument("--output-root", type=Path, required=True)
    convert_parser = commands.add_parser("convert-v1")
    convert_parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "build":
            result = build(args.decisions, args.output_root)
        else:
            result = convert_v1(args.output)
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except (ValueError, KeyError, OSError, TypeError) as error:
        raise SystemExit(f"ERROR: {error}") from error


if __name__ == "__main__":
    main()
