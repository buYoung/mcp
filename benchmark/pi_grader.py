"""Tool-free pi/Astra grading; one harness launch per sealed packet."""
from __future__ import annotations

import json
import os
import selectors
import subprocess
import time
from pathlib import Path

from . import pi_client, tool_artifacts
from .core import require, write_json

PROVIDER = "openai-codex"
MODEL = "gpt-6-astra"


def observations(events: list[dict], *, complete: bool, evidence: str) -> dict:
    """Count only authoritative message_end/compaction usage, not repeated deltas."""
    messages = [e["message"] for e in events if e.get("type") == "message_end"
                and e.get("message", {}).get("role") == "assistant"]
    final = messages[-1] if messages else {}
    finished = pi_client.final_result(events) is not None
    text = "\n".join(c["text"] for c in final.get("content", []) if c.get("type") == "text")
    usage = pi_client.usage(events, terminal_complete=complete, evidence=evidence)
    retries = [e for e in events if e.get("type") in {"auto_retry_start", "auto_retry_end"}]
    compactions = [e for e in events if e.get("type") in {"compaction_start", "compaction_end"}]
    model_valid = pi_client.valid_model(events, {"provider": PROVIDER, "model": MODEL})
    tool_calls = any(e.get("type") == "tool_execution_start" for e in events) or any(
        c.get("type") == "toolCall" for m in messages for c in m.get("content", []))
    return {"finished": finished, "answer": text, "error": final.get("errorMessage"),
            "model_valid": model_valid, "tool_calls": tool_calls,
            "usage": usage, "retry_events": retries, "retry_attempts": sum(e["type"] == "auto_retry_start" for e in retries),
            "compaction_events": compactions}


