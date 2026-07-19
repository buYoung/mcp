#!/usr/bin/env python3
"""Render only one of the 14 fixed development questions into the unchanged prompt."""
from __future__ import annotations

import json
import pathlib
import sys

from common import BENCHMARK_ROOT, QUESTION_SHA256, TASK_IDS, sha256


FORBIDDEN_ANSWER_KEYS = {"answer", "answers", "expected_answer", "reference_answer", "solution", "sealed"}


def render(question: pathlib.Path, expected_sha256: str) -> bytes:
    question = question.resolve()
    allowed_root = (BENCHMARK_ROOT / "questions/development").resolve()
    if question.parent != allowed_root or question.stem not in TASK_IDS:
        raise SystemExit("only the fixed 14 development questions are renderable")
    if QUESTION_SHA256[question.stem] != expected_sha256 or sha256(question) != expected_sha256:
        raise SystemExit("question SHA-256 mismatch")
    raw = question.read_bytes()
    value = json.loads(raw)
    pending = [value]
    while pending:
        current = pending.pop()
        if isinstance(current, dict):
            if FORBIDDEN_ANSWER_KEYS & {str(key).lower() for key in current}:
                raise SystemExit("answer-bearing question field rejected")
            pending.extend(current.values())
        elif isinstance(current, list):
            pending.extend(current)
    template = (pathlib.Path(__file__).resolve().parents[1] / "templates/prompt.txt").read_bytes()
    marker = b"__QUESTION_JSON__"
    if template.count(marker) != 1:
        raise SystemExit("prompt marker contract changed")
    return template.replace(marker, raw.decode("utf-8").strip().encode("utf-8")).rstrip(b"\n")


def main() -> int:
    if len(sys.argv) != 4:
        raise SystemExit("usage: render_prompt.py QUESTION EXPECTED_SHA256 OUTPUT")
    output = pathlib.Path(sys.argv[3])
    output.write_bytes(render(pathlib.Path(sys.argv[1]), sys.argv[2]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
