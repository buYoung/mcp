#!/usr/bin/env python3
"""Execute a child process group under the v4 model-step-only contract."""
from __future__ import annotations
import json, os, pathlib, secrets, selectors, signal, subprocess, sys, time
import protocol
from v4_events import EventReducer

LIMITS=json.loads((pathlib.Path(__file__).resolve().parents[1]/"config/limits.json").read_text())
if set(LIMITS) != {"timeout_seconds","completed_model_steps","counter_semantics","total_output_bytes"} or LIMITS["counter_semantics"] != "model-steps-v4":
    raise RuntimeError("invalid v4 limits schema")
_timeout_override=os.environ.get("HARNESS_SUPERVISOR_TIMEOUT_SECONDS")
if _timeout_override is not None and os.environ.get("HARNESS_SYNTHETIC_TEST") != "1":
    raise RuntimeError("timeout override is restricted to synthetic tests")
TIMEOUT_SECONDS=int(_timeout_override or LIMITS["timeout_seconds"])
OUTPUT_LIMIT_BYTES=int(LIMITS["total_output_bytes"]); MODEL_STEP_LIMIT=int(LIMITS["completed_model_steps"])
def wait_for_paid_child_identity(
    process:subprocess.Popen[bytes],
    guardian_token:str,
    *,
    identity_fn=None,
    monotonic_fn=time.monotonic,
    sleep_fn=time.sleep,
)->bool:
    return protocol.wait_for_process_identity(
        process.pid,"guardian",guardian_token,
        is_alive_fn=lambda: process.poll() is None,
        identity_fn=identity_fn,
        monotonic_fn=monotonic_fn,
        sleep_fn=sleep_fn,
    )

def paid_child_wrapper(arguments:list[str])->int:
    if len(arguments)<2 or not arguments[0].startswith("--baseline-guardian-token="):
        raise SystemExit("invalid paid-child wrapper identity")
    guardian_token=protocol.require_identity_token(arguments[0].split("=",1)[1],"guardian")
    os.environ["BASELINE_GUARDIAN_TOKEN"]=guardian_token
    child_arguments=arguments[1:]
    if len(child_arguments)>=2 and pathlib.Path(child_arguments[0]).name=="env" and child_arguments[1]=="-i":
        child_arguments=[
            child_arguments[0],child_arguments[1],
            f"BASELINE_GUARDIAN_TOKEN={guardian_token}",*child_arguments[2:],
        ]
    process=subprocess.Popen(child_arguments)
    return process.wait()

def bootstrap_guardian(arguments:list[str])->tuple[str,list[str]]:
    markers=[item for item in arguments if item.startswith("--baseline-guardian-token=")]
    if not markers:
        token=secrets.token_hex(32); marker=protocol.token_marker("guardian",token)
        environment={**os.environ,"BASELINE_GUARDIAN_TOKEN":token}
        os.execve(sys.executable,[sys.executable,str(pathlib.Path(__file__).resolve()),*arguments,marker],environment)
        raise AssertionError("execve returned")
    if len(markers)!=1:
        raise SystemExit("ambiguous guardian identity")
    token=protocol.require_identity_token(markers[0].split("=",1)[1],"guardian")
    return token,[item for item in arguments if item!=markers[0]]

