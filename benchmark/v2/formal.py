"""Frozen multi-repository evaluation using the unchanged V2 execution and grading core."""
from __future__ import annotations

import argparse
import collections
import concurrent.futures
import datetime
import fcntl
import random
import re
import shutil
import statistics
import subprocess
import time
from pathlib import Path

from . import grading, runner
from .core import (SPEC, canonical, command, digest, file_digest, metric, percentile,
                   question_schedule, read_json, require, write_json)
from .dataset import source_text
from .metrics import compare, exploration_metrics, normalize_run

PROTOCOL = {
    "name": "multi-repository-formal-r1", "repositories": 6, "questions_per_repository": 5,
    "repeats": 2, "scheduled_runs": 120, "difficulty_counts": {"simple": 10, "medium": 10, "complex": 10},
    "model": SPEC["model"], "reasoning_effort": SPEC["reasoning_effort"],
    "grader_model": SPEC["grader_model"], "grader_reasoning_effort": SPEC["grader_reasoning_effort"],
    "codex_version": SPEC["codex_version"], "seed": SPEC["seed"], "limits": SPEC["limits"],
    "concurrency": 2, "groups": SPEC["groups"], "host": SPEC["host"],
    "selection": "Purposive fixed-code questions from six repositories unused in Grafana development; not PR sampled",
    "weighting": "equal repository and question weight; average repetitions within question",
    "uncertainty": "paired hierarchical bootstrap: repository, then question; retain A/B and repetitions",
    "automatic_retries": False, "grading_questions_per_batch": 1,
    "product_tuning_after_freeze": False, "grading_tuning_after_freeze": False,
}
ANSWER_PROMPT = runner.ANSWER_PROMPT.replace("고정된 Grafana 코드", "제공된 저장소의 고정 코드")


def validate(dataset: dict, root: Path, *, check_source=True):
    require(dataset["protocol"] == PROTOCOL, "formal protocol drift")
    repos = dataset["repositories"]
    require(len(repos) == PROTOCOL["repositories"], "expected six repositories")
    questions = dataset["questions"]
    require(len(questions) == 30 and len({q["id"] for q in questions}) == 30, "expected thirty unique questions")
    require(dict(collections.Counter(q["difficulty"] for q in questions)) == PROTOCOL["difficulty_counts"], "difficulty balance drift")
    for key, repo in repos.items():
        require(re.fullmatch(r"[a-z0-9-]+", key), "unsafe repository key")
        require(len([q for q in questions if q["repository_id"] == key]) == 5, "repository question balance drift")
        if check_source:
            source = root / "repositories" / key
            require(command(["git", "rev-parse", "HEAD"], source).strip() == repo["commit"], "repository commit drift")
            require(not command(["git", "status", "--porcelain"], source).strip(), "modified source checkout")
    for q in questions:
        require(re.fullmatch(r"[a-z0-9-]+", q["id"]), "unsafe question ID")
        require(q["repository_id"] in repos and q["phase"] == "main", "invalid question source/phase")
        require(q["source_reviewed"] and q["difficulty_rationale"] and q["tags"], "missing prospective source review")
        evidence = {e["id"]: e for e in q["evidence"]}
        require(evidence and len(evidence) == len(q["evidence"]), "missing/duplicate evidence")
        for e in evidence.values():
            require(e["text"].strip() and digest(e["text"]) == e["text_sha256"], "evidence hash mismatch")
            require(e["end_line"] - e["start_line"] + 1 == len(e["text"].splitlines()), "evidence range mismatch")
            if check_source:
                require(source_text(root / "repositories" / q["repository_id"], e["path"], e["start_line"], e["end_line"]) == e["text"], "evidence source mismatch")
        require(len(q["facts"]) >= 2 and len({f["id"] for f in q["facts"]}) == len(q["facts"]), "missing/duplicate facts")
        for fact in q["facts"]:
            require(fact["requirement"] in q["prompt"] and fact["correct"] and fact["wrong_claims"], "incomplete fact contract")
            require(fact["evidence_sets"] and all(group and set(group) <= evidence.keys() for group in fact["evidence_sets"]), "invalid evidence sets")
        require(collections.Counter(e["kind"] for e in q["examples"]) == collections.Counter(["minimal", "alternative", "partial", "wrong"]), "expected four controls")
        for example in q["examples"]:
            judgment = dict(example["expected"], answer_id=example["id"], question_id=q["id"])
            grading.validate_judgment(q, judgment, example["id"], example["answer"])
            require(grading.classify(q, judgment) == {"minimal": "correct", "alternative": "correct", "partial": "partial", "wrong": "incorrect"}[example["kind"]], "control classification mismatch")
    require(len(question_schedule(dataset, "main")) == PROTOCOL["scheduled_runs"], "schedule size mismatch")


