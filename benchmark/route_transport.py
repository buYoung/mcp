"""Native MCP/rg transport for route preparation and a future isolated solver host."""
from __future__ import annotations

import json
import os
import selectors
import signal
import shutil
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

from . import route_contract as routes
from .tool_artifacts import clean_environment
from .core import ContractError, append_jsonl, canonical, command, digest, file_digest, read_json, read_jsonl, require, write_json
from .responses import text_result, tool_schema


class RPCError(ContractError):
    def __init__(self, error: dict):
        self.error = error
        super().__init__(str(error))


class MCPClient:
    """One task-owned client, preserving native JSON results without output caps."""
    def __init__(self, config: dict, root: Path, log: Path):
        self.config, self.root, self.log = config, root.resolve(), log
        self.counter = 0
        self.initialized = False
        self.session_id = None
        self.protocol = "2025-03-26"
        self.process = None
        self.http_server = None
        self.http_server_log = None
        self.buffer = b""
        self.stderr = None
        self.selector = selectors.DefaultSelector()
        self.timeout_seconds = config.get("timeout_seconds", 120)
        log.parent.mkdir(parents=True, exist_ok=True)
        if config["transport"] == "stdio":
            self.stderr = log.with_suffix(".stderr.log").open("ab")
            self.process = subprocess.Popen(config["launch"], cwd=self.root, stdin=subprocess.PIPE,
                                            stdout=subprocess.PIPE, stderr=self.stderr,
                                            env=clean_environment(config.get("env")), start_new_session=True)
            self.selector.register(self.process.stdout, selectors.EVENT_READ)
        else:
            url = urllib.parse.urlparse(config["url"])
            require(config["transport"] == "http" and url.scheme == "http"
                    and url.hostname in {"127.0.0.1", "localhost", "::1"}, "작업용 loopback MCP만 허용됩니다")
            if config.get("server_launch"):
                try:
                    connection = socket.create_connection((url.hostname, url.port or 80), timeout=0.2)
                except ConnectionRefusedError:
                    pass
                else:
                    connection.close()
                    raise ContractError("작업용 MCP 포트가 이미 사용 중입니다")

    def start_http_server(self) -> None:
        if self.config["transport"] != "http" or not self.config.get("server_launch") or self.http_server:
            return
        url = urllib.parse.urlparse(self.config["url"])
        self.http_server_log = self.log.with_suffix(".server.log").open("ab")
        try:
            self.http_server = subprocess.Popen(self.config["server_launch"], cwd=self.root,
                                                env=clean_environment(self.config.get("env")), stdout=self.http_server_log,
                                                stderr=subprocess.STDOUT, start_new_session=True)
        except OSError:
            self.http_server_log.close()
            raise
        deadline = time.monotonic() + self.timeout_seconds
        while True:
            require(self.http_server.poll() is None and time.monotonic() < deadline, "작업용 MCP HTTP 서버 시작 실패")
            try:
                with socket.create_connection((url.hostname, url.port or 80), timeout=0.2):
                    return
            except OSError:
                time.sleep(0.1)

    def _stdio_message(self, deadline: float) -> dict:
        while b"\n" not in self.buffer:
            remaining = deadline - time.monotonic()
            require(remaining > 0 and self.selector.select(remaining), "MCP 응답 시간 초과")
            chunk = os.read(self.process.stdout.fileno(), 65536)
            require(bool(chunk), "MCP 서버가 응답 없이 종료했습니다")
            self.buffer += chunk
        line, self.buffer = self.buffer.split(b"\n", 1)
        return json.loads(line)

    def _send(self, message: dict) -> None:
        self.process.stdin.write((canonical(message) + "\n").encode())
        self.process.stdin.flush()

    def _server_request(self, message: dict) -> None:
        if message["method"] == "roots/list":
            response = {"result": {"roots": [{"uri": self.root.as_uri(), "name": "source"}]}}
        else:
            response = {"error": {"code": -32601, "message": "This local benchmark host does not provide sampling or external services."}}
        self._send({"jsonrpc": "2.0", "id": message["id"], **response})

    def _http(self, message: dict) -> dict | None:
        headers = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream"}
        if self.session_id:
            headers["Mcp-Session-Id"] = self.session_id
            headers["MCP-Protocol-Version"] = self.protocol
        request = urllib.request.Request(self.config["url"], data=canonical(message).encode(), headers=headers)
        with urllib.request.urlopen(request, timeout=self.timeout_seconds) as response:
            self.session_id = response.headers.get("Mcp-Session-Id", self.session_id)
            if response.status == 202 or "id" not in message:
                return None
            if "text/event-stream" not in response.headers.get("Content-Type", ""):
                return json.load(response)
            data = []
            for raw in response:
                line = raw.decode("utf-8").rstrip("\r\n")
                if line.startswith("data:"):
                    data.append(line[5:].lstrip())
                elif not line and data:
                    event = json.loads("\n".join(data))
                    data = []
                    if event.get("id") == message["id"] and "method" not in event:
                        return event
            raise ContractError("MCP HTTP stream에 요청 응답이 없습니다")

    def rpc(self, method: str, params: dict, *, notification=False):
        self.counter += 1
        message = {"jsonrpc": "2.0", "method": method, "params": params}
        if not notification:
            message["id"] = self.counter
        append_jsonl(self.log, {"direction": "request", "message": message})
        if self.process:
            self._send(message)
            if notification:
                return None
            deadline = time.monotonic() + self.timeout_seconds
            while True:
                response = self._stdio_message(deadline)
                append_jsonl(self.log, {"direction": "response", "message": response})
                if "method" in response:
                    if "id" in response:
                        self._server_request(response)
                    continue
                if response.get("id") == message["id"]:
                    break
        else:
            response = self._http(message)
            append_jsonl(self.log, {"direction": "response", "message": response})
            if notification:
                return None
        require(isinstance(response, dict), "MCP 응답 형식 오류")
        if "error" in response:
            raise RPCError(response["error"])
        return response["result"]

    def initialize(self, params: dict) -> dict:
        self.start_http_server()
        result = self.rpc("initialize", params)
        self.protocol = result["protocolVersion"]
        self.rpc("notifications/initialized", {}, notification=True)
        self.initialized = True
        return result

    def list_tools(self) -> list[dict]:
        result, seen = [], set()
        params = {}
        while True:
            page = self.rpc("tools/list", params)
            result.extend(page["tools"])
            cursor = page.get("nextCursor")
            if not cursor:
                return result
            require(cursor not in seen, "MCP tools/list cursor 반복")
            seen.add(cursor)
            params = {"cursor": cursor}  # Discovery pagination only, never search-result pagination.

    def close(self) -> dict:
        result = {"frontend_stopped": True, "shared_daemon_stopped": None}
        if self.process:
            if self.process.poll() is None:
                self.process.stdin.close()
                try:
                    self.process.wait(timeout=20)
                except subprocess.TimeoutExpired:
                    self.process.terminate()
                    try:
                        self.process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        os.killpg(self.process.pid, signal.SIGKILL)
                        self.process.wait(timeout=5)
            result["pid"] = self.process.pid
            result["exit_code"] = self.process.returncode
            self.process.stdin.close()
            self.process.stdout.close()
            self.stderr.close()
        elif self.session_id:
            request = urllib.request.Request(self.config["url"], method="DELETE", headers={
                "Mcp-Session-Id": self.session_id, "MCP-Protocol-Version": self.protocol})
            try:
                with urllib.request.urlopen(request, timeout=10) as response:
                    result["session_delete_status"] = response.status
            except urllib.error.HTTPError as exc:
                result["session_delete_status"] = exc.code
            except OSError as exc:
                result["session_delete_error"] = str(exc)
        if self.http_server:
            if self.http_server.poll() is None:
                self.http_server.terminate()
            try:
                self.http_server.wait(timeout=20)
            except subprocess.TimeoutExpired:
                os.killpg(self.http_server.pid, signal.SIGKILL)
                self.http_server.wait(timeout=5)
            result["http_server"] = {"pid": self.http_server.pid, "stopped": self.http_server.poll() is not None,
                                     "exit_code": self.http_server.returncode}
            self.http_server_log.close()
        if self.initialized and self.config.get("shutdown_command"):
            try:
                stopped = subprocess.run(self.config["shutdown_command"], cwd=self.root,
                                         env=clean_environment(self.config.get("env")), capture_output=True, timeout=10)
                result["daemon"] = {"command": self.config["shutdown_command"], "exit_code": stopped.returncode,
                                    "stdout": stopped.stdout.decode(errors="replace"), "stderr": stopped.stderr.decode(errors="replace"),
                                    "stopped": stopped.returncode == 0}
            except subprocess.TimeoutExpired:
                result["daemon"] = {"stopped": False, "reason": "task-owned daemon stop timed out"}
        groups = []
        for process in (self.process, self.http_server):
            if process is None:
                continue
            def is_alive():
                try:
                    os.killpg(process.pid, 0)
                    return True
                except ProcessLookupError:
                    return False
            for signum in (signal.SIGTERM, signal.SIGKILL):
                if not is_alive():
                    break
                try:
                    os.killpg(process.pid, signum)
                except ProcessLookupError:
                    break
                deadline = time.monotonic() + 2
                while is_alive() and time.monotonic() < deadline:
                    time.sleep(0.05)
            groups.append({"pgid": process.pid, "stopped": not is_alive()})
        result["owned_process_groups"] = groups
        result["complete"] = (all(group["stopped"] for group in groups) and result.get("daemon", {}).get("stopped", True)
                              and result.get("http_server", {}).get("stopped", True))
        self.selector.close()
        return result


