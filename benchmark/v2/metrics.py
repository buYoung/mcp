"""Mechanical metrics, independent validity, stratified paired bootstrap."""
from __future__ import annotations

import collections
import random
import statistics

from .core import (DIFFICULTIES, MODEL_TERMINALS, SPEC, canonical, change, metric,
                   percentile, ratio)
from .grading import classify, no_answer_judgment


def usage_metrics(records: list[dict], *, terminal_complete: bool, evidence: str) -> dict:
    unique = {}
    errors = []
    cumulative = None
    for row in records:
        if row.get("type") == "token_usage_record":
            payload = row.get("payload", {})
            response_id = payload.get("response_id")
            usage = payload.get("usage")
            if not response_id or not isinstance(usage, dict):
                errors.append("missing response ID/usage")
                continue
            keys = ("input_tokens", "output_tokens", "cached_input_tokens")
            if any(type(usage.get(k)) is not int or usage[k] < 0 for k in keys):
                errors.append("missing/invalid token component")
                continue
            if usage["cached_input_tokens"] > usage["input_tokens"]:
                errors.append("cache input exceeds input")
            if usage.get("total_tokens", usage["input_tokens"] + usage["output_tokens"]) != usage["input_tokens"] + usage["output_tokens"]:
                errors.append("inconsistent total tokens")
            if response_id in unique and unique[response_id] != usage:
                errors.append("conflicting duplicate response usage")
            unique[response_id] = usage
        if row.get("type") == "event_msg" and row.get("payload", {}).get("type") == "token_count":
            total = (row["payload"].get("info") or {}).get("total_token_usage")
            if total is not None:
                cumulative = total
    sums = {k: sum(u[k] for u in unique.values()) for k in ("input_tokens", "output_tokens", "cached_input_tokens")}
    sums["total_tokens"] = sums["input_tokens"] + sums["output_tokens"]
    if cumulative is None:
        errors.append("missing cumulative reconciliation event")
    elif any(cumulative.get(k) != value for k, value in sums.items()):
        errors.append("request totals disagree with cumulative usage")
    if not unique:
        errors.append("no request usage records")
    if not terminal_complete:
        errors.append("in-flight/unfinished model usage cannot be certified")
    result = {"complete": not errors, "errors": sorted(set(errors)), "observed": sums,
              "response_ids": sorted(unique)}
    for key, value in sums.items():
        result[key] = metric(value if not errors else None, "tokens", reason="; ".join(sorted(set(errors))) or None,
                             evidence=[evidence])
    result["model_responses"] = metric(len(unique) if not errors else None, "responses",
                                        reason="; ".join(sorted(set(errors))) or None, evidence=[evidence])
    result["cached_input_ratio"] = ratio(result["cached_input_tokens"]["value"], result["input_tokens"]["value"], evidence=[evidence])
    return result


