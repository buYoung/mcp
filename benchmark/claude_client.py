"""Claude Code launch policy and native stream observations; no API requests at import."""
from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

from .core import file_digest, metric, ratio, read_json, require, write_json

BACKEND = "claude_code"
NATIVE_TOOLS = ("Bash", "Read", "Glob", "Grep")
RESOURCE_TOOLS = ("ListMcpResourcesTool", "ReadMcpResourceTool")
_ENV_TOKEN = os.environ.pop("CLAUDE_CODE_OAUTH_TOKEN", None)


def operator_token() -> tuple[str | None, str]:
    """An explicitly supplied token is held in memory only; never log its value or digest."""
    global _ENV_TOKEN
    supplied = os.environ.pop("CLAUDE_CODE_OAUTH_TOKEN", None)
    if supplied is not None:
        _ENV_TOKEN = supplied
    token = _ENV_TOKEN
    filename = os.environ.get("CODEMAP_BENCH_CLAUDE_TOKEN_FILE")
    require(not (token and filename), "provide only one Claude token source")
    if filename:
        import stat
        try:
            with os.fdopen(os.open(Path(filename).expanduser(), os.O_RDONLY | os.O_NOFOLLOW)) as stream:
                info = os.fstat(stream.fileno())
                require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and stat.S_IMODE(info.st_mode) == 0o600
                        and info.st_uid == os.getuid(), "Claude token file must be an owned 0600 regular file, not a link")
                require(info.st_size <= 16384, "Claude token file too large")
                token = stream.read(16385).strip()
        except (OSError, UnicodeError):
            raise ValueError("Claude token file could not be read (details withheld)") from None
    if token is not None:
        require(bool(token) and not any(c.isspace() for c in token), "invalid Claude token format")
    return token, "oauth_file" if filename else "oauth_env" if token else "missing"


def require_operator_auth() -> None:
    token, _ = operator_token()
    require(bool(token), "All Claude arms require operator CLAUDE_CODE_OAUTH_TOKEN or CODEMAP_BENCH_CLAUDE_TOKEN_FILE (owned 0600); user login is disabled")


def environment(*, local=False) -> dict:
    # HOME is supplied by each private launch; no user-login or model override inheritance.
    allowed = {"PATH", "USER", "LOGNAME", "SHELL", "LANG", "LC_ALL", "SSL_CERT_FILE", "SSL_CERT_DIR"}
    env = {key: value for key, value in os.environ.items() if key in allowed}
    token, _ = (None, "local_stub") if local else operator_token()
    if token:
        env["CLAUDE_CODE_OAUTH_TOKEN"] = token
    from .settings import CACHE
    env.update(CODEMAP_BENCH_CACHE=str(CACHE), MCP_TOOL_TIMEOUT="300000", MCP_TIMEOUT="120000")
    env.update({key: "1" for key in (
        "CLAUDE_CODE_DISABLE_CLAUDE_MDS", "CLAUDE_CODE_DISABLE_AUTO_MEMORY", "CLAUDE_CODE_DISABLE_BACKGROUND_TASKS",
        "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL",
        "CLAUDE_CODE_DISABLE_TERMINAL_TITLE", "CLAUDE_CODE_DISABLE_BUNDLED_SKILLS",
        "CLAUDE_CODE_DISABLE_FILE_MENTIONS", "CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS", "DISABLE_AUTOUPDATER") })
    return env


def _logged_in(result: subprocess.CompletedProcess) -> bool:
    try:
        status = json.loads(result.stdout)
    except (ValueError, TypeError):
        return False
    return result.returncode == 0 and isinstance(status, dict) and status.get("loggedIn") is True


def executable() -> str:
    from .tool_artifacts import cli_toolchain
    return cli_toolchain()["claude_code"]


def managed_policy() -> dict:
    """CLI flags cannot override admin policy. Refuse it by existence without reading any values."""
    import pwd
    from .claude_settings import MANAGED_ROOT, MANAGED_PREFERENCES
    paths = [MANAGED_ROOT / "managed-settings.json", MANAGED_PREFERENCES / "com.anthropic.claudecode.plist",
             MANAGED_PREFERENCES / pwd.getpwuid(os.getuid()).pw_name / "com.anthropic.claudecode.plist"]
    dropins = MANAGED_ROOT / "managed-settings.d"
    if dropins.exists():
        paths.extend(p for p in dropins.iterdir() if p.suffix == ".json")
    require(not any(p.exists() or p.is_symlink() for p in paths), "Claude managed policy exists; independent review required")
    return {"managed_files_absent": sorted(map(str, paths)), "setting_sources": [], "restricted": True}


