#!/usr/bin/env python3
"""Resume one sealed generation with three distinct pairs and bounded safe replacements."""
from __future__ import annotations

import concurrent.futures
import json
import os
import pathlib
import shutil
import subprocess
import sys
import threading
import time
from collections import OrderedDict
from typing import Any

import abort_claimed
import auth_runtime
import finalize_metrics
import protocol
import seal_artifacts
from common import HARNESS_ROOT, QUALITY_ROOT, load_json, write_json
from generation import require_execution_generation, verify as verify_generation


REPLACEMENT_BACKOFF_SECONDS = {2: 5, 3: 15}


def apply_replacement_backoff(
    latest_attempt_number: int,
    category: str | None,
    *,
    sleep_fn=time.sleep,
    notify_fn=print,
) -> int:
    next_attempt_number = latest_attempt_number + 1
    wait_seconds = REPLACEMENT_BACKOFF_SECONDS.get(next_attempt_number)
    if wait_seconds is None:
        raise RuntimeError("replacement attempt is outside the sealed backoff policy")
    notify_fn(json.dumps({
        "event": "replacement-backoff",
        "failed_attempt_number": latest_attempt_number,
        "next_attempt_number": next_attempt_number,
        "replacement_category": category,
        "retry_wait_seconds": wait_seconds,
    }, sort_keys=True))
    sleep_fn(wait_seconds)
    return wait_seconds


def read_ledger(ledger_path: pathlib.Path, generation: dict[str, Any]) -> dict[str, Any]:
    if not ledger_path.exists():
        return {
            "generation_id": generation["generation_id"],
            "generation_seal_sha256": generation["generation_seal_sha256"],
            "state": "active",
            "slots": {},
            "attempts": {},
        }
    ledger = load_json(ledger_path)
    if ledger.get("generation_id") != generation["generation_id"] or ledger.get("generation_seal_sha256") != generation["generation_seal_sha256"]:
        raise RuntimeError("ledger/generation mismatch")
    return ledger


def valid_slot_counts(ledger: dict[str, Any]) -> tuple[int, int]:
    slots = ledger.get("slots", {}).values()
    published = sum(slot.get("measurement_status") == "valid" for slot in slots)
    sealed = sum(
        slot.get("measurement_status") == "valid" and slot.get("metrics_status") == "sealed"
        for slot in ledger.get("slots", {}).values()
    )
    return published, sealed


def slot_decision(generation: dict[str, Any], ledger: dict[str, Any], entry: dict[str, Any]) -> tuple[str, str]:
    if ledger.get("state") == "invalid-restart-required":
        return "stop", "generation invalidated; a new sealed generation is required"
    slot_key = f"{entry['task_id']}:{entry['trial_id']}:{entry['criterion']}"
    slot = ledger.get("slots", {}).get(slot_key)
    if slot is None:
        return "initial", "no earlier attempt"
    status = slot.get("measurement_status")
    if status == "valid" and slot.get("metrics_status") == "sealed":
        return "skip", "sealed valid measurement already exists"
    latest_run_id = slot.get("latest_run_id")
    latest_attempt = ledger.get("attempts", {}).get(latest_run_id, {})
    if latest_attempt.get("state") in {"claimed", "running", "terminal-unsealed"}:
        return "stop", f"unfinished attempt requires recovery: {latest_run_id}"
    if latest_attempt.get("state") == "terminal" and slot.get("metrics_status") == "pending":
        liveness = protocol.owner_liveness(latest_attempt)
        if liveness == "dead":
            return "resume_metrics", f"published attempt has resumable automatic metrics: {latest_run_id}"
        return "stop", f"published attempt metrics owner is {liveness}; recovery refused: {latest_run_id}"
    if status == "valid":
        return "stop", "published valid measurement is still missing sealed automatic metrics"
    if status == "infrastructure_invalid" and slot.get("replacement_allowed") is True:
        latest_number = int(slot.get("latest_attempt_number", 0))
        maximum = int(generation["execution_policy"]["max_attempts_per_slot"])
        if latest_number < maximum:
            return "replacement", f"replaceable transient infrastructure failure at attempt {latest_number}"
        return "stop", f"replacement budget exhausted at {latest_number} attempts"
    return "stop", f"slot is not resumable: status={status!r}"


