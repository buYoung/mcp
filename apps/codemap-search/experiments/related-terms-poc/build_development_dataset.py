#!/usr/bin/env python3
"""Explicit, offline development candidate construction; never activates or evaluates.

Run snapshot, two builds with distinct --name values, then finalize. All writes are
new files under the candidate area or a new content-ID directory. Failed staging
is retained. Frozen producers, source archives, decisions and active handoffs are
read-only prerequisites; nothing is acquired or automatically prepared.
"""
from __future__ import annotations

import argparse
import collections
import json
import os
import platform
import re
import shutil
import sqlite3
import subprocess
import sys
from pathlib import Path

import build_dataset as storage
import prepare_development_curation as preparation
import project_development_curation as projection
from rtpoc_common import (
    DATA_ROOT, DATASETS_DIR, EXPERIMENT_DIR, NORMALIZATION_VERSION, REPO_ROOT,
    SCHEMA_VERSION, SOURCES_LOCK_PATH, canonical_json, exact_key, read_json,
    repo_relative, sha256_bytes, sha256_file, token_key,
)

AREA = DATA_ROOT / "development-curation/development-dataset-candidate-v1"
FINAL = DATA_ROOT / "development-curation/semantic-adjudication-v2"
BROAD_ID = "rtd1-8efd9676107a983d"
BROAD = DATASETS_DIR / BROAD_ID
FINAL_HASH = "ca849cf6f247a4dc6b27bc9180ae977a87f0126a4f1c32b2a4624976b657d7aa"
DECISIONS_HASH = "465395457b362446a179e7af86896c37613f91d5992217ca3f406f83c2679611"
SOURCE_HANDOFF_HASH = "47a4d15cdf00522531da1ed6804a897cf8a23d6d3379b64766b4c509e169b2c3"
HEAD = "46995808c69cc452bd2fcdd9fd146244d0a4c734"
CURRENT_HELPERS = {
    "build_dataset.py": "8c1a6928b7cf3bbcd9aa40e964e2bfa20ee5676ea56ef07167ffb5a5efda4f17",
    "rtpoc_common.py": "af51ee25d2fdde9c76e3e2cdbd2f5c7a8715b4880ef52b701dd1dbfefe66927f",
    "schema.sql": "10fe8d766c9db972a5b37b45843482c9bd240926981dbaef448c3c4619db7e06",
}
KINDS = storage.JSONL_FILES
KEYS = storage.PRIMARY_KEYS
ALLOWED = frozenset(("synonym", "alias", "abbreviation_expansion", "abbreviation_of"))
TERMS = ("start", "stop", "delete", "retry", "thread", "event loop", "res")
RECIPE = {
    "version": "development-dataset-candidate/1",
    "concepts": "include decisions with kept original lexicalizations only",
    "lexicalizations": "exact original concept-qualified keep IDs; exclude/uncertain/default reject never serve",
    "relations": "original allowed-kind records only; both exact lexicalization endpoints kept; no transitive expansion",
    "relation_kinds": sorted(ALLOWED),
    "expressions": "original IDs/keys; surfaces = sorted unique retained lexicalization surfaces; supported expressions only",
    "sources": "original complete metadata for contributing sources; original attribution/license bytes",
    "content_id": "unchanged rt-dataset/1 JSONL-only formula",
    "derivation_identity": "SHA256 of canonical_json(preimage), UTF-8 without newline; separate from content ID",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=REPO_ROOT, text=True).strip()


