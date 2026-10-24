"""Semantic index readiness shared by disposable preparation and live solver sessions."""
from __future__ import annotations

import json
import os
import re
import subprocess
import time
from collections.abc import Callable
from pathlib import Path

from .core import ContractError, append_jsonl, digest, file_digest, read_json, require

GO_FILE = "pkg/setting/setting.go"
TS_FILE = "packages/grafana-data/src/utils/uuid.ts"
GO_SYMBOL = "DefaultSQLExpressionCellLimit"
TS_SYMBOL = "generateUUID"
POLICY = {"revision": 1, "poll_seconds": 1, "stable_observations": 2,
          "go": {"path": GO_FILE, "symbol": GO_SYMBOL}, "typescript": {"path": TS_FILE, "symbol": TS_SYMBOL},
          "session_strategy": {"codemap-search": "root overview only; forward solver initialize to clear delivery dedup",
              "codegraph": "forward initialize; native CODEGRAPH_EXPLORE_DEDUP off",
              "zvec-grep": "DELETE warm HTTP session; solver initializes fresh session on same server",
              "graphify": "graph_stats only; query_graph checked in disposable preparation",
              "codebase-memory-mcp": "index_status, graph and live-source search/snippet",
              "serena": "config, Go/TypeScript symbols and references; no initial_instructions",
              "rg": "no search MCP or index warm-up"}}


class NotReady(ContractError):
    pass


def text(result: dict) -> str:
    return "\n".join(c.get("text", "") for c in result.get("content", []) if c.get("type") == "text")


def degradation(ident: str, result: dict, *, tool: str | None = None) -> str | None:
    """Inspect native status envelopes/banners only, never search returned source prose."""
    if not isinstance(result, dict):
        return "malformed native response"
    if result.get("isError"):
        return "native isError"
    if result.get("error"):
        return "native error envelope"
    value = text(result)
    # Some products encode errors as JSON text rather than MCP isError.
    json_envelope = (ident == "graphify" and tool not in {None, "read"}
                     or ident == "serena" and tool not in {None, "read_file"}
                     or ident == "codebase-memory-mcp" and tool is not None)
    if json_envelope and value.startswith("{"):
        try:
            native = json.loads(value)
        except ValueError:
            native = None
        if isinstance(native, dict) and native.get("error"):
            return "native JSON error envelope"
    if not tool:
        return None  # resources/prompts can contain arbitrary, unframed source text.
    first = value.split("\n", 1)[0]
    if ident == "zvec-grep" and tool == "zvec_grep_search":
        # These contiguous leading headers end before the first search hit/body.
        for line in value.splitlines():
            key, separator, status = line.partition(": ")
            if not separator or key not in {"freshness", "background_refresh"}:
                break
            if status not in ({"fresh"} if key == "freshness" else {"inactive", "idle", "completed"}):
                return line
    elif ident == "codemap-search":
        if tool in {"overview", "search", "initial_instructions"} and first.startswith((
                "Index unavailable:", "Codemap is warming up (", "Background indexer stopped before the codemap was built;",
                "_Background indexer stopped —", "_Index is warming up (", "_Last background index refresh failed:")):
            return first
        if tool in {"read", "grep", "search"}:
            # Native notices are outside fenced/numbered source bodies; match the exact
            # product block, not words like 'index ... stopped' inside a source line.
            fenced = False
            for line in value.splitlines():
                if line.startswith("```"):
                    fenced = not fenced
                if not fenced and line == "[Symbol index unavailable or stale; live results remain available below.]":
                    return line
    elif ident == "codegraph" and tool == "codegraph_explore":
        if first.startswith(("⚠️ Some files referenced below were edited since the last index sync",
                             "⚠️ CodeGraph auto-sync is DISABLED", "⚠️ CodeGraph auto-sync is RECOVERING",
                             "No CodeGraph project is loaded for this session.")):
            return first
        if re.fullmatch(r"The project at .+ isn't indexed with codegraph.*", first):
            return first
    elif ident == "serena":
        if first.startswith(("Error executing tool:", "Error:", "No active project.")):
            return first
        if tool == "get_current_config":
            for line in value.splitlines():
                if line.startswith("Language server status: ") and line != "Language server status: ready":
                    return line
    elif ident == "graphify" and tool != "read":
        if re.match(r"(?i)^(?:error[^\n]*: )?(?:no graph loaded|graph file not found|failed to load graph)\b", first):
            return first
    elif ident == "codebase-memory-mcp" and tool == "index_status":
        native = result.get("structuredContent")
        if native is None:
            try:
                native = json.loads(value)
            except ValueError:
                native = None
        if isinstance(native, dict) and native.get("status") != "ready":
            return "native index_status not ready"
    return None


