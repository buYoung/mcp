"""Pinned pi client policy, sandboxed bash admission, and JSON stream observations."""
from __future__ import annotations

import json
import os
import pwd
import shlex
import shutil
import subprocess
import sys
import time
import unicodedata
from pathlib import Path
from urllib.parse import urlsplit

from .core import canonical, digest, file_digest, metric, ratio, read_json, require, write_json
from .claude_auth import Credential, ChildEnvChannel

BACKEND = "pi"
TOKEN_ENV = "CODEMAP_BENCH_PI_TOKEN"
NATIVE_TOOLS = ("bash",)
PRIVATE_FLAGS = ["--mode", "json", "-ne", "-ns", "-np", "-nc", "--no-themes", "-na", "--offline"]
NAVIGATION_SERVER = "navigation"
# Client delivery must not cut below finite native budgets; uncapped products use 1 MiB.
# Keep the approved Serena allowance (originally sized for text + structuredContent in adapter 7).
from .client_delivery import (PRODUCT_TEXT_BYTES as OUTPUT_GUARD_BYTES, DEFAULT_TEXT_BYTES,
                              CLAUDE_RESULT_META, CLAUDE_PRODUCT_CHARS)
ADAPTER_SETTINGS = {"autoEnableCodemode": False, "maxOutputBytes": DEFAULT_TEXT_BYTES}
ADAPTER_SERVER = {"timeout": 300, "exposure": "direct"}
RESOURCE_TOOLS = ("list_mcp_resources", "list_mcp_resource_templates", "read_mcp_resource")


def adapter_settings(arm: str) -> dict:
    require(arm in OUTPUT_GUARD_BYTES, "unknown product output guard")
    return {**ADAPTER_SETTINGS, "maxOutputBytes": OUTPUT_GUARD_BYTES[arm]}


def navigation_tool_names(name: str) -> set[str]:
    return {navigation_tool_name(name)}


def navigation_tool_name(name: str) -> str:
    # The pinned inventories use only plain MCP names; fail rather than guess sanitized/hash aliases.
    import re
    require(re.fullmatch(r"[A-Za-z0-9_-]+", name) is not None, "unsupported MCP tool name")
    return f"mcp__{NAVIGATION_SERVER}__{name}"


def server_instructions_prompt(instructions: str) -> str:
    # Neutral harness label; the following instructions remain the product's original text.
    return f"MCP instructions: {NAVIGATION_SERVER}\n\n{instructions}\n"


def private_root(folder: Path) -> Path:
    """Solver-visible cwd and scratch (TMPDIR, shell HOME, rg copy) uses an opaque cache name, never the slot path."""
    from .settings import CACHE
    root = CACHE / digest({"private_root": str(folder.resolve())})[:16]
    require(not root.is_symlink(), "private root symlink 금지")
    root.mkdir(mode=0o700, exist_ok=True)
    root.chmod(0o700)
    record = folder / "private-root.json"
    if not record.exists():
        write_json(record, {"path": str(root), "purpose": "solver-visible cwd, TMPDIR, shell HOME and rg copy"})
    require(read_json(record)["path"] == str(root), "private root binding 변경")
    return root


def requested_model(provider: str, model: str) -> str:
    # Pinned bridge 0.13.1 models.ts: Opus 5.5 is in MEASURED_ONE_M.
    if provider == "claude-bridge":
        require(model == "claude-opus-5-5", "unreviewed Claude runtime model mapping")
        return model + "[1m]"
    return model


def local_endpoint_url(value: str) -> str:
    """Explicit function-parameter seam only; never load a transport override from env/config."""
    url = urlsplit(value)
    require(url.scheme == "http" and url.hostname == "127.0.0.1" and url.port is not None
            and not url.username and not url.password and not url.query and not url.fragment,
            "local pi test endpoint must be an explicit IPv4 loopback HTTP URL")
    return value


def write_models(agent_dir: Path, provider: str, *, local_endpoint: str | None = None) -> None:
    models = {"providers": {provider: {"apiKey": "$" + TOKEN_ENV}}} if provider == "openai-codex" else {}
    if local_endpoint is not None:
        require(provider == "openai-codex", "models.json endpoint seam is openai-codex only")
        models["providers"][provider]["baseUrl"] = local_endpoint_url(local_endpoint)
    write_json(agent_dir / "models.json", models, exclusive=True)
    validate_models(agent_dir, provider, local_endpoint=local_endpoint)


def validate_models(agent_dir: Path, provider: str, *, local_endpoint: str | None = None) -> None:
    # Exact allowlist also prevents api/provider/model overrides from bypassing the pinned transport.
    expected = {"providers": {provider: {"apiKey": "$" + TOKEN_ENV}}} if provider == "openai-codex" else {}
    if local_endpoint is not None:
        require(provider == "openai-codex", "models.json endpoint seam is openai-codex only")
        expected["providers"][provider]["baseUrl"] = local_endpoint_url(local_endpoint)
    require(read_json(agent_dir / "models.json") == expected,
            "private models.json 변경: production baseUrl/API/model override 금지")


def environment(*, agent_dir: Path, tmp: Path, provider: str, token: str | None = None,
                local_endpoint: str | None = None, local_home: Path | None = None) -> dict[str, str]:
    """No inherited provider/agent overrides. The harness, not the shell, owns auth."""
    from .settings import CACHE
    require(provider in {"openai-codex", "claude-bridge"}, "지원하지 않는 pi provider")
    allowed = {"PATH", "USER", "LOGNAME", "LANG", "LC_ALL", "SSL_CERT_FILE", "SSL_CERT_DIR"}
    env = {key: value for key, value in os.environ.items() if key in allowed}
    env.update(HOME=pwd.getpwuid(os.getuid()).pw_dir, TMPDIR=str(tmp.resolve()),
               PI_CODING_AGENT_DIR=str(agent_dir.resolve()), PI_SKIP_VERSION_CHECK="1", PI_TELEMETRY="0",
               CODEMAP_BENCH_CACHE=str(CACHE))
    if provider == "claude-bridge":
        require(local_home is not None, "Claude requires a slot-private HOME")
        home = local_home.resolve()
        require(home != Path(env["HOME"]).resolve() and not local_home.is_symlink(), "Claude 실제 HOME 사용 금지")
        require(home.is_dir(), "Claude private HOME missing")
        env["HOME"] = str(home)
        require(local_endpoint is None, "Claude endpoint/auth must go only through the bridge child-env API")
    else:
        require(local_home is None and local_endpoint is None, "unexpected OpenAI environment override")
    if token is not None:
        require(provider == "openai-codex" and bool(token) and "\n" not in token, "pi token/provider 오류")
        env[TOKEN_ENV] = token
    return env


