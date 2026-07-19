#!/usr/bin/env python3
"""Synthetic no-model test of 84-output/252-judgment blind scoring contracts."""
from __future__ import annotations

import copy
import json
import pathlib
import shutil
import tempfile

import make_scoring_bundles
import scoring_pipeline
import validate_judgment
from common import BENCHMARK_ROOT, HARNESS_ROOT, canonical_sha256, load_json, sha256, write_json
from selftest_contract import SCORING_REQUIRED_CHECKS


def make_assignment(root: pathlib.Path) -> tuple[pathlib.Path, dict]:
    rubric_path = HARNESS_ROOT / "config/scoring-rubric.json"
    rubric = load_json(rubric_path)
    rubric_sha = sha256(rubric_path)
    answer_path = BENCHMARK_ROOT / "answers/development/API-01.json"
    answer = load_json(answer_path)
    answer_sha = sha256(answer_path)
    scoring_items = make_scoring_bundles.derive_scoring_items(answer, answer_sha, rubric)
    bundle_paths = {}
    for scorer in make_scoring_bundles.SCORERS:
        path = root / "bundles" / scorer / "bundle.json"
        write_json(path, {"schema_version": 1, "scorer_id": scorer, "synthetic": True})
        path.chmod(0o444)
        bundle_paths[scorer] = path
    rows = []
    runs_root = root / "runs" / "synthetic-generation"
    ledger_slots = {}
    ledger_attempts = {}
    for session_index in range(84):
        session_id = f"session-{session_index:03d}"
        run_id = f"run-{session_index:03d}"
        run_dir = runs_root / run_id
        write_json(run_dir / "wrapper.json", {"reducer_lines_accepted": 3})
        metric_path = runs_root / "automatic-metrics" / run_id / "automatic-run-metrics.json"
        completed_calls = [
            {
                "identity": f"session:tool:{run_id}-call-1", "session_id": "session",
                "call_id": f"{run_id}-call-1", "family": "read", "completed": True,
                "raw_event_lines": [1], "selected_completion_line": 1, "completed_call_index": 1,
            },
            {
                "identity": f"session:tool:{run_id}-call-2", "session_id": "session",
                "call_id": f"{run_id}-call-2", "family": "other", "completed": True,
                "raw_event_lines": [2], "selected_completion_line": 2, "completed_call_index": 2,
            },
        ]
        write_json(metric_path, {"run": {
            "run_id": run_id, "completed_tool_calls": completed_calls,
            "incomplete_tool_calls": [{
                "identity": f"session:tool:{run_id}-call-3", "session_id": "session",
                "call_id": f"{run_id}-call-3", "family": "read", "completed": False,
                "raw_event_lines": [3], "selected_completion_line": None, "completed_call_index": None,
            }],
        }})
        metric_path.chmod(0o444)
        metric_hash = sha256(metric_path)
        trial_id = f"synthetic-{session_index:03d}"
        slot_key = f"API-01:{trial_id}:B1"
        ledger_slots[slot_key] = {
            "latest_run_id": run_id, "latest_attempt_number": 1,
            "measurement_status": "valid", "replacement_allowed": False,
            "metrics_status": "sealed", "all_run_ids": [run_id],
            "latest_automatic_metrics_path": str(metric_path),
            "latest_automatic_metrics_sha256": metric_hash,
        }
        ledger_attempts[run_id] = {
            "state": "terminal", "run_dir": str(run_dir),
            "automatic_metrics_path": str(metric_path), "automatic_metrics_sha256": metric_hash,
        }
        for scorer_slot, scorer in enumerate(make_scoring_bundles.SCORERS, 1):
            review_id = f"review-{session_index:03d}-{scorer_slot}"
            rows.append({
                "review_id": review_id, "session_id": session_id,
                "run_id": run_id,
                "scorer_id": scorer, "scorer_slot": scorer_slot,
                "run_dir": str(run_dir), "task_id": "API-01",
                "trial_id": trial_id, "arm": "B1", "pair_id": f"pair-{session_index:03d}",
                "pair_order_index": 0, "answer_contract_sha256": answer_sha,
                "scoring_items": scoring_items, "rubric_sha256": rubric_sha,
                "bundle_path": str(bundle_paths[scorer]), "bundle_sha256": sha256(bundle_paths[scorer]),
            })
    ledger_path = runs_root / "ledger.json"
    write_json(ledger_path, {
        "generation_id": "synthetic-generation", "generation_seal_sha256": "synthetic-generation-seal",
        "state": "completed", "slots": ledger_slots, "attempts": ledger_attempts,
    })
    ledger_path.chmod(0o444)
    assignment = {
        "schema_version": 1, "generation_id": "synthetic-generation",
        "generation_seal_sha256": "synthetic-generation-seal",
        "expected_session_count": 84, "expected_judgment_count": 252,
        "rubric_sha256": rubric_sha,
        "runs_root": str(runs_root), "ledger_path": str(ledger_path), "ledger_sha256": sha256(ledger_path),
        "assignments": rows,
    }
    assignment["assignment_seal_sha256"] = canonical_sha256(assignment)
    path = root / "assignments.json"
    write_json(path, assignment)
    path.chmod(0o444)
    return path, assignment


