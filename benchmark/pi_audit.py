"""Model-free wire audit and post-run checks of harness-owned model context."""
from __future__ import annotations

import ast
import base64
import gzip
import http.server
import inspect
import json
import os
import re
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

from . import pi_client, route_contract as routes, settings
from .core import digest, file_digest, read_json, require, write_json


def dummy_token() -> str:
    encode = lambda value: base64.urlsafe_b64encode(json.dumps(value).encode()).decode().rstrip("=")
    return encode({"alg": "none"}) + "." + encode({"https://api.openai.com/auth": {"chatgpt_account_id": "local-audit"}}) + "."


def strings(value, location=""):
    if isinstance(value, str):
        yield location, value
    elif isinstance(value, dict):
        for key, item in value.items():
            yield from strings(item, location + "/" + key)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            yield from strings(item, location + f"/{index}")


def forbidden_words() -> tuple[str, ...]:
    return (*settings.IDENTIFYING_PATH_WORDS, *routes.QUESTION_IDS, "smoke", "grafana")


def server_summary(inventory: dict) -> str:
    # @buyong/pi-mcp-adapter pi/prompt.ts: one server, first line, at most 250 UTF-16 units.
    instructions = inventory.get("initialize", {}).get("instructions", "").strip()
    first = instructions.split("\n", 1)[0].strip()
    units = first.encode("utf-16-le")
    return first if len(units) <= 500 else units[:498].decode("utf-16-le", errors="surrogatepass").rstrip() + "…"


def product_text(inventory: dict) -> list[str]:
    """Only recorded native text/identifiers, never a blanket product-name exemption."""
    values = [inventory.get("initialize", {}).get("instructions", "")]
    for tool in inventory.get("tools", []):
        values.extend(text for _, text in strings(tool))
        values.extend(pi_client.navigation_tool_names(tool["name"]))
        values.append("mcp__custom-tools__" + pi_client.navigation_tool_name(tool["name"]))
    # Very short schema values (e.g. 'string') are not exemption spans.
    return sorted({text for text in values if len(text) >= 4}, key=len, reverse=True)


def path_violations(text: str, location: str) -> list[dict]:
    from urllib.parse import unquote
    # file URIs and escaped components are still filesystem paths, including inside native text.
    normalized = unquote(text).replace("file://localhost/", "/").replace("file:///", "/")
    errors = []
    for match in re.finditer(r"(?<![\w:/])(?:/[A-Za-z0-9_.~@%-]+)+[^\s\"'<>`]*", normalized):
        path = match.group().rstrip(",;:.)}")
        for component in path.split("/"):
            for word in forbidden_words():
                if word.lower() in component.lower():
                    errors.append({"location": location, "kind": "absolute_path", "word": word, "path": path})
    return errors


def audit_text(value, *, inventory: dict | None = None, product_owned=False) -> list[dict]:
    violations = []
    exemptions = product_text(inventory or {}) if product_owned else []
    words = forbidden_words()
    for location, original in strings(value):
        violations.extend(path_violations(original, location))
        text = original
        # The Claude child may cut a long tool description mid-comment. Exempt only the
        # proven native prefix, not the surrounding generated declaration or other lines.
        if product_owned:
            for match in re.finditer(r"(?m)^\s*// (.{30,}?)… \[truncated\]$", text):
                prefix = match.group(1)
                if any(exempt.startswith(prefix) for exempt in exemptions):
                    text = text.replace(match.group(), "[native prefix]")
        for exempt in exemptions:
            text = text.replace(exempt, "[native]")
            # pi renders parameter descriptions as TypeScript line comments in codemode declarations.
            text = text.replace("\n".join("// " + line for line in exempt.splitlines()), "[native]")
        for word in words:
            if word.lower() in text.lower():
                violations.append({"location": location, "kind": "harness_text", "word": word})
    return violations