def prepare(root: Path, dataset: dict):
    validate(dataset, root)
    binary = root / "product/codemap-search"
    build = read_json(root / "product/build.json")
    require(build["exit_code"] == 0 and file_digest(binary) == build["binary_sha256"], "product build mismatch")
    require(digest(runner.source_manifest(root / "product/source/apps/codemap-search")) == build["source_manifest_sha256"], "product source mismatch")
    for key, repo in dataset["repositories"].items():
        output = root / "preparation" / key
        if (output / "preparation.json").exists():
            continue
        output.mkdir(parents=True, exist_ok=False)
        started = time.monotonic()
        snapshot = output / "source"
        runner.copy_source(root / "repositories" / key, snapshot)
        shutil.rmtree(snapshot / ".git")
        (snapshot / ".git").mkdir()
        product_home = output / "product-home"
        product_home.mkdir()
        import os
        index_started = time.monotonic()
        with (output / "index.stdout.log").open("w") as stdout, (output / "index.stderr.log").open("w") as stderr:
            result = subprocess.run([str(binary), "index", "."], cwd=snapshot, stdout=stdout, stderr=stderr,
                                    env={**os.environ, "CODEMAP_HOME": str(product_home)}, timeout=600)
        require(result.returncode == 0, f"index failed: {key}")
        manifest = runner.source_manifest(snapshot)
        write_json(output / "source-manifest.json", manifest, exclusive=True)
        write_json(output / "preparation.json", {"repository": repo, "source_manifest_sha256": digest(manifest),
                   "snapshot": str(snapshot), "index_elapsed_seconds": time.monotonic() - index_started,
                   "preparation_elapsed_seconds": time.monotonic() - started,
                   "index_bytes": sum(p.stat().st_size for p in (snapshot / ".codemap").rglob("*") if p.is_file())}, exclusive=True)
        print(f"prepared {key}", flush=True)


def bundles_for(dataset: dict, runs: list[dict] | None):
    controls_only = runs is None
    if controls_only:
        runs = [{"id": f"calibration-{q['id']}", "question_id": q["id"], "answer": ""} for q in dataset["questions"]]
    bundles, registry = grading.make_bundles(dataset, runs, max_questions=1)
    if controls_only:
        for bundle in bundles:
            for q in bundle["questions"]:
                q["answers"] = [a for a in q["answers"] if registry[a["answer_id"]]["control"]]
        registry = {key: value for key, value in registry.items() if value["control"]}
    return bundles, registry


