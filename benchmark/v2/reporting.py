"""Five report tables, raw links, and distinct construction/product conclusions."""
from __future__ import annotations

import collections
from pathlib import Path

from .core import DIFFICULTIES, LANGUAGES, MODEL_TERMINALS, SPEC, digest, metric, ratio, read_json, require, write_json
from .metrics import COST_KEYS, QUALITY_KEYS, compare, normalize_run, verdict
from .runner import load_experiment_runs, verify_frozen


LABELS = {"full_correct": "완전 정답률", "fact_ratio": "핵심 사실 충족률", "evidence_ratio": "핵심 근거 충족률",
          "major_error": "중대 오류율", "no_answer": "최종 답변 없음 비율", "total_tokens": "총토큰",
          "input_tokens": "입력 토큰", "output_tokens": "출력 토큰", "cached_input_tokens": "캐시 입력 토큰",
          "exploration_calls": "탐색 호출", "host_calls": "호스트 호출", "model_responses": "모델 응답", "elapsed_seconds": "실행 시간(초)"}


def number(value, digits=2):
    return "null" if value is None else f"{value:.{digits}f}"


def table(headers, rows):
    def escape(value):
        return str(value).replace("|", "\\|").replace("\n", " ")
    return "\n".join(["| " + " | ".join(map(escape, headers)) + " |",
                       "| " + " | ".join("---" for _ in headers) + " |"] +
                      ["| " + " | ".join(map(escape, row)) + " |" for row in rows])


def group_mean(comparison, group, key):
    return comparison["groups"][group]["metrics"][key]["mean"]["value"]


