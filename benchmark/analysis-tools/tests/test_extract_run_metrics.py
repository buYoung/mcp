from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Iterator


ANALYSIS_TOOLS = Path(__file__).resolve().parents[1]
SCRIPT = ANALYSIS_TOOLS / "extract_run_metrics.py"
SCHEMA_PATH = ANALYSIS_TOOLS.parent / "harness/schemas/automatic-run-metrics.schema.json"
FIXTURES = Path(__file__).resolve().parent / "fixtures"
ACTUAL_TEMPLATE_ROOT = FIXTURES / "actual-r1"
ACTUAL_RUN_NAME = "API-05-r1-B1-a1-fixture"

SPEC = importlib.util.spec_from_file_location("extract_run_metrics", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("failed to load extractor module")
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)
SCHEMA = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def refresh_normalized_and_capture(run_dir: Path) -> None:
    events_path = run_dir / "raw/events.jsonl"
    stderr_path = run_dir / "raw/stderr.log"
    raw = events_path.read_bytes()
    raw_lines = raw.splitlines()
    parsed = [json.loads(line) for line in raw_lines]
    normalized = load_json(run_dir / "normalized.json")
    normalized["official_opencode"]["raw_reference"] = {
        "path": str(events_path.resolve()),
        "sha256": hashlib.sha256(raw).hexdigest(),
        "line_count": len(raw_lines),
    }
    normalized["official_opencode"]["raw_jsonl"] = [
        {"line": index, "raw": value} for index, value in enumerate(parsed, start=1)
    ]
    step_lines: dict[tuple[str, str], int] = {}
    for index, value in enumerate(parsed, start=1):
        step = MODULE.step_finish_record(value, index)
        if step is not None and step["message_id"] and step["part_id"]:
            step_lines[(step["message_id"], step["part_id"])] = index
    for entry in normalized["official_opencode"]["token_usage"]:
        parts = str(entry["step_id"]).split(":")
        entry["event_line"] = step_lines[(parts[-2], parts[-1])]
    write_json(run_dir / "normalized.json", normalized)

    wrapper = load_json(run_dir / "wrapper.json")
    stdout_bytes = events_path.stat().st_size
    stderr_bytes = stderr_path.stat().st_size
    wrapper["output_bytes"]["observed"] = {"stdout": stdout_bytes, "stderr": stderr_bytes}
    wrapper["output_bytes"]["kept"] = {"stdout": stdout_bytes, "stderr": stderr_bytes}
    wrapper["output_bytes"]["dropped"] = {"stdout": 0, "stderr": 0}
    write_json(run_dir / "wrapper.json", wrapper)


def make_artifact_manifest(run_dir: Path) -> dict[str, Any]:
    artifacts = {}
    root_manifest = run_dir / "artifact-manifest.json"
    for path in sorted(run_dir.rglob("*")):
        if path.is_file() and path != root_manifest:
            evidence: dict[str, Any] = {
                "sha256": sha256(path),
                "bytes": path.stat().st_size,
            }
            if path.is_symlink():
                evidence["symlink_target"] = os.readlink(path)
            artifacts[str(path.relative_to(run_dir))] = evidence
    manifest = {"schema_version": 1, "artifacts": artifacts}
    write_json(run_dir / "artifact-manifest.json", manifest)
    return manifest


def lock_run_tree(run_dir: Path) -> None:
    for path in sorted(run_dir.rglob("*"), reverse=True):
        if not path.is_symlink():
            path.chmod(path.stat().st_mode & ~0o222)
    run_dir.chmod(run_dir.stat().st_mode & ~0o222)


def make_tree_owner_writable(root: Path) -> None:
    for path in [root, *root.rglob("*")]:
        if not path.is_symlink():
            path.chmod(path.stat().st_mode | 0o200)


