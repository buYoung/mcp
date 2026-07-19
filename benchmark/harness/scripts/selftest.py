#!/usr/bin/env python3
"""Offline synthetic checks for retry classification and ledger ordering/concurrency."""
from __future__ import annotations

import contextlib
import concurrent.futures
import hashlib
import io
import json
import multiprocessing
import os
import pathlib
import signal
import shutil
import subprocess
import sys
import tempfile
import time

import classify_attempt
import auth_runtime
import finalize_metrics
import generation as generation_contract
import protocol
import preflight
import recover_run_directory
import render_prompt
import run_queue
import seal_artifacts
import session_supervisor
from common import BENCHMARK_ROOT, HARNESS_ROOT, PROMPT_SHA256, QUESTION_SHA256, load_json, sha256, write_json
from scheduler import schedule
from selftest_contract import EXECUTION_REQUIRED_CHECKS
from v4_events import EventReducer


def synthetic_run(root: pathlib.Path, name: str, wrapper: dict, stderr: str) -> pathlib.Path:
    run = root / name
    (run / "raw").mkdir(parents=True)
    write_json(run / "wrapper.json", wrapper)
    (run / "raw/stderr.log").write_text(stderr, encoding="utf-8")
    write_json(run / "invariants.after.json", {"generation_current": True, "source_unchanged": True, "index_unchanged": True})
    write_json(run / "postprocess-status.json", {"parser_status": 0, "required_files_present": True})
    write_json(run / "auth-cleanup.json", {"status": 0})
    return run


def prepared_attempt(root: pathlib.Path, run_id: str, classification: dict) -> pathlib.Path:
    run = root / "sealed-runs" / run_id
    run.mkdir(parents=True)
    write_json(run / "attempt-classification.json", classification)
    (run / "evidence.txt").write_text("synthetic evidence\n", encoding="utf-8")
    return run


def prepared_metrics(run: pathlib.Path, classification: dict, generation_id: str) -> pathlib.Path:
    metrics_dir = run.parent / "automatic-metrics" / run.name
    metrics_dir.mkdir(parents=True)
    (metrics_dir / "extractor.stderr.log").write_text("", encoding="utf-8")
    is_valid = classification["measurement_status"] == "valid"
    write_json(metrics_dir / "automatic-run-metrics.json", {
        "schema_version": 2,
        "run": {
            "run_id": run.name, "generation_id": generation_id,
            "integrity": {"status": "verified", "aggregation_eligible": is_valid},
        },
        "experiment": {
            "measurement_status": classification["measurement_status"],
            "published": True, "latest_published_attempt": True,
            "artifact_verified": True, "aggregation_eligible": is_valid,
        },
    })
    seal_artifacts.seal(metrics_dir)
    return metrics_dir


def partial_tail_case(root: pathlib.Path, name: str, termination: str) -> dict:
    run = root / name
    (run / "source").mkdir(parents=True)
    (run / "raw").mkdir()
    child_path = run / "emit_partial.py"
    if termination == "model_step_limit":
        child_source = '''import json,sys,time
sys.stdout.write(json.dumps({"type":"tool-started","sessionID":"synthetic","callID":"pending-tool","status":"running"},separators=(",",":"))+"\\n")
for index in range(30):
    value={"type":"step-finish","sessionID":"synthetic","messageID":f"message-{index}","partID":f"part-{index}","reason":"tool-calls"}
    sys.stdout.write(json.dumps(value,separators=(",",":"))+"\\n")
sys.stdout.write('{"type":"text"')
sys.stdout.flush()
time.sleep(5)
'''
    elif termination == "output_limit":
        child_source = '''import json,os,sys,time
sys.stdout.write(json.dumps({"type":"message-start","sessionID":"synthetic","messageID":"pending-message"},separators=(",",":"))+"\\n")
sys.stdout.flush()
remaining=2097152+65536
prefix=b'{"type":"text","text":"'
while remaining:
    chunk=prefix if remaining==2097152+65536 else b"x"*min(65536,remaining)
    written=os.write(1,chunk)
    remaining-=written
time.sleep(5)
'''
    elif termination == "timeout":
        child_source = '''import json,os,sys,time
sys.stdout.write(json.dumps({"type":"step-start","sessionID":"synthetic","messageID":"pending-step"},separators=(",",":"))+"\\n")
sys.stdout.flush()
os.write(1,b'{"type":"text"')
time.sleep(5)
'''
    elif termination == "normal_exit":
        child_source = '''import os
os.write(1,b'{"type":"text"')
'''
    elif termination == "normal_stop":
        child_source = '''import json,sys
events=[
 {"type":"text","sessionID":"synthetic","messageID":"final-message","partID":"final-text","text":"answer"},
 {"type":"step-finish","sessionID":"synthetic","messageID":"final-message","partID":"final-step","reason":"stop"},
]
for value in events: sys.stdout.write(json.dumps(value,separators=(",",":"))+"\\n")
sys.stdout.write('{"type":"text"')
sys.stdout.flush()
'''
    else:
        raise AssertionError(termination)
    child_path.write_text(child_source, encoding="utf-8")
    environment = os.environ.copy()
    environment.update({"HARNESS_SYNTHETIC_TEST": "1", "HARNESS_SUPERVISOR_TIMEOUT_SECONDS": "1" if termination == "timeout" else "10", "PYTHONDONTWRITEBYTECODE": "1"})
    supervisor = subprocess.run(
        [
            sys.executable, str(HARNESS_ROOT / "scripts/session_supervisor.py"), str(run),
            str(run / "raw/events.jsonl"), str(run / "raw/stderr.log"),
            sys.executable, str(child_path),
        ],
        env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, timeout=20,
    )
    normalized_path = run / "normalized.json"
    parser = subprocess.run(
        [
            sys.executable, str(HARNESS_ROOT / "scripts/parse_events.py"),
            str(run / "raw/events.jsonl"), str(run / "wrapper.json"), str(normalized_path),
        ],
        env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, timeout=10,
    )
    write_json(run / "invariants.after.json", {"generation_current": True, "source_unchanged": True, "index_unchanged": True})
    write_json(run / "postprocess-status.json", {"parser_status": parser.returncode, "required_files_present": True})
    write_json(run / "auth-cleanup.json", {"status": 0})
    wrapper = load_json(run / "wrapper.json")
    return {
        "supervisor_status": supervisor.returncode,
        "parser_status": parser.returncode,
        "wrapper": wrapper,
        "normalized": load_json(normalized_path),
        "classification": classify_attempt.classify(run),
        "raw": (run / "raw/events.jsonl").read_bytes(),
    }


def sigkill_owner(
    mode: str,
    ledger: pathlib.Path,
    generation_path: pathlib.Path,
    entry: dict,
    work_dir: pathlib.Path,
    result_queue: multiprocessing.Queue,
) -> None:
    os.environ["BASELINE_CLAIM_OWNER_PID"] = str(os.getpid())
    os.environ.pop("BASELINE_CLAIM_WORK_ID", None)
    os.environ.pop("BASELINE_CLAIM_AUTH_RUNTIME", None)
    work_dir.mkdir(parents=True)
    receipt = work_dir / "claim-receipt.json"
    buffer = io.StringIO()
    with contextlib.redirect_stdout(buffer):
        protocol.claim([
            str(ledger), str(generation_path), entry["task_id"], entry["trial_id"],
            entry["criterion"], "initial", str(receipt),
        ])
    claim = json.loads(buffer.getvalue()); run_id = claim["run_id"]
    process_group = None
    if mode != "claim":
        run_dir = ledger.parent / run_id
        work_dir.rename(run_dir)
        receipt = run_dir / "claim-receipt.json"
        if mode == "running":
            guardian_token = os.urandom(32).hex()
            child = subprocess.Popen(
                [sys.executable, "-c", "import time; time.sleep(60)", protocol.token_marker("guardian", guardian_token)],
                start_new_session=True,
                env={**os.environ, "BASELINE_GUARDIAN_TOKEN": guardian_token},
            )
            process_group = child.pid
            write_json(run_dir / "child-process.json", {
                "pid": child.pid, "guardian_pid": child.pid,
                "guardian_process_group": child.pid, "guardian_token": guardian_token,
                "started_at_ns": time.time_ns(),
            })
            protocol.bind_guardian([
                str(ledger), str(generation_path), run_id, str(run_dir), str(receipt),
                str(child.pid), str(child.pid), guardian_token,
            ])
        else:
            classification = {
                "measurement_status": "valid", "terminal_behavior": "stop",
                "replacement_allowed": False, "replacement_category": None,
                "generation_invalid": False, "reason": "synthetic completed attempt",
            }
            write_json(run_dir / "attempt-classification.json", classification)
            (run_dir / "evidence.txt").write_text("synthetic evidence\n", encoding="utf-8")
            protocol.terminal([
                str(ledger), str(generation_path), run_id,
                str(run_dir / "attempt-classification.json"),
            ])
            if mode == "published":
                seal_artifacts.seal(run_dir)
                protocol.publish([str(ledger), str(generation_path), run_id, str(run_dir)])
                metrics_dir = run_dir.parent / "automatic-metrics" / run_id
                metrics_dir.mkdir(parents=True)
                (metrics_dir / "partial.txt").write_text("interrupted\n", encoding="utf-8")
    result_queue.put({"run_id": run_id, "process_group": process_group})
    while True:
        time.sleep(60)


def sigkill_before_claim_commit(
    ledger: pathlib.Path,
    generation_path: pathlib.Path,
    entry: dict,
    work_dir: pathlib.Path,
) -> None:
    os.environ.update({
        "BASELINE_CLAIM_OWNER_PID": str(os.getpid()),
        "HARNESS_SYNTHETIC_TEST": "1",
        "HARNESS_SYNTHETIC_PAUSE_AFTER_CLAIM_RECEIPT": "1",
    })
    os.environ.pop("BASELINE_CLAIM_WORK_ID", None)
    os.environ.pop("BASELINE_CLAIM_AUTH_RUNTIME", None)
    work_dir.mkdir(parents=True)
    protocol.claim([
        str(ledger), str(generation_path), entry["task_id"], entry["trial_id"],
        entry["criterion"], "initial", str(work_dir / "claim-receipt.json"),
    ])