def execute(folder: Path, prompt: str, *, settings: dict, output_schema: dict,
            cancel_event=None, usage_drain_seconds: float = 0, local_endpoint: str | None = None) -> dict:
    from .runner import ExecutionStop, cleanup_solver
    require(not settings.get("preparation_only"), "route 준비 전용 계약에서는 grader 실행이 금지됩니다")
    require(settings.get("grader_backend") == "pi" and settings["grader_model"] == MODEL
            and settings["grader_reasoning_effort"] == "high", "채점은 pi/openai-codex/gpt-6-astra/high로 고정합니다")
    require(usage_drain_seconds == 0 and isinstance(output_schema, dict), "채점 schema와 zero drain이 필요합니다")
    folder = folder.resolve()
    folder.mkdir(parents=True, exist_ok=True)
    workspace = folder / "workspace"
    workspace.mkdir(exist_ok=True)
    process = stop = None
    token = ""
    events, buffers, eof = [], {"stdout": b"", "stderr": b""}, set()
    error = None
    tools = {}
    settled_at = None
    parse_failed = False
    start = time.monotonic()
    deadline = start + settings["grader_timeout_seconds"]
    cleanup = {"complete": True, "started": False, "steps": []}

    with (folder / "events.jsonl").open("xb") as stdout, (folder / "stderr.log").open("xb") as stderr:
        selector = selectors.DefaultSelector()
        streams = {"stdout": stdout, "stderr": stderr}
        pending_prompt = b""

        def consume(name: str, line: bytes):
            nonlocal parse_failed, settled_at
            line = pi_client.redact(line, token)
            streams[name].write(line)
            streams[name].flush()
            if name == "stdout" and line.strip() and not parse_failed:
                try:
                    event = json.loads(line)
                    require(isinstance(event, dict), "pi event is not an object")
                    events.append(event)
                    if event.get("type") == "agent_settled":
                        settled_at = time.monotonic()
                except ValueError:
                    parse_failed = True

        def pump():
            nonlocal pending_prompt
            for key, _ in selector.select(timeout=.05):
                name = key.data
                if name == "stdin":
                    try:
                        pending_prompt = pending_prompt[os.write(key.fd, pending_prompt[:65536]):]
                    except BrokenPipeError:
                        pending_prompt = b""
                    if not pending_prompt:
                        selector.unregister(key.fileobj)
                        key.fileobj.close()
                    continue
                chunk = os.read(key.fd, 65536)
                if not chunk:
                    eof.add(name)
                    selector.unregister(key.fileobj)
                    continue
                buffers[name] += chunk
                while b"\n" in buffers[name]:
                    line, buffers[name] = buffers[name].split(b"\n", 1)
                    consume(name, line + b"\n")

        try:
            tools = tool_artifacts.pi_toolchain()
            agent, tmp = folder / "pi-agent", folder / "tmp"
            for path in (agent, tmp):
                require(not path.is_symlink(), "private pi grader state must not be a symlink")
                path.mkdir(mode=0o700, parents=True, exist_ok=True)
                path.chmod(0o700)
            write_json(agent / "settings.json", {}, exclusive=True)
            write_json(agent / "mcp.json", {"mcpServers": {}, "autoEnableCodemode": False}, exclusive=True)
            pi_client.write_models(agent, PROVIDER, local_endpoint=local_endpoint)
            token = pi_client.operator_token(PROVIDER)
            env = pi_client.environment(agent_dir=agent, tmp=tmp, provider=PROVIDER, token=token)
            pi_client.validate_models(agent, PROVIDER, local_endpoint=local_endpoint)
            args = [tools["node"], tools["pi_cli"], *pi_client.PRIVATE_FLAGS, "--provider", PROVIDER, "--model", MODEL,
                    "--thinking", "high", "--no-tools", "--no-mcp", "--no-session"]
            # The schema is part of the sealed prompt, not a CLI-specific output-schema flag.
            write_json(folder / "output-schema.json", output_schema, exclusive=True)
            (folder / "prompt.txt").write_bytes(pi_client.redact(prompt.encode(), token))
            write_json(folder / "command.json", {"args": args, "cwd": str(workspace), "backend": "pi", "provider": PROVIDER,
                "model": MODEL, "thinking": "high", "toolchain": tools, "harness_attempts": 1,
                "defaults": {"retry": "pi default (enabled, maxRetries=3)", "compaction": "pi default; occurrence invalidates grading"},
                "session_policy": "no-session; redacted event stream is authoritative"}, exclusive=True)
            if cancel_event is not None and cancel_event.is_set():
                raise ValueError("grader cancelled before launch")
            start, started_unix = time.monotonic(), time.time()
            deadline = start + settings["grader_timeout_seconds"]
            process = subprocess.Popen(args, cwd=workspace, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                       stderr=subprocess.PIPE, start_new_session=True)
            stop = ExecutionStop(folder, process, start, started_unix_seconds=started_unix, wall_deadline=deadline)
            stop.backend = "pi"
            write_json(folder / "process.json", {"pid": process.pid, "workspace": str(workspace), "started_monotonic": start,
                "started_unix_seconds": started_unix, "wall_deadline_monotonic": deadline}, exclusive=True)
            for name in ("stdout", "stderr"):
                selector.register(getattr(process, name), selectors.EVENT_READ, name)
            os.set_blocking(process.stdin.fileno(), False)
            pending_prompt = prompt.encode()
            selector.register(process.stdin, selectors.EVENT_WRITE, "stdin")
            while process.poll() is None:
                pump()
                if cancel_event is not None and cancel_event.is_set():
                    stop.cancel()
                elif parse_failed:
                    stop.request("environment_error", {})
                elif time.monotonic() >= deadline:
                    stop.request("timeout", {})
                stop.advance()
            process.wait()
            if time.monotonic() >= deadline and not stop.cutoff:
                stop.request("timeout", {}, signal_process=False)
        except (OSError, ValueError, KeyboardInterrupt) as exc:
            error = "grader interrupted" if isinstance(exc, KeyboardInterrupt) else pi_client.redact(str(exc), token)
            if stop:
                stop.request("interrupted" if isinstance(exc, KeyboardInterrupt) else "environment_error", {})
                while process.poll() is None:
                    pump()
                    stop.advance()
                process.wait()
        finally:
            cleanup = cleanup_solver(process, pump)
            drain_until = time.monotonic() + 1
            while process and len(eof) != 2 and time.monotonic() < drain_until:
                pump()
            selector.close()
            for name in streams:
                if buffers[name]:
                    consume(name, buffers[name])
                if process:
                    getattr(process, name).close()
            if process and not process.stdin.closed:
                process.stdin.close()

    collection_complete = bool(len(eof) == 2 and not buffers["stdout"] and not parse_failed and cleanup["complete"])
    observation = observations(events, complete=collection_complete and bool(process and process.returncode == 0),
                               evidence=str(folder / "events.jsonl"))
    errors = []
    if not observation["model_valid"]:
        errors.append("pi grader provider/model mismatch or missing identity")
    if observation["tool_calls"]:
        errors.append("tool call in tool-free grader")
    if observation["compaction_events"]:
        errors.append("grader compaction changed the sealed packet context")
    if not collection_complete:
        errors.append("incomplete pi grader event collection")
    if observation["usage"]["token_accounting"] not in {"complete", "lower_bound"}:
        errors.append("pi grader token accounting unconfirmed")
    termination = stop.cutoff["reason"] if stop and stop.cutoff else None
    completed = bool(observation["finished"] and process and process.returncode == 0 and not error)
    status = termination or ("completed" if completed else "environment_error")
    valid = not errors and completed and status == "completed" and settled_at is not None and settled_at <= deadline
    answer = observation["answer"] if valid else ""
    if observation["answer"] and not valid:
        (folder / "excluded-answer.txt").write_text(observation["answer"])
    (folder / "answer.txt").write_text(answer)
    result = {"status": status, "answer": answer, "answer_eligible": valid, "conditions_valid": valid,
        "condition_errors": errors, "error": error or observation["error"], "model": MODEL, "provider": PROVIDER, "backend": "pi",
        "agent_version": tools.get("versions", {}).get("pi"), "elapsed_seconds": time.monotonic() - start,
        "exit_code": process.returncode if process else None, "record_collection_complete": collection_complete,
        **{key: observation["usage"][key] for key in ("token_accounting", "failed_attempts")},
        "usage": observation["usage"], "usage_collection": {"drain_seconds": 0, "whole_cost_includes_cleanup": True},
        "harness_attempts": 1, "retry_attempts": observation["retry_attempts"], "retry_events": observation["retry_events"],
        "compaction_events": observation["compaction_events"], "artifact": str(folder / "execution.json"),
        "shutdown": {"cutoff": stop.cutoff if stop else None, "steps": stop.steps if stop else [], "solver_cleanup": cleanup},
        "effort_observation": {"requested": "high", "source": "explicit CLI --thinking; server-applied value unobserved"}}
    write_json(folder / "runtime.json", result)
    write_json(folder / "execution.json", result)
    return result
