#!/usr/bin/env python3
"""Child-03 evaluation: frozen inputs, recorded lookups, review table, aggregation (offline).

    python3 evaluate.py select-probes        # seeded random probes from repository vocabulary
    python3 evaluate.py freeze               # results/evaluation-protocol.json (hashes everything)
    python3 evaluate.py run                  # reuse existing dataset; lookups + timing/RSS -> observations
    python3 evaluate.py review-template      # results/sample-review.tsv rows (labels filled by reviewer)
    python3 evaluate.py aggregate            # results/measurements.json + results/poc-report.json

The evaluator never edits the dataset, policy, or reader. A protocol hash mismatch stops `run`.
"""

from __future__ import annotations

import argparse
import collections
import csv
import json
import math
import os
import random
import re
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

from rtpoc_common import DATA_ROOT, EXPERIMENT_DIR, REPO_ROOT, RESULTS_DIR, read_json, repo_relative, sha256_bytes, sha256_file, write_json
from workspaces import BINARY, MANIFEST_PATH, manifest, state_dir

CASES_PATH = EXPERIMENT_DIR / "cases.json"
PROTOCOL_PATH = RESULTS_DIR / "evaluation-protocol.json"
POLICY_PATH = EXPERIMENT_DIR / "contracts" / "lookup-policy.json"
LOOKUP_HANDOFF_PATH = RESULTS_DIR / "lookup-handoff.json"
OBSERVATIONS_PATH = RESULTS_DIR / "observations.jsonl"
MEASUREMENTS_PATH = RESULTS_DIR / "measurements.json"
REVIEW_PATH = RESULTS_DIR / "sample-review.tsv"
REPORT_PATH = RESULTS_DIR / "poc-report.json"
RUN_ROOT = DATA_ROOT / "evaluation"

PROBE_SEED = 20261006
PROBE_COUNT = 30
PROBE_PATTERN = re.compile(r"^[a-z]{4,12}$")
PROBE_MIN_FILES = 3
PROBE_MIN_REPOSITORIES = 3
# English function words, language keywords/builtins, and license boilerplate. Fixed before
# sampling; the list is part of the protocol hash.
PROBE_STOPLIST = frozenset(
    """
    about above after again also always another because been before being below between both
    cannot could does doing done down during each either else enough even every from further
    have having here hers into itself just least less like many more most much must neither
    never next none nothing often once only other otherwise ours over same shall should since
    some such than that their theirs them then there these they this those though through
    under until upon very what when where whether which while whom whose will with within
    without would your yours
    async await break case catch class const continue crate default defer delete elif else
    enum export extends false final finally func function goto impl import interface lambda
    let loop match module move mut none null package private protected pub public raise
    require return self static struct super switch this throw trait true type typeof unsafe
    var void where while yield bool char string usize isize float double
    copyright license licensed licenses apache warranty warranties permission permissions
    software notice conditions limitations express implied
    """.split()
)
REVIEW_SAMPLE_SEED = 20261007
FILTER_LOSS_SAMPLE_MAX = 150
LABELS = [
    "synonym_in_intended_sense",
    "near_synonym",
    "alias_same_concept",
    "abbreviation_correct",
    "spelling_or_form_variant",
    "related_only",
    "wrong_sense",
    "uncertain",
]


def stats_connection(target: dict) -> sqlite3.Connection:
    state = state_dir(target)
    published = read_json(state / "published.json")
    return sqlite3.connect(f"file:{state / published['stats_file']}?mode=ro", uri=True)


def select_probes() -> tuple[list[str], list[str]]:
    """Words present (>= PROBE_MIN_FILES files) in at least PROBE_MIN_REPOSITORIES targets."""
    presence: collections.Counter = collections.Counter()
    for target in manifest()["targets"]:
        connection = stats_connection(target)
        for (word,) in connection.execute("SELECT word FROM word_files WHERE file_count >= ?", (PROBE_MIN_FILES,)):
            if PROBE_PATTERN.match(word) and word not in PROBE_STOPLIST:
                presence[word] += 1
        connection.close()
    population = sorted(word for word, count in presence.items() if count >= PROBE_MIN_REPOSITORIES)
    rng = random.Random(PROBE_SEED)
    return population, rng.sample(population, PROBE_COUNT)



def canonical_hash(value: object) -> str:
    return sha256_bytes(json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode())


def load_cases() -> list[dict]:
    cases = read_json(CASES_PATH)["cases"]
    if not cases or len({case["case_id"] for case in cases}) != len(cases):
        raise SystemExit("cases must be non-empty and have unique case_id values")
    if any(not case["input"].strip() or "\n" in case["input"] or "\r" in case["input"] for case in cases):
        raise SystemExit("each case input must be one non-empty line")
    return cases


def check_lookup_artifacts(handoff: dict) -> list[str]:
    """Bind the actual dataset, reader and published workspace files to the child-02 handoff."""
    problems = []

    def expect_hash(path: Path, expected: str, label: str) -> None:
        if not path.is_file() or sha256_file(path) != expected:
            problems.append(f"{label} missing or changed")

    if handoff["technical_status"]["status"] != "pass":
        problems.append("lookup handoff is not passing")
    expect_hash(REPO_ROOT / handoff["dataset_handoff_path"], handoff["dataset_handoff_sha256"], "dataset handoff")
    expect_hash(REPO_ROOT / handoff["dataset_sqlite_path"], handoff["dataset_sqlite_sha256"], "dataset SQLite")
    expect_hash(BINARY, handoff["binary"]["sha256"], "lookup binary")
    expect_hash(POLICY_PATH, handoff["policy"]["sha256"], "lookup policy")
    expect_hash(MANIFEST_PATH, handoff["workspace_manifest_sha256"], "workspace manifest")
    if MANIFEST_PATH.resolve() != (REPO_ROOT / handoff["workspace_manifest_path"]).resolve():
        problems.append("workspace manifest path differs from handoff")
    for name, digest in handoff["crate"]["source_sha256"].items():
        expect_hash(EXPERIMENT_DIR / name, digest, f"reader source {name}")
    expect_hash(EXPERIMENT_DIR / "Cargo.toml", handoff["crate"]["cargo_toml_sha256"], "crate manifest")
    expect_hash(EXPERIMENT_DIR / "Cargo.lock", handoff["crate"]["cargo_lock_sha256"], "crate lock")
    if problems:
        return problems

    targets = manifest()["targets"]
    names = [target["name"] for target in targets]
    if len(names) != len(set(names)) or set(names) != set(handoff["targets"]):
        return ["workspace target population differs from handoff"]
    for target in targets:
        name = target["name"]
        frozen = handoff["targets"][name]
        state = state_dir(target)
        if target["commit"] != frozen["commit"] or state.resolve() != (REPO_ROOT / frozen["state_dir"]).resolve():
            problems.append(f"{name}: workspace identity differs from handoff")
        expect_hash(state / "published.json", frozen["published_json_sha256"], f"{name}: publication")
        expect_hash(state / "index" / "meta.json", frozen["index_generation"], f"{name}: index generation")
        expect_hash(REPO_ROOT / frozen["stats_path"], frozen["stats_sha256"], f"{name}: statistics")
        if frozen["index_generation"] != handoff["index_generation"].get(name):
            problems.append(f"{name}: inconsistent handoff generation")
    return problems


