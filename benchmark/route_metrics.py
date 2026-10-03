"""Route observations with explicit units, provenance and unobserved values."""
from __future__ import annotations

import collections
import json
from pathlib import Path

from . import route_contract as routes
from .core import digest, file_digest, read_json, read_jsonl, require, write_json
from .responses import content_text, source_lines

FIELD_PROVENANCE = {
    "raw_json_bytes": {"unit": "UTF-8 bytes", "origin": "native result serialized as canonical JSON"},
    "relay_json_bytes": {"unit": "UTF-8 bytes", "origin": "actual relay result serialized as canonical JSON"},
    "client_observed_text_bytes": {"unit": "UTF-8 bytes", "origin": "matched client rollout block", "missing": "null with reason"},
    "native_truncated": {"unit": "boolean or null", "origin": "native explicit metadata only", "missing": "null, never inferred from sparsity"},
    "relay_loss": {"unit": "boolean", "origin": "raw/relay canonical result equality"},
    "client_delivery": {"unit": "state", "origin": "wrapper ID plus exact JSON/text match or observed prefix", "missing": "unknown"},
    "source_lines": {"unit": "source locations or null", "origin": "recognized source-bearing client output only", "missing": "null, never inferred from filenames"},
    "tool_calls": {"unit": "calls", "origin": "arm, tool name, auxiliary flag in relay request"},
    "tokens": {"unit": "model tokens", "origin": "response-identified provider usage records", "missing": "null with reason"},
    "elapsed_seconds": {"unit": "seconds", "origin": "total execution clock including collection/cleanup; excludes preparation"},
    "timing": {"unit": "seconds", "origin": "runtime started/model-exited/elapsed timestamps", "missing": "null with reason",
               "scopes": ["model process lifetime including startup", "post-model cleanup", "collection after cutoff may overlap model lifetime"]},
    "quality": {"unit": "fact dimensions and per-route core completeness", "origin": "validated frozen route judgment"},
}


def normalize_usage(records: list[dict], *, terminal_complete: bool) -> dict:
    fields = ("input_tokens", "output_tokens", "total_tokens", "cached_input_tokens", "reasoning_output_tokens")
    raw, unique, conflicts = [], {}, []
    for row in records:
        if row.get("type") != "token_usage_record":
            continue
        payload = row.get("payload", {})
        raw.append(payload)
        ident = payload.get("response_id")
        usage = payload.get("usage")
        if not isinstance(ident, str) or not isinstance(usage, dict):
            conflicts.append("missing response identity or usage object")
            continue
        if ident in unique and unique[ident] != usage:
            conflicts.append(f"conflicting usage: {ident}")
        unique[ident] = usage
    totals = {}
    for name in fields:
        values = []
        for usage in unique.values():
            value = usage.get(name)
            if value is None and name == "cached_input_tokens":
                value = (usage.get("input_tokens_details") or {}).get("cached_tokens")
            if value is None and name == "reasoning_output_tokens":
                value = (usage.get("output_tokens_details") or {}).get("reasoning_tokens")
            values.append(value)
        known = bool(values) and all(type(v) is int and v >= 0 for v in values)
        observed = sum(values) if known else None
        reason = ("conflicting usage records" if conflicts else "no usage records" if not unique
                  else "provider did not supply this component" if not known else "unfinished model usage" if not terminal_complete else None)
        totals[name] = {"value": observed if reason is None else None, "observed": observed,
                        "unit": "tokens", "missing_reason": reason}
    return {"raw_records": raw, "response_ids": list(unique), "conflicts": conflicts,
            "terminal_complete": terminal_complete, "components": totals}


