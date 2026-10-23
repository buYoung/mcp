#!/usr/bin/env python3
"""Build an offline, event-based Pi replay from existing public observation logs.

This reads recorded data only. It never starts a model, MCP server or container.
It is not a PTY recording: unobserved token streams and reasoning are not invented.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
from collections import Counter
from datetime import datetime
from pathlib import Path

TOOL_PREFIX = "mcp__rterms_usage_6d4kwd__"
SAFE_KEY = re.compile(r"^[A-Za-z_][A-Za-z0-9_-]{0,63}$")


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def safe_arguments(value):
    """Keep normal public arguments; never publish malformed framing/deliberation keys."""
    if not isinstance(value, dict):
        return {"비정상 인자": "객체가 아닌 입력은 생략됨"}, True
    result, is_redacted = {}, False
    for key, item in value.items():
        if not SAFE_KEY.fullmatch(key):
            result[f"비정상 추가 속성 {len(result) + 1}"] = f"키 {len(key)}자 · 내용 생략"
            is_redacted = True
        else:
            result[key] = item
    return result, is_redacted


def milliseconds(timestamp: str) -> float:
    return datetime.fromisoformat(timestamp.replace("Z", "+00:00")).timestamp() * 1000


def timeout_records(root: Path) -> dict:
    records = {}
    for name in ["execution-outcomes-before-continuation.json", "execution-outcomes-after-k10.json", "execution-outcomes-after-k11.json"]:
        path = root / name
        if not path.exists():
            continue
        data = read_json(path)
        for item in data.get("failed", []) + ([data["latestFailure"]] if "latestFailure" in data else []):
            if item.get("timeoutMs"):
                records[item["caseId"]] = item["timeoutMs"]
    return records


def build_case(case: dict, root: Path, timeouts: dict) -> dict:
    case_id = case["id"]
    trace_path = root / "cases" / case_id / "trace.jsonl"
    output = {
        "id": case_id, "kind": case["kind"], "title": case.get("identifier") or case["task"].splitlines()[0],
        "repository": case.get("repository") or "빈 작업공간", "task": case["task"],
        "frames": [], "calls": 0, "rejections": 0, "supervisorCalls": 0,
        "status": "not-run", "durationMs": None, "model": None, "thinking": None,
    }
    if not trace_path.exists():
        return output
    events = [json.loads(line) for line in trace_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    if not events:
        raise ValueError(f"Empty trace: {case_id}")
    start = milliseconds(events[0]["time"])
    calls, raw_results = {}, {}
    for event in events:
        if event["event"] == "tool_call":
            calls[event["id"]] = event
        elif event["event"] == "tool_result":
            raw_results[event["id"]] = event
    arguments, redacted_calls = {}, set()
    for event in events:
        if event["event"] == "assistant_message":
            for call in event.get("calls", []):
                args, is_redacted = safe_arguments(call.get("arguments"))
                arguments[call["id"]] = args
                if is_redacted:
                    redacted_calls.add(call["id"])
    output["traceSha256"] = hashlib.sha256(trace_path.read_bytes()).hexdigest()
    output["sourceLines"] = len(events)
    output["startedAt"] = events[0]["time"]
    output["durationMs"] = round(milliseconds(events[-1]["time"]) - start)
    output["frames"].append({"kind": "prompt", "atMs": 0, "timestamp": None,
        "text": case["task"], "source": "cases.json · 원래 과제 (입력 시각 미수집)"})

    def frame(kind: str, event: dict, line: int, **values):
        output["frames"].append({"kind": kind, "atMs": max(0, round(milliseconds(event["time"]) - start)),
            "timestamp": event["time"], "source": f"cases/{case_id}/trace.jsonl:L{line}", **values})

    for line, event in enumerate(events, 1):
        kind = event["event"]
        if kind == "session_start":
            output["thinking"] = event.get("thinking")
        elif kind == "tool_availability":
            if not event.get("synonymsAvailable"):
                raise ValueError(f"Invalid setup must not be counted as an observation: {case_id}")
            frame("setup", event, line, text="새 synonyms MCP 도구 활성화 확인", toolCount=len(event.get("definitions", [])))
        elif kind == "assistant_message":
            model = f"{event.get('provider')}/{event.get('model')}"
            if model != "openai-codex/gpt-6-luna":
                raise ValueError(f"Unexpected observed model in {case_id}: {model}")
            output["model"] = model
            if event.get("text"):
                frame("assistant", event, line, text=event["text"])
            for call in event.get("calls", []):
                is_aborted = event.get("stopReason") == "aborted"
                frame("intent", event, line, callId=call["id"], tool=call["name"],
                    arguments=arguments[call["id"]], redacted=call["id"] in redacted_calls,
                    state="partial" if is_aborted else "proposed")
        elif kind == "tool_call":
            args, is_redacted = safe_arguments(event.get("input"))
            frame("dispatch", event, line, callId=event["id"], tool=event["tool"],
                arguments=args, redacted=is_redacted)
            if event["tool"].endswith("__synonyms"):
                output["calls"] += 1
            if event["tool"] == "contact_supervisor":
                output["supervisorCalls"] += 1
        elif kind == "model_visible_tool_result":
            call_id = event["id"]
            is_rejected = bool(event.get("isError")) and call_id not in calls
            if is_rejected:
                output["rejections"] += 1
            text = event.get("text", "")
            if call_id in redacted_calls:
                text = ("입력 검증 거절: root: must not have additional properties\n"
                        "[재구성 요약] 비정상 추가 속성의 내용은 공개하지 않습니다.\n"
                        "서버 실행 기록 없이 클라이언트에서 거절됐습니다.")
            raw = raw_results.get(call_id, {})
            frame("result", event, line, callId=call_id, tool=event["tool"], text=text,
                state="rejected" if is_rejected else "error" if event.get("isError") else "returned",
                durationMs=raw.get("durationMs"), redacted=call_id in redacted_calls,
                truncated="Warning: truncated output" in text or "[Output truncated" in text,
                evidencePreparing="Evidence preparing" in text,
                rawEqualsVisible=raw.get("text") == event.get("text") if raw else None)
        elif kind == "agent_end":
            reason = event.get("stopReason")
            if case_id in timeouts:
                output["status"] = "timeout"
                text = f"{timeouts[case_id] // 1000}초 제한으로 중단 · 원인 미확인"
            elif reason == "stop" and event.get("finalText"):
                output["status"] = "complete"
                text = "과제 정상 완료"
            else:
                output["status"] = "interrupted"
                text = "실행 중단 · 정상 완료로 집계하지 않음"
            frame("end", event, line, text=text, status=output["status"])
    if output["status"] == "not-run":
        raise ValueError(f"Trace has no terminal outcome: {case_id}")
    return output


def build(input_dir: Path, output: Path, template: Path):
    cases = read_json(input_dir / "cases.json")
    decision = read_json(input_dir / "observation-scope-decision.json")
    replay_cases = [build_case(case, input_dir, timeout_records(input_dir)) for case in cases]
    counts = Counter(case["status"] for case in replay_cases)
    for key, status in [("completedCaseIds", "complete"), ("timedOutCaseIds", "timeout"), ("unexecutedCaseIds", "not-run")]:
        actual = {case["id"] for case in replay_cases if case["status"] == status}
        if actual != set(decision[key]):
            raise ValueError(f"Outcome mismatch for {key}: {sorted(actual)}")
    identity = read_json(input_dir / "source-identity.json")
    data = {
        "schema": "pi-related-terms-replay/1", "cases": replay_cases, "counts": dict(counts),
        "sourceHead": identity["head"], "sourceFingerprint": identity["source_fingerprint"],
        "binarySha256": (input_dir / "binary.sha256").read_text().split()[0],
        "historical": True, "hasTokenStream": False, "hasReasoning": False,
        "omittedSetupAttempts": 1 if (input_dir / "failed-attempt-01/k01/trace.jsonl").exists() else 0,
        "note": "개선 전 실제 관측 로그의 재구성입니다. 화면 녹화가 아니며, 토큰 스트리밍·숨겨진 추론·미실행 작업은 재현하지 않습니다.",
    }
    encoded = json.dumps(data, ensure_ascii=False, separators=(",", ":"))
    # Treat logs as data, never markup: a code snippet containing </script> must
    # not terminate the JSON island or execute in the generated document.
    encoded = encoded.replace("&", "\\u0026").replace("<", "\\u003c").replace(">", "\\u003e").replace("\u2028", "\\u2028").replace("\u2029", "\\u2029")
    source = template.read_text(encoding="utf-8")
    if source.count("__REPLAY_DATA__") != 1:
        raise ValueError("Template must contain exactly one data placeholder")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(source.replace("__REPLAY_DATA__", encoded), encoding="utf-8")
    print(json.dumps({"output": str(output), "bytes": output.stat().st_size, "cases": len(replay_cases),
        "counts": dict(counts), "synonymsCalls": sum(case["calls"] for case in replay_cases),
        "clientRejections": sum(case["rejections"] for case in replay_cases),
        "redactedFrames": sum(frame.get("redacted", False) for case in replay_cases for frame in case["frames"])}, ensure_ascii=False))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--template", type=Path, default=Path(__file__).with_name("luna_replay.template.html"))
    args = parser.parse_args()
    build(args.input_dir, args.output, args.template)


if __name__ == "__main__":
    main()
