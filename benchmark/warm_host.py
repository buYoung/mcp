"""Harness-owned, single-use warm MCP session. The product never restarts at attach."""
from __future__ import annotations

import copy
import json
import os
import queue
import secrets
import signal
import socket
import subprocess
import sys
import threading
import time
import urllib.request
from pathlib import Path

from . import readiness, settings
from .core import append_jsonl, canonical, digest, file_digest, read_json, require, write_json
from .route_transport import NativeRelay, RPCError, auxiliary_call

CONTENT_METHODS = {"tools/call", "resources/list", "resources/templates/list", "resources/read", "prompts/list", "prompts/get"}


def query_configuration(config: dict, inventory: list[dict], ident: str, log: Path) -> dict:
    exclusions = {name: "administration" for name in ("index_repository", "delete_project", "manage_adr", "ingest_traces")}
    exclusions.update({name: "external GitHub PR data outside the fixed corpus" for name in ("list_prs", "get_pr_impact", "triage_prs")})
    if ident == "serena":
        exclusions.update({t["name"]: "native editing or shell operation" for t in inventory
                           if t.get("annotations", {}).get("readOnlyHint") is False})
        exclusions.update({name: "project control or memory outside source retrieval" for name in (
            "activate_project", "remove_project", "switch_modes", "onboarding", "read_memory", "list_memories",
            "query_project", "list_queryable_projects", "open_dashboard")})
    names = [t["name"] for t in inventory if t["name"] not in exclusions]
    if ident == "serena":
        require(not {"execute_shell_command", "create_text_file", "replace_symbol_body", "activate_project", "switch_modes",
                     "write_memory", "serena_repl"} & set(names), "Serena query-only surface changed")
    auxiliaries = ["read"] if ident == "graphify" else []
    necessity = [{"operation": "inspect original source body at a discovered file/line location",
                  "reason": "Native graph tools return metadata and locations, not raw source bodies.",
                  "native_inventory_sha256": digest(inventory), "probe_log": str(log), "added": "read", "quality_based": False}] if auxiliaries else []
    return {**config, "query_tools": names, "auxiliaries": auxiliaries, "auxiliary_necessity": necessity,
            "enabled_tools": [*names, *auxiliaries],
            "excluded_tools": [{"name": t["name"], "reason": exclusions[t["name"]]} for t in inventory if t["name"] in exclusions]}