def normalize_native_calls(folder: Path, *, write_delivery=True) -> tuple[list[dict], bool, int | None, list[dict]]:
    from .native_baseline import command_items
    from .runner import output_blocks, read_live_jsonl
    events, rollout = read_live_jsonl(folder / "events.jsonl"), read_live_jsonl(folder / "rollout.jsonl")
    runtime = read_json(folder / "runtime.json") if (folder / "runtime.json").exists() else {}
    calls = []
    for item in command_items(events):
        output = item.get("aggregated_output")
        calls.append({"id": item["id"], "arm": "rg", "tool": "command_execution", "is_auxiliary": False,
                      "arguments": {"command": item.get("command")},
                      "status": "incomplete" if not item["completed"] else "success" if item.get("exit_code") == 0 else "error",
                      "completed": item["completed"], "native_item": item,
                      "native_output_bytes": len(output.encode()) if isinstance(output, str) else None,
                      "raw_result": None, "relay_result": None, "raw_json_bytes": None, "relay_json_bytes": None,
                      "relay_loss": None, "native_truncated": None, "delivered_lines": None, "duration_seconds": None,
                      "client_delivery": {"status": "unknown", "complete": None,
                                          "reason": "CLI aggregated_output is not proof of what the model saw; preserve native logs."}})
    payloads = [r.get("payload", {}) for r in rollout if r.get("type") == "response_item"]
    host = [p for p in payloads if p.get("type") in {"function_call", "custom_tool_call"}]
    violations = [{"name": e["item"].get("tool"), "reason": "MCP used in native-agent baseline"}
                  for e in events if e.get("type") == "item.started" and e.get("item", {}).get("type") == "mcp_tool_call"]
    total_bytes = sum(len(block.encode()) for p in payloads if p.get("type") in {"function_call_output", "custom_tool_call_output"}
                      for block in output_blocks(p))
    complete = bool(events) and all(c["completed"] for c in calls) and not runtime.get("shutdown", {}).get("partial_jsonl_files")
    complete = complete and runtime.get("record_collection_complete", False)
    if write_delivery:
        write_json(folder / "delivery.json", {"actual_model_output_bytes": total_bytes if rollout else None,
                   "missing_reason": None if rollout else "No real solver rollout was observed.",
                   "complete_result_deliveries": None, "source": "rollout.jsonl; native command items in events.jsonl",
                   "limitation": "Native CLI output and model-delivered output are separate observations."})
    return calls, complete, len(host) if rollout else None, violations


def normalize_calls(folder: Path, *, write_delivery=True) -> tuple[list[dict], bool, int | None, list[dict]]:
    from .runner import delivered_leaves, output_blocks, read_live_jsonl
    runtime_path = folder / "runtime.json"
    if runtime_path.exists() and read_json(runtime_path).get("backend") == "claude_code":
        return normalize_claude_calls(folder, write_delivery=write_delivery)
    if (folder / "native-agent.json").exists():
        return normalize_native_calls(folder, write_delivery=write_delivery)
    relay = read_live_jsonl(folder / "relay.jsonl")
    rollout = read_live_jsonl(folder / "rollout.jsonl")
    runtime = read_json(folder / "runtime.json") if (folder / "runtime.json").exists() else {}
    requests, results, errors, wrappers, deliveries, cells = {}, {}, {}, {}, {}, {}
    complete = not runtime.get("shutdown", {}).get("partial_jsonl_files")
    violations, host_calls, total_bytes = [], 0, 0
    for row in relay:
        event = row.get("event")
        if event in {"request", "result", "rpc_error"}:
            target = {"request": requests, "result": results, "rpc_error": errors}[event]
            if row["id"] in target:
                complete = False
            target[row["id"]] = row
    for row in rollout:
        payload = row.get("payload", {})
        if row.get("type") != "response_item":
            continue
        if payload.get("type") in {"function_call", "custom_tool_call"}:
            name = payload.get("name", "").rsplit(".", 1)[-1]
            wrapper = payload.get("id")
            if name == "wait":
                try:
                    wrapper = cells.get(json.loads(payload.get("arguments", "{}"))["cell_id"], wrapper)
                except (ValueError, KeyError):
                    complete = False
            wrappers[payload.get("call_id")] = wrapper
            host_calls += 1
            if name not in {"exec", "wait"}:
                violations.append({"name": name, "reason": "unapproved host capability"})
        elif payload.get("type") in {"custom_tool_call_output", "function_call_output"}:
            wrapper = wrappers.get(payload.get("call_id"))
            for block in output_blocks(payload):
                total_bytes += len(block.encode())
                import re
                match = re.search(r"Script running with cell ID ([^\s]+)", block)
                if match:
                    cells[match[1]] = wrapper
                deliveries.setdefault(wrapper, []).extend({"text": text, "used": False} for text in delivered_leaves(block))
    calls = []
    for ident, request in requests.items():
        result = results.get(ident)
        raw = result.get("raw_result") if result else None
        relayed = result.get("relay_result") if result else None
        text = content_text(relayed) if relayed else ""
        delivery = {"status": "unknown", "complete": None, "observed_text_bytes": None,
                    "reason": "No uniquely matched client output."}
        parsed_lines = None
        after_exit = bool(result and runtime.get("model_exited_monotonic") is not None
                          and result["finished_monotonic"] > runtime["model_exited_monotonic"])
        candidates = deliveries.get(request.get("metadata", {}).get("itemId"), [])
        if after_exit:
            candidates = []
            delivery["reason"] = "Result completed after solver exit; excluded from delivered evidence."
        for candidate in candidates:
            if candidate["used"]:
                continue
            seen = candidate["text"]
            try:
                whole_json = isinstance(relayed, dict) and json.loads(seen) == relayed
            except ValueError:
                whole_json = False
            exact_text = bool(text) and seen == text
            prefix = bool(seen) and bool(text) and text.startswith(seen) and len(seen) < len(text)
            if whole_json or exact_text or prefix:
                candidate["used"] = True
                delivery = {"status": "complete_json" if whole_json else "partial_text" if prefix else "text_observed",
                            "complete": True if whole_json else False if prefix else None,
                            "observed_text_bytes": len(seen.encode()),
                            "reason": None if whole_json else "Only the observed textual representation is certified."}
                parsed_lines = source_lines(request["tool"], request["arguments"], text if whole_json else seen, partial=prefix)
                break
        calls.append({**request, "status": "error" if ident in errors or raw and raw.get("isError") else "success" if result else "incomplete",
                      "raw_result": raw, "relay_result": relayed,
                      "raw_json_bytes": result.get("raw_json_bytes") if result else None,
                      "relay_json_bytes": result.get("relay_json_bytes") if result else None,
                      "relay_loss": raw != relayed if result else None,
                      "native_truncated": result.get("native_truncated") if result else None,
                      "client_delivery": delivery, "delivered_lines": parsed_lines, "result_after_model_exit": after_exit,
                      "duration_seconds": result["finished_monotonic"] - request["started_monotonic"] if result else None})
    complete = complete and set(requests) == set(results) and not errors
    if write_delivery:
        write_json(folder / "delivery.json", {"actual_model_output_bytes": total_bytes if rollout else None,
                   "missing_reason": None if rollout else "No real solver rollout was observed.",
                   "complete_result_deliveries": sum(c["client_delivery"]["complete"] is True for c in calls),
                   "source": "rollout.jsonl"})
    return calls, complete, host_calls if rollout else None, violations