def create_summary(dataset: dict, runs: list[dict], grading: dict, phase: str, *, frozen: dict) -> dict:
    questions = {q["id"]: q for q in dataset["questions"]}
    repeats = SPEC["phases"][phase]["repeats"]
    normalized = [normalize_run(questions[r["question_id"]], r, grading.get("judgments", {}).get(r["id"])) for r in runs]
    overall = compare(normalized, repeats)
    strata = {}
    for name, key, values in [("difficulty", "difficulty", DIFFICULTIES), ("language", "language", LANGUAGES)]:
        strata[name] = {v: compare([r for r in normalized if r[key] == v], repeats) for v in values}
    question_results = {}
    regressions = []
    extra_cost = []
    known_cost_questions = 0
    for question_id in sorted({r["question_id"] for r in normalized}):
        rows = [r for r in normalized if r["question_id"] == question_id]
        comparison = compare(rows, repeats, with_bootstrap=False)
        question_results[question_id] = {"source": questions[question_id]["source"], "comparison": comparison,
                                       "runs": [r["id"] for r in rows]}
        av, bv = [group_mean(comparison, g, "full_correct") for g in ["A", "B"]]
        if av is not None and bv is not None and bv < av:
            regressions.append(question_id)
        at, bt = [group_mean(comparison, g, "total_tokens") for g in ["A", "B"]]
        if at is not None and bt is not None:
            known_cost_questions += 1
        if at is not None and bt is not None and bt > at:
            components = {}
            for key in ["input_tokens", "output_tokens", "model_responses", "host_calls", "exploration_calls"]:
                a, b = [group_mean(comparison, g, key) for g in ["A", "B"]]
                components[key] = {"A": a, "B": b, "absolute_difference": b - a if a is not None and b is not None else None}
            initial_calls = sum(r["exploration"]["distribution"].get("initial_instructions", 0) for r in rows if r["group"] == "B")
            observations = [f"B 초기 안내 {initial_calls}회"] if initial_calls else []
            responses = components["model_responses"]
            if responses["absolute_difference"] is not None and responses["absolute_difference"] > 0:
                observations.append(f"모델 응답 평균 {number(responses['A'])}→{number(responses['B'])}회")
            inputs = components["input_tokens"]["absolute_difference"]
            if inputs is not None:
                observations.append(f"입력 토큰 {inputs:+.0f}")
            outputs = components["output_tokens"]["absolute_difference"]
            if outputs is not None:
                observations.append(f"출력 토큰 {outputs:+.0f}")
            extra_cost.append({"question_id": question_id, "additional_tokens_per_run": bt - at,
                               "positive_additional_share": None, "components": components,
                               "cause": "; ".join(observations) + ". 관측된 구성 차이이며 인과 원인은 미확정.",
                               "records": [r["artifact"] for r in rows],
                               "observations": {r["id"]: {"category": r["category"],
                                  "distribution": r["exploration"]["distribution"],
                                  "identical_request_repeat_ratio": r["exploration"]["identical_request_repeat_ratio"],
                                  "duplicate_read_ratio": r["exploration"]["duplicate_read_ratio"],
                                  "empty_search_ratio": r["exploration"]["empty_search_ratio"],
                                  "tool_error_ratio": r["exploration"]["tool_error_ratio"],
                                  "delivered_output_bytes": r["exploration"]["delivered_output_bytes"]} for r in rows}})
    extra_cost.sort(key=lambda row: (-row["additional_tokens_per_run"], row["question_id"]))
    positive_total = sum(r["additional_tokens_per_run"] for r in extra_cost)
    for row in extra_cost:
        row["observed_positive_additional_share"] = row["additional_tokens_per_run"] / positive_total
        row["positive_additional_share"] = row["observed_positive_additional_share"] if known_cost_questions == len(question_results) else None
        row["share_missing_reason"] = None if row["positive_additional_share"] is not None else "other questions have unknown total cost"
        row["metrics"] = {
            "additional_tokens_per_run": metric(row["additional_tokens_per_run"], "tokens/run", evidence=row["records"]),
            "positive_additional_share": metric(row["positive_additional_share"], "ratio", row["additional_tokens_per_run"],
                positive_total if row["positive_additional_share"] is not None else None, row["share_missing_reason"], row["records"]),
            "observed_positive_additional_share": ratio(row["additional_tokens_per_run"], positive_total, evidence=row["records"]),
        }
    expected_per_group = 3 * SPEC["phases"][phase]["per_difficulty"] * repeats
    per_group = {}
    for group in ["A", "B"]:
        selected = [r for r in normalized if r["group"] == group]
        statuses = collections.Counter(r["status"] for r in selected)
        per_group[group] = {
            "scheduled": expected_per_group, "recorded": len(selected),
            "completed": statuses["completed"],
            "limited": sum(statuses[k] for k in ["timeout", "call_limit", "token_limit"]),
            "environment_or_unfinished": sum(n for status, n in statuses.items() if status not in MODEL_TERMINALS),
            "quality_valid": sum(r["quality"]["full_correct"]["value"] is not None for r in selected),
            "tokens_valid": sum(r["cost"]["total_tokens"]["value"] is not None for r in selected),
            "calls_valid": sum(r["cost"]["exploration_calls"]["value"] is not None for r in selected),
            "conditions_valid": sum(r["conditions_valid"] for r in selected), "statuses": dict(statuses)}
    batch_complete = bool(grading.get("batches")) and all(b["status"] == "complete" for b in grading["batches"])
    complete = all(all(counts[k] == expected_per_group for k in ["recorded", "quality_valid", "tokens_valid", "calls_valid", "conditions_valid"])
                   and counts["environment_or_unfinished"] == 0 for counts in per_group.values())
    harness = {"per_group": per_group, "grading_controls_passed": batch_complete,
               "grading_batches": grading.get("batches", []),
               "code_boundary_violations": [v for r in runs for v in r.get("boundary_violations", [])],
               "reaggregation_sha256": digest(normalized), "aggregation_version": SPEC["version"],
               "ready_for_main": phase == "preparation" and complete and batch_complete,
               "construction": "사전 검증 완료" if complete and batch_complete else "구현됨; 실제 사전 검증 미완료"}
    from .runner import harness_digest
    harness["aggregation_sha256"] = harness_digest()
    harness["late_tool_results"] = [item for run in runs for item in run.get("late_tool_results", [])]
    harness["metrics"] = {group: {
        **{key: metric(value, "runs", evidence=[r["artifact"] for r in normalized if r["group"] == group])
           for key, value in counts.items() if key != "statuses"},
        **{key + "_coverage": ratio(counts[key], expected_per_group, evidence=[r["artifact"] for r in normalized if r["group"] == group])
           for key in ["quality_valid", "tokens_valid", "calls_valid", "conditions_valid"]},
    } for group, counts in per_group.items()}
    return {"spec": SPEC, "phase": phase, "overall": overall, "strata": strata, "questions": question_results,
            "observed_regressions": {"count": len(regressions), "question_ids": regressions,
                                     "analyzable_questions": sum(q["comparison"]["differences"]["full_correct"]["value"] is not None for q in question_results.values())},
            "cost_increases": extra_cost, "harness": harness,
            "cost_increase_coverage": {"known_questions": known_cost_questions, "scheduled_questions": len(question_results)},
            "verdict": verdict(overall, complete and batch_complete and not harness["code_boundary_violations"]),
            "preparation_cost": frozen["preparation"], "normalized": normalized}


