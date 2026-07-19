#!/usr/bin/env python3
"""Fail-closed, offline extraction of raw automatic OpenCode run metrics.

This artifact contains arm/run linkage and raw evidence. It is forbidden as a
blind scorer input. Only the separate scoring pipeline may produce scorer-safe
artifacts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
import unicodedata
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


SCHEMA_VERSION = 2
EXTRACTOR_VERSION = "automatic-run-metrics-2"
TOOL_DONE = {"completed", "complete", "success", "error", "failed"}
TOOL_ERRORS = {"error", "failed"}
KNOWN_EVENT_LABELS = {
    "step-start",
    "step-finish",
    "tool",
    "tool-started",
    "tool-completed",
    "tool-error",
    "tool-use",
    "text",
    "message",
    "message-start",
    "message-completed",
}
TOOL_EVENT_LABELS = {"tool", "tool-started", "tool-completed", "tool-error"}
NAVIGATION_FAMILIES = {"overview", "search", "grep", "read", "find", "glob"}
QUERY_KEYS = ("query", "pattern", "search", "term", "text", "glob")
LIMIT_KEYS = ("limit", "line_limit", "lineLimit", "max_lines", "maxLines", "length")
RANGE_KEYS = ("range", "line_range", "lineRange")
RANGE_PAIRS = (
    ("start", "end"),
    ("start_line", "end_line"),
    ("startLine", "endLine"),
    ("line_start", "line_end"),
    ("lineStart", "lineEnd"),
)
HASH_PATTERN = re.compile(r"^[0-9a-f]{64}$")
FULL_TOOL_ID_PATTERN = re.compile(r"^.+:tool:.+$")
FULL_MODEL_ID_PATTERN = re.compile(r"^.+:model:.+:.+$")
MISSING = object()


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def payload_bytes(value: Any) -> bytes:
    return value.encode("utf-8") if isinstance(value, str) else canonical_json_bytes(value)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def deep_get(value: Any, path: Sequence[str], default: Any = None) -> Any:
    current = value
    for key in path:
        if not isinstance(current, Mapping) or key not in current:
            return default
        current = current[key]
    return current


def first_not_none(*values: Any) -> Any:
    for value in values:
        if value is not MISSING and value is not None:
            return value
    return None


def is_nonnegative_integer(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def is_positive_integer(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def is_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def canonical_distinct(values: Iterable[Any]) -> list[Any]:
    result: dict[str, Any] = {}
    for value in values:
        result.setdefault(sha256_bytes(canonical_json_bytes(value)), value)
    return list(result.values())


@dataclass
class Diagnostics:
    integrity_issues: list[dict[str, Any]] = field(default_factory=list)
    missing_fields: list[dict[str, Any]] = field(default_factory=list)
    _issue_keys: set[tuple[str, str, str]] = field(default_factory=set)
    _missing_keys: set[tuple[str, str]] = field(default_factory=set)

    def issue(
        self,
        code: str,
        field_path: str,
        detail: str,
        *,
        raw_event_lines: Iterable[int] = (),
        identities: Iterable[str] = (),
    ) -> None:
        key = (code, field_path, detail)
        if key in self._issue_keys:
            return
        self._issue_keys.add(key)
        self.integrity_issues.append(
            {
                "code": code,
                "field": field_path,
                "detail": detail,
                "raw_event_lines": sorted(set(raw_event_lines)),
                "identities": sorted(set(identities)),
            }
        )

    def missing(self, field_path: str, reason: str, *, critical: bool = False) -> None:
        key = (field_path, reason)
        if key not in self._missing_keys:
            self._missing_keys.add(key)
            self.missing_fields.append(
                {"field": field_path, "reason": reason, "critical": critical}
            )
        if critical:
            self.issue("critical_missing_data", field_path, reason)

    def missing_output(self) -> dict[str, Any]:
        return {
            "has_missing": bool(self.missing_fields),
            "count": len(self.missing_fields),
            "fields": sorted(
                self.missing_fields,
                key=lambda item: (item["field"], item["reason"]),
            ),
            "manual_review_required": [
                "dialogue.discovery_stages",
                "dialogue.first_wrong",
                "dialogue.correctness",
                "repeated_searches.groups",
                "post_first_wrong",
            ],
        }


def load_json_file(
    path: Path | None,
    diagnostics: Diagnostics,
    field_path: str,
    *,
    critical: bool,
) -> Any:
    if path is None or not path.is_file():
        diagnostics.missing(field_path, "file_not_present", critical=critical)
        return None
    try:
        with path.open("r", encoding="utf-8") as handle:
            value = json.load(handle)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        diagnostics.missing(field_path, f"invalid_or_unreadable_json:{error}", critical=critical)
        return None
    if not isinstance(value, Mapping):
        diagnostics.issue("invalid_json_root", field_path, "JSON root is not an object")
        return None
    return dict(value)


def find_first_file(root: Path, names: Sequence[str]) -> Path | None:
    for name in names:
        candidate = root / name
        if candidate.is_file():
            return candidate
    return None


def safe_relative(path: Path, run_dir: Path) -> str:
    try:
        return str(path.relative_to(run_dir))
    except ValueError:
        return str(path)


def file_record(path: Path | None, run_dir: Path) -> dict[str, Any]:
    if path is None or not path.is_file():
        return {"present": False, "path": None, "bytes": None, "sha256": None}
    try:
        return {
            "present": True,
            "path": safe_relative(path.resolve(), run_dir),
            "bytes": path.stat().st_size,
            "sha256": sha256_file(path),
        }
    except OSError:
        return {"present": True, "path": str(path), "bytes": None, "sha256": None}


def _candidates(envelope: Mapping[str, Any]) -> list[dict[str, Any]]:
    """Match harness/scripts/v4_events.py: top level plus immediate payloads."""
    values = [dict(envelope)]
    for key in ("event", "data", "part", "message"):
        value = envelope.get(key)
        if isinstance(value, Mapping):
            values.append(dict(value))
    return values


def _label(value: Mapping[str, Any]) -> str:
    return str(value.get("type", "")).lower().replace("_", "-").replace(".", "-")


def _first(values: Sequence[Mapping[str, Any]], keys: Sequence[str]) -> Any:
    for value in reversed(values):
        for key in keys:
            if value.get(key) is not None:
                return value[key]
    return None


def _present(values: Sequence[Mapping[str, Any]], keys: Sequence[str]) -> tuple[bool, Any]:
    for value in reversed(values):
        for key in keys:
            if key in value:
                return True, value[key]
    return False, None


def _state(value: Mapping[str, Any]) -> str | None:
    raw = value.get("status", value.get("state"))
    if isinstance(raw, Mapping):
        raw = raw.get("status", raw.get("type"))
    return str(raw).lower() if raw is not None else None


def _state_mapping(values: Sequence[Mapping[str, Any]], selected: Mapping[str, Any]) -> dict[str, Any]:
    direct = selected.get("state")
    if isinstance(direct, Mapping):
        return dict(direct)
    for value in reversed(values):
        candidate = value.get("state")
        if isinstance(candidate, Mapping):
            return dict(candidate)
    return {}


def event_has_known_label(envelope: Mapping[str, Any]) -> bool:
    return any(_label(value) in KNOWN_EVENT_LABELS for value in _candidates(envelope))


def step_finish_record(envelope: Mapping[str, Any], raw_event_line: int) -> dict[str, Any] | None:
    values = _candidates(envelope)
    selected = next((value for value in reversed(values) if _label(value) == "step-finish"), None)
    if selected is None:
        return None
    session = _first(values, ("sessionID", "sessionId", "session_id"))
    message = _first(values, ("messageID", "messageId", "message_id"))
    part = _first(values, ("partID", "partId", "part_id", "id"))
    identity = None
    if all(value is not None and str(value) for value in (session, message, part)):
        identity = f"{session}:model:{message}:{part}"
    tokens_present, tokens = _present([*values, selected], ("tokens", "usage"))
    return {
        "identity": identity,
        "session_id": str(session) if session is not None else None,
        "message_id": str(message) if message is not None else None,
        "part_id": str(part) if part is not None else None,
        "reason": _first(values, ("reason",)),
        "tokens_present": tokens_present,
        "tokens": tokens,
        "raw_event_line": raw_event_line,
    }


def text_record(envelope: Mapping[str, Any], raw_event_line: int) -> dict[str, Any] | None:
    values = _candidates(envelope)
    selected = next((value for value in reversed(values) if _label(value) == "text"), None)
    if selected is None:
        return None
    session = _first(values, ("sessionID", "sessionId", "session_id"))
    message = _first(values, ("messageID", "messageId", "message_id"))
    part = _first(values, ("partID", "partId", "part_id", "id"))
    text = _first(values, ("text",))
    identity = None
    if all(value is not None and str(value) for value in (session, message, part)):
        identity = f"{session}:text:{message}:{part}"
    return {
        "identity": identity,
        "session_id": str(session) if session is not None else None,
        "message_id": str(message) if message is not None else None,
        "part_id": str(part) if part is not None else None,
        "text": text,
        "raw_event_line": raw_event_line,
    }


def tool_revision(envelope: Mapping[str, Any], raw_event_line: int) -> dict[str, Any] | None:
    values = _candidates(envelope)
    selected = next(
        (value for value in reversed(values) if _label(value) in TOOL_EVENT_LABELS),
        None,
    )
    if selected is None:
        return None
    session = _first(values, ("sessionID", "sessionId", "session_id"))
    call = _first(values, ("callID", "callId", "call_id"))
    identity = None
    if session is not None and str(session) and call is not None and str(call):
        identity = f"{session}:tool:{call}"
    state = _state_mapping(values, selected)
    value_sources = [*values, state]
    input_present, input_value = _present(value_sources, ("input", "arguments", "args"))
    output_present, output_value = _present(value_sources, ("output", "result"))
    error_present, error_value = _present(value_sources, ("error",))
    time_value = state.get("time")
    if not isinstance(time_value, Mapping):
        time_value = _first(values, ("time",))
    time_mapping = dict(time_value) if isinstance(time_value, Mapping) else {}
    status = _state(selected)
    if status is None:
        status = _state(state)
    return {
        "identity": identity,
        "session_id": str(session) if session is not None else None,
        "call_id": str(call) if call is not None else None,
        "label": _label(selected),
        "tool": _first(values, ("tool", "toolName", "tool_name")),
        "status": status,
        "is_terminal": status in TOOL_DONE,
        "input_present": input_present,
        "input": input_value,
        "output_present": output_present,
        "output": output_value,
        "error_present": error_present,
        "error": error_value,
        "time": time_mapping,
        "raw_event_line": raw_event_line,
    }


def load_event_stream(
    events_path: Path,
    wrapper: Any,
    normalized: Any,
    diagnostics: Diagnostics,
) -> tuple[list[dict[str, Any]], dict[str, Any], bytes]:
    if not events_path.is_file():
        diagnostics.missing("run.input_files.events", "file_not_present", critical=True)
        raw = b""
    else:
        try:
            raw = events_path.read_bytes()
        except OSError as error:
            diagnostics.missing(
                "run.input_files.events",
                f"file_unreadable:{error}",
                critical=True,
            )
            raw = b""
    lines = raw.splitlines()
    accepted_value = deep_get(wrapper, ("reducer_lines_accepted",), MISSING)
    accepted_valid = is_nonnegative_integer(accepted_value)
    if not accepted_valid:
        diagnostics.issue(
            "invalid_reducer_boundary",
            "run.event_stream.reducer_lines_accepted",
            "reducer_lines_accepted must be a nonnegative integer",
        )
        accepted = 0
    else:
        accepted = accepted_value
    if accepted > len(lines):
        diagnostics.issue(
            "accepted_line_shortfall",
            "run.event_stream.reducer_lines_accepted",
            f"wrapper accepted {accepted} lines but file has {len(lines)}",
        )
    process_count = min(accepted, len(lines))
    reducer_sealed = deep_get(wrapper, ("reducer_input_sealed",), None)
    if len(lines) > accepted and reducer_sealed is not True:
        diagnostics.issue(
            "unexplained_event_tail",
            "run.event_stream.tail_lines_excluded",
            "lines exist beyond the accepted boundary but reducer_input_sealed is not true",
        )

    rows: list[dict[str, Any]] = []
    malformed_accepted: list[dict[str, Any]] = []
    malformed_tail: list[dict[str, Any]] = []
    unknown_accepted: list[int] = []
    type_counts: dict[str, int] = defaultdict(int)
    for number, line in enumerate(lines, start=1):
        try:
            value = json.loads(line)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            record = {"line": number, "raw": None, "parse_error": str(error)}
            (malformed_accepted if number <= process_count else malformed_tail).append(
                {"line": number, "error": str(error)}
            )
            rows.append(record)
            continue
        if not isinstance(value, Mapping):
            record = {"line": number, "raw": None, "parse_error": "non_object"}
            (malformed_accepted if number <= process_count else malformed_tail).append(
                {"line": number, "error": "non_object"}
            )
            rows.append(record)
            continue
        raw_value = dict(value)
        rows.append({"line": number, "raw": raw_value, "parse_error": None})
        if number <= process_count:
            labels = [_label(candidate) for candidate in _candidates(raw_value)]
            for label in labels:
                type_counts[label or "<missing>"] += 1
            if not event_has_known_label(raw_value):
                unknown_accepted.append(number)

    if malformed_accepted:
        diagnostics.issue(
            "malformed_accepted_event",
            "run.event_stream.malformed_accepted",
            f"{len(malformed_accepted)} accepted lines are not valid event objects",
            raw_event_lines=(item["line"] for item in malformed_accepted),
        )
    if unknown_accepted:
        diagnostics.issue(
            "unknown_accepted_event",
            "run.event_stream.unknown_accepted_lines",
            f"{len(unknown_accepted)} accepted lines have no v4-known event type",
            raw_event_lines=unknown_accepted,
        )

    actual_sha = sha256_bytes(raw)
    normalized_reference = deep_get(
        normalized,
        ("official_opencode", "raw_reference"),
        {},
    )
    reference_sha = deep_get(normalized_reference, ("sha256",), None)
    reference_lines = deep_get(normalized_reference, ("line_count",), None)
    sha_match = reference_sha == actual_sha if isinstance(reference_sha, str) else None
    line_count_match = reference_lines == len(lines) if is_nonnegative_integer(reference_lines) else None
    if sha_match is not True:
        diagnostics.issue(
            "normalized_raw_hash_mismatch",
            "run.event_stream.normalized_raw_sha_match",
            "normalized raw reference hash is missing or differs from events.jsonl",
        )
    if line_count_match is not True:
        diagnostics.issue(
            "normalized_raw_line_count_mismatch",
            "run.event_stream.normalized_line_count_match",
            "normalized raw reference line count is missing or differs from events.jsonl",
        )

    raw_copy = deep_get(normalized, ("official_opencode", "raw_jsonl"), None)
    raw_copy_match: bool | None = None
    if isinstance(raw_copy, list):
        raw_copy_match = len(raw_copy) == len(rows)
        if raw_copy_match:
            for row, copied in zip(rows, raw_copy):
                if not isinstance(copied, Mapping) or copied.get("line") != row["line"]:
                    raw_copy_match = False
                    break
                if row["raw"] is not None and copied.get("raw") != row["raw"]:
                    raw_copy_match = False
                    break
                if row["raw"] is None and "parse_error" not in copied:
                    raw_copy_match = False
                    break
    if raw_copy_match is not True:
        diagnostics.issue(
            "normalized_raw_copy_mismatch",
            "run.event_stream.normalized_raw_copy_match",
            "normalized raw_jsonl is missing or differs from the captured line records",
        )

    component_errors = deep_get(normalized, ("component_errors",), None)
    if not isinstance(component_errors, list) or component_errors:
        diagnostics.issue(
            "normalized_component_error",
            "run.event_stream.normalized_component_errors",
            "normalized component_errors is missing, invalid, or non-empty",
        )

    accepted_rows = [row for row in rows[:process_count] if row["raw"] is not None]
    event_stream = {
        "source": "raw/events.jsonl",
        "actual_file_bytes": len(raw),
        "actual_line_count": len(lines),
        "actual_sha256": actual_sha,
        "reducer_lines_accepted": accepted if accepted_valid else None,
        "accepted_lines_available": process_count,
        "tail_lines_excluded": max(0, len(lines) - process_count),
        "reducer_input_sealed": reducer_sealed if isinstance(reducer_sealed, bool) else None,
        "boundary_valid": accepted_valid and accepted <= len(lines) and not (
            len(lines) > accepted and reducer_sealed is not True
        ),
        "normalized_raw_sha_match": sha_match,
        "normalized_line_count_match": line_count_match,
        "normalized_raw_copy_match": raw_copy_match,
        "malformed_accepted": malformed_accepted,
        "malformed_tail": malformed_tail,
        "unknown_accepted_lines": unknown_accepted,
        "accepted_event_type_counts": dict(sorted(type_counts.items())),
    }
    return accepted_rows, event_stream, raw


def normalized_replay(
    normalized: Any,
    wrapper: Any,
    terminal_behavior: str | None,
    diagnostics: Diagnostics,
) -> dict[str, Any]:
    replay = deep_get(normalized, ("official_opencode", "replay"), None)
    if not isinstance(replay, Mapping):
        diagnostics.missing(
            "run.lifecycle.normalized_replay",
            "normalized accepted replay is absent",
            critical=True,
        )
        return {}
    replay = dict(replay)
    fields = (
        "completed_model_steps",
        "completed_tool_completions",
        "completed_error_tool_calls",
        "completed_model_step_ids",
        "completed_tool_call_ids",
        "completed_error_tool_call_ids",
        "final_assistant_text",
        "final_model_step_reason",
        "protocol_failures",
    )
    for key in fields:
        wrapper_value = deep_get(wrapper, (key,), MISSING)
        replay_value = replay.get(key, MISSING)
        terminal_optional = terminal_behavior != "stop" and key in {
            "final_assistant_text",
            "final_model_step_reason",
        }
        both_expected_absent = False
        if terminal_optional and key == "final_assistant_text":
            both_expected_absent = all(
                value is MISSING or value is None or value == ""
                for value in (wrapper_value, replay_value)
            )
        elif terminal_optional:
            both_expected_absent = all(
                value is MISSING or value is None
                for value in (wrapper_value, replay_value)
            )
        if both_expected_absent:
            continue
        if wrapper_value is MISSING or replay_value is MISSING or wrapper_value != replay_value:
            diagnostics.issue(
                "accepted_replay_wrapper_mismatch",
                f"run.lifecycle.normalized_replay.{key}",
                "normalized accepted replay and wrapper observation differ or a field is missing",
            )
    return replay


def capture_metrics(
    events_path: Path,
    stderr_path: Path,
    wrapper: Any,
    diagnostics: Diagnostics,
) -> dict[str, Any]:
    actual_stdout = events_path.stat().st_size if events_path.is_file() else None
    actual_stderr = stderr_path.stat().st_size if stderr_path.is_file() else None
    output_bytes = deep_get(wrapper, ("output_bytes",), {})
    limit_total = deep_get(output_bytes, ("limit_total",), None)
    output_limit = deep_get(wrapper, ("limits", "output_limit"), None)
    streams: dict[str, dict[str, Any]] = {}
    all_valid = True
    for stream, actual in (("stdout", actual_stdout), ("stderr", actual_stderr)):
        observed = deep_get(output_bytes, ("observed", stream), None)
        kept = deep_get(output_bytes, ("kept", stream), None)
        dropped = deep_get(output_bytes, ("dropped", stream), None)
        values_valid = all(is_nonnegative_integer(value) for value in (observed, kept, dropped))
        equation_valid = values_valid and observed == kept + dropped
        file_match = values_valid and actual is not None and kept == actual
        if not values_valid:
            diagnostics.issue(
                "invalid_capture_counter",
                f"cost.capture_integrity.{stream}",
                "observed/kept/dropped must be nonnegative integers",
            )
        if values_valid and not equation_valid:
            diagnostics.issue(
                "capture_counter_mismatch",
                f"cost.capture_integrity.{stream}",
                "observed must equal kept plus dropped",
            )
        if not file_match:
            diagnostics.issue(
                "capture_file_byte_mismatch",
                f"cost.capture_integrity.{stream}",
                "wrapper kept bytes do not equal captured file size",
            )
        all_valid = all_valid and values_valid and equation_valid and file_match
        streams[stream] = {
            "actual_file_bytes": actual,
            "observed_bytes": observed if is_nonnegative_integer(observed) else None,
            "kept_bytes": kept if is_nonnegative_integer(kept) else None,
            "dropped_bytes": dropped if is_nonnegative_integer(dropped) else None,
            "observed_equals_kept_plus_dropped": equation_valid,
            "kept_equals_file_bytes": file_match,
        }
    total_kept = None
    if all(is_nonnegative_integer(streams[name]["kept_bytes"]) for name in ("stdout", "stderr")):
        total_kept = streams["stdout"]["kept_bytes"] + streams["stderr"]["kept_bytes"]
    limit_valid = is_nonnegative_integer(limit_total)
    within_limit = limit_valid and total_kept is not None and total_kept <= limit_total
    if not within_limit:
        diagnostics.issue(
            "capture_limit_mismatch",
            "cost.capture_integrity.limit_total_bytes",
            "kept byte total exceeds or cannot be checked against limit_total",
        )
    dropped_total = None
    if all(is_nonnegative_integer(streams[name]["dropped_bytes"]) for name in ("stdout", "stderr")):
        dropped_total = streams["stdout"]["dropped_bytes"] + streams["stderr"]["dropped_bytes"]
    truncation_consistent = (
        isinstance(output_limit, bool)
        and dropped_total is not None
        and output_limit == (dropped_total > 0)
    )
    if not truncation_consistent:
        diagnostics.issue(
            "output_limit_truncation_mismatch",
            "cost.capture_integrity.output_limit",
            "output_limit must be true exactly when captured bytes were dropped",
        )
    complete = all_valid and within_limit and truncation_consistent
    return {
        "status": "verified" if complete else "invalid",
        "coverage_complete": complete,
        "stdout": streams["stdout"],
        "stderr": streams["stderr"],
        "limit_total_bytes": limit_total if limit_valid else None,
        "total_kept_bytes": total_kept,
        "total_dropped_bytes": dropped_total,
        "output_limit": output_limit if isinstance(output_limit, bool) else None,
        "truncated": (dropped_total > 0) if dropped_total is not None else None,
        "truncation_consistent": truncation_consistent,
    }


def internal_file_symlink_target(run_dir: Path, path: Path) -> str | None:
    try:
        target = os.readlink(path)
    except OSError:
        return None
    target_path = Path(target)
    if target_path.is_absolute():
        return None
    try:
        resolved_target = path.resolve(strict=True)
        resolved_target.relative_to(run_dir.resolve())
    except (FileNotFoundError, OSError, RuntimeError, ValueError):
        return None
    if not resolved_target.is_file():
        return None
    return target


def verify_artifact_seal(
    run_dir: Path,
    manifest_path: Path | None,
    artifact_manifest: Any,
    diagnostics: Diagnostics,
) -> dict[str, Any]:
    issues: list[str] = []
    manifest_sha = sha256_file(manifest_path) if manifest_path and manifest_path.is_file() else None
    artifacts = deep_get(artifact_manifest, ("artifacts",), None)
    if deep_get(artifact_manifest, ("schema_version",), None) != 1 or not isinstance(artifacts, Mapping):
        issues.append("invalid_manifest_shape")
        artifacts = {}
    actual_files: set[str] = set()
    symlink_paths: list[str] = []
    unsafe_symlink_paths: list[str] = []
    nonregular_paths: list[str] = []
    root_manifest_path = run_dir / "artifact-manifest.json"
    if run_dir.is_dir():
        for path in run_dir.rglob("*"):
            relative = str(path.relative_to(run_dir))
            if path.is_symlink():
                symlink_paths.append(relative)
                if path == root_manifest_path or internal_file_symlink_target(run_dir, path) is None:
                    unsafe_symlink_paths.append(relative)
                else:
                    actual_files.add(relative)
            elif path.is_dir():
                continue
            elif path.is_file() and path != root_manifest_path:
                actual_files.add(relative)
            elif path != root_manifest_path:
                nonregular_paths.append(relative)
    expected_files = set(artifacts)
    if actual_files != expected_files:
        issues.append("artifact_file_set_mismatch")
    checked = 0
    for relative, evidence in artifacts.items():
        relative_path = Path(relative)
        if relative_path.is_absolute() or ".." in relative_path.parts:
            issues.append("unsafe_artifact_path")
            continue
        path = run_dir / relative_path
        if not path.is_file() or not isinstance(evidence, Mapping):
            issues.append("artifact_missing_or_invalid")
            continue
        expected_evidence: dict[str, Any] = {
            "sha256": sha256_file(path),
            "bytes": path.stat().st_size,
        }
        if path.is_symlink():
            target = internal_file_symlink_target(run_dir, path)
            if target is None:
                issues.append("unsafe_artifact_symlink")
                continue
            expected_evidence["symlink_target"] = target
        expected_sha = expected_evidence["sha256"]
        expected_bytes = expected_evidence["bytes"]
        if (
            not isinstance(expected_sha, str)
            or not HASH_PATTERN.fullmatch(expected_sha)
            or not is_nonnegative_integer(expected_bytes)
            or dict(evidence) != expected_evidence
        ):
            issues.append("artifact_content_mismatch")
            continue
        checked += 1
    writable_paths: list[str] = []
    if run_dir.exists() and not run_dir.is_symlink():
        for path in [run_dir, *run_dir.rglob("*")]:
            if not path.is_symlink() and path.stat().st_mode & 0o222:
                writable_paths.append("." if path == run_dir else str(path.relative_to(run_dir)))
    if writable_paths:
        issues.append("sealed_tree_writable")
    if unsafe_symlink_paths:
        issues.append("unsafe_artifact_symlink")
    if nonregular_paths:
        issues.append("nonregular_artifact_entry")
    unique_issues = sorted(set(issues))
    for issue in unique_issues:
        diagnostics.issue(
            "artifact_seal_invalid",
            "run.lifecycle.artifact_seal",
            issue,
        )
    verified = not unique_issues and manifest_sha is not None
    return {
        "manifest_present": manifest_path is not None and manifest_path.is_file(),
        "manifest_sha256": manifest_sha,
        "verified": verified,
        "artifact_count": len(artifacts),
        "checked_artifact_count": checked,
        "issues": unique_issues,
        "writable_paths": sorted(writable_paths),
        "symlink_paths": sorted(symlink_paths),
    }


def tool_family(tool_name: str | None) -> str:
    lowered = (tool_name or "unknown").casefold().replace("-", "_").replace(".", "_")
    if "initial_instructions" in lowered:
        return "handshake"
    if lowered == "invalid" or lowered.endswith("_invalid"):
        return "invalid"
    for family in ("overview", "search", "grep", "read", "find", "glob"):
        if lowered == family or lowered.endswith("_" + family):
            return family
    if lowered in {"write", "edit", "patch", "apply_patch"} or lowered.endswith(("_write", "_edit")):
        return "mutation"
    return "other"


def full_identity_list(
    value: Any,
    kind: str,
    diagnostics: Diagnostics,
    field_path: str,
) -> list[str] | None:
    pattern = FULL_TOOL_ID_PATTERN if kind == "tool" else FULL_MODEL_ID_PATTERN
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        diagnostics.issue(
            "invalid_authoritative_identity_list",
            field_path,
            "authoritative identity list is missing or is not a string list",
        )
        return None
    if any(not pattern.fullmatch(item) for item in value):
        diagnostics.issue(
            "bare_or_invalid_identity",
            field_path,
            f"every {kind} identity must be fully session scoped",
            identities=(item for item in value if not pattern.fullmatch(item)),
        )
        return None
    if len(set(value)) != len(value):
        diagnostics.issue(
            "duplicate_authoritative_identity",
            field_path,
            "authoritative identity list contains duplicates",
        )
        return None
    return list(value)


def _revision_field_entries(
    revisions: Sequence[Mapping[str, Any]],
    key: str,
) -> list[tuple[int, Any]]:
    return [
        (revision["raw_event_line"], revision[key])
        for revision in revisions
        if revision.get(f"{key}_present", revision.get(key) is not None)
        and revision.get(key) is not None
    ]


def _time_field_entries(
    revisions: Sequence[Mapping[str, Any]],
    key: str,
) -> list[tuple[int, Any]]:
    return [
        (revision["raw_event_line"], revision["time"][key])
        for revision in revisions
        if isinstance(revision.get("time"), Mapping)
        and key in revision["time"]
        and revision["time"][key] is not None
    ]


def _entry_values(entries: Sequence[tuple[int, Any]]) -> list[Any]:
    return canonical_distinct(value for _, value in entries)


def normalize_one_tool_call(
    identity: str,
    revisions: Sequence[dict[str, Any]],
    *,
    completed: bool,
    error_identities: set[str],
    diagnostics: Diagnostics,
) -> dict[str, Any]:
    ordered = sorted(revisions, key=lambda revision: revision["raw_event_line"])
    terminals = [revision for revision in ordered if revision["is_terminal"]]
    conflicts: list[str] = []

    tool_entries = [
        (revision["raw_event_line"], revision["tool"])
        for revision in ordered
        if revision.get("tool") is not None
    ]
    input_entries = _revision_field_entries(ordered, "input")
    output_entries = _revision_field_entries(terminals, "output")
    error_entries = _revision_field_entries(terminals, "error")
    status_entries = [
        (revision["raw_event_line"], revision["status"])
        for revision in terminals
        if revision.get("status") is not None
    ]
    start_entries = _time_field_entries(ordered, "start")
    end_entries = _time_field_entries(ordered, "end")
    tool_values = _entry_values(tool_entries)
    input_values = _entry_values(input_entries)
    terminal_output_values = _entry_values(output_entries)
    error_values = _entry_values(error_entries)
    terminal_statuses = _entry_values(status_entries)
    start_values = _entry_values(start_entries)
    end_values = _entry_values(end_entries)
    if len(tool_values) > 1:
        conflicts.append("tool_conflict")
    if len(input_values) > 1:
        conflicts.append("input_conflict")
    if len(terminal_output_values) > 1:
        conflicts.append("output_conflict")
    if len(terminal_statuses) > 1:
        conflicts.append("terminal_status_conflict")
    if len(error_values) > 1:
        conflicts.append("error_conflict")
    if len(start_values) > 1:
        conflicts.append("time_start_conflict")
    if len(end_values) > 1:
        conflicts.append("time_end_conflict")

    if conflicts:
        diagnostics.issue(
            "tool_identity_conflict",
            "run.completed_tool_calls" if completed else "run.incomplete_tool_calls",
            ",".join(sorted(conflicts)),
            raw_event_lines=(revision["raw_event_line"] for revision in ordered),
            identities=[identity],
        )

    selected = terminals[-1] if terminals else ordered[-1]
    session_id, call_id = identity.split(":tool:", 1)
    tool_name = tool_values[0] if len(tool_values) == 1 and isinstance(tool_values[0], str) else None
    status = terminal_statuses[0] if len(terminal_statuses) == 1 else None
    merged_input = input_values[0] if len(input_values) == 1 else None
    merged_output = terminal_output_values[0] if len(terminal_output_values) == 1 else None
    merged_error = error_values[0] if len(error_values) == 1 else None
    input_present = any(revision.get("input_present") is True for revision in ordered)
    output_present = any(revision.get("output_present") is True for revision in terminals)
    is_error = (
        identity in error_identities
        or status in TOOL_ERRORS
        or bool(error_values)
        or tool_name == "invalid"
    )
    explicit_error = len(error_values) == 1
    input_missing = completed and "input_conflict" not in conflicts and merged_input is None
    output_missing = completed and "output_conflict" not in conflicts and merged_output is None
    if input_missing:
        diagnostics.missing(
            f"run.completed_tool_calls[{identity}].input",
            "completed call input is missing or null",
            critical=True,
        )
    if output_missing:
        diagnostics.missing(
            f"run.completed_tool_calls[{identity}].output",
            (
                "explicit error terminal has no output payload; output bytes are null"
                if explicit_error
                else "completed call output is missing or null"
            ),
            critical=not explicit_error,
        )

    input_encoded = (
        payload_bytes(merged_input)
        if merged_input is not None and "input_conflict" not in conflicts
        else None
    )
    output_encoded = (
        payload_bytes(merged_output)
        if merged_output is not None and "output_conflict" not in conflicts
        else None
    )
    start = start_values[0] if len(start_values) == 1 else None
    end = end_values[0] if len(end_values) == 1 else None
    timing_valid = is_number(start) and is_number(end) and end >= start
    elapsed = end - start if timing_valid and not any(item.startswith("time_") for item in conflicts) else None
    if completed and elapsed is None and not any(item.startswith("time_") for item in conflicts):
        diagnostics.missing(
            f"run.completed_tool_calls[{identity}].client_observed_elapsed_ms",
            "completed call has no valid nonconflicting state.time start/end",
            critical=True,
        )

    error_value = merged_error
    if error_value is not None and not isinstance(error_value, str):
        error_value = json.dumps(error_value, ensure_ascii=False, sort_keys=True)
    return {
        "identity": identity,
        "session_id": session_id,
        "call_id": call_id,
        "tool": tool_name,
        "family": tool_family(tool_name),
        "status": status,
        "completed": completed,
        "is_error": is_error,
        "error": error_value if isinstance(error_value, str) else None,
        "raw_event_lines": [revision["raw_event_line"] for revision in ordered],
        "selected_completion_line": selected["raw_event_line"] if terminals else None,
        "revision_count": len(ordered),
        "conflicts": sorted(conflicts),
        "field_provenance": {
            "tool": [line for line, _ in tool_entries],
            "status": [line for line, _ in status_entries],
            "input": [line for line, _ in input_entries],
            "output": [line for line, _ in output_entries],
            "error": [line for line, _ in error_entries],
            "time_start": [line for line, _ in start_entries],
            "time_end": [line for line, _ in end_entries],
        },
        "input_present": input_present,
        "input_null": input_present and not input_entries,
        "output_present": output_present,
        "output_null": output_present and not output_entries,
        "input_utf8_bytes": len(input_encoded) if input_encoded is not None else None,
        "output_utf8_bytes": len(output_encoded) if output_encoded is not None else None,
        "input_sha256": sha256_bytes(input_encoded) if input_encoded is not None else None,
        "output_sha256": sha256_bytes(output_encoded) if output_encoded is not None else None,
        "client_observed_start_ms": start if is_number(start) else None,
        "client_observed_end_ms": end if is_number(end) else None,
        "client_observed_elapsed_ms": elapsed,
        "_input": merged_input if merged_input is not None and "input_conflict" not in conflicts else MISSING,
    }


def normalize_tool_calls(
    accepted_rows: Sequence[Mapping[str, Any]],
    replay: Mapping[str, Any],
    wrapper: Any,
    diagnostics: Diagnostics,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    malformed_revisions = 0
    for row in accepted_rows:
        revision = tool_revision(row["raw"], row["line"])
        if revision is None:
            continue
        identity = revision.get("identity")
        if not isinstance(identity, str) or not FULL_TOOL_ID_PATTERN.fullmatch(identity):
            malformed_revisions += 1
            diagnostics.issue(
                "malformed_tool_identity",
                "run.incomplete_tool_calls",
                "tool event lacks a full session:tool:callID identity",
                raw_event_lines=[row["line"]],
            )
            continue
        grouped[identity].append(revision)

    completed_source = replay.get(
        "completed_tool_call_ids",
        deep_get(wrapper, ("completed_tool_call_ids",), None),
    )
    error_source = replay.get(
        "completed_error_tool_call_ids",
        deep_get(wrapper, ("completed_error_tool_call_ids",), None),
    )
    completed_ids = full_identity_list(
        completed_source,
        "tool",
        diagnostics,
        "run.lifecycle.normalized_replay.completed_tool_call_ids",
    )
    error_ids = full_identity_list(
        error_source,
        "tool",
        diagnostics,
        "run.lifecycle.normalized_replay.completed_error_tool_call_ids",
    )
    authoritative = set(completed_ids or [])
    error_identities = set(error_ids or [])
    raw_terminal = {
        identity
        for identity, revisions in grouped.items()
        if any(revision["is_terminal"] for revision in revisions)
    }
    if completed_ids is not None and raw_terminal != authoritative:
        diagnostics.issue(
            "tool_replay_identity_mismatch",
            "run.completed_tool_calls",
            "raw terminal tool identities differ from normalized accepted replay",
            identities=raw_terminal.symmetric_difference(authoritative),
        )

    completed_calls: list[dict[str, Any]] = []
    incomplete_calls: list[dict[str, Any]] = []
    for identity in sorted(grouped, key=lambda item: min(r["raw_event_line"] for r in grouped[item])):
        is_completed = identity in authoritative and identity in raw_terminal
        call = normalize_one_tool_call(
            identity,
            grouped[identity],
            completed=is_completed,
            error_identities=error_identities,
            diagnostics=diagnostics,
        )
        (completed_calls if is_completed else incomplete_calls).append(call)
    for identity in sorted(authoritative - set(grouped)):
        diagnostics.issue(
            "authoritative_tool_without_raw_event",
            "run.incomplete_tool_calls",
            "accepted replay reports a completed tool identity absent from raw events",
            identities=[identity],
        )

    completed_calls.sort(key=lambda call: call["selected_completion_line"] or 0)
    for index, call in enumerate(completed_calls, start=1):
        call["completed_call_index"] = index
    for call in incomplete_calls:
        call["completed_call_index"] = None

    incomplete_wrapper_count = deep_get(replay, ("partial_events",), deep_get(wrapper, ("partial_events",), None))
    if incomplete_calls and incomplete_wrapper_count == 0:
        diagnostics.issue(
            "unreported_incomplete_tool_call",
            "run.incomplete_tool_calls",
            "raw accepted events contain incomplete calls but replay partial_events is zero",
            identities=(call["identity"] for call in incomplete_calls),
        )
    integrity = {
        "authoritative_completed_ids_available": completed_ids is not None,
        "authoritative_error_ids_available": error_ids is not None,
        "raw_terminal_identity_match": completed_ids is not None and raw_terminal == authoritative,
        "malformed_revision_count": malformed_revisions,
        "completed_count": len(completed_calls),
        "incomplete_count": len(incomplete_calls),
        "conflicting_call_count": sum(bool(call["conflicts"]) for call in completed_calls + incomplete_calls),
    }
    return completed_calls, incomplete_calls, integrity


def normalized_query(value: str) -> tuple[str, list[str]]:
    normalized = unicodedata.normalize("NFKC", value).casefold()
    normalized = " ".join(normalized.split())
    return normalized, re.findall(r"[^\W]+", normalized, flags=re.UNICODE)


def query_from_input(value: Any) -> tuple[str | None, str | None]:
    if not isinstance(value, Mapping):
        return None, None
    for key in QUERY_KEYS:
        candidate = value.get(key)
        if isinstance(candidate, str) and candidate.strip():
            return key, candidate
    return None, None


def scope_signature(value: Any) -> str | None:
    if not isinstance(value, Mapping):
        return None
    scope = {
        key: value[key]
        for key in sorted(value)
        if key.casefold()
        in {
            "path",
            "workspace_scope",
            "workspacescope",
            "scope",
            "include",
            "exclude",
            "file_path",
            "filepath",
        }
    }
    return sha256_bytes(canonical_json_bytes(scope))


def repeated_search_metrics(
    completed_calls: Sequence[Mapping[str, Any]],
    tool_conflict: bool,
) -> dict[str, Any]:
    if tool_conflict:
        return {
            "groups": None,
            "group_count": None,
            "extra_call_count": None,
            "needs_manual_review": True,
            "measurement_rule": "Unavailable because a completed tool identity conflicted.",
        }
    candidates: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for call in completed_calls:
        if call["family"] not in {"search", "grep", "find", "glob"}:
            continue
        input_value = call.get("_input", MISSING)
        key, query = query_from_input(input_value)
        if key is None or query is None:
            continue
        normalized, tokens = normalized_query(query)
        if not tokens:
            continue
        signature = sha256_bytes(canonical_json_bytes(sorted(tokens)))
        candidates[signature].append(
            {
                "identity": call["identity"],
                "call_id": call["call_id"],
                "completed_call_index": call["completed_call_index"],
                "tool": call["tool"],
                "family": call["family"],
                "query_key": key,
                "normalized_query": normalized,
                "ordered_token_sha256": sha256_bytes(canonical_json_bytes(tokens)),
                "scope_sha256": scope_signature(input_value),
                "raw_event_lines": call["raw_event_lines"],
                "output_utf8_bytes": call["output_utf8_bytes"],
            }
        )
    groups: list[dict[str, Any]] = []
    for signature, calls in candidates.items():
        if len(calls) < 2:
            continue
        normalized_values = {call["normalized_query"] for call in calls}
        ordered_values = {call["ordered_token_sha256"] for call in calls}
        if len(normalized_values) == 1:
            basis = "exact_normalized_query"
        elif len(ordered_values) == 1:
            basis = "same_ordered_tokens"
        else:
            basis = "same_term_multiset_different_order"
        groups.append(
            {
                "group_id": "",
                "mechanical_match_basis": basis,
                "term_multiset_sha256": signature,
                "call_count": len(calls),
                "extra_call_count": len(calls) - 1,
                "same_scope": len({call["scope_sha256"] for call in calls}) == 1,
                "calls": calls,
                "needs_manual_review": True,
            }
        )
    groups.sort(key=lambda group: min(call["completed_call_index"] for call in group["calls"]))
    for index, group in enumerate(groups, start=1):
        group["group_id"] = f"repeat-{index}"
    return {
        "groups": groups,
        "group_count": len(groups),
        "extra_call_count": sum(group["extra_call_count"] for group in groups),
        "needs_manual_review": True,
        "measurement_rule": "NFKC, casefold, whitespace collapse, Unicode word-token multiset; semantic repetition is not inferred.",
    }


def _valid_range_pair(start: Any, end: Any) -> bool:
    return (
        is_nonnegative_integer(start)
        and is_nonnegative_integer(end)
        and end >= start
    )


def read_bound(value: Any) -> tuple[bool, list[str], bool]:
    if not isinstance(value, Mapping):
        return False, [], False
    seen_keys: list[str] = []
    malformed = False
    for key in LIMIT_KEYS:
        if key in value:
            seen_keys.append(key)
            if is_positive_integer(value[key]):
                return True, sorted(seen_keys), malformed
            malformed = True
    for key in RANGE_KEYS:
        if key not in value:
            continue
        seen_keys.append(key)
        candidate = value[key]
        if isinstance(candidate, Mapping):
            for start_key, end_key in RANGE_PAIRS:
                if start_key in candidate or end_key in candidate:
                    if _valid_range_pair(candidate.get(start_key), candidate.get(end_key)):
                        return True, sorted(seen_keys), malformed
                    malformed = True
        elif isinstance(candidate, list) and len(candidate) == 2:
            if _valid_range_pair(candidate[0], candidate[1]):
                return True, sorted(seen_keys), malformed
            malformed = True
        else:
            malformed = True
    for start_key, end_key in RANGE_PAIRS:
        if start_key in value or end_key in value:
            seen_keys.extend(key for key in (start_key, end_key) if key in value)
            if _valid_range_pair(value.get(start_key), value.get(end_key)):
                return True, sorted(set(seen_keys)), malformed
            malformed = True
    return False, sorted(set(seen_keys)), malformed


def unbounded_read_metrics(
    completed_calls: Sequence[Mapping[str, Any]],
    tool_conflict: bool,
) -> dict[str, Any]:
    if tool_conflict:
        return {
            "calls": None,
            "count": None,
            "output_bytes": None,
            "measurement_rule": "Unavailable because a completed tool identity conflicted.",
        }
    calls: list[dict[str, Any]] = []
    for call in completed_calls:
        if call["family"] != "read":
            continue
        input_value = call.get("_input", MISSING)
        bounded, bound_keys, malformed = read_bound(input_value)
        if bounded:
            continue
        target = None
        if isinstance(input_value, Mapping):
            target = first_not_none(
                input_value.get("file_path"),
                input_value.get("filePath"),
                input_value.get("path"),
                input_value.get("target"),
            )
        calls.append(
            {
                "identity": call["identity"],
                "call_id": call["call_id"],
                "completed_call_index": call["completed_call_index"],
                "tool": call["tool"],
                "target": target if isinstance(target, str) else None,
                "recognized_bound_keys": bound_keys,
                "malformed_bound_present": malformed,
                "output_utf8_bytes": call["output_utf8_bytes"],
                "raw_event_lines": call["raw_event_lines"],
                "reason": "malformed_bound" if malformed else "no_positive_limit_or_valid_closed_range",
            }
        )
    output_values = [call["output_utf8_bytes"] for call in calls]
    output_bytes = (
        sum(output_values)
        if all(is_nonnegative_integer(value) for value in output_values)
        else None
    )
    return {
        "calls": calls,
        "count": len(calls),
        "output_bytes": output_bytes,
        "measurement_rule": "Only a positive integer limit or a valid closed integer range with end >= start is bounded; incomplete calls are excluded.",
    }


def public_tool_calls(calls: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    return [
        {key: value for key, value in call.items() if not key.startswith("_")}
        for call in calls
    ]


TOKEN_COMPONENT_PATHS = {
    "input": ("input",),
    "output": ("output",),
    "reasoning": ("reasoning",),
    "cache_read": ("cache", "read"),
    "cache_write": ("cache", "write"),
    "total": ("total",),
}


def token_metrics(
    accepted_rows: Sequence[Mapping[str, Any]],
    normalized: Any,
    replay: Mapping[str, Any],
    terminal_behavior: str | None,
    diagnostics: Diagnostics,
) -> tuple[dict[str, Any], int | None]:
    stop_expected = terminal_behavior == "stop"
    rows_by_line = {row["line"]: row for row in accepted_rows}
    usage = deep_get(normalized, ("official_opencode", "token_usage"), None)
    source = "normalized.json:official_opencode.token_usage"
    if not isinstance(usage, list):
        diagnostics.missing(
            "cost.tokens.per_step",
            f"official token usage is absent for terminal_behavior={terminal_behavior}",
            critical=stop_expected,
        )
        source = "raw/events.jsonl:accepted_step_finish_fallback"
        usage = []
        for row in accepted_rows:
            step = step_finish_record(row["raw"], row["line"])
            if step is not None and step["tokens_present"]:
                usage.append(
                    {
                        "step_id": step["identity"],
                        "event_line": row["line"],
                        "tokens": step["tokens"],
                        "provenance": "official-opencode-raw-step_finish",
                    }
                )

    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    identity_failures = 0
    raw_normalized_mismatch = False
    for index, entry in enumerate(usage, start=1):
        if not isinstance(entry, Mapping):
            diagnostics.issue(
                "invalid_token_entry",
                "cost.tokens.per_step",
                f"token entry {index} is not an object",
            )
            identity_failures += 1
            continue
        event_line = entry.get("event_line", entry.get("raw_event_line"))
        raw_step = None
        if is_positive_integer(event_line) and event_line in rows_by_line:
            raw_step = step_finish_record(rows_by_line[event_line]["raw"], event_line)
        provided_id = entry.get("step_id")
        identity = raw_step.get("identity") if isinstance(raw_step, Mapping) else None
        if identity is None and isinstance(provided_id, str) and FULL_MODEL_ID_PATTERN.fullmatch(provided_id):
            identity = provided_id
        if not isinstance(identity, str) or not FULL_MODEL_ID_PATTERN.fullmatch(identity):
            diagnostics.issue(
                "invalid_token_step_identity",
                "cost.tokens.per_step",
                "token entry cannot be linked to a full session:model:message:part identity",
                raw_event_lines=[event_line] if is_positive_integer(event_line) else [],
            )
            identity_failures += 1
            continue
        if (
            isinstance(provided_id, str)
            and FULL_MODEL_ID_PATTERN.fullmatch(provided_id)
            and provided_id != identity
        ):
            diagnostics.issue(
                "token_step_identity_mismatch",
                "cost.tokens.per_step",
                "provided full step ID differs from the accepted raw event identity",
                raw_event_lines=[event_line] if is_positive_integer(event_line) else [],
                identities=[identity, provided_id],
            )
        if not is_positive_integer(event_line) or event_line not in rows_by_line:
            diagnostics.issue(
                "token_outside_accepted_boundary",
                "cost.tokens.per_step",
                "normalized token entry lacks an accepted raw event line",
                identities=[identity],
            )
        raw_tokens_match = (
            isinstance(raw_step, Mapping)
            and raw_step.get("tokens_present") is True
            and canonical_json_bytes(raw_step.get("tokens"))
            == canonical_json_bytes(entry.get("tokens"))
        )
        if not raw_tokens_match:
            raw_normalized_mismatch = True
            diagnostics.issue(
                "token_raw_normalized_mismatch",
                "cost.tokens.per_step",
                "normalized token payload differs from the accepted raw step_finish token payload",
                raw_event_lines=[event_line] if is_positive_integer(event_line) else [],
                identities=[identity],
            )
        grouped[identity].append(
            {
                "identity": identity,
                "raw_event_line": event_line if is_positive_integer(event_line) else None,
                "provided_step_id": provided_id if isinstance(provided_id, str) else None,
                "tokens": entry.get("tokens"),
                "provenance": entry.get("provenance") if isinstance(entry.get("provenance"), str) else source,
            }
        )

    model_ids = full_identity_list(
        replay.get("completed_model_step_ids"),
        "model",
        diagnostics,
        "run.lifecycle.normalized_replay.completed_model_step_ids",
    )
    model_step_count = replay.get("completed_model_steps")
    if not is_nonnegative_integer(model_step_count):
        diagnostics.issue(
            "invalid_model_step_count",
            "cost.model_steps",
            "completed_model_steps must be a nonnegative integer",
        )
        model_step_count = None
    elif model_ids is not None and model_step_count != len(model_ids):
        diagnostics.issue(
            "model_step_count_identity_mismatch",
            "cost.model_steps",
            "completed model count differs from full identity count",
            identities=model_ids,
        )
        model_step_count = None

    model_identity_set = set(model_ids or [])
    token_identity_set = set(grouped)
    missing_token_identities = model_identity_set - token_identity_set
    extra_token_identities = token_identity_set - model_identity_set
    token_set_matches = model_ids is not None and not missing_token_identities and not extra_token_identities
    if extra_token_identities or (missing_token_identities and stop_expected) or model_ids is None:
        diagnostics.issue(
            "token_model_identity_set_mismatch",
            "cost.tokens.per_step",
            "token step identities differ from completed model step identities",
            identities=missing_token_identities | extra_token_identities,
        )
    elif missing_token_identities:
        diagnostics.missing(
            "cost.tokens.per_step",
            "completed non-stop model steps lack official token records: "
            + ",".join(sorted(missing_token_identities)),
            critical=False,
        )

    steps: list[dict[str, Any]] = []
    duplicate_conflict = False
    invalid_token_component = False
    for identity in sorted(grouped, key=lambda item: min(
        entry["raw_event_line"] or sys.maxsize for entry in grouped[item]
    )):
        entries = grouped[identity]
        token_values = canonical_distinct(entry["tokens"] for entry in entries)
        conflicts: list[str] = []
        if len(token_values) != 1:
            conflicts.append("duplicate_token_conflict")
            duplicate_conflict = True
            diagnostics.issue(
                "duplicate_token_conflict",
                "cost.tokens.per_step",
                "same full model identity has conflicting official token records",
                raw_event_lines=(
                    entry["raw_event_line"]
                    for entry in entries
                    if entry["raw_event_line"] is not None
                ),
                identities=[identity],
            )
            token_value: Mapping[str, Any] = {}
        else:
            token_value = token_values[0] if isinstance(token_values[0], Mapping) else {}
            if not isinstance(token_values[0], Mapping):
                conflicts.append("token_payload_not_object")
                diagnostics.issue(
                    "invalid_token_payload",
                    "cost.tokens.per_step",
                    "official token payload is not an object",
                    identities=[identity],
                )

        values: dict[str, int | None] = {}
        for component, path in TOKEN_COMPONENT_PATHS.items():
            value = deep_get(token_value, path, MISSING)
            if not is_nonnegative_integer(value):
                invalid_token_component = True
                values[component] = None
                reason = "missing" if value is MISSING else "not_nonnegative_integer"
                diagnostics.missing(
                    f"cost.tokens.per_step[{identity}].{component}",
                    reason,
                    critical=stop_expected if value is MISSING else True,
                )
            else:
                values[component] = value
        components = [
            values[name]
            for name in ("input", "output", "reasoning", "cache_read", "cache_write")
        ]
        component_sum = sum(components) if all(value is not None for value in components) else None
        total_matches = (
            values["total"] == component_sum
            if values["total"] is not None and component_sum is not None
            else None
        )
        if total_matches is False:
            invalid_token_component = True
            diagnostics.issue(
                "official_token_total_mismatch",
                f"cost.tokens.per_step[{identity}].total",
                "official total differs from the sum of official components",
                identities=[identity],
            )
            values["total"] = None
            conflicts.append("official_total_component_mismatch")
        if duplicate_conflict and len(token_values) != 1:
            values = {component: None for component in TOKEN_COMPONENT_PATHS}
            component_sum = None
            total_matches = None
        steps.append(
            {
                "identity": identity,
                "raw_event_line": entries[0]["raw_event_line"],
                "source": source,
                **values,
                "component_sum": component_sum,
                "official_total_matches_components": total_matches,
                "duplicate_record_count": len(entries),
                "conflicts": sorted(set(conflicts)),
            }
        )

    global_fail = (
        duplicate_conflict
        or invalid_token_component
        or identity_failures > 0
        or raw_normalized_mismatch
        or not token_set_matches
    )
    coverage_complete = bool(steps) and not global_fail
    if not steps:
        diagnostics.missing(
            "cost.tokens.per_step",
            f"no completed model token records for terminal_behavior={terminal_behavior}",
            critical=stop_expected,
        )
    totals: dict[str, int | None] = {}
    for component in TOKEN_COMPONENT_PATHS:
        values = [step[component] for step in steps]
        totals[component] = (
            sum(values)
            if coverage_complete and all(is_nonnegative_integer(value) for value in values)
            else None
        )
    return (
        {
            **totals,
            "source": source,
            "official_entries": len(steps),
            "per_step": steps,
            "identity_set_matches_completed_models": token_set_matches,
            "coverage_complete": coverage_complete,
            "coverage_reason": (
                None
                if coverage_complete
                else (
                    f"no_completed_model_token_records_for_{terminal_behavior or 'unknown'}"
                    if not steps
                    else "token_records_incomplete_or_invalid"
                )
            ),
            "measurement_rule": "Full session:model:message:part identity; exact nonnegative integers only; no missing-value inference.",
        },
        model_step_count,
    )


def final_answer_metrics(
    accepted_rows: Sequence[Mapping[str, Any]],
    replay: Mapping[str, Any],
    wrapper: Any,
    terminal_behavior: str | None,
    diagnostics: Diagnostics,
) -> dict[str, Any]:
    stop_expected = terminal_behavior == "stop"
    final_step = None
    text_records: list[dict[str, Any]] = []
    for row in accepted_rows:
        step = step_finish_record(row["raw"], row["line"])
        if step is not None and str(step.get("reason", "")).casefold() == "stop":
            final_step = step
        text = text_record(row["raw"], row["line"])
        if text is not None:
            text_records.append(text)
    if final_step is None or final_step.get("session_id") is None or final_step.get("message_id") is None:
        diagnostics.missing(
            "dialogue.final_answer.final_message_identity",
            f"accepted events contain no full stop-step identity for terminal_behavior={terminal_behavior}",
            critical=stop_expected,
        )
        final_message_identity = None
    else:
        final_message_identity = f"{final_step['session_id']}:{final_step['message_id']}"

    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for record in text_records:
        if record.get("identity") is None:
            diagnostics.issue(
                "malformed_text_identity",
                "dialogue.final_answer.parts",
                "text event lacks full session:text:message:part identity",
                raw_event_lines=[record["raw_event_line"]],
            )
            continue
        grouped[record["identity"]].append(record)

    parts: list[dict[str, Any]] = []
    for identity, records in grouped.items():
        if final_message_identity is None or not identity.startswith(
            f"{final_step['session_id']}:text:{final_step['message_id']}:"
        ):
            continue
        values = canonical_distinct(record["text"] for record in records)
        if len(values) != 1 or not isinstance(values[0], str):
            diagnostics.issue(
                "text_identity_conflict",
                "dialogue.final_answer.parts",
                "same full text identity has conflicting or non-string text",
                raw_event_lines=(record["raw_event_line"] for record in records),
                identities=[identity],
            )
            continue
        encoded = values[0].encode("utf-8")
        parts.append(
            {
                "identity": identity,
                "raw_event_line": min(record["raw_event_line"] for record in records),
                "text": values[0],
                "utf8_bytes": len(encoded),
                "sha256": sha256_bytes(encoded),
            }
        )
    parts.sort(key=lambda part: part["raw_event_line"])
    raw_text = "".join(part["text"] for part in parts)
    replay_text = replay.get("final_assistant_text")
    wrapper_text = deep_get(wrapper, ("final_assistant_text",), None)
    if not isinstance(replay_text, str):
        diagnostics.missing(
            "dialogue.final_answer.text",
            f"normalized replay final_assistant_text is absent for terminal_behavior={terminal_behavior}",
            critical=stop_expected,
        )
        replay_text = None
    if not isinstance(wrapper_text, str):
        diagnostics.missing(
            "dialogue.final_answer.wrapper_text",
            f"wrapper final_assistant_text is absent for terminal_behavior={terminal_behavior}",
            critical=stop_expected,
        )
        wrapper_text = None
    if stop_expected and replay_text is not None and replay_text != raw_text:
        diagnostics.issue(
            "final_answer_raw_replay_mismatch",
            "dialogue.final_answer.text",
            "concatenated accepted final-message text parts differ from normalized replay",
        )
    if stop_expected and replay_text is not None and wrapper_text is not None and replay_text != wrapper_text:
        diagnostics.issue(
            "final_answer_replay_wrapper_mismatch",
            "dialogue.final_answer.text",
            "normalized replay and wrapper final answer differ",
        )
    raw_matches = replay_text == raw_text if replay_text is not None else None
    wrapper_matches = (
        wrapper_text == replay_text
        if replay_text is not None and wrapper_text is not None
        else None
    )
    coverage_complete = (
        stop_expected
        and final_message_identity is not None
        and raw_matches is True
        and wrapper_matches is True
    )
    text = replay_text if coverage_complete else None
    encoded = text.encode("utf-8") if text is not None else None
    return {
        "final_message_identity": final_message_identity,
        "source": "normalized.json:official_opencode.replay.final_assistant_text",
        "parts": parts,
        "part_count": len(parts),
        "text": text,
        "utf8_bytes": len(encoded) if encoded is not None else None,
        "sha256": sha256_bytes(encoded) if encoded is not None else None,
        "raw_parts_match_replay": raw_matches,
        "wrapper_matches_replay": wrapper_matches,
        "coverage_complete": coverage_complete,
        "coverage_reason": (
            None
            if coverage_complete
            else f"no_verified_final_answer_for_{terminal_behavior or 'unknown'}"
        ),
    }


def normalize_classification(value: Any, diagnostics: Diagnostics) -> dict[str, Any]:
    measurement = deep_get(value, ("measurement_status",), None)
    terminal = deep_get(value, ("terminal_behavior",), None)
    replacement_allowed = deep_get(value, ("replacement_allowed",), None)
    replacement_category = deep_get(value, ("replacement_category",), None)
    generation_invalid = deep_get(value, ("generation_invalid",), None)
    reason = deep_get(value, ("reason",), None)
    if measurement not in {"valid", "infrastructure_invalid"}:
        diagnostics.issue(
            "invalid_attempt_classification",
            "run.lifecycle.classification.measurement_status",
            "measurement_status must be valid or infrastructure_invalid",
        )
        measurement = None
    allowed_terminal = {
        "stop",
        "timeout",
        "model_step_limit",
        "output_limit",
        "process_error",
        "protocol_error",
        "infrastructure_error",
        "unknown",
    }
    if terminal not in allowed_terminal:
        diagnostics.issue(
            "invalid_attempt_classification",
            "run.lifecycle.classification.terminal_behavior",
            "terminal_behavior is missing or outside the fixed classification values",
        )
        terminal = None
    if not isinstance(replacement_allowed, bool):
        diagnostics.issue(
            "invalid_attempt_classification",
            "run.lifecycle.classification.replacement_allowed",
            "replacement_allowed must be boolean",
        )
        replacement_allowed = None
    if replacement_category is not None and not isinstance(replacement_category, str):
        diagnostics.issue(
            "invalid_attempt_classification",
            "run.lifecycle.classification.replacement_category",
            "replacement_category must be string or null",
        )
        replacement_category = None
    if not isinstance(generation_invalid, bool):
        diagnostics.issue(
            "invalid_attempt_classification",
            "run.lifecycle.classification.generation_invalid",
            "generation_invalid must be boolean",
        )
        generation_invalid = None
    if not isinstance(reason, str):
        reason = None
    return {
        "measurement_status": measurement,
        "terminal_behavior": terminal,
        "replacement_allowed": replacement_allowed,
        "replacement_category": replacement_category,
        "generation_invalid": generation_invalid,
        "reason": reason,
    }


def normalize_postprocess(value: Any) -> dict[str, Any]:
    required = deep_get(value, ("required_files",), None)
    return {
        "supervisor_status": deep_get(value, ("supervisor_status",), None)
        if isinstance(deep_get(value, ("supervisor_status",), None), int)
        and not isinstance(deep_get(value, ("supervisor_status",), None), bool)
        else None,
        "parser_status": deep_get(value, ("parser_status",), None)
        if isinstance(deep_get(value, ("parser_status",), None), int)
        and not isinstance(deep_get(value, ("parser_status",), None), bool)
        else None,
        "parser_ok": deep_get(value, ("parser_ok",), None)
        if isinstance(deep_get(value, ("parser_ok",), None), bool)
        else None,
        "required_files": required
        if isinstance(required, list) and all(isinstance(item, str) for item in required)
        else None,
        "required_files_present": deep_get(value, ("required_files_present",), None)
        if isinstance(deep_get(value, ("required_files_present",), None), bool)
        else None,
    }


def normalize_auth_cleanup(value: Any) -> dict[str, Any]:
    status_value = deep_get(value, ("status",), None)
    exit_trap = deep_get(value, ("exit_trap",), None)
    return {
        "status": status_value
        if isinstance(status_value, int) and not isinstance(status_value, bool)
        else None,
        "exit_trap": exit_trap if isinstance(exit_trap, bool) else None,
    }


def normalize_invariants(value: Any) -> dict[str, Any]:
    def boolean(key: str) -> bool | None:
        candidate = deep_get(value, (key,), None)
        return candidate if isinstance(candidate, bool) else None

    def string(key: str) -> str | None:
        candidate = deep_get(value, (key,), None)
        return candidate if isinstance(candidate, str) else None

    return {
        "generation_current": boolean("generation_current"),
        "generation_error": string("generation_error"),
        "source_before_sha256": string("source_before_sha256"),
        "source_after_sha256": string("source_after_sha256"),
        "source_unchanged": boolean("source_unchanged"),
        "index_unchanged": boolean("index_unchanged"),
    }


def publication_linkage(
    run_dir: Path,
    manifest: Any,
    classification_raw: Any,
    classification: Mapping[str, Any],
    artifact_manifest_path: Path | None,
    ledger: Any,
    diagnostics: Diagnostics,
) -> dict[str, Any]:
    run_id = deep_get(manifest, ("run_id",), None)
    generation_id = deep_get(manifest, ("generation_id",), None)
    attempt_number = deep_get(manifest, ("attempt_number",), None)
    schedule = deep_get(manifest, ("schedule",), {})
    task_id = deep_get(schedule, ("task_id",), None)
    trial_id = deep_get(schedule, ("trial_id",), None)
    arm = deep_get(schedule, ("criterion",), deep_get(schedule, ("arm",), None))
    slot_key = f"{task_id}:{trial_id}:{arm}" if all(
        isinstance(value, str) and value for value in (task_id, trial_id, arm)
    ) else None
    ledger_present = isinstance(ledger, Mapping)
    generation_match = ledger_present and ledger.get("generation_id") == generation_id
    attempt = deep_get(ledger, ("attempts", str(run_id)), {}) if ledger_present else {}
    slot = deep_get(ledger, ("slots", str(slot_key)), {}) if ledger_present and slot_key else {}
    attempt_present = isinstance(attempt, Mapping) and bool(attempt)
    slot_present = isinstance(slot, Mapping) and bool(slot)
    manifest_sha = (
        sha256_file(artifact_manifest_path)
        if artifact_manifest_path is not None and artifact_manifest_path.is_file()
        else None
    )
    attempt_state_terminal = attempt_present and attempt.get("state") == "terminal"
    published_at = attempt.get("published_at_ns") if attempt_present else None
    published_at_valid = is_positive_integer(published_at)
    run_dir_match = attempt_present and isinstance(attempt.get("run_dir"), str) and Path(
        attempt["run_dir"]
    ).resolve() == run_dir
    artifact_hash_match = attempt_present and attempt.get("artifact_manifest_sha256") == manifest_sha
    classification_match = attempt_present and attempt.get("classification") == classification_raw
    attempt_number_match = attempt_present and attempt.get("attempt_number") == attempt_number
    latest_run_match = slot_present and slot.get("latest_run_id") == run_id
    latest_number_match = slot_present and slot.get("latest_attempt_number") == attempt_number
    slot_status_match = slot_present and slot.get("measurement_status") == classification.get(
        "measurement_status"
    )
    all_run_ids = slot.get("all_run_ids") if slot_present else None
    if not (isinstance(all_run_ids, list) and all(isinstance(item, str) for item in all_run_ids)):
        all_run_ids = None
    published = all(
        (
            ledger_present,
            generation_match,
            attempt_present,
            attempt_state_terminal,
            published_at_valid,
            run_dir_match,
            artifact_hash_match,
            classification_match,
            attempt_number_match,
        )
    )
    latest = all(
        (
            published,
            slot_present,
            latest_run_match,
            latest_number_match,
            slot_status_match,
            all_run_ids is not None,
            run_id in (all_run_ids or []),
        )
    )
    if not latest:
        diagnostics.issue(
            "latest_publication_linkage_invalid",
            "run.lifecycle.publication",
            "run is not verified as the latest published attempt in its sealed generation ledger",
            identities=[str(run_id)] if run_id is not None else [],
        )
    return {
        "ledger_present": ledger_present,
        "generation_match": generation_match,
        "slot_key": slot_key,
        "attempt_present": attempt_present,
        "attempt_state_terminal": attempt_state_terminal,
        "published_at_ns": published_at if is_nonnegative_integer(published_at) else None,
        "run_dir_match": run_dir_match,
        "artifact_manifest_hash_match": artifact_hash_match,
        "classification_match": classification_match,
        "attempt_number_match": attempt_number_match,
        "slot_present": slot_present,
        "latest_run_id_match": latest_run_match,
        "latest_attempt_number_match": latest_number_match,
        "slot_measurement_status_match": slot_status_match,
        "all_run_ids": all_run_ids,
        "published": published,
        "latest_published_attempt": latest,
    }


def _hash_field(
    value: Any,
    field_path: str,
    diagnostics: Diagnostics,
    *,
    critical: bool,
) -> str | None:
    if isinstance(value, str) and HASH_PATTERN.fullmatch(value):
        return value
    diagnostics.missing(field_path, "recorded sha256 is absent or invalid", critical=critical)
    return None


def _command_model(wrapper: Any) -> str | None:
    command = deep_get(wrapper, ("command",), None)
    if not isinstance(command, list) or not all(isinstance(item, str) for item in command):
        return None
    try:
        index = command.index("--model")
    except ValueError:
        return None
    return command[index + 1] if index + 1 < len(command) else None


def _terminal_observation(wrapper: Any) -> dict[str, Any]:
    limits = deep_get(wrapper, ("limits",), {})
    protocol = deep_get(wrapper, ("protocol_failures",), None)
    return {
        "exit_code": deep_get(wrapper, ("exit_code",), None)
        if isinstance(deep_get(wrapper, ("exit_code",), None), int)
        and not isinstance(deep_get(wrapper, ("exit_code",), None), bool)
        else None,
        "terminal_contract_satisfied": deep_get(wrapper, ("terminal_contract_satisfied",), None)
        if isinstance(deep_get(wrapper, ("terminal_contract_satisfied",), None), bool)
        else None,
        "final_model_step_reason": deep_get(wrapper, ("final_model_step_reason",), None)
        if isinstance(deep_get(wrapper, ("final_model_step_reason",), None), str)
        else None,
        "termination_cause": deep_get(wrapper, ("termination_cause",), None)
        if isinstance(deep_get(wrapper, ("termination_cause",), None), str)
        else None,
        "cleanup_satisfied": deep_get(wrapper, ("cleanup_satisfied",), None)
        if isinstance(deep_get(wrapper, ("cleanup_satisfied",), None), bool)
        else None,
        "remaining_process_group": deep_get(wrapper, ("remaining_process_group",), None)
        if isinstance(deep_get(wrapper, ("remaining_process_group",), None), bool)
        else None,
        "protocol_failures": protocol
        if isinstance(protocol, list) and all(isinstance(item, str) for item in protocol)
        else None,
        "limits": {
            "timeout": limits.get("timeout") if isinstance(limits.get("timeout"), bool) else None,
            "output_limit": limits.get("output_limit")
            if isinstance(limits.get("output_limit"), bool)
            else None,
            "turn_limit": limits.get("turn_limit")
            if isinstance(limits.get("turn_limit"), bool)
            else None,
            "model_step_limit": limits.get("model_step_limit")
            if isinstance(limits.get("model_step_limit"), bool)
            else None,
            "protocol_failure": limits.get("protocol_failure")
            if isinstance(limits.get("protocol_failure"), bool)
            else None,
            "signal": limits.get("signal") if isinstance(limits.get("signal"), str) else None,
            "termination_cause": limits.get("termination_cause")
            if isinstance(limits.get("termination_cause"), str)
            else None,
        },
    }


def _finalization_record(value: Any) -> dict[str, Any]:
    def integer(key: str) -> int | None:
        candidate = deep_get(value, (key,), None)
        return candidate if isinstance(candidate, int) and not isinstance(candidate, bool) else None

    def string(key: str) -> str | None:
        candidate = deep_get(value, (key,), None)
        return candidate if isinstance(candidate, str) else None

    return {
        "original_status": integer("original_status"),
        "runner_status": integer("runner_status"),
        "finalizer_status": integer("finalizer_status"),
        "final_status": integer("final_status"),
        "reason": string("reason"),
        "provenance": string("provenance"),
    }


def _timing_record(wrapper: Any, manifest: Any, diagnostics: Diagnostics) -> dict[str, Any]:
    wrapper_start = deep_get(wrapper, ("started_at_ns",), None)
    wrapper_end = deep_get(wrapper, ("ended_at_ns",), None)
    wall_ms = deep_get(wrapper, ("wall_time_ms",), None)
    timing_valid = (
        is_nonnegative_integer(wrapper_start)
        and is_nonnegative_integer(wrapper_end)
        and wrapper_end >= wrapper_start
        and is_nonnegative_integer(wall_ms)
        and wall_ms == (wrapper_end - wrapper_start) // 1_000_000
    )
    if not timing_valid:
        diagnostics.issue(
            "invalid_wrapper_timing",
            "run.timing",
            "wrapper ns bounds and wall_time_ms are missing, invalid, or inconsistent",
        )
    manifest_start = deep_get(manifest, ("started_at_ns",), None)
    manifest_end = deep_get(manifest, ("ended_at_ns",), None)
    return {
        "wrapper_started_at_ns": wrapper_start if is_nonnegative_integer(wrapper_start) else None,
        "wrapper_ended_at_ns": wrapper_end if is_nonnegative_integer(wrapper_end) else None,
        "wrapper_wall_time_ms": wall_ms if is_nonnegative_integer(wall_ms) else None,
        "wrapper_timing_consistent": timing_valid,
        "manifest_started_at_ns": manifest_start if is_nonnegative_integer(manifest_start) else None,
        "manifest_ended_at_ns": manifest_end if is_nonnegative_integer(manifest_end) else None,
    }


def _tool_cost(
    completed_calls: Sequence[Mapping[str, Any]],
    tool_integrity: Mapping[str, Any],
    capture: Mapping[str, Any],
    tokens: Mapping[str, Any],
    model_steps: int | None,
    replay: Mapping[str, Any],
    wrapper: Any,
) -> dict[str, Any]:
    tool_conflict = any(call["conflicts"] for call in completed_calls)
    identity_valid = (
        tool_integrity.get("authoritative_completed_ids_available") is True
        and tool_integrity.get("raw_terminal_identity_match") is True
        and not tool_conflict
    )
    tool_count = len(completed_calls) if identity_valid else None
    families_valid = identity_valid and all(call["tool"] is not None for call in completed_calls)
    family_counts: dict[str, int] | None = None
    navigation_count = None
    if families_valid:
        counts: dict[str, int] = defaultdict(int)
        for call in completed_calls:
            counts[call["family"]] += 1
        family_counts = dict(sorted(counts.items()))
        navigation_count = sum(call["family"] in NAVIGATION_FAMILIES for call in completed_calls)

    input_values = [call["input_utf8_bytes"] for call in completed_calls]
    output_values = [call["output_utf8_bytes"] for call in completed_calls]
    elapsed_values = [call["client_observed_elapsed_ms"] for call in completed_calls]
    input_total = (
        sum(input_values)
        if identity_valid and all(is_nonnegative_integer(value) for value in input_values)
        else None
    )
    output_total = (
        sum(output_values)
        if identity_valid and all(is_nonnegative_integer(value) for value in output_values)
        else None
    )
    elapsed_total = (
        sum(elapsed_values)
        if identity_valid and all(is_number(value) and value >= 0 for value in elapsed_values)
        else None
    )
    elapsed_by_family: dict[str, int | float] | None = None
    if families_valid and elapsed_total is not None:
        by_family: dict[str, int | float] = defaultdict(int)
        for call in completed_calls:
            by_family[call["family"]] += call["client_observed_elapsed_ms"]
        elapsed_by_family = dict(sorted(by_family.items()))

    tool_errors = [call["identity"] for call in completed_calls if call["is_error"]]
    protocol = replay.get("protocol_failures", deep_get(wrapper, ("protocol_failures",), None))
    protocol_items = (
        protocol
        if isinstance(protocol, list) and all(isinstance(item, str) for item in protocol)
        else None
    )
    stdout_kept = deep_get(capture, ("stdout", "kept_bytes"), None)
    stderr_kept = deep_get(capture, ("stderr", "kept_bytes"), None)
    wall_ms = deep_get(wrapper, ("wall_time_ms",), None)
    return {
        "model_steps": model_steps,
        "tool_calls_total": tool_count,
        "navigation_calls_excluding_handshake": navigation_count,
        "tool_calls_by_family": family_counts,
        "tool_input_bytes": input_total,
        "tool_output_bytes": output_total,
        "tool_byte_measurement": {
            "input_complete": input_total is not None,
            "output_complete": output_total is not None,
            "input_rule": "UTF-8 bytes of canonical compact JSON with sorted keys.",
            "output_rule": "UTF-8 bytes of recorded string; non-string output uses canonical compact JSON.",
            "transport_wire_bytes": False,
        },
        "captured_stdout_bytes": stdout_kept if is_nonnegative_integer(stdout_kept) else None,
        "captured_stderr_bytes": stderr_kept if is_nonnegative_integer(stderr_kept) else None,
        "capture_integrity": capture,
        "tokens": tokens,
        "command_wall_ms": wall_ms if is_nonnegative_integer(wall_ms) else None,
        "client_observed_tool_ms": {
            "total": elapsed_total,
            "by_family": elapsed_by_family,
            "complete": elapsed_total is not None,
            "measurement_rule": "Deduplicated completed event state.time.end - state.time.start.",
        },
        "codemap_internal_ms": None,
        "tool_errors": {
            "count": len(tool_errors) if identity_valid else None,
            "identities": tool_errors if identity_valid else None,
        },
        "protocol_errors": {
            "count": len(protocol_items) if protocol_items is not None else None,
            "items": protocol_items,
        },
    }


def extract_run_metrics(run_dir: Path) -> dict[str, Any]:
    run_dir = run_dir.resolve()
    diagnostics = Diagnostics()
    paths = {
        "events": run_dir / "raw/events.jsonl",
        "stderr": run_dir / "raw/stderr.log",
        "normalized": run_dir / "normalized.json",
        "wrapper": run_dir / "wrapper.json",
        "manifest": find_first_file(run_dir, ("run.manifest.json", "manifest.json")),
        "classification": run_dir / "attempt-classification.json",
        "postprocess": run_dir / "postprocess-status.json",
        "auth_cleanup": run_dir / "auth-cleanup.json",
        "invariants": run_dir / "invariants.after.json",
        "artifact_manifest": run_dir / "artifact-manifest.json",
        "finalization": find_first_file(run_dir, ("finalization.json", "run.finalization.json")),
        "ledger": find_first_file(run_dir.parent, ("ledger.json",)),
    }
    normalized = load_json_file(paths["normalized"], diagnostics, "run.input_files.normalized", critical=True)
    wrapper = load_json_file(paths["wrapper"], diagnostics, "run.input_files.wrapper", critical=True)
    manifest = load_json_file(paths["manifest"], diagnostics, "run.input_files.manifest", critical=True)
    classification_raw = load_json_file(
        paths["classification"], diagnostics, "run.input_files.classification", critical=True
    )
    postprocess_raw = load_json_file(
        paths["postprocess"], diagnostics, "run.input_files.postprocess", critical=True
    )
    auth_raw = load_json_file(
        paths["auth_cleanup"], diagnostics, "run.input_files.auth_cleanup", critical=True
    )
    invariants_raw = load_json_file(
        paths["invariants"], diagnostics, "run.input_files.invariants", critical=True
    )
    artifact_raw = load_json_file(
        paths["artifact_manifest"], diagnostics, "run.input_files.artifact_manifest", critical=True
    )
    ledger = load_json_file(paths["ledger"], diagnostics, "run.input_files.ledger", critical=True)
    finalization_raw = (
        load_json_file(paths["finalization"], diagnostics, "run.input_files.finalization", critical=False)
        if paths["finalization"] is not None
        else deep_get(manifest, ("finalization",), {})
    )

    classification = normalize_classification(classification_raw, diagnostics)
    accepted_rows, event_stream, _ = load_event_stream(
        paths["events"], wrapper, normalized, diagnostics
    )
    replay = normalized_replay(
        normalized,
        wrapper,
        classification["terminal_behavior"],
        diagnostics,
    )
    capture = capture_metrics(paths["events"], paths["stderr"], wrapper, diagnostics)
    artifact_seal = verify_artifact_seal(
        run_dir,
        paths["artifact_manifest"],
        artifact_raw,
        diagnostics,
    )
    postprocess = normalize_postprocess(postprocess_raw)
    auth_cleanup = normalize_auth_cleanup(auth_raw)
    invariants = normalize_invariants(invariants_raw)

    if not (
        postprocess["parser_status"] == 0
        and postprocess["parser_ok"] is True
        and postprocess["required_files_present"] is True
    ):
        diagnostics.issue(
            "postprocess_integrity_invalid",
            "run.lifecycle.postprocess",
            "parser status/ok/required-files evidence is not valid",
        )
    if auth_cleanup["status"] != 0:
        diagnostics.issue(
            "auth_cleanup_invalid",
            "run.lifecycle.auth_cleanup",
            "auth cleanup status is not zero",
        )
    if not (
        invariants["generation_current"] is True
        and invariants["source_unchanged"] is True
        and invariants["index_unchanged"] is True
    ):
        diagnostics.issue(
            "run_invariant_invalid",
            "run.lifecycle.invariants",
            "generation/source/index invariants are not all true",
        )

    publication = publication_linkage(
        run_dir,
        manifest,
        classification_raw,
        classification,
        paths["artifact_manifest"],
        ledger,
        diagnostics,
    )
    completed_calls, incomplete_calls, tool_integrity = normalize_tool_calls(
        accepted_rows, replay, wrapper, diagnostics
    )
    tokens, model_steps = token_metrics(
        accepted_rows,
        normalized,
        replay,
        classification["terminal_behavior"],
        diagnostics,
    )
    final_answer = final_answer_metrics(
        accepted_rows,
        replay,
        wrapper,
        classification["terminal_behavior"],
        diagnostics,
    )
    timing = _timing_record(wrapper, manifest, diagnostics)

    tool_conflict = any(call["conflicts"] for call in completed_calls)
    repeated = repeated_search_metrics(completed_calls, tool_conflict)
    unbounded = unbounded_read_metrics(completed_calls, tool_conflict)
    cost = _tool_cost(
        completed_calls,
        tool_integrity,
        capture,
        tokens,
        model_steps,
        replay,
        wrapper,
    )
    diagnostics.missing(
        "cost.codemap_internal_ms",
        "server-internal time is not in supported inputs and is not inferred",
        critical=False,
    )

    schedule = deep_get(manifest, ("schedule",), {})
    run_id = deep_get(manifest, ("run_id",), None)
    generation_id = deep_get(manifest, ("generation_id",), None)
    attempt_number = deep_get(manifest, ("attempt_number",), None)
    if not isinstance(run_id, str) or not run_id:
        diagnostics.missing("run.run_id", "manifest run_id is absent", critical=True)
        run_id = None
    elif run_dir.name != run_id:
        diagnostics.issue(
            "run_directory_identity_mismatch",
            "run.run_id",
            "run directory name differs from manifest run_id",
            identities=[run_id, run_dir.name],
        )
    if not isinstance(generation_id, str) or not generation_id:
        diagnostics.missing("run.generation_id", "manifest generation_id is absent", critical=True)
        generation_id = None
    if not is_positive_integer(attempt_number):
        diagnostics.missing("run.attempt_number", "attempt_number is not a positive integer", critical=True)
        attempt_number = None

    task_id = deep_get(schedule, ("task_id",), None)
    trial_id = deep_get(schedule, ("trial_id",), deep_get(schedule, ("repeat_id",), None))
    pair_id = deep_get(schedule, ("pair_id",), None)
    arm = deep_get(schedule, ("criterion",), deep_get(schedule, ("arm",), None))
    pair_order_index = deep_get(schedule, ("pair_order_index",), None)
    for field_path, value in (
        ("experiment.task_id", task_id),
        ("experiment.trial_id", trial_id),
        ("experiment.pair_id", pair_id),
        ("experiment.arm", arm),
    ):
        if not isinstance(value, str) or not value:
            diagnostics.missing(field_path, "schedule value is absent", critical=True)
    if pair_order_index not in {0, 1}:
        diagnostics.missing(
            "experiment.pair_order_index",
            "pair_order_index is not 0 or 1",
            critical=True,
        )
        pair_order_index = None

    question_hash = _hash_field(
        deep_get(schedule, ("question_sha256",), None),
        "experiment.question_sha256",
        diagnostics,
        critical=True,
    )
    prompt_hash = _hash_field(
        deep_get(manifest, ("prompt_sha256",), None),
        "experiment.prompt_sha256",
        diagnostics,
        critical=True,
    )
    corpus_hash = _hash_field(
        first_not_none(
            deep_get(manifest, ("source_tree_sha256",), MISSING),
            deep_get(manifest, ("corpus_tree_sha256",), MISSING),
        ),
        "experiment.corpus_tree_sha256",
        diagnostics,
        critical=True,
    )
    model_config_hash = _hash_field(
        deep_get(manifest, ("model_config_sha256",), None),
        "experiment.model_config_sha256",
        diagnostics,
        critical=False,
    )
    limits_hash = _hash_field(
        deep_get(manifest, ("limits_sha256",), None),
        "experiment.limits_sha256",
        diagnostics,
        critical=True,
    )
    arm_config_hash = _hash_field(
        deep_get(manifest, ("arm_config_sha256",), None),
        "experiment.arm_config_sha256",
        diagnostics,
        critical=True,
    )
    model = first_not_none(deep_get(manifest, ("model",), MISSING), _command_model(wrapper))
    if not isinstance(model, str) or not model:
        diagnostics.missing("experiment.model", "model is absent from manifest and command", critical=True)
        model = None

    codemap_calls = [
        call
        for call in completed_calls
        if isinstance(call["tool"], str) and call["tool"].casefold().startswith("codemap_search_")
    ]
    codemap_non_handshake = [call for call in codemap_calls if call["family"] != "handshake"]

    terminal = _terminal_observation(wrapper)
    protocol_items = terminal["protocol_failures"]
    if protocol_items is None:
        diagnostics.issue(
            "protocol_failure_list_missing",
            "run.terminal_observation.protocol_failures",
            "wrapper protocol failure list is absent or invalid",
        )
    elif protocol_items:
        diagnostics.issue(
            "reported_protocol_failure",
            "run.terminal_observation.protocol_failures",
            "accepted replay reports one or more protocol failures",
        )

    aggregation_reasons: list[str] = []
    if classification["measurement_status"] != "valid":
        aggregation_reasons.append("measurement_status_not_valid")
    if classification["generation_invalid"] is not False:
        aggregation_reasons.append("generation_invalid_or_unknown")
    if classification["replacement_allowed"] is not False:
        aggregation_reasons.append("replacement_allowed_or_unknown")
    if publication["latest_published_attempt"] is not True:
        aggregation_reasons.append("not_latest_published_attempt")
    if artifact_seal["verified"] is not True:
        aggregation_reasons.append("artifact_seal_not_verified")
    if diagnostics.integrity_issues:
        aggregation_reasons.append("critical_integrity_invalid")
    aggregation_reasons = sorted(set(aggregation_reasons))
    aggregation_eligible = not aggregation_reasons

    input_files = {
        key: file_record(path if isinstance(path, Path) else None, run_dir)
        for key, path in paths.items()
    }
    result = {
        "schema_version": SCHEMA_VERSION,
        "run": {
            "artifact_kind": "automatic-run-metrics-raw-evidence",
            "scorer_input_allowed": False,
            "scorer_exclusion_reason": "Contains arm/run linkage and raw evidence; use only the separately blinded scoring pipeline.",
            "run_id": run_id,
            "directory": str(run_dir),
            "generation_id": generation_id,
            "attempt_number": attempt_number,
            "is_replacement": (attempt_number > 1) if attempt_number is not None else None,
            "extractor_version": EXTRACTOR_VERSION,
            "input_files": input_files,
            "event_stream": event_stream,
            "tool_identity_integrity": tool_integrity,
            "completed_tool_calls": public_tool_calls(completed_calls),
            "incomplete_tool_calls": public_tool_calls(incomplete_calls),
            "lifecycle": {
                "classification": classification,
                "postprocess": postprocess,
                "auth_cleanup": auth_cleanup,
                "invariants": invariants,
                "publication": publication,
                "artifact_seal": artifact_seal,
                "finalization": _finalization_record(finalization_raw),
            },
            "timing": timing,
            "terminal_observation": terminal,
            "integrity": {
                "status": "verified" if not diagnostics.integrity_issues else "invalid",
                "issues": sorted(
                    diagnostics.integrity_issues,
                    key=lambda issue: (issue["code"], issue["field"], issue["detail"]),
                ),
                "aggregation_eligible": aggregation_eligible,
                "aggregation_ineligible_reasons": aggregation_reasons,
            },
        },
        "experiment": {
            "task_id": task_id if isinstance(task_id, str) else None,
            "trial_id": trial_id if isinstance(trial_id, str) else None,
            "pair_id": pair_id if isinstance(pair_id, str) else None,
            "arm": arm if isinstance(arm, str) else None,
            "pair_order_index": pair_order_index,
            "question_sha256": question_hash,
            "prompt_sha256": prompt_hash,
            "corpus_tree_sha256": corpus_hash,
            "model_config_sha256": model_config_hash,
            "limits_sha256": limits_hash,
            "arm_config_sha256": arm_config_hash,
            "hash_provenance": {
                "question_sha256": "run.manifest.json:schedule.question_sha256" if question_hash else None,
                "prompt_sha256": "run.manifest.json:prompt_sha256" if prompt_hash else None,
                "corpus_tree_sha256": "run.manifest.json:source_tree_sha256" if corpus_hash else None,
                "model_config_sha256": "run.manifest.json:model_config_sha256" if model_config_hash else None,
                "limits_sha256": "run.manifest.json:limits_sha256" if limits_hash else None,
                "arm_config_sha256": "run.manifest.json:arm_config_sha256" if arm_config_hash else None,
            },
            "model": model,
            "generation_id": generation_id,
            "attempt_number": attempt_number,
            "measurement_status": classification["measurement_status"],
            "terminal_behavior": classification["terminal_behavior"],
            "replacement_allowed": classification["replacement_allowed"],
            "replacement_category": classification["replacement_category"],
            "generation_invalid": classification["generation_invalid"],
            "published": publication["published"],
            "latest_published_attempt": publication["latest_published_attempt"],
            "artifact_verified": artifact_seal["verified"],
            "aggregation_eligible": aggregation_eligible,
            "aggregation_ineligible_reasons": aggregation_reasons,
            "started_at_ns": timing["wrapper_started_at_ns"],
            "ended_at_ns": timing["wrapper_ended_at_ns"],
        },
        "dialogue": {
            "discovery_stages": {
                name: {"status": "unreviewed", "evidence": [], "raw_event_lines": []}
                for name in (
                    "decisive_evidence_exposed",
                    "selected_next",
                    "original_code_read",
                    "final_answer_used_correctly",
                )
            },
            "first_wrong": {
                "category": None,
                "raw_event_line": None,
                "completed_call_index": None,
                "explanation": None,
            },
            "correctness": None,
            "final_answer": final_answer,
            "codemap_tool_selection": {
                "selected_non_handshake": bool(codemap_non_handshake),
                "non_handshake_call_count": len(codemap_non_handshake),
                "handshake_call_count": sum(call["family"] == "handshake" for call in codemap_calls),
                "first_non_handshake_completed_call_index": (
                    codemap_non_handshake[0]["completed_call_index"] if codemap_non_handshake else None
                ),
                "identities": [call["identity"] for call in codemap_non_handshake],
                "tools": [call["tool"] for call in codemap_non_handshake],
            },
        },
        "cost": cost,
        "repeated_searches": repeated,
        "unbounded_reads": unbounded,
        "post_first_wrong": {
            "reason": "awaiting_manual_first_wrong",
            "first_wrong_raw_event_line": None,
            "completed_calls_after": None,
            "tool_calls_after": None,
            "tool_output_bytes_after": None,
            "tokens_after": None,
            "wall_ms_after": None,
            "wrong_direction_calls": None,
            "wrong_direction_output_bytes": None,
        },
        "missing_data": diagnostics.missing_output(),
    }
    return result


def _schema_type_matches(value: Any, expected: str) -> bool:
    if expected == "null":
        return value is None
    if expected == "object":
        return isinstance(value, Mapping)
    if expected == "array":
        return isinstance(value, list)
    if expected == "string":
        return isinstance(value, str)
    if expected == "boolean":
        return isinstance(value, bool)
    if expected == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if expected == "number":
        return isinstance(value, (int, float)) and not isinstance(value, bool)
    return False


def validate_json_schema(
    instance: Any,
    schema: Mapping[str, Any],
    *,
    root_schema: Mapping[str, Any] | None = None,
    path: str = "$",
) -> list[str]:
    """Validate the strict schema subset used by automatic-run-metrics."""
    root = root_schema or schema
    if "$ref" in schema:
        reference = schema["$ref"]
        if not isinstance(reference, str) or not reference.startswith("#/"):
            return [f"{path}: unsupported $ref {reference!r}"]
        target: Any = root
        for token in reference[2:].split("/"):
            token = token.replace("~1", "/").replace("~0", "~")
            if not isinstance(target, Mapping) or token not in target:
                return [f"{path}: unresolved $ref {reference}"]
            target = target[token]
        return validate_json_schema(instance, target, root_schema=root, path=path)
    errors: list[str] = []
    if "const" in schema and instance != schema["const"]:
        errors.append(f"{path}: value differs from const")
    if "enum" in schema and instance not in schema["enum"]:
        errors.append(f"{path}: value is outside enum")
    expected_type = schema.get("type")
    if expected_type is not None:
        allowed = expected_type if isinstance(expected_type, list) else [expected_type]
        if not any(_schema_type_matches(instance, item) for item in allowed):
            errors.append(f"{path}: expected type {allowed}, got {type(instance).__name__}")
            return errors
    if isinstance(instance, Mapping):
        required = schema.get("required", [])
        for key in required:
            if key not in instance:
                errors.append(f"{path}: missing required property {key}")
        properties = schema.get("properties", {})
        additional = schema.get("additionalProperties", True)
        for key, value in instance.items():
            if key in properties:
                errors.extend(
                    validate_json_schema(
                        value,
                        properties[key],
                        root_schema=root,
                        path=f"{path}.{key}",
                    )
                )
            elif additional is False:
                errors.append(f"{path}: additional property {key}")
            elif isinstance(additional, Mapping):
                errors.extend(
                    validate_json_schema(
                        value,
                        additional,
                        root_schema=root,
                        path=f"{path}.{key}",
                    )
                )
    if isinstance(instance, list):
        if "minItems" in schema and len(instance) < schema["minItems"]:
            errors.append(f"{path}: fewer than minItems")
        item_schema = schema.get("items")
        if isinstance(item_schema, Mapping):
            for index, value in enumerate(instance):
                errors.extend(
                    validate_json_schema(
                        value,
                        item_schema,
                        root_schema=root,
                        path=f"{path}[{index}]",
                    )
                )
    if isinstance(instance, (int, float)) and not isinstance(instance, bool):
        if "minimum" in schema and instance < schema["minimum"]:
            errors.append(f"{path}: below minimum")
    if isinstance(instance, str) and "pattern" in schema:
        if re.search(schema["pattern"], instance) is None:
            errors.append(f"{path}: string does not match pattern")
    return errors


def build_parser() -> argparse.ArgumentParser:
    default_schema = (
        Path(__file__).resolve().parents[1]
        / "harness/schemas/automatic-run-metrics.schema.json"
    )
    parser = argparse.ArgumentParser(
        description="Extract raw automatic metrics from one OpenCode run; never use as a scorer input."
    )
    parser.add_argument("run_dir", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--schema", type=Path, default=default_schema)
    parser.add_argument("--compact", action="store_true")
    parser.add_argument(
        "--require-aggregation-eligible",
        action="store_true",
        help="Exit nonzero after writing evidence unless schema and all aggregation gates pass.",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if not args.run_dir.is_dir():
        print(f"run directory does not exist: {args.run_dir}", file=sys.stderr)
        return 2
    result = extract_run_metrics(args.run_dir)
    schema_errors: list[str]
    try:
        schema = json.loads(args.schema.read_text(encoding="utf-8"))
        if not isinstance(schema, Mapping):
            schema_errors = ["schema root is not an object"]
        else:
            schema_errors = validate_json_schema(result, schema)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        schema_errors = [f"schema unavailable or invalid: {error}"]
    if schema_errors:
        print("automatic metrics schema validation failed:", file=sys.stderr)
        for error in schema_errors[:20]:
            print(f"- {error}", file=sys.stderr)

    rendered = json.dumps(
        result,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":") if args.compact else None,
        indent=None if args.compact else 2,
    ) + "\n"
    if args.output is None:
        sys.stdout.write(rendered)
    elif not args.output.parent.is_dir():
        print(f"output parent directory does not exist: {args.output.parent}", file=sys.stderr)
        return 2
    else:
        args.output.write_text(rendered, encoding="utf-8")

    if args.require_aggregation_eligible:
        if schema_errors:
            return 4
        if result["experiment"]["aggregation_eligible"] is not True:
            return 3
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