RUN_RESULT_FILES = ("evaluation-protocol.json", "observations.jsonl", "measurements.json", "evaluation-offline.json",
                    "sample-review.tsv", "poc-report.json")


def freeze(revision_reason: str | None = None) -> int:
    """Write the evaluation protocol. A protocol that already has a run is only replaced by an
    explicit revision, which first preserves the superseded run under results/history/."""
    previous = None
    if PROTOCOL_PATH.exists() and OBSERVATIONS_PATH.exists():
        if not revision_reason:
            raise SystemExit("protocol already used by a run; pass --revision-reason to supersede it")
        previous = read_json(PROTOCOL_PATH)
    handoff = read_json(LOOKUP_HANDOFF_PATH)
    problems = check_lookup_artifacts(handoff)
    if problems:
        raise SystemExit("cannot freeze: " + "; ".join(problems))
    cases = load_cases()
    population, probes = select_probes()
    random_inputs = [case["input"] for case in cases if case["group"] == "random_probe"]
    if random_inputs != probes:
        raise SystemExit("random_probe cases differ from the seeded selection")
    policy = read_json(POLICY_PATH)
    protocol = {
        "protocol_version": 2,
        "frozen_on": "2026-10-06",
        "inputs": {
            "cases_path": repo_relative(CASES_PATH),
            "cases_sha256": sha256_file(CASES_PATH),
            "count": len(cases),
            "by_group": dict(collections.Counter(case["group"] for case in cases)),
            "by_category": dict(collections.Counter(case["category"] for case in cases)),
            "development_calibration": "none: rtl-1 was frozen before this protocol and was not tuned. Five control "
                                       "inputs (delete, retry, eventLoop, res, authz) were looked up during child-02 "
                                       "offline checks; they are labeled controls.",
        },
        "random_probe_selection": {
            "source": "published repository statistics of the four pinned targets (not the dataset)",
            "rule": f"words matching {PROBE_PATTERN.pattern}, file_count >= {PROBE_MIN_FILES} in at least "
                    f"{PROBE_MIN_REPOSITORIES} targets, not in the stoplist",
            "stoplist_sha256": sha256_bytes(" ".join(sorted(PROBE_STOPLIST)).encode()),
            "population_size": len(population),
            "seed": PROBE_SEED,
            "count": PROBE_COUNT,
            "selected": probes,
        },
        "frozen_identities": {
            "lookup_handoff_sha256": sha256_file(LOOKUP_HANDOFF_PATH),
            "dataset_id": handoff["dataset_id"],
            "dataset_sqlite_sha256": handoff["dataset_sqlite_sha256"],
            "normalization_version": handoff["normalization_version"],
            "policy_id": policy["policy_id"],
            "policy_sha256": sha256_file(POLICY_PATH),
            "binary_sha256": handoff["binary"]["sha256"],
            "workspace_manifest_sha256": handoff["workspace_manifest_sha256"],
            "index_generation": handoff["index_generation"],
            "output_schema_sha256": sha256_file(EXPERIMENT_DIR / "contracts" / "lookup.schema.json"),
            "index_stats_sha256": sha256_file(RESULTS_DIR / "index-stats.json"),
            "evaluator_sha256": sha256_file(Path(__file__)),
        },
        "frozen_limits_and_filters": {
            "final_limit": policy["final_limit"],
            "commonness_max_file_fraction": policy["commonness_max_file_fraction"],
            "included_relation_kinds": policy["included_relation_kinds"],
            "excluded_relation_kinds": policy["excluded_relation_kinds"],
        },
        "review": {
            "reviewer": "agent-authored: Claude (Opus 5.5) in Claude Code, single annotator. Not independent human "
                        "annotation and not an implemented LLM-judgment stage; no model is called by the pipeline.",
            "method": "Each (input, candidate) pair is labeled against the frozen intended_meaning using only the "
                      "recorded source evidence (sense gloss, concept, aliases, abbreviation record). Labels are "
                      "repository-independent; one label per pair.",
            "labels": {
                "synonym_in_intended_sense": "interchangeable with the input in its intended code meaning",
                "near_synonym": "close in the intended meaning, interchangeable in some code contexts",
                "alias_same_concept": "another name of the same concept (product, protocol, pattern)",
                "abbreviation_correct": "correct abbreviation/expansion for the intended meaning",
                "spelling_or_form_variant": "spelling, hyphenation, or inflection variant of the input",
                "related_only": "related concept, not the same meaning",
                "wrong_sense": "valid only for a different sense of the input",
                "uncertain": "evidence insufficient to decide",
            },
            "populations": {
                "final_pairs": "every unique (case, candidate) that reached final_terms in any target",
                "filter_loss_sample": f"seeded (seed {REVIEW_SAMPLE_SEED}) uniform sample of at most "
                                      f"{FILTER_LOSS_SAMPLE_MAX} unique (case, candidate) pairs that survived "
                                      "normalization in some target but were never final",
            },
            "uncertainty": "Input/target repeats, purposively selected controls and the census of final pairs are "
                           "descriptive only: ci95 is null, not an independent-trial interval. Dictionary coverage "
                           "uses unique inputs. Only the uniform filter-loss sample has approximate Wilson 95% "
                           "intervals (without finite-population correction). Uncertain labels stay separate.",
        },
        "metrics": [
            "dictionary coverage: inputs with at least one exact or token-normalized match",
            "supported synonym return: inputs whose final terms include a synonym_in_intended_sense or near_synonym",
            "useful return: final terms include synonym/near/alias/abbreviation labels",
            "alias-only and abbreviation-only returns",
            "wrong-sense and related-only rates among final candidates, by relation kind and source",
            "abstention by status; technical unavailability (repository not ready)",
            "expected_any found at dictionary stage vs final stage (missing kept separate from incorrect)",
            "filter attrition per stage/reason; filter loss estimated from the filter-loss sample",
        ],
        "measurement_conditions": {
            "binary_profile": "release",
            "repeat": 5,
            "pass_0": "first lookups in a fresh process (OS page cache not controlled)",
            "passes_1_4": "repeated lookups in the same process",
            "memory": "/usr/bin/time -l maximum resident set size per process",
            "dataset_build": "not run: reuse the existing read-only SQLite after its handoff hash is verified; "
                             "building is a separate explicit build_dataset.py command",
            "indexing": "from results/index-stats.json (single run per target)",
        },
    }
    if previous:
        # Reusing semantic labels is safe only when the judged input and serving evidence
        # are unchanged. A semantic revision needs a fresh review table, not this shortcut.
        semantic_keys = ("dataset_id", "dataset_sqlite_sha256", "policy_sha256", "binary_sha256")
        if (previous["inputs"]["cases_sha256"] != protocol["inputs"]["cases_sha256"]
                or any(previous["frozen_identities"][key] != protocol["frozen_identities"][key] for key in semantic_keys)):
            raise SystemExit("semantic inputs changed; preserve the prior run and prepare a fresh review before refreezing")
        protocol["revision"] = previous.get("revision", 1) + 1
        protocol["supersedes"] = previous["protocol_sha256"]
        protocol["revision_reason"] = revision_reason
        protocol["review"]["label_reuse"] = ("labels of the superseded run are reused by (case_id, candidate) key; "
                                             "aggregation fails if any final candidate lacks a label")
    protocol["protocol_sha256"] = canonical_hash(protocol)
    # Validate first, archive second: a rejected revision must leave the current run intact.
    if previous:
        history = RESULTS_DIR / "history" / f"protocol-{previous['protocol_sha256'][:12]}"
        history.mkdir(parents=True, exist_ok=False)
        for name in RUN_RESULT_FILES:
            if (RESULTS_DIR / name).exists():
                (history / name).write_bytes((RESULTS_DIR / name).read_bytes())
        (history / "SUPERSEDED.md").write_text(
            f"# 대체됨: 프로토콜 {previous['protocol_sha256'][:12]}\n\n- 사유: {revision_reason}\n"
            "- 이 디렉터리의 파일은 대체된 실행의 기록이며 최종 보고에 쓰지 않는다.\n",
            encoding="utf-8",
        )
        # Keep labels for review-template, but never let old measurements/offline status
        # masquerade as a result of the new protocol.
        for name in RUN_RESULT_FILES:
            if name != "sample-review.tsv":
                (RESULTS_DIR / name).unlink(missing_ok=True)
    write_json(PROTOCOL_PATH, protocol)
    print(f"protocol {protocol['protocol_sha256'][:16]} frozen")
    return 0


