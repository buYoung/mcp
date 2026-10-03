"""Existing offline execution checks; every Codex invocation uses a local substitute."""
from __future__ import annotations

import copy
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest.mock import patch

from . import grading_support, resources, runner, settings
from .core import ContractError, read_jsonl, write_json
from .responses import source_lines, text_result
from .usage import usage_metrics


FAKE_CODEX = r'''#!/usr/bin/env python3
import datetime, json, os, signal, sys, time, uuid
from pathlib import Path
args = sys.argv[1:]
if args == ['--version']:
    print('codex-cli fixture-99.1'); sys.exit(0)
if args == ['exec', '--help']:
    print('--ignore-user-config --ignore-rules --skip-git-repo-check --json --output-schema'); sys.exit(0)
if args == ['login', 'status']: sys.exit(0)
prompt = sys.stdin.read()
folder = Path.cwd().parent
home = Path(os.environ['CODEX_HOME'])
ident = uuid.uuid4().hex
rollout = home / 'sessions' / (ident + '.jsonl')
rollout.parent.mkdir(parents=True)
def row(value):
    value['timestamp'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    with rollout.open('a') as stream: stream.write(json.dumps(value)+'\n')
model = args[args.index('-m')+1]
effort = next(json.loads(v.split('=',1)[1]) for v in args if v.startswith('model_reasoning_effort='))
row({'type':'turn_context','payload':{'model':model,'effort':effort}})
print(json.dumps({'type':'thread.started','thread_id':ident}),flush=True)
usage={'input_tokens':100,'output_tokens':20,'cached_input_tokens':30,'total_tokens':120}
def record(response, factor):
    total={k:v*factor for k,v in usage.items()}
    row({'type':'token_usage_record','payload':{'response_id':response,'usage':usage,'turn_token_usage':total,'thread_token_usage':total}})
def stop(signum, frame):
    record('cleanup',2)
    row({'type':'event_msg','payload':{'type':'turn_aborted'}})
    sys.exit(0)
signal.signal(signal.SIGINT,stop)
record('first',1)
if 'FIXTURE_WAIT' in prompt:
    (folder/'fixture-pid').write_text(str(os.getpid()))
    while True: time.sleep(.02)
if 'FIXTURE_MALFORMED' in prompt:
    print('not-json',flush=True); time.sleep(1); sys.exit(1)
answer = ''
if 'FIXTURE_DRAIN' in prompt:
    deadline=time.monotonic()+5
    while not (folder/'stop.json').exists() and time.monotonic()<deadline: time.sleep(.01)
    assert (folder/'stop.json').exists()
    record('late-completion',2)
    usage={key:value*2 for key,value in usage.items()}
    answer='제한 이후의 답변'
row({'type':'response_item','payload':{'role':'assistant','phase':'final_answer','content':[{'type':'output_text','text':answer}]}})
row({'type':'event_msg','payload':{'type':'task_complete'}})
print(json.dumps({'type':'turn.completed','usage':usage}),flush=True)
'''



def fixture_usage(input_tokens=100, output_tokens=20, cached_input_tokens=30):
    usage = {"input_tokens": input_tokens, "output_tokens": output_tokens, "cached_input_tokens": cached_input_tokens,
             "total_tokens": input_tokens + output_tokens}
    return [{"type": "token_usage_record", "payload": {"response_id": "r1", "usage": usage,
             "turn_id": "t1", "thread_id": "s1", "turn_token_usage": usage, "thread_token_usage": usage}},
            {"type": "event_msg", "payload": {"type": "token_count", "info": {"total_token_usage": usage}}}]



