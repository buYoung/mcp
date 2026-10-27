"""Route observations with explicit units, provenance and unobserved values."""
from __future__ import annotations

import collections
import json
from pathlib import Path

from . import route_contract as routes
from .core import digest, file_digest, read_json, require, write_json
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
    "observations": {"unit": "per-slot counts, seconds and distributions", "origin": "pi turns/durationMs/outputGuard/script output plus relay or bash wrapper clocks",
                     "missing": "null for unobserved totals; observed partial sums retained"},
    "quality": {"unit": "fact dimensions and per-route core completeness", "origin": "validated frozen route judgment"},
}


def normalize_usage(records: list[dict], *, terminal_complete: bool) -> dict:
    from .usage import usage_metrics
    return usage_metrics(records, terminal_complete=terminal_complete, evidence="rollout.jsonl")


def relay_call_records(folder: Path) -> tuple[dict, dict, dict, dict, bool]:
    """Attempt/terminal completeness is distinct from successful product results."""
    from .runner import read_jsonl_prefix
    rows, state = read_jsonl_prefix(folder / "relay.jsonl")
    attempts, requests, results, errors, terminals, budgets = {}, {}, {}, {}, {}, {}
    tables = {"attempt": attempts, "request": requests, "result": results, "rpc_error": errors,
              "terminal": terminals, "budget": budgets}
    complete = state["complete"]
    for row in rows:
        table = tables.get(row.get("event"))
        if table is None:
            continue
        ident = row.get("id")
        if not isinstance(ident, str) or ident in table:
            complete = False
            continue
        table[ident] = row
    if attempts:
        complete = complete and set(attempts) == set(terminals) == set(budgets) and set(requests) <= set(attempts)
        complete = complete and set(results).isdisjoint(errors) and (set(results) | set(errors)) == set(requests)
        for ident, terminal in terminals.items():
            complete = complete and terminal.get("dispatched") == (ident in requests)
            complete = complete and (terminal.get("status") in {"success", "error"} if ident in requests
                                     else terminal.get("status") == "rejected")
        attempts = {ident: {**row, **requests.get(ident, {}), "dispatched": ident in requests,
                           "budget_consumed": terminals.get(ident, {}).get("budget_consumed")}
                    for ident, row in attempts.items()}
    else:
        # Old preparation transcripts have request/result but no attempt/terminal records.
        complete = complete and not terminals and not budgets and set(requests) == (set(results) | set(errors)) and set(results).isdisjoint(errors)
        attempts = {ident: {**row, "dispatched": True, "budget_consumed": True} for ident, row in requests.items()}
    return attempts, results, errors, terminals, bool(complete)


def collected_call_logs(folder: Path, runtime: dict, names: tuple[str, ...]) -> bool:
    from .runner import read_jsonl_prefix
    states = [read_jsonl_prefix(folder / name)[1] for name in names]
    return bool(runtime.get("record_collection_complete") and not runtime.get("shutdown", {}).get("partial_jsonl_files")
                and all(state["complete"] and state["valid_records"] for state in states))


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
                      "completed": item["completed"], "dispatched": True, "native_item": item,
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
    host_ids = [p.get("call_id") for p in host]
    output_ids = [p.get("call_id") for p in payloads if p.get("type") in {"function_call_output", "custom_tool_call_output"}]
    complete = (bool(events) and all(c["completed"] for c in calls)
                and collected_call_logs(folder, runtime, ("events.jsonl", "rollout.jsonl"))
                and None not in host_ids and len(host_ids) == len(set(host_ids)) == len(output_ids)
                and set(host_ids) == set(output_ids))
    if write_delivery:
        write_json(folder / "delivery.json", {"actual_model_output_bytes": total_bytes if rollout else None,
                   "missing_reason": None if rollout else "No real solver rollout was observed.",
                   "complete_result_deliveries": None, "source": "rollout.jsonl; native command items in events.jsonl",
                   "limitation": "Native CLI output and model-delivered output are separate observations."})
    return calls, complete, len(host) if rollout else None, violations


