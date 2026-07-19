#!/usr/bin/env python3
"""Model-free MCP handshake proving clean B2 starts read-only with the six baseline tools."""
from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
import uuid

from clone_source import clone, owner_writable
from common import HARNESS_ROOT, QUALITY_ROOT, load_json, sha256, tree_digest, write_json


EXPECTED_TOOLS = {"initial_instructions", "overview", "search", "read", "find", "grep"}
FORBIDDEN_SCHEMA_MARKERS = (
    "CODEMAP_TASTE", "candidate-taste", "C2-01", "C2-02", "C2-03", "C2-04", "C2-06", "C2-07",
    "suppress_low_novelty_hint",
)


def main() -> int:
    runtime = load_json(HARNESS_ROOT / "config/b2-runtime.json")
    if runtime.get("status") not in {"binary-supplied-awaiting-probe", "verified-clean-baseline"}:
        raise SystemExit("clean B2 binary path/hash has not been supplied")
    binary = pathlib.Path(str(runtime.get("binary_path", "")))
    if not binary.is_absolute() or not binary.is_file() or sha256(binary) != runtime.get("binary_sha256"):
        raise SystemExit("clean B2 binary is absent or its supplied hash does not match")
    probe_root = HARNESS_ROOT / "synthetic" / f"b2-probe-{uuid.uuid4().hex}"
    probe_root.mkdir(parents=True)
    source = probe_root / "source"
    clone(QUALITY_ROOT / "corpus/directus", source)
    index_before = tree_digest(source / ".codemap")
    requests = [
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "baseline-preflight", "version": "1"}}},
        {"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}},
        {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "search", "arguments": {"query": "DEPLOYMENT_CACHE_TTL", "limit": 5}}},
        {"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "read", "arguments": {"file_path": "api/src/services/deployment.ts", "offset": 25, "limit": 10}}},
        {"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "overview", "arguments": {"path": "api/src/services"}}},
    ]
    payload = "".join(json.dumps(row, separators=(",", ":")) + "\n" for row in requests)
    environment = {
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "HOME": str(probe_root / "home"),
        "TMPDIR": str(probe_root),
        "CODEMAP_BASELINE_READ_ONLY": "1",
    }
    pathlib.Path(environment["HOME"]).mkdir()
    process = subprocess.run(
        [runtime["binary_path"], "mcp"],
        cwd=source,
        env=environment,
        input=payload,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        timeout=30,
        check=False,
    )
    responses = []
    parse_errors = []
    for line_number, line in enumerate(process.stdout.splitlines(), 1):
        try:
            responses.append(json.loads(line))
        except json.JSONDecodeError as error:
            parse_errors.append({"line": line_number, "error": str(error)})
    tool_response = next((row for row in responses if row.get("id") == 2), {})
    tools = tool_response.get("result", {}).get("tools", [])
    names = {tool.get("name") for tool in tools if isinstance(tool, dict)}
    rendered_schema = json.dumps(tools, ensure_ascii=False, sort_keys=True)
    call_responses = {row.get("id"): row for row in responses if row.get("id") in {3, 4, 5}}

    def response_text(request_id: int) -> str:
        response = call_responses.get(request_id, {})
        content = response.get("result", {}).get("content", [])
        return "".join(item.get("text", "") for item in content if isinstance(item, dict))

    def response_success(request_id: int) -> bool:
        response = call_responses.get(request_id, {})
        return bool(response) and "error" not in response and response.get("result", {}).get("isError") is not True

    search_text = response_text(3)
    read_text = response_text(4)
    overview_text = response_text(5)
    index_after = tree_digest(source / ".codemap")
    checks = {
        "binary_hash_matches": sha256(pathlib.Path(runtime["binary_path"])) == runtime["binary_sha256"],
        "process_exit_zero": process.returncode == 0,
        "responses_parse": not parse_errors,
        "six_exact_tools": names == EXPECTED_TOOLS,
        "tool_schema_has_no_candidate_marker": not any(marker in rendered_schema for marker in FORBIDDEN_SCHEMA_MARKERS),
        "search_call_success": response_success(3),
        "search_has_expected_path_and_symbol": "api/src/services/deployment.ts" in search_text and "DEPLOYMENT_CACHE_TTL" in search_text,
        "read_call_success": response_success(4),
        "read_has_expected_symbol": "DEPLOYMENT_CACHE_TTL" in read_text,
        "overview_call_success": response_success(5),
        "overview_nonempty": bool(overview_text.strip()),
        "index_unchanged": index_before == index_after,
        "stderr_empty": not process.stderr.strip(),
    }
    report = {
        "schema_version": 1,
        "external_model_calls": 0,
        "builds": 0,
        "indexing_operations": 0,
        "binary_path": runtime["binary_path"],
        "binary_sha256": runtime["binary_sha256"],
        "activation_environment": {"CODEMAP_BASELINE_READ_ONLY": "1"},
        "command": [runtime["binary_path"], "mcp"],
        "listed_tools": sorted(names),
        "tools_sha256": __import__("hashlib").sha256(rendered_schema.encode("utf-8")).hexdigest(),
        "index_before_sha256": index_before,
        "index_after_sha256": index_after,
        "checks": checks,
        "passed": all(checks.values()),
        "parse_errors": parse_errors,
        "stderr": process.stderr,
    }
    output = HARNESS_ROOT / "reports/b2-mcp-probe.json"
    write_json(output, report)
    owner_writable(probe_root)
    __import__("shutil").rmtree(probe_root)
    print(json.dumps({"passed": report["passed"], "report": str(output)}, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
