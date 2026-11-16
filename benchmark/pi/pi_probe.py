"""Loopback acceptance gate for the installed pi adapter/bridge, not a hosted model test."""
from __future__ import annotations

import json
from pathlib import Path

from . import settings
from .core import digest, file_digest, read_json, read_jsonl, require, write_json
from .claude_auth import CHILD_ENV_CONTRACT, verify_bridge_api

PROBE_SCHEMA = "pi-installed-behavior-v1"
IMPLEMENTATION = ("pi_probe.py", "pi_client.py", "pi_audit.py", "claude_auth.py", "claude_settings.py", "client_delivery.py", "isolation.py", "core.py", "tool_artifacts.py")
NATIVE_DESCRIPTION_MARKER = "… [truncated]"


def implementation_hashes() -> dict:
    return {name: file_digest(settings.ROOT / name) for name in IMPLEMENTATION}


def tool_list_observer(tools: dict, root: Path) -> Path:
    """Record the SDK MCP response unchanged; observe only tools/list, never requests or env."""
    path = root / "observe-tool-list.mjs"
    protocol = Path(tools["pi_cli"]).parents[3] / "@modelcontextprotocol/sdk/dist/esm/shared/protocol.js"
    path.write_text('import {Protocol} from ' + json.dumps(str(protocol)) + ';\n' + '''
import {appendFileSync} from 'node:fs';
const original = Protocol.prototype.setRequestHandler;
Protocol.prototype.setRequestHandler = function(schema, handler) {
  return original.call(this, schema, async (...args) => {
    const response = await handler(...args);
    if (args[0]?.method === 'tools/list') appendFileSync(process.env.BENCH_PROBE_TOOL_LIST, JSON.stringify(response) + '\\n');
    return response;
  });
};
''')
    return path


def bridge_tools(folder: Path) -> dict:
    rows = read_jsonl(folder / "bridge-tools.jsonl")
    require(bool(rows), "bridge tools/list observation missing")
    tools = {}
    for row in rows:
        for tool in row["tools"]:
            require(tool["name"] not in tools or tools[tool["name"]] == tool, "bridge tools/list changed during first request")
            tools[tool["name"]] = tool
    return tools


def description_delivery(body: dict, full_tools: dict) -> dict:
    from .pi_audit import request_tools
    items, violations = [], []
    for tool in request_tools(body):
        name = tool["name"].removeprefix("mcp__custom-tools__")
        if name == "DeferredToolPlaceholder":
            require(tool.get("defer_loading") is True and tool.get("description") ==
                    "Reserved placeholder that keeps deferred tool loading active; never call this tool.", "unexpected SDK placeholder")
            continue
        original = full_tools.get(name, {}).get("description")
        actual = tool.get("description", "")
        truncated = actual.endswith(NATIVE_DESCRIPTION_MARKER)
        prefix = actual[:-len(NATIVE_DESCRIPTION_MARKER)] if truncated else actual
        exact = isinstance(original, str) and (original.startswith(prefix) and len(prefix) < len(original)
                                              if truncated else actual == original)
        if not exact:
            violations.append({"name": name, "kind": "unproven_tool_description_change"})
        size = lambda value: len(value.encode("utf-16-le", "surrogatepass")) // 2 if isinstance(value, str) else None
        items.append({"name": name, "native_truncated": truncated, "original_units": size(original),
                      "delivered_units": size(actual), "exact_full_or_native_prefix": exact})
    return {"passed": not violations, "tools": items, "violations": violations, "length_unit": "UTF-16 code units"}


