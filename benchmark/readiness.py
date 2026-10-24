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


def degradation(ident: str, result: dict) -> str | None:
    """Match native status notices, not prose such as 'not a stale cache' or '0 unavailable'."""
    value = text(result)
    patterns = [r"(?im)^\s*(?:status:\s*)?(?:not[- ]ready|warming(?: up)?|stale|unavailable)\s*[.!:]?\s*$"]
    if ident == "codemap-search":
        patterns += [r"(?i)codemap is warming up|symbol index unavailable|index unavailable:|navigation unavailable",
                     r"(?i)\b(?:index|indexer|snapshot)\b[^\n]{0,100}(?:is stale|or stale|not ready|stopped|dead|unavailable)"]
    elif ident == "codegraph":
        patterns += [r"(?i)isn't indexed|not indexed|no codegraph project is loaded|entries may be stale|pending index sync|auto-sync is DISABLED"]
    elif ident == "zvec-grep":
        patterns += [r"(?m)^freshness: (?!fresh$).+|^background_refresh: (?!inactive$|idle$|completed$).+",
                     r"(?im)^failed to open zvec collection storage"]
    elif ident == "serena":
        patterns += [r"(?i)language server[^\n]*(?:loading|initializing|not initialized|not ready|unavailable)|timeout waiting|indexing did not complete within",
                     r"(?im)^error[^\n]*(?:initializing language server|language server manager)"]
    elif ident == "graphify":
        patterns += [r"(?im)^(?:error[^\n]*: )?(?:no graph loaded|graph file not found|failed to load graph)"]
    elif ident == "codebase-memory-mcp":
        patterns += [r"(?im)^(?:error[^\n]*: )?(?:search failed: cannot create temp file|project[^\n]*not indexed|index[^\n]*not ready)"]
    for pattern in patterns:
        match = re.search(pattern, value)
        if match:
            return match.group(0)
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
    check(not re.search(r"(?im)^\s*(?:error executing\b|error[: ]|no active project\b)", value), "native error text")
    check(degradation(ident, result) is None, degradation(ident, result) or "degraded")
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
        check(not any(line.startswith("background_refresh:") for line in value.splitlines()), "refresh still present")
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
