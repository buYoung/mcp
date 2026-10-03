"""Task-local, source-bound installations for the six Grafana comparison arms."""
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
        "sha256": "4dee7f38b63740e6751d7a7ed7eb10291c1f2a3ea2415f599dc68370ca0a2d18",
        "url": "https://github.com/DeusData/codebase-memory-mcp/releases/download/v0.11.0/codebase-memory-mcp-darwin-arm64.tar.gz"},
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


def run_logged(args: list[str], cwd: Path, log: Path, *, env: dict | None = None, timeout=1800) -> dict:
    log.parent.mkdir(parents=True, exist_ok=True)
    if log.exists():
        stamp = str(time.time_ns())
        log.rename(log.with_name(f"{log.name}.{stamp}.previous"))
        if log.with_suffix(log.suffix + ".json").exists():
            log.with_suffix(log.suffix + ".json").rename(log.with_name(f"{log.name}.{stamp}.previous.json"))
    started = time.monotonic()
    with log.open("wb") as stream:
        completed = subprocess.run(args, cwd=cwd, env=env or clean_environment(), stdout=stream,
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


def prepare_one(ident: str, emit=print) -> dict:
    arm = next(a for a in routes.arm_contracts() if a["id"] == ident)
    if ident == "codemap-search":
        record_path = ARTIFACTS / ident / "artifact.json"
        if record_path.exists():
            existing = read_json(record_path)
            current_source = digest(resources.product_files(settings.REPOSITORY / "apps/codemap-search"))
            if existing["source_pin"]["source_sha256"] == current_source:
                # A later harness commit must not relabel an already pinned product snapshot.
                verify_artifact(existing)
                return existing
        build = resources.build_product({"id": ident, "name": ident, "path": "apps/codemap-search"}, emit)
        binary = Path(build["binary"])
        result = {"id": ident, "source_pin": build["identity"], "actual_version": build["identity"]["version"],
                  "installation": build["source_snapshot"], "launch": [str(binary)],
                  "executables": [{"path": str(binary), "sha256": build["binary_sha256"]}],
                  "dependencies": {"Cargo.lock": file_digest(Path(build["source_snapshot"]) / "Cargo.lock")},
                  "runtime": {"rustc": build["rustc"]}, "backend": "native_rust", "model_assets": [],
                  "build": build}
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
    elif ident == "graphify":
        uv = shutil.which("uv")
        require(uv is not None, "Graphify의 고정 환경 준비에 uv가 필요합니다")
        commands.append(run_logged([uv, "sync", "--locked", "--no-dev", "--extra", "mcp", "--python", "3.12"],
                                   source, output / "install.log"))
        python = source / ".venv/bin/python"
        launch = [str(python), "-m", "graphify"]
        runtime = {"python": command([str(python), "--version"]).strip(),
                   "python_sha256": file_digest(python), "uv": command([uv, "--version"]).strip()}
        dependencies = {"uv.lock": file_digest(source / "uv.lock")}
        backend = "tree_sitter_code_only"
        installation = source
    else:
        require(platform.system() == "Darwin" and platform.machine() == "arm64", "등록한 공식 binary는 darwin-arm64 전용입니다")
        asset = RELEASE_ASSETS[ident]
        release = download(asset["url"], DOWNLOADS / asset["file"], asset["sha256"])
        installation = output / "release"
        if not installation.exists():
            extract(Path(release["path"]), installation)
        found = [p for p in installation.rglob(ident) if p.is_file()]
        require(len(found) == 1, f"공식 binary 경로가 모호합니다: {ident}")
        launch = [str(found[0])]
        provenance["official_release_asset"] = release
        provenance["source_association"] = {"repository": arm["repository"], "commit": arm["commit"],
                                            "release_tag": "15.2.0" if ident == "rg" else "v0.11.0"}
        dependencies = {"distribution": "official_release", "asset_sha256": release["sha256"]}
        backend = "official_native_binary"
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
    result = {"id": ident, "source_pin": arm, "provenance": provenance, "actual_version": observed_version,
              "installation": str(installation), "installation_sha256": digest(tree_files(installation, exclude={"target"})),
              "launch": launch, "executables": executables, "dependencies": dependencies,
              "runtime": runtime, "backend": backend, "model_assets": models, "commands": commands}
    write_json(record_path, result)
    return result


def load_artifacts(*, verify=True) -> dict:
    result = {}
    for ident in routes.ARM_IDS:
        paths = list((ARTIFACTS / ident).glob("**/artifact.json"))
        require(len(paths) == 1, f"도구 준비 기록이 없거나 모호합니다: {ident}")
        value = read_json(paths[0])
        if verify:
            verify_artifact(value)
        result[ident] = value
    return result


def publish_artifacts() -> dict:
    artifacts = load_artifacts()
    return routes.publish_handoff("03-artifacts.json", {
        "status": "verified", "host": {"system": platform.system(), "machine": platform.machine(),
                                          "platform": platform.platform()}, "artifacts": artifacts,
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