class BenchChecks(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        codex = self.bin / "codex"
        codex.write_text(FAKE_CODEX)
        codex.chmod(0o755)
        self.environment = patch.dict(os.environ, {"PATH": str(self.bin) + os.pathsep + os.environ["PATH"],
                                                 "CODEX_HOME": str(self.root / "empty-auth")})
        self.environment.start()

        self.spec = settings.execution_settings()
        self.spec["preparation_only"] = False
        self.defaults = patch("benchmark.runner.default_settings", side_effect=lambda: copy.deepcopy(self.spec))
        self.defaults.start()

    def tearDown(self):
        self.defaults.stop()
        self.environment.stop()
        self.temporary.cleanup()


    def test_snapshot_tracks_dirty_new_deleted_and_git_reference(self):
        repo = self.root / "repo"
        crate = repo / "apps/codemap-search"
        crate.mkdir(parents=True)
        (crate / "Cargo.toml").write_text('[package]\nname="codemap-search"\nversion="1.0.0"\n')
        (crate / "Cargo.lock").write_text("version = 3\n")
        (crate / "old.rs").write_text("old\n")
        def git(*args):
            return subprocess.check_output(["git", *args], cwd=repo, stderr=subprocess.DEVNULL, text=True)
        git("init", "-q")
        git("add", ".")
        git("-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
        (crate / "old.rs").unlink()
        (crate / "new.rs").write_text("new\n")
        (crate / "target").mkdir()
        (crate / "target/cache").write_text("ignored")
        with patch.object(settings, "REPOSITORY", repo):
            current = resources.snapshot_product({"path": "apps/codemap-search"}, self.root / "snapshot")
            frozen = resources.snapshot_product({"git_ref": "HEAD"}, self.root / "git-snapshot")
        self.assertTrue(current["dirty"])
        self.assertIn("new.rs", current["files"])
        self.assertNotIn("old.rs", current["files"])
        self.assertNotIn("target/cache", current["files"])
        self.assertNotEqual(current["source_sha256"], frozen["source_sha256"])
        self.assertIn("old.rs", frozen["files"])


    def test_explicit_settings_version_and_usage_reach_execution(self):
        spec = settings.execution_settings()
        spec["preparation_only"] = False
        spec.update(model="fixture-model", reasoning_effort="high")
        spec["limits"]["total_tokens"] = 4321
        result = runner.execute_codex(self.root / "session", "fixture", settings=spec, usage_drain_seconds=30,
                                      codex_version="codex-cli fixture-99.1")
        self.assertTrue(result["conditions_valid"])
        self.assertEqual(result["status"], "completed")
        self.assertEqual(result["codex_version"], "codex-cli fixture-99.1")
        self.assertEqual(result["budget"]["limit_tokens"], 4321)
        self.assertEqual(result["usage_collection"]["drain_seconds"], 30)
        self.assertEqual(result["usage"]["total_tokens"]["value"], 120)


    def test_cancel_skips_drain_and_reaps_process(self):
        cancelled = threading.Event()
        folder = self.root / "cancel-session"
        def cancel_when_started():
            deadline = time.monotonic() + 5
            while not (folder / "fixture-pid").exists() and time.monotonic() < deadline:
                time.sleep(.01)
            cancelled.set()
        watcher = threading.Thread(target=cancel_when_started)
        watcher.start()
        start = time.monotonic()
        result = runner.execute_codex(folder, "FIXTURE_WAIT", cancel_event=cancelled, usage_drain_seconds=30,
                                      codex_version="fixture")
        watcher.join()
        self.assertLess(time.monotonic() - start, 10)
        self.assertEqual(result["status"], "interrupted")
        self.assertEqual(result["usage"]["observed"]["total_tokens"], 240)
        self.assertIsNone(result["usage"]["total_tokens"]["value"])
        with self.assertRaises(ProcessLookupError):
            os.kill(int((folder / "fixture-pid").read_text()), 0)


    def test_drain_excludes_late_answer_but_certifies_completed_usage(self):
        spec = settings.execution_settings()
        spec["preparation_only"] = False
        spec["limits"]["total_tokens"] = 100
        folder = self.root / "drain-session"
        result = runner.execute_codex(folder, "FIXTURE_DRAIN", settings=spec, usage_drain_seconds=30,
                                      codex_version="fixture")
        self.assertEqual(result["status"], "token_limit")
        self.assertEqual(result["answer"], "")
        self.assertTrue(result["usage_collection"]["post_cutoff_answer_excluded"])
        self.assertEqual((folder / "post-cutoff-answer.txt").read_text(), "제한 이후의 답변")
        self.assertEqual(result["usage"]["total_tokens"]["value"], 240)
        self.assertEqual(result["budget"]["observed_cleanup_tokens"], 120)
        self.assertEqual(result["shutdown"]["steps"], [])


    def test_unsupported_log_is_failure(self):
        result = runner.execute_codex(self.root / "malformed", "FIXTURE_MALFORMED", codex_version="future")
        self.assertEqual(result["status"], "environment_error")


    def test_masking_keeps_original(self):
        source = self.root / "source"
        (source / "pkg").mkdir(parents=True)
        (source / "pkg/gold.go").write_text("gold\n")
        (source / "pkg/other.go").write_text("\n".join(map(str, range(100))))
        raw = f"근거 [{source}/pkg/other.go:10-20]({source}/pkg/other.go:10)"
        run = {"id": "sample", "answer": raw, "source_snapshot": str(source)}
        _, masked, replacements, _, flags = grading_support.mask_answer(run, source, self.root, seed=self.spec["seed"])
        restored = masked
        for original, anonymous in reversed(replacements):
            restored = restored.replace(anonymous, original)
        self.assertEqual(raw, restored)
        self.assertFalse(flags)


class RuntimeChecks(unittest.TestCase):
    def test_model_content_field_is_not_mcp_content_array(self):
        from .runner import delivered_leaves
        self.assertEqual(delivered_leaves('{"file":"demo.ts","content":"demo.ts:1:answer"}'), ["demo.ts", "demo.ts:1:answer"])
        self.assertEqual(delivered_leaves('{"content":["one", "two"]}'), ["one", "two"])


    def test_truncated_serialized_source_prefix(self):
        from .runner import delivered_leaves
        raw = json.dumps(text_result("demo.ts:1:const delay = 5000;\ndemo.ts:2:setTimeout(work, delay);"))
        clipped = raw[:raw.index("setTimeout") + 3]
        leaves = delivered_leaves(clipped)
        prefix = next(leaf for leaf in leaves if leaf.startswith("demo.ts:1:"))
        lines = source_lines("read", {"file_path": "demo.ts"}, prefix, partial=True)
        self.assertEqual(lines, [{"path": "demo.ts", "line": 1, "text": "const delay = 5000;"}])


    def test_duplicate_usage_cache_and_cumulative(self):
        rows = fixture_usage()
        result = usage_metrics(rows + rows, terminal_complete=True, evidence="fixture")
        self.assertEqual(result["total_tokens"]["value"], 120)
        self.assertEqual(result["cached_input_ratio"]["value"], .3)
        self.assertEqual(result["model_responses"]["value"], 1)
        conflict = copy.deepcopy(rows[0]); conflict["payload"]["usage"]["input_tokens"] = 101
        self.assertIsNone(usage_metrics(rows + [conflict], terminal_complete=True, evidence="fixture")["total_tokens"]["value"])
        self.assertIsNone(usage_metrics(rows[1:], terminal_complete=True, evidence="fixture")["total_tokens"]["value"])


    def test_delayed_legacy_usage_and_latest_response_snapshots(self):
        rows = fixture_usage()
        second = copy.deepcopy(rows[0]); second["payload"]["response_id"] = "r2"
        for field in ["turn_token_usage", "thread_token_usage"]:
            second["payload"][field] = {k: v * 2 for k, v in rows[0]["payload"]["usage"].items()}
        rows += [second, copy.deepcopy(rows[1])]
        result = usage_metrics(rows, terminal_complete=False, evidence="fixture")
        self.assertEqual(result["consistency"]["status"], "valid")
        self.assertEqual(result["observed"]["total_tokens"], 240)
        self.assertTrue(all(e["status"] == "delayed" for e in result["consistency"]["legacy_events"]))
        self.assertIsNone(result["total_tokens"]["value"])
        self.assertEqual(usage_metrics(rows, terminal_complete=True, evidence="fixture")["total_tokens"]["value"], 240)
        for field in ["turn_token_usage", "thread_token_usage"]:
            conflict = copy.deepcopy(rows)
            conflict[2]["payload"][field]["cached_input_tokens"] += 1
            self.assertEqual(usage_metrics(conflict, terminal_complete=True, evidence="fixture")["consistency"]["status"], "invalid")
        missing = [second, rows[-1]]
        self.assertEqual(usage_metrics(missing, terminal_complete=True, evidence="fixture")["consistency"]["status"], "invalid")


    def test_duplicate_snapshot_conflict_and_real_legacy_mismatch(self):
        rows = fixture_usage()
        duplicate = copy.deepcopy(rows[0]); duplicate["payload"]["thread_token_usage"] = dict(duplicate["payload"]["thread_token_usage"], input_tokens=999)
        result = usage_metrics(rows + [duplicate], terminal_complete=True, evidence="fixture")
        self.assertTrue(any("conflicting duplicate" in e for e in result["errors"]))
        rows[1] = copy.deepcopy(rows[1]); rows[1]["payload"]["info"]["total_token_usage"]["cached_input_tokens"] = 2
        self.assertEqual(usage_metrics(rows, terminal_complete=True, evidence="fixture")["consistency"]["status"], "invalid")


    def test_stop_latches_cutoff_and_escalates_after_three_seconds(self):
        from unittest.mock import Mock, patch
        from .runner import ExecutionStop
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary); process = Mock(pid=12345); process.poll.return_value = None
            with patch("benchmark.runner.time.monotonic") as clock, patch("benchmark.runner.signal_target", return_value=True) as send:
                clock.return_value = 10
                stop = ExecutionStop(folder, process, 0); stop.request("token_limit", {"total_tokens": 504000})
                stop.request("timeout", {"total_tokens": 900000})
                clock.return_value = 12.99; stop.advance(); self.assertEqual(send.call_count, 1)
                clock.return_value = 13; stop.advance()
                clock.return_value = 15.99; stop.advance(); self.assertEqual(send.call_count, 2)
                clock.return_value = 16; stop.advance()
                self.assertEqual([s["signal"] for s in stop.steps], ["SIGINT", "SIGTERM", "SIGKILL"])
                self.assertEqual(stop.cutoff["observed_usage"]["total_tokens"], 504000)
                self.assertEqual(stop.cutoff["reason"], "token_limit")

    def test_terminal_certification_requires_normal_completion_and_collection(self):
        from .core import append_jsonl
        from .runner import recorded_usage
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            runtime = {"exit_code": 0, "termination": None, "record_collection_complete": True}
            write_json(folder / "runtime.json", runtime)
            rows = fixture_usage() + [{"type": "event_msg", "payload": {"type": "task_complete"}}]
            for row in rows: append_jsonl(folder / "rollout.jsonl", row)
            append_jsonl(folder / "events.jsonl", {"type": "turn.completed", "usage": rows[0]["payload"]["usage"]})
            self.assertTrue(recorded_usage(folder)["complete"])
            for change in [{"exit_code": -9}, {"record_collection_complete": False}, {"shutdown": {"steps": [{"signal": "SIGTERM"}]}}]:
                write_json(folder / "runtime.json", {**runtime, **change})
                result = recorded_usage(folder)
                self.assertEqual(result["consistency"]["status"], "valid")
                self.assertIsNone(result["total_tokens"]["value"])
            write_json(folder / "runtime.json", {**runtime, "termination": "token_limit"})
            self.assertTrue(recorded_usage(folder)["complete"])
            write_json(folder / "runtime.json", runtime)
            append_jsonl(folder / "rollout.jsonl", {"type": "response_item", "payload": {"type": "custom_tool_call", "call_id": "pending"}})
            self.assertEqual(recorded_usage(folder)["cost_completeness"]["pending_host_call_ids"], ["pending"])
            append_jsonl(folder / "rollout.jsonl", {"type": "event_msg", "payload": {"type": "turn_aborted"}})
            self.assertFalse(recorded_usage(folder)["complete"])


    def test_execute_collects_usage_on_sigint_and_reaggregation_agrees(self):
        import subprocess
        import sys
        from unittest.mock import patch
        from .runner import execute_codex, recorded_usage
        script = '''
import json, os, signal, sys, time
from pathlib import Path
home = Path(os.environ["CODEX_HOME"])
rollout = home / "sessions" / "fixture-thread.jsonl"
rollout.parent.mkdir(parents=True)
def row(value):
    with rollout.open("a") as stream: stream.write(json.dumps(value) + "\\n")
usage = {"input_tokens": 100, "output_tokens": 20, "cached_input_tokens": 30, "total_tokens": 120}
def record(response, factor):
    total = {k:v*factor for k,v in usage.items()}
    row({"type":"token_usage_record","payload":{"response_id":response,"usage":usage,"turn_token_usage":total,"thread_token_usage":total}})
def stop(signum, frame):
    record("r2", 2)
    row({"type":"event_msg","payload":{"type":"turn_aborted"}})
    print(json.dumps({"type":"turn.failed"}), flush=True)
    sys.exit(0)
signal.signal(signal.SIGINT, stop)
print(json.dumps({"type":"thread.started","thread_id":"fixture-thread"}), flush=True)
record("r1", 1)
while True: time.sleep(.02)
'''
        spec = settings.execution_settings()
        spec["preparation_only"] = False
        spec["limits"]["total_tokens"] = 100
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            popen = subprocess.Popen
            def launch(args, **kwargs):
                return popen([sys.executable, "-c", script], **kwargs)
            def environment(path):
                import os
                home = path / "codex-home"; home.mkdir()
                return {**os.environ, "CODEX_HOME": str(home)}, home
            with patch("benchmark.runner.subprocess.Popen", side_effect=launch), patch("benchmark.runner.isolated_environment", side_effect=environment):
                result = execute_codex(folder, "fixture", settings=spec)
            self.assertEqual(result["status"], "token_limit")
            self.assertEqual(result["budget"]["observed_tokens_at_cutoff"], 120)
            self.assertEqual(result["budget"]["observed_cleanup_tokens"], 120)
            self.assertEqual(result["usage"]["observed"]["total_tokens"], 240)
            self.assertEqual(result["usage"], recorded_usage(folder))
            self.assertIsNone(result["usage"]["total_tokens"]["value"])
            self.assertEqual([s["signal"] for s in result["shutdown"]["steps"]], ["SIGINT"])


    def test_internal_budget_options_removed(self):
        from .runner import codex_options
        self.assertFalse(any("rollout_budget" in option for option in codex_options()))


    def test_relay_cleanup_finishes(self):
        import subprocess
        import sys
        import time
        from .core import file_digest
        from .runner import cleanup_relay
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"], start_new_session=True)
            try:
                write_json(folder / "relay-runtime.json", {"pid": child.pid, "pgid": child.pid})
                def pump():
                    child.poll()
                    time.sleep(.01)
                result = cleanup_relay(folder, pump)
                self.assertTrue(result["complete"])
                self.assertIsNotNone(child.poll())
                write_json(folder / "run.json", {"status": "token_limit"})
            finally:
                if child.poll() is None: child.kill()
                child.wait()
