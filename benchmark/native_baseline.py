"""Native coding-agent baseline: no search MCP, fixed read boundary, no model probes."""
from __future__ import annotations

import os
import subprocess
from pathlib import Path

from .core import canonical, command, file_digest, require, write_json

PROFILE = "grafana_baseline"


def configuration(source: Path, rg_binary: Path) -> dict:
    source, rg_binary = source.resolve(), rg_binary.resolve()
    search_path = str(rg_binary.parent) + os.pathsep + os.environ.get("PATH", os.defpath)
    prefix = f"permissions.{PROFILE}"
    return {"schema": "grafana-native-agent-v1", "mode": "native_agent", "source": str(source),
            "rg_binary": str(rg_binary), "rg_sha256": file_digest(rg_binary), "search_product_mcp": False,
            "env": {"PATH": search_path},
            "config": {"default_permissions": PROFILE,
                       f"{prefix}.filesystem": {":root": "deny", ":minimal": "read",
                                                str(source): "read", str(rg_binary.parent): "read"},
                       f"{prefix}.network.enabled": False,
                       "shell_environment_policy.set.PATH": search_path},
            "native_tools": "Codex defaults; no shell/unified_exec/code_mode override",
            "call_unit": "command_execution item ID; a compound command is one native call",
            "output_policy": "native tool and client defaults; no harness clipping"}


def options(value: dict) -> list[str]:
    args = []
    for key, setting in value["config"].items():
        encoded = ("{" + ", ".join(f"{canonical(k)} = {canonical(v)}" for k, v in setting.items()) + "}"
                   if isinstance(setting, dict) else canonical(setting))
        args.extend(["-c", f"{key}={encoded}"])
    return args


def command_items(events: list[dict]) -> list[dict]:
    """Join started/completed observations by native ID without double counting."""
    items = {}
    for event in events:
        item = event.get("item", {})
        if event.get("type") not in {"item.started", "item.updated", "item.completed"} or item.get("type") != "command_execution":
            continue
        require(isinstance(item.get("id"), str), "native command item ID 누락")
        prior = items.get(item["id"], {})
        items[item["id"]] = {**prior, **item, "completed": prior.get("completed", False) or event["type"] == "item.completed"}
    return list(items.values())


def probe(value: dict, folder: Path, denied_paths: list[Path]) -> dict:
    """Exercise the exact permissions locally. Never launches Codex exec or an LLM."""
    from .tool_artifacts import clean_environment
    folder.mkdir(parents=True, exist_ok=True)
    codex_home = folder / "codex-home"
    codex_home.mkdir(exist_ok=True)  # Empty configuration; no auth link or credential access.
    env = clean_environment({"CODEX_HOME": str(codex_home)})
    source = Path(value["source"])
    base = ["codex", "sandbox", "-C", str(source), "-P", PROFILE, *options(value), "--"]
    cases = [("source_read", ["/bin/cat", "README.md"], True),
             ("native_rg", ["/bin/sh", "-c", 'command -v rg && rg --version && rg Grafana README.md'], True),
             ("native_rg_login", [os.environ.get("SHELL", "/bin/sh"), "-lc", 'command -v rg && rg --version && rg Grafana README.md'], True)]
    cases.extend((f"outside_read_{i}", ["/bin/cat", str(path)], False) for i, path in enumerate(denied_paths))
    scratch = source / ".benchmark-write-boundary-probe"
    require(not scratch.exists(), "write probe path already exists")
    cases.append(("source_write", ["/usr/bin/touch", str(scratch)], False))
    observations = []
    try:
        for name, args, allowed in cases:
            result = subprocess.run(base + args, cwd=source, env=env, capture_output=True, timeout=30)
            # A failed boundary must not print private source/rubric bytes into probe logs.
            observation = {"name": name, "command": base + args, "expected_access": "allow" if allowed else "deny",
                           "exit_code": result.returncode, "stdout_bytes": len(result.stdout),
                           "stdout_sha256": __import__("hashlib").sha256(result.stdout).hexdigest(),
                           "stderr": result.stderr.decode(errors="replace")}
            if allowed:
                observation["stdout"] = result.stdout.decode(errors="replace")
            observations.append(observation)
            write_json(folder / "probe.json", observations)
            require((result.returncode == 0) == allowed, f"기본 에이전트 파일 경계 검증 실패: {name}; {folder / 'probe.json'}")
            if name.startswith("native_rg"):
                require(result.stdout.decode().splitlines()[0] == value["rg_binary"], "기준군 rg PATH pin 불일치")
    finally:
        if scratch.exists():
            scratch.unlink()  # Only this probe's empty, uniquely named file.
    inventory = {"mode": "native_agent", "codex_version": command(["codex", "--version"]).strip(),
                 "configuration": value, "search_product_mcp": False,
                 "tool_inventory_observation": "No solver was launched; actual model tool inventory is unobserved.",
                 "boundary_probe": "source read and pinned rg allowed; outside reads and source writes denied"}
    write_json(folder / "inventory.json", inventory)
    return {"native_inventory": str(folder / "inventory.json"), "native_inventory_sha256": file_digest(folder / "inventory.json"),
            "native_agent": value, "relay_settings": None, "tool_condition": "native_agent_baseline",
            "auxiliary_necessity": [], "probes": [{"name": r["name"], "exit_code": r["exit_code"], "expected_access": r["expected_access"]} for r in observations],
            "probe_log": str(folder / "probe.json"), "probe_log_sha256": file_digest(folder / "probe.json"),
            "cleanup": {"frontend": {"complete": True}, "daemon": {"shared_daemon": False, "mode": "no server"}}}
