#!/usr/bin/env python3
"""Fail-closed, deterministic aggregation for one sealed baseline-3x generation.

The command reads only files named by explicit metric and judgment indexes.  It
never scans a run tree, so superseded attempts cannot be included by accident.
Automatic metrics must use the sealed ``extract_run_metrics.py`` schema-v2
record shape.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
import os
import re
import statistics
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


SCHEMA_VERSION = 1
EXPECTED_TASKS = 14
EXPECTED_TRIALS = ("r1", "r2", "r3")
EXPECTED_ARMS = ("B1", "B2")
EXPECTED_SESSIONS = 84
EXPECTED_JUDGMENTS_PER_OUTPUT = 3
EXPECTED_SCORERS = ("scorer-1", "scorer-2", "scorer-3")
NAVIGATION_FAMILIES = {"overview", "search", "grep", "read", "find", "glob"}
TRANSIENT_CATEGORIES = ("transient_auth", "transient_provider", "transient_network")
TERMINAL_BEHAVIORS = (
    "stop", "timeout", "model_step_limit", "output_limit", "process_error",
    "protocol_error", "infrastructure_error", "unknown",
)
ATTEMPT_STATES = ("terminal", "canceled-before-start")

TOKEN_COMPONENTS = ("input", "output", "reasoning", "cache_read", "cache_write", "total")
DISCOVERY_STAGES = (
    "decisive_evidence_exposed",
    "selected_next",
    "original_code_read",
    "final_answer_used_correctly",
)
DISCOVERY_STATUSES = ("yes", "no", "not_applicable", "unscored")
SEMANTIC_LABELS = ("correct", "partial", "incorrect")
FIRST_WRONG_CATEGORIES = (
    "none",
    "initial_area",
    "tool_choice",
    "query_too_broad",
    "query_too_narrow",
    "scope",
    "answer_evidence_not_exposed",
    "insufficient_code_evidence",
    "similar_result_dominance",
    "evidence_not_selected",
    "original_not_read",
    "final_misuse",
    "other",
)
ALLOWED_FRACTIONS = (0, 0.5, 1)

BASE_NUMERIC_METRICS = (
    "quality.score_mean",
    "quality.score_min",
    "quality.score_max",
    "quality.score_range",
    "cost.model_steps",
    "cost.tool_calls_total",
    "cost.navigation_calls_excluding_handshake",
    "cost.tool_input_bytes",
    "cost.tool_output_bytes",
    "cost.captured_stdout_bytes",
    "cost.captured_stderr_bytes",
    "cost.tokens.input",
    "cost.tokens.output",
    "cost.tokens.reasoning",
    "cost.tokens.cache_read",
    "cost.tokens.cache_write",
    "cost.tokens.total",
    "cost.command_wall_ms",
    "cost.client_observed_tool_ms",
    "cost.tool_errors",
    "cost.protocol_errors",
    "repeated_searches.group_count",
    "repeated_searches.extra_call_count",
    "unbounded_reads.count",
    "unbounded_reads.output_bytes",
    "dialogue.codemap_tool_selection.non_handshake_call_count",
    "dialogue.codemap_tool_selection.handshake_call_count",
    "post_first_wrong.tool_calls",
    "post_first_wrong.tool_output_bytes",
    "post_first_wrong.tokens",
    "post_first_wrong.wall_ms",
    "post_first_wrong.tool_errors",
    "post_first_wrong.repeated_extra_calls",
    "post_first_wrong.unbounded_read_calls",
    "navigation.first_completed_call_index",
    "navigation.first_input_bytes",
    "navigation.first_output_bytes",
    "navigation.first_client_observed_ms",
    "navigation.codemap_first_use_completed_call_index",
    "navigation.origin_switch_count",
)

QUALITY_COST_METRICS = (
    "cost.model_steps",
    "cost.tool_calls_total",
    "cost.navigation_calls_excluding_handshake",
    "cost.tool_input_bytes",
    "cost.tool_output_bytes",
    "cost.tokens.input",
    "cost.tokens.output",
    "cost.tokens.reasoning",
    "cost.tokens.cache_read",
    "cost.tokens.cache_write",
    "cost.tokens.total",
    "cost.command_wall_ms",
    "cost.client_observed_tool_ms",
)

MISSING = object()


class AggregationError(ValueError):
    """An input contract violation that must stop aggregation."""


def canonical_sha256(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def reject_json_constant(value: str) -> None:
    raise ValueError(f"non-standard JSON constant {value}")


def load_json(path: Path) -> Any:
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            parse_constant=reject_json_constant,
        )
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        raise AggregationError(f"cannot read valid JSON from {path}: {error}") from error


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
        return is_number(value)
    return False


def validate_json_schema(
    instance: Any,
    schema: Mapping[str, Any],
    *,
    root_schema: Mapping[str, Any] | None = None,
    path: str = "$",
) -> list[str]:
    """Validate the strict JSON Schema subset used by automatic metrics."""
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
                errors.extend(validate_json_schema(value, properties[key], root_schema=root, path=f"{path}.{key}"))
            elif additional is False:
                errors.append(f"{path}: additional property {key}")
            elif isinstance(additional, Mapping):
                errors.extend(validate_json_schema(value, additional, root_schema=root, path=f"{path}.{key}"))
    if isinstance(instance, list):
        if "minItems" in schema and len(instance) < schema["minItems"]:
            errors.append(f"{path}: fewer than minItems")
        if "maxItems" in schema and len(instance) > schema["maxItems"]:
            errors.append(f"{path}: more than maxItems")
        item_schema = schema.get("items")
        if isinstance(item_schema, Mapping):
            for index, value in enumerate(instance):
                errors.extend(validate_json_schema(value, item_schema, root_schema=root, path=f"{path}[{index}]"))
    if is_number(instance):
        if "minimum" in schema and instance < schema["minimum"]:
            errors.append(f"{path}: below minimum")
        if "maximum" in schema and instance > schema["maximum"]:
            errors.append(f"{path}: above maximum")
    if isinstance(instance, str):
        if "minLength" in schema and len(instance) < schema["minLength"]:
            errors.append(f"{path}: shorter than minLength")
        if "maxLength" in schema and len(instance) > schema["maxLength"]:
            errors.append(f"{path}: longer than maxLength")
        if "pattern" in schema and re.search(schema["pattern"], instance) is None:
            errors.append(f"{path}: string does not match pattern")
    return errors


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AggregationError(message)


def deep_get(value: Any, path: Sequence[str], default: Any = MISSING) -> Any:
    current = value
    for key in path:
        if not isinstance(current, Mapping) or key not in current:
            return default
        current = current[key]
    return current


def first_present(*values: Any) -> Any:
    for value in values:
        if value is not MISSING:
            return value
    return MISSING


def is_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def optional_number(value: Any, field: str) -> int | float | None:
    if value is MISSING or value is None:
        return None
    if not is_number(value):
        raise AggregationError(f"{field} must be a finite number or null")
    return value


def optional_nonnegative_integer(value: Any, field: str) -> int | None:
    if value is MISSING or value is None:
        return None
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise AggregationError(f"{field} must be a nonnegative integer or null")
    return value


def optional_nonnegative_number(value: Any, field: str) -> int | float | None:
    result = optional_number(value, field)
    if result is not None and result < 0:
        raise AggregationError(f"{field} must be nonnegative or null")
    return result


def path_has_symlink(path: Path) -> bool:
    absolute = path.absolute()
    candidates = [absolute, *absolute.parents]
    return any(candidate.is_symlink() for candidate in candidates)


def require_read_only_regular_file(path: Path, field: str) -> None:
    require(path.is_absolute(), f"{field} must be absolute")
    require(not path_has_symlink(path), f"{field} must not contain a symlink")
    require(path.is_file(), f"{field} must be a regular file")
    require(path.stat().st_mode & 0o222 == 0, f"{field} must be read-only")


def numeric_summary(values: Sequence[int | float | None], expected_n: int | None = None) -> dict[str, Any]:
    """Summarize complete numeric values without imputing missing observations."""
    expected = len(values) if expected_n is None else expected_n
    require(len(values) == expected, f"numeric summary expected {expected} raw values, got {len(values)}")
    checked = [optional_number(value, "numeric_summary.raw") for value in values]
    valid = [value for value in checked if value is not None]
    valid_n = len(valid)
    mean = sum(valid) / valid_n if valid else None
    median = statistics.median(valid) if valid else None
    minimum = min(valid) if valid else None
    maximum = max(valid) if valid else None
    sample_sd = statistics.stdev(valid) if valid_n >= 2 else None
    return {
        "raw": checked,
        "denominator_n": expected,
        "valid_n": valid_n,
        "missing_n": expected - valid_n,
        "mean": mean,
        "median": median,
        "min": minimum,
        "max": maximum,
        "range": maximum - minimum if valid else None,
        "sample_sd": sample_sd,
        "sample_sd_denominator": valid_n - 1 if valid_n >= 2 else None,
    }


def trial_numeric_summary(raw_by_trial: Mapping[str, int | float | None]) -> dict[str, Any]:
    require(set(raw_by_trial) == set(EXPECTED_TRIALS), "task-arm metric must contain exactly r1, r2, and r3")
    result = numeric_summary([raw_by_trial[trial] for trial in EXPECTED_TRIALS], expected_n=3)
    result["raw_by_trial"] = {trial: raw_by_trial[trial] for trial in EXPECTED_TRIALS}
    result["denominator"] = "three sealed trials for this task and arm"
    return result


def categorical_summary(
    values: Sequence[Any],
    allowed_values: Sequence[Any],
    *,
    expected_n: int | None = None,
) -> dict[str, Any]:
    expected = len(values) if expected_n is None else expected_n
    require(len(values) == expected, f"categorical summary expected {expected} raw values, got {len(values)}")
    allowed = tuple(allowed_values)
    for value in values:
        if value is not None and value not in allowed:
            raise AggregationError(f"categorical value {value!r} is outside {allowed!r}")
    valid = [value for value in values if value is not None]
    counts = {str(value).lower() if isinstance(value, bool) else str(value): valid.count(value) for value in allowed}
    valid_n = len(valid)
    rates = {key: count / valid_n if valid_n else None for key, count in counts.items()}
    return {
        "raw": list(values),
        "denominator_n": expected,
        "valid_n": valid_n,
        "missing_n": expected - valid_n,
        "rate_denominator_n": valid_n,
        "counts": counts,
        "rates": rates,
    }


def majority_of_three(values: Sequence[Any], allowed_values: Iterable[Any], field: str) -> Any | None:
    """Return a strict 2-of-3 majority; a valid 1/1/1 split has no majority."""
    require(len(values) == 3, f"{field} requires exactly three values")
    allowed = tuple(allowed_values)
    for value in values:
        if value not in allowed:
            raise AggregationError(f"{field} has malformed value {value!r}")
    counts = Counter(values)
    winners = [value for value, count in counts.items() if count >= 2]
    require(len(winners) <= 1, f"{field} has more than one majority")
    return winners[0] if winners else None


def _mapping(value: Any, field: str) -> Mapping[str, Any]:
    require(isinstance(value, Mapping), f"{field} must be an object")
    return value


def _list(value: Any, field: str) -> list[Any]:
    require(isinstance(value, list), f"{field} must be an array")
    return value


def _string(value: Any, field: str) -> str:
    require(isinstance(value, str) and bool(value), f"{field} must be a non-empty string")
    return value


def _boolean(value: Any, field: str) -> bool:
    require(isinstance(value, bool), f"{field} must be boolean")
    return value


def _validate_scored_items(value: Any, field: str) -> tuple[list[dict[str, Any]], set[str]]:
    items = _list(value, field)
    require(items, f"{field} must not be empty")
    result: list[dict[str, Any]] = []
    identifiers: set[str] = set()
    for index, raw in enumerate(items):
        item = _mapping(raw, f"{field}[{index}]")
        item_id = _string(item.get("item_id"), f"{field}[{index}].item_id")
        require(item_id not in identifiers, f"{field} contains duplicate item_id {item_id!r}")
        identifiers.add(item_id)
        fraction = item.get("fraction")
        require(fraction in ALLOWED_FRACTIONS and not isinstance(fraction, bool), f"{field}[{index}].fraction violates rubric")
        result.append(dict(item))
    return result, identifiers


def _validate_prohibited_items(value: Any, field: str) -> tuple[list[dict[str, Any]], set[str]]:
    items = _list(value, field)
    result: list[dict[str, Any]] = []
    identifiers: set[str] = set()
    for index, raw in enumerate(items):
        item = _mapping(raw, f"{field}[{index}]")
        item_id = _string(item.get("item_id"), f"{field}[{index}].item_id")
        require(item_id not in identifiers, f"{field} contains duplicate item_id {item_id!r}")
        identifiers.add(item_id)
        _boolean(item.get("explicitly_present"), f"{field}[{index}].explicitly_present")
        result.append(dict(item))
    return result, identifiers


def canonical_navigation_call_identities(metric: Mapping[str, Any]) -> list[dict[str, Any]]:
    calls = _list(deep_get(metric, ("run", "completed_tool_calls"), MISSING), "metric.run.completed_tool_calls")
    return [
        {
            "identity": call["identity"],
            "completed_call_index": call["completed_call_index"],
            "call_id": call["call_id"],
            "raw_event_line": call["selected_completion_line"],
            "family": call["family"],
        }
        for call in calls
        if isinstance(call, Mapping) and call.get("completed") is True and call.get("family") in NAVIGATION_FAMILIES
    ]


def _validate_dialogue(value: Any, field: str, canonical_calls: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    dialogue = _mapping(value, field)
    stages = _mapping(dialogue.get("discovery_stages"), f"{field}.discovery_stages")
    require(set(stages) == set(DISCOVERY_STAGES), f"{field}.discovery_stages item set mismatch")
    for stage in DISCOVERY_STAGES:
        stage_value = _mapping(stages[stage], f"{field}.discovery_stages.{stage}")
        require(stage_value.get("status") in DISCOVERY_STATUSES, f"{field}.discovery_stages.{stage}.status is invalid")
        raw_lines = _list(stage_value.get("raw_event_lines"), f"{field}.discovery_stages.{stage}.raw_event_lines")
        call_ids = _list(stage_value.get("call_ids"), f"{field}.discovery_stages.{stage}.call_ids")
        require(
            all(isinstance(line, int) and not isinstance(line, bool) and line >= 1 for line in raw_lines)
            and len(set(raw_lines)) == len(raw_lines),
            f"{field}.discovery_stages.{stage}.raw_event_lines are invalid",
        )
        require(
            all(isinstance(call_id, str) and bool(call_id) for call_id in call_ids)
            and len(set(call_ids)) == len(call_ids),
            f"{field}.discovery_stages.{stage}.call_ids are invalid",
        )
        expected_call_ids = [call["call_id"] for call in canonical_calls if call["raw_event_line"] in set(raw_lines)]
        require(call_ids == expected_call_ids, f"{field}.discovery_stages.{stage} call ids do not match canonical completed navigation order")
    first_wrong = _mapping(dialogue.get("first_wrong"), f"{field}.first_wrong")
    require(first_wrong.get("category") in FIRST_WRONG_CATEGORIES, f"{field}.first_wrong.category is invalid")
    for position_field in ("raw_event_line", "completed_call_index"):
        position = first_wrong.get(position_field)
        require(
            position is None or (isinstance(position, int) and not isinstance(position, bool) and position >= 1),
            f"{field}.first_wrong.{position_field} must be a positive integer or null",
        )
    require(first_wrong.get("call_id") is None or isinstance(first_wrong.get("call_id"), str), f"{field}.first_wrong.call_id must be a string or null")
    require(isinstance(first_wrong.get("explanation"), str), f"{field}.first_wrong.explanation must be a string")
    boundary = (
        first_wrong.get("completed_call_index"),
        first_wrong.get("call_id"),
        first_wrong.get("raw_event_line"),
    )
    if first_wrong["category"] == "none":
        require(boundary == (None, None, None), f"{field}.first_wrong none must have a null boundary")
    else:
        require(all(component is not None for component in boundary), f"{field}.first_wrong requires one complete canonical boundary")
        require(
            any(
                boundary == (call["completed_call_index"], call["call_id"], call["raw_event_line"])
                for call in canonical_calls
            ),
            f"{field}.first_wrong boundary is not one completed navigation call",
        )
    return dict(dialogue)


def validate_judgment(
    value: Any,
    *,
    expected_review_id: str,
    expected_scorer_id: str,
    rubric_sha256: str,
    answer_contract: Mapping[str, Any],
    canonical_calls: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    judgment = _mapping(value, f"judgment[{expected_review_id}]")
    require(judgment.get("schema_version") == 1, f"judgment {expected_review_id} schema_version mismatch")
    require(judgment.get("blind_output_id") == expected_review_id, f"judgment {expected_review_id} blind_output_id mismatch")
    require(judgment.get("scorer_id") == expected_scorer_id, f"judgment {expected_review_id} scorer_id mismatch")
    require(judgment.get("rubric_sha256") == rubric_sha256, f"judgment {expected_review_id} rubric hash mismatch")

    correctness = _mapping(judgment.get("correctness"), f"judgment[{expected_review_id}].correctness")
    core = correctness.get("core_fraction")
    grounding = correctness.get("grounding_fraction")
    require(core in ALLOWED_FRACTIONS and not isinstance(core, bool), f"judgment {expected_review_id} core fraction violates rubric")
    require(grounding in ALLOWED_FRACTIONS and not isinstance(grounding, bool), f"judgment {expected_review_id} grounding fraction violates rubric")
    claims, claim_ids = _validate_scored_items(correctness.get("required_claims"), f"judgment[{expected_review_id}].required_claims")
    relationships, relationship_ids = _validate_scored_items(
        correctness.get("required_relationships"), f"judgment[{expected_review_id}].required_relationships"
    )
    prohibited, prohibited_ids = _validate_prohibited_items(
        correctness.get("prohibited_claims"), f"judgment[{expected_review_id}].prohibited_claims"
    )

    expected_claim_n = len(_list(answer_contract.get("final_answer_required"), "answer_contract.final_answer_required"))
    expected_relationship_n = len(_list(answer_contract.get("required_file_relationships"), "answer_contract.required_file_relationships"))
    expected_prohibited_n = len(_list(answer_contract.get("prohibited_claims"), "answer_contract.prohibited_claims"))
    require(len(claims) == expected_claim_n, f"judgment {expected_review_id} required-claim count does not match answer contract")
    require(len(relationships) == expected_relationship_n, f"judgment {expected_review_id} relationship count does not match answer contract")
    require(len(prohibited) == expected_prohibited_n, f"judgment {expected_review_id} prohibited-claim count does not match answer contract")

    claim_mean = sum(item["fraction"] for item in claims) / len(claims)
    relationship_mean = sum(item["fraction"] for item in relationships) / len(relationships)
    explicit_count = sum(item["explicitly_present"] is True for item in prohibited)
    computed_score = max(
        0,
        min(
            100,
            40 * core + 30 * claim_mean + 15 * grounding + 15 * relationship_mean - min(40, 20 * explicit_count),
        ),
    )
    score = optional_number(correctness.get("score_0_100", MISSING), f"judgment[{expected_review_id}].score_0_100")
    require(score is not None and score == computed_score, f"judgment {expected_review_id} numeric score does not match sealed formula")
    if core == 1 and explicit_count == 0 and computed_score >= 85:
        expected_label = "correct"
    elif core > 0 and computed_score >= 50:
        expected_label = "partial"
    else:
        expected_label = "incorrect"
    require(correctness.get("semantic_label") == expected_label, f"judgment {expected_review_id} semantic label does not match score")
    expected_complete = grounding == 1 and all(item["fraction"] == 1 for item in claims + relationships)
    require(correctness.get("contract_complete") is expected_complete, f"judgment {expected_review_id} contract_complete mismatch")
    require(
        correctness.get("contract_label") == ("complete" if expected_complete else "incomplete"),
        f"judgment {expected_review_id} contract_label mismatch",
    )
    _boolean(correctness.get("format_valid_json"), f"judgment[{expected_review_id}].format_valid_json")
    dialogue = _validate_dialogue(judgment.get("dialogue"), f"judgment[{expected_review_id}].dialogue", canonical_calls)
    require(isinstance(judgment.get("notes"), str), f"judgment {expected_review_id} notes must be a string")
    return {
        "review_id": expected_review_id,
        "scorer_id": expected_scorer_id,
        "correctness": dict(correctness),
        "dialogue": dialogue,
        "notes": judgment["notes"],
        "item_sets": {
            "required_claims": claim_ids,
            "required_relationships": relationship_ids,
            "prohibited_claims": prohibited_ids,
        },
    }


def _raw_component(validated: Sequence[Mapping[str, Any]], path: Sequence[str]) -> list[Any]:
    return [deep_get(item, path, None) for item in validated]


def mapping_scorer_id(row: Mapping[str, Any]) -> str:
    return str(row.get("scorer_id", row.get("scorer")))


def build_judgment_agreement(
    slot: tuple[str, str, str],
    mapping_rows: Sequence[Mapping[str, Any]],
    judgments_by_review: Mapping[str, Any],
    rubric_sha256: str,
    answer_contract: Mapping[str, Any],
    metric: Mapping[str, Any],
) -> dict[str, Any]:
    task_id, trial_id, arm = slot
    require(len(mapping_rows) == 3, f"slot {slot} must have exactly three mapped judgments")
    validated = []
    canonical_calls = canonical_navigation_call_identities(metric)
    for row in sorted(mapping_rows, key=lambda item: (mapping_scorer_id(item), str(item["review_id"]))):
        review_id = str(row["review_id"])
        validated.append(
            validate_judgment(
                judgments_by_review[review_id],
                expected_review_id=review_id,
                expected_scorer_id=mapping_scorer_id(row),
                rubric_sha256=rubric_sha256,
                answer_contract=answer_contract,
                canonical_calls=canonical_calls,
            )
        )

    scorer_ids = [item["scorer_id"] for item in validated]
    require(len(set(scorer_ids)) == 3, f"slot {slot} must have three unique scorers")
    for item_kind in ("required_claims", "required_relationships", "prohibited_claims"):
        sets = [item["item_sets"][item_kind] for item in validated]
        require(all(item_set == sets[0] for item_set in sets[1:]), f"slot {slot} {item_kind} item sets disagree")

    scores = _raw_component(validated, ("correctness", "score_0_100"))
    labels = _raw_component(validated, ("correctness", "semantic_label"))
    complete = _raw_component(validated, ("correctness", "contract_complete"))
    contract_labels = _raw_component(validated, ("correctness", "contract_label"))
    formats = _raw_component(validated, ("correctness", "format_valid_json"))
    score_summary = numeric_summary(scores, expected_n=3)

    component_disagreements: dict[str, Any] = {}
    for name in ("core_fraction", "grounding_fraction"):
        raw = _raw_component(validated, ("correctness", name))
        component_disagreements[name] = {"raw": raw, "disagreement": len(set(raw)) > 1}
    item_specs = (
        ("required_claims", "fraction"),
        ("required_relationships", "fraction"),
        ("prohibited_claims", "explicitly_present"),
    )
    for list_name, value_key in item_specs:
        by_scorer = []
        for item in validated:
            by_scorer.append({entry["item_id"]: entry[value_key] for entry in item["correctness"][list_name]})
        item_rows = {}
        for item_id in sorted(by_scorer[0]):
            raw = [values[item_id] for values in by_scorer]
            item_rows[item_id] = {"raw": raw, "disagreement": len(set(raw)) > 1}
        component_disagreements[list_name] = item_rows

    discovery = {}
    for stage in DISCOVERY_STAGES:
        raw = _raw_component(validated, ("dialogue", "discovery_stages", stage, "status"))
        majority = majority_of_three(raw, DISCOVERY_STATUSES, f"slot {slot} discovery {stage}")
        discovery[stage] = {
            "raw": raw,
            "majority": majority,
            "majority_status": "present" if majority is not None else "no_majority",
            "disagreement": len(set(raw)) > 1,
            "all_different": len(set(raw)) == 3,
        }
    first_wrong_records = [dict(item["dialogue"]["first_wrong"]) for item in validated]
    first_wrong = [item["category"] for item in first_wrong_records]
    atomic_boundaries = [
        (item["category"], item.get("completed_call_index"), item.get("call_id"), item.get("raw_event_line"))
        for item in first_wrong_records
    ]
    atomic_counts = Counter(atomic_boundaries)
    atomic_winners = [boundary for boundary, count in atomic_counts.items() if count >= 2]
    selected_boundary = atomic_winners[0] if len(atomic_winners) == 1 else None
    first_wrong_majority = selected_boundary[0] if selected_boundary is not None else None

    def position_summary(field_name: str) -> dict[str, Any]:
        raw = [item.get(field_name) for item in first_wrong_records]
        summary = numeric_summary(raw, expected_n=3)
        return {
            "raw": raw,
            "valid_n": summary["valid_n"],
            "missing_n": summary["missing_n"],
            "min": summary["min"],
            "max": summary["max"],
            "range": summary["range"],
        }

    selected_completed_call_index = selected_boundary[1] if selected_boundary is not None else None
    selected_call_id = selected_boundary[2] if selected_boundary is not None else None
    selected_raw_event_line = selected_boundary[3] if selected_boundary is not None else None
    first_wrong_agreement = {
        "raw": first_wrong_records,
        "raw_atomic_boundaries": [list(boundary) for boundary in atomic_boundaries],
        "raw_categories": first_wrong,
        "majority_category": first_wrong_majority,
        "majority_status": "present" if first_wrong_majority is not None else "no_majority",
        "category_disagreement": len(set(first_wrong)) > 1,
        "all_categories_different": len(set(first_wrong)) == 3,
        "raw_event_line_range": position_summary("raw_event_line"),
        "completed_call_index_range": position_summary("completed_call_index"),
        "selected_position": {
            "completed_call_index": selected_completed_call_index,
            "call_id": selected_call_id,
            "raw_event_line": selected_raw_event_line,
            "status": (
                "not_applicable_no_wrong"
                if first_wrong_majority == "none"
                else "selected_2_of_3"
                if selected_boundary is not None
                else "no_majority_position"
            ),
        },
    }

    any_component_disagreement = any(
        value["disagreement"] if "disagreement" in value else any(item["disagreement"] for item in value.values())
        for value in component_disagreements.values()
    )
    majority_label = majority_of_three(labels, SEMANTIC_LABELS, f"slot {slot} semantic_label")
    majority_complete = majority_of_three(complete, (True, False), f"slot {slot} contract_complete")
    majority_format = majority_of_three(formats, (True, False), f"slot {slot} format_valid_json")
    return {
        "task_id": task_id,
        "trial_id": trial_id,
        "arm": arm,
        "review_ids": [item["review_id"] for item in validated],
        "scorer_ids": scorer_ids,
        "raw_scores": scores,
        "raw_semantic_labels": labels,
        "raw_contract_complete": complete,
        "raw_contract_labels": contract_labels,
        "raw_format_valid_json": formats,
        "score_mean": score_summary["mean"],
        "score_min": score_summary["min"],
        "score_max": score_summary["max"],
        "score_range": score_summary["range"],
        "score_sample_sd": score_summary["sample_sd"],
        "majority_semantic_label": majority_label,
        "majority_semantic_label_status": "present" if majority_label is not None else "no_majority",
        "majority_contract_complete": majority_complete,
        "majority_contract_label": "complete" if majority_complete else "incomplete",
        "majority_format_valid_json": majority_format,
        "label_disagreement": len(set(labels)) > 1,
        "contract_complete_disagreement": len(set(complete)) > 1,
        "format_disagreement": len(set(formats)) > 1,
        "component_disagreements": component_disagreements,
        "any_component_disagreement": any_component_disagreement,
        "discovery_stages": discovery,
        "first_wrong_categories": first_wrong,
        "majority_first_wrong_category": first_wrong_majority,
        "majority_first_wrong_status": "present" if first_wrong_majority is not None else "no_majority",
        "first_wrong": first_wrong_agreement,
        "scorer_notes": [item["notes"] for item in validated],
    }


def _schedule_slot(row: Mapping[str, Any]) -> tuple[str, str, str]:
    task = row.get("task_id")
    trial = row.get("trial_id", row.get("repeat_id"))
    arm = row.get("criterion", row.get("arm"))
    require(isinstance(task, str) and isinstance(trial, str) and isinstance(arm, str), "schedule row identity is incomplete")
    return task, trial, arm


def validate_generation(generation: Any, rubric: Mapping[str, Any], rubric_sha256: str) -> tuple[list[str], dict[tuple[str, str, str], dict[str, Any]]]:
    generation = _mapping(generation, "generation")
    require(generation.get("schema_version") == 1, "generation schema_version mismatch")
    require(generation.get("generation_kind") == "baseline-3x", "generation kind must be baseline-3x")
    require(generation.get("execution_ready") is True, "generation is not an execution-ready sealed generation")
    seal = generation.get("generation_seal_sha256")
    require(isinstance(seal, str) and len(seal) == 64, "generation self-seal is absent")
    unsealed = dict(generation)
    unsealed.pop("generation_seal_sha256", None)
    require(canonical_sha256(unsealed) == seal, "generation self-seal mismatch")

    tasks = generation.get("tasks")
    require(isinstance(tasks, list) and len(tasks) == EXPECTED_TASKS and len(set(tasks)) == EXPECTED_TASKS, "generation must name 14 unique tasks")
    require(generation.get("trials") == list(EXPECTED_TRIALS), "generation trials must be r1-r3")
    require(generation.get("arms") == list(EXPECTED_ARMS), "generation arms must be B1 and B2")
    require(generation.get("task_count") == EXPECTED_TASKS, "generation task_count must be 14")
    require(generation.get("session_count") == EXPECTED_SESSIONS, "generation session_count must be 84")
    require(generation.get("judgment_count") == EXPECTED_SESSIONS * EXPECTED_JUDGMENTS_PER_OUTPUT, "generation judgment_count must be 252")

    schedule_list = _list(generation.get("schedule"), "generation.schedule")
    require(len(schedule_list) == EXPECTED_SESSIONS, "generation schedule must contain 84 rows")
    schedule: dict[tuple[str, str, str], dict[str, Any]] = {}
    for raw in schedule_list:
        row = _mapping(raw, "generation.schedule[]")
        slot = _schedule_slot(row)
        require(slot not in schedule, f"generation schedule has duplicate slot {slot}")
        schedule[slot] = dict(row)
    expected_slots = {(task, trial, arm) for task in tasks for trial in EXPECTED_TRIALS for arm in EXPECTED_ARMS}
    require(set(schedule) == expected_slots, "generation schedule is not the 14x3x2 Cartesian product")

    scoring = _mapping(generation.get("scoring_seal"), "generation.scoring_seal")
    require(scoring.get("judgments_per_output") == 3, "generation scoring seal must require three judgments")
    require(scoring.get("rubric_sha256") == rubric_sha256, "generation scoring seal/rubric hash mismatch")
    require(rubric.get("schema_version") == 1, "rubric schema_version mismatch")
    require(rubric.get("judgments_per_output") == 3, "rubric must require three judgments per output")
    require(rubric.get("formula") == "clamp(40*core + 30*mean(required_content) + 15*grounding + 15*mean(required_relationships) - min(40, 20*explicit_prohibited_claim_count), 0, 100)", "rubric formula is not the supported sealed formula")
    return [str(task) for task in tasks], schedule


def validate_ledger(
    ledger: Any,
    generation: Mapping[str, Any],
    schedule: Mapping[tuple[str, str, str], Mapping[str, Any]],
) -> dict[tuple[str, str, str], str]:
    ledger = _mapping(ledger, "ledger")
    require(ledger.get("state") == "completed", "ledger state must be completed")
    require(ledger.get("generation_id") == generation.get("generation_id"), "ledger/generation id mismatch")
    require(ledger.get("generation_seal_sha256") == generation.get("generation_seal_sha256"), "ledger/generation seal mismatch")
    slots = _mapping(ledger.get("slots"), "ledger.slots")
    require(len(slots) == EXPECTED_SESSIONS, "ledger must contain exactly 84 slots")
    result: dict[tuple[str, str, str], str] = {}
    run_ids: set[str] = set()
    for slot in schedule:
        key = ":".join(slot)
        require(key in slots, f"ledger is missing scheduled slot {key}")
        value = _mapping(slots[key], f"ledger.slots[{key}]")
        require(value.get("measurement_status") == "valid", f"ledger slot {key} is not valid")
        run_id = _string(value.get("latest_run_id"), f"ledger.slots[{key}].latest_run_id")
        require(run_id not in run_ids, f"ledger reuses latest run id {run_id}")
        run_ids.add(run_id)
        result[slot] = run_id
    require(set(slots) == {":".join(slot) for slot in schedule}, "ledger contains unscheduled slots")
    return result


def validate_ledger_attempts(
    ledger: Any,
    generation: Mapping[str, Any],
    schedule: Mapping[tuple[str, str, str], Mapping[str, Any]],
) -> dict[str, Any]:
    """Validate the full attempt union and return sealed, lossless denominators."""
    ledger = _mapping(ledger, "ledger")
    require(ledger.get("schema_version") == 1, "attempt accounting ledger schema_version mismatch")
    slots = _mapping(ledger.get("slots"), "ledger.slots")
    attempts = _mapping(ledger.get("attempts"), "ledger.attempts")
    require(attempts, "attempt accounting requires a non-empty attempts map")
    require(set(slots) == {":".join(slot) for slot in schedule}, "attempt accounting slot set mismatch")

    tracked_ids: set[str] = set()
    latest_ids: set[str] = set()
    tracked_slot_by_run: dict[str, str] = {}
    for slot, scheduled in schedule.items():
        slot_key = ":".join(slot)
        value = _mapping(slots[slot_key], f"ledger.slots[{slot_key}]")
        all_run_ids = _list(value.get("all_run_ids"), f"ledger.slots[{slot_key}].all_run_ids")
        require(all_run_ids, f"ledger slot {slot_key} has no tracked attempts")
        require(
            all(isinstance(run_id, str) and run_id for run_id in all_run_ids)
            and len(set(all_run_ids)) == len(all_run_ids),
            f"ledger slot {slot_key} tracked run ids are invalid",
        )
        latest_run_id = _string(value.get("latest_run_id"), f"ledger.slots[{slot_key}].latest_run_id")
        require(latest_run_id == all_run_ids[-1], f"ledger slot {slot_key} latest run is not the final tracked attempt")
        require(value.get("latest_attempt_number") == len(all_run_ids), f"ledger slot {slot_key} latest attempt number mismatch")
        require(value.get("measurement_status") == "valid", f"ledger slot {slot_key} latest measurement is not valid")
        require(value.get("metrics_status") == "sealed", f"ledger slot {slot_key} latest metrics are not sealed")
        require(value.get("replacement_allowed") is False, f"ledger slot {slot_key} latest attempt remains replaceable")
        require(not tracked_ids.intersection(all_run_ids), f"ledger reuses a tracked run id across slots: {slot_key}")
        tracked_ids.update(all_run_ids)
        latest_ids.add(latest_run_id)
        for expected_attempt_number, run_id in enumerate(all_run_ids, 1):
            attempt = _mapping(attempts.get(run_id), f"ledger.attempts[{run_id}]")
            require(
                attempt.get("attempt_number") == expected_attempt_number,
                f"ledger tracked attempt sequence mismatch: {run_id}",
            )
            tracked_slot_by_run[run_id] = slot_key
        latest = _mapping(attempts.get(latest_run_id), f"ledger.attempts[{latest_run_id}]")
        classification = _mapping(latest.get("classification"), f"ledger.attempts[{latest_run_id}].classification")
        require(latest.get("state") == "terminal", f"ledger latest attempt is not terminal: {latest_run_id}")
        require(latest.get("metrics_status") == "sealed", f"ledger latest attempt metrics are not sealed: {latest_run_id}")
        require(classification.get("measurement_status") == "valid", f"ledger latest attempt is not valid: {latest_run_id}")
        require(classification.get("replacement_allowed") is False, f"ledger latest attempt remains replaceable: {latest_run_id}")
        require(classification.get("generation_invalid") is False, f"ledger latest attempt invalidates generation: {latest_run_id}")
        require(
            (latest.get("task_id"), latest.get("trial_id"), latest.get("arm")) == slot,
            f"ledger latest attempt slot identity mismatch: {latest_run_id}",
        )
        require(latest.get("pair_id") == scheduled.get("pair_id"), f"ledger latest attempt pair id mismatch: {latest_run_id}")
        require(latest.get("pair_order_index") == scheduled.get("pair_order_index"), f"ledger latest attempt pair order mismatch: {latest_run_id}")

    canceled_ids = {
        run_id
        for run_id, raw_attempt in attempts.items()
        if isinstance(raw_attempt, Mapping) and raw_attempt.get("state") == "canceled-before-start"
    }
    require(set(attempts) == tracked_ids | canceled_ids, "ledger attempts are not the exact tracked/canceled union")
    require(not tracked_ids.intersection(canceled_ids), "ledger canceled attempt is still tracked")

    rows: list[dict[str, Any]] = []
    for run_id, raw_attempt in attempts.items():
        attempt = _mapping(raw_attempt, f"ledger.attempts[{run_id}]")
        require(attempt.get("run_id") == run_id, f"ledger attempt run identity mismatch: {run_id}")
        slot_key = _string(attempt.get("slot_key"), f"ledger.attempts[{run_id}].slot_key")
        slot = tuple(slot_key.split(":"))
        require(len(slot) == 3 and slot in schedule, f"ledger attempt has an unknown slot: {run_id}")
        scheduled = schedule[slot]
        arm = attempt.get("arm")
        attempt_number = attempt.get("attempt_number")
        state = attempt.get("state")
        require(
            (attempt.get("task_id"), attempt.get("trial_id"), arm) == slot,
            f"ledger attempt slot identity mismatch: {run_id}",
        )
        require(attempt.get("pair_id") == scheduled.get("pair_id"), f"ledger attempt pair id mismatch: {run_id}")
        require(attempt.get("pair_order_index") == scheduled.get("pair_order_index"), f"ledger attempt pair order mismatch: {run_id}")
        if run_id in tracked_ids:
            require(tracked_slot_by_run.get(run_id) == slot_key, f"ledger tracked attempt slot binding mismatch: {run_id}")
        require(arm in EXPECTED_ARMS, f"ledger attempt arm mismatch: {run_id}")
        require(
            isinstance(attempt_number, int) and not isinstance(attempt_number, bool) and attempt_number >= 1,
            f"ledger attempt number is invalid: {run_id}",
        )
        require(state in ATTEMPT_STATES, f"ledger attempt state is unsupported: {run_id}")
        classification = attempt.get("classification")
        if state == "terminal":
            require(run_id in tracked_ids, f"terminal ledger attempt is not tracked: {run_id}")
            require(attempt.get("metrics_status") == "sealed", f"terminal ledger attempt metrics are not sealed: {run_id}")
            classification = _mapping(classification, f"ledger.attempts[{run_id}].classification")
            measurement_status = classification.get("measurement_status")
            terminal_behavior = classification.get("terminal_behavior")
            replacement_allowed = classification.get("replacement_allowed")
            generation_invalid = classification.get("generation_invalid")
            replacement_category = classification.get("replacement_category")
            require(measurement_status in {"valid", "infrastructure_invalid"}, f"ledger attempt measurement status mismatch: {run_id}")
            require(terminal_behavior in TERMINAL_BEHAVIORS, f"ledger attempt terminal behavior mismatch: {run_id}")
            require(isinstance(replacement_allowed, bool), f"ledger attempt replacement flag mismatch: {run_id}")
            require(isinstance(generation_invalid, bool), f"ledger attempt generation flag mismatch: {run_id}")
            require(replacement_category in {*TRANSIENT_CATEGORIES, None}, f"ledger attempt replacement category mismatch: {run_id}")
        else:
            require(run_id not in tracked_ids, f"canceled ledger attempt remains tracked: {run_id}")
            require(classification is None, f"canceled ledger attempt must be unclassified: {run_id}")
            measurement_status = None
            terminal_behavior = None
            replacement_allowed = None
            replacement_category = None
        rows.append({
            "arm": arm,
            "state": state,
            "attempt_number": attempt_number,
            "is_latest_valid": run_id in latest_ids,
            "is_superseded": run_id in tracked_ids and run_id not in latest_ids,
            "measurement_status": measurement_status,
            "terminal_behavior": terminal_behavior,
            "replacement_allowed": replacement_allowed,
            "replacement_category": replacement_category,
        })

    def summarize(selected: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
        denominator = len(selected)
        measurement_counts = {
            "valid": sum(row["measurement_status"] == "valid" for row in selected),
            "infrastructure_invalid": sum(row["measurement_status"] == "infrastructure_invalid" for row in selected),
            "unclassified": sum(row["measurement_status"] is None for row in selected),
        }
        terminal_counts = {
            **{value: sum(row["terminal_behavior"] == value for row in selected) for value in TERMINAL_BEHAVIORS},
            "unclassified": sum(row["terminal_behavior"] is None for row in selected),
        }
        replacement_category_counts = {
            **{value: sum(row["replacement_category"] == value for row in selected) for value in TRANSIENT_CATEGORIES},
            "none": sum(row["measurement_status"] is not None and row["replacement_category"] is None for row in selected),
            "unclassified": sum(row["measurement_status"] is None for row in selected),
        }
        state_counts = {value: sum(row["state"] == value for row in selected) for value in ATTEMPT_STATES}
        return {
            "denominator_n": denominator,
            "raw_attempt_count": denominator,
            "latest_valid_count": sum(row["is_latest_valid"] for row in selected),
            "superseded_attempt_count": sum(row["is_superseded"] for row in selected),
            "invalid_attempt_count": measurement_counts["infrastructure_invalid"],
            "transient_attempt_count": sum(row["replacement_category"] in TRANSIENT_CATEGORIES for row in selected),
            "replacement_attempt_count": sum(row["attempt_number"] > 1 for row in selected),
            "canceled_before_start_count": state_counts["canceled-before-start"],
            "measurement_status_counts": measurement_counts,
            "terminal_behavior_counts": terminal_counts,
            "replacement_category_counts": replacement_category_counts,
            "attempt_state_counts": state_counts,
        }

    overall = summarize(rows)
    by_arm = [{"arm": arm, **summarize([row for row in rows if row["arm"] == arm])} for arm in EXPECTED_ARMS]
    require(overall["latest_valid_count"] == EXPECTED_SESSIONS, "ledger latest valid attempt count must be 84")
    require([row["latest_valid_count"] for row in by_arm] == [42, 42], "ledger latest valid attempt counts must be B1=42 and B2=42")
    return {
        "raw_attempt_count": overall["raw_attempt_count"],
        "latest_valid_count": overall["latest_valid_count"],
        "superseded_attempt_count": overall["superseded_attempt_count"],
        "invalid_attempt_count": overall["invalid_attempt_count"],
        "transient_attempt_count": overall["transient_attempt_count"],
        "replacement_attempt_count": overall["replacement_attempt_count"],
        "overall": overall,
        "by_arm": by_arm,
    }


def _metric_identity(metric: Mapping[str, Any]) -> tuple[Any, Any, Any]:
    experiment = _mapping(metric.get("experiment"), "metric.experiment")
    return experiment.get("task_id"), experiment.get("trial_id"), experiment.get("arm")


def unwrap_metric_record(value: Any, expected_run_id: str) -> tuple[Mapping[str, Any], str]:
    """Accept only the sealed schema-v2 extractor record contract."""
    record = _mapping(value, f"metric index entry {expected_run_id}")
    if record.get("input_contract") == "baseline-final-session-record-v1":
        raise AggregationError(
            f"legacy final session envelope {expected_run_id} is unsupported; schema-v2 extractor records are required"
        )
    return record, "extractor_record"


def validate_automatic_metric_consistency(metric: Mapping[str, Any], run_id: str) -> None:
    run = _mapping(metric.get("run"), f"metric[{run_id}].run")
    experiment = _mapping(metric.get("experiment"), f"metric[{run_id}].experiment")
    integrity = _mapping(run.get("integrity"), f"metric[{run_id}].run.integrity")
    lifecycle = _mapping(run.get("lifecycle"), f"metric[{run_id}].run.lifecycle")
    classification = _mapping(lifecycle.get("classification"), f"metric[{run_id}].run.lifecycle.classification")
    publication = _mapping(lifecycle.get("publication"), f"metric[{run_id}].run.lifecycle.publication")
    artifact_seal = _mapping(lifecycle.get("artifact_seal"), f"metric[{run_id}].run.lifecycle.artifact_seal")

    require(integrity.get("status") == "verified", f"extractor metric {run_id} integrity status is not verified")
    require(integrity.get("issues") == [], f"extractor metric {run_id} integrity issues are not empty")
    require(integrity.get("aggregation_eligible") is True, f"extractor metric {run_id} integrity is ineligible")
    require(integrity.get("aggregation_ineligible_reasons") == [], f"extractor metric {run_id} has aggregation-ineligible reasons")
    require(experiment.get("aggregation_eligible") is True, f"extractor metric {run_id} experiment is aggregation-ineligible")
    require(experiment.get("aggregation_ineligible_reasons") == [], f"extractor metric {run_id} experiment has ineligible reasons")
    require(experiment.get("measurement_status") == "valid", f"extractor metric {run_id} measurement is not valid")
    require(experiment.get("generation_invalid") is False, f"extractor metric {run_id} invalidates the generation")
    require(experiment.get("replacement_allowed") is False, f"extractor metric {run_id} remains replaceable")
    require(experiment.get("published") is True, f"extractor metric {run_id} is not published")
    require(experiment.get("latest_published_attempt") is True, f"extractor metric {run_id} is not the latest published attempt")
    require(experiment.get("artifact_verified") is True, f"extractor metric {run_id} artifact is not verified")
    require(classification.get("measurement_status") == "valid", f"extractor metric {run_id} classification is not valid")
    require(classification.get("generation_invalid") is False, f"extractor metric {run_id} classification invalidates generation")
    require(classification.get("replacement_allowed") is False, f"extractor metric {run_id} classification remains replaceable")
    for field in (
        "ledger_present", "generation_match", "attempt_present", "attempt_state_terminal", "run_dir_match",
        "artifact_manifest_hash_match", "classification_match", "attempt_number_match", "slot_present",
        "latest_run_id_match", "latest_attempt_number_match", "slot_measurement_status_match", "published",
        "latest_published_attempt",
    ):
        require(publication.get(field) is True, f"extractor metric {run_id} publication gate {field} is false")
    require(artifact_seal.get("verified") is True, f"extractor metric {run_id} artifact seal is not verified")
    require(artifact_seal.get("issues") == [], f"extractor metric {run_id} artifact seal issues are not empty")
    require(artifact_seal.get("writable_paths") == [], f"extractor metric {run_id} contains writable artifacts")
    symlink_paths = _list(artifact_seal.get("symlink_paths"), f"metric[{run_id}].run.lifecycle.artifact_seal.symlink_paths")
    require(
        all(isinstance(path, str) and path for path in symlink_paths),
        f"extractor metric {run_id} contains invalid verified symlink paths",
    )

    calls = _list(run.get("completed_tool_calls"), f"metric[{run_id}].run.completed_tool_calls")
    require(run.get("incomplete_tool_calls") == [], f"extractor metric {run_id} has incomplete tool calls")
    tool_integrity = _mapping(run.get("tool_identity_integrity"), f"metric[{run_id}].run.tool_identity_integrity")
    require(tool_integrity.get("authoritative_completed_ids_available") is True, f"extractor metric {run_id} lacks authoritative completed tool identities")
    require(tool_integrity.get("authoritative_error_ids_available") is True, f"extractor metric {run_id} lacks authoritative error identities")
    require(tool_integrity.get("raw_terminal_identity_match") is True, f"extractor metric {run_id} raw terminal identities mismatch")
    require(tool_integrity.get("completed_count") == len(calls), f"extractor metric {run_id} completed identity count mismatch")
    require(tool_integrity.get("incomplete_count") == 0, f"extractor metric {run_id} incomplete identity count mismatch")
    require(tool_integrity.get("malformed_revision_count") == 0, f"extractor metric {run_id} has malformed tool revisions")
    require(tool_integrity.get("conflicting_call_count") == 0, f"extractor metric {run_id} has conflicting tool calls")
    identities = [call.get("identity") for call in calls if isinstance(call, Mapping)]
    require(len(identities) == len(calls) and len(set(identities)) == len(calls), f"extractor metric {run_id} tool identities are invalid")
    require(all(call.get("completed") is True for call in calls), f"extractor metric {run_id} contains a non-completed cost call")
    completed_indexes = [call.get("completed_call_index") for call in calls]
    require(completed_indexes == list(range(1, len(calls) + 1)), f"extractor metric {run_id} completed call indexes are not contiguous")
    for call in calls:
        require(
            call.get("identity") == f"{call.get('session_id')}:tool:{call.get('call_id')}",
            f"extractor metric {run_id} tool identity components mismatch",
        )
        start = call.get("client_observed_start_ms")
        end = call.get("client_observed_end_ms")
        elapsed_value = call.get("client_observed_elapsed_ms")
        if start is None or end is None:
            require(elapsed_value is None, f"extractor metric {run_id} tool elapsed time lacks a complete boundary")
        else:
            require(is_number(start) and is_number(end) and end >= start, f"extractor metric {run_id} tool timing boundary is invalid")
            require(elapsed_value == end - start, f"extractor metric {run_id} tool elapsed time mismatch")

    cost = _mapping(metric.get("cost"), f"metric[{run_id}].cost")
    require(cost.get("tool_calls_total") == len(calls), f"extractor metric {run_id} tool call total mismatch")
    family_counts = Counter(str(call["family"]) for call in calls)
    require(cost.get("tool_calls_by_family") == dict(sorted(family_counts.items())), f"extractor metric {run_id} tool family counts mismatch")
    navigation_count = sum(call["family"] in NAVIGATION_FAMILIES for call in calls)
    require(cost.get("navigation_calls_excluding_handshake") == navigation_count, f"extractor metric {run_id} navigation count mismatch")

    byte_contract = _mapping(cost.get("tool_byte_measurement"), f"metric[{run_id}].cost.tool_byte_measurement")
    for value_name, call_name, complete_name in (
        ("tool_input_bytes", "input_utf8_bytes", "input_complete"),
        ("tool_output_bytes", "output_utf8_bytes", "output_complete"),
    ):
        values = [call.get(call_name) for call in calls]
        complete = all(isinstance(value, int) and not isinstance(value, bool) and value >= 0 for value in values)
        require(byte_contract.get(complete_name) is complete, f"extractor metric {run_id} {complete_name} mismatch")
        expected = sum(values) if complete else None
        require(cost.get(value_name) == expected, f"extractor metric {run_id} {value_name} mismatch")

    timing = _mapping(cost.get("client_observed_tool_ms"), f"metric[{run_id}].cost.client_observed_tool_ms")
    elapsed = [call.get("client_observed_elapsed_ms") for call in calls]
    timing_complete = all(is_number(value) and value >= 0 for value in elapsed)
    require(timing.get("complete") is timing_complete, f"extractor metric {run_id} client tool timing completeness mismatch")
    expected_timing_total = sum(elapsed) if timing_complete else None
    require(timing.get("total") == expected_timing_total, f"extractor metric {run_id} client tool timing total mismatch")
    if timing_complete:
        timing_by_family: dict[str, int | float] = defaultdict(int)
        for call, value in zip(calls, elapsed):
            timing_by_family[str(call["family"])] += value
        require(timing.get("by_family") == dict(sorted(timing_by_family.items())), f"extractor metric {run_id} client tool family timing mismatch")
    else:
        require(timing.get("by_family") is None, f"extractor metric {run_id} incomplete timing must have null family totals")

    error_identities = [call["identity"] for call in calls if call.get("is_error") is True]
    tool_errors = _mapping(cost.get("tool_errors"), f"metric[{run_id}].cost.tool_errors")
    require(tool_errors.get("count") == len(error_identities), f"extractor metric {run_id} tool error count mismatch")
    require(tool_errors.get("identities") == error_identities, f"extractor metric {run_id} tool error identities mismatch")
    protocol_errors = _mapping(cost.get("protocol_errors"), f"metric[{run_id}].cost.protocol_errors")
    protocol_items = protocol_errors.get("items")
    require(isinstance(protocol_items, list), f"extractor metric {run_id} protocol error identities are unavailable")
    require(protocol_errors.get("count") == len(protocol_items), f"extractor metric {run_id} protocol error count mismatch")

    tokens = _mapping(cost.get("tokens"), f"metric[{run_id}].cost.tokens")
    steps = _list(tokens.get("per_step"), f"metric[{run_id}].cost.tokens.per_step")
    require(tokens.get("official_entries") == len(steps), f"extractor metric {run_id} official token entry count mismatch")
    require(cost.get("model_steps") == len(steps), f"extractor metric {run_id} model step count mismatch")
    require(tokens.get("identity_set_matches_completed_models") is True, f"extractor metric {run_id} token identity set mismatch")
    token_identities = [step.get("identity") for step in steps if isinstance(step, Mapping)]
    require(len(token_identities) == len(steps) and len(set(token_identities)) == len(steps), f"extractor metric {run_id} token identities are invalid")
    for index, step in enumerate(steps):
        components = [step.get(component) for component in TOKEN_COMPONENTS[:-1]]
        complete_components = all(
            isinstance(value, int) and not isinstance(value, bool) and value >= 0
            for value in components
        )
        component_sum = sum(components) if complete_components else None
        require(step.get("component_sum") == component_sum, f"extractor metric {run_id} token step {index} component sum mismatch")
        require(step.get("total") == component_sum, f"extractor metric {run_id} token step {index} total mismatch")
        require(
            step.get("official_total_matches_components") is (True if complete_components else None),
            f"extractor metric {run_id} token step {index} total flag mismatch",
        )
        require(step.get("conflicts") == [], f"extractor metric {run_id} token step {index} has conflicts")
    for component in TOKEN_COMPONENTS:
        component_values = [step.get(component) for step in steps]
        expected_component = (
            sum(component_values)
            if component_values
            and all(isinstance(value, int) and not isinstance(value, bool) and value >= 0 for value in component_values)
            else None
        )
        require(tokens.get(component) == expected_component, f"extractor metric {run_id} aggregate token {component} mismatch")

    repeats = _mapping(metric.get("repeated_searches"), f"metric[{run_id}].repeated_searches")
    groups = _list(repeats.get("groups"), f"metric[{run_id}].repeated_searches.groups")
    require(repeats.get("group_count") == len(groups), f"extractor metric {run_id} repeated-search group count mismatch")
    require(repeats.get("extra_call_count") == sum(group["extra_call_count"] for group in groups), f"extractor metric {run_id} repeated-search extra count mismatch")
    unbounded = _mapping(metric.get("unbounded_reads"), f"metric[{run_id}].unbounded_reads")
    unbounded_calls = _list(unbounded.get("calls"), f"metric[{run_id}].unbounded_reads.calls")
    require(unbounded.get("count") == len(unbounded_calls), f"extractor metric {run_id} unbounded-read count mismatch")
    unbounded_bytes = [call.get("output_utf8_bytes") for call in unbounded_calls]
    expected_unbounded_bytes = sum(unbounded_bytes) if all(isinstance(value, int) and not isinstance(value, bool) and value >= 0 for value in unbounded_bytes) else None
    require(unbounded.get("output_bytes") == expected_unbounded_bytes, f"extractor metric {run_id} unbounded-read bytes mismatch")


def validate_metric(
    metric: Any,
    *,
    run_id: str,
    slot: tuple[str, str, str],
    schedule: Mapping[str, Any],
    generation: Mapping[str, Any],
    record_kind: str,
    automatic_schema: Mapping[str, Any],
) -> dict[str, Any]:
    metric = _mapping(metric, f"metric[{run_id}]")
    metric_schema_version = metric.get("schema_version")
    require(record_kind == "extractor_record", f"metric {run_id} is not a schema-v2 extractor record")
    require(metric_schema_version == 2, f"extractor metric {run_id} schema_version must be 2")
    schema_errors = validate_json_schema(metric, automatic_schema)
    require(not schema_errors, f"extractor metric {run_id} violates automatic schema: {schema_errors[0] if schema_errors else ''}")
    require(_metric_identity(metric) == slot, f"metric {run_id} slot identity mismatch")
    experiment = _mapping(metric["experiment"], f"metric[{run_id}].experiment")
    require(experiment.get("pair_id") == schedule.get("pair_id"), f"metric {run_id} pair_id mismatch")
    require(experiment.get("pair_order_index") == schedule.get("pair_order_index"), f"metric {run_id} pair order mismatch")
    require(experiment.get("measurement_status") == "valid", f"metric {run_id} is not marked valid")
    recorded_run_id = deep_get(metric, ("run", "run_id"), MISSING)
    require(recorded_run_id == run_id, f"extractor metric {run_id} must embed the latest run id")
    run = _mapping(metric.get("run"), f"metric[{run_id}].run")
    require(run.get("generation_id") == generation.get("generation_id"), f"extractor metric {run_id} generation id mismatch")
    require(
        isinstance(run.get("attempt_number"), int)
        and not isinstance(run.get("attempt_number"), bool)
        and run["attempt_number"] >= 1,
        f"extractor metric {run_id} attempt_number is invalid",
    )
    validate_automatic_metric_consistency(metric, run_id)

    expected_question = schedule.get("question_sha256")
    if expected_question is not None:
        require(experiment.get("question_sha256") == expected_question, f"metric {run_id} question hash mismatch")
    prompt_by_task = generation.get("prompt_sha256_by_task")
    if isinstance(prompt_by_task, Mapping):
        require(experiment.get("prompt_sha256") == prompt_by_task[slot[0]], f"metric {run_id} prompt hash mismatch")
    expected_source = deep_get(generation, ("source", "tree_sha256"), MISSING)
    metric_source = first_present(experiment.get("corpus_tree_sha256", MISSING), experiment.get("source_tree_sha256", MISSING))
    if expected_source is not MISSING:
        require(metric_source == expected_source, f"metric {run_id} source tree hash mismatch")
    if generation.get("model") is not None:
        require(experiment.get("model") == generation.get("model"), f"metric {run_id} model mismatch")
    arm_config_hashes = deep_get(generation, ("b2", "materialized_config_file_sha256"), MISSING)
    if arm_config_hashes is not MISSING:
        arm_config_hashes = _mapping(arm_config_hashes, "generation.b2.materialized_config_file_sha256")
        require(set(arm_config_hashes) == set(EXPECTED_ARMS), "generation materialized arm config hash set mismatch")
        require(
            experiment.get("arm_config_sha256") == arm_config_hashes[slot[2]],
            f"metric {run_id} materialized arm config hash mismatch",
        )

    for required in ("dialogue", "cost", "repeated_searches", "unbounded_reads", "post_first_wrong"):
        require(isinstance(metric.get(required), Mapping), f"metric {run_id} missing object {required}")
    return dict(metric)


def validate_mapping_and_judgments(
    mapping_value: Any,
    judgments_by_review: Mapping[str, Any],
    latest_run_by_slot: Mapping[tuple[str, str, str], str],
    schedule: Mapping[tuple[str, str, str], Mapping[str, Any]],
    generation: Mapping[str, Any],
    expected_runs_root: Path,
) -> dict[tuple[str, str, str], list[dict[str, Any]]]:
    mapping_object = _mapping(mapping_value, "coordinator mapping")
    require(mapping_object.get("schema_version") == 1, "coordinator mapping schema_version mismatch")
    rows = _list(mapping_object.get("assignments", mapping_object.get("mapping")), "coordinator mapping assignments")
    require(len(rows) == EXPECTED_SESSIONS * 3, "coordinator mapping must contain exactly 252 rows")
    expected_runs_root = expected_runs_root.absolute()
    require(expected_runs_root.is_dir(), "expected runs root must exist")
    require(not path_has_symlink(expected_runs_root), "expected runs root must not contain a symlink")
    generation_runs_root = expected_runs_root / str(generation.get("generation_id"))
    require(generation_runs_root.is_dir(), "expected generation runs root must exist")
    if "assignment_seal_sha256" in mapping_object:
        core = dict(mapping_object)
        recorded_seal = core.pop("assignment_seal_sha256")
        require(recorded_seal == canonical_sha256(core), "coordinator assignment self-seal mismatch")
        require(mapping_object.get("generation_id") == generation.get("generation_id"), "coordinator assignment generation id mismatch")
        require(mapping_object.get("generation_seal_sha256") == generation.get("generation_seal_sha256"), "coordinator assignment generation seal mismatch")
        require(Path(str(mapping_object.get("runs_root"))) == generation_runs_root, "coordinator assignment runs root mismatch")
    by_slot: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    review_ids: set[str] = set()
    scorer_slots: set[tuple[tuple[str, str, str], str]] = set()
    for index, raw in enumerate(rows):
        row = _mapping(raw, f"coordinator mapping.mapping[{index}]")
        review_id = _string(row.get("review_id"), f"mapping[{index}].review_id")
        scorer = _string(row.get("scorer_id", row.get("scorer")), f"mapping[{index}].scorer_id")
        slot = (row.get("task_id"), row.get("trial_id"), row.get("arm"))
        require(slot in latest_run_by_slot, f"mapping row {review_id} names an unscheduled slot")
        require(review_id not in review_ids, f"duplicate review id {review_id}")
        require((slot, scorer) not in scorer_slots, f"duplicate scorer {scorer} for slot {slot}")
        require(row.get("pair_id") == schedule[slot].get("pair_id"), f"mapping row {review_id} pair_id mismatch")
        require(
            row.get("pair_order_index") == schedule[slot].get("pair_order_index"),
            f"mapping row {review_id} pair_order_index mismatch",
        )
        review_ids.add(review_id)
        scorer_slots.add((slot, scorer))
        run_dir = _string(row.get("run_dir"), f"mapping[{index}].run_dir")
        run_path = Path(run_dir)
        expected_run_path = generation_runs_root / latest_run_by_slot[slot]
        require(run_path.is_absolute(), f"mapping row {review_id} run_dir must be absolute")
        require(not path_has_symlink(run_path), f"mapping row {review_id} run_dir contains a symlink")
        require(run_path.is_dir(), f"mapping row {review_id} run_dir does not exist")
        require(str(run_path) == str(expected_run_path), f"mapping row {review_id} run_dir is not the exact expected path")
        require(run_path.resolve() == expected_run_path.resolve(), f"mapping row {review_id} run_dir resolves outside expected runs root")
        by_slot[slot].append(dict(row))

    require(set(by_slot) == set(latest_run_by_slot), "mapping slot set does not match ledger")
    require(all(len(rows_for_slot) == 3 for rows_for_slot in by_slot.values()), "every slot must map to exactly three reviews")
    scorer_sets = {tuple(sorted(mapping_scorer_id(row) for row in rows_for_slot)) for rows_for_slot in by_slot.values()}
    require(len(scorer_sets) == 1 and len(next(iter(scorer_sets))) == 3, "all slots must use the same three unique scorer ids")
    require(set(judgments_by_review) == review_ids, "judgment index must exactly match the 252 current review ids")
    return by_slot


def _count_field(value: Any, field: str) -> int | float | None:
    if isinstance(value, Mapping):
        value = value.get("count", MISSING)
    return optional_nonnegative_integer(value, field)


def derive_post_first_wrong(metric: Mapping[str, Any], agreement: Mapping[str, Any]) -> dict[str, Any]:
    """Derive follow-up cost only behind an explicit 2-of-3 process gate."""
    first_wrong = _mapping(agreement.get("first_wrong"), "agreement.first_wrong")
    category = first_wrong.get("majority_category")
    selected = _mapping(first_wrong.get("selected_position"), "agreement.first_wrong.selected_position")
    null_values = {
        "tool_calls": None,
        "tool_output_bytes": None,
        "tokens": None,
        "wall_ms": None,
        "tool_errors": None,
        "repeated_extra_calls": None,
        "unbounded_read_calls": None,
    }
    def component_denominators(values: Mapping[str, Any], eligible_n: int) -> dict[str, dict[str, int]]:
        return {
            field: {"eligible_n": eligible_n, "valid_n": int(eligible_n == 1 and value is not None), "missing_n": int(eligible_n == 1 and value is None)}
            for field, value in values.items()
        }
    if category is None:
        return {"values": null_values, "component_denominators": component_denominators(null_values, 0), "status": "no_majority_category", "warning": "post_first_wrong_not_computed_without_2_of_3_category"}
    if category == "none":
        return {"values": null_values, "component_denominators": component_denominators(null_values, 0), "status": "not_applicable_no_wrong", "warning": None}
    if selected.get("status") != "selected_2_of_3":
        return {"values": null_values, "component_denominators": component_denominators(null_values, 0), "status": "no_majority_position", "warning": "post_first_wrong_not_computed_without_2_of_3_position"}

    run_calls = deep_get(metric, ("run", "tool_calls"), MISSING)
    if run_calls is MISSING:
        run_calls = deep_get(metric, ("run", "completed_tool_calls"), MISSING)
    if run_calls is MISSING:
        run_calls = deep_get(metric, ("cost", "calls"), MISSING)
    calls = run_calls if isinstance(run_calls, list) else None
    selected_index = selected.get("completed_call_index")
    selected_call_id = selected.get("call_id")
    selected_line = selected.get("raw_event_line")
    selected_call: Mapping[str, Any] | None = None
    if calls is not None:
        selected_call = next(
            (
                call for call in calls
                if isinstance(call, Mapping)
                and call.get("completed") is True
                and call.get("family") in NAVIGATION_FAMILIES
                and call.get("completed_call_index") == selected_index
                and call.get("call_id") == selected_call_id
                and call.get("selected_completion_line") == selected_line
            ),
            None,
        )
    require(selected_call is not None, "post-first-wrong boundary is not one canonical completed navigation call")

    post_record = _mapping(metric.get("post_first_wrong"), "metric.post_first_wrong")

    def recorded(field: str, alias: str | None = None) -> int | float | None:
        candidates = [post_record.get(field, MISSING)]
        if alias is not None:
            candidates.append(post_record.get(alias, MISSING))
        value = first_present(*candidates)
        if field == "wall_ms":
            return optional_nonnegative_number(value, f"post_first_wrong.{field}")
        return optional_nonnegative_integer(value, f"post_first_wrong.{field}")

    values = {
        "tool_calls": recorded("tool_calls", "tool_calls_after"),
        "tool_output_bytes": recorded("tool_output_bytes", "tool_output_bytes_after"),
        "tokens": recorded("tokens", "tokens_after"),
        "wall_ms": recorded("wall_ms", "wall_ms_after"),
        "tool_errors": recorded("tool_errors"),
        "repeated_extra_calls": recorded("repeated_extra_calls"),
        "unbounded_read_calls": recorded("unbounded_read_calls"),
    }
    derivation_sources = {field: "recorded_final_session" if value is not None else None for field, value in values.items()}

    after_calls: list[Mapping[str, Any]] | None = None
    if calls is not None and selected_index is not None:
        indexed_calls = [
            call
            for call in calls
            if isinstance(call, Mapping) and isinstance(call.get("completed_call_index"), int)
        ]
        if len(indexed_calls) == len(calls):
            after_calls = [call for call in indexed_calls if call["completed_call_index"] > selected_index]
            values["tool_calls"] = len(after_calls)
            derivation_sources["tool_calls"] = "deduplicated_calls_after_selected_completed_call_index"
            output_values = [optional_number(call.get("output_utf8_bytes", MISSING), "run.tool_calls[].output_utf8_bytes") for call in after_calls]
            values["tool_output_bytes"] = sum(output_values) if all(value is not None for value in output_values) else None
            derivation_sources["tool_output_bytes"] = "exact_sum_after_selected_completed_call_index" if values["tool_output_bytes"] is not None else None
            error_values = [call.get("is_error", MISSING) for call in after_calls]
            if all(isinstance(value, bool) for value in error_values):
                values["tool_errors"] = sum(value is True for value in error_values)
                derivation_sources["tool_errors"] = "deduplicated_error_calls_after_selected_completed_call_index"

    repeated_groups = deep_get(metric, ("repeated_searches", "groups"), MISSING)
    if isinstance(repeated_groups, list) and selected_index is not None:
        repeated_after = 0
        repeat_exact = True
        for group in repeated_groups:
            if not isinstance(group, Mapping) or not isinstance(group.get("calls"), list):
                repeat_exact = False
                break
            indexes = sorted(
                call.get("completed_call_index")
                for call in group["calls"]
                if isinstance(call, Mapping) and isinstance(call.get("completed_call_index"), int)
            )
            if len(indexes) != len(group["calls"]):
                repeat_exact = False
                break
            repeated_after += sum(index > selected_index for index in indexes[1:])
        if repeat_exact:
            values["repeated_extra_calls"] = repeated_after
            derivation_sources["repeated_extra_calls"] = "repeat_candidates_after_selected_completed_call_index"

    unbounded_calls = deep_get(metric, ("unbounded_reads", "calls"), MISSING)
    if isinstance(unbounded_calls, list) and selected_index is not None:
        indexes = [call.get("completed_call_index") for call in unbounded_calls if isinstance(call, Mapping)]
        if len(indexes) == len(unbounded_calls) and all(isinstance(index, int) for index in indexes):
            values["unbounded_read_calls"] = sum(index > selected_index for index in indexes)
            derivation_sources["unbounded_read_calls"] = "unbounded_reads_after_selected_completed_call_index"

    per_step = deep_get(metric, ("cost", "tokens", "per_step"), MISSING)
    if isinstance(per_step, list) and selected_line is not None:
        valid_steps = [step for step in per_step if isinstance(step, Mapping)]
        if len(valid_steps) == len(per_step) and all(isinstance(step.get("raw_event_line"), int) for step in valid_steps):
            after_steps = [step for step in valid_steps if step["raw_event_line"] > selected_line]
            totals = [optional_number(step.get("total", MISSING), "cost.tokens.per_step[].total") for step in after_steps]
            values["tokens"] = sum(totals) if all(value is not None for value in totals) else None
            derivation_sources["tokens"] = "official_token_steps_after_selected_raw_event_line" if values["tokens"] is not None else None

    # Current extractor records total command wall time but no trustworthy wall
    # timeline boundary.  Never substitute summed tool latency for wall time.
    warning = None
    if values["wall_ms"] is None:
        warning = "post_first_wrong_wall_unavailable_without_timeline_boundary"
    return {
        "values": values,
        "component_denominators": component_denominators(values, 1),
        "status": "computed_where_supported",
        "warning": warning,
        "selected_completed_call_index": selected_index,
        "selected_raw_event_line": selected_line,
        "derivation_sources": derivation_sources,
    }


def derive_navigation_behavior(metric: Mapping[str, Any]) -> dict[str, Any]:
    """Preserve the exact completed navigation sequence and derive bounded summaries."""
    calls_value = first_present(
        deep_get(metric, ("run", "completed_tool_calls"), MISSING),
        deep_get(metric, ("run", "tool_calls"), MISSING),
        deep_get(metric, ("cost", "calls"), MISSING),
    )
    if calls_value is MISSING or calls_value is None:
        calls: list[Mapping[str, Any]] = []
    else:
        raw_calls = _list(calls_value, "metric.run.completed_tool_calls")
        require(all(isinstance(call, Mapping) for call in raw_calls), "completed tool calls must contain objects")
        calls = [call for call in raw_calls if isinstance(call, Mapping)]

    completed_family_sequence = [call.get("family") for call in calls]
    require(
        all(isinstance(family, str) for family in completed_family_sequence),
        "completed tool calls must expose a family",
    )
    navigation_calls = [call for call in calls if call.get("family") in NAVIGATION_FAMILIES]

    def origin(call: Mapping[str, Any]) -> str:
        tool = call.get("tool")
        return "codemap" if isinstance(tool, str) and tool.casefold().startswith("codemap_search_") else "builtin"

    navigation_origins = [origin(call) for call in navigation_calls]
    navigation_sequence = [
        {
            "identity": call.get("identity"),
            "completed_call_index": call.get("completed_call_index"),
            "tool": call.get("tool"),
            "family": call.get("family"),
            "origin": origin(call),
        }
        for call in navigation_calls
    ]
    first = navigation_calls[0] if navigation_calls else None
    codemap_first = next((call for call in navigation_calls if origin(call) == "codemap"), None)
    switch_count = sum(
        previous != current
        for previous, current in zip(navigation_origins, navigation_origins[1:])
    )
    return {
        "completed_tool_family_sequence": completed_family_sequence,
        "completed_navigation_sequence": navigation_sequence,
        "first_navigation_tool_family": first.get("family") if first is not None else None,
        "first_navigation_tool_origin": origin(first) if first is not None else None,
        "first_navigation_completed_call_index": optional_nonnegative_integer(
            first.get("completed_call_index", MISSING) if first is not None else None,
            "navigation.first_completed_call_index",
        ),
        "first_navigation_input_bytes": optional_nonnegative_integer(
            first.get("input_utf8_bytes", MISSING) if first is not None else None,
            "navigation.first_input_bytes",
        ),
        "first_navigation_output_bytes": optional_nonnegative_integer(
            first.get("output_utf8_bytes", MISSING) if first is not None else None,
            "navigation.first_output_bytes",
        ),
        "first_navigation_client_observed_ms": optional_nonnegative_number(
            first.get("client_observed_elapsed_ms", MISSING) if first is not None else None,
            "navigation.first_client_observed_ms",
        ),
        "codemap_first_use_completed_call_index": optional_nonnegative_integer(
            codemap_first.get("completed_call_index", MISSING) if codemap_first is not None else None,
            "navigation.codemap_first_use_completed_call_index",
        ),
        "origin_switch_count": switch_count,
        "raw_query_and_range_source": "automatic_metrics.run.completed_tool_calls",
        "manual_first_wrong_linkage_source": "judgment_agreement.first_wrong",
    }


def normalize_session(metric: Mapping[str, Any], agreement: Mapping[str, Any]) -> dict[str, Any]:
    """Normalize one validated schema-v2 extractor record."""
    cost = _mapping(metric.get("cost"), "metric.cost")
    tokens = _mapping(cost.get("tokens"), "metric.cost.tokens")
    client_tool = cost.get("client_observed_tool_ms", MISSING)
    if isinstance(client_tool, Mapping):
        client_tool = client_tool.get("total", MISSING)
    repeated = _mapping(metric.get("repeated_searches"), "metric.repeated_searches")
    unbounded = _mapping(metric.get("unbounded_reads"), "metric.unbounded_reads")
    dialogue = _mapping(metric.get("dialogue"), "metric.dialogue")
    selection = dialogue.get("codemap_tool_selection", {})
    require(isinstance(selection, Mapping), "metric.dialogue.codemap_tool_selection must be an object when present")
    post_derivation = derive_post_first_wrong(metric, agreement)
    post_values = post_derivation["values"]
    navigation = derive_navigation_behavior(metric)

    values: dict[str, int | float | None] = {
        "quality.score_mean": optional_number(agreement.get("score_mean", MISSING), "agreement.score_mean"),
        "quality.score_min": optional_number(agreement.get("score_min", MISSING), "agreement.score_min"),
        "quality.score_max": optional_number(agreement.get("score_max", MISSING), "agreement.score_max"),
        "quality.score_range": optional_number(agreement.get("score_range", MISSING), "agreement.score_range"),
        "cost.model_steps": optional_nonnegative_integer(cost.get("model_steps", MISSING), "cost.model_steps"),
        "cost.tool_calls_total": optional_nonnegative_integer(cost.get("tool_calls_total", MISSING), "cost.tool_calls_total"),
        "cost.navigation_calls_excluding_handshake": optional_nonnegative_integer(cost.get("navigation_calls_excluding_handshake", MISSING), "cost.navigation_calls_excluding_handshake"),
        "cost.tool_input_bytes": optional_nonnegative_integer(cost.get("tool_input_bytes", MISSING), "cost.tool_input_bytes"),
        "cost.tool_output_bytes": optional_nonnegative_integer(cost.get("tool_output_bytes", MISSING), "cost.tool_output_bytes"),
        "cost.captured_stdout_bytes": optional_nonnegative_integer(cost.get("captured_stdout_bytes", MISSING), "cost.captured_stdout_bytes"),
        "cost.captured_stderr_bytes": optional_nonnegative_integer(cost.get("captured_stderr_bytes", MISSING), "cost.captured_stderr_bytes"),
        "cost.command_wall_ms": optional_nonnegative_number(cost.get("command_wall_ms", MISSING), "cost.command_wall_ms"),
        "cost.client_observed_tool_ms": optional_nonnegative_number(client_tool, "cost.client_observed_tool_ms"),
        "cost.tool_errors": _count_field(cost.get("tool_errors", MISSING), "cost.tool_errors"),
        "cost.protocol_errors": _count_field(cost.get("protocol_errors", MISSING), "cost.protocol_errors"),
        "repeated_searches.group_count": optional_nonnegative_integer(repeated.get("group_count", MISSING), "repeated_searches.group_count"),
        "repeated_searches.extra_call_count": optional_nonnegative_integer(repeated.get("extra_call_count", MISSING), "repeated_searches.extra_call_count"),
        "unbounded_reads.count": optional_nonnegative_integer(unbounded.get("count", MISSING), "unbounded_reads.count"),
        "unbounded_reads.output_bytes": optional_nonnegative_integer(unbounded.get("output_bytes", MISSING), "unbounded_reads.output_bytes"),
        "dialogue.codemap_tool_selection.non_handshake_call_count": optional_nonnegative_integer(selection.get("non_handshake_call_count", MISSING), "dialogue.codemap_tool_selection.non_handshake_call_count"),
        "dialogue.codemap_tool_selection.handshake_call_count": optional_nonnegative_integer(selection.get("handshake_call_count", MISSING), "dialogue.codemap_tool_selection.handshake_call_count"),
        "post_first_wrong.tool_calls": post_values["tool_calls"],
        "post_first_wrong.tool_output_bytes": post_values["tool_output_bytes"],
        "post_first_wrong.tokens": post_values["tokens"],
        "post_first_wrong.wall_ms": post_values["wall_ms"],
        "post_first_wrong.tool_errors": post_values["tool_errors"],
        "post_first_wrong.repeated_extra_calls": post_values["repeated_extra_calls"],
        "post_first_wrong.unbounded_read_calls": post_values["unbounded_read_calls"],
        "navigation.first_completed_call_index": navigation["first_navigation_completed_call_index"],
        "navigation.first_input_bytes": navigation["first_navigation_input_bytes"],
        "navigation.first_output_bytes": navigation["first_navigation_output_bytes"],
        "navigation.first_client_observed_ms": navigation["first_navigation_client_observed_ms"],
        "navigation.codemap_first_use_completed_call_index": navigation["codemap_first_use_completed_call_index"],
        "navigation.origin_switch_count": navigation["origin_switch_count"],
    }
    for component in TOKEN_COMPONENTS:
        values[f"cost.tokens.{component}"] = optional_nonnegative_integer(tokens.get(component, MISSING), f"cost.tokens.{component}")

    by_family = cost.get("tool_calls_by_family", MISSING)
    if by_family is MISSING or by_family is None:
        family_counts = None
    else:
        by_family = _mapping(by_family, "cost.tool_calls_by_family")
        family_counts = {str(family): optional_nonnegative_integer(count, f"cost.tool_calls_by_family.{family}") for family, count in by_family.items()}

    terminal = metric["experiment"].get("terminal_behavior")
    if isinstance(terminal, Mapping):
        candidates = (terminal.get("termination_cause"), terminal.get("automated_status"))
        terminal = next((value for value in candidates if isinstance(value, str) and value), None)
    elif not isinstance(terminal, str):
        terminal = None

    metric_first_wrong = agreement.get("majority_first_wrong_category")
    discovery = {
        stage: deep_get(agreement, ("discovery_stages", stage, "majority"), None)
        for stage in DISCOVERY_STAGES
    }

    return {
        "numeric": values,
        "tool_calls_by_family": family_counts,
        "categorical": {
            "semantic_label": agreement.get("majority_semantic_label"),
            "contract_complete": agreement.get("majority_contract_complete"),
            "format_valid_json": agreement.get("majority_format_valid_json"),
            "terminal_behavior": terminal,
            "first_wrong_category": metric_first_wrong,
            "discovery_stages": discovery,
            "first_navigation_tool_family": navigation["first_navigation_tool_family"],
            "first_navigation_tool_origin": navigation["first_navigation_tool_origin"],
        },
        "post_first_wrong_derivation": post_derivation,
        "navigation_behavior": navigation,
    }


def _task_arm_rows(
    tasks: Sequence[str],
    sessions: Mapping[tuple[str, str, str], Mapping[str, Any]],
    numeric_metric_names: Sequence[str],
) -> list[dict[str, Any]]:
    rows = []
    for task in tasks:
        for arm in EXPECTED_ARMS:
            metrics = {}
            for metric_name in numeric_metric_names:
                raw = {trial: sessions[(task, trial, arm)]["normalized"]["numeric"].get(metric_name) for trial in EXPECTED_TRIALS}
                metrics[metric_name] = trial_numeric_summary(raw)
            rows.append({"task_id": task, "arm": arm, "trials": list(EXPECTED_TRIALS), "metrics": metrics})
    return rows


def _quality_rate_rows(tasks: Sequence[str], sessions: Mapping[tuple[str, str, str], Mapping[str, Any]]) -> dict[str, Any]:
    rows = []
    for task in tasks:
        for arm in EXPECTED_ARMS:
            categorical = {}
            specs = (
                ("semantic_label", SEMANTIC_LABELS),
                ("contract_complete", (True, False)),
                ("format_valid_json", (True, False)),
            )
            for field, allowed in specs:
                raw_by_trial = {trial: sessions[(task, trial, arm)]["normalized"]["categorical"][field] for trial in EXPECTED_TRIALS}
                summary = categorical_summary([raw_by_trial[trial] for trial in EXPECTED_TRIALS], allowed, expected_n=3)
                summary["raw_by_trial"] = raw_by_trial
                summary["denominator"] = "valid non-null output majorities among three sealed trials"
                categorical[field] = summary
            rows.append({"task_id": task, "arm": arm, **categorical})

    overall_by_arm = []
    for arm in EXPECTED_ARMS:
        arm_sessions = [sessions[(task, trial, arm)] for task in tasks for trial in EXPECTED_TRIALS]
        overall_by_arm.append({
            "arm": arm,
            "semantic_label": categorical_summary([item["normalized"]["categorical"]["semantic_label"] for item in arm_sessions], SEMANTIC_LABELS, expected_n=42),
            "contract_complete": categorical_summary([item["normalized"]["categorical"]["contract_complete"] for item in arm_sessions], (True, False), expected_n=42),
            "format_valid_json": categorical_summary([item["normalized"]["categorical"]["format_valid_json"] for item in arm_sessions], (True, False), expected_n=42),
        })
    return {"task_arm_rows": rows, "overall_by_arm": overall_by_arm}


def _behavior_rows(tasks: Sequence[str], sessions: Mapping[tuple[str, str, str], Mapping[str, Any]]) -> dict[str, Any]:
    terminal_values = sorted({
        session["normalized"]["categorical"]["terminal_behavior"]
        for session in sessions.values()
        if session["normalized"]["categorical"]["terminal_behavior"] is not None
    })
    terminal_values = terminal_values or ["<none>"]
    rows = []
    for task in tasks:
        for arm in EXPECTED_ARMS:
            current = [sessions[(task, trial, arm)]["normalized"]["categorical"] for trial in EXPECTED_TRIALS]
            discovery = {}
            for stage in DISCOVERY_STAGES:
                discovery[stage] = categorical_summary(
                    [item["discovery_stages"][stage] for item in current], DISCOVERY_STATUSES, expected_n=3
                )
            rows.append({
                "task_id": task,
                "arm": arm,
                "terminal_behavior": categorical_summary([item["terminal_behavior"] for item in current], terminal_values, expected_n=3),
                "first_wrong_category": categorical_summary([item["first_wrong_category"] for item in current], FIRST_WRONG_CATEGORIES, expected_n=3),
                "discovery_stages": discovery,
            })
    overall_by_arm = []
    for arm in EXPECTED_ARMS:
        current = [sessions[(task, trial, arm)]["normalized"]["categorical"] for task in tasks for trial in EXPECTED_TRIALS]
        overall_by_arm.append({
            "arm": arm,
            "terminal_behavior": categorical_summary([item["terminal_behavior"] for item in current], terminal_values, expected_n=42),
            "first_wrong_category": categorical_summary([item["first_wrong_category"] for item in current], FIRST_WRONG_CATEGORIES, expected_n=42),
            "discovery_stages": {
                stage: categorical_summary([item["discovery_stages"][stage] for item in current], DISCOVERY_STATUSES, expected_n=42)
                for stage in DISCOVERY_STAGES
            },
        })
    return {
        "task_arm_rows": rows,
        "overall_by_arm": overall_by_arm,
        "terminal_allowed_values_observed": terminal_values,
    }


def _navigation_behavior_rows(
    tasks: Sequence[str],
    sessions: Mapping[tuple[str, str, str], Mapping[str, Any]],
) -> dict[str, Any]:
    numeric_names = (
        "navigation.first_completed_call_index",
        "navigation.first_input_bytes",
        "navigation.first_output_bytes",
        "navigation.first_client_observed_ms",
        "navigation.codemap_first_use_completed_call_index",
        "navigation.origin_switch_count",
    )
    family_values = tuple(sorted(NAVIGATION_FAMILIES))
    origin_values = ("builtin", "codemap")

    def summarize(current: Sequence[Mapping[str, Any]], expected_n: int) -> dict[str, Any]:
        return {
            "first_navigation_tool_family": categorical_summary(
                [item["normalized"]["categorical"]["first_navigation_tool_family"] for item in current],
                family_values,
                expected_n=expected_n,
            ),
            "first_navigation_tool_origin": categorical_summary(
                [item["normalized"]["categorical"]["first_navigation_tool_origin"] for item in current],
                origin_values,
                expected_n=expected_n,
            ),
            "numeric": {
                name: numeric_summary(
                    [item["normalized"]["numeric"][name] for item in current],
                    expected_n=expected_n,
                )
                for name in numeric_names
            },
        }

    task_arm = []
    for task in tasks:
        for arm in EXPECTED_ARMS:
            current = [sessions[(task, trial, arm)] for trial in EXPECTED_TRIALS]
            task_arm.append({"task_id": task, "arm": arm, **summarize(current, 3)})
    overall = []
    for arm in EXPECTED_ARMS:
        current = [sessions[(task, trial, arm)] for task in tasks for trial in EXPECTED_TRIALS]
        overall.append({"arm": arm, **summarize(current, 42)})
    return {
        "task_arm_rows": task_arm,
        "overall_by_arm": overall,
        "family_allowed_values": list(family_values),
        "origin_allowed_values": list(origin_values),
    }


def _paired_rows(
    tasks: Sequence[str],
    sessions: Mapping[tuple[str, str, str], Mapping[str, Any]],
    numeric_metric_names: Sequence[str],
) -> list[dict[str, Any]]:
    rows = []
    for task in tasks:
        for trial in EXPECTED_TRIALS:
            values = {}
            b1_values = sessions[(task, trial, "B1")]["normalized"]["numeric"]
            b2_values = sessions[(task, trial, "B2")]["normalized"]["numeric"]
            for metric_name in numeric_metric_names:
                b1 = b1_values.get(metric_name)
                b2 = b2_values.get(metric_name)
                delta = b2 - b1 if b1 is not None and b2 is not None else None
                values[metric_name] = {
                    "B1": b1,
                    "B2": b2,
                    "delta_B2_minus_B1": delta,
                    "paired_complete": delta is not None,
                }
            rows.append({"task_id": task, "trial_id": trial, "delta_direction": "B2-B1", "metrics": values})
    return rows


def _delta_summaries(
    tasks: Sequence[str],
    paired_rows: Sequence[Mapping[str, Any]],
    numeric_metric_names: Sequence[str],
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    lookup = {(row["task_id"], row["trial_id"]): row for row in paired_rows}
    by_task = []
    task_metric_means: dict[str, list[int | float | None]] = {name: [] for name in numeric_metric_names}
    for task in tasks:
        metrics = {}
        for metric_name in numeric_metric_names:
            raw_by_trial = {
                trial: lookup[(task, trial)]["metrics"][metric_name]["delta_B2_minus_B1"]
                for trial in EXPECTED_TRIALS
            }
            summary = trial_numeric_summary(raw_by_trial)
            summary["delta_direction"] = "B2-B1"
            summary["complete_case_rule"] = "a trial contributes only when both B1 and B2 are non-null"
            metrics[metric_name] = summary
            task_metric_means[metric_name].append(summary["mean"])
        by_task.append({"task_id": task, "metrics": metrics})

    overall_metrics = {}
    for metric_name in numeric_metric_names:
        session_deltas = [row["metrics"][metric_name]["delta_B2_minus_B1"] for row in paired_rows]
        weighted = numeric_summary(session_deltas, expected_n=42)
        weighted.update({
            "denominator": "42 sealed task+trial pairs; only paired complete cases enter statistics",
            "weighting": "each valid task+trial pair has equal weight",
        })
        macro = numeric_summary(task_metric_means[metric_name], expected_n=14)
        macro.update({
            "denominator": "14 sealed tasks; each task contributes its mean over valid paired trials",
            "weighting": "each task with at least one valid paired trial has equal weight",
            "raw_task_means": {task: task_metric_means[metric_name][index] for index, task in enumerate(tasks)},
        })
        overall_metrics[metric_name] = {"session_weighted": weighted, "task_macro": macro, "delta_direction": "B2-B1"}
    return by_task, {"metrics": overall_metrics, "delta_direction": "B2-B1"}


def _sign(value: int | float | None) -> int | None:
    if value is None:
        return None
    return 1 if value > 0 else -1 if value < 0 else 0


def variance_warning_kinds(values: Sequence[int | float | None]) -> list[str]:
    summary = numeric_summary(values)
    valid = [value for value in values if value is not None]
    warnings = []
    if summary["valid_n"] < 3:
        warnings.append("valid_pairs_below_3")
    if any(value < 0 for value in valid) and any(value > 0 for value in valid):
        warnings.append("mixed_sign")
    if summary["mean"] is not None and summary["median"] is not None and _sign(summary["mean"]) != _sign(summary["median"]):
        warnings.append("mean_median_sign_mismatch")
    if summary["sample_sd"] is not None and summary["mean"] is not None and summary["sample_sd"] >= abs(summary["mean"]):
        warnings.append("sample_sd_at_least_absolute_mean")
    if len(valid) >= 3 and summary["mean"] is not None:
        full_sign = _sign(summary["mean"])
        for index in range(len(valid)):
            leave_one_out = valid[:index] + valid[index + 1 :]
            loo_sign = _sign(sum(leave_one_out) / len(leave_one_out))
            if full_sign in (-1, 1) and loo_sign == -full_sign:
                warnings.append("leave_one_out_sign_flip")
                break
    return warnings


def classify_quality_cost(quality_delta: int | float | None, cost_delta: int | float | None) -> str:
    if quality_delta is None or cost_delta is None:
        return "missing_complete_pair"
    if cost_delta < 0 and quality_delta > 0:
        return "dominant_improvement"
    if cost_delta > 0 and quality_delta < 0:
        return "dominant_regression"
    if cost_delta < 0 and quality_delta < 0:
        return "cost_lower_quality_worse"
    if cost_delta > 0 and quality_delta > 0:
        return "quality_better_cost_higher"
    if cost_delta < 0 and quality_delta == 0:
        return "cost_lower_quality_equal"
    if cost_delta > 0 and quality_delta == 0:
        return "cost_higher_quality_equal"
    if cost_delta == 0 and quality_delta > 0:
        return "quality_better_cost_equal"
    if cost_delta == 0 and quality_delta < 0:
        return "quality_worse_cost_equal"
    return "no_change"


def _warnings_and_quality_cost(
    tasks: Sequence[str],
    paired_rows: Sequence[Mapping[str, Any]],
    task_deltas: Sequence[Mapping[str, Any]],
    agreements: Sequence[Mapping[str, Any]],
    numeric_metric_names: Sequence[str],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    warnings: list[dict[str, Any]] = []
    for task_row in task_deltas:
        for metric_name in numeric_metric_names:
            summary = task_row["metrics"][metric_name]
            for warning in variance_warning_kinds(summary["raw"]):
                warnings.append({
                    "kind": warning,
                    "scope": "task_paired_delta",
                    "task_id": task_row["task_id"],
                    "metric": metric_name,
                    "raw_deltas_B2_minus_B1": summary["raw"],
                    "valid_n": summary["valid_n"],
                })

    for agreement in agreements:
        identity = {key: agreement[key] for key in ("task_id", "trial_id", "arm")}
        if agreement["score_range"] >= 20:
            warnings.append({"kind": "judgment_score_range_at_least_20", "scope": "output", **identity, "score_range": agreement["score_range"], "raw_scores": agreement["raw_scores"]})
        if agreement["label_disagreement"]:
            warnings.append({"kind": "judgment_label_change", "scope": "output", **identity, "raw_semantic_labels": agreement["raw_semantic_labels"]})
        if any(score == 0 for score in agreement["raw_scores"]):
            warnings.append({"kind": "quality_floor_observed", "scope": "output", **identity, "raw_scores": agreement["raw_scores"]})
        if any(score == 100 for score in agreement["raw_scores"]):
            warnings.append({"kind": "quality_ceiling_observed", "scope": "output", **identity, "raw_scores": agreement["raw_scores"]})
        first_wrong_status = agreement["first_wrong"]["majority_status"]
        position_status = agreement["first_wrong"]["selected_position"]["status"]
        if first_wrong_status == "no_majority":
            warnings.append({"kind": "first_wrong_no_majority", "scope": "output", **identity, "raw_categories": agreement["first_wrong_categories"]})
        elif position_status == "no_majority_position":
            warnings.append({"kind": "first_wrong_position_no_majority", "scope": "output", **identity, "first_wrong": agreement["first_wrong"]})

    quality_cost_flags = []
    agreement_by_slot = {
        (item["task_id"], item["trial_id"], item["arm"]): item
        for item in agreements
    }
    for row in paired_rows:
        b1_label = agreement_by_slot[(row["task_id"], row["trial_id"], "B1")]["majority_semantic_label"]
        b2_label = agreement_by_slot[(row["task_id"], row["trial_id"], "B2")]["majority_semantic_label"]
        if b1_label is not None and b2_label is not None and b1_label != b2_label:
            warnings.append({
                "kind": "paired_quality_label_change",
                "scope": "task_trial_pair",
                "task_id": row["task_id"],
                "trial_id": row["trial_id"],
                "B1": b1_label,
                "B2": b2_label,
            })
        quality = row["metrics"]["quality.score_mean"]
        quality_delta = quality["delta_B2_minus_B1"]
        for cost_metric in QUALITY_COST_METRICS:
            if cost_metric not in row["metrics"]:
                continue
            cost = row["metrics"][cost_metric]
            classification = classify_quality_cost(quality_delta, cost["delta_B2_minus_B1"])
            flag = {
                "task_id": row["task_id"],
                "trial_id": row["trial_id"],
                "quality_metric": "quality.score_mean",
                "cost_metric": cost_metric,
                "delta_direction": "B2-B1",
                "quality": quality,
                "cost": cost,
                "classification": classification,
            }
            quality_cost_flags.append(flag)
            if classification == "cost_lower_quality_worse":
                warnings.append({
                    "kind": "cost_lower_quality_worse",
                    "scope": "task_trial_pair",
                    "task_id": row["task_id"],
                    "trial_id": row["trial_id"],
                    "cost_metric": cost_metric,
                    "quality_delta_B2_minus_B1": quality_delta,
                    "cost_delta_B2_minus_B1": cost["delta_B2_minus_B1"],
                })
    return warnings, quality_cost_flags


def _judgment_summary(agreements: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    require(len(agreements) == 84, "judgment agreement summary requires 84 outputs")
    unanimous = [len(set(item["raw_semantic_labels"])) == 1 for item in agreements]
    pairwise_rows = []
    pair_totals: dict[tuple[str, str], list[bool]] = defaultdict(list)
    absolute_score_differences = []
    per_output_absolute_differences = []
    for item in agreements:
        labels_by_scorer = dict(zip(item["scorer_ids"], item["raw_semantic_labels"]))
        scores_by_scorer = dict(zip(item["scorer_ids"], item["raw_scores"]))
        scorer_ids = sorted(labels_by_scorer)
        output_differences = []
        for left_index in range(len(scorer_ids)):
            for right_index in range(left_index + 1, len(scorer_ids)):
                left = scorer_ids[left_index]
                right = scorer_ids[right_index]
                agrees = labels_by_scorer[left] == labels_by_scorer[right]
                difference = abs(scores_by_scorer[left] - scores_by_scorer[right])
                pair_totals[(left, right)].append(agrees)
                absolute_score_differences.append(difference)
                output_differences.append(difference)
                pairwise_rows.append({
                    "task_id": item["task_id"],
                    "trial_id": item["trial_id"],
                    "arm": item["arm"],
                    "scorer_pair": [left, right],
                    "label_agrees": agrees,
                    "absolute_score_difference": difference,
                })
        per_output_absolute_differences.append(sum(output_differences) / len(output_differences))

    pairwise_equal_n = sum(row["label_agrees"] for row in pairwise_rows)
    pairwise_denominator = len(pairwise_rows)
    per_pair = []
    for scorer_pair, values in sorted(pair_totals.items()):
        equal_n = sum(values)
        per_pair.append({
            "scorer_pair": list(scorer_pair),
            "equal_label_n": equal_n,
            "denominator_n": len(values),
            "agreement_rate": equal_n / len(values),
        })

    rating_counts = Counter(label for item in agreements for label in item["raw_semantic_labels"])
    output_agreements = []
    for item in agreements:
        counts = Counter(item["raw_semantic_labels"])
        output_agreements.append((sum(count * count for count in counts.values()) - 3) / (3 * 2))
    observed_agreement = sum(output_agreements) / len(output_agreements)
    total_ratings = len(agreements) * 3
    category_proportions = {label: rating_counts[label] / total_ratings for label in SEMANTIC_LABELS}
    expected_agreement = sum(value * value for value in category_proportions.values())
    kappa_denominator = 1 - expected_agreement
    fleiss_kappa = (observed_agreement - expected_agreement) / kappa_denominator if kappa_denominator != 0 else None

    return {
        "output_count": len(agreements),
        "score_mean_across_outputs": numeric_summary([item["score_mean"] for item in agreements], expected_n=84),
        "score_range_distribution": numeric_summary([item["score_range"] for item in agreements], expected_n=84),
        "mean_absolute_pairwise_score_difference": {
            **numeric_summary(absolute_score_differences, expected_n=252),
            "definition": "absolute score difference for each of three scorer pairs on each of 84 outputs",
        },
        "per_output_mean_absolute_pairwise_score_difference": {
            **numeric_summary(per_output_absolute_differences, expected_n=84),
            "definition": "within-output mean of the three absolute pairwise score differences",
        },
        "majority_semantic_label": categorical_summary([item["majority_semantic_label"] for item in agreements], SEMANTIC_LABELS, expected_n=84),
        "majority_contract_complete": categorical_summary([item["majority_contract_complete"] for item in agreements], (True, False), expected_n=84),
        "majority_format_valid_json": categorical_summary([item["majority_format_valid_json"] for item in agreements], (True, False), expected_n=84),
        "label_disagreement": categorical_summary([item["label_disagreement"] for item in agreements], (True, False), expected_n=84),
        "component_disagreement": categorical_summary([item["any_component_disagreement"] for item in agreements], (True, False), expected_n=84),
        "score_range_at_least_20": categorical_summary([item["score_range"] >= 20 for item in agreements], (True, False), expected_n=84),
        "three_way_unanimous_semantic_label": {
            "unanimous_n": sum(unanimous),
            "denominator_n": 84,
            "rate": sum(unanimous) / 84,
            "definition": "all three scorers assigned the same semantic label",
        },
        "pairwise_semantic_label_agreement": {
            "equal_label_n": pairwise_equal_n,
            "denominator_n": pairwise_denominator,
            "agreement_rate": pairwise_equal_n / pairwise_denominator,
            "per_scorer_pair": per_pair,
            "definition": "equal-label scorer pairs divided by 84 outputs x 3 scorer pairs",
        },
        "fleiss_kappa_semantic_label": {
            "value": fleiss_kappa,
            "output_n": 84,
            "ratings_per_output_n": 3,
            "rating_denominator_n": total_ratings,
            "categories": list(SEMANTIC_LABELS),
            "category_rating_counts": {label: rating_counts[label] for label in SEMANTIC_LABELS},
            "category_proportions": category_proportions,
            "observed_agreement": observed_agreement,
            "expected_agreement": expected_agreement,
            "kappa_denominator": kappa_denominator,
            "degenerate": kappa_denominator == 0,
            "degenerate_rule": "value is null when 1 - expected_agreement is zero",
            "definition": "Fleiss' kappa over 84 outputs, three ratings per output, and the sealed semantic-label categories",
        },
        "pairwise_rows": pairwise_rows,
    }


def validate_attempt_accounting(value: Any) -> None:
    accounting = _mapping(value, "aggregate.attempt_accounting")
    top_keys = {
        "raw_attempt_count", "latest_valid_count", "superseded_attempt_count",
        "invalid_attempt_count", "transient_attempt_count", "replacement_attempt_count",
        "overall", "by_arm",
    }
    summary_keys = {
        "denominator_n", "raw_attempt_count", "latest_valid_count", "superseded_attempt_count",
        "invalid_attempt_count", "transient_attempt_count", "replacement_attempt_count",
        "canceled_before_start_count", "measurement_status_counts", "terminal_behavior_counts",
        "replacement_category_counts", "attempt_state_counts",
    }
    require(set(accounting) == top_keys, "aggregate attempt accounting top-level keys mismatch")

    def check_summary(raw: Any, label: str, *, arm: str | None = None) -> Mapping[str, Any]:
        summary = _mapping(raw, label)
        expected_keys = summary_keys | ({"arm"} if arm is not None else set())
        require(set(summary) == expected_keys, f"{label} keys mismatch")
        if arm is not None:
            require(summary.get("arm") == arm, f"{label} arm mismatch")
        for key in summary_keys - {
            "measurement_status_counts", "terminal_behavior_counts", "replacement_category_counts", "attempt_state_counts",
        }:
            require(
                isinstance(summary.get(key), int) and not isinstance(summary.get(key), bool) and summary[key] >= 0,
                f"{label}.{key} must be a nonnegative integer",
            )
        measurement = _mapping(summary.get("measurement_status_counts"), f"{label}.measurement_status_counts")
        terminal = _mapping(summary.get("terminal_behavior_counts"), f"{label}.terminal_behavior_counts")
        category = _mapping(summary.get("replacement_category_counts"), f"{label}.replacement_category_counts")
        states = _mapping(summary.get("attempt_state_counts"), f"{label}.attempt_state_counts")
        require(set(measurement) == {"valid", "infrastructure_invalid", "unclassified"}, f"{label} measurement keys mismatch")
        require(set(terminal) == {*TERMINAL_BEHAVIORS, "unclassified"}, f"{label} terminal behavior keys mismatch")
        require(set(category) == {*TRANSIENT_CATEGORIES, "none", "unclassified"}, f"{label} replacement category keys mismatch")
        require(set(states) == set(ATTEMPT_STATES), f"{label} attempt state keys mismatch")
        for counts, count_label in ((measurement, "measurement"), (terminal, "terminal"), (category, "category"), (states, "state")):
            require(
                all(isinstance(count, int) and not isinstance(count, bool) and count >= 0 for count in counts.values()),
                f"{label} {count_label} counts must be nonnegative integers",
            )
            require(sum(counts.values()) == summary["denominator_n"], f"{label} {count_label} counts do not preserve denominator")
        require(summary["denominator_n"] == summary["raw_attempt_count"], f"{label} raw denominator mismatch")
        require(
            summary["raw_attempt_count"]
            == summary["latest_valid_count"] + summary["superseded_attempt_count"] + summary["canceled_before_start_count"],
            f"{label} latest/superseded/canceled union mismatch",
        )
        require(summary["invalid_attempt_count"] == measurement["infrastructure_invalid"], f"{label} invalid count mismatch")
        require(
            summary["transient_attempt_count"] == sum(category[name] for name in TRANSIENT_CATEGORIES),
            f"{label} transient count mismatch",
        )
        return summary

    overall = check_summary(accounting.get("overall"), "aggregate.attempt_accounting.overall")
    for key in top_keys - {"overall", "by_arm"}:
        require(accounting[key] == overall[key], f"aggregate attempt accounting alias mismatch: {key}")
    require(overall["latest_valid_count"] == 84, "aggregate latest valid count must be 84")
    by_arm = _list(accounting.get("by_arm"), "aggregate.attempt_accounting.by_arm")
    require(len(by_arm) == 2, "aggregate attempt accounting must contain two arms")
    arm_rows = [check_summary(row, f"aggregate.attempt_accounting.by_arm[{arm}]", arm=arm) for row, arm in zip(by_arm, EXPECTED_ARMS)]
    require([row["latest_valid_count"] for row in arm_rows] == [42, 42], "aggregate latest valid arm counts must be 42/42")
    for key in summary_keys:
        if key.endswith("_counts"):
            continue
        require(sum(row[key] for row in arm_rows) == overall[key], f"aggregate arm totals mismatch: {key}")


def validate_aggregate_output(value: Any, *, require_provenance: bool) -> None:
    """Dependency-free exact validation for the aggregate emitted by this program."""
    result = _mapping(value, "aggregate output")
    expected_keys = {
        "schema_version", "generation_id", "aggregation_contract", "attempt_accounting",
        "raw_sessions", "task_arm_rows", "paired_deltas", "task_delta_summaries", "overall_deltas",
        "judgment_agreement", "quality_rates", "behavior_rates", "navigation_behavior",
        "quality_cost_flags", "variance_warnings", "metric_catalog",
    }
    if require_provenance:
        expected_keys.add("input_provenance")
    require(set(result) == expected_keys, "aggregate output top-level keys mismatch")
    require(result.get("schema_version") == SCHEMA_VERSION, "aggregate output schema_version mismatch")
    contract = _mapping(result.get("aggregation_contract"), "aggregate.aggregation_contract")
    require(contract.get("session_count") == 84 and contract.get("task_count") == 14, "aggregate contract count mismatch")
    require(contract.get("trial_ids") == list(EXPECTED_TRIALS), "aggregate contract trials mismatch")
    require(contract.get("arms") == list(EXPECTED_ARMS), "aggregate contract arms mismatch")
    require(contract.get("judgments_per_output") == 3 and contract.get("delta_direction") == "B2-B1", "aggregate contract scoring/delta mismatch")
    validate_attempt_accounting(result.get("attempt_accounting"))

    raw_sessions = _list(result.get("raw_sessions"), "aggregate.raw_sessions")
    require(len(raw_sessions) == 84, "aggregate raw session count must be 84")
    identities = {(row.get("task_id"), row.get("trial_id"), row.get("arm")) for row in raw_sessions if isinstance(row, Mapping)}
    require(len(identities) == 84, "aggregate raw session identities are not unique")
    require(all(row.get("automatic_metric_record_kind") == "extractor_record" for row in raw_sessions), "aggregate contains a legacy metric record")
    catalog = _mapping(result.get("metric_catalog"), "aggregate.metric_catalog")
    metric_names = _list(catalog.get("numeric"), "aggregate.metric_catalog.numeric")
    require(metric_names and len(set(metric_names)) == len(metric_names), "aggregate numeric metric catalog is empty or duplicated")

    task_arm_rows = _list(result.get("task_arm_rows"), "aggregate.task_arm_rows")
    require(len(task_arm_rows) == 28, "aggregate task-arm row count must be 28")
    require(len({(row.get("task_id"), row.get("arm")) for row in task_arm_rows if isinstance(row, Mapping)}) == 28, "aggregate task-arm identities mismatch")
    for row in task_arm_rows:
        metrics = _mapping(row.get("metrics"), "aggregate.task_arm_rows[].metrics")
        require(set(metrics) == set(metric_names), "aggregate task-arm metric set mismatch")
        for summary in metrics.values():
            summary = _mapping(summary, "aggregate task-arm metric summary")
            require(summary.get("denominator_n") == 3, "aggregate task-arm denominator must be 3")
            require(summary.get("valid_n", -1) + summary.get("missing_n", -1) == 3, "aggregate task-arm valid/missing denominator mismatch")
            require(set(_mapping(summary.get("raw_by_trial"), "aggregate task-arm raw_by_trial")) == set(EXPECTED_TRIALS), "aggregate task-arm trial set mismatch")

    paired = _list(result.get("paired_deltas"), "aggregate.paired_deltas")
    require(len(paired) == 42, "aggregate paired delta count must be 42")
    require(len({(row.get("task_id"), row.get("trial_id")) for row in paired if isinstance(row, Mapping)}) == 42, "aggregate paired identities mismatch")
    for row in paired:
        require(row.get("delta_direction") == "B2-B1", "aggregate paired delta direction mismatch")
        require(set(_mapping(row.get("metrics"), "aggregate paired metrics")) == set(metric_names), "aggregate paired metric set mismatch")
    task_deltas = _list(result.get("task_delta_summaries"), "aggregate.task_delta_summaries")
    require(len(task_deltas) == 14, "aggregate task delta row count must be 14")
    overall = _mapping(result.get("overall_deltas"), "aggregate.overall_deltas")
    require(overall.get("delta_direction") == "B2-B1", "aggregate overall delta direction mismatch")
    overall_metrics = _mapping(overall.get("metrics"), "aggregate.overall_deltas.metrics")
    require(set(overall_metrics) == set(metric_names), "aggregate overall metric set mismatch")
    for summary in overall_metrics.values():
        summary = _mapping(summary, "aggregate overall metric")
        require(_mapping(summary.get("session_weighted"), "aggregate overall session_weighted").get("denominator_n") == 42, "aggregate weighted denominator must be 42")
        require(_mapping(summary.get("task_macro"), "aggregate overall task_macro").get("denominator_n") == 14, "aggregate macro denominator must be 14")

    agreement = _mapping(result.get("judgment_agreement"), "aggregate.judgment_agreement")
    outputs = _list(agreement.get("outputs"), "aggregate.judgment_agreement.outputs")
    summary = _mapping(agreement.get("summary"), "aggregate.judgment_agreement.summary")
    require(len(outputs) == 84 and summary.get("output_count") == 84, "aggregate judgment output count mismatch")
    require(_mapping(summary.get("pairwise_semantic_label_agreement"), "aggregate pairwise agreement").get("denominator_n") == 252, "aggregate pairwise denominator must be 252")
    require(_mapping(summary.get("fleiss_kappa_semantic_label"), "aggregate Fleiss kappa").get("rating_denominator_n") == 252, "aggregate rating denominator must be 252")
    for key in ("quality_rates", "behavior_rates", "navigation_behavior"):
        section = _mapping(result.get(key), f"aggregate.{key}")
        require(len(_list(section.get("task_arm_rows"), f"aggregate.{key}.task_arm_rows")) == 28, f"aggregate {key} task-arm row count mismatch")
        require(len(_list(section.get("overall_by_arm"), f"aggregate.{key}.overall_by_arm")) == 2, f"aggregate {key} arm count mismatch")
    require(isinstance(result.get("quality_cost_flags"), list), "aggregate quality-cost flags must be an array")
    require(isinstance(result.get("variance_warnings"), list), "aggregate variance warnings must be an array")

    if require_provenance:
        provenance = _mapping(result.get("input_provenance"), "aggregate.input_provenance")
        expected_provenance_keys = {
            "analysis_input_seal", "generation", "ledger", "automatic_schema", "metrics_index", "metrics",
            "coordinator_mapping", "scoring_manifest", "judgments_index", "judgments", "rubric", "answer_contracts",
        }
        require(set(provenance) == expected_provenance_keys, "aggregate provenance keys mismatch")
        require(len(_list(provenance.get("metrics"), "aggregate provenance metrics")) == 84, "aggregate metric provenance count mismatch")
        require(len(_list(provenance.get("judgments"), "aggregate provenance judgments")) == 252, "aggregate judgment provenance count mismatch")
        require(len(_list(provenance.get("answer_contracts"), "aggregate provenance answers")) == 14, "aggregate answer provenance count mismatch")


def aggregate_dataset(
    *,
    generation: Any,
    ledger: Any,
    metrics_by_run: Mapping[str, Any],
    coordinator_mapping: Any,
    judgments_by_review: Mapping[str, Any],
    rubric: Mapping[str, Any],
    rubric_sha256: str,
    answers_by_task: Mapping[str, Mapping[str, Any]],
    automatic_schema: Mapping[str, Any],
    expected_runs_root: Path,
) -> dict[str, Any]:
    """Aggregate already-loaded inputs.  This is the synthetic-test boundary."""
    tasks, schedule = validate_generation(generation, rubric, rubric_sha256)
    latest_run_by_slot = validate_ledger(ledger, generation, schedule)
    attempt_accounting = validate_ledger_attempts(ledger, generation, schedule)
    require(set(metrics_by_run) == set(latest_run_by_slot.values()), "metric index must exactly match the 84 latest valid run ids")
    require(set(answers_by_task) == set(tasks), "answer-contract task set must exactly match generation tasks")
    mapping_by_slot = validate_mapping_and_judgments(
        coordinator_mapping,
        judgments_by_review,
        latest_run_by_slot,
        schedule,
        generation,
        expected_runs_root,
    )

    sessions: dict[tuple[str, str, str], dict[str, Any]] = {}
    agreements = []
    for task in tasks:
        answer = _mapping(answers_by_task[task], f"answer_contract[{task}]")
        require(answer.get("task_id") == task, f"answer contract identity mismatch for {task}")
        for trial in EXPECTED_TRIALS:
            for arm in EXPECTED_ARMS:
                slot = (task, trial, arm)
                run_id = latest_run_by_slot[slot]
                unwrapped_metric, record_kind = unwrap_metric_record(metrics_by_run[run_id], run_id)
                metric = validate_metric(
                    unwrapped_metric,
                    run_id=run_id,
                    slot=slot,
                    schedule=schedule[slot],
                    generation=generation,
                    record_kind=record_kind,
                    automatic_schema=automatic_schema,
                )
                agreement = build_judgment_agreement(
                    slot,
                    mapping_by_slot[slot],
                    judgments_by_review,
                    rubric_sha256,
                    answer,
                    metric,
                )
                normalized = normalize_session(metric, agreement)
                session = {
                    "task_id": task,
                    "trial_id": trial,
                    "arm": arm,
                    "run_id": run_id,
                    "automatic_metric_record_kind": record_kind,
                    "automatic_metrics": metric,
                    "judgment_agreement": agreement,
                    "navigation_behavior": normalized["navigation_behavior"],
                    "normalized": normalized,
                }
                sessions[slot] = session
                agreements.append(agreement)

    family_names = sorted({
        family
        for session in sessions.values()
        if session["normalized"]["tool_calls_by_family"] is not None
        for family in session["normalized"]["tool_calls_by_family"]
    })
    for session in sessions.values():
        family_counts = session["normalized"]["tool_calls_by_family"]
        for family in family_names:
            metric_name = f"cost.tool_calls_by_family.{family}"
            if family_counts is None:
                session["normalized"]["numeric"][metric_name] = None
            else:
                # A present exact family-count map defines an absent family as zero calls.
                session["normalized"]["numeric"][metric_name] = family_counts.get(family, 0)
    numeric_metric_names = list(BASE_NUMERIC_METRICS) + [f"cost.tool_calls_by_family.{family}" for family in family_names]

    task_arm_rows = _task_arm_rows(tasks, sessions, numeric_metric_names)
    paired_rows = _paired_rows(tasks, sessions, numeric_metric_names)
    task_deltas, overall_delta = _delta_summaries(tasks, paired_rows, numeric_metric_names)
    warnings, quality_cost_flags = _warnings_and_quality_cost(tasks, paired_rows, task_deltas, agreements, numeric_metric_names)
    quality_rates = _quality_rate_rows(tasks, sessions)
    behavior_rates = _behavior_rows(tasks, sessions)
    navigation_behavior = _navigation_behavior_rows(tasks, sessions)
    for slot, session in sessions.items():
        derivation = session["normalized"]["post_first_wrong_derivation"]
        if derivation.get("warning") is not None:
            warnings.append({
                "kind": derivation["warning"],
                "scope": "output",
                "task_id": slot[0],
                "trial_id": slot[1],
                "arm": slot[2],
                "status": derivation["status"],
            })
        session["post_first_wrong_derivation"] = derivation
    raw_sessions = [sessions[(task, trial, arm)] for task in tasks for trial in EXPECTED_TRIALS for arm in EXPECTED_ARMS]
    for session in raw_sessions:
        session.pop("normalized", None)

    result = {
        "schema_version": SCHEMA_VERSION,
        "generation_id": generation.get("generation_id"),
        "aggregation_contract": {
            "session_count": 84,
            "task_count": 14,
            "trial_ids": list(EXPECTED_TRIALS),
            "arms": list(EXPECTED_ARMS),
            "judgments_per_output": 3,
            "delta_direction": "B2-B1",
            "null_policy": "null is never replaced with zero; numeric statistics use explicitly reported valid_n/missing_n",
            "paired_complete_case_policy": "B2-B1 is computed only after matching task+trial and only when both arm values are non-null",
            "sample_sd_policy": "sample standard deviation with n-1 denominator; null for fewer than two valid observations",
            "rounding_policy": "no rounding is applied",
            "old_run_policy": "metric and judgment index key sets must exactly equal ledger latest run ids and coordinator current review ids",
            "attempt_denominator_policy": "all ledger attempts are preserved; latest valid, superseded, invalid, transient, replacement, canceled, and terminal behavior counts are explicit overall and by arm",
            "macro_policy": "task macro gives every task with at least one valid paired trial equal weight",
            "session_weighted_policy": "every valid task+trial paired complete case has equal weight",
            "navigation_origin_policy": "tool names beginning with codemap_search_ are codemap; other completed navigation tools are builtin",
            "scoring_separation_policy": "navigation origin is derived only after immutable correctness/process judgments are sealed and is never a correctness-bundle input",
        },
        "attempt_accounting": attempt_accounting,
        "raw_sessions": raw_sessions,
        "task_arm_rows": task_arm_rows,
        "paired_deltas": paired_rows,
        "task_delta_summaries": task_deltas,
        "overall_deltas": overall_delta,
        "judgment_agreement": {"outputs": agreements, "summary": _judgment_summary(agreements)},
        "quality_rates": quality_rates,
        "behavior_rates": behavior_rates,
        "navigation_behavior": navigation_behavior,
        "quality_cost_flags": quality_cost_flags,
        "variance_warnings": warnings,
        "metric_catalog": {
            "numeric": numeric_metric_names,
            "tool_families_observed": family_names,
            "quality_labels_rates_and_format_are_separate": True,
        },
    }
    validate_aggregate_output(result, require_provenance=False)
    return result


def _load_file_index(index_path: Path, collection_key: str, identifier_key: str) -> tuple[dict[str, Any], list[dict[str, str]]]:
    value = _mapping(load_json(index_path), str(index_path))
    require(value.get("schema_version") == 1, f"{index_path} schema_version mismatch")
    entries = _list(value.get(collection_key), f"{index_path}:{collection_key}")
    result = {}
    provenance = []
    for index, raw in enumerate(entries):
        entry = _mapping(raw, f"{index_path}:{collection_key}[{index}]")
        identifier = _string(entry.get(identifier_key), f"{collection_key}[{index}].{identifier_key}")
        require(identifier not in result, f"duplicate {identifier_key} {identifier!r} in {index_path}")
        raw_path = _string(entry.get("path"), f"{collection_key}[{index}].path")
        path = Path(raw_path)
        require_read_only_regular_file(path, f"indexed {collection_key} file {identifier}")
        result[identifier] = load_json(path)
        provenance.append({identifier_key: identifier, "path": str(path), "sha256": sha256_file(path)})
    return result, provenance


def _load_answers(answers_dir: Path, tasks: Sequence[str], generation: Mapping[str, Any]) -> tuple[dict[str, Mapping[str, Any]], list[dict[str, str]]]:
    answers = {}
    provenance = []
    expected_hashes = _mapping(generation.get("answer_sha256_by_task"), "generation.answer_sha256_by_task")
    require(set(expected_hashes) == set(tasks), "generation answer hash task set mismatch")
    for task in tasks:
        path = answers_dir / f"{task}.json"
        require(path.is_file(), f"answer contract does not exist: {path}")
        digest = sha256_file(path)
        require(digest == expected_hashes[task], f"answer contract hash mismatch for {task}")
        value = _mapping(load_json(path), str(path))
        answers[task] = value
        provenance.append({"task_id": task, "path": str(path.resolve()), "sha256": digest})
    return answers, provenance


def verify_current_generation(generation_path: Path, harness_root: Path) -> None:
    generation_path = generation_path.absolute()
    require(generation_path.is_file(), f"generation file does not exist: {generation_path}")
    require(not path_has_symlink(generation_path), "generation path must not contain a symlink")
    verify_script = harness_root / "scripts/generation.py"
    require(verify_script.is_file(), f"generation verifier does not exist: {verify_script}")
    environment = dict(os.environ)
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    try:
        completed = subprocess.run(
            [sys.executable, str(verify_script), "verify", str(generation_path)],
            cwd=str(harness_root.parent),
            env=environment,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as error:
        raise AggregationError(f"generation current-snapshot verifier could not run: {error}") from error
    require(
        completed.returncode == 0,
        "generation current-snapshot verification failed: "
        + (completed.stderr.strip() or completed.stdout.strip() or f"exit {completed.returncode}"),
    )


def verify_analysis_input_seal(seal_path: Path, harness_root: Path) -> Mapping[str, Any]:
    """Invoke the harness-owned verifier and translate its fail-closed exit."""
    verifier_path = harness_root / "scripts/build_analysis_inputs.py"
    require(verifier_path.is_file(), f"analysis input verifier does not exist: {verifier_path}")
    module_name = "baseline_analysis_input_verifier_for_aggregator"
    spec = importlib.util.spec_from_file_location(module_name, verifier_path)
    require(spec is not None and spec.loader is not None, "analysis input verifier could not be loaded")
    scripts_path = str(verifier_path.parent)
    inserted = scripts_path not in sys.path
    if inserted:
        sys.path.insert(0, scripts_path)
    value: Any = MISSING
    try:
        module = importlib.util.module_from_spec(spec)
        sys.modules[module_name] = module
        spec.loader.exec_module(module)
        verifier = getattr(module, "verify_analysis_input_seal", None)
        require(callable(verifier), "analysis input verifier does not expose verify_analysis_input_seal")
        try:
            value = verifier(seal_path.absolute())
        except SystemExit as error:
            raise AggregationError(f"analysis input seal verification failed: {error}") from error
    except AggregationError:
        raise
    except Exception as error:
        raise AggregationError(f"analysis input verifier failed to load: {error}") from error
    finally:
        sys.modules.pop(module_name, None)
        if inserted:
            sys.path.remove(scripts_path)
    require(value is not MISSING, "analysis input verifier returned no seal")
    return _mapping(value, "verified analysis input seal")


def require_cli_path_binding(actual: Path, recorded: Any, label: str) -> Path:
    require(isinstance(recorded, str), f"analysis input seal {label} path is invalid")
    absolute = actual.absolute()
    require(str(absolute) == recorded, f"--{label.replace('_', '-')} does not exactly match the analysis input seal")
    require(not path_has_symlink(absolute), f"--{label.replace('_', '-')} path contains a symlink")
    return absolute


def verify_internal_seal(value: Mapping[str, Any], seal_key: str, label: str) -> None:
    core = dict(value)
    recorded = core.pop(seal_key, None)
    require(isinstance(recorded, str) and recorded == canonical_sha256(core), f"{label} self-seal mismatch")


def verify_scoring_manifest(
    *,
    manifest_path: Path,
    generation: Mapping[str, Any],
    ledger_path: Path,
    mapping_path: Path,
    mapping_value: Mapping[str, Any],
    judgments_by_review: Mapping[str, Any],
    judgment_provenance: Sequence[Mapping[str, str]],
    harness_root: Path,
) -> Mapping[str, Any]:
    manifest_path = manifest_path.absolute()
    require_read_only_regular_file(manifest_path, "scoring manifest")
    scoring_root = harness_root / "scoring" / str(generation.get("generation_id"))
    expected_manifest = scoring_root / "final-judgments/final-seal.json"
    require(str(manifest_path) == str(expected_manifest), "scoring manifest path is not the exact generation final-seal path")
    manifest = _mapping(load_json(manifest_path), "scoring manifest")
    expected_keys = {
        "schema_version", "input_contract", "phase", "generation_id", "generation_seal_sha256",
        "assignment_path", "assignment_file_sha256", "assignment_seal_sha256", "ledger_path", "ledger_sha256",
        "bundle_sha256_by_scorer", "phase1_seal_path", "phase1_seal_file_sha256", "phase1_seal_sha256",
        "phase2_seal_path", "phase2_seal_file_sha256", "phase2_seal_sha256",
        "process_bundle_bindings_by_scorer", "judgment_count", "judgments",
        "seal_sha256",
    }
    require(set(manifest) == expected_keys, "scoring manifest top-level keys mismatch")
    require(manifest.get("schema_version") == 1, "scoring manifest schema version mismatch")
    require(manifest.get("input_contract") == "baseline-scoring-final-manifest-v1", "scoring manifest input contract mismatch")
    require(manifest.get("phase") == "final-merged", "scoring manifest phase mismatch")
    require(manifest.get("generation_id") == generation.get("generation_id"), "scoring manifest generation id mismatch")
    require(manifest.get("generation_seal_sha256") == generation.get("generation_seal_sha256"), "scoring manifest generation seal mismatch")
    require(manifest.get("judgment_count") == 252, "scoring manifest judgment count mismatch")
    verify_internal_seal(manifest, "seal_sha256", "scoring manifest")

    mapping_path = mapping_path.absolute()
    require_read_only_regular_file(mapping_path, "scoring assignment")
    require(str(mapping_path) == str(scoring_root / "coordinator-only/assignments.json"), "scoring assignment path mismatch")
    require(manifest.get("assignment_path") == str(mapping_path), "scoring manifest assignment path mismatch")
    require(manifest.get("assignment_file_sha256") == sha256_file(mapping_path), "scoring manifest assignment file hash mismatch")
    verify_internal_seal(mapping_value, "assignment_seal_sha256", "scoring assignment")
    require(manifest.get("assignment_seal_sha256") == mapping_value.get("assignment_seal_sha256"), "scoring manifest assignment internal seal mismatch")

    ledger_path = ledger_path.absolute()
    require_read_only_regular_file(ledger_path, "completed ledger")
    require(manifest.get("ledger_path") == str(ledger_path), "scoring manifest ledger path mismatch")
    require(manifest.get("ledger_sha256") == sha256_file(ledger_path), "scoring manifest ledger hash mismatch")
    require(mapping_value.get("ledger_path") == str(ledger_path), "scoring assignment ledger path mismatch")
    require(mapping_value.get("ledger_sha256") == manifest.get("ledger_sha256"), "scoring assignment ledger hash mismatch")

    phase1_path = Path(str(manifest.get("phase1_seal_path"))).absolute()
    phase2_path = Path(str(manifest.get("phase2_seal_path"))).absolute()
    for path, label, file_hash_key in (
        (phase1_path, "phase-1 seal", "phase1_seal_file_sha256"),
        (phase2_path, "phase-2 seal", "phase2_seal_file_sha256"),
    ):
        require_read_only_regular_file(path, label)
        require(path.is_relative_to(scoring_root), f"{label} path escapes scoring generation root")
        require(manifest.get(file_hash_key) == sha256_file(path), f"{label} file hash mismatch")
    phase1 = _mapping(load_json(phase1_path), "phase-1 seal")
    phase2 = _mapping(load_json(phase2_path), "phase-2 seal")
    phase1_keys = {
        "schema_version", "phase", "generation_id", "generation_seal_sha256", "assignment_path",
        "assignment_file_sha256", "assignment_seal_sha256", "judgment_count", "judgments", "seal_sha256",
    }
    phase2_keys = phase1_keys | {
        "phase1_seal_path", "phase1_seal_file_sha256", "phase1_seal_sha256",
        "process_bundle_bindings_by_scorer",
    }
    require(set(phase1) == phase1_keys, "phase-1 seal top-level keys mismatch")
    require(set(phase2) == phase2_keys, "phase-2 seal top-level keys mismatch")
    verify_internal_seal(phase1, "seal_sha256", "phase-1 seal")
    verify_internal_seal(phase2, "seal_sha256", "phase-2 seal")
    require(manifest.get("phase1_seal_sha256") == phase1.get("seal_sha256"), "phase-1 internal seal mismatch")
    require(manifest.get("phase2_seal_sha256") == phase2.get("seal_sha256"), "phase-2 internal seal mismatch")
    for seal, expected_phase, label in ((phase1, "correctness", "phase-1"), (phase2, "process", "phase-2")):
        require(seal.get("schema_version") == 1, f"{label} schema version mismatch")
        require(seal.get("phase") == expected_phase, f"{label} phase mismatch")
        require(seal.get("generation_id") == generation.get("generation_id"), f"{label} generation id mismatch")
        require(seal.get("generation_seal_sha256") == generation.get("generation_seal_sha256"), f"{label} generation seal mismatch")
        require(seal.get("assignment_seal_sha256") == mapping_value.get("assignment_seal_sha256"), f"{label} assignment seal mismatch")
        require(seal.get("assignment_path") == str(mapping_path), f"{label} assignment path mismatch")
        require(seal.get("assignment_file_sha256") == sha256_file(mapping_path), f"{label} assignment file hash mismatch")
        require(seal.get("judgment_count") == 252, f"{label} judgment count mismatch")
    require(phase2.get("phase1_seal_path") == str(phase1_path), "phase-2 seal phase-1 path mismatch")
    require(phase2.get("phase1_seal_file_sha256") == sha256_file(phase1_path), "phase-2 seal phase-1 file hash mismatch")
    require(phase2.get("phase1_seal_sha256") == phase1.get("seal_sha256"), "phase-2 seal is not bound to phase 1")

    process_bindings = _mapping(
        phase2.get("process_bundle_bindings_by_scorer"),
        "phase-2 process bundle bindings",
    )
    require(set(process_bindings) == set(EXPECTED_SCORERS), "phase-2 process bundle scorer set mismatch")
    require(
        manifest.get("process_bundle_bindings_by_scorer") == process_bindings,
        "final process bundle bindings differ from phase-2 seal",
    )
    for scorer_id in EXPECTED_SCORERS:
        binding = _mapping(process_bindings[scorer_id], f"phase-2 process bundle binding {scorer_id}")
        require(set(binding) == {"path", "sha256", "mode"}, f"phase-2 process bundle binding fields mismatch: {scorer_id}")
        expected_bundle = (scoring_root / "phase2-process" / scorer_id / "bundle.json").absolute()
        require(binding.get("path") == str(expected_bundle), f"phase-2 process bundle path mismatch: {scorer_id}")
        require_read_only_regular_file(expected_bundle, f"phase-2 process bundle {scorer_id}")
        mode = expected_bundle.stat().st_mode & 0o777
        require(binding.get("mode") == 0o444 and mode == 0o444, f"phase-2 process bundle mode mismatch: {scorer_id}")
        require(binding.get("sha256") == sha256_file(expected_bundle), f"phase-2 process bundle hash mismatch: {scorer_id}")

    assignment_rows = _list(mapping_value.get("assignments"), "scoring assignment.assignments")
    rows = {str(row.get("review_id")): _mapping(row, "scoring assignment row") for row in assignment_rows}
    require(len(rows) == 252, "scoring assignment review set is not unique and complete")
    expected_reviews = set(rows)
    phase1_judgments = _mapping(phase1.get("judgments"), "phase-1 judgments")
    phase2_judgments = _mapping(phase2.get("judgments"), "phase-2 judgments")
    final_judgments = _mapping(manifest.get("judgments"), "final judgments")
    require(set(phase1_judgments) == expected_reviews, "phase-1 review set mismatch")
    require(set(phase2_judgments) == expected_reviews, "phase-2 review set mismatch")
    require(set(final_judgments) == expected_reviews, "final review set mismatch")
    require(set(judgments_by_review) == expected_reviews, "judgments index review set mismatch")
    provenance = {str(item["review_id"]): item for item in judgment_provenance}
    require(set(provenance) == expected_reviews, "judgments index provenance review set mismatch")

    scorer_bundle_hashes = _mapping(manifest.get("bundle_sha256_by_scorer"), "scoring manifest bundle hashes")
    require(set(scorer_bundle_hashes) == {"scorer-1", "scorer-2", "scorer-3"}, "scoring manifest scorer bundle set mismatch")
    phase_hash_sets = {"phase1": set(), "phase2": set(), "final": set()}
    for review_id, row in rows.items():
        scorer_id = row.get("scorer_id")
        require(scorer_id in scorer_bundle_hashes, f"assignment scorer mismatch for {review_id}")
        bundle_path = Path(str(row.get("bundle_path"))).absolute()
        require_read_only_regular_file(bundle_path, f"assignment bundle {review_id}")
        require(bundle_path.is_relative_to(scoring_root), f"assignment bundle path escapes scoring root: {review_id}")
        bundle_hash = sha256_file(bundle_path)
        require(row.get("bundle_sha256") == bundle_hash, f"assignment bundle hash mismatch: {review_id}")
        require(scorer_bundle_hashes[scorer_id] == bundle_hash, f"manifest scorer bundle hash mismatch: {review_id}")

        phase1_item = _mapping(phase1_judgments[review_id], f"phase-1 judgment {review_id}")
        phase2_item = _mapping(phase2_judgments[review_id], f"phase-2 judgment {review_id}")
        final_item = _mapping(final_judgments[review_id], f"final judgment {review_id}")
        for item, label in ((phase1_item, "phase-1"), (phase2_item, "phase-2")):
            require(set(item) == {"path", "sha256", "scorer_id", "session_id"}, f"{label} judgment fields mismatch: {review_id}")
            require(item.get("scorer_id") == scorer_id, f"{label} scorer mismatch: {review_id}")
            require(item.get("session_id") == row.get("session_id"), f"{label} session mismatch: {review_id}")
            path = Path(str(item.get("path"))).absolute()
            require_read_only_regular_file(path, f"{label} judgment {review_id}")
            require(path.is_relative_to(scoring_root), f"{label} judgment path escapes scoring root: {review_id}")
            require(item.get("sha256") == sha256_file(path), f"{label} judgment hash mismatch: {review_id}")
            phase_hash_sets["phase1" if label == "phase-1" else "phase2"].add(item["sha256"])

        require(final_item.get("scorer_id") == scorer_id, f"final scorer mismatch: {review_id}")
        require(
            set(final_item) == {"path", "sha256", "scorer_id", "session_id", "run_id", "phase1_sha256", "phase2_sha256"},
            f"final judgment fields mismatch: {review_id}",
        )
        require(final_item.get("session_id") == row.get("session_id"), f"final session mismatch: {review_id}")
        require(final_item.get("run_id") == row.get("run_id"), f"final run mismatch: {review_id}")
        require(final_item.get("phase1_sha256") == phase1_item.get("sha256"), f"final phase-1 hash mismatch: {review_id}")
        require(final_item.get("phase2_sha256") == phase2_item.get("sha256"), f"final phase-2 hash mismatch: {review_id}")
        final_path = Path(str(final_item.get("path"))).absolute()
        expected_final_path = manifest_path.parent / str(scorer_id) / f"{review_id}.json"
        require(str(final_path) == str(expected_final_path), f"final judgment path is not assignment-derived: {review_id}")
        require_read_only_regular_file(final_path, f"final judgment {review_id}")
        final_hash = sha256_file(final_path)
        require(final_item.get("sha256") == final_hash, f"final judgment hash mismatch: {review_id}")
        require(provenance[review_id].get("path") == str(final_path), f"judgments index path mismatch: {review_id}")
        require(provenance[review_id].get("sha256") == final_hash, f"judgments index hash mismatch: {review_id}")
        phase_hash_sets["final"].add(final_hash)
    require(all(len(hashes) == 252 for hashes in phase_hash_sets.values()), "one scoring judgment hash is reused")
    return manifest


def build_parser() -> argparse.ArgumentParser:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description="Aggregate one completed baseline-3x generation from explicit current-run indexes.")
    parser.add_argument("--generation", type=Path, required=True)
    parser.add_argument("--ledger", type=Path, required=True)
    parser.add_argument("--metrics-index", type=Path, required=True, help="JSON with schema_version and metrics[{run_id,path}]")
    parser.add_argument("--mapping", type=Path, required=True, help="coordinator-only assignments.json from the current scoring generation")
    parser.add_argument("--scoring-manifest", type=Path, required=True, help="immutable final scoring manifest")
    parser.add_argument("--judgments-index", type=Path, required=True, help="JSON with schema_version and judgments[{review_id,path}]")
    parser.add_argument("--analysis-input-seal", type=Path, required=True, help="immutable seal binding every analysis input")
    parser.add_argument("--automatic-schema", type=Path, default=root / "harness/schemas/automatic-run-metrics.schema.json")
    parser.add_argument("--rubric", type=Path, default=root / "harness/config/scoring-rubric.json")
    parser.add_argument("--answers-dir", type=Path, default=root / "benchmark/answers/development")
    parser.add_argument("--output", type=Path, help="write JSON here; stdout when omitted")
    parser.add_argument("--compact", action="store_true")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        root = Path(__file__).resolve().parents[1]
        harness_root = root / "harness"
        # Production aggregation must prove the requested generation is the
        # current snapshot before any ledger, index, score, or answer is read.
        verify_current_generation(args.generation, harness_root)
        input_seal = verify_analysis_input_seal(args.analysis_input_seal, harness_root)
        generation_binding = _mapping(input_seal.get("generation"), "analysis input seal.generation")
        ledger_binding = _mapping(input_seal.get("ledger"), "analysis input seal.ledger")
        automatic_binding = _mapping(input_seal.get("automatic_metrics_contract"), "analysis input seal.automatic_metrics_contract")
        metrics_binding = _mapping(input_seal.get("metrics_index"), "analysis input seal.metrics_index")
        scoring_binding = _mapping(input_seal.get("scoring"), "analysis input seal.scoring")
        judgments_binding = _mapping(input_seal.get("judgments_index"), "analysis input seal.judgments_index")
        generation_path = require_cli_path_binding(args.generation, generation_binding.get("path"), "generation")
        ledger_path = require_cli_path_binding(args.ledger, ledger_binding.get("path"), "ledger")
        metrics_index_path = require_cli_path_binding(args.metrics_index, metrics_binding.get("path"), "metrics_index")
        mapping_path = require_cli_path_binding(args.mapping, scoring_binding.get("assignment_path"), "mapping")
        scoring_manifest_path = require_cli_path_binding(
            args.scoring_manifest, scoring_binding.get("final_manifest_path"), "scoring_manifest",
        )
        judgments_index_path = require_cli_path_binding(
            args.judgments_index, judgments_binding.get("path"), "judgments_index",
        )
        automatic_schema_path = require_cli_path_binding(
            args.automatic_schema, automatic_binding.get("schema_path"), "automatic_schema",
        )

        generation = _mapping(load_json(generation_path), str(generation_path))
        ledger = load_json(ledger_path)
        automatic_schema = _mapping(load_json(automatic_schema_path), str(automatic_schema_path))
        rubric = _mapping(load_json(args.rubric), str(args.rubric))
        rubric_sha = sha256_file(args.rubric)
        tasks, _ = validate_generation(generation, rubric, rubric_sha)
        metrics, metric_provenance = _load_file_index(metrics_index_path, "metrics", "run_id")
        judgments, judgment_provenance = _load_file_index(judgments_index_path, "judgments", "review_id")
        mapping = _mapping(load_json(mapping_path), str(mapping_path))
        scoring_manifest = verify_scoring_manifest(
            manifest_path=scoring_manifest_path,
            generation=generation,
            ledger_path=ledger_path,
            mapping_path=mapping_path,
            mapping_value=mapping,
            judgments_by_review=judgments,
            judgment_provenance=judgment_provenance,
            harness_root=harness_root,
        )
        require(
            scoring_manifest.get("seal_sha256") == scoring_binding.get("final_manifest_seal_sha256"),
            "scoring manifest internal seal differs from analysis input seal",
        )
        answers, answer_provenance = _load_answers(args.answers_dir, tasks, generation)
        result = aggregate_dataset(
            generation=generation,
            ledger=ledger,
            metrics_by_run=metrics,
            coordinator_mapping=mapping,
            judgments_by_review=judgments,
            rubric=rubric,
            rubric_sha256=rubric_sha,
            answers_by_task=answers,
            automatic_schema=automatic_schema,
            expected_runs_root=harness_root / "runs",
        )
        require(
            result.get("attempt_accounting") == ledger_binding.get("attempt_accounting"),
            "aggregate attempt accounting differs from the sealed analysis input accounting",
        )
        result["input_provenance"] = {
            "analysis_input_seal": {
                "path": str(args.analysis_input_seal.absolute()),
                "sha256": sha256_file(args.analysis_input_seal.absolute()),
                "seal_sha256": input_seal.get("seal_sha256"),
            },
            "generation": {"path": str(generation_path), "sha256": sha256_file(generation_path)},
            "ledger": {"path": str(ledger_path), "sha256": sha256_file(ledger_path)},
            "automatic_schema": {"path": str(automatic_schema_path), "sha256": sha256_file(automatic_schema_path)},
            "metrics_index": {"path": str(metrics_index_path), "sha256": sha256_file(metrics_index_path)},
            "metrics": metric_provenance,
            "coordinator_mapping": {"path": str(mapping_path), "sha256": sha256_file(mapping_path)},
            "scoring_manifest": {
                "path": str(scoring_manifest_path),
                "sha256": sha256_file(scoring_manifest_path),
                "seal_sha256": scoring_manifest.get("seal_sha256"),
            },
            "judgments_index": {"path": str(judgments_index_path), "sha256": sha256_file(judgments_index_path)},
            "judgments": judgment_provenance,
            "rubric": {"path": str(args.rubric.resolve()), "sha256": rubric_sha},
            "answer_contracts": answer_provenance,
        }
        validate_aggregate_output(result, require_provenance=True)
        rendered = json.dumps(
            result,
            allow_nan=False,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":") if args.compact else None,
            indent=None if args.compact else 2,
        ) + "\n"
        if args.output is None:
            sys.stdout.write(rendered)
        else:
            require(args.output.parent.is_dir(), f"output parent does not exist: {args.output.parent}")
            args.output.write_text(rendered, encoding="utf-8")
        return 0
    except AggregationError as error:
        print(f"aggregation failed closed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
