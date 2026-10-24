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
from .campaign_store import campaign_lease, durable_bytes, durable_json, immutable_copy
from .core import ContractError, command, digest, file_digest, read_json, require, write_json

HANDOFFS = ("01-contract.json", "02-delivery.json", "03-artifacts.json", "04-indexes.json", "05-grading.json", "06-metrics.json")


def execution_spec(state: dict | None = None, *, solver_model: str | None = None) -> dict:
    profile = routes.load_profile()
    spec = settings.execution_settings(solver_model)
    spec.update(profile=routes.PROFILE, preparation_only=True, seed=profile["seed"],
                grader_model=profile["grader"]["model"], grader_reasoning_effort=profile["grader"]["reasoning_effort"],
                limits=profile["limits"],
                native_output_policy=profile["native_output_policy"], tool_access=profile["tool_access"],
                index_mode=profile["index_mode"])
    if state and state["id"] == "rg":
        spec["native_agent"] = state["native_agent"]
    return spec


def readiness_path(solver_model: str | None = None) -> Path:
    solver = routes.solver_model(solver_model)
    suffix = "" if solver["model"] == routes.load_profile()["solver"]["model"] else "-" + solver["model"]
    return routes.EVIDENCE / f"07-readiness{suffix}.json"


def handoff_names(solver_model: str | None = None) -> tuple[str, ...]:
    backend = routes.solver_model(solver_model)["backend"]
    return tuple(tool_indexes.index_handoff(backend) if name == "04-indexes.json" else name for name in HANDOFFS)


def check_handoffs(solver_model: str | None = None, *, snapshot: Path | None = None,
                   evidence_files: dict | None = None) -> dict:
    root = snapshot or routes.EVIDENCE
    def evidence_path(name):
        item = (evidence_files or {}).get(name)
        if item:
            path = Path(item["path"])
            require(file_digest(path) == item["sha256"], "불변 준비 근거 변경")
            return path
        require(snapshot is None, f"snapshot에 없는 근거: {name}")
        return Path(name)
    identity = routes.contract()["contract_hash"]
    hashes = {}
    for name in handoff_names(solver_model):
        path = root / name
        value = read_json(path)
        require(value["contract_hash"] == identity, f"공유 계약 변경: {name}")
        for relative, checksum in value["implementation_hashes"].items():
            require(file_digest(settings.REPOSITORY / relative) == checksum, f"구현 변경 후 근거 재검증 필요: {relative}")
        for relative, checksum in value["dependency_hashes"].items():
            require(file_digest(root / relative) == checksum, f"선행 근거 변경: {name} → {relative}")
        hashes[name] = file_digest(path)
    delivery = read_json(root / "02-delivery.json")["raw_relay_checks"]
    for path_key, hash_key in (("protocol_log", "protocol_sha256"), ("relay_log", "relay_sha256")):
        require(file_digest(evidence_path(delivery[path_key])) == delivery[hash_key], "응답 전달 검증 로그 변경")
    grading = read_json(root / "05-grading.json")
    for section in ("judgment_schema", "packet_contract"):
        for item in grading[section].values():
            require(file_digest(evidence_path(item["path"])) == item["sha256"], "채점 계약 산출물 변경")
    metrics = read_json(root / "06-metrics.json")["result_schema"]
    require(file_digest(evidence_path(metrics["preview"])) == metrics["preview_sha256"], "측정 계약 산출물 변경")
    return hashes


def plan_condition(record: dict) -> dict:
    """The same projection defines identity at creation, verification and authorization."""
    return {"contract_hash": record["contract_hash"], "harness_sha256": record["harness_sha256"],
            "models": record["models"], "host": record["host"], "source_sha256": record["source_sha256"],
            "artifacts": {ident: s["artifact_sha256"] for ident, s in record["arms"].items()},
            "native_conditions": {ident: {"inventory": s["native_inventory_sha256"],
                "baseline": s["baseline"]["state_sha256"], "tool_condition": s["tool_condition"],
                "env": s["env"], "probe_identity": s["probe_identity"],
                **{key: s[key] for key in ("isolation", "warmup_policy") if key in s}}
                for ident, s in record["arms"].items()}}