def render_report(summary: dict) -> str:
    overall = summary["overall"]
    lines = ["# codemap-search V2 평가 보고서", "", summary["verdict"]["decision"], "",
             f"하네스: {summary['harness']['construction']}. 단계: `{summary['phase']}`.", "",
             "정확도와 비용은 별도로 판정한다. `null`은 결측·분모 0·불완전 자료이며 0이 아니다. "
             "신뢰구간은 난이도별 문제를 10,000회 재표집하고 A/B와 반복을 함께 유지한 95% 구간이다.", "",
             "## 1. 전체 비교", ""]
    rows = []
    for key in QUALITY_KEYS + COST_KEYS:
        values = [group_mean(overall, g, key) for g in ["A", "B"]]
        if key in QUALITY_KEYS:
            values = [v * 100 if v is not None else None for v in values]
        interval = overall["bootstrap"]["intervals"].get(key, {})
        rows.append([LABELS[key] + (" (%)" if key in QUALITY_KEYS else ""), *[number(v) for v in values], number(overall["differences"][key]["value"]),
                     f"{number(interval.get('low'))} ~ {number(interval.get('high'))}" if interval else "—"])
    lines += [table(["지표", "A 평균", "B 평균", "차이(pp / %)", "차이 95% 구간"], rows), "",
              "토큰·탐색 호출의 평균은 문제별 반복 평균을 동일하게 가중한다. 중앙값과 p95는 예정 실행의 관측값 분포다.", ""]
    rows = []
    for group in ["A", "B"]:
        for key in ["total_tokens", "exploration_calls"]:
            distribution = overall["groups"][group]["metrics"][key]
            rows.append([group, LABELS[key], *[number(distribution[k]["value"]) for k in ["mean", "median", "p95", "sum"]],
                         f"{distribution['observed_count']}/{distribution['scheduled_count']}"])
    lines += [table(["집단", "지표", "평균", "중앙값", "p95", "합계", "유효/예정"], rows), "",
              "대응쌍: " + ", ".join(f"{label} {overall['paired_outcomes'][key]}" for key, label in
                  [("A_only", "A만 정답"), ("B_only", "B만 정답"), ("both", "둘 다 정답"), ("neither", "둘 다 미달"), ("indeterminate", "판정 불가")]) + ". 양쪽 모두 정답인 보조 비용 비교에는 "
              f"{overall['both_correct_cost']['pairs']}쌍 / {overall['both_correct_cost']['questions']}문제가 포함된다.", "",
              "완전 정답 1건당 관측 총토큰: " + ", ".join(f"{g} {number(overall['groups'][g]['tokens_per_correct']['value'])}" for g in ["A", "B"]) +
              ". 재시도의 기대 비용을 뜻하지 않는다.", "", "## 2. 난이도·언어 비교", ""]
    rows = []
    for category, values in summary["strata"].items():
        for name, comparison in values.items():
            for group in ["A", "B"]:
                s = comparison["groups"][group]
                category_label = {"difficulty": "난이도", "language": "언어"}[category]
                stratum_label = {"simple": "간단", "medium": "중간", "complex": "복잡", "typescript": "TypeScript/TSX", "go": "Go"}[name]
                quality_values = [group_mean(comparison, group, key) for key in ["full_correct", "fact_ratio", "evidence_ratio", "major_error"]]
                rows.append([f"{category_label}/{stratum_label}", group, f"{s['questions']} / {s['scheduled_runs']}",
                             *[number(100 * value if value is not None else None) for value in quality_values],
                             *[number(group_mean(comparison, group, key)) for key in ["total_tokens", "exploration_calls"]]])
    lines += [table(["층", "집단", "문제/실행", "정답률 %", "사실 %", "근거 %", "중대 오류 %", "토큰", "호출"], rows), "",
              "층별 차이와 신뢰구간은 [기계 판독 보고서](summary.json)의 `strata`에 포함한다.", "",
              "## 3. 문제별 비교", ""]
    rows = []
    by_id = {r["id"]: r for r in summary["normalized"]}
    repeats = SPEC["phases"][summary["phase"]]["repeats"]
    for qid, question in summary["questions"].items():
        comp = question["comparison"]
        counts = [comp["groups"][g]["metrics"]["full_correct"]["sum"]["value"] for g in ["A", "B"]]
        issues = []
        for run_id in question["runs"]:
            run = by_id[run_id]
            j = run["judgment"]
            if j and j["status"] == "judged":
                missing = [f["fact_id"] for f in j["facts"] if not f["correct"] or not f["supported"]]
                issues.append(f"{run['group']}{run['repeat']}: {','.join(missing) or '충족'}; 중대 오류 {len(j['major_errors'])}")
            else:
                issues.append(f"{run['group']}{run['repeat']}: 판정 불가")
        rows.append([qid, f"[#{question['source']['pr']}]({question['source']['url']})",
                     *[f"{number(c, 0)}/{repeats}" for c in counts],
                     number(comp["differences"]["total_tokens"]["value"]), number(comp["differences"]["exploration_calls"]["value"]), "; ".join(issues)])
    lines += [table(["문제", "PR", "A 정답", "B 정답", "토큰 변화 %", "호출 변화 %", "누락·오류"], rows), "",
              f"문제별 관측 퇴행: {summary['observed_regressions']['count']}개. "
              f"분석 가능 문제: {summary['observed_regressions']['analyzable_questions']}개.", "",
              "## 4. 비용 증가와 실패 원인", ""]
    rows = [[r["question_id"], number(r["additional_tokens_per_run"]), number(100 * r["positive_additional_share"] if r["positive_additional_share"] is not None else None),
             number(100 * r["observed_positive_additional_share"]), r["cause"],
             " · ".join(f"[기록 {i + 1}]({path})" for i, path in enumerate(r["records"]))] for r in summary["cost_increases"]]
    lines += [table(["문제", "실행당 추가 토큰", "전체 양의 추가량 비중 %", "관측 부분 내 비중 %", "관측·원인 판단", "원자료"], rows) if rows else "완전한 토큰 자료에서 확인된 양의 추가 비용이 없다.", "",
              f"비용 비교 가능 문제: {summary['cost_increase_coverage']['known_questions']}/{summary['cost_increase_coverage']['scheduled_questions']}. "
              "비용이 미확정인 문제가 있으면 전체 추가량 비중은 계산하지 않는다.", "",
              "반복 요청·중복 읽기·빈 검색·출력량·오류와 결과의 연관성은 `cost_increases.observations`에 기록한다. "
              "이 기록만으로 인과관계를 단정하지 않는다. 원인 확정 시 필요한 근거 추가 확보, 큰 출력·문맥 재입력, 반복 탐색, "
              "범위 이동, 초기 안내·호스트, 오류 복구·제한 종료를 구분해야 한다.", "",
              "## 5. 하네스 유효성", ""]
    rows = [[g, *[c[k] for k in ["scheduled", "recorded", "completed", "limited", "environment_or_unfinished", "quality_valid", "tokens_valid", "calls_valid", "conditions_valid"]]]
            for g, c in summary["harness"]["per_group"].items()]
    lines += [table(["집단", "예정", "상태 기록", "완료", "제한", "환경·미완료", "품질 유효", "토큰 유효", "호출 유효", "조건 일치"], rows), "",
              f"채점 대조 사례: {'통과' if summary['harness']['grading_controls_passed'] else '미완료'}. "
              f"코드 경계 위반 기록: {len(summary['harness']['code_boundary_violations'])}건.", "",
              f"Codex 종료 뒤 도착한 기존 도구 결과: {len(summary['harness'].get('late_tool_results', []))}건. "
              "초기 로그 접두사와 종료 시각을 검증해 별도 고정했으며 모델 전달 근거로 세지 않는다.", "",
              "준비 색인 비용과 Astra 채점 사용량은 제품 실행 비용에서 분리한다. "
              "[전체 지표와 유효성](summary.json), [실행별 정규화 지표](normalized.json), "
              "[고정 명세](../frozen.json), [예정 실행](../schedule.json), [원자료](../runs/), [채점 자료](../grading/)를 함께 확인한다.", "",
              summary["verdict"]["scope"] + ".", ""]
    preparation = summary.get("preparation_cost", {})
    if "index_elapsed_seconds" in preparation:
        lines += [f"별도 색인 준비: {number(preparation['index_elapsed_seconds'])}초, {preparation['index_bytes']:,}바이트.", ""]
    grading_cost = summary.get("grading_cost", [])
    if grading_cost:
        token_values = [item["usage"]["total_tokens"]["value"] for item in grading_cost]
        total = sum(token_values) if None not in token_values else None
        lines += [f"별도 Astra 채점: {len(grading_cost)}개 실행, 총토큰 {number(total, 0)}. "
                  "불완전한 채점 실행의 토큰이 있으면 전체 합계를 확정하지 않는다.", ""]
    if summary["verdict"]["accuracy_observation"]:
        lines += [summary["verdict"]["accuracy_observation"], ""]
    return "\n".join(lines)


