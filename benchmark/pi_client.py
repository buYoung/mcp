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

from .core import digest, file_digest, metric, ratio, read_json, require, write_json

BACKEND = "pi"
TOKEN_ENV = "CODEMAP_BENCH_PI_TOKEN"
NATIVE_TOOLS = ("bash",)
PRIVATE_FLAGS = ["--mode", "json", "-ne", "-ns", "-np", "-nc", "--no-themes", "-na", "--offline"]
NAVIGATION_SERVER = "navigation"
# Client delivery must not cut below finite native budgets; uncapped products use 1 MiB.
# Serena counts Unicode characters: at most four UTF-8 bytes per character.
OUTPUT_GUARD_BYTES = {"codemap-search": 100000, "serena": 150000 * 4}
ADAPTER_SETTINGS = {"semanticSearch": False, "allowInstall": False, "allowScriptToolSave": False, "autoAuth": False,
                    "sampling": False, "elicitation": False, "requestTimeoutMs": 300000,
                    "outputGuard": {"maxBytes": 1024 * 1024, "maxLines": 1024 * 1024 + 1}}
# The warm-host shim accepts one connection. "keep-alive" also connects at extension load and again for the
# session, so the second connection fails; "lazy-keep-alive" connects once at session start and stays connected.
# Tools are declared directly instead of through `mcp`.
ADAPTER_SERVER = {"lifecycle": "lazy-keep-alive", "directTools": True}


def adapter_settings(arm: str) -> dict:
    maximum = OUTPUT_GUARD_BYTES.get(arm, ADAPTER_SETTINGS["outputGuard"]["maxBytes"])
    return {**ADAPTER_SETTINGS, "outputGuard": {"maxBytes": maximum, "maxLines": maximum + 1}}


def navigation_tool_names(name: str) -> set[str]:
    # Deferred adapter tools use the same server/relay as direct aliases.
    return {navigation_tool_name(name), f"mcp__{NAVIGATION_SERVER}__{name}"}


def navigation_tool_name(name: str) -> str:
    """pi-mcp-adapter 7.0.0 formatToolName with the default "server" prefix."""
    sanitized, prefix = name.replace(".", "_"), NAVIGATION_SERVER + "_"
    return sanitized if sanitized.startswith(prefix) and len(sanitized) > len(prefix) else prefix + sanitized


def server_instructions_prompt(instructions: str) -> str:
    # Same placement and shape as Claude Code's MCP server instructions section; the text is the product's own.
    return ("# MCP Server Instructions\n\nThe following MCP servers have provided instructions for how to use "
            f"their tools and resources:\n\n## {NAVIGATION_SERVER}\n{instructions}\n")


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
    if local_endpoint is not None or local_home is not None:
        require(provider == "claude-bridge" and local_endpoint is not None and local_home is not None,
                "Claude local transport requires both loopback endpoint and private HOME")
        home = local_home.resolve()
        require(home != Path(env["HOME"]).resolve() and not local_home.is_symlink(), "local Claude HOME must be private")
        home.mkdir(parents=True, exist_ok=True, mode=0o700)
        env.update(HOME=str(home), ANTHROPIC_BASE_URL=local_endpoint_url(local_endpoint),
                   ANTHROPIC_API_KEY="local-model-free-dummy-key")
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