def normalize_calls(folder: Path, *, write_delivery=True, probe_only=False) -> tuple[list[dict], bool, int | None, list[dict]]:
    from .runner import delivered_leaves, output_blocks, read_live_jsonl
    runtime_path = folder / "runtime.json"
    if runtime_path.exists() and read_json(runtime_path).get("backend") in {"claude_code", "pi"}:
        if read_json(runtime_path)["backend"] == "pi" and (folder / "native-agent.json").exists():
            return normalize_pi_native_calls(folder, write_delivery=write_delivery)
        return normalize_claude_calls(folder, write_delivery=write_delivery)
    if (folder / "native-agent.json").exists():
        return normalize_native_calls(folder, write_delivery=write_delivery)
    rollout = read_live_jsonl(folder / "rollout.jsonl")
    runtime = read_json(folder / "runtime.json") if (folder / "runtime.json").exists() else {}
    requests, results, errors, terminals, transcript_complete = relay_call_records(folder)
    wrappers, deliveries, cells, pending_host = {}, {}, {}, set()
    complete = transcript_complete and (probe_only or collected_call_logs(folder, runtime, ("events.jsonl", "rollout.jsonl")))
    if not probe_only:
        complete = complete and runtime.get("shutdown", {}).get("relay_cleanup", {}).get("complete", False)
    violations, host_calls, total_bytes = [], 0, 0
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
            call_id = payload.get("call_id")
            if not isinstance(call_id, str) or call_id in wrappers:
                complete = False
            pending_host.add(call_id)
            wrappers[call_id] = wrapper
            host_calls += 1
            if name not in {"exec", "wait"}:
                violations.append({"name": name, "reason": "unapproved host capability"})
        elif payload.get("type") in {"custom_tool_call_output", "function_call_output"}:
            call_id = payload.get("call_id")
            if call_id not in pending_host:
                complete = False
            pending_host.discard(call_id)
            wrapper = wrappers.get(call_id)
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
        terminal = terminals.get(ident, {})
        calls.append({**request, "status": terminal.get("status") or ("error" if ident in errors or raw and raw.get("isError") else "success" if result else "incomplete"),
                      "terminal": terminal or errors.get(ident),
                      "raw_result": raw, "relay_result": relayed,
                      "raw_json_bytes": result.get("raw_json_bytes") if result else None,
                      "relay_json_bytes": result.get("relay_json_bytes") if result else None,
                      "relay_loss": raw != relayed if result else None,
                      "native_truncated": result.get("native_truncated") if result else None,
                      "client_delivery": delivery, "delivered_lines": parsed_lines, "result_after_model_exit": after_exit,
                      "duration_seconds": (terminal or result)["finished_monotonic"] - request["started_monotonic"]
                          if request.get("dispatched") and (terminal or result or {}).get("finished_monotonic") is not None else None})
    complete = complete and not pending_host
    if write_delivery:
        write_json(folder / "delivery.json", {"actual_model_output_bytes": total_bytes if rollout else None,
                   "missing_reason": None if rollout else "No real solver rollout was observed.",
                   "complete_result_deliveries": sum(c["client_delivery"]["complete"] is True for c in calls),
                   "source": "rollout.jsonl"})
    return calls, complete, host_calls if rollout else None, violations


def normalize_pi_native_calls(folder: Path, *, write_delivery=True) -> tuple[list[dict], bool, int | None, list[dict]]:
    """The wrapper owns admission IDs; repeated commands have no invented event-ID join."""
    import re
    from .pi_client import cancelled_before_admission, tool_items
    from .runner import read_live_jsonl, read_jsonl_prefix
    runtime = read_json(folder / "runtime.json")
    items = tool_items(read_live_jsonl(folder / "events.jsonl"))
    admissions = read_live_jsonl(folder / "bash-calls.jsonl")
    mode = read_json(folder / "pi-policy.json").get("measurement_mode", "direct")
    violations = [{"name": item["name"], "reason": "unapproved pi capability"} for item in items
                  if item["name"] not in ({"bash", "codemode"} if mode == "codemode" else {"bash"})]
    host_calls = len(items)
    items = [item for item in items if item["name"] == "bash"]
    finishes, _ = read_jsonl_prefix(folder / "bash-results.jsonl")
    finish_counts = collections.Counter(row["id"] for row in finishes)
    by_id = {row["id"]: row for row in finishes if finish_counts[row["id"]] == 1}
    cancellations = cancelled_before_admission(items, admissions, native=True)
    before_admission = {item["id"] for item in cancellations["calls"]}
    by_command = collections.defaultdict(list)
    for item in items:
        if item["id"] not in before_admission:
            by_command[item["input"].get("command")].append(item)
    counts = collections.Counter(row["command"] for row in admissions)
    matched = counts == collections.Counter({command: len(values) for command, values in by_command.items()})
    if not matched:
        violations.append({"reason": "wrapper admission/tool_execution_start command inventory mismatch"})
    complete = (collected_call_logs(folder, runtime, ("events.jsonl",)) and matched and not cancellations["ambiguous"]
                and read_jsonl_prefix(folder / "bash-calls.jsonl")[1]["complete"]
                and all(item["completed"] and not item.get("missing_start") for item in items))
    calls = []
    for row in admissions:
        finish = by_id.get(row["id"])
        candidates = by_command[row["command"]]
        item = candidates[0] if len(candidates) == counts[row["command"]] == 1 else None
        result = item.get("result") if item else None
        text = "\n".join(block.get("text", "") for block in (result or {}).get("content", []) if block.get("type") == "text") if result else None
        killed = bool(not finish and item and (item.get("status") == "cancelled" or
                      result and result.get("is_error") and re.search(r"(?:^|\n)Command (?:aborted|timed out after [0-9.]+ seconds)$", text or "")))
        calls.append({**row, "arm": "rg", "arguments": {"command": row["command"]}, "is_auxiliary": False,
            "status": "rejected" if not row["dispatched"] else "killed" if killed else "unknown" if result is None else "error" if result.get("is_error") else "success",
            "completed": bool(item and item["completed"]), "native_item": item,
            "raw_result": None, "relay_result": None, "raw_json_bytes": None, "relay_json_bytes": None,
            "relay_loss": None, "native_truncated": None, "delivered_lines": None,
            "duration_seconds": finish["finished_monotonic"] - finish["started_monotonic"] if finish else None,
            "duration_missing_reason": None if finish else "wrapper killed; pi timeout/abort observed" if killed else "wrapper finish unobserved or invalid",
            "native_output_bytes": len(text.encode()) if text is not None else None,
            "client_delivery": {"status": "client_stream_observed" if text is not None else "unknown", "complete": None,
                "reason": "Unique command matched; provider input unobserved." if item else "Repeated/unmatched command; no invented wrapper/event ID association."}})
    if write_delivery:
        write_json(folder / "delivery.json", {"actual_model_output_bytes": None, "complete_result_deliveries": None,
            "source": "events.jsonl", "backend": "pi", "missing_reason": "Complete subsequent provider input is not observed."})
    return calls, complete, host_calls, violations