def canonical_for_report(value):
    from .core import canonical
    return canonical(value)


def report(dataset: dict, experiment: Path) -> dict:
    frozen = verify_frozen(dataset, experiment)
    runs = load_experiment_runs(experiment)
    grading_path = experiment / "grading" / "judgments.json"
    grading = read_json(grading_path) if grading_path.exists() else {}
    if grading:
        from .grading import validate_saved_grading
        validate_saved_grading(dataset, experiment, grading)
    summary = create_summary(dataset, runs, grading, frozen["phase"], frozen=frozen)
    # Re-run mechanical aggregation once to certify deterministic output.
    repeated = create_summary(dataset, runs, grading, frozen["phase"], frozen=frozen)
    require(digest(summary) == digest(repeated), "non-reproducible reaggregation")
    output = experiment / "report"
    write_json(output / "normalized.json", summary["normalized"])
    costs = []
    for path in sorted((experiment / "grading").glob("batch-*/execution.json")):
        execution = read_json(path)
        costs.append({"path": str(path), "status": execution["status"], "usage": execution["usage"], "elapsed_seconds": execution["elapsed_seconds"]})
    summary["grading_cost"] = costs
    summary["harness"]["reaggregation_verified"] = True
    write_json(output / "summary.json", summary)
    (output / "report.md").write_text(render_report(summary), encoding="utf-8")
    return summary
