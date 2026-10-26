"""Explicitly authorized, durable Grafana campaigns; preparation never launches a model."""
from __future__ import annotations

import copy
import importlib
import inspect
import json
import os
import queue
import signal
import subprocess
import threading
import time
import uuid
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from . import runner, settings, route_contract as routes, route_prepare, route_grading, route_metrics, tool_indexes, tool_artifacts
from .campaign_store import campaign_lease, durable_json, immutable_copy, timestamp
from .core import ContractError, digest, file_digest, read_json, require


def answer_prompt(spec: dict, question: dict) -> str:
    prefix = runner.ANSWER_PROMPT.split("실행 제한은")[0]
    limits = spec["limits"]
    return (prefix + f"실행 제한은 {limits['elapsed_seconds']}초, 실제 탐색 호출 {limits['exploration_calls']}회, "
            f"캐시 읽기를 제외한 누적 입력+출력 토큰 {limits['total_tokens']}개다.\n\n" + question["prompt"])


def catalog() -> dict:
    profile = routes.load_profile()
    return {"profiles": settings.profiles(), "route_targets": settings.targets(),
            "solver_choices": profile["solver_choices"], "default_solver_model": profile["solver"]["model"],
            "grader": profile["grader"]}


def prepare_plan(profile: str, selected: list[str], emit=print, *, solver_model: str | None = None) -> dict:
    require(profile == routes.PROFILE, "지원하는 벤치마크는 grafana-routes뿐입니다")
    require(len(selected) == len(set(selected)) == len(routes.ARM_IDS) and set(selected) == set(routes.ARM_IDS),
            "고정된 비교군을 모두 준비합니다")
    return route_prepare.prepare_all(emit, solver_model=solver_model)


def verify_prepared_plan(plan_id: str, readiness: Path, solver_model: str, readiness_sha256: str) -> dict:
    readiness = readiness.resolve()
    require(readiness == (routes.PREPARATION / "plans" / plan_id / "readiness.json").resolve(), "불변 계획 경로가 아닙니다")
    require(file_digest(readiness) == readiness_sha256, "승인할 준비 기록 hash 불일치")
    record = read_json(readiness)
    require(record["plan_id"] == plan_id and record["models"]["solver"]["model"] == solver_model, "선택한 계획/모델 불일치")
    route_prepare.verify_readiness(record)
    return record


def authorize_run(prepared: dict, readiness: Path, approval: dict) -> tuple[Path, dict]:
    """The only place that changes preparation_only, on independent deep copies."""
    require(approval.get("confirmed") is True and isinstance(approval.get("actor"), str)
            and approval["actor"].strip(), "명시적 실행 승인과 승인 주체가 필요합니다")
    checksum = file_digest(readiness)
    require(approval.get("plan_id") == prepared["plan_id"] and approval.get("readiness_sha256") == checksum
            and approval.get("solver_model") == prepared["models"]["solver"]["model"], "승인 범위 불일치")
    require(read_json(readiness) == prepared and prepared["plan_id"] == digest(route_prepare.plan_condition(prepared))[:32], "승인 조건 변경")
    planned_ids = [slot["id"] for slot in prepared["planned_runs"]]
    scope = approval.get("scope", {"kind": "full"})
    if isinstance(scope, dict) and scope.get("kind") == "smoke":
        # A smoke campaign runs only the named slots through the same lifecycle; its results are not comparisons.
        selected = scope.get("slot_ids")
        require(set(scope) == {"kind", "slot_ids"} and isinstance(selected, list) and selected
                and len(set(selected)) == len(selected) and set(selected) <= set(planned_ids), "스모크 슬롯 범위 오류")
    else:
        require(scope == {"kind": "full"}, "알 수 없는 실행 범위")
        selected = planned_ids
    campaign_id = uuid.uuid4().hex
    root = routes.PREPARATION / "campaigns" / (("smoke-" if scope["kind"] == "smoke" else "") + campaign_id)
    authorization = {**approval, "scope": scope, "recorded_at": timestamp(), "campaign_id": campaign_id,
                     "max_solver_launches": len(selected), "max_grader_launches": len(selected), "calibration_runs": 0}
    specs = copy.deepcopy(prepared["execution_specs"])
    for spec in specs.values():
        require(spec["preparation_only"] is True and spec["concurrency"] == 4, "준비 실행 조건 변경")
        spec.update(preparation_only=False, campaign_id=campaign_id, plan_id=prepared["plan_id"],
                    readiness_sha256=checksum, authorization_sha256=digest(authorization), usage_drain_seconds=0)
    manifest = {"schema_version": 1, "campaign_id": campaign_id, "plan_id": prepared["plan_id"],
                "cache": str(settings.CACHE.resolve()), "readiness": str(readiness.resolve()), "readiness_sha256": checksum,
                "condition_sha256": digest(route_prepare.plan_condition(prepared)), "authorization": authorization,
                "prepared": copy.deepcopy(prepared), "models": copy.deepcopy(prepared["models"]), "run_specs": specs,
                "run_spec_sha256": {arm: digest(spec) for arm, spec in specs.items()},
                "concurrency": 4, "grader_timeout_seconds": 300, "packet_bytes_limit": 384 * 1024}
    durable_json(root / "manifest.json", manifest, exclusive=True)
    rows = []
    for rank, slot in enumerate(prepared["planned_runs"]):
        rows.append({**slot, "plan_id": prepared["plan_id"], "campaign_id": campaign_id, "schedule_rank": rank,
                     "model": prepared["models"]["solver"]["model"], "backend": prepared["models"]["solver"]["backend"],
                     "reasoning_effort": prepared["models"]["solver"]["reasoning_effort"],
                     "run_spec_sha256": manifest["run_spec_sha256"][routes.execution_key(slot["arm"], slot["mode"])],
                     "prompt_sha256": prepared["solver_inputs"][slot["id"]]["sha256"],
                     "phase": "pending", "launch_state": "not_started", "cli_launches": 0,
                     "provider_call_observed": None, "conditions_valid": None, "grade_state": "not_eligible"})
        if slot["id"] not in selected:
            rows[-1].update(phase="terminal", status="out_of_scope", provider_call_observed=False,
                            grade_reason="outside smoke scope")
    durable_json(root / "ledger.json", {"schema_version": 1, "revision": 0, "campaign_id": campaign_id,
        "manifest_sha256": file_digest(root / "manifest.json"), "status": "authorized", "slots": rows}, exclusive=True)
    return root, manifest