def check_protocol() -> dict:
    protocol = read_json(PROTOCOL_PATH)
    body = {key: value for key, value in protocol.items() if key != "protocol_sha256"}
    identities = protocol["frozen_identities"]
    handoff = read_json(LOOKUP_HANDOFF_PATH)
    problems = []
    if canonical_hash(body) != protocol["protocol_sha256"]:
        problems.append("protocol edited after freeze")
    if sha256_file(CASES_PATH) != protocol["inputs"]["cases_sha256"]:
        problems.append("cases.json changed")
    if sha256_file(POLICY_PATH) != identities["policy_sha256"]:
        problems.append("policy changed")
    if sha256_file(LOOKUP_HANDOFF_PATH) != identities["lookup_handoff_sha256"]:
        problems.append("lookup handoff changed")
    if protocol.get("protocol_version") != 2:
        problems.append("superseded evaluator protocol; freeze a new revision with --revision-reason")
    for path, key in (
        (BINARY, "binary_sha256"),
        (REPO_ROOT / handoff["dataset_sqlite_path"], "dataset_sqlite_sha256"),
        (MANIFEST_PATH, "workspace_manifest_sha256"),
        (EXPERIMENT_DIR / "contracts" / "lookup.schema.json", "output_schema_sha256"),
        (RESULTS_DIR / "index-stats.json", "index_stats_sha256"),
        (Path(__file__), "evaluator_sha256"),
    ):
        if not path.is_file() or sha256_file(path) != identities.get(key):
            problems.append(f"{path.name} missing or changed")
    if handoff["index_generation"] != identities["index_generation"]:
        problems.append("handoff index generations differ from protocol")
    if not problems:
        problems.extend(check_lookup_artifacts(handoff))
    if problems:
        raise SystemExit("protocol check failed: " + "; ".join(problems))
    return protocol


def validate_outputs(protocol: dict, target: str, outputs: list[dict]) -> None:
    """Reject a lookup that does not describe the exact frozen inputs and artifact generation."""
    cases = load_cases()
    frozen = protocol["frozen_identities"]
    if len(outputs) != len(cases) or [o["input"] for o in outputs] != [case["input"] for case in cases]:
        raise SystemExit(f"{target}: outputs do not account for every frozen input in order")
    generation = frozen["index_generation"][target]
    for output in outputs:
        repository = output["repository"]
        if (output["schema"] != "rt-lookup/1" or output["dataset_id"] != frozen["dataset_id"]
                or output["policy_id"] != frozen["policy_id"] or output["policy_sha256"] != frozen["policy_sha256"]
                or repository["status"] != "ready" or repository["published_generation"] != generation
                or repository["current_generation"] != generation):
            raise SystemExit(f"{target}: lookup artifact/generation mismatch for {output['input']!r}")


def measure(command: list[str], cwd: Path) -> tuple[subprocess.CompletedProcess, float, int | None]:
    started = time.perf_counter()
    process = subprocess.run(["/usr/bin/time", "-l", *command], cwd=cwd, capture_output=True, text=True)
    wall = time.perf_counter() - started
    match = re.search(r"(\d+)\s+maximum resident set size", process.stderr)
    return process, wall, int(match.group(1)) if match else None


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, max(0, math.ceil(fraction * len(ordered)) - 1))]


