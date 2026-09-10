"""A narrow stdio MCP relay. No model-authored shell commands are executed."""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import signal
import fcntl
import time
from pathlib import Path

from .core import SPEC, ContractError, append_jsonl, canonical, read_json, require, safe_path


def tool_schema(name, description, properties, required):
    return {"name": name, "description": description,
            "inputSchema": {"type": "object", "properties": properties, "required": required, "additionalProperties": False},
            "annotations": {"readOnlyHint": True, "destructiveHint": False}}


TEXT = {"type": "string"}
INTEGER = {"type": "integer", "minimum": 1}
BASELINE_TOOLS = read_json(Path(__file__).resolve().parents[1] / "data/a-tools.json")


def content_text(result: dict) -> str:
    return "\n".join(c.get("text", "") for c in result.get("content", []) if c.get("type") == "text")


def text_result(text: str, is_error=False) -> dict:
    return {"content": [{"type": "text", "text": text}], "isError": is_error}


def trim_result(result: dict, byte_limit: int) -> tuple[dict, bool]:
    # Only the response sent over stdio is eligible for later exposure analysis.
    raw = content_text(result)
    encoded = raw.encode()
    if len(encoded) <= byte_limit:
        return result, False
    suffix = "\n[host output truncated]"
    text = encoded[:byte_limit - len(suffix.encode())].decode("utf-8", errors="ignore") + suffix
    return text_result(text, result.get("isError", False)), True


def baseline(root: Path, name: str, arguments: dict) -> tuple[dict, dict]:
    schema = next(t for t in BASELINE_TOOLS if t["name"] == name)["inputSchema"]
    require(set(arguments) <= set(schema["properties"]) and set(schema["required"]) <= set(arguments), "unknown/missing arguments")
    path = safe_path(root, arguments.get("path", "."))
    if name == "read":
        path = safe_path(root, arguments["file_path"])
        offset, limit = arguments.get("offset", 1), arguments.get("limit", SPEC["host"]["baseline_read_lines"])
        require(type(offset) is int and type(limit) is int and 1 <= offset and 1 <= limit <= 10000, "invalid read window")
        lines = path.read_text(encoding="utf-8").splitlines()
        text = "\n".join(f"{i + 1:6}→{line}" for i, line in enumerate(lines) if offset <= i + 1 < offset + limit)
        return text_result(text), {"result_count": None, "product_truncated": False}
    pattern = arguments["pattern"]
    require(isinstance(pattern, str) and "\x00" not in pattern, "invalid search pattern")
    if name == "find":
        selector = "-path" if "/" in pattern else "-name"
        args = ["find", str(path), "-type", "f", selector, ("*/" + pattern) if selector == "-path" else pattern]
    else:
        context = arguments.get("context_lines", 0)
        require(type(context) is int and 0 <= context <= 20, "invalid context")
        if name == "rg":
            args = ["rg", "--no-config", "--color", "never", "--line-number", "--with-filename", "--no-heading",
                    "--glob", "!.git/**", "--glob", "!.codemap/**", "--glob", "!.codex/**"]
            if arguments.get("glob"):
                args += ["--glob", arguments["glob"]]
        else:
            args = ["grep", "-r", "-n", "-H", "-I", "-E", "--exclude-dir=.git", "--exclude-dir=.codemap", "--exclude-dir=.codex"]
            if arguments.get("glob"):
                args += ["--include=" + arguments["glob"]]
        if arguments.get("case_insensitive"):
            args += ["-i"]
        if context:
            args += ["-C", str(context)]
        args += ["--", pattern, str(path)]
    proc = subprocess.run(args, cwd=root, capture_output=True, timeout=45)
    if proc.returncode not in ({0, 1} if name != "find" else {0}):
        return text_result(proc.stderr.decode(errors="replace"), True), {"result_count": None, "product_truncated": False}
    text = proc.stdout.decode(errors="replace").replace(str(root.resolve()) + "/", "")
    lines = text.splitlines()
    if name == "find":
        lines = [line for line in lines if not any(part in {".git", ".codemap", ".codex"} for part in Path(line).parts)]
    count = len(lines)
    limit = SPEC["host"]["baseline_search_matches"]
    result = "\n".join(lines[:limit])
    if count > limit:
        result += f"\n[product output truncated; total lines: {count}]"
    return text_result(result), {"result_count": count, "product_truncated": count > limit}


