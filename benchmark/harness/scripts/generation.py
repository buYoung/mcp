#!/usr/bin/env python3
"""Seal or verify one immutable 84-session baseline generation."""
from __future__ import annotations

import json
import hashlib
import filecmp
import pathlib
import stat
import sys
from typing import Any

from common import (
    ANSWER_SHA256, BENCHMARK_ROOT, CORPUS_ROOT, DERIVED_PRIVATE_MANIFEST_SHA256, DERIVED_PUBLIC_MANIFEST_SHA256, GOLDEN_CODEMAP, GOLDEN_TREE_SHA256, HARNESS_ROOT, LEGACY_ROOT,
    MODEL, OPENCODE_BINARY, OPENCODE_SHA256, ORIGINAL_GENERATION_FILE_SHA256,
    ORIGINAL_BASELINE_1_SHA256, ORIGINAL_BASELINE_2_TEMPLATE_SHA256, ORIGINAL_LIMITS_SHA256, ORIGINAL_PROMPT_TEMPLATE_SHA256,
    ORIGINAL_GENERATION_INTERNAL_SEAL, ORIGINAL_PRIVATE_MANIFEST_SHA256,
    ORIGINAL_PUBLIC_MANIFEST_SHA256, PRODUCT_ROOT, PROVIDER, QUALITY_ROOT,
    PROMPT_SHA256, SOURCE_TREE_SHA256, TASK_IDS, canonical_sha256, load_json, sha256, tree_digest, write_json,
)
from attest_b2 import (
    EXPECTED_PATCH_SHA256, EXPECTED_SOURCE_COMMIT, EXPECTED_SOURCE_GIT_TREE,
    EXPECTED_SOURCE_TAR_SHA256, EXPECTED_SOURCE_TREE_DIGEST, current_rustc_vv,
    git_tree, rustc_vv_from_info, tar_stream_sha256,
)
from materialize_config import pair
from render_prompt import render
from scheduler import PAIR_ORDER_RULE, schedule, validate
from selftest_contract import (
    AGGREGATOR_REQUIRED_TESTS, ANALYSIS_INPUT_REQUIRED_CHECKS,
    EXECUTION_REQUIRED_CHECKS, EXTRACTOR_REQUIRED_TESTS, SCORING_REQUIRED_CHECKS,
)


def file_hashes(root: pathlib.Path, patterns: tuple[str, ...]) -> dict[str, str]:
    paths = set()
    for pattern in patterns:
        paths.update(path for path in root.glob(pattern) if path.is_file())
    return {str(path.relative_to(QUALITY_ROOT)): sha256(path) for path in sorted(paths)}


def rendered_json_sha256(value: Any) -> str:
    rendered = json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    return hashlib.sha256(rendered.encode("utf-8")).hexdigest()


def verify_bound_selftest(path: pathlib.Path, required_checks: frozenset[str]) -> dict[str, Any]:
    if not path.is_file():
        raise SystemExit(f"required no-model selftest report is absent: {path.name}")
    report = load_json(path)
    if report.get("passed") is not True or report.get("external_model_calls") != 0 or report.get("builds") != 0 or report.get("indexing_operations") != 0:
        raise SystemExit(f"required no-model selftest did not pass cleanly: {path.name}")
    hashes = report.get("tested_file_sha256")
    if not isinstance(hashes, dict) or not hashes:
        raise SystemExit(f"selftest does not bind tested files: {path.name}")
    for relative, expected in hashes.items():
        tested = HARNESS_ROOT / relative
        if not tested.is_file() or sha256(tested) != expected:
            raise SystemExit(f"selftest report is stale for {relative}")
    checks = report.get("checks")
    if (
        report.get("required_check_names") != sorted(required_checks)
        or not isinstance(checks, dict)
        or not required_checks.issubset(checks)
        or any(checks[name] is not True for name in required_checks)
    ):
        raise SystemExit(f"selftest required check-name contract failed: {path.name}")
    return report