def phase1_input(row: dict) -> dict:
    items = row["scoring_items"]
    return {
        "schema_version": 1, "review_id": row["review_id"], "scorer_id": row["scorer_id"],
        "rubric_sha256": row["rubric_sha256"],
        "correctness": {
            "core_fraction": 1, "core_evidence": f"central answer evidence {row['review_id']}",
            "required_claims": [
                {"item_id": item["item_id"], "fraction": 1, "evidence": f"claim evidence {row['review_id']} {item['item_id']}", "raw_event_lines": []}
                for item in items["required_claims"]
            ],
            "grounding_fraction": 1, "grounding_evidence": f"file and symbol evidence {row['review_id']}",
            "required_relationships": [
                {"item_id": item["item_id"], "fraction": 1, "evidence": f"relationship evidence {row['review_id']} {item['item_id']}", "raw_event_lines": []}
                for item in items["required_relationships"]
            ],
            "prohibited_claims": [
                {"item_id": item["item_id"], "explicitly_present": False, "evidence": f"not asserted {row['review_id']} {item['item_id']}", "raw_event_lines": []}
                for item in items["prohibited_claims"]
            ],
            "format_valid_json": False,
        },
        "notes": f"phase1 {row['review_id']}",
    }


def phase2_input(row: dict, phase1_sha: str) -> dict:
    stage = {"status": "yes", "evidence": f"event evidence {row['review_id']}", "raw_event_lines": [1], "call_ids": [f"{row['run_id']}-call-1"]}
    return {
        "schema_version": 1, "review_id": row["review_id"], "scorer_id": row["scorer_id"],
        "rubric_sha256": row["rubric_sha256"], "phase1_judgment_sha256": phase1_sha,
        "dialogue": {
            "discovery_stages": {name: dict(stage) for name in validate_judgment.STAGE_NAMES},
            "first_wrong": {"category": "none", "raw_event_line": None, "completed_call_index": None, "call_id": None, "explanation": "No first wrong turn."},
        },
        "notes": f"phase2 {row['review_id']}",
    }


def expect_rejected(function, *args) -> bool:
    try:
        function(*args)
        return False
    except (SystemExit, KeyError, TypeError, ValueError):
        return True


def make_writable(root: pathlib.Path) -> None:
    if not root.exists():
        return
    for path in [root, *root.rglob("*")]:
        if not path.is_symlink():
            try:
                path.chmod(path.stat().st_mode | 0o200)
            except FileNotFoundError:
                pass