def snapshot_handoffs(root: Path, solver_model: str) -> tuple[dict, dict]:
    names = set(handoff_names(solver_model))
    pending = list(names)
    while pending:
        for name in read_json(routes.EVIDENCE / pending.pop())["dependency_hashes"]:
            if name not in names:
                names.add(name)
                pending.append(name)
    files = {}
    def retain(value, key=""):
        if isinstance(value, dict):
            for name, item in value.items():
                retain(item, name)
        elif isinstance(value, list):
            for item in value:
                retain(item, key)
        elif (key in {"path", "log", "protocol_log", "relay_log", "native_inventory", "probe_log", "preview"}
              and isinstance(value, str) and value.startswith("/") and Path(value).is_file()):
            path = Path(value)
            checksum = file_digest(path)
            target = root / "evidence" / checksum / path.name
            immutable_copy(path, target)
            files[value] = {"path": str(target), "sha256": checksum}
    for name in sorted(names):
        original = routes.EVIDENCE / name
        immutable_copy(original, root / "handoffs" / name)
        retain(read_json(original))
    return {name: file_digest(root / "handoffs" / name) for name in handoff_names(solver_model)}, files


def verify_readiness(record: dict) -> None:
    require(record.get("plan_id") == digest(plan_condition(record))[:32], "준비 plan_id 위조 또는 조건 변경")
    require(record["status"] == "prepared_not_executed" and record["technical_ready"] is True, "기술적 준비 미완료")
    require(record["execution_authorized"] is False, "준비 전용 경계 변경")
    require(record["contract_hash"] == routes.contract()["contract_hash"], "route 실행 계약 변경")
    require(record["harness_sha256"] == settings.harness_identity(), "준비 이후 하네스 변경")
    solver = routes.solver_model(record["models"]["solver"]["model"])
    require(record["handoff_hashes"] == check_handoffs(solver["model"], snapshot=Path(record["handoff_snapshot"]),
            evidence_files=record["evidence_files"]), "준비 근거 변경")
    require(record["models"] == {"solver": solver, "grader": routes.load_profile()["grader"]}, "모델 선택 또는 고정 채점 조건 변경")
    for check in record["checks"]["offline_commands"]:
        require(check["exit_code"] == 0 and file_digest(Path(check["log"])) == check["log_sha256"], "오프라인 검증 근거 변경")
    require(record["host"]["codex_version"] == command(["codex", "--version"]).strip(), "Codex 버전 변경")
    require(record["host"]["solver_backend"] == solver["backend"], "풀이 실행기 변경")
    if solver["backend"] == "claude_code":
        require(record["host"]["claude"]["version"] == command(["claude", "--version"]).strip(), "Claude Code 버전 변경")
    artifacts = tool_artifacts.load_artifacts()
    corpus = read_json(Path(record["source_manifest_path"]))
    require(corpus["source_sha256"] == record["source_sha256"], "source identity 변경")
    require(file_digest(Path(record["source_manifest_path"])) == record["source_manifest_sha256"], "source manifest 변경")
    for ident in routes.ARM_IDS:
        state = record["arms"][ident]
        require(state["agent_backend"] == solver["backend"], "선택한 실행기와 조회 준비 기록 불일치")
        tool_indexes.verify_probe(state, artifacts)
        require(state["artifact_sha256"] == digest(artifacts[ident]), "실행 artifact 변경")
        require(digest(tool_indexes.tracked_manifest(Path(state["source"]), corpus["files"])) == corpus["source_sha256"], "source 변경")
        for part in ("source", "home"):
            require(digest(tool_artifacts.tree_files(Path(state[part]))) == state["baseline"]["state_sha256"][part], "native 초기 상태 변경")
            require(digest(tool_artifacts.tree_files(Path(state["baseline"]["paths"][part]))) == state["baseline"]["state_sha256"][part],
                    "복원용 native baseline 변경")
        require(file_digest(Path(state["native_inventory"])) == state["native_inventory_sha256"], "native tools/defaults 기록 변경")
        require(file_digest(Path(state["probe_log"])) == state["probe_log_sha256"], "native 중립 조회 로그 변경")
    require(record["planned_runs"] == routes.schedule() and len(record["planned_runs"]) == len(routes.QUESTION_IDS) * len(routes.ARM_IDS), "계획 슬롯 변경")
    require(set(record["solver_inputs"]) == {row["id"] for row in record["planned_runs"]}, "풀이 입력 슬롯 누락")
    for row in record["planned_runs"]:
        item = record["solver_inputs"][row["id"]]
        prompt = routes.solver_prompt(row["question_id"], Path(record["arms"][row["arm"]]["source"]), row["arm"])
        require(Path(item["path"]).read_text() == prompt and file_digest(Path(item["path"])) == item["sha256"], "풀이 입력 변경")
    require(record["execution_specs"] == {ident: execution_spec(state, solver_model=solver["model"])
                                          for ident, state in record["arms"].items()}, "비교군별 실행 설정 변경")
    require(all(record[name] == 0 for name in ("solver_runs", "grader_runs", "calibration_runs")), "준비에 실행 결과 혼입")


