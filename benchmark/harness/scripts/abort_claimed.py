#!/usr/bin/env python3
"""Best-effort claimed-attempt finalizer used by the shell EXIT trap."""
from __future__ import annotations

import json
import os
import pathlib
import signal
import sys
import time

import protocol
from common import load_json, write_json


def group_exists(pgid: int) -> bool:
    try: os.killpg(pgid, 0)
    except ProcessLookupError: return False
    except PermissionError: return True
    return True


def stop_group(pgid: int, guardian_token: str) -> list[str]:
    return protocol.stop_verified_process_group(pgid, guardian_token)


def abort(run_dir: pathlib.Path, stage: str, exit_status: int) -> dict:
    run_dir.mkdir(parents=True, exist_ok=True)
    raw = run_dir / "raw"; raw.mkdir(exist_ok=True)
    (raw / "events.jsonl").touch(exist_ok=True); (raw / "stderr.log").touch(exist_ok=True)
    child_path = run_dir / "child-process.json"; receipt_path = run_dir / "claim-receipt.json"
    signals = []
    identity = load_json(child_path) if child_path.is_file() else (load_json(receipt_path) if receipt_path.is_file() else {})
    pgid = int(identity.get("guardian_process_group", 0) or 0); guardian_token = identity.get("guardian_token")
    if pgid > 1:
        if not isinstance(guardian_token, str):
            raise RuntimeError("claimed attempt guardian identity is missing")
        signals = stop_group(pgid, guardian_token)
    cleanup_satisfied = True
    if pgid > 1:
        cleanup_satisfied = not protocol.verified_group_targets(pgid, guardian_token)
    wrapper_path = run_dir / "wrapper.json"
    if not wrapper_path.is_file():
        write_json(wrapper_path, {
            "exit_code": exit_status, "wall_time_ms": 0, "final_model_step_reason": None,
            "terminal_contract_satisfied": False, "cleanup_satisfied": cleanup_satisfied,
            "remaining_process_group": not cleanup_satisfied, "protocol_failures": ["claimed_attempt_aborted"],
            "limits": {"timeout": False, "output_limit": False, "turn_limit": False, "model_step_limit": False, "protocol_failure": True, "signal": None},
            "output_bytes": {"kept": {"stdout": (raw / "events.jsonl").stat().st_size, "stderr": (raw / "stderr.log").stat().st_size}},
        })
    write_json(run_dir / "postprocess-status.json", {"parser_status": 1, "parser_ok": False, "required_files_present": False, "aborted_stage": stage})
    classification = {
        "measurement_status": "infrastructure_invalid", "terminal_behavior": "protocol_error",
        "replacement_allowed": False, "replacement_category": None, "generation_invalid": True,
        "reason": f"claimed attempt aborted during {stage}; fixed runner/postprocessor failure requires a new generation",
    }
    write_json(run_dir / "attempt-classification.json", classification)
    write_json(run_dir / "abort-record.json", {"stage": stage, "exit_status": exit_status, "cleanup_signals": signals, "cleanup_satisfied": cleanup_satisfied})
    if (run_dir / "run.manifest.json").is_file():
        manifest = load_json(run_dir / "run.manifest.json"); manifest.update({"state": "aborted", "abort_stage": stage, "abort_exit_status": exit_status}); write_json(run_dir / "run.manifest.json", manifest)
    return classification


def main() -> int:
    if len(sys.argv) != 4: raise SystemExit("usage: abort_claimed.py RUN_DIR STAGE EXIT_STATUS")
    abort(pathlib.Path(sys.argv[1]).resolve(), sys.argv[2], int(sys.argv[3])); return 0


if __name__ == "__main__": raise SystemExit(main())