def normalize_claude_calls(folder: Path, *, write_delivery=True) -> tuple[list[dict], bool, int | None, list[dict]]:
    from .claude_client import NATIVE_TOOLS, tool_items
    from .runner import read_live_jsonl
    events = read_live_jsonl(folder / "events.jsonl")
    runtime = read_json(folder / "runtime.json")
    native = (folder / "native-agent.json").exists()
    configured = read_json(folder / ("native-agent.json" if native else "relay-settings.json"))
    items = [c for c in tool_items(events) if c.get("name") != "EndConversation"]
    allowed = set(NATIVE_TOOLS) if native else {"ToolSearch", *["mcp__navigation__" + name for name in configured["enabled_tools"]]}
    violations = [{"name": c.get("name"), "reason": "unapproved Claude capability"} for c in items if c.get("name") not in allowed]
    complete = runtime.get("record_collection_complete", False) and all(c["completed"] for c in items)
    calls, stream_bytes = [], 0
    outputs = {}
    for item in items:
        content = (item.get("result") or {}).get("content")
        text = content if isinstance(content, str) else "\n".join(b.get("text", "") for b in content if b.get("type") == "text") if isinstance(content, list) else None
        outputs[item["id"]] = text
        if text is not None:
            stream_bytes += len(text.encode())
        if native:
            result = item.get("result")
            calls.append({"id": item["id"], "arm": "rg", "tool": item.get("name"), "arguments": item.get("input"),
                          "is_auxiliary": False, "status": "incomplete" if result is None else "error" if result.get("is_error") else "success",
                          "completed": item["completed"], "native_item": item, "raw_result": None, "relay_result": None,
                          "native_output_bytes": len(text.encode()) if text is not None else None,
                          "raw_json_bytes": None, "relay_json_bytes": None, "relay_loss": None, "native_truncated": None,
                          "delivered_lines": None, "duration_seconds": None,
                          "client_delivery": {"status": "client_stream_observed" if text is not None else "unknown", "complete": None,
                                              "reason": "CLI stream is observed; the complete provider input is not observed."}})
    if not native:
        relay = read_live_jsonl(folder / "relay.jsonl")
        requests = {r["id"]: r for r in relay if r.get("event") == "request"}
        results = {r["id"]: r for r in relay if r.get("event") == "result"}
        errors = {r["id"] for r in relay if r.get("event") == "rpc_error"}
        signature = lambda tool, args: (tool.removeprefix("mcp__navigation__"), json.dumps(args, sort_keys=True, ensure_ascii=False))
        request_counts = collections.Counter(signature(r["tool"], r["arguments"]) for r in requests.values())
        by_signature = collections.defaultdict(list)
        for item in items:
            by_signature[signature(item.get("name", ""), item.get("input", {}))].append(item)
        for ident, request in requests.items():
            result = results.get(ident)
            raw, relayed = (result.get("raw_result"), result.get("relay_result")) if result else (None, None)
            key = signature(request["tool"], request["arguments"])
            matches = by_signature[key]
            seen = outputs.get(matches[0]["id"]) if len(matches) == request_counts[key] == 1 else None
            calls.append({**request, "status": "error" if ident in errors or raw and raw.get("isError") else "success" if result else "incomplete",
                          "raw_result": raw, "relay_result": relayed,
                          "raw_json_bytes": result.get("raw_json_bytes") if result else None,
                          "relay_json_bytes": result.get("relay_json_bytes") if result else None,
                          "relay_loss": raw != relayed if result else None, "native_truncated": None,
                          "client_delivery": {"status": "client_stream_observed" if seen is not None else "unknown", "complete": None,
                                              "observed_text_bytes": len(seen.encode()) if seen is not None else None,
                                              "reason": "Unique native call matched in CLI stream; complete provider input remains unobserved."
                                              if seen is not None else "No uniquely matched native client call."},
                          "delivered_lines": None,
                          "duration_seconds": result["finished_monotonic"] - request["started_monotonic"] if result else None})
        complete = complete and set(requests) == set(results) and not errors
    if write_delivery:
        write_json(folder / "delivery.json", {"actual_model_output_bytes": None, "client_stream_text_bytes": stream_bytes,
            "missing_reason": "Claude CLI events do not expose the complete subsequent provider input.",
            "complete_result_deliveries": None, "source": "events.jsonl", "backend": "claude_code"})
    return calls, complete, len(items) if events else None, violations


