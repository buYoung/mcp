#!/usr/bin/env python3
"""Create and verify immutable, lossless inputs for baseline aggregation."""
from __future__ import annotations

import importlib.util
import json
import os
import pathlib
import sys
import uuid
from typing import Any, Callable

from common import HARNESS_ROOT, QUALITY_ROOT, canonical_sha256, load_json, sha256, write_json
from generation import verify as verify_generation
from protocol import verify_artifact_seal
from scoring_pipeline import verify_final_manifest
from validate_judgment import load_assignment


sys.dont_write_bytecode = True

INPUT_CONTRACT = "baseline-analysis-input-seal-v1"
CREATED_CONTRACT = "baseline-analysis-inputs-v1"
EXPECTED_METRICS = 84
EXPECTED_JUDGMENTS = 252

SEAL_KEYS = {
    "schema_version", "input_contract", "created_input_contract_version",
    "generation", "ledger", "automatic_metrics_contract", "metrics_index",
    "scoring", "judgments_index", "aggregator", "seal_schema", "seal_sha256",
}
GENERATION_KEYS = {"path", "sha256", "generation_id", "generation_seal_sha256"}
LEDGER_KEYS = {"path", "sha256", "state", "attempt_accounting"}
AUTOMATIC_KEYS = {"schema_path", "schema_sha256", "extractor_path", "extractor_sha256"}
INDEX_KEYS = {"path", "sha256", "count", "entries"}
METRIC_ENTRY_KEYS = {"run_id", "path", "sha256"}
SCORING_KEYS = {
    "final_manifest_path", "final_manifest_file_sha256", "final_manifest_seal_sha256",
    "assignment_path", "assignment_file_sha256", "assignment_seal_sha256",
}
JUDGMENT_ENTRY_KEYS = {"review_id", "path", "sha256"}
FILE_BINDING_KEYS = {"path", "sha256"}
EXPECTED_ARMS = ("B1", "B2")
TRANSIENT_CATEGORIES = ("transient_auth", "transient_provider", "transient_network")
TERMINAL_BEHAVIORS = (
    "stop", "timeout", "model_step_limit", "output_limit", "process_error",
    "protocol_error", "infrastructure_error", "unknown",
)
ATTEMPT_STATES = ("terminal", "canceled-before-start")