def assert_ready(ident: str, name: str, result: dict, state: dict | None = None) -> None:
    state = state or {}
    if not isinstance(result, dict):
        raise NotReady(f"{ident}/{name}: malformed native response")
    value = text(result)
    def check(ok, reason):
        if not ok:
            raise NotReady(f"{ident}/{name}: {reason}")
    check(isinstance(result, dict) and not result.get("isError"), "native isError")
    check(bool(value), "empty native response")
    check(degradation(ident, result, tool=name) is None, degradation(ident, result, tool=name) or "degraded")
    if ident == "codemap-search":
        if name == "overview":
            for metric in ("Total Files", "Total Symbols"):
                check(re.search(rf"(?m)^- \*\*{metric}\*\*: [1-9]\d*$", value), f"empty {metric}")
        elif name in {"search", "read"}:
            check(GO_FILE in value or TS_FILE in value, "neutral indexed source missing")
    elif ident == "codegraph":
        check(re.search(r"Found [1-9]\d* symbols? across [1-9]\d* files?", value), "empty explore result")
        check("Source Code" in value, "live source missing")
    elif ident == "zvec-grep":
        check(value.splitlines()[0] == "freshness: fresh", "query is not fresh")
        for line in value.splitlines():
            if not line.startswith(("freshness: ", "background_refresh: ")):
                break
            check(not line.startswith("background_refresh: "), "refresh still present")
        check(re.search(r"(?m)^#\d+ .*", value), "empty search")
    elif ident == "graphify":
        if name == "graph_stats":
            for metric in ("Nodes", "Edges"):
                check(re.search(rf"(?m)^{metric}: [1-9]\d*$", value), f"empty {metric}")
        elif name == "query_graph":
            check(re.search(r"[1-9]\d* nodes found", value) and "NODE " in value, "empty graph query")
    elif ident == "serena":
        if name == "get_current_config":
            for marker in (f"Active project: {state.get('modes', {}).get('project', 'grafana')}", "Language backend: LSP",
                           "Language server status: ready", "Agent interface: tools"):
                check(marker in value, f"missing {marker}")
        else:
            symbols = json.loads(value)
            check(isinstance(symbols, (dict, list)) and bool(symbols), "empty symbols/references")
            if name == "find_referencing_symbols":
                check("content_around_reference" in value and "name_path" in value, "no live LSP reference locations")
    elif ident == "codebase-memory-mcp":
        if name == "index_status":
            native = result.get("structuredContent") or json.loads(value)
            expected = state["modes"]["native_status"]
            for key in ("project", "root_path", "nodes", "edges"):
                check(native.get(key) == expected[key], f"index {key} identity/count changed")
            check(native.get("status") == "ready" and native["root_path"] == state["source"], "wrong root/status")
            for key, count in (("parse_partial", "count"), ("parse_unusable", "count"), ("skipped", "count"), ("not_indexed", "files_count")):
                check(native.get(key, {}).get(count) == expected.get(key, {}).get(count), f"coverage changed: {key}")
        elif name == "search_code":
            check("package setting" in value and GO_FILE in value, "live-source match missing")
        elif name == "search_graph":
            check(GO_SYMBOL in value and GO_FILE in value and re.search(r"(?m)^results: [1-9]\d*", value), "neutral graph symbol missing")
        elif name == "get_code_snippet":
            check(GO_SYMBOL in value and f"file_path: {Path(state['source']) / GO_FILE}\n" in value
                  and "source_mode: full\n" in value and "source: |\n  " in value, "live-source snippet missing")