def load_ledger(root: Path) -> tuple[dict, dict]:
    manifest, ledger = read_json(root / "manifest.json"), read_json(root / "ledger.json")
    require(ledger["manifest_sha256"] == file_digest(root / "manifest.json"), "RunManifest 변경")
    require(ledger["campaign_id"] == manifest["campaign_id"] and manifest["cache"] == str(settings.CACHE.resolve()), "campaign/cache 불일치")
    expected = routes.schedule()
    require(len(ledger["slots"]) == len(expected), "계획 slot ledger 누락")
    for row, slot in zip(ledger["slots"], expected):
        require(all(row[k] == slot[k] for k in ("id", "question_id", "arm", "mode", "attempt")), "ledger 순서/슬롯 변경")
        verify_run_binding(manifest, row, manifest["run_specs"][routes.execution_key(row["arm"], row["mode"])], current=False)
    return manifest, ledger


def verify_run_binding(manifest: dict, slot: dict, spec: dict, *, current=True) -> None:
    prepared = manifest["prepared"]
    arm = routes.execution_key(slot["arm"], slot["mode"])
    require(manifest["condition_sha256"] == digest(route_prepare.plan_condition(prepared)), "계획 조건 변경")
    expected = copy.deepcopy(prepared["execution_specs"][arm])
    expected.update(preparation_only=False, campaign_id=manifest["campaign_id"], plan_id=manifest["plan_id"],
                    readiness_sha256=manifest["readiness_sha256"], authorization_sha256=digest(manifest["authorization"]),
                    usage_drain_seconds=0)
    require(spec == expected and digest(spec) == slot["run_spec_sha256"] == manifest["run_spec_sha256"][arm], "실행 spec binding 불일치")
    solver = manifest["models"]["solver"]
    require(slot["model"] == solver["model"] and slot["backend"] == solver["backend"]
            and slot["reasoning_effort"] == solver["reasoning_effort"] and slot["plan_id"] == manifest["plan_id"]
            and slot["campaign_id"] == manifest["campaign_id"], "실행 모델/계획 binding 불일치")
    require(slot["prompt_sha256"] == prepared["solver_inputs"][slot["id"]]["sha256"], "풀이 입력 binding 변경")
    if current:
        require(file_digest(Path(prepared["solver_inputs"][slot["id"]]["path"])) == slot["prompt_sha256"], "풀이 입력 변경")
        require(settings.harness_identity() == prepared["harness_sha256"], "실행 중 하네스 변경")