def source_lines(tool: str, arguments: dict, text: str, *, partial=False) -> list[dict] | None:
    lines = []
    current_path = arguments.get("file_path", arguments.get("path", arguments.get("file"))) if tool == "read" else None
    recognized = tool in {"find", "initial_instructions"}
    # A byte cap may cut a line exactly at an apparently valid source prefix.
    marker = "\n[host output truncated]"
    if marker in text:
        text = text.split(marker, 1)[0]
        partial = True
    if partial and not text.endswith("\n"):
        text = text.rsplit("\n", 1)[0] if "\n" in text else ""
    for line in text.splitlines():
        match = re.match(r"^(.+\.(?:tsx?|jsx?|go|rs|py|json|yaml|yml|toml|md|html|css|sql))(?::|-)(\d+)(?::|-)(.*)$", line)
        if match:
            lines.append({"path": match[1].removeprefix("./"), "line": int(match[2]), "text": match[3]})
            recognized = True
            continue
        match = re.match(r"^\s*(\d+)\s*[→│|](.*)$", line)
        if match and current_path:
            lines.append({"path": current_path.removeprefix("./"), "line": int(match[1]), "text": match[2]})
            recognized = True
    # Unknown search/overview formats remain unobservable; never infer evidence from a filename.
    return lines if recognized or text.strip() in {"", "No matches found.", "No matches found"} else None