def compact_observation(case: dict, target: str, output: dict, line_bytes: int) -> dict:
    final = [c for c in output["candidates"] if c["outcome"] == "final"]
    reasons = collections.Counter(c["reason"] for c in output["candidates"] if c["outcome"] != "final")
    return {
        "case_id": case["case_id"],
        "input": case["input"],
        "group": case["group"],
        "category": case["category"],
        "target": target,
        "status": output["status"],
        "repository_status": output["repository"]["status"],
        "index_generation": output["repository"]["published_generation"],
        "matches": [{"exact_key": m["exact_key"], "match_kind": m["match_kind"], "concepts": m["concepts"]} for m in output["matches"]],
        "stage_counts": output["stage_counts"],
        "rejections": dict(sorted((str(k), v) for k, v in reasons.items())),
        "final": [
            {
                "rank": c["final_rank"],
                "candidate": c["exact_key"],
                "primary_kind": c["primary_kind"],
                "sources": sorted({e["source_id"] for e in c["evidence"] if not e["is_policy_excluded"]}),
                "components": [[x["token"], x["file_count"]] for x in c["repository"]["components"]],
            }
            for c in final
        ],
        "output_bytes": line_bytes,
        "final_terms_bytes": len(" ".join(output["final_terms"]).encode()),
    }


def run() -> int:
    protocol = check_protocol()
    handoff = read_json(LOOKUP_HANDOFF_PATH)
    run_dir = RUN_ROOT / protocol["protocol_sha256"][:12]
    run_dir.mkdir(parents=True, exist_ok=True)
    cases = load_cases()
    terms_path = run_dir / "terms.txt"
    terms_path.write_text("".join(case["input"] + "\n" for case in cases), encoding="utf-8")
    dataset_sqlite = REPO_ROOT / handoff["dataset_sqlite_path"]

    # check_protocol verified the existing artifact. Evaluation never builds, repairs or
    # replaces it; a missing/incompatible dataset is an explicit failure, not a fallback.

    observations = []
    lookups = {}
    commands = []
    for target in manifest()["targets"]:
        name = target["name"]
        output_path = run_dir / f"{name}.jsonl"
        timing_path = run_dir / f"{name}-timing.json"
        command = [
            str(BINARY), "lookup", "--dataset", str(dataset_sqlite), "--policy", str(POLICY_PATH),
            "--state", str(state_dir(target)), "--expect-dataset-id", handoff["dataset_id"],
            "--repeat", str(protocol["measurement_conditions"]["repeat"]),
            "--terms-file", str(terms_path), "--output", str(output_path), "--timing-output", str(timing_path),
        ]
        process, wall, rss = measure(command, EXPERIMENT_DIR)
        if process.returncode != 0:
            raise SystemExit(f"{name}: lookup failed: {process.stderr[-500:]}")
        commands.append({"cwd": repo_relative(EXPERIMENT_DIR), "command": " ".join(
            os.path.relpath(part, EXPERIMENT_DIR) if part.startswith(str(REPO_ROOT)) else part for part in command), "exit": 0})
        lines = output_path.read_text(encoding="utf-8").splitlines()
        outputs = [json.loads(line) for line in lines]
        validate_outputs(protocol, name, outputs)
        for case, output, line in zip(cases, outputs, lines):
            observations.append(compact_observation(case, name, output, len(line.encode())))
        timing = read_json(timing_path)
        first = [us / 1000 for us in timing["passes"][0]["per_lookup_us"]]
        repeat = [us / 1000 for p in timing["passes"][1:] for us in p["per_lookup_us"]]
        lookups[name] = {
            "process_wall_s": round(wall, 3),
            "process_max_rss_bytes": rss,
            "process_start_to_ready_ms": timing["process_start_to_ready_us"] / 1000,
            "dataset_open_ms": timing["dataset_open_us"] / 1000,
            "stats_open_ms": timing["stats_open_us"] / 1000,
            "repeat_outputs_identical": timing["repeat_outputs_identical"],
            "first_pass_ms": {"n": len(first), "median": percentile(first, 0.5), "p90": percentile(first, 0.9), "max": max(first)},
            "repeat_passes_ms": {"n": len(repeat), "median": percentile(repeat, 0.5), "p90": percentile(repeat, 0.9), "max": max(repeat)},
            "output_bytes": {"median": percentile([len(l.encode()) for l in lines], 0.5), "max": max(len(l.encode()) for l in lines)},
            "raw_output_sha256": sha256_file(output_path),
        }
    check_protocol()  # A refresh during measurement must not be published as the frozen run.
    with OBSERVATIONS_PATH.open("w", encoding="utf-8") as handle:
        for observation in observations:
            handle.write(json.dumps(observation, ensure_ascii=False, sort_keys=True) + "\n")
    index_stats = read_json(RESULTS_DIR / "index-stats.json")
    lock = read_json(EXPERIMENT_DIR / "sources.lock.json")
    write_json(
        MEASUREMENTS_PATH,
        {
            "protocol_sha256": protocol["protocol_sha256"],
            "run_dir": repo_relative(run_dir),
            "commands": commands,
            "environment": {"platform": sys.platform, "python": sys.version.split()[0], "binary_profile": "release"},
            "source_acquisition": {
                "archive_bytes": {entry["source_id"]: entry["archive_bytes"] for entry in lock["sources"]},
                "wall_time": "unmeasured (network downloads were run interactively)",
            },
            "dataset_access": {
                "mode": "reuse_existing", "path": handoff["dataset_sqlite_path"],
                "sha256": handoff["dataset_sqlite_sha256"], "matches_frozen_artifact": True,
            },
            "dataset_build": {
                "status": "not_run", "reason": "existing dataset reused; build cost belongs to earlier build evidence",
                "wall_s": None, "max_rss_bytes": None,
            },
            "repository_index_finalization": {
                name: {key: t["costs"][key] for key in ("indexing_ms", "vocabulary_materialize_ms", "stats_persist_ms",
                                                       "stats_bytes", "index_command_max_rss_bytes")}
                for name, t in index_stats["targets"].items()
            },
            "lookup": lookups,
            "memory_note": "max RSS is per whole process (dataset open + all lookups); per-lookup memory unmeasured",
        },
    )
    print(f"run complete: {len(observations)} observations in {repo_relative(run_dir)}")
    return 0


