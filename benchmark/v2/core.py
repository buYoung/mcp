"""Shared data and fixed experiment contract (Python 3.12+)."""
from __future__ import annotations

import hashlib
import json
import math
import os
import random
import subprocess
from pathlib import Path
from typing import Any

SPEC = {
    "version": "2.0.0",
    "codex_version": "0.153.4",
    "model": "gpt-5.6-luna",
    "reasoning_effort": "medium",
    "grader_model": "gpt-6-astra",
    "grader_reasoning_effort": "low",
    "repository": "grafana/grafana",
    "source_commit": "c6fad8695a96577eb466d425e6ac4a759ca30f47",
    "product_version": "0.7.0",
    "product_commit": "146d779a7d186327e765c2837637ad0543f802d1",
    "collect_since": "2026-07-01",
    "max_candidates": 400,
    "seed": 20260907,
    "bootstrap_samples": 10000,
    "limits": {"elapsed_seconds": 300, "exploration_calls": 80, "total_tokens": 500000},
    "concurrency": 2,
    "cost_surge_ratio": 1.25,
    "batch_limits": {"questions": 10, "answers": 40, "bytes": 160 * 1024},
    "groups": {"A": ["rg", "grep", "find", "read"],
               "B": ["initial_instructions", "overview", "search", "grep", "read", "find"]},
    "phases": {"preparation": {"per_difficulty": 2, "repeats": 1},
               "main": {"per_difficulty": 10, "repeats": 2}},
    "host": {"transport": "stdio-mcp", "code_mode": True, "allowed_wrappers": ["exec", "wait"],
             "delivered_output_bytes": 32000, "tool_output_token_limit": 65536,
             "baseline_search_matches": 250, "baseline_read_lines": 200},
}
DIFFICULTIES = ("simple", "medium", "complex")
LANGUAGES = ("typescript", "go")
MODEL_TERMINALS = {"completed", "timeout", "call_limit", "token_limit"}
TERMINALS = MODEL_TERMINALS | {"environment_error", "interrupted", "measurement_error"}


class ContractError(ValueError):
    """A saved input or observation cannot satisfy the declared contract."""


def canonical(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)


def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value).encode()).hexdigest()


def file_digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def read_json(path: Path) -> Any:
    def reject_duplicates(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ContractError(f"duplicate JSON key: {key} ({path})")
            result[key] = value
        return result
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=reject_duplicates,
                      parse_constant=lambda value: (_ for _ in ()).throw(ContractError(value)))


def write_json(path: Path, value: Any, *, exclusive: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2, allow_nan=False) + "\n"
    if exclusive:
        with path.open("x", encoding="utf-8") as stream:
            stream.write(text)
    else:
        temporary = path.with_name(path.name + f".{os.getpid()}.tmp")
        temporary.write_text(text, encoding="utf-8")
        temporary.replace(path)


def read_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    rows = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if line.strip():
            try:
                rows.append(json.loads(line))
            except ValueError as exc:
                raise ContractError(f"invalid JSONL {path}:{number}") from exc
    return rows


def append_jsonl(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as stream:
        stream.write(canonical(value) + "\n")
        stream.flush()


def metric(value: float | int | None, unit: str, numerator=None, denominator=None,
           reason: str | None = None, evidence: list[str] | None = None) -> dict:
    if value is None and not reason:
        raise ContractError("null metric requires a reason")
    if numerator is None and denominator is None and value is not None:
        numerator, denominator = value, 1
    return {"value": value, "unit": unit, "numerator": numerator, "denominator": denominator,
            "validity": "valid" if value is not None else "unavailable",
            "missing_reason": reason, "evidence": evidence or []}


def ratio(numerator, denominator, unit="ratio", evidence=None) -> dict:
    if numerator is None or denominator is None:
        return metric(None, unit, numerator, denominator, "missing operand", evidence)
    if denominator == 0:
        return metric(None, unit, numerator, denominator, "zero denominator", evidence)
    return metric(numerator / denominator, unit, numerator, denominator, evidence=evidence)


def percentile(values: list[float], fraction: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    lower = math.floor(position)
    upper = math.ceil(position)
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def change(a, b, unit="percent") -> dict:
    if a is None or b is None:
        return metric(None, unit, b, a, "incomplete comparison")
    if a == 0:
        result = metric(None, unit, b, a, "zero baseline; see absolute_difference")
        result["absolute_difference"] = b - a
        return result
    return metric(100 * (b / a - 1), unit, b, a)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ContractError(message)


def command(args: list[str], cwd: Path | None = None, timeout_seconds=120) -> str:
    result = subprocess.run(args, cwd=cwd, text=True, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=timeout_seconds)
    require(result.returncode == 0, f"command failed ({args[0]}): {result.stderr[-2000:]}")
    return result.stdout


def safe_path(root: Path, relative: str, *, must_exist=True) -> Path:
    require(isinstance(relative, str) and bool(relative), "empty path")
    path = Path(relative)
    require(not path.is_absolute() and ".." not in path.parts, "path outside source")
    require(not any(part in {".git", ".codemap", ".codex"} for part in path.parts), "private source metadata")
    resolved = (root / path).resolve()
    require(resolved.is_relative_to(root.resolve()), "symlink outside source")
    require(not must_exist or resolved.exists(), "source path does not exist")
    return resolved


def question_schedule(dataset: dict, phase: str) -> list[dict]:
    questions = sorted((q for q in dataset["questions"] if q["phase"] == phase), key=lambda q: q["id"])
    rng = random.Random(SPEC["seed"])
    rng.shuffle(questions)
    first_groups = {q["id"]: rng.choice(["A", "B"]) for q in questions}
    schedule = []
    for repeat in range(1, SPEC["phases"][phase]["repeats"] + 1):
        for question in questions:
            first = first_groups[question["id"]]
            groups = [first, "B" if first == "A" else "A"]
            if repeat == 2:
                groups.reverse()
            for group in groups:
                schedule.append({"id": f"{question['id']}-{group}-{repeat}", "question_id": question["id"],
                                 "group": group, "repeat": repeat, "phase": phase})
    return schedule