def capabilities(*, check_auth=True) -> dict:
    import tempfile
    from .isolation import private_environment
    from .settings import CACHE
    before = operator_home_state()
    binary = executable()
    with tempfile.TemporaryDirectory(dir=CACHE, prefix="") as directory:
        env = {**environment(local=True), **private_environment(Path(directory) / "home")}
        version = subprocess.run([binary, "--version"], cwd=directory, env=env, capture_output=True, text=True, check=True, timeout=30).stdout.strip()
        require(version == "2.1.311 (Claude Code)", "Claude Code pin mismatch")
        help_text = subprocess.run([binary, "--help"], cwd=directory, env=env, capture_output=True, text=True, check=True, timeout=30).stdout
        if check_auth:
            require_operator_auth()
            env.update(environment())
            login = subprocess.run([binary, "auth", "status"], cwd=directory, env=env, capture_output=True, timeout=30)
            require(_logged_in(login), "Claude operator OAuth login not recognized in private HOME")
    require(operator_home_observation(before, [Path(directory), Path(directory) / "home"])["passed"],
            "operator Claude HOME changed during capability probe")
    flags = ("--restricted", "--model", "--effort", "--tools", "--settings", "--setting-sources",
             "--strict-mcp-config", "--mcp-config", "--disable-slash-commands", "--include-partial-messages",
             "--output-format", "--system-prompt-snapshot")
    require(all(flag in help_text for flag in flags), "Claude Code에 필요한 실행 옵션이 없습니다")
    return {"backend": BACKEND, "version": version, "required_flags": list(flags)}


def configuration(source: Path, rg_binary: Path) -> dict:
    from .native_baseline import pinned_rg
    source, rg_binary = source.resolve(), pinned_rg(source, rg_binary)
    search_path = str(rg_binary.parent) + os.pathsep + "/usr/bin:/bin"
    # One outer Seatbelt fences the CLI and all native tools. Nested Bash Seatbelt is impossible.
    return {"schema": "grafana-native-agent-v1", "backend": BACKEND, "mode": "native_agent", "source": str(source),
            "rg_binary": str(rg_binary), "rg_sha256": file_digest(rg_binary), "search_product_mcp": False,
            "env": {"PATH": search_path}, "tools": list(NATIVE_TOOLS),
            "config": {"permissions": {"defaultMode": "dontAsk", "blockReadsOutsideWorkingDirectories": True,
                                        "deny": ["Edit", "Write", "NotebookEdit", "WebFetch", "WebSearch", "Agent", "Cd"]},
                       "sandbox": {"enabled": False},
                       "env": {"PATH": search_path}},
            "native_tools": "Claude Code native Bash/Read/Glob/Grep; no search product MCP",
            "call_unit": "native exploration tool_use ID; a compound Bash command is one call",
            "output_policy": "native tool and client defaults; no harness clipping"}


