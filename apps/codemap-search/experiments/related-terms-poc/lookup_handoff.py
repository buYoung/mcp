#!/usr/bin/env python3
"""Re-verify child-02 artifacts and write results/lookup-handoff.json (offline).

    python3 lookup_handoff.py

Runs the two required Cargo checks, re-audits every pinned target's published statistics,
statically audits the serving entry points, and records only observed outcomes.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys

from rtpoc_common import EXPERIMENT_DIR, REPO_ROOT, RESULTS_DIR, read_json, repo_relative, sha256_file, write_json
from workspaces import BINARY, INDEX_STATS_PATH, MANIFEST_PATH, manifest, state_dir

HANDOFF_PATH = RESULTS_DIR / "lookup-handoff.json"
POLICY_PATH = EXPERIMENT_DIR / "contracts" / "lookup-policy.json"
OUTPUT_SCHEMA_PATH = EXPERIMENT_DIR / "contracts" / "lookup.schema.json"
SERVING_MODULES = ["src/lookup.rs", "src/dataset.rs", "src/stats.rs", "src/normalize.rs"]
# Anything that would open the Tantivy index, scan postings, walk/read source, spawn
# processes, or reach the network. `meta.json` is read via std::fs in stats.rs by design.
FORBIDDEN = [
    r"\bIndex::", r"open_in_dir", r"\bsearcher\b", r"Searcher", r"read_postings", r"inverted_index",
    r"TantivySearchEngine", r"build_walker", r"WalkBuilder", r"walkdir", r"read_dir", r"std::process",
    r"Command::new", r"TcpStream", r"reqwest", r"https?://", r"vocabulary::", r"\.search\(",
]
CARGO_CHECKS = [
    "cargo check --manifest-path apps/codemap-search/experiments/related-terms-poc/Cargo.toml",
    "cargo check --manifest-path apps/codemap-search/Cargo.toml",
]


def run_cargo_checks() -> list[dict]:
    results = []
    for command in CARGO_CHECKS:
        process = subprocess.run(command.split(), cwd=REPO_ROOT, capture_output=True, text=True)
        # Recorded output must not carry the local absolute checkout path.
        tail = [line.replace(str(REPO_ROOT), ".") for line in process.stderr.splitlines() if line.strip()][-3:]
        results.append({"cwd": ".", "command": command, "exit": process.returncode, "stderr_tail": tail})
    return results


def serving_audit() -> dict:
    sources = {path: (EXPERIMENT_DIR / path).read_text(encoding="utf-8") for path in SERVING_MODULES}
    main = (EXPERIMENT_DIR / "src/main.rs").read_text(encoding="utf-8")
    run_lookup = re.search(r"fn run_lookup\(.*?\n}\n", main, re.S).group(0)
    stats_words = re.search(r'"stats-words" => \{.*?\n        \}', main, re.S).group(0)
    sources["src/main.rs::run_lookup"] = run_lookup
    sources["src/main.rs::stats-words"] = stats_words
    findings = [
        {"location": location, "pattern": pattern, "line": line.strip()}
        for location, text in sources.items()
        for line in text.splitlines()
        if not line.strip().startswith("//")
        for pattern in FORBIDDEN
        if re.search(pattern, line)
    ]
    return {
        "entry_points": ["rtpoc lookup", "rtpoc stats-words"],
        "audited": sorted(sources),
        "file_reads_by_design": [
            "dataset SQLite (read-only open, validated meta)",
            "lookup policy JSON",
            "<state>/published.json",
            "<state>/index/meta.json (commit point only, to detect an unpublished generation)",
            "<state>/stats/stats-<generation>.sqlite3 (read-only, SHA-256 checked against published.json)",
        ],
        "forbidden_patterns": FORBIDDEN,
        "findings": findings,
        "status": "pass" if not findings else "fail",
    }


def reverify_targets(index_stats: dict) -> dict:
    results = {}
    for target in manifest()["targets"]:
        name = target["name"]
        state = state_dir(target)
        recorded = index_stats["targets"][name]["published"]
        served = json.loads(
            subprocess.run([str(BINARY), "stats-words", "--state", str(state)], capture_output=True, text=True).stdout
        )["repository"]
        audit = subprocess.run([str(BINARY), "verify-stats", "--state", str(state)], capture_output=True, text=True)
        audit_payload = json.loads(audit.stdout)
        stats_path = state / recorded["stats_file"]
        results[name] = {
            "commit": target["commit"],
            "state_dir": repo_relative(state),
            "index_dir": repo_relative(state / "index"),
            "index_generation": recorded["index_generation"],
            "index_opstamp": recorded["index_opstamp"],
            "published_json_sha256": sha256_file(state / "published.json"),
            "published_json_unchanged_since_indexing": sha256_file(state / "published.json") == recorded["published_json_sha256"],
            "index_meta_sha256": sha256_file(state / "index" / "meta.json"),
            "stats_path": repo_relative(stats_path),
            "stats_sha256": sha256_file(stats_path),
            "live_files": recorded["live_files"],
            "vocabulary_size": recorded["vocabulary_size"],
            "serving_status": served["status"],
            "generation_matches": served["published_generation"] == served["current_generation"] == recorded["index_generation"],
            "verify_stats": {key: audit_payload[key] for key in ("status", "recount_mismatches", "missing_live_terms", "denominator")},
            "effective_config": index_stats["targets"][name]["effective_config"],
        }
    return results


def main() -> int:
    dataset_handoff = read_json(RESULTS_DIR / "dataset-handoff.json")
    index_stats = read_json(INDEX_STATS_PATH)
    lifecycle = read_json(RESULTS_DIR / "lifecycle-evidence.json")
    offline = read_json(RESULTS_DIR / "offline-evidence.json")
    policy = read_json(POLICY_PATH)
    cargo = run_cargo_checks()
    audit = serving_audit()
    targets = reverify_targets(index_stats)
    normalize_check = json.loads(
        subprocess.run(
            [str(BINARY), "normalize-check", "--dataset", str(REPO_ROOT / dataset_handoff["sqlite_path"])],
            capture_output=True, text=True,
        ).stdout
    )
    dataset_sqlite_sha256 = sha256_file(REPO_ROOT / dataset_handoff["sqlite_path"])
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, capture_output=True, text=True).stdout.strip()
    checks = {
        "dataset_handoff_integrity": dataset_handoff["integrity_status"]["status"] == "pass",
        "dataset_sqlite_hash_unchanged": dataset_sqlite_sha256 == dataset_handoff["artifact_hashes"]["dataset.sqlite3"],
        "normalization_conformance": normalize_check["status"] == "pass",
        "cargo_checks": all(result["exit"] == 0 for result in cargo),
        "targets_ready_and_matching": all(
            t["serving_status"] == "ready" and t["generation_matches"] and t["published_json_unchanged_since_indexing"]
            and t["verify_stats"]["status"] == "pass" and t["live_files"] > 0
            for t in targets.values()
        ),
        "population_visible": all(
            t["population"]["indexed_primary_files"] == t["population"]["tracked_primary_files"] > 0
            and t["population"]["indexed_not_tracked"] == 0
            for t in index_stats["targets"].values()
        ),
        "lifecycle": lifecycle["status"] == "pass",
        "offline_serving": offline["status"] == "pass",
        "serving_path_audit": audit["status"] == "pass",
    }
    sources = sorted(path.relative_to(EXPERIMENT_DIR).as_posix() for path in (EXPERIMENT_DIR / "src").glob("*.rs"))
    write_json(
        HANDOFF_PATH,
        {
            "handoff": "related-terms-poc/lookup",
            "dataset_handoff_path": repo_relative(RESULTS_DIR / "dataset-handoff.json"),
            "dataset_handoff_sha256": sha256_file(RESULTS_DIR / "dataset-handoff.json"),
            "dataset_id": dataset_handoff["dataset_id"],
            "normalization_version": dataset_handoff["normalization_version"],
            "dataset_sqlite_path": dataset_handoff["sqlite_path"],
            "dataset_sqlite_sha256": dataset_sqlite_sha256,
            "policy": {"policy_id": policy["policy_id"], "path": repo_relative(POLICY_PATH), "sha256": sha256_file(POLICY_PATH)},
            "stats_policies": index_stats["stats_policies"],
            "crate": {
                "manifest": repo_relative(EXPERIMENT_DIR / "Cargo.toml"),
                "cargo_toml_sha256": sha256_file(EXPERIMENT_DIR / "Cargo.toml"),
                "cargo_lock_sha256": sha256_file(EXPERIMENT_DIR / "Cargo.lock"),
                "source_sha256": {path: sha256_file(EXPERIMENT_DIR / path) for path in sources},
                "repository_head": head,
                "note": "The experiment directory is untracked in this checkout; source identity is the per-file SHA-256.",
            },
            "build_profile": "release",
            "binary": {"path": repo_relative(BINARY), "sha256": sha256_file(BINARY)},
            "workspace_manifest_path": repo_relative(MANIFEST_PATH),
            "workspace_manifest_sha256": sha256_file(MANIFEST_PATH),
            "index_stats_path": repo_relative(INDEX_STATS_PATH),
            "index_generation": {name: t["index_generation"] for name, t in targets.items()},
            "targets": targets,
            "commands": {
                "build": "cargo build --release --manifest-path apps/codemap-search/experiments/related-terms-poc/Cargo.toml",
                "index": "cd <checkout> && CODEMAP_HOME=<state>/codemap-home <binary> index --state <state>",
                "verify_stats": "<binary> verify-stats --state <state>",
                "lookup": "<binary> lookup --dataset <dataset.sqlite3> --policy contracts/lookup-policy.json "
                          "--state <state> [--expect-dataset-id ID] [--repeat N] [--output FILE] "
                          "[--timing-output FILE] (--terms-file FILE | TERM...)",
                "drivers": ["python3 workspaces.py index", "python3 lifecycle.py run", "python3 lifecycle.py offline",
                            "python3 lookup_handoff.py"],
            },
            "output_schema_path": repo_relative(OUTPUT_SCHEMA_PATH),
            "lifecycle_evidence_path": repo_relative(RESULTS_DIR / "lifecycle-evidence.json"),
            "offline_lookup_evidence": {
                "path": repo_relative(RESULTS_DIR / "offline-evidence.json"),
                "status": offline["status"],
                "method": offline["method"],
            },
            "normalization_check": {k: v for k, v in normalize_check.items() if not k.endswith("examples")},
            "cargo_checks": cargo,
            "serving_path_audit": audit,
            "technical_status": {"status": "pass" if all(checks.values()) else "fail", "checks": checks},
            "limitations": [
                "Statistics are rebuilt in full after every committed refresh (no per-file incremental subtraction); "
                "cost was measured on four small/medium repositories only (109-820 files).",
                "Phrase candidates are judged by per-token presence; no phrase or adjacency counts exist.",
                "Serving reads index meta.json to detect a newer unpublished generation; a stale state yields no final "
                "terms rather than an older matching index/statistics pair.",
                "Readiness is per state directory; the PoC enforces one writer by procedure, not by a lock of its own.",
                "Offline evidence uses macOS sandbox-exec; behavior on other platforms was not observed.",
                "Startup/lookup cost and semantic quality are measured by child 03, not here.",
            ],
        },
    )
    print(f"lookup handoff: {'pass' if all(checks.values()) else 'FAIL'} {checks}")
    return 0 if all(checks.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