def exploration_metrics(calls: list[dict], *, complete: bool, evidence: str) -> dict:
    reasons = [] if complete else ["request/result recording incomplete"]
    ids = [c["id"] for c in calls]
    if len(ids) != len(set(ids)):
        reasons.append("duplicate exploration request ID")
    if any(c.get("status") not in {"success", "error", "cancelled"} for c in calls):
        reasons.append("unclosed exploration request")
    distribution = collections.Counter(c["tool"] for c in calls)
    fingerprints = collections.Counter(canonical({"tool": c["tool"], "arguments": c["arguments"],
                                                  "scope": c.get("effective_scope") if c["tool"] == "search"
                                                  and c.get("effective_scope") not in {None, "", ".", "all", "전체"} else None}) for c in calls)
    search_arguments = collections.Counter(canonical(c["arguments"]) for c in calls if c["tool"] == "search")
    # The relay records the overview argument, while the product resolves aliases
    # to a workspace root. Do not treat different file/folder views as proven scope changes.
    ambiguous_repetition_scope = any(c["tool"] == "search" and search_arguments[canonical(c["arguments"])] > 1
                                     and c.get("effective_scope") not in {None, "", ".", "all", "전체"} for c in calls)
    read_rows = [row for c in calls if c["tool"] == "read" for row in (c.get("delivered_lines") or [])]
    unique_rows = {(row["path"], row["line"]) for row in read_rows}
    searches = [c for c in calls if c["tool"] in {"rg", "grep", "search"} and c.get("status") == "success"]
    known_searches = [c for c in searches if c.get("result_count") is not None]
    raw_bytes = [c.get("raw_bytes") for c in calls]
    delivered_bytes = [c.get("delivered_bytes") for c in calls]
    result = {"complete": not reasons, "errors": reasons, "distribution": dict(distribution)}
    result["tool_distribution_metrics"] = {tool: metric(count, "calls", evidence=[evidence]) for tool, count in distribution.items()}
    def scalar(name, value, unit, reason=None):
        result[name] = metric(value if not reasons and not reason else None, unit,
                              reason=reason or "; ".join(reasons) or None, evidence=[evidence])
    scalar("exploration_calls", len(calls), "calls")
    scalar("unique_read_files", len({r["path"] for r in read_rows}), "files",
           "unrecognized read output" if any(c["tool"] == "read" and c.get("status") == "success"
                and c.get("delivered_lines") is None for c in calls) else None)
    result["read_zero"] = distribution["read"] == 0
    result["search_read_ratio"] = ratio(sum(distribution[t] for t in ["rg", "grep", "search"]), distribution["read"], evidence=[evidence])
    result["identical_request_repeat_ratio"] = (metric(None, "ratio", denominator=len(calls),
        reason="product-resolved workspace scope not observable for repeated search", evidence=[evidence]) if ambiguous_repetition_scope else
        ratio(sum(n - 1 for n in fingerprints.values()), len(calls), evidence=[evidence]))
    read_unknown = any(c["tool"] == "read" and c.get("status") == "success" and c.get("delivered_lines") is None for c in calls)
    result["duplicate_read_ratio"] = ratio(None if read_unknown else len(read_rows) - len(unique_rows),
                                            None if read_unknown else len(read_rows), evidence=[evidence])
    result["empty_search_ratio"] = ratio(sum(c["result_count"] == 0 for c in known_searches) if len(known_searches) == len(searches) else None,
                                          len(searches), evidence=[evidence])
    result["search_result_count_coverage"] = ratio(len(known_searches), len(searches), evidence=[evidence])
    result["tool_error_ratio"] = ratio(sum(c.get("status") == "error" for c in calls), len(calls), evidence=[evidence])
    for origin in ["product", "host"]:
        known = all(c.get(f"{origin}_truncated") is not None for c in calls)
        result[f"{origin}_truncation_ratio"] = ratio(sum(c.get(f"{origin}_truncated") is True for c in calls) if known else None,
                                                      len(calls), evidence=[evidence])
    scalar("raw_output_bytes", sum(v or 0 for v in raw_bytes), "bytes", "missing raw output" if None in raw_bytes else None)
    scalar("delivered_output_bytes", sum(v or 0 for v in delivered_bytes), "bytes", "missing delivered output" if None in delivered_bytes else None)
    if reasons:
        for value in result.values():
            if isinstance(value, dict) and "validity" in value:
                value.update(value=None, validity="unavailable", missing_reason="; ".join(reasons))
    return result


def evidence_exposure(question: dict, calls: list[dict]) -> dict:
    """Only exact, delivered source lines prove exposure; unknown formats stay unknown."""
    evidence = {e["id"]: e for e in question["evidence"]}
    seen = {}
    covered = set()
    first = None
    known = sum(c.get("status") == "cancelled" and c.get("delivered_lines") == [] for c in calls)
    observations = collections.defaultdict(list)
    for call in calls:
        if call.get("delivered_at_seconds") is not None:
            observations[call.get("observation_step")].append(call)
    for step in sorted(observations, key=lambda key: min(c["delivered_at_seconds"] for c in observations[key])):
        for call in observations[step]:
            if call.get("delivered_lines") is not None:
                known += 1
                for line in call["delivered_lines"]:
                    seen[(line["path"], line["line"])] = line["text"]
        evidence_ids = {key for key, e in evidence.items() if all(
            seen.get((e["path"], e["start_line"] + i)) == text for i, text in enumerate(e["text"].splitlines()))}
        for fact in question["facts"]:
            if any(set(group) <= evidence_ids for group in fact["evidence_sets"]):
                covered.add(fact["id"])
        if covered and first is None:
            first = {"step": step, "elapsed_seconds": max(c["delivered_at_seconds"] for c in observations[step])}
    # An unknown/partial format may contain an unrecognized alternative. Do not call it absence.
    unknown = known != len(calls)
    complete = len(covered) == len(question["facts"])
    reason = "unrecognized delivered source/alternative evidence" if unknown and not complete else None
    return {"covered_fact_ids": sorted(covered), "first_required_evidence": first,
            "first_required_evidence_step": metric(first["step"] if first else None, "observation_step",
                                                     reason=None if first else (reason or "no complete required evidence observed")),
            "first_required_evidence_seconds": metric(first["elapsed_seconds"] if first else None, "seconds",
                                                        reason=(None if first["elapsed_seconds"] is not None else "missing delivery timestamp") if first else (reason or "no complete required evidence observed")),
            "analysis_coverage": ratio(known, len(calls)),
            "required_evidence_ratio": metric(None if reason else len(covered) / len(question["facts"]), "ratio",
                                               len(covered), len(question["facts"]), reason),
            "all_required_evidence": metric(None if reason else int(complete), "ratio", reason=reason)}