def exact_keys(value: Any, expected: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != expected:
        actual = sorted(value) if isinstance(value, dict) else type(value).__name__
        raise SystemExit(f"{label} keys mismatch: {actual}")


def exact_file(raw: Any, expected: pathlib.Path, label: str, *, read_only: bool = True) -> pathlib.Path:
    if not isinstance(raw, str):
        raise SystemExit(f"{label} path must be an absolute string")
    path = pathlib.Path(raw)
    expected = expected.resolve()
    if (
        not path.is_absolute()
        or str(path) != str(expected)
        or path.is_symlink()
        or not path.is_file()
        or path.resolve() != expected
    ):
        raise SystemExit(f"{label} is not the exact expected non-symlink file")
    if read_only and path.stat().st_mode & 0o222:
        raise SystemExit(f"{label} remains writable")
    return path


def exact_directory(raw: Any, expected: pathlib.Path, label: str, *, read_only: bool = True) -> pathlib.Path:
    if not isinstance(raw, (str, pathlib.Path)):
        raise SystemExit(f"{label} path is invalid")
    path = pathlib.Path(raw)
    expected = expected.resolve()
    if (
        not path.is_absolute()
        or str(path) != str(expected)
        or path.is_symlink()
        or not path.is_dir()
        or path.resolve() != expected
    ):
        raise SystemExit(f"{label} is not the exact expected non-symlink directory")
    if read_only and path.stat().st_mode & 0o222:
        raise SystemExit(f"{label} remains writable")
    return path


def strict_json(path: pathlib.Path) -> Any:
    def reject(value: str) -> None:
        raise ValueError(f"non-standard JSON constant {value}")

    try:
        return json.loads(path.read_text(encoding="utf-8"), parse_constant=reject)
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        raise SystemExit(f"invalid JSON in {path}: {error}") from error


def schema_validator(extractor_path: pathlib.Path) -> Callable[[Any, dict], list[str]]:
    name = "sealed_extract_run_metrics_schema_validator"
    spec = importlib.util.spec_from_file_location(name, extractor_path)
    if spec is None or spec.loader is None:
        raise SystemExit("automatic metrics schema validator could not be loaded")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    validator = getattr(module, "validate_json_schema", None)
    if not callable(validator):
        raise SystemExit("automatic metrics extractor does not expose its sealed schema validator")
    return validator


def lock_tree(root: pathlib.Path) -> None:
    for path in sorted([root, *root.rglob("*")], reverse=True):
        if not path.is_symlink():
            path.chmod(path.stat().st_mode & ~0o222)


def ledger_attempt_accounting(generation: dict, ledger: dict) -> dict[str, Any]:
    """Validate every ledger attempt and preserve exact latest/superseded denominators."""
    schedule = {
        f"{row['task_id']}:{row['trial_id']}:{row['criterion']}": row
        for row in generation["schedule"]
    }
    slots = ledger.get("slots")
    attempts = ledger.get("attempts")
    if ledger.get("schema_version") != 1 or not isinstance(slots, dict) or set(slots) != set(schedule):
        raise SystemExit("attempt accounting requires the exact completed scheduled ledger")
    if not isinstance(attempts, dict) or not attempts:
        raise SystemExit("attempt accounting requires a non-empty attempts map")

    tracked_ids: set[str] = set()
    latest_ids: set[str] = set()
    slot_by_run: dict[str, str] = {}
    for slot_key, scheduled in schedule.items():
        slot = slots[slot_key]
        all_run_ids = slot.get("all_run_ids")
        latest_run_id = slot.get("latest_run_id")
        if (
            not isinstance(all_run_ids, list)
            or not all_run_ids
            or not all(isinstance(run_id, str) and run_id for run_id in all_run_ids)
            or len(set(all_run_ids)) != len(all_run_ids)
            or latest_run_id != all_run_ids[-1]
            or slot.get("latest_attempt_number") != len(all_run_ids)
            or slot.get("measurement_status") != "valid"
            or slot.get("metrics_status") != "sealed"
            or slot.get("replacement_allowed") is not False
        ):
            raise SystemExit(f"attempt accounting slot contract mismatch: {slot_key}")
        overlap = tracked_ids.intersection(all_run_ids)
        if overlap:
            raise SystemExit(f"attempt accounting reuses run ids across slots: {sorted(overlap)[:3]}")
        tracked_ids.update(all_run_ids)
        latest_ids.add(latest_run_id)
        for expected_number, run_id in enumerate(all_run_ids, 1):
            attempt = attempts.get(run_id)
            if not isinstance(attempt, dict):
                raise SystemExit(f"attempt accounting tracked run is absent: {run_id}")
            if attempt.get("attempt_number") != expected_number:
                raise SystemExit(f"attempt accounting sequence mismatch: {run_id}")
            slot_by_run[run_id] = slot_key
        latest = attempts[latest_run_id]
        classification = latest.get("classification")
        if (
            latest.get("state") != "terminal"
            or latest.get("metrics_status") != "sealed"
            or not isinstance(classification, dict)
            or classification.get("measurement_status") != "valid"
            or classification.get("replacement_allowed") is not False
            or classification.get("generation_invalid") is not False
            or latest.get("task_id") != scheduled["task_id"]
            or latest.get("trial_id") != scheduled["trial_id"]
            or latest.get("arm") != scheduled["criterion"]
            or latest.get("pair_id") != scheduled["pair_id"]
            or latest.get("pair_order_index") != scheduled["pair_order_index"]
        ):
            raise SystemExit(f"attempt accounting latest run is not the exact valid terminal: {slot_key}")

    canceled_ids = {
        run_id for run_id, attempt in attempts.items()
        if isinstance(attempt, dict) and attempt.get("state") == "canceled-before-start"
    }
    if set(attempts) != tracked_ids | canceled_ids or tracked_ids & canceled_ids:
        raise SystemExit("attempt accounting attempts are not exactly tracked or canceled runs")

    rows: list[dict[str, Any]] = []
    for run_id, attempt in attempts.items():
        if not isinstance(attempt, dict) or attempt.get("run_id") != run_id:
            raise SystemExit(f"attempt accounting run identity mismatch: {run_id}")
        slot_key = attempt.get("slot_key")
        if slot_key not in schedule:
            raise SystemExit(f"attempt accounting run has an unknown slot: {run_id}")
        scheduled = schedule[slot_key]
        arm = attempt.get("arm")
        attempt_number = attempt.get("attempt_number")
        state = attempt.get("state")
        if (
            attempt.get("task_id") != scheduled["task_id"]
            or attempt.get("trial_id") != scheduled["trial_id"]
            or arm != scheduled["criterion"]
            or attempt.get("pair_id") != scheduled["pair_id"]
            or attempt.get("pair_order_index") != scheduled["pair_order_index"]
            or (run_id in tracked_ids and slot_by_run.get(run_id) != slot_key)
            or arm not in EXPECTED_ARMS
            or not isinstance(attempt_number, int)
            or isinstance(attempt_number, bool)
            or attempt_number < 1
            or state not in ATTEMPT_STATES
        ):
            raise SystemExit(f"attempt accounting run metadata mismatch: {run_id}")
        classification = attempt.get("classification")
        if state == "terminal":
            if run_id not in tracked_ids or attempt.get("metrics_status") != "sealed" or not isinstance(classification, dict):
                raise SystemExit(f"attempt accounting terminal run is not tracked and sealed: {run_id}")
            measurement = classification.get("measurement_status")
            terminal = classification.get("terminal_behavior")
            replacement_allowed = classification.get("replacement_allowed")
            generation_invalid = classification.get("generation_invalid")
            replacement_category = classification.get("replacement_category")
            if (
                measurement not in {"valid", "infrastructure_invalid"}
                or terminal not in TERMINAL_BEHAVIORS
                or not isinstance(replacement_allowed, bool)
                or not isinstance(generation_invalid, bool)
                or replacement_category not in {*TRANSIENT_CATEGORIES, None}
            ):
                raise SystemExit(f"attempt accounting classification mismatch: {run_id}")
        else:
            if run_id in tracked_ids or classification is not None:
                raise SystemExit(f"attempt accounting canceled run must be unclassified and untracked: {run_id}")
            measurement = None
            terminal = None
            replacement_allowed = None
            replacement_category = None
        rows.append({
            "run_id": run_id,
            "arm": arm,
            "state": state,
            "attempt_number": attempt_number,
            "is_latest_valid": run_id in latest_ids,
            "is_superseded": run_id in tracked_ids and run_id not in latest_ids,
            "measurement_status": measurement,
            "terminal_behavior": terminal,
            "replacement_allowed": replacement_allowed,
            "replacement_category": replacement_category,
        })

    def summarize(selected: list[dict[str, Any]]) -> dict[str, Any]:
        denominator = len(selected)
        measurement_counts = {
            name: sum(row["measurement_status"] == value for row in selected)
            for name, value in (
                ("valid", "valid"),
                ("infrastructure_invalid", "infrastructure_invalid"),
                ("unclassified", None),
            )
        }
        terminal_counts = {
            **{value: sum(row["terminal_behavior"] == value for row in selected) for value in TERMINAL_BEHAVIORS},
            "unclassified": sum(row["terminal_behavior"] is None for row in selected),
        }
        category_counts = {
            **{value: sum(row["replacement_category"] == value for row in selected) for value in TRANSIENT_CATEGORIES},
            "none": sum(row["measurement_status"] is not None and row["replacement_category"] is None for row in selected),
            "unclassified": sum(row["measurement_status"] is None for row in selected),
        }
        state_counts = {value: sum(row["state"] == value for row in selected) for value in ATTEMPT_STATES}
        return {
            "denominator_n": denominator,
            "raw_attempt_count": denominator,
            "latest_valid_count": sum(row["is_latest_valid"] for row in selected),
            "superseded_attempt_count": sum(row["is_superseded"] for row in selected),
            "invalid_attempt_count": measurement_counts["infrastructure_invalid"],
            "transient_attempt_count": sum(row["replacement_category"] in TRANSIENT_CATEGORIES for row in selected),
            "replacement_attempt_count": sum(row["attempt_number"] > 1 for row in selected),
            "canceled_before_start_count": state_counts["canceled-before-start"],
            "measurement_status_counts": measurement_counts,
            "terminal_behavior_counts": terminal_counts,
            "replacement_category_counts": category_counts,
            "attempt_state_counts": state_counts,
        }

    overall = summarize(rows)
    if overall["latest_valid_count"] != EXPECTED_METRICS:
        raise SystemExit("attempt accounting latest valid count must be exactly 84")
    by_arm = [{"arm": arm, **summarize([row for row in rows if row["arm"] == arm])} for arm in EXPECTED_ARMS]
    if [item["latest_valid_count"] for item in by_arm] != [42, 42]:
        raise SystemExit("attempt accounting latest valid arm counts must be B1=42 and B2=42")
    return {
        "raw_attempt_count": overall["raw_attempt_count"],
        "latest_valid_count": overall["latest_valid_count"],
        "superseded_attempt_count": overall["superseded_attempt_count"],
        "invalid_attempt_count": overall["invalid_attempt_count"],
        "transient_attempt_count": overall["transient_attempt_count"],
        "replacement_attempt_count": overall["replacement_attempt_count"],
        "overall": overall,
        "by_arm": by_arm,
    }


def expected_static_paths() -> dict[str, pathlib.Path]:
    return {
        "automatic_schema": HARNESS_ROOT / "schemas/automatic-run-metrics.schema.json",
        "extractor": QUALITY_ROOT / "analysis-tools/extract_run_metrics.py",
        "aggregator": QUALITY_ROOT / "analysis-tools/aggregate_baseline_metrics.py",
        "seal_schema": HARNESS_ROOT / "schemas/analysis-input-seal.schema.json",
    }


def validate_schema(value: dict, schema_path: pathlib.Path, extractor_path: pathlib.Path) -> None:
    schema = strict_json(schema_path)
    errors = schema_validator(extractor_path)(value, schema)
    if errors:
        raise SystemExit(f"analysis input seal schema validation failed: {errors[:5]}")


def latest_metric_entries(
    generation: dict,
    ledger: dict,
    runs_root: pathlib.Path,
    automatic_schema_path: pathlib.Path,
    extractor_path: pathlib.Path,
    *,
    require_read_only: bool,
) -> list[dict[str, str]]:
    schedule_by_slot = {
        f"{row['task_id']}:{row['trial_id']}:{row['criterion']}": row
        for row in generation["schedule"]
    }
    expected_slots = set(schedule_by_slot)
    slots = ledger.get("slots")
    if not isinstance(slots, dict) or set(slots) != expected_slots or len(slots) != EXPECTED_METRICS:
        raise SystemExit("completed ledger slot set is not the sealed 14x3x2 schedule")
    automatic_schema = strict_json(automatic_schema_path)
    validate_automatic = schema_validator(extractor_path)
    entries = []
    run_ids: set[str] = set()
    for slot_key in sorted(expected_slots):
        slot = slots[slot_key]
        if (
            slot.get("measurement_status") != "valid"
            or slot.get("metrics_status") != "sealed"
            or slot.get("replacement_allowed") is not False
        ):
            raise SystemExit(f"slot is not a latest valid metrics-sealed observation: {slot_key}")
        run_id = slot.get("latest_run_id")
        if not isinstance(run_id, str) or not run_id or run_id in run_ids:
            raise SystemExit(f"slot has an invalid or repeated latest run id: {slot_key}")
        run_ids.add(run_id)
        attempt = ledger.get("attempts", {}).get(run_id, {})
        run_dir = runs_root / run_id
        metric_path = runs_root / "automatic-metrics" / run_id / "automatic-run-metrics.json"
        if (
            attempt.get("state") != "terminal"
            or attempt.get("run_dir") != str(run_dir)
            or attempt.get("metrics_status") != "sealed"
            or attempt.get("automatic_metrics_path") != str(metric_path)
            or slot.get("latest_automatic_metrics_path") != str(metric_path)
        ):
            raise SystemExit(f"ledger automatic metric publication binding mismatch: {run_id}")
        exact_directory(str(run_dir), run_dir, f"published run {run_id}", read_only=require_read_only)
        verify_artifact_seal(run_dir)
        if attempt.get("artifact_manifest_sha256") != sha256(run_dir / "artifact-manifest.json"):
            raise SystemExit(f"published run artifact seal differs from the ledger: {run_id}")
        if require_read_only and any(
            path.stat().st_mode & 0o222
            for path in [run_dir, *run_dir.rglob("*")]
            if not path.is_symlink()
        ):
            raise SystemExit(f"published run artifact tree remains writable: {run_id}")
        metric_path = exact_file(str(metric_path), metric_path, f"automatic metric {run_id}", read_only=require_read_only)
        metric_hash = sha256(metric_path)
        if (
            attempt.get("automatic_metrics_sha256") != metric_hash
            or slot.get("latest_automatic_metrics_sha256") != metric_hash
        ):
            raise SystemExit(f"ledger automatic metric hash mismatch: {run_id}")
        metrics_dir = metric_path.parent
        if metrics_dir.is_symlink() or metrics_dir.resolve() != metrics_dir:
            raise SystemExit(f"automatic metric directory traverses a symlink: {run_id}")
        verify_artifact_seal(metrics_dir, ("automatic-run-metrics.json", "extractor.stderr.log"))
        if require_read_only and any(
            path.stat().st_mode & 0o222
            for path in [metrics_dir, *metrics_dir.rglob("*")]
            if not path.is_symlink()
        ):
            raise SystemExit(f"automatic metric artifact tree remains writable: {run_id}")
        metric = strict_json(metric_path)
        schema_errors = validate_automatic(metric, automatic_schema)
        if schema_errors:
            raise SystemExit(f"automatic metric schema mismatch for {run_id}: {schema_errors[:5]}")
        experiment = metric.get("experiment", {})
        run = metric.get("run", {})
        integrity = run.get("integrity", {})
        scheduled = schedule_by_slot[slot_key]
        expected_arm_hash = generation.get("b2", {}).get("materialized_config_file_sha256", {}).get(scheduled["criterion"])
        expected_limits_hash = generation.get("critical_file_sha256", {}).get("harness/config/limits.json")
        publication = run.get("lifecycle", {}).get("publication", {})
        artifact_seal = run.get("lifecycle", {}).get("artifact_seal", {})
        if (
            metric.get("schema_version") != 2
            or run.get("run_id") != run_id
            or run.get("directory") != str(run_dir)
            or run.get("generation_id") != generation["generation_id"]
            or run.get("attempt_number") != slot.get("latest_attempt_number")
            or experiment.get("task_id") != scheduled["task_id"]
            or experiment.get("trial_id") != scheduled["trial_id"]
            or experiment.get("pair_id") != scheduled["pair_id"]
            or experiment.get("arm") != scheduled["criterion"]
            or experiment.get("pair_order_index") != scheduled["pair_order_index"]
            or experiment.get("question_sha256") != scheduled["question_sha256"]
            or experiment.get("prompt_sha256") != generation["prompt_sha256_by_task"][scheduled["task_id"]]
            or experiment.get("corpus_tree_sha256") != generation["source"]["tree_sha256"]
            or experiment.get("model") != generation["model"]
            or experiment.get("generation_id") != generation["generation_id"]
            or experiment.get("attempt_number") != slot.get("latest_attempt_number")
            or not isinstance(expected_arm_hash, str)
            or experiment.get("arm_config_sha256") != expected_arm_hash
            or not isinstance(expected_limits_hash, str)
            or experiment.get("limits_sha256") != expected_limits_hash
            or experiment.get("measurement_status") != "valid"
            or experiment.get("replacement_allowed") is not False
            or experiment.get("generation_invalid") is not False
            or experiment.get("published") is not True
            or experiment.get("latest_published_attempt") is not True
            or experiment.get("artifact_verified") is not True
            or experiment.get("aggregation_eligible") is not True
            or experiment.get("aggregation_ineligible_reasons") != []
            or publication.get("latest_published_attempt") is not True
            or artifact_seal.get("verified") is not True
            or integrity.get("status") != "verified"
            or integrity.get("aggregation_eligible") is not True
            or integrity.get("aggregation_ineligible_reasons") != []
        ):
            raise SystemExit(f"automatic metric is not aggregation-eligible current evidence: {run_id}")
        entries.append({"run_id": run_id, "path": str(metric_path), "sha256": metric_hash})
    return sorted(entries, key=lambda item: item["run_id"])


def judgment_entries(final_manifest: dict, *, require_read_only: bool) -> list[dict[str, str]]:
    judgments = final_manifest.get("judgments")
    if not isinstance(judgments, dict) or len(judgments) != EXPECTED_JUDGMENTS:
        raise SystemExit("final scoring manifest does not contain exactly 252 judgments")
    entries = []
    for review_id, item in sorted(judgments.items()):
        path = pathlib.Path(item["path"])
        path = exact_file(str(path), path, f"final judgment {review_id}", read_only=require_read_only)
        digest = sha256(path)
        if digest != item.get("sha256"):
            raise SystemExit(f"final judgment hash drift: {review_id}")
        entries.append({"review_id": review_id, "path": str(path), "sha256": digest})
    return entries


def verify_analysis_input_seal(path: pathlib.Path, *, require_read_only: bool = True) -> dict:
    """Fail closed on drift in any aggregation input named by the seal."""
    path = exact_file(str(path), path.resolve(), "analysis input seal", read_only=require_read_only)
    seal = strict_json(path)
    exact_keys(seal, SEAL_KEYS, "analysis input seal")
    core = dict(seal)
    recorded = core.pop("seal_sha256", None)
    if recorded != canonical_sha256(core):
        raise SystemExit("analysis input seal self-hash mismatch")
    if (
        seal.get("schema_version") != 1
        or seal.get("input_contract") != INPUT_CONTRACT
        or seal.get("created_input_contract_version") != CREATED_CONTRACT
    ):
        raise SystemExit("analysis input seal contract/version mismatch")

    exact_keys(seal["generation"], GENERATION_KEYS, "analysis generation binding")
    generation_path = pathlib.Path(seal["generation"]["path"])
    generation_path = exact_file(
        str(generation_path), generation_path, "sealed generation", read_only=require_read_only,
    )
    # This is deliberately the first semantic verifier. Any current-input drift stops here.
    verify_generation(generation_path)
    generation = strict_json(generation_path)
    if (
        sha256(generation_path) != seal["generation"]["sha256"]
        or generation.get("generation_id") != seal["generation"]["generation_id"]
        or generation.get("generation_seal_sha256") != seal["generation"]["generation_seal_sha256"]
    ):
        raise SystemExit("analysis input generation binding mismatch")
    generation_id = generation["generation_id"]
    output_root = HARNESS_ROOT / "analysis-inputs" / generation_id
    exact_directory(path.parent, output_root, "analysis input root", read_only=require_read_only)

    runs_root = HARNESS_ROOT / "runs" / generation_id
    ledger_expected = runs_root / "ledger.json"
    exact_keys(seal["ledger"], LEDGER_KEYS, "analysis ledger binding")
    ledger_path = exact_file(
        seal["ledger"]["path"], ledger_expected, "completed ledger", read_only=require_read_only,
    )
    ledger = strict_json(ledger_path)
    if (
        seal["ledger"]["state"] != "completed"
        or ledger.get("state") != "completed"
        or ledger.get("generation_id") != generation_id
        or ledger.get("generation_seal_sha256") != generation["generation_seal_sha256"]
        or sha256(ledger_path) != seal["ledger"]["sha256"]
    ):
        raise SystemExit("analysis input ledger binding mismatch")
    attempt_accounting = ledger_attempt_accounting(generation, ledger)
    if seal["ledger"]["attempt_accounting"] != attempt_accounting:
        raise SystemExit("analysis input ledger attempt accounting mismatch")

    static = expected_static_paths()
    exact_keys(seal["automatic_metrics_contract"], AUTOMATIC_KEYS, "automatic metrics contract")
    automatic_schema = exact_file(
        seal["automatic_metrics_contract"]["schema_path"], static["automatic_schema"],
        "automatic metrics schema", read_only=require_read_only,
    )
    extractor = exact_file(
        seal["automatic_metrics_contract"]["extractor_path"], static["extractor"],
        "automatic metrics extractor", read_only=require_read_only,
    )
    if (
        sha256(automatic_schema) != seal["automatic_metrics_contract"]["schema_sha256"]
        or sha256(extractor) != seal["automatic_metrics_contract"]["extractor_sha256"]
    ):
        raise SystemExit("automatic metrics code/schema hash drift")

    exact_keys(seal["aggregator"], FILE_BINDING_KEYS, "aggregator binding")
    aggregator = exact_file(
        seal["aggregator"]["path"], static["aggregator"], "baseline aggregator",
        read_only=require_read_only,
    )
    if sha256(aggregator) != seal["aggregator"]["sha256"]:
        raise SystemExit("baseline aggregator hash drift")
    exact_keys(seal["seal_schema"], FILE_BINDING_KEYS, "analysis seal schema binding")
    seal_schema = exact_file(
        seal["seal_schema"]["path"], static["seal_schema"], "analysis input seal schema",
        read_only=require_read_only,
    )
    if sha256(seal_schema) != seal["seal_schema"]["sha256"]:
        raise SystemExit("analysis input seal schema hash drift")
    validate_schema(seal, seal_schema, extractor)

    assignment_expected = HARNESS_ROOT / "scoring" / generation_id / "coordinator-only/assignments.json"
    final_expected = HARNESS_ROOT / "scoring" / generation_id / "final-judgments/final-seal.json"
    exact_keys(seal["scoring"], SCORING_KEYS, "analysis scoring binding")
    assignment_path = exact_file(
        seal["scoring"]["assignment_path"], assignment_expected, "scoring assignment",
        read_only=require_read_only,
    )
    assignment = load_assignment(assignment_path, runs_root)
    if (
        sha256(assignment_path) != seal["scoring"]["assignment_file_sha256"]
        or assignment.get("assignment_seal_sha256") != seal["scoring"]["assignment_seal_sha256"]
    ):
        raise SystemExit("analysis scoring assignment binding mismatch")
    final_path = exact_file(
        seal["scoring"]["final_manifest_path"], final_expected, "final scoring manifest",
        read_only=require_read_only,
    )
    final_manifest = verify_final_manifest(final_path, generation_path, assignment_path)
    if (
        sha256(final_path) != seal["scoring"]["final_manifest_file_sha256"]
        or final_manifest.get("seal_sha256") != seal["scoring"]["final_manifest_seal_sha256"]
    ):
        raise SystemExit("analysis final scoring manifest binding mismatch")

    expected_metrics = latest_metric_entries(
        generation, ledger, runs_root, automatic_schema, extractor,
        require_read_only=require_read_only,
    )
    exact_keys(seal["metrics_index"], INDEX_KEYS, "metrics index binding")
    metrics_index_path = exact_file(
        seal["metrics_index"]["path"], output_root / "metrics-index.json", "metrics index",
        read_only=require_read_only,
    )
    metrics_index = strict_json(metrics_index_path)
    expected_metrics_index = {
        "schema_version": 1,
        "metrics": [{"run_id": item["run_id"], "path": item["path"]} for item in expected_metrics],
    }
    for item in seal["metrics_index"].get("entries", []):
        exact_keys(item, METRIC_ENTRY_KEYS, "metrics index sealed entry")
    if (
        seal["metrics_index"].get("count") != EXPECTED_METRICS
        or seal["metrics_index"].get("entries") != expected_metrics
        or metrics_index != expected_metrics_index
        or sha256(metrics_index_path) != seal["metrics_index"]["sha256"]
    ):
        raise SystemExit("metrics index does not exactly match 84 current ledger attempts")

    expected_judgments = judgment_entries(final_manifest, require_read_only=require_read_only)
    exact_keys(seal["judgments_index"], INDEX_KEYS, "judgments index binding")
    judgments_index_path = exact_file(
        seal["judgments_index"]["path"], output_root / "judgments-index.json", "judgments index",
        read_only=require_read_only,
    )
    judgments_index = strict_json(judgments_index_path)
    expected_judgments_index = {
        "schema_version": 1,
        "judgments": [{"review_id": item["review_id"], "path": item["path"]} for item in expected_judgments],
    }
    for item in seal["judgments_index"].get("entries", []):
        exact_keys(item, JUDGMENT_ENTRY_KEYS, "judgments index sealed entry")
    if (
        seal["judgments_index"].get("count") != EXPECTED_JUDGMENTS
        or seal["judgments_index"].get("entries") != expected_judgments
        or judgments_index != expected_judgments_index
        or sha256(judgments_index_path) != seal["judgments_index"]["sha256"]
    ):
        raise SystemExit("judgments index does not exactly match 252 final sealed judgments")
    return seal


def create(
    generation_path: pathlib.Path,
    assignment_path: pathlib.Path,
    final_manifest_path: pathlib.Path,
    output_root: pathlib.Path,
) -> pathlib.Path:
    generation_path = exact_file(
        str(generation_path), generation_path.resolve(), "sealed generation",
    )
    verify_generation(generation_path)
    generation = strict_json(generation_path)
    generation_id = generation["generation_id"]
    expected_output_root = HARNESS_ROOT / "analysis-inputs" / generation_id
    if output_root != expected_output_root or output_root.exists():
        raise SystemExit("analysis input output must be the new fixed generation analysis-inputs directory")
    runs_root = HARNESS_ROOT / "runs" / generation_id
    ledger_path = runs_root / "ledger.json"
    ledger_path = exact_file(str(ledger_path), ledger_path, "completed ledger")
    ledger = strict_json(ledger_path)
    if ledger.get("state") != "completed":
        raise SystemExit("analysis inputs require the completed 84-session ledger")
    attempt_accounting = ledger_attempt_accounting(generation, ledger)
    expected_assignment = HARNESS_ROOT / "scoring" / generation_id / "coordinator-only/assignments.json"
    assignment_path = exact_file(str(assignment_path), expected_assignment, "scoring assignment")
    assignment = load_assignment(assignment_path, runs_root)
    expected_final = HARNESS_ROOT / "scoring" / generation_id / "final-judgments/final-seal.json"
    final_manifest_path = exact_file(str(final_manifest_path), expected_final, "final scoring manifest")
    final_manifest = verify_final_manifest(final_manifest_path, generation_path, assignment_path)
    static = expected_static_paths()
    for label, static_path in static.items():
        exact_file(str(static_path), static_path, label)
    metric_entries = latest_metric_entries(
        generation, ledger, runs_root, static["automatic_schema"], static["extractor"],
        require_read_only=True,
    )
    final_judgments = judgment_entries(final_manifest, require_read_only=True)

    temporary_root = output_root.parent / f".{output_root.name}.{uuid.uuid4().hex}.tmp"
    metrics_index_path = temporary_root / "metrics-index.json"
    judgments_index_path = temporary_root / "judgments-index.json"
    write_json(metrics_index_path, {
        "schema_version": 1,
        "metrics": [{"run_id": item["run_id"], "path": item["path"]} for item in metric_entries],
    })
    write_json(judgments_index_path, {
        "schema_version": 1,
        "judgments": [{"review_id": item["review_id"], "path": item["path"]} for item in final_judgments],
    })
    final_metrics_index = output_root / "metrics-index.json"
    final_judgments_index = output_root / "judgments-index.json"
    value = {
        "schema_version": 1,
        "input_contract": INPUT_CONTRACT,
        "created_input_contract_version": CREATED_CONTRACT,
        "generation": {
            "path": str(generation_path), "sha256": sha256(generation_path),
            "generation_id": generation_id,
            "generation_seal_sha256": generation["generation_seal_sha256"],
        },
        "ledger": {
            "path": str(ledger_path),
            "sha256": sha256(ledger_path),
            "state": "completed",
            "attempt_accounting": attempt_accounting,
        },
        "automatic_metrics_contract": {
            "schema_path": str(static["automatic_schema"]), "schema_sha256": sha256(static["automatic_schema"]),
            "extractor_path": str(static["extractor"]), "extractor_sha256": sha256(static["extractor"]),
        },
        "metrics_index": {
            "path": str(final_metrics_index), "sha256": sha256(metrics_index_path),
            "count": EXPECTED_METRICS, "entries": metric_entries,
        },
        "scoring": {
            "final_manifest_path": str(final_manifest_path),
            "final_manifest_file_sha256": sha256(final_manifest_path),
            "final_manifest_seal_sha256": final_manifest["seal_sha256"],
            "assignment_path": str(assignment_path),
            "assignment_file_sha256": sha256(assignment_path),
            "assignment_seal_sha256": assignment["assignment_seal_sha256"],
        },
        "judgments_index": {
            "path": str(final_judgments_index), "sha256": sha256(judgments_index_path),
            "count": EXPECTED_JUDGMENTS, "entries": final_judgments,
        },
        "aggregator": {"path": str(static["aggregator"]), "sha256": sha256(static["aggregator"])},
        "seal_schema": {"path": str(static["seal_schema"]), "sha256": sha256(static["seal_schema"])},
    }
    value["seal_sha256"] = canonical_sha256(value)
    validate_schema(value, static["seal_schema"], static["extractor"])
    write_json(temporary_root / "analysis-input-seal.json", value)
    lock_tree(temporary_root)
    output_root.parent.mkdir(parents=True, exist_ok=True)
    os.replace(temporary_root, output_root)
    result_path = output_root / "analysis-input-seal.json"
    verify_analysis_input_seal(result_path)
    return result_path


def main() -> int:
    if len(sys.argv) == 3 and sys.argv[1] == "verify":
        verify_analysis_input_seal(pathlib.Path(sys.argv[2]))
        return 0
    if len(sys.argv) == 6 and sys.argv[1] == "create":
        result = create(*map(pathlib.Path, sys.argv[2:]))
        print(result)
        return 0
    raise SystemExit(
        "usage: build_analysis_inputs.py create GENERATION ASSIGNMENTS FINAL_SCORING_MANIFEST OUTPUT_ROOT | "
        "verify ANALYSIS_INPUT_SEAL"
    )


if __name__ == "__main__":
    raise SystemExit(main())
