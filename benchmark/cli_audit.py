"""First-request wire and post-slot context audits for the pinned model-specific CLI solvers."""
from __future__ import annotations

import json
import os
import re
import shutil
import signal
import subprocess
import sys
from pathlib import Path

from . import claude_client, codex_client, pi_audit, route_contract as routes, settings
from .core import digest, file_digest, read_json, require, write_json
from .pi_client import private_root


def native_text(inventory: dict) -> dict:
    # MCP clients use their own namespace spelling; the identifier itself remains product-owned.
    result = {**inventory, "tools": []}
    for tool in inventory.get("tools", []):
        result["tools"].extend([tool, {"name": "mcp__navigation__" + tool["name"]},
                                {"name": "mcp__navigation." + tool["name"]}])
    return result


def skill_context(value) -> tuple[list[dict], list[dict]]:
    """Only the pinned client's private built-in skill root may supply skill entries."""
    entries, errors = [], []
    system_root = re.compile(re.escape(str(settings.CACHE.resolve())) + r"/[0-9a-f]{16}/codex-home/skills/\.system")
    for location, text in pi_audit.strings(value):
        for section in re.findall(r"<skills_instructions>([\s\S]*?)</skills_instructions>", text):
            roots = dict(re.findall(r"^- `([^`]+)` = `([^`]+)`$", section, re.MULTILINE))
            for alias, root in roots.items():
                if not system_root.fullmatch(root):
                    errors.append({"kind": "user_skill_root", "location": location, "root": root})
            for name, path in re.findall(r"^- ([^:\n]+):[^\n]*\(file: ([^)]+)\)$", section, re.MULTILINE):
                alias, _, relative = path.partition("/")
                root = roots.get(alias, "")
                if not system_root.fullmatch(root) or ".." in Path(relative).parts:
                    errors.append({"kind": "user_skill_entry", "location": location, "name": name, "file": path})
                else:
                    entry = {"name": name, "root": root, "file": relative}
                    if entry not in entries:
                        entries.append(entry)
            listed = re.findall(r"^- [^`][^\n]+", section, re.MULTILINE)
            parsed = re.findall(r"^- ([^:\n]+):[^\n]*\(file: ([^)]+)\)$", section, re.MULTILINE)
            if len(listed) != len(parsed):
                errors.append({"kind": "unrecognized_skill_entry", "location": location})
    return entries, errors


def private_context_violations(value) -> list[dict]:
    errors = skill_context(value)[1]
    for location, text in pi_audit.strings(value):
        if str(Path.home()) in text:
            errors.append({"kind": "operator_home_in_context", "location": location})
        if re.search(r"(?:~|\$HOME|/[^\s`<>\"']*)/\.(?:agents|claude|codex)/skills(?!/\.system)", text):
            errors.append({"kind": "user_skill_path", "location": location})
    return errors


def audit_system(value, inventory: dict) -> list[dict]:
    errors = private_context_violations(value)
    instructions = inventory.get("initialize", {}).get("instructions", "")
    for location, original in pi_audit.strings(value):
        errors += pi_audit.path_violations(original, location)
        text = original.replace(instructions, "[native instructions]") if instructions else original
        if instructions.strip():
            # Claude trims instructions and can render a bounded native prefix. Exempt only an exact proven prefix.
            text = text.replace(instructions.strip(), "[native instructions]")
            for match in re.finditer(re.escape(instructions[:64]) + r"[\s\S]*?… \[truncated\]", text):
                prefix = match.group().removesuffix("… [truncated]")
                if instructions.startswith(prefix):
                    text = text.replace(match.group(), "[native instructions prefix]")
        errors += pi_audit.audit_text(text)
    return errors


def audit_request(body: dict, inventory: dict) -> list[dict]:
    errors = private_context_violations(body)
    errors += pi_audit.path_violations(json.dumps(body, ensure_ascii=False), "request")
    errors += audit_system(body.get("instructions", body.get("system", "")), inventory)
    errors += pi_audit.audit_text(body.get("tools", []), inventory=native_text(inventory), product_owned=True)
    for item in body.get("input", body.get("messages", [])):
        if item.get("type") == "additional_tools":
            errors += pi_audit.audit_text(item, inventory=native_text(inventory), product_owned=True)
            continue
        errors += (audit_system(item, inventory) if item.get("role") in {"system", "developer"} else pi_audit.audit_text(item))
    return errors


