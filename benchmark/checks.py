"""Offline regression checks. All Codex executions use an isolated local substitute."""
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

from . import evaluate, execution, grading_support, resources, settings
from .v2 import runner
from .v2.checks import example_question
from .v2.core import SPEC, ContractError, digest, file_digest, read_json, write_json
from .v2.transport import BASELINE_TOOLS, Relay


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
if '--output-schema' in args:
    # The test-only substitute reads private controls to produce deterministic codec results.
    batch = folder.parent
    registry=json.loads((batch/'registry.json').read_text())
    judgments=[]
    for ident,item in registry.items():
        if item['control']:
            value=item['expected']
            value.update(answer_id=ident,question_id=item['question_id'])
            for part in value['facts']+value['major_errors']:
                quote=part.pop('answer_quote')
                part['answer_unit_id']='none' if not quote else next(u['id'] for u in item['units'] if quote in u['text'])
            judgments.append(value)
    answer=json.dumps({'judgments':judgments})
row({'type':'response_item','payload':{'role':'assistant','phase':'final_answer','content':[{'type':'output_text','text':answer}]}})
row({'type':'event_msg','payload':{'type':'task_complete'}})
print(json.dumps({'type':'turn.completed','usage':usage}),flush=True)
'''


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

    def tearDown(self):
        self.environment.stop()
        self.temporary.cleanup()

    def test_profiles_preserve_original_questions_and_subset(self):
        original = read_json(settings.ROOT / "v2/data/dataset.json")
        lookup = {q["id"]: q for q in original["questions"]}
        for name, count in [("candidate", 3), ("validation", 10), ("formal", 30)]:
            dataset = settings.dataset_for(name)
            self.assertEqual(len(dataset["questions"]), count)
            self.assertTrue(all(q == lookup[q["id"]] for q in dataset["questions"]))
        self.assertEqual(original["spec_sha256"], digest(SPEC))

    def test_approved_A_manifest_and_regex_help(self):
        self.assertEqual(file_digest(settings.ROOT / "data/a-tools.json"),
                         "00ec51d0ca1f2202942bb988db4f43ef530b473701bcdb07810a765b63bf64a0")
        self.assertEqual(BASELINE_TOOLS, read_json(settings.ROOT / "data/a-tools.json"))

    def test_relay_receives_selected_call_limit(self):
        (self.root / "demo.ts").write_text("hello\n")
        relay = Relay({"group": "A", "source": str(self.root), "log": str(self.root / "relay.jsonl"),
                       "limits": {"exploration_calls": 1}})
        self.assertFalse(relay.call(1, {"name": "read", "arguments": {"file_path": "demo.ts"}}).get("isError"))
        self.assertTrue(relay.call(2, {"name": "read", "arguments": {"file_path": "demo.ts"}})["isError"])

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
        spec = settings.execution_settings(settings.load_config())
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
        spec = settings.execution_settings(settings.load_config())
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

    def test_label_ranges_do_not_expand_single_anchor_or_mismatched_file(self):
        self.assertEqual(grading_support.label_ranges("[f.go:10-20](pkg/f.go:10)")[0]["end_line"], 20)
        self.assertEqual(grading_support.label_ranges("[f.go:10](pkg/f.go:10)"), [])
        self.assertEqual(grading_support.label_ranges("[other.go:10-20](pkg/f.go:10)"), [])

    def test_alternative_evidence_and_masking_keep_original(self):
        source = self.root / "source"
        (source / "pkg").mkdir(parents=True)
        (source / "pkg/gold.go").write_text("gold\n")
        (source / "pkg/other.go").write_text("\n".join(map(str, range(100))))
        raw = f"근거 [{source}/pkg/other.go:10-20]({source}/pkg/other.go:10)"
        run = {"id": "sample", "answer": raw, "source_snapshot": str(source)}
        _, masked, replacements, _, flags = grading_support.mask_answer(run, source, self.root)
        restored = masked
        for original, anonymous in reversed(replacements):
            restored = restored.replace(anonymous, original)
        self.assertEqual(raw, restored)
        self.assertFalse(flags)
        extra = grading_support.source_supplement({"evidence": [{"path": "pkg/gold.go"}]}, [{"answer": "pkg/other.go:10-20"}], source)
        self.assertEqual(extra[1]["explicit_cited_ranges"], [[10, 20]])
        self.assertTrue(extra[1]["context_does_not_expand_answer_citation"])

    def test_schedule_denominators_and_baseline_conditions(self):
        plan = {"dataset": settings.dataset_for("candidate"), "spec": settings.execution_settings(settings.load_config()),
                "targets": {t["id"]: t for t in settings.targets(settings.load_config())}, "repeats": 3,
                "profile": "candidate", "harness_sha256": "fixture", "source": {}, "codex_version": "one"}
        rows = execution.schedule(plan)
        self.assertEqual(len(rows), 18)
        self.assertEqual(len({r["id"] for r in rows}), 18)
        other = {**plan, "codex_version": "two"}
        self.assertNotEqual(execution.baseline_key(plan), execution.baseline_key(other))

    def test_full_scheduler_fake_codex_report_resume_and_A_reuse(self):
        source = self.root / "source"
        source.mkdir()
        (source / "fixture.go").write_text("package fixture\n")
        config = settings.load_config()
        spec = settings.execution_settings(config)
        selected_dataset = settings.dataset_for("candidate")
        plan = {"profile": "candidate", "dataset": selected_dataset, "spec": spec,
                "source": {"snapshot": str(source), "source_sha256": digest(runner.source_manifest(source))},
                "targets": {"A": {"id": "A", "name": "A", "group": "A", "snapshot": str(source)}},
                "codex_version": "codex-cli fixture-99.1", "harness_sha256": "fixture-harness", "repeats": 3,
                "reuse_choices": []}
        plan["baseline_key"] = execution.baseline_key(plan)
        cache = self.root / "cache"
        runs = self.root / "runs"
        with patch.object(settings, "CACHE", cache), patch.object(settings, "RUNS", runs), \
                patch.object(settings, "harness_identity", return_value="fixture-harness"), \
                patch.object(resources, "environment", return_value={"codex_version": plan["codex_version"]}):
            plan_id = "a" * 32
            write_json(cache / "plans" / f"{plan_id}.json", plan)
            root = execution.create_run(plan_id, None)
            result = execution.execute(root, lambda _: None)
            self.assertEqual(result["status"], "complete")
            saved = read_json(root / "report/summary.json")
            self.assertEqual(saved["scheduled"], 9)
            self.assertEqual(saved["targets"]["A"]["categories"], {"no_answer": 9})
            self.assertEqual(saved["targets"]["A"]["metrics"]["total_tokens"]["sum"]["value"], 1080)
            hashes = {str(p): file_digest(p) for p in root.glob("runs/*/seal.json")}
            execution.execute(root, lambda _: None)
            self.assertEqual(hashes, {str(p): file_digest(p) for p in root.glob("runs/*/seal.json")})
            plan["reuse_choices"] = [{"id": root.name, "runs": 9}]
            write_json(cache / "plans" / f"{plan_id}.json", plan)
            reused_root = execution.create_run(plan_id, root.name)
            execution.execute(reused_root, lambda _: None)
            reused = read_json(reused_root / "report/summary.json")
            self.assertEqual(reused["targets"]["A"]["reused_runs"], 9)
            self.assertEqual(len(list(reused_root.glob("runs/*/command.json"))), 0)

    def test_bounded_supplement_keeps_all_evidence_and_explicit_ranges(self):
        source = self.root / "source"
        (source / "pkg").mkdir(parents=True)
        (source / "pkg/large.go").write_text("\n".join(f"source {i}" for i in range(1, 10001)))
        question = {"evidence": [{"path": "pkg/large.go", "start_line": 300, "end_line": 350}]}
        packet = grading_support.source_supplement(question, [{"answer": "pkg/large.go:1000-1010"}], source, full_files=False)[0]
        lines = {int(line.split(":", 1)[0]) for line in packet["numbered_source"].splitlines()}
        self.assertTrue(set(range(300, 351)) <= lines)
        self.assertTrue(set(range(1000, 1011)) <= lines)
        self.assertFalse(packet["full_file"])

    def test_resume_preserves_completed_and_recovers_interrupted_slots(self):
        root = self.root / "run"
        entries = [{"id": f"q-A-{r}", "question_id": "q", "group": "A", "target": "A", "repeat": r, "phase": "main"} for r in range(1, 4)]
        write_json(root / "schedule.json", entries)
        for index, entry in enumerate(entries):
            folder = root / "runs" / entry["id"]
            row = runner.placeholder_run(entry, ["completed", "running", "scheduled"][index])
            write_json(folder / "run.json", row)
            if index == 0:
                runner.seal_run(folder)
        before = file_digest(root / "runs/q-A-1/run.json")
        execution.recover(root)
        rows = runner.load_experiment_runs(root)
        self.assertEqual([r["status"] for r in rows], ["completed", "interrupted", "scheduled"])
        self.assertEqual(file_digest(root / "runs/q-A-1/run.json"), before)

    def test_grader_controls_codec_is_executable_and_resumable(self):
        question = example_question()
        # Fixture paths are outside Grafana's public/pkg prefix; render_control leaves them intact.
        source = self.root / "source"
        source.mkdir()
        for evidence in question["evidence"]:
            path = source / evidence["path"]
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(evidence["text"])
        root = self.root / "experiment"
        root.mkdir()
        packet, registry, _ = evaluate.packet(question, [], source, root, 1)
        folder = root / "grading/batch-001"
        write_json(folder / "packet.json", packet)
        write_json(folder / "registry.json", registry)
        schema = grading_support.unit_schema(max(len(item["units"]) for item in registry.values()))
        write_json(folder / "schema.json", schema)
        (folder / "prompt.txt").write_text(evaluate.INSTRUCTIONS + json.dumps(packet))
        write_json(root / "grading/mechanical.json", {})
        with patch.object(evaluate, "prepare", return_value=[folder]):
            first = evaluate.run(root, {"questions": [question]}, [], source, settings.execution_settings(settings.load_config()), threading.Event(), lambda _: None)
            second = evaluate.run(root, {"questions": [question]}, [], source, settings.execution_settings(settings.load_config()), threading.Event(), lambda _: None)
        self.assertTrue(first["complete"])
        self.assertEqual(first, second)


if __name__ == "__main__":
    unittest.main()