REVIEW_COLUMNS = [
    "review_id", "population", "case_id", "group", "category", "input", "intended_meaning", "candidate",
    "primary_kind", "relation_kinds", "sources", "evidence_glosses", "source_records", "outcome_by_target",
    "label", "rationale",
]


def raw_outputs(protocol: dict) -> dict[str, list[dict]]:
    run_dir = RUN_ROOT / protocol["protocol_sha256"][:12]
    measurements = read_json(MEASUREMENTS_PATH)
    if measurements["protocol_sha256"] != protocol["protocol_sha256"] or measurements["run_dir"] != repo_relative(run_dir):
        raise SystemExit("measurements belong to another protocol/run directory")
    if set(measurements["lookup"]) != set(protocol["frozen_identities"]["index_generation"]):
        raise SystemExit("measurement targets differ from frozen targets")
    outputs = {}
    for target in manifest()["targets"]:
        path = run_dir / f"{target['name']}.jsonl"
        if sha256_file(path) != measurements["lookup"][target["name"]]["raw_output_sha256"]:
            raise SystemExit(f"raw output changed since the run: {path}")
        outputs[target["name"]] = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
        validate_outputs(protocol, target["name"], outputs[target["name"]])
    return outputs


def validate_observations(protocol: dict, observations: list[dict], outputs: dict[str, list[dict]]) -> None:
    """Require one observation per case/target and exact agreement with the saved raw output."""
    cases = load_cases()
    expected_keys = {(case["case_id"], target) for case in cases for target in outputs}
    counts = collections.Counter((o["case_id"], o["target"]) for o in observations)
    if set(counts) != expected_keys or any(count != 1 for count in counts.values()):
        raise SystemExit("observations have missing, duplicate or unexpected case/target pairs")
    by_key = {(o["case_id"], o["target"]): o for o in observations}
    run_dir = RUN_ROOT / protocol["protocol_sha256"][:12]
    for target, rows in outputs.items():
        lines = (run_dir / f"{target}.jsonl").read_text(encoding="utf-8").splitlines()
        for case, output, line in zip(cases, rows, lines):
            expected = compact_observation(case, target, output, len(line.encode()))
            if by_key[(case["case_id"], target)] != expected:
                raise SystemExit(f"observation differs from raw output: {case['case_id']}/{target}")


def candidate_index(protocol: dict, outputs: dict[str, list[dict]] | None = None) -> dict[tuple[str, str], dict]:
    """(case_id, candidate) -> merged evidence and per-target outcome across all targets."""
    cases = load_cases()
    merged: dict[tuple[str, str], dict] = {}
    for target, rows in (raw_outputs(protocol) if outputs is None else outputs).items():
        for case, output in zip(cases, rows):
            for candidate in output["candidates"]:
                key = (case["case_id"], candidate["exact_key"])
                entry = merged.setdefault(key, {"case": case, "candidate": candidate, "outcomes": {}})
                outcome = f"final#{candidate['final_rank']}" if candidate["outcome"] == "final" else (
                    f"{candidate['stage']}:{candidate['reason']}")
                entry["outcomes"][target] = outcome
    return merged


def review_template() -> int:
    protocol = check_protocol()
    merged = candidate_index(protocol)
    final_keys = sorted(key for key, entry in merged.items() if any(o.startswith("final") for o in entry["outcomes"].values()))
    loss_population = sorted(
        key for key, entry in merged.items()
        if key not in final_keys and any(not o.startswith("normalization") for o in entry["outcomes"].values())
    )
    rng = random.Random(REVIEW_SAMPLE_SEED)
    loss_sample = sorted(rng.sample(loss_population, min(FILTER_LOSS_SAMPLE_MAX, len(loss_population))))
    existing = {}
    if REVIEW_PATH.exists():
        with REVIEW_PATH.open(encoding="utf-8") as handle:
            for row in csv.DictReader(handle, delimiter="\t"):
                existing[(row["case_id"], row["candidate"])] = (row["label"], row["rationale"])
    rows = []
    for population, keys in (("final_pairs", final_keys), ("filter_loss_sample", loss_sample)):
        for key in keys:
            entry = merged[key]
            case, candidate = entry["case"], entry["candidate"]
            evidence = [e for e in candidate["evidence"] if not e["is_policy_excluded"]] or candidate["evidence"]
            glosses = []
            for item in evidence:
                gloss = item["to_gloss"] or item["gloss"] or ""
                text = f"{item['relation_kind']}@{item['source_id']}: {gloss[:90]}"
                if text not in glosses:
                    glosses.append(text)
            label, rationale = existing.get(key, ("", ""))
            rows.append({
                "review_id": f"{'F' if population == 'final_pairs' else 'L'}{len(rows) + 1:03d}",
                "population": population,
                "case_id": case["case_id"],
                "group": case["group"],
                "category": case["category"],
                "input": case["input"],
                "intended_meaning": case["intended_meaning"],
                "candidate": key[1],
                "primary_kind": candidate["primary_kind"] or "",
                "relation_kinds": ",".join(candidate["relation_kinds"]),
                "sources": ",".join(sorted({e["source_id"] for e in evidence})),
                "evidence_glosses": " || ".join(glosses[:3]),
                "source_records": ",".join(sorted({f"{e['from_source_record']}>{e['to_source_record']}" for e in evidence})[:3]),
                "outcome_by_target": ";".join(f"{t}={o}" for t, o in sorted(entry["outcomes"].items())),
                "label": label,
                "rationale": rationale,
            })
    with REVIEW_PATH.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=REVIEW_COLUMNS, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    print(f"review rows: {len(final_keys)} final pairs + {len(loss_sample)} filter-loss sample "
          f"(population {len(loss_population)})")
    return 0


USEFUL = {"synonym_in_intended_sense", "near_synonym", "alias_same_concept", "abbreviation_correct"}
SYNONYMISH = {"synonym_in_intended_sense", "near_synonym"}


def wilson(successes: int, total: int) -> dict:
    if total == 0:
        return {"k": 0, "n": 0, "rate": None, "ci95": None}
    z = 1.959964
    rate = successes / total
    denominator = 1 + z * z / total
    centre = (rate + z * z / (2 * total)) / denominator
    margin = z * math.sqrt(rate * (1 - rate) / total + z * z / (4 * total * total)) / denominator
    return {"k": successes, "n": total, "rate": round(rate, 4), "ci95": [round(centre - margin, 4), round(centre + margin, 4)]}


