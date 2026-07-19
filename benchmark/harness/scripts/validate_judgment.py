#!/usr/bin/env python3
"""Fail-closed validation for separate correctness and process judgments."""
from __future__ import annotations

import hashlib
import pathlib
import sys
from typing import Any

from common import BENCHMARK_ROOT, HARNESS_ROOT, canonical_sha256, load_json, sha256, write_json


ALLOWED_FRACTIONS = {0, 0.5, 1}
STAGE_NAMES = ("decisive_evidence_exposed", "selected_next", "original_code_read", "final_answer_used_correctly")
STAGE_STATUSES = {"yes", "no", "not_applicable", "unscored"}
FIRST_WRONG_CATEGORIES = {
    "none", "initial_area", "tool_choice", "query_too_broad", "query_too_narrow", "scope",
    "answer_evidence_not_exposed", "insufficient_code_evidence", "similar_result_dominance",
    "evidence_not_selected", "original_not_read", "final_misuse", "other",
}
NAVIGATION_FAMILIES = {"overview", "search", "grep", "read", "find", "glob"}


def exact_keys(value: Any, expected: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != expected:
        actual = sorted(value) if isinstance(value, dict) else type(value).__name__
        raise SystemExit(f"{label} keys mismatch: {actual}")


def load_assignment(path: pathlib.Path, expected_runs_root: pathlib.Path) -> dict:
    assignment = load_json(path)
    core = dict(assignment)
    recorded = core.pop("assignment_seal_sha256", None)
    if recorded != canonical_sha256(core):
        raise SystemExit("assignment manifest self-seal mismatch")
    rows = assignment.get("assignments")
    if not isinstance(rows, list) or len(rows) != assignment.get("expected_judgment_count") or len(rows) != 252:
        raise SystemExit("assignment manifest must contain exactly 252 rows")
    review_ids = [row.get("review_id") for row in rows]
    if len(set(review_ids)) != len(review_ids):
        raise SystemExit("assignment review ids are not globally unique")
    sessions: dict[str, list[dict]] = {}
    expected_runs_root = expected_runs_root.absolute()
    recorded_runs_root = pathlib.Path(assignment.get("runs_root", ""))
    if (
        not recorded_runs_root.is_absolute()
        or str(recorded_runs_root) != str(expected_runs_root)
        or recorded_runs_root.is_symlink()
        or recorded_runs_root.resolve() != expected_runs_root
    ):
        raise SystemExit("assignment runs_root is not the exact expected generation runs directory")
    expected_ledger = expected_runs_root / "ledger.json"
    ledger_path = pathlib.Path(assignment.get("ledger_path", ""))
    if (
        str(ledger_path) != str(expected_ledger)
        or ledger_path.is_symlink()
        or not ledger_path.is_file()
        or sha256(ledger_path) != assignment.get("ledger_sha256")
        or ledger_path.stat().st_mode & 0o222
    ):
        raise SystemExit("assignment ledger path/hash/read-only contract mismatch")
    ledger = load_json(ledger_path)
    if ledger.get("state") != "completed" or ledger.get("generation_id") != assignment.get("generation_id") or ledger.get("generation_seal_sha256") != assignment.get("generation_seal_sha256"):
        raise SystemExit("assignment ledger is not the completed sealed generation ledger")
    for row in rows:
        sessions.setdefault(row.get("session_id"), []).append(row)
        run_id = row.get("run_id")
        expected_run_dir = expected_runs_root / str(run_id)
        run_dir = pathlib.Path(row.get("run_dir", ""))
        if (
            not isinstance(run_id, str) or not run_id
            or str(run_dir) != str(expected_run_dir)
            or run_dir.is_symlink()
            or not run_dir.is_dir()
            or run_dir.resolve() != expected_run_dir
        ):
            raise SystemExit("assignment run_dir is not the exact non-symlink latest run path")
        slot_key = f"{row['task_id']}:{row['trial_id']}:{row['arm']}"
        slot = ledger.get("slots", {}).get(slot_key, {})
        attempt = ledger.get("attempts", {}).get(run_id, {})
        if (
            slot.get("latest_run_id") != run_id
            or slot.get("measurement_status") != "valid"
            or slot.get("metrics_status") != "sealed"
            or attempt.get("state") != "terminal"
            or attempt.get("run_dir") != str(expected_run_dir)
        ):
            raise SystemExit("assignment does not point to the latest published valid metrics-sealed attempt")
        bundle_path = pathlib.Path(row["bundle_path"])
        if not bundle_path.is_file() or sha256(bundle_path) != row["bundle_sha256"]:
            raise SystemExit("assigned public bundle hash mismatch")
        answer_path = BENCHMARK_ROOT / "answers/development" / f"{row['task_id']}.json"
        if sha256(answer_path) != row["answer_contract_sha256"]:
            raise SystemExit("assigned answer contract hash mismatch")
        verify_scoring_items(row["scoring_items"], row["answer_contract_sha256"])
    if len(sessions) != assignment.get("expected_session_count") or len(sessions) != 84:
        raise SystemExit("assignment manifest must contain exactly 84 sessions")
    if any(
        len(items) != 3
        or {item["scorer_slot"] for item in items} != {1, 2, 3}
        or len({item["scorer_id"] for item in items}) != 3
        or len({(item["run_id"], item["run_dir"], item["task_id"], item["trial_id"], item["arm"]) for item in items}) != 1
        for items in sessions.values()
    ):
        raise SystemExit("one output is not assigned to three distinct scorers")
    return assignment


def verify_scoring_items(items: dict, answer_sha256: str) -> None:
    core = dict(items)
    recorded = core.pop("scoring_items_sha256", None)
    if recorded != canonical_sha256(core):
        raise SystemExit("derived scoring items self-hash mismatch")
    if items.get("source_answer_sha256") != answer_sha256:
        raise SystemExit("derived scoring items refer to another answer contract")
    for category, prefix in (
        ("required_claims", "required_claim"),
        ("required_relationships", "relationship"),
        ("prohibited_claims", "prohibited"),
    ):
        values = items.get(category)
        if not isinstance(values, list) or (category != "prohibited_claims" and not values):
            raise SystemExit(f"invalid derived item list: {category}")
        expected_ids = [f"{prefix}:{index}" for index in range(len(values))]
        if [item.get("item_id") for item in values] != expected_ids:
            raise SystemExit(f"derived item ids are not exact/ordered: {category}")
        for item in values:
            if item.get("source_answer_sha256") != answer_sha256:
                raise SystemExit("derived item source hash mismatch")
            text = item.get("text")
            if not isinstance(text, str) or hashlib.sha256(text.encode("utf-8")).hexdigest() != item.get("text_sha256"):
                raise SystemExit("derived item text hash mismatch")


def assignment_row(assignment: dict, review_id: str, scorer_id: str) -> dict:
    matches = [row for row in assignment["assignments"] if row["review_id"] == review_id]
    if len(matches) != 1:
        raise SystemExit("judgment review id is unassigned or duplicated")
    row = matches[0]
    if row["scorer_id"] != scorer_id:
        raise SystemExit("judgment scorer does not own this review slot")
    return row


def load_phase1_seal(path: pathlib.Path, assignment: dict) -> dict:
    seal = load_json(path)
    core = dict(seal)
    recorded = core.pop("seal_sha256", None)
    if recorded != canonical_sha256(core):
        raise SystemExit("phase-1 seal self-hash mismatch")
    if (
        seal.get("phase") != "correctness"
        or seal.get("generation_id") != assignment["generation_id"]
        or seal.get("generation_seal_sha256") != assignment["generation_seal_sha256"]
        or seal.get("assignment_seal_sha256") != assignment["assignment_seal_sha256"]
    ):
        raise SystemExit("phase-1 seal/assignment mismatch")
    expected = {row["review_id"] for row in assignment["assignments"]}
    judgments = seal.get("judgments", {})
    if set(judgments) != expected or len(judgments) != 252:
        raise SystemExit("phase-1 seal does not contain exactly 252 assigned judgments")
    rows = {row["review_id"]: row for row in assignment["assignments"]}
    hashes = []
    for review_id, item in judgments.items():
        judgment_path = pathlib.Path(item["path"])
        if item.get("scorer_id") != rows[review_id]["scorer_id"] or item.get("session_id") != rows[review_id]["session_id"]:
            raise SystemExit("phase-1 seal identity mismatch")
        if not judgment_path.is_file() or sha256(judgment_path) != item.get("sha256") or judgment_path.stat().st_mode & 0o222:
            raise SystemExit("phase-1 sealed judgment drift or writable file")
        hashes.append(item["sha256"])
    if len(set(hashes)) != 252:
        raise SystemExit("one phase-1 judgment file/hash cannot count twice")
    return seal


def validate_evidence_lines(lines: Any, maximum: int | None, label: str) -> None:
    if not isinstance(lines, list) or any(not isinstance(line, int) or isinstance(line, bool) or line < 1 for line in lines):
        raise SystemExit(f"{label} raw_event_lines are invalid")
    if len(set(lines)) != len(lines):
        raise SystemExit(f"{label} raw_event_lines contain duplicates")
    if maximum is not None and any(line > maximum for line in lines):
        raise SystemExit(f"{label} cites an event beyond the accepted reducer boundary")


def validate_scored_list(actual: Any, expected: list[dict], prohibited: bool = False) -> None:
    if not isinstance(actual, list) or len(actual) != len(expected):
        raise SystemExit("judgment item count differs from the assigned answer contract")
    actual_ids = [item.get("item_id") for item in actual if isinstance(item, dict)]
    expected_ids = [item["item_id"] for item in expected]
    if len(actual_ids) != len(actual) or len(set(actual_ids)) != len(actual_ids) or actual_ids != expected_ids:
        raise SystemExit("judgment item ids/order differ from the assigned answer contract")
    for item in actual:
        if prohibited:
            exact_keys(item, {"item_id", "explicitly_present", "evidence", "raw_event_lines"}, "prohibited item")
            if not isinstance(item["explicitly_present"], bool):
                raise SystemExit("prohibited item flag must be boolean")
        else:
            exact_keys(item, {"item_id", "fraction", "evidence", "raw_event_lines"}, "scored item")
            if item["fraction"] not in ALLOWED_FRACTIONS:
                raise SystemExit("scored item fraction violates the rubric")
        if not isinstance(item["evidence"], str) or not item["evidence"].strip():
            raise SystemExit("every scored decision needs written evidence")
        validate_evidence_lines(item["raw_event_lines"], None, item["item_id"])


def validate_phase1(value: dict, assignment: dict) -> dict:
    exact_keys(value, {"schema_version", "review_id", "scorer_id", "rubric_sha256", "correctness", "notes"}, "phase-1 judgment")
    if value["schema_version"] != 1 or not isinstance(value["review_id"], str) or not isinstance(value["scorer_id"], str):
        raise SystemExit("invalid phase-1 judgment identity")
    row = assignment_row(assignment, value["review_id"], value["scorer_id"])
    rubric_path = HARNESS_ROOT / "config/scoring-rubric.json"
    if value["rubric_sha256"] != assignment["rubric_sha256"] or value["rubric_sha256"] != row["rubric_sha256"] or value["rubric_sha256"] != sha256(rubric_path):
        raise SystemExit("phase-1 judgment/rubric hash mismatch")
    correctness = value["correctness"]
    exact_keys(
        correctness,
        {"core_fraction", "core_evidence", "required_claims", "grounding_fraction", "grounding_evidence", "required_relationships", "prohibited_claims", "format_valid_json"},
        "phase-1 correctness",
    )
    if correctness["core_fraction"] not in ALLOWED_FRACTIONS or correctness["grounding_fraction"] not in ALLOWED_FRACTIONS:
        raise SystemExit("core or grounding fraction violates the rubric")
    if not isinstance(correctness["core_evidence"], str) or not correctness["core_evidence"].strip() or not isinstance(correctness["grounding_evidence"], str) or not correctness["grounding_evidence"].strip():
        raise SystemExit("core and grounding decisions require final-answer or answer-contract evidence")
    expected = row["scoring_items"]
    validate_scored_list(correctness["required_claims"], expected["required_claims"])
    validate_scored_list(correctness["required_relationships"], expected["required_relationships"])
    validate_scored_list(correctness["prohibited_claims"], expected["prohibited_claims"], prohibited=True)
    if any(item["raw_event_lines"] for category in ("required_claims", "required_relationships", "prohibited_claims") for item in correctness[category]):
        raise SystemExit("phase-1 correctness may not cite process events that were not disclosed")
    if not isinstance(correctness["format_valid_json"], bool) or not isinstance(value["notes"], str):
        raise SystemExit("phase-1 format metric or notes type is invalid")
    claim_mean = sum(item["fraction"] for item in correctness["required_claims"]) / len(correctness["required_claims"])
    relationship_mean = sum(item["fraction"] for item in correctness["required_relationships"]) / len(correctness["required_relationships"])
    explicit_count = sum(item["explicitly_present"] for item in correctness["prohibited_claims"])
    score = max(0, min(100, 40 * correctness["core_fraction"] + 30 * claim_mean + 15 * correctness["grounding_fraction"] + 15 * relationship_mean - min(40, 20 * explicit_count)))
    if correctness["core_fraction"] == 1 and explicit_count == 0 and score >= 85:
        semantic_label = "correct"
    elif correctness["core_fraction"] > 0 and score >= 50:
        semantic_label = "partial"
    else:
        semantic_label = "incorrect"
    contract_complete = (
        correctness["grounding_fraction"] == 1
        and all(item["fraction"] == 1 for item in correctness["required_claims"] + correctness["required_relationships"])
    )
    validated = dict(value)
    validated["correctness"] = {
        **correctness,
        "score_0_100": score,
        "semantic_label": semantic_label,
        "contract_complete": contract_complete,
        "contract_label": "complete" if contract_complete else "incomplete",
        "scorer_id": value["scorer_id"],
        "manual_status": "scored",
    }
    validated["phase"] = "correctness"
    validated["assignment_seal_sha256"] = assignment["assignment_seal_sha256"]
    return validated


def canonical_navigation_call_identities(assignment: dict, row: dict, maximum_line: int) -> list[dict[str, Any]]:
    """Load the sealed automatic metric and expose one canonical tuple per navigation call."""
    run_dir = pathlib.Path(row["run_dir"])
    run_id = row["run_id"]
    metric_path = run_dir.parent / "automatic-metrics" / run_id / "automatic-run-metrics.json"
    ledger_path = pathlib.Path(assignment["ledger_path"])
    if sha256(ledger_path) != assignment.get("ledger_sha256"):
        raise SystemExit("phase-2 ledger binding changed after assignment validation")
    ledger = load_json(ledger_path)
    slot_key = f"{row['task_id']}:{row['trial_id']}:{row['arm']}"
    slot = ledger.get("slots", {}).get(slot_key, {})
    attempt = ledger.get("attempts", {}).get(run_id, {})
    metric_hash = sha256(metric_path) if metric_path.is_file() else None
    if (
        metric_path.is_symlink()
        or not metric_path.is_file()
        or metric_path.stat().st_mode & 0o222
        or attempt.get("automatic_metrics_path") != str(metric_path)
        or slot.get("latest_automatic_metrics_path") != str(metric_path)
        or attempt.get("automatic_metrics_sha256") != metric_hash
        or slot.get("latest_automatic_metrics_sha256") != metric_hash
    ):
        raise SystemExit("phase-2 canonical call table is not bound to the sealed automatic metric")
    metric = load_json(metric_path)
    run = metric.get("run")
    if not isinstance(run, dict) or run.get("run_id") != run_id:
        raise SystemExit("phase-2 automatic metric/run identity mismatch")
    calls = run.get("completed_tool_calls")
    if not isinstance(calls, list):
        raise SystemExit("phase-2 automatic metric lacks completed tool calls")
    indexes = [call.get("completed_call_index") for call in calls if isinstance(call, dict)]
    if len(indexes) != len(calls) or indexes != list(range(1, len(calls) + 1)):
        raise SystemExit("phase-2 completed-call order is not canonical")
    identities = []
    for call in calls:
        call_id = call.get("call_id")
        session_id = call.get("session_id")
        raw_line = call.get("selected_completion_line")
        raw_lines = call.get("raw_event_lines")
        if (
            call.get("completed") is not True
            or not isinstance(call_id, str)
            or not call_id
            or not isinstance(session_id, str)
            or call.get("identity") != f"{session_id}:tool:{call_id}"
            or not isinstance(raw_line, int)
            or isinstance(raw_line, bool)
            or raw_line < 1
            or raw_line > maximum_line
            or not isinstance(raw_lines, list)
            or raw_line not in raw_lines
        ):
            raise SystemExit("phase-2 completed-call identity is not tied to an accepted terminal event")
        if call.get("family") in NAVIGATION_FAMILIES:
            identities.append({
                "identity": call["identity"],
                "completed_call_index": call["completed_call_index"],
                "call_id": call_id,
                "raw_event_line": raw_line,
                "family": call["family"],
            })
    return identities


def validate_stage(value: Any, maximum_line: int, label: str, canonical_calls: list[dict[str, Any]]) -> None:
    exact_keys(value, {"status", "evidence", "raw_event_lines", "call_ids"}, label)
    if value["status"] not in STAGE_STATUSES or value["status"] == "unscored" or not isinstance(value["evidence"], str) or not value["evidence"].strip():
        raise SystemExit(f"invalid dialogue stage: {label}")
    validate_evidence_lines(value["raw_event_lines"], maximum_line, label)
    if not isinstance(value["call_ids"], list) or any(not isinstance(item, str) or not item for item in value["call_ids"]) or len(set(value["call_ids"])) != len(value["call_ids"]):
        raise SystemExit(f"invalid call ids: {label}")
    cited_lines = set(value["raw_event_lines"])
    expected_call_ids = [call["call_id"] for call in canonical_calls if call["raw_event_line"] in cited_lines]
    if value["call_ids"] != expected_call_ids:
        raise SystemExit(f"{label} call ids do not match cited completed navigation calls in canonical order")


def validate_phase2(value: dict, assignment: dict, phase1_seal: dict) -> dict:
    exact_keys(value, {"schema_version", "review_id", "scorer_id", "rubric_sha256", "phase1_judgment_sha256", "dialogue", "notes"}, "phase-2 judgment")
    if value["schema_version"] != 1:
        raise SystemExit("invalid phase-2 schema version")
    row = assignment_row(assignment, value["review_id"], value["scorer_id"])
    if value["rubric_sha256"] != assignment["rubric_sha256"] or value["rubric_sha256"] != row["rubric_sha256"]:
        raise SystemExit("phase-2 judgment/rubric hash mismatch")
    sealed_phase1 = phase1_seal.get("judgments", {}).get(value["review_id"])
    if not sealed_phase1 or sealed_phase1.get("scorer_id") != row["scorer_id"] or value["phase1_judgment_sha256"] != sealed_phase1.get("sha256"):
        raise SystemExit("phase-2 judgment is not bound to the sealed phase-1 score")
    run_dir = pathlib.Path(row["run_dir"])
    maximum_line = int(load_json(run_dir / "wrapper.json").get("reducer_lines_accepted", 0))
    canonical_calls = canonical_navigation_call_identities(assignment, row, maximum_line)
    dialogue = value["dialogue"]
    exact_keys(dialogue, {"discovery_stages", "first_wrong"}, "dialogue")
    stages = dialogue["discovery_stages"]
    exact_keys(stages, set(STAGE_NAMES), "discovery stages")
    for name in STAGE_NAMES:
        validate_stage(stages[name], maximum_line, name, canonical_calls)
    first = dialogue["first_wrong"]
    exact_keys(first, {"category", "raw_event_line", "completed_call_index", "call_id", "explanation"}, "first wrong")
    if first["category"] not in FIRST_WRONG_CATEGORIES or not isinstance(first["explanation"], str):
        raise SystemExit("invalid first-wrong decision")
    raw_line = first["raw_event_line"]
    if raw_line is not None and (not isinstance(raw_line, int) or isinstance(raw_line, bool) or raw_line < 1 or raw_line > maximum_line):
        raise SystemExit("first-wrong raw line is outside the accepted event boundary")
    call_index = first["completed_call_index"]
    if call_index is not None and (not isinstance(call_index, int) or isinstance(call_index, bool) or call_index < 1):
        raise SystemExit("first-wrong completed-call index is invalid")
    if first["call_id"] is not None and (not isinstance(first["call_id"], str) or not first["call_id"]):
        raise SystemExit("first-wrong call id is invalid")
    if first["category"] == "none":
        if any(first[key] is not None for key in ("raw_event_line", "completed_call_index", "call_id")):
            raise SystemExit("first-wrong none must not point at an event or call")
    else:
        if any(first[key] is None for key in ("raw_event_line", "completed_call_index", "call_id")) or not first["explanation"].strip():
            raise SystemExit("a first-wrong finding requires one complete canonical call identity and explanation")
        boundary = {
            "completed_call_index": first["completed_call_index"],
            "call_id": first["call_id"],
            "raw_event_line": first["raw_event_line"],
        }
        if not any(all(call[key] == boundary[key] for key in boundary) for call in canonical_calls):
            raise SystemExit("first-wrong identity is not one accepted completed navigation call")
    if not isinstance(value["notes"], str):
        raise SystemExit("phase-2 notes type is invalid")
    return {
        **value,
        "phase": "process",
        "assignment_seal_sha256": assignment["assignment_seal_sha256"],
    }


def main() -> int:
    if len(sys.argv) == 6 and sys.argv[1] == "phase1":
        input_path, output_path, assignment_path = map(pathlib.Path, sys.argv[2:5])
        # The sixth argument is a literal mode guard to prevent legacy invocation ambiguity.
        if sys.argv[5] != "correctness-only":
            raise SystemExit("phase-1 mode guard must be correctness-only")
        assignment_value = load_json(assignment_path)
        assignment = load_assignment(assignment_path, HARNESS_ROOT / "runs" / assignment_value["generation_id"])
        write_json(output_path, validate_phase1(load_json(input_path), assignment))
        return 0
    if len(sys.argv) == 7 and sys.argv[1] == "phase2":
        input_path, output_path, assignment_path, phase1_seal_path = map(pathlib.Path, sys.argv[2:6])
        if sys.argv[6] != "process-only":
            raise SystemExit("phase-2 mode guard must be process-only")
        assignment_value = load_json(assignment_path)
        assignment = load_assignment(assignment_path, HARNESS_ROOT / "runs" / assignment_value["generation_id"])
        phase1_seal = load_phase1_seal(phase1_seal_path, assignment)
        write_json(output_path, validate_phase2(load_json(input_path), assignment, phase1_seal))
        return 0
    raise SystemExit(
        "usage: validate_judgment.py phase1 INPUT OUTPUT ASSIGNMENTS correctness-only | "
        "phase2 INPUT OUTPUT ASSIGNMENTS PHASE1_SEAL process-only"
    )


if __name__ == "__main__":
    raise SystemExit(main())
