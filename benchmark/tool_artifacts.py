"""Task-local, source-bound installations for the Grafana comparison arms."""
from __future__ import annotations

import argparse
import os
import platform
import shutil
import subprocess
import tarfile
import time
import urllib.request
from pathlib import Path

from . import resources, route_contract as routes, settings
from .core import command, digest, file_digest, read_json, require, write_json

ARTIFACTS = routes.PREPARATION / "artifacts"
DOWNLOADS = routes.PREPARATION / "downloads"
# Official assets of the already locked releases, with their published SHA256.
RELEASE_ASSETS = {
    "codebase-memory-mcp": {
        "file": "codebase-memory-mcp-darwin-arm64.tar.gz",
        "sha256": "9bfcbe38414b47969a3e1d1c1999f5357409387c86fa3d24221b5458267660d2",
        "url": "https://github.com/DeusData/codebase-memory-mcp/releases/download/v0.12.1/codebase-memory-mcp-darwin-arm64.tar.gz"},
    "rg": {
        "file": "ripgrep-15.2.0-aarch64-apple-darwin.tar.gz",
        "sha256": "3750b2e93f37e0c692657da574d7019a101c0084da05a790c83fd335bad973e4",
        "url": "https://github.com/BurntSushi/ripgrep/releases/download/15.2.0/ripgrep-15.2.0-aarch64-apple-darwin.tar.gz"},
}


