#!/usr/bin/env python3
"""Attest that clean B2 differs from c160dee only by the read-only startup path."""
from __future__ import annotations

import difflib
import hashlib
import os
import pathlib
import subprocess
import sys
import tempfile

from common import (
    HARNESS_ROOT,
    PRODUCT_ROOT,
    QUALITY_ROOT,
    load_json,
    sha256,
    tree_digest,
    write_json,
)


EXPECTED_SOURCE_COMMIT = "c160dee10f400950eb141e09e284d4d930f44ce6"
EXPECTED_SOURCE_GIT_TREE = "522317026e29186a704c370cbcee161f20a3e3e8"
EXPECTED_SOURCE_TAR_SHA256 = "ce3116241ba895051316b618eb5f8d403e4b236072677de7fc496c9eaca09d00"
EXPECTED_SOURCE_TREE_DIGEST = "429e16601421dcde861abfed0dc72aece4739dfa9ea69ca25502fb477e002999"
EXPECTED_PATCH_SHA256 = "0c23bc2ed8073010ebd7ed8be02d4a5d45f61f4aae6ef6d87ca9f3fe3e1cd8b5"
EXPECTED_CHANGED = {
    "src/index/engine.rs",
    "src/index/supervisor.rs",
    "src/main.rs",
}
FORBIDDEN = (
    "CODEMAP_TASTE", "candidate-taste", "suppress_low_novelty_hint",
    "C2-01", "C2-02", "C2-03", "C2-04", "C2-06", "C2-07",
)


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def files(root: pathlib.Path) -> dict[str, pathlib.Path]:
    return {str(path.relative_to(root)): path for path in root.rglob("*") if path.is_file()}


def tar_stream_sha256(root: pathlib.Path) -> str:
    """Hash the exact deterministic tar stream used in the source preparation audit."""
    environment = dict(os.environ, LC_ALL="C")
    with tempfile.TemporaryFile() as stderr_file:
        process = subprocess.Popen(
            ["tar", "-cf", "-", "."],
            cwd=root,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=stderr_file,
        )
        assert process.stdout is not None
        digest = hashlib.sha256()
        try:
            for chunk in iter(lambda: process.stdout.read(1024 * 1024), b""):
                digest.update(chunk)
            returncode = process.wait()
        except BaseException:
            process.kill()
            process.wait()
            raise
        stderr_file.seek(0)
        stderr = stderr_file.read().decode("utf-8", errors="replace")
    if returncode:
        raise SystemExit(f"could not create B2 source tar stream: {stderr.strip()}")
    return digest.hexdigest()