class LedgerCoordinator:
    """Only the scheduler thread commits; workers wait for durable acknowledgements."""
    def __init__(self, root: Path, ledger: dict, cancel_event):
        self.root, self.ledger, self.cancel_event = root, ledger, cancel_event
        self.requests = queue.Queue()
        self.owner = threading.get_ident()
        self.fatal = None

    def commit(self):
        require(threading.get_ident() == self.owner, "ledger 단일 작성자 위반")
        require(read_json(self.root / "ledger.json")["revision"] == self.ledger["revision"], "ledger revision 충돌")
        value = {**self.ledger, "revision": self.ledger["revision"] + 1, "updated_at": timestamp()}
        durable_json(self.root / "ledger.json", value)
        self.ledger = value

    def change(self, ident: str, values: dict, *, intent: str | None = None):
        reply = queue.Queue(maxsize=1)
        self.requests.put((ident, values, intent, reply))
        result = reply.get()
        if isinstance(result, BaseException):
            raise result
        return result

    def drain(self):
        while True:
            try:
                ident, values, intent, reply = self.requests.get_nowait()
            except queue.Empty:
                return
            try:
                if self.fatal:
                    raise ContractError(f"ledger write failed: {self.fatal}")
                row = next(r for r in self.ledger["slots"] if r["id"] == ident)
                if intent:
                    require(not self.cancel_event.is_set(), "cancelled_before_launch")
                    path = self.root / "runs" / ident / f"{intent}-intent.json"
                    if intent == "launch":
                        require(row["launch_state"] == "not_started", "solver 재실행 금지")
                    durable_json(path, {"slot_id": ident, "campaign_id": row["campaign_id"], "created_at": timestamp(),
                        "run_spec_sha256": row["run_spec_sha256"], "packet_seal_sha256": row.get("packet_seal_sha256")}, exclusive=True)
                    values = {**values, f"{intent}_intent_sha256": file_digest(path)}
                row.update(values, updated_at=timestamp())
                self.commit()
                reply.put(copy.deepcopy(row))
            except (OSError, ValueError) as exc:
                if isinstance(exc, OSError):
                    self.fatal = str(exc)
                    self.cancel_event.set()
                reply.put(exc)


class ProcessRegistry:
    """Register owned groups before releasing prompt/attach; never signal an unknown PID."""
    def __init__(self, folder: Path):
        self.folder, self.groups, self.lock = folder, [], threading.Lock()

    @staticmethod
    def fingerprint(pid: int) -> str:
        result = subprocess.run(["ps", "-p", str(pid), "-o", "lstart="], text=True, capture_output=True, timeout=5)
        return result.stdout.strip()

    def register(self, pid: int, pgid: int, owner: str) -> dict:
        require(pid == pgid and pgid != os.getpgrp(), "독립 소유 process group만 등록합니다")
        fingerprint = self.fingerprint(pid)
        require(bool(fingerprint) and os.getpgid(pid) == pgid, "process identity 확인 실패")
        item = {"pid": pid, "pgid": pgid, "owner": owner, "start_fingerprint": fingerprint}
        with self.lock:
            self.groups.append(item)
            durable_json(self.folder / "owned-groups.json", self.groups)
        return item

    def cleanup(self) -> dict:
        errors = []
        for item in self.groups:
            pgid = item["pgid"]
            try:
                os.killpg(pgid, 0)
            except ProcessLookupError:
                continue
            if self.fingerprint(item["pid"]) != item["start_fingerprint"]:
                errors.append({**item, "error": "group ownership cannot be re-established; no signal sent"})
                continue
            for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGKILL):
                try:
                    os.killpg(pgid, sig)
                    deadline = time.monotonic() + 1
                    while time.monotonic() < deadline:
                        os.killpg(pgid, 0)
                        time.sleep(.02)
                except ProcessLookupError:
                    break
            else:
                errors.append({**item, "error": "group still alive"})
        return {"complete": not errors, "groups": self.groups, "errors": errors}


def session_factory():
    """Stage-2 integration is mandatory; never fall back to a cold unsandboxed relay."""
    try:
        module = importlib.import_module("benchmark.warm_host")
    except ModuleNotFoundError as exc:
        raise ContractError("query-ready warm_host 통합 전에는 실제 campaign 실행을 차단합니다") from exc
    require("prepared_session" in inspect.signature(runner.execute_model).parameters,
            "runner의 clock 직전 prepared_session 연결이 필요합니다")
    return module.prepare_session


def execute_solver(folder: Path, prompt: str, *, session, settings: dict, cancel_event, process_registry, **versions) -> dict:
    # The runner owns t0 immediately before Popen and calls session.open_solver_gate(t0).
    return runner.execute_model(folder, prompt, settings=settings, cancel_event=cancel_event,
                                prepared_session=session, usage_drain_seconds=0, **versions)


def restore_slot(state: dict) -> dict:
    return tool_indexes.restore_baseline(state["id"], state, state["baseline"])


def verify_source(state: dict, corpus: dict) -> None:
    tool_indexes.verify_source_inventory(Path(corpus["source"]), corpus["files"])
    require(digest(tool_indexes.tracked_manifest(Path(state["source"]), corpus["files"],
            allowed_metadata=tool_indexes.SOURCE_METADATA[state["id"]])) == corpus["source_sha256"],
            "solver/product changed tracked source bytes")


def seal_slot(folder: Path, row: dict) -> dict:
    # Product homes and the CLI workspace are not raw logs (and may contain credentials).
    files = {p.name: {"sha256": file_digest(p), "bytes": p.stat().st_size}
             for p in sorted(folder.iterdir()) if p.is_file() and not p.is_symlink() and p.name != "raw-seal.json"}
    value = {"schema_version": 1, "slot_id": row["id"], "run_spec_sha256": row["run_spec_sha256"],
             "files": files, "collection_complete": row.get("call_log_complete", False),
             "cleanup_complete": row.get("cleanup", {}).get("complete", False)}
    durable_json(folder / "raw-seal.json", value, exclusive=True)
    return {"raw_seal": str(folder / "raw-seal.json"), "raw_seal_sha256": file_digest(folder / "raw-seal.json")}


