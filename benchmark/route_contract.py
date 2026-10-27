"""Versioned Grafana route inputs. Loading or preparing never invokes a model."""
from __future__ import annotations

import datetime
import random
from pathlib import Path

from . import settings
from .core import digest, file_digest, read_json, require, write_json

PROFILE = "grafana-routes"
SOURCE_COMMIT = "c0a598f5a49abf2014f67e806075d8ad267c9efb"
DATASET_MANIFEST_SHA256 = "0739674c1fbb46bc15bcf0f4819c20a50e9f3aa2b5702ce81b21dee657a9ddf4"
TARGET_LOCK_SHA256 = "de55409dc07a18364e1701fb63fffda315e576aad3c4761a22efa245f7b055b8"
QUESTION_IDS = ("git-oauth-authorization", "plugin-http-route-bridge")
REPETITIONS = 3
MEASUREMENT_MODES = ("direct", "codemode")
ARM_IDS = ("codemap-search", "codegraph", "zvec-grep", "graphify", "codebase-memory-mcp", "serena", "rg")
DATA = settings.ROOT / "data/grafana-routes-v4"
TARGETS = settings.ROOT / "data/grafana-comparison-targets-v1"
PROFILE_PATH = settings.ROOT / "data/grafana-execution-v1.json"
EVIDENCE = settings.REPOSITORY / "docs/briefs/evidence/bench-ready"
PREPARATION = settings.CACHE / "route-preparation"


def verify_bundle(root: Path) -> dict:
    """Checksum lists are data, never shell commands or paths outside the bundle."""
    listed = {}
    for line in (root / "SHA256SUMS").read_text().splitlines():
        checksum, name = line.split(maxsplit=1)
        name = name.strip().removeprefix("*")
        require(Path(name).name == name and name not in listed, "잘못된 고정 파일 목록")
        path = root / name
        require(path.is_file() and file_digest(path) == checksum, f"고정 파일 변경: {path}")
        listed[name] = checksum
    require(set(listed) | {"SHA256SUMS"} == {p.name for p in root.iterdir() if p.is_file()},
            f"고정 패키지 파일 목록 불일치: {root}")
    return {str((root / name).relative_to(settings.REPOSITORY)): file_digest(root / name)
            for name in sorted({*listed, "SHA256SUMS"})}


def frozen_inputs() -> dict:
    hashes = {**verify_bundle(DATA), **verify_bundle(TARGETS)}
    require(file_digest(DATA / "manifest.json") == DATASET_MANIFEST_SHA256, "Grafana manifest identity 변경")
    require(file_digest(TARGETS / "targets.lock.json") == TARGET_LOCK_SHA256, "비교 대상 lock identity 변경")
    manifest = read_json(DATA / "manifest.json")
    lock = read_json(TARGETS / "targets.lock.json")
    require(lock["dataset"]["manifest_sha256"] == DATASET_MANIFEST_SHA256, "dataset/lock 결합 오류")
    require(manifest["source"]["commit"] == lock["dataset"]["source"]["commit"] == SOURCE_COMMIT,
            "고정 Grafana commit 불일치")
    for name, metadata in manifest["files"].items():
        path = DATA / name
        require(file_digest(path) == metadata["sha256"] and path.stat().st_size == metadata["bytes"],
                f"manifest 파일 메타데이터 불일치: {name}")
    require(tuple(t["id"] for t in lock["comparisons"]) == ARM_IDS[1:], "비교군 변경")
    return {"hashes": hashes, "manifest": manifest, "lock": lock}


def load_dataset() -> dict:
    """Private rubric dataset; never pass this object to the solver."""
    frozen_inputs()
    dataset = read_json(DATA / "dataset.json")
    questions = read_json(DATA / "questions.json")
    evidence = read_json(DATA / "evidence.json")
    require(dataset["source"]["commit"] == evidence["source"]["commit"] == SOURCE_COMMIT,
            "route source identity 불일치")
    require(tuple(q["id"] for q in dataset["questions"]) == QUESTION_IDS, "route 문항 변경")
    require(questions == {"dataset_id": dataset["dataset_id"], "questions": [
        {"id": q["id"], "prompt": q["prompt"]} for q in dataset["questions"]]}, "풀이 입력 분리 오류")
    seen = set()
    for question in dataset["questions"]:
        counts = (question["core_fact_count"], question["extended_fact_count"])
        for level, count in zip(("core", "extended"), counts, strict=True):
            require(sum(f["level"] == level for f in question["facts"]) == count, "고정 fact 개수 변경")
        for fact in question["facts"]:
            require(fact["id"] not in seen, "중복 fact ID")
            seen.add(fact["id"])
            require(set(fact["evidence_ids"]) <= evidence["evidence"].keys(), "끊긴 fact evidence 참조")
    return dataset