def declared_tools(body: dict) -> set[str]:
    declarations = list(body.get("tools", []))
    declarations.extend(tool for item in body.get("input", []) if item.get("type") == "additional_tools" for tool in item.get("tools", []))
    result = set()
    for tool in declarations:
        if tool.get("type") == "namespace":
            result.update(tool["name"] + "." + (child.get("name") or child.get("type", "unknown")) for child in tool["tools"])
        else:
            result.add(tool.get("name") or tool.get("type", "unknown"))
    return result


def audit_tools(body: dict, spec: dict, relay: dict | None, inventory: dict) -> list[dict]:
    declared = declared_tools(body)
    if spec["solver_backend"] == "claude_code":
        expected = {"mcp__navigation__" + name for name in relay["enabled_tools"]} if relay else set(claude_client.NATIVE_TOOLS)
        if relay and "resources" in inventory.get("initialize", {}).get("capabilities", {}):
            expected |= set(claude_client.RESOURCE_TOOLS)
        allowed = expected
    else:
        expected = {"functions.exec", "functions.wait"}
        allowed = expected | {"functions.update_plan", "functions.request_user_input", "functions.request_user_input_async", "clock.curr_time", "clock.sleep"}
    if not expected <= declared or declared - allowed:
        return [{"kind": "unexpected_declared_tools", "expected": sorted(expected), "allowed": sorted(allowed), "actual": sorted(declared)}]
    return []


def audit_nested_tools(body: dict, relay: dict | None) -> tuple[list[str], list[dict]]:
    names = None
    for item in body.get("input", []):
        if item.get("type") != "custom_tool_call_output" or item.get("call_id") != "inspect_tools":
            continue
        output = item.get("output", [])
        for block in output if isinstance(output, list) else [{"text": output}]:
            try:
                value = json.loads(block.get("text", ""))
            except ValueError:
                continue
            if isinstance(value, list) and all(isinstance(name, str) for name in value):
                names = value
    expected = {"mcp__navigation__" + name for name in relay["enabled_tools"]} if relay else {"exec_command", "write_stdin"}
    allowed = expected | {"apply_patch", "update_plan", "clock__curr_time", "request_user_input", "request_user_input_async"}
    allowed |= {"list_mcp_resources", "list_mcp_resource_templates", "read_mcp_resource"}
    if names is None or not expected <= set(names) or set(names) - allowed:
        return names or [], [{"kind": "unexpected_nested_tools", "expected": sorted(expected), "allowed": sorted(allowed), "actual": names}]
    return names, []