def normalize_claude_calls(folder: Path, *, write_delivery=True) -> tuple[list[dict], bool, int | None, list[dict]]:
    from .claude_client import NATIVE_TOOLS, tool_items
    from .runner import read_live_jsonl
    events = read_live_jsonl(folder / "events.jsonl")
    runtime = read_json(folder / "runtime.json")
    is_pi = runtime.get("backend") == "pi"
    client_tool_name = lambda name: "mcp__navigation__" + name
    if is_pi:
        from .pi_client import NATIVE_TOOLS, navigation_tool_name, navigation_tool_names, tool_items
        client_tool_name = navigation_tool_name
    native = (folder / "native-agent.json").exists()
    configured = read_json(folder / ("native-agent.json" if native else "relay-settings.json"))
    items = [c for c in tool_items(events) if c.get("name") != "EndConversation"]
    allowed = set(NATIVE_TOOLS) if native else {"ToolSearch", *[client_tool_name(name) for name in configured["enabled_tools"]]}
    if is_pi:
        allowed.discard("ToolSearch")
        if read_json(folder / "pi-policy.json").get("measurement_mode") == "codemode":
            allowed.add("codemode")
            allowed.update(alias for name in configured["enabled_tools"] for alias in navigation_tool_names(name))
    violations = [{"name": c.get("name"), "reason": "unapproved agent capability"} for c in items if c.get("name") not in allowed]
    complete = collected_call_logs(folder, runtime, ("events.jsonl",)) and all(c["completed"] for c in items)
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
                          "completed": item["completed"], "dispatched": True, "native_item": item, "raw_result": None, "relay_result": None,
                          "native_output_bytes": len(text.encode()) if text is not None else None,
                          "raw_json_bytes": None, "relay_json_bytes": None, "relay_loss": None, "native_truncated": None,
                          "delivered_lines": None, "duration_seconds": None,
                          "client_delivery": {"status": "client_stream_observed" if text is not None else "unknown", "complete": None,
                                              "reason": "CLI stream is observed; the complete provider input is not observed."}})
    if not native:
        requests, results, errors, terminals, transcript_complete = relay_call_records(folder)
        if is_pi:
            from .pi_client import cancelled_before_admission
            cancellations = cancelled_before_admission(items, list(requests.values()), native=False, enabled_tools=configured["enabled_tools"])
            complete = complete and not cancellations["ambiguous"]
        complete = complete and transcript_complete and runtime.get("shutdown", {}).get("relay_cleanup", {}).get("complete", False)
        # Relay records keep the product tool name; client items carry the client's prefixed name.
        signature = lambda tool, args: (tool, json.dumps(args, sort_keys=True, ensure_ascii=False))
        request_counts = collections.Counter(signature(client_tool_name(r["tool"]), r["arguments"]) for r in requests.values())
        by_signature = collections.defaultdict(list)
        aliases = {alias: navigation_tool_name(name) for name in configured["enabled_tools"]
                   for alias in navigation_tool_names(name)} if is_pi else {}
        for item in items:
            name = item.get("name", "")
            by_signature[signature(aliases.get(name, name), item.get("input", {}))].append(item)
        # Warm host finished these RPCs but withheld the response after the client cancelled them.
        from .runner import read_jsonl_prefix
        discarded = {str(row.get("client_request_id")) for row in read_jsonl_prefix(folder / "relay.jsonl")[0]
                     if row.get("event") == "cancelled_response_discarded"}
        for ident, request in requests.items():
            result = results.get(ident)
            raw, relayed = (result.get("raw_result"), result.get("relay_result")) if result else (None, None)
            key = signature(client_tool_name(request["tool"]), request["arguments"])
            matches = by_signature[key]
            is_discarded = str(request.get("client_request_id")) in discarded
            seen = outputs.get(matches[0]["id"]) if len(matches) == request_counts[key] == 1 and not is_discarded else None
            terminal = terminals.get(ident, {})
            calls.append({**request, "status": terminal.get("status") or ("error" if ident in errors or raw and raw.get("isError") else "success" if result else "incomplete"),
                          "terminal": terminal or errors.get(ident),
                          "raw_result": raw, "relay_result": relayed,
                          "raw_json_bytes": result.get("raw_json_bytes") if result else None,
                          "relay_json_bytes": result.get("relay_json_bytes") if result else None,
                          "relay_loss": raw != relayed if result else None, "native_truncated": None,
                          "response_discarded": is_discarded,
                          "client_delivery": {"status": "client_stream_observed" if seen is not None else "unknown", "complete": None,
                                              "observed_text_bytes": len(seen.encode()) if seen is not None else None,
                                              "reason": "Unique native call matched in CLI stream; complete provider input remains unobserved."
                                              if seen is not None else "Response withheld after client cancellation." if is_discarded
                                              else "No uniquely matched native client call."},
                          "delivered_lines": None,
                          "duration_seconds": (terminal or result)["finished_monotonic"] - request["started_monotonic"]
                          if request.get("dispatched") and (terminal or result or {}).get("finished_monotonic") is not None else None})
    if write_delivery:
        write_json(folder / "delivery.json", {"actual_model_output_bytes": None, "client_stream_text_bytes": stream_bytes,
            "missing_reason": "Agent JSON events do not expose the complete subsequent provider input.",
            "complete_result_deliveries": None, "source": "events.jsonl", "backend": runtime["backend"]})
    return calls, complete, len(items) if events else None, violations