def offline_checks(emit=print) -> list[dict]:
    validation = routes.PREPARATION / "validation" / str(time.time_ns())
    offline_commands = []
    for name, args in (
        ("existing-suites", [sys.executable, "-m", "unittest", "benchmark.checks", "benchmark.cui_checks"]),
        ("cui-syntax", ["node", "--check", "benchmark/start.mjs"]),
    ):
        emit(f"오프라인 검증 ({settings.REPOSITORY}): {' '.join(args)}")
        offline_commands.append(tool_artifacts.run_logged(args, settings.REPOSITORY, validation / f"{name}.log", timeout=180))
    return offline_commands


def _build_plan(emit=print, *, solver_model: str | None = None, offline_commands: list | None = None) -> dict:
    """Validate existing prepared evidence and create only a reviewable, unexecuted plan."""
    solver = routes.solver_model(solver_model)
    check_handoffs(solver["model"])
    offline_commands = offline_checks(emit) if offline_commands is None else offline_commands
    indexes = read_json(routes.EVIDENCE / tool_indexes.index_handoff(solver["backend"]))
    artifacts = read_json(routes.EVIDENCE / "03-artifacts.json")["artifacts"]
    host = resources.environment(solver["backend"])
    host.update(system=platform.platform(), machine=platform.machine(), processors=__import__("os").cpu_count())
    profile = routes.load_profile()
    identity = {"contract_hash": routes.contract()["contract_hash"], "harness_sha256": settings.harness_identity(),
                "models": {"solver": solver, "grader": profile["grader"]}, "host": host,
                "source_sha256": indexes["source_manifest"]["source_sha256"], "arms": indexes["prepared_states"]}
    condition = plan_condition(identity)
    plan_id = digest(condition)[:32]
    root = routes.PREPARATION / "plans" / plan_id
    if (root / "readiness.json").exists():
        record = read_json(root / "readiness.json")
        verify_readiness(record)
        durable_json(readiness_path(solver["model"]), record)
        return readiness_projection(record, root / "readiness.json")
    require(not root.exists(), f"미완료 불변 계획이 남았습니다. 검토 후 별도로 보존하세요: {root}")
    handoffs, evidence_files = snapshot_handoffs(root, solver["model"])
    immutable_copy(tool_indexes.CORPUS, root / "corpus.json")
    checks = []
    for check in offline_commands:
        original = Path(check["log"])
        target = root / "validation" / original.name
        immutable_copy(original, target)
        checks.append({**check, "log": str(target)})
    for state in indexes["prepared_states"].values():
        for key in ("native_inventory", "probe_log"):
            original = Path(state[key])
            target = root / "arm-evidence" / state["id"] / key
            immutable_copy(original, target)
            state[key] = str(target)
    solver_inputs = {}
    for row in routes.schedule():
        path = root / "solver-inputs" / f"{row['id']}.txt"
        durable_bytes(path, routes.solver_prompt(row["question_id"], Path(indexes["prepared_states"][row["arm"]]["source"]), row["arm"]).encode(), exclusive=True)
        solver_inputs[row["id"]] = {"path": str(path), "sha256": file_digest(path), "question_id": row["question_id"]}
    record = {"schema_version": 1, "profile": routes.PROFILE, "status": "prepared_not_executed", "technical_ready": True,
              "execution_authorized": False, "plan_id": plan_id, "contract_hash": condition["contract_hash"],
              "harness_sha256": condition["harness_sha256"], "handoff_hashes": handoffs,
              "handoff_snapshot": str(root / "handoffs"), "evidence_files": evidence_files,
              "source_sha256": condition["source_sha256"], "source_manifest_path": str(root / "corpus.json"),
              "input_hashes": routes.contract()["input_hashes"], "source_manifest_sha256": file_digest(tool_indexes.CORPUS),
              "host": host, "models": condition["models"],
              "actual_versions": {ident: artifact["actual_version"] for ident, artifact in artifacts.items()},
              "arms": indexes["prepared_states"], "planned_runs": routes.schedule(),
              "execution_specs": {ident: execution_spec(state, solver_model=solver["model"])
                                   for ident, state in indexes["prepared_states"].items()},
              "solver_inputs": solver_inputs,
              "policy_decisions": profile["policy_decisions"],
              "checks": {"component_evidence": "verified", "planned_slots": len(routes.schedule()), "offline_commands": checks},
              "invalidation_rules": ["frozen inputs", "source bytes", "profile/models/effort/limits", "harness code",
                                     "binaries and installed dependencies", "embedding weights", "native index state",
                                     "native tool schema/default record", "Codex/Claude Code versions"],
              "solver_runs": 0, "grader_runs": 0, "calibration_runs": 0,
              "limitations": ["Subject build retains its actual version; it is not relabeled 1.0.0.",
                              "Real solver delivery, account model availability, judge calibration and empirical difficulty are untested.",
                              "OS page cache is uncontrolled; native index coverage and response limits are retained."]}
    if solver["backend"] == "claude_code":
        record["limitations"].append("Claude init-only checks startup and settings; actual native-tool enforcement, effective shell PATH and server-applied effort remain unobserved.")
    verify_readiness(record)
    durable_json(root / "readiness.json", record, exclusive=True)
    route_metrics.report(root, record, [], {})
    spec = execution_spec(solver_model=solver["model"])
    write_json(settings.CACHE / "plans" / f"{plan_id}.json", {
        "profile": routes.PROFILE, "preparation_only": True, "readiness": str(root / "readiness.json"),
        "harness_sha256": condition["harness_sha256"], "spec": spec, "execution_specs": record["execution_specs"],
        "planned_runs": record["planned_runs"]})
    durable_json(readiness_path(solver["model"]), record)
    emit(f"준비 완료: 21회 예정. 실제 풀이·채점 0회. plan_id={plan_id}")
    return readiness_projection(record, root / "readiness.json")


