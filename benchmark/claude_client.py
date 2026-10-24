"""Claude Code launch policy and native stream observations; no API requests at import."""
from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

from .core import canonical, command, file_digest, metric, ratio, require, write_json

BACKEND = "claude_code"
NATIVE_TOOLS = ("Bash", "Read", "Glob", "Grep")


def environment() -> dict:
    # The CLI owns authentication. Do not read, copy or log credentials or inherit model overrides.
    allowed = {"PATH", "HOME", "USER", "LOGNAME", "SHELL", "TMPDIR", "LANG", "LC_ALL", "SSL_CERT_FILE", "SSL_CERT_DIR",
               "CLAUDE_CONFIG_DIR", "ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN"}
    env = {key: value for key, value in os.environ.items() if key in allowed}
    from .settings import CACHE
    env.update(CODEMAP_BENCH_CACHE=str(CACHE), MCP_TOOL_TIMEOUT="300000", MCP_TIMEOUT="120000")
    env.update({key: "1" for key in (
        "CLAUDE_CODE_DISABLE_CLAUDE_MDS", "CLAUDE_CODE_DISABLE_AUTO_MEMORY", "CLAUDE_CODE_DISABLE_BACKGROUND_TASKS",
        "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL",
        "CLAUDE_CODE_DISABLE_TERMINAL_TITLE")})
    return env


def capabilities() -> dict:
    require(shutil.which("claude") is not None, "Claude Code CLI가 필요합니다: claude")
    version = command(["claude", "--version"]).strip()
    help_text = command(["claude", "--help"])
    flags = ("--restricted", "--model", "--effort", "--tools", "--settings", "--setting-sources",
             "--strict-mcp-config", "--mcp-config", "--disable-slash-commands", "--include-partial-messages",
             "--output-format", "--no-session-persistence")
    require(all(flag in help_text for flag in flags), "Claude Code에 필요한 실행 옵션이 없습니다")
    login = subprocess.run(["claude", "auth", "status"], env=environment(), capture_output=True, timeout=30)
    require(login.returncode == 0, "Claude Code 로그인이 필요합니다: claude auth login")
    return {"backend": BACKEND, "version": version, "required_flags": list(flags), "login_available": True}


def configuration(source: Path, rg_binary: Path) -> dict:
    source, rg_binary = source.resolve(), rg_binary.resolve()
    search_path = str(rg_binary.parent) + os.pathsep + os.environ.get("PATH", os.defpath)
    # Restricted mode fences native file tools; the native Bash sandbox fences subprocesses.
    return {"schema": "grafana-native-agent-v1", "backend": BACKEND, "mode": "native_agent", "source": str(source),
            "rg_binary": str(rg_binary), "rg_sha256": file_digest(rg_binary), "search_product_mcp": False,
            "env": {"PATH": search_path}, "tools": list(NATIVE_TOOLS),
            "config": {"permissions": {"defaultMode": "dontAsk", "blockReadsOutsideWorkingDirectories": True,
                                        "deny": ["Edit", "Write", "NotebookEdit", "WebFetch", "WebSearch", "Agent", "Cd"]},
                       "sandbox": {"enabled": True, "autoAllowBashIfSandboxed": True, "allowUnsandboxedCommands": False,
                                   "excludedCommands": [],
                                   "filesystem": {"denyRead": ["/"],
                                                  "allowRead": [str(source), str(rg_binary.parent), "/bin", "/sbin", "/usr", "/System", "/Library", "/dev", "/opt/homebrew"],
                                                  "denyWrite": [str(source)]},
                                   "network": {"allowedDomains": [], "strictAllowlist": True}},
                       "env": {"PATH": search_path}},
            "native_tools": "Claude Code native Bash/Read/Glob/Grep; no search product MCP",
            "call_unit": "native exploration tool_use ID; a compound Bash command is one call",
            "output_policy": "native tool and client defaults; no harness clipping"}


