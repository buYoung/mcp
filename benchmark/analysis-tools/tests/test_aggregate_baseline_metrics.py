from __future__ import annotations

import copy
import atexit
import contextlib
import importlib.util
import io
import json
import shutil
import sys
import tempfile
import unittest
from itertools import count
from pathlib import Path
from unittest import mock


ANALYSIS_TOOLS = Path(__file__).resolve().parents[1]
SCRIPT = ANALYSIS_TOOLS / "aggregate_baseline_metrics.py"
SPEC = importlib.util.spec_from_file_location("aggregate_baseline_metrics", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("failed to load aggregator module")
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

SCHEMA_PATH = ANALYSIS_TOOLS.parent / "harness/schemas/automatic-run-metrics.schema.json"
AUTOMATIC_SCHEMA = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
TEMPORARY_ROOT = Path(tempfile.mkdtemp(prefix="aggregate-baseline-test.")).resolve()
atexit.register(shutil.rmtree, TEMPORARY_ROOT, True)
DATASET_SEQUENCE = count(1)


TASKS = [f"T{index:02d}" for index in range(1, 15)]
TRIALS = ("r1", "r2", "r3")
ARMS = ("B1", "B2")
SCORERS = ("scorer-1", "scorer-2", "scorer-3")
RUBRIC_SHA256 = "a" * 64
SOURCE_SHA256 = "b" * 64
QUESTION_SHA256 = "c" * 64
PROMPT_SHA256 = "d" * 64


def rubric() -> dict:
    return {
        "schema_version": 1,
        "judgments_per_output": 3,
        "formula": "clamp(40*core + 30*mean(required_content) + 15*grounding + 15*mean(required_relationships) - min(40, 20*explicit_prohibited_claim_count), 0, 100)",
    }


def answer_contract(task: str) -> dict:
    return {
        "task_id": task,
        "final_answer_required": ["claim"],
        "required_file_relationships": ["relationship"],
        "prohibited_claims": ["prohibited"],
    }


def schema_example(schema: dict, root: dict = AUTOMATIC_SCHEMA) -> object:
    if "$ref" in schema:
        target: object = root
        for token in schema["$ref"][2:].split("/"):
            target = target[token.replace("~1", "/").replace("~0", "~")]  # type: ignore[index]
        return schema_example(target, root)  # type: ignore[arg-type]
    if "const" in schema:
        return copy.deepcopy(schema["const"])
    if "enum" in schema:
        return copy.deepcopy(schema["enum"][0])
    kind = schema.get("type")
    if isinstance(kind, list):
        kind = next(item for item in kind if item != "null")
    if kind == "object":
        properties = schema.get("properties", {})
        return {key: schema_example(properties[key], root) for key in schema.get("required", [])}
    if kind == "array":
        return [schema_example(schema["items"], root) for _ in range(schema.get("minItems", 0))]
    if kind == "string":
        pattern = schema.get("pattern", "")
        if "[0-9a-f]{64}" in pattern:
            return "0" * 64
        if ":tool:" in pattern:
            return "session:tool:call"
        if ":model:" in pattern:
            return "session:model:message:part"
        return "x" * max(1, schema.get("minLength", 1))
    if kind == "integer":
        return schema.get("minimum", 0)
    if kind == "number":
        return schema.get("minimum", 0)
    if kind == "boolean":
        return False
    if kind == "null" or kind is None:
        return None
    raise AssertionError(f"unsupported schema fixture type: {kind}")


def tool_call(run_id: str, index: int, tool: str, family: str, input_bytes: int, output_bytes: int, elapsed_ms: int) -> dict:
    call_id = f"{run_id}-call-{index}"
    return {
        "identity": f"session:tool:{call_id}",
        "session_id": "session",
        "call_id": call_id,
        "tool": tool,
        "family": family,
        "status": "completed",
        "completed": True,
        "is_error": False,
        "error": None,
        "raw_event_lines": [2 if index == 1 else 5],
        "selected_completion_line": 2 if index == 1 else 5,
        "revision_count": 1,
        "conflicts": [],
        "field_provenance": {
            "tool": [2 if index == 1 else 5],
            "status": [2 if index == 1 else 5],
            "input": [2 if index == 1 else 5],
            "output": [2 if index == 1 else 5],
            "error": [],
            "time_start": [2 if index == 1 else 5],
            "time_end": [2 if index == 1 else 5],
        },
        "input_present": True,
        "input_null": False,
        "output_present": True,
        "output_null": False,
        "input_utf8_bytes": input_bytes,
        "output_utf8_bytes": output_bytes,
        "input_sha256": f"{index}" * 64,
        "output_sha256": f"{index + 2}" * 64,
        "client_observed_start_ms": 10 * index,
        "client_observed_end_ms": 10 * index + elapsed_ms,
        "client_observed_elapsed_ms": elapsed_ms,
        "completed_call_index": index,
    }


def token_step(index: int, total: int, raw_event_line: int) -> dict:
    return {
        "identity": f"session:model:message-{index}:part-{index}",
        "raw_event_line": raw_event_line,
        "source": "synthetic",
        "input": total,
        "output": 0,
        "reasoning": 0,
        "cache_read": 0,
        "cache_write": 0,
        "total": total,
        "component_sum": total,
        "official_total_matches_components": True,
        "duplicate_record_count": 1,
        "conflicts": [],
    }


def set_token_total(value: dict, total: int) -> None:
    first_total = min(4, total)
    steps = [token_step(1, first_total, 1), token_step(2, total - first_total, 6)]
    tokens = value["cost"]["tokens"]
    tokens.update({
        "input": total, "output": 0, "reasoning": 0, "cache_read": 0,
        "cache_write": 0, "total": total, "official_entries": 2,
        "per_step": steps, "identity_set_matches_completed_models": True,
    })
    value["cost"]["model_steps"] = 2


def metric(run_id: str, task: str, trial: str, arm: str, pair_order_index: int, generation_id: str) -> dict:
    value = schema_example(AUTOMATIC_SCHEMA)
    assert isinstance(value, dict)
    calls = [
        tool_call(run_id, 1, "read", "read", 4, 20, 5),
        tool_call(run_id, 2, "codemap_search_grep", "grep", 6, 30, 10),
    ]
    value["run"].update({
        "run_id": run_id,
        "directory": f"/{run_id}",
        "generation_id": generation_id,
        "attempt_number": 1,
        "is_replacement": False,
        "completed_tool_calls": calls,
        "incomplete_tool_calls": [],
    })
    value["run"]["tool_identity_integrity"].update({
        "authoritative_completed_ids_available": True,
        "authoritative_error_ids_available": True,
        "raw_terminal_identity_match": True,
        "completed_count": 2,
        "incomplete_count": 0,
        "conflicting_call_count": 0,
        "malformed_revision_count": 0,
    })
    classification = value["run"]["lifecycle"]["classification"]
    classification.update({
        "measurement_status": "valid", "terminal_behavior": "stop",
        "replacement_allowed": False, "replacement_category": None,
        "generation_invalid": False, "reason": "valid",
    })
    publication = value["run"]["lifecycle"]["publication"]
    for key in (
        "ledger_present", "generation_match", "attempt_present", "attempt_state_terminal", "run_dir_match",
        "artifact_manifest_hash_match", "classification_match", "attempt_number_match", "slot_present",
        "latest_run_id_match", "latest_attempt_number_match", "slot_measurement_status_match", "published",
        "latest_published_attempt",
    ):
        publication[key] = True
    publication.update({"slot_key": f"{task}:{trial}:{arm}", "all_run_ids": [run_id]})
    value["run"]["lifecycle"]["artifact_seal"].update({
        "manifest_present": True, "verified": True, "issues": [], "writable_paths": [], "symlink_paths": [],
    })
    value["run"]["integrity"].update({
        "status": "verified", "issues": [], "aggregation_eligible": True,
        "aggregation_ineligible_reasons": [],
    })
    value["experiment"].update({
        "task_id": task, "trial_id": trial, "pair_id": f"{task}-{trial}", "arm": arm,
        "pair_order_index": pair_order_index, "question_sha256": QUESTION_SHA256,
        "prompt_sha256": PROMPT_SHA256, "corpus_tree_sha256": SOURCE_SHA256,
        "model": "synthetic-model", "generation_id": generation_id, "attempt_number": 1,
        "measurement_status": "valid", "terminal_behavior": "stop", "replacement_allowed": False,
        "replacement_category": None, "generation_invalid": False, "published": True,
        "latest_published_attempt": True, "artifact_verified": True, "aggregation_eligible": True,
        "aggregation_ineligible_reasons": [],
    })
    value["dialogue"]["codemap_tool_selection"].update({
        "selected_non_handshake": True, "non_handshake_call_count": 1, "handshake_call_count": 0,
        "first_non_handshake_completed_call_index": 2,
        "identities": [calls[1]["identity"]], "tools": [calls[1]["tool"]],
    })
    value["cost"].update({
        "tool_calls_total": 2, "navigation_calls_excluding_handshake": 2,
        "tool_calls_by_family": {"grep": 1, "read": 1}, "tool_input_bytes": 10,
        "tool_output_bytes": 50, "captured_stdout_bytes": 100, "captured_stderr_bytes": 0,
        "command_wall_ms": 100, "codemap_internal_ms": None,
    })
    value["cost"]["tool_byte_measurement"].update({"input_complete": True, "output_complete": True})
    value["cost"]["client_observed_tool_ms"].update({
        "total": 15, "by_family": {"grep": 10, "read": 5}, "complete": True,
    })
    value["cost"]["tool_errors"].update({"count": 0, "identities": []})
    value["cost"]["protocol_errors"].update({"count": 0, "items": []})
    set_token_total(value, 10)
    value["repeated_searches"].update({"groups": [], "group_count": 0, "extra_call_count": 0})
    value["unbounded_reads"].update({"calls": [], "count": 0, "output_bytes": 0})
    value["missing_data"].update({"has_missing": False, "count": 0, "fields": [], "manual_review_required": []})
    return value


def judgment(review_id: str, scorer: str, run_id: str) -> dict:
    return {
        "schema_version": 1,
        "blind_output_id": review_id,
        "scorer_id": scorer,
        "rubric_sha256": RUBRIC_SHA256,
        "correctness": {
            "core_fraction": 1,
            "required_claims": [{"item_id": "claim-1", "fraction": 1, "evidence": "answer", "raw_event_lines": [7]}],
            "grounding_fraction": 1,
            "required_relationships": [{"item_id": "relationship-1", "fraction": 1, "evidence": "answer", "raw_event_lines": [7]}],
            "prohibited_claims": [{"item_id": "prohibited-1", "explicitly_present": False, "evidence": "answer", "raw_event_lines": [7]}],
            "score_0_100": 100,
            "semantic_label": "correct",
            "contract_complete": True,
            "contract_label": "complete",
            "format_valid_json": True,
        },
        "dialogue": {
            "discovery_stages": {
                stage: {"status": "yes", "evidence": "event", "raw_event_lines": [2], "call_ids": [f"{run_id}-call-1"]}
                for stage in MODULE.DISCOVERY_STAGES
            },
            "first_wrong": {
                "category": "none",
                "raw_event_line": None,
                "completed_call_index": None,
                "call_id": None,
                "explanation": "none",
            },
        },
        "notes": "synthetic",
    }


def dataset() -> dict:
    dataset_id = next(DATASET_SEQUENCE)
    generation_id = f"synthetic-generation-{dataset_id}"
    expected_runs_root = TEMPORARY_ROOT / f"dataset-{dataset_id}" / "runs"
    generation_runs_root = expected_runs_root / generation_id
    generation_runs_root.mkdir(parents=True)
    schedule = []
    slots = {}
    attempts = {}
    metrics = {}
    mapping_rows = []
    judgments = {}
    for task in TASKS:
        for trial in TRIALS:
            for arm_index, arm in enumerate(ARMS):
                run_id = f"{task}-{trial}-{arm}-latest"
                schedule.append({
                    "task_id": task,
                    "trial_id": trial,
                    "criterion": arm,
                    "pair_id": f"{task}-{trial}",
                    "pair_order_index": arm_index,
                    "pair_order_index": arm_index,
                    "question_sha256": QUESTION_SHA256,
                })
                slot_key = f"{task}:{trial}:{arm}"
                slots[slot_key] = {
                    "latest_run_id": run_id,
                    "all_run_ids": [run_id],
                    "latest_attempt_number": 1,
                    "measurement_status": "valid",
                    "metrics_status": "sealed",
                    "replacement_allowed": False,
                }
                attempts[run_id] = {
                    "run_id": run_id,
                    "slot_key": slot_key,
                    "task_id": task,
                    "trial_id": trial,
                    "arm": arm,
                    "pair_id": f"{task}-{trial}",
                    "pair_order_index": arm_index,
                    "attempt_number": 1,
                    "state": "terminal",
                    "metrics_status": "sealed",
                    "classification": {
                        "measurement_status": "valid",
                        "terminal_behavior": "stop",
                        "replacement_allowed": False,
                        "replacement_category": None,
                        "generation_invalid": False,
                    },
                }
                (generation_runs_root / run_id).mkdir()
                metrics[run_id] = metric(run_id, task, trial, arm, arm_index, generation_id)
                for scorer in SCORERS:
                    review_id = f"review-{run_id}-{scorer}"
                    mapping_rows.append({
                        "review_id": review_id,
                        "scorer_id": scorer,
                        "session_id": f"session-{run_id}",
                        "run_id": run_id,
                        "run_dir": str(generation_runs_root / run_id),
                        "task_id": task,
                        "trial_id": trial,
                        "arm": arm,
                        "pair_id": f"{task}-{trial}",
                        "pair_order_index": arm_index,
                    })
                    judgments[review_id] = judgment(review_id, scorer, run_id)
    generation = {
        "schema_version": 1,
        "generation_kind": "baseline-3x",
        "execution_ready": True,
        "generation_id": generation_id,
        "tasks": TASKS,
        "trials": list(TRIALS),
        "arms": list(ARMS),
        "task_count": 14,
        "session_count": 84,
        "judgment_count": 252,
        "schedule": schedule,
        "prompt_sha256_by_task": {task: PROMPT_SHA256 for task in TASKS},
        "answer_sha256_by_task": {task: "e" * 64 for task in TASKS},
        "source": {"tree_sha256": SOURCE_SHA256},
        "model": "synthetic-model",
        "b2": {
            "materialized_config_file_sha256": {
                "B1": "1" * 64,
                "B2": "2" * 64,
            },
        },
        "scoring_seal": {
            "judgments_per_output": 3,
            "rubric_sha256": RUBRIC_SHA256,
        },
    }
    generation["generation_seal_sha256"] = MODULE.canonical_sha256(generation)
    for item in metrics.values():
        arm = item["experiment"]["arm"]
        item["experiment"]["arm_config_sha256"] = generation["b2"]["materialized_config_file_sha256"][arm]
    ledger = {
        "schema_version": 1,
        "generation_id": generation["generation_id"],
        "generation_seal_sha256": generation["generation_seal_sha256"],
        "state": "completed",
        "slots": slots,
        "attempts": attempts,
    }
    return {
        "generation": generation,
        "ledger": ledger,
        "metrics_by_run": metrics,
        "coordinator_mapping": {"schema_version": 1, "assignments": mapping_rows},
        "judgments_by_review": judgments,
        "rubric": rubric(),
        "rubric_sha256": RUBRIC_SHA256,
        "answers_by_task": {task: answer_contract(task) for task in TASKS},
        "automatic_schema": AUTOMATIC_SCHEMA,
        "expected_runs_root": expected_runs_root,
    }


def aggregate(value: dict) -> dict:
    return MODULE.aggregate_dataset(**value)


def current_metric(value: dict, task: str, trial: str, arm: str) -> dict:
    run_id = value["ledger"]["slots"][f"{task}:{trial}:{arm}"]["latest_run_id"]
    return value["metrics_by_run"][run_id]


def write_locked_json(path: Path, value: object) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")
    path.chmod(0o444)
    return MODULE.sha256_file(path)


def scoring_manifest_fixture() -> dict:
    fixture_id = next(DATASET_SEQUENCE)
    harness_root = TEMPORARY_ROOT / f"scoring-{fixture_id}" / "harness"
    generation_id = f"scoring-generation-{fixture_id}"
    generation = {"generation_id": generation_id, "generation_seal_sha256": "9" * 64}
    scoring_root = harness_root / "scoring" / generation_id
    ledger_path = harness_root / "runs" / generation_id / "ledger.json"
    ledger_hash = write_locked_json(ledger_path, {"state": "completed", "generation_id": generation_id})

    bundle_bindings = {}
    process_bundle_bindings = {}
    for scorer in SCORERS:
        path = scoring_root / "public-bundles" / f"{scorer}.json"
        bundle_bindings[scorer] = (path, write_locked_json(path, {"scorer_id": scorer}))
        process_path = scoring_root / "phase2-process" / scorer / "bundle.json"
        process_bundle_bindings[scorer] = {
            "path": str(process_path),
            "sha256": write_locked_json(process_path, {"scorer_id": scorer, "phase": "process"}),
            "mode": 0o444,
        }

    rows = []
    for index in range(252):
        scorer = SCORERS[index % 3]
        review_id = f"review-{index:03d}"
        bundle_path, bundle_hash = bundle_bindings[scorer]
        rows.append({
            "review_id": review_id,
            "scorer_id": scorer,
            "session_id": f"session-{index:03d}",
            "run_id": f"run-{index // 3:03d}",
            "bundle_path": str(bundle_path),
            "bundle_sha256": bundle_hash,
        })
    mapping = {
        "schema_version": 1,
        "generation_id": generation_id,
        "generation_seal_sha256": generation["generation_seal_sha256"],
        "ledger_path": str(ledger_path),
        "ledger_sha256": ledger_hash,
        "assignments": rows,
    }
    mapping["assignment_seal_sha256"] = MODULE.canonical_sha256(mapping)
    mapping_path = scoring_root / "coordinator-only" / "assignments.json"
    mapping_file_hash = write_locked_json(mapping_path, mapping)

    phase1_items = {}
    phase2_items = {}
    final_items = {}
    judgments = {}
    provenance = []
    for row in rows:
        review_id = row["review_id"]
        scorer = row["scorer_id"]
        phase1_path = scoring_root / "phase1" / scorer / f"{review_id}.json"
        phase2_path = scoring_root / "phase2" / scorer / f"{review_id}.json"
        final_path = scoring_root / "final-judgments" / scorer / f"{review_id}.json"
        phase1_hash = write_locked_json(phase1_path, {"phase": 1, "review_id": review_id})
        phase2_hash = write_locked_json(phase2_path, {"phase": 2, "review_id": review_id})
        final_hash = write_locked_json(final_path, {"phase": "final", "review_id": review_id})
        phase1_items[review_id] = {
            "path": str(phase1_path), "sha256": phase1_hash,
            "scorer_id": scorer, "session_id": row["session_id"],
        }
        phase2_items[review_id] = {
            "path": str(phase2_path), "sha256": phase2_hash,
            "scorer_id": scorer, "session_id": row["session_id"],
        }
        final_items[review_id] = {
            "path": str(final_path), "sha256": final_hash, "scorer_id": scorer,
            "session_id": row["session_id"], "run_id": row["run_id"],
            "phase1_sha256": phase1_hash, "phase2_sha256": phase2_hash,
        }
        judgments[review_id] = {}
        provenance.append({"review_id": review_id, "path": str(final_path), "sha256": final_hash})

    phase1 = {
        "schema_version": 1, "phase": "correctness", "generation_id": generation_id,
        "generation_seal_sha256": generation["generation_seal_sha256"],
        "assignment_path": str(mapping_path), "assignment_file_sha256": mapping_file_hash,
        "assignment_seal_sha256": mapping["assignment_seal_sha256"],
        "judgment_count": 252, "judgments": phase1_items,
    }
    phase1["seal_sha256"] = MODULE.canonical_sha256(phase1)
    phase1_path = scoring_root / "phase1" / "phase1-seal.json"
    phase1_file_hash = write_locked_json(phase1_path, phase1)
    phase2 = {
        "schema_version": 1, "phase": "process", "generation_id": generation_id,
        "generation_seal_sha256": generation["generation_seal_sha256"],
        "assignment_path": str(mapping_path), "assignment_file_sha256": mapping_file_hash,
        "assignment_seal_sha256": mapping["assignment_seal_sha256"],
        "phase1_seal_path": str(phase1_path), "phase1_seal_file_sha256": phase1_file_hash,
        "phase1_seal_sha256": phase1["seal_sha256"],
        "process_bundle_bindings_by_scorer": process_bundle_bindings,
        "judgment_count": 252, "judgments": phase2_items,
    }
    phase2["seal_sha256"] = MODULE.canonical_sha256(phase2)
    phase2_path = scoring_root / "phase2" / "phase2-seal.json"
    phase2_file_hash = write_locked_json(phase2_path, phase2)
    manifest = {
        "schema_version": 1, "input_contract": "baseline-scoring-final-manifest-v1", "phase": "final-merged",
        "generation_id": generation_id, "generation_seal_sha256": generation["generation_seal_sha256"],
        "assignment_path": str(mapping_path), "assignment_file_sha256": mapping_file_hash,
        "assignment_seal_sha256": mapping["assignment_seal_sha256"],
        "ledger_path": str(ledger_path), "ledger_sha256": ledger_hash,
        "bundle_sha256_by_scorer": {scorer: bundle_bindings[scorer][1] for scorer in SCORERS},
        "phase1_seal_path": str(phase1_path), "phase1_seal_file_sha256": phase1_file_hash,
        "phase1_seal_sha256": phase1["seal_sha256"],
        "phase2_seal_path": str(phase2_path), "phase2_seal_file_sha256": phase2_file_hash,
        "phase2_seal_sha256": phase2["seal_sha256"],
        "process_bundle_bindings_by_scorer": process_bundle_bindings,
        "judgment_count": 252, "judgments": final_items,
    }
    manifest["seal_sha256"] = MODULE.canonical_sha256(manifest)
    manifest_path = scoring_root / "final-judgments" / "final-seal.json"
    write_locked_json(manifest_path, manifest)
    return {
        "manifest_path": manifest_path,
        "generation": generation,
        "ledger_path": ledger_path,
        "mapping_path": mapping_path,
        "mapping_value": mapping,
        "judgments_by_review": judgments,
        "judgment_provenance": provenance,
        "harness_root": harness_root,
    }


class AggregateBaselineMetricsTest(unittest.TestCase):
    def test_null_is_not_replaced_and_denominator_is_explicit(self) -> None:
        summary = MODULE.numeric_summary([1, None, 3], expected_n=3)
        self.assertEqual(summary["raw"], [1, None, 3])
        self.assertEqual(summary["valid_n"], 2)
        self.assertEqual(summary["missing_n"], 1)
        self.assertEqual(summary["denominator_n"], 3)
        self.assertEqual(summary["mean"], 2)
        self.assertAlmostEqual(summary["sample_sd"], 2**0.5)

    def test_duplicate_review_id_fails_closed(self) -> None:
        value = dataset()
        rows = value["coordinator_mapping"]["assignments"]
        rows[1]["review_id"] = rows[0]["review_id"]
        with self.assertRaisesRegex(MODULE.AggregationError, "duplicate review id"):
            aggregate(value)

    def test_missing_judgment_fails_closed(self) -> None:
        value = dataset()
        value["judgments_by_review"].pop(next(iter(value["judgments_by_review"])))
        with self.assertRaisesRegex(MODULE.AggregationError, "exactly match"):
            aggregate(value)

    def test_paired_delta_is_b2_minus_b1(self) -> None:
        value = dataset()
        set_token_total(current_metric(value, TASKS[0], "r1", "B1"), 10)
        set_token_total(current_metric(value, TASKS[0], "r1", "B2"), 7)
        result = aggregate(value)
        pair = next(row for row in result["paired_deltas"] if row["task_id"] == TASKS[0] and row["trial_id"] == "r1")
        self.assertEqual(pair["metrics"]["cost.tokens.total"]["delta_B2_minus_B1"], -3)
        self.assertEqual(pair["delta_direction"], "B2-B1")

    def test_agreement_statistics_report_denominators_and_degenerate_kappa(self) -> None:
        result = aggregate(dataset())
        summary = result["judgment_agreement"]["summary"]
        self.assertEqual(summary["three_way_unanimous_semantic_label"]["rate"], 1)
        self.assertEqual(summary["three_way_unanimous_semantic_label"]["denominator_n"], 84)
        self.assertEqual(summary["pairwise_semantic_label_agreement"]["agreement_rate"], 1)
        self.assertEqual(summary["pairwise_semantic_label_agreement"]["denominator_n"], 252)
        self.assertTrue(summary["fleiss_kappa_semantic_label"]["degenerate"])
        self.assertIsNone(summary["fleiss_kappa_semantic_label"]["value"])
        self.assertEqual(summary["mean_absolute_pairwise_score_difference"]["mean"], 0)

    def test_task_macro_and_session_weighted_are_distinct(self) -> None:
        value = dataset()
        for trial in TRIALS:
            current_metric(value, TASKS[0], trial, "B1")["cost"]["command_wall_ms"] = 100
            current_metric(value, TASKS[0], trial, "B2")["cost"]["command_wall_ms"] = 110
        current_metric(value, TASKS[1], "r1", "B1")["cost"]["command_wall_ms"] = 100
        current_metric(value, TASKS[1], "r1", "B2")["cost"]["command_wall_ms"] = 90
        for trial in ("r2", "r3"):
            current_metric(value, TASKS[1], trial, "B2")["cost"]["command_wall_ms"] = None
        result = aggregate(value)
        summary = result["overall_deltas"]["metrics"]["cost.command_wall_ms"]
        self.assertEqual(summary["session_weighted"]["mean"], 0.5)
        self.assertEqual(summary["task_macro"]["mean"], 0)
        self.assertNotEqual(summary["session_weighted"]["mean"], summary["task_macro"]["mean"])

    def test_variance_warning_conditions(self) -> None:
        warnings = MODULE.variance_warning_kinds([1, -2, 3])
        self.assertIn("mixed_sign", warnings)
        self.assertIn("sample_sd_at_least_absolute_mean", warnings)
        self.assertIn("leave_one_out_sign_flip", warnings)
        self.assertIn("mean_median_sign_mismatch", MODULE.variance_warning_kinds([-10, 1, 2]))
        self.assertIn("valid_pairs_below_3", MODULE.variance_warning_kinds([1, None, None]))

    def test_majority_rejects_malformed_and_preserves_no_majority(self) -> None:
        with self.assertRaisesRegex(MODULE.AggregationError, "malformed"):
            MODULE.majority_of_three(["correct", "partial", "malformed"], MODULE.SEMANTIC_LABELS, "label")
        self.assertIsNone(MODULE.majority_of_three(list(MODULE.SEMANTIC_LABELS), MODULE.SEMANTIC_LABELS, "label"))

    def test_item_disagreement_fails_closed(self) -> None:
        value = dataset()
        review_ids = list(value["judgments_by_review"])
        value["judgments_by_review"][review_ids[0]]["correctness"]["required_claims"][0]["item_id"] = "claim-other"
        with self.assertRaisesRegex(MODULE.AggregationError, "item sets disagree"):
            aggregate(value)

    def test_legacy_final_session_envelope_is_rejected(self) -> None:
        value = dataset()
        run_id = value["ledger"]["slots"][f"{TASKS[0]}:r1:B1"]["latest_run_id"]
        final_session = copy.deepcopy(value["metrics_by_run"][run_id])
        final_session["schema_version"] = 1
        final_session.pop("run")
        value["metrics_by_run"][run_id] = {
            "input_contract": "baseline-final-session-record-v1",
            "run_id": run_id,
            "session_metrics": final_session,
        }
        with self.assertRaisesRegex(MODULE.AggregationError, "legacy final session envelope"):
            aggregate(value)

        unsafe = dataset()
        unsafe_run_id = unsafe["ledger"]["slots"][f"{TASKS[0]}:r1:B1"]["latest_run_id"]
        unsafe["metrics_by_run"][unsafe_run_id]["schema_version"] = 1
        with self.assertRaisesRegex(MODULE.AggregationError, "schema_version must be 2"):
            aggregate(unsafe)

    def test_extractor_v2_requires_integrity_and_latest_publication(self) -> None:
        value = dataset()
        run_id = value["ledger"]["slots"][f"{TASKS[0]}:r1:B1"]["latest_run_id"]
        raw = value["metrics_by_run"][run_id]
        aggregate(value)

        raw["run"]["lifecycle"]["publication"]["latest_published_attempt"] = False
        with self.assertRaisesRegex(MODULE.AggregationError, "latest_published_attempt"):
            aggregate(value)

    def test_verified_internal_symlink_artifact_is_accepted(self) -> None:
        run_id = "T01-r1-B1-a1-symlink"
        value = metric(run_id, "T01", "r1", "B1", 0, "baseline-3x-synthetic")
        value["run"]["lifecycle"]["artifact_seal"]["symlink_paths"] = ["source/readme.md"]
        MODULE.validate_automatic_metric_consistency(value, run_id)
        value["run"]["lifecycle"]["artifact_seal"]["symlink_paths"] = [""]
        with self.assertRaises(MODULE.AggregationError):
            MODULE.validate_automatic_metric_consistency(value, run_id)

    def test_raw_v2_schema_rejects_spoofed_or_malformed_numeric_evidence(self) -> None:
        mutations = (
            ("additional field", lambda item: item["cost"].__setitem__("unexpected", 1), "additional property"),
            ("negative count", lambda item: item["cost"].__setitem__("tool_calls_total", -1), "below minimum"),
            ("float count", lambda item: item["cost"].__setitem__("model_steps", 2.0), "expected type"),
            ("missing integrity field", lambda item: item["run"]["integrity"].pop("issues"), "missing required property"),
        )
        for label, mutate, message in mutations:
            with self.subTest(label=label):
                value = dataset()
                mutate(current_metric(value, TASKS[0], "r1", "B1"))
                with self.assertRaisesRegex(MODULE.AggregationError, message):
                    aggregate(value)

    def test_raw_v2_token_family_timing_and_arm_hash_consistency_fail_closed(self) -> None:
        cases = (
            (
                "negative token",
                lambda item: item["cost"]["tokens"]["per_step"][0].__setitem__("input", -1),
                "below minimum",
            ),
            (
                "float token",
                lambda item: item["cost"]["tokens"]["per_step"][0].__setitem__("input", 4.0),
                "expected type",
            ),
            (
                "family mismatch",
                lambda item: item["cost"].__setitem__("tool_calls_by_family", {"read": 2}),
                "family counts mismatch",
            ),
            (
                "timing mismatch",
                lambda item: item["run"]["completed_tool_calls"][0].__setitem__("client_observed_elapsed_ms", 6),
                "elapsed time mismatch",
            ),
            (
                "arm config mismatch",
                lambda item: item["experiment"].__setitem__("arm_config_sha256", "f" * 64),
                "materialized arm config hash mismatch",
            ),
        )
        for label, mutate, message in cases:
            with self.subTest(label=label):
                value = dataset()
                mutate(current_metric(value, TASKS[0], "r1", "B1"))
                with self.assertRaisesRegex(MODULE.AggregationError, message):
                    aggregate(value)

    def test_zero_completed_model_steps_preserve_null_token_totals(self) -> None:
        value = dataset()
        item = current_metric(value, TASKS[0], "r1", "B1")
        item["cost"]["model_steps"] = 0
        tokens = item["cost"]["tokens"]
        tokens["official_entries"] = 0
        tokens["per_step"] = []
        for component in MODULE.TOKEN_COMPONENTS:
            tokens[component] = None
        result = aggregate(value)
        pair = next(row for row in result["paired_deltas"] if row["task_id"] == TASKS[0] and row["trial_id"] == "r1")
        self.assertIsNone(pair["metrics"]["cost.tokens.total"]["B1"])

    def test_mapping_run_dir_requires_exact_generation_parent_and_no_symlink(self) -> None:
        value = dataset()
        row = value["coordinator_mapping"]["assignments"][0]
        wrong = value["expected_runs_root"].parent / "wrong-parent" / Path(row["run_dir"]).name
        wrong.mkdir(parents=True)
        row["run_dir"] = str(wrong)
        with self.assertRaisesRegex(MODULE.AggregationError, "exact expected path"):
            aggregate(value)

        value = dataset()
        row = value["coordinator_mapping"]["assignments"][0]
        alias = value["expected_runs_root"].parent / "runs-alias"
        alias.symlink_to(value["expected_runs_root"])
        generation_id = value["generation"]["generation_id"]
        row["run_dir"] = str(alias / generation_id / Path(row["run_dir"]).name)
        with self.assertRaisesRegex(MODULE.AggregationError, "symlink"):
            aggregate(value)

    def test_navigation_behavior_preserves_sequence_and_aggregates(self) -> None:
        result = aggregate(dataset())
        raw = result["raw_sessions"][0]["navigation_behavior"]
        self.assertEqual(raw["completed_tool_family_sequence"], ["read", "grep"])
        self.assertEqual(raw["first_navigation_tool_family"], "read")
        self.assertEqual(raw["first_navigation_tool_origin"], "builtin")
        self.assertEqual(raw["codemap_first_use_completed_call_index"], 2)
        self.assertEqual(raw["origin_switch_count"], 1)
        overall_b1 = next(item for item in result["navigation_behavior"]["overall_by_arm"] if item["arm"] == "B1")
        self.assertEqual(overall_b1["first_navigation_tool_origin"]["counts"]["builtin"], 42)
        self.assertEqual(overall_b1["numeric"]["navigation.origin_switch_count"]["mean"], 1)

    def test_legacy_final_session_envelope_with_extra_field_is_rejected(self) -> None:
        value = dataset()
        run_id = value["ledger"]["slots"][f"{TASKS[0]}:r1:B1"]["latest_run_id"]
        final_session = copy.deepcopy(value["metrics_by_run"][run_id])
        final_session["schema_version"] = 1
        value["metrics_by_run"][run_id] = {
            "input_contract": "baseline-final-session-record-v1",
            "run_id": run_id,
            "session_metrics": final_session,
            "unsealed_alias": True,
        }
        with self.assertRaisesRegex(MODULE.AggregationError, "legacy final session envelope"):
            aggregate(value)

    def test_minimal_aggregate_output_is_rejected(self) -> None:
        with self.assertRaisesRegex(MODULE.AggregationError, "top-level keys mismatch"):
            MODULE.validate_aggregate_output({"schema_version": 1}, require_provenance=False)

    def test_attempt_accounting_preserves_superseded_attempts_and_arm_denominators(self) -> None:
        value = dataset()
        slot_key = f"{TASKS[0]}:r1:B1"
        slot = value["ledger"]["slots"][slot_key]
        latest_run_id = slot["latest_run_id"]
        superseded_run_id = f"{latest_run_id}-superseded"
        latest_attempt = value["ledger"]["attempts"][latest_run_id]
        superseded_attempt = copy.deepcopy(latest_attempt)
        superseded_attempt.update({
            "run_id": superseded_run_id,
            "attempt_number": 1,
            "classification": {
                "measurement_status": "infrastructure_invalid",
                "terminal_behavior": "infrastructure_error",
                "replacement_allowed": True,
                "replacement_category": "transient_provider",
                "generation_invalid": False,
            },
        })
        value["ledger"]["attempts"][superseded_run_id] = superseded_attempt
        latest_attempt["attempt_number"] = 2
        slot.update({"all_run_ids": [superseded_run_id, latest_run_id], "latest_attempt_number": 2})
        current = value["metrics_by_run"][latest_run_id]
        current["run"].update({"attempt_number": 2, "is_replacement": True})
        current["run"]["lifecycle"]["publication"]["all_run_ids"] = [superseded_run_id, latest_run_id]
        current["experiment"]["attempt_number"] = 2

        accounting = aggregate(value)["attempt_accounting"]
        self.assertEqual(accounting["raw_attempt_count"], 85)
        self.assertEqual(accounting["latest_valid_count"], 84)
        self.assertEqual(accounting["superseded_attempt_count"], 1)
        self.assertEqual(accounting["invalid_attempt_count"], 1)
        self.assertEqual(accounting["transient_attempt_count"], 1)
        self.assertEqual(accounting["replacement_attempt_count"], 1)
        self.assertEqual([row["latest_valid_count"] for row in accounting["by_arm"]], [42, 42])

    def test_attempt_accounting_rejects_orphan_attempt(self) -> None:
        value = dataset()
        original = next(iter(value["ledger"]["attempts"].values()))
        value["ledger"]["attempts"]["orphan"] = {**copy.deepcopy(original), "run_id": "orphan"}
        with self.assertRaisesRegex(MODULE.AggregationError, "exact tracked/canceled union"):
            aggregate(value)

    def test_cli_rejects_current_generation_drift_before_other_inputs(self) -> None:
        generation_path = TEMPORARY_ROOT / "drift-generation.json"
        forged_generation = {"schema_version": 1, "generation_kind": "baseline-3x", "execution_ready": True}
        forged_generation["generation_seal_sha256"] = MODULE.canonical_sha256(forged_generation)
        generation_path.write_text(json.dumps(forged_generation) + "\n", encoding="utf-8")
        arguments = [
            "--generation", str(generation_path), "--ledger", "/unread/ledger.json",
            "--metrics-index", "/unread/metrics.json", "--mapping", "/unread/assignments.json",
            "--scoring-manifest", "/unread/final-seal.json", "--judgments-index", "/unread/judgments.json",
            "--analysis-input-seal", "/unread/analysis-input-seal.json",
        ]
        completed = MODULE.subprocess.CompletedProcess(args=[], returncode=1, stdout="", stderr="current snapshot drift")
        with mock.patch.object(MODULE.subprocess, "run", return_value=completed) as run_mock:
            with mock.patch.object(MODULE, "verify_analysis_input_seal") as seal_mock:
                with contextlib.redirect_stderr(io.StringIO()):
                    self.assertEqual(MODULE.main(arguments), 2)
        self.assertTrue(run_mock.called)
        seal_mock.assert_not_called()

    def test_scoring_manifest_rejects_swapped_final_paths_even_when_resealed(self) -> None:
        fixture = scoring_manifest_fixture()
        MODULE.verify_scoring_manifest(**fixture)
        manifest_path = fixture["manifest_path"]
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        first, second = "review-000", "review-003"
        for key in ("path", "sha256"):
            manifest["judgments"][first][key], manifest["judgments"][second][key] = (
                manifest["judgments"][second][key], manifest["judgments"][first][key]
            )
        provenance = {item["review_id"]: item for item in fixture["judgment_provenance"]}
        for key in ("path", "sha256"):
            provenance[first][key], provenance[second][key] = provenance[second][key], provenance[first][key]
        manifest.pop("seal_sha256")
        manifest["seal_sha256"] = MODULE.canonical_sha256(manifest)
        manifest_path.chmod(0o644)
        write_locked_json(manifest_path, manifest)
        with self.assertRaisesRegex(MODULE.AggregationError, "assignment-derived"):
            MODULE.verify_scoring_manifest(**fixture)

    def test_scoring_manifest_rejects_process_bundle_hash_and_mode_drift(self) -> None:
        for drift, message in (("hash", "hash mismatch"), ("mode", "mode mismatch")):
            with self.subTest(drift=drift):
                fixture = scoring_manifest_fixture()
                MODULE.verify_scoring_manifest(**fixture)
                manifest = json.loads(fixture["manifest_path"].read_text(encoding="utf-8"))
                bundle_path = Path(manifest["process_bundle_bindings_by_scorer"]["scorer-1"]["path"])
                if drift == "hash":
                    bundle_path.chmod(0o644)
                    bundle_path.write_text('{"drift":true}\n', encoding="utf-8")
                    bundle_path.chmod(0o444)
                else:
                    bundle_path.chmod(0o400)
                with self.assertRaisesRegex(MODULE.AggregationError, message):
                    MODULE.verify_scoring_manifest(**fixture)

    def test_process_majority_gates_post_first_wrong_cost(self) -> None:
        value = dataset()
        run_id = value["ledger"]["slots"][f"{TASKS[0]}:r1:B1"]["latest_run_id"]
        rows = [row for row in value["coordinator_mapping"]["assignments"] if Path(row["run_dir"]).name == run_id]
        for row in rows[:2]:
            first_wrong = value["judgments_by_review"][row["review_id"]]["dialogue"]["first_wrong"]
            first_wrong.update({
                "category": "tool_choice",
                "raw_event_line": 2,
                "completed_call_index": 1,
                "call_id": f"{run_id}-call-1",
                "explanation": "wrong tool",
            })
        result = aggregate(value)
        raw = next(item for item in result["raw_sessions"] if item["run_id"] == run_id)
        self.assertEqual(raw["post_first_wrong_derivation"]["values"]["tool_calls"], 1)
        self.assertEqual(raw["post_first_wrong_derivation"]["values"]["tool_output_bytes"], 30)
        self.assertEqual(raw["post_first_wrong_derivation"]["values"]["tokens"], 6)
        self.assertIsNone(raw["post_first_wrong_derivation"]["values"]["wall_ms"])
        self.assertEqual(raw["post_first_wrong_derivation"]["component_denominators"]["wall_ms"], {"eligible_n": 1, "valid_n": 0, "missing_n": 1})

    def test_process_boundaries_are_canonical_and_majority_is_atomic(self) -> None:
        value = dataset()
        run_id = value["ledger"]["slots"][f"{TASKS[0]}:r1:B1"]["latest_run_id"]
        rows = [row for row in value["coordinator_mapping"]["assignments"] if Path(row["run_dir"]).name == run_id]
        judgments = [value["judgments_by_review"][row["review_id"]] for row in rows]
        boundaries = [
            ("tool_choice", 1, f"{run_id}-call-1", 2),
            ("tool_choice", 2, f"{run_id}-call-2", 5),
            ("scope", 1, f"{run_id}-call-1", 2),
        ]
        for judgment_value, (category, index, call_id, line) in zip(judgments, boundaries):
            judgment_value["dialogue"]["first_wrong"].update({
                "category": category, "completed_call_index": index, "call_id": call_id,
                "raw_event_line": line, "explanation": "wrong boundary",
            })
        result = aggregate(value)
        raw = next(item for item in result["raw_sessions"] if item["run_id"] == run_id)
        self.assertIsNone(raw["judgment_agreement"]["majority_first_wrong_category"])
        self.assertEqual(raw["judgment_agreement"]["first_wrong"]["selected_position"]["status"], "no_majority_position")
        self.assertIsNone(raw["post_first_wrong_derivation"]["values"]["tool_calls"])

        for field, invalid in (("call_id", "missing-call"), ("completed_call_index", 2), ("raw_event_line", 5)):
            malformed = dataset()
            malformed_run_id = malformed["ledger"]["slots"][f"{TASKS[0]}:r1:B1"]["latest_run_id"]
            row = next(row for row in malformed["coordinator_mapping"]["assignments"] if Path(row["run_dir"]).name == malformed_run_id)
            first_wrong = malformed["judgments_by_review"][row["review_id"]]["dialogue"]["first_wrong"]
            first_wrong.update({
                "category": "tool_choice", "completed_call_index": 1,
                "call_id": f"{malformed_run_id}-call-1", "raw_event_line": 2,
                "explanation": "bad tuple",
            })
            first_wrong[field] = invalid
            with self.subTest(field=field), self.assertRaises(MODULE.AggregationError):
                aggregate(malformed)

        mismatched_stage = dataset()
        mismatched_run_id = mismatched_stage["ledger"]["slots"][f"{TASKS[0]}:r1:B1"]["latest_run_id"]
        row = next(row for row in mismatched_stage["coordinator_mapping"]["assignments"] if Path(row["run_dir"]).name == mismatched_run_id)
        stage = mismatched_stage["judgments_by_review"][row["review_id"]]["dialogue"]["discovery_stages"][MODULE.DISCOVERY_STAGES[0]]
        stage["call_ids"] = [f"{mismatched_run_id}-call-2"]
        with self.assertRaises(MODULE.AggregationError):
            aggregate(mismatched_stage)


if __name__ == "__main__":
    unittest.main()
