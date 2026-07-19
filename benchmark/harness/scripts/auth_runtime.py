#!/usr/bin/env python3
"""Create and remove an isolated OpenCode data directory containing one credential."""
from __future__ import annotations

import json
import fcntl
import os
import pathlib
import shutil
import sys
import tempfile


def provider_credential(source: pathlib.Path, provider: str) -> dict:
    if source.is_symlink() or not source.is_file():
        raise SystemExit("OpenCode auth file is unavailable")
    source_stat = source.stat()
    if source_stat.st_uid != os.getuid() or source_stat.st_mode & 0o077:
        raise SystemExit("OpenCode auth file ownership or permissions are unsafe")
    value = json.loads(source.read_text())
    credential = value.get(provider) if isinstance(value, dict) else None
    if not isinstance(credential, dict):
        raise SystemExit("required OpenCode provider credential is unavailable")
    if credential.get("type") != "api" or not isinstance(credential.get("key"), str) or not credential["key"]:
        raise SystemExit("required OpenCode provider credential is not a non-empty API key")
    return credential


def ensure_private_parent(parent: pathlib.Path) -> pathlib.Path:
    parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    if parent.is_symlink():
        raise SystemExit("auth runtime parent must not be a symlink")
    parent = parent.resolve()
    if parent.stat().st_uid != os.getuid():
        raise SystemExit("auth runtime parent ownership is unsafe")
    parent.chmod(0o700)
    return parent


def create_runtime(source: pathlib.Path, provider: str, parent: pathlib.Path, prefix: str) -> pathlib.Path:
    credential = provider_credential(source, provider)
    parent = ensure_private_parent(parent)
    if not prefix or pathlib.Path(prefix).name != prefix:
        raise SystemExit("auth runtime prefix is unsafe")
    lock_path = parent / ".create.lock"
    lock_descriptor = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o600)
    try:
        try:
            # Serialize only the short directory-creation transaction. Existing
            # session runtimes are expected while the two permitted lanes run.
            fcntl.flock(lock_descriptor, fcntl.LOCK_EX)
        except OSError as error:
            raise SystemExit("auth runtime creation lock failed") from error
        runtime = pathlib.Path(tempfile.mkdtemp(prefix=prefix, dir=parent))
        try:
            runtime.chmod(0o700)
            owner_path = runtime / ".owner.json"
            owner_descriptor = os.open(owner_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(owner_descriptor, "w") as stream:
                json.dump({"prefix": prefix, "runtime_name": runtime.name}, stream, sort_keys=True)
                stream.write("\n")
            owner_path.chmod(0o600)
            data_dir = runtime / "opencode"
            data_dir.mkdir(mode=0o700)
            auth_path = data_dir / "auth.json"
            descriptor = os.open(auth_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(descriptor, "w") as stream:
                json.dump({provider: credential}, stream, indent=2, sort_keys=True)
                stream.write("\n")
            auth_path.chmod(0o600)
            return runtime
        except BaseException:
            shutil.rmtree(runtime, ignore_errors=True)
            raise
    finally:
        os.close(lock_descriptor)


def remove_runtime(runtime: pathlib.Path, parent: pathlib.Path, expected_prefix: str) -> None:
    parent = parent.resolve(strict=True)
    if runtime.is_symlink():
        raise SystemExit("auth runtime must not be a symlink")
    runtime = runtime.resolve(strict=True)
    if runtime.parent != parent:
        raise SystemExit("auth runtime removal escaped its private parent")
    if not expected_prefix or pathlib.Path(expected_prefix).name != expected_prefix:
        raise SystemExit("auth runtime cleanup owner prefix is unsafe")
    owner_path = runtime / ".owner.json"
    if owner_path.is_symlink() or not owner_path.is_file():
        raise SystemExit("auth runtime owner metadata is absent")
    owner = json.loads(owner_path.read_text())
    if owner != {"prefix": expected_prefix, "runtime_name": runtime.name} or not runtime.name.startswith(expected_prefix):
        raise SystemExit("auth runtime cleanup ownership mismatch")
    shutil.rmtree(runtime)


def remove_matching_runtime(parent: pathlib.Path, prefix: str) -> None:
    if not parent.exists():
        return
    if parent.is_symlink() or not prefix or pathlib.Path(prefix).name != prefix:
        raise SystemExit("auth runtime prefix cleanup is unsafe")
    parent = parent.resolve(strict=True)
    matches = [path for path in parent.iterdir() if path.name.startswith(prefix)]
    if len(matches) > 1:
        raise SystemExit("multiple matching auth runtimes detected")
    if matches:
        remove_runtime(matches[0], parent, prefix)


def main() -> int:
    if len(sys.argv) == 4 and sys.argv[1] == "validate":
        provider_credential(pathlib.Path(sys.argv[2]), sys.argv[3])
        return 0
    if len(sys.argv) == 6 and sys.argv[1] == "create":
        runtime = create_runtime(pathlib.Path(sys.argv[2]), sys.argv[3], pathlib.Path(sys.argv[4]), sys.argv[5])
        print(runtime)
        return 0
    if len(sys.argv) == 5 and sys.argv[1] == "remove":
        remove_runtime(pathlib.Path(sys.argv[2]), pathlib.Path(sys.argv[3]), sys.argv[4])
        return 0
    if len(sys.argv) == 4 and sys.argv[1] == "remove-prefix":
        remove_matching_runtime(pathlib.Path(sys.argv[2]), sys.argv[3])
        return 0
    raise SystemExit(
        "usage: auth_runtime.py validate SOURCE PROVIDER | "
        "create SOURCE PROVIDER PARENT PREFIX | remove RUNTIME PARENT OWNER_PREFIX | remove-prefix PARENT PREFIX"
    )


if __name__ == "__main__":
    raise SystemExit(main())
