"""Multi-target schedules, cancellation, resume, comparable A reuse and reports."""
from __future__ import annotations

import collections
import concurrent.futures
import datetime
import json
import random
import shutil
import signal
import subprocess
import threading
import uuid
from pathlib import Path

from . import evaluate, resources, settings
from .v2 import metrics, runner
from .v2.core import (ContractError, MODEL_TERMINALS, command, digest, file_digest, metric,
                      read_json, require, write_json)


def answer_prompt(spec: dict, question: dict) -> str:
    prefix = runner.ANSWER_PROMPT.split("실행 제한은")[0]
    limits = spec["limits"]
    return (prefix + f"실행 제한은 {limits['elapsed_seconds']}초, 실제 탐색 호출 {limits['exploration_calls']}회, "
            f"누적 입력+출력 토큰 {limits['total_tokens']}개다.\n\n" + question["prompt"])


def baseline_key(plan: dict) -> str:
    return digest({key: plan[key] for key in ("profile", "dataset", "spec", "harness_sha256", "source", "codex_version")})


def catalog() -> dict:
    config = settings.load_config()
    pending = []
    if settings.RUNS.exists():
        for path in sorted(settings.RUNS.glob("*/manifest.json"), reverse=True):
            status_path = path.with_name("status.json")
            status = read_json(status_path) if status_path.exists() else {"status": "prepared"}
            if status["status"] not in {"complete", "finished_with_errors"}:
                manifest = read_json(path)
                scheduled = read_json(path.with_name("schedule.json"))
                remaining = sum(not (path.parent / "runs" / row["id"] / "run.json").exists() or
                                read_json(path.parent / "runs" / row["id"] / "run.json")["status"] == "scheduled" for row in scheduled)
                pending.append({"id": path.parent.name, "profile": manifest["profile"], "status": status["status"],
                                "targets": [target["name"] for target in manifest["targets"].values()],
                                "remaining_solver_runs": remaining, "scheduled": len(scheduled), "spec": manifest["spec"],
                                "error": status.get("error")})
    return {"profiles": settings.profiles(), "targets": settings.targets(config), "pending": pending,
            "config": {key: value for key, value in config.items() if key != "candidates"}}


def prepare_plan(profile: str, selected: list[str], emit=print) -> dict:
    config = settings.load_config()
    observed = resources.environment()
    prepared = resources.prepare_targets(config, selected, emit)
    source = prepared["source"]
    spec = settings.execution_settings(config)
    plan = {"profile": profile, "dataset": settings.dataset_for(profile, Path(source["snapshot"])),
            "spec": spec, "source": source, "targets": prepared["targets"], "codex_version": observed["codex_version"],
            "environment": observed, "harness_sha256": settings.harness_identity(),
            "repeats": settings.profiles()[profile]["repeats"]}
    plan["baseline_key"] = baseline_key(plan)
    choices = []
    if "A" in selected:
        for manifest_path in sorted(settings.RUNS.glob("*/manifest.json"), reverse=True):
            previous = read_json(manifest_path)
            if previous.get("baseline_key") != plan["baseline_key"]:
                continue
            root = manifest_path.parent
            try:
                verify_manifest(root)
                rows = [row for row in runner.load_experiment_runs(root) if row["group"] == "A"]
                expected_count = len(plan["dataset"]["questions"]) * plan["repeats"]
                if len(rows) == expected_count and all(r["status"] in MODEL_TERMINALS and r["conditions_valid"]
                                                      and r["usage"]["complete"] and r.get("codex_version") == plan["codex_version"] for r in rows):
                    choices.append({"id": root.name, "runs": len(rows)})
            except (ValueError, KeyError, OSError):
                continue
    plan["reuse_choices"] = choices
    ident = uuid.uuid4().hex
    write_json(settings.CACHE / "plans" / f"{ident}.json", plan, exclusive=True)
    return {"plan_id": ident, "profile_name": settings.profiles()[profile]["name"],
            "targets": [{"id": key, "name": value["name"], "source_sha256": value.get("build", {}).get("identity", {}).get("source_sha256")}
                        for key, value in plan["targets"].items()],
            "questions": len(plan["dataset"]["questions"]), "repeats": plan["repeats"],
            "runs": len(selected) * len(plan["dataset"]["questions"]) * plan["repeats"],
            "spec": spec, "codex_version": plan["codex_version"], "reuse_choices": choices,
            "output_root": str(settings.RUNS)}


