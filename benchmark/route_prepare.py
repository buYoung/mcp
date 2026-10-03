"""Integrated Grafana readiness gate. This module has no solver/grader launch path."""
from __future__ import annotations

import argparse
import platform
import subprocess
import sys
import time
from pathlib import Path

from . import resources, route_contract as routes, route_grading, route_metrics, route_transport, settings
from . import tool_artifacts, tool_indexes
from .core import ContractError, command, digest, file_digest, read_json, require, write_json

HANDOFFS = ("01-contract.json", "02-delivery.json", "03-artifacts.json", "04-indexes.json", "05-grading.json", "06-metrics.json")


def execution_spec(state: dict | None = None) -> dict:
    profile = routes.load_profile()
    spec = settings.execution_settings()
    spec.update(profile=routes.PROFILE, preparation_only=True, concurrency=1, seed=profile["seed"],
                model=profile["solver"]["model"], reasoning_effort=profile["solver"]["reasoning_effort"],
                grader_model=profile["grader"]["model"], grader_reasoning_effort=profile["grader"]["reasoning_effort"],
                limits=profile["limits"],
                native_output_policy=profile["native_output_policy"], tool_access=profile["tool_access"],
                index_mode=profile["index_mode"])
    if state and state["id"] == "rg":
        spec["native_agent"] = state["native_agent"]
    return spec


def check_handoffs() -> dict:
    identity = routes.contract()["contract_hash"]
    hashes = {}
    for name in HANDOFFS:
        path = routes.EVIDENCE / name
        value = read_json(path)
        require(value["contract_hash"] == identity, f"공유 계약 변경: {name}")
        for relative, checksum in value["implementation_hashes"].items():
            require(file_digest(settings.REPOSITORY / relative) == checksum, f"구현 변경 후 근거 재검증 필요: {relative}")
        for relative, checksum in value["dependency_hashes"].items():
            require(file_digest(routes.EVIDENCE / relative) == checksum, f"선행 근거 변경: {name} → {relative}")
        hashes[name] = file_digest(path)
    delivery = read_json(routes.EVIDENCE / "02-delivery.json")["raw_relay_checks"]
    for path_key, hash_key in (("protocol_log", "protocol_sha256"), ("relay_log", "relay_sha256")):
        require(file_digest(Path(delivery[path_key])) == delivery[hash_key], "응답 전달 검증 로그 변경")
    grading = read_json(routes.EVIDENCE / "05-grading.json")
    for section in ("judgment_schema", "packet_contract"):
        for item in grading[section].values():
            require(file_digest(Path(item["path"])) == item["sha256"], "채점 계약 산출물 변경")
    metrics = read_json(routes.EVIDENCE / "06-metrics.json")["result_schema"]
    require(file_digest(Path(metrics["preview"])) == metrics["preview_sha256"], "측정 계약 산출물 변경")
    return hashes