def probes(state: dict, *, preparation=False) -> list[tuple[str, dict]]:
    ident, source = state["id"], state["source"]
    if ident == "codemap-search":
        return [("overview", {})] + ([("search", {"query": GO_SYMBOL}), ("read", {"file_path": TS_FILE})] if preparation else [])
    if ident == "codegraph":
        return [("codegraph_explore", {"query": "configuration settings", "projectPath": source})]
    if ident == "zvec-grep":
        return [("zvec_grep_search", {"query": "configuration settings", "root": source})]
    if ident == "graphify":
        return [("graph_stats", {})] + ([("query_graph", {"question": "configuration settings"})] if preparation else [])
    if ident == "codebase-memory-mcp":
        project = state["modes"]["native_readiness"]["project"]
        return [("index_status", {"project": project, "format": "json"}),
                ("search_graph", {"project": project, "name_pattern": GO_SYMBOL}),
                ("search_code", {"pattern": "package setting", "project": project}),
                ("get_code_snippet", {"project": project, "qualified_name": GO_SYMBOL})]
    if ident == "serena":
        return [("get_current_config", {}), ("get_symbols_overview", {"relative_path": GO_FILE}),
                ("get_symbols_overview", {"relative_path": TS_FILE}),
                ("find_referencing_symbols", {"name_path": GO_SYMBOL, "relative_path": GO_FILE}),
                ("find_referencing_symbols", {"name_path": TS_SYMBOL, "relative_path": TS_FILE})]
    require(ident == "rg", "unknown readiness arm")
    return []


def load_snapshot() -> dict:
    result = subprocess.run(["ps", "-A", "-o", "pid,ppid,%cpu,%mem,comm"], capture_output=True, text=True, timeout=5)
    rows = result.stdout.splitlines()[1:]
    rows.sort(key=lambda row: float(row.split(None, 4)[2]), reverse=True)
    return {"monotonic": time.monotonic(), "unix_seconds": time.time(), "load_average": list(os.getloadavg()),
            "cpu_count": os.cpu_count(), "top_cpu": rows[:10], "ps_exit_code": result.returncode}