def run_behavior_probe(tools: dict, modules: Path, root: Path, *, port: int = 0, test_double: bool = False) -> dict:
    from . import pi_audit, pi_client, tool_artifacts
    expected = {"pi_cli": "@earendil-works/pi-coding-agent/dist/cli.js", "bridge_extension": "@buyong/pi-claude-bridge/src/index.ts",
                "mcp_adapter_extension": "@buyong/pi-mcp-adapter", "claude_code_binary": "@anthropic-ai/claude-agent-sdk-darwin-arm64/claude"}
    require(test_double or all(Path(tools[key]).resolve() == (modules / value).resolve() for key, value in expected.items()),
            "behavior probe must use the installed package entries")
    tool_artifacts.validate_bridge_entry(modules)
    root.mkdir(parents=True, exist_ok=False)
    tree = digest(tool_artifacts.tree_files(modules))
    record = {"schema": PROBE_SCHEMA, "passed": False, "test_double": test_double, "node_modules_sha256": tree,
              "implementation_hashes": implementation_hashes(), "conditions": {}, "hosted_model_requests": 0}
    try:
        record["bridge_api"] = verify_bridge_api(tools, root / "api")
        candidate = {**tools, "bridge_child_env_api": CHILD_ENV_CONTRACT}
        source = root / "source"
        source.mkdir()
        (source / "README.md").write_text("Local behavior fixture.\n")
        native_fixture = root / "native-fixture"
        native_fixture.write_text("#!/bin/sh\nexit 0\n")
        native_fixture.chmod(0o700)
        cases = {
            "codemap-search": [{"name": "read", "description": "Read supplied source.", "inputSchema": {"type": "object", "properties": {}},
                                "_meta": {"anthropic/maxResultSizeChars": 100000, "vendor/test": "preserved"}}],
            "codegraph": [{"name": "codegraph_explore", "description": "Inspect supplied source.", "inputSchema": {"type": "object", "properties": {}}}],
            "graphify": [{"name": "graph_stats", "description": "Summarize supplied source.", "inputSchema": {"type": "object", "properties": {}}}],
            "rg": [],
        }
        for arm, inventory_tools in cases.items():
            inventory = {"initialize": {"protocolVersion": "2025-03-26", "serverInfo": {"name": "local", "version": "1"},
                "capabilities": {"tools": {}, **({"resources": {}} if arm == "graphify" else {})},
                "instructions": "Inspect only supplied source." if arm != "rg" else ""}, "tools": inventory_tools}
            inventory_path = root / (arm + ".json")
            write_json(inventory_path, inventory)
            state = {"source": str(source), "native_inventory": str(inventory_path), "native_inventory_sha256": file_digest(inventory_path),
                     "relay_settings": {"arm": arm, "source": str(source), "rg_binary": str(native_fixture),
                                        "fd_binary": str(native_fixture), "enabled_tools": [t["name"] for t in inventory_tools], "auxiliaries": []} if arm != "rg" else None}
            for mode in ("direct", "codemode"):
                spec = settings.execution_settings("claude-opus-5-5")
                spec["measurement_mode"] = mode
                if arm == "rg":
                    # The guard probe reads outside the source only in direct; no rg or fd invocation is needed.
                    spec["native_agent"] = pi_client.configuration(source, native_fixture, native_fixture)
                folder = root / f"{arm}-{mode}"
                env_probe = arm == "rg" and mode == "direct"
                result = pi_audit.capture(folder, state, spec, "Inspect only supplied source.", toolchain=candidate, port=port,
                                          guard_probe=env_probe)
                require(result["passed"], "installed pi first-request/codemode audit failed")
                actual = bridge_tools(folder)
                # pi read tools and codemode keep the bridge default in every arm; product budgets are unchanged.
                expected = {name: 52224 if name in {*pi_client.NATIVE_TOOLS, "codemode"} else 100000 if arm == "codemap-search" else 500000
                            for name in actual}
                require(actual and all(t.get("_meta", {}).get("anthropic/maxResultSizeChars") == expected[name] for name, t in actual.items()),
                        "installed bridge result-size metadata mismatch")
                if arm == "codemap-search" and mode == "direct":
                    require(actual["mcp__navigation__read"]["_meta"].get("vendor/test") == "preserved", "product metadata was not preserved")
                require(result["oauth_header_matches"] == [True] * (2 if env_probe else 1), "bridge provider OAuth/header check failed")
                cleanup = read_json(folder / "private-cleanup.json")
                require(cleanup["complete"] and cleanup["private_home_removed"], "private Claude HOME cleanup failed")
                record["conditions"][f"{arm}/{mode}"] = {"passed": True, "expected_result_chars": expected,
                    "tools": {name: tool["_meta"] for name, tool in actual.items()}, "audit": str(folder / "audit.json"),
                    "audit_sha256": file_digest(folder / "audit.json"), "tool_list": str(folder / "bridge-tools.jsonl"),
                    "tool_list_sha256": file_digest(folder / "bridge-tools.jsonl"), "request": result["request"],
                    "request_sha256": result["request_sha256"], "nested_catalog": result["nested_catalog"],
                    "description_delivery": result["description_delivery"], "read_guard": result["read_guard"],
                    "private_home_removed": True}
        require(digest(tool_artifacts.tree_files(modules)) == tree, "installation changed during behavior probe")
        require(implementation_hashes() == record["implementation_hashes"], "harness changed during behavior probe")
        record["passed"] = True
    finally:
        write_json(root / "result.json", record)
    return {"path": str(root / "result.json"), "sha256": file_digest(root / "result.json"), "node_modules_sha256": tree}


def verify_behavior_proof(pins: dict, actual_tree: str | None = None) -> dict:
    proof = pins.get("behavior_probe") or {}
    path = Path(proof.get("path") or ".")
    require(path.is_file() and file_digest(path) == proof.get("sha256"), "pi 게시물 행동 검증 증거가 없습니다/변경됐습니다")
    value = read_json(path)
    require(value.get("test_double") is False, "대역 extension 행동 증거는 운영 준비에 사용할 수 없습니다")
    require(value.get("schema") == PROBE_SCHEMA and value.get("passed") is True
            and value.get("node_modules_sha256") == proof.get("node_modules_sha256") == pins["node_modules_sha256"]
            and (actual_tree is None or actual_tree == value["node_modules_sha256"]), "pi 행동 증거와 설치 tree 불일치")
    require(value.get("implementation_hashes") == implementation_hashes(), "pi 검사 구현 변경: python3 -m benchmark.pi.pi_lock reprobe 필요")
    require(value.get("bridge_api", {}).get("contract") == CHILD_ENV_CONTRACT, "bridge child-env API 증거 누락")
    expected = {f"{arm}/{mode}" for arm in ("codemap-search", "codegraph", "graphify", "rg") for mode in ("direct", "codemode")}
    require(set(value.get("conditions", {})) == expected and all(row.get("passed") is True for row in value["conditions"].values()),
            "pi 게시물 행동 검사 조건 누락")
    require(value["conditions"]["rg/direct"].get("read_guard") ==
            {"requests": 2, "outside_read_rejected": True, "credential_and_path_absent": True}, "pi provider read guard 검사 증거 누락")
    for row in value["conditions"].values():
        for key in ("audit", "tool_list", "request"):
            require(file_digest(Path(row[key])) == row[key + "_sha256"], "pi 행동 검증 원자료 변경")
    return value