def verify_readiness(record: dict) -> None:
    require(record["status"] == "prepared_not_executed" and record["technical_ready"] is True, "기술적 준비 미완료")
    require(record["execution_authorized"] is False, "준비 전용 경계 변경")
    require(record["contract_hash"] == routes.contract()["contract_hash"], "route 실행 계약 변경")
    require(record["harness_sha256"] == settings.harness_identity(), "준비 이후 하네스 변경")
    require(record["handoff_hashes"] == check_handoffs(), "준비 근거 변경")
    for check in record["checks"]["offline_commands"]:
        require(check["exit_code"] == 0 and file_digest(Path(check["log"])) == check["log_sha256"], "오프라인 검증 근거 변경")
    require(record["host"]["codex_version"] == command(["codex", "--version"]).strip(), "Codex 버전 변경")
    artifacts = tool_artifacts.load_artifacts()
    corpus = read_json(tool_indexes.CORPUS)
    require(file_digest(tool_indexes.CORPUS) == record["source_manifest_sha256"], "source manifest 변경")
    for ident in routes.ARM_IDS:
        state = record["arms"][ident]
        tool_indexes.verify_probe(state, artifacts)
        require(state["artifact_sha256"] == digest(artifacts[ident]), "실행 artifact 변경")
        require(digest(tool_indexes.tracked_manifest(Path(state["source"]), corpus["files"])) == corpus["source_sha256"], "source 변경")
        for part in ("source", "home"):
            require(digest(tool_artifacts.tree_files(Path(state[part]))) == state["baseline"]["state_sha256"][part], "native 초기 상태 변경")
            require(digest(tool_artifacts.tree_files(Path(state["baseline"]["paths"][part]))) == state["baseline"]["state_sha256"][part],
                    "복원용 native baseline 변경")
        require(file_digest(Path(state["native_inventory"])) == state["native_inventory_sha256"], "native tools/defaults 기록 변경")
        require(file_digest(Path(state["probe_log"])) == state["probe_log_sha256"], "native 중립 조회 로그 변경")
    require(record["planned_runs"] == routes.schedule() and len(record["planned_runs"]) == 18, "계획 슬롯 변경")
    require(set(record["solver_inputs"]) == {row["id"] for row in record["planned_runs"]}, "풀이 입력 슬롯 누락")
    for row in record["planned_runs"]:
        item = record["solver_inputs"][row["id"]]
        prompt = routes.solver_prompt(row["question_id"], Path(record["arms"][row["arm"]]["source"]), row["arm"])
        require(Path(item["path"]).read_text() == prompt and file_digest(Path(item["path"])) == item["sha256"], "풀이 입력 변경")
    require(record["execution_specs"] == {ident: execution_spec(state) for ident, state in record["arms"].items()}, "비교군별 실행 설정 변경")
    require(all(record[name] == 0 for name in ("solver_runs", "grader_runs", "calibration_runs")), "준비에 실행 결과 혼입")


def build_plan(emit=print) -> dict:
    """Validate existing prepared evidence and create only a reviewable, unexecuted plan."""
    handoffs = check_handoffs()
    validation = routes.PREPARATION / "validation" / str(time.time_ns())
    offline_commands = []
    for name, args in (
        ("existing-suites", [sys.executable, "-m", "unittest", "benchmark.checks", "benchmark.cui_checks"]),
        ("cui-syntax", ["node", "--check", "benchmark/start.mjs"]),
    ):
        emit(f"오프라인 검증 ({settings.REPOSITORY}): {' '.join(args)}")
        offline_commands.append(tool_artifacts.run_logged(args, settings.REPOSITORY, validation / f"{name}.log", timeout=180))
    indexes = read_json(routes.EVIDENCE / "04-indexes.json")
    artifacts = read_json(routes.EVIDENCE / "03-artifacts.json")["artifacts"]
    host = resources.environment()
    host.update(system=platform.platform(), machine=platform.machine(), processors=__import__("os").cpu_count())
    profile = routes.load_profile()
    condition = {"contract_hash": routes.contract()["contract_hash"], "harness_sha256": settings.harness_identity(),
                 "artifacts": {ident: value["artifact_sha256"] for ident, value in indexes["prepared_states"].items()},
                 "source_sha256": indexes["source_manifest"]["source_sha256"], "host": host,
                 "native_conditions": {ident: {"inventory": s["native_inventory_sha256"], "baseline": s["baseline"]["state_sha256"],
                                                "tool_condition": s["tool_condition"], "env": s["env"], "probe_identity": s["probe_identity"]}
                                       for ident, s in indexes["prepared_states"].items()}}
    plan_id = digest(condition)[:32]
    root = routes.PREPARATION / "plans" / plan_id
    solver_inputs = {}
    for row in routes.schedule():
        path = root / "solver-inputs" / f"{row['id']}.txt"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(routes.solver_prompt(row["question_id"], Path(indexes["prepared_states"][row["arm"]]["source"]), row["arm"]))
        solver_inputs[row["id"]] = {"path": str(path), "sha256": file_digest(path), "question_id": row["question_id"]}
    record = {"schema_version": 1, "profile": routes.PROFILE, "status": "prepared_not_executed", "technical_ready": True,
              "execution_authorized": False, "plan_id": plan_id, "contract_hash": condition["contract_hash"],
              "harness_sha256": condition["harness_sha256"], "handoff_hashes": handoffs,
              "input_hashes": routes.contract()["input_hashes"], "source_manifest_sha256": file_digest(tool_indexes.CORPUS),
              "host": host, "models": {"solver": profile["solver"], "grader": profile["grader"]},
              "actual_versions": {ident: artifact["actual_version"] for ident, artifact in artifacts.items()},
              "arms": indexes["prepared_states"], "planned_runs": routes.schedule(),
              "execution_specs": {ident: execution_spec(state) for ident, state in indexes["prepared_states"].items()},
              "solver_inputs": solver_inputs,
              "policy_decisions": profile["policy_decisions"],
              "checks": {"component_evidence": "verified", "planned_slots": 18, "offline_commands": offline_commands},
              "invalidation_rules": ["frozen inputs", "source bytes", "profile/models/effort/limits", "harness code",
                                     "binaries and installed dependencies", "embedding weights", "native index state",
                                     "native tool schema/default record", "Codex version"],
              "solver_runs": 0, "grader_runs": 0, "calibration_runs": 0,
              "limitations": ["Subject build retains its actual version; it is not relabeled 1.0.0.",
                              "Real solver delivery, account model availability, judge calibration and empirical difficulty are untested.",
                              "OS page cache is uncontrolled; native index coverage and response limits are retained."]}
    verify_readiness(record)
    write_json(root / "readiness.json", record)
    route_metrics.report(root, record, [], {})
    spec = execution_spec()
    write_json(settings.CACHE / "plans" / f"{plan_id}.json", {
        "profile": routes.PROFILE, "preparation_only": True, "readiness": str(root / "readiness.json"),
        "harness_sha256": condition["harness_sha256"], "spec": spec, "execution_specs": record["execution_specs"],
        "planned_runs": record["planned_runs"]})
    routes.publish_handoff("07-readiness.json", record,
        implementation=["benchmark/route_prepare.py", "benchmark/cli.py", "benchmark/start.mjs", "benchmark/ready.py", "benchmark/execution.py"],
        dependencies=list(HANDOFFS))
    emit(f"준비 완료: 3문항 × 6개 도구 × 1회 = 18회 예정. 실제 풀이·채점 0회.")
    return {"plan_id": plan_id, "profile_name": profile["name"], "preparation_only": True,
            "status": record["status"], "technical_ready": True,
            "targets": [{"id": ident, "name": "기본 에이전트 (검색 제품 없음)" if ident == "rg" else ident,
                         "source_sha256": s["source_sha256"], "tool_condition": s["tool_condition"],
                         "actual_version": artifacts[ident]["actual_version"]}
                        for ident, s in record["arms"].items()],
            "questions": 3, "repeats": 1, "runs": 18, "spec": spec, "codex_version": host["codex_version"],
            "output_root": str(root), "readiness": str(root / "readiness.json"),
            "report": str(root / "report/results.md"), "solver_runs": 0, "grader_runs": 0}


