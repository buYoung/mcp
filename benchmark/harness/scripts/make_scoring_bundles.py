#!/usr/bin/env python3
"""Create immutable three-scorer correctness bundles, then gated process bundles."""
from __future__ import annotations

import hashlib
import json
import pathlib
import random
import re
import secrets
import sys
from typing import Any

from common import BENCHMARK_ROOT, HARNESS_ROOT, TASK_IDS, canonical_sha256, load_json, sha256, write_json
from generation import verify as verify_generation
from protocol import verify_artifact_seal
from validate_judgment import canonical_navigation_call_identities, load_assignment


SCORERS = ("scorer-1", "scorer-2", "scorer-3")
FAMILY_NAMES = ("search", "read", "grep", "find", "overview")
FORBIDDEN_PUBLIC_KEYS = {
    "arm", "criterion", "trial_id", "repeat_id", "pair_id", "pair_order", "pair_order_index",
    "config", "provider", "mcp", "run_dir", "run_id", "session_id", "scorer_slot",
}


def valid_runs(generation: dict) -> list[pathlib.Path]:
    runs_root = HARNESS_ROOT / "runs" / generation["generation_id"]
    ledger = load_json(runs_root / "ledger.json")
    if ledger.get("generation_id") != generation["generation_id"] or ledger.get("generation_seal_sha256") != generation["generation_seal_sha256"]:
        raise SystemExit("scoring ledger/generation mismatch")
    if ledger.get("state") != "completed":
        raise SystemExit("all 84 valid observed outputs must exist before blinding")
    expected_slots = {
        f"{row['task_id']}:{row['trial_id']}:{row['criterion']}"
        for row in generation["schedule"]
    }
    if set(ledger.get("slots", {})) != expected_slots:
        raise SystemExit("ledger slot set differs from the sealed schedule")
    paths = []
    for slot_key in sorted(expected_slots):
        slot = ledger["slots"][slot_key]
        if slot.get("measurement_status") != "valid" or slot.get("replacement_allowed") is not False:
            raise SystemExit("scoring input is not a sealed valid measurement")
        run_id = slot.get("latest_run_id")
        attempt = ledger.get("attempts", {}).get(run_id, {})
        run_dir = runs_root / str(run_id)
        if attempt.get("state") != "terminal" or pathlib.Path(attempt.get("run_dir", "")).resolve() != run_dir.resolve():
            raise SystemExit("latest valid attempt was not published from its claimed run directory")
        verify_artifact_seal(run_dir)
        if attempt.get("artifact_manifest_sha256") != sha256(run_dir / "artifact-manifest.json"):
            raise SystemExit("ledger artifact seal differs from the run manifest")
        paths.append(run_dir)
    if len(paths) != generation["session_count"]:
        raise SystemExit("expected exactly 84 latest valid runs")
    return paths


def normalized_text(text: str, run_dir: pathlib.Path) -> str:
    source = str(run_dir / "source")
    text = text.replace(source + "/", "<SOURCE>/").replace(source, "<SOURCE>")
    text = text.replace(str(run_dir), "<REDACTED_PATH>")
    text = re.sub(r"/private/tmp/codemap-search-quality\.[^/]+/harness/runs/[^/]+/[^/]+/source/?", "<SOURCE>/", text)
    return text


def final_answer(run_dir: pathlib.Path) -> str:
    wrapper = load_json(run_dir / "wrapper.json")
    return normalized_text(str(wrapper.get("final_assistant_text", "")), run_dir)


def family(tool: str) -> str:
    lower = tool.casefold()
    for name in FAMILY_NAMES:
        if name in lower:
            return name
    return "navigation_other"


def neutralize(value: Any, run_dir: pathlib.Path) -> Any:
    if isinstance(value, str):
        return normalized_text(value, run_dir)
    if isinstance(value, list):
        return [neutralize(item, run_dir) for item in value]
    if isinstance(value, dict):
        result = {}
        for key, item in value.items():
            if key in FORBIDDEN_PUBLIC_KEYS:
                continue
            if key == "tool" and isinstance(item, str):
                result[key] = family(item)
            else:
                result[key] = neutralize(item, run_dir)
        return result
    return value


def scored_items(prefix: str, values: list[str], source_answer_sha256: str) -> list[dict[str, str]]:
    return [
        {
            "item_id": f"{prefix}:{index}",
            "text": text,
            "text_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
            "source_answer_sha256": source_answer_sha256,
        }
        for index, text in enumerate(values)
    ]