def run_slot(root: Path, manifest: dict, initial: dict, coordinator: LedgerCoordinator, cancel_event,
             prepare_session, solver_executor, collector, restore, source_check) -> dict:
    row = copy.deepcopy(initial)
    state = manifest["prepared"]["arms"][row["arm"]]
    spec = manifest["run_specs"][routes.execution_key(row["arm"], row["mode"])]
    folder = Path(row["folder"])
    folder.mkdir(parents=True, exist_ok=True)
    registry, session, cleanup_errors = ProcessRegistry(folder), None, []
    solver_invoked = False
    started = time.monotonic()
    def phase(name, **values):
        nonlocal row
        row = coordinator.change(row["id"], {"phase": name, **values})
        require(not cancel_event.is_set(), "cancelled")
    try:
        phase("restoring")
        receipt = restore(state)
        durable_json(folder / "restore.json", receipt)
        phase("preflight", restore=receipt)
        verify_run_binding(manifest, row, spec)
        source_check(state, read_json(Path(manifest["prepared"]["source_manifest_path"])))
        phase("warming")
        binding = {"plan_id": row["plan_id"], "slot_id": row["id"], "arm": row["arm"],
                   "artifact_sha256": state["artifact_sha256"], "source_sha256": state["source_sha256"],
                   "baseline_sha256": digest(state["baseline"]), "inventory_sha256": state["native_inventory_sha256"],
                   "isolation_sha256": digest(state.get("isolation")), "warmup_policy_sha256": digest(state.get("warmup_policy"))}
        session = prepare_session({**state, "campaign_binding": binding}, folder, manifest["models"]["solver"], cancel_event, registry)
        warm = session.await_ready(time.monotonic() + 300)
        require(warm.get("ready") is True and warm.get("binding") == binding, "semantic ready gate/binding failed")
        require(isinstance(warm.get("ready_at"), (int, float)) and started <= warm["ready_at"] <= time.monotonic(),
                "warm readiness clock missing or invalid")
        durable_json(folder / "warm-receipt.json", warm)
        attachment = session.attachment()
        require(isinstance(attachment, dict), "warmed attachment 누락")
        phase("ready", warm_receipt={"path": str(folder / "warm-receipt.json"), "sha256": file_digest(folder / "warm-receipt.json")})
        require(not session.health().get("condition_errors"), "ready session unhealthy")
        verify_run_binding(manifest, row, spec)
        row = coordinator.change(row["id"], {"phase": "launch_reserved", "launch_state": "reserved"}, intent="launch")
        phase("running")
        solver_invoked = True
        execution = solver_executor(folder, Path(manifest["prepared"]["solver_inputs"][row["id"]]["path"]).read_text(),
            session=session, settings=copy.deepcopy(spec), cancel_event=cancel_event, process_registry=registry,
            codex_version=manifest["prepared"]["host"]["codex_version"],
            agent_version=manifest["prepared"]["host"].get("pi", manifest["prepared"]["host"].get("claude", {})).get("version"))
        verify_run_binding(manifest, row, spec)
        # A returned execution without a process receipt does not prove that the CLI started.
        process = read_json(folder / "process.json") if (folder / "process.json").exists() else None
        observed = {"launch_state": "started" if process else "unknown", "cli_launches": 1 if process else None,
                    "process": process, "status": execution["status"]}
        row = coordinator.change(row["id"], {"phase": "collecting", **observed})
        result = collector(folder, row, execution)
        require(result.get("model") == row["model"] and result.get("backend") == row["backend"], "관측 풀이 모델/실행기 불일치")
        result["answer_eligible"] = result.get("answer_eligible", False)
        # A recorded provider response ID proves a provider call; without one it stays unobserved, never false.
        result["provider_call_observed"] = True if (result.get("usage") or {}).get("response_ids") else None
        if not process:
            result.update(conditions_valid=False, answer_eligible=False,
                          failure_reason="CLI launch receipt missing; launch remains unknown")
        health = session.health()
        if health.get("condition_errors"):
            result.update(conditions_valid=False, answer_eligible=False, condition_errors=health["condition_errors"])
        row = coordinator.change(row["id"], result)
    except Exception as exc:
        if solver_invoked:
            try:
                process = read_json(folder / "process.json") if (folder / "process.json").exists() else None
            except (OSError, ValueError):
                process = None
            row = coordinator.change(row["id"], {"launch_state": "started" if process else "unknown",
                                                  "cli_launches": 1 if process else None, "process": process})
        status = ("cancelled" if cancel_event.is_set() else "warmup_failed" if row["phase"] in {"warming", "ready"}
                  else "preflight_failed" if row["launch_state"] == "not_started"
                  else "environment_error" if row["launch_state"] == "started" else "launch_unknown")
        row = coordinator.change(row["id"], {"status": status, "failure_stage": row["phase"], "failure_reason": str(exc),
            "conditions_valid": False, "answer_eligible": False,
            "provider_call_observed": False if row["launch_state"] == "not_started" else None})
    finally:
        row = coordinator.change(row["id"], {"phase": "cleanup", "cancel_requested": cancel_event.is_set()})
        receipt = {"complete": True}
        if session:
            try:
                receipt = session.close(row["status"])
                require(receipt.get("complete") is True, "warm session cleanup incomplete")
            except Exception as exc:
                receipt = {"complete": False, "error": str(exc)}
                cleanup_errors.append(str(exc))
        groups = registry.cleanup()
        if not groups["complete"]:
            cleanup_errors.append("owned process group cleanup incomplete")
        try:
            source_check(state, read_json(Path(manifest["prepared"]["source_manifest_path"])))
        except Exception as exc:
            cleanup_errors.append(str(exc))
        if receipt.get("complete") is True and groups["complete"]:
            try:
                restore(state)
            except Exception as exc:
                cleanup_errors.append(str(exc))
        cleanup = {"complete": not cleanup_errors, "session": receipt, "groups": groups, "errors": cleanup_errors}
        durable_json(folder / "cleanup.json", cleanup)
        row.update(cleanup=cleanup, lifecycle_seconds=time.monotonic() - started)
        if cleanup_errors:
            row["conditions_valid"] = False
            cancel_event.set()
        durable_json(folder / "collected.json", row)
        row.update(seal_slot(folder, row))
        row = coordinator.change(row["id"], {**row, "phase": "terminal"})
    return row


