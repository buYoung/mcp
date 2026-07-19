#!/usr/bin/env python3
"""Strict, shared v4 JSONL event reducer used by supervision and replay."""
from __future__ import annotations

import json
from collections import defaultdict
from typing import Any

KNOWN = {"step-start", "step-finish", "tool", "tool-started", "tool-completed", "tool-error", "tool-use",
         "text", "message", "message-start", "message-completed"}
TOOL_DONE = {"completed", "complete", "success", "error", "failed"}
MESSAGE_DONE = {"completed", "complete", "success"}


def _candidates(envelope: dict[str, Any]) -> list[dict[str, Any]]:
    """Top-level plus immediate nested payloads; reducer identity removes mirrors."""
    values = [envelope]
    for key in ("event", "data", "part", "message"):
        value = envelope.get(key)
        if isinstance(value, dict):
            values.append(value)
    return values


def _first(values: list[dict[str, Any]], keys: tuple[str, ...]) -> Any:
    for value in reversed(values):
        for key in keys:
            if value.get(key) is not None:
                return value[key]
    return None


def _label(value: dict[str, Any]) -> str:
    return str(value.get("type", "")).lower().replace("_", "-").replace(".", "-")


def _state(value: dict[str, Any]) -> str | None:
    raw = value.get("status", value.get("state"))
    if isinstance(raw, dict): raw = raw.get("status", raw.get("type"))
    return str(raw).lower() if raw is not None else None