def pi_observations(folder: Path, calls: list[dict], *, call_log_complete: bool) -> dict:
    from .pi_client import assistant_messages, cancelled_before_admission, tool_items
    from .runner import read_jsonl_prefix
    events, collection = read_jsonl_prefix(folder / "events.jsonl")
    per_turn, model_calls = [], []
    guard = script_truncations = direct_calls = 0
    policy = read_json(folder / "pi-policy.json") if (folder / "pi-policy.json").exists() else {}
    for event in events:
        if event.get("type") == "turn_start":
            per_turn.append(0)
            model_calls.append(0)
        elif event.get("type") == "tool_execution_start":
            outside_script = not event.get("parentToolCallId")
            direct_calls += outside_script and event["toolName"] != "codemode"
            if per_turn:
                per_turn[-1] += event["toolName"] != "codemode"
                model_calls[-1] += outside_script
        elif event.get("type") == "tool_execution_end":
            result = event.get("result", {})
            guard += result.get("details", {}).get("outputGuard", {}).get("truncated") is True
            if event.get("toolName") == "codemode":
                script_truncations += any(block.get("text", "").startswith("Warning: truncated output (")
                                          for block in result.get("content", []))
    messages = assistant_messages(events)
    events_complete = collection["complete"] and bool(events) and events[-1].get("type") == "agent_settled"
    durations = [m.get("durationMs") for m in messages]
    known = [v for v in durations if type(v) in (int, float) and v >= 0]
    dispatched = [c for c in calls if c.get("dispatched") and not c.get("discovery")]
    tool_durations = [c["duration_seconds"] for c in dispatched if c.get("duration_seconds") is not None]
    native = (folder / "native-agent.json").exists()
    timing_complete = True
    if native:
        finishes, finish_state = read_jsonl_prefix(folder / "bash-results.jsonl")
        timing_complete = finish_state["complete"] and {row["id"] for row in finishes} <= {c["id"] for c in dispatched}
    enabled = read_json(folder / "relay-settings.json")["enabled_tools"] if (folder / "relay-settings.json").exists() else []
    admissions = read_jsonl_prefix(folder / ("bash-calls.jsonl" if native else "relay.jsonl"))[0]
    if not native:
        admissions = [row for row in admissions if row.get("event") == "attempt"]
    cancellations = cancelled_before_admission(tool_items(events), admissions, native=native, enabled_tools=enabled)
    return {"turns": len(per_turn) if events else None, "turns_complete": collection["complete"] and
                sum(e.get("type") == "turn_end" for e in events) == len(per_turn),
            "tool_calls_per_turn": per_turn, "tool_calls_per_turn_histogram": dict(collections.Counter(map(str, per_turn))),
            "model_tool_calls_per_turn": model_calls,
            "direct_calls_outside_script": direct_calls if events and policy.get("measurement_mode") == "codemode" else None,
            "cancelled_before_admission": len(cancellations["calls"]) if events else None,
            "cancelled_before_admission_calls": cancellations["calls"],
            "ambiguous_cancellation_ids": cancellations["ambiguous"],
            "call_distribution_basis": "pi tool_execution_start: direct/nested attempts excluding script envelopes; not authoritative admission",
            "event_metrics_complete": events_complete, "call_metrics_complete": call_log_complete,
            "model_seconds": sum(known) / 1000 if events_complete and messages and len(known) == len(messages) else None,
            "observed_model_seconds": sum(known) / 1000 if known else None,
            "tool_seconds": sum(tool_durations) if call_log_complete and timing_complete and len(tool_durations) == len(dispatched) else None,
            "observed_tool_seconds": sum(tool_durations),
            "tool_seconds_observed_calls": len(tool_durations),
            "tool_seconds_missing_calls": len(dispatched) - len(tool_durations),
            "timing_basis": "assistant message_end.durationMs sum; dispatched relay/wrapper durations sum (parallel intervals overlap)",
            "adapter_output_guard_truncated": guard if events else None,
            "codemode_output_truncated": script_truncations if events else None,
            "include_seen_true_calls": sum(c.get("arm") == "codemap-search" and c.get("arguments", {}).get("include_seen") is True
                                            for c in calls if not c.get("discovery")),
            "evidence": [str(folder / "events.jsonl"), str(folder / ("bash-results.jsonl" if (folder / "native-agent.json").exists() else "relay.jsonl"))]}