def verify_analysis_tool_suites(report: dict[str, Any]) -> None:
    suites = report.get("suites", {})
    for name, required in (
        ("extractor", EXTRACTOR_REQUIRED_TESTS),
        ("aggregator", AGGREGATOR_REQUIRED_TESTS),
    ):
        suite = suites.get(name, {})
        if (
            suite.get("returncode") != 0
            or suite.get("required_test_names") != sorted(required)
            or not required.issubset(suite.get("passed_test_names", []))
            or suite.get("missing_required_test_names") != []
            or suite.get("test_count") != len(suite.get("executed_test_names", []))
        ):
            raise SystemExit(f"analysis tool suite required test-name contract failed: {name}")


def validate_b2_reproducibility(contract: dict[str, Any]) -> dict[str, Any]:
    """Fail closed unless the attested source deterministically maps to this runtime binary."""
    attestation_path = pathlib.Path(contract.get("attestation_path", "/nonexistent"))
    if not attestation_path.is_file() or sha256(attestation_path) != contract.get("attestation_sha256"):
        raise SystemExit("B2 runtime does not bind the current attestation bytes")
    attestation = load_json(attestation_path)
    source_root = QUALITY_ROOT / "b2/source"
    source_hashes = attestation.get("source_hashes", {})
    if (
        contract.get("status") != "verified-clean-baseline"
        or contract.get("source_commit") != EXPECTED_SOURCE_COMMIT
        or attestation.get("verdict") != "verified-clean-baseline"
        or attestation.get("source_commit") != EXPECTED_SOURCE_COMMIT
        or attestation.get("source_git_tree") != EXPECTED_SOURCE_GIT_TREE
        or git_tree(EXPECTED_SOURCE_COMMIT) != EXPECTED_SOURCE_GIT_TREE
        or source_hashes.get("tar_stream", {}).get("sha256") != EXPECTED_SOURCE_TAR_SHA256
        or source_hashes.get("common_tree_digest", {}).get("sha256") != EXPECTED_SOURCE_TREE_DIGEST
        or tar_stream_sha256(source_root) != EXPECTED_SOURCE_TAR_SHA256
        or tree_digest(source_root) != EXPECTED_SOURCE_TREE_DIGEST
        or attestation.get("source_diff_sha256") != EXPECTED_PATCH_SHA256
        or sha256(HARNESS_ROOT / "provenance/b2-clean-runtime.patch") != EXPECTED_PATCH_SHA256
    ):
        raise SystemExit("B2 source commit, tree, hashes, or reviewed patch contract failed")

    builds = attestation.get("builds")
    expected_roots = [QUALITY_ROOT / "b2/build-evidence/target-build1", QUALITY_ROOT / "b2/target"]
    if attestation.get("build_count") != 2 or not isinstance(builds, list) or len(builds) != 2:
        raise SystemExit("B2 attestation must contain exactly two builds")
    rustc_values = []
    for build, build_root in zip(builds, expected_roots):
        binary_path = build_root / "release/codemap-search"
        rustc_info_path = build_root / ".rustc_info.json"
        if (
            build.get("cargo_target_dir") != str(build_root)
            or build.get("binary_path") != str(binary_path)
            or build.get("cargo_rustc_info_path") != str(rustc_info_path)
            or not binary_path.is_file()
            or not rustc_info_path.is_file()
            or sha256(binary_path) != build.get("binary_sha256")
            or sha256(rustc_info_path) != build.get("cargo_rustc_info_sha256")
            or build.get("binary_sha256") != contract.get("binary_sha256")
        ):
            raise SystemExit("B2 build path or hash evidence failed")
        rustc_info, rustc_vv = rustc_vv_from_info(rustc_info_path)
        if (
            build.get("rustc_fingerprint") != rustc_info.get("rustc_fingerprint")
            or build.get("rustc_vv") != rustc_vv
            or build.get("rustc_vv_sha256") != hashlib.sha256(rustc_vv.encode("utf-8")).hexdigest()
        ):
            raise SystemExit("B2 Cargo rustc evidence failed")
        rustc_values.append(rustc_vv)
    comparison = attestation.get("binary_comparison")
    if (
        contract.get("binary_path") != builds[1].get("binary_path")
        or attestation.get("binary_path") != contract.get("binary_path")
        or attestation.get("binary_sha256") != contract.get("binary_sha256")
        or comparison != {
            "command": "cmp -s <build1 binary> <build2 binary>",
            "exit_code": 0,
            "bytes_identical": True,
        }
        or not filecmp.cmp(builds[0]["binary_path"], builds[1]["binary_path"], shallow=False)
    ):
        raise SystemExit("B2 two-build binary comparison contract failed")

    cargo_lock = attestation.get("cargo_lock")
    source_lock = source_root / "apps/codemap-search/Cargo.lock"
    baseline_identity = f"git:{EXPECTED_SOURCE_COMMIT}:apps/codemap-search/Cargo.lock"
    if (
        not isinstance(cargo_lock, dict)
        or cargo_lock.get("source_path") != str(source_lock)
        or not source_lock.is_file()
        or cargo_lock.get("source_sha256") != sha256(source_lock)
        or cargo_lock.get("baseline_identity") != baseline_identity
        or cargo_lock.get("baseline_sha256") != cargo_lock.get("source_sha256")
        or cargo_lock.get("matches_baseline") is not True
    ):
        raise SystemExit("B2 Cargo.lock Git-object identity or hash contract failed")
    toolchain = attestation.get("toolchain")
    active_rustc = current_rustc_vv()
    if (
        not isinstance(toolchain, dict)
        or rustc_values != [active_rustc, active_rustc]
        or toolchain.get("active_rustc_vv") != active_rustc
        or toolchain.get("active_rustc_vv_sha256") != hashlib.sha256(active_rustc.encode("utf-8")).hexdigest()
        or toolchain.get("both_builds_match_active_rustc") is not True
        or toolchain.get("build_rustc_info_sha256") != [build["cargo_rustc_info_sha256"] for build in builds]
    ):
        raise SystemExit("B2 active Rust toolchain does not match both Cargo build records")

    probe_path = pathlib.Path(attestation.get("probe_report_path", "/nonexistent"))
    probe = load_json(probe_path) if probe_path.is_file() else {}
    if (
        sha256(probe_path) != attestation.get("probe_report_sha256")
        or probe.get("passed") is not True
        or probe.get("builds") != 0
        or probe.get("indexing_operations") != 0
        or probe.get("external_model_calls") != 0
        or probe.get("binary_path") != contract.get("binary_path")
        or probe.get("binary_sha256") != contract.get("binary_sha256")
        or attestation.get("probe_builds") != 0
        or attestation.get("candidate_code_present") is not False
        or attestation.get("candidate_environment_present") is not False
    ):
        raise SystemExit("B2 probe or candidate-absence contract failed")
    return {
        "source_commit": attestation["source_commit"],
        "source_git_tree": attestation["source_git_tree"],
        "source_hashes": source_hashes,
        "source_diff_sha256": attestation["source_diff_sha256"],
        "build_count": 2,
        "builds": builds,
        "binary_comparison": comparison,
        "cargo_lock": cargo_lock,
        "toolchain": toolchain,
        "probe_report_path": str(probe_path),
        "probe_report_sha256": attestation["probe_report_sha256"],
        "probe_builds": 0,
        "candidate_code_present": False,
        "candidate_environment_present": False,
    }


