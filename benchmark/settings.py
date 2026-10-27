"""One Grafana execution contract for the codemap-search 1.0.0 benchmark."""
from __future__ import annotations

import copy
import os
from pathlib import Path

from .core import digest, file_digest, require

ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parent
# Preparation data (sources, builds, indexes, baselines) may live outside the repository,
# e.g. on a larger disk. prepare, verify and later runs must use the same location.
CACHE = (Path(os.environ["CODEMAP_BENCH_CACHE"]).expanduser().resolve()
         if os.environ.get("CODEMAP_BENCH_CACHE") else ROOT / ".cache")
RUNS = ROOT / "runs"
# Solver-visible paths (source, TMPDIR, shell HOME) live under CACHE, so the path must not name a product or the harness.
IDENTIFYING_PATH_WORDS = ("codemap", "codegraph", "zvec", "graphify", "codebase-memory", "serena", "bench", "route", "campaign")


def require_neutral_cache() -> None:
    found = [word for word in IDENTIFYING_PATH_WORDS if word in str(CACHE).lower()]
    require(not found, f"CODEMAP_BENCH_CACHE 경로가 제품·하네스를 드러냅니다({', '.join(found)}). 중립적인 경로를 쓰세요")


def execution_settings(solver_model: str | None = None) -> dict:
    from .route_contract import load_profile, solver_model as resolve_solver
    from .pi_client import requested_model
    profile = load_profile()
    solver = resolve_solver(solver_model)
    return {"profile": profile["profile"], "preparation_only": True,
            "model": solver["model"], "requested_model": requested_model(solver["provider"], solver["model"]),
            "reasoning_effort": solver["reasoning_effort"],
            "solver_backend": solver["backend"], "provider": solver["provider"],
            "grader_backend": "pi",
            "grader_model": profile["grader"]["model"], "grader_reasoning_effort": profile["grader"]["reasoning_effort"],
            "seed": profile["seed"], "concurrency": profile["concurrency"], "limits": copy.deepcopy(profile["limits"]),
            "grader_timeout_seconds": profile["grader_timeout_seconds"],
            # A grading packet carries the fixed evidence (~67-98 KB per question) plus each cited range; 384 KiB stays near 115K Astra tokens.
            "batch_limits": {"bytes": 384 * 1024},
            **{key: copy.deepcopy(profile[key]) for key in ("native_output_policy", "tool_access", "index_mode")}}


def profiles() -> dict:
    from . import route_contract as routes
    profile = routes.load_profile()
    return {routes.PROFILE: {"name": profile["name"], "question_ids": list(routes.QUESTION_IDS),
                            "repeats": profile["repetitions"], "measurement_modes": profile["measurement_modes"], "preparation_only": True}}


def targets() -> list[dict]:
    from .route_contract import arm_contracts
    return arm_contracts()


def harness_identity() -> str:
    paths = [*ROOT.glob("*.py"), *ROOT.glob("*.mjs"), *[p for p in (ROOT / "data").rglob("*") if p.is_file()]]
    return digest({str(path.relative_to(ROOT)): file_digest(path) for path in sorted(set(paths))})
