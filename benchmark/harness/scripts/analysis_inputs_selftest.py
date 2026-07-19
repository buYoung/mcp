#!/usr/bin/env python3
"""No-model synthetic checks for the immutable aggregation-input producer."""
from __future__ import annotations

import copy
import json
import os
import pathlib
import shutil
import tempfile

import build_analysis_inputs as analysis_inputs
from common import HARNESS_ROOT, canonical_sha256, sha256, write_json
from selftest_contract import ANALYSIS_INPUT_REQUIRED_CHECKS


ZERO_SHA = "0" * 64
ONE_SHA = "1" * 64


def rejected(function, *args, **kwargs) -> bool:
    try:
        function(*args, **kwargs)
        return False
    except (SystemExit, KeyError, TypeError, ValueError):
        return True


def schedule() -> list[dict]:
    rows = []
    for task_index in range(14):
        task_id = f"T-{task_index:02d}"
        for trial in ("r1", "r2", "r3"):
            for pair_order_index, arm in enumerate(("B1", "B2")):
                rows.append({
                    "task_id": task_id, "trial_id": trial, "criterion": arm,
                    "pair_id": f"{task_id}-{trial}", "pair_order_index": pair_order_index,
                    "question_sha256": ZERO_SHA,
                })
    return rows


def synthetic_metrics(root: pathlib.Path) -> tuple[dict, dict, pathlib.Path]:
    generation = {
        "generation_id": "synthetic-analysis-generation",
        "generation_seal_sha256": ONE_SHA,
        "schedule": schedule(),
        "prompt_sha256_by_task": {f"T-{index:02d}": ZERO_SHA for index in range(14)},
        "source": {"tree_sha256": ZERO_SHA},
        "model": "ollama-cloud/deepseek-v4-flash",
        "critical_file_sha256": {"harness/config/limits.json": ZERO_SHA},
        "b2": {"materialized_config_file_sha256": {"B1": ZERO_SHA, "B2": ONE_SHA}},
    }
    runs_root = root / "runs"
    ledger = {
        "schema_version": 1,
        "generation_id": generation["generation_id"],
        "generation_seal_sha256": generation["generation_seal_sha256"],
        "state": "completed",
        "slots": {},
        "attempts": {},
    }
    for index, row in enumerate(generation["schedule"]):
        run_id = f"run-{index:03d}"
        run_dir = runs_root / run_id
        write_json(run_dir / "artifact-manifest.json", {"schema_version": 1, "artifacts": {}})
        metric_dir = runs_root / "automatic-metrics" / run_id
        metric_path = metric_dir / "automatic-run-metrics.json"
        write_json(metric_path, {
            "schema_version": 2,
            "run": {
                "run_id": run_id, "directory": str(run_dir),
                "generation_id": generation["generation_id"], "attempt_number": 1,
                "lifecycle": {
                    "publication": {"latest_published_attempt": True},
                    "artifact_seal": {"verified": True},
                },
                "integrity": {
                    "status": "verified", "aggregation_eligible": True,
                    "aggregation_ineligible_reasons": [],
                },
            },
            "experiment": {
                "task_id": row["task_id"], "trial_id": row["trial_id"],
                "pair_id": row["pair_id"], "arm": row["criterion"],
                "pair_order_index": row["pair_order_index"],
                "question_sha256": row["question_sha256"],
                "prompt_sha256": ZERO_SHA, "corpus_tree_sha256": ZERO_SHA,
                "model": generation["model"], "generation_id": generation["generation_id"],
                "attempt_number": 1,
                "arm_config_sha256": ZERO_SHA if row["criterion"] == "B1" else ONE_SHA,
                "limits_sha256": ZERO_SHA,
                "measurement_status": "valid", "replacement_allowed": False,
                "generation_invalid": False, "published": True,
                "latest_published_attempt": True, "artifact_verified": True,
                "aggregation_eligible": True, "aggregation_ineligible_reasons": [],
            },
        })
        (metric_dir / "extractor.stderr.log").write_text("", encoding="utf-8")
        digest = sha256(metric_path)
        slot_key = f"{row['task_id']}:{row['trial_id']}:{row['criterion']}"
        ledger["slots"][slot_key] = {
            "measurement_status": "valid", "metrics_status": "sealed",
            "replacement_allowed": False, "latest_run_id": run_id,
            "latest_attempt_number": 1,
            "all_run_ids": [run_id],
            "latest_automatic_metrics_path": str(metric_path),
            "latest_automatic_metrics_sha256": digest,
        }
        ledger["attempts"][run_id] = {
            "run_id": run_id, "attempt_number": 1, "slot_key": slot_key,
            "task_id": row["task_id"], "trial_id": row["trial_id"],
            "arm": row["criterion"], "pair_id": row["pair_id"],
            "pair_order_index": row["pair_order_index"],
            "state": "terminal", "run_dir": str(run_dir), "metrics_status": "sealed",
            "automatic_metrics_path": str(metric_path), "automatic_metrics_sha256": digest,
            "artifact_manifest_sha256": sha256(run_dir / "artifact-manifest.json"),
            "classification": {
                "measurement_status": "valid", "terminal_behavior": "stop",
                "replacement_allowed": False, "replacement_category": None,
                "generation_invalid": False,
            },
        }
    first_slot_key = sorted(ledger["slots"])[0]
    first_slot = ledger["slots"][first_slot_key]
    latest_run_id = first_slot["latest_run_id"]
    latest_attempt = ledger["attempts"][latest_run_id]
    superseded = copy.deepcopy(latest_attempt)
    superseded.update({
        "run_id": "superseded-run", "attempt_number": 1,
        "run_dir": str(runs_root / "superseded-run"),
        "classification": {
            "measurement_status": "infrastructure_invalid",
            "terminal_behavior": "infrastructure_error",
            "replacement_allowed": True,
            "replacement_category": "transient_provider",
            "generation_invalid": False,
        },
    })
    ledger["attempts"]["superseded-run"] = superseded
    latest_attempt["attempt_number"] = 2
    first_slot.update({"all_run_ids": ["superseded-run", latest_run_id], "latest_attempt_number": 2})
    latest_metric_path = pathlib.Path(first_slot["latest_automatic_metrics_path"])
    latest_metric = json.loads(latest_metric_path.read_text(encoding="utf-8"))
    latest_metric["run"]["attempt_number"] = 2
    latest_metric["experiment"]["attempt_number"] = 2
    write_json(latest_metric_path, latest_metric)
    latest_digest = sha256(latest_metric_path)
    first_slot["latest_automatic_metrics_sha256"] = latest_digest
    latest_attempt["automatic_metrics_sha256"] = latest_digest
    return generation, ledger, runs_root


