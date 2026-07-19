#!/usr/bin/env python3
"""Seal separate scoring phases and deterministically merge 252 final judgments."""
from __future__ import annotations

import pathlib
import sys
from typing import Any

from common import HARNESS_ROOT, canonical_sha256, load_json, sha256, write_json
from generation import verify as verify_generation
from make_scoring_bundles import SCORERS, verify_phase1_seal
from validate_judgment import load_assignment, validate_phase1, validate_phase2


PHASE1_COMPUTED = {
    "score_0_100", "semantic_label", "contract_complete", "contract_label", "scorer_id", "manual_status",
}

PHASE1_SEAL_KEYS = {
    "schema_version", "phase", "generation_id", "generation_seal_sha256",
    "assignment_path", "assignment_file_sha256", "assignment_seal_sha256",
    "judgment_count", "judgments", "seal_sha256",
}
PHASE2_SEAL_KEYS = {
    "schema_version", "phase", "generation_id", "generation_seal_sha256",
    "assignment_path", "assignment_file_sha256", "assignment_seal_sha256",
    "phase1_seal_path", "phase1_seal_file_sha256", "phase1_seal_sha256",
    "process_bundle_bindings_by_scorer", "judgment_count", "judgments", "seal_sha256",
}
FINAL_SEAL_KEYS = {
    "schema_version", "input_contract", "phase", "generation_id",
    "generation_seal_sha256", "assignment_path", "assignment_file_sha256",
    "assignment_seal_sha256", "ledger_path", "ledger_sha256",
    "bundle_sha256_by_scorer", "phase1_seal_path",
    "phase1_seal_file_sha256", "phase1_seal_sha256", "phase2_seal_path",
    "phase2_seal_file_sha256", "phase2_seal_sha256", "process_bundle_bindings_by_scorer", "judgment_count",
    "judgments", "seal_sha256",
}
PHASE_JUDGMENT_KEYS = {"path", "sha256", "scorer_id", "session_id"}
PROCESS_BUNDLE_BINDING_KEYS = {"path", "sha256", "mode"}
FINAL_JUDGMENT_KEYS = {
    "path", "sha256", "scorer_id", "session_id", "run_id",
    "phase1_sha256", "phase2_sha256",
}


def lock_tree(root: pathlib.Path) -> None:
    for path in sorted([root, *root.rglob("*")], reverse=True):
        if not path.is_symlink():
            path.chmod(path.stat().st_mode & ~0o222)


