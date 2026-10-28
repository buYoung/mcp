"""Explicit npm publication pinning; preparation never resolves or accepts local packages."""
from __future__ import annotations

import argparse
import importlib.util
import json
import platform
import re
import shutil
import time
from pathlib import Path

from . import route_contract as routes, settings
from .core import digest, file_digest, read_json, require, write_json

PUBLISHED_PACKAGES = {"mcp_adapter": "@buyong/pi-mcp-adapter", "bridge": "@buyong/pi-claude-bridge"}
VERSION = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
INTEGRITY = r"sha512-[A-Za-z0-9+/]{86}=="


def validate_npm_lock(manifest: dict, lock: dict, pins: dict) -> None:
    """Only registry tarballs with integrity are admissible, including optional/transitive packages."""
    require(lock.get("lockfileVersion") == 3, "pi npm lockfileVersion must be 3")
    require(lock["packages"][""]["dependencies"] == manifest["dependencies"], "pi npm dependency binding changed")
    for name, version in manifest["dependencies"].items():
        require(re.fullmatch(VERSION, version) is not None, f"pi dependency must be an exact npm version: {name}")
        require(lock["packages"]["node_modules/" + name]["version"] == version, "pi npm version mismatch")
    for path, package in lock["packages"].items():
        if not path:
            continue
        require(path.startswith("node_modules/") and not package.get("link"), "pi local package/link forbidden")
        require(isinstance(package.get("version"), str) and bool(package["version"]), "pi dependency version missing")
        require(re.fullmatch(r"https://registry\.npmjs\.org/[^?#]+\.tgz", package.get("resolved") or "") is not None,
                f"pi registry tarball required (scratch/file/git forbidden): {path}")
        require(re.fullmatch(INTEGRITY, package.get("integrity") or "") is not None, f"pi dependency integrity missing: {path}")
    for key, name in PUBLISHED_PACKAGES.items():
        pin = pins["publications"][key]
        entry = lock["packages"]["node_modules/" + name]
        require(pin["name"] == name and pin["version"] == manifest["dependencies"][name] == entry["version"]
                and pin["integrity"] == entry["integrity"], "pi published package pin mismatch")
    sdk = pins["versions"]["agent_sdk"]
    require(manifest["overrides"]["@anthropic-ai/claude-agent-sdk"] == sdk
            and manifest["dependencies"]["@anthropic-ai/claude-agent-sdk"] == sdk, "Agent SDK override must stay pinned")
    require(lock["packages"]["node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64"]["version"] == sdk,
            "Agent SDK native binary version mismatch")


def require_publications() -> dict:
    require(file_digest(routes.TARGETS / "targets.lock.json") == routes.TARGET_LOCK_SHA256, "pi tools lock 변경")
    pins = read_json(routes.TARGETS / "targets.lock.json")["pi_toolchain"]
    require(pins.get("status") == "published",
            "pi toolchain 게시 대기: @buyong/pi-mcp-adapter와 결과 크기 수정 @buyong/pi-claude-bridge의 npm 버전 + integrity가 필요합니다. "
            "게시 후 python3 -m benchmark.pi_lock pin으로 고정하세요. 로컬 tarball/이전 설치물로 prepare할 수 없습니다.")
    for key, name in PUBLISHED_PACKAGES.items():
        pin = pins.get("publications", {}).get(key, {})
        require(pin.get("name") == name and re.fullmatch(VERSION, pin.get("version") or "") is not None
                and re.fullmatch(INTEGRITY, pin.get("integrity") or "") is not None, "pi publication pin incomplete")
    for name, key in (("pi-package.json", "package_sha256"), ("pi-package-lock.json", "npm_lock_sha256")):
        require(file_digest(routes.TARGETS / name) == pins[key], "pi npm pin 변경")
    require(re.fullmatch(r"[0-9a-f]{64}", pins.get("node_modules_sha256") or "") is not None, "pi installation tree pin missing")
    validate_npm_lock(read_json(routes.TARGETS / "pi-package.json"), read_json(routes.TARGETS / "pi-package-lock.json"), pins)
    from .pi_probe import verify_behavior_proof
    verify_behavior_proof(pins)
    return pins