def verify_raw_seal(row: dict) -> None:
    path = Path(row["raw_seal"])
    require(file_digest(path) == row["raw_seal_sha256"], "원자료 seal 변경")
    seal = read_json(path)
    require(seal["slot_id"] == row["id"] and seal["run_spec_sha256"] == row["run_spec_sha256"], "원자료 실행 binding 변경")
    for name, item in seal["files"].items():
        require(Path(name).name == name and file_digest(path.parent / name) == item["sha256"]
                and (path.parent / name).stat().st_size == item["bytes"], "봉인된 원자료 변경")


def grade_packet(root: Path, manifest: dict, initial: dict, coordinator: LedgerCoordinator, cancel_event, grader_executor) -> dict:
    row = copy.deepcopy(initial)
    try:
        folder = Path(row["packet"])
        seal = read_json(folder / "seal.json")
        require(seal["run_id"] == row["id"] and seal["raw_seal_sha256"] == row["raw_seal_sha256"], "채점/원자료 binding 변경")
        require(file_digest(folder / "seal.json") == row["packet_seal_sha256"], "packet seal 변경")
        for name, checksum in seal["files"].items():
            require(Path(name).name == name and file_digest(folder / name) == checksum, "채점 입력 변경")
        require(file_digest(Path(seal["registry"])) == seal["registry_sha256"], "private registry 변경")
        verify_raw_seal(row)
        row = coordinator.change(row["id"], {"grade_state": "grading"}, intent="grader-launch")
        spec = copy.deepcopy(manifest["run_specs"][routes.execution_key(row["arm"], row["mode"])])
        spec.pop("native_agent", None)
        spec["limits"] = {**spec["limits"], "elapsed_seconds": manifest["grader_timeout_seconds"]}
        result = grader_executor(folder / "job", (folder / "prompt.txt").read_text(), grading=True,
            output_schema=read_json(folder / "schema.json"), relay_settings=None, settings=spec,
            usage_drain_seconds=0, cancel_event=cancel_event)
        durable_json(folder / "grader-result.json", result, exclusive=True)
        row = coordinator.change(row["id"], {"grader_execution": {
            **{key: result.get(key) for key in ("status", "backend", "provider", "model", "agent_version", "usage", "exit_code",
                "conditions_valid", "condition_errors", "harness_attempts", "retry_attempts", "failed_attempts", "token_accounting", "retry_events", "compaction_events")},
            "artifact": str(folder / "grader-result.json"), "sha256": file_digest(folder / "grader-result.json")}})
        require(not cancel_event.is_set(), "grader cancelled")
        require(result.get("status") == "completed" and result.get("conditions_valid") is True
                and result.get("model") == "gpt-6-astra" and result.get("backend") == "pi"
                and result.get("provider") == "openai-codex",
                f"pi/Astra 실행 조건/모델 불일치 또는 미완료: {result.get('status')}; {result.get('condition_errors', [])}")
        from .campaign_store import durable_bytes
        durable_bytes(folder / "judgment-raw.json", result["answer"].encode(), exclusive=True)
        judgment = route_grading.parse_judgment(result["answer"])
        question = next(q for q in routes.load_dataset()["questions"] if q["id"] == row["question_id"])
        judgment = route_grading.validate_judgment(question, judgment, row["blind_id"], read_json(Path(seal["registry"]))[row["blind_id"]])
        durable_json(folder / "judgment.json", judgment, exclusive=True)
        return coordinator.change(row["id"], {"grade_state": "validated", "judgment": str(folder / "judgment.json"),
            "judgment_sha256": file_digest(folder / "judgment.json"), "judgment_packet_sha256": row["packet_seal_sha256"]})
    except Exception as exc:
        return coordinator.change(row["id"], {"grade_state": "cancelled" if cancel_event.is_set() else "failed", "grade_reason": str(exc)})


