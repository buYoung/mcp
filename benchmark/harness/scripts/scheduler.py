#!/usr/bin/env python3
"""The immutable 14-task, three-trial, two-arm baseline schedule."""
from __future__ import annotations

import json
import sys

from common import ARMS, BENCHMARK_ROOT, QUESTION_SHA256, TASK_IDS, TRIAL_IDS


PAIR_ORDER_RULE = (
    "r1: first 7 task positions B1-first; r2: exact inverse; "
    "r3: even task positions B1-first. Each trial is 7 B1-first/7 B2-first; "
    "across three trials 7 tasks are B1-first twice and 7 are B2-first twice."
)


def pair_order(task_index: int, trial_id: str) -> tuple[str, str]:
    if trial_id == "r1":
        is_b1_first = task_index < 7
    elif trial_id == "r2":
        is_b1_first = task_index >= 7
    elif trial_id == "r3":
        is_b1_first = task_index % 2 == 0
    else:
        raise ValueError(f"unknown trial: {trial_id}")
    return ("B1", "B2") if is_b1_first else ("B2", "B1")


def schedule() -> list[dict]:
    entries: list[dict] = []
    sequence = 0
    for trial_id in TRIAL_IDS:
        for task_index, task_id in enumerate(TASK_IDS):
            order = pair_order(task_index, trial_id)
            pair_id = f"{task_id}-{trial_id}"
            question = BENCHMARK_ROOT / "questions/development" / f"{task_id}.json"
            for order_index, arm in enumerate(order):
                sequence += 1
                entries.append(
                    {
                        "sequence": sequence,
                        "task_id": task_id,
                        "split": "development",
                        "trial_id": trial_id,
                        "criterion": arm,
                        "pair_id": pair_id,
                        "pair_order": list(order),
                        "pair_order_index": order_index,
                        "pair_order_rule": PAIR_ORDER_RULE,
                        "question_path": str(question),
                        "question_sha256": QUESTION_SHA256[task_id],
                    }
                )
    return entries


def validate(entries: list[dict]) -> None:
    keys = {(row["task_id"], row["trial_id"], row["criterion"]) for row in entries}
    expected = {(task, trial, arm) for task in TASK_IDS for trial in TRIAL_IDS for arm in ARMS}
    if len(entries) != 84 or keys != expected:
        raise RuntimeError("schedule is not exactly 14 tasks x 3 trials x 2 arms")
    for trial in TRIAL_IDS:
        pairs = [row for row in entries if row["trial_id"] == trial and row["pair_order_index"] == 0]
        if sum(row["criterion"] == "B1" for row in pairs) != 7:
            raise RuntimeError(f"{trial} is not balanced 7/7 by first arm")
    counts = {
        task: sum(
            row["criterion"] == "B1" and row["pair_order_index"] == 0
            for row in entries if row["task_id"] == task
        )
        for task in TASK_IDS
    }
    if sorted(counts.values()) != [1] * 7 + [2] * 7:
        raise RuntimeError("cross-trial first-arm exposure is not balanced")


def main() -> int:
    entries = schedule()
    validate(entries)
    if len(sys.argv) == 1 or sys.argv[1] == "list":
        print(json.dumps(entries, ensure_ascii=False, indent=2, sort_keys=True))
        return 0
    if len(sys.argv) == 5 and sys.argv[1] == "resolve":
        task_id, trial_id, arm = sys.argv[2:]
        match = [row for row in entries if (row["task_id"], row["trial_id"], row["criterion"]) == (task_id, trial_id, arm)]
        if len(match) != 1:
            raise SystemExit("rejected task/trial/arm outside the sealed 84-session schedule")
        print(json.dumps(match[0], ensure_ascii=False, sort_keys=True))
        return 0
    raise SystemExit("usage: scheduler.py [list|resolve TASK_ID r1|r2|r3 B1|B2]")


if __name__ == "__main__":
    raise SystemExit(main())