def main() -> int:
    root = pathlib.Path(tempfile.mkdtemp(prefix="scoring-selftest-", dir=HARNESS_ROOT / "synthetic"))
    checks = {}
    original_verify = scoring_pipeline.verify_generation
    original_pipeline_harness_root = scoring_pipeline.HARNESS_ROOT
    scoring_pipeline.verify_generation = lambda _: None
    scoring_pipeline.HARNESS_ROOT = root
    try:
        generation_path = root / "generation.json"
        write_json(generation_path, {"generation_id": "synthetic-generation", "generation_seal_sha256": "synthetic-generation-seal"})
        assignment_path, assignment = make_assignment(root)
        loaded_assignment = validate_judgment.load_assignment(assignment_path, root / "runs" / "synthetic-generation")
        checks["exact-84x3-assignment-loads"] = len(loaded_assignment["assignments"]) == 252
        checks["active-scoring-schemas-do-not-reference-legacy-session-schema"] = all(
            "session-metrics.schema.json" not in (HARNESS_ROOT / relative).read_text(encoding="utf-8")
            for relative in (
                "schemas/judgment.schema.json",
                "schemas/phase1-correctness-judgment.schema.json",
                "schemas/phase2-process-judgment.schema.json",
                "schemas/manual-evaluation-components.schema.json",
            )
        )
        first_items = loaded_assignment["assignments"][0]["scoring_items"]
        checks["derived-item-ids-are-exact-and-ordered"] = (
            [item["item_id"] for item in first_items["required_claims"]] == [f"required_claim:{index}" for index in range(len(first_items["required_claims"]))]
            and [item["item_id"] for item in first_items["required_relationships"]] == [f"relationship:{index}" for index in range(len(first_items["required_relationships"]))]
            and [item["item_id"] for item in first_items["prohibited_claims"]] == [f"prohibited:{index}" for index in range(len(first_items["prohibited_claims"]))]
        )
        first_row = assignment["assignments"][0]
        valid_phase1 = phase1_input(first_row)
        checks["wrong-scorer-rejected"] = expect_rejected(
            validate_judgment.validate_phase1,
            {**valid_phase1, "scorer_id": "scorer-2"}, assignment,
        )
        duplicate_item = copy.deepcopy(valid_phase1)
        duplicate_item["correctness"]["required_claims"][1]["item_id"] = duplicate_item["correctness"]["required_claims"][0]["item_id"]
        checks["duplicate-answer-item-id-rejected"] = expect_rejected(validate_judgment.validate_phase1, duplicate_item, assignment)
        phase1_with_process = {**valid_phase1, "dialogue": {}}
        checks["phase1-process-field-rejected"] = expect_rejected(validate_judgment.validate_phase1, phase1_with_process, assignment)
        phase1_with_raw_line = copy.deepcopy(valid_phase1)
        phase1_with_raw_line["correctness"]["required_claims"][0]["raw_event_lines"] = [1]
        checks["phase1-undisclosed-event-citation-rejected"] = expect_rejected(validate_judgment.validate_phase1, phase1_with_raw_line, assignment)
        checks["blind-identity-key-rejected"] = expect_rejected(make_scoring_bundles.assert_blinded, {"arm": "B1"}, set())
        checks["blind-run-path-rejected"] = expect_rejected(
            make_scoring_bundles.assert_blinded,
            {"answer": str(HARNESS_ROOT / "runs/generation/run-id/source/file.ts")}, {"run-id"},
        )

        tampered = copy.deepcopy(assignment)
        tampered["assignments"][0]["scoring_items"]["required_claims"][0]["text"] += " tampered"
        tampered_core = dict(tampered["assignments"][0]["scoring_items"])
        tampered_core.pop("scoring_items_sha256")
        tampered["assignments"][0]["scoring_items"]["scoring_items_sha256"] = canonical_sha256(tampered_core)
        tampered.pop("assignment_seal_sha256")
        tampered["assignment_seal_sha256"] = canonical_sha256(tampered)
        tampered_path = root / "tampered-assignment.json"
        write_json(tampered_path, tampered)
        checks["derived-text-hash-tamper-rejected"] = expect_rejected(validate_judgment.load_assignment, tampered_path, root / "runs" / "synthetic-generation")

        duplicate_review = copy.deepcopy(assignment)
        duplicate_review["assignments"][1]["review_id"] = duplicate_review["assignments"][0]["review_id"]
        duplicate_review.pop("assignment_seal_sha256")
        duplicate_review["assignment_seal_sha256"] = canonical_sha256(duplicate_review)
        duplicate_path = root / "duplicate-assignment.json"
        write_json(duplicate_path, duplicate_review)
        checks["duplicate-review-id-rejected"] = expect_rejected(validate_judgment.load_assignment, duplicate_path, root / "runs" / "synthetic-generation")

        wrong_parent = copy.deepcopy(assignment)
        wrong_parent_dir = root / "wrong-parent" / wrong_parent["assignments"][0]["run_id"]
        write_json(wrong_parent_dir / "wrapper.json", {"reducer_lines_accepted": 3})
        wrong_parent["assignments"][0]["run_dir"] = str(wrong_parent_dir)
        wrong_parent.pop("assignment_seal_sha256")
        wrong_parent["assignment_seal_sha256"] = canonical_sha256(wrong_parent)
        wrong_parent_path = root / "wrong-parent-assignment.json"
        write_json(wrong_parent_path, wrong_parent)
        checks["same-basename-wrong-parent-run-dir-rejected"] = expect_rejected(
            validate_judgment.load_assignment, wrong_parent_path, root / "runs" / "synthetic-generation",
        )

        phase1_root = root / "phase1"
        phase1_values = {}
        for row in assignment["assignments"]:
            validated = validate_judgment.validate_phase1(phase1_input(row), assignment)
            path = phase1_root / row["scorer_id"] / f"{row['review_id']}.json"
            write_json(path, validated)
            phase1_values[row["review_id"]] = validated
        reuse_target = phase1_root / assignment["assignments"][1]["scorer_id"] / f"{assignment['assignments'][1]['review_id']}.json"
        original_bytes = reuse_target.read_bytes()
        reuse_target.write_bytes((phase1_root / first_row["scorer_id"] / f"{first_row['review_id']}.json").read_bytes())
        checks["reused-judgment-file-rejected"] = expect_rejected(
            scoring_pipeline.seal_phase1, generation_path, assignment_path, phase1_root, phase1_root / "phase1-seal.json",
        )
        reuse_target.write_bytes(original_bytes)
        phase1_seal_path = phase1_root / "phase1-seal.json"
        scoring_pipeline.seal_phase1(generation_path, assignment_path, phase1_root, phase1_seal_path)
        phase1_seal = load_json(phase1_seal_path)
        checks["phase1-252-hash-sealed-readonly"] = len(phase1_seal["judgments"]) == 252 and not any(path.stat().st_mode & 0o222 for path in phase1_root.rglob("*"))

        valid_phase2 = phase2_input(first_row, phase1_seal["judgments"][first_row["review_id"]]["sha256"])
        injected_score = {**valid_phase2, "correctness": {"score_0_100": 99}}
        checks["phase2-score-change-rejected"] = expect_rejected(validate_judgment.validate_phase2, injected_score, assignment, phase1_seal)
        wrong_phase1_hash = {**valid_phase2, "phase1_judgment_sha256": "0" * 64}
        checks["phase2-wrong-phase1-hash-rejected"] = expect_rejected(validate_judgment.validate_phase2, wrong_phase1_hash, assignment, phase1_seal)
        valid_first_wrong = copy.deepcopy(valid_phase2)
        valid_first_wrong["dialogue"]["first_wrong"].update({
            "category": "tool_choice", "raw_event_line": 1, "completed_call_index": 1,
            "call_id": f"{first_row['run_id']}-call-1", "explanation": "wrong navigation call",
        })
        checks["phase2-canonical-first-wrong-accepted"] = not expect_rejected(
            validate_judgment.validate_phase2, valid_first_wrong, assignment, phase1_seal,
        )
        for name, field, invalid in (
            ("missing-call-id", "call_id", "missing-call"),
            ("mixed-call-index", "completed_call_index", 2),
            ("mixed-raw-line", "raw_event_line", 2),
        ):
            malformed = copy.deepcopy(valid_first_wrong)
            malformed["dialogue"]["first_wrong"][field] = invalid
            checks[f"phase2-{name}-rejected"] = expect_rejected(
                validate_judgment.validate_phase2, malformed, assignment, phase1_seal,
            )
        non_navigation = copy.deepcopy(valid_first_wrong)
        non_navigation["dialogue"]["first_wrong"].update({
            "raw_event_line": 2, "completed_call_index": 2, "call_id": f"{first_row['run_id']}-call-2",
        })
        checks["phase2-non-navigation-call-rejected"] = expect_rejected(
            validate_judgment.validate_phase2, non_navigation, assignment, phase1_seal,
        )
        incomplete = copy.deepcopy(valid_first_wrong)
        incomplete["dialogue"]["first_wrong"].update({
            "raw_event_line": 3, "completed_call_index": 3, "call_id": f"{first_row['run_id']}-call-3",
        })
        checks["phase2-incomplete-call-rejected"] = expect_rejected(
            validate_judgment.validate_phase2, incomplete, assignment, phase1_seal,
        )
        mismatched_stage = copy.deepcopy(valid_phase2)
        mismatched_stage["dialogue"]["discovery_stages"][validate_judgment.STAGE_NAMES[0]]["call_ids"] = [f"{first_row['run_id']}-call-2"]
        checks["phase2-stage-call-order-and-line-rejected"] = expect_rejected(
            validate_judgment.validate_phase2, mismatched_stage, assignment, phase1_seal,
        )

        phase2_root = root / "phase2"
        process_bundle_paths = scoring_pipeline.process_bundle_paths("synthetic-generation")
        for scorer_id, bundle_path in process_bundle_paths.items():
            write_json(bundle_path, {"schema_version": 1, "phase": "process", "scorer_id": scorer_id})
            bundle_path.chmod(0o444)
        for row in assignment["assignments"]:
            raw = phase2_input(row, phase1_seal["judgments"][row["review_id"]]["sha256"])
            validated = validate_judgment.validate_phase2(raw, assignment, phase1_seal)
            write_json(phase2_root / row["scorer_id"] / f"{row['review_id']}.json", validated)
        phase2_seal_path = phase2_root / "phase2-seal.json"
        scoring_pipeline.seal_phase2(generation_path, assignment_path, phase1_seal_path, phase2_root, phase2_seal_path)
        phase2_seal = load_json(phase2_seal_path)
        checks["phase2-252-hash-sealed-readonly"] = len(phase2_seal["judgments"]) == 252 and not any(path.stat().st_mode & 0o222 for path in phase2_root.rglob("*"))
        checks["phase2-process-bundles-exact-3x-bound-readonly"] = (
            set(phase2_seal["process_bundle_bindings_by_scorer"]) == set(make_scoring_bundles.SCORERS)
            and all(
                item["path"] == str(process_bundle_paths[scorer_id])
                and item["sha256"] == sha256(process_bundle_paths[scorer_id])
                and item["mode"] == 0o444
                for scorer_id, item in phase2_seal["process_bundle_bindings_by_scorer"].items()
            )
        )

        first_process_scorer = make_scoring_bundles.SCORERS[0]
        first_process_bundle = process_bundle_paths[first_process_scorer]
        original_process_bundle = first_process_bundle.read_bytes()
        first_process_bundle.unlink()
        checks["phase2-process-bundle-deletion-rejected"] = expect_rejected(
            scoring_pipeline.verify_phase2_seal, phase2_seal_path, assignment_path, assignment, phase1_seal_path, phase1_seal,
        )
        first_process_bundle.write_bytes(original_process_bundle)
        first_process_bundle.chmod(0o444)

        first_process_bundle.chmod(0o644)
        first_process_bundle.write_bytes(original_process_bundle + b"\n")
        first_process_bundle.chmod(0o444)
        checks["phase2-process-bundle-content-drift-rejected"] = expect_rejected(
            scoring_pipeline.verify_phase2_seal, phase2_seal_path, assignment_path, assignment, phase1_seal_path, phase1_seal,
        )
        first_process_bundle.chmod(0o644)
        first_process_bundle.write_bytes(original_process_bundle)
        first_process_bundle.chmod(0o444)

        first_process_bundle.chmod(0o644)
        checks["phase2-process-bundle-writable-rejected"] = expect_rejected(
            scoring_pipeline.verify_phase2_seal, phase2_seal_path, assignment_path, assignment, phase1_seal_path, phase1_seal,
        )
        first_process_bundle.chmod(0o444)

        forged_bundle = root / "forged-process-bundle.json"
        forged_bundle.write_bytes(original_process_bundle)
        forged_bundle.chmod(0o444)
        original_phase2_seal = phase2_seal_path.read_bytes()
        forged_phase2_seal = copy.deepcopy(phase2_seal)
        forged_phase2_seal["process_bundle_bindings_by_scorer"][first_process_scorer]["path"] = str(forged_bundle)
        forged_phase2_seal.pop("seal_sha256")
        forged_phase2_seal["seal_sha256"] = canonical_sha256(forged_phase2_seal)
        phase2_root.chmod(0o755)
        phase2_seal_path.chmod(0o644)
        write_json(phase2_seal_path, forged_phase2_seal)
        phase2_seal_path.chmod(0o444)
        checks["phase2-process-bundle-resealed-path-forgery-rejected"] = expect_rejected(
            scoring_pipeline.verify_phase2_seal, phase2_seal_path, assignment_path, assignment, phase1_seal_path, phase1_seal,
        )
        phase2_seal_path.chmod(0o644)
        phase2_seal_path.write_bytes(original_phase2_seal)
        phase2_seal_path.chmod(0o444)
        phase2_root.chmod(0o555)

        first_process_bundle.unlink()
        first_process_bundle.symlink_to(forged_bundle)
        checks["phase2-process-bundle-symlink-rejected"] = expect_rejected(
            scoring_pipeline.verify_phase2_seal, phase2_seal_path, assignment_path, assignment, phase1_seal_path, phase1_seal,
        )
        first_process_bundle.unlink()
        first_process_bundle.write_bytes(original_process_bundle)
        first_process_bundle.chmod(0o444)

        final_root = root / "final"
        scoring_pipeline.merge(generation_path, assignment_path, phase1_seal_path, phase2_seal_path, final_root)
        final_seal = load_json(final_root / "final-seal.json")
        first_final = load_json(pathlib.Path(final_seal["judgments"][first_row["review_id"]]["path"]))
        checks["final-252-deterministic-merge"] = len(final_seal["judgments"]) == 252 and first_final["correctness"] == phase1_values[first_row["review_id"]]["correctness"]
        checks["final-files-readonly"] = not any(path.stat().st_mode & 0o222 for path in [final_root, *final_root.rglob("*")])
        verified_final = scoring_pipeline.verify_final_manifest(
            final_root / "final-seal.json", generation_path, assignment_path,
        )
        checks["final-manifest-exact-identity-hash-binding"] = len(verified_final["judgments"]) == 252
        checks["final-manifest-rechecks-phase2-process-bundles"] = (
            verified_final["process_bundle_bindings_by_scorer"] == phase2_seal["process_bundle_bindings_by_scorer"]
        )

        swapped_root = root / "swapped-final"
        shutil.copytree(final_root, swapped_root, copy_function=shutil.copy2)
        make_writable(swapped_root)
        swapped_manifest_path = swapped_root / "final-seal.json"
        swapped = load_json(swapped_manifest_path)
        for review_id, item in swapped["judgments"].items():
            item["path"] = str(swapped_root / item["scorer_id"] / f"{review_id}.json")
        first_review, second_review = sorted(swapped["judgments"])[:2]
        first_path = pathlib.Path(swapped["judgments"][first_review]["path"])
        second_path = pathlib.Path(swapped["judgments"][second_review]["path"])
        first_bytes, second_bytes = first_path.read_bytes(), second_path.read_bytes()
        first_path.write_bytes(second_bytes)
        second_path.write_bytes(first_bytes)
        swapped["judgments"][first_review]["sha256"] = sha256(first_path)
        swapped["judgments"][second_review]["sha256"] = sha256(second_path)
        swapped.pop("seal_sha256")
        swapped["seal_sha256"] = canonical_sha256(swapped)
        write_json(swapped_manifest_path, swapped)
        scoring_pipeline.lock_tree(swapped_root)
        checks["final-joint-review-file-hash-swap-rejected"] = expect_rejected(
            scoring_pipeline.verify_final_manifest,
            swapped_manifest_path, generation_path, assignment_path,
        )
    finally:
        scoring_pipeline.verify_generation = original_verify
        scoring_pipeline.HARNESS_ROOT = original_pipeline_harness_root
        make_writable(root)
        shutil.rmtree(root, ignore_errors=True)
    tested_paths = [
        HARNESS_ROOT / "scripts/make_scoring_bundles.py",
        HARNESS_ROOT / "scripts/scoring_pipeline.py",
        HARNESS_ROOT / "scripts/scoring_selftest.py",
        HARNESS_ROOT / "scripts/validate_judgment.py",
        HARNESS_ROOT / "config/scoring-rubric.json",
        HARNESS_ROOT / "schemas/judgment.schema.json",
        HARNESS_ROOT / "schemas/phase1-correctness-judgment.schema.json",
        HARNESS_ROOT / "schemas/phase2-process-judgment.schema.json",
        HARNESS_ROOT / "schemas/manual-evaluation-components.schema.json",
        HARNESS_ROOT / "schemas/session-metrics.schema.json",
        HARNESS_ROOT / "scripts/selftest_contract.py",
    ]
    report = {
        "schema_version": 1, "external_model_calls": 0, "builds": 0, "indexing_operations": 0,
        "tested_file_sha256": {str(path.relative_to(HARNESS_ROOT)): sha256(path) for path in tested_paths},
        "required_check_names": sorted(SCORING_REQUIRED_CHECKS),
        "checks": checks,
        "passed": SCORING_REQUIRED_CHECKS.issubset(checks) and all(checks[name] is True for name in SCORING_REQUIRED_CHECKS),
    }
    output = HARNESS_ROOT / "reports/scoring-selftest.json"
    write_json(output, report)
    print(json.dumps({"passed": report["passed"], "checks": checks, "report": str(output)}, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
