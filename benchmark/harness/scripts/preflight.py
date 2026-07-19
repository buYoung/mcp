#!/usr/bin/env python3
"""Offline fail-closed preflight. It never calls DeepSeek, builds, or indexes."""
from __future__ import annotations

import json
import os
import pathlib
import stat
import subprocess
import sys
import uuid

from common import (
    ANSWER_SHA256, BENCHMARK_ROOT, CORPUS_ROOT, DERIVED_PRIVATE_MANIFEST_SHA256, DERIVED_PUBLIC_MANIFEST_SHA256, GOLDEN_CODEMAP, GOLDEN_TREE_SHA256, HARNESS_ROOT,
    LEGACY_ROOT, MODEL, OPENCODE_BINARY, OPENCODE_SHA256, ORIGINAL_BASELINE_1_SHA256,
    ORIGINAL_BASELINE_2_TEMPLATE_SHA256, ORIGINAL_LIMITS_SHA256,
    ORIGINAL_PROMPT_TEMPLATE_SHA256, PRODUCT_ROOT, QUALITY_ROOT, QUESTION_SHA256,
    PROMPT_SHA256, SOURCE_TREE_SHA256, TASK_IDS, load_json, sha256, tree_digest, write_json,
)
from generation import require_execution_generation, validate_b2_reproducibility, verify as verify_generation
from materialize_config import pair
from render_prompt import render
from scheduler import schedule, validate
from selftest_contract import (
    AGGREGATOR_REQUIRED_TESTS, ANALYSIS_INPUT_REQUIRED_CHECKS,
    EXECUTION_REQUIRED_CHECKS, EXTRACTOR_REQUIRED_TESTS, SCORING_REQUIRED_CHECKS,
)


def related_processes(process_lines: list[str]) -> list[dict[str, object]]:
    related: list[dict[str, object]] = []
    for line in process_lines:
        fields = line.strip().split(None, 2)
        if len(fields) != 3:
            continue
        try:
            pid, process_group = map(int, fields[:2])
        except ValueError:
            continue
        command = fields[2]
        if str(QUALITY_ROOT) not in command or ("opencode run" not in command and "/codemap-search mcp" not in command):
            continue
        related.append({"pid": pid, "process_group": process_group, "command": command})
    return related


def lane_process_policy(
    session_lane: bool,
    related: list[dict[str, object]],
    active_ledger_attempts: list[dict[str, object]],
) -> bool:
    if not session_lane:
        return not related
    if len(active_ledger_attempts) > 2:
        return False
    if not related:
        return True
    if not active_ledger_attempts:
        return False
    process_groups = {
        attempt.get("guardian_process_group")
        for attempt in active_ledger_attempts
        if isinstance(attempt.get("guardian_process_group"), int)
        and attempt.get("guardian_process_group") > 1
    }
    return bool(process_groups) and all(
        process.get("process_group") in process_groups for process in related
    )


def load_execution_generation(path: pathlib.Path) -> dict:
    verify_generation(path)
    return require_execution_generation(load_json(path))