def launch(folder: Path, spec: dict, relay_settings: dict | None, *, server: dict | None = None,
           local_endpoint: str | None = None, local_home: Path | None = None) -> tuple[list[str], Path, dict]:
    from .pi_client import private_root, local_endpoint_url
    root = private_root(folder)
    if local_endpoint is None:
        require_operator_auth()
    current_policy = managed_policy()
    require(spec.get("claude_settings", current_policy) == current_policy, "Claude policy changed since preparation")
    native = spec.get("native_agent")
    workspace = Path(native["source"]) if native else root / "work"
    workspace.mkdir(parents=True, exist_ok=True)
    require(not native or native.get("backend") == BACKEND and not relay_settings, "Claude 기준군 설정 오류")
    policy = {"disableAllHooks": True, "autoMemoryEnabled": False, "enabledPlugins": {},
              "permissions": {"defaultMode": "dontAsk", "blockReadsOutsideWorkingDirectories": True}}
    if native:
        require(file_digest(Path(native["rg_binary"])) == native["rg_sha256"], "rg copy drift")
        policy.update(native["config"])
    mcp = {"mcpServers": {}}
    permitted = []
    if relay_settings:
        from .runner import navigation_server
        server = server if server is not None else navigation_server(folder, relay_settings)
        mcp["mcpServers"]["navigation"] = {key: server[key] for key in ("command", "args", "env")}
        permitted = [*RESOURCE_TOOLS, *("mcp__navigation__" + name for name in relay_settings["enabled_tools"])]
    policy_path, mcp_path = folder / "claude-settings.json", folder / "mcp-config.json"
    write_json(policy_path, policy, exclusive=True)
    write_json(mcp_path, mcp, exclusive=True)
    args = [executable(), "--print", "--verbose", "--output-format", "stream-json",
            "--include-partial-messages", "--model", spec["model"], "--effort", spec["reasoning_effort"],
            "--restricted", "--setting-sources", "", "--settings", str(policy_path),
            "--strict-mcp-config", "--mcp-config", str(mcp_path), "--disable-slash-commands",
            "--system-prompt-snapshot", "on", "--permission-mode", "dontAsk", "--tools", ",".join(NATIVE_TOOLS if native else RESOURCE_TOOLS if relay_settings else ())]
    if native or permitted:
        args += ["--allowedTools", ",".join(NATIVE_TOOLS if native else permitted)]
    home = root / "home"
    require(local_home is None or local_endpoint is not None and local_home == home, "local HOME must equal the slot private HOME")
    require(not home.exists() and not home.is_symlink(), "pre-existing Claude HOME: refuse unrelated residue cleanup")
    from .isolation import private_environment
    env = {**environment(local=local_endpoint is not None), **private_environment(home)}
    env.update(CLAUDE_CODE_TMPDIR=env["TMPDIR"], ENABLE_TOOL_SEARCH="false")
    if relay_settings:
        from .client_delivery import output_limits
        env["MAX_MCP_OUTPUT_TOKENS"] = str(output_limits(relay_settings["arm"])["claude_tokens"])
    if local_endpoint is not None:
        env.update(ANTHROPIC_BASE_URL=local_endpoint_url(local_endpoint), ANTHROPIC_API_KEY="local-model-free-dummy-key")
    if native:
        from .isolation import claude_baseline_command, claude_baseline_profile
        env.update(native["env"])
        profile = claude_baseline_profile(native, folder, local_endpoint=local_endpoint, local_home=home)
        env.update(profile["env"])
        env["CLAUDE_CODE_SHELL"] = native_shell(folder, native, env)
        args = claude_baseline_command(args, profile, folder)
    env.update(MCP_TOOL_TIMEOUT="300000", MCP_TIMEOUT="120000")
    prepare_private(folder, workspace, env, source=Path(relay_settings["source"]) if relay_settings and relay_settings.get("source") else workspace)
    write_json(folder / "cli-policy.json", {"backend": BACKEND, "measurement_mode": "direct",
        "mcp_output_tokens": int(env["MAX_MCP_OUTPUT_TOKENS"]) if relay_settings else None,
        "local_provider": local_endpoint is not None, "private_home": str(home),
        "authentication": "local_stub" if local_endpoint else "operator_oauth"}, exclusive=True)
    return args, workspace, env


def native_shell(folder: Path, native: dict, env: dict) -> str:
    """Keep the native Bash tool, but never give its shell the CLI's authentication environment."""
    from .cli_shell import wrapper
    allowed = {"PATH": native["env"]["PATH"], "HOME": env["HOME"], "TMPDIR": env["TMPDIR"],
               "LANG": env.get("LANG", "en_US.UTF-8"), "LC_ALL": env.get("LC_ALL", "en_US.UTF-8")}
    return wrapper(folder, allowed)


def config_file_stat(home: Path) -> dict | None:
    """Observe changes without opening the operator's .claude.json (which may contain credentials)."""
    path = home / ".claude.json"
    if not path.exists():
        return None
    info = path.stat()
    return {"size": info.st_size, "mtime_ns": info.st_mtime_ns, "inode": info.st_ino}


def operator_home_state() -> dict:
    """Metadata only: never open operator settings, credentials, caches or transcripts."""
    home = Path.home()
    result = {"claude_json": config_file_stat(home), "trees": {}}
    for name in (".claude/projects", ".claude/backups", "Library/Caches/claude-cli-nodejs"):
        root = home / name
        paths = [root, *root.rglob("*")] if root.exists() else []
        result["trees"][name] = {str(path.relative_to(home)): {
            "size": info.st_size, "mtime_ns": info.st_mtime_ns, "inode": info.st_ino}
            for path in paths for info in [path.lstat()]}
    return result