def audit_system(value, inventory: dict) -> list[dict]:
    """Native exemptions apply to tool declarations and the exact appended envelope, not preamble/rules/docs/cwd."""
    if isinstance(value, dict) and "sections" in value:
        errors = path_violations(json.dumps(value, ensure_ascii=False), "system")
        errors += audit_text(value.get("toolsAdded", []), inventory=inventory, product_owned=True)
        errors += audit_text(value.get("toolsRemoved", []), inventory=inventory, product_owned=True)
        errors += audit_text({key: item for key, item in value.items()
                              if key not in {"sections", "toolsAdded", "toolsRemoved", "content"}})
        for section, text in value["sections"].items():
            errors += (audit_text(text, inventory=inventory, product_owned=True) if section == "tools"
                       else audit_system(text, inventory))
        errors += audit_system(value.get("content", ""), inventory)
        return errors
    errors = []
    instructions = inventory.get("initialize", {}).get("instructions", "")
    envelope = pi_client.server_instructions_prompt(instructions).strip() if instructions.strip() else None
    for location, original in strings(value):
        errors.extend(path_violations(original, location))
        text = original.replace(envelope, "[native instructions]") if envelope else original
        summary = server_summary(inventory)
        if summary:
            text = text.replace(f"- mcp__{pi_client.NAVIGATION_SERVER} (codemode): {summary}", "[native server summary]")
        for match in re.finditer(r"<tools>.*?</tools>", text, flags=re.DOTALL):
            errors += audit_text(match.group(), inventory=inventory, product_owned=True)
            text = text.replace(match.group(), "[tool declarations]")
        errors += audit_text(text)
    return errors


def audit_request(body: dict, inventory: dict) -> list[dict]:
    errors = path_violations(json.dumps(body, ensure_ascii=False), "request")
    errors += audit_system(body.get("instructions", body.get("system", "")), inventory)
    errors += audit_text(body.get("tools", []), inventory=inventory, product_owned=True)
    for item in body.get("input", body.get("messages", [])):
        if item.get("type") == "additional_tools":
            errors += audit_text(item, inventory=inventory, product_owned=True)
        elif item.get("role") in {"system", "developer"}:
            errors += audit_system(item, inventory)
        else:
            errors += audit_text(item)
    # Transport metadata is not prompt prose, but any absolute path anywhere in the body is audited.
    for key, value in body.items():
        if key not in {"instructions", "system", "tools", "input", "messages"}:
            for location, text in strings(value, key):
                errors.extend(path_violations(text, location))
    return errors


def request_tools(body: dict) -> list[dict]:
    tools = [*body.get("tools", [])]
    for item in body.get("input", []):
        if item.get("type") == "additional_tools":
            tools.extend(item["tools"])
    return tools


def declared_tool_names(body: dict) -> set[str]:
    return {tool["name"].removeprefix("mcp__custom-tools__") for tool in request_tools(body)
            if tool["name"] != "DeferredToolPlaceholder"}


def audit_codemode_catalog(body: dict, allowed: set[str], *, product: bool) -> dict:
    tools = [tool for tool in request_tools(body) if tool["name"].removeprefix("mcp__custom-tools__") == "codemode"]
    text = tools[0].get("description", "") if len(tools) == 1 else ""
    listed = set(re.findall(r"(?m)^### `([A-Za-z0-9_]+)`", text))
    omitted = re.findall(r"(?m)^## [^\n]*\((?:some )?tools not listed\)", text)
    namespace = "## mcp__navigation" in text
    valid = ("Nested tools:" in text and listed <= allowed and (bool(listed) or namespace)
             and (listed == allowed or bool(omitted)) and (not product or namespace))
    return {"passed": valid, "listed_tools": sorted(listed), "inline_count": len(listed),
            "allowed_count": len(allowed), "omission_markers": omitted, "inline_budget": "pi default (3000)"}


def model_error_messages() -> list[str]:
    """Audit all runtime relay/read-guard error paths, including formatted messages and RPC envelopes."""
    from . import isolation, route_transport
    import textwrap
    messages = []
    targets = (isolation.RelayPolicy, isolation.wrap, route_transport.MCPClient, route_transport.NativeRelay,
               route_transport.source_path, route_transport.auxiliary_call)
    for target in targets:
        tree = ast.parse(textwrap.dedent(inspect.getsource(target)))
        module = inspect.getmodule(target)
        def text(node):
            if isinstance(node, ast.Constant) and isinstance(node.value, str):
                return node.value
            if isinstance(node, ast.JoinedStr):
                return "".join(text(part) if not isinstance(part, ast.FormattedValue) else "{}" for part in node.values)
            if isinstance(node, ast.Name) and isinstance(getattr(module, node.id, None), str):
                return getattr(module, node.id)
            if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add):
                return text(node.left) + text(node.right)
            return ""
        for node in ast.walk(tree):
            if isinstance(node, ast.Call):
                name = node.func.id if isinstance(node.func, ast.Name) else ""
                args = node.args[1:] if name == "require" else node.args if name in {"ContractError", "ValueError", "RuntimeError", "print"} else []
                messages.extend(value for arg in args if (value := text(arg)))
            elif isinstance(node, ast.Dict):
                messages.extend(value for key, arg in zip(node.keys, node.values)
                                if isinstance(key, ast.Constant) and key.value in {"message", "text"} and (value := text(arg)))
    # The read-tool guard runs inside pi; these are the only texts it returns.
    return sorted({*messages, pi_client.REJECTION, pi_client.STOPPED})