def finish_execution(folder: Path, entry: dict, execution: dict) -> dict:
    calls, complete, host_calls, violations = normalize_calls(folder)
    runtime = read_json(folder / "runtime.json")
    from .runner import read_jsonl_prefix, recorded_usage
    usage = recorded_usage(folder)
    collection = {name: read_jsonl_prefix(folder / name)[1] for name in ("events.jsonl", "rollout.jsonl", "relay.jsonl")
                  if (folder / name).exists() or name == "events.jsonl" or name == "rollout.jsonl" and runtime.get("backend") == "codex"}
    access = read_json(folder / ("native-agent.json" if (folder / "native-agent.json").exists() else "relay-settings.json"))
    error = execution.get("error") or runtime.get("launch_error")
    if not error and execution.get("status") == "environment_error":
        events = read_jsonl_prefix(folder / "events.jsonl")[0]
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
            "observations": pi_observations(folder, calls, call_log_complete=complete) if runtime.get("backend") == "pi" else None,
            "source_snapshot": access["source"],
            "artifacts": {name: str(folder / name) for name in ("execution.json", "runtime.json", "events.jsonl", "rollout.jsonl",
                           "relay.jsonl", "native-agent.json", "relay-settings.json", "delivery.json", "stderr.log", "command.json",
                           "claude-settings.json", "mcp-config.json", "pi-policy.json", "pi-bash.json", "pi-bash.sb", "bash-calls.jsonl", "bash-results.jsonl", "context-audit.json") if (folder / name).exists()},
            "call_log_complete": complete, "jsonl_collection": collection, "boundary_violations": violations,
            "conditions_valid": execution.get("conditions_valid", False) and not violations,
            "tool_call_counts": dict(collections.Counter(("auxiliary:" if c.get("is_auxiliary") else "native:") + c["tool"]
                                                          for c in calls if c.get("dispatched") and not c.get("discovery"))),
            "client_discovery_calls": dict(collections.Counter(c["tool"] for c in calls if c.get("discovery"))),
            "exploration_attempts": {"total": sum(not c.get("discovery") for c in calls),
                                     "dispatched": sum(bool(c.get("dispatched")) and not c.get("discovery") for c in calls),
                                     "rejected": sum(c["status"] == "rejected" and not c.get("discovery") for c in calls),
                                     "errored": sum(c["status"] == "error" and not c.get("discovery") for c in calls)}}


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
        require(len(facts) <= expected, "계획보다 많은 fact 판정")
        counts[level] = {"planned_facts": expected, "graded_facts": len(facts), "dimensions": dimensions,
                         "fact_weighted_fulfilled_rate": sum(f["state"] == "fulfilled" for f in facts) / expected
                         if len(facts) == expected and all(f["state"] != "indeterminate" for f in facts) else None}
    states = [j["core_complete"] for j in judgments]
    counts["core_complete"] = {"true": sum(v is True for v in states), "false": sum(v is False for v in states),
                                "indeterminate": sum(v == "indeterminate" for v in states), "ungraded": len(questions) - len(states),
                                "denominator": len(questions),
                                "macro_rate": sum(v is True for v in states) / len(questions)
                                if len(states) == len(questions) and "indeterminate" not in states else None}
    return counts


