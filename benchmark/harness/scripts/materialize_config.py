#!/usr/bin/env python3
"""Create exact B1/B2 OpenCode configs; fail closed while clean B2 is unresolved."""
from __future__ import annotations

import json
import pathlib
import re
import sys

from common import HARNESS_ROOT, MODEL, sha256, write_json


def load_b2_runtime() -> dict:
    contract_path = HARNESS_ROOT / "config/b2-runtime.json"
    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    required = {
        "status", "binary_path", "binary_sha256", "source_commit", "source_patch_scope",
        "attestation_path", "attestation_sha256",
    }
    if set(contract) != required or contract.get("status") != "verified-clean-baseline":
        raise SystemExit("clean B2 runtime is unresolved; baseline execution is fail-closed")
    binary = pathlib.Path(contract["binary_path"])
    attestation = pathlib.Path(contract["attestation_path"])
    if not binary.is_absolute() or not binary.is_file() or not binary.stat().st_mode & 0o111:
        raise SystemExit("clean B2 binary is absent or not executable")
    if not re.fullmatch(r"[0-9a-f]{64}", contract["binary_sha256"]) or sha256(binary) != contract["binary_sha256"]:
        raise SystemExit("clean B2 binary hash mismatch")
    if not attestation.is_absolute() or not attestation.is_file():
        raise SystemExit("clean B2 attestation is absent")
    if not re.fullmatch(r"[0-9a-f]{64}", contract["attestation_sha256"]) or sha256(attestation) != contract["attestation_sha256"]:
        raise SystemExit("clean B2 attestation hash mismatch")
    return contract


def materialize(criterion: str) -> tuple[dict, dict | None]:
    if criterion == "B1":
        config = json.loads((HARNESS_ROOT / "config/baseline-1.json").read_text(encoding="utf-8"))
        runtime = None
    elif criterion == "B2":
        runtime = load_b2_runtime()
        config = json.loads((HARNESS_ROOT / "config/baseline-2.template.json").read_text(encoding="utf-8"))
        config["mcp"]["codemap_search"]["command"][0] = runtime["binary_path"]
    else:
        raise SystemExit("criterion must be B1 or B2")
    if config.get("model") != MODEL:
        raise SystemExit("model drift")
    return config, runtime


def pair() -> tuple[dict, dict, dict]:
    b1, _ = materialize("B1")
    b2, runtime = materialize("B2")
    if b1.get("mcp") != {} or list(b2.get("mcp", {})) != ["codemap_search"]:
        raise SystemExit("MCP contract is not exactly B1=0/B2=codemap_search")
    if {key: value for key, value in b1.items() if key != "mcp"} != {key: value for key, value in b2.items() if key != "mcp"}:
        raise SystemExit("B1/B2 non-MCP configuration drift")
    evidence = {
        "model": MODEL,
        "allowed_normalized_diff": ["mcp"],
        "b1": {"mcp_count": 0, "mcp_names": []},
        "b2": {
            "mcp_count": 1,
            "mcp_names": ["codemap_search"],
            "binary_path": runtime["binary_path"],
            "binary_sha256": runtime["binary_sha256"],
            "attestation_sha256": runtime["attestation_sha256"],
        },
        "candidate_sessions": 0,
        "forced_mcp_use": False,
    }
    return b1, b2, evidence


def main() -> int:
    if len(sys.argv) == 3 and sys.argv[1] in {"B1", "B2"}:
        config, _ = materialize(sys.argv[1])
        write_json(pathlib.Path(sys.argv[2]), config)
        return 0
    if len(sys.argv) == 5 and sys.argv[1] == "pair-evidence":
        b1, b2, evidence = pair()
        b1_path, b2_path, evidence_path = map(pathlib.Path, sys.argv[2:])
        write_json(b1_path, b1)
        write_json(b2_path, b2)
        evidence["b1"]["config_sha256"] = sha256(b1_path)
        evidence["b2"]["config_sha256"] = sha256(b2_path)
        write_json(evidence_path, evidence)
        return 0
    raise SystemExit("usage: materialize_config.py B1|B2 OUTPUT | pair-evidence B1_OUTPUT B2_OUTPUT EVIDENCE")


if __name__ == "__main__":
    raise SystemExit(main())
