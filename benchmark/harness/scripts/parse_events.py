#!/usr/bin/env python3
"""Offline v4 replay; diagnostics must agree with the live reducer."""
from __future__ import annotations
import hashlib, json, pathlib, sys
from typing import Any
from v4_events import EventReducer

def main() -> int:
    if len(sys.argv)!=4: raise SystemExit("usage: parse_events.py EVENTS_JSONL WRAPPER_JSON OUTPUT_JSON")
    raw_path, wrapper_path, output_path=map(pathlib.Path,sys.argv[1:]); raw=raw_path.read_bytes() if raw_path.exists() else b"";wrapper=json.loads(wrapper_path.read_text()) if wrapper_path.exists() else {}
    accepted_limit=wrapper.get("reducer_lines_accepted",len(raw.splitlines()));reducer=EventReducer(); rows=[]
    for number,line in enumerate(raw.splitlines(),1):
        try: rows.append({"line":number,"raw":json.loads(line)})
        except (UnicodeDecodeError,json.JSONDecodeError) as error: rows.append({"line":number,"raw_text":line.decode("utf-8","replace"),"parse_error":str(error)})
        if number<=accepted_limit:reducer.record_line(line)
    limits=wrapper.get("limits",{}) if isinstance(wrapper.get("limits",{}),dict) else {}
    intentional_limits=[name for name in ("timeout","model_step_limit","output_limit") if limits.get(name) is True]
    intentional_limit=intentional_limits[0] if len(intentional_limits)==1 else None
    replay=reducer.finish(intentional_limit=intentional_limit); errors=[]
    if len(intentional_limits)>1: errors.append("multiple_intentional_limits")
    if not raw_path.exists(): errors.append("raw_events_missing")
    if not wrapper_path.exists(): errors.append("wrapper_observation_missing")
    fields=("completed_model_steps","completed_tool_completions","completed_error_tool_calls","final_assistant_text","final_model_step_reason","partial_events","limit_partial_events","protocol_failures")
    if wrapper and any(wrapper.get(key)!=replay.get(key) for key in fields): errors.append("event_replay_mismatch")
    usage=[]; seen=set()
    for row in rows[:accepted_limit]:
        raw_event=row.get("raw",{})
        if not isinstance(raw_event,dict): continue
        values=[raw_event]+[value for value in raw_event.values() if isinstance(value,dict)]
        for item in values:
            if str(item.get("type","")).replace("_","-").replace(".","-").lower()!="step-finish": continue
            session=item.get("sessionID",item.get("sessionId")); message=item.get("messageID",item.get("messageId")); part=item.get("partID",item.get("partId",item.get("id"))); tokens=item.get("tokens",item.get("usage"))
            identity=f"{session}:{message}:{part}"
            if identity not in seen and isinstance(tokens,dict): seen.add(identity); usage.append({"provenance":"official-opencode-event","event_line":row.get("line"),"step_id":identity,"tokens":tokens})
    output={"schema_version":4,"official_opencode":{"raw_reference":{"path":str(raw_path.resolve()),"sha256":hashlib.sha256(raw).hexdigest(),"line_count":len(rows)},"raw_jsonl":rows,"token_usage":usage,"replay":replay},"wrapper_observation":wrapper,"component_errors":errors,"notes":["v4 reducer is the single event interpretation; legacy combined count is diagnostic only."]}
    output_path.write_text(json.dumps(output,indent=2,sort_keys=True)+"\n")
    return 1 if errors else 0
if __name__=="__main__": raise SystemExit(main())
