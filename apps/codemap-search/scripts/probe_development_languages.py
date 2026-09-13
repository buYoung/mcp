#!/usr/bin/env python3
"""Replay the language probes without cloning or executing public repository code."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess

from public_validation import DATA, Checks, McpClient, save_json, sha256
from validate_development_languages import create_probe, validate_probe


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--language", action="append")
    args = parser.parse_args()
    output = args.output.expanduser().resolve()
    output.mkdir(parents=True, exist_ok=False)
    shutil.copy(args.binary, output / "tested-codemap-search")
    binary = output / "tested-codemap-search"
    manifest = json.loads((DATA / "development-languages.json").read_text())
    summary = {"binary_sha256": sha256(binary), "results": [], "errors": []}
    for language in manifest["languages"]:
        name = language["language"]
        if name in {"rust", "go"} or args.language and name not in args.language:
            continue
        for profile in ("default", "structural"):
            root = output / f"{name}-{profile}" / "worktree"
            root.mkdir(parents=True)
            subprocess.run(["git", "init", "--quiet", str(root)], check=True)
            probe = create_probe(root, name, language["extensions"][0])
            config = (DATA / "config.toml").read_text().replace("is_shell_support_enabled = false", "is_shell_support_enabled = true")
            if profile == "structural":
                config = config.replace("navigation_context_default = false", "navigation_context_default = true").replace("navigation_store_references = false", "navigation_store_references = true")
            (root / ".codemap").mkdir()
            (root / ".codemap/config.toml").write_text(config)
            try:
                with McpClient(binary, root, root.parent) as client:
                    client.ready(probe, timeout=30)
                    checks = Checks(client, root)
                    validate_probe(checks, probe, name)
                result = {"language": name, "profile": profile, "checks": checks.results}
                result["counts"] = {status: sum(check["status"] == status for check in checks.results) for status in ("pass", "fail", "unverified")}
                summary["results"].append(result)
                save_json(root.parent / "results.json", result)
                print(name, profile, result["counts"], flush=True)
            except Exception as error:
                record = {"language": name, "profile": profile, "error": f"{type(error).__name__}: {error}"}
                summary["errors"].append(record)
                print(json.dumps(record), flush=True)
            save_json(output / "summary.json", summary)
    return int(bool(summary["errors"]) or any(result["counts"]["fail"] for result in summary["results"]))


if __name__ == "__main__":
    raise SystemExit(main())