def main() -> int:
    if len(sys.argv) == 1:
        generation_path = None
        session_lane = False
        report_path = HARNESS_ROOT / "reports/preflight-latest.json"
    elif len(sys.argv) == 2:
        generation_path = pathlib.Path(sys.argv[1]).resolve()
        session_lane = False
        report_path = HARNESS_ROOT / "reports/preflight-latest.json"
    elif len(sys.argv) == 4 and sys.argv[2] == "--session-lane":
        generation_path = pathlib.Path(sys.argv[1]).resolve()
        session_lane = True
        report_path = pathlib.Path(sys.argv[3]).resolve()
        work_root = (HARNESS_ROOT / "work").resolve()
        if report_path.parent.parent != work_root or report_path.name != "preflight.report.json":
            raise SystemExit("session-lane report must be one run preparation directory below harness/work")
    else:
        raise SystemExit("usage: preflight.py [SEALED_GENERATION [--session-lane WORK_REPORT]]")
    checks: list[dict] = []

    def check(name: str, condition: bool, detail: object = None, blocker: str | None = None) -> None:
        checks.append({
            "name": name,
            "status": "pass" if condition else "fail",
            "detail": detail,
            "blocker": None if condition else blocker,
        })

    caches = [str(path) for path in HARNESS_ROOT.rglob("__pycache__")] + [str(path) for path in HARNESS_ROOT.rglob("*.pyc")]
    check("no-python-cache-artifacts", not caches, caches, "delete caches and rerun with PYTHONDONTWRITEBYTECODE=1")
    check("fixed-prompt-template", sha256(HARNESS_ROOT / "templates/prompt.txt") == ORIGINAL_PROMPT_TEMPLATE_SHA256)
    check("fixed-baseline-1-config", sha256(HARNESS_ROOT / "config/baseline-1.json") == ORIGINAL_BASELINE_1_SHA256)
    check("fixed-baseline-2-template", sha256(HARNESS_ROOT / "config/baseline-2.template.json") == ORIGINAL_BASELINE_2_TEMPLATE_SHA256)
    check("fixed-limits", sha256(HARNESS_ROOT / "config/limits.json") == ORIGINAL_LIMITS_SHA256)
    check("fixed-opencode-binary", sha256(OPENCODE_BINARY) == OPENCODE_SHA256 and bool(OPENCODE_BINARY.stat().st_mode & 0o111))

    public = load_json(BENCHMARK_ROOT / "manifests/public.json")
    private = load_json(BENCHMARK_ROOT / "manifests/private.json")
    public_tasks = [task.get("task_id") for task in public.get("tasks", [])]
    private_tasks = [task.get("task_id") for task in private.get("tasks", [])]
    check(
        "development-only-public-manifest",
        public_tasks == list(TASK_IDS)
        and all(task.get("split") == "development" for task in public.get("tasks", []))
        and "repeat_task_ids" not in public
        and sha256(BENCHMARK_ROOT / "manifests/public.json") == DERIVED_PUBLIC_MANIFEST_SHA256,
    )
    check(
        "development-only-private-manifest",
        private_tasks == list(TASK_IDS)
        and all(task.get("split") == "development" for task in private.get("tasks", []))
        and sha256(BENCHMARK_ROOT / "manifests/private.json") == DERIVED_PRIVATE_MANIFEST_SHA256,
    )
    removed_paths = [
        BENCHMARK_ROOT / "questions/practice", BENCHMARK_ROOT / "answers/practice", BENCHMARK_ROOT / "sealed",
    ]
    check("practice-and-sealed-material-absent", not any(path.exists() for path in removed_paths), [str(path) for path in removed_paths if path.exists()])
    expected_question_files = {f"{task}.json" for task in TASK_IDS}
    actual_question_files = {path.name for path in (BENCHMARK_ROOT / "questions/development").glob("*.json")}
    actual_answer_files = {path.name for path in (BENCHMARK_ROOT / "answers/development").glob("*.json")}
    check("exact-14-question-answer-files", actual_question_files == expected_question_files and actual_answer_files == expected_question_files)
    question_checks = {
        task: sha256(BENCHMARK_ROOT / "questions/development" / f"{task}.json") == QUESTION_SHA256[task]
        for task in TASK_IDS
    }
    check("question-byte-hashes", all(question_checks.values()), {task: ok for task, ok in question_checks.items() if not ok})
    answer_checks = {
        task: sha256(BENCHMARK_ROOT / "answers/development" / f"{task}.json") == ANSWER_SHA256[task]
        for task in TASK_IDS
    }
    private_answer_checks = {
        task["task_id"]: task.get("answer_sha256") == ANSWER_SHA256.get(task["task_id"])
        for task in private.get("tasks", [])
    }
    check("answer-byte-and-private-manifest-hashes", all(answer_checks.values()) and all(private_answer_checks.values()), {
        "file_mismatches": [task for task, ok in answer_checks.items() if not ok],
        "manifest_mismatches": [task for task, ok in private_answer_checks.items() if not ok],
    })
    prompt_hashes = {}
    try:
        for task in TASK_IDS:
            prompt_hashes[task] = __import__("hashlib").sha256(render(BENCHMARK_ROOT / "questions/development" / f"{task}.json", QUESTION_SHA256[task])).hexdigest()
        prompt_ok = prompt_hashes == PROMPT_SHA256
    except SystemExit as error:
        prompt_ok = False
        prompt_hashes = {"error": str(error)}
    check("neutral-prompt-rendering", prompt_ok, prompt_hashes)

    try:
        entries = schedule()
        validate(entries)
        schedule_ok = (
            len(entries) == 84
            and sum(row["criterion"] == "B1" for row in entries) == 42
            and sum(row["criterion"] == "B2" for row in entries) == 42
            and len({(row["task_id"], row["trial_id"]) for row in entries}) == 42
        )
    except Exception as error:
        schedule_ok = False
        entries = [{"error": repr(error)}]
    check("exact-84-balanced-schedule", schedule_ok, {"sessions": len(entries), "candidate_sessions": 0})
    selftest_path = HARNESS_ROOT / "reports/offline-selftest.json"
    selftest = load_json(selftest_path) if selftest_path.is_file() else {}
    execution_checks = selftest.get("checks", {})
    tested_file_hashes = selftest.get("tested_file_sha256", {})
    tested_files_current = bool(tested_file_hashes) and all(
        (HARNESS_ROOT / relative).is_file() and sha256(HARNESS_ROOT / relative) == expected
        for relative, expected in tested_file_hashes.items()
    )
    required_execution_checks = EXECUTION_REQUIRED_CHECKS
    execution_policy_ok = (
        selftest.get("passed") is True
        and selftest.get("required_check_names") == sorted(required_execution_checks)
        and tested_files_current
        and all(execution_checks.get(name) is True for name in required_execution_checks)
    )
    check(
        "execution-policy-concurrency-three",
        execution_policy_ok,
        {"selftest_sha256": sha256(selftest_path) if selftest_path.is_file() else None, "tested_files_current": tested_files_current, "required_checks": sorted(required_execution_checks)},
        "rerun the offline execution-policy selftest after fixing the runner",
    )
    scoring_selftest_path = HARNESS_ROOT / "reports/scoring-selftest.json"
    scoring_selftest = load_json(scoring_selftest_path) if scoring_selftest_path.is_file() else {}
    scoring_hashes = scoring_selftest.get("tested_file_sha256", {})
    scoring_files_current = bool(scoring_hashes) and all(
        (HARNESS_ROOT / relative).is_file() and sha256(HARNESS_ROOT / relative) == expected
        for relative, expected in scoring_hashes.items()
    )
    required_scoring_checks = SCORING_REQUIRED_CHECKS
    scoring_ok = (
        scoring_selftest.get("passed") is True
        and scoring_selftest.get("required_check_names") == sorted(required_scoring_checks)
        and scoring_files_current
        and all(scoring_selftest.get("checks", {}).get(name) is True for name in required_scoring_checks)
    )
    check(
        "blind-three-scorer-two-phase-pipeline",
        scoring_ok,
        {"selftest_sha256": sha256(scoring_selftest_path) if scoring_selftest_path.is_file() else None, "tested_files_current": scoring_files_current, "required_checks": sorted(required_scoring_checks)},
        "rerun the no-model 84-output/252-judgment scoring selftest",
    )
    analysis_inputs_path = HARNESS_ROOT / "reports/analysis-inputs-selftest.json"
    analysis_inputs = load_json(analysis_inputs_path) if analysis_inputs_path.is_file() else {}
    analysis_input_hashes = analysis_inputs.get("tested_file_sha256", {})
    analysis_input_files_current = bool(analysis_input_hashes) and all(
        (HARNESS_ROOT / relative).is_file() and sha256(HARNESS_ROOT / relative) == expected
        for relative, expected in analysis_input_hashes.items()
    )
    required_analysis_input_checks = ANALYSIS_INPUT_REQUIRED_CHECKS
    analysis_inputs_ok = (
        analysis_inputs.get("passed") is True
        and analysis_inputs.get("required_check_names") == sorted(required_analysis_input_checks)
        and analysis_input_files_current
        and all(analysis_inputs.get("checks", {}).get(name) is True for name in required_analysis_input_checks)
    )
    check(
        "lossless-84-metric-252-judgment-analysis-input-seal",
        analysis_inputs_ok,
        {"selftest_sha256": sha256(analysis_inputs_path) if analysis_inputs_path.is_file() else None, "tested_files_current": analysis_input_files_current, "required_checks": sorted(required_analysis_input_checks)},
        "rerun the no-model analysis-input producer selftest",
    )
    analysis_tools_path = HARNESS_ROOT / "reports/analysis-tools-selftest.json"
    analysis_tools = load_json(analysis_tools_path) if analysis_tools_path.is_file() else {}
    analysis_tool_hashes = analysis_tools.get("tested_file_sha256", {})
    analysis_tool_files_current = bool(analysis_tool_hashes) and all(
        (HARNESS_ROOT / relative).is_file() and sha256(HARNESS_ROOT / relative) == expected
        for relative, expected in analysis_tool_hashes.items()
    )
    required_analysis_tool_checks = {
        "extractor-required-test-names-all-pass", "aggregator-required-test-names-all-pass",
    }
    suites = analysis_tools.get("suites", {})
    extractor_suite = suites.get("extractor", {})
    aggregator_suite = suites.get("aggregator", {})
    analysis_tools_ok = (
        analysis_tools.get("passed") is True
        and analysis_tools.get("required_check_names") == sorted(required_analysis_tool_checks)
        and analysis_tool_files_current
        and all(analysis_tools.get("checks", {}).get(name) is True for name in required_analysis_tool_checks)
        and extractor_suite.get("required_test_names") == sorted(EXTRACTOR_REQUIRED_TESTS)
        and aggregator_suite.get("required_test_names") == sorted(AGGREGATOR_REQUIRED_TESTS)
        and EXTRACTOR_REQUIRED_TESTS.issubset(extractor_suite.get("passed_test_names", []))
        and AGGREGATOR_REQUIRED_TESTS.issubset(aggregator_suite.get("passed_test_names", []))
        and extractor_suite.get("missing_required_test_names") == []
        and aggregator_suite.get("missing_required_test_names") == []
    )
    check(
        "automatic-metrics-and-aggregator-unit-suites",
        analysis_tools_ok,
        {"selftest_sha256": sha256(analysis_tools_path) if analysis_tools_path.is_file() else None, "tested_files_current": analysis_tool_files_current, "required_checks": sorted(required_analysis_tool_checks)},
        "rerun the model-free extractor and aggregator unit suites",
    )

    source_digest = tree_digest(CORPUS_ROOT)
    golden_digest = tree_digest(GOLDEN_CODEMAP)
    check("immutable-source-tree", source_digest == SOURCE_TREE_SHA256, source_digest)
    check("immutable-golden-index", golden_digest == GOLDEN_TREE_SHA256, golden_digest)
    source_writable = [str(path) for path in [CORPUS_ROOT, *CORPUS_ROOT.rglob("*")] if not path.is_symlink() and path.stat().st_mode & 0o222]
    benchmark_writable = [str(path) for path in [BENCHMARK_ROOT, *BENCHMARK_ROOT.rglob("*")] if not path.is_symlink() and path.stat().st_mode & 0o222]
    check("master-source-read-only", not source_writable, source_writable[:10])
    benchmark_lock_ok = generation_path is None or not benchmark_writable
    check(
        "benchmark-lock-deferred-only-until-final-seal",
        benchmark_lock_ok,
        {"sealed_generation_supplied": generation_path is not None, "writable_before_seal": benchmark_writable[:10]},
        "the final generation must lock every benchmark input read-only",
    )
    clone_report = load_json(HARNESS_ROOT / "reports/clonefile-synthetic.json") if (HARNESS_ROOT / "reports/clonefile-synthetic.json").is_file() else {}
    check("apfs-clonefile-proven", clone_report.get("passed") is True and clone_report.get("command") == ["/bin/cp", "-cRp"], clone_report)
    full_clone_report = load_json(HARNESS_ROOT / "reports/full-clone-materialization.json") if (HARNESS_ROOT / "reports/full-clone-materialization.json").is_file() else {}
    check("full-source-and-index-cow-materialization", full_clone_report.get("fallback_used") is False and full_clone_report.get("source_tree_sha256_before_index_replacement") == SOURCE_TREE_SHA256 and full_clone_report.get("working_index_tree_sha256") == GOLDEN_TREE_SHA256, full_clone_report)

    try:
        b1, b2, evidence = pair()
        config_ok = b1.get("mcp") == {} and list(b2.get("mcp", {})) == ["codemap_search"] and evidence.get("forced_mcp_use") is False
        b2_command = b2["mcp"]["codemap_search"]["command"]
        config_ok = config_ok and len(b2_command) == 2 and b2_command[1] == "mcp"
        b2_detail: object = evidence
    except (SystemExit, OSError, KeyError, json.JSONDecodeError) as error:
        config_ok = False
        b2_detail = str(error)
    check("b1-b2-only-mcp-difference-and-clean-b2", config_ok, b2_detail, "supply and attest the clean B2 binary")
    try:
        reproducibility_detail = validate_b2_reproducibility(load_json(HARNESS_ROOT / "config/b2-runtime.json"))
        reproducibility_ok = True
    except (SystemExit, OSError, KeyError, json.JSONDecodeError) as error:
        reproducibility_ok = False
        reproducibility_detail = str(error)
    check(
        "b2-source-to-binary-reproducibility",
        reproducibility_ok,
        reproducibility_detail,
        "the sealed source, two build outputs, Cargo.lock, rustc evidence, and model-free probe must agree",
    )
    runner_text = (HARNESS_ROOT / "scripts/run-session.sh").read_text(encoding="utf-8")
    command_lines = [line for line in runner_text.splitlines() if line.startswith("COMMAND=(")]
    shared_command_ok = (
        len(command_lines) == 1
        and "CODEMAP_BASELINE_READ_ONLY=1" in command_lines[0]
        and 'OPENCODE_CONFIG="$RUN_DIR/opencode.json"' in command_lines[0]
        and "$ARM" not in command_lines[0]
        and "CODEMAP_BASELINE_READ_ONLY=0" not in runner_text
    )
    check(
        "common-read-only-env-for-both-arms",
        shared_command_ok,
        {
            "single_shared_process_command": len(command_lines) == 1,
            "arm_reference_in_process_command": bool(command_lines and "$ARM" in command_lines[0]),
            "B1": {"CODEMAP_BASELINE_READ_ONLY": "1"},
            "B2": {"CODEMAP_BASELINE_READ_ONLY": "1"},
            "diff": [],
        },
        "both arms must execute through one common environment command",
    )

    sandbox_root = HARNESS_ROOT / "synthetic" / f"preflight-sandbox-{uuid.uuid4().hex}"
    source_fixture = sandbox_root / "source"
    codemap_fixture = source_fixture / ".codemap"
    source_fixture.mkdir(parents=True)
    if session_lane:
        codemap_fixture.mkdir()
    else:
        index_clone = subprocess.run(
            ["/bin/cp", "-cRp", str(GOLDEN_CODEMAP), str(codemap_fixture)],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False,
        )
        if index_clone.returncode != 0:
            raise SystemExit(f"sandbox MCP probe index clone failed without fallback: {index_clone.stderr.strip()}")
    visible = source_fixture / "visible.txt"
    visible.write_text("visible\n", encoding="utf-8")
    for path in [visible, *codemap_fixture.rglob("*"), codemap_fixture, source_fixture]:
        if path.is_symlink():
            continue
        path.chmod(path.stat().st_mode & ~0o222)
    profile = sandbox_root / "sandbox.sb"
    writable = sandbox_root / "writable.txt"
    auth_fixture = sandbox_root / "auth-runtime"
    auth_fixture.mkdir()
    (sandbox_root / "home").mkdir()
    b2_runtime = load_json(HARNESS_ROOT / "config/b2-runtime.json")
    subprocess.run(
        [
            sys.executable, str(HARNESS_ROOT / "scripts/materialize_sandbox.py"), str(profile),
            str(BENCHMARK_ROOT), str(LEGACY_ROOT), str(PRODUCT_ROOT), str(QUALITY_ROOT / "b2/source"),
            str(source_fixture), str(codemap_fixture), "/Users/example/.config/opencode", "/Users/example/.local/share/opencode",
            str(QUALITY_ROOT), str(sandbox_root), str(OPENCODE_BINARY), str(b2_runtime["binary_path"]), str(auth_fixture),
        ],
        env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
        check=True,
    )
    sensitive_roots = [
        HARNESS_ROOT,
        QUALITY_ROOT / "analysis-tools",
        QUALITY_ROOT / "audits",
        QUALITY_ROOT / ".agents",
        QUALITY_ROOT / "corpus",
        BENCHMARK_ROOT,
        QUALITY_ROOT / "b2/source",
        QUALITY_ROOT / "b2/build-evidence",
        QUALITY_ROOT / "provenance",
    ]
    sensitive_files = []
    for sensitive_root in sensitive_roots:
        first_file = next((path for path in sensitive_root.rglob("*") if path.is_file() and not path.is_symlink() and sandbox_root not in path.parents), None)
        if first_file is None:
            raise SystemExit(f"sensitive sandbox root has no readable test file: {sensitive_root}")
        sensitive_files.append(first_file)
    denied_checks = " && ".join(
        f"! /bin/ls '{root}' >/dev/null 2>&1 && ! /bin/cat '{file}' >/dev/null 2>&1"
        for root, file in zip(sensitive_roots, sensitive_files)
    )
    shell = (
        f"test -r '{visible}' && "
        f"{denied_checks} && "
        f"! /bin/ls '{LEGACY_ROOT}' >/dev/null 2>&1 && "
        f"! /bin/ls '{PRODUCT_ROOT}' >/dev/null 2>&1 && "
        f"! /usr/bin/touch '{source_fixture / 'blocked'}' && /usr/bin/touch '{writable}'"
    )
    sandbox = subprocess.run(["sandbox-exec", "-f", str(profile), "/bin/sh", "-c", shell], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    mcp_requests = "".join(json.dumps(row, separators=(",", ":")) + "\n" for row in (
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "sandbox-preflight", "version": "1"}}},
        {"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}},
    ))
    if session_lane:
        mcp_returncode = 0
        mcp_stderr = "session-lane reuses the full-idle sandbox MCP proof"
        listed_tools = {"initial_instructions", "overview", "search", "read", "find", "grep"}
    else:
        mcp_probe = subprocess.run(
            ["sandbox-exec", "-f", str(profile), str(b2_runtime["binary_path"]), "mcp"],
            cwd=source_fixture,
            env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": str(sandbox_root / "home"), "TMPDIR": str(sandbox_root), "CODEMAP_BASELINE_READ_ONLY": "1"},
            input=mcp_requests, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False, timeout=30,
        )
        mcp_returncode = mcp_probe.returncode
        mcp_stderr = mcp_probe.stderr
        mcp_responses = []
        try:
            mcp_responses = [json.loads(line) for line in mcp_probe.stdout.splitlines()]
        except json.JSONDecodeError:
            mcp_responses = []
        tool_response = next((row for row in mcp_responses if row.get("id") == 2), {})
        listed_tools = {tool.get("name") for tool in tool_response.get("result", {}).get("tools", []) if isinstance(tool, dict)}
    expected_tools = {"initial_instructions", "overview", "search", "read", "find", "grep"}
    sandbox_contract_ok = sandbox.returncode == 0 and writable.exists() and mcp_returncode == 0 and listed_tools == expected_tools
    check(
        "sandbox-sensitive-siblings-denied-run-source-and-b2-mcp-allowed",
        sandbox_contract_ok,
        {
            "filesystem_stderr": sandbox.stderr.decode("utf-8", "replace"),
            "sensitive_roots": [str(path) for path in sensitive_roots],
            "sensitive_files": [str(path) for path in sensitive_files],
            "mcp_returncode": mcp_returncode,
            "mcp_stderr": mcp_stderr,
            "mcp_probe_mode": "reused-full-idle-proof" if session_lane else "fresh-model-free-handshake",
            "temporary_cow_index_clone_count": 0 if session_lane else 1,
            "listed_tools": sorted(name for name in listed_tools if isinstance(name, str)),
        },
    )
    for path in [sandbox_root, *sandbox_root.rglob("*")]:
        if not path.is_symlink():
            path.chmod(path.stat().st_mode | 0o200)
    __import__("shutil").rmtree(sandbox_root)

    auth_path = pathlib.Path(os.environ.get("XDG_DATA_HOME", pathlib.Path.home() / ".local/share")) / "opencode/auth.json"
    auth_ok = False
    if auth_path.is_file():
        try:
            auth = load_json(auth_path)
            auth_ok = set(auth) >= {"ollama-cloud"} and bool(auth["ollama-cloud"].get("key"))
        except Exception:
            auth_ok = False
    check("provider-auth-present-without-value-disclosure", auth_ok, {"provider": "ollama-cloud", "key_value_recorded": False})
    auth_runtime_parent = HARNESS_ROOT / "runtime-auth"
    active_auth_runtimes = [
        str(path) for path in auth_runtime_parent.iterdir()
        if path.is_dir()
    ] if auth_runtime_parent.is_dir() else []
    auth_runtime_ok = len(active_auth_runtimes) <= (2 if session_lane else 0)
    check("session-auth-runtime-lane-capacity", auth_runtime_ok, {"session_lane": session_lane, "active": active_auth_runtimes}, "clean up an interrupted session credential runtime before sealing or resuming")
    process_list = subprocess.run(["ps", "-axo", "pid=,pgid=,command="], text=True, stdout=subprocess.PIPE, check=True).stdout.splitlines()
    related = related_processes(process_list)
    active_ledger_attempts = []
    if session_lane and generation_path is not None and generation_path.is_file():
        generation_for_lane = load_json(generation_path)
        lane_ledger = HARNESS_ROOT / "runs" / generation_for_lane.get("generation_id", "missing") / "ledger.json"
        if lane_ledger.is_file():
            lane_value = load_json(lane_ledger)
            active_ledger_attempts = [
                {
                    "run_id": run_id,
                    "state": attempt.get("state"),
                    "guardian_process_group": attempt.get("guardian_process_group"),
                }
                for run_id, attempt in lane_value.get("attempts", {}).items()
                if attempt.get("state") in {"claimed", "running"}
            ]
    related_ok = lane_process_policy(session_lane, related, active_ledger_attempts)
    check(
        "no-unaccounted-opencode-codemap-watcher-indexer-process",
        related_ok,
        {"session_lane": session_lane, "related_processes": related, "active_ledger_attempts": active_ledger_attempts},
        "a new lane may start only while at most two sealed-ledger lanes are active",
    )

    if generation_path is not None:
        generation_value = None
        try:
            verify_generation(generation_path)
            generation_ok = True
            generation_detail: object = str(generation_path)
        except (SystemExit, OSError, json.JSONDecodeError) as error:
            generation_ok = False
            generation_detail = str(error)
        check("sealed-generation-current", generation_ok, generation_detail, "changed inputs require a new generation and full restart")
        try:
            if not generation_ok:
                raise SystemExit("generation is not current")
            generation_value = load_execution_generation(generation_path)
            execution_generation_ok = True
            execution_generation_detail: object = {
                "generation_kind": generation_value["generation_kind"],
                "execution_ready": generation_value["execution_ready"],
            }
        except (SystemExit, OSError, json.JSONDecodeError, KeyError) as error:
            execution_generation_ok = False
            execution_generation_detail = str(error)
        check(
            "execution-ready-sealed-baseline-generation",
            execution_generation_ok,
            execution_generation_detail,
            "a planning snapshot may not enter any external session lane",
        )
        if generation_ok:
            generation_value = load_json(generation_path)
            critical_paths = [QUALITY_ROOT / relative for relative in generation_value.get("critical_file_sha256", {})]
            b2_contract = generation_value.get("b2", {}).get("contract", {})
            critical_paths.extend([
                generation_path, CORPUS_ROOT, GOLDEN_CODEMAP, OPENCODE_BINARY,
                QUALITY_ROOT / "b2/source", pathlib.Path(b2_contract.get("binary_path", "/nonexistent")),
                pathlib.Path(b2_contract.get("attestation_path", "/nonexistent")),
            ])
            writable_critical = []
            for root in critical_paths:
                if not root.exists():
                    writable_critical.append(f"missing:{root}")
                    continue
                paths = [root, *root.rglob("*")] if root.is_dir() else [root]
                writable_critical.extend(str(path) for path in paths if not path.is_symlink() and path.stat().st_mode & 0o222)
            check("sealed-critical-inputs-read-only", not writable_critical, writable_critical[:20], "sealed inputs may never be unlocked in the same generation")
    else:
        check("sealed-generation-supplied", False, None, "seal B2 and the full generation before external execution")

    report = {
        "schema_version": 1,
        "offline_only": True,
        "external_model_calls": 0,
        "builds": 0,
        "indexing_operations": 0,
        "model": MODEL,
        "mode": "session-lane" if session_lane else "full-idle",
        "passed": sum(row["status"] == "pass" for row in checks),
        "failed": sum(row["status"] == "fail" for row in checks),
        "checks": checks,
        "execution_ready": all(row["status"] == "pass" for row in checks),
    }
    write_json(report_path, report)
    print(json.dumps({"passed": report["passed"], "failed": report["failed"], "execution_ready": report["execution_ready"], "report": str(report_path)}, indent=2))
    return 0 if report["execution_ready"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