def finish_execution(folder: Path, entry: dict, execution: dict) -> dict:
    calls, complete, host_calls, violations = normalize_calls(folder)
    runtime = read_json(folder / "runtime.json")
    if runtime.get("backend") == "claude_code":
        from .claude_client import usage as claude_usage
        usage = claude_usage(read_jsonl(folder / "events.jsonl"),
                            terminal_complete=execution.get("usage", {}).get("complete", False), evidence=str(folder / "events.jsonl"))
    else:
        usage = normalize_usage(read_jsonl(folder / "rollout.jsonl"),
                                terminal_complete=execution.get("usage", {}).get("complete", execution.get("status") == "completed"))
    access = read_json(folder / ("native-agent.json" if (folder / "native-agent.json").exists() else "relay-settings.json"))
    error = execution.get("error") or runtime.get("launch_error")
    if not error and execution.get("status") == "environment_error":
        events = read_jsonl(folder / "events.jsonl")
        reported = [e.get("error") or e.get("message") or e for e in events if e.get("type") in {"turn.failed", "error"}]
        error = reported[-1] if reported else f"{runtime.get('backend', 'codex')} exit_code={runtime.get('exit_code')}; see stderr.log and events.jsonl."
    exited, started = runtime.get("model_exited_monotonic"), runtime.get("started_monotonic")
    lifetime = exited - started if exited is not None and started is not None else None
    total = execution.get("elapsed_seconds")
    timing = {"unit": "seconds", "model_process_lifetime": lifetime,
              "post_model_cleanup": max(0, total - lifetime) if total is not None and lifetime is not None else None,
              "missing_reason": None if lifetime is not None and total is not None else "No complete process clock was observed.",
              "collection_after_cutoff": execution.get("shutdown", {}).get("cleanup_elapsed_seconds"),
              "collection_scope": "may overlap model process lifetime; not an additive duration"}
    return {**entry, **execution, "error": error, "calls": calls, "host_calls": host_calls, "usage": usage, "timing": timing,
            "source_snapshot": access["source"],
            "artifacts": {name: str(folder / name) for name in ("execution.json", "runtime.json", "events.jsonl", "rollout.jsonl",
                           "relay.jsonl", "native-agent.json", "relay-settings.json", "delivery.json", "stderr.log", "command.json",
                           "claude-settings.json", "mcp-config.json") if (folder / name).exists()},
            "call_log_complete": complete, "boundary_violations": violations,
            "conditions_valid": execution.get("conditions_valid", False) and not violations,
            "tool_call_counts": dict(collections.Counter(("auxiliary:" if c.get("is_auxiliary") else "native:") + c["tool"] for c in calls))}