def derive_scoring_items(answer: dict, source_answer_sha256: str, rubric: dict) -> dict:
    required_claims = answer.get("final_answer_required")
    relationships = answer.get("required_file_relationships")
    prohibited = answer.get("prohibited_claims")
    if not isinstance(required_claims, list) or not required_claims or not all(isinstance(item, str) and item for item in required_claims):
        raise SystemExit("answer contract requires a non-empty final_answer_required string list")
    if not isinstance(relationships, list) or not relationships or not all(isinstance(item, str) and item for item in relationships):
        raise SystemExit("answer contract requires a non-empty required_file_relationships string list")
    if not isinstance(prohibited, list) or not all(isinstance(item, str) and item for item in prohibited):
        raise SystemExit("answer contract prohibited_claims must be a string list")
    core = {
        "correct_file": answer.get("correct_file"),
        "correct_symbol": answer.get("correct_symbol"),
        "decisive_code_evidence": answer.get("decisive_code_evidence"),
        "fraction_definition": rubric["score_components"]["core_conclusion"]["fraction_definition"],
    }
    grounding = {
        "correct_file": answer.get("correct_file"),
        "essential_correct_symbols": answer.get("correct_symbol"),
        "fraction_definition": rubric["score_components"]["main_file_symbol_grounding"]["fraction_definition"],
    }
    if not core["correct_file"] or not core["correct_symbol"] or not core["decisive_code_evidence"]:
        raise SystemExit("answer contract core fields are incomplete")
    result = {
        "source_answer_sha256": source_answer_sha256,
        "core_contract": core,
        "grounding_contract": grounding,
        "required_claims": scored_items("required_claim", required_claims, source_answer_sha256),
        "required_relationships": scored_items("relationship", relationships, source_answer_sha256),
        "prohibited_claims": scored_items("prohibited", prohibited, source_answer_sha256),
        "required_item_fraction_definition": rubric["score_components"]["required_content_mean"]["item_fraction_definition"],
        "relationship_item_fraction_definition": rubric["score_components"]["required_relationship_mean"]["item_fraction_definition"],
        "prohibited_explicit_definition": rubric["prohibited_claim_penalty"]["explicit_definition"],
    }
    result["scoring_items_sha256"] = canonical_sha256(result)
    return result


def public_keys(value: Any) -> set[str]:
    if isinstance(value, dict):
        return set(value) | {key for item in value.values() for key in public_keys(item)}
    if isinstance(value, list):
        return {key for item in value for key in public_keys(item)}
    return set()


def assert_blinded(value: Any, run_ids: set[str]) -> None:
    keys = public_keys(value)
    leaked_keys = sorted(keys & FORBIDDEN_PUBLIC_KEYS)
    serialized = json.dumps(value, ensure_ascii=False, sort_keys=True)
    leaked_values = [item for item in run_ids if item in serialized]
    if leaked_keys or leaked_values or str(HARNESS_ROOT / "runs") in serialized:
        raise SystemExit(f"blind bundle identity leak: keys={leaked_keys}, run_ids={leaked_values[:3]}")


def lock_tree(root: pathlib.Path) -> None:
    for path in sorted([root, *root.rglob("*")], reverse=True):
        if not path.is_symlink():
            path.chmod(path.stat().st_mode & ~0o222)


def identity_rows(paths: list[pathlib.Path], secret: bytes) -> list[dict]:
    rows = []
    for run_dir in paths:
        manifest = load_json(run_dir / "run.manifest.json")
        schedule = manifest["schedule"]
        session_id = "session-" + hashlib.sha256(secret + run_dir.name.encode("utf-8")).hexdigest()[:24]
        rows.append({"session_id": session_id, "run_dir": str(run_dir), "schedule": schedule})
    if len({row["session_id"] for row in rows}) != len(rows):
        raise SystemExit("opaque session id collision")
    return rows