def resume_pending_metrics(
    generation_path: pathlib.Path,
    ledger_path: pathlib.Path,
    generation: dict[str, Any],
    entry: dict[str, Any],
    *,
    extractor_path: pathlib.Path | None = None,
    schema_path: pathlib.Path | None = None,
) -> int:
    ledger = read_ledger(ledger_path, generation)
    slot_key = f"{entry['task_id']}:{entry['trial_id']}:{entry['criterion']}"
    slot = ledger.get("slots", {}).get(slot_key, {})
    run_id = slot.get("latest_run_id")
    attempt = ledger.get("attempts", {}).get(run_id, {})
    if (
        not isinstance(run_id, str)
        or attempt.get("state") != "terminal"
        or slot.get("metrics_status") != "pending"
        or attempt.get("run_dir") != str((ledger_path.parent / run_id).resolve())
    ):
        raise RuntimeError("pending automatic metrics do not identify one published run")
    liveness = protocol.owner_liveness(attempt)
    if liveness != "dead":
        raise RuntimeError(f"pending automatic metrics owner is {liveness}; recovery refused")
    return finalize_metrics.finalize(
        ledger_path,
        generation_path,
        pathlib.Path(attempt["run_dir"]),
        extractor_path or QUALITY_ROOT / "analysis-tools/extract_run_metrics.py",
        schema_path or HARNESS_ROOT / "schemas/automatic-run-metrics.schema.json",
    )


def interrupted_run_directory(ledger_path: pathlib.Path, attempt: dict[str, Any]) -> pathlib.Path | None:
    run_id = attempt.get("run_id")
    if not isinstance(run_id, str):
        return None
    expected = (ledger_path.parent / run_id).resolve()
    if expected.is_dir():
        return expected
    recorded = attempt.get("run_dir")
    if isinstance(recorded, str) and pathlib.Path(recorded).resolve().is_dir():
        return pathlib.Path(recorded).resolve()
    receipt = attempt.get("claim_receipt_path")
    if isinstance(receipt, str) and pathlib.Path(receipt).resolve().is_file():
        return pathlib.Path(receipt).resolve().parent
    return None


def stop_owned_process_group(attempt: dict[str, Any], run_dir: pathlib.Path | None) -> list[str]:
    process_group = attempt.get("guardian_process_group")
    guardian_token = attempt.get("guardian_token")
    child_path = run_dir / "child-process.json" if run_dir is not None else None
    receipt_path = run_dir / "claim-receipt.json" if run_dir is not None else None
    if (not isinstance(process_group, int) or not isinstance(guardian_token, str)) and child_path is not None and child_path.is_file():
        child = load_json(child_path)
        process_group = child.get("guardian_process_group")
        guardian_token = child.get("guardian_token")
    if (not isinstance(process_group, int) or not isinstance(guardian_token, str)) and receipt_path is not None and receipt_path.is_file():
        receipt = load_json(receipt_path)
        process_group = receipt.get("guardian_process_group")
        guardian_token = receipt.get("guardian_token")
    if not isinstance(process_group, int) or process_group <= 1:
        return []
    if not protocol.process_group_members(process_group):
        return []
    if not isinstance(guardian_token, str):
        raise RuntimeError("interrupted guardian token is missing")
    signals = abort_claimed.stop_group(process_group, guardian_token)
    if protocol.verified_group_targets(process_group, guardian_token):
        raise RuntimeError("interrupted child process group could not be stopped")
    return signals


def cleanup_interrupted_auth(attempt: dict[str, Any]) -> None:
    work_id = attempt.get("work_id")
    if not isinstance(work_id, str) or not work_id:
        return
    auth_runtime.remove_matching_runtime(HARNESS_ROOT / "runtime-auth", f"{work_id}-")