def grade_collection(root: Path, dataset: dict, *, calibration=False):
    if not calibration:
        verify_freeze(root, dataset)
        runs = runner.load_experiment_runs(root)
        require(all(r["status"] not in {"scheduled", "running"} for r in runs), "all 120 runs must end before grading")
    else:
        require(not (root / "frozen.json").exists(), "calibration cannot change after freeze")
        validate(dataset, root)
        runs = None
    bundles, registry = bundles_for(dataset, runs)
    collection = root / ("calibration-final" if calibration else "evaluation-grading")
    collection.mkdir(exist_ok=True)
    identity = {"dataset_sha256": digest(dataset), "grader_instructions_sha256": digest(grading.GRADER_INSTRUCTIONS),
                "schema_sha256": digest(grading.judgment_schema()), "bundles_sha256": digest(bundles), "registry_sha256": digest(registry)}
    if (collection / "input-identity.json").exists():
        require(read_json(collection / "input-identity.json") == identity, "grading input drift")
    else:
        write_json(collection / "input-identity.json", identity, exclusive=True)
    def one(bundle):
        qid = bundle["questions"][0]["question_id"]
        folder = collection / qid
        folder.mkdir(exist_ok=True)
        ids = {a["answer_id"] for q in bundle["questions"] for a in q["answers"]}
        source = folder
        selected_registry = {key: registry[key] for key in ids}
        prior = root / "calibration" / qid
        if calibration and (prior / "grading/judgments.json").exists():
            original = read_json(root / "dataset-initial.json")
            old_question = next((q for q in original["questions"] if q["id"] == qid), None)
            new_question = next(q for q in dataset["questions"] if q["id"] == qid)
            require(old_question == new_question, "cannot reuse a changed calibration contract")
            old_bundle = read_json(prior / "grading/batch-001/input.json")
            old_registry = read_json(prior / "grading/private-registry.json")
            require(old_registry == selected_registry, "calibration answers changed")
            old_group, new_group = old_bundle["questions"][0], bundle["questions"][0]
            require({k: v for k, v in old_group.items() if k != "answers"} == {k: v for k, v in new_group.items() if k != "answers"}
                    and sorted(old_group["answers"], key=lambda a: a["answer_id"]) == sorted(new_group["answers"], key=lambda a: a["answer_id"]),
                    "calibration bundle contents changed")
            # Reuse the original order and sealed observations, without a model retry.
            source, bundle = prior, old_bundle
        result = grading.grade_bundles(dataset, source, [bundle], selected_registry)
        result["source_experiment"] = str(source)
        print(f"{'calibration' if calibration else 'grading'} {qid}: {result['batches'][0]['status']}", flush=True)
        return qid, result
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
        results = dict(executor.map(one, bundles))
    summary = {"identity": identity, "collections": results,
               "passed": all(all(b["status"] == "complete" for b in value["batches"]) for value in results.values())}
    write_json(collection / "summary.json", summary)
    return summary


def verify_grades(root: Path, dataset: dict, name: str) -> dict:
    collection = root / name
    summary = read_json(collection / "summary.json")
    require(summary["identity"]["dataset_sha256"] == digest(dataset), "graded dataset mismatch")
    require(summary["identity"]["grader_instructions_sha256"] == digest(grading.GRADER_INSTRUCTIONS), "grader protocol drift")
    require(summary["identity"]["schema_sha256"] == digest(grading.judgment_schema()), "grader schema drift")
    require(set(summary["collections"]) == {q["id"] for q in dataset["questions"]}, "missing grading collections")
    for qid, result in summary["collections"].items():
        source = Path(result.get("source_experiment", str(collection / qid))).resolve()
        require(source in {(collection / qid).resolve(), (root / "calibration" / qid).resolve()}, "grading source outside declared collections")
        if source != (collection / qid).resolve():
            require(name == "calibration-final", "only preflight controls may reuse initial calibration")
            old = read_json(root / "dataset-initial.json")
            require(next(q for q in old["questions"] if q["id"] == qid) == next(q for q in dataset["questions"] if q["id"] == qid), "reused contract drift")
        grading.validate_saved_grading(dataset, source, result)
    if name == "evaluation-grading":
        observed = {r["id"]: r for r in runner.load_experiment_runs(root)}
        for qid, result in summary["collections"].items():
            registry = read_json(collection / qid / "grading/private-registry.json")
            for item in registry.values():
                if item["control"]:
                    continue
                run = observed.get(item["run_id"])
                require(run is not None and run["question_id"] == qid and run["answer"] == item["answer"], "graded answer differs from sealed execution")
    require(summary["passed"] == all(all(b["status"] == "complete" for b in result["batches"]) for result in summary["collections"].values()), "grading completeness mismatch")
    return summary


