#!/usr/bin/env python3
"""Generation-bound ledger enforcing pair order, three-pair concurrency, and safe replacements."""
from __future__ import annotations

import json
import fcntl
import hashlib
import os
import pathlib
import re
import signal
import socket
import subprocess
import sys
import time
import uuid
from typing import Any

from common import load_json, sha256, write_json
from generation import require_execution_generation, verify as verify_generation
from seal_artifacts import artifact_evidence


TOKEN_PATTERN = re.compile(r"^[0-9a-f]{64}$")
IDENTITY_READY_TIMEOUT_SECONDS = 2.0
IDENTITY_READY_POLL_SECONDS = 0.02


def host_identity() -> str:
    value = f"{socket.gethostname()}:{uuid.getnode()}".encode("utf-8")
    return hashlib.sha256(value).hexdigest()


def require_identity_token(value: object, kind: str) -> str:
    if not isinstance(value, str) or TOKEN_PATTERN.fullmatch(value) is None:
        raise RuntimeError(f"invalid {kind} identity token")
    return value


def token_marker(kind: str, token: str) -> str:
    require_identity_token(token, kind)
    return f"--baseline-{kind}-token={token}"


def process_has_identity_token(
    pid: int, kind: str, token: str, *, timeout_seconds: float | None = None,
) -> bool | None:
    """Return an exact argv or environment token match without exposing process data."""
    argv_marker = token_marker(kind, token)
    environment_marker = f"BASELINE_{kind.upper().replace('-', '_')}_TOKEN={token}"
    try:
        result = subprocess.run(
            ["ps", "eww", "-p", str(pid), "-o", "command="],
            text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired:
        return None
    if result.returncode != 0 or not result.stdout.strip():
        return None
    process_data = result.stdout.strip()
    return any(
        re.search(rf"(?:^|\s){re.escape(marker)}(?:\s|$)", process_data) is not None
        for marker in (argv_marker, environment_marker)
    )


def wait_for_process_identity(
    pid: int,
    kind: str,
    token: str,
    *,
    is_alive_fn,
    identity_fn=None,
    monotonic_fn=time.monotonic,
    sleep_fn=time.sleep,
) -> bool:
    deadline = monotonic_fn() + IDENTITY_READY_TIMEOUT_SECONDS
    while is_alive_fn():
        remaining = deadline - monotonic_fn()
        if remaining <= 0:
            return False
        matched = (
            identity_fn(pid, kind, token)
            if identity_fn is not None
            else process_has_identity_token(
                pid, kind, token, timeout_seconds=remaining,
            )
        )
        if matched is True:
            return True
        remaining = deadline - monotonic_fn()
        if remaining <= 0:
            return False
        sleep_fn(min(IDENTITY_READY_POLL_SECONDS, remaining))
    return False


def process_exists(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def process_group_members(pgid: int) -> list[dict[str, object]]:
    if not isinstance(pgid, int) or pgid <= 1:
        return []
    result = subprocess.run(
        ["ps", "-axo", "pid=,pgid=,stat="],
        text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False,
    )
    if result.returncode != 0:
        raise RuntimeError("process-group membership is unavailable")
    members: list[dict[str, object]] = []
    for line in result.stdout.splitlines():
        fields = line.split(None, 2)
        if len(fields) != 3:
            continue
        try:
            pid, member_pgid = map(int, fields[:2])
        except ValueError:
            continue
        if member_pgid == pgid:
            members.append({"pid": pid, "state": fields[2]})
    return members


def guardian_group_token_match(pgid: int, guardian_token: str) -> bool:
    require_identity_token(guardian_token, "guardian")
    for member in process_group_members(pgid):
        if str(member["state"]).startswith("Z"):
            continue
        matched = process_has_identity_token(int(member["pid"]), "guardian", guardian_token)
        if matched is True:
            return True
    return False


def is_live_group_member(pgid: int, pid: int) -> bool:
    return any(
        int(member["pid"]) == pid and not str(member["state"]).startswith("Z")
        for member in process_group_members(pgid)
    )


def verified_group_targets(pgid: int, guardian_token: str, exclude_pid: int | None = None) -> list[int]:
    targets: list[int] = []
    for member in process_group_members(pgid):
        pid = int(member["pid"])
        if str(member["state"]).startswith("Z") or pid == exclude_pid:
            continue
        matched = process_has_identity_token(pid, "guardian", guardian_token)
        if matched is not True and not is_live_group_member(pgid, pid):
            continue
        if matched is not True:
            raise RuntimeError("process group member exists without the exact guardian token; signal refused")
        targets.append(pid)
    return targets


def stop_verified_process_group(
    pgid: int,
    guardian_token: str,
    *,
    exclude_pid: int | None = None,
    wait_seconds: float = 1.0,
) -> list[str]:
    """Stop only group members that retain the exact guardian token at signal time."""
    signals: list[str] = []
    def signal_targets(signum: int) -> bool:
        targets = verified_group_targets(pgid, guardian_token, exclude_pid)
        for pid in targets:
            matched = process_has_identity_token(pid, "guardian", guardian_token)
            if matched is not True and not is_live_group_member(pgid, pid):
                continue
            if matched is not True:
                raise RuntimeError("guardian identity changed before signal; signal refused")
            if not is_live_group_member(pgid, pid):
                continue
            try:
                os.kill(pid, signum)
            except ProcessLookupError:
                pass
        return bool(targets)

    if not signal_targets(signal.SIGTERM):
        return signals
    signals.append("SIGTERM")
    deadline = time.monotonic() + wait_seconds
    while time.monotonic() < deadline:
        if not verified_group_targets(pgid, guardian_token, exclude_pid):
            return signals
        time.sleep(0.02)
    signal_targets(signal.SIGKILL)
    signals.append("SIGKILL")
    deadline = time.monotonic() + wait_seconds
    while time.monotonic() < deadline:
        if not verified_group_targets(pgid, guardian_token, exclude_pid):
            return signals
        time.sleep(0.02)
    if verified_group_targets(pgid, guardian_token, exclude_pid):
        raise RuntimeError("verified guardian process group could not be stopped")
    return signals


def process_snapshot(pid: int) -> dict[str, str] | None:
    if pid <= 1:
        return None
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return None
    except PermissionError:
        return {"start_token": "", "state": "permission-denied"}
    start = subprocess.run(
        ["ps", "-o", "lstart=", "-p", str(pid)], text=True,
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False,
    )
    state = subprocess.run(
        ["ps", "-o", "stat=", "-p", str(pid)], text=True,
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False,
    )
    if start.returncode != 0 or state.returncode != 0 or not start.stdout.strip() or not state.stdout.strip():
        return {"start_token": "", "state": "unknown"}
    return {"start_token": start.stdout.strip(), "state": state.stdout.strip()}


def owner_liveness(attempt: dict[str, Any]) -> str:
    if attempt.get("owner_host_identity") != host_identity():
        return "foreign-host"
    pid = attempt.get("owner_pid")
    token = attempt.get("owner_token")
    if not isinstance(pid, int) or not isinstance(token, str) or TOKEN_PATTERN.fullmatch(token) is None:
        return "unknown"
    current = process_snapshot(pid)
    if current is None:
        return "dead"
    if current["state"].startswith("Z"):
        return "dead"
    matched = process_has_identity_token(pid, "owner", token)
    if matched is False:
        return "identity-mismatch"
    if matched is None:
        return "unknown"
    return "alive"


def lock(ledger: pathlib.Path, timeout_seconds: float = 10.0) -> int:
    """Kernel-owned bounded lock: stale lock files never imply stale ownership."""
    guard = ledger.with_suffix(".flock")
    descriptor = os.open(guard, os.O_RDWR | os.O_CREAT, 0o600)
    deadline = time.monotonic() + timeout_seconds
    while True:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return descriptor
        except BlockingIOError:
            if time.monotonic() >= deadline:
                os.close(descriptor)
                raise SystemExit("ledger lock timeout")
            time.sleep(0.02)


def unlock(descriptor: int) -> None:
    fcntl.flock(descriptor, fcntl.LOCK_UN)
    os.close(descriptor)


def load_or_create(ledger: pathlib.Path, generation: dict) -> dict:
    require_execution_generation(generation)
    if ledger.exists():
        value = load_json(ledger)
        if value.get("generation_id") != generation["generation_id"] or value.get("generation_seal_sha256") != generation["generation_seal_sha256"]:
            raise SystemExit("ledger/generation mismatch")
        return value
    return {
        "schema_version": 1,
        "generation_id": generation["generation_id"],
        "generation_seal_sha256": generation["generation_seal_sha256"],
        "state": "active",
        "max_concurrency": 3,
        "slots": {},
        "attempts": {},
    }


def scheduled(generation: dict, task: str, trial: str, arm: str) -> dict:
    matches = [row for row in generation["schedule"] if (row["task_id"], row["trial_id"], row["criterion"]) == (task, trial, arm)]
    if len(matches) != 1:
        raise SystemExit("request is outside sealed schedule")
    return matches[0]


def claim(args: list[str]) -> int:
    ledger_path, generation_path = map(pathlib.Path, args[:2])
    task, trial, arm = args[2:5]
    replacement = args[5] == "replacement"
    receipt_path = pathlib.Path(args[6]).resolve() if len(args) == 7 else None
    crash_after_commit = os.environ.get("HARNESS_SYNTHETIC_CRASH_AFTER_CLAIM_COMMIT") == "1"
    if crash_after_commit and os.environ.get("HARNESS_SYNTHETIC_TEST") != "1":
        raise SystemExit("claim crash hook is restricted to synthetic tests")
    verify_generation(generation_path)
    generation = require_execution_generation(load_json(generation_path))
    entry = scheduled(generation, task, trial, arm)
    ledger_path.parent.mkdir(parents=True, exist_ok=True)
    guard = lock(ledger_path)
    try:
        ledger = load_or_create(ledger_path, generation)
        if ledger["state"] != "active":
            raise SystemExit(f"generation is not runnable: {ledger['state']}")
        active = [attempt for attempt in ledger["attempts"].values() if attempt["state"] in {"claimed", "running"}]
        if len(active) >= 3:
            raise SystemExit("maximum concurrency 3 reached")
        if any(attempt["pair_id"] == entry["pair_id"] for attempt in active):
            raise SystemExit("same pair may not run concurrently")
        if entry["pair_order_index"] == 1:
            predecessor_arm = entry["pair_order"][0]
            predecessor_key = f"{task}:{trial}:{predecessor_arm}"
            predecessor = ledger["slots"].get(predecessor_key, {})
            if predecessor.get("measurement_status") != "valid" or predecessor.get("metrics_status") != "sealed":
                raise SystemExit("pair predecessor must have a valid metrics-sealed terminal outcome before successor starts")
        slot_key = f"{task}:{trial}:{arm}"
        old = ledger["slots"].get(slot_key)
        if old is None and replacement:
            raise SystemExit("replacement requested without an earlier attempt")
        if old is not None:
            if (
                not replacement
                or old.get("measurement_status") != "infrastructure_invalid"
                or old.get("replacement_allowed") is not True
                or old.get("metrics_status") != "sealed"
            ):
                raise SystemExit("slot is complete or not safely replaceable")
        attempt_number = 1 if old is None else int(old["latest_attempt_number"]) + 1
        max_attempts = int(generation["execution_policy"]["max_attempts_per_slot"])
        if attempt_number > max_attempts:
            raise SystemExit(f"slot replacement limit reached: {attempt_number}>{max_attempts}")
        run_id = f"{task}-{trial}-{arm}-a{attempt_number}-{uuid.uuid4().hex}"
        old_snapshot = None if old is None else json.loads(json.dumps(old))
        owner_pid = int(os.environ.get("BASELINE_CLAIM_OWNER_PID", os.getppid()))
        owner = process_snapshot(owner_pid)
        owner_token = os.environ.get("BASELINE_OWNER_TOKEN")
        if owner is None or not owner.get("start_token") or not isinstance(owner_token, str) or TOKEN_PATTERN.fullmatch(owner_token) is None:
            raise SystemExit("claim owner process identity is unavailable")
        if process_has_identity_token(owner_pid, "owner", owner_token) is not True:
            raise SystemExit("claim owner token does not match the owner process")
        receipt = None
        if receipt_path is not None:
            if receipt_path.name != "claim-receipt.json" or not receipt_path.parent.is_dir() or receipt_path.exists() or receipt_path.is_symlink():
                raise SystemExit("claim receipt must be a new regular path in the prepared run directory")
            lease_now = time.time_ns()
            receipt = {
                "schema_version": 1,
                "generation_id": generation["generation_id"],
                "generation_seal_sha256": generation["generation_seal_sha256"],
                "ledger_path": str(ledger_path.resolve()),
                "run_id": run_id,
                "slot_key": slot_key,
                "attempt_number": attempt_number,
                "owner_pid": owner_pid,
                "owner_host_identity": host_identity(),
                "owner_process_start_token": owner["start_token"],
                "owner_token": owner_token,
                "lease_created_at_ns": lease_now,
                "lease_updated_at_ns": lease_now,
                "process_group": None,
                "process_group_start_token": None,
                "work_id": os.environ.get("BASELINE_CLAIM_WORK_ID"),
                "auth_runtime_path": os.environ.get("BASELINE_CLAIM_AUTH_RUNTIME"),
                "prepared_at_ns": time.time_ns(),
            }
            write_json(receipt_path, receipt)
            if os.environ.get("HARNESS_SYNTHETIC_PAUSE_AFTER_CLAIM_RECEIPT") == "1":
                if os.environ.get("HARNESS_SYNTHETIC_TEST") != "1":
                    raise SystemExit("claim receipt pause hook is restricted to synthetic tests")
                time.sleep(60)
        attempt = {
            "run_id": run_id, "attempt_number": attempt_number, "slot_key": slot_key,
            "pair_id": entry["pair_id"], "task_id": task, "trial_id": trial, "arm": arm,
            "pair_order_index": entry["pair_order_index"], "state": "claimed", "claimed_at_ns": time.time_ns(),
            "previous_slot": old_snapshot,
            "claim_receipt_path": str(receipt_path) if receipt_path is not None else None,
            "claim_receipt_sha256": sha256(receipt_path) if receipt_path is not None else None,
            "owner_pid": receipt["owner_pid"] if receipt is not None else owner_pid,
            "owner_host_identity": receipt["owner_host_identity"] if receipt is not None else host_identity(),
            "owner_process_start_token": receipt["owner_process_start_token"] if receipt is not None else owner["start_token"],
            "owner_token": receipt["owner_token"] if receipt is not None else owner_token,
            "lease_created_at_ns": receipt["lease_created_at_ns"] if receipt is not None else time.time_ns(),
            "lease_updated_at_ns": receipt["lease_updated_at_ns"] if receipt is not None else time.time_ns(),
            "process_group": None,
            "process_group_start_token": None,
            "work_id": receipt.get("work_id") if receipt is not None else os.environ.get("BASELINE_CLAIM_WORK_ID"),
            "auth_runtime_path": receipt.get("auth_runtime_path") if receipt is not None else os.environ.get("BASELINE_CLAIM_AUTH_RUNTIME"),
        }
        ledger["attempts"][run_id] = attempt
        ledger["slots"][slot_key] = {
            "latest_run_id": run_id, "latest_attempt_number": attempt_number,
            "measurement_status": None, "replacement_allowed": False,
            "metrics_status": None,
            "all_run_ids": ([] if old is None else old["all_run_ids"]) + [run_id],
        }
        write_json(ledger_path, ledger)
    finally:
        unlock(guard)
    if crash_after_commit:
        raise SystemExit("synthetic crash after claim commit")
    print(json.dumps({"run_id": run_id, "attempt_number": attempt_number, "entry": entry}, sort_keys=True))
    return 0


def cancel_claim(args: list[str]) -> int:
    ledger_path, generation_path, receipt_path = map(pathlib.Path, args[:3])
    reason = args[3]
    verify_generation(generation_path)
    generation = require_execution_generation(load_json(generation_path))
    receipt_path = receipt_path.resolve()
    if not receipt_path.is_file() or receipt_path.is_symlink():
        raise SystemExit("claim receipt is missing or unsafe")
    receipt = load_json(receipt_path)
    if (
        receipt.get("schema_version") != 1
        or receipt.get("generation_id") != generation["generation_id"]
        or receipt.get("generation_seal_sha256") != generation["generation_seal_sha256"]
        or receipt.get("ledger_path") != str(ledger_path.resolve())
        or not isinstance(receipt.get("run_id"), str)
        or not isinstance(receipt.get("slot_key"), str)
    ):
        raise SystemExit("claim receipt identity mismatch")
    if not ledger_path.is_file():
        print(json.dumps({"canceled": False, "reason": "claim was never committed"}, sort_keys=True))
        return 0
    guard = lock(ledger_path)
    try:
        ledger = load_or_create(ledger_path, generation)
        run_id = receipt["run_id"]
        attempt = ledger["attempts"].get(run_id)
        if attempt is None:
            print(json.dumps({"canceled": False, "reason": "claim is absent"}, sort_keys=True))
            return 0
        if attempt.get("state") != "claimed":
            raise SystemExit("only an unstarted claimed reservation may be canceled")
        moved_receipt = (ledger_path.parent / run_id / "claim-receipt.json").resolve()
        if (
            attempt.get("slot_key") != receipt["slot_key"]
            or not (
                attempt.get("claim_receipt_path") == str(receipt_path)
                or receipt_path == moved_receipt
            )
            or attempt.get("claim_receipt_sha256") != sha256(receipt_path)
        ):
            raise SystemExit("claim receipt does not bind the claimed reservation")
        slot_key = attempt["slot_key"]
        slot = ledger["slots"].get(slot_key)
        if not isinstance(slot, dict) or slot.get("latest_run_id") != run_id:
            raise SystemExit("claimed reservation is no longer the latest slot state")
        previous_slot = attempt.pop("previous_slot", None)
        attempt.update({
            "state": "canceled-before-start",
            "canceled_at_ns": time.time_ns(),
            "cancel_reason": reason,
        })
        if previous_slot is None:
            del ledger["slots"][slot_key]
        else:
            ledger["slots"][slot_key] = previous_slot
        write_json(ledger_path, ledger)
    finally:
        unlock(guard)
    print(json.dumps({"canceled": True, "run_id": run_id, "slot_key": slot_key}, sort_keys=True))
    return 0


def start(args: list[str]) -> int:
    ledger_path, generation_path = map(pathlib.Path, args[:2]); run_id = args[2]
    verify_generation(generation_path); generation = require_execution_generation(load_json(generation_path)); guard = lock(ledger_path)
    try:
        ledger = load_or_create(ledger_path, generation); attempt = ledger["attempts"].get(run_id)
        if not attempt or attempt["state"] != "claimed": raise SystemExit("attempt is not claim-owned")
        attempt.update({"state": "running", "started_at_ns": time.time_ns()}); write_json(ledger_path, ledger)
    finally: unlock(guard)
    return 0


def bind_guardian(args: list[str]) -> int:
    ledger_path, generation_path = map(pathlib.Path, args[:2]); run_id = args[2]
    run_dir = pathlib.Path(args[3]).resolve(); receipt_path = pathlib.Path(args[4]).resolve()
    guardian_pid = int(args[5]); guardian_pgid = int(args[6]); guardian_token = require_identity_token(args[7], "guardian")
    verify_generation(generation_path); generation = require_execution_generation(load_json(generation_path))
    if (
        run_dir != (ledger_path.parent / run_id).resolve()
        or receipt_path != run_dir / "claim-receipt.json"
        or not receipt_path.is_file()
        or receipt_path.is_symlink()
        or guardian_pid != guardian_pgid
    ):
        raise SystemExit("guardian binding paths or dedicated process group are invalid")
    try:
        is_dedicated = os.getsid(guardian_pid) == guardian_pid and os.getpgid(guardian_pid) == guardian_pgid
    except ProcessLookupError:
        is_dedicated = False
    if not is_dedicated or not wait_for_process_identity(
        guardian_pid, "guardian", guardian_token,
        is_alive_fn=lambda: process_exists(guardian_pid),
    ):
        raise SystemExit("guardian process identity is unavailable")
    guard = lock(ledger_path)
    try:
        ledger = load_or_create(ledger_path, generation); attempt = ledger["attempts"].get(run_id)
        if not attempt or attempt.get("state") != "claimed":
            raise SystemExit("attempt is not awaiting guardian binding")
        receipt = load_json(receipt_path)
        if (
            receipt.get("run_id") != run_id
            or receipt.get("generation_id") != generation["generation_id"]
            or receipt.get("generation_seal_sha256") != generation["generation_seal_sha256"]
            or receipt.get("ledger_path") != str(ledger_path.resolve())
            or receipt.get("owner_pid") != attempt.get("owner_pid")
            or receipt.get("owner_host_identity") != attempt.get("owner_host_identity")
            or receipt.get("owner_token") != attempt.get("owner_token")
            or attempt.get("claim_receipt_sha256") != sha256(receipt_path)
        ):
            raise SystemExit("claim receipt owner does not match the attempt")
        now = time.time_ns()
        guardian = {
            "guardian_pid": guardian_pid,
            "guardian_process_group": guardian_pgid,
            "guardian_token": guardian_token,
            "guardian_bound_at_ns": now,
            "run_dir": str(run_dir),
        }
        # Ledger-first write-ahead binding guarantees that no paid child can be
        # spawned without a recoverable token, even if this process is SIGKILLed
        # between the two atomic file replacements.
        attempt.update({"state": "running", "guardian_binding_state": "ledger-prebound", **guardian})
        write_json(ledger_path, ledger)
        receipt.update({"guardian_binding_state": "bound", **guardian})
        write_json(receipt_path, receipt)
        attempt.update({
            "guardian_binding_state": "bound",
            "claim_receipt_path": str(receipt_path),
            "claim_receipt_sha256": sha256(receipt_path),
        })
        write_json(ledger_path, ledger)
    finally:
        unlock(guard)
    return 0


def bind_process(args: list[str]) -> int:
    raise SystemExit("legacy post-spawn process binding is disabled; guardian prebinding is required")


def terminal(args: list[str]) -> int:
    ledger_path, generation_path = map(pathlib.Path, args[:2]); run_id = args[2]; classification = load_json(pathlib.Path(args[3]))
    crash_after_commit = os.environ.get("HARNESS_SYNTHETIC_CRASH_AFTER_TERMINAL_COMMIT") == "1"
    if crash_after_commit and os.environ.get("HARNESS_SYNTHETIC_TEST") != "1":
        raise SystemExit("terminal crash hook is restricted to synthetic tests")
    verify_generation(generation_path)
    generation = load_json(generation_path); guard = lock(ledger_path)
    try:
        ledger = load_or_create(ledger_path, generation); attempt = ledger["attempts"].get(run_id)
        if not attempt:
            raise SystemExit("attempt cannot terminalize")
        if classification.get("measurement_status") not in {"valid", "infrastructure_invalid"}:
            raise SystemExit("invalid measurement status")
        if not isinstance(classification.get("replacement_allowed"), bool) or not isinstance(classification.get("generation_invalid"), bool):
            raise SystemExit("classification flags must be booleans")
        if attempt["state"] in {"terminal-unsealed", "terminal"}:
            if attempt.get("classification") != classification:
                raise SystemExit("terminal classification differs from the committed record")
        elif attempt["state"] in {"claimed", "running"}:
            attempt.update({"state": "terminal-unsealed", "ended_at_ns": time.time_ns(), "classification": classification})
            if classification.get("generation_invalid"):
                ledger["state"] = "invalid-restart-required"; ledger["invalidated_by_run_id"] = run_id
            write_json(ledger_path, ledger)
        else:
            raise SystemExit("attempt cannot terminalize")
    finally: unlock(guard)
    if crash_after_commit:
        raise SystemExit("synthetic crash after terminal commit")
    return 0


def verify_artifact_seal(run_dir: pathlib.Path, required_artifacts: tuple[str, ...] = ("attempt-classification.json",)) -> dict:
    run_dir = run_dir.resolve()
    manifest_path = run_dir / "artifact-manifest.json"
    if not run_dir.is_dir() or not manifest_path.is_file():
        raise SystemExit("sealed run directory or artifact manifest is missing")
    manifest = load_json(manifest_path)
    artifacts = manifest.get("artifacts")
    if manifest.get("schema_version") != 1 or not isinstance(artifacts, dict):
        raise SystemExit("invalid artifact manifest")
    missing_required = [relative for relative in required_artifacts if relative not in artifacts]
    if missing_required:
        raise SystemExit(f"required files are absent from artifact seal: {missing_required}")
    if artifacts != artifact_evidence(run_dir):
        raise SystemExit("artifact file set or content changed after sealing")
    writable = [
        str(path.relative_to(run_dir)) if path != run_dir else "."
        for path in [run_dir, *run_dir.rglob("*")]
        if not path.is_symlink() and path.stat().st_mode & 0o222
    ]
    if writable:
        raise SystemExit(f"sealed run remains writable: {writable[:3]}")
    return manifest


def publish(args: list[str]) -> int:
    ledger_path, generation_path = map(pathlib.Path, args[:2]); run_id = args[2]; run_dir = pathlib.Path(args[3])
    if run_dir.resolve().name != run_id:
        raise SystemExit("run directory does not match claimed run id")
    verify_generation(generation_path)
    crash_after_commit = os.environ.get("HARNESS_SYNTHETIC_CRASH_AFTER_PUBLISH_COMMIT") == "1"
    if crash_after_commit and os.environ.get("HARNESS_SYNTHETIC_TEST") != "1":
        raise SystemExit("publish crash hook is restricted to synthetic tests")
    generation = load_json(generation_path)
    manifest = verify_artifact_seal(run_dir)
    classification = load_json(run_dir / "attempt-classification.json")
    guard = lock(ledger_path)
    try:
        ledger = load_or_create(ledger_path, generation); attempt = ledger["attempts"].get(run_id)
        if not attempt:
            raise SystemExit("attempt is not awaiting sealed publication")
        expected_publication = {
            "run_dir": str(run_dir.resolve()),
            "artifact_manifest_sha256": sha256(run_dir / "artifact-manifest.json"),
            "artifact_count": len(manifest["artifacts"]),
        }
        if attempt["state"] == "terminal":
            if attempt.get("classification") != classification or any(attempt.get(key) != value for key, value in expected_publication.items()):
                raise SystemExit("published attempt differs from the sealed publication")
        elif attempt["state"] == "terminal-unsealed":
            if attempt.get("classification") != classification:
                raise SystemExit("sealed classification differs from terminal record")
            attempt.update({"state": "terminal", "published_at_ns": time.time_ns(), **expected_publication})
            slot = ledger["slots"][attempt["slot_key"]]
            slot.update({
                "measurement_status": classification["measurement_status"],
                "replacement_allowed": classification["replacement_allowed"],
                "latest_classification": classification,
                "metrics_status": "pending",
            })
            write_json(ledger_path, ledger)
        else:
            raise SystemExit("attempt is not awaiting sealed publication")
    finally:
        unlock(guard)
    if crash_after_commit:
        raise SystemExit("synthetic crash after publish commit")
    return 0


def status(args: list[str]) -> int:
    ledger_path, generation_path = map(pathlib.Path, args[:2]); run_id = args[2]
    verify_generation(generation_path)
    generation = require_execution_generation(load_json(generation_path))
    guard = lock(ledger_path)
    try:
        ledger = load_or_create(ledger_path, generation)
        attempt = ledger["attempts"].get(run_id)
        if not attempt:
            raise SystemExit("attempt is absent")
        slot = ledger["slots"].get(attempt["slot_key"], {})
        value = {
            "run_id": run_id,
            "state": attempt["state"],
            "terminal_recorded": attempt["state"] in {"terminal-unsealed", "terminal"},
            "published": attempt["state"] == "terminal",
            "artifacts_sealed": attempt["state"] == "terminal",
            "metrics_status": slot.get("metrics_status"),
        }
    finally:
        unlock(guard)
    print(json.dumps(value, sort_keys=True))
    return 0


def metrics(args: list[str]) -> int:
    ledger_path, generation_path = map(pathlib.Path, args[:2]); run_id = args[2]
    run_dir = pathlib.Path(args[3]).resolve(); metrics_dir = pathlib.Path(args[4]).resolve()
    verify_generation(generation_path)
    generation = load_json(generation_path)
    expected_metrics_dir = run_dir.parent / "automatic-metrics" / run_id
    if run_dir.name != run_id or metrics_dir != expected_metrics_dir or metrics_dir.is_symlink():
        raise SystemExit("automatic metrics directory does not match the published run")
    verify_artifact_seal(metrics_dir, ("automatic-run-metrics.json", "extractor.stderr.log"))
    metrics_path = metrics_dir / "automatic-run-metrics.json"
    value = load_json(metrics_path)
    guard = lock(ledger_path)
    try:
        ledger = load_or_create(ledger_path, generation); attempt = ledger["attempts"].get(run_id)
        if not attempt or attempt.get("state") != "terminal" or attempt.get("run_dir") != str(run_dir):
            raise SystemExit("automatic metrics do not belong to a published terminal attempt")
        slot = ledger["slots"][attempt["slot_key"]]
        classification = attempt["classification"]
        if slot.get("latest_run_id") != run_id or slot.get("metrics_status") != "pending":
            raise SystemExit("automatic metrics are not pending for the latest slot attempt")
        experiment = value.get("experiment", {})
        run = value.get("run", {})
        integrity = run.get("integrity", {})
        if (
            value.get("schema_version") != 2
            or run.get("run_id") != run_id
            or run.get("generation_id") != generation["generation_id"]
            or experiment.get("measurement_status") != classification["measurement_status"]
            or experiment.get("published") is not True
            or experiment.get("latest_published_attempt") is not True
            or experiment.get("artifact_verified") is not True
            or integrity.get("status") != "verified"
        ):
            raise SystemExit("automatic metrics identity/publication/integrity contract failed")
        if classification["measurement_status"] == "valid":
            if experiment.get("aggregation_eligible") is not True:
                raise SystemExit("valid published attempt is not aggregation eligible")
        elif experiment.get("aggregation_eligible") is not False:
            raise SystemExit("infrastructure-invalid attempt must not be aggregation eligible")
        attempt.update({
            "metrics_status": "sealed",
            "automatic_metrics_path": str(metrics_path),
            "automatic_metrics_sha256": sha256(metrics_path),
            "automatic_metrics_manifest_sha256": sha256(metrics_dir / "artifact-manifest.json"),
        })
        slot.update({
            "metrics_status": "sealed",
            "latest_automatic_metrics_path": str(metrics_path),
            "latest_automatic_metrics_sha256": sha256(metrics_path),
        })
        valid_slots = sum(
            item.get("measurement_status") == "valid" and item.get("metrics_status") == "sealed"
            for item in ledger["slots"].values()
        )
        if valid_slots == generation["session_count"]:
            ledger["state"] = "completed"
        write_json(ledger_path, ledger)
        if ledger["state"] == "completed":
            ledger_path.chmod(ledger_path.stat().st_mode & ~0o222)
    finally:
        unlock(guard)
    return 0


def metrics_failure(args: list[str]) -> int:
    ledger_path, generation_path = map(pathlib.Path, args[:2]); run_id = args[2]
    run_dir = pathlib.Path(args[3]).resolve(); failure_dir = pathlib.Path(args[4]).resolve()
    verify_generation(generation_path)
    generation = load_json(generation_path)
    if run_dir.name != run_id or failure_dir != run_dir.parent / "automatic-metrics-failures" / run_id:
        raise SystemExit("metrics failure record path mismatch")
    verify_artifact_seal(failure_dir, ("failure-record.json",))
    guard = lock(ledger_path)
    try:
        ledger = load_or_create(ledger_path, generation); attempt = ledger["attempts"].get(run_id)
        if not attempt or attempt.get("state") != "terminal":
            raise SystemExit("metrics failure does not belong to a published terminal attempt")
        failure_manifest_sha = sha256(failure_dir / "artifact-manifest.json")
        if attempt.get("metrics_status") == "failed":
            if attempt.get("metrics_failure_manifest_sha256") != failure_manifest_sha:
                raise SystemExit("existing metrics failure record differs")
            return 0
        attempt.update({
            "metrics_status": "failed",
            "metrics_failure_record_path": str(failure_dir / "failure-record.json"),
            "metrics_failure_manifest_sha256": failure_manifest_sha,
        })
        slot = ledger["slots"][attempt["slot_key"]]
        slot["metrics_status"] = "failed"
        ledger["state"] = "invalid-restart-required"
        ledger["invalidated_by_run_id"] = run_id
        ledger["invalidated_reason"] = "sealed automatic metrics extraction or validation failed"
        write_json(ledger_path, ledger)
    finally:
        unlock(guard)
    return 0


def main() -> int:
    if len(sys.argv) < 2: raise SystemExit("usage: protocol.py claim|start|terminal|publish|metrics|metrics-failure ...")
    action, args = sys.argv[1], sys.argv[2:]
    if action == "claim" and len(args) in {6, 7} and args[5] in {"initial", "replacement"}: return claim(args)
    if action == "cancel-claim" and len(args) == 4: return cancel_claim(args)
    if action == "start" and len(args) == 3: return start(args)
    if action == "bind-guardian" and len(args) == 8: return bind_guardian(args)
    if action == "bind-process" and len(args) == 6: return bind_process(args)
    if action == "status" and len(args) == 3: return status(args)
    if action == "terminal" and len(args) == 4: return terminal(args)
    if action == "publish" and len(args) == 4: return publish(args)
    if action == "metrics" and len(args) == 5: return metrics(args)
    if action == "metrics-failure" and len(args) == 5: return metrics_failure(args)
    raise SystemExit("invalid protocol arguments")


if __name__ == "__main__": raise SystemExit(main())
