"""Blind grading with fixed controls, verbatim units and sealed, resumable batches."""
from __future__ import annotations

import copy
import json
import random
import time
from pathlib import Path

from . import grading_support as support
from .v2 import grading, runner
from .v2.core import MODEL_TERMINALS, ContractError, canonical, digest, file_digest, read_json, require, write_json


INSTRUCTIONS = grading.GRADER_INSTRUCTIONS.split("출력 형식:")[0].replace(
    "각 fact의 answer_quote는 저장된 answer 안의 정확한 부분문자열이다. 누락이면 빈 문자열을 쓴다.",
    "각 fact의 answer_unit_id는 저장된 답안의 실제 단위 id이다. 누락이면 none을 쓴다.") + support.SEMANTICS


def packet(question: dict, runs: list[dict], source: Path, experiment: Path, seed: int) -> tuple[dict, dict, list]:
    registry, answers, warnings = {}, [], []
    for run in runs:
        ident, masked, replacements, positions, flags = support.mask_answer(run, source, experiment)
        require(ident not in registry, "익명 답변 ID 충돌")
        registry[ident] = {"run_id": run["id"], "question_id": question["id"], "answer": run["answer"],
                           "blinded_answer": masked, "blinding": replacements, "positions": positions,
                           "control": False, "units": support.units(masked)}
        answers.append({"answer_id": ident, "answer": masked})
        if flags:
            warnings.append({"run_id": run["id"], "answer_sha256": digest(run["answer"]), "flags": flags})
    for control in question["examples"]:
        ident = digest({"seed": seed, "question": question["id"], "control": control["id"]})[:24]
        rendered = support.render_control(control["answer"])
        registry[ident] = {"question_id": question["id"], "answer": rendered, "control": True,
                           "expected": control["expected"], "units": support.units(rendered)}
        answers.append({"answer_id": ident, "answer": rendered})
        # The same original control claims must survive the line-ID codec and presentation change.
        expected = copy.deepcopy(control["expected"])
        expected.update(answer_id=ident, question_id=question["id"])
        original_units = support.units(control["answer"])
        for part in expected["facts"] + expected["major_errors"]:
            quote = part.pop("answer_quote")
            part["answer_unit_id"] = "none" if not quote else next(u["id"] for u in original_units if quote in u["text"])
        decoded = support.decode({"judgments": [expected]}, registry)["judgments"][0]
        grading.validate_judgment(question, decoded, ident, rendered)
        require(grading.control_signature(decoded) == grading.control_signature(control["expected"]), "채점 대조 의미 변경")
    random.Random(seed).shuffle(answers)
    supplemental_answers = []
    for item in answers:
        rendered = item.pop("answer")
        item["answer_units"] = [{"id": u["id"], "text": u["text"]} for u in registry[item["answer_id"]]["units"]]
        item["citation_index"] = support.label_ranges(rendered)
        citations = "\n".join(f"{c['path']}:{c['start_line']}-{c['end_line']}" for c in item["citation_index"])
        supplemental_answers.append({"answer": rendered + "\n" + citations})
    bundle = {"questions": [{"question_id": question["id"], "prompt": question["prompt"],
                             "facts": question["facts"], "evidence": question["evidence"], "answers": answers}]}
    return {"bundle": bundle, "source_supplement": support.source_supplement(question, supplemental_answers, source, full_files=False)}, registry, warnings


def check_review(experiment: Path, warnings: list):
    if not warnings:
        return
    path = experiment / "grading/review-required.json"
    write_json(path, warnings)
    resolutions_path = path.with_name("review-resolutions.json")
    resolutions = read_json(resolutions_path) if resolutions_path.exists() else {}
    pending = [item for item in warnings if not (
        isinstance(resolutions.get(item["run_id"]), dict)
        and resolutions[item["run_id"]].get("answer_sha256") == item["answer_sha256"]
        and resolutions[item["run_id"]].get("decision") == "allow"
        and isinstance(resolutions[item["run_id"]].get("reason"), str)
        and resolutions[item["run_id"]]["reason"].strip())]
    require(not pending, f"익명화 검토가 필요합니다: {path}. 원답변을 고치지 말고 review-resolutions.json에 검토 근거를 기록한 뒤 이어가세요")