def sigkill_supervisor_after_paid_child_spawn(
    ledger: pathlib.Path,
    generation_path: pathlib.Path,
    entry: dict,
    work_dir: pathlib.Path,
    result_queue: multiprocessing.Queue,
) -> None:
    """Own one attempt whose supervisor pauses immediately after paid-child spawn."""
    os.environ["BASELINE_CLAIM_OWNER_PID"] = str(os.getpid())
    os.environ.pop("BASELINE_CLAIM_WORK_ID", None)
    os.environ.pop("BASELINE_CLAIM_AUTH_RUNTIME", None)
    work_dir.mkdir(parents=True)
    receipt = work_dir / "claim-receipt.json"
    buffer = io.StringIO()
    with contextlib.redirect_stdout(buffer):
        protocol.claim([
            str(ledger), str(generation_path), entry["task_id"], entry["trial_id"],
            entry["criterion"], "initial", str(receipt),
        ])
    claim = json.loads(buffer.getvalue()); run_id = claim["run_id"]
    run_dir = ledger.parent / run_id
    work_dir.rename(run_dir)
    (run_dir / "source").mkdir()
    (run_dir / "raw").mkdir()
    marker_path = run_dir / "paid-child-guardian-token.txt"
    paid_child = run_dir / "paid-child.py"
    paid_child.write_text(
        "import os,pathlib,time\n"
        f"pathlib.Path({str(marker_path)!r}).write_text(os.environ.get('BASELINE_GUARDIAN_TOKEN',''), encoding='utf-8')\n"
        "time.sleep(60)\n",
        encoding="utf-8",
    )
    guardian_token = os.urandom(32).hex()
    supervisor_shim = run_dir / "synthetic-supervisor.py"
    supervisor_shim.write_text(
        "import sys\n"
        f"sys.path.insert(0, {str(HARNESS_ROOT / 'scripts')!r})\n"
        "import protocol,session_supervisor\n"
        "protocol.verify_generation=lambda _path: None\n"
        "raise SystemExit(session_supervisor.main())\n",
        encoding="utf-8",
    )
    supervisor = subprocess.Popen(
        [
            sys.executable, str(supervisor_shim),
            str(run_dir), str(run_dir / "raw/events.jsonl"),
            str(run_dir / "raw/stderr.log"),
            "/usr/bin/env", "-i", "PATH=/usr/bin:/bin",
            "PYTHONDONTWRITEBYTECODE=1", sys.executable, str(paid_child),
            protocol.token_marker("guardian", guardian_token),
        ],
        env={
            **os.environ,
            "BASELINE_GUARDIAN_TOKEN": guardian_token,
            "BASELINE_RECOVERY_LEDGER": str(ledger),
            "BASELINE_RECOVERY_GENERATION": str(generation_path),
            "BASELINE_RECOVERY_RUN_ID": run_id,
            "BASELINE_RECOVERY_RECEIPT": str(run_dir / "claim-receipt.json"),
            "HARNESS_SYNTHETIC_TEST": "1",
            "HARNESS_SYNTHETIC_PAUSE_AFTER_CHILD_SPAWN": "1",
            "HARNESS_SUPERVISOR_TIMEOUT_SECONDS": "10",
            "PYTHONDONTWRITEBYTECODE": "1",
        },
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    result_queue.put({
        "run_id": run_id,
        "supervisor_pid": supervisor.pid,
        "marker_path": str(marker_path),
    })
    while True:
        time.sleep(60)


def stop_synthetic_process_group(process: subprocess.Popen | None) -> None:
    if process is None:
        return
    process_group = None
    try:
        process_group = os.getpgid(process.pid)
    except ProcessLookupError:
        pass
    if process_group is not None:
        try:
            os.killpg(process_group, signal.SIGTERM)
        except ProcessLookupError:
            pass
    try:
        process.wait(timeout=2)
        return
    except subprocess.TimeoutExpired:
        pass
    if process_group is not None:
        try:
            os.killpg(process_group, signal.SIGKILL)
        except ProcessLookupError:
            pass
    elif process.poll() is None:
        process.kill()
    process.wait(timeout=2)


def main() -> int:
    root = pathlib.Path(tempfile.mkdtemp(prefix="baseline-3x-selftest-", dir=HARNESS_ROOT / "synthetic"))
    checks = {}
    try:
        class SyntheticPaidChild:
            pid = 424242

            def __init__(self, returncodes: list[int | None]) -> None:
                self.returncodes = iter(returncodes)

            def poll(self) -> int | None:
                return next(self.returncodes)

        identity_observations = iter([False, False, True])
        identity_checks = []
        identity_sleeps = []
        delayed_identity_ready = session_supervisor.wait_for_paid_child_identity(
            SyntheticPaidChild([None, None, None]),
            "a" * 64,
            identity_fn=lambda pid, kind, token: (
                identity_checks.append((pid, kind, token)) or next(identity_observations)
            ),
            monotonic_fn=lambda: 0.0,
            sleep_fn=identity_sleeps.append,
        )
        exited_identity_checks = []
        exited_child_rejected = not session_supervisor.wait_for_paid_child_identity(
            SyntheticPaidChild([1]),
            "b" * 64,
            identity_fn=lambda *args: exited_identity_checks.append(args) or True,
            monotonic_fn=lambda: 0.0,
            sleep_fn=lambda _seconds: None,
        )
        timeout_clock = iter([0.0, 2.0])
        timeout_sleeps = []
        timed_out_child_rejected = not session_supervisor.wait_for_paid_child_identity(
            SyntheticPaidChild([None]),
            "c" * 64,
            identity_fn=lambda *_args: False,
            monotonic_fn=lambda: next(timeout_clock),
            sleep_fn=timeout_sleeps.append,
        )
        checks["paid-child-identity-readiness-retries-before-failing"] = (
            delayed_identity_ready
            and len(identity_checks) == 3
            and identity_sleeps == [0.02, 0.02]
            and exited_child_rejected
            and exited_identity_checks == []
            and timed_out_child_rejected
            and timeout_sleeps == []
        )
        real_identity_results = []
        for _index in range(32):
            real_guardian_token = os.urandom(32).hex()
            real_wrapper = subprocess.Popen(
                [
                    sys.executable,
                    str(HARNESS_ROOT / "scripts/session_supervisor.py"),
                    "--paid-child-wrapper",
                    protocol.token_marker("guardian", real_guardian_token),
                    sys.executable,
                    "-c",
                    "import time; time.sleep(2)",
                ],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
            try:
                real_identity_results.append(
                    session_supervisor.wait_for_paid_child_identity(
                        real_wrapper, real_guardian_token
                    )
                )
            finally:
                stop_synthetic_process_group(real_wrapper)
        checks["paid-child-identity-readiness-real-process-stress"] = (
            len(real_identity_results) == 32 and all(real_identity_results)
        )
        wrong_token = os.urandom(32).hex()
        actual_token = os.urandom(32).hex()
        wrong_token_wrapper = subprocess.Popen(
            [
                sys.executable,
                str(HARNESS_ROOT / "scripts/session_supervisor.py"),
                "--paid-child-wrapper",
                protocol.token_marker("guardian", actual_token),
                sys.executable,
                "-c",
                "import time; time.sleep(3)",
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        wrong_token_process_group = wrong_token_wrapper.pid
        try:
            wrong_token_rejected = not session_supervisor.wait_for_paid_child_identity(
                wrong_token_wrapper, wrong_token
            )
        finally:
            stop_synthetic_process_group(wrong_token_wrapper)
        wrong_token_remaining = [
            member for member in protocol.process_group_members(wrong_token_process_group)
            if not str(member["state"]).startswith("Z")
        ]
        original_protocol_run = protocol.subprocess.run
        def synthetic_ps_timeout(*_args, **_kwargs):
            raise subprocess.TimeoutExpired(cmd="ps", timeout=0.001)
        protocol.subprocess.run = synthetic_ps_timeout
        try:
            ps_timeout_rejected = (
                protocol.process_has_identity_token(
                    424242, "guardian", "d" * 64, timeout_seconds=0.001
                ) is None
            )
        finally:
            protocol.subprocess.run = original_protocol_run
        checks["paid-child-identity-readiness-rejects-wrong-token-and-ps-timeout"] = (
            wrong_token_rejected
            and wrong_token_remaining == []
            and ps_timeout_rejected
        )
        identity_failure_root = root / "paid-child-identity-failure-cleanup"
        (identity_failure_root / "source").mkdir(parents=True)
        (identity_failure_root / "raw").mkdir()
        identity_failure_child = identity_failure_root / "paid-child.py"
        identity_failure_child.write_text(
            "import time\ntime.sleep(60)\n",
            encoding="utf-8",
        )
        identity_failure_shim = identity_failure_root / "supervisor-shim.py"
        identity_failure_shim.write_text(
            "import sys\n"
            f"sys.path.insert(0, {str(HARNESS_ROOT / 'scripts')!r})\n"
            "import session_supervisor\n"
            "session_supervisor.wait_for_paid_child_identity=lambda *_args,**_kwargs: False\n"
            "raise SystemExit(session_supervisor.main())\n",
            encoding="utf-8",
        )
        identity_failure_guardian_token = os.urandom(32).hex()
        identity_failure_supervisor = subprocess.Popen(
            [
                sys.executable,
                str(identity_failure_shim),
                str(identity_failure_root),
                str(identity_failure_root / "raw/events.jsonl"),
                str(identity_failure_root / "raw/stderr.log"),
                sys.executable,
                str(identity_failure_child),
                protocol.token_marker("guardian", identity_failure_guardian_token),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env={
                **os.environ,
                "BASELINE_GUARDIAN_TOKEN": identity_failure_guardian_token,
                "PYTHONDONTWRITEBYTECODE": "1",
            },
        )
        identity_failure_process_group = identity_failure_supervisor.pid
        identity_failure_timed_out = False
        try:
            identity_failure_stdout, identity_failure_stderr = identity_failure_supervisor.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            identity_failure_timed_out = True
            stop_synthetic_process_group(identity_failure_supervisor)
            identity_failure_stdout, identity_failure_stderr = identity_failure_supervisor.communicate()
        identity_failure_remaining = [
            member for member in protocol.process_group_members(identity_failure_process_group)
            if not str(member["state"]).startswith("Z")
        ]
        checks["paid-child-identity-failure-product-path-cleans-process-group"] = (
            not identity_failure_timed_out
            and identity_failure_supervisor.returncode != 0
            and identity_failure_stdout == b""
            and b"paid-child wrapper identity is unavailable" in identity_failure_stderr
            and identity_failure_remaining == []
        )
        fake_bin = root / "fake-bin"
        fake_bin.mkdir()
        fake_tar = fake_bin / "tar"
        fake_tar.write_text(
            "#!/usr/bin/python3\n"
            "import sys\n"
            "sys.stderr.buffer.write(b'w' * (256 * 1024))\n"
            "sys.stderr.buffer.flush()\n"
            "sys.stdout.buffer.write(b'synthetic-tar-stream')\n",
            encoding="utf-8",
        )
        fake_tar.chmod(0o755)
        original_path = os.environ.get("PATH")
        os.environ["PATH"] = f"{fake_bin}:{original_path or ''}"
        try:
            warning_heavy_digest = generation_contract.tar_stream_sha256(root)
        finally:
            if original_path is None:
                os.environ.pop("PATH", None)
            else:
                os.environ["PATH"] = original_path
        checks["b2-tar-hash-does-not-deadlock-on-large-stderr"] = (
            warning_heavy_digest == hashlib.sha256(b"synthetic-tar-stream").hexdigest()
        )
        shell_check = subprocess.run(
            ["/bin/bash", "-n", str(HARNESS_ROOT / "scripts/run-session.sh")],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )
        checks["run-session-shell-syntax"] = shell_check.returncode == 0
        runner_text = (HARNESS_ROOT / "scripts/run-session.sh").read_text(encoding="utf-8")
        checks["run-session-uses-lane-safe-preflight"] = runner_text.count("--session-lane") == 1 and "preflight.report.json" in runner_text
        checks["run-session-verifies-execution-generation-before-auth"] = (
            runner_text.count('generation.py" verify-execution') == 1
            and runner_text.index('generation.py" verify-execution') < runner_text.index('auth_runtime.py" validate')
        )
        rendered_prompts = {
            task: render_prompt.render(
                BENCHMARK_ROOT / "questions/development" / f"{task}.json",
                QUESTION_SHA256[task],
            )
            for task in PROMPT_SHA256
        }
        checks["rendered-prompt-hash-is-exact-delivered-argv-bytes"] = (
            runner_text.count('"$(<"$RUN_DIR/prompt.txt")"') == 1
            and all(not payload.endswith(b"\n") for payload in rendered_prompts.values())
            and {
                task: hashlib.sha256(payload).hexdigest()
                for task, payload in rendered_prompts.items()
            } == PROMPT_SHA256
        )
        cleanup_trap = "trap cleanup_unclaimed EXIT"
        claimed_trap = "trap finalize_claimed_on_exit EXIT"
        trap_swap_segment = runner_text[runner_text.index("CLAIMED=1"):runner_text.index('STAGE="publish-run-directory"')]
        checks["run-session-exit-trap-is-atomically-replaced"] = (
            runner_text.index(cleanup_trap) < runner_text.index("CLAIMED=1")
            and trap_swap_segment.count(claimed_trap) == 1
            and "trap - EXIT" not in trap_swap_segment
        )
        move_window_root = root / "move-window"
        move_source = move_window_root / "work" / "prepared"
        move_target = move_window_root / "runs" / "claimed-run"
        move_source.mkdir(parents=True)
        move_target.parent.mkdir(parents=True)
        move_source.rename(move_target)
        recovered_after_move = recover_run_directory.recover(move_source, move_target)
        checks["post-move-signal-recovers-target-run-directory"] = (
            recovered_after_move == move_target.absolute()
            and recovered_after_move.is_dir()
            and not move_source.exists()
            and "recover_run_directory.py" in runner_text
        )
        quality_root = str(preflight.QUALITY_ROOT)
        accounted_attempt = [{"run_id": "other-lane", "state": "running", "guardian_process_group": 51001}]
        b1_process_lines = [
            f"51001 51001 python session_supervisor.py {quality_root}/runtime/opencode run",
            f"51002 51001 python session_supervisor.py --paid-child-wrapper {quality_root}/runtime/opencode run",
            f"51003 51001 {quality_root}/runtime/opencode run --pure",
        ]
        b1_related = preflight.related_processes(b1_process_lines)
        checks["lane-preflight-allows-one-accounted-b1-process-group"] = (
            len(b1_related) == 3 and preflight.lane_process_policy(True, b1_related, accounted_attempt)
        )
        checks["lane-preflight-rejects-unaccounted-process-group"] = not preflight.lane_process_policy(True, b1_related, [])
        b2_process_lines = b1_process_lines + [
            f"51004 51001 {quality_root}/b2/codemap-search mcp",
        ]
        b2_related = preflight.related_processes(b2_process_lines)
        checks["lane-preflight-allows-one-accounted-b2-process-group"] = (
            len(b2_related) == 4 and preflight.lane_process_policy(True, b2_related, accounted_attempt)
        )
        mixed_process_lines = b2_process_lines + [
            f"52001 52001 {quality_root}/runtime/opencode run --pure",
        ]
        mixed_related = preflight.related_processes(mixed_process_lines)
        checks["lane-preflight-rejects-mixed-process-groups"] = not preflight.lane_process_policy(
            True, mixed_related, accounted_attempt,
        )
        second_attempt = accounted_attempt + [
            {"run_id": "second-lane", "state": "claimed", "guardian_process_group": None},
        ]
        checks["lane-preflight-allows-second-active-ledger-attempt"] = preflight.lane_process_policy(
            True, b1_related, second_attempt,
        )
        third_attempt = second_attempt + [
            {"run_id": "third-lane", "state": "claimed", "guardian_process_group": None},
        ]
        checks["lane-preflight-rejects-third-active-ledger-attempt"] = not preflight.lane_process_policy(
            True, b1_related, third_attempt,
        )
        live_group = subprocess.Popen(
            [
                "/bin/bash", "-c",
                "python3 -c 'import time; time.sleep(10)' \"$1/runtime/opencode\" run & "
                "python3 -c 'import time; time.sleep(10)' \"$1/runtime/opencode\" run & "
                "python3 -c 'import time; time.sleep(10)' \"$1/runtime/opencode\" run & "
                "python3 -c 'import time; time.sleep(10)' \"$1/b2/codemap-search\" mcp & wait",
                "live-lane", quality_root,
            ],
            start_new_session=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        rogue_process = None
        try:
            live_process_group = os.getpgid(live_group.pid)
            live_related = []
            for _ in range(100):
                process_lines = subprocess.run(
                    ["ps", "-axo", "pid=,pgid=,command="],
                    text=True, stdout=subprocess.PIPE, check=True,
                ).stdout.splitlines()
                live_related = [
                    process for process in preflight.related_processes(process_lines)
                    if process["process_group"] == live_process_group
                ]
                if len(live_related) >= 4:
                    break
                time.sleep(0.01)
            live_attempt = [{
                "run_id": "live-lane", "state": "running",
                "guardian_process_group": live_process_group,
            }]
            accounted_live_group = preflight.lane_process_policy(True, live_related, live_attempt)
            rogue_process = subprocess.Popen(
                [sys.executable, "-c", "import time; time.sleep(10)", f"{quality_root}/runtime/opencode", "run"],
                start_new_session=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            rogue_group = os.getpgid(rogue_process.pid)
            rogue_related = []
            for _ in range(100):
                process_lines = subprocess.run(
                    ["ps", "-axo", "pid=,pgid=,command="],
                    text=True, stdout=subprocess.PIPE, check=True,
                ).stdout.splitlines()
                rogue_related = [
                    process for process in preflight.related_processes(process_lines)
                    if process["process_group"] == rogue_group
                ]
                if rogue_related:
                    break
                time.sleep(0.01)
            checks["lane-preflight-live-pgid-topology-is-accounted-and-foreign-rejected"] = (
                len(live_related) >= 4
                and bool(rogue_related)
                and accounted_live_group
                and not preflight.lane_process_policy(True, live_related + rogue_related, live_attempt)
            )
        finally:
            stop_synthetic_process_group(rogue_process)
            stop_synthetic_process_group(live_group)
        auth_source = root / "auth.json"
        auth_source.write_text(json.dumps({"ollama-cloud": {"type": "api", "key": "synthetic-secret"}}), encoding="utf-8")
        auth_source.chmod(0o600)
        auth_parent = root / "runtime-auth"
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
            futures = [
                executor.submit(auth_runtime.create_runtime, auth_source, "ollama-cloud", auth_parent, prefix)
                for prefix in ("session-a-", "session-b-")
            ]
            runtimes = [future.result() for future in futures]
        checks["two-auth-runtimes-coexist"] = len(set(runtimes)) == 2 and all((runtime / "opencode/auth.json").is_file() for runtime in runtimes)
        auth_runtime.remove_runtime(runtimes[0], auth_parent, "session-a-")
        checks["auth-cleanup-is-session-isolated"] = not runtimes[0].exists() and runtimes[1].exists()
        try:
            auth_runtime.remove_runtime(runtimes[1], auth_parent, "wrong-owner-")
            wrong_owner_blocked = False
        except SystemExit:
            wrong_owner_blocked = True
        checks["auth-cleanup-rejects-wrong-owner"] = wrong_owner_blocked and runtimes[1].exists()
        auth_runtime.remove_runtime(runtimes[1], auth_parent, "session-b-")
        checks["auth-runtimes-clean-independently"] = not any(path.is_dir() for path in auth_parent.iterdir())

        base = {
            "limits": {"timeout": False, "model_step_limit": False, "turn_limit": False, "output_limit": False},
            "cleanup_satisfied": True, "remaining_process_group": False, "protocol_failures": [],
            "terminal_contract_satisfied": False, "final_model_step_reason": None,
        }
        timeout = dict(base); timeout["limits"] = {**base["limits"], "timeout": True}
        result = classify_attempt.classify(synthetic_run(root, "timeout", timeout, ""))
        checks["agent-timeout-is-valid-not-retried"] = result["measurement_status"] == "valid" and not result["replacement_allowed"] and not result["generation_invalid"]
        transient = classify_attempt.classify(synthetic_run(root, "transient", base, "provider returned 503 Service Unavailable"))
        checks["transient-provider-is-replaceable"] = transient["measurement_status"] == "infrastructure_invalid" and transient["replacement_allowed"] and not transient["generation_invalid"]
        signal_results = []
        for index, stderr in enumerate(("", "provider returned 429 rate limit", "connection reset timed out", "401 unauthorized authentication failed")):
            signal_wrapper = {**base, "limits": {**base["limits"], "signal": "SIGTERM"}}
            signal_results.append(classify_attempt.classify(synthetic_run(root, f"operator-signal-{index}", signal_wrapper, stderr)))
        checks["operator-signal-is-never-transient-replaceable"] = all(
            result["measurement_status"] == "infrastructure_invalid"
            and result["replacement_allowed"] is False
            and result["replacement_category"] is None
            and result["generation_invalid"] is True
            for result in signal_results
        )
        mcp = classify_attempt.classify(synthetic_run(root, "mcp", base, "MCP codemap failed to acquire lockfile"))
        checks["mcp-failure-invalidates-generation"] = mcp["generation_invalid"] and not mcp["replacement_allowed"]
        mcp_context_messages = (
            "MCP codemap_search connection refused",
            "MCP codemap_search timed out",
            "codemap_search exited with 503 Service Unavailable",
            "mcp server CODEMAP-SEARCH: CONNECTION REFUSED",
            "Codemap search MCP transport temporarily timed out",
            "CODEMAP-SEARCH returned 503 service unavailable",
        )
        mcp_context_results = [
            classify_attempt.classify(synthetic_run(root, f"mcp-context-{index}", base, message))
            for index, message in enumerate(mcp_context_messages)
        ]
        checks["mcp-context-transients-invalidate-generation"] = all(
            result["measurement_status"] == "infrastructure_invalid"
            and result["generation_invalid"] is True
            and result["replacement_allowed"] is False
            and result["replacement_category"] is None
            for result in mcp_context_results
        )
        timeout_bad_cleanup = dict(timeout); timeout_bad_cleanup["cleanup_satisfied"] = False; timeout_bad_cleanup["remaining_process_group"] = True
        bad_cleanup_result = classify_attempt.classify(synthetic_run(root, "timeout-bad-cleanup", timeout_bad_cleanup, ""))
        checks["timeout-with-cleanup-failure-is-invalid"] = bad_cleanup_result["measurement_status"] == "infrastructure_invalid" and bad_cleanup_result["generation_invalid"]
        output_protocol = dict(base); output_protocol["limits"] = {**base["limits"], "output_limit": True}; output_protocol["protocol_failures"] = ["partial_event"]
        output_protocol_result = classify_attempt.classify(synthetic_run(root, "output-protocol", output_protocol, ""))
        checks["output-limit-with-protocol-error-is-invalid"] = output_protocol_result["measurement_status"] == "infrastructure_invalid"
        parse_run = synthetic_run(root, "parse-mismatch", {**base, "terminal_contract_satisfied": True, "final_model_step_reason": "stop"}, "")
        write_json(parse_run / "postprocess-status.json", {"parser_status": 1, "required_files_present": True})
        parse_result = classify_attempt.classify(parse_run)
        checks["stop-with-parser-failure-is-invalid"] = parse_result["measurement_status"] == "infrastructure_invalid" and parse_result["generation_invalid"]

        for termination, expected_behavior in (
            ("timeout", "timeout"),
            ("model_step_limit", "model_step_limit"),
            ("output_limit", "output_limit"),
        ):
            case = partial_tail_case(root, f"partial-tail-{termination}", termination)
            wrapper = case["wrapper"]
            limit_key = termination
            expected_pending_key = {
                "timeout": "step_ids", "model_step_limit": "tool_call_ids", "output_limit": "message_ids",
            }[termination]
            checks[f"{termination}-partial-tail-is-valid-boundary-evidence"] = (
                case["supervisor_status"] == 1
                and case["parser_status"] == 0
                and wrapper["limits"][limit_key] is True
                and wrapper["protocol_failures"] == []
                and wrapper["limit_partial_events"]["termination_cause"] == termination
                and wrapper["limit_partial_events"]["count"] == 1
                and len(wrapper["limit_partial_events"][expected_pending_key]) == 1
                and wrapper["truncated_tail_bytes"] > 0
                and wrapper["truncated_tail_reason"] == f"{termination}_incomplete_stdout_line"
                and wrapper["reducer_lines_accepted"] < len(case["raw"].splitlines())
                and not case["raw"].endswith(b"\n")
                and case["normalized"]["component_errors"] == []
                and case["classification"]["measurement_status"] == "valid"
                and case["classification"]["terminal_behavior"] == expected_behavior
                and case["classification"]["generation_invalid"] is False
            )
        checks["timeout-pending-step-is-observed-not-protocol-failure"] = (
            load_json(root / "partial-tail-timeout" / "wrapper.json")["limit_partial_events"]["step_ids"]
            == ["synthetic:pending-step"]
        )
        checks["model-step-limit-pending-tool-is-observed-not-protocol-failure"] = (
            load_json(root / "partial-tail-model_step_limit" / "wrapper.json")["limit_partial_events"]["tool_call_ids"]
            == ["synthetic:pending-tool"]
        )
        checks["output-limit-pending-message-is-observed-not-protocol-failure"] = (
            load_json(root / "partial-tail-output_limit" / "wrapper.json")["limit_partial_events"]["message_ids"]
            == ["synthetic:pending-message"]
        )

        pending_lines = {
            "step": b'{"type":"step-start","sessionID":"synthetic","messageID":"pending-step"}',
            "tool": b'{"type":"tool-started","sessionID":"synthetic","callID":"pending-tool","status":"running"}',
            "message": b'{"type":"message-start","sessionID":"synthetic","messageID":"pending-message"}',
        }
        normal_pending_failures = []
        for line in pending_lines.values():
            reducer = EventReducer(); reducer.record_line(line); normal_pending_failures.append(reducer.finish())
        checks["pending-lifecycle-normal-finish-remains-protocol-invalid"] = all(
            "partial_event" in summary["protocol_failures"]
            and summary["limit_partial_events"]["count"] == 0
            for summary in normal_pending_failures
        )
        malformed_limit_reducer = EventReducer()
        malformed_limit_reducer.record_line(pending_lines["step"])
        malformed_limit_reducer.record_line(b'{"broken"')
        malformed_limit_summary = malformed_limit_reducer.finish(intentional_limit="timeout")
        conflict_limit_reducer = EventReducer()
        conflict_limit_reducer.record_line(pending_lines["tool"])
        conflict_limit_reducer.record_line(b'{"type":"tool-started","sessionID":"synthetic","callID":"pending-tool","status":"queued"}')
        conflict_limit_summary = conflict_limit_reducer.finish(intentional_limit="output_limit")
        checks["limit-pending-lifecycle-keeps-other-protocol-failures-fatal"] = (
            malformed_limit_summary["limit_partial_events"]["count"] == 1
            and "partial_event" not in malformed_limit_summary["protocol_failures"]
            and "malformed_json" in malformed_limit_summary["protocol_failures"]
            and conflict_limit_summary["limit_partial_events"]["count"] == 1
            and "partial_event" not in conflict_limit_summary["protocol_failures"]
            and "duplicate_conflict" in conflict_limit_summary["protocol_failures"]
        )
        normal_tail = partial_tail_case(root, "partial-tail-normal-exit", "normal_exit")
        checks["normal-exit-partial-tail-remains-protocol-invalid"] = (
            normal_tail["parser_status"] == 0
            and normal_tail["wrapper"]["truncated_tail_bytes"] == 0
            and normal_tail["wrapper"]["truncated_tail_reason"] is None
            and "malformed_json" in normal_tail["wrapper"]["protocol_failures"]
            and normal_tail["wrapper"]["reducer_lines_accepted"] == len(normal_tail["raw"].splitlines())
            and normal_tail["classification"]["measurement_status"] == "infrastructure_invalid"
            and normal_tail["classification"]["generation_invalid"] is True
        )
        normal_stop_tail = partial_tail_case(root, "partial-tail-normal-stop", "normal_stop")
        checks["normal-stop-partial-tail-remains-protocol-invalid"] = (
            normal_stop_tail["parser_status"] == 0
            and normal_stop_tail["wrapper"]["final_model_step_reason"] == "stop"
            and normal_stop_tail["wrapper"]["final_assistant_text"] == "answer"
            and normal_stop_tail["wrapper"]["truncated_tail_bytes"] == 0
            and "malformed_json" in normal_stop_tail["wrapper"]["protocol_failures"]
            and normal_stop_tail["classification"]["measurement_status"] == "infrastructure_invalid"
            and normal_stop_tail["classification"]["generation_invalid"] is True
        )
        text_after_stop = EventReducer()
        for event in (
            {"type": "text", "sessionID": "synthetic", "messageID": "final", "partID": "text-1", "text": "answer"},
            {"type": "step-finish", "sessionID": "synthetic", "messageID": "final", "partID": "step-1", "reason": "stop"},
            {"type": "text", "sessionID": "synthetic", "messageID": "final", "partID": "text-2", "text": " late"},
        ):
            text_after_stop.record_line(json.dumps(event).encode("utf-8"))
        text_after_stop_summary = text_after_stop.finish()
        step_after_stop = EventReducer()
        for event in (
            {"type": "text", "sessionID": "synthetic", "messageID": "final", "partID": "text-1", "text": "answer"},
            {"type": "step-finish", "sessionID": "synthetic", "messageID": "final", "partID": "step-1", "reason": "stop"},
            {"type": "step-start", "sessionID": "synthetic", "messageID": "late"},
        ):
            step_after_stop.record_line(json.dumps(event).encode("utf-8"))
        step_after_stop_summary = step_after_stop.finish(intentional_limit="timeout")
        allowed_final_completion = EventReducer()
        for event in (
            {"type": "message-start", "sessionID": "synthetic", "messageID": "final"},
            {"type": "text", "sessionID": "synthetic", "messageID": "final", "partID": "text-1", "text": "answer"},
            {"type": "step-finish", "sessionID": "synthetic", "messageID": "final", "partID": "step-1", "reason": "stop"},
            {"type": "message-completed", "sessionID": "synthetic", "messageID": "final", "status": "completed"},
        ):
            allowed_final_completion.record_line(json.dumps(event).encode("utf-8"))
        allowed_completion_summary = allowed_final_completion.finish()
        checks["text-after-stop-is-protocol-invalid"] = (
            "terminal_after_activity" in text_after_stop_summary["protocol_failures"]
            and text_after_stop_summary["final_assistant_text"] == "answer"
            and text_after_stop_summary["termination_cause"] == "protocol_failure"
            and "terminal_after_activity" in step_after_stop_summary["protocol_failures"]
            and allowed_completion_summary["protocol_failures"] == []
            and allowed_completion_summary["final_assistant_text"] == "answer"
        )

        generation = {
            "generation_id": "synthetic-generation", "generation_seal_sha256": "synthetic-seal",
            "generation_kind": "baseline-3x", "execution_ready": True,
            "schedule": schedule(), "session_count": 84,
            "execution_policy": {"max_attempts_per_slot": 3},
        }
        generation_path = root / "generation.json"; write_json(generation_path, generation)
        planning_generation = {**generation, "generation_id": "synthetic-plan", "execution_ready": False}
        planning_path = root / "planning-generation.json"; write_json(planning_path, planning_generation)
        try:
            generation_contract.require_execution_generation(planning_generation)
            planning_contract_blocked = False
        except SystemExit:
            planning_contract_blocked = True
        checks["planning-snapshot-rejected-by-execution-contract"] = planning_contract_blocked

        planning_ledger = root / "planning-ledger.json"
        first_planning_entry = generation["schedule"][0]
        original_protocol_verify_for_plan = protocol.verify_generation
        protocol.verify_generation = lambda _: None
        try:
            try:
                protocol.claim([
                    str(planning_ledger), str(planning_path), first_planning_entry["task_id"],
                    first_planning_entry["trial_id"], first_planning_entry["criterion"], "initial",
                ])
                planning_claim_blocked = False
            except SystemExit:
                planning_claim_blocked = True
            try:
                protocol.start([str(planning_ledger), str(planning_path), "not-claimed"])
                planning_start_blocked = False
            except SystemExit:
                planning_start_blocked = True
        finally:
            protocol.verify_generation = original_protocol_verify_for_plan
        checks["planning-snapshot-rejected-by-protocol-claim-before-ledger"] = planning_claim_blocked and not planning_ledger.exists()
        checks["planning-snapshot-rejected-by-protocol-start-before-ledger"] = planning_start_blocked and not planning_ledger.exists()

        original_preflight_verify = preflight.verify_generation
        preflight.verify_generation = lambda _: None
        try:
            try:
                preflight.load_execution_generation(planning_path)
                planning_preflight_blocked = False
            except SystemExit:
                planning_preflight_blocked = True
        finally:
            preflight.verify_generation = original_preflight_verify
        checks["planning-snapshot-rejected-by-session-preflight-contract"] = planning_preflight_blocked

        original_queue_verify = run_queue.verify_generation
        original_queue_subprocess_run = run_queue.subprocess.run
        original_argv = sys.argv[:]
        original_external = os.environ.get("BASELINE_3X_EXTERNAL_APPROVED")
        original_auth_ready = os.environ.get("BASELINE_3X_AUTH_READY")
        run_queue.verify_generation = lambda _: None
        queue_subprocess_calls = []
        run_queue.subprocess.run = lambda *args, **kwargs: queue_subprocess_calls.append((args, kwargs))
        os.environ["BASELINE_3X_EXTERNAL_APPROVED"] = "1"
        os.environ["BASELINE_3X_AUTH_READY"] = "1"
        sys.argv = [str(HARNESS_ROOT / "scripts/run_queue.py"), str(planning_path)]
        try:
            try:
                run_queue.main()
                planning_queue_blocked = False
            except SystemExit:
                planning_queue_blocked = True
        finally:
            run_queue.verify_generation = original_queue_verify
            run_queue.subprocess.run = original_queue_subprocess_run
            sys.argv = original_argv
            if original_external is None:
                os.environ.pop("BASELINE_3X_EXTERNAL_APPROVED", None)
            else:
                os.environ["BASELINE_3X_EXTERNAL_APPROVED"] = original_external
            if original_auth_ready is None:
                os.environ.pop("BASELINE_3X_AUTH_READY", None)
            else:
                os.environ["BASELINE_3X_AUTH_READY"] = original_auth_ready
        checks["planning-snapshot-rejected-by-queue-before-subprocess"] = planning_queue_blocked and queue_subprocess_calls == []
        ledger = root / "ledger.json"
        original_verify = protocol.verify_generation
        protocol.verify_generation = lambda _: None
        try:
            first_pairs = []
            for pair_id in list(dict.fromkeys(row["pair_id"] for row in generation["schedule"]))[:4]:
                entries = sorted([row for row in generation["schedule"] if row["pair_id"] == pair_id], key=lambda row: row["pair_order_index"])
                first_pairs.append(entries)
            crash_ledger = root / "claim-crash-ledger.json"
            crash_work = root / "claim-crash-work"; crash_work.mkdir()
            crash_receipt = crash_work / "claim-receipt.json"
            crash_entry = first_pairs[2][0]
            original_crash_hook = os.environ.get("HARNESS_SYNTHETIC_CRASH_AFTER_CLAIM_COMMIT")
            original_synthetic_hook = os.environ.get("HARNESS_SYNTHETIC_TEST")
            os.environ["HARNESS_SYNTHETIC_CRASH_AFTER_CLAIM_COMMIT"] = "1"
            os.environ["HARNESS_SYNTHETIC_TEST"] = "1"
            try:
                try:
                    protocol.claim([
                        str(crash_ledger), str(generation_path), crash_entry["task_id"], crash_entry["trial_id"],
                        crash_entry["criterion"], "initial", str(crash_receipt),
                    ])
                    crash_observed = False
                except SystemExit:
                    crash_observed = True
            finally:
                if original_crash_hook is None:
                    os.environ.pop("HARNESS_SYNTHETIC_CRASH_AFTER_CLAIM_COMMIT", None)
                else:
                    os.environ["HARNESS_SYNTHETIC_CRASH_AFTER_CLAIM_COMMIT"] = original_crash_hook
                if original_synthetic_hook is None:
                    os.environ.pop("HARNESS_SYNTHETIC_TEST", None)
                else:
                    os.environ["HARNESS_SYNTHETIC_TEST"] = original_synthetic_hook
            committed_receipt = load_json(crash_receipt)
            committed_run_id = committed_receipt["run_id"]
            committed_ledger = load_json(crash_ledger)
            committed_before_cancel = committed_ledger["attempts"][committed_run_id]["state"] == "claimed"
            with contextlib.redirect_stdout(io.StringIO()):
                protocol.cancel_claim([str(crash_ledger), str(generation_path), str(crash_receipt), "synthetic post-commit shell crash"])
            canceled_ledger = load_json(crash_ledger)
            buffer = io.StringIO()
            with contextlib.redirect_stdout(buffer):
                protocol.claim([
                    str(crash_ledger), str(generation_path), crash_entry["task_id"], crash_entry["trial_id"],
                    crash_entry["criterion"], "initial",
                ])
            resumed_claim = json.loads(buffer.getvalue())
            resumed_ledger = load_json(crash_ledger)
            resumed_slot = resumed_ledger["slots"][f"{crash_entry['task_id']}:{crash_entry['trial_id']}:{crash_entry['criterion']}"]
            checks["claim-commit-crash-cancels-unstarted-reservation-and-resumes-slot"] = (
                crash_observed
                and committed_before_cancel
                and canceled_ledger["attempts"][committed_run_id]["state"] == "canceled-before-start"
                and resumed_claim["attempt_number"] == 1
                and resumed_slot["latest_run_id"] == resumed_claim["run_id"]
                and committed_run_id != resumed_claim["run_id"]
            )
            commit_ledger = root / "terminal-publish-crash-ledger.json"
            commit_entry = first_pairs[2][0]
            buffer = io.StringIO()
            with contextlib.redirect_stdout(buffer):
                protocol.claim([
                    str(commit_ledger), str(generation_path), commit_entry["task_id"], commit_entry["trial_id"],
                    commit_entry["criterion"], "initial",
                ])
            commit_claim = json.loads(buffer.getvalue())
            commit_run = prepared_attempt(root, commit_claim["run_id"], {"measurement_status": "valid", "replacement_allowed": False, "generation_invalid": False})
            original_synthetic_hook = os.environ.get("HARNESS_SYNTHETIC_TEST")
            os.environ["HARNESS_SYNTHETIC_TEST"] = "1"
            os.environ["HARNESS_SYNTHETIC_CRASH_AFTER_TERMINAL_COMMIT"] = "1"
            try:
                try:
                    protocol.terminal([str(commit_ledger), str(generation_path), commit_claim["run_id"], str(commit_run / "attempt-classification.json")])
                    terminal_commit_crash = False
                except SystemExit:
                    terminal_commit_crash = True
            finally:
                os.environ.pop("HARNESS_SYNTHETIC_CRASH_AFTER_TERMINAL_COMMIT", None)
            terminal_status_buffer = io.StringIO()
            with contextlib.redirect_stdout(terminal_status_buffer):
                protocol.status([str(commit_ledger), str(generation_path), commit_claim["run_id"]])
            terminal_status = json.loads(terminal_status_buffer.getvalue())
            protocol.terminal([str(commit_ledger), str(generation_path), commit_claim["run_id"], str(commit_run / "attempt-classification.json")])
            original_synthetic_hook = os.environ.get("HARNESS_SYNTHETIC_TEST")
            os.environ["HARNESS_SYNTHETIC_TEST"] = "1"
            os.environ["HARNESS_SYNTHETIC_CRASH_AFTER_ARTIFACT_SEAL_COMMIT"] = "1"
            try:
                try:
                    seal_artifacts.seal(commit_run)
                    artifact_seal_commit_crash = False
                except SystemExit:
                    artifact_seal_commit_crash = True
            finally:
                os.environ.pop("HARNESS_SYNTHETIC_CRASH_AFTER_ARTIFACT_SEAL_COMMIT", None)
                if original_synthetic_hook is None:
                    os.environ.pop("HARNESS_SYNTHETIC_TEST", None)
                else:
                    os.environ["HARNESS_SYNTHETIC_TEST"] = original_synthetic_hook
            resealed_manifest = seal_artifacts.seal(commit_run)
            checks["artifact-seal-commit-crash-is-idempotent-and-publishable"] = (
                artifact_seal_commit_crash
                and resealed_manifest == load_json(commit_run / "artifact-manifest.json")
                and not any(
                    path.stat().st_mode & 0o222
                    for path in [commit_run, *commit_run.rglob("*")]
                    if not path.is_symlink()
                )
            )
            safe_symlink_run = root / "safe-internal-symlink"
            safe_symlink_run.mkdir()
            safe_target = safe_symlink_run / "target.txt"
            safe_target.write_text("bound target\n", encoding="utf-8")
            (safe_symlink_run / "link.txt").symlink_to("./target.txt")
            nested_manifest = safe_symlink_run / "nested/artifact-manifest.json"
            nested_manifest.parent.mkdir()
            nested_manifest.write_text("nested evidence\n", encoding="utf-8")
            safe_symlink_manifest = seal_artifacts.seal(safe_symlink_run)
            safe_symlink_verified = protocol.verify_artifact_seal(
                safe_symlink_run, ("link.txt", "target.txt"),
            ) == safe_symlink_manifest
            safe_target.chmod(safe_target.stat().st_mode | 0o200)
            safe_target.write_text("changed target\n", encoding="utf-8")
            try:
                protocol.verify_artifact_seal(safe_symlink_run, ("link.txt", "target.txt"))
                safe_symlink_drift_rejected = False
            except SystemExit:
                safe_symlink_drift_rejected = True
            checks["artifact-seal-safe-internal-symlink-is-bound"] = (
                safe_symlink_verified
                and safe_symlink_manifest["artifacts"]["link.txt"].get("symlink_target") == "./target.txt"
                and "nested/artifact-manifest.json" in safe_symlink_manifest["artifacts"]
                and safe_symlink_drift_rejected
            )

            outside_target = root / "outside-symlink-target.txt"
            outside_target.write_text("outside\n", encoding="utf-8")
            unsafe_symlink_cases: list[pathlib.Path] = []
            for case_name in ("absolute", "escape", "broken", "directory", "cycle"):
                case_root = root / f"unsafe-symlink-{case_name}"
                case_root.mkdir()
                if case_name == "absolute":
                    (case_root / "link").symlink_to(outside_target)
                elif case_name == "escape":
                    (case_root / "link").symlink_to("../outside-symlink-target.txt")
                elif case_name == "broken":
                    (case_root / "link").symlink_to("missing.txt")
                elif case_name == "directory":
                    (case_root / "target").mkdir()
                    (case_root / "link").symlink_to("target")
                else:
                    (case_root / "one").symlink_to("two")
                    (case_root / "two").symlink_to("one")
                unsafe_symlink_cases.append(case_root)
            unsafe_symlink_rejections = []
            for case_root in unsafe_symlink_cases:
                try:
                    seal_artifacts.seal(case_root)
                    unsafe_symlink_rejections.append(False)
                except SystemExit:
                    unsafe_symlink_rejections.append(True)
            checks["artifact-seal-unsafe-symlink-is-rejected"] = all(unsafe_symlink_rejections)
            nonregular_root = root / "nonregular-artifact-entry"
            nonregular_root.mkdir()
            os.mkfifo(nonregular_root / "fifo")
            try:
                seal_artifacts.seal(nonregular_root)
                nonregular_rejected = False
            except SystemExit:
                nonregular_rejected = True
            checks["artifact-seal-nonregular-entry-is-rejected"] = nonregular_rejected
            os.environ["HARNESS_SYNTHETIC_CRASH_AFTER_PUBLISH_COMMIT"] = "1"
            try:
                try:
                    protocol.publish([str(commit_ledger), str(generation_path), commit_claim["run_id"], str(commit_run)])
                    publish_commit_crash = False
                except SystemExit:
                    publish_commit_crash = True
            finally:
                os.environ.pop("HARNESS_SYNTHETIC_CRASH_AFTER_PUBLISH_COMMIT", None)
                if original_synthetic_hook is None:
                    os.environ.pop("HARNESS_SYNTHETIC_TEST", None)
                else:
                    os.environ["HARNESS_SYNTHETIC_TEST"] = original_synthetic_hook
            publish_status_buffer = io.StringIO()
            with contextlib.redirect_stdout(publish_status_buffer):
                protocol.status([str(commit_ledger), str(generation_path), commit_claim["run_id"]])
            publish_status = json.loads(publish_status_buffer.getvalue())
            protocol.publish([str(commit_ledger), str(generation_path), commit_claim["run_id"], str(commit_run)])
            commit_metrics = prepared_metrics(commit_run, {"measurement_status": "valid", "replacement_allowed": False, "generation_invalid": False}, generation["generation_id"])
            protocol.metrics([str(commit_ledger), str(generation_path), commit_claim["run_id"], str(commit_run), str(commit_metrics)])
            final_status_buffer = io.StringIO()
            with contextlib.redirect_stdout(final_status_buffer):
                protocol.status([str(commit_ledger), str(generation_path), commit_claim["run_id"]])
            final_status = json.loads(final_status_buffer.getvalue())
            checks["terminal-commit-crash-is-idempotent-and-finalizable"] = (
                terminal_commit_crash and terminal_status["state"] == "terminal-unsealed" and terminal_status["terminal_recorded"] is True
            )
            checks["publish-commit-crash-is-idempotent-and-metrics-resumable"] = (
                publish_commit_crash
                and publish_status["state"] == "terminal"
                and publish_status["published"] is True
                and publish_status["metrics_status"] == "pending"
                and final_status["metrics_status"] == "sealed"
            )
            claimed = []
            for entries in first_pairs[:3]:
                entry = entries[0]
                buffer = io.StringIO()
                with contextlib.redirect_stdout(buffer):
                    protocol.claim([str(ledger), str(generation_path), entry["task_id"], entry["trial_id"], entry["criterion"], "initial"])
                claimed.append(json.loads(buffer.getvalue()))
            fourth = first_pairs[3][0]
            try:
                with contextlib.redirect_stdout(io.StringIO()):
                    protocol.claim([str(ledger), str(generation_path), fourth["task_id"], fourth["trial_id"], fourth["criterion"], "initial"])
                fourth_blocked = False
            except SystemExit:
                fourth_blocked = True
            checks["ledger-blocks-fourth-concurrent-pair"] = fourth_blocked

            valid = {"measurement_status": "valid", "replacement_allowed": False, "generation_invalid": False}
            valid_run = prepared_attempt(root, claimed[0]["run_id"], valid)
            protocol.terminal([str(ledger), str(generation_path), claimed[0]["run_id"], str(valid_run / "attempt-classification.json")])
            successor = first_pairs[0][1]
            try:
                with contextlib.redirect_stdout(io.StringIO()):
                    protocol.claim([str(ledger), str(generation_path), successor["task_id"], successor["trial_id"], successor["criterion"], "initial"])
                unsealed_successor_blocked = False
            except SystemExit:
                unsealed_successor_blocked = True
            checks["unsealed-terminal-does-not-release-successor"] = unsealed_successor_blocked
            try:
                protocol.publish([str(ledger), str(generation_path), claimed[0]["run_id"], str(valid_run)])
                unsealed_publish_blocked = False
            except SystemExit:
                unsealed_publish_blocked = True
            checks["unsealed-artifacts-cannot-be-published"] = unsealed_publish_blocked
            seal_artifacts.seal(valid_run)
            protocol.publish([str(ledger), str(generation_path), claimed[0]["run_id"], str(valid_run)])
            try:
                with contextlib.redirect_stdout(io.StringIO()):
                    protocol.claim([str(ledger), str(generation_path), successor["task_id"], successor["trial_id"], successor["criterion"], "initial"])
                pending_metrics_successor_blocked = False
            except SystemExit:
                pending_metrics_successor_blocked = True
            checks["published-run-with-pending-metrics-does-not-release-successor"] = pending_metrics_successor_blocked
            valid_metrics = prepared_metrics(valid_run, valid, generation["generation_id"])
            protocol.metrics([str(ledger), str(generation_path), claimed[0]["run_id"], str(valid_run), str(valid_metrics)])
            buffer = io.StringIO()
            with contextlib.redirect_stdout(buffer):
                protocol.claim([str(ledger), str(generation_path), successor["task_id"], successor["trial_id"], successor["criterion"], "initial"])
            checks["successor-allowed-only-after-valid-predecessor"] = bool(json.loads(buffer.getvalue())["run_id"])

            transient = {"measurement_status": "infrastructure_invalid", "replacement_allowed": True, "generation_invalid": False}
            transient_run = prepared_attempt(root, claimed[1]["run_id"], transient)
            protocol.terminal([str(ledger), str(generation_path), claimed[1]["run_id"], str(transient_run / "attempt-classification.json")])
            seal_artifacts.seal(transient_run)
            protocol.publish([str(ledger), str(generation_path), claimed[1]["run_id"], str(transient_run)])
            replacement_entry = first_pairs[1][0]
            try:
                with contextlib.redirect_stdout(io.StringIO()):
                    protocol.claim([str(ledger), str(generation_path), replacement_entry["task_id"], replacement_entry["trial_id"], replacement_entry["criterion"], "replacement"])
                pending_metrics_replacement_blocked = False
            except SystemExit:
                pending_metrics_replacement_blocked = True
            checks["published-transient-with-pending-metrics-does-not-release-replacement"] = pending_metrics_replacement_blocked
            transient_metrics = prepared_metrics(transient_run, transient, generation["generation_id"])
            protocol.metrics([str(ledger), str(generation_path), claimed[1]["run_id"], str(transient_run), str(transient_metrics)])
            buffer = io.StringIO()
            with contextlib.redirect_stdout(buffer):
                protocol.claim([str(ledger), str(generation_path), replacement_entry["task_id"], replacement_entry["trial_id"], replacement_entry["criterion"], "replacement"])
            replacement_two = json.loads(buffer.getvalue())
            checks["transient-attempt-retained-and-replacement-numbered"] = replacement_two["attempt_number"] == 2 and len(json.load(open(ledger))["slots"][f"{replacement_entry['task_id']}:{replacement_entry['trial_id']}:{replacement_entry['criterion']}"]["all_run_ids"]) == 2
            replacement_two_run = prepared_attempt(root, replacement_two["run_id"], transient)
            protocol.terminal([str(ledger), str(generation_path), replacement_two["run_id"], str(replacement_two_run / "attempt-classification.json")])
            seal_artifacts.seal(replacement_two_run)
            protocol.publish([str(ledger), str(generation_path), replacement_two["run_id"], str(replacement_two_run)])
            replacement_two_metrics = prepared_metrics(replacement_two_run, transient, generation["generation_id"])
            protocol.metrics([str(ledger), str(generation_path), replacement_two["run_id"], str(replacement_two_run), str(replacement_two_metrics)])
            buffer = io.StringIO()
            with contextlib.redirect_stdout(buffer):
                protocol.claim([str(ledger), str(generation_path), replacement_entry["task_id"], replacement_entry["trial_id"], replacement_entry["criterion"], "replacement"])
            replacement_three = json.loads(buffer.getvalue())
            replacement_three_run = prepared_attempt(root, replacement_three["run_id"], transient)
            protocol.terminal([str(ledger), str(generation_path), replacement_three["run_id"], str(replacement_three_run / "attempt-classification.json")])
            seal_artifacts.seal(replacement_three_run)
            protocol.publish([str(ledger), str(generation_path), replacement_three["run_id"], str(replacement_three_run)])
            replacement_three_metrics = prepared_metrics(replacement_three_run, transient, generation["generation_id"])
            protocol.metrics([str(ledger), str(generation_path), replacement_three["run_id"], str(replacement_three_run), str(replacement_three_metrics)])
            try:
                with contextlib.redirect_stdout(io.StringIO()):
                    protocol.claim([str(ledger), str(generation_path), replacement_entry["task_id"], replacement_entry["trial_id"], replacement_entry["criterion"], "replacement"])
                fourth_attempt_blocked = False
            except SystemExit:
                fourth_attempt_blocked = True
            checks["replacement-budget-is-exactly-two"] = replacement_three["attempt_number"] == 3 and fourth_attempt_blocked

            decision_ledger = load_json(ledger)
            mode, _ = run_queue.slot_decision(generation, decision_ledger, replacement_entry)
            checks["queue-resume-stops-after-replacement-budget"] = mode == "stop"
            valid_mode, _ = run_queue.slot_decision(generation, decision_ledger, first_pairs[0][0])
            checks["queue-resume-skips-sealed-valid-slot"] = valid_mode == "skip"
            fresh_mode, _ = run_queue.slot_decision(generation, decision_ledger, first_pairs[3][0])
            checks["queue-resume-starts-only-unsampled-slot"] = fresh_mode == "initial"
            all_initial = {
                run_queue.slot_decision(generation, {
                    "generation_id": generation["generation_id"],
                    "generation_seal_sha256": generation["generation_seal_sha256"],
                    "state": "active", "slots": {}, "attempts": {},
                }, entry)[0]
                for entry in generation["schedule"]
            }
            checks["queue-plans-all-84-sealed-slots-without-repeats"] = len(generation["schedule"]) == 84 and all_initial == {"initial"}
            published_count, sealed_count = run_queue.valid_slot_counts({"slots": {
                "published": {"measurement_status": "valid", "metrics_status": "pending"},
                "sealed": {"measurement_status": "valid", "metrics_status": "sealed"},
                "invalid": {"measurement_status": "infrastructure_invalid", "metrics_status": "sealed"},
            }})
            checks["queue-summary-distinguishes-published-and-sealed-valid"] = (
                published_count == 2 and sealed_count == 1
            )
            observed_waits = []
            notifications = []
            wait_two = run_queue.apply_replacement_backoff(1, "transient_provider", sleep_fn=observed_waits.append, notify_fn=notifications.append)
            wait_three = run_queue.apply_replacement_backoff(2, "transient_network", sleep_fn=observed_waits.append, notify_fn=notifications.append)
            checks["replacement-backoff-is-bounded-deterministic-and-testable"] = (
                (wait_two, wait_three) == (5, 15)
                and observed_waits == [5, 15]
                and all("replacement_category" in json.loads(message) for message in notifications)
            )

            concurrent_ledger = root / "concurrent-ledger.json"
            context = multiprocessing.get_context("fork")
            start_event = context.Event(); result_queue = context.Queue()

            def claim_child(entry: dict) -> None:
                start_event.wait()
                try:
                    buffer = io.StringIO()
                    with contextlib.redirect_stdout(buffer):
                        protocol.claim([str(concurrent_ledger), str(generation_path), entry["task_id"], entry["trial_id"], entry["criterion"], "initial"])
                    result_queue.put((True, json.loads(buffer.getvalue())["run_id"]))
                except BaseException as error:
                    result_queue.put((False, repr(error)))

            processes = [context.Process(target=claim_child, args=(entries[0],)) for entries in first_pairs[:3]]
            for process in processes: process.start()
            start_event.set()
            for process in processes: process.join(10)
            concurrent_results = [result_queue.get(timeout=2) for _ in processes]
            concurrent_value = load_json(concurrent_ledger)
            active_count = sum(item["state"] in {"claimed", "running"} for item in concurrent_value["attempts"].values())
            checks["multiprocess-three-claims-no-false-busy"] = all(ok for ok, _ in concurrent_results) and all(process.exitcode == 0 for process in processes)
            checks["multiprocess-ledger-json-intact-and-max-three"] = active_count == 3 and concurrent_value["max_concurrency"] == 3
            try:
                with contextlib.redirect_stdout(io.StringIO()):
                    protocol.claim([str(concurrent_ledger), str(generation_path), fourth["task_id"], fourth["trial_id"], fourth["criterion"], "initial"])
                concurrent_fourth_blocked = False
            except SystemExit:
                concurrent_fourth_blocked = True
            checks["multiprocess-fourth-active-claim-policy-blocked"] = concurrent_fourth_blocked

            terminal_event = context.Event(); terminal_queue = context.Queue()
            concurrent_valid = root / "concurrent-valid.json"; write_json(concurrent_valid, valid)

            def terminal_child(run_id: str) -> None:
                terminal_event.wait()
                try:
                    protocol.terminal([str(concurrent_ledger), str(generation_path), run_id, str(concurrent_valid)])
                    terminal_queue.put(True)
                except BaseException:
                    terminal_queue.put(False)

            terminal_processes = [context.Process(target=terminal_child, args=(run_id,)) for ok, run_id in concurrent_results if ok]
            for process in terminal_processes: process.start()
            terminal_event.set()
            for process in terminal_processes: process.join(10)
            terminal_results = [terminal_queue.get(timeout=2) for _ in terminal_processes]
            terminal_value = load_json(concurrent_ledger)
            checks["multiprocess-terminal-updates-no-loss"] = all(terminal_results) and all(item["state"] == "terminal-unsealed" for item in terminal_value["attempts"].values())

            publish_event = context.Event(); publish_queue = context.Queue()
            concurrent_run_dirs = {}
            for ok, run_id in concurrent_results:
                if ok:
                    concurrent_run_dirs[run_id] = prepared_attempt(root, run_id, valid)
                    seal_artifacts.seal(concurrent_run_dirs[run_id])

            def publish_child(run_id: str) -> None:
                publish_event.wait()
                try:
                    protocol.publish([str(concurrent_ledger), str(generation_path), run_id, str(concurrent_run_dirs[run_id])])
                    publish_queue.put(True)
                except BaseException:
                    publish_queue.put(False)

            publish_processes = [context.Process(target=publish_child, args=(run_id,)) for ok, run_id in concurrent_results if ok]
            for process in publish_processes: process.start()
            publish_event.set()
            for process in publish_processes: process.join(10)
            publish_results = [publish_queue.get(timeout=2) for _ in publish_processes]
            published_value = load_json(concurrent_ledger)
            checks["multiprocess-sealed-publications-no-loss"] = all(publish_results) and all(item["state"] == "terminal" for item in published_value["attempts"].values())

            metrics_event = context.Event(); metrics_queue = context.Queue()
            concurrent_metrics_dirs = {
                run_id: prepared_metrics(concurrent_run_dirs[run_id], valid, generation["generation_id"])
                for ok, run_id in concurrent_results if ok
            }

            def metrics_child(run_id: str) -> None:
                metrics_event.wait()
                try:
                    protocol.metrics([
                        str(concurrent_ledger), str(generation_path), run_id,
                        str(concurrent_run_dirs[run_id]), str(concurrent_metrics_dirs[run_id]),
                    ])
                    metrics_queue.put(True)
                except BaseException:
                    metrics_queue.put(False)

            metrics_processes = [context.Process(target=metrics_child, args=(run_id,)) for ok, run_id in concurrent_results if ok]
            for process in metrics_processes: process.start()
            metrics_event.set()
            for process in metrics_processes: process.join(10)
            metrics_results = [metrics_queue.get(timeout=2) for _ in metrics_processes]
            metrics_value = load_json(concurrent_ledger)
            checks["multiprocess-metrics-seals-no-loss"] = all(metrics_results) and all(
                slot.get("metrics_status") == "sealed" for slot in metrics_value["slots"].values()
            )

            fake_extractor = root / "fake-extractor.py"
            fake_extractor.write_text('''import json,pathlib,sys\nrun=pathlib.Path(sys.argv[1]); out=pathlib.Path(sys.argv[sys.argv.index("--output")+1]); c=json.load(open(run/"attempt-classification.json")); valid=c["measurement_status"]=="valid"\nvalue={"schema_version":2,"run":{"run_id":run.name,"generation_id":"synthetic-generation","integrity":{"status":"verified","aggregation_eligible":valid}},"experiment":{"measurement_status":c["measurement_status"],"published":True,"latest_published_attempt":True,"artifact_verified":True,"aggregation_eligible":valid}}\nout.write_text(json.dumps(value)+"\\n")\nraise SystemExit(0 if valid else 3)\n''', encoding="utf-8")
            fake_schema = root / "fake-schema.json"; write_json(fake_schema, {})
            finalizer_ledger = root / "sealed-runs/finalizer-ledger.json"
            finalizer_entry = first_pairs[2][0]
            buffer = io.StringIO()
            with contextlib.redirect_stdout(buffer):
                protocol.claim([str(finalizer_ledger), str(generation_path), finalizer_entry["task_id"], finalizer_entry["trial_id"], finalizer_entry["criterion"], "initial"])
            finalizer_claim = json.loads(buffer.getvalue())
            finalizer_run = prepared_attempt(root, finalizer_claim["run_id"], valid)
            protocol.terminal([str(finalizer_ledger), str(generation_path), finalizer_claim["run_id"], str(finalizer_run / "attempt-classification.json")])
            seal_artifacts.seal(finalizer_run)
            protocol.publish([str(finalizer_ledger), str(generation_path), finalizer_claim["run_id"], str(finalizer_run)])
            partial_metrics = finalizer_run.parent / "automatic-metrics" / finalizer_run.name
            partial_metrics.mkdir(parents=True); (partial_metrics / "partial.txt").write_text("interrupted\n", encoding="utf-8")
            pending_ledger = load_json(finalizer_ledger)
            pending_attempt = pending_ledger["attempts"][finalizer_claim["run_id"]]
            pending_attempt.update({
                "owner_host_identity": protocol.host_identity(),
                "owner_pid": 99_999_999,
                "owner_process_start_token": "synthetic-dead-owner",
                "owner_token": "0" * 64,
            })
            write_json(finalizer_ledger, pending_ledger)
            pending_mode, _ = run_queue.slot_decision(generation, pending_ledger, finalizer_entry)
            finalizer_status = run_queue.resume_pending_metrics(
                generation_path, finalizer_ledger, generation, finalizer_entry,
                extractor_path=fake_extractor, schema_path=fake_schema,
            )
            finalizer_value = load_json(finalizer_ledger)
            interrupted_root = finalizer_run.parent / "automatic-metrics-interrupted"
            checks["post-publish-metrics-resume-preserves-partial-and-finishes"] = (
                finalizer_status == 0
                and finalizer_value["slots"][f"{finalizer_entry['task_id']}:{finalizer_entry['trial_id']}:{finalizer_entry['criterion']}"]["metrics_status"] == "sealed"
                and len(list(interrupted_root.glob(f"{finalizer_run.name}-*"))) == 1
            )
            resumed_mode, _ = run_queue.slot_decision(generation, finalizer_value, finalizer_entry)
            checks["queue-rerun-resumes-published-pending-metrics"] = (
                pending_mode == "resume_metrics"
                and finalizer_status == 0
                and resumed_mode == "skip"
            )

            def killed_owner_case(mode: str) -> tuple[pathlib.Path, dict, multiprocessing.Process]:
                case_root = root / f"sigkill-{mode}"
                ledger_path = case_root / "runs/ledger.json"
                work_dir = case_root / "work/prepared"
                result_queue = context.Queue()
                process = context.Process(
                    target=sigkill_owner,
                    args=(mode, ledger_path, generation_path, finalizer_entry, work_dir, result_queue),
                )
                process.start()
                result = result_queue.get(timeout=10)
                return ledger_path, result, process

            precommit_harness = root / "precommit-harness"
            precommit_ledger = precommit_harness / "runs/synthetic-generation/ledger.json"
            precommit_work = precommit_harness / "work/prep-precommit"
            precommit_owner = context.Process(
                target=sigkill_before_claim_commit,
                args=(precommit_ledger, generation_path, finalizer_entry, precommit_work),
            )
            precommit_owner.start()
            precommit_receipt = precommit_work / "claim-receipt.json"
            deadline = time.monotonic() + 10
            while not precommit_receipt.is_file() and precommit_owner.is_alive() and time.monotonic() < deadline:
                time.sleep(0.02)
            precommit_value = load_json(precommit_receipt)
            os.kill(precommit_owner.pid, signal.SIGKILL); precommit_owner.join(10)
            original_queue_harness_root = run_queue.HARNESS_ROOT
            run_queue.HARNESS_ROOT = precommit_harness
            try:
                precommit_actions = run_queue.recover_orphan_claim_receipts(
                    generation_path, precommit_ledger, generation,
                )
            finally:
                run_queue.HARNESS_ROOT = original_queue_harness_root
            checks["sigkill-precommit-claim-receipt-is-cleaned"] = (
                precommit_owner.exitcode == -signal.SIGKILL
                and not precommit_ledger.exists()
                and not precommit_work.exists()
                and precommit_actions == [{
                    "run_id": precommit_value["run_id"],
                    "recovery": "removed-dead-uncommitted-claim",
                }]
            )

            claim_ledger, claim_result, claim_owner = killed_owner_case("claim")
            try:
                run_queue.recover_interrupted_attempts(
                    generation_path, claim_ledger, generation,
                    extractor_path=fake_extractor, schema_path=fake_schema,
                )
                live_owner_refused = False
            except RuntimeError as error:
                live_owner_refused = "owner is alive" in str(error)
            os.kill(claim_owner.pid, signal.SIGKILL); claim_owner.join(10)
            claim_actions = run_queue.recover_interrupted_attempts(
                generation_path, claim_ledger, generation,
                extractor_path=fake_extractor, schema_path=fake_schema,
            )
            claim_value = load_json(claim_ledger)
            claim_attempt = claim_value["attempts"][claim_result["run_id"]]
            claim_slot_key = claim_attempt["slot_key"]
            checks["sigkill-dead-claim-is-canceled-and-resumable"] = (
                claim_owner.exitcode == -signal.SIGKILL
                and claim_attempt["state"] == "canceled-before-start"
                and claim_slot_key not in claim_value["slots"]
                and claim_actions == [{
                    "run_id": claim_result["run_id"],
                    "recovery": "canceled-dead-unstarted-claim",
                }]
            )
            checks["recovery-refuses-live-foreign-and-unknown-owners"] = (
                live_owner_refused
                and protocol.owner_liveness({"owner_host_identity": "foreign"}) == "foreign-host"
                and protocol.owner_liveness({"owner_host_identity": protocol.host_identity()}) == "unknown"
            )

            running_ledger, running_result, running_owner = killed_owner_case("running")
            os.kill(running_owner.pid, signal.SIGKILL); running_owner.join(10)
            running_actions = run_queue.recover_interrupted_attempts(
                generation_path, running_ledger, generation,
                extractor_path=fake_extractor, schema_path=fake_schema,
            )
            running_value = load_json(running_ledger)
            running_attempt = running_value["attempts"][running_result["run_id"]]
            checks["sigkill-running-child-is-cleaned-and-fail-closed"] = (
                running_owner.exitcode == -signal.SIGKILL
                and protocol.process_snapshot(running_result["process_group"]) is None
                and running_attempt["state"] == "terminal"
                and running_attempt["classification"]["generation_invalid"] is True
                and running_attempt["metrics_status"] == "sealed"
                and running_value["state"] == "invalid-restart-required"
                and running_actions[0]["metrics_exit_code"] == 0
            )

            stale_guardian_token = os.urandom(32).hex()
            unrelated = subprocess.Popen(
                [sys.executable, "-c", "import time; time.sleep(60)"],
                start_new_session=True,
            )
            try:
                try:
                    run_queue.stop_owned_process_group({
                        "guardian_process_group": unrelated.pid,
                        "guardian_token": stale_guardian_token,
                    }, None)
                    stale_group_refused = False
                except RuntimeError as error:
                    stale_group_refused = "exact guardian token" in str(error)
                checks["same-second-reused-pgid-without-token-is-not-killed"] = (
                    stale_group_refused and unrelated.poll() is None
                )
            finally:
                if unrelated.poll() is None:
                    unrelated.terminate()
                    try:
                        unrelated.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        unrelated.kill(); unrelated.wait(timeout=2)

            try:
                protocol.bind_process([])
                legacy_binding_rejected = False
            except SystemExit as error:
                legacy_binding_rejected = "disabled" in str(error)
            checks["legacy-bind-process-is-rejected"] = legacy_binding_rejected

            supervisor_case_root = root / "sigkill-supervisor-after-spawn"
            supervisor_ledger = supervisor_case_root / "runs/ledger.json"
            supervisor_work = supervisor_case_root / "work/prepared"
            supervisor_result_queue = context.Queue()
            supervisor_owner = context.Process(
                target=sigkill_supervisor_after_paid_child_spawn,
                args=(
                    supervisor_ledger, generation_path, finalizer_entry,
                    supervisor_work, supervisor_result_queue,
                ),
            )
            supervisor_owner.start()
            supervisor_result = supervisor_result_queue.get(timeout=10)
            supervisor_run_id = supervisor_result["run_id"]
            marker_path = pathlib.Path(supervisor_result["marker_path"])
            supervisor_attempt = {}
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if supervisor_ledger.is_file():
                    supervisor_attempt = load_json(supervisor_ledger).get("attempts", {}).get(supervisor_run_id, {})
                if (
                    supervisor_attempt.get("state") == "running"
                    and supervisor_attempt.get("guardian_binding_state") == "bound"
                    and marker_path.is_file()
                ):
                    break
                time.sleep(0.02)
            guardian_pgid = supervisor_attempt.get("guardian_process_group")
            guardian_token = supervisor_attempt.get("guardian_token")
            supervisor_run_dir = supervisor_ledger.parent / supervisor_run_id
            receipt_before_kill = load_json(supervisor_run_dir / "claim-receipt.json")
            members_before_kill = (
                protocol.process_group_members(guardian_pgid)
                if isinstance(guardian_pgid, int) else []
            )
            non_zombie_before_kill = [
                member for member in members_before_kill
                if not str(member["state"]).startswith("Z")
            ]
            checks["guardian-prebind-survives-paid-child-spawn-edge"] = (
                supervisor_attempt.get("guardian_binding_state") == "bound"
                and receipt_before_kill.get("guardian_binding_state") == "bound"
                and not (supervisor_run_dir / "child-process.json").exists()
                and len(non_zombie_before_kill) >= 3
            )
            checks["env-i-paid-child-receives-guardian-token"] = (
                isinstance(guardian_token, str)
                and marker_path.read_text(encoding="utf-8") == guardian_token
            )
            os.kill(supervisor_result["supervisor_pid"], signal.SIGKILL)
            os.kill(supervisor_owner.pid, signal.SIGKILL)
            supervisor_owner.join(10)
            supervisor_actions = run_queue.recover_interrupted_attempts(
                generation_path, supervisor_ledger, generation,
                extractor_path=fake_extractor, schema_path=fake_schema,
            )
            supervisor_value = load_json(supervisor_ledger)
            recovered_supervisor_attempt = supervisor_value["attempts"][supervisor_run_id]
            remaining_non_zombies = [
                member for member in protocol.process_group_members(guardian_pgid)
                if not str(member["state"]).startswith("Z")
            ]
            checks["sigkill-supervisor-after-spawn-cleans-prebound-child"] = (
                supervisor_owner.exitcode == -signal.SIGKILL
                and not remaining_non_zombies
                and recovered_supervisor_attempt["state"] == "terminal"
                and recovered_supervisor_attempt["classification"]["generation_invalid"] is True
                and recovered_supervisor_attempt["metrics_status"] == "sealed"
                and supervisor_value["state"] == "invalid-restart-required"
                and supervisor_actions[0]["metrics_exit_code"] == 0
            )

            terminal_ledger, terminal_result, terminal_owner = killed_owner_case("terminal")
            os.kill(terminal_owner.pid, signal.SIGKILL); terminal_owner.join(10)
            terminal_actions = run_queue.recover_interrupted_attempts(
                generation_path, terminal_ledger, generation,
                extractor_path=fake_extractor, schema_path=fake_schema,
            )
            terminal_value = load_json(terminal_ledger)
            terminal_attempt = terminal_value["attempts"][terminal_result["run_id"]]
            checks["sigkill-terminal-unsealed-is-finalized"] = (
                terminal_owner.exitcode == -signal.SIGKILL
                and terminal_attempt["state"] == "terminal"
                and terminal_attempt["classification"]["measurement_status"] == "valid"
                and terminal_attempt["metrics_status"] == "sealed"
                and terminal_actions[0]["metrics_exit_code"] == 0
            )

            published_ledger, published_result, published_owner = killed_owner_case("published")
            os.kill(published_owner.pid, signal.SIGKILL); published_owner.join(10)
            published_before = load_json(published_ledger)
            published_mode, _ = run_queue.slot_decision(generation, published_before, finalizer_entry)
            published_resume = run_queue.resume_pending_metrics(
                generation_path, published_ledger, generation, finalizer_entry,
                extractor_path=fake_extractor, schema_path=fake_schema,
            )
            published_after = load_json(published_ledger)
            published_attempt = published_after["attempts"][published_result["run_id"]]
            checks["sigkill-published-pending-metrics-is-resumed"] = (
                published_owner.exitcode == -signal.SIGKILL
                and published_mode == "resume_metrics"
                and published_resume == 0
                and published_attempt["metrics_status"] == "sealed"
            )

            failing_extractor = root / "failing-extractor.py"
            failing_extractor.write_text("raise SystemExit(4)\n", encoding="utf-8")
            failure_ledger = root / "metrics-failure-ledger.json"
            failure_entry = first_pairs[1][0]
            buffer = io.StringIO()
            with contextlib.redirect_stdout(buffer):
                protocol.claim([str(failure_ledger), str(generation_path), failure_entry["task_id"], failure_entry["trial_id"], failure_entry["criterion"], "initial"])
            failure_claim = json.loads(buffer.getvalue())
            failure_run = prepared_attempt(root, failure_claim["run_id"], valid)
            protocol.terminal([str(failure_ledger), str(generation_path), failure_claim["run_id"], str(failure_run / "attempt-classification.json")])
            seal_artifacts.seal(failure_run)
            protocol.publish([str(failure_ledger), str(generation_path), failure_claim["run_id"], str(failure_run)])
            failure_status = finalize_metrics.finalize(failure_ledger, generation_path, failure_run, failing_extractor, fake_schema)
            failure_value = load_json(failure_ledger)
            checks["post-publish-metrics-failure-invalidates-generation"] = (
                failure_status == 79
                and failure_value["state"] == "invalid-restart-required"
                and failure_value["attempts"][failure_claim["run_id"]]["metrics_status"] == "failed"
            )
        finally:
            protocol.verify_generation = original_verify
        stale_bound_file = root / "stale-bound-code.py"
        stale_bound_file.write_text("version = 1\n", encoding="utf-8")
        stale_report = root / "stale-selftest-report.json"
        write_json(stale_report, {
            "passed": True,
            "external_model_calls": 0,
            "builds": 0,
            "indexing_operations": 0,
            "tested_file_sha256": {str(stale_bound_file.relative_to(HARNESS_ROOT)): sha256(stale_bound_file)},
            "required_check_names": ["bound-check"],
            "checks": {"bound-check": True},
        })
        generation_contract.verify_bound_selftest(stale_report, frozenset({"bound-check"}))
        stale_bound_file.write_text("version = 2\n", encoding="utf-8")
        try:
            generation_contract.verify_bound_selftest(stale_report, frozenset({"bound-check"}))
            stale_mutation_rejected = False
        except SystemExit:
            stale_mutation_rejected = True
        checks["selftest-report-rejects-tested-code-mutation"] = stale_mutation_rejected
        runtime_contract = load_json(HARNESS_ROOT / "config/b2-runtime.json")
        runtime_attestation = load_json(pathlib.Path(runtime_contract["attestation_path"]))
        for check_name in (
            "b2-reproducibility-rejects-nonexact-build-count",
            "b2-reproducibility-rejects-false-byte-comparison",
            "b2-reproducibility-rejects-legacy-cargo-lock-identity",
            "b2-reproducibility-rejects-candidate-contamination",
        ):
            mutated = json.loads(json.dumps(runtime_attestation))
            if check_name.endswith("nonexact-build-count"):
                mutated["build_count"] = 1
            elif check_name.endswith("false-byte-comparison"):
                mutated["binary_comparison"]["bytes_identical"] = False
            elif check_name.endswith("legacy-cargo-lock-identity"):
                mutated["cargo_lock"]["baseline_identity"] = "/legacy/Cargo.lock"
            else:
                mutated["candidate_code_present"] = True
            mutated_path = root / f"{check_name}.json"
            write_json(mutated_path, mutated)
            mutated_contract = {
                **runtime_contract,
                "attestation_path": str(mutated_path),
                "attestation_sha256": sha256(mutated_path),
            }
            try:
                generation_contract.validate_b2_reproducibility(mutated_contract)
                mutation_rejected = False
            except SystemExit:
                mutation_rejected = True
            checks[check_name] = mutation_rejected
    finally:
        shutil.rmtree(root, ignore_errors=True)
    tested_paths = [
        HARNESS_ROOT / "scripts/attest_b2.py",
        HARNESS_ROOT / "scripts/auth_runtime.py",
        HARNESS_ROOT / "scripts/classify_attempt.py",
        HARNESS_ROOT / "scripts/common.py",
        HARNESS_ROOT / "scripts/abort_claimed.py",
        HARNESS_ROOT / "scripts/finalize_attempt.py",
        HARNESS_ROOT / "scripts/finalize_metrics.py",
        HARNESS_ROOT / "scripts/generation.py",
        HARNESS_ROOT / "scripts/materialize_config.py",
        HARNESS_ROOT / "scripts/materialize_sandbox.py",
        HARNESS_ROOT / "scripts/parse_events.py",
        HARNESS_ROOT / "scripts/preflight.py",
        HARNESS_ROOT / "scripts/protocol.py",
        HARNESS_ROOT / "scripts/record_postprocess.py",
        HARNESS_ROOT / "scripts/recover_run_directory.py",
        HARNESS_ROOT / "scripts/render_prompt.py",
        HARNESS_ROOT / "scripts/run_queue.py",
        HARNESS_ROOT / "scripts/run-session.sh",
        HARNESS_ROOT / "scripts/scheduler.py",
        HARNESS_ROOT / "scripts/seal_artifacts.py",
        HARNESS_ROOT / "scripts/selftest.py",
        HARNESS_ROOT / "scripts/session_supervisor.py",
        HARNESS_ROOT / "scripts/selftest_contract.py",
        HARNESS_ROOT / "scripts/v4_events.py",
    ]
    report = {
        "schema_version": 1,
        "external_model_calls": 0,
        "builds": 0,
        "indexing_operations": 0,
        "tested_file_sha256": {str(path.relative_to(HARNESS_ROOT)): sha256(path) for path in tested_paths},
        "required_check_names": sorted(EXECUTION_REQUIRED_CHECKS),
        "checks": checks,
        "passed": (
            EXECUTION_REQUIRED_CHECKS.issubset(checks)
            and all(checks[name] is True for name in EXECUTION_REQUIRED_CHECKS)
            and all(checks[name] is True for name in (
                "b2-reproducibility-rejects-nonexact-build-count",
                "b2-reproducibility-rejects-false-byte-comparison",
                "b2-reproducibility-rejects-legacy-cargo-lock-identity",
                "b2-reproducibility-rejects-candidate-contamination",
            ))
        ),
    }
    output = HARNESS_ROOT / "reports/offline-selftest.json"; write_json(output, report)
    print(json.dumps({"passed": report["passed"], "checks": checks, "report": str(output)}, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    owner_markers = [item for item in sys.argv[1:] if item.startswith("--baseline-owner-token=")]
    if not owner_markers:
        import secrets
        owner_token = secrets.token_hex(32)
        owner_marker = protocol.token_marker("owner", owner_token)
        os.execve(sys.executable, [sys.executable, str(pathlib.Path(__file__).resolve()), owner_marker], {
            **os.environ, "BASELINE_OWNER_TOKEN": owner_token,
        })
    if len(owner_markers) != 1:
        raise SystemExit("ambiguous synthetic owner identity")
    os.environ["BASELINE_OWNER_TOKEN"] = owner_markers[0].split("=", 1)[1]
    os.environ["BASELINE_CLAIM_OWNER_PID"] = str(os.getpid())
    raise SystemExit(main())