def correctness(generation_path: pathlib.Path) -> int:
    verify_generation(generation_path)
    generation = load_json(generation_path)
    paths = valid_runs(generation)
    scoring_root = HARNESS_ROOT / "scoring" / generation["generation_id"]
    private_root = scoring_root / "coordinator-only"
    phase_root = scoring_root / "phase1-correctness"
    if scoring_root.exists():
        raise SystemExit("scoring generation already exists; bundles are immutable")
    private_root.mkdir(parents=True)
    secret = secrets.token_bytes(32)
    rows = identity_rows(paths, secret)
    rubric_path = HARNESS_ROOT / "config/scoring-rubric.json"
    rubric = load_json(rubric_path)
    rubric_sha = sha256(rubric_path)
    assignments = []
    bundle_paths = {}
    run_ids = {path.name for path in paths}
    for scorer_slot, scorer in enumerate(SCORERS, 1):
        order = list(rows)
        random.Random(int.from_bytes(hashlib.sha256(secret + scorer.encode()).digest()[:8], "big")).shuffle(order)
        items = []
        for row in order:
            run_dir = pathlib.Path(row["run_dir"])
            task_id = row["schedule"]["task_id"]
            review_id = "review-" + hashlib.sha256(
                secret + row["session_id"].encode("utf-8") + scorer_slot.to_bytes(1, "big")
            ).hexdigest()[:24]
            question_path = BENCHMARK_ROOT / "questions/development" / f"{task_id}.json"
            answer_path = BENCHMARK_ROOT / "answers/development" / f"{task_id}.json"
            answer = load_json(answer_path)
            answer_sha = sha256(answer_path)
            scoring_items = derive_scoring_items(answer, answer_sha, rubric)
            public_item = {
                "review_id": review_id,
                "question": load_json(question_path),
                "answer_contract": answer,
                "answer_contract_sha256": answer_sha,
                "scoring_items": scoring_items,
                "normalized_final_answer": final_answer(run_dir),
                "rubric_sha256": rubric_sha,
            }
            assert_blinded(public_item, run_ids)
            items.append(public_item)
            assignments.append({
                "review_id": review_id,
                "session_id": row["session_id"],
                "run_id": run_dir.name,
                "scorer_id": scorer,
                "scorer_slot": scorer_slot,
                "run_dir": row["run_dir"],
                "task_id": task_id,
                "trial_id": row["schedule"]["trial_id"],
                "arm": row["schedule"]["criterion"],
                "pair_id": row["schedule"]["pair_id"],
                "pair_order_index": row["schedule"]["pair_order_index"],
                "answer_contract_sha256": answer_sha,
                "scoring_items": scoring_items,
                "rubric_sha256": rubric_sha,
            })
        bundle = {
            "schema_version": 1,
            "phase": "correctness-first",
            "scorer_id": scorer,
            "arm_trial_pair_order_hidden": True,
            "tool_events_included": False,
            "instructions": "Score only correctness. Do not access run artifacts, process bundles, mappings, or other scorer outputs.",
            "rubric": rubric,
            "items": items,
        }
        assert_blinded(bundle, run_ids)
        bundle_path = phase_root / scorer / "bundle.json"
        write_json(bundle_path, bundle)
        bundle_paths[scorer] = bundle_path
    if len(assignments) != 252 or len({row["review_id"] for row in assignments}) != 252:
        raise SystemExit("expected 252 globally unique review assignments")
    per_session: dict[str, list[dict]] = {}
    for row in assignments:
        per_session.setdefault(row["session_id"], []).append(row)
    if len(per_session) != 84 or any(
        len(rows_for_session) != 3
        or {row["scorer_slot"] for row in rows_for_session} != {1, 2, 3}
        or len({row["scorer_id"] for row in rows_for_session}) != 3
        for rows_for_session in per_session.values()
    ):
        raise SystemExit("each output must have exactly three distinct scorer assignments")
    for row in assignments:
        row["bundle_path"] = str(bundle_paths[row["scorer_id"]])
        row["bundle_sha256"] = sha256(bundle_paths[row["scorer_id"]])
    assignment_value = {
        "schema_version": 1,
        "generation_id": generation["generation_id"],
        "generation_seal_sha256": generation["generation_seal_sha256"],
        "expected_session_count": 84,
        "expected_judgment_count": 252,
        "rubric_sha256": rubric_sha,
        "runs_root": str((HARNESS_ROOT / "runs" / generation["generation_id"]).resolve()),
        "ledger_path": str((HARNESS_ROOT / "runs" / generation["generation_id"] / "ledger.json").resolve()),
        "ledger_sha256": sha256(HARNESS_ROOT / "runs" / generation["generation_id"] / "ledger.json"),
        "assignments": assignments,
    }
    assignment_value["assignment_seal_sha256"] = canonical_sha256(assignment_value)
    write_json(private_root / "assignments.json", assignment_value)
    load_assignment(private_root / "assignments.json", HARNESS_ROOT / "runs" / generation["generation_id"])
    (private_root / "secret.bin").write_bytes(secret)
    (private_root / "secret.bin").chmod(0o400)
    (private_root / "assignments.json").chmod(0o400)
    for path in bundle_paths.values():
        path.chmod(0o444)
    lock_tree(private_root)
    lock_tree(phase_root)
    return 0