def capabilities() -> dict:
    from .tool_artifacts import pi_toolchain
    import tempfile
    from .settings import CACHE
    tools = pi_toolchain()
    with tempfile.TemporaryDirectory(prefix="pi-capabilities-", dir=CACHE) as directory:
        root = Path(directory)
        agent = root / "agent"
        agent.mkdir(mode=0o700)
        write_models(agent, "openai-codex")
        env = environment(agent_dir=agent, tmp=root, provider="openai-codex", token="model-free-dummy")
        probes = {}
        adapter_config = root / "mcp-adapter.json"
        write_json(adapter_config, {"settings": ADAPTER_SETTINGS, "mcpServers": {}})
        for name, args in (("help", [*PRIVATE_FLAGS, "--help"]),
                           ("models", [*PRIVATE_FLAGS, "-e", tools["bridge_extension"], "--list-models"]),
                           ("mcp_adapter_help", [*PRIVATE_FLAGS, "-e", tools["mcp_adapter_extension"],
                                                 "--mcp-config", str(adapter_config), "--help"]),
                           ("private_auth_reference", ["auth", "check", "--provider", "openai-codex"])):
            result = subprocess.run([tools["node"], tools["pi_cli"], *args], cwd=root,
                                    env={**env, "PI_MCP_CONFIG_MODE": "exclusive"} if name == "mcp_adapter_help" else env,
                                    capture_output=True, text=True, timeout=60)
            require(result.returncode == 0, f"model-free pi probe 실패: {name}")
            # The temporary config path differs per call; the recorded argv must stay comparable.
            recorded = ["<temporary>/mcp-adapter.json" if arg == str(adapter_config) else arg for arg in args]
            probes[name] = {"args": recorded, "exit_code": result.returncode, "output": result.stdout + result.stderr}
        help_text = probes["help"]["output"]
        models = {tuple(line.split()[:2]) for line in probes["models"]["output"].splitlines()}
        require({("openai-codex", "gpt-6.1-sol"), ("openai-codex", "gpt-6-astra"),
                 ("claude-bridge", "claude-opus-5-5")} <= models, "pinned pi 모델 목록 누락")
        require(probes["private_auth_reference"]["output"].strip() == "ready", "private pi auth reference 오류")
        require("--mcp-config" in probes["mcp_adapter_help"]["output"], "pi-mcp-adapter 로드 실패")
    flags = ["--no-tools", "--no-mcp", "--no-extensions", "--no-approve", "--offline", "--session-dir", "--thinking"]
    require(all(flag in help_text for flag in flags), "pi 실행 옵션 누락")
    return {"backend": BACKEND, "version": tools["versions"]["pi"], "versions": tools["versions"],
            "lock_sha256": tools["lock_sha256"], "toolchain_sha256": digest(tools),
            "node_path": tools["node"], "node_sha256": tools["node_sha256"], "required_flags": flags,
            "auth": "runtime only; operator token/Claude login not exercised during preparation",
            "model_free_probes": probes, "model_invocations": 0,
            "defaults": {"compaction": "pi default", "retry": "pi default", "output_truncation": "pi default",
                         "mcp_client": f"pi-mcp-adapter {tools['versions']['mcp_adapter']} defaults except {sorted(ADAPTER_SETTINGS)}",
                         "mcp_output_guard": {"default": ADAPTER_SETTINGS["outputGuard"], "per_arm_bytes": OUTPUT_GUARD_BYTES}}}