def source_path(root: Path, value: str, *, must_exist=True) -> Path:
    require(isinstance(value, str) and "\x00" not in value, "잘못된 source 경로")
    path = Path(value)
    resolved = (path if path.is_absolute() else root / path).resolve()
    require(resolved.is_relative_to(root.resolve()), "고정 source 밖의 경로")
    relative = resolved.relative_to(root.resolve())
    require(not any(p in {".git", ".codemap", ".codegraph", ".codebase-memory", ".zvec-grep", ".codex", "graphify-out"}
                    for p in relative.parts), "제품/실행 메타데이터 접근 금지")
    require(not must_exist or resolved.exists(), "source 경로가 없습니다")
    return resolved


RG_TOOL = tool_schema("rg", "Run the pinned ripgrep CLI in the fixed source. Return its full stdout/stderr and exit code. Output options are opt-in; native defaults apply.", {
    "pattern": {"type": "string"}, "path": {"type": "string"}, "glob": {"type": "string"},
    "case_insensitive": {"type": "boolean"}, "context_lines": {"type": "integer", "minimum": 0},
    "line_number": {"type": "boolean"}, "files": {"type": "boolean"}}, [])
READ_TOOL = tool_schema("read", "Read original UTF-8 source. Omitted limit reads through EOF; explicit ranges are not expanded.", {
    "file_path": {"type": "string"}, "offset": {"type": "integer", "minimum": 1},
    "limit": {"type": "integer", "minimum": 1}}, ["file_path"])