def quality_counts(judgments: list[dict], questions: list[dict]) -> dict:
    counts = {}
    for level in ("core", "extended"):
        expected = sum(sum(f["level"] == level for f in q["facts"]) for q in questions)
        facts = [f for judgment in judgments for f in judgment["facts"] if f["level"] == level]
        dimensions = {}
        for dimension in ("correct", "supported"):
            values = [f[dimension]["value"] for f in facts]
            dimensions[dimension] = {"true": sum(v is True for v in values), "false": sum(v is False for v in values),
                                     "indeterminate": sum(v == "indeterminate" for v in values), "ungraded": expected - len(values)}
        counts[level] = {"planned_facts": expected, "graded_facts": len(facts), "dimensions": dimensions}
    states = [j["core_complete"] for j in judgments]
    counts["core_complete"] = {"true": sum(v is True for v in states), "false": sum(v is False for v in states),
                                "indeterminate": sum(v == "indeterminate" for v in states), "ungraded": len(questions) - len(states),
                                "denominator": len(questions),
                                "macro_rate": sum(v is True for v in states) / len(questions)
                                if len(states) == len(questions) and "indeterminate" not in states else None}
    return counts


def report(root: Path, manifest: dict, rows: list[dict], judgments: dict) -> dict:
    dataset = routes.load_dataset()
    planned = routes.schedule()
    by_id = {r["id"]: r for r in rows}
    require(len(by_id) == len(rows) and set(by_id) <= {r["id"] for r in planned}, "계획 밖 또는 중복 결과")
    require(set(judgments) <= set(by_id), "실행 기록 없는 채점")
    result_rows, arms = [], {}
    for slot in planned:
        row = by_id.get(slot["id"])
        judgment = judgments.get(slot["id"])
        require(not row or row.get("model") == manifest["models"]["solver"]["model"], "선택한 풀이 모델과 다른 실행 결과")
        require(not row or row.get("backend") == manifest["models"]["solver"]["backend"], "선택한 실행기와 다른 실행 결과")
        require(judgment is None or row.get("conditions_valid") is True, "유효한 실행 조건이 확인되지 않은 결과에 채점이 연결되었습니다")
        require(not row or not row.get("reused_from"), "route 프로필은 기존 A 답변을 재사용하지 않습니다")
        require(not row or all(row[key] == slot[key] for key in ("question_id", "arm", "attempt")), "결과 슬롯 식별 불일치")
        result_rows.append({**slot, "status": row["status"] if row else "planned", "judgment": judgment,
                            "elapsed_seconds": row.get("elapsed_seconds") if row else None,
                            "timing": row.get("timing") if row else None,
                            "usage": row.get("usage") if row else None,
                            "tool_call_counts": row.get("tool_call_counts") if row else None,
                            **{key: row.get(key) if row else None for key in (
                                "conditions_valid", "error", "failure_reason", "condition_errors", "boundary_violations",
                                "call_log_complete", "host_calls", "artifact", "artifacts", "shutdown", "budget", "native_call_budget",
                                "usage_collection", "model", "backend", "agent_version", "effort_observation", "codex_version", "thread_id", "provenance")}})
    for arm in routes.ARM_IDS:
        selected = [r for r in result_rows if r["arm"] == arm]
        graded = [r["judgment"] for r in selected if r["judgment"] is not None]
        arms[arm] = {"planned": 3, "observed": sum(r["status"] != "planned" for r in selected),
                     "invalid_conditions": sum(r["conditions_valid"] is False for r in selected),
                     "unconfirmed_conditions": sum(r["status"] != "planned" and r["conditions_valid"] is None for r in selected),
                     "quality": quality_counts(graded, dataset["questions"]),
                     "tool_condition": manifest.get("arms", {}).get(arm, {}).get("tool_condition")}
    result = {"schema_version": 1, "profile": routes.PROFILE, "planned_solver_runs": len(routes.schedule()),
              "condition_identity": {key: manifest.get(key) for key in ("contract_hash", "harness_sha256", "input_hashes", "models", "actual_versions")},
              "observed_solver_runs": sum(r["status"] != "planned" for r in result_rows),
              "arms": arms, "rows": result_rows, "scope": "Three fixed routes, one attempt per arm. Descriptive observations only.",
              "variance_estimated": False}
    write_json(root / "report/summary.json", result)
    lines = ["# Grafana 경로 벤치마크", ""]
    if manifest.get("models"):
        models = manifest["models"]
        lines += [f"풀이: {models['solver']['model']} / {models['solver']['reasoning_effort']}; "
                  f"채점: {models['grader']['model']} / {models['grader']['reasoning_effort']}", ""]
    lines += ["| 도구 | 도구 구성 | 예정 | 관측 | 핵심 완전 충족 | 미채점 |", "|---|---|---:|---:|---:|---:|"]
    for arm, value in arms.items():
        complete = value["quality"]["core_complete"]
        condition = {"assisted": "필수 보조 도구 포함", "native_only": "자체 도구", "native_agent_baseline": "기본 에이전트 · 추가 MCP 없음"}.get(value["tool_condition"], "미연결")
        label = "기본 에이전트 (rg 사용 가능)" if arm == "rg" else arm
        lines.append(f"| {label} | {condition} | 3 | {value['observed']} | {complete['true'] if value['observed'] else '미실행'} | {complete['ungraded']} |")
    lines += ["", "| 문항·비교군 | 실행 상태 | 조건 유효 | 호출 로그 완결 | 오류·실패 사유 | 근거 |",
              "|---|---|---|---|---|---|"]
    def cell(value):
        return (json.dumps(value, ensure_ascii=False) if isinstance(value, (list, dict)) else str(value)).replace("|", "\\|").replace("\n", " ")
    for row in result_rows:
        reason = row["error"] or row["failure_reason"] or row["condition_errors"] or row["boundary_violations"]
        reason = reason or ("미실행" if row["status"] == "planned" else "실행 조건 확인 실패" if row["conditions_valid"] is False else "기록 없음")
        evidence = row["artifacts"] or row["artifact"] or "미관측"
        lines.append("| " + " | ".join(cell(v) for v in (row["id"], row["status"], row["conditions_valid"], row["call_log_complete"], reason, evidence)) + " |")
    lines += ["", "미실행·미관측 값은 0점으로 치환하지 않습니다. 핵심과 확장, true/false/indeterminate/미채점을 별도 기록합니다.",
              "한 문항당 1회 계획이므로 반복 분산이나 통계적 유의성을 주장하지 않습니다."]
    (root / "report/results.md").write_text("\n".join(lines) + "\n")
    return result