def operator_home_observation(before: dict, harness_paths: list[Path]) -> dict:
    """Harness Claude runs use private HOMEs, so a leak appears under a slug computed from harness paths.

    The parent agent session and the operator's other Claude sessions keep writing the real HOME (new
    transcripts, MCP logs, `.claude.json`, backups). Those are observed, never a reason to void a slot.
    """
    from .pi_client import claude_project_slug
    from .settings import CACHE
    slugs = {claude_project_slug(path) for path in [*harness_paths, CACHE]}
    cache_slug = claude_project_slug(CACHE)
    after = operator_home_state()

    def harness_related(name: str, path: str) -> bool:
        relative = Path(path).relative_to(name).parts
        return (name in {".claude/projects", "Library/Caches/claude-cli-nodejs"} and bool(relative)
                and (relative[0] in slugs or relative[0].startswith(cache_slug + "-")))

    added, removed, changed = [], [], []
    for name, previous in before["trees"].items():
        current = after["trees"][name]
        added += [{"path": path, "harness_related": harness_related(name, path)}
                  for path in sorted(current.keys() - previous.keys())]
        removed += [{"path": path, "harness_related": harness_related(name, path)}
                    for path in sorted(previous.keys() - current.keys())]
        changed += [{"path": path, "before": previous[path], "after": current[path],
                     "harness_related": harness_related(name, path)}
                    for path in sorted(previous.keys() & current.keys()) if previous[path] != current[path]]
    present = sorted({slug for slug in slugs for name in (".claude/projects", "Library/Caches/claude-cli-nodejs")
                      if (Path.home() / name / slug).exists()})
    config_unchanged = before["claude_json"] == after["claude_json"]
    related = [item["path"] for item in (*added, *removed, *changed) if item["harness_related"]]
    return {"passed": not related and not present, "harness_related_paths": related,
            "concurrent_session_changes": {"new": sum(not i["harness_related"] for i in added),
                                           "removed": sum(not i["harness_related"] for i in removed),
                                           "changed": sum(not i["harness_related"] for i in changed),
                                           "claude_json_changed": not config_unchanged},
            "new_paths": added, "removed_paths": removed, "changed_existing": changed,
            "harness_slugs_checked": sorted(slugs), "harness_slugs_present": present,
            "claude_json_before": before["claude_json"], "claude_json_after": after["claude_json"],
            "claude_json_unchanged": config_unchanged}


def private_home_metadata(home: Path) -> dict:
    """Count owned residue without reading config, backups, MCP logs or persisted output."""
    import stat
    result = {"files": 0, "bytes": 0, "categories": {}}
    require(not home.is_symlink(), "Claude private HOME link forbidden")
    for path in home.rglob("*"):
        info = path.lstat()
        require(not stat.S_ISLNK(info.st_mode) and info.st_uid == os.getuid(), "Claude private residue ownership mismatch")
        if stat.S_ISREG(info.st_mode):
            require(info.st_nlink == 1, "Claude private residue hard link forbidden")
            relative = str(path.relative_to(home))
            category = next((prefix for prefix in (".claude/projects", ".claude/backups", "Library/Caches/claude-cli-nodejs", ".claude.json")
                             if relative == prefix or relative.startswith(prefix + "/")), "other")
            counts = result["categories"].setdefault(category, {"files": 0, "bytes": 0})
            for key, value in (("files", 1), ("bytes", info.st_size)):
                counts[key] += value
                result[key] += value
    return result


def prepare_private(folder: Path, workspace: Path, env: dict, *, source: Path | None = None) -> None:
    from .pi_client import claude_project_slug, private_root
    home = Path(env["HOME"]).resolve()
    require(home == private_root(folder) / "home" and home != Path.home().resolve(), "Claude HOME is not slot-owned")
    target = home / ".claude/projects" / claude_project_slug(workspace)
    require(not target.exists() and not target.is_symlink(), "pre-existing Claude project: refuse unrelated residue cleanup")
    write_json(folder / "claude-private.json", {"workspace": str(workspace.resolve()), "home": str(home),
        "project": str(target), "claude_json_before": config_file_stat(home),
        "operator_home_before": operator_home_state(),
        "harness_paths": [str(path.resolve()) for path in (folder, workspace, home, home.parent, source or workspace)]}, exclusive=True)