def read_review() -> dict[tuple[str, str], dict]:
    with REVIEW_PATH.open(encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle, delimiter="\t"))
    unlabeled = [row["review_id"] for row in rows if row["label"] not in LABELS]
    if unlabeled:
        raise SystemExit(f"unlabeled review rows: {unlabeled[:5]}")
    indexed = {(row["case_id"], row["candidate"]): row for row in rows}
    if len(indexed) != len(rows):
        raise SystemExit("duplicate case/candidate rows in review table")
    return indexed


def observed_proportion(successes: int, total: int) -> dict:
    """Describe fixed/repeated observations without treating them as independent trials."""
    return {"k": successes, "n": total, "rate": round(successes / total, 4) if total else None, "ci95": None}


def normalize_term(text: str) -> str:
    return " ".join(re.split(r"[^0-9a-z]+", text.lower())).strip()


def aggregate() -> int:
    protocol = check_protocol()
    cases = load_cases()
    case_by_id = {case["case_id"]: case for case in cases}
    observations = [json.loads(line) for line in OBSERVATIONS_PATH.read_text(encoding="utf-8").splitlines()]
    review = read_review()
    outputs = raw_outputs(protocol)
    validate_observations(protocol, observations, outputs)
    merged = candidate_index(protocol, outputs)
    targets = [target["name"] for target in manifest()["targets"]]

    # Every reviewed final pair must be exactly the set of observed final candidates.
    observed_final = {(o["case_id"], f["candidate"]) for o in observations for f in o["final"]}
    reviewed_final = {key for key, row in review.items() if row["population"] == "final_pairs"}
    if observed_final != reviewed_final:
        raise SystemExit("review table does not match observed final candidates")

    def label(case_id: str, candidate: str) -> str:
        return review[(case_id, candidate)]["label"]

    # --- input-level outcomes (per observation = case x target) -------------------------------
    per_observation = []
    for o in observations:
        labels = [label(o["case_id"], f["candidate"]) for f in o["final"]]
        case = case_by_id[o["case_id"]]
        expected = {normalize_term(term) for term in case["expected_any"]}
        dictionary_terms = {normalize_term(c[1]) for c in merged if c[0] == o["case_id"]}
        final_terms = {normalize_term(f["candidate"]) for f in o["final"]}
        per_observation.append({
            "case_id": o["case_id"], "group": o["group"], "category": o["category"], "target": o["target"],
            "status": o["status"], "repository_status": o["repository_status"],
            "final_count": len(labels),
            "has_synonym": any(l == "synonym_in_intended_sense" for l in labels),
            "has_synonymish": any(l in SYNONYMISH for l in labels),
            "has_useful": any(l in USEFUL for l in labels),
            "alias_or_abbreviation_only": any(l in USEFUL for l in labels) and not any(l in SYNONYMISH for l in labels),
            "nonempty_without_useful": bool(labels) and not any(l in USEFUL for l in labels),
            "has_wrong_sense": "wrong_sense" in labels,
            "expected_defined": bool(expected),
            "expected_in_dictionary": bool(expected & dictionary_terms),
            "expected_in_final": bool(expected & final_terms),
        })

    def summarize(rows: list[dict]) -> dict:
        n = len(rows)
        statuses = collections.Counter(row["status"] for row in rows)
        with_expected = [row for row in rows if row["expected_defined"]]
        unique_inputs = list({row["case_id"]: row for row in rows}.values())
        expected_inputs = [row for row in unique_inputs if row["expected_defined"]]
        return {
            "observations": n,
            "unique_inputs": len(unique_inputs),
            "status": dict(sorted(statuses.items())),
            "dictionary_metric_unit": "unique input (independent of repository)",
            "return_metric_unit": "input x target observation (repeated inputs, descriptive rates only)",
            "dictionary_coverage": observed_proportion(sum(row["status"] != "no_dictionary_entry" for row in unique_inputs), len(unique_inputs)),
            "nonempty_final": observed_proportion(sum(row["final_count"] > 0 for row in rows), n),
            "supported_synonym_return_strict": observed_proportion(sum(row["has_synonym"] for row in rows), n),
            "supported_synonym_return_incl_near": observed_proportion(sum(row["has_synonymish"] for row in rows), n),
            "useful_return": observed_proportion(sum(row["has_useful"] for row in rows), n),
            "alias_or_abbreviation_only": observed_proportion(sum(row["alias_or_abbreviation_only"] for row in rows), n),
            "nonempty_without_useful": observed_proportion(sum(row["nonempty_without_useful"] for row in rows), n),
            "any_wrong_sense_in_final": observed_proportion(sum(row["has_wrong_sense"] for row in rows), n),
            "abstention_no_final": observed_proportion(sum(row["final_count"] == 0 for row in rows), n),
            "technical_unavailable": sum(row["repository_status"] != "ready" for row in rows),
            "expected_found_in_dictionary": observed_proportion(sum(row["expected_in_dictionary"] for row in expected_inputs), len(expected_inputs)),
            "expected_found_in_final": observed_proportion(sum(row["expected_in_final"] for row in with_expected), len(with_expected)),
        }

    groups = {"all": per_observation}
    for name in ("control", "category_probe", "random_probe"):
        groups[name] = [row for row in per_observation if row["group"] == name]
    input_level = {name: summarize(rows) for name, rows in groups.items()}
    by_target = {target: summarize([row for row in per_observation if row["target"] == target]) for target in targets}
    by_category = {
        category: summarize([row for row in per_observation if row["category"] == category])
        for category in sorted({row["category"] for row in per_observation})
    }

    # --- candidate-level precision ---------------------------------------------------------------
    def label_table(items: list[tuple[str, str]]) -> dict:
        counts = collections.Counter(items)
        total = collections.Counter(key for key, _ in items)
        return {
            key: {
                "n": total[key],
                "labels": {l: counts[(key, l)] for l in LABELS if counts[(key, l)]},
                "synonym_or_near": observed_proportion(counts[(key, "synonym_in_intended_sense")] + counts[(key, "near_synonym")], total[key]),
                "useful": observed_proportion(sum(counts[(key, l)] for l in USEFUL), total[key]),
                "wrong_sense": observed_proportion(counts[(key, "wrong_sense")], total[key]),
            }
            for key in sorted(total)
        }

    unique_final = [review[key] for key in sorted(reviewed_final)]
    pooled_final = [(o, f) for o in observations for f in o["final"]]
    final_by_kind_unique = label_table([(row["primary_kind"], row["label"]) for row in unique_final])
    final_by_source_unique = label_table([(row["sources"], row["label"]) for row in unique_final])
    final_by_kind_pooled = label_table([(f["primary_kind"], label(o["case_id"], f["candidate"])) for o, f in pooled_final])
    overall_unique = label_table([("all", row["label"]) for row in unique_final])["all"]
    overall_pooled = label_table([("all", label(o["case_id"], f["candidate"])) for o, f in pooled_final])["all"]
    random_unique = label_table([("random_probe", row["label"]) for row in unique_final if row["group"] == "random_probe"])

    # --- filter attrition and loss ---------------------------------------------------------------
    attrition = {}
    for target in targets:
        rows = [o for o in observations if o["target"] == target]
        stage_sum = collections.Counter()
        reasons = collections.Counter()
        for o in rows:
            stage_sum.update(o["stage_counts"])
            reasons.update(o["rejections"])
        attrition[target] = {"stage_totals": dict(stage_sum), "rejection_reasons": dict(sorted(reasons.items()))}
    loss_rows = [row for row in review.values() if row["population"] == "filter_loss_sample"]
    loss_population = sum(
        1 for key, entry in merged.items()
        if key not in reviewed_final and any(not o.startswith("normalization") for o in entry["outcomes"].values())
    )
    loss_useful = sum(row["label"] in USEFUL for row in loss_rows)
    loss_synonymish = sum(row["label"] in SYNONYMISH for row in loss_rows)
    filter_loss = {
        "population": loss_population,
        "sample": len(loss_rows),
        "interval_method": "approximate Wilson 95% for the uniform pair sample only; no finite-population correction",
        "labels": dict(collections.Counter(row["label"] for row in loss_rows)),
        "useful_rate": wilson(loss_useful, len(loss_rows)),
        "synonym_or_near_rate": wilson(loss_synonymish, len(loss_rows)),
        "wrong_sense_rate": wilson(sum(row["label"] == "wrong_sense" for row in loss_rows), len(loss_rows)),
        "estimated_useful_lost": round(loss_useful / len(loss_rows) * loss_population, 1) if loss_rows else None,
        "useful_examples": [
            f"{row['input']} -> {row['candidate']} ({row['label']}; {row['outcome_by_target']})"
            for row in loss_rows if row["label"] in USEFUL
        ][:12],
    }

    # --- examples per failure class --------------------------------------------------------------
    def examples(predicate, limit=6):
        return [f"{row['input']} -> {row['candidate']} [{row['primary_kind']}]: {row['rationale']}"
                for row in unique_final if predicate(row)][:limit]

    no_entry = sorted({case_by_id[r["case_id"]]["input"] for r in per_observation if r["status"] == "no_dictionary_entry"})
    no_rel = sorted({case_by_id[r["case_id"]]["input"] for r in per_observation if r["status"] == "no_relations"})
    empty = sorted({case_by_id[r["case_id"]]["input"] for r in per_observation if r["status"] == "empty_after_filters"})
    missing_expected = sorted({
        case_by_id[r["case_id"]]["input"] for r in per_observation
        if r["expected_defined"] and not r["expected_in_dictionary"]
    })
    failure_examples = {
        "wrong_sense": examples(lambda r: r["label"] == "wrong_sense"),
        "related_only": examples(lambda r: r["label"] == "related_only"),
        "ambiguous_abbreviation_wrong_expansion": examples(lambda r: r["label"] == "wrong_sense" and r["primary_kind"].startswith("abbreviation")),
        "coincidental_sense": [f"{r['input']} -> {r['candidate']}: {r['rationale']}" for r in unique_final if "coincidence" in r["rationale"]],
        "alias_only_return": sorted({case_by_id[r["case_id"]]["input"] for r in per_observation if r["alias_or_abbreviation_only"]}),
        "no_dictionary_entry": no_entry,
        "no_relations": no_rel,
        "empty_after_filters_on_some_target": empty,
        "expected_missing_from_dictionary": missing_expected,
        "useful_lost_by_filters": filter_loss["useful_examples"],
    }

    measurements = read_json(MEASUREMENTS_PATH)
    dataset_handoff = read_json(RESULTS_DIR / "dataset-handoff.json")
    lookup_handoff = read_json(LOOKUP_HANDOFF_PATH)
    offline = read_json(OFFLINE_CHECK_PATH) if OFFLINE_CHECK_PATH.exists() else None
    is_offline_current = (offline is not None and offline["protocol_sha256"] == protocol["protocol_sha256"]
                          and offline["status"] == "pass"
                          and offline.get("raw_output_sha256") == {name: m["raw_output_sha256"] for name, m in measurements["lookup"].items()})
    is_repeat_identical = all(m["repeat_outputs_identical"] for m in measurements["lookup"].values())
    is_dataset_matching = (measurements.get("dataset_access", {}).get("sha256") == protocol["frozen_identities"]["dataset_sqlite_sha256"]
                           and measurements["dataset_access"]["matches_frozen_artifact"])
    report = {
        "report_version": 2,
        "dataset_handoff": {"path": repo_relative(RESULTS_DIR / "dataset-handoff.json"),
                            "sha256": sha256_file(RESULTS_DIR / "dataset-handoff.json"), "dataset_id": dataset_handoff["dataset_id"]},
        "lookup_handoff": {"path": repo_relative(LOOKUP_HANDOFF_PATH), "sha256": sha256_file(LOOKUP_HANDOFF_PATH),
                           "technical_status": lookup_handoff["technical_status"]["status"]},
        "protocol": {"path": repo_relative(PROTOCOL_PATH), "protocol_sha256": protocol["protocol_sha256"]},
        "denominators": {
            "inputs": len(cases), "inputs_by_group": protocol["inputs"]["by_group"], "targets": len(targets),
            "observations": len(observations), "reviewed_final_pairs": len(unique_final),
            "pooled_final_candidates": len(pooled_final), "filter_loss_population": loss_population,
            "filter_loss_sample": len(loss_rows),
        },
        "technical_status": {
            "status": "pass" if lookup_handoff["technical_status"]["status"] == "pass"
            and all(row["repository_status"] == "ready" for row in per_observation)
            and is_offline_current and is_repeat_identical and is_dataset_matching else "fail",
            "all_inputs_accounted": True,
            "observations_match_raw_outputs": True,
            "repository_not_ready_observations": sum(row["repository_status"] != "ready" for row in per_observation),
            "repeat_outputs_identical": is_repeat_identical,
            "dataset_artifact_matches_handoff": is_dataset_matching,
            "dataset_rebuild_reproduced": None,
            "dataset_build_status": "not_run_existing_artifact_reused",
            "evaluation_lookups_offline": "pass" if is_offline_current else "missing_stale_or_failed",
            "index_time_statistics": "materialized during indexing and published before ready; lookups read published "
                                     "statistics only (child-02 lifecycle/offline evidence)",
        },
        "semantic_findings": {
            "reviewer": protocol["review"]["reviewer"],
            "uncertainty": protocol["review"]["uncertainty"],
            "input_level": input_level,
            "input_level_by_target": by_target,
            "input_level_by_category": by_category,
            "final_candidates_unique": overall_unique,
            "final_candidates_pooled": overall_pooled,
            "final_candidates_random_probe_unique": random_unique.get("random_probe"),
            "final_by_relation_kind_unique": final_by_kind_unique,
            "final_by_relation_kind_pooled": final_by_kind_pooled,
            "final_by_source_unique": final_by_source_unique,
        },
        "filter_attrition": attrition,
        "filter_loss": filter_loss,
        "costs": {
            "source_archives_bytes": measurements["source_acquisition"]["archive_bytes"],
            "dataset_build": measurements["dataset_build"],
            "dataset_access": measurements["dataset_access"],
            "repository_index_finalization": measurements["repository_index_finalization"],
            "lookup": measurements["lookup"],
            "unmeasured": ["source download wall time", "per-lookup memory", "OS page-cache-cold latency"],
        },
        "failure_examples": failure_examples,
        "readme_path": repo_relative(EXPERIMENT_DIR / "README.ko.md"),
    }
    report["next_phase_recommendation"] = read_json(RESULTS_DIR / "next-phase.json") if (RESULTS_DIR / "next-phase.json").exists() else None
    report["unverified_limits"] = [
        "Semantic labels are agent-authored by one annotator; no independent human annotation or inter-annotator agreement.",
        "Inputs are 92 English terms (16 controls, 46 hand-selected category probes, 30 seeded repository-vocabulary probes); "
        "results do not generalize to all code vocabulary or other languages.",
        "Four public repositories (109-820 files); repository filtering behavior on large or private codebases is unmeasured.",
        "Phrase candidates were filtered by per-token presence only; phrase occurrence in the repository was never measured.",
        "No LLM judgment, product integration, agent search behavior, token savings, or Grafana question solving was executed.",
        "Latency is a single machine, release build, OS page cache not controlled.",
        "Input/target observations share inputs, and controls/category probes are purposively selected. Return and "
        "final-candidate rates are descriptive with no confidence interval; they are not independent trials.",
    ]
    write_json(REPORT_PATH, report)
    print(json.dumps({k: report["semantic_findings"]["input_level"][k]["supported_synonym_return_incl_near"] for k in groups}, indent=1))
    return 0