def prepared_evidence(state: dict, artifact: dict) -> dict:
    """Check offline completion at final paths before any live process can mutate indexes."""
    ident, source = state["id"], Path(state["source"]).resolve()
    require(str(source) == state["source"] and digest(artifact) == state["artifact_sha256"], "prepared artifact/root mismatch")
    commands = state["commands"]
    require(ident == "rg" or commands, "missing index build evidence")
    for row in commands:
        require(row["exit_code"] == 0 and file_digest(Path(row["log"])) == row["log_sha256"], "index build evidence changed")
    evidence = {"index_build_seconds": sum(row["elapsed_seconds"] for row in commands), "files": {}}
    paths = []
    if ident == "codemap-search":
        meta = source / ".codemap/index/meta.json"
        require(read_json(meta).get("segments"), "no committed codemap segments")
        roots = source / ".codemap/index/indexed-roots.json"
        require({Path(p).resolve() for p in read_json(roots)} == {source}, "codemap indexed root mismatch")
        paths = [meta, roots, source / ".codemap/config.toml"]
    elif ident == "codegraph":
        native = state["modes"]["native_readiness"]
        index = native["index"]
        require(native["initialized"] and native["fileCount"] > 0 and index["state"] == "complete"
                and index["pendingRefs"] == 0 and not index["reindexRecommended"] and not native["worktreeMismatch"]
                and native["pendingChanges"] == {"added": 0, "modified": 0, "removed": 0}
                and native["version"] == index["builtWithVersion"] == artifact["source_pin"]["version"], "CodeGraph index incomplete")
        require(not state["env"].get("CODEGRAPH_EXPLORE_DEDUP"), "CodeGraph native dedup default changed")
    elif ident == "zvec-grep":
        manifest = source / ".zvec-grep/manifest.json"
        native = read_json(manifest)
        require(Path(native["path"]).resolve() == source / ".zvec-grep"
                and {Path(p["absolutePath"]).resolve() for p in native["rootPaths"]} == {source}, "zvec root mismatch")
        require(native["embedding"]["model"] == "potion-code-16m-v2" and native["embedding"]["provider"] == "local", "zvec model mismatch")
        from .route_contract import load_profile
        revision = load_profile()["index_mode"]["zvec_model_revision"]
        require(state["modes"]["model_revision"] == revision, "zvec model revision mismatch")
        for item in artifact["model_assets"]:
            name = "tokenizer/tokenizer.json" if Path(item["path"]).name == "tokenizer.json" else "model.safetensors"
            path = Path(state["home"]) / "model-cache/model2vec/minishlab--potion-code-16M-v2" / revision / name
            require(file_digest(path) == item["sha256"], "zvec model bytes changed")
            paths.append(path)
        paths.append(manifest)
    elif ident == "graphify":
        paths = [Path(state["modes"]["graph_path"])]
        require(paths[0].is_relative_to(Path(state["home"])) and paths[0].is_file(), "Graphify final graph missing")
    elif ident == "serena":
        modes = state["modes"]
        require(modes["configuration"]["registered_projects"] == [str(source)]
                and modes["configuration"]["project"] == str(source / ".serena/project.yml"), "Serena root mismatch")
        require(modes["failed_files"] == 0 and all(modes["indexed_files_per_language"].get(k, 0) > 0 for k in ("go", "typescript")), "Serena indexing incomplete")
        for item in modes["symbol_caches"]:
            path = Path(item["path"])
            require(file_digest(path) == item["sha256"], "Serena symbol cache changed")
            paths.append(path)
    elif ident == "codebase-memory-mcp":
        modes = state["modes"]
        require(modes["native_status"]["root_path"] == str(source), "CBM final root mismatch")
        for key in ("nodes", "edges"):
            require(modes["native_status"][key] == modes["native_readiness"][key] > 0, "CBM index counts mismatch")
        require(state["env"]["CBM_RUNTIME_DIR"] == modes["runtime_dir"] and state["env"]["CBM_CACHE_DIR"] == state["home"], "CBM runtime/cache mismatch")
    evidence["files"] = {str(p): file_digest(p) for p in paths}
    return evidence


def await_ready(state: dict, call: Callable[[str, dict], dict], deadline: float, cancel_event, log: Path) -> dict:
    started, consecutive, observations = time.monotonic(), 0, []
    first_usable = last_not_ready = None
    while consecutive < POLICY["stable_observations"]:
        require(not cancel_event.is_set(), "warm-up cancelled")
        require(time.monotonic() < deadline, "semantic readiness deadline exceeded")
        try:
            for name, arguments in probes(state):
                result = call(name, arguments)
                assert_ready(state["id"], name, result, state)
                item = {"tool": name, "arguments": arguments, "result_sha256": digest(result), "at": time.monotonic()}
                observations.append(item)
                first_usable = first_usable or item["at"]
            consecutive += 1
            append_jsonl(log, {"event": "semantic_ready", "consecutive": consecutive, "at": time.monotonic()})
        except NotReady as exc:
            consecutive = 0
            last_not_ready = {"at": time.monotonic(), "reason": str(exc)}
            append_jsonl(log, {"event": "not_ready", **last_not_ready})
        if consecutive < POLICY["stable_observations"]:
            cancel_event.wait(min(POLICY["poll_seconds"], max(0, deadline - time.monotonic())))
    require(time.monotonic() <= deadline and not cancel_event.is_set(), "semantic readiness exceeded deadline/cancelled")
    return {"assertions": observations, "last_not_ready": last_not_ready, "first_usable_at": first_usable,
            "stable_ready_at": time.monotonic(), "query_gate_seconds": time.monotonic() - started,
            "policy": POLICY, "load_ready": load_snapshot()}