def freeze(root: Path, dataset: dict):
    validate(dataset, root)
    verification = read_json(root / "verification.json")
    preparation_report = Path(__file__).parent / "artifacts/preparation-r3/report/summary.json"
    require(read_json(preparation_report)["harness"]["ready_for_main"], "preparation readiness gate failed")
    require(verification["offline"]["passed"] and verification["runtime"]["passed"], "preflight verification failed")
    require(verification["harness_sha256"] == runner.harness_digest(), "verification harness drift")
    require(verify_grades(root, dataset, "calibration-final")["passed"], "calibration failed")
    require(file_digest(root / "product/codemap-search") == verification["product_binary_sha256"], "verified product binary mismatch")
    preparations = {key: read_json(root / "preparation" / key / "preparation.json") for key in dataset["repositories"]}
    build = read_json(root / "product/build.json")
    require(build["exit_code"] == 0 and file_digest(root / "product/codemap-search") == build["binary_sha256"], "freeze product build mismatch")
    require(digest(runner.source_manifest(root / "product/source/apps/codemap-search")) == build["source_manifest_sha256"], "freeze product source drift")
    for key, preparation in preparations.items():
        require(preparation["repository"] == dataset["repositories"][key], "prepared repository identity mismatch")
        require(digest(runner.source_manifest(root / "repositories" / key)) == preparation["source_manifest_sha256"], "prepared source differs from checkout")
    frozen = {"protocol": PROTOCOL, "dataset_sha256": digest(dataset), "harness_sha256": runner.harness_digest(),
              "product_build": build, "preparations": preparations, "verification": verification,
              "answer_prompt_sha256": digest(ANSWER_PROMPT), "frozen_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "specification_sha256": file_digest(Path(__file__).parent / "data/formal-r1/README.md"),
              "preparation_readiness": {"artifact": str(preparation_report), "sha256": file_digest(preparation_report)},
              "calibration_summary_sha256": file_digest(root / "calibration-final/summary.json")}
    write_json(root / "frozen.json", frozen, exclusive=True)
    archive = root / "harness-at-freeze"
    archive.mkdir()
    for path in Path(__file__).parent.glob("*.py"):
        shutil.copyfile(path, archive / path.name)
    schedule = question_schedule(dataset, "main")
    write_json(root / "schedule.json", schedule, exclusive=True)
    for entry in schedule:
        write_json(root / "runs" / entry["id"] / "run.json", runner.placeholder_run(entry), exclusive=True)
    verify_freeze(root, dataset)


def verify_freeze(root: Path, dataset: dict) -> dict:
    frozen = read_json(root / "frozen.json")
    require(frozen["protocol"] == PROTOCOL and frozen["dataset_sha256"] == digest(dataset), "formal frozen contract drift")
    require(frozen["harness_sha256"] == runner.harness_digest(), "formal frozen harness drift")
    require(file_digest(root / "product/codemap-search") == frozen["product_build"]["binary_sha256"], "formal product drift")
    require(read_json(root / "schedule.json") == question_schedule(dataset, "main"), "formal scheduled denominator drift")
    require(file_digest(root / "calibration-final/summary.json") == frozen["calibration_summary_sha256"], "calibration summary drift")
    for key, preparation in frozen["preparations"].items():
        require(digest(runner.source_manifest(Path(preparation["snapshot"]))) == preparation["source_manifest_sha256"], f"prepared source drift: {key}")
    return frozen