def schedule_workers(rows: list[dict], coordinator: LedgerCoordinator, cancel_event, worker):
    pending, active, arms = list(rows), {}, set()
    with ThreadPoolExecutor(max_workers=4) as pool:
        while pending or active:
            coordinator.drain()
            if cancel_event.is_set():
                pending.clear()
            while len(active) < 4:
                round_number = min((r["attempt"] for r in [*pending, *active.values()]), default=None)
                candidate = next((r for r in pending if r["attempt"] == round_number and r["arm"] not in arms), None)
                if candidate is None:
                    break
                pending.remove(candidate)
                arms.add(candidate["arm"])
                active[pool.submit(worker, candidate)] = candidate
            for future in list(active):
                if future.done():
                    row = active.pop(future)
                    arms.remove(row["arm"])
                    try:
                        future.result()
                    except Exception as exc:
                        cancel_event.set()
                        coordinator.ledger["error"] = str(exc)
            time.sleep(.005)
    coordinator.drain()
    if coordinator.fatal:
        raise ContractError(coordinator.fatal)


def recover_interrupted(root: Path, coordinator: LedgerCoordinator) -> None:
    """An intent is irreversible, including a crash before Popen. Never guess safe cleanup."""
    for row in coordinator.ledger["slots"]:
        intent = root / "runs" / row["id"] / "launch-intent.json"
        folder = Path(row.get("folder", str(intent.parent)))
        if row["phase"] not in {"pending", "terminal"} or intent.exists() and row["phase"] != "terminal":
            if (folder / "raw-seal.json").exists() and (folder / "collected.json").exists():
                recovered = read_json(folder / "collected.json")
                recovered.update(raw_seal=str(folder / "raw-seal.json"), raw_seal_sha256=file_digest(folder / "raw-seal.json"))
                require(recovered["id"] == row["id"] and recovered["run_spec_sha256"] == row["run_spec_sha256"], "orphan raw binding 변경")
                verify_raw_seal(recovered)
                row.update(recovered, phase="terminal")
            else:
                row.update(phase="terminal", status="launch_unknown" if intent.exists() else "preflight_failed",
                           launch_state="unknown" if intent.exists() else "not_started", conditions_valid=False,
                           failure_reason="interrupted lifecycle; no automatic relaunch or unsafe restore", grade_state="not_eligible",
                           cleanup={"complete": False, "errors": ["ownership/cleanup requires operator inspection"]})
        if ((root / "runs" / row["id"] / "grader-launch-intent.json").exists()
                and row["grade_state"] not in {"validated", "failed", "cancelled"}):
            row.update(grade_state="failed", grade_reason="interrupted grader launch; automatic retry forbidden")
    coordinator.commit()


def grading_eligible(row: dict) -> bool:
    return (row.get("status") in {"completed", "timeout", "call_limit", "token_limit"}
            and row.get("conditions_valid") is True and row.get("answer_eligible") is True
            and bool(row.get("answer", "").strip()))


def prepare_review_packets(root: Path, manifest: dict, coordinator: LedgerCoordinator,
                           rows: list[dict], source_check) -> None:
    """Read sealed raw evidence and the fixed baseline; never restore a live workspace."""
    dataset = routes.load_dataset()
    for row in rows:
        if row["grade_state"] != "not_eligible" or row.get("grade_reason"):
            continue
        if not grading_eligible(row):
            row["grade_reason"] = "no eligible complete answer with valid conditions"
            continue
        try:
            verify_raw_seal(row)
            require(read_json(Path(row["raw_seal"])).get("cleanup_complete") is True, "sealed cleanup incomplete")
            state = manifest["prepared"]["arms"][row["arm"]]
            corpus_path = Path(manifest["prepared"]["source_manifest_path"])
            require(file_digest(corpus_path) == manifest["prepared"]["source_manifest_sha256"], "고정 corpus 변경")
            corpus = read_json(corpus_path)
            source = Path(state["baseline"]["paths"]["source"])
            source_check({**state, "source": str(source)}, corpus)
            result = route_grading.build_review_packet(root,
                next(q for q in dataset["questions"] if q["id"] == row["question_id"]), row, source,
                manifest["run_specs"][routes.execution_key(row["arm"], row["mode"])], Path(row["raw_seal"]), corpus)
            row.update(result)
        except Exception as exc:
            row.update(grade_state="failed", grade_reason=str(exc))
        coordinator.commit()