def readiness_projection(record: dict, path: Path) -> dict:
    spec = execution_spec(solver_model=record["models"]["solver"]["model"])
    return {"plan_id": record["plan_id"], "profile_name": routes.load_profile()["name"], "preparation_only": True,
            "status": record["status"], "technical_ready": record["technical_ready"],
            "targets": [{"id": ident, "name": "기본 에이전트 (검색 제품 없음)" if ident == "rg" else ident,
                         "source_sha256": s["source_sha256"], "tool_condition": s["tool_condition"],
                         "actual_version": record["actual_versions"][ident]} for ident, s in record["arms"].items()],
            "questions": len(routes.QUESTION_IDS), "repeats": 1, "runs": len(routes.schedule()), "spec": spec,
            "codex_version": record["host"]["codex_version"], "output_root": str(path.parent),
            "readiness": str(path), "readiness_sha256": file_digest(path),
            "report": str(path.parent / "report/results.md"), "solver_runs": 0, "grader_runs": 0}


def build_plan(emit=print, *, solver_model: str | None = None) -> dict:
    with campaign_lease():
        return _build_plan(emit, solver_model=solver_model)


def prepare_all(emit=print, *, solver_model: str | None = None) -> dict:
    """Cache-aware local preparation; no implicit start/resume or model launch."""
    solvers = routes.load_profile()["solver_choices"] if solver_model == "all" else [routes.solver_model(solver_model)]
    if any(s["backend"] == "claude_code" for s in solvers):
        from .claude_client import capabilities
        capabilities()
    with campaign_lease():
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
        for solver in solvers:
            for ident in routes.ARM_IDS:
                if not tool_indexes.probe_is_current(ident, solver["backend"]):
                    tool_indexes.probe_index(ident, emit, backend=solver["backend"])
            tool_indexes.publish_indexes(solver["backend"])
        route_metrics.publish_metrics()
        require(settings.harness_identity() == harness_at_start,
                "준비 중 하네스 파일이 변경되었습니다. 현재 코드로 prepare를 다시 수행하세요")
        commands = offline_checks(emit)
        plans = [_build_plan(emit, solver_model=s["model"], offline_commands=commands) for s in solvers]
        return {"status": "prepared_not_executed", "plans": plans} if solver_model == "all" else plans[0]