def prepare(experiment: Path, dataset: dict, runs: list[dict], source: Path, spec: dict) -> list[Path]:
    root = experiment / "grading"
    root.mkdir(exist_ok=True)
    manifest = root / "manifest.json"
    if manifest.exists():
        frozen = read_json(manifest)
        require(frozen["runs_sha256"] == digest(runs), "채점 입력 변경")
        batches = [root / name for name in frozen["batches"]]
        for folder in batches:
            for name, checksum in read_json(folder / "inputs.json").items():
                require(file_digest(folder / name) == checksum, "채점 패킷 변경")
        check_review(experiment, read_json(root / "review-required.json"))
        return batches
    batches, warnings, mechanical = [], [], {}
    for question in dataset["questions"]:
        selected = [r for r in runs if r["question_id"] == question["id"] and r["status"] in MODEL_TERMINALS and r["conditions_valid"]]
        for run in selected:
            if not run["answer"].strip():
                mechanical[run["id"]] = grading.no_answer_judgment(question, run["id"])
        selected = [r for r in selected if r["answer"].strip()]
        start = 0
        while start < len(selected):
            count = min(5, len(selected) - start)
            while True:
                value, registry, flags = packet(question, selected[start:start + count], source, experiment, spec["seed"])
                prompt = INSTRUCTIONS + canonical(value)
                if len(prompt.encode()) <= spec["batch_limits"]["bytes"]:
                    break
                require(count > 1, "답변 하나의 채점 패킷이 160 KiB를 초과했습니다. 원문을 자동 축약하거나 모델을 실행하지 않습니다")
                count -= 1
            start += count
            folder = root / f"batch-{len(batches) + 1:03}"
            folder.mkdir(exist_ok=True)
            write_json(folder / "packet.json", value)
            write_json(folder / "registry.json", registry)
            write_json(folder / "schema.json", support.unit_schema(max(len(r["units"]) for r in registry.values())))
            (folder / "prompt.txt").write_text(prompt)
            write_json(folder / "inputs.json", {name: file_digest(folder / name) for name in
                                               ("packet.json", "registry.json", "schema.json", "prompt.txt")})
            batches.append(folder)
            warnings.extend(flags)
    write_json(root / "mechanical.json", mechanical)
    write_json(root / "review-required.json", warnings)
    write_json(manifest, {"runs_sha256": digest(runs), "batches": [path.name for path in batches]}, exclusive=True)
    check_review(experiment, warnings)
    return batches


def run(experiment: Path, dataset: dict, rows: list[dict], source: Path, spec: dict, cancel_event, emit=print) -> dict:
    from .v2.core import command
    batches = prepare(experiment, dataset, rows, source, spec)
    combined = read_json(experiment / "grading/mechanical.json")
    statuses = []
    for folder in batches:
        if cancel_event.is_set():
            break
        registry = read_json(folder / "registry.json")
        saved = folder / "status.json"
        if saved.exists():
            status = read_json(saved)
            require(file_digest(saved) == read_json(folder / "seal.json")["status_sha256"], "채점 결과 변경")
            if status["status"] == "complete":
                decoded = support.decode(read_json(folder / "raw.json"), registry)
                verified = grading.validate_batch(dataset, read_json(folder / "packet.json")["bundle"], registry, decoded)
                require(verified == status["judgments"], "채점 결과 재검증 실패")
        else:
            session = folder / "session"
            if (session / "started.json").exists() and not (session / "execution.json").exists():
                status = {"status": "incomplete", "judgments": {}, "error": "이전 채점 세션 중단; 자동 재시도 없음"}
            else:
                try:
                    if not (session / "started.json").exists():
                        version = command(["codex", "--version"]).strip()
                        write_json(session / "started.json", {"started_unix_seconds": time.time(), "codex_version": version}, exclusive=True)
                        runner.execute_codex(session, (folder / "prompt.txt").read_text(), grading=True,
                                             output_schema=read_json(folder / "schema.json"), settings=spec,
                                             cancel_event=cancel_event, codex_version=version)
                    result = read_json(session / "execution.json")
                    require(result["status"] == "completed" and result["conditions_valid"], "채점 실행 조건 또는 종료 상태 오류")
                    raw = json.loads(result["answer"])
                    write_json(folder / "raw.json", raw)
                    decoded = support.decode(raw, registry)
                    write_json(folder / "decoded.json", decoded)
                    validated = grading.validate_batch(dataset, read_json(folder / "packet.json")["bundle"], registry, decoded)
                    status = {"status": "complete", "judgments": validated, "codex_version": result["codex_version"]}
                except (ValueError, KeyError, TypeError, OSError, StopIteration) as exc:
                    status = {"status": "incomplete", "judgments": {}, "error": str(exc)}
            write_json(saved, status, exclusive=True)
            write_json(folder / "seal.json", {"status_sha256": file_digest(saved),
                       "files": {str(path.relative_to(folder)): file_digest(path) for path in folder.rglob("*")
                                 if path.is_file() and "codex-home" not in path.parts}}, exclusive=True)
        for name, checksum in read_json(folder / "seal.json")["files"].items():
            require(file_digest(folder / name) == checksum, "채점 원자료 변경")
        combined.update(status["judgments"])
        statuses.append({"batch": folder.name, **{key: value for key, value in status.items() if key != "judgments"}})
        emit(f"채점 {folder.name}: {status['status']}")
    result = {"judgments": combined, "batches": statuses, "grader": spec["grader_model"],
              "reasoning_effort": spec["grader_reasoning_effort"],
              "complete": len(statuses) == len(batches) and all(s["status"] == "complete" for s in statuses)}
    write_json(experiment / "grading/judgments.json", result)
    return result