def auxiliary_call(root: Path, name: str, arguments: dict, rg_binary: str | None = None) -> dict:
    schema = RG_TOOL if name == "rg" else READ_TOOL
    require(set(arguments) <= schema["inputSchema"]["properties"].keys(), "지원하지 않는 보조 도구 인자")
    if name == "read":
        path = source_path(root, arguments["file_path"])
        offset, limit = arguments.get("offset", 1), arguments.get("limit")
        require(type(offset) is int and offset > 0 and (limit is None or type(limit) is int and limit > 0), "잘못된 read 범위")
        lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
        selected = lines[offset - 1:None if limit is None else offset - 1 + limit]
        return {**text_result("".join(selected)), "structuredContent": {
            "path": str(path.relative_to(root)), "start_line": offset, "end_line": offset + len(selected) - 1}}
    require(name == "rg" and rg_binary, "준비된 rg 실행 파일이 필요합니다")
    path = source_path(root, arguments.get("path", "."))
    args = [rg_binary]
    for key, flag in (("case_insensitive", "-i"), ("line_number", "-n"), ("files", "--files")):
        if arguments.get(key):
            args.append(flag)
    if arguments.get("glob"):
        args += ["--glob", arguments["glob"]]
    if "context_lines" in arguments:
        require(type(arguments["context_lines"]) is int and arguments["context_lines"] >= 0, "잘못된 context")
        args += ["-C", str(arguments["context_lines"])]
    if not arguments.get("files"):
        require(isinstance(arguments.get("pattern"), str), "rg pattern이 필요합니다")
        args += ["-e", arguments["pattern"]]
    args += ["--", str(path.relative_to(root))]
    result = subprocess.run(args, cwd=root, env=clean_environment(), capture_output=True, timeout=60)
    stdout, stderr = result.stdout.decode("utf-8", errors="replace"), result.stderr.decode("utf-8", errors="replace")
    return {"content": [{"type": "text", "text": stdout}, {"type": "text", "text": stderr}],
            "isError": result.returncode not in {0, 1},
            "structuredContent": {"exit_code": result.returncode}}