def _run_campaign(root: Path, *, resume=False, retry_unstarted=False, cancel_event=None, prepare_session=None,
                  solver_executor=execute_solver, grader_executor=None, collector=None,
                  restore=restore_slot, source_check=verify_source) -> dict:
    manifest, ledger = load_ledger(root)
    verify_prepared_plan(manifest["plan_id"], Path(manifest["readiness"]), manifest["models"]["solver"]["model"], manifest["readiness_sha256"])
    cancel_event = cancel_event or threading.Event()
    coordinator = LedgerCoordinator(root, ledger, cancel_event)
    if resume:
        recover_interrupted(root, coordinator)
    rows = coordinator.ledger["slots"]
    if any(r.get("cleanup", {}).get("complete") is False for r in rows):
        coordinator.ledger["status"] = "blocked"
        coordinator.commit()
        return report_campaign(root)
    eligible = [r for r in rows if r["launch_state"] == "not_started" and r.get("status") != "out_of_scope" and
                (r["phase"] == "pending" and not resume or resume and retry_unstarted)]
    if eligible:
        try:
            factory = prepare_session or session_factory()
        except ContractError as exc:
            coordinator.ledger.update(status="blocked", error=str(exc))
            coordinator.commit()
            return report_campaign(root)
        for row in eligible:
            require(not (root / "runs" / row["id"] / "launch-intent.json").exists(), "launch intent가 있는 슬롯은 재실행하지 않습니다")
            attempt = row.get("lifecycle_attempts", 0) + 1
            if attempt > 1:
                durable_json(root / "runs" / row["id"] / f"preflight-{attempt - 1}.json", row, exclusive=True)
            for key in ("cleanup", "failure_reason", "failure_stage", "grade_reason", "raw_seal", "raw_seal_sha256"):
                row.pop(key, None)
            row.update(folder=str(root / "runs" / row["id"] / f"lifecycle-{attempt}"), lifecycle_attempts=attempt,
                       phase="pending", status="planned", conditions_valid=None, grade_state="not_eligible")
        coordinator.commit()
        schedule_workers(eligible, coordinator, cancel_event, lambda row: run_slot(root, manifest, row, coordinator, cancel_event,
            factory, solver_executor, collector or route_metrics.finish_execution, restore, source_check))
    for row in coordinator.ledger["slots"]:
        if row["phase"] == "pending" and cancel_event.is_set():
            row.update(phase="terminal", status="cancelled", cancel_requested=True, provider_call_observed=False)
    coordinator.commit()
    # No graders overlap solver/product lifecycles or a failed cleanup.
    if not cancel_event.is_set():
        prepare_review_packets(root, manifest, coordinator, coordinator.ledger["slots"], source_check)
        schedule_workers([r for r in coordinator.ledger["slots"] if r["grade_state"] == "sealed"], coordinator, cancel_event,
            lambda row: grade_packet(root, manifest, row, coordinator, cancel_event, grader_executor or runner.execute_model))
    rows = [r for r in coordinator.ledger["slots"] if r.get("status") != "out_of_scope"]
    status = ("blocked" if any(r.get("cleanup", {}).get("complete") is False for r in rows) else
              "cancelled" if cancel_event.is_set() else "awaiting_review" if any(r["grade_state"] == "review_required" for r in rows) else
              "completed" if all(r["grade_state"] == "validated" for r in rows) else "completed_with_failures")
    coordinator.ledger["status"] = status
    coordinator.commit()
    return report_campaign(root)


def run_campaign(root: Path, **options) -> dict:
    with campaign_lease():
        return _run_campaign(root.resolve(), **options)


def run_new(request: dict, *, cancel_event=None) -> dict:
    with campaign_lease():
        readiness = Path(request["readiness"])
        prepared = verify_prepared_plan(request["plan_id"], readiness, request["solver_model"], request["readiness_sha256"])
        root, _ = authorize_run(prepared, readiness, request)
        return _run_campaign(root, cancel_event=cancel_event)


def grade_sealed(root: Path, *, cancel_event=None, grader_executor=None, source_check=verify_source) -> dict:
    """Explicit partial grading of clean raw seals; never unblock or restore a campaign."""
    with campaign_lease():
        root = root.resolve()
        manifest, ledger = load_ledger(root)
        require(ledger["status"] == "blocked", "grade-sealed is only for a blocked campaign")
        cancel_event = cancel_event or threading.Event()
        coordinator = LedgerCoordinator(root, ledger, cancel_event)
        rows = [row for row in ledger["slots"] if row.get("raw_seal") and grading_eligible(row)
                and row.get("cleanup", {}).get("complete") is True
                and row["grade_state"] in {"not_eligible", "review_required", "sealed"}]
        for row in rows:
            require(not (root / "runs" / row["id"] / "grader-launch-intent.json").exists(), "grader retry forbidden")
            verify_raw_seal(row)
            require(read_json(Path(row["raw_seal"])).get("cleanup_complete") is True, "sealed cleanup incomplete")
        request = {"mode": "explicit_partial_grading", "requested_at": timestamp(),
            "candidate_slots": [row["id"] for row in rows], "campaign_error": ledger.get("error"),
            "blocking_slots": [{"id": row["id"], "status": row.get("status"),
                "failure_reason": row.get("failure_reason"), "cleanup_errors": row.get("cleanup", {}).get("errors", [])}
                for row in ledger["slots"] if row.get("cleanup", {}).get("complete") is False]}
        coordinator.ledger.setdefault("blocked_grading", []).append(request)
        coordinator.commit()
        prepare_review_packets(root, manifest, coordinator, rows, source_check)
        schedule_workers([row for row in rows if row["grade_state"] == "sealed"], coordinator, cancel_event,
            lambda row: grade_packet(root, manifest, row, coordinator, cancel_event, grader_executor or runner.execute_model))
        request["validated_slots"] = [row["id"] for row in rows if row["grade_state"] == "validated"]
        # Failed/unknown cleanup in other slots remains blocked, with its raw evidence intact.
        coordinator.commit()
        return report_campaign(root)