OFFLINE_CHECK_PATH = RESULTS_DIR / "evaluation-offline.json"


def offline_check() -> int:
    """Repeat the frozen evaluation lookups under the child-02 sandbox and compare outputs."""
    from lifecycle import sandbox_profile
    from workspaces import checkout_dir

    protocol = check_protocol()
    handoff = read_json(LOOKUP_HANDOFF_PATH)
    run_dir = RUN_ROOT / protocol["protocol_sha256"][:12]
    targets = manifest()["targets"]
    recorded = raw_outputs(protocol)
    profile = sandbox_profile([checkout_dir(targets[0]).parent], [state_dir(t) / "index" for t in targets])
    dataset_sqlite = REPO_ROOT / handoff["dataset_sqlite_path"]
    results = {}
    for target in targets:
        name = target["name"]
        output_path = run_dir / f"{name}-sandboxed.jsonl"
        process = subprocess.run(
            ["sandbox-exec", "-p", profile, str(BINARY), "lookup", "--dataset", str(dataset_sqlite),
             "--policy", str(POLICY_PATH), "--state", str(state_dir(target)),
             "--expect-dataset-id", handoff["dataset_id"], "--repeat", "2",
             "--terms-file", str(run_dir / "terms.txt"), "--output", str(output_path)],
            capture_output=True, text=True, cwd=EXPERIMENT_DIR,
        )

        def without_timing(path: Path) -> list[dict]:
            outputs = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
            for output in outputs:
                output["lookup_us"] = None
            return outputs

        expected = recorded[name]
        for output in expected:
            output["lookup_us"] = None
        identical = process.returncode == 0 and without_timing(output_path) == expected
        results[name] = {"sandboxed_exit": process.returncode, "outputs_identical_to_recorded_run": identical,
                         "stderr": process.stderr.strip()[-200:] or None}
    check_protocol()
    status = "pass" if all(r["outputs_identical_to_recorded_run"] for r in results.values()) else "fail"
    write_json(OFFLINE_CHECK_PATH, {
        "protocol_sha256": protocol["protocol_sha256"],
        "raw_output_sha256": {name: sha256_file(run_dir / f"{name}.jsonl") for name in recorded},
        "method": "same sandbox profile as results/offline-evidence.json (no network; no reads of checkouts or "
                  "index segments except meta.json); all frozen inputs on every target, 2 passes",
        "targets": results,
        "status": status,
    })
    print(f"evaluation offline check: {status}")
    return 0 if status == "pass" else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["select-probes", "freeze", "run", "offline-check", "review-template", "aggregate"])
    parser.add_argument("--revision-reason")
    args = parser.parse_args()
    if args.command == "select-probes":
        population, probes = select_probes()
        print(json.dumps({"population_size": len(population), "probes": probes}, indent=1))
        return 0
    if args.command == "freeze":
        return freeze(args.revision_reason)
    if args.command == "run":
        return run()
    if args.command == "offline-check":
        return offline_check()
    if args.command == "review-template":
        return review_template()
    return aggregate()


if __name__ == "__main__":
    sys.exit(main())