def publish_metrics() -> dict:
    preview = routes.PREPARATION / "report-contract"
    result = report(preview, {}, [], {})
    require(result["observed_solver_runs"] == 0 and all(r["judgment"] is None for r in result["rows"]), "가상 관측 생성")
    delivery = read_json(routes.EVIDENCE / "02-delivery.json")
    folder = Path(delivery["raw_relay_checks"]["relay_log"]).parent
    calls, complete, host_calls, violations = normalize_calls(folder)
    require(complete and len(calls) == 1 and not calls[0]["relay_loss"] and host_calls is None and not violations,
            "준비 probe 관측 정규화 오류")
    return routes.publish_handoff("06-metrics.json", {
        "status": "verified_offline", "measurement_schema": FIELD_PROVENANCE,
        "result_schema": {"slot_keys": [r["id"] for r in result["rows"]], "attempts_per_slot": 1,
                          "preview": str(preview / "report/summary.json"), "preview_sha256": file_digest(preview / "report/summary.json")},
        "field_provenance": FIELD_PROVENANCE, "unknown_rules": "Preserve null/reason and dimension-level indeterminate; never fill zero.",
        "native_baseline": {"exploration_calls": "native command_execution item IDs, deduplicated across started/completed",
                            "delivery": "CLI output is retained separately; model delivery is unknown without rollout evidence"},
        "aggregation_rules": "Fixed three-route macro denominator, separate core/extended fact denominators; no variance estimate.",
        "offline_verification": {"native_probe_calls": len(calls), "relay_loss": False,
                                 "actual_solver_delivery": "unobserved", "planned_unique_slots": len(routes.schedule())},
        "observed_solver_runs": 0, "limitations": ["Real solver usage and history delivery have not been measured for either selectable model."]},
        implementation=["benchmark/route_metrics.py", "benchmark/runner.py", "benchmark/execution.py", "benchmark/claude_client.py"],
        dependencies=["02-delivery.json", "05-grading.json"])


if __name__ == "__main__":
    print(publish_metrics()["status"])