class Product:
    def __init__(self, binary: str, root: Path, home: Path, stderr: Path):
        home.mkdir(parents=True, exist_ok=True)
        self.stderr = stderr.open("ab")
        self.process = subprocess.Popen([binary, "mcp"], cwd=root, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=self.stderr,
                                        env={**os.environ, "CODEMAP_HOME": str(home)})
        self.request_id = 0
        self.initialize_result = self.rpc("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                "clientInfo": {"name": "codemap-benchmark-v2", "version": SPEC["version"]}})
        self.process.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
        self.process.stdin.flush()

    def rpc(self, method, params):
        self.request_id += 1
        request_id = self.request_id
        self.process.stdin.write((canonical({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}) + "\n").encode())
        self.process.stdin.flush()
        while True:
            raw = self.process.stdout.readline()
            require(bool(raw), "product MCP exited without response")
            message = json.loads(raw)
            if message.get("id") == request_id:
                require("error" not in message, f"product RPC error: {message.get('error')}")
                return message["result"]

    def close(self):
        self.process.terminate()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
        self.stderr.close()


class Relay:
    def __init__(self, settings: dict):
        self.settings = settings
        self.limits = settings.get("limits", SPEC["limits"])
        self.group = settings["group"]
        self.root = Path(settings["source"]).resolve()
        self.log = Path(settings["log"])
        self.count = 0
        self.scope = None
        self.product = (Product(settings["product_binary"], self.root, Path(settings["product_home"]),
                                self.log.with_name("product.stderr.log")) if self.group == "B" else None)

    def list_tools(self):
        if self.product:
            tools = self.product.rpc("tools/list", {})["tools"]
            require({t["name"] for t in tools} == set(SPEC["groups"]["B"]), "product tool inventory drift")
            return tools
        return BASELINE_TOOLS

    def call(self, request_id, params):
        name, arguments = params["name"], params.get("arguments", {})
        if name not in SPEC["groups"][self.group]:
            append_jsonl(self.log, {"event": "boundary_denied", "id": str(request_id), "tool": name, "successful_access": False})
            return text_result("code access denied: tool not allowed", True)
        call_id = str(request_id)
        with self.log.with_name(".navigation.lock").open("a") as gate:
            fcntl.flock(gate, fcntl.LOCK_EX)
            if self.log.with_name("stop.json").exists():
                append_jsonl(self.log, {"event": "stopping_denied", "id": call_id, "tool": name,
                                       "observed_monotonic": time.monotonic()})
                stop = read_json(self.log.with_name("stop.json"))
                message = "execution stopping; new exploration denied"
                if stop.get("usage_drain_seconds"):
                    message += "; collection-only shutdown: make no more tool calls and finish this turn now. Any answer after the cutoff is excluded from evaluation."
                return text_result(message, True)
            if self.count >= self.limits["exploration_calls"]:
                append_jsonl(self.log, {"event": "limit", "reason": "call_limit"})
                return text_result("exploration call limit reached", True)
            self.count += 1
            start = time.monotonic()
            append_jsonl(self.log, {"event": "request", "id": call_id, "tool": name, "arguments": arguments,
                                   "effective_scope": self.scope, "started_monotonic": start,
                                   "metadata": params.get("_meta", {})})
        details = {"result_count": None, "product_truncated": None}
        try:
            for key in ["file_path", "path", "file"]:
                if key in arguments and arguments[key] not in {"", "all", "전체"}:
                    safe_path(self.root, arguments[key], must_exist=name == "read")
            if self.product:
                result = self.product.rpc("tools/call", params)
                if name == "overview" and not result.get("isError"):
                    self.scope = arguments.get("path", self.scope)
                text = content_text(result)
                truncation_marker = bool(re.search(r"(?mi)^\s*(?:\(Results are truncated|\[Showing results|\[.*output truncated|Results are partial)", text))
                details["product_truncated"] = True if truncation_marker else (None if name in {"search", "overview"} else False)
                if name in {"search", "grep"}:
                    if re.search(r"No (?:matches|results|files) (?:found|matched)", text, re.I):
                        details["result_count"] = 0
                    elif source_lines(name, arguments, text):
                        details["result_count"] = len(source_lines(name, arguments, text))
            else:
                result, details = baseline(self.root, name, arguments)
        except (ValueError, OSError, KeyError, TypeError, subprocess.TimeoutExpired) as exc:
            result = text_result(str(exc), True)
            details["product_truncated"] = False
        delivered, truncated = trim_result(result, SPEC["host"]["delivered_output_bytes"])
        append_jsonl(self.log, {"event": "result", "id": call_id, "status": "error" if result.get("isError") else "success",
                               "finished_monotonic": time.monotonic(), "raw_result": result, "relay_result": delivered,
                               "raw_bytes": len(content_text(result).encode()), "relay_bytes": len(content_text(delivered).encode()),
                               "host_truncated": truncated, **details})
        if self.count == self.limits["exploration_calls"]:
            append_jsonl(self.log, {"event": "limit", "reason": "call_limit"})
        return delivered

    def serve(self):
        for line in sys.stdin:
            request = json.loads(line)
            if "id" not in request:
                continue
            try:
                method = request["method"]
                if method == "initialize":
                    result = {"protocolVersion": request.get("params", {}).get("protocolVersion", "2024-11-05"),
                              "capabilities": {"tools": {}}, "serverInfo": {"name": "navigation", "version": SPEC["version"]}}
                    if self.product and self.product.initialize_result.get("instructions"):
                        result["instructions"] = self.product.initialize_result["instructions"]
                elif method == "tools/list":
                    result = {"tools": self.list_tools()}
                    append_jsonl(self.log, {"event": "inventory", "tools": result["tools"]})
                elif method == "tools/call":
                    result = self.call(request["id"], request["params"])
                elif method == "ping":
                    result = {}
                elif method in {"resources/list", "resources/templates/list", "prompts/list"}:
                    result = {method.split("/")[0] if "templates" not in method else "resourceTemplates": []}
                else:
                    raise ContractError("unsupported MCP method")
                response = {"jsonrpc": "2.0", "id": request["id"], "result": result}
            except (ValueError, KeyError, TypeError) as exc:
                response = {"jsonrpc": "2.0", "id": request["id"], "error": {"code": -32602, "message": str(exc)}}
            print(canonical(response), flush=True)


def main():
    from .core import write_json
    settings = read_json(Path(sys.argv[1]))
    # Own the relay/product/subcommand group even when Codex uses a separate group.
    if os.getpgrp() != os.getpid():
        os.setsid()
    runtime_path = Path(settings["log"]).with_name("relay-runtime.json")
    runtime = {"pid": os.getpid(), "pgid": os.getpgrp(), "started_monotonic": time.monotonic(), "closed": False}
    write_json(runtime_path, runtime, exclusive=True)
    def terminate(signum, frame):
        raise SystemExit(128 + signum)
    signal.signal(signal.SIGTERM, terminate)
    relay = None
    try:
        relay = Relay(settings)
        relay.serve()
    finally:
        if relay and relay.product:
            relay.product.close()
        write_json(runtime_path, {**runtime, "closed": True, "closed_monotonic": time.monotonic()})


if __name__ == "__main__":
    main()