def audit_session(folder: Path, inventory: dict, backend: str) -> dict:
    from .runner import read_jsonl_prefix
    paths = [folder / "rollout.jsonl"] if backend == "codex" else sorted(folder.glob("claude-session-*.jsonl"))
    errors, files, systems, users, system_skills = [], [], 0, 0, []
    for path in paths:
        rows, state = read_jsonl_prefix(path)
        if not state["complete"]:
            errors.append({"kind": "incomplete_session", "path": str(path)})
        if path.is_file():
            files.append({"path": str(path), "sha256": file_digest(path)})
        for row in rows:
            for skill in skill_context(row)[0]:
                if skill not in system_skills:
                    system_skills.append(skill)
            payload = row.get("payload", {})
            if backend == "codex":
                if row.get("type") == "session_meta":
                    text = payload.get("base_instructions", {}).get("text")
                    if text:
                        systems += 1
                        errors += audit_system(text, inventory)
                if row.get("type") == "turn_context":
                    errors += audit_system({key: payload[key] for key in ("cwd", "developer_instructions", "user_instructions") if key in payload}, inventory)
                message = payload if row.get("type") == "response_item" else {}
            else:
                message = row.get("message", {})
                if not isinstance(message, dict):
                    message = {}
                attachment = row.get("attachment", {})
                if attachment.get("type") == "prompt_snapshot":
                    systems += 1
                    errors += audit_system(attachment.get("systemPrompt", []), inventory)
                elif row.get("type") == "api-request-shape":
                    errors += audit_request(row["shape"], inventory)
                elif row.get("type") == "attachment" and row.get("rendered"):
                    errors += audit_system(row["rendered"], inventory)
            if message.get("type") == "additional_tools":
                errors += pi_audit.audit_text(message, inventory=native_text(inventory), product_owned=True)
            elif message.get("role") in {"system", "developer"}:
                systems += 1
                errors += audit_system(message, inventory)
            elif message.get("role") == "user":
                # Tool results are product/source content, not harness-owned instructions.
                content = message.get("content", [])
                if isinstance(content, list):
                    content = [block for block in content if block.get("type") != "tool_result"]
                if content:
                    users += 1
                    errors += private_context_violations(content)
                    errors += pi_audit.audit_text(content)
    if not files or not systems or not users:
        errors.append({"kind": "missing_system_or_user_session", "systems": systems, "users": users})
    result = {"passed": not errors, "violations": errors, "files": files, "system_messages_including_patches": systems,
              "user_messages": users, "system_skills": system_skills}
    write_json(folder / "context-audit.json", result)
    return result