class NativeRelay:
    def __init__(self, config: dict):
        self.config = config
        self.root = Path(config["source"]).resolve()
        self.log = Path(config["log"])
        self.log.parent.mkdir(parents=True, exist_ok=True)
        self.product = MCPClient(config["client"], self.root, self.log.with_name("native.jsonl")) if config.get("client") else None
        self.inventory = None
        self.count = 0
        self.record_runtime()

    def record_runtime(self) -> None:
        if self.config.get("runtime_path"):
            write_json(Path(self.config["runtime_path"]), {"pid": os.getpid(), "pgid": os.getpgrp(),
                "transport": "grafana-route", "closed": False,
                "owned_process_groups": [p.pid for p in (self.product.process, self.product.http_server) if p] if self.product else []})

    def initialize(self, params: dict) -> dict:
        result = self.product.initialize(params) if self.product else {
            "protocolVersion": params["protocolVersion"], "capabilities": {"tools": {}},
            "serverInfo": {"name": "rg", "version": "15.2.0"}}
        append_jsonl(self.log, {"event": "initialize", "client": params, "result": result})
        self.record_runtime()
        return result

    def list_tools(self) -> list[dict]:
        if self.inventory is None:
            native = self.product.list_tools() if self.product else [RG_TOOL]
            permitted = self.config.get("query_tools", [t["name"] for t in native])
            require(set(permitted) <= {t["name"] for t in native}, "준비된 native 도구가 없습니다")
            auxiliaries = self.config.get("auxiliaries", [])
            require(not auxiliaries or self.config.get("auxiliary_necessity"), "보조 도구 필요성 기록 없음")
            require(set(auxiliaries) <= {"rg", "read"}, "허용되지 않은 보조 도구")
            self.inventory = [t for t in native if t["name"] in permitted]
            for tool in (RG_TOOL, READ_TOOL):
                if tool["name"] in auxiliaries:
                    require(tool["name"] not in {t["name"] for t in self.inventory}, "native/보조 도구 이름 충돌")
                    self.inventory.append(tool)
            append_jsonl(self.log, {"event": "inventory", "native_tools": native, "tools": self.inventory})
        return self.inventory

    def call(self, ident, params: dict) -> dict:
        name, arguments = params["name"], params.get("arguments", {})
        require(name in {t["name"] for t in self.list_tools()}, "허용되지 않은 탐색 도구")
        require(not self.log.with_name("stop.json").exists(), "실행 중단 이후 탐색 금지")
        limit = self.config.get("limits", routes.load_profile()["limits"])["exploration_calls"]
        if self.count >= limit:
            append_jsonl(self.log, {"event": "limit", "reason": "call_limit"})
            raise ContractError("탐색 호출 한도")
        self.count += 1
        for key in ("path", "file_path", "file", "root", "projectPath", "project_path", "repo_path", "repoPath"):
            if isinstance(arguments.get(key), str) and arguments[key] not in {"", "all", "전체"}:
                source_path(self.root, arguments[key], must_exist=False)
        started = time.monotonic()
        append_jsonl(self.log, {"event": "request", "id": str(ident), "arm": self.config["arm"],
                               "tool": name, "arguments": arguments, "started_monotonic": started,
                               "metadata": params.get("_meta", {}),
                               "is_auxiliary": name in self.config.get("auxiliaries", [])})
        if not self.product or name in self.config.get("auxiliaries", []):
            result = auxiliary_call(self.root, name, arguments, self.config.get("rg_binary"))
        else:
            result = self.product.rpc("tools/call", params)
        append_jsonl(self.log, {"event": "result", "id": str(ident), "arm": self.config["arm"],
                               "tool": name, "raw_result": result, "relay_result": result,
                               "result_sha256": digest(result), "raw_json_bytes": len(canonical(result).encode()),
                               "relay_json_bytes": len(canonical(result).encode()), "host_truncated": False,
                               "native_truncated": None, "client_delivered": None,
                               "finished_monotonic": time.monotonic()})
        if self.count == limit:
            append_jsonl(self.log, {"event": "limit", "reason": "call_limit"})
        return result

    def serve(self):
        for line in sys.stdin:
            request = json.loads(line)
            if "id" not in request:
                continue
            try:
                method, params = request["method"], request.get("params", {})
                if method == "initialize":
                    result = self.initialize(params)
                elif method == "tools/list":
                    result = {"tools": self.list_tools()}
                elif method == "tools/call":
                    result = self.call(request["id"], params)
                elif method == "ping":
                    result = {}
                elif self.product and method in {"resources/list", "resources/templates/list", "resources/read", "prompts/list", "prompts/get"}:
                    result = self.product.rpc(method, params)
                else:
                    raise RPCError({"code": -32601, "message": "unsupported method"})
                response = {"jsonrpc": "2.0", "id": request["id"], "result": result}
            except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as exc:
                error = exc.error if isinstance(exc, RPCError) else {"code": -32602, "message": str(exc)}
                append_jsonl(self.log, {"event": "rpc_error", "id": str(request["id"]), "error": error})
                response = {"jsonrpc": "2.0", "id": request["id"], "error": error}
            print(canonical(response), flush=True)