def operator_token(provider: str) -> str:
    """Obtain only an in-memory bearer token; never expose stdout/stderr on failure."""
    from .tool_artifacts import pi_toolchain
    require(provider == "openai-codex", "bearer-token은 openai-codex 전용입니다")
    tools = pi_toolchain()
    home = Path(pwd.getpwuid(os.getuid()).pw_dir)
    env = environment(agent_dir=home / ".pi/agent", tmp=Path(os.environ.get("TMPDIR", "/tmp")), provider=provider)
    try:
        result = subprocess.run([tools["node"], tools["pi_cli"], "auth", "print-bearer-token", "--provider", provider,
                                 "--min-expiry", "30m"], env=env, capture_output=True, timeout=60)
    except (OSError, subprocess.SubprocessError):
        raise ValueError("pi operator token 획득 실패 (출력 비공개)") from None
    require(result.returncode == 0, "pi operator token 획득 실패 (출력 비공개)")
    try:
        token = result.stdout.decode().strip()
    except UnicodeError:
        raise ValueError("pi bearer token 형식 오류 (출력 비공개)") from None
    require(bool(token) and not any(c.isspace() for c in token), "pi bearer token 형식 오류")
    return token


def capabilities(*, toolchain: dict | None = None) -> dict:
    from .tool_artifacts import pi_toolchain
    import tempfile
    from .settings import CACHE
    tools = toolchain or pi_toolchain()
    with tempfile.TemporaryDirectory(prefix="pi-capabilities-", dir=CACHE) as directory:
        root = Path(directory)
        agent = root / "agent"
        agent.mkdir(mode=0o700)
        write_models(agent, "openai-codex")
        (root / "home").mkdir(mode=0o700)
        env = environment(agent_dir=agent, tmp=root, provider="openai-codex", token="model-free-dummy")
        env["HOME"] = str(root / "home")  # Model-free bridge loading must not inspect operator HOME.
        probes = {}
        write_json(agent / "mcp.json", {**ADAPTER_SETTINGS, "mcpServers": {}})
        for name, args in (("help", [*PRIVATE_FLAGS, "--help"]),
                           ("models", [*PRIVATE_FLAGS, "-e", tools["bridge_extension"], "--list-models"]),
                           ("mcp_adapter_help", [*PRIVATE_FLAGS, "-e", tools["mcp_adapter_extension"], "--help"]),
                           ("private_auth_reference", ["auth", "check", "--provider", "openai-codex"])):
            result = subprocess.run([tools["node"], tools["pi_cli"], *args], cwd=root,
                                    env=env, capture_output=True, text=True, timeout=60)
            require(result.returncode == 0, f"model-free pi probe 실패: {name}")
            require("Failed to load extension" not in result.stderr, "pi extension load 실패")
            probes[name] = {"args": args, "exit_code": result.returncode, "output": result.stdout + result.stderr}
        help_text = probes["help"]["output"]
        models = {tuple(line.split()[:2]) for line in probes["models"]["output"].splitlines()}
        require({("openai-codex", "gpt-6.1-sol"), ("openai-codex", "gpt-6-astra"),
                 ("claude-bridge", "claude-opus-5-5")} <= models, "pinned pi 모델 목록 누락")
        require(probes["private_auth_reference"]["output"].strip() == "ready", "private pi auth reference 오류")
        require("--no-mcp" in probes["mcp_adapter_help"]["output"], "pi MCP option 누락")
    flags = ["--no-tools", "--no-mcp", "--no-extensions", "--no-approve", "--offline", "--session-dir", "--thinking"]
    require(all(flag in help_text for flag in flags), "pi 실행 옵션 누락")
    return {"backend": BACKEND, "version": tools["versions"]["pi"], "versions": tools["versions"],
            "lock_sha256": tools["lock_sha256"], "toolchain_sha256": digest(tools),
            "node_path": tools["node"], "node_sha256": tools["node_sha256"], "required_flags": flags,
            "auth": "runtime only; operator token/Claude login not exercised during preparation",
            "model_free_probes": probes, "model_invocations": 0,
            "defaults": {"compaction": "pi default", "retry": "pi default", "output_truncation": "pi bash default; pi codemode default output unless the script's // @options line sets it",
                         "mcp_client": f"@buyong/pi-mcp-adapter {tools['versions']['mcp_adapter']}; private mcp.json; explicit exposure/timeout/output guard",
                         "mcp_output_guard": {"default": DEFAULT_TEXT_BYTES, "per_arm_bytes": OUTPUT_GUARD_BYTES},
                         "claude_result_chars": {"missing_product_meta": CLAUDE_PRODUCT_CHARS, "codemode": "bridge default (52224)",
                                                 "native_bash": "bridge default (52224)"}}}


def configuration(source: Path, rg_binary: Path) -> dict:
    return {"schema": "grafana-native-agent-v1", "backend": BACKEND, "mode": "native_agent",
            "source": str(source.resolve()), "rg_binary": str(rg_binary.resolve()), "rg_sha256": file_digest(rg_binary),
            "search_product_mcp": False, "tools": ["bash"], "native_tools": "pi bash via per-slot Seatbelt wrapper",
            "call_unit": "atomic wrapper admission; calls measured without a limit",
            "output_policy": "pi defaults; no harness clipping"}