def main()->int:
    if len(sys.argv)>=3 and sys.argv[1]=="--paid-child-wrapper":
        return paid_child_wrapper(sys.argv[2:])
    guardian_token,arguments=bootstrap_guardian(sys.argv[1:])
    if len(arguments)<4: raise SystemExit("usage: session_supervisor.py RUN_DIR EVENTS STDERR COMMAND...")
    run_dir=pathlib.Path(arguments[0]); events_path=pathlib.Path(arguments[1]); stderr_path=pathlib.Path(arguments[2]); command=arguments[3:]
    os.setsid(); guardian_pid=os.getpid(); guardian_pgid=os.getpgrp()
    if guardian_pid!=guardian_pgid: raise RuntimeError("supervisor did not become a dedicated guardian group")
    started=time.time_ns();process:subprocess.Popen[bytes]|None=None
    reducer=EventReducer(); reducer_sealed=False; reducer_lines_accepted=0; state={"timeout":False,"output_limit":False,"turn_limit":False,"model_step_limit":False,"protocol_failure":False,"signal":None,"termination_cause":None}; escalation=[]; cleanup_error=None; observed={"stdout":0,"stderr":0}; kept={"stdout":0,"stderr":0}; dropped={"stdout":0,"stderr":0}; buffer=b""; truncated_tail_bytes=0; truncated_tail_reason=None
    def guardian_targets_exist()->bool:
        nonlocal cleanup_error
        try:return bool(protocol.verified_group_targets(guardian_pgid,guardian_token,guardian_pid))
        except RuntimeError as error:cleanup_error=str(error);return True
    def kill_group()->None:
        nonlocal cleanup_error
        try:escalation.extend(protocol.stop_verified_process_group(guardian_pgid,guardian_token,exclude_pid=guardian_pid))
        except RuntimeError as error:cleanup_error=str(error);state["protocol_failure"]=True
    def terminate(cause:str)->None:
        if state["termination_cause"] is None:
            state["termination_cause"]=cause; state["timeout"]|=cause=="timeout"; state["output_limit"]|=cause=="output_limit"; state["model_step_limit"]|=cause=="model_step_limit"; state["turn_limit"]|=cause=="model_step_limit"; state["protocol_failure"]|=cause=="protocol_failure"
        if process is not None:kill_group()
    def interrupted(signum:int,_frame:object)->None:
        state["signal"]=signal.Signals(signum).name;terminate("signal")
    handled=[signal.SIGINT,signal.SIGTERM]
    if hasattr(signal,"SIGHUP"):handled.append(signal.SIGHUP)
    old_handlers={item:signal.signal(item,interrupted) for item in handled}
    binding = [os.environ.get(name) for name in (
        "BASELINE_RECOVERY_LEDGER", "BASELINE_RECOVERY_GENERATION",
        "BASELINE_RECOVERY_RUN_ID", "BASELINE_RECOVERY_RECEIPT",
    )]
    if any(value is not None for value in binding):
        if not all(binding):
            raise RuntimeError("incomplete recovery process binding")
        protocol.bind_guardian([
            binding[0], binding[1], binding[2], str(run_dir), binding[3],
            str(guardian_pid),str(guardian_pgid),guardian_token,
        ])
    wrapper_command=[sys.executable,str(pathlib.Path(__file__).resolve()),"--paid-child-wrapper",protocol.token_marker("guardian",guardian_token),*command]
    process=subprocess.Popen(wrapper_command,cwd=run_dir/"source",stdout=subprocess.PIPE,stderr=subprocess.PIPE,env={**os.environ,"BASELINE_GUARDIAN_TOKEN":guardian_token})
    if os.environ.get("HARNESS_SYNTHETIC_PAUSE_AFTER_CHILD_SPAWN"):
        if os.environ.get("HARNESS_SYNTHETIC_TEST")!="1":raise RuntimeError("child-spawn pause hook is synthetic-only")
        time.sleep(60)
    if not wait_for_paid_child_identity(process,guardian_token):
        kill_group();raise RuntimeError("paid-child wrapper identity is unavailable")
    (run_dir/"child-process.json").write_text(json.dumps({
        "pid":process.pid,"guardian_pid":guardian_pid,"guardian_process_group":guardian_pgid,
        "guardian_token":guardian_token,"started_at_ns":time.time_ns(),
    },indent=2,sort_keys=True)+"\n")
    if os.environ.get("HARNESS_SYNTHETIC_SIGNAL_AFTER_SPAWN"):
        if os.environ.get("HARNESS_SYNTHETIC_TEST")!="1":raise RuntimeError("spawn-edge signal hook is synthetic-only")
        os.kill(os.getpid(),getattr(signal,os.environ["HARNESS_SYNTHETIC_SIGNAL_AFTER_SPAWN"]))
    if state["termination_cause"] is not None and guardian_targets_exist():kill_group()
    selector=selectors.DefaultSelector(); assert process.stdout and process.stderr
    selector.register(process.stdout,selectors.EVENT_READ,"stdout"); selector.register(process.stderr,selectors.EVENT_READ,"stderr")
    try:
      with events_path.open("wb") as events, stderr_path.open("wb") as errors:
       while selector.get_map() or process.poll() is None:
        if time.time_ns()-started>TIMEOUT_SECONDS*1_000_000_000: terminate("timeout")
        for key,_ in selector.select(.1):
          data=os.read(key.fileobj.fileno(),65536)
          if not data: selector.unregister(key.fileobj); continue
          stream=key.data; observed[stream]+=len(data); remaining=OUTPUT_LIMIT_BYTES-sum(kept.values()); chunk=data[:max(0,remaining)]; dropped[stream]+=len(data)-len(chunk); kept[stream]+=len(chunk)
          (events if stream=="stdout" else errors).write(chunk)
          if len(chunk)<len(data): terminate("output_limit")
          if stream=="stdout" and not reducer_sealed:
            buffer+=chunk
            while b"\n" in buffer:
              line,buffer=buffer.split(b"\n",1); reducer.record_line(line); reducer_lines_accepted+=1
              if reducer.termination_cause:
                terminate(reducer.termination_cause); reducer_sealed=True; break
        if process.poll() is not None and not selector.get_map(): break
    finally:
      if buffer:
        if state["termination_cause"] in {"timeout","model_step_limit","output_limit"}:
          # The paid agent's exact bytes stay in raw/events.jsonl, but an
          # incomplete JSONL tail created by our intentional cutoff is outside
          # the reducer boundary.  It is an observed limit outcome, not a
          # protocol defect.  A normal exit still feeds the tail to the reducer.
          truncated_tail_bytes=len(buffer); truncated_tail_reason=f"{state['termination_cause']}_incomplete_stdout_line"; reducer_sealed=True
        elif not reducer_sealed:
          reducer.record_line(buffer); reducer_lines_accepted+=1
      intentional_limit = state["termination_cause"] if state["termination_cause"] in {"timeout", "model_step_limit", "output_limit"} else None
      summary=reducer.finish(intentional_limit=intentional_limit)
      if summary["termination_cause"]: terminate(summary["termination_cause"])
      if guardian_targets_exist(): kill_group()
      for item,handler in old_handlers.items():signal.signal(item,handler)
    cleanup_satisfied=not guardian_targets_exist()
    if not cleanup_satisfied:
      state["protocol_failure"]=True
      if "process_group_remaining" not in summary["protocol_failures"]: summary["protocol_failures"].append("process_group_remaining")
    success=process.returncode==0 and bool(summary["final_assistant_text"].strip()) and summary["final_model_step_reason"]=="stop" and cleanup_satisfied and not any((state["timeout"],state["output_limit"],state["turn_limit"],state["signal"],summary["protocol_failures"]))
    ended=time.time_ns(); payload={"provenance":"wrapper-observation","command":command,"pid":process.pid,"process_group":guardian_pgid,"guardian_identity_verified":cleanup_error is None,"started_at_ns":started,"ended_at_ns":ended,"wall_time_ms":(ended-started)//1_000_000,"exit_code":process.returncode,"output_bytes":{"limit_total":OUTPUT_LIMIT_BYTES,"observed":observed,"kept":kept,"dropped":dropped},"counter_semantics":"model-steps-v4","model_step_limit":MODEL_STEP_LIMIT,"effective_timeout_seconds":TIMEOUT_SECONDS,"signal_handlers_installed_before_spawn":True,"reducer_lines_accepted":reducer_lines_accepted,"reducer_input_sealed":reducer_sealed,"truncated_tail_bytes":truncated_tail_bytes,"truncated_tail_reason":truncated_tail_reason,**summary,"completed_model_or_tool_turns":summary["legacy_combined_diagnostic"],"terminal_contract_satisfied":success,"limits":state,"cleanup_signals":escalation,"cleanup_satisfied":cleanup_satisfied,"remaining_process_group":not cleanup_satisfied}
    (run_dir/"wrapper.json").write_text(json.dumps(payload,indent=2,sort_keys=True)+"\n")
    return 0 if success else 1
if __name__=="__main__": raise SystemExit(main())