def capture(folder: Path, state: dict, spec: dict, prompt: str, *, port=0) -> dict:
    from .route_transport import READ_TOOL, client_tool_metadata
    folder.mkdir(parents=True, exist_ok=False)
    inventory = read_json(Path(state["native_inventory"]))
    require(file_digest(Path(state["native_inventory"])) == state["native_inventory_sha256"], "audit inventory changed")
    relay, server = state.get("relay_settings"), None
    selected = []
    if relay:
        selected = [tool for tool in inventory["tools"] if tool["name"] in relay["enabled_tools"]]
        if "read" in relay.get("auxiliaries", []):
            selected.append(READ_TOOL)
        replay_path = folder / "inventory.json"
        write_json(replay_path, {"initialize": inventory["initialize"], "tools": [client_tool_metadata(t) for t in selected]})
        server = {"command": sys.executable, "args": ["-m", "benchmark.pi_audit", str(replay_path)],
                  "env": {"PYTHONPATH": str(settings.REPOSITORY), "PYTHONDONTWRITEBYTECODE": "1"},
                  "required": True, "startup_timeout_sec": 120, "tool_timeout_sec": 300, "enabled_tools": relay["enabled_tools"],
                  "instructions": inventory["initialize"].get("instructions", "")}
    backend = spec["solver_backend"]
    provider = pi_audit.CaptureProvider(shutil.which("node"), port=port, inspect_codex_tools=backend == "codex")
    home = private_root(folder) / "home" if backend == "claude_code" else None
    process = cwd = env = monitor = None
    try:
        launch = claude_client.launch if home else codex_client.launch
        args, cwd, env = launch(folder, spec, relay, server=server, local_endpoint=provider.endpoint, **({"local_home": home} if home else {}))
        if home and spec.get("native_agent"):
            from .cli_shell import ShellMonitor
            monitor = ShellMonitor(folder)
        process = subprocess.Popen(args, cwd=cwd, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True)
        if monitor:
            monitor.started(process)
        try:
            stdout, stderr = process.communicate(prompt.encode(), timeout=120)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate()
        (folder / "events.jsonl").write_bytes(stdout)
        (folder / "stderr.log").write_bytes(stderr)
        write_json(folder / "request.json", provider.requests)
        write_json(folder / "command.json", {"args": args, "cwd": str(cwd)})
        require(process.returncode == 0 and len(provider.requests) == (2 if backend == "codex" else 1) and not provider.errors, f"local request capture failed: {folder}")
        from .runner import cleanup_solver
        cleanup = cleanup_solver(process, lambda: None)
        if monitor:
            native_cleanup = monitor.close(lambda: None)
            cleanup["native_shell_groups"] = native_cleanup
            cleanup["complete"] = cleanup["complete"] and native_cleanup["complete"]
        write_json(folder / "capture-cleanup.json", cleanup)
        require(cleanup["complete"], "local capture cleanup incomplete")
        violations = audit_request(provider.requests[0], inventory)
        violations += audit_tools(provider.requests[0], spec, relay, inventory)
        nested = []
        if backend == "codex":
            nested, nested_errors = audit_nested_tools(provider.requests[1], relay)
            violations += nested_errors
        if backend == "codex":
            from .runner import find_rollout, read_jsonl_prefix
            thread = next((e.get("thread_id") for e in read_jsonl_prefix(folder / "events.jsonl")[0] if e.get("type") == "thread.started"), None)
            rollout = find_rollout(Path(env["CODEX_HOME"]), thread)
            if rollout:
                shutil.copyfile(rollout, folder / "rollout.jsonl")
        else:
            private_cleanup = claude_client.cleanup_private(folder, cwd, env)
            write_json(folder / "private-cleanup.json", private_cleanup)
            require(private_cleanup["complete"], "local capture private HOME cleanup incomplete")
        session = audit_session(folder, inventory, backend)
        violations += session["violations"]
        root = private_root(folder)
        surfaces = {"paths": [state["source"], str(cwd), str(root / "tmp"), str(root / "home")],
                    "rejection_messages": pi_audit.model_error_messages()}
        violations += pi_audit.audit_text(surfaces)
        result = {"passed": not violations, "violations": violations, "static_surfaces": surfaces,
                  "inventory": state["native_inventory"], "inventory_sha256": state["native_inventory_sha256"],
                  "replayed_initialize_sha256": digest(inventory.get("initialize")), "replayed_tools_sha256": digest(selected),
                  "request": str(folder / "request.json"), "request_sha256": file_digest(folder / "request.json"),
                  "command": str(folder / "command.json"), "command_sha256": file_digest(folder / "command.json"),
                  "session_audit": session, "system_skills": skill_context(provider.requests[0])[0],
                  "declared_tools": sorted(declared_tools(provider.requests[0])), "nested_tools": nested, "hosted_model_requests": 0}
        write_json(folder / "audit.json", result)
        return result
    finally:
        provider.close()
        if process is not None and not (folder / "capture-cleanup.json").exists():
            from .runner import cleanup_solver
            cleanup = cleanup_solver(process, lambda: None)
            if monitor:
                native_cleanup = monitor.close(lambda: None)
                cleanup["native_shell_groups"] = native_cleanup
                cleanup["complete"] = cleanup["complete"] and native_cleanup["complete"]
            write_json(folder / "capture-cleanup.json", cleanup)
            require(cleanup["complete"], "local capture cleanup incomplete")
        if cwd is not None and home is not None and not (folder / "private-cleanup.json").exists():
            write_json(folder / "private-cleanup.json", claude_client.cleanup_private(folder, cwd, env))


def prepare_audit(states: dict, specs: dict, root: Path, *, port=0) -> dict:
    results, violations = {}, []
    for arm, state in states.items():
        prompts = {qid: routes.solver_prompt(qid, Path(state["source"]), arm) for qid in routes.QUESTION_IDS}
        for qid, prompt in prompts.items():
            violations += [{"question_id": qid, "arm": arm, **v} for v in pi_audit.audit_text(prompt)]
        key = next(key for key in specs if key.startswith(arm + "/"))
        results[key] = capture(root / arm / specs[key]["measurement_mode"], state, specs[key], next(iter(prompts.values())), port=port)
        violations += [{"condition": key, **v} for v in results[key]["violations"]]
    result = {"passed": not violations, "conditions": results, "violations": violations, "hosted_model_requests": 0}
    write_json(root / "audit.json", result)
    require(result["passed"], f"model-context audit failed; evidence: {root / 'audit.json'}")
    return {"path": str(root / "audit.json"), "sha256": file_digest(root / "audit.json")}