def baseline_boundary(native: dict, folder: Path) -> dict:
    from .isolation import METADATA, _profile_text, preflight
    source, pinned = Path(native["source"]).resolve(), Path(native["rg_binary"]).resolve()
    require(file_digest(pinned) == native["rg_sha256"], "고정 rg 변경")
    root = private_root(folder)
    tmp, home, bin_dir = root / "tmp", root / "home", root / "bin"
    for path in (tmp, home, bin_dir):
        path.mkdir(parents=True, exist_ok=True, mode=0o700)
    # A byte-identical copy keeps the artifact path (and its naming) out of the solver's PATH.
    rg = bin_dir / "rg"
    if not rg.exists():
        shutil.copy2(pinned, rg)
    require(not rg.is_symlink() and file_digest(rg) == native["rg_sha256"], "rg 복사본 변경")
    preflight(source, home)
    text = _profile_text([source, bin_dir, tmp, home], [tmp])
    # Basic shell/navigation utilities need no external Mach service lookup.
    text += "(deny lsopen)\n(deny appleevent-send)\n(deny mach-lookup)\n"
    text += "(deny signal)\n(allow signal (target same-sandbox))\n"
    # Other processes' names and arguments (products, harness, other slots) are not exploration targets.
    # Seatbelt target "others" excludes same-group peers; deny all targets and allow self only.
    text += "(deny process-info*)\n(allow process-info* (target self))\n(deny sysctl-read (sysctl-name-prefix \"kern.proc\"))\n"
    import re
    # Products can create state below nested projects after the profile is written.
    metadata = "|".join(re.escape(name) for name in sorted(METADATA | {".pi", ".claude"}))
    pattern = "^" + re.escape(str(source)) + "/(.*/)?(" + metadata + ")(/|$)"
    text += f"(deny file-read-data file-write* (regex {json.dumps(pattern)}))\n"
    profile = folder / "pi-bash.sb"
    profile.write_text(text)
    env = {"PATH": str(bin_dir) + ":/usr/bin:/bin", "HOME": str(home), "TMPDIR": str(tmp), "LANG": "en_US.UTF-8"}
    return {"profile_path": str(profile), "profile_sha256": file_digest(profile), "env": env, "source": str(source),
            "allowed": {"read": [str(source), str(rg), str(tmp), str(home)], "write": [str(tmp)], "network": False,
                        "process_info_others": False, "signal_outside_sandbox": False}}


def shell_wrapper(native: dict, folder: Path) -> Path:
    profile = baseline_boundary(native, folder)
    config = folder / "pi-bash.json"
    (folder / "bash-calls.jsonl").touch(exist_ok=False)
    (folder / "bash-results.jsonl").touch(exist_ok=False)
    write_json(config, {"folder": str(folder), "profile": profile}, exclusive=True)
    wrapper = folder / "pi-shell"
    wrapper.write_text(f"#!{sys.executable}\nimport sys\nsys.path.insert(0, {str(Path(__file__).resolve().parent.parent)!r})\n"
                       f"from benchmark.pi_client import shell_main\nshell_main({str(config)!r}, sys.argv[1:])\n")
    wrapper.chmod(0o700)
    return wrapper


def shell_main(config_path: str, args: list[str]) -> None:
    """Trusted unsandboxed launcher. Admission and stop share the relay's atomic lock."""
    from .route_transport import navigation_lock, append_jsonl
    from .runner import read_jsonl_prefix
    from .isolation import wrap
    from .execution import ProcessRegistry
    require(len(args) == 2 and args[0] == "-c", "pi shell argv 오류")
    config = read_json(Path(config_path))
    folder = Path(config["folder"])
    with navigation_lock(folder):
        log = folder / "bash-calls.jsonl"
        rows, collection = read_jsonl_prefix(log)
        require(collection["complete"], "pi bash admission log 불완전")
        ident = len(rows) + 1
        stopped = (folder / "stop.json").exists()
        allowed = not stopped
        append_jsonl(log, {"id": ident, "tool": "bash", "command": redact(args[1], os.environ.get(TOKEN_ENV)), "dispatched": allowed,
                           "pid": os.getpid(), "pgid": os.getpgrp(), "start_fingerprint": ProcessRegistry.fingerprint(os.getpid()),
                           "status": "admitted" if allowed else "rejected", "monotonic": time.monotonic()})
    if not allowed:
        print("Exploration call rejected before execution: solver stopped.", file=sys.stderr)
        raise SystemExit(81)
    profile = config["profile"]
    command = wrap(["/usr/bin/env", "-i", *[f"{k}={v}" for k, v in profile["env"].items()],
                    "/bin/bash", "--noprofile", "--norc", "-c", args[1]], profile)
    started = time.monotonic()
    try:
        result = subprocess.run(command, env={}, cwd=profile["source"])
        code = result.returncode
    except BaseException:
        code = None
        raise
    finally:
        with navigation_lock(folder):
            append_jsonl(folder / "bash-results.jsonl", {"id": ident, "started_monotonic": started,
                "finished_monotonic": time.monotonic(), "exit_code": code})
    raise SystemExit(code if code >= 0 else 128 - code)


def codemode_extension_source(factory: Path) -> str:
    """Pinned codemode without the models API; description, parsing and output budget stay native."""
    return ("import {createCodemodeExtension} from " + json.dumps(str(factory)) + ";\n"
            "export default createCodemodeExtension({models:false});\n")


def launch(folder: Path, spec: dict, relay_settings: dict | None, *, server: dict | None = None,
           local_endpoint: str | None = None, credential: Credential | None = None,
           toolchain: dict | None = None) -> tuple[list[str], Path, dict, ChildEnvChannel | None]:
    owned_before = (folder / "claude-home.json").exists()
    try:
        return _launch(folder, spec, relay_settings, server=server, local_endpoint=local_endpoint,
                       credential=credential, toolchain=toolchain)
    except BaseException:
        if not owned_before and (folder / "claude-home.json").is_file():
            from .claude_auth import cleanup_home
            cleanup_home(folder, Path(spec.get("native_agent", {}).get("source", folder)), None)
        raise