def append_or_insert_event(run_dir: Path, event: dict[str, Any], before_final_step: bool) -> None:
    path = run_dir / "raw/events.jsonl"
    lines = path.read_text(encoding="utf-8").splitlines()
    rendered = json.dumps(event, ensure_ascii=False, separators=(",", ":"))
    if before_final_step:
        lines.insert(len(lines) - 1, rendered)
    else:
        lines.append(rendered)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def change_nested_read_input(run_dir: Path, new_input: dict[str, Any]) -> None:
    path = run_dir / "raw/events.jsonl"
    values = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    values[4]["data"]["input"] = new_input
    path.write_text(
        "".join(json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n" for value in values),
        encoding="utf-8",
    )


def configure_clean_non_stop_terminal(run_dir: Path, terminal_behavior: str) -> None:
    (run_dir / "raw/events.jsonl").write_bytes(b"")
    normalized = load_json(run_dir / "normalized.json")
    normalized["official_opencode"]["token_usage"] = []
    replay = normalized["official_opencode"]["replay"]
    replay.update(
        {
            "completed_model_steps": 0,
            "completed_model_step_ids": [],
            "completed_tool_completions": 0,
            "completed_tool_call_ids": [],
            "completed_error_tool_calls": 0,
            "completed_error_tool_call_ids": [],
            "assistant_text_parts": 0,
            "unknown_events": 0,
            "malformed_events": 0,
            "partial_events": 0,
            "duplicate_events": 0,
            "duplicate_conflicts": 0,
            "protocol_failures": [],
            "termination_cause": terminal_behavior,
            "legacy_combined_diagnostic": 0,
        }
    )
    replay.pop("final_assistant_text", None)
    replay.pop("final_model_step_reason", None)
    write_json(run_dir / "normalized.json", normalized)

    wrapper = load_json(run_dir / "wrapper.json")
    for key, value in replay.items():
        if key in wrapper:
            wrapper[key] = copy.deepcopy(value)
    wrapper.pop("final_assistant_text", None)
    wrapper.pop("final_model_step_reason", None)
    wrapper["reducer_lines_accepted"] = 0
    wrapper["termination_cause"] = terminal_behavior
    wrapper["exit_code"] = 124 if terminal_behavior == "timeout" else 0
    wrapper["limits"].update(
        {
            "timeout": terminal_behavior == "timeout",
            "output_limit": terminal_behavior == "output_limit",
            "turn_limit": False,
            "model_step_limit": terminal_behavior == "model_step_limit",
            "protocol_failure": False,
            "signal": None,
            "termination_cause": terminal_behavior,
        }
    )
    write_json(run_dir / "wrapper.json", wrapper)


def replace_read_with_split_lifecycle(run_dir: Path, *, conflicting: bool = False) -> None:
    path = run_dir / "raw/events.jsonl"
    values = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    values[4:5] = [
        {
            "type": "tool_started",
            "session_id": "ses_r1",
            "call_id": "call_read",
            "status": "running",
            "tool": "read",
            "input": {"filePath": "src/a.ts", "limit": 5},
            "time": {"start": 20},
        },
        {
            "type": "tool_completed",
            "session_id": "ses_r1",
            "call_id": "call_read",
            "status": "completed",
            "tool": "read",
            "input": {"filePath": "src/a.ts", "limit": 6} if conflicting else None,
            "output": "code",
            "time": {"start": 21, "end": 24} if conflicting else {"end": 24},
        },
    ]
    if not conflicting:
        values[5].pop("input")
    path.write_text(
        "".join(json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n" for value in values),
        encoding="utf-8",
    )


def replace_read_with_error_only_terminal(run_dir: Path) -> None:
    path = run_dir / "raw/events.jsonl"
    values = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    values[4] = {
        "type": "tool_error",
        "session_id": "ses_r1",
        "call_id": "call_read",
        "status": "error",
        "tool": "read",
        "input": {"filePath": "src/a.ts", "limit": 5},
        "error": "synthetic read failure",
        "time": {"start": 20, "end": 24},
    }
    path.write_text(
        "".join(json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n" for value in values),
        encoding="utf-8",
    )


@contextmanager
def materialized_r1(
    *,
    attempt_number: int = 1,
    classification: dict[str, Any] | None = None,
    capture_mismatch: bool = False,
    accepted_tail: bool = False,
    multi_session_same_call_id: bool = False,
    partial_read: bool = False,
    malformed_completed_range: bool = False,
    tool_conflict: bool = False,
    token_mutation: str | None = None,
    clean_non_stop_terminal: str | None = None,
    split_tool_lifecycle: str | None = None,
    artifact_symlink: str | None = None,
    artifact_special: str | None = None,
) -> Iterator[Path]:
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary) / "actual-r1"
        shutil.copytree(ACTUAL_TEMPLATE_ROOT, root)
        make_tree_owner_writable(root)
        run_dir = root / ACTUAL_RUN_NAME
        run_name = ACTUAL_RUN_NAME
        if attempt_number != 1:
            run_name = f"API-05-r1-B1-a{attempt_number}-fixture"
            target = root / run_name
            run_dir.rename(target)
            run_dir = target
            manifest = load_json(run_dir / "run.manifest.json")
            manifest["run_id"] = run_name
            manifest["attempt_number"] = attempt_number
            write_json(run_dir / "run.manifest.json", manifest)

        if classification is not None:
            write_json(run_dir / "attempt-classification.json", classification)

        if clean_non_stop_terminal is not None:
            configure_clean_non_stop_terminal(run_dir, clean_non_stop_terminal)

        if split_tool_lifecycle == "completed":
            replace_read_with_split_lifecycle(run_dir)
        elif split_tool_lifecycle == "conflict":
            replace_read_with_split_lifecycle(run_dir, conflicting=True)
        elif split_tool_lifecycle == "error":
            replace_read_with_error_only_terminal(run_dir)
            for name in ("wrapper.json", "normalized.json"):
                value = load_json(run_dir / name)
                container = value if name == "wrapper.json" else value["official_opencode"]["replay"]
                container["completed_error_tool_calls"] = 1
                container["completed_error_tool_call_ids"] = ["ses_r1:tool:call_read"]
                write_json(run_dir / name, value)
        elif split_tool_lifecycle is not None:
            raise AssertionError(split_tool_lifecycle)

        if accepted_tail:
            append_or_insert_event(
                run_dir,
                {
                    "type": "tool_completed",
                    "session_id": "ses_tail",
                    "call_id": "ignored_tail",
                    "status": "completed",
                    "tool": "grep",
                    "input": {"pattern": "tail"},
                    "output": "tail",
                    "time": {"start": 90, "end": 91},
                },
                before_final_step=False,
            )
            wrapper = load_json(run_dir / "wrapper.json")
            wrapper["reducer_input_sealed"] = True
            write_json(run_dir / "wrapper.json", wrapper)

        if multi_session_same_call_id:
            append_or_insert_event(
                run_dir,
                {
                    "type": "message",
                    "data": {
                        "type": "tool_completed",
                        "session_id": "ses_other",
                        "call_id": "call_shared",
                        "status": "completed",
                        "tool": "grep",
                        "input": {"pattern": "other"},
                        "output": "other",
                        "time": {"start": 30, "end": 31},
                    },
                },
                before_final_step=True,
            )
            for name in ("wrapper.json", "normalized.json"):
                value = load_json(run_dir / name)
                container = value if name == "wrapper.json" else value["official_opencode"]["replay"]
                container["completed_tool_completions"] = 3
                container["completed_tool_call_ids"] = [
                    "ses_other:tool:call_shared",
                    "ses_r1:tool:call_read",
                    "ses_r1:tool:call_shared",
                ]
                write_json(run_dir / name, value)

        if partial_read:
            append_or_insert_event(
                run_dir,
                {
                    "type": "tool_started",
                    "session_id": "ses_r1",
                    "call_id": "partial_read",
                    "status": "running",
                    "tool": "read",
                    "input": {"filePath": "src/partial.ts", "limit": 0},
                    "time": {"start": 40},
                },
                before_final_step=True,
            )
            for name in ("wrapper.json", "normalized.json"):
                value = load_json(run_dir / name)
                container = value if name == "wrapper.json" else value["official_opencode"]["replay"]
                container["partial_events"] = 1
                container["protocol_failures"] = ["partial_event"]
                write_json(run_dir / name, value)

        if malformed_completed_range:
            change_nested_read_input(
                run_dir,
                {"filePath": "src/a.ts", "range": {"start": 9, "end": 2}},
            )

        if tool_conflict:
            append_or_insert_event(
                run_dir,
                {
                    "type": "tool_completed",
                    "session_id": "ses_r1",
                    "call_id": "call_read",
                    "status": "completed",
                    "tool": "read",
                    "input": {"filePath": "src/a.ts", "limit": 5},
                    "output": "conflicting output",
                    "time": {"start": 20, "end": 24},
                },
                before_final_step=True,
            )

        wrapper = load_json(run_dir / "wrapper.json")
        if not accepted_tail:
            wrapper["reducer_lines_accepted"] = len(
                (run_dir / "raw/events.jsonl").read_bytes().splitlines()
            )
        write_json(run_dir / "wrapper.json", wrapper)
        refresh_normalized_and_capture(run_dir)

        if clean_non_stop_terminal == "output_limit":
            wrapper = load_json(run_dir / "wrapper.json")
            wrapper["output_bytes"]["observed"]["stdout"] += 1
            wrapper["output_bytes"]["dropped"]["stdout"] = 1
            write_json(run_dir / "wrapper.json", wrapper)

        if token_mutation is not None:
            normalized = load_json(run_dir / "normalized.json")
            first = normalized["official_opencode"]["token_usage"][0]
            if token_mutation == "duplicate_conflict":
                duplicate = copy.deepcopy(first)
                duplicate["tokens"]["input"] = 11
                duplicate["tokens"]["total"] = 21
                normalized["official_opencode"]["token_usage"].append(duplicate)
            elif token_mutation == "total_mismatch":
                first["tokens"]["total"] = 99
            elif token_mutation == "negative":
                first["tokens"]["input"] = -1
            elif token_mutation == "float":
                first["tokens"]["input"] = 10.0
            else:
                raise AssertionError(token_mutation)
            write_json(run_dir / "normalized.json", normalized)

        if capture_mismatch:
            wrapper = load_json(run_dir / "wrapper.json")
            wrapper["output_bytes"]["kept"]["stdout"] += 1
            wrapper["output_bytes"]["observed"]["stdout"] += 1
            write_json(run_dir / "wrapper.json", wrapper)

        if artifact_symlink in {"safe", "mismatched"}:
            (run_dir / "artifact-target.txt").write_text("bound target\n", encoding="utf-8")
            (run_dir / "artifact-link.txt").symlink_to("./artifact-target.txt")
            nested_manifest = run_dir / "nested/artifact-manifest.json"
            nested_manifest.parent.mkdir()
            nested_manifest.write_text("nested evidence\n", encoding="utf-8")
        elif artifact_symlink == "escape":
            (root / "outside-target.txt").write_text("outside target\n", encoding="utf-8")
            (run_dir / "artifact-link.txt").symlink_to("../outside-target.txt")
        elif artifact_symlink is not None:
            raise AssertionError(artifact_symlink)
        if artifact_special == "fifo":
            os.mkfifo(run_dir / "artifact-fifo")
        elif artifact_special is not None:
            raise AssertionError(artifact_special)

        artifact = make_artifact_manifest(run_dir)
        if artifact_symlink == "mismatched":
            artifact["artifacts"]["artifact-link.txt"]["symlink_target"] = "different-target.txt"
            write_json(run_dir / "artifact-manifest.json", artifact)
        ledger = load_json(root / "ledger.template.json")
        ledger.pop("schema_version", None)
        ledger_path = root / "ledger.json"
        ledger["attempts"] = {}
        ledger["slots"] = {}
        classification_value = load_json(run_dir / "attempt-classification.json")
        attempt = {
            "run_id": run_name,
            "attempt_number": attempt_number,
            "slot_key": "API-05:r1:B1",
            "pair_id": "API-05-r1",
            "task_id": "API-05",
            "trial_id": "r1",
            "arm": "B1",
            "pair_order_index": 0,
            "state": "terminal",
            "published_at_ns": 2200000000,
            "run_dir": str(run_dir.resolve()),
            "artifact_manifest_sha256": sha256(run_dir / "artifact-manifest.json"),
            "artifact_count": len(artifact["artifacts"]),
            "classification": classification_value,
        }
        ledger["attempts"][run_name] = attempt
        all_ids = [run_name] if attempt_number == 1 else [ACTUAL_RUN_NAME, run_name]
        ledger["slots"]["API-05:r1:B1"] = {
            "latest_run_id": run_name,
            "latest_attempt_number": attempt_number,
            "measurement_status": classification_value["measurement_status"],
            "replacement_allowed": classification_value["replacement_allowed"],
            "all_run_ids": all_ids,
        }
        write_json(ledger_path, ledger)
        lock_run_tree(run_dir)
        yield run_dir


class ExtractRunMetricsTest(unittest.TestCase):
    def assert_schema_valid(self, result: dict[str, Any]) -> None:
        errors = MODULE.validate_json_schema(result, SCHEMA)
        self.assertEqual(errors, [], "\n".join(errors[:20]))

    def test_actual_r1_is_eligible_and_uses_parser_normalized_records(self) -> None:
        with materialized_r1() as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(result)
        self.assertEqual(result["schema_version"], 2)
        self.assertFalse(result["run"]["scorer_input_allowed"])
        self.assertEqual(result["run"]["integrity"]["status"], "verified")
        self.assertTrue(result["run"]["integrity"]["aggregation_eligible"])
        self.assertTrue(result["experiment"]["aggregation_eligible"])
        self.assertEqual(result["experiment"]["measurement_status"], "valid")
        self.assertTrue(result["run"]["lifecycle"]["publication"]["latest_published_attempt"])
        self.assertTrue(result["run"]["lifecycle"]["artifact_seal"]["verified"])
        self.assertEqual(result["cost"]["tool_calls_total"], 2)
        self.assertEqual(result["run"]["incomplete_tool_calls"], [])
        self.assertEqual(result["cost"]["tokens"]["source"], "normalized.json:official_opencode.token_usage")
        self.assertEqual(result["cost"]["tokens"]["total"], 48)
        self.assertTrue(all(":model:" in step["identity"] for step in result["cost"]["tokens"]["per_step"]))
        self.assertEqual(result["dialogue"]["final_answer"]["part_count"], 2)
        self.assertEqual(result["dialogue"]["final_answer"]["text"], "final answer")
        nested_call = next(call for call in result["run"]["completed_tool_calls"] if call["call_id"] == "call_read")
        self.assertEqual(nested_call["selected_completion_line"], 5)
        self.assertEqual(result["experiment"]["started_at_ns"], 1000000000)
        self.assertEqual(result["experiment"]["ended_at_ns"], 2000000000)

    def test_safe_internal_symlink_is_bound_and_eligible(self) -> None:
        with materialized_r1(artifact_symlink="safe") as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(result)
        artifact_seal = result["run"]["lifecycle"]["artifact_seal"]
        self.assertTrue(artifact_seal["verified"])
        self.assertEqual(artifact_seal["issues"], [])
        self.assertEqual(artifact_seal["symlink_paths"], ["artifact-link.txt"])
        self.assertTrue(result["experiment"]["aggregation_eligible"])

    def test_unsafe_or_mismatched_symlink_fails_closed(self) -> None:
        expected_issue = {
            "escape": "unsafe_artifact_symlink",
            "mismatched": "artifact_content_mismatch",
        }
        for symlink_kind, issue in expected_issue.items():
            with self.subTest(symlink_kind=symlink_kind):
                with materialized_r1(artifact_symlink=symlink_kind) as run_dir:
                    result = MODULE.extract_run_metrics(run_dir)
                self.assert_schema_valid(result)
                artifact_seal = result["run"]["lifecycle"]["artifact_seal"]
                self.assertFalse(artifact_seal["verified"])
                self.assertIn(issue, artifact_seal["issues"])
                self.assertFalse(result["experiment"]["aggregation_eligible"])

    def test_nonregular_artifact_entry_fails_closed(self) -> None:
        with materialized_r1(artifact_special="fifo") as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(result)
        artifact_seal = result["run"]["lifecycle"]["artifact_seal"]
        self.assertFalse(artifact_seal["verified"])
        self.assertIn("nonregular_artifact_entry", artifact_seal["issues"])
        self.assertFalse(result["experiment"]["aggregation_eligible"])

    def test_clean_valid_non_stop_terminals_are_verified_and_eligible(self) -> None:
        environment = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}
        for terminal_behavior in ("timeout", "model_step_limit", "output_limit"):
            with self.subTest(terminal_behavior=terminal_behavior):
                classification = {
                    "measurement_status": "valid",
                    "terminal_behavior": terminal_behavior,
                    "replacement_allowed": False,
                    "replacement_category": None,
                    "generation_invalid": False,
                    "reason": f"synthetic clean {terminal_behavior}",
                }
                with materialized_r1(
                    classification=classification,
                    clean_non_stop_terminal=terminal_behavior,
                ) as run_dir:
                    result = MODULE.extract_run_metrics(run_dir)
                    strict = subprocess.run(
                        [
                            sys.executable,
                            str(SCRIPT),
                            str(run_dir),
                            "--require-aggregation-eligible",
                            "--compact",
                        ],
                        check=False,
                        capture_output=True,
                        text=True,
                        env=environment,
                    )
                self.assert_schema_valid(result)
                self.assertEqual(result["run"]["integrity"]["status"], "verified")
                self.assertTrue(result["experiment"]["aggregation_eligible"])
                self.assertEqual(strict.returncode, 0, strict.stderr)
                self.assertIsNone(result["cost"]["tokens"]["total"])
                self.assertFalse(result["cost"]["tokens"]["coverage_complete"])
                self.assertIsNone(result["dialogue"]["final_answer"]["text"])
                self.assertFalse(result["dialogue"]["final_answer"]["coverage_complete"])
                terminal_missing = [
                    item
                    for item in result["missing_data"]["fields"]
                    if item["field"].startswith("dialogue.final_answer")
                    or item["field"].startswith("cost.tokens")
                ]
                self.assertTrue(terminal_missing)
                self.assertTrue(all(item["critical"] is False for item in terminal_missing))

    def test_clean_transient_infrastructure_failures_are_verified_but_ineligible(self) -> None:
        environment = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}
        for category in ("auth", "provider", "network"):
            with self.subTest(category=category):
                classification = {
                    "measurement_status": "infrastructure_invalid",
                    "terminal_behavior": "infrastructure_error",
                    "replacement_allowed": True,
                    "replacement_category": f"transient_{category}",
                    "generation_invalid": False,
                    "reason": f"synthetic clean transient {category} failure",
                }
                with materialized_r1(
                    classification=classification,
                    clean_non_stop_terminal="infrastructure_error",
                ) as run_dir:
                    result = MODULE.extract_run_metrics(run_dir)
                    strict = subprocess.run(
                        [
                            sys.executable,
                            str(SCRIPT),
                            str(run_dir),
                            "--require-aggregation-eligible",
                            "--compact",
                        ],
                        check=False,
                        capture_output=True,
                        text=True,
                        env=environment,
                    )
                self.assert_schema_valid(result)
                self.assertEqual(result["run"]["integrity"]["status"], "verified")
                self.assertFalse(result["experiment"]["aggregation_eligible"])
                self.assertEqual(strict.returncode, 3, strict.stderr)
                self.assertNotIn(
                    "critical_integrity_invalid",
                    result["experiment"]["aggregation_ineligible_reasons"],
                )

    def test_split_tool_lifecycle_merges_started_and_completed_fields(self) -> None:
        with materialized_r1(split_tool_lifecycle="completed") as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(result)
        self.assertEqual(result["run"]["integrity"]["status"], "verified")
        call = next(
            item for item in result["run"]["completed_tool_calls"] if item["call_id"] == "call_read"
        )
        self.assertEqual(call["revision_count"], 2)
        self.assertEqual(call["client_observed_start_ms"], 20)
        self.assertEqual(call["client_observed_end_ms"], 24)
        self.assertEqual(call["client_observed_elapsed_ms"], 4)
        self.assertEqual(call["input_utf8_bytes"], len(b'{"filePath":"src/a.ts","limit":5}'))
        self.assertEqual(call["output_utf8_bytes"], 4)
        self.assertEqual(call["field_provenance"]["input"], [5])
        self.assertEqual(call["field_provenance"]["output"], [6])
        self.assertEqual(call["field_provenance"]["time_start"], [5])
        self.assertEqual(call["field_provenance"]["time_end"], [6])

    def test_error_only_tool_terminal_preserves_error_without_critical_output_missing(self) -> None:
        with materialized_r1(split_tool_lifecycle="error") as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(result)
        self.assertEqual(result["run"]["integrity"]["status"], "verified")
        self.assertTrue(result["experiment"]["aggregation_eligible"])
        call = next(
            item for item in result["run"]["completed_tool_calls"] if item["call_id"] == "call_read"
        )
        self.assertTrue(call["is_error"])
        self.assertEqual(call["error"], "synthetic read failure")
        self.assertFalse(call["output_present"])
        self.assertIsNone(call["output_utf8_bytes"])
        self.assertEqual(call["field_provenance"]["error"], [5])
        output_missing = [
            item
            for item in result["missing_data"]["fields"]
            if item["field"].endswith(".output")
        ]
        self.assertEqual(len(output_missing), 1)
        self.assertFalse(output_missing[0]["critical"])

    def test_split_tool_lifecycle_conflicts_fail_closed(self) -> None:
        with materialized_r1(split_tool_lifecycle="conflict") as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(result)
        self.assertEqual(result["run"]["integrity"]["status"], "invalid")
        self.assertFalse(result["experiment"]["aggregation_eligible"])
        call = next(
            item for item in result["run"]["completed_tool_calls"] if item["call_id"] == "call_read"
        )
        self.assertIn("input_conflict", call["conflicts"])
        self.assertIn("time_start_conflict", call["conflicts"])
        self.assertIsNone(call["input_utf8_bytes"])
        self.assertIsNone(call["client_observed_start_ms"])
        self.assertIsNone(call["client_observed_elapsed_ms"])

    def test_missing_fixture_is_schema_valid_raw_evidence_and_ineligible(self) -> None:
        result = MODULE.extract_run_metrics(FIXTURES / "missing")
        self.assert_schema_valid(result)
        self.assertFalse(result["experiment"]["aggregation_eligible"])
        self.assertEqual(result["run"]["integrity"]["status"], "invalid")
        self.assertIsNone(result["cost"]["tool_calls_total"])
        self.assertIsNone(result["cost"]["tokens"]["input"])

    def test_multi_session_same_call_id_stays_two_full_identities(self) -> None:
        with materialized_r1(multi_session_same_call_id=True) as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(result)
        shared = [call for call in result["run"]["completed_tool_calls"] if call["call_id"] == "call_shared"]
        self.assertEqual(len(shared), 2)
        self.assertEqual({call["identity"] for call in shared}, {"ses_r1:tool:call_shared", "ses_other:tool:call_shared"})
        self.assertEqual(result["cost"]["tool_calls_total"], 3)

    def test_accepted_boundary_tail_is_excluded(self) -> None:
        with materialized_r1(accepted_tail=True) as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(result)
        self.assertEqual(result["run"]["event_stream"]["tail_lines_excluded"], 1)
        self.assertTrue(result["run"]["event_stream"]["boundary_valid"])
        identities = {call["identity"] for call in result["run"]["completed_tool_calls"]}
        self.assertNotIn("ses_tail:tool:ignored_tail", identities)

    def test_invalid_replacement_classification_is_preserved_and_ineligible(self) -> None:
        classification = {
            "measurement_status": "infrastructure_invalid",
            "terminal_behavior": "infrastructure_error",
            "replacement_allowed": True,
            "replacement_category": "transient_provider",
            "generation_invalid": False,
            "reason": "synthetic provider failure",
        }
        with materialized_r1(attempt_number=2, classification=classification) as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(result)
        self.assertEqual(result["run"]["attempt_number"], 2)
        self.assertTrue(result["run"]["is_replacement"])
        self.assertEqual(result["experiment"]["measurement_status"], "infrastructure_invalid")
        self.assertTrue(result["experiment"]["replacement_allowed"])
        self.assertFalse(result["experiment"]["aggregation_eligible"])
        self.assertTrue(result["run"]["lifecycle"]["publication"]["latest_published_attempt"])

    def test_capture_byte_mismatch_fails_closed_while_artifact_seal_is_valid(self) -> None:
        with materialized_r1(capture_mismatch=True) as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(result)
        self.assertTrue(result["run"]["lifecycle"]["artifact_seal"]["verified"])
        self.assertEqual(result["cost"]["capture_integrity"]["status"], "invalid")
        self.assertFalse(result["experiment"]["aggregation_eligible"])
        codes = {issue["code"] for issue in result["run"]["integrity"]["issues"]}
        self.assertIn("capture_file_byte_mismatch", codes)

    def test_token_conflicts_mismatch_negative_and_float_fail_closed(self) -> None:
        expected_codes = {
            "duplicate_conflict": "duplicate_token_conflict",
            "total_mismatch": "official_token_total_mismatch",
            "negative": "critical_missing_data",
            "float": "critical_missing_data",
        }
        for mutation, expected_code in expected_codes.items():
            with self.subTest(mutation=mutation):
                with materialized_r1(token_mutation=mutation) as run_dir:
                    result = MODULE.extract_run_metrics(run_dir)
                self.assert_schema_valid(result)
                self.assertFalse(result["experiment"]["aggregation_eligible"])
                self.assertIsNone(result["cost"]["tokens"]["total"])
                codes = {issue["code"] for issue in result["run"]["integrity"]["issues"]}
                self.assertIn(expected_code, codes)

    def test_partial_read_is_diagnostic_only_and_malformed_completed_range_counts(self) -> None:
        invalid = {
            "measurement_status": "infrastructure_invalid",
            "terminal_behavior": "protocol_error",
            "replacement_allowed": False,
            "replacement_category": None,
            "generation_invalid": True,
            "reason": "synthetic partial event",
        }
        with materialized_r1(partial_read=True, classification=invalid) as run_dir:
            partial = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(partial)
        self.assertEqual(partial["run"]["integrity"]["status"], "invalid")
        self.assertIn(
            "reported_protocol_failure",
            {item["code"] for item in partial["run"]["integrity"]["issues"]},
        )
        self.assertEqual(len(partial["run"]["incomplete_tool_calls"]), 1)
        self.assertEqual(partial["cost"]["tool_calls_total"], 2)
        self.assertEqual(partial["unbounded_reads"]["count"], 0)

        with materialized_r1(malformed_completed_range=True) as run_dir:
            malformed = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(malformed)
        self.assertEqual(malformed["unbounded_reads"]["count"], 1)
        self.assertTrue(malformed["unbounded_reads"]["calls"][0]["malformed_bound_present"])
        self.assertEqual(malformed["unbounded_reads"]["calls"][0]["reason"], "malformed_bound")

    def test_same_tool_identity_conflict_nulls_affected_aggregates(self) -> None:
        with materialized_r1(tool_conflict=True) as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        self.assert_schema_valid(result)
        self.assertFalse(result["experiment"]["aggregation_eligible"])
        self.assertIsNone(result["cost"]["tool_calls_total"])
        self.assertIsNone(result["cost"]["tool_input_bytes"])
        self.assertIsNone(result["cost"]["client_observed_tool_ms"]["total"])
        self.assertIsNone(result["repeated_searches"]["groups"])
        self.assertIsNone(result["unbounded_reads"]["calls"])

    def test_schema_rejects_additional_property(self) -> None:
        with materialized_r1() as run_dir:
            result = MODULE.extract_run_metrics(run_dir)
        result["run"]["unexpected"] = True
        errors = MODULE.validate_json_schema(result, SCHEMA)
        self.assertTrue(any("additional property unexpected" in error for error in errors))

    def test_cli_require_aggregation_eligible_exit_codes(self) -> None:
        environment = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}
        with materialized_r1() as run_dir:
            valid = subprocess.run(
                [sys.executable, str(SCRIPT), str(run_dir), "--require-aggregation-eligible", "--compact"],
                check=False,
                capture_output=True,
                text=True,
                env=environment,
            )
        self.assertEqual(valid.returncode, 0, valid.stderr)

        invalid = subprocess.run(
            [sys.executable, str(SCRIPT), str(FIXTURES / "missing"), "--require-aggregation-eligible", "--compact"],
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )
        self.assertEqual(invalid.returncode, 3, invalid.stderr)

        with tempfile.TemporaryDirectory() as temporary:
            bad_schema = Path(temporary) / "bad-schema.json"
            write_json(bad_schema, {"const": {"impossible": True}})
            schema_failure = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    str(FIXTURES / "missing"),
                    "--schema",
                    str(bad_schema),
                    "--require-aggregation-eligible",
                    "--compact",
                ],
                check=False,
                capture_output=True,
                text=True,
                env=environment,
            )
        self.assertEqual(schema_failure.returncode, 4)


if __name__ == "__main__":
    unittest.main()