def static_surfaces(folder: Path, source: str) -> dict:
    """Paths revealed only after truncation/errors must also pass before a model can run."""
    root = pi_client.private_root(folder)
    return {"paths": [source, str(root / "work"), str(root / "home"), str(root / "bin/rg"), str(root / "bin/fd"),
                       str(root / "tmp/pi-mcp-0000000000000000.txt"),
                       str(root / "tmp/pi-codemode-000000.txt")],
            "rejection_messages": model_error_messages()}


def audit_session(folder: Path, inventory: dict) -> dict:
    from .runner import read_jsonl_prefix
    files, violations, system_count, user_count = [], [], 0, 0
    for path in sorted((folder / "pi-session").glob("*.jsonl")):
        rows, state = read_jsonl_prefix(path)
        files.append({"path": str(path), "sha256": file_digest(path)})
        if not state["complete"]:
            violations.append({"kind": "incomplete_session", "path": str(path)})
        for row in rows:
            message = row.get("message", {})
            role = message.get("role")
            if role in {"system", "user"}:
                system_count += role == "system"
                user_count += role == "user"
                violations.extend(audit_system(message, inventory) if role == "system" else audit_text(message))
    if not files or not system_count or not user_count:
        violations.append({"kind": "missing_system_or_user_session"})
    result = {"passed": not violations, "files": files, "system_messages_including_patches": system_count,
              "user_messages": user_count, "violations": violations}
    write_json(folder / "context-audit.json", result)
    return result