def _launch(folder: Path, spec: dict, relay_settings: dict | None, *, server: dict | None = None,
           local_endpoint: str | None = None, credential: Credential | None = None,
           toolchain: dict | None = None) -> tuple[list[str], Path, dict, ChildEnvChannel | None]:
    from .tool_artifacts import pi_toolchain
    tools = toolchain or pi_toolchain()
    from . import claude_auth
    is_claude = spec["provider"] == "claude-bridge"
    if is_claude:
        require(tools.get("bridge_child_env_api") == claude_auth.CHILD_ENV_CONTRACT,
                "Claude bridge API 존재/provider OAuth·도구 메타 검사 증거가 필요합니다 (summary·callback 실패 동작은 별도 검토)")
        credential = credential or claude_auth.read_token_file()
    else:
        require(credential is None, "Claude credentials cannot be used by GPT")
    require(Path(tools["mcp_adapter_extension"]).name == "pi-mcp-adapter"
            and Path(tools["mcp_adapter_extension"]).parent.name == "@buyong", "scoped MCP adapter lock required")
    folder = folder.resolve()
    native = spec.get("native_agent")
    # The product is already running in its own warm host; pi only starts the absolute-path stdio shim.
    workspace = Path((native or relay_settings)["source"]) if native or relay_settings else private_root(folder) / "work"
    require(not workspace.is_symlink(), "pi cwd symlink 금지")
    workspace.mkdir(parents=True, exist_ok=True, mode=0o700)
    require(not native or native.get("backend") == BACKEND and not relay_settings, "pi 기준군 설정 오류")
    agent, tmp, sessions = folder / "pi-agent", private_root(folder) / "tmp", folder / "pi-session"
    for path in (agent, tmp, sessions):
        require(not path.is_symlink() and not path.resolve().is_relative_to(workspace.resolve()), "private pi state must be outside workspace and not a symlink")
        path.mkdir(parents=True, exist_ok=True, mode=0o700)
        path.chmod(0o700)
    provider = spec["provider"]
    bash = native or (configuration(Path(relay_settings["source"]), Path(relay_settings["rg_binary"])) if relay_settings else None)
    settings = {"shellPath": str(shell_wrapper(bash, folder))} if bash else {}
    mode = spec.get("measurement_mode", "direct")
    require(mode in {"direct", "codemode"}, "unknown measurement mode")
    if mode == "codemode":
        settings.update(defaultTools=["+codemode"], codemode={"mode": "only"})
    model_endpoint = local_endpoint if provider == "openai-codex" else None
    write_models(agent, provider, local_endpoint=model_endpoint)
    write_json(agent / "settings.json", settings, exclusive=True)
    args = [tools["node"], tools["pi_cli"], *PRIVATE_FLAGS, "--provider", provider, "--model", spec["model"],
            "--thinking", spec["reasoning_effort"], "--session-dir", str(sessions)]
    if mode == "codemode":
        # Built-in is replaceable; only the models API is turned off.
        extension = agent / "script-tools.mjs"
        factory = Path(tools["pi_cli"]).parent / "extensions/codemode/index.js"
        extension.write_text(codemode_extension_source(factory))
        args += ["-e", str(extension)]
    if is_claude:
        home = claude_auth.create_home(folder)
        bridge = agent / "private-bridge.mjs"
        bridge.write_text(claude_auth.bridge_extension_source(Path(tools["bridge_extension"])))
        # Credentials must be read/closed before other extension/session-start child processes.
        args[2:2] = ["-e", str(bridge)]
    else:
        home = None
    instructions = None
    if relay_settings:
        require(server is not None, "pi MCP requires warm-host attachment")
        require(not (workspace / ".pi/mcp.json").exists(), "project MCP config forbidden")
        adapter_config = agent / "mcp.json"
        # python -m shim/replay must not import modules from the exploration source.
        server_work = private_root(folder) / "work"
        require(not server_work.is_symlink(), "pi MCP cwd symlink 금지")
        server_work.mkdir(mode=0o700, exist_ok=True)
        server_config = {**{k: server[k] for k in ("command", "args", "env")}, "cwd": str(server_work), **ADAPTER_SERVER,
                         "maxOutputBytes": OUTPUT_GUARD_BYTES[relay_settings["arm"]]}
        write_json(adapter_config, {**adapter_settings(relay_settings["arm"]),
                                   "mcpServers": {NAVIGATION_SERVER: server_config}}, exclusive=True)
        product_tools = [navigation_tool_name(name) for name in relay_settings["enabled_tools"]]
        resource_tools = list(RESOURCE_TOOLS) if "resources" in server.get("capabilities", {}) else []
        require(len(set(product_tools)) == len(product_tools), "navigation 도구 이름 충돌")
        args += ["-e", tools["mcp_adapter_extension"], "--tools",
                 ",".join([*product_tools, *resource_tools, "bash", *(["codemode"] if mode == "codemode" else [])])]
        instructions = server.get("instructions") or ""
        require(isinstance(instructions, str), "server instructions 형식 오류")
        if instructions.strip():
            prompt_file = agent / "server-instructions.md"
            prompt_file.write_text(server_instructions_prompt(instructions), encoding="utf-8")
            args += ["--append-system-prompt", str(prompt_file)]
    if provider == "claude-bridge":
        from . import claude_settings
        claude_settings.verify(spec.get("claude_settings"), workspace, home=home)
        require(spec.get("requested_model") == requested_model(provider, spec["model"]), "Claude requested model condition changed")
        # The bridge reads this file independently of pi's -na project policy.
        require(not (workspace / ".pi/claude-bridge.json").exists(), "project bridge config would override the pinned binary")
        write_json(agent / "claude-bridge.json", {"provider": {"pathToClaudeCodeExecutable": tools["claude_code_binary"]}}, exclusive=True)
        metadata = {name: {CLAUDE_RESULT_META: CLAUDE_PRODUCT_CHARS}
                    for name in (resource_tools if relay_settings else [])}
        meta_extension = agent / "delivery-meta.mjs"
        meta_extension.write_text("export default pi => pi.events.on('tool-meta', request => {\n"
                                  "  if (typeof request?.reply === 'function') request.reply(" + json.dumps(metadata) + ");\n});\n")
        args += ["-e", str(meta_extension)]
    if not relay_settings:
        args += ["--no-mcp", "--tools", "bash,codemode" if mode == "codemode" else "bash"] if native else ["--no-mcp", "--no-tools"]
    # A loopback-only test launch must never touch operator credentials.
    if provider == "openai-codex" and local_endpoint is not None:
        from .pi_audit import dummy_token
        token = dummy_token()
    else:
        token = operator_token(provider) if provider == "openai-codex" else None
    env = environment(agent_dir=agent, tmp=tmp, provider=provider, token=token,
                      local_home=home)
    validate_models(agent, provider, local_endpoint=model_endpoint)
    write_json(folder / "pi-policy.json", {"toolchain": tools, "provider": provider, "flags": PRIVATE_FLAGS,
        "requested_model": requested_model(provider, spec["model"]), "measurement_mode": mode,
        "codemode_models": False if mode == "codemode" else None,
        "codemode_output_budget": "pi default; the script's // @options line wins" if mode == "codemode" else None,
        "claude_result_meta": {"product": "preserve native _meta; fill absent anthropic/maxResultSizeChars with 500000",
                               "codemode": "bridge default (52224)" if mode == "codemode" else None,
                               "resources": CLAUDE_PRODUCT_CHARS if relay_settings else None,
                               "native_bash": "bridge default (52224)"} if provider == "claude-bridge" else None,
        "mcp_client": ({"extension": "@buyong/pi-mcp-adapter", "version": tools["versions"]["mcp_adapter"],
                        "config_mode": "private PI_CODING_AGENT_DIR/mcp.json, project config rejected",
                        "settings": adapter_settings(relay_settings["arm"]), "server": {**ADAPTER_SERVER, "cwd": str(server_work), "maxOutputBytes": OUTPUT_GUARD_BYTES[relay_settings["arm"]]}, "tools": product_tools,
                        "resource_tools": resource_tools, "model_api_features": "none in adapter; codemode models:false",
                        "setting_reasons": {"exposure": "direct in both modes; pi mode only hides declarations but lists nested tools", "maxOutputBytes": "client >= product cap, including resources",
                                            "timeout": "existing per-RPC 300 seconds; no solver limit", "autoEnableCodemode": "harness selects slot mode",
                                            "cwd": "shim and audit replay run in private work, not exploration source"},
                        "server_instructions_sha256": digest(instructions) if instructions else None,
                        "server_instructions": "appended to the system prompt" if instructions and instructions.strip() else "none provided"}
                       if relay_settings else None),
        "claude_settings": spec.get("claude_settings"),
        "defaults": capabilities(toolchain=tools)["defaults"],
        "auth": "operator token file -> anonymous pipe -> bridge SDK child env only" if is_claude else "memory-only bearer env",
        "claude_child_env_api": claude_auth.CHILD_ENV_CONTRACT if is_claude else None,
        "claude_residue": "entire owned private HOME removed; real HOME is not used" if is_claude else None}, exclusive=True)
    channel = claude_auth.ChildEnvChannel(credential, local_endpoint=local_endpoint) if is_claude else None
    if channel:
        env[claude_auth.CHILD_ENV_FD] = str(channel.read_fd)
    return args, workspace, env, channel