def normalize_run(question: dict, run: dict, judgment: dict | None) -> dict:
    from .grading import validate_judgment
    quality_reason = None
    if run["status"] not in MODEL_TERMINALS:
        quality_reason = "environment/instrumentation run failure"
    elif not run.get("conditions_valid", False):
        quality_reason = "execution conditions not verified"
    elif not run["answer"].strip():
        judgment = no_answer_judgment(question, run["id"])
    elif judgment:
        try:
            validate_judgment(question, judgment, judgment["answer_id"], run["answer"])
        except (ValueError, KeyError, TypeError) as exc:
            quality_reason = f"invalid judgment: {exc}"
    if judgment is None or judgment.get("status") == "indeterminate":
        quality_reason = quality_reason or "missing/indeterminate judgment"
    category = "indeterminate" if quality_reason else classify(question, judgment)
    k = len(question["facts"])
    f = sum(item["correct"] for item in judgment["facts"]) if not quality_reason else None
    g = sum(item["correct"] and item["supported"] for item in judgment["facts"]) if not quality_reason else None
    source = [run.get("artifact", run["id"])]
    from pathlib import Path
    artifact_path = Path(source[0])
    quality_sources = source
    if artifact_path.name == "runtime.json":
        quality_sources = [str(artifact_path.with_name("answer.txt")), str(artifact_path), str(artifact_path.parents[2] / "dataset.json")]
        if judgment is not None and run["answer"].strip():
            quality_sources.append(str(artifact_path.parents[2] / "grading/judgments.json"))
    exploration = exploration_metrics(run.get("calls", []), complete=run["exploration"]["complete"],
                                      evidence=(run["exploration"]["exploration_calls"]["evidence"] or source)[0])
    exploration["delivered_output_bytes"] = run["exploration"]["delivered_output_bytes"]
    quality = {
        "full_correct": metric(int(category == "correct") if not quality_reason else None, "ratio", reason=quality_reason, evidence=source),
        "fact_ratio": metric(f / k if f is not None else None, "ratio", f, k, quality_reason, source),
        "evidence_ratio": metric(g / k if g is not None else None, "ratio", g, k, quality_reason, source),
        "major_error": metric(int(bool(judgment["major_errors"])) if not quality_reason else None, "ratio", reason=quality_reason, evidence=source),
        "no_answer": metric(int(not run["answer"].strip()) if run["status"] in MODEL_TERMINALS else None, "ratio",
                            reason=None if run["status"] in MODEL_TERMINALS else "environment failure", evidence=source),
    }
    for value in quality.values():
        value["evidence"] = quality_sources
    exposure = evidence_exposure(question, run.get("calls", []))
    observation_sources = run["exploration"]["exploration_calls"]["evidence"] + run["usage"]["total_tokens"]["evidence"]
    if artifact_path.name == "runtime.json":
        observation_sources.append(str(artifact_path.parents[2] / "dataset.json"))
    for value in exposure.values():
        if isinstance(value, dict) and "validity" in value:
            value["evidence"] = sorted(set(observation_sources))
    return {"id": run["id"], "question_id": question["id"], "difficulty": question["difficulty"],
            "language": question["language"], "group": run["group"], "repeat": run["repeat"],
            "status": run["status"], "category": category, "quality": quality,
            "cost": {**{k: v for k, v in run["usage"].items() if isinstance(v, dict) and "value" in v},
                     "exploration_calls": run["exploration"]["exploration_calls"],
                     "elapsed_seconds": metric(run.get("elapsed_seconds"), "seconds", reason=None if run.get("elapsed_seconds") is not None else "not executed", evidence=source),
                     "host_calls": metric(run.get("host_calls"), "calls", reason=None if run.get("host_calls") is not None else "host recording incomplete", evidence=source)},
            "exploration": exploration, "exposure": exposure,
            "judgment": judgment, "conditions_valid": run.get("conditions_valid", False),
            "artifact": source[0]}


