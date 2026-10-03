"""One Grafana execution contract for the codemap-search 1.0.0 benchmark."""
from __future__ import annotations

import copy
from pathlib import Path

from .core import digest, file_digest

ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parent
CACHE = ROOT / ".cache"
RUNS = ROOT / "runs"


def execution_settings() -> dict:
    from .route_contract import load_profile
    profile = load_profile()
    return {"profile": profile["profile"], "preparation_only": True,
            "model": profile["solver"]["model"], "reasoning_effort": profile["solver"]["reasoning_effort"],
            "grader_model": profile["grader"]["model"], "grader_reasoning_effort": profile["grader"]["reasoning_effort"],
            "seed": profile["seed"], "concurrency": profile["concurrency"], "limits": copy.deepcopy(profile["limits"]),
            "batch_limits": {"bytes": 160 * 1024},
            **{key: copy.deepcopy(profile[key]) for key in ("native_output_policy", "tool_access", "index_mode")}}


def profiles() -> dict:
    from . import route_contract as routes
    profile = routes.load_profile()
    return {routes.PROFILE: {"name": profile["name"], "question_ids": list(routes.QUESTION_IDS),
                            "repeats": profile["repetitions"], "preparation_only": True}}


def targets() -> list[dict]:
    from .route_contract import arm_contracts
    return arm_contracts()


def harness_identity() -> str:
    paths = [*ROOT.glob("*.py"), *ROOT.glob("*.mjs"), *[p for p in (ROOT / "data").rglob("*") if p.is_file()]]
    return digest({str(path.relative_to(ROOT)): file_digest(path) for path in sorted(set(paths))})
