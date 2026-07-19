#!/usr/bin/env python3
"""Create development-only relocated manifests without reading removed task files."""
from __future__ import annotations

import json
import pathlib

from common import (
    BENCHMARK_ROOT,
    CORPUS_ROOT,
    ORIGINAL_PRIVATE_MANIFEST_SHA256,
    ORIGINAL_PUBLIC_MANIFEST_SHA256,
    QUALITY_ROOT,
    TASK_IDS,
    write_json,
)


def relocate(value: str) -> str:
    old = "/tmp/codemap-search-quality.7f4a91c2"
    if value == old or value.startswith(old + "/"):
        return str(QUALITY_ROOT) + value[len(old):]
    return value


def main() -> int:
    manifest_root = BENCHMARK_ROOT / "manifests"
    public_path = manifest_root / "public.json"
    private_path = manifest_root / "private.json"
    public = json.loads(public_path.read_text(encoding="utf-8"))
    private = json.loads(private_path.read_text(encoding="utf-8"))
    allowed = set(TASK_IDS)

    public["tasks"] = [
        task for task in public["tasks"]
        if task.get("split") == "development" and task.get("task_id") in allowed
    ]
    public.pop("repeat_task_ids", None)
    private["tasks"] = [
        task for task in private["tasks"]
        if task.get("split") == "development" and task.get("task_id") in allowed
    ]
    if [task["task_id"] for task in public["tasks"]] != list(TASK_IDS):
        raise SystemExit("public development task order or membership changed")
    if [task["task_id"] for task in private["tasks"]] != list(TASK_IDS):
        raise SystemExit("private development task order or membership changed")
    private["corpus"]["cwd"] = str(CORPUS_ROOT)
    for task in private["tasks"]:
        task["question_path"] = relocate(task["question_path"])
        task["answer_path"] = relocate(task["answer_path"])

    write_json(public_path, public)
    write_json(private_path, private)
    write_json(
        QUALITY_ROOT / "harness/provenance/original-manifests.json",
        {
            "original_root": "/private/tmp/codemap-search-quality.7f4a91c2",
            "original_public_manifest_sha256": ORIGINAL_PUBLIC_MANIFEST_SHA256,
            "original_private_manifest_sha256": ORIGINAL_PRIVATE_MANIFEST_SHA256,
            "derivation": "selected only the 14 development entries in original order; preserved entry fields; relocated private absolute paths to the new root",
            "removed_without_use": ["practice questions and answers", "sealed questions and answers"],
        },
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