def snapshot(require_b2: bool) -> dict[str, Any]:
    caches = (
        [str(path) for path in HARNESS_ROOT.rglob("__pycache__")]
        + [str(path) for path in HARNESS_ROOT.rglob("*.pyc")]
        + [str(path) for path in (QUALITY_ROOT / "analysis-tools").rglob("__pycache__")]
        + [str(path) for path in (QUALITY_ROOT / "analysis-tools").rglob("*.pyc")]
    )
    if caches:
        raise SystemExit(f"Python cache artifacts are prohibited: {caches[:3]}")
    entries = schedule()
    validate(entries)
    prompts = {
        task_id: __import__("hashlib").sha256(
            render(BENCHMARK_ROOT / "questions/development" / f"{task_id}.json", entries[next(i for i, row in enumerate(entries) if row["task_id"] == task_id)]["question_sha256"])
        ).hexdigest()
        for task_id in TASK_IDS
    }
    if prompts != PROMPT_SHA256:
        raise SystemExit("rendered prompt hash drift")
    answers = {task: sha256(BENCHMARK_ROOT / "answers/development" / f"{task}.json") for task in TASK_IDS}
    if answers != ANSWER_SHA256:
        raise SystemExit("answer contract hash drift")
    fixed_files = {
        HARNESS_ROOT / "templates/prompt.txt": ORIGINAL_PROMPT_TEMPLATE_SHA256,
        HARNESS_ROOT / "config/baseline-1.json": ORIGINAL_BASELINE_1_SHA256,
        HARNESS_ROOT / "config/baseline-2.template.json": ORIGINAL_BASELINE_2_TEMPLATE_SHA256,
        HARNESS_ROOT / "config/limits.json": ORIGINAL_LIMITS_SHA256,
        BENCHMARK_ROOT / "manifests/public.json": DERIVED_PUBLIC_MANIFEST_SHA256,
        BENCHMARK_ROOT / "manifests/private.json": DERIVED_PRIVATE_MANIFEST_SHA256,
    }
    for path, expected in fixed_files.items():
        if sha256(path) != expected:
            raise SystemExit(f"fixed baseline input drift: {path}")
    public_manifest = load_json(BENCHMARK_ROOT / "manifests/public.json")
    if "repeat_task_ids" in public_manifest:
        raise SystemExit("legacy repeat_task_ids may not influence the exact three-trial schedule")
    offline_selftest = verify_bound_selftest(HARNESS_ROOT / "reports/offline-selftest.json", EXECUTION_REQUIRED_CHECKS)
    scoring_selftest = verify_bound_selftest(HARNESS_ROOT / "reports/scoring-selftest.json", SCORING_REQUIRED_CHECKS)
    analysis_inputs_selftest = verify_bound_selftest(HARNESS_ROOT / "reports/analysis-inputs-selftest.json", ANALYSIS_INPUT_REQUIRED_CHECKS)
    analysis_tools_required_checks = frozenset({
        "extractor-required-test-names-all-pass", "aggregator-required-test-names-all-pass",
    })
    analysis_tools_selftest = verify_bound_selftest(
        HARNESS_ROOT / "reports/analysis-tools-selftest.json", analysis_tools_required_checks,
    )
    verify_analysis_tool_suites(analysis_tools_selftest)
    b2_contract = load_json(HARNESS_ROOT / "config/b2-runtime.json")
    if require_b2:
        b1, b2, pair_evidence = pair()
        attestation = load_json(pathlib.Path(b2_contract["attestation_path"]))
        probe_path = pathlib.Path(attestation["probe_report_path"])
        probe = load_json(probe_path)
        reproducibility = validate_b2_reproducibility(b2_contract)
        if attestation.get("verdict") != "verified-clean-baseline" or attestation.get("binary_sha256") != b2_contract["binary_sha256"]:
            raise SystemExit("B2 attestation semantic contract failed")
        if attestation.get("candidate_code_present") is not False or attestation.get("candidate_environment_present") is not False:
            raise SystemExit("B2 attestation reports candidate contamination")
        if attestation.get("activation_environment") != {"CODEMAP_BASELINE_READ_ONLY": "1"} or attestation.get("mcp_command_tail") != ["mcp"]:
            raise SystemExit("B2 activation contract drift")
        if sha256(probe_path) != attestation.get("probe_report_sha256") or probe.get("passed") is not True:
            raise SystemExit("B2 model-free MCP probe is absent or failed")
        b2_state = {
            "contract": b2_contract,
            "attestation_sha256": sha256(pathlib.Path(b2_contract["attestation_path"])),
            "probe_report_sha256": sha256(probe_path),
            "pair_evidence": pair_evidence,
            "materialized_config_canonical_sha256": {
                "B1": canonical_sha256(b1), "B2": canonical_sha256(b2)
            },
            "materialized_config_file_sha256": {
                "B1": rendered_json_sha256(b1), "B2": rendered_json_sha256(b2)
            },
            "source_tree_path": str(QUALITY_ROOT / "b2/source"),
            "source_tree_sha256": tree_digest(QUALITY_ROOT / "b2/source"),
            "source_to_binary_reproducibility": reproducibility,
        }
    else:
        b2_state = {"contract": b2_contract, "execution_ready": False}

    critical = file_hashes(
        QUALITY_ROOT,
        (
            "harness/scripts/*.py", "harness/scripts/*.sh", "harness/config/*.json",
            "harness/templates/*", "harness/schemas/*.json", "harness/provenance/*.json",
            "benchmark/manifests/*.json", "benchmark/questions/development/*.json",
            "benchmark/answers/development/*.json", "runtime/opencode",
            "analysis-tools/**/*",
            "audits/*.md",
            "harness/reports/offline-selftest.json", "harness/reports/scoring-selftest.json",
            "harness/reports/analysis-inputs-selftest.json", "harness/reports/analysis-tools-selftest.json",
        ),
    )
    value = {
        "schema_version": 1,
        "generation_kind": "baseline-3x",
        "execution_ready": require_b2,
        "model": MODEL,
        "provider": PROVIDER,
        "tasks": list(TASK_IDS),
        "trials": ["r1", "r2", "r3"],
        "arms": ["B1", "B2"],
        "task_count": 14,
        "pair_count": 42,
        "session_count": 84,
        "candidate_session_count": 0,
        "judgment_count": 252,
        "schedule": entries,
        "pair_order_rule": PAIR_ORDER_RULE,
        "execution_policy": {
            "max_concurrency": 3,
            "max_attempts_per_slot": 3,
            "max_replacements_per_slot": 2,
            "replacement_backoff_seconds": {"attempt_2": 5, "attempt_3": 15},
            "provider_retry_after_used": False,
            "same_pair_parallel": False,
            "predecessor_terminal_before_successor": True,
            "agent_timeout_step_cap_output_cap_are_observed_outcomes": True,
            "replacement_allowed_only_for": ["transient_provider", "transient_network", "transient_auth"],
            "replacement_preserves_original_attempt": True,
            "mcp_or_code_or_config_fix_requires_new_generation": True,
        },
        "common_process_environment": {
            "CODEMAP_BASELINE_READ_ONLY": "1",
            "OPENCODE_DISABLE_MODELS_FETCH": "1",
            "OPENCODE_DISABLE_AUTOUPDATE": "1",
            "OPENCODE_DISABLE_PROJECT_CONFIG": "1",
            "NO_COLOR": "1",
        },
        "arm_environment_diff": [],
        "allowed_arm_config_diff": ["mcp"],
        "b2_mcp_command_contract": ["<clean-b2-binary-from-b2-runtime.json>", "mcp"],
        "forced_mcp_use": False,
        "limits": load_json(HARNESS_ROOT / "config/limits.json"),
        "prompt_sha256_by_task": prompts,
        "answer_sha256_by_task": answers,
        "source": {"path": str(CORPUS_ROOT), "tree_sha256": tree_digest(CORPUS_ROOT), "expected_tree_sha256": SOURCE_TREE_SHA256},
        "index": {"path": str(GOLDEN_CODEMAP), "tree_sha256": tree_digest(GOLDEN_CODEMAP), "expected_tree_sha256": GOLDEN_TREE_SHA256, "prebuilt_only": True},
        "opencode": {"path": str(OPENCODE_BINARY), "sha256": sha256(OPENCODE_BINARY), "expected_sha256": OPENCODE_SHA256},
        "isolation": {
            "quality_root_denied_except_exact_runtime_allowlist": str(QUALITY_ROOT),
            "agent_visible_quality_allowlist": ["<current-run-root>", str(OPENCODE_BINARY), "<clean-b2-binary>", "<current-auth-runtime>"],
            "benchmark_root_denied": str(BENCHMARK_ROOT),
            "legacy_root_denied": str(LEGACY_ROOT),
            "product_root_denied": str(PRODUCT_ROOT),
            "source_read_only": True,
            "index_read_only": True,
            "copy_method": "/bin/cp -cRp; no fallback",
            "clonefile_synthetic_report_sha256": sha256(HARNESS_ROOT / "reports/clonefile-synthetic.json"),
            "full_clone_materialization_report_sha256": sha256(HARNESS_ROOT / "reports/full-clone-materialization.json"),
        },
        "b2": b2_state,
        "critical_file_sha256": critical,
        "provenance": {
            "original_generation_file_sha256": ORIGINAL_GENERATION_FILE_SHA256,
            "original_generation_internal_seal": ORIGINAL_GENERATION_INTERNAL_SEAL,
            "original_public_manifest_sha256": ORIGINAL_PUBLIC_MANIFEST_SHA256,
            "original_private_manifest_sha256": ORIGINAL_PRIVATE_MANIFEST_SHA256,
        },
        "no_model_selftests": {
            "execution_report_sha256": sha256(HARNESS_ROOT / "reports/offline-selftest.json"),
            "execution_check_count": len(offline_selftest["checks"]),
            "scoring_report_sha256": sha256(HARNESS_ROOT / "reports/scoring-selftest.json"),
            "scoring_check_count": len(scoring_selftest["checks"]),
            "analysis_inputs_report_sha256": sha256(HARNESS_ROOT / "reports/analysis-inputs-selftest.json"),
            "analysis_inputs_check_count": len(analysis_inputs_selftest["checks"]),
            "analysis_tools_report_sha256": sha256(HARNESS_ROOT / "reports/analysis-tools-selftest.json"),
            "analysis_tools_check_count": len(analysis_tools_selftest["checks"]),
        },
        "legacy_contracts": {
            "session_metrics_schema": {
                "status": "unused_legacy_not_accepted",
                "sha256": sha256(HARNESS_ROOT / "schemas/session-metrics.schema.json"),
            }
        },
    }
    if value["source"]["tree_sha256"] != SOURCE_TREE_SHA256 or value["index"]["tree_sha256"] != GOLDEN_TREE_SHA256:
        raise SystemExit("source or golden index drift")
    if value["opencode"]["sha256"] != OPENCODE_SHA256:
        raise SystemExit("OpenCode binary drift")
    rubric_path = HARNESS_ROOT / "config/scoring-rubric.json"
    value["scoring_seal"] = {
        "rubric_sha256": sha256(rubric_path),
        "automatic_run_metrics_schema_sha256": sha256(HARNESS_ROOT / "schemas/automatic-run-metrics.schema.json"),
        "analysis_input_seal_schema_sha256": sha256(HARNESS_ROOT / "schemas/analysis-input-seal.schema.json"),
        "phase1_judgment_schema_sha256": sha256(HARNESS_ROOT / "schemas/phase1-correctness-judgment.schema.json"),
        "phase2_judgment_schema_sha256": sha256(HARNESS_ROOT / "schemas/phase2-process-judgment.schema.json"),
        "manual_evaluation_components_schema_sha256": sha256(HARNESS_ROOT / "schemas/manual-evaluation-components.schema.json"),
        "final_judgment_schema_sha256": sha256(HARNESS_ROOT / "schemas/judgment.schema.json"),
        "scoring_pipeline_sha256": sha256(HARNESS_ROOT / "scripts/scoring_pipeline.py"),
        "analysis_input_builder_sha256": sha256(HARNESS_ROOT / "scripts/build_analysis_inputs.py"),
        "automatic_metrics_extractor_sha256": sha256(QUALITY_ROOT / "analysis-tools/extract_run_metrics.py"),
        "baseline_aggregator_sha256": sha256(QUALITY_ROOT / "analysis-tools/aggregate_baseline_metrics.py"),
        "judgments_per_output": 3,
        "blind_arm_trial_pair_order": True,
        "raw_three_judgments_preserved": True,
    }
    generation_core = canonical_sha256(value)
    value["generation_id"] = f"baseline-3x-{generation_core[:16]}"
    value["generation_seal_sha256"] = canonical_sha256(value)
    return value