QUALITY_KEYS = ("full_correct", "fact_ratio", "evidence_ratio", "major_error", "no_answer")
COST_KEYS = ("total_tokens", "input_tokens", "output_tokens", "cached_input_tokens", "exploration_calls", "host_calls", "model_responses", "elapsed_seconds")


def summarize(rows: list[dict], repeats: int) -> dict:
    result = {"scheduled_runs": len(rows), "questions": len({r["question_id"] for r in rows}), "metrics": {}}
    for section, keys in [("quality", QUALITY_KEYS), ("cost", COST_KEYS)]:
        for key in keys:
            values = [r[section][key]["value"] for r in rows]
            observed = [v for v in values if v is not None]
            groups = collections.defaultdict(list)
            for row, value in zip(rows, values):
                groups[row["question_id"]].append(value)
            valid = bool(rows) and all(len(group) == repeats and None not in group for group in groups.values())
            reason = None if valid else "missing scheduled run/metric/repeat; denominator retained"
            mean = statistics.mean(statistics.mean(group) for group in groups.values()) if valid else None
            unit = rows[0][section][key]["unit"] if rows else "unknown"
            refs = sorted({path for r in rows for path in r[section][key]["evidence"]})
            result["metrics"][key] = {
                "mean": metric(mean, unit, sum(values) if valid else None, len(rows), reason, refs),
                "median": metric(percentile(values, .5) if valid else None, unit, reason=reason, evidence=refs),
                "p95": metric(percentile(values, .95) if valid else None, unit, reason=reason, evidence=refs),
                "sum": metric(sum(values) if valid else None, unit, reason=reason, evidence=refs),
                "observed_count": len(observed), "scheduled_count": len(values),
                "observed_partial_sum": sum(observed) if observed else None,
            }
    inputs = result["metrics"]["input_tokens"]["sum"]["value"]
    cached = result["metrics"]["cached_input_tokens"]["sum"]["value"]
    result["cached_input_ratio"] = ratio(cached, inputs, evidence=result["metrics"]["input_tokens"]["sum"]["evidence"])
    result["tokens_per_correct"] = ratio(result["metrics"]["total_tokens"]["sum"]["value"],
                                           result["metrics"]["full_correct"]["sum"]["value"], "tokens/correct",
                                           evidence=sorted(set(result["metrics"]["total_tokens"]["sum"]["evidence"] + result["metrics"]["full_correct"]["sum"]["evidence"])))
    return result


def paired_rows(rows: list[dict]) -> list[tuple[dict, dict]]:
    pairs = collections.defaultdict(dict)
    for row in rows:
        pairs[(row["question_id"], row["repeat"])][row["group"]] = row
    return [(pair["A"], pair["B"]) for pair in pairs.values() if set(pair) == {"A", "B"}]


def bootstrap(rows: list[dict], samples=10000) -> dict:
    strata = collections.defaultdict(dict)
    for a, b in paired_rows(rows):
        strata[a["difficulty"]].setdefault(a["question_id"], []).append((a, b))
    output = {}
    for section, key in [("quality", "full_correct"), ("cost", "total_tokens"), ("cost", "exploration_calls")]:
        if not strata or any(r[section][key]["value"] is None for r in rows):
            output[key] = {"low": None, "high": None, "reason": "incomplete paired metric"}
            continue
        prepared = [[(statistics.mean(a[section][key]["value"] for a, _ in pairs),
                      statistics.mean(b[section][key]["value"] for _, b in pairs)) for pairs in questions.values()]
                    for _, questions in sorted(strata.items())]
        # Reuse the same deterministic question draws for every metric. Missing
        # quality data must not change a cost interval's random sample sequence.
        rng = random.Random(SPEC["seed"])
        estimates = []
        for _ in range(samples):
            selected = [rng.choice(stratum) for stratum in prepared for _ in stratum]
            a = statistics.mean(pair[0] for pair in selected)
            b = statistics.mean(pair[1] for pair in selected)
            value = 100 * (b - a) if section == "quality" else (100 * (b / a - 1) if a else None)
            estimates.append(value)
        if None in estimates:
            output[key] = {"low": None, "high": None, "reason": "zero baseline in bootstrap resample"}
        else:
            output[key] = {"low": percentile(estimates, .025), "high": percentile(estimates, .975), "reason": None}
    return {"samples": samples, "seed": SPEC["seed"], "unit": "question cluster within difficulty; A/B and repetitions retained", "intervals": output}