def cleanup_private(folder: Path, workspace: Path, env: dict) -> dict:
    from .pi_client import cleanup_claude_project as cleanup, private_root, claude_project_slug
    binding = read_json(folder / "claude-private.json")
    home = Path(env["HOME"]).resolve()
    require(binding["workspace"] == str(workspace.resolve()) and binding["home"] == str(home)
            and home == private_root(folder) / "home" and home != Path.home().resolve(), "Claude cleanup binding changed")
    residue = private_home_metadata(home)
    target = Path(binding["project"])
    require(target == home / ".claude/projects" / claude_project_slug(workspace), "Claude project binding changed")
    # Only harness-created, cwd-bound transcripts are copied. Never inspect other operator sessions or credentials.
    for path in sorted(target.glob("*.jsonl")):
        require(not path.is_symlink() and path.stat().st_nlink == 1, "Claude transcript link forbidden")
        rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
        require({r["cwd"] for r in rows if isinstance(r.get("cwd"), str)} == {str(workspace.resolve())}, "Claude transcript cwd mismatch")
        destination = folder / ("claude-session-" + path.name)
        from .pi_client import redact
        destination.write_bytes(redact(path.read_bytes(), env.get("CLAUDE_CODE_OAUTH_TOKEN") or env.get("ANTHROPIC_API_KEY")))
    result = cleanup(workspace, home=home)
    result.update(claude_json_before=binding["claude_json_before"], claude_json_after=config_file_stat(home),
                  claude_json_changed=binding["claude_json_before"] != config_file_stat(home), private_home_residue=residue)
    shutil.rmtree(home)
    observation = operator_home_observation(binding["operator_home_before"], [Path(path) for path in binding["harness_paths"]])
    result.update(removed_home=str(home), private_home_remaining=home.exists(), operator_home_observation=observation)
    result["complete"] = result["complete"] and not home.exists() and observation["passed"]
    return result


def probe(value: dict, folder: Path, denied_paths: list[Path]) -> dict:
    require_operator_auth()
    try:
        return _probe(value, folder, denied_paths)
    finally:
        binding = folder / "claude-private.json"
        if binding.exists() and not (folder / "private-cleanup.json").exists():
            value = read_json(binding)
            write_json(folder / "private-cleanup.json", cleanup_private(folder, Path(value["workspace"]), {"HOME": value["home"]}))