def verify_phase1_seal(path: pathlib.Path, assignment: dict) -> dict:
    seal = load_json(path)
    core = dict(seal)
    recorded = core.pop("seal_sha256", None)
    if recorded != canonical_sha256(core):
        raise SystemExit("phase-1 score seal self-hash mismatch")
    if (
        seal.get("phase") != "correctness"
        or seal.get("generation_id") != assignment["generation_id"]
        or seal.get("generation_seal_sha256") != assignment["generation_seal_sha256"]
        or seal.get("assignment_seal_sha256") != assignment["assignment_seal_sha256"]
    ):
        raise SystemExit("phase-1 score seal/assignment mismatch")
    judgments = seal.get("judgments", {})
    expected = {row["review_id"] for row in assignment["assignments"]}
    rows = {row["review_id"]: row for row in assignment["assignments"]}
    if set(judgments) != expected or len(judgments) != 252:
        raise SystemExit("phase-1 seal must contain every assigned review exactly once")
    hashes = []
    for review_id, item in judgments.items():
        judgment_path = pathlib.Path(item["path"])
        if item.get("scorer_id") != rows[review_id]["scorer_id"] or item.get("session_id") != rows[review_id]["session_id"]:
            raise SystemExit(f"phase-1 score identity mismatch: {review_id}")
        if not judgment_path.is_file() or sha256(judgment_path) != item["sha256"]:
            raise SystemExit(f"phase-1 score seal mismatch: {review_id}")
        if judgment_path.stat().st_mode & 0o222:
            raise SystemExit(f"phase-1 judgment remains writable: {review_id}")
        hashes.append(item["sha256"])
    if len(set(hashes)) != len(hashes):
        raise SystemExit("one phase-1 judgment file/hash may not count twice")
    return seal


def process_bundle(generation_path: pathlib.Path, phase1_seals: pathlib.Path) -> int:
    verify_generation(generation_path)
    generation = load_json(generation_path)
    scoring_root = HARNESS_ROOT / "scoring" / generation["generation_id"]
    assignment_path = scoring_root / "coordinator-only/assignments.json"
    assignment = load_assignment(assignment_path, HARNESS_ROOT / "runs" / generation["generation_id"])
    phase1_seal = verify_phase1_seal(phase1_seals, assignment)
    by_scorer = {scorer: [] for scorer in SCORERS}
    run_ids = {pathlib.Path(row["run_dir"]).name for row in assignment["assignments"]}
    for row in assignment["assignments"]:
        run_dir = pathlib.Path(row["run_dir"])
        events = []
        wrapper = load_json(run_dir / "wrapper.json")
        accepted = int(wrapper.get("reducer_lines_accepted", 0))
        canonical_calls = canonical_navigation_call_identities(assignment, row, accepted)
        for line_number, line in enumerate((run_dir / "raw/events.jsonl").read_bytes().splitlines()[:accepted], 1):
            try:
                value = json.loads(line)
            except (UnicodeDecodeError, json.JSONDecodeError):
                value = {"parse_error": True, "bytes": len(line)}
            events.append({"raw_event_line": line_number, "event": neutralize(value, run_dir)})
        task_id = row["task_id"]
        public_item = {
            "review_id": row["review_id"],
            "question": load_json(BENCHMARK_ROOT / "questions/development" / f"{task_id}.json"),
            "answer_contract": load_json(BENCHMARK_ROOT / "answers/development" / f"{task_id}.json"),
            "scoring_items": row["scoring_items"],
            "normalized_final_answer": final_answer(run_dir),
            "neutralized_events": events,
            "canonical_completed_navigation_calls": canonical_calls,
            "phase1_judgment_sha256": phase1_seal["judgments"][row["review_id"]]["sha256"],
        }
        assert_blinded(public_item, run_ids)
        by_scorer[row["scorer_id"]].append(public_item)
    for scorer, items in by_scorer.items():
        bundle = {
            "schema_version": 1,
            "phase": "process-after-correctness-lock",
            "scorer_id": scorer,
            "arm_trial_pair_order_config_provider_hidden": True,
            "correctness_rescoring_prohibited": True,
            "instructions": (
                "Use canonical_completed_navigation_calls for every discovery-stage call_id and first_wrong "
                "raw_event_line/completed_call_index/call_id boundary; preserve its order and exact tuples."
            ),
            "items": items,
        }
        assert_blinded(bundle, run_ids)
        output = scoring_root / "phase2-process" / scorer / "bundle.json"
        write_json(output, bundle)
        output.chmod(0o444)
    lock_tree(scoring_root / "phase2-process")
    return 0


def main() -> int:
    if len(sys.argv) == 3 and sys.argv[1] == "correctness":
        return correctness(pathlib.Path(sys.argv[2]).resolve())
    if len(sys.argv) == 4 and sys.argv[1] == "process":
        return process_bundle(pathlib.Path(sys.argv[2]).resolve(), pathlib.Path(sys.argv[3]).resolve())
    raise SystemExit("usage: make_scoring_bundles.py correctness GENERATION | process GENERATION PHASE1_SEAL")


if __name__ == "__main__":
    raise SystemExit(main())