class EventReducer:
    """The sole v4 meaning of events; all identity is session-scoped and stable."""
    def __init__(self) -> None:
        self.model_ids: set[str] = set(); self.tool_ids: set[str] = set(); self.error_tool_ids: set[str] = set()
        self.text_ids: set[str] = set(); self.text_by_message: dict[str, list[str]] = defaultdict(list)
        self.seen: dict[str, str] = {}; self.pending_steps: set[str] = set(); self.pending_tools: set[str] = set()
        self.pending_messages: set[str] = set()
        self.protocol_failures: list[str] = []; self.unknown_events = 0; self.malformed_events = 0
        self.partial_events = 0; self.duplicate_events = 0; self.duplicate_conflicts = 0
        self.final_model_step_reason: str | None = None; self.final_message_id: str | None = None
        self.termination_cause: str | None = None; self.terminal_activity = False

    def fail(self, reason: str) -> None:
        if reason not in self.protocol_failures: self.protocol_failures.append(reason)
        if self.termination_cause is None: self.termination_cause = "protocol_failure"

    def _unique(self, identity: str, semantic: Any) -> bool:
        digest = json.dumps(semantic, separators=(",", ":"), sort_keys=True)
        old = self.seen.get(identity)
        if old is None:
            self.seen[identity] = digest; return True
        self.duplicate_events += 1
        if old != digest:
            self.duplicate_conflicts += 1; self.fail("duplicate_conflict")
        return False

    def record_line(self, line: bytes) -> None:
        try: value = json.loads(line)
        except (UnicodeDecodeError, json.JSONDecodeError):
            self.malformed_events += 1; self.fail("malformed_json"); return
        if not isinstance(value, dict):
            self.malformed_events += 1; self.fail("malformed_non_object"); return
        candidates = _candidates(value)
        labels = [_label(item) for item in candidates]
        recognized = False
        for candidate, label in zip(candidates, labels):
            if label in KNOWN:
                recognized = True; self._record(candidate, candidates, label)
        if not recognized:
            self.unknown_events += 1; self.fail("unknown_event")

    def _need(self, value: Any, name: str) -> str | None:
        if value is None or str(value) == "":
            self.malformed_events += 1; self.fail(f"malformed_missing_{name}"); return None
        return str(value)

    def _record(self, item: dict[str, Any], all_values: list[dict[str, Any]], label: str) -> None:
        session = self._need(_first(all_values, ("sessionID", "sessionId", "session_id")), "session_id")
        if session is None: return
        message = _first(all_values, ("messageID", "messageId", "message_id"))
        if message is None and label in {"message-start", "message-completed", "message"}:
            message = _first(all_values, ("id",))
        if label == "tool-use":
            # OpenCode's outer tool_use envelope is represented by its nested
            # part.type=tool payload.  It is known but never counted separately.
            return
        if label == "step-start":
            mid = self._need(message, "message_id")
            if mid and self._unique(f"{session}:step-start:{mid}", [label, mid]):
                if self.final_model_step_reason == "stop":
                    self.terminal_activity = True; self.fail("terminal_after_activity"); return
                self.pending_steps.add(f"{session}:{mid}")
            return
        if label == "step-finish":
            mid = self._need(message, "message_id"); part = self._need(_first(all_values, ("partID", "partId", "part_id", "id")), "part_id"); reason = _first(all_values, ("reason",))
            if mid is None or part is None or reason is None or str(reason).lower() not in {"stop", "tool-calls"}:
                self.malformed_events += 1; self.fail("malformed_model_step"); return
            identity = f"{session}:model:{mid}:{part}"; semantic = ["step-finish", str(reason).lower(), mid, part]
            if not self._unique(identity, semantic): return
            if self.final_model_step_reason == "stop": self.terminal_activity = True; self.fail("terminal_after_activity"); return
            self.model_ids.add(identity); self.pending_steps.discard(f"{session}:{mid}")
            self.final_model_step_reason = str(reason).lower(); self.final_message_id = f"{session}:{mid}"
            if len(self.model_ids) == 30 and self.final_model_step_reason != "stop":
                self.termination_cause = "model_step_limit"
            elif len(self.model_ids) > 30:
                self.fail("model_step_overflow")
            return
        if label in {"tool", "tool-started"}:
            call = self._need(_first(all_values, ("callID", "callId", "call_id")), "call_id")
            status = _state(item)
            if call is None: return
            if status in TOOL_DONE:
                identity = f"{session}:tool:{call}"
                if not self._unique(identity, ["tool-completion", call, status]): return
                if self.final_model_step_reason == "stop": self.terminal_activity = True; self.fail("terminal_after_activity")
                self.tool_ids.add(identity); self.pending_tools.discard(f"{session}:{call}")
                if status in {"error", "failed"}: self.error_tool_ids.add(identity)
            elif self._unique(f"{session}:tool-start:{call}", ["tool-start", call, status]):
                if self.final_model_step_reason == "stop": self.terminal_activity = True; self.fail("terminal_after_activity")
                self.pending_tools.add(f"{session}:{call}")
            return
        if label in {"tool-completed", "tool-error"}:
            call = self._need(_first(all_values, ("callID", "callId", "call_id")), "call_id"); status = _state(item)
            if call is None or status not in TOOL_DONE:
                self.malformed_events += 1; self.fail("malformed_tool_completion"); return
            identity = f"{session}:tool:{call}"
            if not self._unique(identity, ["tool-completion", call, status]): return
            if self.final_model_step_reason == "stop": self.terminal_activity = True; self.fail("terminal_after_activity")
            self.tool_ids.add(identity); self.pending_tools.discard(f"{session}:{call}")
            if status in {"error", "failed"}: self.error_tool_ids.add(identity)
            return
        if label == "text":
            if self.final_model_step_reason == "stop":
                self.terminal_activity = True; self.fail("terminal_after_activity"); return
            mid = self._need(message, "message_id"); part = self._need(_first(all_values, ("partID", "partId", "part_id", "id")), "part_id")
            text = _first(all_values, ("text",))
            if mid is None or part is None or not isinstance(text, str):
                self.malformed_events += 1; self.fail("malformed_text"); return
            identity = f"{session}:text:{mid}:{part}"
            if self._unique(identity, ["text", mid, part, text]): self.text_ids.add(identity); self.text_by_message[f"{session}:{mid}"].append(text)
            return
        if label == "message-start":
            mid = self._need(message, "message_id")
            if mid and self._unique(f"{session}:message-start:{mid}", [label, mid]):
                if self.final_model_step_reason == "stop":
                    self.terminal_activity = True; self.fail("terminal_after_activity"); return
                self.pending_messages.add(f"{session}:{mid}")
            return
        if label == "message-completed":
            mid = self._need(message, "message_id"); status = next((_state(candidate) for candidate in reversed(all_values) if _state(candidate) is not None), None)
            if mid is None or status not in TOOL_DONE:
                self.malformed_events += 1; self.fail("malformed_message_completion"); return
            message_key = f"{session}:{mid}"
            text = _first(all_values, ("text",))
            if not self._unique(f"{session}:message:{mid}", [label, mid, status, text]): return
            if message_key not in self.pending_messages:
                self.fail("unmatched_message_completion"); return
            if self.final_model_step_reason == "stop" and message_key != self.final_message_id:
                self.terminal_activity = True; self.fail("terminal_after_activity"); return
            if status not in MESSAGE_DONE:
                self.pending_messages.discard(message_key); self.fail("message_completion_error"); return
            self.pending_messages.discard(message_key)
            # Lifecycle metadata never substitutes for an explicit assistant
            # text part, even when the completion carries text.
            return
        # A bare message envelope must carry a lifecycle status.
        if label == "message":
            self.malformed_events += 1; self.fail("malformed_message_event")

    def finish(self, intentional_limit: str | None = None) -> dict[str, Any]:
        allowed_limits = {"timeout", "model_step_limit", "output_limit"}
        if intentional_limit is not None and intentional_limit not in allowed_limits:
            raise ValueError(f"unsupported intentional limit: {intentional_limit}")
        pending = {
            "step_ids": sorted(self.pending_steps),
            "tool_call_ids": sorted(self.pending_tools),
            "message_ids": sorted(self.pending_messages),
        }
        pending_count = sum(len(values) for values in pending.values())
        limit_partial_events = {
            "termination_cause": intentional_limit if pending_count and intentional_limit else None,
            "count": pending_count if intentional_limit else 0,
            **({key: values for key, values in pending.items()} if intentional_limit else {
                "step_ids": [], "tool_call_ids": [], "message_ids": [],
            }),
        }
        if pending_count:
            self.partial_events += pending_count
            if intentional_limit is None:
                self.fail("partial_event")
        final_text = "".join(self.text_by_message.get(self.final_message_id or "", []))
        return {"completed_model_steps": len(self.model_ids), "completed_model_step_ids": sorted(self.model_ids),
                "completed_tool_completions": len(self.tool_ids), "completed_tool_call_ids": sorted(self.tool_ids),
                "completed_error_tool_calls": len(self.error_tool_ids), "completed_error_tool_call_ids": sorted(self.error_tool_ids),
                "assistant_text_parts": len(self.text_ids), "final_assistant_text": final_text,
                "final_model_step_reason": self.final_model_step_reason, "unknown_events": self.unknown_events,
                "malformed_events": self.malformed_events, "partial_events": self.partial_events,
                "duplicate_events": self.duplicate_events, "duplicate_conflicts": self.duplicate_conflicts,
                "limit_partial_events": limit_partial_events,
                "protocol_failures": self.protocol_failures, "termination_cause": self.termination_cause,
                "legacy_combined_diagnostic": len(self.model_ids) + len(self.tool_ids)}
