"""Response-identified usage reconciliation, separate from whole-run certification."""
from __future__ import annotations

from .core import metric, ratio


def usage_metrics(records: list[dict], *, terminal_complete: bool, evidence: str,
                  completion: dict | None = None) -> dict:
    keys = ("input_tokens", "output_tokens", "cached_input_tokens", "total_tokens")
    optional = ("cache_write_input_tokens", "reasoning_output_tokens")
    unique, snapshots, turns = {}, [], {}
    errors, checks, legacy = [], [], []
    sums = dict.fromkeys(keys + optional, 0)

    def components(value, location):
        if not isinstance(value, dict) or any(type(value.get(k)) is not int or value[k] < 0 for k in keys):
            errors.append(f"missing/invalid token component: {location}")
            return False
        if value["cached_input_tokens"] > value["input_tokens"]:
            errors.append(f"cache input exceeds input: {location}")
        if value["total_tokens"] != value["input_tokens"] + value["output_tokens"]:
            errors.append(f"inconsistent total tokens: {location}")
        for key in optional:
            if key in value and (type(value[key]) is not int or value[key] < 0):
                errors.append(f"invalid optional token component: {location}/{key}")
                return False
        return True

    def matches(value, expected):
        return all(value.get(k) == expected[k] for k in keys) and all(
            value[k] == expected[k] for k in optional if k in value)

    for line, row in enumerate(records, 1):
        payload = row.get("payload", {})
        location = f"{evidence}:{line}"
        if row.get("type") == "event_msg" and payload.get("type") == "token_count":
            total = (payload.get("info") or {}).get("total_token_usage")
            if total is not None:
                legacy.append((line, total))
        if row.get("type") != "token_usage_record":
            continue
        response_id, usage = payload.get("response_id"), payload.get("usage")
        if not isinstance(response_id, str) or not response_id:
            errors.append(f"missing response ID: {location}")
            continue
        if not components(usage, location):
            continue
        signature = {k: payload.get(k) for k in ("usage", "turn_id", "thread_id", "turn_token_usage", "thread_token_usage")}
        if response_id in unique:
            if unique[response_id] != signature:
                errors.append(f"conflicting duplicate response usage: {location}")
            continue
        unique[response_id] = signature
        turn = turns.setdefault(payload.get("turn_id"), dict.fromkeys(keys + optional, 0))
        for key in sums:
            sums[key] += usage.get(key, 0)
            turn[key] += usage.get(key, 0)
        has_snapshot = any(k in payload for k in ("turn_token_usage", "thread_token_usage"))
        for field, expected in [("turn_token_usage", turn), ("thread_token_usage", sums)]:
            if has_snapshot:
                value = payload.get(field)
                valid = components(value, f"{location}/{field}") and matches(value, expected)
                if not valid:
                    errors.append(f"request totals disagree with {field}: {location}")
                checks.append({"response_id": response_id, "field": field, "valid": valid, "evidence": location})
        snapshots.append({"response_id": response_id, "line": line, "total": dict(sums),
                          "reconciled": has_snapshot})

    legacy_checks = []
    for line, total in legacy:
        location = f"{evidence}:{line}"
        if not components(total, location):
            continue
        candidates = [s for s in snapshots if s["line"] <= line and matches(total, s["total"])]
        snapshot = candidates[-1] if candidates else None
        if snapshot is None and any(total[k] for k in keys):
            errors.append(f"request totals disagree with cumulative usage: {location}")
        status = "current" if snapshot and matches(total, sums) else "delayed"
        legacy_checks.append({"evidence": location, "status": status if snapshot or not any(total[k] for k in keys) else "conflict",
                              "response_id": snapshot["response_id"] if snapshot else None})
    if not unique:
        errors.append("no request usage records")
    if not snapshots or not (snapshots[-1]["reconciled"] or any(c["status"] == "current" for c in legacy_checks)):
        errors.append("missing cumulative reconciliation event")
    completion = completion or {"complete": terminal_complete, "reasons": [], "evidence": [evidence]}
    terminal_usage = completion.get("terminal_usage")
    if terminal_usage is not None and any(terminal_usage.get(k) != sums[k] for k in keys if k != "total_tokens"):
        errors.append("exec terminal usage disagrees with recorded usage")
    cost_reasons = list(completion.get("reasons", []))
    if not terminal_complete or not completion.get("complete"):
        cost_reasons.append("in-flight/unfinished model usage cannot be certified")
    consistency_errors = sorted(set(errors))
    all_errors = sorted(set(errors + cost_reasons))
    observed = {k: sums[k] for k in keys}
    result = {"complete": not all_errors, "errors": all_errors, "observed": observed,
              "response_ids": sorted(unique),
              "consistency": {"status": "valid" if not errors else "invalid", "errors": consistency_errors,
                              "response_checks": checks, "legacy_events": legacy_checks, "evidence": [evidence]},
              "cost_completeness": {**completion, "complete": not all_errors,
                                    "status": "complete" if not all_errors else "incomplete", "reasons": all_errors}}
    sources = sorted(set([evidence] + completion.get("evidence", [])))
    for key, value in observed.items():
        result[key] = metric(value if not all_errors else None, "tokens", reason="; ".join(all_errors) or None, evidence=sources)
    result["model_responses"] = metric(len(unique) if not all_errors else None, "responses",
                                       reason="; ".join(all_errors) or None, evidence=sources)
    result["cached_input_ratio"] = ratio(result["cached_input_tokens"]["value"], result["input_tokens"]["value"], evidence=sources)
    return result