def launch(folder: Path, spec: dict, relay_settings: dict | None, *, server: dict | None = None) -> tuple[list[str], Path, dict]:
    native = spec.get("native_agent")
    workspace = Path(native["source"]) if native else folder / "workspace"
    workspace.mkdir(parents=True, exist_ok=True)
    require(not native or native.get("backend") == BACKEND and not relay_settings, "Claude 기준군 설정 오류")
    policy = {"disableAllHooks": True, "autoMemoryEnabled": False, "enabledPlugins": {},
              "permissions": {"defaultMode": "dontAsk", "blockReadsOutsideWorkingDirectories": True}}
    if native:
        policy.update(native["config"])
    mcp = {"mcpServers": {}}
    permitted = []
    if relay_settings:
        from .runner import navigation_server
        server = server if server is not None else navigation_server(folder, relay_settings)
        mcp["mcpServers"]["navigation"] = {key: server[key] for key in ("command", "args", "env")}
        permitted = ["mcp__navigation__" + name for name in relay_settings["enabled_tools"]]
    policy_path, mcp_path = folder / "claude-settings.json", folder / "mcp-config.json"
    write_json(policy_path, policy, exclusive=True)
    write_json(mcp_path, mcp, exclusive=True)
    args = [shutil.which("claude") or "claude", "--print", "--verbose", "--output-format", "stream-json",
            "--include-partial-messages", "--model", spec["model"], "--effort", spec["reasoning_effort"],
            "--restricted", "--setting-sources", "", "--settings", str(policy_path),
            "--strict-mcp-config", "--mcp-config", str(mcp_path), "--disable-slash-commands",
            "--no-session-persistence", "--permission-mode", "dontAsk", "--tools", ",".join(NATIVE_TOOLS) if native else ""]
    if permitted:
        args += ["--allowedTools", ",".join(permitted)]
    env = environment()
    if native:
        env.update(native["env"])
    env.update(MCP_TOOL_TIMEOUT="300000", MCP_TIMEOUT="120000")
    return args, workspace, env


def probe(value: dict, folder: Path, denied_paths: list[Path]) -> dict:
    """Validate CLI startup with no prompt/model. Native tool enforcement remains unobserved."""
    from .settings import execution_settings
    folder.mkdir(parents=True, exist_ok=True)
    host = capabilities()
    spec = execution_settings("claude-opus-5-5")
    spec["native_agent"] = value
    args, workspace, env = launch(folder, spec, None)
    args.append("--init-only")
    result = subprocess.run(args, cwd=workspace, env=env, input=b"", capture_output=True, timeout=60)
    (folder / "startup.stdout").write_bytes(result.stdout)
    (folder / "startup.stderr").write_bytes(result.stderr)
    require(result.returncode == 0, f"Claude 준비 시작 검증 실패: {folder / 'startup.stderr'}")
    observation = {"name": "claude_init_only", "exit_code": result.returncode, "command": args,
                   "cli": host, "configuration": value, "model_invocations": 0,
                   "policy_sha256": file_digest(folder / "claude-settings.json"),
                   "denied_reference_paths": [str(p) for p in denied_paths],
                   "stdout_sha256": file_digest(folder / "startup.stdout"), "stderr_sha256": file_digest(folder / "startup.stderr"),
                   "boundary_probe": "Configuration and CLI startup inspected; no native tool call or model request was made."}
    write_json(folder / "probe.json", observation)
    write_json(folder / "inventory.json", {"backend": BACKEND, "configuration": value, "cli": host,
        "tool_inventory_observation": "Configured native tool allowlist only; actual model inventory is unobserved."})
    return {"native_inventory": str(folder / "inventory.json"), "native_inventory_sha256": file_digest(folder / "inventory.json"),
            "native_agent": value, "relay_settings": None, "tool_condition": "native_agent_baseline",
            "auxiliary_necessity": [], "probes": [{"name": observation["name"], "exit_code": result.returncode}],
            "probe_log": str(folder / "probe.json"), "probe_log_sha256": file_digest(folder / "probe.json"),
            "cleanup": {"frontend": {"complete": True}, "daemon": {"shared_daemon": False, "mode": "init-only exited"}}}


def tool_items(events: list[dict]) -> list[dict]:
    """Deduplicate partial/final tool blocks by the provider's tool_use ID."""
    items, results = {}, {}
    for event in events:
        if event.get("parent_tool_use_id"):
            continue
        blocks = event.get("message", {}).get("content", [])
        if event.get("type") == "stream_event" and event.get("event", {}).get("type") == "content_block_start":
            blocks = [event["event"].get("content_block", {})]
        if not isinstance(blocks, list):
            continue
        for block in blocks:
            if block.get("type") == "tool_use" and isinstance(block.get("id"), str):
                items[block["id"]] = {**items.get(block["id"], {}), **block}
            elif block.get("type") == "tool_result" and isinstance(block.get("tool_use_id"), str):
                results[block["tool_use_id"]] = block
    return [{**item, "completed": ident in results, "result": results.get(ident)} for ident, item in items.items()]