def refresh_hashes() -> None:
    """Update the coupled digests after reviewed edits, never manufacture publication/integrity pins."""
    lock_path = routes.TARGETS / "targets.lock.json"
    lock = read_json(lock_path)
    pins = lock["pi_toolchain"]
    pins["package_sha256"] = file_digest(routes.TARGETS / "pi-package.json")
    pins["npm_lock_sha256"] = file_digest(routes.TARGETS / "pi-package-lock.json")
    lock_path.write_text(json.dumps(lock, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    contract = Path(routes.__file__)
    text, count = re.subn(r'^TARGET_LOCK_SHA256 = "[0-9a-f]{64}"$',
                         f'TARGET_LOCK_SHA256 = "{file_digest(lock_path)}"', contract.read_text(), flags=re.MULTILINE)
    require(count == 1, "TARGET_LOCK_SHA256 declaration missing")
    contract.write_text(text)
    # A SHA replacement keeps the file size; same-second edits can otherwise reuse stale bytecode.
    Path(importlib.util.cache_from_source(str(contract))).unlink(missing_ok=True)
    for root in (routes.TARGETS, routes.DATA):
        paths = sorted(p for p in root.iterdir() if p.is_file() and p.name != "SHA256SUMS")
        (root / "SHA256SUMS").write_text("".join(f"{file_digest(p)}  {p.name}\n" for p in paths))


def pin_publications(versions: dict, integrities: dict) -> None:
    from .tool_artifacts import clean_environment, run_logged, tree_files
    require(platform.system() == "Darwin" and platform.machine() == "arm64", "pi pinning requires darwin-arm64")
    settings.require_neutral_cache()
    lock = read_json(routes.TARGETS / "targets.lock.json")
    pins = lock["pi_toolchain"]
    require(pins["status"] == "pending_publication", "이미 게시 고정됨: 별도 개정 검토 필요")
    for key in PUBLISHED_PACKAGES:
        require(re.fullmatch(VERSION, versions[key]) is not None, "exact stable npm version required")
        require(re.fullmatch(INTEGRITY, integrities[key]) is not None, "npm sha512 integrity required")
    require(tuple(map(int, versions["bridge"].split("."))) > (0, 13, 1), "기존 bridge 0.13.1은 결과 크기 수정 전 게시물입니다")
    root = settings.CACHE / (digest({"pi-publication": time.time_ns()})[:16])
    root.mkdir(parents=True, exist_ok=False)
    manifest = read_json(routes.TARGETS / "pi-package.json")
    for key, name in PUBLISHED_PACKAGES.items():
        manifest["dependencies"][name] = versions[key]
        pins["publications"][key] = {"name": name, "version": versions[key], "integrity": integrities[key]}
        pins["versions"][key] = versions[key]
        pins["packages"][key] = name + "@" + versions[key]
    write_json(root / "package.json", manifest)
    shutil.copyfile(routes.TARGETS / "pi-package-lock.json", root / "package-lock.json")
    env = clean_environment({"HOME": str(root / "home"), "npm_config_cache": str(root / "npm-cache"),
                             "npm_config_userconfig": str(root / "user.npmrc"),
                             "npm_config_globalconfig": str(root / "global.npmrc"), "npm_config_registry": "https://registry.npmjs.org/"})
    Path(env["HOME"]).mkdir()
    npm = shutil.which("npm")
    require(npm is not None, "npm required")
    run_logged([npm, "install", "--package-lock-only", "--ignore-scripts", "--no-audit", "--no-fund"], root, root / "resolve.log", env=env)
    validate_npm_lock(manifest, read_json(root / "package-lock.json"), pins)
    run_logged([npm, *pins["install_flags"]], root, root / "install.log", env=env)
    pins["node_modules_sha256"] = digest(tree_files(root / "node_modules"))
    pins["npm_lock_sha256"] = file_digest(root / "package-lock.json")
    from .tool_artifacts import installed_pi_tools
    from .pi_probe import run_behavior_probe
    pins["behavior_probe"] = run_behavior_probe(installed_pi_tools(root, pins), root / "node_modules", root / "behavior-probe")
    pins["status"] = "published"
    lock["revision_reason"] = ("pi npm 게시물 고정 및 loopback 행동 검증 완료: "
        + ", ".join(name + "@" + versions[key] for key, name in PUBLISHED_PACKAGES.items())
        + "; private HOME/child-env API, 결과 메타, codemode 노출을 설치 tree에 결합. 실제 모델 실행 승인은 별도.")
    for source, target in (("package.json", "pi-package.json"), ("package-lock.json", "pi-package-lock.json")):
        shutil.copyfile(root / source, routes.TARGETS / target)
    (routes.TARGETS / "targets.lock.json").write_text(json.dumps(lock, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    refresh_hashes()
    print(f"npm publication pins and coupled hashes updated; install evidence: {root}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    commands.add_parser("refresh-hashes")
    pin = commands.add_parser("pin")
    for flag in ("adapter-version", "adapter-integrity", "bridge-version", "bridge-integrity"):
        pin.add_argument("--" + flag, required=True)
    args = parser.parse_args()
    if args.action == "refresh-hashes":
        refresh_hashes()
    else:
        pin_publications({"mcp_adapter": args.adapter_version, "bridge": args.bridge_version},
                         {"mcp_adapter": args.adapter_integrity, "bridge": args.bridge_integrity})


if __name__ == "__main__":
    main()