class CaptureProvider:
    """Loopback first request. With a script, that request is answered with the scripted read-tool calls
    ([name, arguments] pairs; one script-tool call in codemode) and a second request ends the turn. No hosted transport."""
    def __init__(self, node: str, *, port: int = 0, expected_auth: str | None = None,
                 script: list | None = None, script_mode: str = "direct", forbidden_values: tuple[str, ...] = ()):
        self.requests, self.errors, self.auth_matches = [], [], []
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_GET(self):
                self.send_error(426, "SSE only")

            def do_POST(self):
                try:
                    raw = self.rfile.read(int(self.headers.get("Content-Length", 0)))
                    if self.headers.get("Content-Encoding") == "gzip":
                        raw = gzip.decompress(raw)
                    elif self.headers.get("Content-Encoding") == "zstd":
                        raw = subprocess.run([node, "-e", "process.stdin.on('data',x=>a.push(x));const a=[];process.stdin.on('end',()=>process.stdout.write(require('node:zlib').zstdDecompressSync(Buffer.concat(a))));"],
                                             input=raw, capture_output=True, check=True).stdout
                    body = json.loads(raw)
                    if self.path.split("?", 1)[0].endswith("/count_tokens"):
                        self.send_response(200)
                        self.end_headers()
                        self.wfile.write(b'{"input_tokens":100}')
                        return
                    require(self.path in {"/codex/responses", "/v1/messages", "/v1/messages?beta=true"}, "unexpected local endpoint")
                    if expected_auth is not None:
                        match = self.headers.get("Authorization") == "Bearer " + expected_auth
                        owner.auth_matches.append(match)
                        require(match, "local OAuth authentication header mismatch")
                        require(expected_auth not in json.dumps(body), "credential leaked into request body")
                    require(all(value not in json.dumps(body) for value in forbidden_values), "private credential/path leaked into request body")
                    owner.requests.append(body)
                    require(len(owner.requests) <= (1 if script is None else 2), "unexpected request count in local audit")
                    calls = script if script and len(owner.requests) == 1 else []
                    self.send_response(200)
                    self.send_header("Content-Type", "text/event-stream")
                    self.end_headers()
                    def send(kind, **fields):
                        self.wfile.write(("event: " + kind + "\ndata: " + json.dumps({"type": kind, **fields}) + "\n\n").encode())
                        self.wfile.flush()
                    if self.path == "/codex/responses":
                        send("response.created", response={"id": "resp_local", "status": "in_progress"})
                        items = []
                        if calls and script_mode == "codemode":
                            code = ("for (const [name, args] of " + json.dumps(calls) + ") {\n"
                                    "  try { await tools[name](args); } catch (error) {}\n}\ntext(\"done\");\n")
                            item = {"type": "custom_tool_call", "id": "ctc_local", "call_id": "call_local", "name": "codemode", "input": ""}
                            send("response.output_item.added", output_index=0, item=item)
                            item = {**item, "input": code, "status": "completed"}
                            send("response.custom_tool_call_input.done", output_index=0, item_id=item["id"], input=code)
                            send("response.output_item.done", output_index=0, item=item)
                            items.append(item)
                        for index, (name, arguments) in enumerate(calls if script_mode != "codemode" else []):
                            item = {"type": "function_call", "id": f"fc_{index}", "call_id": f"call_{index}", "name": name, "arguments": ""}
                            send("response.output_item.added", output_index=index, item=item)
                            item = {**item, "arguments": json.dumps(arguments), "status": "completed"}
                            send("response.function_call_arguments.done", output_index=index, item_id=item["id"], arguments=item["arguments"])
                            send("response.output_item.done", output_index=index, item=item)
                            items.append(item)
                        if not calls:
                            item = {"type": "message", "id": "msg_local", "role": "assistant", "content": []}
                            send("response.output_item.added", output_index=0, item=item)
                            send("response.output_text.delta", output_index=0, content_index=0, delta="Done.")
                            item.update(status="completed", content=[{"type": "output_text", "text": "Done.", "annotations": []}])
                            send("response.output_item.done", output_index=0, item=item)
                            items.append(item)
                        send("response.completed", response={"id": "resp_local", "status": "completed", "model": body["model"], "output": items,
                             "usage": {"input_tokens": 100, "output_tokens": 1, "total_tokens": 101}})
                    else:
                        send("message_start", message={"id": "msg_local_" + str(len(owner.requests)), "type": "message", "role": "assistant", "model": body["model"],
                             "content": [], "stop_reason": None, "stop_sequence": None, "usage": {"input_tokens": 100, "output_tokens": 0}})
                        for index, (name, arguments) in enumerate(calls):
                            send("content_block_start", index=index, content_block={"type": "tool_use", "id": f"tool_{index}",
                                 "name": "mcp__custom-tools__" + name, "input": {}})
                            send("content_block_delta", index=index, delta={"type": "input_json_delta", "partial_json": json.dumps(arguments)})
                            send("content_block_stop", index=index)
                        if not calls:
                            send("content_block_start", index=0, content_block={"type": "text", "text": ""})
                            send("content_block_delta", index=0, delta={"type": "text_delta", "text": "Done."})
                            send("content_block_stop", index=0)
                        send("message_delta", delta={"stop_reason": "tool_use" if calls else "end_turn", "stop_sequence": None}, usage={"output_tokens": 1})
                        send("message_stop")
                except (BrokenPipeError, ConnectionResetError):
                    pass
                except Exception as exc:
                    owner.errors.append(str(exc))
                    self.send_error(500, "local capture failed")
        # Ephemeral loopback sockets avoid taking over or terminating any existing listener.
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.endpoint = f"http://127.0.0.1:{self.server.server_port}"

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()


def replay(path: Path) -> None:
    data = read_json(path)
    keys = ("CLAUDE_CODE_OAUTH_TOKEN", "CODEMAP_BENCH_CLAUDE_TOKEN_FILE", "CODEMAP_BENCH_CLAUDE_CHILD_ENV_FD")
    canary = {"server": {key: key in os.environ for key in keys}}
    child = subprocess.run([sys.executable, "-c", "import json,os;print(json.dumps({k:k in os.environ for k in " + repr(keys) + "}))"],
                           capture_output=True, check=True)
    canary["child"] = json.loads(child.stdout)
    write_json(path.with_suffix(".environment.json"), canary)
    for line in sys.stdin:
        request = json.loads(line)
        from .core import append_jsonl
        append_jsonl(path.with_suffix(".requests.jsonl"), request)
        if "id" not in request:
            continue
        method = request["method"]
        result = data["initialize"] if method == "initialize" else {"tools": data["tools"]} if method == "tools/list" else {
            "resources": [], "prompts": [], "resourceTemplates": []}
        print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)