def redact(data: bytes | str, token: str | tuple[str, ...] | None) -> bytes | str:
    if not token:
        return data
    values = (token,) if isinstance(token, str) else token
    variants = {variant for value in values for variant in (value, json.dumps(value)[1:-1]) if variant}
    for value in sorted(variants, key=len, reverse=True):
        data = data.replace(value.encode(), b"[REDACTED]") if isinstance(data, bytes) else data.replace(value, "[REDACTED]")
    return data


def cleanup_shells(folder: Path, pump) -> dict:
    """Pi launches bash in detached groups; killing only the pi PGID is insufficient."""
    from types import SimpleNamespace
    from .runner import cleanup_solver, read_live_jsonl, signal_target
    from .execution import ProcessRegistry
    rows = read_live_jsonl(folder / "bash-calls.jsonl")
    groups = []
    for row in rows:
        if row["dispatched"]:
            require(row["pid"] == row["pgid"] and row["pgid"] != os.getpgrp(), "pi shell group ownership 오류")
            if signal_target(row["pgid"], 0, group=True):
                current = ProcessRegistry.fingerprint(row["pid"])
                if not row.get("start_fingerprint") or current and current != row["start_fingerprint"]:
                    groups.append({"complete": False, "pgid": row["pgid"], "error": "shell group ownership changed; no signal sent"})
                    continue
            groups.append(cleanup_solver(SimpleNamespace(pid=row["pgid"]), pump))
    return {"complete": all(row["complete"] for row in groups), "groups": groups}


def claude_project_slug(workspace: Path) -> str:
    """cc-session-io 0.4.0: NFC realpath, JS UTF-16 sanitization, 200-character + int32 hash."""
    raw = unicodedata.normalize("NFC", str(workspace.resolve())).encode("utf-16-le")
    units = [int.from_bytes(raw[i:i + 2], "little") for i in range(0, len(raw), 2)]
    sanitized = "".join(chr(unit) if 48 <= unit <= 57 or 65 <= unit <= 90 or 97 <= unit <= 122 else "-" for unit in units)
    if len(sanitized) <= 200:
        return sanitized
    value = 0
    for unit in units:
        value = (31 * value + unit) & 0xffffffff
    value = abs(value if value < 0x80000000 else value - 0x100000000)
    suffix = ""
    while value:
        value, digit = divmod(value, 36)
        suffix = "0123456789abcdefghijklmnopqrstuvwxyz"[digit] + suffix
    return sanitized[:200] + "-" + (suffix or "0")


def cleanup_private(folder: Path, workspace: Path, provider: str, token: str | None, *, home: Path | None = None) -> dict:
    # Pi's own session writer is not routed through stdout. Sanitize it after owned children stop.
    credential_leak = False
    paths = [p for p in folder.iterdir() if p.suffix in {".json", ".jsonl", ".txt", ".log"}]
    scratch = private_root(folder)
    for root in (folder / "pi-session", folder / "pi-agent", scratch / "tmp", scratch / "home"):
        paths.extend(root.rglob("*") if root.exists() else ())
    for path in paths:
        if path.is_file() and not path.is_symlink():
            original = path.read_bytes()
            clean = redact(original, token)
            if clean != original:
                credential_leak |= provider == "claude-bridge"
                path.write_bytes(clean)
    if provider == "claude-bridge":
        from .claude_auth import cleanup_home
        result = cleanup_home(folder, workspace, token)
        result["credential_leak_detected"] |= credential_leak
        result["complete"] = result["complete"] and not credential_leak
        return result
    return {"complete": True, "removed_project": None, "claude_json_modified_by_harness": False, "claude_persisted_tool_results": None}