def git_tree(commit: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(PRODUCT_ROOT), "rev-parse", f"{commit}^{{tree}}"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode:
        raise SystemExit(f"could not resolve B2 source commit tree: {result.stderr.strip()}")
    return result.stdout.strip()


def rustc_vv_from_info(path: pathlib.Path) -> tuple[dict[str, object], str]:
    info = load_json(path)
    outputs = info.get("outputs") if isinstance(info, dict) else None
    if not isinstance(outputs, dict):
        raise SystemExit(f"invalid Cargo rustc evidence: {path}")
    rustc_outputs = [
        output.get("stdout")
        for output in outputs.values()
        if isinstance(output, dict) and isinstance(output.get("stdout"), str)
        and output["stdout"].startswith("rustc ")
    ]
    if len(rustc_outputs) != 1:
        raise SystemExit(f"Cargo rustc evidence does not contain one rustc -Vv result: {path}")
    return info, rustc_outputs[0]


def current_rustc_vv() -> str:
    result = subprocess.run(["rustc", "-Vv"], check=False, capture_output=True, text=True)
    if result.returncode:
        raise SystemExit(f"could not read active Rust toolchain: {result.stderr.strip()}")
    return result.stdout


def main() -> int:
    new = QUALITY_ROOT / "b2/source/apps/codemap-search"
    with tempfile.TemporaryDirectory(prefix="b2-c160dee-archive-") as temporary:
        archive_root = pathlib.Path(temporary)
        archive_path = archive_root / "source.tar"
        with archive_path.open("wb") as archive_stream:
            archived = subprocess.run(
                [
                    "git", "-C", str(PRODUCT_ROOT), "archive", "--format=tar",
                    EXPECTED_SOURCE_COMMIT, "apps/codemap-search",
                ],
                stdout=archive_stream,
                stderr=subprocess.PIPE,
                check=False,
            )
        if archived.returncode:
            raise SystemExit(f"could not archive B2 source commit: {archived.stderr.decode('utf-8', errors='replace').strip()}")
        extracted = subprocess.run(
            ["tar", "-xf", str(archive_path), "-C", str(archive_root)],
            capture_output=True,
            check=False,
        )
        if extracted.returncode:
            raise SystemExit(f"could not extract B2 source commit archive: {extracted.stderr.decode('utf-8', errors='replace').strip()}")
        authoritative = archive_root / "apps/codemap-search"
        authoritative_files, new_files = files(authoritative), files(new)
        if set(authoritative_files) != set(new_files):
            raise SystemExit("clean B2 file membership differs from the c160dee Git object")
        changed = {
            relative for relative in authoritative_files
            if authoritative_files[relative].read_bytes() != new_files[relative].read_bytes()
        }
        if changed != EXPECTED_CHANGED:
            raise SystemExit(f"unexpected clean B2 changed files: {sorted(changed)}")
        patch_lines = []
        for relative in sorted(changed):
            before = authoritative_files[relative].read_text(encoding="utf-8").splitlines(keepends=True)
            after = new_files[relative].read_text(encoding="utf-8").splitlines(keepends=True)
            patch_lines.extend(difflib.unified_diff(before, after, fromfile=f"c160dee/{relative}", tofile=f"clean-b2/{relative}"))
        patch_bytes = "".join(patch_lines).encode("utf-8")
        patch_path = HARNESS_ROOT / "provenance/b2-clean-runtime.patch"
        reviewed_patch_bytes = patch_path.read_bytes()
        patch_sha256 = hashlib.sha256(reviewed_patch_bytes).hexdigest()
        if (
            patch_bytes != reviewed_patch_bytes
            or hashlib.sha256(patch_bytes).hexdigest() != EXPECTED_PATCH_SHA256
            or patch_sha256 != EXPECTED_PATCH_SHA256
        ):
            raise SystemExit("clean B2 in-memory diff does not match the reviewed startup-only patch bytes and digest")
        authoritative_lock = authoritative / "Cargo.lock"
        new_lock = new / "Cargo.lock"
        if (
            not authoritative_lock.is_file()
            or not new_lock.is_file()
            or authoritative_lock.read_bytes() != new_lock.read_bytes()
        ):
            raise SystemExit("Cargo.lock differs from the reviewed source Git object")
        baseline_lock_sha256 = sha256(authoritative_lock)

    source_root = QUALITY_ROOT / "b2/source"
    source_tar_sha256 = tar_stream_sha256(source_root)
    source_tree_digest = tree_digest(source_root)
    if source_tar_sha256 != EXPECTED_SOURCE_TAR_SHA256:
        raise SystemExit("clean B2 source tar stream digest does not match the reviewed source")
    if source_tree_digest != EXPECTED_SOURCE_TREE_DIGEST:
        raise SystemExit("clean B2 common.tree_digest does not match the reviewed source")
    source_git_tree = git_tree(EXPECTED_SOURCE_COMMIT)
    if source_git_tree != EXPECTED_SOURCE_GIT_TREE:
        raise SystemExit("source commit no longer resolves to the reviewed Git tree")

    new_text = "\n".join(path.read_text(encoding="utf-8", errors="replace") for path in new_files.values())
    forbidden_hits = [marker for marker in FORBIDDEN if marker in new_text]
    if forbidden_hits:
        raise SystemExit(f"candidate marker in clean B2 source: {forbidden_hits}")
    if new_text.count("CODEMAP_BASELINE_READ_ONLY") != 1:
        raise SystemExit("clean B2 activation environment must occur exactly once")
    probe_path = HARNESS_ROOT / "reports/b2-mcp-probe.json"
    probe = load_json(probe_path)
    runtime = load_json(HARNESS_ROOT / "config/b2-runtime.json")
    runtime_binary = pathlib.Path(runtime["binary_path"])
    build_roots = (
        QUALITY_ROOT / "b2/build-evidence/target-build1",
        QUALITY_ROOT / "b2/target",
    )
    builds = []
    for build_root in build_roots:
        binary = build_root / "release/codemap-search"
        rustc_info_path = build_root / ".rustc_info.json"
        if not binary.is_file() or not rustc_info_path.is_file():
            raise SystemExit(f"missing reproducible-build evidence under {build_root}")
        rustc_info, rustc_vv = rustc_vv_from_info(rustc_info_path)
        builds.append({
            "cargo_target_dir": str(build_root),
            "binary_path": str(binary),
            "binary_sha256": sha256(binary),
            "cargo_rustc_info_path": str(rustc_info_path),
            "cargo_rustc_info_sha256": sha256(rustc_info_path),
            "rustc_fingerprint": rustc_info.get("rustc_fingerprint"),
            "rustc_vv": rustc_vv,
            "rustc_vv_sha256": sha256_text(rustc_vv),
        })
    if runtime_binary.resolve() != pathlib.Path(builds[1]["binary_path"]).resolve():
        raise SystemExit("B2 runtime binary is not the second reproducible-build output")
    if builds[0]["binary_sha256"] != builds[1]["binary_sha256"]:
        raise SystemExit("the two clean B2 build binaries have different SHA-256 digests")
    comparison = subprocess.run(
        ["cmp", "-s", builds[0]["binary_path"], builds[1]["binary_path"]],
        check=False,
    )
    if comparison.returncode:
        raise SystemExit("the two clean B2 build binaries are not byte-identical")
    if probe.get("passed") is not True or probe.get("binary_sha256") != runtime["binary_sha256"]:
        raise SystemExit("B2 probe did not pass for the supplied binary")
    if runtime["binary_sha256"] != builds[1]["binary_sha256"]:
        raise SystemExit("B2 runtime contract hash does not match the second reproducible-build output")

    active_rustc_vv = current_rustc_vv()
    if any(build["rustc_vv"] != active_rustc_vv for build in builds):
        raise SystemExit("active Rust toolchain does not match the toolchain recorded by both builds")

    attestation = {
        "schema_version": 1,
        "verdict": "verified-clean-baseline",
        "source_commit": EXPECTED_SOURCE_COMMIT,
        "source_git_tree": source_git_tree,
        "source_hashes": {
            "tar_stream": {
                "definition": "SHA-256 of stdout from (cd b2/source && LC_ALL=C tar -cf - .)",
                "sha256": source_tar_sha256,
            },
            "common_tree_digest": {
                "definition": "common.tree_digest(b2/source): SHA-256 of sorted relative paths plus NUL and each file SHA-256, with no exclusions",
                "sha256": source_tree_digest,
            },
        },
        "binary_path": str(runtime_binary),
        "binary_sha256": builds[1]["binary_sha256"],
        "build_count": len(builds),
        "builds": builds,
        "binary_comparison": {
            "command": "cmp -s <build1 binary> <build2 binary>",
            "exit_code": comparison.returncode,
            "bytes_identical": True,
        },
        "cargo_lock": {
            "source_path": str(new_lock),
            "source_sha256": sha256(new_lock),
            "baseline_identity": f"git:{EXPECTED_SOURCE_COMMIT}:apps/codemap-search/Cargo.lock",
            "baseline_sha256": baseline_lock_sha256,
            "matches_baseline": True,
        },
        "toolchain": {
            "active_rustc_vv": active_rustc_vv,
            "active_rustc_vv_sha256": sha256_text(active_rustc_vv),
            "both_builds_match_active_rustc": True,
            "build_rustc_info_sha256": [build["cargo_rustc_info_sha256"] for build in builds],
        },
        "probe_builds": 0,
        "activation_environment": {"CODEMAP_BASELINE_READ_ONLY": "1"},
        "mcp_command_tail": ["mcp"],
        "candidate_code_present": False,
        "candidate_environment_present": False,
        "patch_scope": "read-only startup lock fix only; no candidate code, mode, metrics, prompt, schema, ranking, or index change",
        "source_diff_sha256": patch_sha256,
        "audit_evidence": [
            {"check": "file membership", "result": "identical"},
            {"check": "changed files", "result": sorted(changed)},
            {"check": "forbidden candidate markers", "result": []},
            {"check": "activation variable occurrences", "result": 1},
            {"check": "model-free MCP probe", "result": "passed"},
        ],
        "probe_report_path": str(probe_path),
        "probe_report_sha256": sha256(probe_path),
    }
    output = HARNESS_ROOT / "provenance/b2-clean-runtime-attestation.json"
    write_json(output, attestation)
    runtime["attestation_path"] = str(output)
    runtime["attestation_sha256"] = sha256(output)
    write_json(HARNESS_ROOT / "config/b2-runtime.json", runtime)
    print(output)
    return 0


if __name__ == "__main__": raise SystemExit(main())