def new_json(path: Path, value: object) -> None:
    # Exclusive create, including evidence: reruns cannot silently replace a result.
    with path.open("x", encoding="utf-8") as handle:
        handle.write(json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n")


def identity(path: Path) -> dict:
    require(path.is_file() and not path.is_symlink(), f"missing/symlink input: {path}")
    return {"path": repo_relative(path), "sha256": sha256_file(path), "bytes": path.stat().st_size}


def checked(path: Path, digest: str, size: int | None = None) -> dict:
    value = identity(path)
    require(value["sha256"] == digest and (size is None or value["bytes"] == size),
            f"input hash/size mismatch: {repo_relative(path)}")
    return value


def snapshot() -> dict:
    require(not AREA.exists(), "candidate area already exists; refusing snapshot overwrite")
    require(git("rev-parse", "HEAD") == HEAD and not git("diff", "--cached", "--name-only"), "unexpected HEAD/staging")
    # The original inventory covers old workspace/source/history files. Extend it
    # with every current curation artifact and every tracked/untracked repo file.
    old = read_json(DATA_ROOT / "development-curation/preservation-baseline.json")
    paths = {REPO_ROOT / p for p in old["files_sha256"]}
    paths.update(REPO_ROOT / p for p in git("ls-files", "--cached", "--others", "--exclude-standard").splitlines())
    for root in (DATA_ROOT / "development-curation", DATASETS_DIR, DATA_ROOT / "sources",
                 DATA_ROOT / "workspaces/state"):
        paths.update(p for p in root.rglob("*") if p.is_file())
    paths.discard(Path(__file__).resolve())
    entries = [identity(p) for p in sorted(paths)]
    result = {"head": HEAD, "git_status": git("status", "--short"), "files": entries}
    AREA.mkdir()
    new_json(AREA / "preservation-before.json", result)
    return {"status": "pass", "files": len(entries), "snapshot": repo_relative(AREA / "preservation-before.json")}


def preservation() -> dict:
    baseline = read_json(AREA / "preservation-before.json")
    changed = []
    for entry in baseline["files"]:
        path = REPO_ROOT / entry["path"]
        if not path.is_file() or path.stat().st_size != entry["bytes"] or sha256_file(path) != entry["sha256"]:
            changed.append(entry["path"])
    result = {"head": git("rev-parse", "HEAD"), "no_staged_files": not git("diff", "--cached", "--name-only"),
              "unchanged_preexisting_files": len(baseline["files"]) - len(changed), "changed": changed}
    require(not changed and result["head"] == HEAD and result["no_staged_files"], f"preservation failure: {result}")
    return result


def frozen_inputs() -> tuple[dict, list[dict], dict]:
    bindings = [checked(FINAL / "final-decisions-handoff.json", FINAL_HASH)]
    handoff = read_json(FINAL / "final-decisions-handoff.json")
    require(handoff["status"] == "ready" and handoff["final_combined"]["sha256"] == DECISIONS_HASH,
            "unapproved final decisions")
    for entry in handoff["material"]:
        # Existing handoff absolute paths are input locators only, never serving metadata.
        path = Path(entry["path"])
        require(path.resolve().is_relative_to(FINAL.resolve()), "material outside frozen final area")
        bindings.append(checked(path, entry["sha256"], entry["bytes"]))
    for key in ("baseline", "collection_handoff"):
        entry = handoff[key]
        bindings.append(checked(Path(entry["path"]), entry["sha256"], entry["bytes"]))
    bindings.append(checked(FINAL / "provenance/evidence-audit.json", handoff["evidence_gate_audit"]["sha256"]))
    source_handoff_path = EXPERIMENT_DIR / "results/dataset-handoff.json"
    bindings.append(checked(source_handoff_path, SOURCE_HANDOFF_HASH))
    source_handoff = read_json(source_handoff_path)
    require(source_handoff["dataset_id"] == BROAD_ID, "wrong source dataset")
    for relative, digest in source_handoff["artifact_hashes"].items():
        bindings.append(checked(BROAD / relative, digest))
    bindings.append(checked(SOURCES_LOCK_PATH, source_handoff["source_lock_sha256"]))
    lock = read_json(SOURCES_LOCK_PATH)
    for source in lock["sources"]:
        for field in ("archive", "license_archive"):
            if field + "_path" in source:
                bindings.append(checked(REPO_ROOT / source[field + "_path"], source[field + "_sha256"]))
    # The source handoff records the historical producer, not today's helper.
    # Current helper bytes are separately pinned to both frozen curation inventories.
    baseline = read_json(DATA_ROOT / "development-curation/preservation-baseline.json")
    final_preservation = {entry["path"]: entry["sha256"] for entry in read_json(FINAL / "preservation-before.json")}
    for filename, digest in CURRENT_HELPERS.items():
        path = EXPERIMENT_DIR / filename
        require(baseline["files_sha256"][repo_relative(path)] == digest and
                final_preservation[str(path)] == digest, "unapproved current helper identity")
        bindings.append(checked(path, digest))
    for path, manifest_path in ((Path(preparation.__file__), preparation.PACKETS / "manifest.json"),
                                (Path(projection.__file__), projection.ROOT / "manifest.json")):
        digest = final_preservation[str(path)]
        require(read_json(manifest_path)["input_inventory"]["script_sha256"] == digest,
                "curation validator identity drift")
        bindings.append(checked(path, digest))
    # Inspected read-only validators. Do not call prepare/build/handoff publishers.
    official = projection.validate_decisions(FINAL / "effective/decisions.jsonl", None)
    decisions = list(preparation.jsonl(FINAL / "effective/decisions.jsonl"))
    packets = []
    for name, entry in sorted(handoff["final_packets"].items()):
        path = Path(entry["path"])
        checked(path, entry["sha256"], entry["bytes"])
        rows = list(preparation.jsonl(path))
        require(all(row["packet_name"] == name for row in rows), "misassigned effective packet")
        packets.extend(rows)
    require(sorted(packets, key=lambda r: r["concept_id"]) == sorted(decisions, key=lambda r: r["concept_id"]),
            "effective packets do not equal combined decisions")
    counts = collections.Counter(row["decision"] for row in decisions)
    require(len(decisions) == 7147 and dict(counts) == handoff["after"]["decision_counts"], "decision count mismatch")
    require(sum(len(row["lexicalization_rationales"]) for row in decisions) == 17830, "lexicalization coverage mismatch")
    require(sum(len(row["keep_lexicalization_ids"]) for row in decisions) == 8445, "kept count mismatch")
    source_bindings = read_json(FINAL / "source-license-bindings.json")
    require(storage.read_jsonl(BROAD, "sources") == source_bindings["raw_source_records_verbatim"], "source/license drift")
    # Bind portable local provenance, not the original runtime evidence path.
    bindings += [identity(Path(__file__)), identity(projection.CONTRACT), identity(storage.RECORD_SCHEMA_PATH),
                 identity(projection.ROOT / "manifest.json"), identity(preparation.PACKETS / "manifest.json")]
    bindings = list({entry["path"]: entry for entry in bindings}.values())
    return {"artifacts": sorted(bindings, key=lambda r: r["path"]), "source_dataset_id": BROAD_ID,
            "historical_producer_environment": source_handoff["environment"],
            "current_helper_sha256": CURRENT_HELPERS,
            "producer_identity_note": "Historical producer fingerprint preserved verbatim; current approved helpers separately pinned. Historical producer bytes not reexecuted or recovered."}, decisions, official


def original_records(name: str):
    schema = read_json(storage.RECORD_SCHEMA_PATH)["$defs"][name[:-1]]
    manifest = read_json(BROAD / "manifest.json")
    for record in preparation.checked_records(name, manifest):
        require(not storage.schema_errors(record, schema), f"original schema violation: {name}")
        yield record


def check_support(relation: dict, lexicalizations: dict, concepts: dict, sources: dict) -> None:
    kind = relation["relation_kind"]
    evidence, same_concept = storage.RELATION_RULES[kind]
    require(relation["evidence_kind"] in evidence, "unresolved original relation evidence")
    require(not same_concept or relation["from_concept_id"] == relation["to_concept_id"], "cross-concept equivalence")
    for end in ("from", "to"):
        lex = lexicalizations[relation[end + "_lexicalization_id"]]
        concept = concepts[lex["concept_id"]]
        require((lex["concept_id"], lex["expression_id"]) ==
                (relation[end + "_concept_id"], relation[end + "_expression_id"]), "misqualified relation endpoint")
        require(concept["source_id"] == relation["source_id"] and concept["source_id"] in sources,
                "misqualified relation source")
    roles = storage.ABBREVIATION_ROLES.get(kind)
    if roles:
        require(tuple(lexicalizations[relation[e + "_lexicalization_id"]]["role"] for e in ("from", "to")) == roles,
                "wrong abbreviation support roles")
    expected = "r:" + sha256_bytes(canonical_json([kind, relation["evidence_kind"],
               relation["from_lexicalization_id"], relation["to_lexicalization_id"]]).encode())[:24]
    require(relation["relation_id"] == expected, "original relation ID mismatch")
    require(relation["from_expression_id"] != relation["to_expression_id"], "self-expression relation")
    require(relation["direction"] == ("symmetric_pair" if kind in ("synonym", "alias") else "declared"),
            "original direction mismatch")


def filtered_records(decisions: list[dict]) -> tuple[dict, dict]:
    concepts = {r["concept_id"]: r for r in original_records("concepts")}
    lexicalizations = {r["lexicalization_id"]: r for r in original_records("lexicalizations")}
    sources = {r["source_id"]: r for r in original_records("sources")}
    by_concept = collections.defaultdict(set)
    for lid, lex in lexicalizations.items():
        require(lex["concept_id"] in concepts, "unresolved lexicalization concept")
        by_concept[lex["concept_id"]].add(lid)
    kept = set()
    included = set()
    for row in decisions:
        cid = row["concept_id"]
        require(cid in concepts and set(row["lexicalization_rationales"]) == by_concept[cid], "decision/source scope mismatch")
        if row["decision"] == "include":
            included.add(cid)
            for lid in row["keep_lexicalization_ids"]:
                require(lid in by_concept[cid] and lid not in kept, "duplicate/misqualified kept endpoint")
                kept.add(lid)
    require(len(included) == 3810 and len(kept) == 8445 and len(concepts) - len(decisions) == 104571,
            "approved partition mismatch")
    retained_lex = [lexicalizations[lid] for lid in sorted(kept)]
    surfaces = collections.defaultdict(set)
    for lex in retained_lex:
        surfaces[lex["expression_id"]].add(lex["surface"])
    expressions = []
    for original in original_records("expressions"):
        eid = original["expression_id"]
        if eid not in surfaces:
            continue
        key, lossy = token_key(original["exact_key"])
        require(eid == "x:" + original["exact_key"] and (key, lossy, len(key.split())) ==
                (original["token_key"], original["token_key_lossy"], original["token_count"]), "normalization drift")
        require(all(exact_key(s) == original["exact_key"] and s in original["surfaces"] for s in surfaces[eid]),
                "new or misnormalized spelling")
        expressions.append({**original, "surfaces": sorted(surfaces[eid])})
    require({r["expression_id"] for r in expressions} == set(surfaces) and len(expressions) == 5965,
            "unsupported expression")
    retained_relations = []
    drops = collections.Counter()
    expression_only_leaks = collections.Counter()
    for relation in original_records("relations"):
        check_support(relation, lexicalizations, concepts, sources)
        if relation["relation_kind"] not in ALLOWED:
            drops["prohibited_kind"] += 1
        elif not all(relation[e + "_lexicalization_id"] in kept for e in ("from", "to")):
            drops["unkept_concept_qualified_endpoint"] += 1
            if all(relation[e + "_expression_id"] in surfaces for e in ("from", "to")):
                expression_only_leaks[relation["relation_kind"]] += 1
        else:
            retained_relations.append(relation)
    used_sources = {concepts[cid]["source_id"] for cid in included}
    records = {"sources": [sources[sid] for sid in sorted(used_sources)],
               "concepts": [concepts[cid] for cid in sorted(included)], "expressions": expressions,
               "lexicalizations": retained_lex, "relations": retained_relations}
    for name, rows in records.items():
        require(rows == sorted(rows, key=lambda r: r[KEYS[name]]), "unsorted filtered records")
        require("/Users/" not in canonical_json(rows) and "/.pi/" not in canonical_json(rows), "local-path serving leak")
    return records, {"status": "pass", "candidate_concepts": len(decisions), "original_lexicalizations_reviewed": 17830,
                     "default_excluded_non_serving": 104571, "decision_counts": dict(collections.Counter(r["decision"] for r in decisions)),
                     "retained_counts": {n: len(records[n]) for n in KINDS}, "dropped_relations": dict(drops),
                     "global_expression_filter_would_resurrect": dict(expression_only_leaks),
                     "relation_support": "every original relation resolved to exact original lexicalization/concept/expression/source IDs",
                     "surfaces": "exact sorted unique kept lexicalization surfaces; no new or unkept-only surfaces"}


def binding(inputs: dict, manifest: dict) -> dict:
    preimage = {"schema_version": "development-dataset-derivation/1", "recipe": RECIPE,
                "recipe_sha256": sha256_bytes(canonical_json(RECIPE).encode()), "inputs": inputs,
                "content": manifest}
    return {"preimage": preimage, "derivation_sha256": sha256_bytes(canonical_json(preimage).encode()),
            "content_id_is_jsonl_only": True, "candidate_only": True}


def build(name: str) -> dict:
    require(re.fullmatch(r"build-[A-Za-z0-9-]+", name) is not None, "invalid owned build name")
    require((AREA / "preservation-before.json").is_file(), "snapshot prerequisite missing")
    root = AREA / name
    require(not root.exists() and not root.is_symlink(), "build location collision")
    root.mkdir()
    try:
        inputs, decisions, official = frozen_inputs()
        records, filters = filtered_records(decisions)
        staging = root / "canonical"
        staging.mkdir()
        hashes = storage.write_jsonl(staging, records)
        dataset_id = storage.dataset_id_from_hashes(hashes)
        require(dataset_id != BROAD_ID, "broad identity reused")
        require(not (DATASETS_DIR / dataset_id).exists(), "published content-ID collision")
        manifest = {"dataset_id": dataset_id, "schema_version": SCHEMA_VERSION,
                    "normalization_version": NORMALIZATION_VERSION, "jsonl_sha256": hashes,
                    "record_counts": {n: len(records[n]) for n in KINDS}}
        new_json(staging / "manifest.json", manifest)
        new_json(staging / "curation-binding.json", binding(inputs, manifest))
        shutil.copyfile(BROAD / "ATTRIBUTION.md", staging / "ATTRIBUTION.md")
        (staging / "LICENSES").mkdir()
        lock = read_json(SOURCES_LOCK_PATH)
        for source in lock["sources"]:
            if source["source_id"] in {r["source_id"] for r in records["sources"]}:
                for filename in source["license_files"]:
                    leaf = source["source_id"] + "-" + Path(filename).name
                    shutil.copyfile(BROAD / "LICENSES" / leaf, staging / "LICENSES" / leaf)
        candidate = root / dataset_id
        staging.rename(candidate)
        # Safe only in this exclusive new owned directory. The frozen helper's
        # unlink/replace operations cannot reach existing published artifacts.
        require(not (candidate / storage.SQLITE_NAME).exists() and not (candidate / (storage.SQLITE_NAME + ".tmp")).exists(),
                "SQLite output collision")
        storage.build_sqlite(candidate)
        verification = storage.verify_dataset(candidate)
        verification.update(validate_filter(candidate, decisions))
        report = {"status": "pass", "dataset_dir": repo_relative(candidate), "manifest": manifest,
                  "derivation_sha256": read_json(candidate / "curation-binding.json")["derivation_sha256"],
                  "official_decision_validation": official, "filter": filters, "verification": verification,
                  "environment": {"python": platform.python_version(), "sqlite": sqlite3.sqlite_version}}
        new_json(root / "build-report.json", report)
        return report
    except BaseException as error:
        new_json(root / "failure.json", {"status": "fail", "error": str(error), "cwd": ".", "head": git("rev-parse", "HEAD")})
        raise


def validate_filter(directory: Path, decisions: list[dict]) -> dict:
    # Independent readback checks against broad canonical records, not global IDs.
    records = {n: storage.read_jsonl(directory, n) for n in KINDS}
    included = {r["concept_id"] for r in decisions if r["decision"] == "include"}
    qualified = {(r["concept_id"], lid) for r in decisions if r["decision"] == "include" for lid in r["keep_lexicalization_ids"]}
    require({r["concept_id"] for r in records["concepts"]} == included, "excluded concept leak or omitted include")
    require({(r["concept_id"], r["lexicalization_id"]) for r in records["lexicalizations"]} == qualified,
            "lexicalization readback differs from exact approval")
    for name in ("sources", "concepts", "lexicalizations", "relations"):
        observed = {r[KEYS[name]]: r for r in records[name]}
        found = set()
        for original in original_records(name):
            key = original[KEYS[name]]
            if key in observed:
                require(observed[key] == original, f"non-original {name} record")
                found.add(key)
            if name == "relations":
                expected = original["relation_kind"] in ALLOWED and all(
                    (original[e + "_concept_id"], original[e + "_lexicalization_id"]) in qualified for e in ("from", "to"))
                require((key in observed) == expected, "relationship filter missing/extra original support")
        require(found == set(observed), "invented record")
    surfaces = collections.defaultdict(set)
    for lex in records["lexicalizations"]:
        surfaces[lex["expression_id"]].add(lex["surface"])
    require({r["expression_id"] for r in records["expressions"]} == set(surfaces), "unsupported expression readback")
    for expression in records["expressions"]:
        require(expression["surfaces"] == sorted(surfaces[expression["expression_id"]]), "unkept-only surface leak")
    require(all(r["relation_kind"] in ALLOWED for r in records["relations"]), "prohibited kind leak")
    manifest = read_json(directory / "manifest.json")
    with sqlite3.connect(f"file:{directory / storage.SQLITE_NAME}?mode=ro", uri=True) as connection:
        meta = dict(connection.execute("SELECT key, value FROM dataset_meta"))
        expected_meta = {"dataset_id": manifest["dataset_id"], "schema_version": SCHEMA_VERSION,
                         "normalization_version": NORMALIZATION_VERSION,
                         "jsonl_sha256": canonical_json(manifest["jsonl_sha256"]),
                         "record_counts": canonical_json(manifest["record_counts"])}
        require(meta == expected_meta, "SQLite metadata compatibility mismatch")
        counts = {n: connection.execute(f"SELECT count(*) FROM {n}").fetchone()[0] for n in KINDS}
        require(counts == manifest["record_counts"], "SQLite count mismatch")
    for p in (directory / "ATTRIBUTION.md", *sorted((directory / "LICENSES").iterdir())):
        require(sha256_file(p) == sha256_file(BROAD / p.relative_to(directory)), "license/attribution changed")
    return {"approved_endpoint_filter": "pass", "original_subset_provenance_license": "pass",
            "rebuilt_surfaces_no_excluded_leaks": "pass", "all_sqlite_meta_and_counts": "pass"}


def runtime_inputs() -> tuple[dict, dict]:
    handoff_path = EXPERIMENT_DIR / "results/lookup-handoff.json"
    handoff = read_json(handoff_path)
    evidence = [identity(handoff_path)]
    crate = handoff["crate"]
    for name, digest in crate["source_sha256"].items():
        evidence.append(checked(EXPERIMENT_DIR / name, digest))
    for filename, field in (("Cargo.toml", "cargo_toml_sha256"), ("Cargo.lock", "cargo_lock_sha256")):
        evidence.append(checked(EXPERIMENT_DIR / filename, crate[field]))
    for entry in (handoff["binary"], handoff["policy"]):
        evidence.append(checked(REPO_ROOT / entry["path"], entry["sha256"]))
    evidence.append(checked(REPO_ROOT / handoff["workspace_manifest_path"], handoff["workspace_manifest_sha256"]))
    for target in handoff["targets"].values():
        state = REPO_ROOT / target["state_dir"]
        evidence += [checked(state / "published.json", target["published_json_sha256"]),
                     checked(state / "index/meta.json", target["index_meta_sha256"]),
                     checked(REPO_ROOT / target["stats_path"], target["stats_sha256"])]
    require(all(check["exit"] == 0 for check in handoff["cargo_checks"]), "prior cargo evidence failed")
    require(git("rev-parse", "HEAD") == crate["repository_head"], "product source HEAD changed")
    return handoff, {"artifacts": evidence, "reused_checks": handoff["cargo_checks"],
                     "reuse_basis": "exact experiment source/Cargo/binary hashes verified; product HEAD unchanged; no Rust/TS/dependency changes",
                     "offline_code_path": handoff["serving_path_audit"],
                     "new_run_network_denial_observed": False}


def run_recorded(command: list[str], name: str) -> dict:
    # Print the exact command and cwd before each actual Rust invocation.
    import shlex
    print(f"cwd={REPO_ROOT}; {shlex.join(command)}", flush=True)
    require(not (AREA / name).exists(), "validation output collision")
    run = subprocess.run(command, cwd=REPO_ROOT, text=True, capture_output=True)
    report = {"command": command, "cwd": ".", "exit_code": run.returncode, "stdout": run.stdout, "stderr": run.stderr}
    new_json(AREA / name, report)
    require(run.returncode == 0, f"command failed; evidence preserved: {name}")
    return report


def finalize(first: str, second: str) -> dict:
    require(first != second and all(re.fullmatch(r"build-[A-Za-z0-9-]+", n) for n in (first, second)), "distinct owned builds required")
    require(not (AREA / "candidate-handoff.json").exists(), "handoff collision")
    inputs, decisions, _ = frozen_inputs()
    reports = [read_json(AREA / n / "build-report.json") for n in (first, second)]
    directories = [REPO_ROOT / r["dataset_dir"] for r in reports]
    dataset_id = reports[0]["manifest"]["dataset_id"]
    require(all(d == AREA / n / dataset_id for d, n in zip(directories, (first, second))), "unowned candidate path")
    checks = [storage.verify_dataset(d) for d in directories]
    for d in directories:
        validate_filter(d, decisions)
        require(read_json(d / "curation-binding.json") == binding(inputs, read_json(d / "manifest.json")), "curation derivation mismatch")
    require(reports[0]["manifest"] == reports[1]["manifest"] and
            reports[0]["derivation_sha256"] == reports[1]["derivation_sha256"] and
            checks[0]["logical_sqlite_sha256"] == checks[1]["logical_sqlite_sha256"], "nondeterministic repeat")
    first_files = {p.relative_to(directories[0]).as_posix(): sha256_file(p) for p in directories[0].rglob("*") if p.is_file()}
    second_files = {p.relative_to(directories[1]).as_posix(): sha256_file(p) for p in directories[1].rglob("*") if p.is_file()}
    semantic_files = set(first_files) - {storage.SQLITE_NAME}
    require(set(first_files) == set(second_files) and all(first_files[n] == second_files[n] for n in semantic_files), "repeat artifact mismatch")
    destination = DATASETS_DIR / dataset_id
    require(not destination.exists() and not destination.is_symlink() and dataset_id != BROAD_ID, "publication collision")
    runtime, reused = runtime_inputs()
    # Preserve both construction directories. Publish a verified copy by rename;
    # failed copy/validation leaves its owned staging, never cleans up old evidence.
    staging = AREA / "publication-staging"
    require(not staging.exists(), "publication staging collision")
    staging.mkdir()
    publish = staging / dataset_id
    shutil.copytree(directories[0], publish)
    publish_check = storage.verify_dataset(publish)
    require(publish_check["logical_sqlite_sha256"] == checks[0]["logical_sqlite_sha256"], "publication copy mismatch")
    publish.rename(destination)
    binary = runtime["binary"]["path"]
    sqlite_path = repo_relative(destination / storage.SQLITE_NAME)
    normalize = run_recorded([binary, "normalize-check", "--dataset", sqlite_path], "normalize-check.json")
    normalized = json.loads(normalize["stdout"])
    require(normalized["status"] == "pass" and normalized["expressions_checked"] == 5965 and normalized["surfaces_checked"] == 8445,
            "Rust normalization mismatch")
    observations = []
    query_commands = []
    for name, target in sorted(runtime["targets"].items()):
        command = [binary, "lookup", "--dataset", sqlite_path, "--expect-dataset-id", dataset_id,
                   "--policy", runtime["policy"]["path"], "--state", target["state_dir"], "--repeat", "2", *TERMS]
        result = run_recorded(command, f"lookup-{name}.json")
        rows = [json.loads(line) for line in result["stdout"].splitlines()]
        require([r["input"] for r in rows] == list(TERMS), "wrong smoke input population")
        for row in rows:
            require(row["dataset_id"] == dataset_id and row["policy_sha256"] == runtime["policy"]["sha256"] and
                    row["repository"]["status"] == "ready" and row["repository"]["current_generation"] == target["index_generation"],
                    "Rust consumer identity/state mismatch")
            observations.append({"workspace": name, "input": row["input"], "status": row["status"],
                                 "dictionary_relations": row["stage_counts"]["dictionary_relations"], "final_terms": row["final_terms"]})
        query_commands.append(command)
    preserved = preservation()
    new_json(AREA / "preservation-after.json", preserved)
    artifacts = [identity(p) for p in sorted(destination.rglob("*")) if p.is_file()]
    builder = repo_relative(Path(__file__))
    handoff = {"status": "ready", "candidate_only": True, "activated": False, "fresh_evaluation_performed": False,
               "dataset_id": dataset_id, "schema_version": SCHEMA_VERSION, "normalization_version": NORMALIZATION_VERSION,
               "dataset_dir": repo_relative(destination), "artifacts": artifacts, "counts": reports[0]["manifest"]["record_counts"],
               "derivation_sha256": reports[0]["derivation_sha256"], "curation_binding": identity(destination / "curation-binding.json"),
               "final_decision_identity": checked(FINAL / "effective/decisions.jsonl", DECISIONS_HASH),
               "final_handoff_identity": checked(FINAL / "final-decisions-handoff.json", FINAL_HASH),
               "curation_policy_identity": identity(FINAL / "policy.json"), "lookup_policy_identity": runtime["policy"],
               "evidence_identity": identity(FINAL / "provenance/acceptance-bindings.json"),
               "implementation_files": [identity(Path(__file__))], "filter": reports[0]["filter"],
               "round_trip_integrity": checks, "rust_normalization": normalized,
               "deterministic": {"status": "pass", "build_directories": [repo_relative(d) for d in directories],
                                 "both_builds_preserved": True, "content_id_and_derivation_match": True,
                                 "jsonl_and_sidecar_bytes_match": True, "logical_sqlite_match": True,
                                 "same_environment_sqlite_bytes_match": first_files[storage.SQLITE_NAME] == second_files[storage.SQLITE_NAME]},
               "lookup": {"status": "pass", "observations": observations, "terms": list(TERMS), "repeat": 2,
                          "semantic_accuracy_claimed": False, "repository_stats_rebuilt": False},
               "reused_validation": reused, "preservation": preserved,
               "validation_outputs": [identity(p) for p in sorted(AREA.glob("*.json"))],
               "reproduction_commands": [
                   {"cwd": ".", "command": ["python3", "-B", builder, "snapshot"]},
                   *[{"cwd": ".", "command": ["python3", "-B", builder, "build", "--name", n]} for n in (first, second)],
                   {"cwd": ".", "command": ["python3", "-B", builder, "finalize", "--first", first, "--second", second]}],
               "query_commands": [{"cwd": ".", "command": c} for c in query_commands],
               "reproduction_note": "Commands require all frozen prerequisites and absent owned output paths/content-ID directory. Existing evidence is never overwritten; use a clean copy for full reproduction.",
               "limits": ["Independent candidate-data/pipeline review required before explicit activation and fresh evaluation.",
                          "Smoke results are bounded observations, not semantic accuracy evidence.",
                          "Offline source/code paths and actual local lookups checked; no new network-denied sandbox run.",
                          "content ID identifies JSONL; derivation_sha256 separately binds frozen curation/recipe/content.",
                          "SQLite byte reproducibility is environment-specific; logical identity is portable.",
                          "Historical producer bytes not recovered/reexecuted. Source artifact hashes and round-trip verified; current helper identity pinned separately to frozen curation inventories."]}
    new_json(AREA / "candidate-handoff.json", handoff)
    return {"status": "ready", "dataset_id": dataset_id, "handoff": repo_relative(AREA / "candidate-handoff.json"),
            "counts": handoff["counts"], "preservation": preserved}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("snapshot")
    build_parser = commands.add_parser("build")
    build_parser.add_argument("--name", required=True)
    final_parser = commands.add_parser("finalize")
    final_parser.add_argument("--first", required=True)
    final_parser.add_argument("--second", required=True)
    args = parser.parse_args()
    try:
        if args.command == "snapshot":
            result = snapshot()
        elif args.command == "build":
            result = build(args.name)
        else:
            result = finalize(args.first, args.second)
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except (ValueError, KeyError, OSError, TypeError) as error:
        raise SystemExit(f"ERROR: {error}") from error


if __name__ == "__main__":
    main()