class WarmSession:
    def __init__(self, state: dict, slot_dir: Path, expected_client: dict, cancel_event, process_registry):
        self.state, self.folder = copy.deepcopy(state), Path(slot_dir).resolve()
        self.folder.mkdir(parents=True, exist_ok=True)
        self.backend = expected_client["backend"]
        require(self.backend in {"codex", "claude_code", "pi"}, "unknown warm client backend")
        self.cancel_event, self.registry = cancel_event, process_registry
        self.ident, self.generation = state["id"], secrets.token_hex(8)
        self.relay = self.listener = self.connection = self.socket_dir = None
        self.receipt = self.cleanup = self.profile = None
        self.started = time.monotonic()
        self.deadline = None
        self.solver_start = None
        self.errors, self.groups = [], []
        self.closing, self.closed = threading.Event(), threading.Event()
        self.rpc_lock, self.close_lock = threading.Lock(), threading.Lock()
        self.threads = []
        self.attached = False
        self.session_reset = {}
        self.warm_log = self.folder / "warmup.jsonl"
        self.relay_settings = state.get("relay_settings")

    def _register_groups(self):
        if not self.relay:
            return
        for process in (self.relay.product.process, self.relay.product.http_server):
            if process and process.pid not in {g["pid"] for g in self.groups}:
                require(process.poll() is None and os.getpgid(process.pid) == process.pid, "product group is not live/owned")
                item = self.registry.register(process.pid, process.pid, f"warm-product:{self.ident}")
                self.groups.append(item)
        self._runtime()

    def _runtime(self, *, closed=False):
        write_json(self.folder / "relay-runtime.json", {"transport": "warm-host", "pid": os.getpid(), "pgid": os.getpgrp(),
            "generation": self.generation, "closed": closed, "owned_process_groups": [g["pgid"] for g in self.groups],
            "cleanup": self.cleanup})

    def _thread(self, target, name):
        thread = threading.Thread(target=target, name=name, daemon=True)
        self.threads.append(thread)
        thread.start()

    def _watch(self):
        while not self.closing.wait(.05):
            if self.cancel_event.is_set():
                self.close("cancelled")
                return
            if self.receipt is None and self.deadline is not None and time.monotonic() >= self.deadline:
                self.errors.append("warm-up deadline exceeded")
                self.close("warmup_timeout")
                return
            if self.relay:
                for process in (self.relay.product.process, self.relay.product.http_server):
                    if process and process.poll() is not None:
                        self.errors.append("live product exited")
                        self.close("product_exited")
                        return

    def _remaining(self):
        require(not self.closing.is_set() and not self.cancel_event.is_set(), "warm session closed/cancelled")
        remaining = self.deadline - time.monotonic()
        require(remaining > 0, "warm-up deadline exceeded")
        self.relay.product.timeout_seconds = min(300, remaining)

    def warm_call(self, name: str, arguments: dict) -> dict:
        """Same argument boundary as solver calls, but no solver admission/counter/stop files."""
        with self.rpc_lock:
            self._remaining()
            params = {"name": name, "arguments": arguments}
            started = time.monotonic()
            try:
                self.relay.validate_request("tools/call", params)
                result = (auxiliary_call(self.relay.root, name, arguments, self.relay.config.get("rg_binary"))
                          if name in self.relay.config.get("auxiliaries", []) else self.relay.product.rpc("tools/call", params))
                append_jsonl(self.warm_log, {"event": "result", "tool": name, "arguments": arguments,
                    "started_monotonic": started, "finished_monotonic": time.monotonic(), "result": result})
                return result
            except Exception as exc:
                append_jsonl(self.warm_log, {"event": "error", "tool": name, "arguments": arguments, "error": str(exc),
                    "started_monotonic": started, "finished_monotonic": time.monotonic()})
                raise

    def _delete_http_session(self):
        product = self.relay.product
        require(product.session_id, "warm HTTP session missing")
        old = product.session_id
        request = urllib.request.Request(product.config["url"], method="DELETE", headers={
            "Mcp-Session-Id": old, "MCP-Protocol-Version": product.protocol})
        with urllib.request.urlopen(request, timeout=min(10, max(.01, self.deadline - time.monotonic()))) as response:
            require(response.status == 200, "warm HTTP session DELETE failed")
        product.session_id, product.initialized = None, False
        self.session_reset = {"deleted_session_id": old, "delete_status": 200, "server_pid": product.http_server.pid}
        append_jsonl(self.warm_log, {"event": "session_reset", **self.session_reset})

    def await_ready(self, deadline: float) -> dict:
        require(self.receipt is None and self.deadline is None, "warm readiness is single use")
        self.deadline = deadline
        try:
            require(not self.cancel_event.is_set() and time.monotonic() < deadline, "warm-up cancelled/expired")
            from .tool_artifacts import load_artifacts
            artifacts = load_artifacts(verify=False)
            artifact = artifacts[self.ident]
            offline = readiness.prepared_evidence(self.state, artifact)
            if self.state.get("campaign_binding"):
                require(self.state.get("prepared_index_ready", {}).get("ready") is True
                        and self.state.get("warmup_policy") == readiness.POLICY, "prepared semantic gate/policy missing or changed")
            before = readiness.load_snapshot()
            if self.ident == "rg":
                native = self.state["native_agent"]
                require(native["search_product_mcp"] is False and file_digest(Path(native["rg_binary"])) == native["rg_sha256"], "native rg binding changed")
                gate = {"not_applicable": True, "assertions": ["pinned rg and prepared native permission boundary; no MCP"],
                        "first_usable_at": None, "stable_ready_at": time.monotonic()}
                initialized_seconds = 0
            else:
                from . import isolation
                from .tool_indexes import probe_configuration
                tmp = Path(self.state["home"]) / "tmp" / self.generation
                tmp.mkdir(parents=True, mode=0o700)
                tmp.chmod(0o700)
                self.profile = isolation.product_profile(self.ident, self.state, artifact, slot_tmp=tmp)
                write_json(self.folder / "isolation-profile.json", self.profile)
                (self.folder / "product.sb").write_bytes(Path(self.profile["profile_path"]).read_bytes())
                config = copy.deepcopy(self.relay_settings or probe_configuration(self.ident, self.state, artifacts))
                config["client"]["isolation_profile"] = self.profile
                config["log"] = str(self.warm_log)
                require(not self.cancel_event.is_set() and time.monotonic() < deadline, "warm-up cancelled/expired")
                config.pop("runtime_path", None)  # Host shares the campaign PID; never treat it as a child PGID.
                with self.rpc_lock:
                    self.relay = NativeRelay(config)
                    self.relay.product.log = self.folder / "warm-native.jsonl"
                    self._register_groups()
                self._thread(self._watch, "warm-watch")
                with self.rpc_lock:
                    self._remaining()
                    start = time.monotonic()
                    self.initialized = self.relay.initialize({"protocolVersion": "2025-03-26", "capabilities": {},
                        "clientInfo": {"name": "codex-mcp-client" if self.backend == "codex" else "claude-code" if self.backend == "claude_code" else "pi", "version": "preparation-probe"}})
                    initialized_seconds = time.monotonic() - start
                    self._register_groups()
                    self.inventory = self.relay.product.list_tools()
                    if self.state.get("native_inventory"):
                        prepared = read_json(Path(self.state["native_inventory"]))
                        require(self.inventory == prepared["tools"], "warm native inventory differs from prepared inventory")
                        # The solver prompt carries the prepared server instructions; the live product must match.
                        if "initialize" in prepared:
                            require((self.initialized.get("instructions") or "") == (prepared["initialize"].get("instructions") or ""),
                                    "warm server instructions differ from prepared instructions")
                    config = query_configuration(config, self.inventory, self.ident, self.warm_log)
                    if self.relay_settings:
                        require(config["enabled_tools"] == self.relay_settings["enabled_tools"], "prepared query surface changed")
                    self.relay.config = config
                    self.relay.list_tools()
                    write_json(self.folder / "inventory.json", {"initialize": self.initialized, "tools": self.inventory})
                gate = readiness.await_ready(self.state, self.warm_call, deadline, self.cancel_event, self.warm_log)
                with self.rpc_lock:
                    if self.ident == "zvec-grep":
                        self._delete_http_session()
                    self.relay.product.timeout_seconds = 300
                self.relay_settings = {**config, "client": {k: v for k, v in config["client"].items() if k != "isolation_profile"},
                                       "log": "<per-run directory>/relay.jsonl"}
                self._listen()
            require(not self.closing.is_set() and not self.cancel_event.is_set() and time.monotonic() < deadline, "warm readiness cancelled/expired")
            ready = time.monotonic()
            self.receipt = {"ready": True, "kind": "live_query_ready", "binding": self.state.get("campaign_binding"),
                "generation": self.generation, "ready_at": ready, "started_at": self.started,
                "assertions": gate, "offline_index": offline, "source": self.state["source"], "home": self.state["home"],
                "artifact_sha256": self.state["artifact_sha256"], "source_sha256": self.state["source_sha256"],
                "baseline_sha256": digest(self.state.get("baseline")), "inventory_sha256": digest(getattr(self, "inventory", [])),
                "profile_sha256": self.profile["profile_sha256"] if self.profile else None,
                "strategy": readiness.POLICY["session_strategy"][self.ident], "session_reset": self.session_reset,
                "owned_process_groups": self.groups, "load_before": before, "load_ready": readiness.load_snapshot(),
                "timings": {"index_build_seconds": offline["index_build_seconds"], "initialize_seconds": initialized_seconds,
                    "restore_seconds": (read_json(self.folder / "restore.json").get("elapsed_seconds")
                                        if (self.folder / "restore.json").exists() else None),
                    "first_usable_query_seconds": gate["first_usable_at"] - self.started if gate["first_usable_at"] else None,
                    "stable_ready_seconds": gate["stable_ready_at"] - self.started, "ready_seconds": ready - self.started,
                    "attach_seconds": None, "cleanup_seconds": None},
                "warm_log_sha256": file_digest(self.warm_log) if self.warm_log.exists() else None}
            write_json(self.folder / "live-ready.json", self.receipt)
            return copy.deepcopy(self.receipt)
        except Exception:
            self.close("warmup_failed")
            raise

    def verify_preparation(self) -> dict:
        """Extra body/session tests are disposable; never run them on a solver hand-off."""
        require(self.solver_start is None and self.receipt is not None, "preparation only")
        params = {"protocolVersion": "2025-03-26", "capabilities": {},
                  "clientInfo": {"name": "pi" if self.backend == "pi" else "claude-code" if self.backend == "claude_code" else "codex-mcp-client", "version": "preparation-probe"}}
        observations = []
        if self.ident == "zvec-grep":
            with self.rpc_lock:
                self._remaining()
                self.relay.initialize(params)
                require(self.relay.product.session_id != self.session_reset["deleted_session_id"], "zvec session reused")
                require(self.relay.product.http_server.pid == self.session_reset["server_pid"], "zvec server restarted")
        for name, arguments in readiness.probes(self.state, preparation=True):
            result = self.warm_call(name, arguments)
            readiness.assert_ready(self.ident, name, result, self.state)
            observations.append({"tool": name, "arguments": arguments, "result_sha256": digest(result)})
        if self.ident in {"codemap-search", "codegraph"}:
            with self.rpc_lock:
                self._remaining()
                self.relay.initialize(params)
            name, arguments = (("read", {"file_path": readiness.TS_FILE}) if self.ident == "codemap-search"
                               else readiness.probes(self.state)[0])
            first = self.warm_call(name, arguments)
            repeat = self.warm_call(name, arguments)
            with self.rpc_lock:
                self._remaining()
                self.relay.initialize(params)
            after = self.warm_call(name, arguments)
            require(first == after, "initialize did not preserve native first-response semantics")
            if self.ident == "codemap-search":
                require("<--removed duplicated-->" not in readiness.text(first)
                        and "<--removed duplicated-->" in readiness.text(repeat), "codemap delivery dedup reset not proven")
            else:
                require(first == repeat, "CodeGraph default response equivalence failed")
            observations.append({"session_equivalence": True, "first_sha256": digest(first), "after_sha256": digest(after)})
        # MCP clients list advertised resources/prompts on connect; a failing listing would void every slot.
        capabilities = self.initialized.get("capabilities", {})
        for capability, method in (("resources", "resources/list"), ("prompts", "prompts/list")):
            if capability in capabilities:
                with self.rpc_lock:
                    self._remaining()
                    listed = self.relay.product.rpc(method, {})
                require(isinstance(listed, dict) and isinstance(listed.get(capability), list), f"{method} discovery failed")
                observations.append({"discovery": method, "items": len(listed[capability]), "result_sha256": digest(listed)})
        from . import isolation
        canaries = isolation.boundary_canaries(self.ident, self.warm_call, self.state)
        if self.ident == "graphify":
            result = self.warm_call("read", {"file_path": "README.md"})
            require(result["content"][0]["text"] == (Path(self.state["source"]) / "README.md").read_text(), "auxiliary source changed")
        return {"observations": observations, "boundary_canaries": canaries}

    def _listen(self):
        root = settings.CACHE / "w"
        root.mkdir(parents=True, exist_ok=True)
        self.socket_dir = root / self.generation
        self.socket_dir.mkdir(mode=0o700)
        self.socket_path = self.socket_dir / "s"
        require(len(os.fsencode(self.socket_path)) < 104, "warm socket path too long; choose a shorter CODEMAP_BENCH_CACHE")
        self.listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.listener.bind(str(self.socket_path))
        self.socket_path.chmod(0o600)
        self.listener.listen(1)
        self.listener.settimeout(.1)
        self._thread(self._serve, "warm-socket")

    def attachment(self) -> dict:
        require(self.receipt is not None and not self.closing.is_set() and not self.attached, "no live unused warm attachment")
        require(not self.health()["condition_errors"], "warm session unhealthy")
        if self.ident == "rg":
            return {"native_agent": self.state["native_agent"], "generation": self.generation}
        return {"command": sys.executable, "args": ["-m", "benchmark.stdio_shim", str(self.socket_path)],
                "cwd": str(settings.REPOSITORY), "env": {"PYTHONPATH": str(settings.REPOSITORY), "PYTHONDONTWRITEBYTECODE": "1",
                    "CODEMAP_BENCH_CACHE": str(settings.CACHE)}, "required": True,
                "startup_timeout_sec": 120, "tool_timeout_sec": 300, "enabled_tools": self.relay_settings["enabled_tools"],
                "instructions": self.initialized.get("instructions") or ""}

    def open_solver_gate(self, start: float):
        require(self.receipt is not None and self.receipt["ready_at"] <= start <= time.monotonic(), "solver clock precedes ready")
        require(self.solver_start is None and not self.health()["condition_errors"] and not self.closing.is_set(), "warm gate unavailable")
        require(not self.cancel_event.is_set(), "solver cancelled before launch")
        with self.rpc_lock:
            if self.relay:
                require(self.relay.count == self.relay.attempts == self.relay.dispatched == 0, "warm traffic entered solver budget")
                self.relay.log = self.folder / "relay.jsonl"
                self.relay.product.log = self.folder / "native.jsonl"
                self.relay.product.timeout_seconds = 300
            self.solver_start = start
        write_json(self.folder / "solver-gate.json", {"start": start, "ready_at": self.receipt["ready_at"], "generation": self.generation})

    def register_solver(self, process):
        self.registry.register(process.pid, process.pid, "solver")

    def _serve(self):
        try:
            while not self.closing.is_set():
                try:
                    conn, _ = self.listener.accept()
                    break
                except socket.timeout:
                    continue
            else:
                return
            self.connection, self.attached = conn, True
            self.listener.close()
            self.socket_path.unlink(missing_ok=True)  # No reconnect, including after EOF.
            require(self.solver_start is not None, "attach before solver gate")
            attached_at = time.monotonic()
            write_json(self.folder / "attachment.json", {"generation": self.generation, "attached_at": attached_at,
                "attach_seconds": attached_at - self.solver_start, "ready_at": self.receipt["ready_at"]})
            incoming = queue.Queue()
            cancelled, request_states = set(), {}
            cancellation_lock = threading.Lock()
            def receive():
                try:
                    with conn.makefile("rb") as stream:
                        for line in stream:
                            message = json.loads(line)
                            if message.get("method") == "notifications/cancelled":
                                ident = message.get("params", {}).get("requestId")
                                require(type(ident) in (str, int), "invalid cancellation requestId")
                                with cancellation_lock:
                                    cancelled.add(ident)
                                    append_jsonl(self.relay.log, {"event": "client_cancelled", "client_request_id": str(ident),
                                        "request_state": request_states.get(ident, "queued"), "monotonic": time.monotonic()})
                                continue
                            incoming.put(line)
                except (OSError, ValueError) as exc:
                    if not self.closing.is_set():
                        self.errors.append(str(exc))
                finally:
                    incoming.put(None)
                    if not self.closing.is_set():
                        self.close("disconnect")
            self._thread(receive, "warm-reader")
            initialized = False
            while not self.closing.is_set():
                try:
                    line = incoming.get(timeout=.1)
                except queue.Empty:
                    continue
                if line is None or self.closing.is_set():
                    break
                request = json.loads(line)
                if "id" not in request:
                    if request.get("method") == "notifications/initialized":
                        continue  # MCPClient.initialize already forwards exactly one initialized notification.
                    continue
                with cancellation_lock:
                    if request["id"] in cancelled:
                        # Admission has not happened: no attempt or budget consumption to invent.
                        request_states[request["id"]] = "skipped"
                        append_jsonl(self.relay.log, {"event": "cancelled_before_dispatch", "client_request_id": str(request["id"]),
                            "dispatched": False, "budget_consumed": False, "monotonic": time.monotonic()})
                        continue
                    request_states[request["id"]] = "in_flight"
                try:
                    method, params = request["method"], request.get("params", {})
                    with self.rpc_lock:
                        if self.closing.is_set():
                            break
                        if method == "initialize":
                            require(not initialized, "solver initialize is single use")
                            info = params.get("clientInfo", {})
                            expected = "pi" if self.backend == "pi" else "claude" if self.backend == "claude_code" else "codex"
                            require(expected in info.get("name", "").lower(), "unexpected MCP client identity")
                            result = self.relay.initialize(params)  # Actual clientInfo/capabilities/protocol, never cached.
                            if (result.get("instructions") or "") != (self.initialized.get("instructions") or ""):
                                self.errors.append("live server instructions differ from prepared instructions")
                                raise ValueError(self.errors[-1])
                            if self.ident == "zvec-grep":
                                product = self.relay.product
                                require(product.session_id and product.session_id != self.session_reset["deleted_session_id"]
                                        and product.http_server.pid == self.session_reset["server_pid"], "zvec solver session not fresh")
                                append_jsonl(self.relay.log, {"event": "fresh_http_session", "session_id": product.session_id,
                                    "server_pid": product.http_server.pid, "old_session_deleted": True})
                            initialized = True
                        else:
                            require(initialized, "initialize required")
                            if method == "tools/list":
                                result = {"tools": self.relay.list_tools()}
                            elif method in CONTENT_METHODS:
                                tool = params.get("name") if method == "tools/call" else None
                                result = self.relay.content_call(request["id"], method, params)
                                # Only an explicit index/session notice voids the slot; request errors are product answers.
                                degraded = readiness.index_degradation(self.ident, result, tool=tool)
                                if degraded:
                                    self.errors.append(degraded)
                                elif method == "tools/call" and readiness.tool_error(self.ident, result, tool=tool):
                                    append_jsonl(self.relay.log, {"event": "tool_error_observed", "client_request_id": str(request["id"]),
                                                                  "kind": readiness.tool_error(self.ident, result, tool=tool)})
                            elif method == "ping":
                                result = self.relay.product.rpc("ping", params)
                            else:
                                raise RPCError({"code": -32601, "message": "unsupported method"})
                    response = {"jsonrpc": "2.0", "id": request["id"], "result": result}
                except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as exc:
                    error = exc.error if isinstance(exc, RPCError) else {"code": -32602, "message": str(exc)}
                    if isinstance(exc, RPCError) and method in {"initialize", "tools/list", "ping"}:
                        self.errors.append("native RPC error envelope")
                    if self.closing.is_set():
                        break
                    append_jsonl(self.relay.log, {"event": "protocol_error", "client_request_id": str(request["id"]), "error": error})
                    response = {"jsonrpc": "2.0", "id": request["id"], "error": error}
                    if method == "tools/call":
                        # pi-mcp-adapter closes the server connection after any tool-call protocol error, and this
                        # single-use attachment cannot reconnect. Relay rejections and native JSON-RPC errors reach
                        # the client as an error tool result with the same message; the original stays in the log.
                        message = error.get("message") if isinstance(error, dict) else str(error)
                        response = {"jsonrpc": "2.0", "id": request["id"], "result": {
                            "content": [{"type": "text", "text": str(message)}], "isError": True}}
                        append_jsonl(self.relay.log, {"event": "error_as_tool_result", "client_request_id": str(request["id"]),
                                                      "native": isinstance(exc, RPCError), "error": error})
                with cancellation_lock:
                    request_states[request["id"]] = "finished"
                    if request["id"] in cancelled:
                        # Finish native RPC/logging normally; only delivery is cancelled.
                        append_jsonl(self.relay.log, {"event": "cancelled_response_discarded", "client_request_id": str(request["id"]),
                            "monotonic": time.monotonic()})
                    else:
                        conn.sendall((canonical(response) + "\n").encode())
                if self.errors:
                    break
        except Exception as exc:
            if not self.closing.is_set():
                self.errors.append(str(exc))
        finally:
            if not self.closing.is_set():
                self.close("transport_closed")

    def health(self) -> dict:
        if self.relay and not self.closing.is_set():
            for process in (self.relay.product.process, self.relay.product.http_server):
                if process and process.poll() is not None and "live product exited" not in self.errors:
                    self.errors.append("live product exited")
        return {"condition_errors": list(self.errors), "ready": self.receipt is not None,
                "closed": self.closed.is_set(), "attached": self.attached, "generation": self.generation}

    def close(self, reason: str) -> dict:
        with self.close_lock:
            if self.closed.is_set():
                return copy.deepcopy(self.cleanup)
            owner = not self.closing.is_set()
            self.closing.set()
        if not owner:
            # Background workers must not wait for the closer that is joining them.
            if threading.current_thread() in self.threads:
                return {"complete": False, "pending": True}
            require(self.closed.wait(65), "warm cleanup did not finish")
            return copy.deepcopy(self.cleanup)
        started = time.monotonic()
        for sock in (self.connection, self.listener):
            if sock:
                try:
                    sock.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass
                sock.close()
        # Failed initialization may have timed out locally while the server is still blocked.
        if self.relay and reason in {"warmup_failed", "warmup_timeout", "cancelled", "product_exited"}:
            for process in (self.relay.product.process, self.relay.product.http_server):
                if process and process.poll() is None:
                    try:
                        os.killpg(process.pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
        # Interrupt a blocked product RPC before joining its IO lock. Never signal the harness group.
        acquired = self.rpc_lock.acquire(timeout=.2)
        if not acquired and self.relay:
            for process in (self.relay.product.process, self.relay.product.http_server):
                if process:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
            acquired = self.rpc_lock.acquire(timeout=12)
        native = {"complete": acquired}
        try:
            if acquired and self.relay:
                native = self.relay.product.close()
        except Exception as exc:
            native = {"complete": False, "error": str(exc)}
        finally:
            if acquired:
                self.rpc_lock.release()
        current = threading.current_thread()
        for thread in self.threads:
            if thread is not current:
                thread.join(timeout=2)
        pending = [thread.name for thread in self.threads if thread is not current and thread.is_alive()]
        if self.socket_dir:
            (self.socket_dir / "s").unlink(missing_ok=True)
            self.socket_dir.rmdir()
        self.cleanup = {"complete": native.get("complete", False) and not pending, "reason": reason, "frontend": native,
                        "pending_threads": pending, "cleanup_seconds": time.monotonic() - started, "generation": self.generation,
                        "owned_process_groups": self.groups, "load_after": readiness.load_snapshot()}
        if self.relay:
            append_jsonl(self.relay.log, {"event": "cleanup", **self.cleanup})
        write_json(self.folder / "warm-cleanup.json", self.cleanup)
        # Publish closed only after the last writer; the campaign may now seal raw logs.
        self._runtime(closed=True)
        self.closed.set()
        return copy.deepcopy(self.cleanup)


def prepare_session(state: dict, slot_dir: Path, expected_client: dict, cancel_event, process_registry) -> WarmSession:
    return WarmSession(state, slot_dir, expected_client, cancel_event, process_registry)
