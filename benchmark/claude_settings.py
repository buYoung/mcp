"""Fail-closed audit of the settings files implicitly loaded by the pinned Claude child."""
from __future__ import annotations

import hashlib
import json
import os
import plistlib
import pwd
from pathlib import Path
from xml.parsers.expat import ExpatError

from .core import require

MANAGED_ROOT = Path("/Library/Application Support/ClaudeCode")
MANAGED_PREFERENCES = Path("/Library/Managed Preferences")


def snapshot(workspace: Path, *, home: Path | None = None) -> dict:
    """Keep only file hashes/key names. Never retain or report setting values or parse errors."""
    user = pwd.getpwuid(os.getuid())
    home = home or Path(user.pw_dir)
    workspace = workspace.resolve()
    paths = {str(p): p for p in (home / ".claude/settings.json", MANAGED_ROOT / "managed-settings.json",
        MANAGED_PREFERENCES / "com.anthropic.claudecode.plist",
        MANAGED_PREFERENCES / user.pw_name / "com.anthropic.claudecode.plist")}
    dropins = MANAGED_ROOT / "managed-settings.d"
    try:
        if dropins.exists():
            require(dropins.is_dir() and not dropins.is_symlink(), f"Claude settings directory rejected: {dropins}")
            paths.update({str(p): p for p in sorted(dropins.iterdir()) if p.suffix == ".json"})
    except OSError:
        raise ValueError(f"Claude settings directory unreadable: {dropins}") from None
    for name in ("settings.json", "settings.local.json"):
        paths[f"<workspace>/.claude/{name}"] = workspace / ".claude" / name
        # Claude can discover project settings above cwd. Missing ancestors need no
        # identity entry; newly created files are detected by the next snapshot.
        for parent in workspace.parents:
            path = parent / ".claude" / name
            if path.exists() or path.is_symlink():
                paths[str(path)] = path
    files = {}
    for label, path in sorted(paths.items()):
        require(not path.is_symlink(), f"Claude settings symlink rejected: {path}")
        try:
            data = path.read_bytes()
        except FileNotFoundError:
            files[label] = {"sha256": None, "keys": []}
            continue
        except OSError:
            raise ValueError(f"Claude settings unreadable: {path}") from None
        try:
            value = plistlib.loads(data) if path.suffix == ".plist" else json.loads(data)
            require(isinstance(value, dict), "settings object required")
            keys = sorted(value)
        except (ValueError, TypeError, plistlib.InvalidFileException, ExpatError):
            raise ValueError(f"Claude settings malformed (values withheld): {path}") from None
        # Unknown keys are not presumed cosmetic: env/model/outputStyle/language,
        # helpers, limits, plugins, permissions, statusLine and hooks all fail closed.
        rejected = [key for key in keys if key != "$schema"]
        require(not rejected, f"Claude condition-affecting/unknown settings: {path}; keys={json.dumps(rejected, ensure_ascii=False)}")
        files[label] = {"sha256": hashlib.sha256(data).hexdigest(), "keys": keys}
    return {"files": files, "policy": "empty settings or $schema only; unknown keys fail closed"}


def verify(expected: dict, workspace: Path, *, home: Path | None = None) -> dict:
    require(isinstance(expected, dict), "Claude prepared settings condition missing")
    current = snapshot(workspace, home=home)
    changed = sorted(key for key in expected.get("files", {}).keys() | current["files"].keys()
                     if expected.get("files", {}).get(key) != current["files"].get(key))
    require(current == expected, f"Claude settings changed since preparation: {', '.join(changed)}")
    return current