def prepare_all(emit=print) -> dict:
    """Cache-aware local preparation; no implicit start/resume or model launch."""
    with resources.lease(routes.PREPARATION / ".prepare.lock"):
        harness_at_start = settings.harness_identity()
        routes.publish_contract()
        for ident in routes.ARM_IDS:
            tool_artifacts.prepare_one(ident, emit)
        tool_artifacts.publish_artifacts()
        route_transport.publish_delivery()
        route_grading.publish_grading()
        tool_indexes.prepare_source(emit)
        for ident in routes.ARM_IDS:
            tool_indexes.prepare_index(ident, emit)
            if not tool_indexes.probe_is_current(ident):
                tool_indexes.probe_index(ident, emit)
        tool_indexes.publish_indexes()
        route_metrics.publish_metrics()
        require(settings.harness_identity() == harness_at_start,
                "준비 중 하네스 파일이 변경되었습니다. 현재 코드로 prepare를 다시 수행하세요")
        return build_plan(emit)


def main() -> int:
    parser = argparse.ArgumentParser(description="Grafana 18회 계획의 준비 전용 게이트; 실제 모델 실행 없음")
    parser.add_argument("action", choices=["prepare", "finalize", "verify"], nargs="?", default="prepare")
    args = parser.parse_args()
    try:
        if args.action == "prepare":
            value = prepare_all(lambda message: print(message, flush=True))
        elif args.action == "finalize":
            value = build_plan(lambda message: print(message, flush=True))
        else:
            record = read_json(routes.EVIDENCE / "07-readiness.json")
            verify_readiness(record)
            value = {"status": "prepared_not_executed", "plan_id": record["plan_id"]}
        print(f"{value['status']}: {value['plan_id']}")
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        print(f"준비 미완료: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