def run(root: Path, dataset: dict):
    with (root / ".run.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        frozen = verify_freeze(root, dataset)
        command(["codex", "--version"])  # No CLI version pin.
        runner.recover_observations(root)
        questions = {q["id"]: q for q in dataset["questions"]}
        def one(entry):
            folder = root / "runs" / entry["id"]
            if read_json(folder / "run.json")["status"] != "scheduled":
                return
            write_json(folder / "run.json", runner.placeholder_run(entry, "running"))
            try:
                q = questions[entry["question_id"]]
                preparation = frozen["preparations"][q["repository_id"]]
                started = time.monotonic()
                runner.copy_source(Path(preparation["snapshot"]), folder / "source")
                require(digest(runner.source_manifest(folder / "source")) == preparation["source_manifest_sha256"], "run clone mismatch")
                copy_seconds = time.monotonic() - started
                settings = {"group": entry["group"], "source": str(folder / "source"), "log": str(folder / "relay.jsonl"),
                            "product_binary": str(root / "product/codemap-search"), "product_home": str(folder / "product-home")}
                execution = runner.execute_codex(folder, ANSWER_PROMPT + "\n" + q["prompt"], settings)
                calls, complete, host_calls, violations = runner.normalize_calls(folder)
                record = {**entry, **execution, "repository_id": q["repository_id"], "calls": calls,
                          "copy_elapsed_seconds": copy_seconds, "host_calls": host_calls, "boundary_violations": violations,
                          "conditions_valid": execution["conditions_valid"] and not violations,
                          "exploration": exploration_metrics(calls, complete=complete, evidence=str(folder / "relay.jsonl"))}
                record["exploration"]["delivered_output_bytes"] = metric(read_json(folder / "delivery.json")["actual_model_output_bytes"], "bytes", evidence=[str(folder / "rollout.jsonl")])
                write_json(folder / "run.json", record)
            except (OSError, ValueError, KeyError, TypeError, AttributeError, subprocess.SubprocessError) as exc:
                write_json(folder / "run.json", runner.placeholder_run(entry, "environment_error", str(exc)))
            finally:
                if read_json(folder / "run.json")["status"] != "running":
                    runner.seal_run(folder)
                shutil.rmtree(folder / "source", ignore_errors=True)
            print(f"{entry['id']}: {read_json(folder / 'run.json')['status']}", flush=True)
        rows = runner.load_experiment_runs(root)
        require(not any(r["status"] == "running" for r in rows), "unfinished process requires explicit recovery; do not repeat")
        schedule = read_json(root / "schedule.json")
        def pair(entries):
            for entry in entries:
                one(entry)
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
            list(executor.map(pair, [schedule[i:i + 2] for i in range(0, len(schedule), 2)]))


def intervals(rows: list[dict]) -> dict:
    repos = collections.defaultdict(lambda: collections.defaultdict(list))
    for row in rows:
        repos[row["repository_id"]][row["question_id"]].append(row)
    output = {}
    for section, key in [("quality", "full_correct"), ("cost", "total_tokens"), ("cost", "exploration_calls"), ("cost", "elapsed_seconds")]:
        if any(r[section][key]["value"] is None for r in rows):
            output[key] = {"low": None, "high": None, "reason": "incomplete paired metric"}
            continue
        prepared = [[tuple(statistics.mean(r[section][key]["value"] for r in qrows if r["group"] == g) for g in ["A", "B"])
                     for _, qrows in sorted(questions.items())] for _, questions in sorted(repos.items())]
        rng = random.Random(SPEC["seed"])
        estimates = []
        for _ in range(SPEC["bootstrap_samples"]):
            selected = [pair for _ in prepared for questions in [rng.choice(prepared)] for pair in [rng.choice(questions) for _ in questions]]
            a, b = [statistics.mean(pair[i] for pair in selected) for i in [0, 1]]
            estimates.append(100 * (b - a) if section == "quality" else (100 * (b / a - 1) if a else None))
        output[key] = {"low": percentile(estimates, .025), "high": percentile(estimates, .975), "reason": None} if None not in estimates else {"low": None, "high": None, "reason": "zero baseline"}
    return {"samples": SPEC["bootstrap_samples"], "seed": SPEC["seed"], "unit": PROTOCOL["uncertainty"], "intervals": output}


def report(root: Path, dataset: dict):
    frozen = verify_freeze(root, dataset)
    grades = verify_grades(root, dataset, "evaluation-grading")
    judgments = {key: value for result in grades["collections"].values() for key, value in result["judgments"].items()}
    questions = {q["id"]: q for q in dataset["questions"]}
    runs = runner.load_experiment_runs(root)
    require(len(runs) == 120 and all(r["status"] not in {"scheduled", "running"} for r in runs), "formal execution incomplete")
    rows = []
    for observation in runs:
        q = questions[observation["question_id"]]
        observation["grading_artifact"] = str(root / "evaluation-grading" / q["id"] / "grading/judgments.json")
        row = normalize_run(q, observation, judgments.get(observation["id"]))
        rows.append({**row, "repository_id": q["repository_id"], "tags": q["tags"]})
    comparison = compare(rows, 2, with_bootstrap=False)
    comparison["bootstrap"] = intervals(rows)
    by_repo = {key: compare([r for r in rows if r["repository_id"] == key], 2, with_bootstrap=False) for key in dataset["repositories"]}
    by_tag = {tag: compare([r for r in rows if tag in r["tags"]], 2, with_bootstrap=False) for tag in sorted({tag for r in rows for tag in r["tags"]})}
    overhead = {}
    for name, paths in [("runtime", list((root / "runtime-probe").glob("*/execution.json"))),
                        ("calibration", list((root / "calibration").glob("*/grading/batch-*/execution.json")) + list((root / "calibration-final").glob("*/grading/batch-*/execution.json"))),
                        ("grading", list((root / "evaluation-grading").glob("*/grading/batch-*/execution.json")))]:
        executions = [read_json(p) for p in paths]
        tokens = [e["usage"]["total_tokens"]["value"] for e in executions]
        overhead[name] = {"executions": len(executions), "total_tokens": sum(tokens) if None not in tokens else None,
                          "observed_token_sum": sum(v for v in tokens if v is not None), "cost_complete_count": sum(v is not None for v in tokens),
                          "elapsed_seconds_sum": sum(e["elapsed_seconds"] for e in executions), "evidence": [str(p) for p in paths]}
    summary = {"frozen": frozen, "scheduled": 120, "ended": len(runs), "statuses": dict(collections.Counter(r["status"] for r in runs)),
               "grading_complete": grades["passed"], "comparison": comparison, "by_repository": by_repo, "by_tag": by_tag,
               "overhead": overhead, "rows": rows, "aggregation_sha256": runner.harness_digest()}
    summary["by_language"] = {language: compare([r for r in rows if r["language"] == language], 2, with_bootstrap=False) for language in sorted({r["language"] for r in rows})}
    summary["by_difficulty"] = {difficulty: compare([r for r in rows if r["difficulty"] == difficulty], 2, with_bootstrap=False) for difficulty in PROTOCOL["difficulty_counts"]}
    summary["validity"] = {group: {"scheduled": 60, "conditions_valid": sum(r["conditions_valid"] for r in rows if r["group"] == group),
        "whole_cost_complete": sum(r["usage"]["complete"] for r in rows if r["group"] == group),
        "quality_valid": sum(r["quality"]["full_correct"]["value"] is not None for r in rows if r["group"] == group),
        "correct": sum(r["quality"]["full_correct"]["value"] == 1 for r in rows if r["group"] == group)} for group in ["A", "B"]}
    output = root / "report"
    output.mkdir(exist_ok=True)
    write_json(output / "summary.json", summary)
    def display(value):
        return "결측" if value is None else f"{value:,.2f}"
    lines = ["# 범용 MCP 정식 벤치마크", "", "6개 저장소 × 5문제 × A/B × 2회. 기존 Grafana 준비 실험과 합산하지 않는다.", "",
             f"실행 {len(runs)}/120, 채점 전체 유효: {grades['passed']}. 제품 커밋 `{frozen['product_build']['repository_commit']}`.", "",
             "| 항목 | A | B | B−A 또는 변화율 |", "| --- | ---: | ---: | ---: |"]
    labels = {"full_correct": "완전 정답률(0–1)", "total_tokens": "평균 전체 토큰", "exploration_calls": "평균 탐색 호출", "elapsed_seconds": "평균 시간(초)"}
    for key in ["full_correct", "total_tokens", "exploration_calls", "elapsed_seconds"]:
        a, b = [comparison["groups"][g]["metrics"][key]["mean"]["value"] for g in ["A", "B"]]
        lines.append(f"| {labels[key]} | {display(a)} | {display(b)} | {display(comparison['differences'][key]['value'])} |")
    lines += ["", "95% 구간은 저장소와 문제를 순서대로 재표집하며 A/B와 반복을 함께 유지한다. 6개 저장소는 확률 표본이 아니므로 모든 MCP 사용 환경으로 일반화할 수 없다.", "", "```json", canonical(comparison["bootstrap"]), "```", "",
              "| 저장소 | A 정답 평균 | B 정답 평균 | 정확도 차이(pp) | 토큰 변화(%) | 호출 변화(%) | 시간 변화(%) |", "| --- | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for key, result in by_repo.items():
        values = [result["groups"][g]["metrics"]["full_correct"]["mean"]["value"] for g in ["A", "B"]]
        values += [result["differences"][k]["value"] for k in ["full_correct", "total_tokens", "exploration_calls", "elapsed_seconds"]]
        lines.append(f"| {key} | " + " | ".join(display(v) for v in values) + " |")
    lines += ["", "실행/채점 실패와 제한 종료는 삭제하거나 재실행하지 않는다. 불완전한 전체 비용은 null로 유지하며, 관측된 일부 비용은 summary.json에 별도로 기록한다.", "", "## 평가 외 비용", "", "| 구분 | 실행 수 | 전체 토큰 | 시간 합(초) |", "| --- | ---: | ---: | ---: |"]
    for name, values in overhead.items():
        lines.append(f"| {name} | {values['executions']} | {display(values['total_tokens'])} | {display(values['elapsed_seconds_sum'])} |")
    lines += ["", "사전 채점 비용에는 표본 교체 전에 실행한 client-go 5문제의 대조 답안도 포함한다. 변경되지 않은 25문제의 사전 채점은 재사용했으며 중복 실행하지 않았다.",
              "", "생성·자원 파일과 감점 경로가 정답인 하위 집단, 언어별·난도별 집계 및 각 지표의 결측 사유·원자료 위치는 summary.json에 보존한다.",
              "", "## 문제별 실제 답변과 판정", ""]
    for q in dataset["questions"]:
        lines += [f"### {q['id']}", "", q["prompt"], ""]
        for row in rows:
            if row["question_id"] != q["id"]:
                continue
            lines += [f"#### {row['group']} / {row['repeat']}회 — {row['category']} ({row['status']})", "",
                      f"토큰 {display(row['cost']['total_tokens']['value'])}, 호출 {display(row['cost']['exploration_calls']['value'])}, 시간 {display(row['cost']['elapsed_seconds']['value'])}초.", "",
                      row["answer"] or "최종 답변 없음.", "", "판정 근거:", "", "```json", __import__("json").dumps(row["judgment"], ensure_ascii=False, indent=2), "```", ""]
    (output / "report.md").write_text("\n".join(lines), encoding="utf-8")
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["validate", "prepare", "calibrate", "freeze", "run", "grade", "report"])
    parser.add_argument("--experiment", type=Path, required=True)
    args = parser.parse_args()
    root = args.experiment.resolve()
    dataset = read_json(root / "dataset.json")
    if args.action == "validate":
        validate(dataset, root)
    elif args.action == "prepare":
        prepare(root, dataset)
    elif args.action == "calibrate":
        require(grade_collection(root, dataset, calibration=True)["passed"], "calibration incomplete; formal run remains blocked")
    elif args.action == "freeze":
        freeze(root, dataset)
    elif args.action == "run":
        run(root, dataset)
    elif args.action == "grade":
        grade_collection(root, dataset)
    elif args.action == "report":
        report(root, dataset)


if __name__ == "__main__":
    main()