def sample_seal(root: pathlib.Path) -> dict:
    metric_entries = [
        {"run_id": f"run-{index:03d}", "path": str(root / f"metric-{index:03d}.json"), "sha256": ZERO_SHA}
        for index in range(84)
    ]
    judgment_entries = [
        {"review_id": f"review-{index:03d}", "path": str(root / f"judgment-{index:03d}.json"), "sha256": ONE_SHA}
        for index in range(252)
    ]
    value = {
        "schema_version": 1,
        "input_contract": analysis_inputs.INPUT_CONTRACT,
        "created_input_contract_version": analysis_inputs.CREATED_CONTRACT,
        "generation": {"path": str(root / "generation.json"), "sha256": ZERO_SHA, "generation_id": "g", "generation_seal_sha256": ONE_SHA},
        "ledger": {
            "path": str(root / "ledger.json"), "sha256": ZERO_SHA, "state": "completed",
            "attempt_accounting": {
                "raw_attempt_count": 84, "latest_valid_count": 84,
                "superseded_attempt_count": 0, "invalid_attempt_count": 0,
                "transient_attempt_count": 0, "replacement_attempt_count": 0,
                "overall": {}, "by_arm": [{}, {}],
            },
        },
        "automatic_metrics_contract": {
            "schema_path": str(root / "automatic.schema.json"), "schema_sha256": ZERO_SHA,
            "extractor_path": str(root / "extractor.py"), "extractor_sha256": ONE_SHA,
        },
        "metrics_index": {"path": str(root / "metrics-index.json"), "sha256": ZERO_SHA, "count": 84, "entries": metric_entries},
        "scoring": {
            "final_manifest_path": str(root / "final-seal.json"),
            "final_manifest_file_sha256": ZERO_SHA, "final_manifest_seal_sha256": ONE_SHA,
            "assignment_path": str(root / "assignments.json"),
            "assignment_file_sha256": ZERO_SHA, "assignment_seal_sha256": ONE_SHA,
        },
        "judgments_index": {"path": str(root / "judgments-index.json"), "sha256": ONE_SHA, "count": 252, "entries": judgment_entries},
        "aggregator": {"path": str(root / "aggregator.py"), "sha256": ZERO_SHA},
        "seal_schema": {"path": str(root / "analysis.schema.json"), "sha256": ONE_SHA},
    }
    value["seal_sha256"] = canonical_sha256(value)
    return value