def _probe(value: dict, folder: Path, denied_paths: list[Path]) -> dict:
    """Model-free auth/startup plus real commands under the identical outer boundary."""
    from .settings import execution_settings
    folder.mkdir(parents=True, exist_ok=True)
    host = capabilities()
    spec = execution_settings("claude-opus-5-5")
    spec["native_agent"] = value
    args, workspace, env = launch(folder, spec, None)
    args.append("--init-only")
    from .cli_shell import ShellMonitor
    from .runner import cleanup_solver
    from .pi_client import redact
    monitor = ShellMonitor(folder)
    process = None
    try:
        process = subprocess.Popen(args, cwd=workspace, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True)
        monitor.started(process)
        stdout, stderr = process.communicate(b"", timeout=60)
        result = subprocess.CompletedProcess(args, process.returncode, stdout, stderr)
    finally:
        cleanup = cleanup_solver(process, lambda: None)
        native_cleanup = monitor.close(lambda: None)
        require(cleanup["complete"] and native_cleanup["complete"], "Claude startup cleanup incomplete")
    (folder / "startup.stdout").write_bytes(redact(result.stdout, env.get("CLAUDE_CODE_OAUTH_TOKEN")))
    (folder / "startup.stderr").write_bytes(redact(result.stderr, env.get("CLAUDE_CODE_OAUTH_TOKEN")))
    require(result.returncode == 0, f"Claude 준비 시작 검증 실패: {folder / 'startup.stderr'}")
    from .isolation import canary_files, claude_baseline_command, hidden_references
    profile = read_json(folder / "claude-boundary.json")["profile"]
    auth = subprocess.run(claude_baseline_command([executable(), "auth", "status"], profile, folder),
                          cwd=workspace, env=env, capture_output=True, timeout=60)
    write_json(folder / "auth-probe.json", {"exit_code": auth.returncode, "logged_in": _logged_in(auth), "model_invocations": 0})
    # Keep boundary evidence even when auth is blocked; never publish a ready baseline in that case.
    boundaries = []
    with canary_files(workspace, folder / "boundary") as (_, targets):
        cases = [("source_read", ["/bin/cat", "README.md"], True),
                 ("pinned_rg", [value["rg_binary"], "--version"], True),
                 ("proxy_github_denied", ["/usr/bin/curl", "--fail", "--max-time", "10", "https://github.com"], False),
                 ("direct_github_denied", ["/usr/bin/curl", "--fail", "--noproxy", "*", "--max-time", "10", "https://github.com"], False)]
        proxy_url = f"http://127.0.0.1:{profile['port']}"
        cases += [("bash_proxy_connect_denied", ["/bin/zsh", "-f", "-c",
                   f"exec /usr/bin/curl --fail --max-time 10 --proxy {proxy_url} https://api.anthropic.com/"], False),
                  ("bash_proxy_plain_denied", ["/bin/zsh", "-f", "-c",
                   f"exec /usr/bin/curl --fail --noproxy '*' --max-time 10 {proxy_url}/"], False)]
        cases.extend((name, ["/bin/cat", str(path)], False) for name, path in targets.items())
        cases.extend(("rg_" + name, [value["rg_binary"], "ISOLATION_", str(path)], False)
                     for name, path in targets.items() if name.startswith("claude-"))
        cases.append(("source_read_after_denials", ["/bin/cat", "README.md"], True))
        for name, argv, allowed in cases:
            probe_env = {key: value for key, value in env.items() if key not in {"CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY"}}
            checked = subprocess.run(claude_baseline_command(argv, profile, folder), cwd=workspace, env=probe_env,
                                     capture_output=True, timeout=30)
            boundaries.append({"name": name, "expected_access": "allow" if allowed else "deny",
                               "exit_code": checked.returncode, "stderr": checked.stderr.decode(errors="replace")})
            write_json(folder / "boundary-probe.json", boundaries)
            require(checked.returncode >= 0 and checked.returncode != 125, f"Claude boundary launcher 실패: {name}")
            require((checked.returncode == 0) == allowed, f"Claude outer boundary 실패: {name}")
    private_cleanup = cleanup_private(folder, workspace, env)
    write_json(folder / "private-cleanup.json", private_cleanup)
    require(private_cleanup["complete"], "Claude probe private HOME cleanup incomplete")
    require(_logged_in(auth), "Claude outer profile에서 로그인 인식 실패: baseline blocked; "
            "자격 파일/기록을 허용하지 않습니다. 운영자가 CLAUDE_CODE_OAUTH_TOKEN을 환경으로 제공한 뒤 재검증하세요")
    observation = {"name": "claude_init_only", "exit_code": result.returncode, "command": args,
                   "cli": host, "configuration": value, "model_invocations": 0,
                   "policy_sha256": file_digest(folder / "claude-settings.json"),
                   "references": [{"path": str(p), "exists": p.exists(), "kind": "existence_only"}
                                  for p in dict.fromkeys([*denied_paths, *hidden_references()])],
                   "boundaries": boundaries, "isolation_profile": profile,
                   "auth_status": "verified under outer profile",
                   "stdout_sha256": file_digest(folder / "startup.stdout"), "stderr_sha256": file_digest(folder / "startup.stderr"),
                   "boundary_probe": "CLI auth/init and cat/rg/curl canaries used the identical outer profile; no model/native tool dispatch."}
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
    items, results, non_execution = {}, {}, {}
    for event in events:
        if event.get("parent_tool_use_id"):
            continue
        for meta in event.get("tool_result_meta", []):
            if meta.get("non_execution_kind"):
                non_execution[meta.get("id")] = meta["non_execution_kind"]
        message = event.get("message")
        blocks = message.get("content", []) if isinstance(message, dict) else []
        if event.get("type") == "stream_event" and event.get("event", {}).get("type") == "content_block_start":
            blocks = [event["event"].get("content_block", {})]
        if not isinstance(blocks, list):
            continue
        for block in blocks:
            if block.get("type") == "tool_use" and isinstance(block.get("id"), str):
                items[block["id"]] = {**items.get(block["id"], {}), **block}
            elif block.get("type") == "tool_result" and isinstance(block.get("tool_use_id"), str):
                results[block["tool_use_id"]] = block
    return [{**item, "completed": ident in results, "result": results.get(ident),
             "non_execution_kind": non_execution.get(ident), "dispatched": ident not in non_execution} for ident, item in items.items()]


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