def lock_tree(root: pathlib.Path) -> None:
    if not root.exists():
        return
    paths = [root, *root.rglob("*")] if root.is_dir() else [root]
    for path in sorted(paths, reverse=True):
        if not path.is_symlink():
            path.chmod(path.stat().st_mode & ~0o222)


def lock_generation_inputs(generation_path: pathlib.Path, value: dict[str, Any]) -> None:
    for root in (
        HARNESS_ROOT / "scripts", HARNESS_ROOT / "config", HARNESS_ROOT / "templates",
        HARNESS_ROOT / "schemas", HARNESS_ROOT / "provenance", BENCHMARK_ROOT,
        CORPUS_ROOT, GOLDEN_CODEMAP, QUALITY_ROOT / "b2/source", QUALITY_ROOT / "analysis-tools", QUALITY_ROOT / "audits",
    ):
        lock_tree(root)
    lock_tree(OPENCODE_BINARY)
    lock_tree(HARNESS_ROOT / "reports/clonefile-synthetic.json")
    lock_tree(HARNESS_ROOT / "reports/full-clone-materialization.json")
    lock_tree(HARNESS_ROOT / "reports/offline-selftest.json")
    lock_tree(HARNESS_ROOT / "reports/scoring-selftest.json")
    lock_tree(HARNESS_ROOT / "reports/analysis-inputs-selftest.json")
    lock_tree(HARNESS_ROOT / "reports/analysis-tools-selftest.json")
    contract = value["b2"]["contract"]
    lock_tree(pathlib.Path(contract["binary_path"]))
    lock_tree(pathlib.Path(contract["attestation_path"]))
    attestation = load_json(pathlib.Path(contract["attestation_path"]))
    lock_tree(pathlib.Path(attestation["probe_report_path"]))
    for build in attestation["builds"]:
        lock_tree(pathlib.Path(build["binary_path"]))
        lock_tree(pathlib.Path(build["cargo_rustc_info_path"]))
    lock_tree(generation_path)