def play(folder: Path, spec: dict, script, *, toolchain: dict, client_output: dict | None = None) -> dict:
    """Run the pinned pi once against the loopback provider, which answers with the scripted read-tool calls.
    `script` is called after the launch, so it can place canaries the launch preflight would refuse, and returns
    the [name, arguments] calls. No hosted request and no operator credential; returns the redacted stream and
    the guard's admission rows."""
    from .core import read_jsonl
    folder.mkdir(parents=True, exist_ok=True)
    require(not (folder / "events.jsonl").exists(), "scripted probe folder was already used")
    calls = []
    provider = CaptureProvider(toolchain["node"], script=calls, script_mode=spec.get("measurement_mode", "direct"))
    process = cwd = env = None
    try:
        args, cwd, env, _ = pi_client.launch(folder, spec, None, local_endpoint=provider.endpoint, toolchain=toolchain)
        calls.extend(script())
        require(bool(calls), "scripted probe needs at least one call")
        for name, text in (client_output or {}).items():
            (pi_client.private_root(folder) / "tmp" / name).write_text(text)
        process = subprocess.Popen(args, cwd=cwd, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True)
        try:
            stdout, stderr = process.communicate(b"Local boundary probe.", timeout=180)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
            raise ValueError("local read-tool probe timed out") from None
        token = env.get(pi_client.TOKEN_ENV)
        (folder / "events.jsonl").write_bytes(pi_client.redact(stdout, token))
        (folder / "stderr.log").write_bytes(pi_client.redact(stderr, token))
        write_json(folder / "request.json", provider.requests)
        write_json(folder / "command.json", {"args": args, "cwd": str(cwd)})
        require(process.returncode == 0 and len(provider.requests) == 2 and not provider.errors, "local read-tool probe failed")
        return {"events": read_jsonl(folder / "events.jsonl"), "admissions": read_jsonl(folder / pi_client.READ_CALLS)}
    finally:
        provider.close()
        if process is not None:
            from .runner import cleanup_solver
            cleanup = cleanup_solver(process, lambda: None)
            write_json(folder / "capture-cleanup.json", cleanup)
            require(cleanup["complete"], "local probe process cleanup incomplete")
        if cwd is not None:
            cleanup = pi_client.cleanup_private(folder, cwd, spec["provider"], env.get(pi_client.TOKEN_ENV))
            write_json(folder / "private-cleanup.json", cleanup)
            require(cleanup["complete"], "private state cleanup validation failed")