def clean_environment(extra: dict | None = None) -> dict:
    allowed = {"PATH", "HOME", "USER", "LOGNAME", "TMPDIR", "LANG", "LC_ALL", "SSL_CERT_FILE",
               "SSL_CERT_DIR", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN"}
    return {**{k: v for k, v in os.environ.items() if k in allowed}, "PYTHONDONTWRITEBYTECODE": "1",
            "npm_config_cache": str(routes.PREPARATION / "npm-cache"),
            "npm_config_userconfig": str(routes.PREPARATION / "npm-user.npmrc"),
            "npm_config_globalconfig": str(routes.PREPARATION / "npm-global.npmrc"),
            "UV_CACHE_DIR": str(routes.PREPARATION / "uv-cache"),
            "UV_PYTHON_INSTALL_DIR": str(routes.PREPARATION / "python-runtimes"), **(extra or {})}


def tree_files(root: Path, *, exclude: set[str] | None = None) -> dict:
    """Hash bytes, modes and links without following links into unrelated installations."""
    excluded = {".git", "__pycache__", ".DS_Store", *(exclude or ())}
    files = {}
    for folder, dirs, names in os.walk(root, followlinks=False):
        dirs[:] = sorted(d for d in dirs if d not in excluded)
        for name in sorted(names + [d for d in dirs if (Path(folder) / d).is_symlink()]):
            if name in excluded:
                continue
            path = Path(folder) / name
            relative = str(path.relative_to(root))
            if path.is_symlink():
                files[relative] = {"symlink": os.readlink(path)}
            elif path.is_file():
                files[relative] = {"sha256": file_digest(path), "mode": path.stat().st_mode & 0o777}
    return files


def download(url: str, path: Path, checksum: str | None = None) -> dict:
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        temporary = path.with_suffix(path.suffix + ".partial")
        request = urllib.request.Request(url, headers={"User-Agent": "grafana-benchmark-preparation/1"})
        with urllib.request.urlopen(request, timeout=120) as response, temporary.open("wb") as stream:
            shutil.copyfileobj(response, stream)
        if checksum:
            require(file_digest(temporary) == checksum, f"다운로드 해시 불일치: {url}")
        temporary.replace(path)
    actual = file_digest(path)
    require(checksum is None or actual == checksum, f"다운로드 캐시 변경: {path}")
    return {"url": url, "path": str(path), "sha256": actual, "bytes": path.stat().st_size}


def run_logged(args: list[str], cwd: Path, log: Path, *, env: dict | None = None, timeout=1800, stdin=None) -> dict:
    log.parent.mkdir(parents=True, exist_ok=True)
    if log.exists():
        stamp = str(time.time_ns())
        log.rename(log.with_name(f"{log.name}.{stamp}.previous"))
        if log.with_suffix(log.suffix + ".json").exists():
            log.with_suffix(log.suffix + ".json").rename(log.with_name(f"{log.name}.{stamp}.previous.json"))
    started = time.monotonic()
    with log.open("wb") as stream:
        completed = subprocess.run(args, cwd=cwd, env=env or clean_environment(), stdin=stdin, stdout=stream,
                                   stderr=subprocess.STDOUT, timeout=timeout)
    record = {"command": args, "cwd": str(cwd), "log": str(log), "exit_code": completed.returncode,
              "elapsed_seconds": time.monotonic() - started, "log_sha256": file_digest(log)}
    write_json(log.with_suffix(log.suffix + ".json"), record)
    require(completed.returncode == 0, f"준비 명령 실패: {log}")
    return record


def extract(archive: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=False)
    with tarfile.open(archive) as stream:
        stream.extractall(destination, filter="data")


def source_install(arm: dict, output: Path) -> tuple[Path, dict]:
    archive = download(arm["source_archive"]["url"], DOWNLOADS / f"{arm['id']}-{arm['commit']}.tar.gz",
                       arm["source_archive"]["sha256"])
    source = output / "source"
    if not source.exists():
        staging = output / "unpacked"
        if staging.exists():
            staging.rename(output / f"unpacked-incomplete-{time.time_ns()}")
        extract(Path(archive["path"]), staging)
        roots = list(staging.iterdir())
        require(len(roots) == 1 and roots[0].is_dir(), "upstream archive root 오류")
        roots[0].rename(source)
        staging.rmdir()
    baseline = output / "source-files.json"
    if not baseline.exists():
        write_json(baseline, tree_files(source), exclusive=True)
    return source, {"archive": archive, "files_manifest": str(baseline), "source_sha256": file_digest(baseline)}


def verify_artifact(artifact: dict, *, full=True) -> None:
    for binary in artifact["executables"]:
        require(file_digest(Path(binary["path"])) == binary["sha256"], f"실행 파일 변경: {binary['path']}")
    for model in artifact.get("model_assets", []):
        require(file_digest(Path(model["path"])) == model["sha256"], "임베딩 모델 변경")
    if full:
        require(digest(tree_files(Path(artifact["installation"]), exclude={"target"})) == artifact["installation_sha256"],
                f"설치 파일 변경: {artifact['id']}")


def pi_toolchain() -> dict:
    """Benchmark-owned npm install; every reuse verifies the complete pinned package tree."""
    import fcntl
    require(file_digest(routes.TARGETS / "targets.lock.json") == routes.TARGET_LOCK_SHA256, "pi tools lock 변경")
    pins = read_json(routes.TARGETS / "targets.lock.json")["pi_toolchain"]
    require(platform.system() == "Darwin" and platform.machine() == "arm64", "pi toolchain은 darwin-arm64 전용입니다")
    root = settings.CACHE / "pi-toolchain"
    root.mkdir(parents=True, exist_ok=True)
    with (root / "install.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        receipt = root / "artifact.json"
        installation = root / "installation"
        if not receipt.exists():
            require(not installation.exists(), "미완료 pi install을 검토하고 제거한 뒤 다시 준비하세요")
            installation.mkdir()
            for source, target, checksum in (("pi-package.json", "package.json", pins["package_sha256"]),
                                              ("pi-package-lock.json", "package-lock.json", pins["npm_lock_sha256"])):
                require(file_digest(routes.TARGETS / source) == checksum, "pi npm pin 변경")
                shutil.copyfile(routes.TARGETS / source, installation / target)
            node = Path(shutil.which("node") or "").resolve()
            npm = shutil.which("npm")
            require(node.is_file() and npm is not None, "pi install에 node/npm 필요")
            run_logged([npm, *pins["install_flags"]], installation, root / "install.log")
            modules = installation / "node_modules"
            value = {"node": str(node), "pi_cli": str(modules / "@earendil-works/pi-coding-agent/dist/cli.js"),
                     "bridge_extension": str(modules / "@buyong/pi-claude-bridge/src/index.ts"),
                     "mcp_adapter_extension": str(modules / "pi-mcp-adapter"),
                     "claude_code_binary": str(modules / "@anthropic-ai/claude-agent-sdk-darwin-arm64/claude"),
                     "versions": {"node": command([str(node), "--version"]).strip(), **pins["versions"]},
                     "lock_sha256": pins["npm_lock_sha256"], "node_sha256": file_digest(node)}
            require(subprocess.check_output([value["node"], value["pi_cli"], "--version"], text=True,
                    env=clean_environment({"PI_CODING_AGENT_DIR": str(root / "version-agent"),
                                           "PI_SKIP_VERSION_CHECK": "1", "PI_TELEMETRY": "0"})).strip()
                    == pins["versions"]["pi"], "pi 버전 변경")
            require(command([value["claude_code_binary"], "--version"]).strip() == pins["versions"]["claude_code"] + " (Claude Code)", "Claude binary 버전 변경")
            require(read_json(Path(value["mcp_adapter_extension"]) / "package.json")["version"] == pins["versions"]["mcp_adapter"],
                    "pi-mcp-adapter 버전 변경")
            require(digest(tree_files(modules)) == pins["node_modules_sha256"], "pi 설치 tree hash 불일치")
            write_json(receipt, value, exclusive=True)
        value = read_json(receipt)
        require(value["versions"] == {"node": command([value["node"], "--version"]).strip(), **pins["versions"]}
                and file_digest(Path(value["node"])) == value["node_sha256"], "pi host node 변경")
        require(value["lock_sha256"] == pins["npm_lock_sha256"]
                and file_digest(installation / "package-lock.json") == pins["npm_lock_sha256"]
                and file_digest(installation / "package.json") == pins["package_sha256"]
                and digest(tree_files(installation / "node_modules")) == pins["node_modules_sha256"], "pi toolchain drift")
        expected = {"pi_cli": "@earendil-works/pi-coding-agent/dist/cli.js",
                    "bridge_extension": "@buyong/pi-claude-bridge/src/index.ts",
                    "mcp_adapter_extension": "pi-mcp-adapter",
                    "claude_code_binary": "@anthropic-ai/claude-agent-sdk-darwin-arm64/claude"}
        require(all(Path(value[key]) == installation / "node_modules" / path for key, path in expected.items()), "pi artifact 경로 변경")
        return value


def cli_toolchain() -> dict:
    """Private stable solver CLIs; npm integrity, installed tree and native binaries are locked."""
    import fcntl
    require(file_digest(routes.TARGETS / "targets.lock.json") == routes.TARGET_LOCK_SHA256, "CLI tools lock 변경")
    pins = read_json(routes.TARGETS / "targets.lock.json")["cli_toolchain"]
    require(platform.system() == "Darwin" and platform.machine() == "arm64", "CLI toolchain은 darwin-arm64 전용입니다")
    root = settings.CACHE / "cli-toolchain"
    root.mkdir(parents=True, exist_ok=True)
    with (root / "install.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        installation = root / "installation"
        receipt = root / "artifact.json"
        if not receipt.exists():
            require(not installation.exists(), "미완료 CLI install을 검토하고 보존한 뒤 다시 준비하세요")
            installation.mkdir()
            for name, key in (("package.json", "package_sha256"), ("package-lock.json", "npm_lock_sha256")):
                source = routes.TARGETS / ("cli-" + name)
                require(file_digest(source) == pins[key], "CLI npm pin 변경")
                shutil.copyfile(source, installation / name)
            run_logged([shutil.which("npm"), *pins["install_flags"]], installation, root / "install.log")
            write_json(receipt, {"versions": pins["versions"], "lock_sha256": pins["npm_lock_sha256"]}, exclusive=True)
        require(read_json(receipt) == {"versions": pins["versions"], "lock_sha256": pins["npm_lock_sha256"]}, "CLI receipt drift")
        for name, key in (("package.json", "package_sha256"), ("package-lock.json", "npm_lock_sha256")):
            require(file_digest(installation / name) == pins[key], "CLI npm lock drift")
        require(digest(tree_files(installation / "node_modules")) == pins["node_modules_sha256"], "CLI installation drift")
        value = {"versions": pins["versions"], "lock_sha256": pins["npm_lock_sha256"],
                 "installation_sha256": pins["node_modules_sha256"]}
        for backend, item in pins["executables"].items():
            binary = installation / "node_modules" / item["relative_path"]
            require(file_digest(binary) == item["sha256"], "CLI executable drift")
            value[backend] = str(binary)
        return value


def prepare_one(ident: str, emit=print) -> dict:
    arm = next(a for a in routes.arm_contracts() if a["id"] == ident)
    if ident == "codemap-search":
        record_path = ARTIFACTS / ident / "artifact.json"
        commit = command(["git", "rev-parse", "--verify", "--end-of-options", arm["git_ref"] + "^{commit}"],
                         settings.REPOSITORY).strip()
        require(commit == arm["commit"], "codemap release tag의 고정 commit 변경")
        if record_path.exists():
            existing = read_json(record_path)
            if all(existing["source_pin"].get(key) == arm[key] for key in ("commit", "git_ref", "version")):
                verify_artifact(existing)
                return existing
        build = resources.build_product(arm, emit)
        require(all(build["identity"][key] == arm[key] for key in ("commit", "git_ref", "version")),
                "codemap release snapshot identity 불일치")
        binary = Path(build["binary"])
        observed_version = command([str(binary), "--version"], timeout_seconds=60).strip()
        require(observed_version == f"codemap-search {arm['version']}", f"codemap 설치 버전 불일치: {observed_version}")
        result = {"id": ident, "source_pin": build["identity"], "actual_version": build["identity"]["version"],
                  "installation": build["source_snapshot"], "launch": [str(binary)],
                  "executables": [{"path": str(binary), "sha256": build["binary_sha256"]}],
                  "dependencies": {"Cargo.lock": file_digest(Path(build["source_snapshot"]) / "Cargo.lock")},
                  "runtime": {"rustc": build["rustc"]}, "backend": "native_rust", "model_assets": [],
                  "build": build, "version_output": observed_version}
        result["installation_sha256"] = digest(tree_files(Path(result["installation"]), exclude={"target"}))
        output = ARTIFACTS / ident
        output.mkdir(parents=True, exist_ok=True)
        write_json(output / "artifact.json", result)
        return result
    output = ARTIFACTS / ident / arm["commit"]
    output.mkdir(parents=True, exist_ok=True)
    record_path = output / "artifact.json"
    if record_path.exists():
        record = read_json(record_path)
        verify_artifact(record)
        return record
    emit(f"도구 준비: {ident} {arm['version']}")
    source, provenance = source_install(arm, output)
    commands = []
    models = []
    runtime = {}
    if ident in {"codegraph", "zvec-grep"}:
        node = str(Path(shutil.which("node")).resolve())
        npm = shutil.which("npm")
        runtime = {"node": command([node, "--version"]).strip(), "node_path": node,
                   "node_sha256": file_digest(Path(node)), "npm": command([npm, "--version"]).strip()}
        commands.append(run_logged([npm, "ci", "--no-audit", "--no-fund"], source, output / "install.log"))
        commands.append(run_logged([npm, "run", "build"], source, output / "build.log"))
        entry = "dist/bin/codegraph.js" if ident == "codegraph" else "dist/cli/index.js"
        launch = [node, str(source / entry)]
        if ident == "codegraph":
            commands.append(run_logged([npm, "run", "build:kernel"], source, output / "kernel-build.log"))
            backend = "native_kernel_built_from_locked_source"
        else:
            backend = "zvec_native_model2vec"
            revision = routes.load_profile()["index_mode"]["zvec_model_revision"]
            for filename in ("model.safetensors", "tokenizer.json"):
                models.append(download(f"https://huggingface.co/minishlab/potion-code-16M-v2/resolve/{revision}/{filename}",
                                       routes.PREPARATION / "model-assets" / revision / filename))
        dependencies = {"package-lock.json": file_digest(source / "package-lock.json")}
        installation = source
    elif ident in {"graphify", "serena"}:
        uv = shutil.which("uv")
        require(uv is not None, f"{ident}의 고정 환경 준비에 uv가 필요합니다")
        if ident == "graphify":
            require(arm["version"] == "0.9.95" and platform.system() == "Darwin"
                    and platform.machine() == "arm64", "Graphify runtime lock 대상 불일치")
            requirements = routes.TARGETS / "graphifyy-0.9.95-py312-darwin-arm64.requirements.txt"
            require(file_digest(requirements) == "7fbfcf6a069ad0c4861bd7a39efdb160e365f54fb5c4262f38423117c6e87e99",
                    "Graphify runtime dependency lock 변경")
            commands.append(run_logged([uv, "venv", "--python", "3.12", str(source / ".venv")],
                                       source, output / "venv.log"))
            commands.append(run_logged([uv, "pip", "sync", "--python", str(source / ".venv/bin/python"),
                                        "--require-hashes", "--only-binary", ":all:", str(requirements)],
                                       source, output / "install.log"))
        else:
            commands.append(run_logged([uv, "sync", "--locked", "--no-dev", "--python", "3.12"],
                                       source, output / "install.log"))
        python = source / ".venv/bin/python"
        commands.append(run_logged([uv, "pip", "check", "--python", str(python)],
                                   source, output / "dependency-check.log"))
        launch = [str(python), "-m", "graphify"] if ident == "graphify" else [str(source / ".venv/bin/serena")]
        runtime = {"python": command([str(python), "--version"]).strip(),
                   "python_sha256": file_digest(python), "uv": command([uv, "--version"]).strip()}
        dependencies = {"uv.lock": file_digest(source / "uv.lock")}
        backend = "tree_sitter_code_only" if ident == "graphify" else "local_lsp_go_typescript"
        if ident == "graphify":
            dependencies["runtime_requirements_sha256"] = file_digest(requirements)
            verify_wheel = """
import hashlib, json, sys
from importlib.metadata import distribution
from pathlib import Path
source = Path(sys.argv[1])
dist = distribution('graphifyy')
checked = []
for name in dist.files or []:
    if name.parts[0] == 'graphify' and '__pycache__' not in name.parts:
        expected, installed = source / name, Path(dist.locate_file(name))
        assert expected.is_file() and installed.is_file(), str(name)
        assert hashlib.sha256(expected.read_bytes()).digest() == hashlib.sha256(installed.read_bytes()).digest(), str(name)
        checked.append(str(name))
assert checked
print(json.dumps({'source_matching_files': len(checked), 'version': dist.version}))
"""
            commands.append(run_logged([str(python), "-c", verify_wheel, str(source)],
                                       source, output / "wheel-source-check.log"))
            provenance["official_package_wheel"] = {
                "url": "https://files.pythonhosted.org/packages/79/87/4c0808afdf99568e27f24838595506d6a286aca6f3bbf8fbb2520574f413/graphifyy-0.9.95-py3-none-any.whl",
                "sha256": "e067c9256e5e762406816e51039c4e775f8d2f39a893ecc0b7095d594f6c3120",
                "source_verification_log": str(output / "wheel-source-check.log")}
        if ident == "serena":
            for name in ("go", "gopls", "node", "npm"):
                executable = shutil.which(name)
                require(executable is not None, f"Serena 언어 서버 준비에 {name}가 필요합니다")
                path = Path(executable).resolve()
                runtime[name] = {"path": str(path), "sha256": file_digest(path),
                                 "version": command([str(path), "version" if name in {"go", "gopls"} else "--version"]).strip()}
        installation = source
    else:
        require(platform.system() == "Darwin" and platform.machine() == "arm64", "등록한 공식 binary는 darwin-arm64 전용입니다")
        asset = RELEASE_ASSETS[ident]
        release = download(asset["url"], DOWNLOADS / ident / arm["commit"] / asset["file"], asset["sha256"])
        installation = output / "release"
        if not installation.exists():
            extract(Path(release["path"]), installation)
        found = [p for p in installation.rglob(ident) if p.is_file()]
        require(len(found) == 1, f"공식 binary 경로가 모호합니다: {ident}")
        launch = [str(found[0])]
        provenance["official_release_asset"] = release
        provenance["source_association"] = {"repository": arm["repository"], "commit": arm["commit"],
                                            "release_tag": arm["release_tag"]}
        dependencies = {"distribution": "official_release", "asset_sha256": release["sha256"]}
        backend = "official_native_binary"
    if ident == "serena":
        # An archive has no .git; do not let its version command discover the harness's parent repository.
        commands.append(run_logged([*launch, "--version"], source, output / "version.log",
                                   env=clean_environment({"GIT_CEILING_DIRECTORIES": str(ARTIFACTS)}), timeout=60))
        observed_version = (output / "version.log").read_text().strip()
    else:
        observed_version = command([*launch, "--version"], timeout_seconds=60).strip()
    require(arm["version"] in observed_version, f"설치 버전 불일치: {ident}: {observed_version}")
    baseline = read_json(Path(provenance["files_manifest"]))
    for name, metadata in baseline.items():
        if "sha256" in metadata:
            require(file_digest(source / name) == metadata["sha256"], f"빌드가 upstream source를 변경했습니다: {name}")
    executables = []
    for arg in launch:
        path = Path(arg)
        if path.is_file():
            executables.append({"path": str(path), "sha256": file_digest(path)})
    if ident == "serena":
        executables.append({"path": str(python), "sha256": file_digest(python)})
        executables.extend({"path": runtime[name]["path"], "sha256": runtime[name]["sha256"]}
                           for name in ("go", "gopls", "node", "npm"))
    result = {"id": ident, "source_pin": arm, "provenance": provenance, "actual_version": observed_version,
              "installation": str(installation), "installation_sha256": digest(tree_files(installation, exclude={"target"})),
              "launch": launch, "executables": executables, "dependencies": dependencies,
              "runtime": runtime, "backend": backend, "model_assets": models, "commands": commands}
    write_json(record_path, result)
    return result


def load_artifacts(*, verify=True) -> dict:
    result = {}
    for arm in routes.arm_contracts():
        ident = arm["id"]
        output = ARTIFACTS / ident
        if ident != "codemap-search":
            output /= arm["commit"]
        path = output / "artifact.json"
        require(path.is_file(), f"현재 pin의 도구 준비 기록이 없습니다: {ident}")
        value = read_json(path)
        require(value["id"] == ident and value["source_pin"]["commit"] == arm["commit"]
                and value["source_pin"]["version"] == arm["version"], f"설치 source pin 변경: {ident}")
        if ident == "codemap-search":
            require(value["source_pin"].get("git_ref") == arm["git_ref"], "codemap release snapshot이 아닙니다")
        if verify:
            verify_artifact(value)
        result[ident] = value
    return result


def publish_artifacts() -> dict:
    artifacts = load_artifacts()
    return routes.publish_handoff("03-artifacts.json", {
        "status": "verified", "host": {"system": platform.system(), "machine": platform.machine(),
                                          "platform": platform.platform()}, "artifacts": artifacts,
        "pi_toolchain": pi_toolchain(), "cli_toolchain": cli_toolchain(),
        "verification": {"artifact_hashes": "passed", "source_versions": "passed"},
        "limitations": ["No Grafana index or solver/grader inference was created by artifact preparation."]},
        implementation=["benchmark/tool_artifacts.py", "benchmark/resources.py"], dependencies=["01-contract.json"])


def main() -> None:
    parser = argparse.ArgumentParser(description="고정 비교 도구를 작업 전용 경로에 준비")
    parser.add_argument("arm", nargs="?", default="all", choices=["all", "publish", *routes.ARM_IDS])
    args = parser.parse_args()
    if args.arm != "publish":
        for ident in routes.ARM_IDS if args.arm == "all" else (args.arm,):
            result = prepare_one(ident, lambda message: print(message, flush=True))
            print(f"준비 완료: {ident} {result['actual_version']}", flush=True)
    if args.arm in {"all", "publish"}:
        publish_artifacts()


if __name__ == "__main__":
    main()