ARGS_PREVIEW_CHARS = 200  # pinned codemode previewArgs: JSON.stringify(args), longer text cut to 197 UTF-16 units + "..."


def script_args_preview(arguments) -> str:
    """Mirror of the pinned codemode previewArgs; JavaScript lengths and slices count UTF-16 code units."""
    text = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"))
    units = text.encode("utf-16-le", "surrogatepass")
    if len(units) // 2 <= ARGS_PREVIEW_CHARS:
        return text
    return units[:(ARGS_PREVIEW_CHARS - 3) * 2].decode("utf-16-le", "surrogatepass") + "..."


def script_args_match(arguments, preview) -> bool:
    if not isinstance(preview, str):
        return False
    if script_args_preview(arguments) == preview:
        return True
    try:
        return canonical(json.loads(preview)) == canonical(arguments)
    except (ValueError, TypeError):
        return False


def reconcile_script_cancellations(items: dict, parent: dict) -> None:
    """Pinned pi uses parent/? for aborted calls; require a complete signature/multiplicity match.

    The summary keeps only an argument preview, so long arguments match by the same 197-unit prefix.
    Calls sharing a prefix count as duplicates and stay unreconciled or ambiguous (fail-closed).
    """
    pending = {key: item for key, item in items.items() if item.get("parentToolCallId") == parent["id"]}
    summary = [call for call in parent.get("result", {}).get("details", {}).get("calls", []) if isinstance(call.get("args"), str)]
    cancelled = []
    same = lambda item, call: item["name"] == call.get("name") and script_args_match(item["input"], call["args"])
    def mark(item):
        preview = script_args_preview(item["input"])
        starts = [entry for entry in items.values() if entry.get("parentToolCallId") == parent["id"]
                  and entry["name"] == item["name"] and script_args_preview(entry["input"]) == preview]
        matches = [call for call in summary if same(item, call)]
        item.update(completed=True, status="cancelled", cancellation_evidence=parent["id"],
                    cancellation_match="parent+name+args unique" if len(starts) == len(matches) == 1 else "parent+name+args multiplicity")
    for call in summary:
        ident = call.get("id")
        item = pending.get(ident)
        if item and same(item, call):
            pending.pop(ident)
            if call.get("status") == "cancelled":
                mark(item)
        elif call.get("status") == "cancelled" and ident == parent["id"] + "/?":
            cancelled.append(call)
    for name, preview in {(call.get("name"), call["args"]) for call in cancelled}:
        matches = [item for item in pending.values() if item["name"] == name and script_args_match(item["input"], preview)]
        count = sum(call.get("name") == name and call["args"] == preview for call in cancelled)
        if len(matches) == count:
            for item in matches:
                mark(item)


def cancelled_before_admission(items: list[dict], admissions: list[dict], *, native: bool,
                               enabled_tools: list[str] | tuple[str, ...] = ()) -> dict:
    """Only unique parent/name/args cancellation evidence can explain a missing atomic admission."""
    aliases = {alias: name for name in enabled_tools for alias in navigation_tool_names(name)}
    calls, ambiguous = [], []
    for item in items:
        if item.get("status") != "cancelled":
            continue
        if native and item["name"] == "bash":
            admitted = sum(row.get("command") == bash_command(item) for row in admissions)
            starts = sum(other["name"] == "bash" and bash_command(other) == bash_command(item) for other in items)
        elif not native and item["name"] in aliases:
            admitted = sum(row.get("method") == "tools/call" and row.get("tool") == aliases[item["name"]]
                           and canonical(row.get("arguments")) == canonical(item["input"]) for row in admissions)
            starts = sum(aliases.get(other["name"]) == aliases[item["name"]] and canonical(other["input"]) == canonical(item["input"]) for other in items)
        else:
            continue
        if admitted:
            if admitted < starts:
                ambiguous.append(item["id"])
            continue
        if item.get("cancellation_match") != "parent+name+args unique":
            ambiguous.append(item["id"])
            continue
        calls.append({"id": item["id"], "parentToolCallId": item["parentToolCallId"], "name": item["name"], "arguments": item["input"],
                      "match": item["cancellation_match"], "dispatched": False, "budget_consumed": False})
    return {"calls": calls, "ambiguous": ambiguous}


def bash_command(item: dict) -> object:
    arguments = item.get("input")
    return arguments.get("command") if isinstance(arguments, dict) else None


def bash_admission_check(items: list[dict], admissions: list[dict]) -> dict:
    """Pinned pi validation/truncated-message errors are host-only, not wrapper admissions."""
    from collections import Counter
    cancellations = cancelled_before_admission(items, admissions, native=True)
    excluded = {item["id"] for item in cancellations["calls"]}
    rejected = []
    for item in items:
        result = item.get("result") or {}
        if (item["name"] != "bash" or not item.get("completed") or item.get("missing_start")
                or not result.get("is_error") or "structuredContent" in result):
            continue
        content = result.get("content")
        if not isinstance(content, list) or len(content) != 1 or content[0].get("type") != "text":
            continue
        text = content[0].get("text", "")
        reason = None
        if (text.startswith('Validation failed for tool "bash":\n') and "\n\nReceived arguments:\n" in text
                or text.startswith('Invalid arguments for tool "bash": ')):
            reason = "argument_validation"
        elif text == ('Tool call "bash" was not executed: the response hit the output token limit, '
                      'so its arguments may be truncated. Re-issue the tool call with complete arguments.'):
            reason = "truncated_message"
        if reason:
            excluded.add(item["id"])
            rejected.append({"id": item["id"], "parentToolCallId": item.get("parentToolCallId"), "name": "bash",
                             "arguments": item["input"], "reason": reason, "dispatched": False})
    # Invalid arguments can themselves be arrays/objects; never use them as Counter keys directly.
    expected = Counter(canonical(bash_command(item)) for item in items if item["name"] == "bash" and item["id"] not in excluded)
    actual = Counter(canonical(row["command"]) for row in admissions)
    return {"matched": expected == actual, "cancellations": cancellations, "host_rejected_calls": rejected,
            "excluded_ids": excluded}