def report(root: Path, manifest: dict, rows: list[dict], judgments: dict) -> dict:
    dataset = routes.load_dataset()
    questions = {q["id"]: q for q in dataset["questions"]}
    planned = routes.schedule()
    by_id = {r["id"]: r for r in rows}
    require(len(by_id) == len(rows) and set(by_id) <= {r["id"] for r in planned}, "계획 밖 또는 중복 결과")
    require(set(judgments) <= set(by_id), "실행 기록 없는 채점")
    result_rows, modes = [], {}
    for slot in planned:
        row = by_id.get(slot["id"])
        judgment = judgments.get(slot["id"])
        require(not row or row.get("model") == manifest["models"]["solver"]["model"], "선택한 풀이 모델과 다른 실행 결과")
        require(not row or row.get("backend") == manifest["models"]["solver"]["backend"], "선택한 실행기와 다른 실행 결과")
        require(judgment is None or row.get("conditions_valid") is True, "유효한 실행 조건이 확인되지 않은 결과에 채점이 연결되었습니다")
        require(not row or not row.get("reused_from"), "route 프로필은 기존 A 답변을 재사용하지 않습니다")
        require(not row or all(row[key] == slot[key] for key in ("question_id", "arm", "mode", "attempt")), "결과 슬롯 식별 불일치")
        result_rows.append({**slot, "status": row["status"] if row else "planned", "judgment": judgment,
                            "solver_limits": None,
                            "elapsed_seconds": row.get("elapsed_seconds") if row else None,
                            "timing": row.get("timing") if row else None,
                            "observations": row.get("observations") if row else None,
                            "usage": row.get("usage") if row else None,
                            "tool_call_counts": row.get("tool_call_counts") if row else None,
                            **{key: row.get(key) if row else None for key in (
                                "conditions_valid", "error", "failure_reason", "condition_errors", "boundary_violations",
                                "call_log_complete", "host_calls", "artifact", "artifacts", "shutdown", "token_measurement", "native_calls",
                                "usage_collection", "token_accounting", "retry_attempts", "failed_attempts", "model", "requested_model", "backend", "agent_version", "effort_observation", "codex_version", "thread_id", "provenance",
                                "phase", "launch_state", "cli_launches", "provider_call_observed", "answer_eligible", "cleanup",
                                "failure_stage", "grade_state", "grade_reason", "grader_execution", "review", "packet_seal_sha256", "raw_seal", "raw_seal_sha256",
                                "warm_receipt", "lifecycle_seconds", "cancel_requested", "claude_persisted_tool_results")}})
    for mode in routes.MEASUREMENT_MODES:
        arms = {}
        mode_rows = [row for row in result_rows if row["mode"] == mode]
        for arm in routes.ARM_IDS:
            selected = [r for r in mode_rows if r["arm"] == arm]
            graded = [r["judgment"] for r in selected if r["judgment"] is not None]
            arms[arm] = {"planned": len(selected), "observed": sum(r["cli_launches"] == 1 for r in selected),
                         "invalid_conditions": sum(r["conditions_valid"] is False for r in selected),
                         "unconfirmed_conditions": sum(r["status"] not in {"planned", "out_of_scope"} and r["conditions_valid"] is None for r in selected),
                         # One expected judgment per planned slot (question x repetition) of this arm.
                         "quality": quality_counts(graded, [questions[r["question_id"]] for r in selected]),
                         "tool_condition": manifest.get("arms", {}).get(arm, {}).get("tool_condition")}
        modes[mode] = {"arms": arms, "quality": quality_counts(
            [r["judgment"] for r in mode_rows if r["judgment"] is not None], [questions[r["question_id"]] for r in mode_rows])}
    counts = {"planned": len(planned), "cli_started": sum(r["cli_launches"] == 1 for r in result_rows),
              "launch_reserved": sum(r["launch_state"] in {"reserved", "started", "unknown"} for r in result_rows),
              "provider_observed": sum(r["provider_call_observed"] is True for r in result_rows),
              "provider_unknown": sum(r["provider_call_observed"] is None and r["launch_state"] in {"reserved", "started", "unknown"} for r in result_rows),
              "terminal": sum(r["phase"] == "terminal" for r in result_rows),
              "graded": len(judgments), "review_required": sum(r["grade_state"] == "review_required" for r in result_rows),
              "failed": sum(r["status"] in {"preflight_failed", "warmup_failed", "environment_error", "launch_unknown"}
                            or r["grade_state"] == "failed" for r in result_rows),
              "cancelled": sum(r["status"] == "cancelled" for r in result_rows),
              "out_of_scope": sum(r["status"] == "out_of_scope" for r in result_rows),
              "not_started": sum(r["launch_state"] in {None, "not_started"} for r in result_rows),
              "launch_unknown": sum(r["launch_state"] == "unknown" for r in result_rows)}
    result = {"schema_version": 4, "solver_limits": None, "counts": counts, "profile": routes.PROFILE, "planned_solver_runs": len(routes.schedule()),
              "campaign_status": manifest.get("campaign_status"), "blocked_grading": manifest.get("blocked_grading", []),
              "run_scope": manifest.get("run_scope", {"kind": "full"}),
              "condition_identity": {key: manifest.get(key) for key in ("contract_hash", "harness_sha256", "input_hashes", "models", "actual_versions", "execution_specs")},
              "observed_solver_runs": sum(r["cli_launches"] == 1 for r in result_rows),
              "modes": modes, "rows": result_rows,
              "scope": f"{len(routes.QUESTION_IDS)} fixed routes, {routes.REPETITIONS} attempts per question and arm. Descriptive observations only.",
              "variance_estimated": False}
    write_json(root / "report/summary.json", result)
    lines = ["# Grafana 경로 벤치마크", "", "풀이 제한 없음(시간·탐색 호출·토큰). 사용량과 token accounting은 계속 기록합니다.", ""]
    if manifest.get("run_scope", {}).get("kind") == "smoke":
        lines += ["**스모크 실행입니다. 지정한 슬롯만 실행했고 나머지는 `out_of_scope`입니다. 비교 결과로 쓰지 않습니다.**",
                  "지정 슬롯: `" + ", ".join(manifest["run_scope"]["slot_ids"]) + "`", ""]
    if manifest.get("blocked_grading"):
        lines += ["**blocked campaign의 명시적 부분 채점입니다. 정리 실패/미시작 슬롯은 해제하지 않았습니다.**",
                  "차단 원인과 부분 채점 요청: `" + json.dumps(manifest["blocked_grading"], ensure_ascii=False) + "`", ""]
    if manifest.get("models"):
        models = manifest["models"]
        lines += [f"풀이: {models['solver']['model']} / {models['solver']['reasoning_effort']}; "
                  f"채점: {models['grader']['model']} / {models['grader']['reasoning_effort']}", ""]
    lines += [f"실행/검토 집계: `{json.dumps(counts, ensure_ascii=False)}`", ""]
    for mode, aggregate in modes.items():
        lines += [f"## {mode} (모드별 독립 집계)", ""]
        arms = aggregate["arms"]
        lines += ["| 도구 | 도구 구성 | 예정 | 관측 | 핵심 완전 충족 | 미채점 |", "|---|---|---:|---:|---:|---:|"]
        for arm, value in arms.items():
            complete = value["quality"]["core_complete"]
            condition = {"assisted": "필수 보조 도구 포함", "native_only": "자체 도구", "native_agent_baseline": "기본 에이전트 · 추가 MCP 없음"}.get(value["tool_condition"], "미연결")
            label = "기본 에이전트 (rg 사용 가능)" if arm == "rg" else arm
            lines.append(f"| {label} | {condition} | {value['planned']} | {value['observed']} | {complete['true'] if value['observed'] else '미실행'} | {complete['ungraded']} |")
    lines += ["", "| 문항·비교군 | 실행 상태 | launch | 채점/검토 | 조건 유효 | 호출 로그 완결 | 토큰 집계 | retry/실패 attempt | 오류·실패 사유 | 근거 |",
              "|---|---|---|---|---|---|---|---|---|---|"]
    def cell(value):
        return (json.dumps(value, ensure_ascii=False) if isinstance(value, (list, dict)) else str(value)).replace("|", "\\|").replace("\n", " ")
    for row in result_rows:
        reason = row["error"] or row["failure_reason"] or row["grade_reason"] or row["condition_errors"] or row["boundary_violations"]
        reason = reason or ("미실행" if row["status"] == "planned" else "실행 조건 확인 실패" if row["conditions_valid"] is False else "기록 없음")
        evidence = row["raw_seal"] or row["artifacts"] or row["artifact"] or "미관측"
        lines.append("| " + " | ".join(cell(v) for v in (row["id"], row["status"], row["launch_state"], row["grade_state"], row["conditions_valid"], row["call_log_complete"], row["token_accounting"],
            f"{row['retry_attempts']}/{row['failed_attempts']}", reason, evidence)) + " |")
    lines += ["", "미실행·미관측 값은 0점으로 치환하지 않습니다. 핵심과 확장, true/false/indeterminate/미채점을 별도 기록합니다.",
              f"문항·비교군·모드마다 {routes.REPETITIONS}회 반복이며 통계적 유의성을 주장하지 않습니다.",
              "제품 탐색 호출 0회인 답변은 제품 검색 기여의 근거가 아닙니다. 미관측 호출을 0회로 해석하지 않습니다."]
    lines += ["", "## 슬롯 관측 지표", "", "| 슬롯 | 턴 | 턴당 탐색 호출 | 모델 초 | 도구 초(합) | 도구 초(관측분) | 시간 누락 호출 | script 밖 직접 호출 | admission 전 취소 | adapter 절단 | script 절단 | include_seen=true | Claude 저장 결과(count/bytes) |",
              "|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|"]
    for row in result_rows:
        observed = row.get("observations") or {}
        lines.append("| " + " | ".join(cell(v) for v in (row["id"], *[observed.get(key) for key in (
            "turns", "tool_calls_per_turn", "model_seconds", "tool_seconds", "observed_tool_seconds", "tool_seconds_missing_calls",
            "direct_calls_outside_script", "cancelled_before_admission", "adapter_output_guard_truncated", "codemode_output_truncated", "include_seen_true_calls")],
            row.get("claude_persisted_tool_results"))) + " |")
    lines += ["", "모델 시간은 assistant durationMs 합, 도구 시간은 dispatch된 relay/wrapper 구간 합입니다. 병렬 구간이 겹치므로 wall time과 더하지 않습니다."]
    (root / "report/results.md").write_text("\n".join(lines) + "\n")
    return result