def compare(rows: list[dict], repeats: int, *, with_bootstrap=True) -> dict:
    groups = {g: summarize([r for r in rows if r["group"] == g], repeats) for g in ["A", "B"]}
    delta = {}
    for key in QUALITY_KEYS + COST_KEYS:
        a, b = [groups[g]["metrics"][key]["mean"]["value"] for g in ["A", "B"]]
        delta[key] = (metric(100 * (b - a) if a is not None and b is not None else None, "pp",
                             reason=None if a is not None and b is not None else "incomplete quality metric")
                      if key in QUALITY_KEYS else change(a, b))
        delta[key]["evidence"] = sorted({path for g in ["A", "B"] for path in groups[g]["metrics"][key]["mean"]["evidence"]})
    cells = collections.Counter()
    both_correct = []
    for a, b in paired_rows(rows):
        av, bv = a["quality"]["full_correct"]["value"], b["quality"]["full_correct"]["value"]
        if av is None or bv is None:
            cells["indeterminate"] += 1
        else:
            cells[{(1, 0): "A_only", (0, 1): "B_only", (1, 1): "both", (0, 0): "neither"}[(av, bv)]] += 1
            if av == bv == 1:
                both_correct.extend([a, b])
    auxiliary = {"pairs": len(both_correct) // 2, "questions": len({r["question_id"] for r in both_correct}), "metrics": {}}
    # This auxiliary estimates the selected paired executions only, and never replaces the primary denominator.
    for key in ["total_tokens", "exploration_calls"]:
        means = []
        for group in ["A", "B"]:
            by_question = collections.defaultdict(list)
            for row in both_correct:
                if row["group"] == group:
                    by_question[row["question_id"]].append(row["cost"][key]["value"])
            valid = bool(by_question) and all(None not in vals for vals in by_question.values())
            means.append(statistics.mean(statistics.mean(vals) for vals in by_question.values()) if valid else None)
        auxiliary["metrics"][key] = {"A_mean": means[0], "B_mean": means[1], "change": change(*means)}
        refs = sorted({path for r in both_correct for path in r["cost"][key]["evidence"]})
        auxiliary["metrics"][key]["change"]["evidence"] = refs
    return {"groups": groups, "differences": delta, "paired_outcomes": {k: cells[k] for k in ["A_only", "B_only", "both", "neither", "indeterminate"]},
            "both_correct_cost": auxiliary, "bootstrap": bootstrap(rows, SPEC["bootstrap_samples"]) if with_bootstrap else None}


def verdict(comparison: dict, reliability_valid: bool) -> dict:
    diffs = comparison["differences"]
    accuracy = diffs["full_correct"]["value"]
    costs = [diffs[k]["value"] for k in ["total_tokens", "exploration_calls"]]
    interval = (comparison.get("bootstrap") or {}).get("intervals", {}).get("full_correct", {})
    if not reliability_valid or accuracy is None or None in costs:
        result = "해당 비교 판정 보류"
    elif accuracy < 0:
        result = "최우선 목표 미달"
    elif accuracy > 0 and any(c >= 25 for c in costs):
        result = "비용 급증으로 비허용, 원인 분석 필요"
    elif accuracy > 0 and interval.get("low") is not None and interval["low"] > 0:
        result = "전체 목표 달성" if all(c < 0 for c in costs) else "허용 범위의 정확도 개선, 비용 최적화 과제 남음"
    elif all(c < 0 for c in costs):
        result = "효율 개선 관측, 정확도 상승 미확인"
    else:
        result = "정확도 상승 미확인"
    return {"decision": result, "accuracy_observation": "상승 관측, 확정 근거 부족" if accuracy is not None and accuracy > 0
            and (interval.get("low") is None or interval["low"] <= 0) else None,
            "scope": "고정된 데이터셋에 한정하며, 차이 미검출은 동등성 입증이 아님"}
