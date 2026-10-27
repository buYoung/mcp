"""Pinned Codex exec launch, with a loopback-only model-free validation seam."""
from __future__ import annotations

from pathlib import Path

from .core import canonical, command, file_digest, require, write_json


def executable() -> str:
    from .tool_artifacts import cli_toolchain
    return cli_toolchain()["codex"]


def capabilities() -> dict:
    binary = executable()
    version = command([binary, "--version"]).strip()
    require(version == "codex-cli 0.164.0", "Codex pin mismatch")
    help_text = command([binary, "exec", "--help"])
    flags = ("--ignore-user-config", "--ignore-rules", "--skip-git-repo-check", "--json")
    require(all(flag in help_text for flag in flags), "Codex launch flags missing")
    return {"backend": "codex", "version": version, "required_flags": list(flags)}


def launch(folder: Path, spec: dict, relay_settings: dict | None, *, server: dict | None = None,
           local_endpoint: str | None = None) -> tuple[list[str], Path, dict]:
    from .client_delivery import output_limits
    from .pi_client import local_endpoint_url, private_root
    from .runner import codex_options, isolated_environment
    limits = output_limits(relay_settings["arm"] if relay_settings else "rg")
    root = private_root(folder)
    workspace = Path(spec["native_agent"]["source"]) if spec.get("native_agent") else root / "work"
    workspace.mkdir(parents=True, exist_ok=True)
    env, home = isolated_environment(root, link_auth=local_endpoint is None)
    env["TMPDIR"] = str(root / "tmp")
    (root / "tmp").mkdir(exist_ok=True)
    args = [executable(), "exec", "--ignore-user-config", "--ignore-rules", "--skip-git-repo-check", "--json",
            "-C", str(workspace), "-m", spec["model"], *codex_options(settings=spec),
            "-c", "projects={" + canonical(str(workspace)) + '={trust_level="untrusted"}}',
            "-c", "cloud.skills.enabled=false"]
    if not spec.get("native_agent"):
        from .native_baseline import options, PROFILE
        args += options({"config": {"default_permissions": PROFILE,
            f"permissions.{PROFILE}.filesystem": {":root": "deny", ":minimal": "read", str(workspace): "read"},
            f"permissions.{PROFILE}.network.enabled": False}})
    else:
        require(file_digest(Path(spec["native_agent"]["rg_binary"])) == spec["native_agent"]["rg_sha256"], "rg copy drift")
        from .native_baseline import options, PROFILE
        args += options({"config": {f"permissions.{PROFILE}.filesystem": {
            **spec["native_agent"]["config"][f"permissions.{PROFILE}.filesystem"], str(root / "tmp"): "write", str(root / "home"): "read"},
            "shell_environment_policy.inherit": "none", "shell_environment_policy.set.HOME": str(root / "home"),
            "shell_environment_policy.set.TMPDIR": str(root / "tmp"), "shell_environment_policy.set.LANG": "en_US.UTF-8"}})
        (root / "home").mkdir(exist_ok=True)
    if local_endpoint is not None:
        endpoint = local_endpoint_url(local_endpoint)
        for key, value in {"model_provider": "local", "model_providers.local.name": "Local",
                           "model_providers.local.base_url": endpoint + "/v1", "model_providers.local.wire_api": "responses",
                           "model_providers.local.requires_openai_auth": False,
                           "model_providers.local.supports_websockets": False}.items():
            args += ["-c", f"{key}={canonical(value)}"]
    if relay_settings:
        require(server is not None, "warm MCP attachment required")
        if server.get("instructions"):
            from .pi_client import server_instructions_prompt
            args += ["-c", "developer_instructions=" + canonical(server_instructions_prompt(server["instructions"]))]
        for key, value in server.items():
            if key == "instructions":  # Already forwarded verbatim above; this is not a supported MCP config key.
                continue
            encoded = ("{" + ", ".join(f"{canonical(k)} = {canonical(v)}" for k, v in value.items()) + "}"
                       if isinstance(value, dict) else canonical(value))
            args += ["-c", f"mcp_servers.navigation.{key}={encoded}"]
        args += ["-c", 'mcp_servers.navigation.default_tools_approval_mode="approve"']
        limit = limits["codex_tokens"]
        # Exec output has an independent history budget; its default can clip a larger model-specified pragma.
        args += ["-c", f"tool_output_token_limit={limit}"]
        for name in relay_settings["enabled_tools"]:
            args += ["-c", f"mcp_servers.navigation.tools.{canonical(name)}.output_token_limit={limit}"]
    write_json(folder / "cli-policy.json", {"backend": "codex", "measurement_mode": "codemode",
        "output_token_limit": limits["codex_tokens"],
        "history_output_token_limit": limits["codex_tokens"],
        "exec_default_max_output_tokens": 10000, "reference_text_bytes": limits["reference_text_bytes"],
        "code_mode": "model default code_mode_only", "operator_auth_linked": local_endpoint is None,
        "local_provider": local_endpoint is not None}, exclusive=True)
    return [*args, "-"], workspace, env