def recover_orphan_claim_receipts(
    generation_path: pathlib.Path,
    ledger_path: pathlib.Path,
    generation: dict[str, Any],
) -> list[dict[str, Any]]:
    work_root = HARNESS_ROOT / "work"
    if not work_root.is_dir():
        return []
    ledger = read_ledger(ledger_path, generation)
    actions: list[dict[str, Any]] = []
    for receipt_path in sorted(work_root.glob("prep-*/claim-receipt.json")):
        receipt = load_json(receipt_path)
        if (
            receipt.get("generation_id") != generation["generation_id"]
            or receipt.get("generation_seal_sha256") != generation["generation_seal_sha256"]
            or receipt.get("ledger_path") != str(ledger_path.resolve())
        ):
            continue
        run_id = receipt.get("run_id")
        if not isinstance(run_id, str):
            raise RuntimeError(f"orphan claim receipt has no run identity: {receipt_path}")
        if run_id in ledger.get("attempts", {}):
            continue
        owner = {
            "owner_host_identity": receipt.get("owner_host_identity"),
            "owner_pid": receipt.get("owner_pid"),
            "owner_process_start_token": receipt.get("owner_process_start_token"),
            "owner_token": receipt.get("owner_token"),
        }
        liveness = protocol.owner_liveness(owner)
        if liveness != "dead":
            raise RuntimeError(f"uncommitted claim receipt {run_id} owner is {liveness}; recovery refused")
        cleanup_interrupted_auth(receipt)
        work_dir = receipt_path.parent.resolve()
        if work_dir.parent != work_root.resolve() or not work_dir.name.startswith("prep-"):
            raise RuntimeError("orphan claim work directory escaped the harness work root")
        for path in [work_dir, *work_dir.rglob("*")]:
            if not path.is_symlink():
                path.chmod(path.stat().st_mode | 0o700)
        shutil.rmtree(work_dir)
        actions.append({"run_id": run_id, "recovery": "removed-dead-uncommitted-claim"})
    return actions


def recover_interrupted_attempts(
    generation_path: pathlib.Path,
    ledger_path: pathlib.Path,
    generation: dict[str, Any],
    *,
    extractor_path: pathlib.Path | None = None,
    schema_path: pathlib.Path | None = None,
) -> list[dict[str, Any]]:
    if not ledger_path.exists():
        return []
    actions: list[dict[str, Any]] = []
    snapshot = read_ledger(ledger_path, generation)
    for run_id, stale_attempt in list(snapshot.get("attempts", {}).items()):
        state = stale_attempt.get("state")
        if state not in {"claimed", "running", "terminal-unsealed"}:
            continue
        liveness = protocol.owner_liveness(stale_attempt)
        if liveness != "dead":
            raise RuntimeError(f"unfinished attempt {run_id} owner is {liveness}; recovery refused")
        current = read_ledger(ledger_path, generation).get("attempts", {}).get(run_id, {})
        if current.get("state") not in {"claimed", "running", "terminal-unsealed"}:
            continue
        if protocol.owner_liveness(current) != "dead":
            raise RuntimeError(f"unfinished attempt {run_id} owner changed during recovery")
        run_dir = interrupted_run_directory(ledger_path, current)
        child_exists = run_dir is not None and (run_dir / "child-process.json").is_file()
        if current["state"] == "claimed" and not child_exists:
            receipt = pathlib.Path(current.get("claim_receipt_path") or "")
            moved_receipt = ledger_path.parent / run_id / "claim-receipt.json"
            if moved_receipt.is_file():
                receipt = moved_receipt
            if not receipt.is_file():
                raise RuntimeError(f"dead claimed attempt {run_id} has no verifiable receipt")
            protocol.cancel_claim([
                str(ledger_path), str(generation_path), str(receipt),
                "same-host claim owner died before the paid process started",
            ])
            cleanup_interrupted_auth(current)
            if run_dir is not None and run_dir.is_dir():
                run_dir.chmod(run_dir.stat().st_mode | 0o700)
                shutil.rmtree(run_dir)
            actions.append({"run_id": run_id, "recovery": "canceled-dead-unstarted-claim"})
            continue
        stop_owned_process_group(current, run_dir)
        cleanup_interrupted_auth(current)
        if run_dir is None:
            raise RuntimeError(f"interrupted attempt {run_id} has no recoverable run directory")
        if current["state"] in {"claimed", "running"}:
            abort_claimed.abort(run_dir, "queue-recovery-dead-owner", 79)
            protocol.terminal([
                str(ledger_path), str(generation_path), run_id,
                str(run_dir / "attempt-classification.json"),
            ])
        seal_artifacts.seal(run_dir)
        protocol.publish([str(ledger_path), str(generation_path), run_id, str(run_dir)])
        metrics_status = finalize_metrics.finalize(
            ledger_path, generation_path, run_dir,
            extractor_path or QUALITY_ROOT / "analysis-tools/extract_run_metrics.py",
            schema_path or HARNESS_ROOT / "schemas/automatic-run-metrics.schema.json",
        )
        actions.append({
            "run_id": run_id, "recovery": "finalized-dead-owner-attempt",
            "metrics_exit_code": metrics_status,
        })
    return actions