def publish_metrics() -> dict:
    preview = routes.PREPARATION / "report-contract"
    result = report(preview, {}, [], {})
    require(result["observed_solver_runs"] == 0 and all(r["judgment"] is None for r in result["rows"]), "가상 관측 생성")
    delivery = read_json(routes.EVIDENCE / "02-delivery.json")
    folder = Path(delivery["raw_relay_checks"]["relay_log"]).parent
    calls, complete, host_calls, violations = normalize_calls(folder, probe_only=True)
    require(complete and len(calls) == 1 and not calls[0]["relay_loss"] and host_calls is None and not violations,
            "준비 probe 관측 정규화 오류")
    return routes.publish_handoff("06-metrics.json", {
        "status": "verified_offline", "measurement_schema": FIELD_PROVENANCE,
        "result_schema": {"slot_keys": [r["id"] for r in result["rows"]], "attempts_per_slot": 1,
                          "preview": str(preview / "report/summary.json"), "preview_sha256": file_digest(preview / "report/summary.json")},
        "field_provenance": FIELD_PROVENANCE, "unknown_rules": "Preserve null/reason and dimension-level indeterminate; never fill zero.",
        "native_baseline": {"exploration_calls": "atomic pi bash wrapper admission IDs, cross-checked with tool_execution_start",
                            "delivery": "CLI output is retained separately; repeated command IDs and complete provider input may be unobserved"},
        "aggregation_rules": "Separate modes, fixed question x repetition macro denominators, separate core/extended facts; no cross-mode quality aggregation.",
        "offline_verification": {"native_probe_calls": len(calls), "relay_loss": False,
                                 "actual_solver_delivery": "unobserved", "planned_unique_slots": len(routes.schedule())},
        "observed_solver_runs": 0, "limitations": ["Real solver usage and history delivery have not been measured for either selectable model."]},
        implementation=["benchmark/route_metrics.py", "benchmark/runner.py", "benchmark/execution.py", "benchmark/claude_client.py", "benchmark/pi_client.py"],
        dependencies=["02-delivery.json", "05-grading.json"])


if __name__ == "__main__":
    print(publish_metrics()["status"])