def observe_tool_event(items: dict, event: dict) -> None:
    ident = event["toolCallId"]
    if event["type"] == "tool_execution_start":
        items[ident] = {"id": ident, "name": event["toolName"], "input": event.get("args", {}), "completed": False,
                        "parentToolCallId": event.get("parentToolCallId")}
    else:
        item = items.setdefault(ident, {"id": ident, "name": event.get("toolName"), "input": {}, "missing_start": True})
        item.update(completed=True, result={**event.get("result", {}), "is_error": event.get("isError", False)})
        if item["name"] == "codemode":
            reconcile_script_cancellations(items, item)


def tool_items(events: list[dict]) -> list[dict]:
    items = {}
    for event in events:
        if event.get("type") in {"tool_execution_start", "tool_execution_end"}:
            observe_tool_event(items, event)
    return list(items.values())


def assistant_messages(events: list[dict]) -> list[dict]:
    return [e["message"] for e in events if e.get("type") == "message_end" and e.get("message", {}).get("role") == "assistant"]


def final_result(events: list[dict]) -> dict | None:
    settled = [i for i, e in enumerate(events) if e.get("type") == "agent_settled"]
    if not settled or settled[-1] != len(events) - 1 or events[settled[-1]].get("aborted"):
        return None
    messages = assistant_messages(events[:settled[-1]])
    if not messages or messages[-1].get("stopReason") != "stop":
        return None
    last = messages[-1]
    text = "\n".join(c["text"] for c in last.get("content", []) if c.get("type") == "text")
    return {"result": text, "subtype": "success", "is_error": False, "message": last} if text.strip() else None


def valid_model(events: list[dict], expected: str | dict) -> bool:
    if isinstance(expected, dict):
        provider, model = expected["provider"], expected["model"]
    elif "/" in expected:
        provider, model = expected.split("/", 1)
    else:
        model = expected
        provider = "claude-bridge" if expected.startswith("claude-") else "openai-codex"
    messages = assistant_messages(events)
    return bool(messages) and all(m.get("provider") == provider and m.get("model") == model
        and all(m[k] == model for k in ("upstreamModel", "upstream_model", "responseModel", "response_model") if k in m) for m in messages)


class EventState:
    """Incremental pi observations; raw events stay in memory and on the redacted stream."""
    def __init__(self):
        self.events, self.messages, self.raw_usage, self.errors = [], [], [], []
        self.tools, self.active_compaction = {}, 0
        self.observed = dict.fromkeys(("input_tokens", "output_tokens", "cached_input_tokens",
                                       "cache_write_input_tokens", "total_tokens"), 0)
        self.lifecycle_events = []
        self.last_assistant = None
        self.answer = None
        self.failed_attempts = self.retry_attempts = self.unretried_errors = self.usage_records = 0
        self.retry_active = False

    def _usage(self, value, *, failed=False):
        self.raw_usage.append(value)
        if value is None and failed:
            return  # Missing failed-attempt usage contributes zero only to a lower bound.
        if not isinstance(value, dict) or not all(type(value.get(k)) is int and value[k] >= 0
                                                  for k in ("input", "output", "cacheRead", "cacheWrite")):
            self.errors.append("invalid or missing assistant usage")
            return
        self.usage_records += 1
        inputs = value["input"] + value["cacheRead"] + value["cacheWrite"]
        total = inputs + value["output"]
        if value.get("totalTokens") != total:
            self.errors.append("pi totalTokens mismatch")
        for key, amount in zip(self.observed, (inputs, value["output"], value["cacheRead"], value["cacheWrite"], total)):
            self.observed[key] += amount

    def add(self, event: dict):
        self.events.append(event)
        self.answer = None  # A final settlement must be the last record.
        kind = event.get("type", "")
        if kind == "message_end" and event.get("message", {}).get("role") == "assistant":
            message = event["message"]
            self.messages.append(message)
            self.last_assistant = message
            failed = message.get("stopReason") in {"error", "aborted"}
            if failed:
                self.failed_attempts += 1
                if message.get("stopReason") == "error":
                    self.unretried_errors += 1
                else:
                    self.errors.append("aborted assistant response")
            self._usage(message.get("usage"), failed=failed)
        elif kind in {"tool_execution_start", "tool_execution_end"}:
            observe_tool_event(self.tools, event)
        elif kind == "auto_retry_start":
            self.retry_attempts += 1
            if self.unretried_errors != 1 or not self.last_assistant or self.last_assistant.get("stopReason") != "error":
                self.errors.append("auto retry not bound to a failed assistant attempt")
            else:
                self.unretried_errors -= 1
            self.retry_active = True
        elif kind == "auto_retry_end":
            if not self.retry_active or event.get("success") is not True:
                self.errors.append("auto retry did not finish successfully")
            self.retry_active = False
        elif kind == "compaction_start":
            self.active_compaction += 1
        elif kind == "compaction_end":
            self.active_compaction -= 1
            value = event.get("result", {}).get("usage") if isinstance(event.get("result"), dict) else None
            if value is None:
                self.errors.append("compaction usage unobserved")
            else:
                self._usage(value)
        elif kind == "agent_settled" and not event.get("aborted") and self.last_assistant:
            if self.last_assistant.get("stopReason") == "stop":
                text = "\n".join(c["text"] for c in self.last_assistant.get("content", []) if c.get("type") == "text")
                if text.strip():
                    self.answer = {"result": text, "subtype": "success", "is_error": False, "message": self.last_assistant}
        if "retry" in kind or "compaction" in kind:
            self.lifecycle_events.append(event)

    def usage(self, *, terminal_complete: bool, evidence: str) -> dict:
        errors = list(self.errors)
        if self.active_compaction:
            errors.append("unfinished compaction usage")
        if not terminal_complete or self.answer is None:
            errors.append("unfinished process or unsettled answer")
        if not self.messages or not self.usage_records:
            errors.append("assistant usage unobserved")
        if self.unretried_errors or self.retry_active:
            errors.append("failed response without completed auto retry")
        if any(not item["completed"] or item.get("missing_start") for item in self.tools.values()):
            errors.append("unfinished tool inventory")
        lower_bound = not errors and self.failed_attempts > 0
        complete = not errors and not lower_bound
        accounting = "complete" if complete else "lower_bound" if lower_bound else "incomplete"
        reasons = errors or (["failed auto-retry attempts may omit consumed usage"] if lower_bound else [])
        reason = "; ".join(reasons) or None
        result = {"complete": complete, "token_accounting": accounting,
                  "retry_attempts": self.retry_attempts, "failed_attempts": self.failed_attempts, "errors": errors, "observed": dict(self.observed), "raw_usage": self.raw_usage,
                  "response_ids": [m.get("responseId", f"message_end:{i}") for i, m in enumerate(self.messages)],
                  "consistency": {"status": "valid" if complete else accounting, "errors": errors, "evidence": [evidence]},
                  "cost_completeness": {"complete": complete, "status": accounting, "reasons": reasons},
                  "components": {}}
        for key, amount in self.observed.items():
            result[key] = metric(amount if complete or lower_bound else None, "tokens", reason=reason, evidence=[evidence])
            if lower_bound:
                result[key]["validity"] = "lower_bound"
            result["components"][key] = {**result[key], "observed": amount, "missing_reason": reason}
        result["components"]["reasoning_output_tokens"] = {"value": None, "observed": None, "unit": "tokens", "missing_reason": "not consistently reported by both pi providers"}
        result["model_responses"] = metric(len(self.raw_usage) if complete or lower_bound else None, "responses", reason=reason, evidence=[evidence])
        result["cached_input_ratio"] = (metric(None, "ratio", reason="ratio unavailable for lower-bound totals", evidence=[evidence])
                                        if lower_bound else ratio(result["cached_input_tokens"]["value"], result["input_tokens"]["value"], evidence=[evidence]))
        result["lifecycle_events"] = self.lifecycle_events
        return result


