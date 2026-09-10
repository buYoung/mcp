"""Current execution settings, separate from the immutable V2 dataset contract."""
from __future__ import annotations

import copy
import re
from pathlib import Path

from .v2.core import SPEC, digest, file_digest, read_json, require
from .v2.dataset import validate_dataset

ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parent
CACHE = ROOT / ".cache"
RUNS = ROOT / "runs"


def load_config() -> dict:
    config = read_json(ROOT / "config.json")
    require(set(config) == {"model", "reasoning_effort", "grader_model", "grader_reasoning_effort",
                            "limits", "concurrency", "seed", "candidates"}, "지원하지 않는 벤치 설정 키")
    for key in ("model", "grader_model"):
        require(isinstance(config[key], str) and config[key].strip(), f"모델 설정 오류: {key}")
    for key in ("reasoning_effort", "grader_reasoning_effort"):
        require(config[key] in {"minimal", "low", "medium", "high", "xhigh"}, f"추론 강도 설정 오류: {key}")
    require(set(config["limits"]) == set(SPEC["limits"]), "실행 한도 키 오류")
    for key, value in {**config["limits"], "concurrency": config["concurrency"]}.items():
        require(type(value) is int and value > 0, f"양의 정수가 필요합니다: {key}")
    require(type(config["seed"]) is int, "seed는 정수여야 합니다")
    identifiers = {"A", "current"}
    require(isinstance(config["candidates"], list), "candidates는 배열이어야 합니다")
    for candidate in config["candidates"]:
        require(isinstance(candidate, dict) and set(candidate) in ({"id", "name", "path"}, {"id", "name", "git_ref"}),
                "후보는 id, name 및 path 또는 git_ref 하나로 등록하세요")
        ident = candidate["id"]
        require(isinstance(ident, str) and re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}", ident)
                and ident not in identifiers, "중복되거나 안전하지 않은 후보 ID")
        identifiers.add(ident)
        require(all(isinstance(value, str) and value.strip() and "\x00" not in value for value in candidate.values()),
                "후보 설정은 비어 있지 않은 문자열이어야 합니다")
    return config


def execution_settings(config: dict) -> dict:
    result = copy.deepcopy(SPEC)
    for key in ("codex_version", "product_version", "product_commit"):
        result.pop(key)  # Original SPEC stays untouched for historical dataset hashes.
    result.update({key: value for key, value in config.items() if key != "candidates"})
    return result


def profiles() -> dict:
    result = read_json(ROOT / "data/profiles.json")
    expected = {"candidate": (3, 3), "validation": (10, 1), "formal": (30, 2)}
    require(set(result) == set(expected), "평가 유형 변경")
    for key, (count, repeats) in expected.items():
        require(len(result[key]["question_ids"]) == len(set(result[key]["question_ids"])) == count
                and result[key]["repeats"] == repeats, f"고정 평가량 변경: {key}")
    require(set(result["candidate"]["question_ids"]) < set(result["validation"]["question_ids"])
            < set(result["formal"]["question_ids"]), "문항 부분집합 관계가 깨졌습니다")
    return result


def dataset_for(profile: str, source: Path | None = None) -> dict:
    original = read_json(ROOT / "v2/data/dataset.json")
    validate_dataset(original, source)
    groups = profiles()
    require(profile in groups, "알 수 없는 평가 유형")
    lookup = {q["id"]: q for q in original["questions"]}
    require(set(groups["formal"]["question_ids"]) == {q["id"] for q in original["questions"] if q["phase"] == "main"},
            "정식 Grafana 문항 변경")
    return {**original, "questions": [lookup[key] for key in groups[profile]["question_ids"]]}


def targets(config: dict) -> list[dict]:
    return [{"id": "A", "name": "A · 기본 도구", "group": "A"},
            {"id": "current", "name": "현재 버전 · 작업 파일", "group": "B", "path": "apps/codemap-search"},
            *[{**candidate, "group": "B"} for candidate in config["candidates"]]]


def harness_identity() -> str:
    paths = [*ROOT.glob("*.py"), *ROOT.glob("*.mjs"), *ROOT.glob("data/*.json"), *ROOT.glob("v2/*.py")]
    return digest({str(path.relative_to(ROOT)): file_digest(path) for path in sorted(paths)})