def main() -> int:
    parser = argparse.ArgumentParser(description="Grafana 비교 계획의 준비 전용 게이트; 실제 모델 실행 없음")
    parser.add_argument("action", choices=["prepare", "finalize", "verify"], nargs="?", default="prepare")
    parser.add_argument("--solver-model", choices=["all", *[item["model"] for item in routes.load_profile()["solver_choices"]]],
                        help="풀이 모델; 생략하면 gpt-6.1-sol, 채점은 항상 gpt-6-astra")
    args = parser.parse_args()
    try:
        if args.action == "prepare":
            value = prepare_all(lambda message: print(message, flush=True), solver_model=args.solver_model)
        elif args.action == "finalize":
            with campaign_lease():
                models = routes.load_profile()["solver_choices"] if args.solver_model == "all" else [routes.solver_model(args.solver_model)]
                commands = offline_checks(lambda message: print(message, flush=True))
                plans = [_build_plan(lambda message: print(message, flush=True), solver_model=s["model"], offline_commands=commands) for s in models]
                value = {"status": "prepared_not_executed", "plans": plans} if args.solver_model == "all" else plans[0]
        else:
            with campaign_lease():
                models = routes.load_profile()["solver_choices"] if args.solver_model == "all" else [routes.solver_model(args.solver_model)]
                plans = []
                for solver in models:
                    pointer = read_json(readiness_path(solver["model"]))
                    path = routes.PREPARATION / "plans" / pointer["plan_id"] / "readiness.json"
                    record = read_json(path)
                    require(record == pointer, "최신 준비 포인터와 불변 계획 불일치")
                    verify_readiness(record)
                    plans.append({"status": "prepared_not_executed", "plan_id": record["plan_id"]})
                value = {"status": "prepared_not_executed", "plans": plans} if args.solver_model == "all" else plans[0]
        print(f"{value['status']}: {value.get('plan_id') or [p['plan_id'] for p in value['plans']]}")
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        print(f"준비 미완료: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
