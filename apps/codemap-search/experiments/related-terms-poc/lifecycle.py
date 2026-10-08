#!/usr/bin/env python3
"""Lifecycle and offline-serving observations for index-time statistics (offline).

    python3 lifecycle.py run       # scratch copy of a pinned target: add/change, delete,
                                   # restart, failed publication, recovery, encoding failure
    python3 lifecycle.py offline   # sandboxed lookups on the pinned targets (no network,
                                   # no source or index-segment reads) + negative controls

Writes results/lifecycle-evidence.json and results/offline-evidence.json. Destructive steps
touch only the scratch copy under .codemap/poc/related-terms/workspaces/scratch/.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

from rtpoc_common import DATA_ROOT, EXPERIMENT_DIR, REPO_ROOT, RESULTS_DIR, read_json, repo_relative, sha256_file, write_json
from workspaces import BINARY, checkout_dir, manifest, state_dir

SCRATCH_ROOT = DATA_ROOT / "workspaces" / "scratch"
POLICY_PATH = EXPERIMENT_DIR / "contracts" / "lookup-policy.json"
PROBE_WORDS = ["rtpocprobealpha", "rtpocprobebeta"]
# Existing files of the pinned flask checkout; the probe text is appended to copies only.
FILE_A = "src/flask/helpers.py"
FILE_B = "src/flask/cli.py"
APPEND_A = '''

def rtpoc_probe_alpha_fn():
    """rtpocprobealpha twice in one docstring: rtpocprobealpha."""
    return "rtpocprobealpha rtpocprobealpha"
'''
APPEND_B = '''

RTPOC_PROBE = "rtpocprobealpha"
'''
APPEND_A_BETA = '''

RTPOC_PROBE_BETA = "rtpocprobebeta"
'''


def dataset_sqlite() -> Path:
    handoff = read_json(RESULTS_DIR / "dataset-handoff.json")
    return REPO_ROOT / handoff["sqlite_path"]


def without_local_paths(text: str) -> str | None:
    """Recorded evidence keeps repository-relative paths only."""
    return text.strip().replace(str(REPO_ROOT.resolve()), ".").replace(str(REPO_ROOT), ".") or None


def rtpoc(args: list[str], cwd: Path, env: dict | None = None) -> tuple[int, dict | None, str]:
    process = subprocess.run([str(BINARY), *args], cwd=cwd, env=env, capture_output=True, text=True)
    try:
        payload = json.loads(process.stdout) if process.stdout.strip() else None
    except json.JSONDecodeError:
        payload = None
    return process.returncode, payload, process.stderr.strip()


def observe(name: str, workspace: Path, state: Path, env: dict, *, index_args: list[str] | None) -> dict:
    """Optionally run `index`, then read serving-side state and an index-side audit."""
    step: dict = {"step": name}
    if index_args is not None:
        code, payload, stderr = rtpoc(["index", "--state", str(state), *index_args], workspace, env)
        step["index"] = {"args": index_args, "exit": code, "stderr": (without_local_paths(stderr) or "")[-300:] or None}
        if payload:
            finalization = payload["finalization"]
            step["index"]["published_generation"] = payload["index_generation"]
            step["index"]["opstamp"] = payload["index_opstamp"]
            step["index"]["index_refresh_committed"] = finalization["index_refresh_committed"]
            # Postings of deleted documents still present in segments and skipped by the union.
            step["index"]["deleted_postings_skipped"] = finalization["deleted_postings_skipped"]
            step["index"]["deleted_docs_by_segment"] = [
                (segment.get("deletes") or {}).get("num_deleted_docs", 0) for segment in finalization["segments"]
            ]
            step["index"]["materialize_ms"] = round(finalization["materialize_ms"], 2)
    published = state / "published.json"
    step["published_json_sha256"] = sha256_file(published) if published.exists() else None
    code, served, _ = rtpoc(["stats-words", "--state", str(state), *PROBE_WORDS], workspace)
    step["serving"] = {
        "status": served["repository"]["status"],
        "detail": served["repository"]["detail"],
        "published_generation": served["repository"]["published_generation"],
        "current_generation": served["repository"]["current_generation"],
        "live_files": served["repository"]["live_files"],
        "probe_file_counts": served["file_counts"],
    }
    if served["repository"]["status"] == "ready":
        code, audit, stderr = rtpoc(["verify-stats", "--state", str(state)], workspace)
        step["index_side_audit"] = {
            key: audit[key]
            for key in ("status", "recount_mismatches", "missing_live_terms", "deleted_only_terms_absent", "denominator")
        }
    code, raw, _ = rtpoc(["raw-doc-freq", "--state", str(state), *PROBE_WORDS], workspace)
    step["raw_tantivy_doc_freq_including_deleted"] = raw["words"] if raw else None
    step["segments"] = raw["segments"] if raw else None
    return step


def run_lifecycle() -> int:
    target = next(t for t in manifest()["targets"] if t["name"] == "flask")
    scratch = SCRATCH_ROOT / "flask-lifecycle"
    state = SCRATCH_ROOT / "flask-lifecycle-state"
    shutil.rmtree(scratch, ignore_errors=True)
    shutil.rmtree(state, ignore_errors=True)
    shutil.copytree(checkout_dir(target), scratch, symlinks=True)
    home = state / "codemap-home"
    home.mkdir(parents=True)
    env = {**os.environ, "CODEMAP_HOME": str(home)}
    file_a, file_b = scratch / FILE_A, scratch / FILE_B
    steps = []

    steps.append(observe("L0 baseline full build", scratch, state, env, index_args=[]))
    with file_a.open("a", encoding="utf-8") as handle:
        handle.write(APPEND_A)
    with file_b.open("a", encoding="utf-8") as handle:
        handle.write(APPEND_B)
    steps.append(observe("L1 change: probe in A (docstring x2 + literal x2) and B (literal)", scratch, state, env, index_args=[]))
    file_b.unlink()
    steps.append(observe("L2 delete B", scratch, state, env, index_args=[]))
    steps.append(observe("L3 restart: fresh processes read persisted state, no index command", scratch, state, env, index_args=None))
    with file_a.open("a", encoding="utf-8") as handle:
        handle.write(APPEND_A_BETA)
    steps.append(observe("L4 change A + injected failure before publication", scratch, state, env, index_args=["--fail-before-publish"]))
    steps.append(observe("L5 recovery: index again", scratch, state, env, index_args=[]))
    file_a.write_bytes(file_a.read_bytes() + b"\n# invalid utf-8: \xff\xfe\n")
    steps.append(observe("L6 A becomes invalid UTF-8 (encoding exclusion)", scratch, state, env, index_args=[]))

    expectations = [
        ("L1 counts unique files across fields/repeats", steps[1]["serving"]["probe_file_counts"]["rtpocprobealpha"] == 2),
        ("L1 denominator unchanged", steps[1]["serving"]["live_files"] == steps[0]["serving"]["live_files"]),
        ("L2 delete subtracts the file", steps[2]["serving"]["probe_file_counts"]["rtpocprobealpha"] == 1),
        ("L2 denominator minus one", steps[2]["serving"]["live_files"] == steps[0]["serving"]["live_files"] - 1),
        ("L3 restart keeps the same published pair", steps[3]["published_json_sha256"] == steps[2]["published_json_sha256"]
         and steps[3]["serving"]["status"] == "ready"),
        ("L4 failed publication leaves published.json untouched", steps[4]["published_json_sha256"] == steps[3]["published_json_sha256"]),
        ("L4 newer index with old counts is reported stale, not ready", steps[4]["serving"]["status"] == "stale"
         and steps[4]["serving"]["probe_file_counts"]["rtpocprobebeta"] is None),
        ("L5 recovery publishes matching counts", steps[5]["serving"]["status"] == "ready"
         and steps[5]["serving"]["probe_file_counts"]["rtpocprobebeta"] == 1),
        ("L6 encoding exclusion removes the file's words", steps[6]["serving"]["probe_file_counts"]["rtpocprobealpha"] == 0
         and steps[6]["serving"]["live_files"] == steps[0]["serving"]["live_files"] - 2),
        ("every ready step passes the index-side audit", all(
            step.get("index_side_audit", {}).get("status", "pass") == "pass" for step in steps)),
    ]
    evidence = {
        "scratch_copy_of": {"name": target["name"], "commit": target["commit"]},
        "scratch_workspace": repo_relative(scratch),
        "scratch_state": repo_relative(state),
        "binary_sha256": sha256_file(BINARY),
        "probe_words": PROBE_WORDS,
        "probe_edits": {"A": FILE_A, "B": FILE_B, "A_append": APPEND_A, "B_append": APPEND_B, "A_beta_append": APPEND_A_BETA},
        "steps": steps,
        "expectations": [{"check": name, "pass": passed} for name, passed in expectations],
        "status": "pass" if all(passed for _, passed in expectations) else "fail",
        "not_exercised": ["parser extraction failure (no deterministic trigger without editing product code)",
                          "concurrent second writer (single-writer procedure is enforced by the operator)"],
    }
    write_json(RESULTS_DIR / "lifecycle-evidence.json", evidence)
    print(f"lifecycle: {evidence['status']}")
    for item in evidence["expectations"]:
        print(f"  {'ok' if item['pass'] else 'FAIL'} {item['check']}")
    return 0 if evidence["status"] == "pass" else 1


def sandbox_profile(deny_read_roots: list[Path], index_dirs: list[Path]) -> str:
    rules = ["(version 1)", "(allow default)", "(deny network*)"]
    for root in deny_read_roots:
        rules.append(f'(deny file-read* (subpath "{root.resolve()}"))')
    for index_dir in index_dirs:
        resolved = index_dir.resolve()
        rules.append(f'(deny file-read* (subpath "{resolved}"))')
        # The serving path may read only the commit point to detect an unpublished generation.
        rules.append(f'(allow file-read* (literal "{resolved / "meta.json"}"))')
    return "".join(rules)


def run_offline() -> int:
    targets = manifest()["targets"]
    dataset = dataset_sqlite()
    dataset_hash_before = sha256_file(dataset)
    terms = ["delete", "retry", "eventLoop", "res", "authz"]
    checkouts = [checkout_dir(target) for target in targets]
    profile = sandbox_profile(
        [checkouts[0].parent],
        [state_dir(target) / "index" for target in targets],
    )
    results = {}
    for target in targets:
        state = state_dir(target)
        base = ["lookup", "--dataset", str(dataset), "--policy", str(POLICY_PATH), "--state", str(state), "--repeat", "3"]
        plain = subprocess.run([str(BINARY), *base, *terms], capture_output=True, text=True, cwd=EXPERIMENT_DIR)
        sandboxed = subprocess.run(
            ["sandbox-exec", "-p", profile, str(BINARY), *base, *terms], capture_output=True, text=True, cwd=EXPERIMENT_DIR
        )

        def strip_timing(stdout: str) -> list[dict]:
            outputs = [json.loads(line) for line in stdout.splitlines() if line.strip()]
            for output in outputs:
                output["lookup_us"] = None
            return outputs

        plain_outputs = strip_timing(plain.stdout) if plain.returncode == 0 else []
        sandboxed_outputs = strip_timing(sandboxed.stdout) if sandboxed.returncode == 0 else []
        audit = subprocess.run(
            ["sandbox-exec", "-p", profile, str(BINARY), "verify-stats", "--state", str(state)],
            capture_output=True, text=True, cwd=EXPERIMENT_DIR,
        )
        index = subprocess.run(
            ["sandbox-exec", "-p", profile, str(BINARY), "index", "--state", str(state)],
            capture_output=True, text=True, cwd=checkout_dir(target),
            env={**os.environ, "CODEMAP_HOME": str(state / "codemap-home")},
        )
        results[target["name"]] = {
            "sandboxed_lookup_exit": sandboxed.returncode,
            "sandboxed_lookup_stderr": (without_local_paths(sandboxed.stderr) or "")[-300:] or None,
            "plain_lookup_exit": plain.returncode,
            "outputs_identical_to_unsandboxed": bool(plain_outputs) and plain_outputs == sandboxed_outputs,
            "repository_status": sandboxed_outputs[0]["repository"]["status"] if sandboxed_outputs else None,
            "final_terms": {o["input"]: o["final_terms"] for o in sandboxed_outputs},
            "negative_control_verify_stats_exit": audit.returncode,
            "negative_control_verify_stats_stderr": (without_local_paths(audit.stderr) or "")[-200:],
            "negative_control_index_exit": index.returncode,
            "negative_control_index_stderr": (without_local_paths(index.stderr) or "")[-200:],
        }
    dataset_hash_after = sha256_file(dataset)
    is_pass = (
        all(r["sandboxed_lookup_exit"] == 0 and r["outputs_identical_to_unsandboxed"]
            and r["repository_status"] == "ready"
            and r["negative_control_verify_stats_exit"] != 0 and r["negative_control_index_exit"] != 0
            for r in results.values())
        and dataset_hash_before == dataset_hash_after
    )
    evidence = {
        "method": "macOS sandbox-exec profile: deny all network; deny reads of every checkout and of each index "
                  "directory except meta.json. Lookup must succeed and equal the unsandboxed output; index-side "
                  "commands under the same profile must fail (proves the denial is effective).",
        "sandbox_profile": profile.replace(str(DATA_ROOT.resolve()), "<data-root>"),
        "terms": terms,
        "repeat": 3,
        "dataset_sha256_before": dataset_hash_before,
        "dataset_sha256_after": dataset_hash_after,
        "targets": results,
        "status": "pass" if is_pass else "fail",
    }
    write_json(RESULTS_DIR / "offline-evidence.json", evidence)
    print(f"offline: {evidence['status']}")
    return 0 if is_pass else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["run", "offline"])
    args = parser.parse_args()
    return run_lifecycle() if args.command == "run" else run_offline()


if __name__ == "__main__":
    sys.exit(main())