def usage(events: list[dict], *, terminal_complete: bool, evidence: str) -> dict:
    """Input includes both cache reads and writes. Streaming output is cumulative per message."""
    messages, active, stopped, terminals = {}, None, set(), []
    errors = []
    for event in events:
        if event.get("parent_tool_use_id"):
            errors.append("unexpected subagent usage")
            continue
        if event.get("type") == "assistant":
            message = event.get("message", {})
            if isinstance(message.get("id"), str):
                # Assistant output_tokens is a message_start placeholder, never overwrite a later delta.
                current = messages.setdefault(message["id"], {})
                for key, value in message.get("usage", {}).items():
                    current.setdefault(key, value)
        stream = event.get("event", {}) if event.get("type") == "stream_event" else {}
        if stream.get("type") == "message_start":
            message = stream["message"]
            active = message.get("id")
            if isinstance(active, str):
                messages.setdefault(active, {}).update(message.get("usage", {}))
        elif stream.get("type") == "message_delta" and active:
            messages[active].update(stream.get("usage", {}))
        elif stream.get("type") == "message_stop" and active:
            stopped.add(active)
            active = None
        if event.get("type") == "result":
            terminals.append(event)

    def normalized(value):
        names = ("input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")
        if not isinstance(value, dict) or not all(type(value.get(k)) is int and value[k] >= 0 for k in names):
            return None
        inputs = value["input_tokens"] + value["cache_read_input_tokens"] + value["cache_creation_input_tokens"]
        return {"input_tokens": inputs, "output_tokens": value["output_tokens"],
                "cached_input_tokens": value["cache_read_input_tokens"], "cache_write_input_tokens": value["cache_creation_input_tokens"],
                "total_tokens": inputs + value["output_tokens"]}

    keys = ("input_tokens", "output_tokens", "cached_input_tokens", "cache_write_input_tokens", "total_tokens")
    observed = dict.fromkeys(keys, 0)
    for value in messages.values():
        parts = normalized(value)
        if parts is not None:
            for key in keys:
                observed[key] += parts[key]
    terminal = terminals[-1] if len(terminals) == 1 else None
    total = normalized(terminal.get("usage")) if terminal else None
    if total is None:
        errors.append("no complete native result usage")
    elif any(total[k] < observed[k] for k in keys):
        errors.append("terminal usage is smaller than observed stream usage")
    elif total:
        observed = total
    if not terminal_complete:
        errors.append("unfinished model process or event collection")
    if terminal and (terminal.get("is_error") or terminal.get("subtype") != "success"):
        errors.append("error result may omit consumed usage")
    if any(not item["completed"] for item in tool_items(events) if item.get("name") != "EndConversation"):
        errors.append("unfinished tool calls")
    complete = not errors
    result = {"complete": complete, "errors": errors, "observed": observed, "response_ids": sorted(messages),
              "raw_terminal_usage": terminal.get("usage") if terminal else None,
              "raw_model_usage": terminal.get("modelUsage") if terminal else None,
              "consistency": {"status": "valid" if complete else "unconfirmed", "errors": errors, "evidence": [evidence]},
              "cost_completeness": {"complete": complete, "status": "complete" if complete else "incomplete", "reasons": errors},
              "components": {}}
    reason = "; ".join(errors) or None
    for key in keys:
        result[key] = metric(observed[key] if complete else None, "tokens", reason=reason, evidence=[evidence])
        result["components"][key] = {**result[key], "observed": observed[key], "missing_reason": reason}
    result["components"]["reasoning_output_tokens"] = {"value": None, "observed": None, "unit": "tokens",
                                                       "missing_reason": "Claude Code does not report a separate reasoning token count."}
    result["model_responses"] = metric(len(messages) if complete and messages else None, "responses",
                                       reason=None if complete and messages else "No complete response inventory", evidence=[evidence])
    result["cached_input_ratio"] = ratio(result["cached_input_tokens"]["value"], result["input_tokens"]["value"], evidence=[evidence])
    return result


def final_result(events: list[dict]) -> dict | None:
    values = [e for e in events if e.get("type") == "result"]
    return values[0] if len(values) == 1 else None


def valid_model(events: list[dict], expected: str) -> bool:
    init = [e for e in events if e.get("type") == "system" and e.get("subtype") == "init"]
    models = [e.get("message", {}).get("model") for e in events if e.get("type") == "assistant"]
    result = final_result(events) or {}
    recorded_models = result.get("modelUsage", {})
    return (bool(init and models) and all(e.get("model") == expected for e in init)
            and all(model == expected for model in models)
            and (not recorded_models or set(recorded_models) == {expected}))