def verify(path: pathlib.Path) -> None:
    sealed = load_json(path)
    unsealed = dict(sealed)
    seal = unsealed.pop("generation_seal_sha256", None)
    if seal != canonical_sha256(unsealed):
        raise SystemExit("generation file self-seal mismatch")
    current = snapshot(require_b2=bool(sealed.get("execution_ready")))
    if current != sealed:
        raise SystemExit("generation drift: same generation may not resume; create a new generation and restart all samples")


def require_execution_generation(value: dict[str, Any]) -> dict[str, Any]:
    """Reject a signed planning snapshot at every external-execution boundary."""
    if value.get("generation_kind") != "baseline-3x" or value.get("execution_ready") is not True:
        raise SystemExit("generation is not an execution-ready sealed baseline-3x generation")
    return value


def verify_execution(path: pathlib.Path) -> dict[str, Any]:
    verify(path)
    return require_execution_generation(load_json(path))


def main() -> int:
    if len(sys.argv) != 3 or sys.argv[1] not in {"plan", "seal", "verify", "verify-execution"}:
        raise SystemExit("usage: generation.py plan|seal|verify|verify-execution OUTPUT")
    action, output = sys.argv[1], pathlib.Path(sys.argv[2])
    if action == "verify":
        verify(output)
        return 0
    if action == "verify-execution":
        verify_execution(output)
        return 0
    if output.exists():
        raise SystemExit("generation files are immutable and may not be overwritten")
    value = snapshot(require_b2=action == "seal")
    write_json(output, value)
    if action == "seal":
        lock_generation_inputs(output, value)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