def approve_review(root: Path, slot_id: str, approval: dict) -> dict:
    with campaign_lease():
        manifest, ledger = load_ledger(root)
        row = next(r for r in ledger["slots"] if r["id"] == slot_id)
        require(row["grade_state"] in {"review_required", "sealed"}, "검토 대기/승인 슬롯이 아닙니다")
        verify_raw_seal(row)
        require(file_digest(Path(row["review"])) == row["review_sha256"], "검토 draft 변경")
        result = route_grading.approve_identity_review(root, Path(row["review"]), approval)
        row.update(result)
        coordinator = LedgerCoordinator(root, ledger, threading.Event())
        coordinator.commit()
        return report_campaign(root)


def report_campaign(root: Path) -> dict:
    manifest, ledger = load_ledger(root)
    judgments = {}
    for row in ledger["slots"]:
        if row["grade_state"] == "validated":
            require(row["judgment_packet_sha256"] == row["packet_seal_sha256"] and
                    file_digest(Path(row["judgment"])) == row["judgment_sha256"], "채점/packet binding 변경")
            judgments[row["id"]] = read_json(Path(row["judgment"]))
    result = route_metrics.report(root, {**manifest["prepared"], "campaign_status": ledger["status"],
                                        "blocked_grading": ledger.get("blocked_grading", []),
                                        "run_scope": manifest["authorization"].get("scope", {"kind": "full"})}, ledger["slots"], judgments)
    return {"campaign_id": manifest["campaign_id"], "plan_id": manifest["plan_id"], "status": ledger["status"],
            "scope": manifest["authorization"].get("scope", {"kind": "full"}), "error": ledger.get("error"), "blocked_grading": ledger.get("blocked_grading", []),
            "counts": result["counts"], "ledger": str(root / "ledger.json"),
            "report": str(root / "report/results.md"), "run_root": str(root)}


def operator_inspection(solver_model: str | None = None) -> dict:
    """Reproducible model-free CUI evidence using the actual catalog and immutable plan."""
    solver = routes.solver_model(solver_model)
    record = read_json(route_prepare.readiness_path(solver["model"]))
    path = routes.PREPARATION / "plans" / record["plan_id"] / "readiness.json"
    verify_prepared_plan(record["plan_id"], path, solver["model"], file_digest(path))
    inputs = {"catalog": catalog(), "plan": route_prepare.readiness_projection(record, path)}
    script = f"""
import {{ interactive }} from {json.dumps((settings.ROOT / 'start.mjs').as_uri())};
let text = ''; for await (const chunk of process.stdin) text += chunk;
const inputs = JSON.parse(text), calls = [], messages = [], menus = [];
const prompts = {{
  select: async (options) => {{ menus.push(options); return menus.length === 1 ? 'grafana-routes' : inputs.plan.spec.model; }},
  confirm: async (options) => {{ menus.push(options); if (options.default !== false) throw Error('unsafe confirmation default'); return false; }},
}};
await interactive({{ prompts, output: {{info: (text) => messages.push(text)}}, call: async (action, request) => {{
  calls.push({{action, request}});
  if (action === 'catalog') return inputs.catalog;
  if (action === 'prepare') return inputs.plan;
  throw Error('model launch forbidden during inspection: ' + action);
}} }});
console.log(JSON.stringify({{calls, menus, messages}}));
"""
    observed = subprocess.run(["node", "--input-type=module", "-e", script], input=json.dumps(inputs),
                              text=True, capture_output=True, timeout=30, check=True)
    flow = json.loads(observed.stdout)
    require([c["action"] for c in flow["calls"]] == ["catalog", "prepare"], "inspection 실행 경계 위반")
    value = {"schema_version": 1, "recorded_at": timestamp(), **inputs, "cui_flow": flow,
             "solver_runs": 0, "grader_runs": 0, "implementation_sha256": settings.harness_identity(),
             "inspection": "real CUI with verified catalog/readiness projection; confirmation declined; backend calls intercepted"}
    suffix = "" if solver["model"] == routes.load_profile()["solver"]["model"] else "-" + solver["model"]
    output = routes.EVIDENCE / f"07-operator-inspection{suffix}.json"
    durable_json(output, value)
    return {**value, "evidence": str(output)}
