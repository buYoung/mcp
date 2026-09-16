#!/usr/bin/env python3
"""Materialize pinned supplemental sources; never install or execute packages."""

import argparse
import base64
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import urllib.request


DIRECTORY = Path(__file__).resolve().parent


def write_checked(root, relative, data, expected):
    path = root / relative
    if not path.resolve().is_relative_to(root.resolve()) or path.is_symlink():
        raise ValueError("unsafe source path: " + relative)
    if hashlib.sha256(data).hexdigest() != expected:
        raise ValueError("source hash mismatch: " + relative)
    if path.exists() and path.read_bytes() != data:
        raise ValueError("refusing to replace different source: " + str(path))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", type=Path, required=True)
    parser.add_argument("--dependencies", type=Path, default=Path("/tmp/codemap-language-dependencies"))
    parser.add_argument("--composites", type=Path, default=Path("/tmp/codemap-language-composite-sources"))
    args = parser.parse_args()
    variants = json.loads((DIRECTORY / "inputs.json").read_text())["variants"]
    package = json.loads((DIRECTORY / "effect-source.lock.json").read_text())
    dependency_name = package["name"] + "-" + package["version"]
    dependency = args.dependencies / dependency_name
    missing = any(not (dependency / path).is_file() for path in package["source_sha256"])
    if missing:
        with urllib.request.urlopen(package["tarball_url"], timeout=30) as response:
            archive = response.read()
        actual = "sha512-" + base64.b64encode(hashlib.sha512(archive).digest()).decode()
        if actual != package["integrity"] or hashlib.sha256(archive).hexdigest() != package["tarball_sha256"]:
            raise ValueError("dependency archive integrity mismatch")
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as bundle:
            for member in bundle.getmembers():
                path = PurePosixPath(member.name)
                if path.is_absolute() or ".." in path.parts:
                    raise ValueError("unsafe archive path")
                relative = str(PurePosixPath(*path.parts[1:]))
                if member.isfile() and path.parts[0] == "package" and relative in package["source_sha256"]:
                    write_checked(dependency, relative, bundle.extractfile(member).read(), package["source_sha256"][relative])
    for relative, expected in package["source_sha256"].items():
        if hashlib.sha256((dependency / relative).read_bytes()).hexdigest() != expected:
            raise ValueError("dependency source mismatch: " + relative)
    for name, spec in variants.items():
        repository = args.sources / spec["repository"]
        revision = subprocess.check_output(["git", "-C", str(repository), "rev-parse", "HEAD"], text=True).strip()
        if revision != spec["revision"]:
            raise ValueError(name + ": base checkout revision differs")
        root = args.composites / spec["composite"] if spec.get("composite") else repository
        for relative, expected in spec["source_sha256"].items():
            origin = spec.get("origins", {}).get(relative, {"repository": spec["repository"], "path": relative})
            if "dependency" in origin:
                if origin["dependency"] != dependency_name:
                    raise ValueError("unlocked dependency")
                data = (dependency / origin["path"]).read_bytes()
            else:
                data = subprocess.check_output(["git", "-C", str(args.sources / origin["repository"]), "show", revision + ":" + origin["path"]])
            write_checked(root, relative, data, expected)
        print(json.dumps({"variant": name, "files": len(spec["source_sha256"]), "target_executed": False}), flush=True)


if __name__ == "__main__":
    main()
