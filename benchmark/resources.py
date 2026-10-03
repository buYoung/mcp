"""Content-addressed source, release-build and index preparation. No model requests."""
from __future__ import annotations

import contextlib
import fcntl
import io
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import tomllib
from pathlib import Path

from . import settings
from .core import ContractError, command, digest, file_digest, read_json, require, write_json


@contextlib.contextmanager
def lease(path: Path):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a") as stream:
        try:
            fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as exc:
            raise ContractError(f"다른 준비/실행 프로세스가 사용 중입니다: {path}") from exc
        yield


def environment() -> dict:
    require(sys.version_info >= (3, 12), "Python 3.12 이상이 필요합니다")
    require(os.name == "posix", "이 벤치 하네스는 macOS/Linux를 지원합니다")
    for executable in ("git", "cargo", "rustc", "codex", "node"):
        require(shutil.which(executable) is not None, f"실행 파일이 없습니다: {executable}")
    version = command(["codex", "--version"]).strip()
    help_text = command(["codex", "exec", "--help"])
    required = ("--ignore-user-config", "--ignore-rules", "--skip-git-repo-check", "--json", "--output-schema")
    require(all(option in help_text for option in required),
            f"{version}: 벤치 실행에 필요한 Codex 옵션이 없습니다: {', '.join(x for x in required if x not in help_text)}")
    login = subprocess.run(["codex", "login", "status"], capture_output=True, timeout=30)
    require(login.returncode == 0, "Codex 로그인이 필요합니다: codex login")
    return {"codex_version": version, "python": platform.python_version(),
            "node": command(["node", "--version"]).strip(),
            "rustc": command(["rustc", "-Vv"]).strip(), "cargo": command(["cargo", "--version"]).strip()}


def product_files(root: Path) -> dict:
    """Include tracked edits/deletions and nonignored new files; never include build caches."""
    ignored = {".git", "target", ".codemap", ".codemap-index", "__pycache__", "node_modules"}
    try:
        names = command(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "."], root).split("\0")
    except ContractError:
        names = []
    if not any(names):
        for folder, dirs, files in os.walk(root, followlinks=False):
            dirs[:] = [d for d in dirs if d not in ignored]
            names.extend(str((Path(folder) / name).relative_to(root)) for name in files)
    result = {}
    for name in sorted(set(names)):
        if not name or any(part in ignored for part in Path(name).parts):
            continue
        path = root / name
        if not path.exists() and not path.is_symlink():
            continue  # A tracked deletion is part of the current snapshot.
        require(path.resolve().is_relative_to(root.resolve()), f"제품 소스 밖을 가리키는 경로: {name}")
        require(path.is_file(), f"제품 입력 파일이 아닙니다: {name}")
        result[name] = {"sha256": file_digest(path), "mode": path.stat().st_mode & 0o777}
    require("Cargo.toml" in result and "Cargo.lock" in result, "제품 경로에는 Cargo.toml과 Cargo.lock이 필요합니다")
    return result


def snapshot_product(target: dict, output: Path) -> dict:
    output.mkdir(parents=True, exist_ok=False)
    if "git_ref" in target:
        commit = command(["git", "rev-parse", "--verify", "--end-of-options", target["git_ref"] + "^{commit}"], settings.REPOSITORY).strip()
        archive = subprocess.check_output(["git", "archive", commit, "apps/codemap-search"], cwd=settings.REPOSITORY)
        with tempfile.TemporaryDirectory(dir=output.parent) as temporary:
            with tarfile.open(fileobj=io.BytesIO(archive)) as stream:
                stream.extractall(temporary, filter="data")
            root = Path(temporary) / "apps/codemap-search"
            files = product_files(root)
            for name in files:
                (output / name).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(root / name, output / name)
        identity = {"kind": "git", "commit": commit, "git_ref": target["git_ref"], "dirty": False}
    else:
        root = (settings.REPOSITORY / target["path"]).resolve()
        require(root.is_dir(), f"제품 소스 경로가 없습니다: {root}")
        files = product_files(root)
        try:
            commit = command(["git", "rev-parse", "HEAD"], root).strip()
            dirty = bool(command(["git", "status", "--porcelain", "--untracked-files=all", "--", "."], root).strip())
        except ContractError:
            commit, dirty = None, True
        for name in files:
            (output / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(root / name, output / name)
        require(product_files(root) == files, "스냅샷 중 제품 소스가 변경되었습니다. 다시 준비하세요")
        identity = {"kind": "worktree", "commit": commit, "path": str(root), "dirty": dirty}
    require(product_files(output) == files, "제품 스냅샷 복사 불일치")
    package = tomllib.loads((output / "Cargo.toml").read_text())["package"]
    require(package["name"] == "codemap-search", "codemap-search crate가 아닙니다")
    return {**identity, "source_sha256": digest(files), "files": files, "version": package["version"]}


def build_product(target: dict, emit=print) -> dict:
    builds = settings.CACHE / "products"
    builds.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="snapshot-", dir=builds) as temporary:
        source = Path(temporary) / "source"
        identity = snapshot_product(target, source)
        rustc = command(["rustc", "-Vv"]).strip()
        host = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
        key = digest({"source": identity["source_sha256"], "rustc": rustc, "cargo": command(["cargo", "--version"]).strip()})
        output = builds / key
        with lease(builds / f"{key}.lock"):
            record = output / "build.json"
            if record.exists():
                build = read_json(record)
                verify_product(build)
                return {**build, "identity": identity}
            if output.exists():
                previous = output.with_name(f"{key}.incomplete-{time.time_ns()}")
                output.rename(previous)
                emit(f"미완료 빌드 로그 보존: {previous}")
            output.mkdir()
            source.rename(output / "source")
            binary = output / "target" / host / "release/codemap-search"
            args = ["cargo", "build", "--release", "--locked", "--target", host,
                    "--manifest-path", str(output / "source/Cargo.toml"), "--target-dir", str(output / "target")]
            emit(f"제품 빌드: {target['name']} ({identity['source_sha256'][:12]})")
            started = time.monotonic()
            # Exclude ambient RUSTFLAGS/CARGO_BUILD_TARGET; the recorded command owns build settings.
            env = {key: value for key, value in os.environ.items() if key in
                   {"PATH", "HOME", "USER", "TMPDIR", "LANG", "LC_ALL", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN"}}
            with (output / "build.stdout.log").open("w") as stdout, (output / "build.stderr.log").open("w") as stderr:
                result = subprocess.run(args, cwd=output / "source", env=env, stdout=stdout, stderr=stderr, timeout=1800)
            require(result.returncode == 0 and binary.is_file(), f"제품 빌드 실패: {output / 'build.stderr.log'}")
            build = {"identity": identity, "source_snapshot": str(output / "source"), "command": args,
                     "rustc": rustc, "binary": str(binary), "binary_sha256": file_digest(binary),
                     "elapsed_seconds": time.monotonic() - started, "exit_code": result.returncode}
            verify_product(build)
            write_json(record, build, exclusive=True)
            return build


def verify_product(build: dict):
    require(file_digest(Path(build["binary"])) == build["binary_sha256"], "제품 바이너리 변경")
    require(digest(product_files(Path(build["source_snapshot"]))) == build["identity"]["source_sha256"], "제품 스냅샷 변경")