def main() -> int:
    root = pathlib.Path(tempfile.mkdtemp(prefix="analysis-inputs-selftest-", dir=HARNESS_ROOT / "synthetic"))
    checks: dict[str, bool] = {}
    original_validator = analysis_inputs.schema_validator
    original_artifact_verifier = analysis_inputs.verify_artifact_seal
    try:
        seal = sample_seal(root)
        schema_path = HARNESS_ROOT / "schemas/analysis-input-seal.schema.json"
        extractor_path = HARNESS_ROOT.parent / "analysis-tools/extract_run_metrics.py"
        analysis_inputs.validate_schema(seal, schema_path, extractor_path)
        checks["analysis-seal-schema-exact"] = True
        extra = copy.deepcopy(seal)
        extra["unexpected"] = True
        extra["seal_sha256"] = canonical_sha256({key: value for key, value in extra.items() if key != "seal_sha256"})
        checks["analysis-seal-extra-field-rejected"] = rejected(
            analysis_inputs.validate_schema, extra, schema_path, extractor_path,
        )

        generation, ledger, runs_root = synthetic_metrics(root)
        accounting = analysis_inputs.ledger_attempt_accounting(generation, ledger)
        checks["attempt-accounting-exact-and-sealed"] = (
            accounting["raw_attempt_count"] == 85
            and accounting["latest_valid_count"] == 84
            and accounting["superseded_attempt_count"] == 1
            and accounting["invalid_attempt_count"] == 1
            and accounting["transient_attempt_count"] == 1
            and accounting["replacement_attempt_count"] == 1
            and [row["latest_valid_count"] for row in accounting["by_arm"]] == [42, 42]
        )
        orphan_ledger = copy.deepcopy(ledger)
        orphan_ledger["attempts"]["orphan"] = {
            **copy.deepcopy(next(iter(orphan_ledger["attempts"].values()))),
            "run_id": "orphan",
        }
        checks["orphan-attempt-rejected"] = rejected(
            analysis_inputs.ledger_attempt_accounting, generation, orphan_ledger,
        )
        analysis_inputs.schema_validator = lambda _: (lambda _value, _schema: [])
        analysis_inputs.verify_artifact_seal = lambda *_args, **_kwargs: {}
        automatic_schema_path = HARNESS_ROOT / "schemas/automatic-run-metrics.schema.json"
        unused_extractor_path = pathlib.Path("unused-extractor")
        entries = analysis_inputs.latest_metric_entries(
            generation, ledger, runs_root, automatic_schema_path, unused_extractor_path,
            require_read_only=False,
        )
        checks["latest-84-metrics-exact"] = len(entries) == 84 and len({item["run_id"] for item in entries}) == 84
        checks["superseded-attempt-excluded"] = "superseded-run" not in {item["run_id"] for item in entries}

        first_entry = entries[0]
        first_path = pathlib.Path(first_entry["path"])
        original_metric = json.loads(first_path.read_text(encoding="utf-8"))
        old_v1 = copy.deepcopy(original_metric)
        old_v1["schema_version"] = 1
        write_json(first_path, old_v1)
        checks["raw-v1-metric-rejected"] = rejected(
            analysis_inputs.latest_metric_entries,
            generation, ledger, runs_root, automatic_schema_path, unused_extractor_path,
            require_read_only=False,
        )
        write_json(first_path, original_metric)

        wrong_identity = copy.deepcopy(original_metric)
        wrong_identity["experiment"]["task_id"] = "another-task"
        write_json(first_path, wrong_identity)
        checks["slot-metric-identity-swap-rejected"] = rejected(
            analysis_inputs.latest_metric_entries,
            generation, ledger, runs_root, automatic_schema_path, unused_extractor_path,
            require_read_only=False,
        )
        write_json(first_path, original_metric)

        first_slot_key = sorted(ledger["slots"])[0]
        original_recorded_path = ledger["slots"][first_slot_key]["latest_automatic_metrics_path"]
        ledger["slots"][first_slot_key]["latest_automatic_metrics_path"] = str(root / "wrong-parent" / first_path.name)
        checks["same-name-wrong-metric-path-rejected"] = rejected(
            analysis_inputs.latest_metric_entries,
            generation, ledger, runs_root, automatic_schema_path, unused_extractor_path,
            require_read_only=False,
        )
        ledger["slots"][first_slot_key]["latest_automatic_metrics_path"] = original_recorded_path

        path_test = root / "path-test.json"
        write_json(path_test, {"ok": True})
        checks["writable-input-path-rejected"] = rejected(
            analysis_inputs.exact_file, str(path_test), path_test, "synthetic writable input",
        )
        path_test.chmod(0o444)
        symlink = root / "path-test-link.json"
        symlink.symlink_to(path_test)
        checks["symlink-input-path-rejected"] = rejected(
            analysis_inputs.exact_file, str(symlink), path_test, "synthetic symlink input",
        )
    finally:
        analysis_inputs.schema_validator = original_validator
        analysis_inputs.verify_artifact_seal = original_artifact_verifier
        for path in [root, *root.rglob("*")]:
            if not path.is_symlink():
                try:
                    path.chmod(path.stat().st_mode | 0o200)
                except FileNotFoundError:
                    pass
        shutil.rmtree(root, ignore_errors=True)

    tested_paths = [
        HARNESS_ROOT / "scripts/build_analysis_inputs.py",
        HARNESS_ROOT / "scripts/analysis_inputs_selftest.py",
        HARNESS_ROOT / "schemas/analysis-input-seal.schema.json",
        HARNESS_ROOT / "schemas/automatic-run-metrics.schema.json",
        HARNESS_ROOT.parent / "analysis-tools/extract_run_metrics.py",
        HARNESS_ROOT / "scripts/selftest_contract.py",
    ]
    report = {
        "schema_version": 1, "external_model_calls": 0, "builds": 0, "indexing_operations": 0,
        "tested_file_sha256": {
            os.path.relpath(path, HARNESS_ROOT): sha256(path)
            for path in tested_paths
        },
        "required_check_names": sorted(ANALYSIS_INPUT_REQUIRED_CHECKS),
        "checks": checks,
        "passed": ANALYSIS_INPUT_REQUIRED_CHECKS.issubset(checks) and all(checks[name] is True for name in ANALYSIS_INPUT_REQUIRED_CHECKS),
    }
    output = HARNESS_ROOT / "reports/analysis-inputs-selftest.json"
    write_json(output, report)
    print(json.dumps({"passed": report["passed"], "checks": checks, "report": str(output)}, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