def publish_delivery() -> dict:
    """A neutral CLI/MCP round-trip, not a solver run or a gold-question trial."""
    from . import settings
    from .tool_artifacts import load_artifacts
    from . import runner
    artifact = load_artifacts(verify=False)["rg"]
    binary = artifact["launch"][0]
    root = routes.PREPARATION / "delivery-probe" / str(time.time_ns())
    root.mkdir(parents=True, exist_ok=True)
    # Keep the existing over-limit transport probe independent of one module's size.
    (root / "sample.py").write_text((settings.ROOT / "runner.py").read_text() + "\n" + Path(__file__).read_text())
    config = {"arm": "rg", "source": str(root), "log": str(root / "relay.jsonl"),
              "query_tools": ["rg"], "auxiliaries": [], "rg_binary": binary}
    write_json(root / "relay-settings.json", config)
    client = MCPClient({"transport": "stdio", "launch": [sys.executable, "-m", "benchmark.route_transport",
                                                          str(root / "relay-settings.json")]},
                       settings.REPOSITORY, root / "protocol.jsonl")
    try:
        initialized = client.initialize({"protocolVersion": "2025-03-26", "capabilities": {},
                                         "clientInfo": {"name": "codex-mcp-client", "version": "preparation-probe"}})
        inventory = client.list_tools()
        result = client.rpc("tools/call", {"name": "rg", "arguments": {"pattern": ".", "path": "sample.py"}})
    finally:
        cleanup = client.close()
    original = subprocess.run([binary, "-e", ".", "--", "sample.py"], cwd=root,
                              env=clean_environment(), capture_output=True, check=True)
    require(result["content"][0]["text"].encode() == original.stdout, "rg MCP 전달 바이트 불일치")
    require(len(original.stdout.splitlines()) > 250 and len(original.stdout) > 32000,
            "기존 두 제한을 초과하는 probe 입력이 필요합니다")
    logged = [row for row in read_jsonl(root / "relay.jsonl") if row.get("event") == "result"][-1]
    require(logged["raw_result"] == logged["relay_result"] == result, "native/relay 결과 필드 손실")
    spec = settings.execution_settings()
    spec.update(native_output_policy=routes.load_profile()["native_output_policy"], preparation_only=True)
    options = runner.codex_options(settings=spec)
    require(not any("tool_output_token_limit=" in arg for arg in options), "route host 출력 크기 override")
    return routes.publish_handoff("02-delivery.json", {
        "status": "verified_offline", "transport_interfaces": {"stdio": "MCPClient", "http": "Streamable HTTP JSON/SSE"},
        "native_output_policy": routes.load_profile()["native_output_policy"],
        "auxiliary_exception_contract": routes.load_profile()["tool_access"],
        "client_identity": "forward the actual initialize clientInfo and capabilities",
        "client_limits": {"codex_version": command(["codex", "--version"]).strip(),
                          "history_limit": None, "history_limit_reason": "Client default has not been measured with a live model.",
                          "harness_override": None, "code_mode_budget": "Native server guidance is forwarded unchanged."},
        "raw_relay_checks": {"status": "passed", "probe": "rg over a copied harness source file",
                             "scope": "neutral relay transport fixture only; not the native-agent baseline configuration",
                             "stdout_bytes": len(original.stdout), "stdout_lines": len(original.stdout.splitlines()),
                             "byte_identical": True, "fields_preserved": True, "inventory": inventory,
                             "initialize": initialized, "cleanup": cleanup,
                             "protocol_log": str(root / "protocol.jsonl"), "relay_log": str(root / "relay.jsonl"),
                             "protocol_sha256": file_digest(root / "protocol.jsonl"),
                             "relay_sha256": file_digest(root / "relay.jsonl"), "probe_source_sha256": file_digest(root / "sample.py")},
        "codex_options": options,
        "unobserved_stages": ["Real solver history and code-mode printing", "Native product startup probes are recorded in 04-indexes.json"],
        "solver_runs": 0, "grader_runs": 0},
        implementation=["benchmark/route_transport.py", "benchmark/runner.py"], dependencies=["01-contract.json"])


def main():
    if sys.argv[1:] == ["inspect"]:
        print(publish_delivery()["status"])
        return
    config = read_json(Path(sys.argv[1]))
    if os.getpgrp() != os.getpid():
        os.setsid()
    runtime_path = Path(config["log"]).with_name("relay-runtime.json")
    config["runtime_path"] = str(runtime_path)
    runtime = {"pid": os.getpid(), "pgid": os.getpgrp(), "transport": "grafana-route", "closed": False}
    write_json(runtime_path, runtime, exclusive=True)
    relay = None
    def terminate(signum, frame):
        raise SystemExit(128 + signum)
    signal.signal(signal.SIGTERM, terminate)
    try:
        relay = NativeRelay(config)
        relay.serve()
    finally:
        cleanup = relay.product.close() if relay and relay.product else {"complete": True}
        if relay:
            append_jsonl(relay.log, {"event": "cleanup", **cleanup})
        write_json(runtime_path, {**read_json(runtime_path), "closed": True, "cleanup": cleanup})


if __name__ == "__main__":
    main()