def schedule(plan: dict) -> list[dict]:
    rng = random.Random(plan["spec"]["seed"])
    questions = list(plan["dataset"]["questions"])
    rng.shuffle(questions)
    first_order = {}
    for question in questions:
        order = list(plan["targets"])
        rng.shuffle(order)
        first_order[question["id"]] = order
    result = []
    for repeat in range(1, plan["repeats"] + 1):
        for question in questions:
            order = first_order[question["id"]]
            shift = (repeat - 1) % len(order)
            for target_id in order[shift:] + order[:shift]:
                result.append({"id": f"{question['id']}-{target_id}-{repeat}", "question_id": question["id"],
                               "target": target_id, "group": plan["targets"][target_id]["group"],
                               "repeat": repeat, "phase": "main"})
    return result


def verify_manifest(root: Path) -> dict:
    manifest = read_json(root / "manifest.json")
    for name, checksum in read_json(root / "manifest-seal.json").items():
        require(file_digest(root / name) == checksum, f"동결 실행 조건 변경: {name}")
    require(manifest["harness_sha256"] == settings.harness_identity(), "하네스가 변경되었습니다. 기존 실행을 보존하고 새 실행을 시작하세요")
    return manifest


def create_run(plan_id: str, reuse: str | None) -> Path:
    require(len(plan_id) == 32 and all(c in "0123456789abcdef" for c in plan_id), "준비 ID 오류")
    plan = read_json(settings.CACHE / "plans" / f"{plan_id}.json")
    require(plan["harness_sha256"] == settings.harness_identity(), "준비 이후 하네스가 변경되었습니다")
    require(not reuse or reuse in {item["id"] for item in plan["reuse_choices"]}, "선택할 수 없는 A 재사용 결과")
    ident = datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-" + plan["profile"] + "-" + uuid.uuid4().hex[:8]
    root = settings.RUNS / ident
    root.mkdir(parents=True, exist_ok=False)
    entries = schedule(plan)
    manifest = {**plan, "reuse_A": reuse, "created_at": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    write_json(root / "manifest.json", manifest, exclusive=True)
    write_json(root / "dataset.json", plan["dataset"], exclusive=True)
    write_json(root / "schedule.json", entries, exclusive=True)
    write_json(root / "manifest-seal.json", {name: file_digest(root / name) for name in
                                           ("manifest.json", "dataset.json", "schedule.json")}, exclusive=True)
    archive = root / "harness"
    archive.mkdir()
    for path in settings.ROOT.glob("*.py"):
        shutil.copy2(path, archive / path.name)
    shutil.copytree(settings.ROOT / "v2", archive / "v2", ignore=lambda p, names: [n for n in names if n in {"artifacts", "__pycache__", "data"}])
    shutil.copytree(settings.ROOT / "data", archive / "data")
    if reuse:
        original_root = settings.RUNS / reuse
        previous = verify_manifest(original_root)
        require(previous["baseline_key"] == manifest["baseline_key"], "A 재사용 조건 변경")
        originals = {r["id"]: r for r in runner.load_experiment_runs(original_root) if r["group"] == "A"}
    else:
        originals = {}
    for entry in entries:
        folder = root / "runs" / entry["id"]
        if reuse and entry["group"] == "A":
            original = originals[entry["id"]]
            require(original["status"] in MODEL_TERMINALS and original["conditions_valid"] and original["usage"]["complete"]
                    and original.get("codex_version") == manifest["codex_version"], "재사용 A 실행 조건 오류")
            write_json(folder / "run.json", {**original, **entry, "reused_from": str(original_root)}, exclusive=True)
            original_folder = original_root / "runs" / entry["id"]
            write_json(folder / "reuse.json", {"folder": str(original_folder), "run_sha256": file_digest(original_folder / "run.json")}, exclusive=True)
            runner.seal_run(folder)
        else:
            write_json(folder / "run.json", runner.placeholder_run(entry), exclusive=True)
    write_json(root / "status.json", {"status": "prepared"})
    return root


def finish_execution(folder: Path, entry: dict, execution: dict) -> dict:
    calls, complete, host_calls, violations = runner.normalize_calls(folder)
    exploration = metrics.exploration_metrics(calls, complete=complete, evidence=str(folder / "relay.jsonl"))
    exploration["delivered_output_bytes"] = metric(read_json(folder / "delivery.json")["actual_model_output_bytes"], "bytes",
                                                   evidence=[str(folder / "rollout.jsonl")])
    return {**entry, **execution, "calls": calls, "host_calls": host_calls, "boundary_violations": violations,
            "conditions_valid": execution["conditions_valid"] and not violations, "exploration": exploration,
            "source_snapshot": str(folder / "source")}


def recover(root: Path):
    for entry in read_json(root / "schedule.json"):
        folder = root / "runs" / entry["id"]
        if not (folder / "run.json").exists():
            write_json(folder / "run.json", runner.placeholder_run(entry), exclusive=True)
        row = read_json(folder / "run.json")
        if row["status"] == "running":
            for record_name in ("process.json", "relay-runtime.json"):
                record_path = folder / record_name
                if record_path.exists():
                    recorded = read_json(record_path)
                    if runner.signal_target(recorded["pid"], 0, group=True):
                        raise ContractError(f"이전 실행의 프로세스 그룹이 남아 있습니다. 종료를 확인한 뒤 이어가세요: {recorded['pid']} ({folder})")
            shutil.copy2(folder / "run.json", folder / "run-before-recovery.json")
            if (folder / "execution.json").exists():
                row = finish_execution(folder, entry, read_json(folder / "execution.json"))
            else:
                row = runner.placeholder_run(entry, "interrupted", "이전 실행 중단; 원자료 보존, 자동 재실행 없음")
            write_json(folder / "run.json", row)
        if row["status"] not in {"running", "scheduled"} and not (folder / "seal.json").exists():
            runner.seal_run(folder)
        if (folder / "reuse.json").exists():
            origin = read_json(folder / "reuse.json")
            original = Path(origin["folder"])
            require(file_digest(original / "run.json") == origin["run_sha256"], "재사용 A 원자료 변경")
            runner.verify_run_seal(original)


def run_one(root: Path, entry: dict, manifest: dict, cancel_event, emit=print):
    folder = root / "runs" / entry["id"]
    if cancel_event.is_set() or read_json(folder / "run.json")["status"] != "scheduled":
        return
    write_json(folder / "run.json", runner.placeholder_run(entry, "running"))
    try:
        target = manifest["targets"][entry["target"]]
        runner.copy_source(Path(target["snapshot"]), folder / "source")
        require(digest(runner.source_manifest(folder / "source")) == manifest["source"]["source_sha256"], "풀이 원문 복사 불일치")
        if cancel_event.is_set():
            raise InterruptedError("사용자가 취소했습니다")
        relay = {"group": entry["group"], "source": str(folder / "source"), "log": str(folder / "relay.jsonl"),
                 "product_binary": target.get("build", {}).get("binary"), "product_home": str(folder / "product-home"),
                 "limits": manifest["spec"]["limits"]}
        question = next(q for q in manifest["dataset"]["questions"] if q["id"] == entry["question_id"])
        version = command(["codex", "--version"]).strip()
        result = runner.execute_codex(folder, answer_prompt(manifest["spec"], question), relay, usage_drain_seconds=30,
                                      settings=manifest["spec"], cancel_event=cancel_event, codex_version=version)
        row = finish_execution(folder, entry, result)
    except (ValueError, KeyError, TypeError, OSError, subprocess.SubprocessError) as exc:
        row = runner.placeholder_run(entry, "interrupted" if cancel_event.is_set() else "environment_error", str(exc))
    write_json(folder / "run.json", row)
    runner.seal_run(folder)
    shutil.rmtree(folder / "source", ignore_errors=True)
    emit(f"풀이 {entry['id']}: {row['status']}")


def report(root: Path, manifest: dict, rows: list[dict], grades: dict) -> dict:
    lookup = {q["id"]: q for q in manifest["dataset"]["questions"]}
    normalized = [{**metrics.normalize_run(lookup[row["question_id"]], row, grades["judgments"].get(row["id"])),
                   "target": row["target"], "codex_version": row.get("codex_version"), "reused_from": row.get("reused_from")}
                  for row in rows]
    targets = {}
    for ident, target in manifest["targets"].items():
        selected = [r for r in normalized if r["target"] == ident]
        targets[ident] = {"name": target["name"], "categories": dict(collections.Counter(r["category"] for r in selected)),
                          "reused_runs": sum(bool(r["reused_from"]) for r in selected),
                          "codex_versions": dict(collections.Counter(r["codex_version"] or "미측정" for r in selected)),
                          **metrics.summarize(selected, manifest["repeats"])}
    summary = {"profile": manifest["profile"], "scheduled": len(rows), "targets": targets,
               "grading_complete": grades.get("complete", False), "baseline_key": manifest["baseline_key"],
               "manifest_sha256": file_digest(root / "manifest.json"), "grading_batches": grades.get("batches", []),
               "rows": normalized}
    versions = {row["codex_version"] for row in normalized if row["status"] != "scheduled"}
    summary["same_codex_version"] = len(versions) == 1 and None not in versions
    summary["comparisons"] = {}
    if "A" in targets:
        for ident in targets.keys() - {"A"}:
            paired = [r for r in normalized if r["target"] in {"A", ident}]
            summary["comparisons"][ident] = {
                "same_codex_version": summary["same_codex_version"],
                "A_reused": bool(manifest.get("reuse_A")),
                "scope": "선택한 고정 문항의 관측 차이; 시점이 다른 재사용 결과는 무작위 동시 대조가 아님",
                **metrics.compare(paired, manifest["repeats"], with_bootstrap=False)}
    write_json(root / "report/summary.json", summary)
    lines = [f"# Grafana · {settings.profiles()[manifest['profile']]['name']}", "",
             f"풀이 {manifest['spec']['model']} / {manifest['spec']['reasoning_effort']}, "
             f"채점 {manifest['spec']['grader_model']} / {manifest['spec']['grader_reasoning_effort']}", "",
             "| 대상 | 예정 | 정답 | 부분 | 오답 | 무응답 | 판정 불가 | 핵심 사실 | 인용 근거 | 평균 토큰 | A 재사용 |",
             "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for target in targets.values():
        categories = target["categories"]
        tokens = target["metrics"]["total_tokens"]["mean"]["value"]
        ratios = [target["metrics"][key]["mean"]["value"] for key in ("fact_ratio", "evidence_ratio")]
        values = [target["name"], target["scheduled_runs"], *[categories.get(c, 0) for c in
                  ("correct", "partial", "incorrect", "no_answer", "indeterminate")],
                  *["미확인" if value is None else f"{value:.1%}" for value in ratios],
                  "미확인" if tokens is None else f"{tokens:,.2f}", target["reused_runs"]]
        lines.append("| " + " | ".join(map(str, values)) + " |")
    lines += ["", "누락된 사용량·미실행·실행 조건 오류는 0으로 대체하지 않습니다. 전체 예정 횟수를 분모로 보존합니다.",
              "후보/오류 검증 결과를 정식 결과나 인과관계 증명으로 승격하지 않습니다.", "",
              "실제 Codex 버전: " + json.dumps({key: value["codex_versions"] for key, value in targets.items()}, ensure_ascii=False),
              "", "소스·바이너리·설정 식별 정보: [manifest.json](../manifest.json). 원자료와 상세 지표: [summary.json](summary.json)."]
    if not summary["same_codex_version"]:
        lines += ["", "Codex 버전이 섞였거나 확인되지 않은 실행이 있습니다. 동일 실행 환경의 비교로 해석하지 않습니다."]
    (root / "report/results.md").write_text("\n".join(lines) + "\n")
    return summary


def execute(root: Path, emit=print) -> dict:
    cancelled = threading.Event()
    previous_signals = {}
    def interrupt(signum, frame):
        cancelled.set()
    with resources.lease(root / ".run.lock"):
        manifest = verify_manifest(root)
        # Current settings must still agree; a CLI version change alone never blocks resume.
        require(settings.execution_settings(settings.load_config()) == manifest["spec"], "실행 설정이 변경되었습니다. 새 실행을 시작하세요")
        resources.environment()
        require(digest(runner.source_manifest(Path(manifest["source"]["snapshot"]))) == manifest["source"]["source_sha256"], "평가 소스 변경")
        for target in manifest["targets"].values():
            if target["group"] == "B":
                resources.verify_product(target["build"])
                resources.verify_index(target["index"])
        for signum in (signal.SIGINT, signal.SIGTERM):
            previous_signals[signum] = signal.signal(signum, interrupt)
        grades = {"judgments": {}, "complete": False}
        try:
            recover(root)
            write_json(root / "status.json", {"status": "running"})
            entries = read_json(root / "schedule.json")
            blocks = collections.defaultdict(list)
            for entry in entries:
                blocks[(entry["question_id"], entry["repeat"])].append(entry)
            def run_block(block):
                for entry in block:
                    run_one(root, entry, manifest, cancelled, emit)
            with concurrent.futures.ThreadPoolExecutor(max_workers=manifest["spec"]["concurrency"]) as executor:
                list(executor.map(run_block, blocks.values()))
            rows = runner.load_experiment_runs(root)
            if not cancelled.is_set():
                write_json(root / "status.json", {"status": "grading"})
                grades = evaluate.run(root, manifest["dataset"], rows, Path(manifest["source"]["snapshot"]), manifest["spec"], cancelled, emit)
            report(root, manifest, rows, grades)
            state = "interrupted" if cancelled.is_set() else "complete" if grades["complete"] and all(
                row["status"] in MODEL_TERMINALS and row["conditions_valid"] for row in rows) else "finished_with_errors"
            result = {"status": state, "run_id": root.name, "report": str(root / "report/results.md")}
            write_json(root / "status.json", result)
            return result
        except (ValueError, KeyError, OSError, subprocess.SubprocessError) as exc:
            write_json(root / "status.json", {"status": "needs_review", "error": str(exc)})
            report(root, manifest, runner.load_experiment_runs(root), grades)
            raise
        finally:
            for signum, handler in previous_signals.items():
                signal.signal(signum, handler)