def grouped_pairs(generation: dict[str, Any]) -> OrderedDict[str, list[dict[str, Any]]]:
    pairs: OrderedDict[str, list[dict[str, Any]]] = OrderedDict()
    for entry in generation["schedule"]:
        pairs.setdefault(entry["pair_id"], []).append(entry)
    for pair_id, entries in pairs.items():
        entries.sort(key=lambda row: row["pair_order_index"])
        if len(entries) != 2 or [row["pair_order_index"] for row in entries] != [0, 1]:
            raise SystemExit(f"invalid pair: {pair_id}")
    return pairs


def run_preflight(generation_path: pathlib.Path) -> None:
    subprocess.run(
        [sys.executable, str(HARNESS_ROOT / "scripts/preflight.py"), str(generation_path)],
        env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
        check=True,
    )


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: run_queue.py SEALED_GENERATION")
    generation_path = pathlib.Path(sys.argv[1]).resolve()
    if os.environ.get("BASELINE_3X_EXTERNAL_APPROVED") != "1" or os.environ.get("BASELINE_3X_AUTH_READY") != "1":
        raise SystemExit("external-model/auth gate is closed")
    verify_generation(generation_path)
    generation = require_execution_generation(load_json(generation_path))
    policy = generation["execution_policy"]
    if (
        policy["max_concurrency"] != 3
        or policy["max_attempts_per_slot"] != 3
        or policy["max_replacements_per_slot"] != 2
        or policy.get("replacement_backoff_seconds") != {"attempt_2": 5, "attempt_3": 15}
        or policy.get("provider_retry_after_used") is not False
    ):
        raise SystemExit("sealed execution policy is not the approved three-pair/three-attempt policy")
    ledger_path = HARNESS_ROOT / "runs" / generation["generation_id"] / "ledger.json"
    recovery_actions = recover_orphan_claim_receipts(generation_path, ledger_path, generation)
    recovery_actions.extend(recover_interrupted_attempts(generation_path, ledger_path, generation))
    run_preflight(generation_path)
    pairs = grouped_pairs(generation)
    report_lock = threading.Lock()
    outcomes: list[dict[str, Any]] = []

    def run_pair(pair_id: str, entries: list[dict[str, Any]]) -> dict[str, Any]:
        actions: list[dict[str, Any]] = []
        for entry in entries:
            while True:
                ledger = read_ledger(ledger_path, generation)
                mode, reason = slot_decision(generation, ledger, entry)
                action = {
                    "task_id": entry["task_id"], "trial_id": entry["trial_id"],
                    "arm": entry["criterion"], "decision": mode, "reason": reason,
                }
                if mode == "skip":
                    actions.append(action)
                    break
                if mode == "stop":
                    actions.append(action)
                    return {"pair_id": pair_id, "actions": actions, "completed": False, "stop_reason": reason}
                if mode == "resume_metrics":
                    metrics_status = resume_pending_metrics(generation_path, ledger_path, generation, entry)
                    action["metrics_resume_exit_code"] = metrics_status
                    actions.append(action)
                    if metrics_status != 0:
                        return {
                            "pair_id": pair_id, "actions": actions, "completed": False,
                            "stop_reason": f"automatic metrics resume failed with exit {metrics_status}",
                        }
                    continue
                if mode == "replacement":
                    slot_key = f"{entry['task_id']}:{entry['trial_id']}:{entry['criterion']}"
                    slot = ledger["slots"][slot_key]
                    action["replacement_category"] = slot.get("latest_classification", {}).get("replacement_category")
                    action["retry_wait_seconds"] = apply_replacement_backoff(
                        int(slot["latest_attempt_number"]),
                        action["replacement_category"],
                        notify_fn=lambda message: print(message, flush=True),
                    )
                    refreshed = read_ledger(ledger_path, generation)
                    refreshed_mode, refreshed_reason = slot_decision(generation, refreshed, entry)
                    if refreshed_mode != "replacement":
                        action["decision_after_wait"] = refreshed_mode
                        action["reason_after_wait"] = refreshed_reason
                        actions.append(action)
                        if refreshed_mode == "skip":
                            break
                        return {"pair_id": pair_id, "actions": actions, "completed": False, "stop_reason": refreshed_reason}
                command = [
                    str(HARNESS_ROOT / "scripts/run-session.sh"), str(generation_path), entry["task_id"],
                    entry["trial_id"], entry["criterion"], mode,
                ]
                process = subprocess.run(
                    command, env=os.environ.copy(), text=True,
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
                )
                action.update({
                    "exit_code": process.returncode,
                    "stdout": process.stdout,
                    "stderr": process.stderr,
                })
                actions.append(action)
                after = read_ledger(ledger_path, generation)
                after_mode, after_reason = slot_decision(generation, after, entry)
                if process.returncode == 0 and after_mode == "skip":
                    break
                if process.returncode == 75 and after_mode == "replacement":
                    continue
                return {
                    "pair_id": pair_id, "actions": actions, "completed": False,
                    "stop_reason": f"runner exit {process.returncode}; ledger decision {after_mode}: {after_reason}",
                }
        return {"pair_id": pair_id, "actions": actions, "completed": True}

    pending = iter(pairs.items())
    active: dict[concurrent.futures.Future, str] = {}
    stop_submitting = False
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as executor:
        for _ in range(3):
            try:
                pair_id, entries = next(pending)
            except StopIteration:
                break
            active[executor.submit(run_pair, pair_id, entries)] = pair_id
        while active:
            done, _ = concurrent.futures.wait(active, return_when=concurrent.futures.FIRST_COMPLETED)
            for future in done:
                active.pop(future)
                result = future.result()
                with report_lock:
                    outcomes.append(result)
                if not result["completed"]:
                    stop_submitting = True
                if not stop_submitting:
                    try:
                        pair_id, entries = next(pending)
                    except StopIteration:
                        continue
                    active[executor.submit(run_pair, pair_id, entries)] = pair_id
    final_ledger = read_ledger(ledger_path, generation)
    published_valid_slots, sealed_valid_slots = valid_slot_counts(final_ledger)
    attempt_count = len(final_ledger.get("attempts", {}))
    report = {
        "schema_version": 1,
        "generation_id": generation["generation_id"],
        "max_concurrency": 3,
        "maximum_attempts_per_slot": 3,
        "startup_recovery_actions": recovery_actions,
        "pair_outcomes": outcomes,
        "valid_slot_count": sealed_valid_slots,
        "published_valid_slot_count": published_valid_slots,
        "sealed_valid_slot_count": sealed_valid_slots,
        "preserved_attempt_count": attempt_count,
        "ledger_state": final_ledger.get("state"),
        "all_completed": sealed_valid_slots == generation["session_count"] and final_ledger.get("state") == "completed",
    }
    output = HARNESS_ROOT / "reports/queue-latest.json"
    write_json(output, report)
    print(json.dumps({
        "report": str(output), "all_completed": report["all_completed"],
        "published_valid_slots": published_valid_slots, "sealed_valid_slots": sealed_valid_slots,
    }, sort_keys=True))
    return 0 if report["all_completed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