def exact_keys(value: Any, expected: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != expected:
        actual = sorted(value) if isinstance(value, dict) else type(value).__name__
        raise SystemExit(f"{label} keys mismatch: {actual}")


def exact_file(raw_path: Any, expected: pathlib.Path, label: str, *, read_only: bool = True) -> pathlib.Path:
    if not isinstance(raw_path, str):
        raise SystemExit(f"{label} path must be an absolute string")
    expected = expected.resolve()
    path = pathlib.Path(raw_path)
    if (
        not path.is_absolute()
        or str(path) != str(expected)
        or path.is_symlink()
        or not path.is_file()
        or path.resolve() != expected
    ):
        raise SystemExit(f"{label} path is not the exact expected non-symlink file")
    if read_only and path.stat().st_mode & 0o222:
        raise SystemExit(f"{label} remains writable")
    return path


def recorded_file(raw_path: Any, label: str, *, read_only: bool = True) -> pathlib.Path:
    if not isinstance(raw_path, str):
        raise SystemExit(f"{label} path must be an absolute string")
    path = pathlib.Path(raw_path)
    if not path.is_absolute() or str(path) != str(path.resolve()):
        raise SystemExit(f"{label} path must be canonical and may not traverse a symlink")
    return exact_file(raw_path, path, label, read_only=read_only)


def process_bundle_paths(generation_id: str) -> dict[str, pathlib.Path]:
    return {
        scorer_id: HARNESS_ROOT / "scoring" / generation_id / "phase2-process" / scorer_id / "bundle.json"
        for scorer_id in SCORERS
    }


def phase2_process_bundle_bindings(generation_id: str) -> dict[str, dict[str, Any]]:
    bindings = {}
    for scorer_id, expected_path in process_bundle_paths(generation_id).items():
        path = exact_file(str(expected_path), expected_path, f"phase-2 process bundle {scorer_id}")
        mode = path.stat().st_mode & 0o777
        if mode != 0o444:
            raise SystemExit(f"phase-2 process bundle mode must be 0444: {scorer_id}")
        bindings[scorer_id] = {"path": str(path), "sha256": sha256(path), "mode": mode}
    return bindings


def verify_phase2_process_bundle_bindings(value: Any, generation_id: str) -> dict[str, dict[str, Any]]:
    if not isinstance(value, dict) or set(value) != set(SCORERS):
        raise SystemExit("phase-2 process bundle scorer set mismatch")
    expected_paths = process_bundle_paths(generation_id)
    for scorer_id in SCORERS:
        binding = value[scorer_id]
        exact_keys(binding, PROCESS_BUNDLE_BINDING_KEYS, f"phase-2 process bundle binding {scorer_id}")
        path = exact_file(binding.get("path"), expected_paths[scorer_id], f"phase-2 process bundle {scorer_id}")
        mode = path.stat().st_mode & 0o777
        if binding.get("mode") != 0o444 or mode != binding["mode"]:
            raise SystemExit(f"phase-2 process bundle mode mismatch: {scorer_id}")
        if sha256(path) != binding.get("sha256"):
            raise SystemExit(f"phase-2 process bundle hash mismatch: {scorer_id}")
    return value


def expected_paths(root: pathlib.Path, assignment: dict) -> dict[str, pathlib.Path]:
    expected = {
        row["review_id"]: root / row["scorer_id"] / f"{row['review_id']}.json"
        for row in assignment["assignments"]
    }
    actual = {path.resolve() for path in root.rglob("*.json")} if root.is_dir() else set()
    wanted = {path.resolve() for path in expected.values()}
    if actual != wanted:
        missing = sorted(str(path) for path in wanted - actual)
        extra = sorted(str(path) for path in actual - wanted)
        raise SystemExit(f"judgment file set mismatch: missing={missing[:3]} extra={extra[:3]}")
    return expected


def raw_phase1(validated: dict) -> dict:
    value = dict(validated)
    if value.pop("phase", None) != "correctness":
        raise SystemExit("validated phase-1 file has the wrong phase")
    value.pop("assignment_seal_sha256", None)
    correctness = dict(value["correctness"])
    for key in PHASE1_COMPUTED:
        correctness.pop(key, None)
    value["correctness"] = correctness
    return value


def raw_phase2(validated: dict) -> dict:
    value = dict(validated)
    if value.pop("phase", None) != "process":
        raise SystemExit("validated phase-2 file has the wrong phase")
    value.pop("assignment_seal_sha256", None)
    return value


def seal_value(value: dict) -> dict:
    result = dict(value)
    result["seal_sha256"] = canonical_sha256(result)
    return result


def write_seal(path: pathlib.Path, value: dict) -> None:
    if path.exists():
        raise SystemExit("score seal is immutable and may not be overwritten")
    write_json(path, seal_value(value))
    path.chmod(path.stat().st_mode & ~0o222)


def verify_common(generation_path: pathlib.Path, assignment_path: pathlib.Path) -> tuple[dict, dict]:
    verify_generation(generation_path)
    generation = load_json(generation_path)
    assignment = load_assignment(assignment_path, HARNESS_ROOT / "runs" / generation["generation_id"])
    if assignment["generation_id"] != generation["generation_id"] or assignment["generation_seal_sha256"] != generation["generation_seal_sha256"]:
        raise SystemExit("scoring assignment/generation mismatch")
    return generation, assignment


def verify_phase1_binding(path: pathlib.Path, assignment_path: pathlib.Path, assignment: dict) -> dict:
    seal = verify_phase1_seal(path, assignment)
    exact_keys(seal, PHASE1_SEAL_KEYS, "phase-1 seal")
    if seal.get("schema_version") != 1 or seal.get("judgment_count") != 252:
        raise SystemExit("phase-1 score seal version/count mismatch")
    assignment_file = exact_file(
        seal.get("assignment_path"), assignment_path, "phase-1 assignment",
    )
    if sha256(assignment_file) != seal.get("assignment_file_sha256"):
        raise SystemExit("phase-1 assignment file hash mismatch")
    rows = {row["review_id"]: row for row in assignment["assignments"]}
    hashes = []
    for review_id, item in seal["judgments"].items():
        exact_keys(item, PHASE_JUDGMENT_KEYS, f"phase-1 judgment record {review_id}")
        row = rows[review_id]
        expected_path = path.resolve().parent / row["scorer_id"] / f"{review_id}.json"
        judgment_path = exact_file(item.get("path"), expected_path, f"phase-1 judgment {review_id}")
        if (
            item.get("scorer_id") != row["scorer_id"]
            or item.get("session_id") != row["session_id"]
            or sha256(judgment_path) != item.get("sha256")
        ):
            raise SystemExit(f"phase-1 judgment identity/hash mismatch: {review_id}")
        value = load_json(judgment_path)
        if validate_phase1(raw_phase1(value), assignment) != value:
            raise SystemExit(f"phase-1 judgment content drift: {review_id}")
        hashes.append(item["sha256"])
    if len(hashes) != 252 or len(set(hashes)) != 252:
        raise SystemExit("phase-1 seal must bind 252 distinct judgment files")
    return seal


def seal_phase1(generation_path: pathlib.Path, assignment_path: pathlib.Path, root: pathlib.Path, output: pathlib.Path) -> int:
    generation, assignment = verify_common(generation_path, assignment_path)
    if output.resolve().parent != root.resolve() or output.name != "phase1-seal.json":
        raise SystemExit("phase-1 seal must be INPUT_ROOT/phase1-seal.json")
    paths = expected_paths(root, assignment)
    judgments = {}
    hashes = []
    rows = {row["review_id"]: row for row in assignment["assignments"]}
    for review_id, path in sorted(paths.items()):
        value = load_json(path)
        if validate_phase1(raw_phase1(value), assignment) != value:
            raise SystemExit(f"phase-1 validated output drift: {review_id}")
        row = rows[review_id]
        digest = sha256(path)
        hashes.append(digest)
        judgments[review_id] = {
            "path": str(path.resolve()), "sha256": digest,
            "scorer_id": row["scorer_id"], "session_id": row["session_id"],
        }
    if len(set(hashes)) != 252:
        raise SystemExit("one phase-1 judgment file/hash cannot count twice")
    write_seal(output, {
        "schema_version": 1, "phase": "correctness",
        "generation_id": generation["generation_id"],
        "generation_seal_sha256": generation["generation_seal_sha256"],
        "assignment_path": str(assignment_path.resolve()),
        "assignment_file_sha256": sha256(assignment_path),
        "assignment_seal_sha256": assignment["assignment_seal_sha256"],
        "judgment_count": 252, "judgments": judgments,
    })
    lock_tree(root)
    return 0


def verify_phase2_seal(
    path: pathlib.Path,
    assignment_path: pathlib.Path,
    assignment: dict,
    phase1_seal_path: pathlib.Path,
    phase1_seal: dict,
) -> dict:
    seal = load_json(path)
    exact_keys(seal, PHASE2_SEAL_KEYS, "phase-2 seal")
    core = dict(seal)
    recorded = core.pop("seal_sha256", None)
    if recorded != canonical_sha256(core):
        raise SystemExit("phase-2 score seal self-hash mismatch")
    if (
        seal.get("schema_version") != 1
        or seal.get("phase") != "process"
        or seal.get("generation_id") != assignment["generation_id"]
        or seal.get("generation_seal_sha256") != assignment["generation_seal_sha256"]
        or seal.get("assignment_seal_sha256") != assignment["assignment_seal_sha256"]
        or seal.get("judgment_count") != 252
    ):
        raise SystemExit("phase-2 score seal/assignment mismatch")
    assignment_file = exact_file(
        seal.get("assignment_path"), assignment_path, "phase-2 assignment",
    )
    if sha256(assignment_file) != seal.get("assignment_file_sha256"):
        raise SystemExit("phase-2 assignment file hash mismatch")
    bound_phase1 = exact_file(
        seal.get("phase1_seal_path"), phase1_seal_path, "phase-2 bound phase-1 seal",
    )
    if sha256(bound_phase1) != seal.get("phase1_seal_file_sha256"):
        raise SystemExit("phase-2 phase-1 seal file hash mismatch")
    if seal.get("phase1_seal_sha256") != phase1_seal["seal_sha256"]:
        raise SystemExit("phase-2 score seal is not bound to phase 1")
    verify_phase2_process_bundle_bindings(
        seal.get("process_bundle_bindings_by_scorer"), assignment["generation_id"],
    )
    judgments = seal.get("judgments", {})
    expected = {row["review_id"] for row in assignment["assignments"]}
    if set(judgments) != expected or len(judgments) != 252:
        raise SystemExit("phase-2 seal must contain every assigned review exactly once")
    hashes = []
    rows = {row["review_id"]: row for row in assignment["assignments"]}
    for review_id, item in judgments.items():
        exact_keys(item, PHASE_JUDGMENT_KEYS, f"phase-2 judgment record {review_id}")
        row = rows[review_id]
        expected_path = path.resolve().parent / row["scorer_id"] / f"{review_id}.json"
        judgment_path = exact_file(item.get("path"), expected_path, f"phase-2 judgment {review_id}")
        if (
            item.get("scorer_id") != row["scorer_id"]
            or item.get("session_id") != row["session_id"]
            or sha256(judgment_path) != item.get("sha256")
        ):
            raise SystemExit(f"phase-2 score seal mismatch: {review_id}")
        value = load_json(judgment_path)
        if validate_phase2(raw_phase2(value), assignment, phase1_seal) != value:
            raise SystemExit(f"phase-2 judgment content drift: {review_id}")
        hashes.append(item["sha256"])
    if len(set(hashes)) != 252:
        raise SystemExit("one phase-2 judgment file/hash cannot count twice")
    return seal


def seal_phase2(
    generation_path: pathlib.Path, assignment_path: pathlib.Path, phase1_seal_path: pathlib.Path,
    root: pathlib.Path, output: pathlib.Path,
) -> int:
    generation, assignment = verify_common(generation_path, assignment_path)
    if output.resolve().parent != root.resolve() or output.name != "phase2-seal.json":
        raise SystemExit("phase-2 seal must be INPUT_ROOT/phase2-seal.json")
    phase1_seal = verify_phase1_binding(phase1_seal_path, assignment_path, assignment)
    paths = expected_paths(root, assignment)
    judgments = {}
    hashes = []
    rows = {row["review_id"]: row for row in assignment["assignments"]}
    for review_id, path in sorted(paths.items()):
        value = load_json(path)
        if validate_phase2(raw_phase2(value), assignment, phase1_seal) != value:
            raise SystemExit(f"phase-2 validated output drift: {review_id}")
        if value["phase1_judgment_sha256"] != phase1_seal["judgments"][review_id]["sha256"]:
            raise SystemExit("phase-2 attempted to replace its sealed phase-1 score")
        row = rows[review_id]
        digest = sha256(path)
        hashes.append(digest)
        judgments[review_id] = {
            "path": str(path.resolve()), "sha256": digest,
            "scorer_id": row["scorer_id"], "session_id": row["session_id"],
        }
    if len(set(hashes)) != 252:
        raise SystemExit("one phase-2 judgment file/hash cannot count twice")
    write_seal(output, {
        "schema_version": 1, "phase": "process",
        "generation_id": generation["generation_id"],
        "generation_seal_sha256": generation["generation_seal_sha256"],
        "assignment_path": str(assignment_path.resolve()),
        "assignment_file_sha256": sha256(assignment_path),
        "assignment_seal_sha256": assignment["assignment_seal_sha256"],
        "phase1_seal_path": str(phase1_seal_path.resolve()),
        "phase1_seal_file_sha256": sha256(phase1_seal_path),
        "phase1_seal_sha256": phase1_seal["seal_sha256"],
        "process_bundle_bindings_by_scorer": phase2_process_bundle_bindings(generation["generation_id"]),
        "judgment_count": 252, "judgments": judgments,
    })
    lock_tree(root)
    return 0


def validate_final(value: dict, row: dict, phase1: dict, phase2: dict) -> None:
    expected = {
        "schema_version": 1,
        "blind_output_id": row["review_id"],
        "scorer_id": row["scorer_id"],
        "rubric_sha256": row["rubric_sha256"],
        "correctness": phase1["correctness"],
        "dialogue": phase2["dialogue"],
        "notes": f"Correctness phase: {phase1['notes']}\nProcess phase: {phase2['notes']}",
    }
    if value != expected:
        raise SystemExit("final judgment is not an exact deterministic phase merge")


def verify_final_manifest(
    path: pathlib.Path,
    generation_path: pathlib.Path,
    assignment_path: pathlib.Path,
) -> dict:
    """Verify every identity, file, and hash in one immutable final scoring result."""
    generation, assignment = verify_common(generation_path, assignment_path)
    path = exact_file(str(path), path.resolve(), "final scoring manifest")
    if path.name != "final-seal.json":
        raise SystemExit("final scoring manifest must be named final-seal.json")
    seal = load_json(path)
    exact_keys(seal, FINAL_SEAL_KEYS, "final scoring manifest")
    core = dict(seal)
    recorded = core.pop("seal_sha256", None)
    if recorded != canonical_sha256(core):
        raise SystemExit("final scoring manifest self-hash mismatch")
    if (
        seal.get("schema_version") != 1
        or seal.get("input_contract") != "baseline-scoring-final-manifest-v1"
        or seal.get("phase") != "final-merged"
        or seal.get("generation_id") != generation["generation_id"]
        or seal.get("generation_seal_sha256") != generation["generation_seal_sha256"]
        or seal.get("assignment_seal_sha256") != assignment["assignment_seal_sha256"]
        or seal.get("judgment_count") != 252
    ):
        raise SystemExit("final scoring manifest generation/assignment/count mismatch")

    assignment_file = exact_file(
        seal.get("assignment_path"), assignment_path, "final scoring assignment",
    )
    if sha256(assignment_file) != seal.get("assignment_file_sha256"):
        raise SystemExit("final scoring assignment file hash mismatch")
    ledger_path = exact_file(
        seal.get("ledger_path"), pathlib.Path(assignment["ledger_path"]), "final scoring ledger",
    )
    if (
        seal.get("ledger_path") != assignment["ledger_path"]
        or seal.get("ledger_sha256") != assignment["ledger_sha256"]
        or sha256(ledger_path) != seal.get("ledger_sha256")
    ):
        raise SystemExit("final scoring ledger binding mismatch")

    rows = {row["review_id"]: row for row in assignment["assignments"]}
    scorer_ids = sorted({row["scorer_id"] for row in rows.values()})
    if len(scorer_ids) != 3:
        raise SystemExit("final scoring manifest requires exactly three scorer ids")
    bundle_hashes = seal.get("bundle_sha256_by_scorer")
    if not isinstance(bundle_hashes, dict) or set(bundle_hashes) != set(scorer_ids):
        raise SystemExit("final scoring bundle scorer set mismatch")
    for scorer_id in scorer_ids:
        scorer_rows = [row for row in rows.values() if row["scorer_id"] == scorer_id]
        paths = {row["bundle_path"] for row in scorer_rows}
        hashes = {row["bundle_sha256"] for row in scorer_rows}
        if len(paths) != 1 or len(hashes) != 1:
            raise SystemExit(f"assignment has inconsistent bundle binding for {scorer_id}")
        bundle_path = recorded_file(next(iter(paths)), f"public bundle {scorer_id}")
        expected_hash = next(iter(hashes))
        if sha256(bundle_path) != expected_hash or bundle_hashes[scorer_id] != expected_hash:
            raise SystemExit(f"final scoring bundle hash mismatch for {scorer_id}")

    phase1_path = recorded_file(seal.get("phase1_seal_path"), "final bound phase-1 seal")
    if sha256(phase1_path) != seal.get("phase1_seal_file_sha256"):
        raise SystemExit("final phase-1 seal file hash mismatch")
    phase1 = verify_phase1_binding(phase1_path, assignment_path, assignment)
    if phase1.get("seal_sha256") != seal.get("phase1_seal_sha256"):
        raise SystemExit("final phase-1 internal seal mismatch")

    phase2_path = recorded_file(seal.get("phase2_seal_path"), "final bound phase-2 seal")
    if sha256(phase2_path) != seal.get("phase2_seal_file_sha256"):
        raise SystemExit("final phase-2 seal file hash mismatch")
    phase2 = verify_phase2_seal(
        phase2_path, assignment_path, assignment, phase1_path, phase1,
    )
    if phase2.get("seal_sha256") != seal.get("phase2_seal_sha256"):
        raise SystemExit("final phase-2 internal seal mismatch")
    if seal.get("process_bundle_bindings_by_scorer") != phase2.get("process_bundle_bindings_by_scorer"):
        raise SystemExit("final process bundle binding differs from phase-2 seal")
    verify_phase2_process_bundle_bindings(
        seal.get("process_bundle_bindings_by_scorer"), generation["generation_id"],
    )

    judgments = seal.get("judgments")
    if not isinstance(judgments, dict) or set(judgments) != set(rows) or len(judgments) != 252:
        raise SystemExit("final scoring manifest must contain every current review exactly once")
    hashes = []
    for review_id, item in judgments.items():
        exact_keys(item, FINAL_JUDGMENT_KEYS, f"final judgment record {review_id}")
        row = rows[review_id]
        expected_path = path.parent / row["scorer_id"] / f"{review_id}.json"
        judgment_path = exact_file(item.get("path"), expected_path, f"final judgment {review_id}")
        if (
            item.get("scorer_id") != row["scorer_id"]
            or item.get("session_id") != row["session_id"]
            or item.get("run_id") != row["run_id"]
            or item.get("phase1_sha256") != phase1["judgments"][review_id]["sha256"]
            or item.get("phase2_sha256") != phase2["judgments"][review_id]["sha256"]
            or sha256(judgment_path) != item.get("sha256")
        ):
            raise SystemExit(f"final judgment identity/hash mismatch: {review_id}")
        validate_final(
            load_json(judgment_path), row,
            load_json(pathlib.Path(phase1["judgments"][review_id]["path"])),
            load_json(pathlib.Path(phase2["judgments"][review_id]["path"])),
        )
        hashes.append(item["sha256"])
    if len(set(hashes)) != 252:
        raise SystemExit("one final judgment file/hash cannot count twice")
    if any(item.stat().st_mode & 0o222 for item in [path.parent, *path.parent.rglob("*")] if not item.is_symlink()):
        raise SystemExit("final scoring output tree remains writable")
    return seal


def merge(
    generation_path: pathlib.Path, assignment_path: pathlib.Path, phase1_seal_path: pathlib.Path,
    phase2_seal_path: pathlib.Path, output_root: pathlib.Path,
) -> int:
    generation, assignment = verify_common(generation_path, assignment_path)
    phase1_seal = verify_phase1_binding(phase1_seal_path, assignment_path, assignment)
    phase2_seal = verify_phase2_seal(
        phase2_seal_path, assignment_path, assignment, phase1_seal_path, phase1_seal,
    )
    if output_root.exists():
        raise SystemExit("final judgment directory is immutable and may not be overwritten")
    rows = {row["review_id"]: row for row in assignment["assignments"]}
    judgments = {}
    hashes = []
    for review_id, row in sorted(rows.items()):
        phase1 = load_json(pathlib.Path(phase1_seal["judgments"][review_id]["path"]))
        phase2 = load_json(pathlib.Path(phase2_seal["judgments"][review_id]["path"]))
        value = {
            "schema_version": 1,
            "blind_output_id": review_id,
            "scorer_id": row["scorer_id"],
            "rubric_sha256": row["rubric_sha256"],
            "correctness": phase1["correctness"],
            "dialogue": phase2["dialogue"],
            "notes": f"Correctness phase: {phase1['notes']}\nProcess phase: {phase2['notes']}",
        }
        validate_final(value, row, phase1, phase2)
        path = output_root / row["scorer_id"] / f"{review_id}.json"
        write_json(path, value)
        digest = sha256(path)
        hashes.append(digest)
        judgments[review_id] = {
            "path": str(path.resolve()), "sha256": digest,
            "scorer_id": row["scorer_id"], "session_id": row["session_id"], "run_id": row["run_id"],
            "phase1_sha256": phase1_seal["judgments"][review_id]["sha256"],
            "phase2_sha256": phase2_seal["judgments"][review_id]["sha256"],
        }
    if len(set(hashes)) != 252:
        raise SystemExit("one final judgment file/hash cannot count twice")
    final_seal_path = output_root / "final-seal.json"
    write_seal(final_seal_path, {
        "schema_version": 1, "input_contract": "baseline-scoring-final-manifest-v1", "phase": "final-merged",
        "generation_id": generation["generation_id"],
        "generation_seal_sha256": generation["generation_seal_sha256"],
        "assignment_path": str(assignment_path.resolve()),
        "assignment_file_sha256": sha256(assignment_path),
        "assignment_seal_sha256": assignment["assignment_seal_sha256"],
        "ledger_path": assignment["ledger_path"],
        "ledger_sha256": assignment["ledger_sha256"],
        "bundle_sha256_by_scorer": {
            scorer: next(row["bundle_sha256"] for row in assignment["assignments"] if row["scorer_id"] == scorer)
            for scorer in sorted({row["scorer_id"] for row in assignment["assignments"]})
        },
        "phase1_seal_path": str(phase1_seal_path.resolve()),
        "phase1_seal_file_sha256": sha256(phase1_seal_path),
        "phase1_seal_sha256": phase1_seal["seal_sha256"],
        "phase2_seal_path": str(phase2_seal_path.resolve()),
        "phase2_seal_file_sha256": sha256(phase2_seal_path),
        "phase2_seal_sha256": phase2_seal["seal_sha256"],
        "process_bundle_bindings_by_scorer": phase2_seal["process_bundle_bindings_by_scorer"],
        "judgment_count": 252, "judgments": judgments,
    })
    lock_tree(output_root)
    verify_final_manifest(final_seal_path, generation_path, assignment_path)
    return 0


def main() -> int:
    args = sys.argv[2:]
    if len(sys.argv) == 6 and sys.argv[1] == "seal-phase1":
        return seal_phase1(*map(pathlib.Path, args))
    if len(sys.argv) == 7 and sys.argv[1] == "seal-phase2":
        return seal_phase2(*map(pathlib.Path, args))
    if len(sys.argv) == 7 and sys.argv[1] == "merge":
        return merge(*map(pathlib.Path, args))
    raise SystemExit(
        "usage: scoring_pipeline.py seal-phase1 GENERATION ASSIGNMENTS INPUT_ROOT OUTPUT_SEAL | "
        "seal-phase2 GENERATION ASSIGNMENTS PHASE1_SEAL INPUT_ROOT OUTPUT_SEAL | "
        "merge GENERATION ASSIGNMENTS PHASE1_SEAL PHASE2_SEAL OUTPUT_ROOT"
    )


if __name__ == "__main__":
    raise SystemExit(main())
