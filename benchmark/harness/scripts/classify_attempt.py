#!/usr/bin/env python3
"""Conservative retry classification: only clear provider/network/auth faults are replaceable."""
from __future__ import annotations

import json
import pathlib
import re
import sys

from common import load_json, write_json


TRANSIENT = {
    "transient_auth": (r"\b401\b", r"unauthori[sz]ed", r"authentication.*failed", r"invalid.*api.*key"),
    "transient_provider": (r"\b429\b", r"rate.?limit", r"service unavailable", r"\b502\b", r"\b503\b", r"\b504\b"),
    "transient_network": (r"connection reset", r"connection refused", r"temporary failure", r"timed out", r"dns", r"econnreset", r"enotfound"),
}
MCP_FAILURE = (
    r"mcp.*failed", r"failed.*mcp", r"codemap.*permission denied",
    r"codemap.*lockfile", r"tools/list.*error",
)
MCP_CONTEXT = (r"\bmcp\b", r"\bcodemap(?:[_-]?search)?\b", r"\btools/list\b")


def classify(run_dir: pathlib.Path) -> dict:
    wrapper = load_json(run_dir / "wrapper.json")
    stderr = (run_dir / "raw/stderr.log").read_text(encoding="utf-8", errors="replace") if (run_dir / "raw/stderr.log").is_file() else ""
    invariants = load_json(run_dir / "invariants.after.json")
    limits = wrapper.get("limits", {})
    generation_current = invariants.get("generation_current") is True
    source_unchanged = invariants.get("source_unchanged") is True
    index_unchanged = invariants.get("index_unchanged") is True
    cleanup = wrapper.get("cleanup_satisfied") is True and wrapper.get("remaining_process_group") is False
    protocol_failures = wrapper.get("protocol_failures", [])
    postprocess_path = run_dir / "postprocess-status.json"
    postprocess = load_json(postprocess_path) if postprocess_path.is_file() else {"parser_status": 1, "required_files_present": False}
    auth_cleanup_path = run_dir / "auth-cleanup.json"
    auth_cleanup = load_json(auth_cleanup_path) if auth_cleanup_path.is_file() else {"status": 1}

    if not generation_current or not source_unchanged or not index_unchanged:
        return {
            "measurement_status": "infrastructure_invalid", "terminal_behavior": "infrastructure_error",
            "replacement_allowed": False, "replacement_category": None, "generation_invalid": True,
            "reason": "sealed generation, source, or index changed; all samples in this generation must be discarded",
        }
    if any(re.search(pattern, stderr, re.I) for pattern in (*MCP_FAILURE, *MCP_CONTEXT)):
        return {
            "measurement_status": "infrastructure_invalid", "terminal_behavior": "infrastructure_error",
            "replacement_allowed": False, "replacement_category": None, "generation_invalid": True,
            "reason": "MCP startup/runtime failure requires a new generation after a code/config fix",
        }
    if limits.get("signal"):
        return {
            "measurement_status": "infrastructure_invalid", "terminal_behavior": "infrastructure_error",
            "replacement_allowed": False, "replacement_category": None, "generation_invalid": True,
            "reason": "operator or harness signal interrupted the attempt; transient provider replacement is not allowed",
        }
    if not cleanup or protocol_failures or postprocess.get("parser_status") != 0 or postprocess.get("required_files_present") is not True or auth_cleanup.get("status") != 0:
        return {
            "measurement_status": "infrastructure_invalid", "terminal_behavior": "protocol_error",
            "replacement_allowed": False, "replacement_category": None, "generation_invalid": True,
            "reason": "process/auth cleanup, protocol, raw capture, or fixed parser integrity failure; generation code/contract must be repaired",
        }
    if limits.get("timeout"):
        behavior = "timeout"
    elif limits.get("model_step_limit") or limits.get("turn_limit"):
        behavior = "model_step_limit"
    elif limits.get("output_limit"):
        behavior = "output_limit"
    else:
        behavior = None
    if behavior is not None:
        return {
            "measurement_status": "valid", "terminal_behavior": behavior,
            "replacement_allowed": False, "replacement_category": None, "generation_invalid": False,
            "reason": "agent-observed limit outcome; retained and never silently retried",
        }
    if wrapper.get("terminal_contract_satisfied") is True and wrapper.get("final_model_step_reason") == "stop":
        return {
            "measurement_status": "valid", "terminal_behavior": "stop",
            "replacement_allowed": False, "replacement_category": None, "generation_invalid": False,
            "reason": "normal terminal answer",
        }
    for category, patterns in TRANSIENT.items():
        if any(re.search(pattern, stderr, re.I) for pattern in patterns):
            return {
                "measurement_status": "infrastructure_invalid", "terminal_behavior": "infrastructure_error",
                "replacement_allowed": True, "replacement_category": category, "generation_invalid": False,
                "reason": "clear transient provider/network/auth failure; unchanged sealed prompt/config may be retried and this attempt remains preserved",
            }
    return {
        "measurement_status": "infrastructure_invalid", "terminal_behavior": "protocol_error" if protocol_failures or not cleanup else "process_error",
        "replacement_allowed": False, "replacement_category": None, "generation_invalid": True,
        "reason": "unclassified infrastructure/protocol failure is not safely retryable in the same generation",
    }


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: classify_attempt.py RUN_DIR OUTPUT")
    result = classify(pathlib.Path(sys.argv[1]).resolve())
    write_json(pathlib.Path(sys.argv[2]), result)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