def capture(folder: Path, state: dict, spec: dict, prompt: str, *, toolchain: dict | None = None, port: int = 0,
            observer: Path | None = None, guard_probe: bool = False) -> dict:
    from . import claude_settings, tool_artifacts, claude_auth
    import secrets
    import tempfile
    require(not guard_probe or (spec["provider"] == "claude-bridge" and spec.get("native_agent")
                                and spec.get("measurement_mode") == "direct"), "read guard probe requires Claude rg/direct")
    from .route_transport import READ_TOOL
    folder.mkdir(parents=True, exist_ok=False)
    inventory = read_json(Path(state["native_inventory"]))
    require(file_digest(Path(state["native_inventory"])) == state["native_inventory_sha256"], "audit inventory changed")
    relay = state.get("relay_settings")
    server = None
    if relay:
        selected = [tool for tool in inventory["tools"] if tool["name"] in relay["enabled_tools"]]
        if "read" in relay.get("auxiliaries", []):
            selected.append(READ_TOOL)
        replay_path = folder / "inventory.json"
        from .client_delivery import client_tool_metadata
        selected = [client_tool_metadata(tool) for tool in selected]
        write_json(replay_path, {"initialize": inventory["initialize"], "tools": selected})
        server = {"command": sys.executable, "args": ["-m", "benchmark.pi_audit", str(replay_path)],
                  "env": {"PYTHONPATH": str(settings.REPOSITORY), "PYTHONDONTWRITEBYTECODE": "1"},
                  "instructions": inventory["initialize"].get("instructions", ""),
                  "capabilities": inventory["initialize"].get("capabilities", {})}
    tools = toolchain or tool_artifacts.pi_toolchain()
    if spec["provider"] == "claude-bridge" and observer is None:
        from .pi_probe import tool_list_observer
        observer = tool_list_observer(tools, folder)
    credential = claude_auth.Credential("local-" + secrets.token_urlsafe(24)) if spec["provider"] == "claude-bridge" else None
    credential_file = tempfile.NamedTemporaryFile(mode="w", dir=folder.parent, prefix=".probe-token-") if guard_probe else None
    canary = tempfile.NamedTemporaryFile(mode="w", dir=folder.parent, prefix=".probe-canary-") if guard_probe else None
    canary_text = "PI_READ_GUARD_CANARY_" + secrets.token_hex(16)
    if credential_file:
        credential_file.write(credential.token)
        credential_file.flush()
        path = Path(credential_file.name).absolute()
        credential.paths = tuple(dict.fromkeys((str(path), str(path.resolve()))))
        canary.write(canary_text)
        canary.flush()
    # The scripted call asks the bridge to read a file next to the source; its text must never come back in a request.
    provider = CaptureProvider(tools["node"], port=port, expected_auth=credential.token if credential else None,
                               script=[["read", {"path": str(Path(canary.name).absolute())}]] if guard_probe else None,
                               forbidden_values=(*(credential.redactions if credential else ()), *((canary_text,) if guard_probe else ())))
    home = claude_auth.home_path(folder) if credential else None
    spec = dict(spec)
    if home:
        workspace = Path(state["source"])
        spec["claude_settings"] = claude_settings.snapshot(workspace, home=home)
    process = None
    cwd = env = channel = None
    try:
        args, cwd, env, channel = pi_client.launch(folder, spec, relay, server=server, local_endpoint=provider.endpoint,
                                                  credential=credential, toolchain=tools)
        if observer:
            args[1:1] = ["--import", str(observer)]
            env["BENCH_PROBE_TOOL_LIST"] = str(folder / "bridge-tools.jsonl")
        process = subprocess.Popen(args, cwd=cwd, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True,
                                   pass_fds=(channel.read_fd,) if channel else ())
        if channel:
            channel.close_parent_read()
        try:
            stdout, stderr = process.communicate(prompt.encode(), timeout=90)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
            raise ValueError("local request audit timed out") from None
        redactions = credential.redactions if credential else env.get(pi_client.TOKEN_ENV)
        (folder / "events.jsonl").write_bytes(pi_client.redact(stdout, redactions))
        (folder / "stderr.log").write_bytes(pi_client.redact(stderr, redactions))
        write_json(folder / "request.json", provider.requests)
        write_json(folder / "command.json", {"args": args, "cwd": str(cwd)})
        require(process.returncode == 0 and len(provider.requests) == (2 if guard_probe else 1) and not provider.errors,
                "local provider request capture failed (summary/callback-failure behavior is not tested)")
        read_guard = None
        if guard_probe:
            from .core import read_jsonl
            events = read_jsonl(folder / "events.jsonl")
            ends = [e for e in events if e.get("type") == "tool_execution_end" and e.get("toolName") == "read"]
            rows = read_jsonl(folder / pi_client.READ_CALLS)
            require(len(ends) == len(rows) == 1 and ends[0].get("isError") is True and rows[0]["status"] == "rejected"
                    and any(pi_client.REJECTION in text for _, text in strings(ends[0].get("result"))),
                    "provider probe did not reject the read outside the source")
            read_guard = {"requests": 2, "outside_read_rejected": True, "credential_and_path_absent": True}
        body = provider.requests[0]
        violations = audit_request(body, inventory)
        env_canary = read_json(replay_path.with_suffix(".environment.json")) if relay else None
        if env_canary and any(value for process in env_canary.values() for value in process.values()):
            violations.append({"kind": "MCP child credential environment leak"})
        native = set(pi_client.NATIVE_TOOLS)
        expected = ({"codemode"} if spec.get("measurement_mode") == "codemode" else
                    {*native, *[pi_client.navigation_tool_name(name) for name in relay["enabled_tools"]]} if relay else native)
        if relay and spec.get("measurement_mode") != "codemode" and "resources" in inventory["initialize"].get("capabilities", {}):
            expected.update(pi_client.RESOURCE_TOOLS)
        declared = declared_tool_names(body)
        descriptions = None
        full_body = body
        if credential:
            from .pi_probe import bridge_tools, description_delivery
            full_tools = bridge_tools(folder)
            descriptions = description_delivery(body, full_tools)
            violations += descriptions["violations"]
            full_body = {"tools": list(full_tools.values())}
        nested = None
        if spec.get("measurement_mode") == "codemode":
            allowed = {*native, *[pi_client.navigation_tool_name(name) for name in relay["enabled_tools"]]} if relay else set(native)
            if relay and "resources" in inventory["initialize"].get("capabilities", {}):
                allowed.update(pi_client.RESOURCE_TOOLS)
            nested = audit_codemode_catalog(full_body, allowed, product=bool(relay))
            delivered = audit_codemode_catalog(body, allowed, product=bool(relay))
            nested["delivered_complete_headers"] = delivered["inline_count"]
            nested["delivered_tools"] = delivered["listed_tools"]
            if not nested["passed"]:
                violations.append({"kind": "codemode_nested_catalog", **nested})
        instructions = inventory.get("initialize", {}).get("instructions", "")
        system = body.get("instructions", body.get("system", ""))
        if instructions.strip() and not any(instructions in text for _, text in strings(system)):
            violations.append({"kind": "missing_full_server_instructions"})
        if declared != expected:
            violations.append({"kind": "unexpected_declared_tools", "expected": sorted(expected), "actual": sorted(declared)})
        surfaces = static_surfaces(folder, state["source"])
        if spec["provider"] == "claude-bridge":
            surfaces["paths"].append(str(Path(env["HOME"]) / ".claude/projects" / pi_client.claude_project_slug(cwd) /
                                         "00000000/tool-results/00000000.json"))
        violations += audit_text(surfaces)
        session = audit_session(folder, inventory)
        violations += session["violations"]
        result = {"passed": not violations, "violations": violations, "static_surfaces": surfaces,
                  "inventory": state["native_inventory"], "inventory_sha256": state["native_inventory_sha256"],
                  "replayed_initialize_sha256": digest(inventory.get("initialize")),
                  "replayed_tools_sha256": digest(selected) if relay else None,
                  "request": str(folder / "request.json"), "request_sha256": file_digest(folder / "request.json"),
                  "command": str(folder / "command.json"), "command_sha256": file_digest(folder / "command.json"),
                  "session_audit": session, "declared_tools": sorted(declared), "nested_catalog": nested,
                  "description_delivery": descriptions, "read_guard": read_guard,
                  "oauth_header_matches": provider.auth_matches if credential else None, "mcp_environment_canary": env_canary, "hosted_model_requests": 0}
        write_json(folder / "audit.json", result)
        return result
    finally:
        provider.close()
        if credential_file:
            credential_file.close()
            canary.close()
        if channel:
            channel.close()
        if process is not None:
            from .runner import cleanup_solver
            cleanup = cleanup_solver(process, lambda: None)
            write_json(folder / "capture-cleanup.json", cleanup)
            require(cleanup["complete"], "local capture process cleanup incomplete")
        if cwd is not None:
            cleanup = pi_client.cleanup_private(folder, cwd, spec["provider"],
                credential.redactions if credential else env.get(pi_client.TOKEN_ENV), home=home)
            write_json(folder / "private-cleanup.json", cleanup)
            require(cleanup["complete"], "private credential/HOME cleanup validation failed")


def prepare_audit(states: dict, specs: dict, root: Path) -> dict:
    results, violations = {}, []
    for arm, state in states.items():
        prompts = {qid: routes.solver_prompt(qid, Path(state["source"]), arm) for qid in routes.QUESTION_IDS}
        for qid, prompt in prompts.items():
            violations.extend({"question_id": qid, "arm": arm, **v} for v in audit_text(prompt))
        for mode in routes.MEASUREMENT_MODES:
            key = routes.execution_key(arm, mode)
            results[key] = capture(root / arm / mode, state, specs[key], next(iter(prompts.values())))
            violations.extend({"condition": key, **v} for v in results[key]["violations"])
    result = {"passed": not violations, "conditions": results, "violations": violations, "hosted_model_requests": 0}
    write_json(root / "audit.json", result)
    require(result["passed"], f"model-context audit failed; evidence: {root / 'audit.json'}")
    return {"path": str(root / "audit.json"), "sha256": file_digest(root / "audit.json")}


if __name__ == "__main__":
    replay(Path(sys.argv[1]))