def configuration(source: Path, rg_binary: Path) -> dict:
    return {"schema": "grafana-native-agent-v1", "backend": BACKEND, "mode": "native_agent",
            "source": str(source.resolve()), "rg_binary": str(rg_binary.resolve()), "rg_sha256": file_digest(rg_binary),
            "search_product_mcp": False, "tools": ["bash"], "native_tools": "pi bash via per-slot Seatbelt wrapper",
            "call_unit": "atomic wrapper admission; 80th allowed, 81st rejected before exec",
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
    # Other processes' names and arguments (products, harness, other slots) are not exploration targets.
    text += "(deny process-info* (target others))\n(deny sysctl-read (sysctl-name-prefix \"kern.proc\"))\n"
    for name in sorted(METADATA | {".pi", ".claude"}):
        text += f"(deny file-read-data file-write* (subpath {json.dumps(str(source / name))}))\n"
    profile = folder / "pi-bash.sb"
    profile.write_text(text)
    env = {"PATH": str(bin_dir) + ":/usr/bin:/bin", "HOME": str(home), "TMPDIR": str(tmp), "LANG": "en_US.UTF-8"}
    return {"profile_path": str(profile), "profile_sha256": file_digest(profile), "env": env, "source": str(source),
            "allowed": {"read": [str(source), str(rg), str(tmp), str(home)], "write": [str(tmp)], "network": False,
                        "process_info_others": False}}


def shell_wrapper(native: dict, folder: Path) -> Path:
    profile = baseline_boundary(native, folder)
    config = folder / "pi-bash.json"
    (folder / "bash-calls.jsonl").touch(exist_ok=False)
    (folder / "bash-results.jsonl").touch(exist_ok=False)
    write_json(config, {"folder": str(folder), "profile": profile, "limit": 80}, exclusive=True)
    wrapper = folder / "pi-shell"
    wrapper.write_text(f"#!{sys.executable}\nimport sys\nsys.path.insert(0, {str(Path(__file__).resolve().parent.parent)!r})\n"
                       f"from benchmark.pi_client import shell_main\nshell_main({str(config)!r}, sys.argv[1:])\n")
    wrapper.chmod(0o700)
    return wrapper


def shell_main(config_path: str, args: list[str]) -> None:
    """Trusted unsandboxed launcher. Admission and stop share the relay's atomic lock."""
    from .route_transport import navigation_lock, latch_cutoff, append_jsonl
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
        process = read_json(folder / "process.json") if (folder / "process.json").exists() else {}
        expired = time.monotonic() >= process.get("wall_deadline_monotonic", float("inf"))
        allowed = not stopped and not expired and ident <= config["limit"]
        append_jsonl(log, {"id": ident, "tool": "bash", "command": redact(args[1], os.environ.get(TOKEN_ENV)), "dispatched": allowed,
                           "pid": os.getpid(), "pgid": os.getpgrp(), "start_fingerprint": ProcessRegistry.fingerprint(os.getpid()),
                           "status": "admitted" if allowed else "rejected", "monotonic": time.monotonic()})
        if not allowed and not stopped:
            latch_cutoff(folder, "timeout" if expired else "call_limit", {},
                         start=process.get("started_monotonic", time.monotonic()),
                         started_unix_seconds=process.get("started_unix_seconds"),
                         wall_deadline=process.get("wall_deadline_monotonic"))
    if not allowed:
        print("Exploration call rejected before execution: 80-call budget or solver cutoff.", file=sys.stderr)
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


def launch(folder: Path, spec: dict, relay_settings: dict | None, *, server: dict | None = None,
           local_endpoint: str | None = None, local_home: Path | None = None) -> tuple[list[str], Path, dict]:
    from .tool_artifacts import pi_toolchain
    tools = pi_toolchain()
    require(tools["versions"]["mcp_adapter"] == "7.0.0", "pi-mcp-adapter 7.0.0 lock required")
    folder = folder.resolve()
    native = spec.get("native_agent")
    workspace = private_root(folder) / "work"
    require(not workspace.is_symlink(), "pi cwd symlink 금지")
    workspace.mkdir(parents=True, exist_ok=True, mode=0o700)
    require(not native or native.get("backend") == BACKEND and not relay_settings, "pi 기준군 설정 오류")
    agent, tmp, sessions = folder / "pi-agent", private_root(folder) / "tmp", folder / "pi-session"
    for path in (agent, tmp, sessions):
        require(not path.is_symlink() and not path.resolve().is_relative_to(workspace.resolve()), "private pi state must be outside workspace and not a symlink")
        path.mkdir(parents=True, exist_ok=True, mode=0o700)
        path.chmod(0o700)
    provider = spec["provider"]
    settings = {"shellPath": str(shell_wrapper(native, folder))} if native else {}
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
        # Built-in is replaceable; keep pi's implementation, disabling only out-of-budget model APIs.
        extension = agent / "script-tools.mjs"
        factory = Path(tools["pi_cli"]).parent / "extensions/codemode/index.js"
        extension.write_text("import {createCodemodeExtension} from " + json.dumps(str(factory)) +
                             ";\nexport default createCodemodeExtension({models:false});\n")
        args += ["-e", str(extension)]
    instructions = None
    if relay_settings:
        require(server is not None, "pi MCP requires warm-host attachment")
        adapter_config = agent / "mcp-adapter.json"
        write_json(adapter_config, {"settings": adapter_settings(relay_settings["arm"]), "mcpServers": {NAVIGATION_SERVER: {
            **{k: server[k] for k in ("command", "args", "env")}, **ADAPTER_SERVER}}}, exclusive=True)
        product_tools = [navigation_tool_name(name) for name in relay_settings["enabled_tools"]]
        require(len(set(product_tools)) == len(product_tools), "navigation 도구 이름 충돌")
        args += ["-e", tools["mcp_adapter_extension"], "--mcp-config", str(adapter_config), "--tools", ",".join([*product_tools, *(["codemode"] if mode == "codemode" else [])])]
        instructions = server.get("instructions") or ""
        require(isinstance(instructions, str), "server instructions 형식 오류")
        if instructions.strip():
            prompt_file = agent / "server-instructions.md"
            prompt_file.write_text(server_instructions_prompt(instructions), encoding="utf-8")
            args += ["--append-system-prompt", str(prompt_file)]
    if provider == "claude-bridge":
        from . import claude_settings
        claude_settings.verify(spec.get("claude_settings"), workspace, home=local_home)
        require(spec.get("requested_model") == requested_model(provider, spec["model"]), "Claude requested model condition changed")
        # Bridge 0.13.1 reads this file independently of pi's -na project policy.
        require(not (workspace / ".pi/claude-bridge.json").exists(), "project bridge config would override the pinned binary")
        write_json(agent / "claude-bridge.json", {"provider": {"pathToClaudeCodeExecutable": tools["claude_code_binary"]}}, exclusive=True)
        args += ["-e", tools["bridge_extension"]]
    if not relay_settings:
        args += ["--no-mcp", "--tools", "bash,codemode" if mode == "codemode" else "bash"] if native else ["--no-mcp", "--no-tools"]
    # A loopback-only test launch must never touch operator credentials.
    if provider == "openai-codex" and local_endpoint is not None:
        from .pi_audit import dummy_token
        token = dummy_token()
    else:
        token = operator_token(provider) if provider == "openai-codex" else None
    env = environment(agent_dir=agent, tmp=tmp, provider=provider, token=token,
                      local_endpoint=local_endpoint if provider == "claude-bridge" else None, local_home=local_home)
    if relay_settings:
        env["PI_MCP_CONFIG_MODE"] = "exclusive"  # Only --mcp-config; no user, project, host or plugin MCP sources.
        # With no cached metadata, a cold MCP_DIRECT_TOOLS selection makes session start wait for the connection, so
        # the first model request already declares the tools.
        env["MCP_DIRECT_TOOLS"] = NAVIGATION_SERVER
    validate_models(agent, provider, local_endpoint=model_endpoint)
    write_json(folder / "pi-policy.json", {"toolchain": tools, "provider": provider, "flags": PRIVATE_FLAGS,
        "requested_model": requested_model(provider, spec["model"]), "measurement_mode": mode,
        "codemode_models": False if mode == "codemode" else None,
        "mcp_client": ({"extension": "pi-mcp-adapter", "version": tools["versions"]["mcp_adapter"], "config_mode": "exclusive",
                        "settings": adapter_settings(relay_settings["arm"]), "server": ADAPTER_SERVER, "tools": product_tools,
                        "server_instructions_sha256": digest(instructions) if instructions else None,
                        "server_instructions": "appended to the system prompt" if instructions and instructions.strip() else "none provided"}
                       if relay_settings else None),
        "claude_settings": spec.get("claude_settings"),
        "defaults": capabilities()["defaults"], "auth": "memory-only bearer env" if token else "Claude Code existing login, real HOME",
        "claude_residue": "only exact workspace project slug removed; ~/.claude.json project entries may remain"}, exclusive=True)
    return args, workspace, env


def redact(data: bytes | str, token: str | None) -> bytes | str:
    if not token:
        return data
    variants = {token, json.dumps(token)[1:-1]}
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
    paths = [p for p in folder.iterdir() if p.suffix in {".json", ".jsonl", ".txt", ".log"}]
    scratch = private_root(folder)
    for root in (folder / "pi-session", folder / "pi-agent", scratch / "tmp", scratch / "home"):
        paths.extend(root.rglob("*") if root.exists() else ())
    for path in paths:
        if path.is_file() and not path.is_symlink():
            original = path.read_bytes()
            clean = redact(original, token)
            if clean != original:
                path.write_bytes(clean)
    removed = None
    if provider == "claude-bridge":
        real = unicodedata.normalize("NFC", str(workspace.resolve()))
        slug = claude_project_slug(workspace)
        projects = (home or Path(pwd.getpwuid(os.getuid()).pw_dir)).resolve() / ".claude/projects"
        target = projects / slug
        require(target.name == slug and target.parent == projects and not target.is_symlink(), "Claude residue 경로 오류")
        if target.exists():
            # A slug is lossy. Refuse deletion unless every session header observed binds to this cwd.
            files = list(target.glob("*.jsonl"))
            require(bool(files), "Claude residue workspace binding 없음")
            for path in files:
                require(not path.is_symlink() and path.stat().st_nlink == 1, "Claude residue session link 금지")
                rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
                cwds = {unicodedata.normalize("NFC", r["cwd"]) for r in rows if isinstance(r.get("cwd"), str)}
                require(cwds == {real}, "Claude residue workspace binding 불일치")
            shutil.rmtree(target)
            removed = str(target)
    return {"complete": True, "removed_project": removed, "claude_json_modified_by_harness": False}


def tool_items(events: list[dict]) -> list[dict]:
    items = {}
    for event in events:
        if event.get("type") == "tool_execution_start":
            ident = event["toolCallId"]
            items[ident] = {"id": ident, "name": event["toolName"], "input": event.get("args", {}), "completed": False}
        elif event.get("type") == "tool_execution_end":
            ident = event["toolCallId"]
            item = items.setdefault(ident, {"id": ident, "name": event.get("toolName"), "input": {}, "missing_start": True})
            item.update(completed=True, result={**event.get("result", {}), "is_error": event.get("isError", False)})
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
        elif kind == "tool_execution_start":
            self.tools[event["toolCallId"]] = {"id": event["toolCallId"], "name": event["toolName"],
                                                "input": event.get("args", {}), "completed": False}
        elif kind == "tool_execution_end":
            ident = event["toolCallId"]
            item = self.tools.setdefault(ident, {"id": ident, "name": event.get("toolName"), "input": {}, "missing_start": True})
            item.update(completed=True, result={**event.get("result", {}), "is_error": event.get("isError", False)})
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


def probe(value: dict, folder: Path, denied_paths: list[Path]) -> dict:
    """No model/auth requests: execute canaries through the actual shell wrapper."""
    from .isolation import canary_files, hidden_references
    from .settings import CACHE, RUNS
    folder.mkdir(parents=True, exist_ok=True)
    wrapper = shell_wrapper(value, folder)
    tmp = private_root(folder) / "tmp"
    (tmp / "truncated-output.txt").write_text("PI_PRIVATE_OUTPUT_CANARY")
    process_table = ("perl -e '$m=pack(\"i4\",1,14,0,0);$l=pack(\"Q\",0);"
                     "exit((syscall(202,$m,3,0,$l,0,0)==0 && unpack(\"Q\",$l)>0) ? 0 : 1)'")
    boundaries = []
    with canary_files(Path(value["source"]), folder / "canaries") as (_, targets):
        cases = [("source", "cat README.md", True), ("rg", "rg --version", True),
                 ("pi_tmp", 'cat "$TMPDIR/truncated-output.txt"', True),
                 ("network", "curl --noproxy '*' --max-time 2 http://127.0.0.1:19849", False),
                 ("source_write", "touch .unauthorized-write", False),
                 ("lsopen", "open -a Finder", False),
                 ("appleevent", 'osascript -e \'tell application "Finder" to count windows\'', False),
                 ("launchctl", "launchctl submit -l codemap-bench-canary-$$ -p /usr/bin/true", False),
                 ("process_list", "ps -ax", False),
                 ("process_table", process_table, False),
                 ("private_paths", 'case "$TMPDIR $HOME $PATH" in *route-preparation*|*campaigns*|*codemap*|*bench*) exit 1;; esac', True),
                 ("coreutils", "set -e -o pipefail; bash --version; cat README.md | head -n 1 | grep . | sed -n 1p | awk \'{print}\' | wc -l; ls .; find . -maxdepth 1 -type f", True)]
        cases.extend((name, "cat " + shlex.quote(str(path)), False) for name, path in targets.items())
        for name, text, allowed in cases:
            result = subprocess.run([str(wrapper), "-c", text], cwd=value["source"], capture_output=True, timeout=10)
            boundaries.append({"name": name, "exit_code": result.returncode, "expected_allowed": allowed,
                               "stderr": result.stderr.decode(errors="replace")})
            require((result.returncode == 0) == allowed, f"pi bash boundary 실패: {name}")
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