def usage(events: list[dict], *, terminal_complete: bool, evidence: str) -> dict:
    state = EventState()
    for event in events:
        state.add(event)
    return state.usage(terminal_complete=terminal_complete, evidence=evidence)


def probe(value: dict, folder: Path, denied_paths: list[Path], *, metadata_root: Path | None = None) -> dict:
    """No model/auth requests: execute canaries through the actual shell wrapper."""
    from .isolation import canary_files, hidden_references, kern_procargs_command, process_environment_observation
    from .settings import CACHE, RUNS
    folder.mkdir(parents=True, exist_ok=True)
    wrapper = shell_wrapper(value, folder)
    tmp = private_root(folder) / "tmp"
    (tmp / "truncated-output.txt").write_text("PI_PRIVATE_OUTPUT_CANARY")
    process_table = ("perl -e '$m=pack(\"i4\",1,14,0,0);$l=pack(\"Q\",0);"
                     "exit((syscall(202,$m,3,0,$l,0,0)==0 && unpack(\"Q\",$l)>0) ? 0 : 1)'")
    boundaries = []
    with canary_files(Path(value["source"]), folder / "canaries", metadata_root=metadata_root) as (_, targets):
        cases = [("source", "cat README.md", True), ("rg", "rg --version", True),
                 ("pi_tmp", 'cat "$TMPDIR/truncated-output.txt"', True),
                 ("network", "curl --noproxy '*' --max-time 2 http://127.0.0.1:19849", False),
                 ("source_write", "touch .unauthorized-write", False),
                 ("lsopen", "open -a Finder", False),
                 ("appleevent", 'osascript -e \'tell application "Finder" to count windows\'', False),
                 ("launchctl", "launchctl submit -l codemap-bench-canary-$$ -p /usr/bin/true", False),
                 ("process_list", "ps -ax", False),
                 ("process_table", process_table, False),
                 ("process_environment", shlex.join(kern_procargs_command(os.getpid())), False),
                 ("signal_outside_sandbox", f"kill -0 {os.getpid()}", False),
                 ("signal_child", "sleep 5 & child=$!; kill $child; wait $child; test $? -eq 143", True),
                 ("pipeline_sigpipe", "yes | head -n 1", True),
                 ("private_paths", 'case "$TMPDIR $HOME $PATH" in *route-preparation*|*campaigns*|*codemap*|*bench*) exit 1;; esac', True),
                 ("coreutils", "set -e -o pipefail; bash --version; cat README.md | head -n 1 | grep . | sed -n 1p | awk \'{print}\' | wc -l; ls .; find . -maxdepth 1 -type f", True)]
        cases.extend((name, "cat " + shlex.quote(str(path)), False) for name, path in targets.items())
        for name, text, allowed in cases:
            result = subprocess.run([str(wrapper), "-c", text], cwd=value["source"], capture_output=True, timeout=10)
            boundaries.append({"name": name, "exit_code": result.returncode, "expected_allowed": allowed,
                               "stderr": result.stderr.decode(errors="replace")})
            require((result.returncode == 0) == allowed, f"pi bash boundary 실패: {name}")
            if name == "process_environment":
                boundaries[-1].update(process_environment_observation(result))
    result = subprocess.run([str(wrapper), "-c", "env"], cwd=value["source"],
                            env={**os.environ, TOKEN_ENV: "STUB_SECRET", "PI_PARENT_SECRET": "STUB_SECRET"}, capture_output=True, timeout=10)
    require(result.returncode == 0 and b"STUB_SECRET" not in result.stdout and b"PI_" not in result.stdout,
            "pi bash 환경 누출")
    boundaries.append({"name": "env_allowlist", "stdout": result.stdout.decode()})
    host = capabilities()
    write_json(folder / "inventory.json", {"backend": BACKEND, "configuration": value, "cli": host})
    write_json(folder / "probe.json", {"boundaries": boundaries, "model_invocations": 0,
        "references": [{"path": str(p), "exists": p.exists(), "kind": "existence_only"} for p in [*denied_paths, *hidden_references(), CACHE / "plans", CACHE / "runs", CACHE / "grading-contract", RUNS]],
        "configuration": value, "cli": host})
    return {"native_inventory": str(folder / "inventory.json"), "native_inventory_sha256": file_digest(folder / "inventory.json"),
            "native_agent": value, "relay_settings": None, "tool_condition": "native_agent_baseline", "auxiliary_necessity": [],
            "probes": boundaries, "probe_log": str(folder / "probe.json"), "probe_log_sha256": file_digest(folder / "probe.json"),
            "cleanup": {"frontend": {"complete": True}, "daemon": {"shared_daemon": False}}}