def solver_questions() -> list[dict]:
    load_dataset()
    return read_json(DATA / "questions.json")["questions"]


def solver_prompt(question_id: str, source: Path, arm: str | None = None) -> str:
    from .execution import answer_prompt
    question = next(q for q in solver_questions() if q["id"] == question_id)
    prompt = answer_prompt(load_profile(), question)
    if arm == "rg":
        prompt = prompt.replace(
            "코드 접근은 navigation MCP에 노출된 도구만 사용한다. 다른 파일 접근·셸·네트워크·하위 에이전트는 사용하지 않는다.",
            "기본 코딩 에이전트 도구로 제공된 소스를 탐색한다. 사용할 명령과 탐색 방법은 스스로 선택한다. 탐색은 제공된 소스 루트 안에서 수행하고 네트워크와 하위 에이전트는 사용하지 않는다.")
    return prompt + f"\n\n탐색 소스 루트: {source.resolve()}\n"


def load_profile() -> dict:
    profile = read_json(PROFILE_PATH)
    require(profile["schema_version"] == 1 and profile["profile"] == PROFILE, "route 프로필 버전 오류")
    require(tuple(profile["arm_ids"]) == ARM_IDS and profile["repetitions"] == REPETITIONS, "문항별 반복 계획 변경")
    require(tuple(profile["measurement_modes"]) == MEASUREMENT_MODES, "측정 모드 계약 변경")
    require(profile["concurrency"] == 4 and type(profile["seed"]) is int, "route 동시 실행 수/seed 변경")
    require(profile["solver"] == {"model": "gpt-6.1-sol", "reasoning_effort": "medium"}, "풀이 모델 계약 변경")
    require(profile["solver_choices"] == [
        {"model": "gpt-6.1-sol", "name": "GPT 6.1 Sol", "backend": "pi", "reasoning_effort": "medium", "provider": "openai-codex"},
        {"model": "claude-opus-5-5", "name": "Claude Opus 5.5", "backend": "pi", "reasoning_effort": "medium", "provider": "claude-bridge"},
    ], "풀이 모델 선택 계약 변경")
    require(profile["grader"] == {"model": "gpt-6-astra", "reasoning_effort": "high"}, "채점 모델 계약 변경")
    require(profile["limits"] is None and "token_limit_basis" not in profile
            and profile["grader_timeout_seconds"] == 300
            and profile["execution_authorized"] is False,
            "준비 전용 실행 경계 변경")
    require(profile["baseline"]["arm"] == "rg" and profile["baseline"]["mode"] == "native_agent"
            and profile["baseline"]["search_product_mcp"] is False, "기본 에이전트 기준군 계약 변경")
    require(profile["index_mode"]["external_ai"] is False and profile["tool_access"]["default"] == "native_only",
            "확정된 도구/색인 정책 변경")
    require(profile["native_output_policy"] == {
        "product_defaults": True, "harness_byte_limit": None, "harness_line_limit": None,
        "automatic_pagination": False, "enrichment": False,
        "client_delivery": "record_observed_limits_and_unknowns_separately"}, "기본 응답 정책 변경")
    return profile


def solver_model(model: str | None = None) -> dict:
    profile = load_profile()
    selected = profile["solver"]["model"] if model is None else model
    choices = {item["model"]: item for item in profile["solver_choices"]}
    require(selected in choices, f"지원하지 않는 풀이 모델: {selected}")
    return {key: value for key, value in choices[selected].items() if key != "name"}


def arm_contracts() -> list[dict]:
    locked = frozen_inputs()["lock"]
    subject = locked["subject"]
    return [{"id": "codemap-search", "name": "codemap-search", "role": "subject",
             "path": "apps/codemap-search", **{key: subject[key] for key in ("version", "git_ref", "commit")},
             "pin_status": "release_source_frozen_binary_at_preparation"},
            *[{**{key: row[key] for key in ("id", "name", "role", "repository", "version", "release_tag", "commit", "source_archive")},
               **({"name": "기본 에이전트 (검색제품 없음)", "role": "native_agent_baseline"} if row["id"] == "rg" else {})}
              for row in locked["comparisons"]]]


def contract() -> dict:
    profile = load_profile()
    inputs = frozen_inputs()
    value = {"schema_version": 1, "profile": profile, "input_hashes": inputs["hashes"],
             "source_commit": SOURCE_COMMIT, "question_ids": list(QUESTION_IDS), "arms": arm_contracts()}
    value["contract_hash"] = digest(value)
    return value


def execution_key(arm: str, mode: str) -> str:
    require(arm in ARM_IDS and mode in MEASUREMENT_MODES, "unknown arm/mode")
    return f"{arm}/{mode}"


def schedule() -> list[dict]:
    profile = load_profile()
    rng = random.Random(profile["seed"])
    rows = []
    # Each repetition is a full question x arm round, so a stopped campaign keeps whole rounds comparable.
    for attempt in range(1, profile["repetitions"] + 1):
        questions = list(QUESTION_IDS)
        rng.shuffle(questions)
        for question in questions:
            conditions = [(arm, mode) for arm in ARM_IDS for mode in profile["measurement_modes"]]
            rng.shuffle(conditions)
            rows.extend({"id": f"{question}-{arm}-{mode}-{attempt}", "question_id": question, "arm": arm,
                         "mode": mode, "attempt": attempt, "status": "planned"} for arm, mode in conditions)
    require(len(rows) == len({r["id"] for r in rows}) == len(QUESTION_IDS) * len(ARM_IDS) * profile["repetitions"] * len(MEASUREMENT_MODES),
            "계획 슬롯 오류")
    return rows


def implementation_hashes(names: list[str]) -> dict:
    return {name: file_digest(settings.REPOSITORY / name) for name in names}


def publish_handoff(name: str, payload: dict, *, implementation: list[str], dependencies: list[str] = ()) -> dict:
    shared = contract()
    result = {"schema_version": 1, "recorded_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "contract_hash": shared["contract_hash"], "input_hashes": shared["input_hashes"], **payload,
              "implementation_hashes": implementation_hashes(implementation),
              "dependency_hashes": {path: file_digest(EVIDENCE / path) for path in dependencies}}
    write_json(EVIDENCE / name, result)
    return result


def publish_contract() -> dict:
    value = contract()
    dataset = load_dataset()
    profile = value["profile"]
    return publish_handoff("01-contract.json", {
        **value, "status": "verified", "solver": profile["solver"], "solver_choices": profile["solver_choices"],
        "grader": profile["grader"],
        "repetitions": profile["repetitions"], "planned_solver_runs": len(schedule()), "limits": profile["limits"],
        "policy_decisions": profile["policy_decisions"], "tool_access": profile["tool_access"],
        "contract_paths": {"profile": str(PROFILE_PATH.relative_to(settings.REPOSITORY)),
                           "private_dataset": str((DATA / 'dataset.json').relative_to(settings.REPOSITORY)),
                           "solver_questions": str((DATA / 'questions.json').relative_to(settings.REPOSITORY))},
        "fact_counts": {q["id"]: {level: sum(f["level"] == level for f in q["facts"])
                                   for level in ("core", "extended")} for q in dataset["questions"]},
        "checks": {"bundle_hashes": "passed", "prompt_separation": "passed", "planned_unique_keys": len(schedule()),
                   "profiles": sorted(settings.profiles())},
        "solver_runs": 0, "grader_runs": 0,
        "limitations": ["Full source validation belongs to index preparation.",
                        "Exact model account availability has not been exercised."]},
        implementation=["benchmark/route_contract.py", "benchmark/settings.py", "benchmark/data/grafana-execution-v1.json", "benchmark/execution.py", "benchmark/runner.py"])


if __name__ == "__main__":
    record = publish_contract()
    print(f"{record['status']}: {record['planned_solver_runs']} planned slots; solver/grader calls: 0")
