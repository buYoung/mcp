"""Existing offline execution checks; every agent invocation uses a local substitute."""
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

from . import grading_support, pi_grader, resources, runner, settings
from .core import ContractError, digest, read_jsonl, write_json
from .responses import source_lines, text_result
from .usage import usage_metrics


FAKE_CODEX = r'''#!/usr/bin/env python3
import datetime, json, os, signal, sys, time, uuid
from pathlib import Path
args = sys.argv[1:]
if args == ['--version']:
    print('codex-cli 0.164.0'); sys.exit(0)
if args == ['exec', '--help']:
    print('--ignore-user-config --ignore-rules --skip-git-repo-check --json --output-schema'); sys.exit(0)
if args == ['login', 'status']: sys.exit(0)
signal.signal(signal.SIGALRM, lambda signum, frame: sys.exit(124))
signal.alarm(15)  # Fixture safety only; never a solver execution limit.
prompt = sys.stdin.read()
home = Path(os.environ['CODEX_HOME'])
folder = home.parent
ident = uuid.uuid4().hex
rollout = home / 'sessions' / (ident + '.jsonl')
rollout.parent.mkdir(parents=True)
def row(value):
    value['timestamp'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    with rollout.open('a') as stream: stream.write(json.dumps(value)+'\n')
model = args[args.index('-m')+1]
effort = next(json.loads(v.split('=',1)[1]) for v in args if v.startswith('model_reasoning_effort='))
row({'type':'session_meta','payload':{'base_instructions':{'text':'Inspect source.'}}})
row({'type':'response_item','payload':{'role':'user','content':[{'type':'input_text','text':prompt}]}})
row({'type':'turn_context','payload':{'model':model,'effort':effort}})
print(json.dumps({'type':'thread.started','thread_id':ident}),flush=True)
usage={'input_tokens':100,'output_tokens':20,'cached_input_tokens':30,'total_tokens':120}
if 'FIXTURE_LARGE' in prompt:
    usage.update(input_tokens=600040,total_tokens=600060)
def record(response, factor):
    total={k:v*factor for k,v in usage.items()}
    row({'type':'token_usage_record','payload':{'response_id':response,'usage':usage,'turn_token_usage':total,'thread_token_usage':total}})
def stop(signum, frame):
    record('cleanup',2)
    if 'FIXTURE_DRAIN' in prompt:
        row({'type':'response_item','payload':{'role':'assistant','phase':'final_answer','content':[{'type':'output_text','text':'제한 이후의 답변'}]}})
        row({'type':'event_msg','payload':{'type':'task_complete'}})
        print(json.dumps({'type':'turn.completed','usage':{key:value*2 for key,value in usage.items()}}),flush=True)
    else:
        row({'type':'event_msg','payload':{'type':'turn_aborted'}})
    sys.exit(0)
signal.signal(signal.SIGINT,stop)
record('first',1)
if 'FIXTURE_WAIT' in prompt:
    (folder/'fixture-pid').write_text(str(os.getpid()))
    while True: time.sleep(.02)
if 'FIXTURE_MALFORMED' in prompt:
    print('not-json',flush=True); time.sleep(1); sys.exit(1)
answer = '완성 답변'
if 'FIXTURE_DRAIN' in prompt:
    (folder/'fixture-pid').write_text(str(os.getpid()))
    while True: time.sleep(.01)
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
        for target, value in (("benchmark.settings.CACHE", self.root), ("benchmark.codex_client.executable", lambda: str(codex))):
            patched = patch(target, value)
            patched.start()
            self.addCleanup(patched.stop)

        self.spec = settings.execution_settings()
        self.spec["preparation_only"] = False
        self.spec["solver_backend"] = "codex"
        self.defaults = patch("benchmark.runner.default_settings", side_effect=lambda: copy.deepcopy(self.spec))
        self.defaults.start()

    def tearDown(self):
        import signal
        for path in self.root.rglob("process.json"):
            value = json.loads(path.read_text())
            pid = value["pid"]
            command = subprocess.run(["ps", "-p", str(pid), "-o", "command="], capture_output=True, text=True).stdout
            if str(self.bin / "codex") in command:
                try:
                    if os.getpgid(pid) == pid:
                        os.killpg(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        self.defaults.stop()
        self.environment.stop()
        self.temporary.cleanup()


    def test_measurement_modes_schedule_and_report_are_separate(self):
        from . import route_contract as routes, route_metrics
        rows = routes.schedule()
        expected = len(routes.QUESTION_IDS) * len(routes.ARM_IDS) * routes.REPETITIONS * len(routes.solver_measurement_modes())
        self.assertEqual(len(rows), expected)
        self.assertEqual(len({row["id"] for row in rows}), expected)
        self.assertEqual([row["attempt"] for row in rows], sorted(row["attempt"] for row in rows))
        for row in rows:
            self.assertIn(f"-{row['mode']}-", row["id"])
        report = route_metrics.report(self.root, {}, [], {})
        self.assertEqual(report["schema_version"], 4)
        self.assertIsNone(report["solver_limits"])
        self.assertTrue(all(row["solver_limits"] is None for row in report["rows"]))
        self.assertIn("풀이 제한 없음", (self.root / "report/results.md").read_text())
        self.assertNotIn("budget", report["rows"][0])
        self.assertNotIn("quality", report)
        self.assertNotIn("arms", report)
        for mode in routes.solver_measurement_modes():
            for arm in routes.ARM_IDS:
                self.assertEqual(report["modes"][mode]["arms"][arm]["planned"], len(routes.QUESTION_IDS) * routes.REPETITIONS)
        models = {"solver": routes.solver_model(),
                  "grader": {"model": "fixture", "reasoning_effort": "high"}}
        observed = {**rows[0], "model": "gpt-6.1-sol", "backend": "codex", "status": "completed",
                    "observations": {"direct_calls_outside_script": 1, "cancelled_before_admission": 1},
                    "claude_persisted_tool_results": {"count": 1, "bytes": 85592}}
        report = route_metrics.report(self.root, {"models": models}, [observed], {})
        self.assertEqual(report["rows"][0]["claude_persisted_tool_results"], {"count": 1, "bytes": 85592})
        self.assertEqual(report["rows"][0]["observations"]["cancelled_before_admission"], 1)
        self.assertIn('"bytes": 85592', (self.root / "report/results.md").read_text())

    def test_cli_solver_contract_retains_pi_grader_and_model_specific_slot_keys(self):
        from . import route_contract as routes
        profile = routes.load_profile()
        self.assertEqual(routes.solver_measurement_modes(), ("codemode",))
        self.assertEqual(routes.solver_measurement_modes("claude-opus-5-5"), ("direct",))
        self.assertEqual(len(routes.schedule("claude-opus-5-5")), 42)
        self.assertEqual(len(routes.schedule()), 42)
        self.assertEqual([s["backend"] for s in profile["solver_choices"]], ["codex", "claude_code"])
        self.assertEqual(settings.execution_settings()["grader_backend"], "pi")
        self.assertTrue(all("-codemode-" in row["id"] for row in routes.schedule()))
        pins = routes.frozen_inputs()["lock"]
        self.assertEqual(pins["revision"], 8)
        self.assertEqual(pins["cli_toolchain"]["versions"], {"codex": "0.164.0", "claude_code": "2.1.311"})
        self.assertIn("pi_toolchain", pins)

    def test_codex_solver_cannot_select_legacy_grader(self):
        with self.assertRaisesRegex(ContractError, "pi/Astra"):
            runner.execute_codex(self.root / "legacy-grader", "unused", grading=True)

    def test_client_delivery_converts_reference_bytes_once_and_leaves_rg_defaults(self):
        from .client_delivery import output_limits
        self.assertEqual(output_limits("codemap-search"), {"reference_text_bytes": 100000, "codex_tokens": 100000, "claude_tokens": 50000})
        self.assertEqual(output_limits("serena"), {"reference_text_bytes": 1250000, "codex_tokens": 312500, "claude_tokens": 625000})
        for arm in ("codegraph", "zvec-grep", "graphify", "codebase-memory-mcp"):
            self.assertEqual(output_limits(arm), {"reference_text_bytes": 1048576, "codex_tokens": 262144, "claude_tokens": 524288})
        self.assertTrue(all(value is None for value in output_limits("rg").values()))

    def test_codex_history_observation_never_certifies_raw_rollout_as_wire(self):
        from .route_metrics import codex_history_observation
        folder = self.root / "history"; folder.mkdir()
        payload = {"type": "custom_tool_call_output", "call_id": "c1", "output": [{"type": "input_text", "text": "a" * 50000}]}
        (folder / "rollout.jsonl").write_text(json.dumps({"type": "response_item", "payload": payload}) + "\n")
        self.assertIsNone(codex_history_observation(folder)["history_truncated"])
        write_json(folder / "cli-policy.json", {"local_provider": True})
        write_json(folder / "requests.json", [{"input": []}, {"input": [payload]}])
        self.assertEqual(codex_history_observation(folder)["history_truncated"], 0)
        write_json(folder / "requests.json", [{"input": []}, {"input": [{**payload, "output": [{"type": "input_text", "text": "a…100 tokens truncated…a"}]}]}])
        self.assertEqual(codex_history_observation(folder)["history_truncated"], 1)
        write_json(folder / "cli-policy.json", {"local_provider": False})
        self.assertFalse(codex_history_observation(folder)["observed"])

    def test_claude_permission_denial_and_explicit_private_token_file(self):
        from .claude_client import operator_token, tool_items
        events = [{"type": "system", "subtype": "permission_denied", "message": "Permission denied"},
                  {"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": "false"}}]}},
                  {"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "t1", "content": "Permission denied", "is_error": True}]},
                   "tool_result_meta": [{"id": "t1", "non_execution_kind": "permission-rule"}]}]
        self.assertFalse(tool_items(events)[0]["dispatched"])
        path = self.root / "local-token"; path.write_text("local-fixture-value\n"); path.chmod(0o644)
        with patch.dict(os.environ, {"CODEMAP_BENCH_CLAUDE_TOKEN_FILE": str(path)}, clear=True):
            with self.assertRaises(ContractError): operator_token()
            path.chmod(0o600)
            self.assertEqual(operator_token(), ("local-fixture-value", "oauth_file"))

    def test_relay_client_metadata_preserves_native_and_fills_only_absent_budget(self):
        from .route_transport import client_tool_metadata
        native = {"name": "read", "_meta": {"anthropic/maxResultSizeChars": 100000, "other": "original"}}
        self.assertIs(client_tool_metadata(native), native)
        without = {"name": "graph_stats", "_meta": {"other": "original"}}
        result = client_tool_metadata(without)
        self.assertEqual(result["_meta"], {"other": "original", "anthropic/maxResultSizeChars": 500000})
        self.assertNotIn("anthropic/maxResultSizeChars", without["_meta"])

    def test_cli_launch_uses_private_paths_and_product_only_output_policy(self):
        from . import codex_client, claude_client
        folder = self.root / "launch"; folder.mkdir()
        relay = {"arm": "codemap-search", "enabled_tools": ["read"]}
        server = {"command": "/usr/bin/false", "args": [], "env": {}, "instructions": "Read source."}
        args, cwd, env = codex_client.launch(folder, self.spec, relay, server=server, local_endpoint="http://127.0.0.1:20651")
        self.assertNotEqual(cwd, folder / "workspace")
        self.assertIn("agents.enabled=false", args)
        self.assertTrue(any('tools."read".output_token_limit=100000' in arg for arg in args))
        self.assertIn("tool_output_token_limit=100000", args)
        self.assertFalse((Path(env["CODEX_HOME"]) / "auth.json").exists())
        self.assertEqual(Path(env["HOME"]), Path(env["CODEX_HOME"]).parent / "home")
        self.assertNotEqual(Path(env["HOME"]), Path.home())
        self.assertIn('developer_instructions="MCP instructions: navigation\\n\\nRead source.\\n"', args)
        folder = self.root / "launch-a"; folder.mkdir()
        with patch("benchmark.claude_client.executable", return_value="/usr/bin/false"), patch("benchmark.claude_client.managed_policy", return_value={}):
            args, cwd, env = claude_client.launch(folder, settings.execution_settings("claude-opus-5-5"), relay,
                server=server, local_endpoint="http://127.0.0.1:20752")
        self.assertEqual(args[args.index("--setting-sources") + 1], "")
        self.assertEqual(set(args[args.index("--tools") + 1].split(",")), set(claude_client.RESOURCE_TOOLS))
        self.assertEqual(set(args[args.index("--allowedTools") + 1].split(",")), {*claude_client.RESOURCE_TOOLS, "mcp__navigation__read"})
        self.assertIn("--restricted", args)
        self.assertEqual(env["MAX_MCP_OUTPUT_TOKENS"], "50000")
        self.assertEqual(env["ENABLE_TOOL_SEARCH"], "false")
        from .pi_client import private_root
        self.assertEqual(env["HOME"], str(private_root(folder) / "home"))
        self.assertEqual(env["TMPDIR"], str(private_root(folder) / "home/tmp"))
        folder = self.root / "launch-production"; folder.mkdir()
        with patch("benchmark.claude_client.executable", return_value="/usr/bin/false"), \
                patch("benchmark.claude_client.managed_policy", return_value={}), \
                patch("benchmark.claude_client.operator_token", return_value=(None, "missing")):
            with self.assertRaisesRegex(ContractError, "All Claude arms require"):
                claude_client.launch(folder, settings.execution_settings("claude-opus-5-5"), relay, server=server)
        with patch("benchmark.claude_client.executable", return_value="/usr/bin/false"), \
                patch("benchmark.claude_client.managed_policy", return_value={}), \
                patch("benchmark.claude_client.operator_token", return_value=("fixture-value", "oauth_env")):
            args, cwd, env = claude_client.launch(folder, settings.execution_settings("claude-opus-5-5"), relay, server=server)
        self.assertEqual(env["HOME"], str(private_root(folder) / "home"))
        self.assertEqual(env["CLAUDE_CODE_OAUTH_TOKEN"], "fixture-value")
        self.assertNotIn("ANTHROPIC_BASE_URL", env)
        self.assertTrue(claude_client.cleanup_private(folder, cwd, env)["complete"])

    def test_claude_operator_token_errors_hide_paths_and_env_is_consumed(self):
        from . import claude_client, core
        with patch.object(claude_client, "_ENV_TOKEN", None), patch.dict(os.environ, {}, clear=True):
            missing = self.root / "sensitive-location"
            os.environ["CODEMAP_BENCH_CLAUDE_TOKEN_FILE"] = str(missing)
            with self.assertRaises(ValueError) as error:
                claude_client.operator_token()
            self.assertNotIn(str(missing), str(error.exception))
            self.assertIsNone(error.exception.__cause__)
            linked = self.root / "linked-token"
            linked.symlink_to(missing)
            os.environ["CODEMAP_BENCH_CLAUDE_TOKEN_FILE"] = str(linked)
            with self.assertRaises(ValueError) as error:
                claude_client.operator_token()
            self.assertNotIn(str(linked), str(error.exception))
            os.environ.pop("CODEMAP_BENCH_CLAUDE_TOKEN_FILE")
            os.environ["CLAUDE_CODE_OAUTH_TOKEN"] = "fixture-value"
            self.assertEqual(core.command([sys.executable, "-c", "import os; print('CLAUDE_CODE_OAUTH_TOKEN' in os.environ)"]).strip(), "False")
            self.assertEqual(claude_client.operator_token(), ("fixture-value", "oauth_env"))
            self.assertNotIn("CLAUDE_CODE_OAUTH_TOKEN", os.environ)
            self.assertEqual(claude_client.environment()["CLAUDE_CODE_OAUTH_TOKEN"], "fixture-value")
            self.assertEqual(claude_client.operator_token(), ("fixture-value", "oauth_env"))

    def test_claude_cleanup_removes_all_private_home_residue_without_reading_it(self):
        from . import claude_client
        from .pi_client import private_root, claude_project_slug
        folder = self.root / "private"; folder.mkdir()
        workspace = private_root(folder) / "work"; workspace.mkdir()
        home = private_root(folder) / "home"; home.mkdir()
        env = {"HOME": str(home)}
        claude_client.prepare_private(folder, workspace, env)
        project = home / ".claude/projects" / claude_project_slug(workspace)
        project.mkdir(parents=True)
        (project / "session.jsonl").write_text(json.dumps({"cwd": str(workspace), "type": "user"}) + "\n")
        output = project / "session/tool-results/result.txt"; output.parent.mkdir(parents=True); output.write_text("a" * 600000)
        for name in (".claude/backups/backup", "Library/Caches/claude-cli-nodejs/local/mcp-logs-navigation/log", ".claude.json"):
            path = home / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(b"\xff")
        result = claude_client.cleanup_private(folder, workspace, env)
        self.assertTrue(result["complete"])
        self.assertTrue(result["operator_home_observation"]["passed"])
        self.assertFalse(home.exists())
        self.assertEqual(result["claude_persisted_tool_results"], {"count": 1, "bytes": 600000})
        self.assertEqual(result["private_home_residue"]["files"], 5)
        self.assertEqual(result["private_home_residue"]["categories"][".claude/backups"]["files"], 1)

    def test_claude_operator_home_observation_fails_only_on_harness_related_entries(self):
        from .claude_client import operator_home_observation
        from .pi_client import claude_project_slug
        workspace = self.root / "work"
        own = ".claude/projects/" + claude_project_slug(workspace) + "/a.jsonl"
        unrelated = ".claude/projects/-unrelated-work/b.jsonl"
        before = {"claude_json": None, "trees": {".claude/projects": {own: {"size": 1}, unrelated: {"size": 1}}}}
        after = copy.deepcopy(before); after["trees"][".claude/projects"][unrelated]["size"] = 2
        with patch("benchmark.claude_client.operator_home_state", return_value=after):
            self.assertTrue(operator_home_observation(before, [workspace])["passed"])
            other_slot = ".claude/projects/" + claude_project_slug(self.root / "other-slot/work") + "/c.jsonl"
            before["trees"][".claude/projects"][other_slot] = {"size": 1}
            after["trees"][".claude/projects"][other_slot] = {"size": 2}
            self.assertFalse(operator_home_observation(before, [workspace])["passed"])
            after["trees"][".claude/projects"][other_slot]["size"] = 1
            after["trees"][".claude/projects"][own]["size"] = 2
            self.assertFalse(operator_home_observation(before, [workspace])["passed"])
            after["trees"][".claude/projects"][own]["size"] = 1
            # Concurrent sessions (the parent agent, other projects) create files and rewrite .claude.json.
            after["trees"][".claude/projects"][".claude/projects/-unrelated-work/new.jsonl"] = {"size": 1}
            after["claude_json"] = {"size": 9}
            observed = operator_home_observation(before, [workspace])
            self.assertTrue(observed["passed"])
            self.assertEqual(observed["concurrent_session_changes"],
                             {"new": 1, "removed": 0, "changed": 1, "claude_json_changed": True})
            after["trees"][".claude/projects"][own.replace("a.jsonl", "new.jsonl")] = {"size": 1}
            self.assertFalse(operator_home_observation(before, [workspace])["passed"])

    def test_cli_context_audit_rejects_user_skills_home_and_hosted_tools(self):
        from . import cli_audit
        # A cache outside HOME: a TMPDIR under HOME would trip the operator-home check itself.
        cache = Path("/private/var/fixture-cache")
        root = str(cache / "0123456789abcdef/codex-home/skills/.system")
        section = ("<skills_instructions>\n### Skill roots\n- `r0` = `" + root + "`\n### Available skills\n"
                   "- imagegen: Built-in skill. (file: r0/imagegen/SKILL.md)\n</skills_instructions>")
        request = {"input": [{"role": "developer", "content": section}], "tools": [{"type": "namespace", "name": "functions",
                   "tools": [{"name": "exec"}, {"name": "wait"}]}]}
        with patch.object(settings, "CACHE", cache):
            self.assertEqual(cli_audit.audit_request(request, {}), [])
            self.assertEqual(cli_audit.skill_context(request)[0][0]["name"], "imagegen")
        self.assertTrue(cli_audit.audit_request({"system": section.replace(root, str(Path.home() / ".agents/skills"))}, {}))
        self.assertTrue(cli_audit.audit_request({"system": "Use ~/.agents/skills"}, {}))
        self.assertTrue(cli_audit.audit_request({"messages": [{"role": "user", "content": str(Path.home())}]}, {}))
        request["tools"].append({"type": "web_search"})
        self.assertIn("web_search", cli_audit.declared_tools(request))
        self.assertTrue(cli_audit.audit_tools(request, self.spec, None, {}))
        resources = {"tools": [{"name": name} for name in ("mcp__navigation__read", "ListMcpResourcesTool", "ReadMcpResourceTool")]}
        self.assertEqual(cli_audit.audit_tools(resources, {"solver_backend": "claude_code"}, {"enabled_tools": ["read"]},
                         {"initialize": {"capabilities": {"resources": {}}}}), [])
        self.assertTrue(cli_audit.audit_tools(resources, {"solver_backend": "claude_code"}, {"enabled_tools": ["read"]}, {}))

    def test_codex_native_command_attribution_uses_preceding_custom_call_turn(self):
        from .route_metrics import cli_observations
        folder = self.root / "turns"; folder.mkdir()
        write_json(folder / "runtime.json", {"backend": "codex"})
        write_json(folder / "native-agent.json", {})
        (folder / "events.jsonl").write_text('{}\n')
        rows = []
        for number in (1, 2):
            rows += [{"type": "response_item", "payload": {"type": "custom_tool_call", "id": f"c{number}"}},
                     {"type": "token_usage_record", "payload": {}},
                     {"type": "event_msg", "payload": {"type": "item_completed", "item": {"type": "CommandExecution", "id": f"n{number}"}}}]
        rows.append({"type": "token_usage_record", "payload": {}})
        (folder / "rollout.jsonl").write_text("".join(json.dumps(row) + "\n" for row in rows))
        observed = cli_observations(folder, [{}, {}], call_log_complete=True)
        self.assertEqual(observed["tool_calls_per_turn"], [1, 1, 0])
        self.assertEqual(observed["model_tool_calls_per_turn"], [1, 1, 0])

    def test_proxy_peer_pid_matches_client_endpoint_not_server_or_ambiguous_owner(self):
        from .isolation import proxy_peer_pid
        output = "p21\nn127.0.0.1:20798->127.0.0.1:51000\np22\nn127.0.0.1:51000->127.0.0.1:20798\n"
        with patch("benchmark.isolation.subprocess.run", return_value=subprocess.CompletedProcess([], 0, output)):
            self.assertEqual(proxy_peer_pid(("127.0.0.1", 51000), ("127.0.0.1", 20798)), 22)
        output += "p23\nn127.0.0.1:51000->127.0.0.1:20798\n"
        with patch("benchmark.isolation.subprocess.run", return_value=subprocess.CompletedProcess([], 0, output)):
            self.assertIsNone(proxy_peer_pid(("127.0.0.1", 51000), ("127.0.0.1", 20798)))
        with patch("benchmark.isolation.subprocess.run", side_effect=subprocess.TimeoutExpired("lsof", 5)):
            self.assertIsNone(proxy_peer_pid(("127.0.0.1", 51000), ("127.0.0.1", 20798)))

    @unittest.skipUnless(sys.platform == "darwin", "TCP owner verification uses Darwin lsof")
    def test_claude_proxy_allows_only_authenticated_cli_socket_owner(self):
        import selectors
        import socket
        import socketserver
        class Echo(socketserver.BaseRequestHandler):
            def handle(self):
                self.request.settimeout(5)
                self.request.sendall(self.request.recv(16))
        class Local(socketserver.TCPServer):
            allow_reuse_address = True
        with Local(("127.0.0.1", 20797), Echo) as upstream:
            upstream.timeout = 5
            thread = threading.Thread(target=upstream.handle_request); thread.start()
            log = self.root / "proxy.jsonl"
            code = ("import socket; from pathlib import Path; from benchmark.isolation import _serve_proxy; "
                    "original=socket.create_connection; "
                    "socket.create_connection=lambda target,timeout: original(('127.0.0.1',20797),timeout) "
                    "if target==('api.anthropic.com',443) else (_ for _ in ()).throw(ValueError('unexpected upstream')); "
                    f"_serve_proxy(20798,Path({str(log)!r}),{os.getpid()})")
            proxy = subprocess.Popen([sys.executable, "-c", code], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                with selectors.DefaultSelector() as selector:
                    selector.register(proxy.stdout, selectors.EVENT_READ)
                    self.assertTrue(selector.select(5), "proxy did not start")
                    self.assertEqual(proxy.stdout.readline(), b"ready\n")
                with socket.create_connection(("127.0.0.1", 20798), timeout=5) as connection:
                    connection.sendall(b"CONNECT api.anthropic.com:443 HTTP/1.1\r\n\r\n")
                    self.assertIn(b"200 Connection Established", connection.recv(4096))
                    connection.sendall(b"echo")
                    self.assertEqual(connection.recv(16), b"echo")
                for request in ("CONNECT api.anthropic.com:443 HTTP/1.1\r\n\r\n", "GET / HTTP/1.1\r\n\r\n"):
                    child_code = ("import socket; s=socket.create_connection(('127.0.0.1',20798),timeout=5); "
                                  f"s.sendall({request.encode()!r}); print(s.recv(4096).decode())")
                    child = subprocess.run([sys.executable, "-c", child_code], capture_output=True, text=True, timeout=10)
                    self.assertEqual(child.returncode, 0, child.stderr)
                    self.assertIn("403 Forbidden", child.stdout)
                rows = read_jsonl(log)
                self.assertTrue(rows[0]["permitted"])
                self.assertEqual(rows[0]["peer_pid"], os.getpid())
                self.assertEqual(len(rows), 3)
                self.assertTrue(all(not row["permitted"] and row["reason"] == "not_cli_socket_owner" for row in rows[1:]))
            finally:
                proxy.terminate()
                try:
                    proxy.communicate(timeout=5)
                except subprocess.TimeoutExpired:
                    proxy.kill(); proxy.communicate(timeout=5)
                thread.join(timeout=6)
            self.assertFalse(thread.is_alive())

    def test_cli_context_audit_covers_native_instructions_and_every_snapshot(self):
        from . import cli_audit
        inventory = {"initialize": {"instructions": "Use codemap for source."}, "tools": [{"name": "read", "description": "codemap source read"}]}
        request = {"system": "Use codemap for source.", "tools": [{"name": "mcp__navigation__read", "description": "codemap source read"}], "messages": [{"role": "user", "content": "Inspect source."}]}
        self.assertEqual(cli_audit.audit_request(request, inventory), [])
        request["messages"][0]["content"] = "Use codemap for source."
        self.assertTrue(cli_audit.audit_request(request, inventory))
        folder = self.root / "session"; folder.mkdir(parents=True)
        rows = [{"type": "attachment", "attachment": {"type": "prompt_snapshot", "systemPrompt": ["Use codemap for source."]}},
                {"type": "user", "message": {"role": "user", "content": "Inspect source."}},
                {"type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "content": "Grafana source body"}]}}]
        path = folder / "claude-session-local.jsonl"
        path.write_text("".join(json.dumps(r) + "\n" for r in rows))
        self.assertTrue(cli_audit.audit_session(folder, inventory, "claude_code")["passed"])
        rows.append({"type": "attachment", "attachment": {"type": "prompt_snapshot", "systemPrompt": ["Grafana hint"]}})
        path.write_text("".join(json.dumps(r) + "\n" for r in rows))
        self.assertFalse(cli_audit.audit_session(folder, inventory, "claude_code")["passed"])
        self.assertTrue(cli_audit.audit_tools({"tools": [{"name": "Bash"}]}, {"solver_backend": "claude_code"}, {"enabled_tools": ["read"]}, {}))

    def test_schedule_workers_serializes_arms_and_rounds(self):
        from . import execution, route_contract as routes
        from types import SimpleNamespace
        lock = threading.Lock()
        active, finished, maximum = set(), [], [0]
        rows = routes.schedule()
        def worker(row):
            with lock:
                self.assertNotIn(row["arm"], active)
                self.assertTrue(all(r in finished for r in [s["id"] for s in rows if s["attempt"] < row["attempt"]]))
                active.add(row["arm"])
                maximum[0] = max(maximum[0], len(active))
            time.sleep(.002)
            with lock:
                active.remove(row["arm"])
                finished.append(row["id"])
        coordinator = SimpleNamespace(drain=lambda: None, fatal=False, ledger={})
        execution.schedule_workers(rows, coordinator, threading.Event(), worker, solver_rounds=True)
        self.assertNotIn("error", coordinator.ledger)
        self.assertEqual(len(finished), len(rows))
        self.assertLessEqual(maximum[0], 4)

    def test_pi_audit_native_provenance_never_exempts_user_or_paths(self):
        from . import pi_audit, pi_client
        native = {"initialize": {"instructions": "Use Serena to inspect source."}, "tools": []}
        body = {"instructions": pi_client.server_instructions_prompt(native["initialize"]["instructions"]),
                "input": [{"role": "user", "content": "Inspect source."}]}
        self.assertEqual(pi_audit.audit_request(body, native), [])
        self.assertTrue(pi_audit.audit_system({"sections": {"preamble": "Use Serena to inspect source."}}, native))
        body["input"][0]["content"] = "Use Serena to inspect source."
        self.assertTrue(pi_audit.audit_request(body, native))
        self.assertTrue(pi_audit.audit_text("SMOKE grafana alerting-alertmanager"))
        text = "Source: file:///tmp/%62ench/private.txt"
        native["initialize"]["instructions"] = text
        self.assertTrue(any(v["kind"] == "absolute_path" for v in pi_audit.audit_text(text, inventory=native, product_owned=True)))
        body = {"instructions": "neutral", "metadata": {"path": "/tmp/graphify/data"}}
        self.assertTrue(pi_audit.audit_request(body, {}))

    def test_pi_audit_checks_every_system_patch_and_full_user(self):
        from . import pi_audit
        folder = self.root / "audit"
        session = folder / "pi-session"
        session.mkdir(parents=True)
        rows = [{"type": "message", "message": {"role": "system", "sections": {"cwd": "/neutral/work"}}},
                {"type": "message", "message": {"role": "user", "content": "Inspect source."}}]
        path = session / "session.jsonl"
        path.write_text("".join(json.dumps(row) + "\n" for row in rows))
        self.assertTrue(pi_audit.audit_session(folder, {})["passed"])
        rows.append({"type": "message", "message": {"role": "system", "sections": {"rules": "Grafana hint"}}})
        path.write_text("".join(json.dumps(row) + "\n" for row in rows))
        result = pi_audit.audit_session(folder, {})
        self.assertFalse(result["passed"])
        self.assertEqual(result["system_messages_including_patches"], 2)

    def test_read_ranges_policy_uses_each_advertised_path_without_rewriting(self):
        from .isolation import RelayPolicy
        source = self.root / "source"
        source.mkdir()
        (source / "a.py").write_text("pass\n")
        schema = {"name": "read", "inputSchema": {"properties": {"file_path": {"type": "string"},
                  "ranges": {"type": "array", "minItems": 1, "maxItems": 20}}}}
        policy = RelayPolicy("codemap-search", {"source": str(source)}, [schema], tracked=["a.py"])
        for alias in ("file_path", "filePath", "file-path", "FilePath", "path", "file", "query"):
            args = {"ranges": [{alias: "a.py", "offset": 1, "limit": 1}, {alias: (source / "a.py").as_uri()}]}
            original = copy.deepcopy(args)
            policy.validate("read", args)
            self.assertEqual(args, original)
            for bad in ("../private", ".git/config", "/tmp/private"):
                with self.assertRaises(ContractError):
                    policy.validate("read", {"ranges": [{alias: "a.py"}, {alias: bad}]})
        for args in ({"ranges": []}, {"ranges": [{"file_path": "a.py"}] * 21},
                     {"ranges": [{"file_path": "a.py"}], "file_path": "a.py"},
                     {"ranges": [{"path": "a.py", "file": "../private"}]}):
            with self.assertRaises(ContractError):
                policy.validate("read", args)
        for window in ({"start_line": "1", "end_line": "2"}, {"startLine": "1", "end-line": "2"},
                       {"start": 1, "end": 2}, {"offset": "1", "limit": "2"}):
            for args in ({"filePath": "a.py", **window}, {"ranges": [{"file-path": "a.py", **window}]}):
                before = copy.deepcopy(args)
                policy.validate("read", args)
                self.assertEqual(args, before)
        for args in ({"file_path": "a.py", "filePath": "../private"}, {"filePath": "a.py", "unknownWindow": 1},
                     {"ranges": [{"filePath": "a.py", "unknownWindow": 1}]}):
            with self.assertRaises(ContractError):
                policy.validate("read", args)
        nullable = {"file_path": "a.py", "ranges": None}
        policy.validate("read", nullable)
        self.assertEqual(nullable, {"file_path": "a.py", "ranges": None})
        with self.assertRaises(ContractError):
            policy.validate("read", {"file_path": "../private", "ranges": None})
        schema["inputSchema"]["properties"].pop("ranges")
        with self.assertRaises(ContractError):
            policy.validate("read", {"ranges": [{"file_path": "a.py"}]})

    def test_pi_observations_separate_model_tools_and_script_envelopes(self):
        from .route_metrics import pi_observations
        events = [{"type": "turn_start"},
                  {"type": "message_end", "message": {"role": "assistant", "durationMs": 250}},
                  {"type": "tool_execution_start", "toolName": "codemode", "toolCallId": "script"},
                  {"type": "tool_execution_start", "toolName": "navigation_read", "toolCallId": "script/1", "parentToolCallId": "script"},
                  {"type": "tool_execution_end", "toolName": "navigation_read", "toolCallId": "script/1", "result": {"details": {"outputGuard": {"truncated": True}}}},
                  {"type": "tool_execution_end", "toolName": "codemode", "toolCallId": "script", "result": {"content": [{"text": "Warning: truncated output (original token count: 20000)"}]}},
                  {"type": "turn_end"}, {"type": "agent_settled"}]
        events.insert(4, {"type": "tool_execution_start", "toolName": "navigation_read", "toolCallId": "outside"})
        write_json(self.root / "pi-policy.json", {"measurement_mode": "codemode"})
        (self.root / "events.jsonl").write_text("".join(json.dumps(e) + "\n" for e in events))
        calls = [{"arm": "codemap-search", "arguments": {"include_seen": True}, "dispatched": True, "duration_seconds": .5}]
        observed = pi_observations(self.root, calls, call_log_complete=True)
        self.assertEqual(observed["model_seconds"], .25)
        self.assertEqual(observed["tool_seconds"], .5)
        self.assertEqual(observed["tool_calls_per_turn"], [2])
        self.assertEqual(observed["model_tool_calls_per_turn"], [2])
        self.assertEqual(observed["direct_calls_outside_script"], 1)
        for key in ("adapter_output_guard_truncated", "codemode_output_truncated", "include_seen_true_calls"):
            self.assertEqual(observed[key], 1)

    def test_pi_observations_separate_bash_content_and_nested_structured_truncation(self):
        from .route_metrics import pi_observations
        events = [{"type": "tool_execution_end", "toolName": "bash", "toolCallId": "direct", "result": {
                      "details": {"truncation": {"truncated": True}}, "structuredContent": {"truncated": True}}},
                  {"type": "tool_execution_end", "toolName": "bash", "toolCallId": "script/1", "parentToolCallId": "script", "result": {
                      "details": {"truncation": {"truncated": True}}, "structuredContent": {"truncated": False}}},
                  {"type": "tool_execution_end", "toolName": "bash", "toolCallId": "script/2", "parentToolCallId": "script", "result": {
                      "details": {"truncation": {"truncated": True}}, "structuredContent": {"truncated": True}}},
                  {"type": "agent_settled"}]
        (self.root / "events.jsonl").write_text("".join(json.dumps(e) + "\n" for e in events))
        observed = pi_observations(self.root, [], call_log_complete=True)
        self.assertEqual(observed["bash_content_truncated"], 1)
        self.assertEqual(observed["bash_nested_structured_content_truncated"], 1)
        self.assertEqual(observed["codemode_output_truncated"], 0)
        self.assertEqual(observed["adapter_output_guard_truncated"], 0)

    def test_pi_output_guard_covers_native_utf8_budgets(self):
        from .pi_client import adapter_settings
        self.assertGreaterEqual(adapter_settings("codemap-search")["outputGuard"]["maxBytes"], 100000)
        text = "😀" * 150000
        appended = text + "\nstructuredContent:\n" + json.dumps({"result": text}, ensure_ascii=False, indent=2)
        self.assertGreaterEqual(adapter_settings("serena")["outputGuard"]["maxBytes"], len(appended.encode()))
        self.assertNotIn("appendStructuredContent", adapter_settings("serena"))
        for arm in ("codemap-search", "serena", "codegraph", "zvec-grep", "graphify", "codebase-memory-mcp"):
            config = adapter_settings(arm)
            self.assertFalse(config["semanticSearch"])
            self.assertGreater(config["outputGuard"]["maxLines"], config["outputGuard"]["maxBytes"])

    def test_pi_codemode_extension_defaults_output_without_changing_description(self):
        from .pi_client import codemode_extension_source
        factory = self.root / "@earendil-works/pi-coding-agent/dist/extensions/codemode/index.js"
        parser = self.root / "@earendil-works/pi-codemode/dist/source.js"
        factory.parent.mkdir(parents=True)
        parser.parent.mkdir(parents=True)
        write_json(self.root / "package.json", {"type": "module"})
        factory.write_text('''export const createCodemodeExtension = options => pi => pi.registerTool({
  name: "codemode", description: "unchanged native description", parameters: {type: "object"},
  execute: (...args) => args[1].code.includes('fail')
    ? {isError: true, content: [{type: 'text', text: 'Script failed'}, {type: 'image'},
       {type: 'text', text: 'printed codemode.js:9:1\\nScript error:\\nError: x\\n    at <anonymous> (codemode.js:3:1)'}]}
    : {args, models: options.models}
});''')
        parser.write_text('''export function parseCodemodeSource(input) {
  if (input === "invalid") throw new Error("native parse error");
  const newline = input.indexOf("\\n");
  const directive = input.startsWith("// @options:");
  const options = directive ? JSON.parse(input.slice(12, newline)) : {};
  return {code: directive ? input.slice(newline) : input,
          options: {maxOutputTokens: options.max_output_tokens, timeoutMs: options.timeout_ms}};
}''')
        extension = self.root / "script-tools.mjs"
        extension.write_text(codemode_extension_source(factory))
        script = '''import assert from 'node:assert/strict';
import extension from %s;
let tool;
extension({registerTool: value => {tool = value;}});
assert.equal(tool.description, 'unchanged native description');
assert.deepEqual(tool.parameters, {type:'object'});
for (const options of [null, {timeout_ms:900}, {max_output_tokens:32, timeout_ms:700}, {max_output_tokens:0}]) {
  const params = {code: (options ? '// @options: '+JSON.stringify(options)+'\\n' : '')+'text("sample");'};
  const result = await tool.execute('id', params, 'signal', 'update', 'context');
  assert.equal(result.models, false);
  assert.deepEqual(result.args.slice(2), ['signal','update','context']);
  if (options?.max_output_tokens !== undefined) assert.equal(result.args[1], params);
  else {
    const [header, ...body] = result.args[1].code.split('\\n');
    const applied = JSON.parse(header.slice(12));
    assert.equal(applied.max_output_tokens, 100000);
    assert.equal(applied.timeout_ms, options?.timeout_ms);
    // Script lines keep their positions when the model already wrote an options line.
    assert.deepEqual(body, ['text("sample");']);
  }
}
const failed = await tool.execute('id', {code: 'fail();'});
assert.equal(failed.content[2].text, 'printed codemode.js:9:1\\nScript error:\\nError: x\\n    at <anonymous> (codemode.js:2:1)');
assert.deepEqual(failed.content.slice(0, 2), [{type: 'text', text: 'Script failed'}, {type: 'image'}]);
const explicit = await tool.execute('id', {code: '// @options: {"timeout_ms":5}\\nfail();'});
assert.match(explicit.content[2].text, /codemode\\.js:3:1/);
assert.throws(() => tool.execute('id', {code:'invalid'}), /native parse error/);
''' % json.dumps(extension.as_uri())
        result = subprocess.run(["node", "--input-type=module"], input=script, text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)

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


    def test_solver_contract_prompt_and_readiness_require_unlimited_execution(self):
        from . import execution, route_contract as routes, route_prepare
        from .core import digest
        profile = routes.load_profile()
        self.assertIsNone(profile["limits"])
        self.assertNotIn("token_limit_basis", profile)
        self.assertIsNone(settings.execution_settings()["limits"])
        self.assertEqual(profile["grader_timeout_seconds"], 300)
        question = routes.solver_questions()[0]
        prompt = execution.answer_prompt(profile, question)
        self.assertEqual(prompt, runner.ANSWER_PROMPT + "\n" + question["prompt"])
        self.assertNotIn("실행 제한은", prompt)
        old = {**profile, "limits": {"elapsed_seconds": 300, "exploration_calls": 80, "total_tokens": 500000},
               "token_limit_basis": "input_excluding_cache_reads_plus_output"}
        path = self.root / "old-profile.json"
        write_json(path, old)
        with patch.object(routes, "PROFILE_PATH", path), self.assertRaises(ContractError):
            routes.load_profile()
        condition = {"contract_hash": "old-contract"}
        record = {**condition, "plan_id": digest(condition)[:32], "status": "prepared_not_executed",
                  "technical_ready": True, "execution_authorized": False}
        with patch.object(route_prepare, "plan_condition", return_value=condition):
            with self.assertRaisesRegex(ContractError, "route 실행 계약 변경"):
                route_prepare.verify_readiness(record)

    def test_authorize_run_keeps_grader_timeout_separate_from_solver_limits(self):
        from . import execution, route_contract as routes, route_prepare
        from .core import digest, file_digest, read_json
        condition = {"contract_hash": routes.contract()["contract_hash"]}
        planned = routes.schedule()
        prepared = {"plan_id": digest(condition)[:32], "models": {"solver": routes.solver_model()},
                    "planned_runs": planned,
                    "execution_specs": {routes.execution_key(arm, mode): settings.execution_settings()
                                        for arm in routes.ARM_IDS for mode in routes.solver_measurement_modes()},
                    "solver_inputs": {row["id"]: {"sha256": "fixture"} for row in planned}}
        path = self.root / "readiness.json"
        write_json(path, prepared)
        approval = {"confirmed": True, "actor": "offline fixture", "plan_id": prepared["plan_id"],
                    "readiness_sha256": file_digest(path), "solver_model": prepared["models"]["solver"]["model"]}
        with patch.object(route_prepare, "plan_condition", return_value=condition), \
                patch.object(routes, "PREPARATION", self.root / "preparation"):
            root, manifest = execution.authorize_run(prepared, path, approval)
        self.assertEqual(manifest["grader_timeout_seconds"], 300)
        self.assertTrue(all(spec["limits"] is None and not spec["preparation_only"] for spec in manifest["run_specs"].values()))
        self.assertTrue(all(spec["preparation_only"] for spec in prepared["execution_specs"].values()))
        self.assertTrue(all(row["solver_limits"] is None for row in read_json(root / "ledger.json")["slots"]))

    def test_runner_records_over_500000_tokens_without_disqualifying_answer(self):
        result = runner.execute_codex(self.root / "large-usage", "FIXTURE_LARGE", codex_version="fixture")
        self.assertEqual(result["status"], "completed")
        self.assertTrue(result["answer_eligible"] and result["conditions_valid"])
        self.assertEqual(result["token_measurement"]["observed_tokens_after_collection"], 600030)
        self.assertEqual(result["usage"]["observed"]["total_tokens"], 600060)
        self.assertIsNone(result["solver_limits"])
        self.assertIsNone(result["shutdown"]["cutoff"])
        self.assertNotIn("budget", result)
        self.assertNotIn("limit_tokens", result["token_measurement"])
        self.assertEqual(result["usage"], runner.recorded_usage(self.root / "large-usage"))
        eligibility = runner.answer_eligibility(backend="pi", completed=True, observed_monotonic=1000,
            final_row=None, start=0, started_unix_seconds=0, wall_deadline=None, cutoff=None, conditions_valid=True)
        self.assertTrue(eligibility["eligible"])
        self.assertIsNone(eligibility["effective_cutoff_monotonic"])

    def test_native_relay_counts_over_80_calls_and_still_honors_stop(self):
        from .route_transport import NativeRelay, append_jsonl
        relay = NativeRelay({"arm": "codemap-search", "source": str(self.root), "log": str(self.root / "relay.jsonl")})
        for ident in range(82):
            request = relay.admit(ident, "tools/call", {"name": "read", "arguments": {}}, lambda: None)
            append_jsonl(relay.log, {**request, "event": "terminal", "status": "completed"})
        self.assertEqual(relay.count, 82)
        self.assertEqual(relay.dispatched, 82)
        self.assertFalse((self.root / "stop.json").exists())
        write_json(self.root / "stop.json", {"reason": "interrupted"})
        with self.assertRaisesRegex(ContractError, "실행 중단 이후"):
            relay.admit(83, "tools/call", {"name": "read"}, lambda: None)
        self.assertEqual(relay.dispatched, 82)
        self.assertFalse(any("limit_calls" in row or row.get("event") == "limit" for row in read_jsonl(relay.log)))

    def test_pi_grader_retains_its_separate_timeout(self):
        spec = settings.execution_settings()
        spec.update(preparation_only=False, grader_timeout_seconds=.2)
        cli = self.root / "slow_pi.py"
        cli.write_text("import sys, time\nsys.stdin.read()\ntime.sleep(10)\n")
        tools = {"node": sys.executable, "pi_cli": str(cli), "versions": {"pi": "fixture"}}
        with patch.object(pi_grader.tool_artifacts, "pi_toolchain", return_value=tools), \
                patch.object(pi_grader.pi_client, "operator_token", return_value="fixture-token"):
            result = runner.execute_model(self.root / "grader-timeout", "fixture", settings=spec,
                                          grading=True, output_schema={})
        self.assertEqual(result["status"], "timeout")
        self.assertFalse(result["conditions_valid"])
        self.assertLess(result["elapsed_seconds"], 5)

    def test_explicit_settings_version_and_usage_reach_execution(self):
        spec = settings.execution_settings("claude-opus-5-5")
        spec["preparation_only"] = False
        spec["grader_timeout_seconds"] = 3
        cli = self.root / "fake_pi.py"
        cli.write_text('''import json, sys
sys.stdin.read()
print(json.dumps({"type":"message_end", "message":{"role":"assistant", "provider":"openai-codex",
    "model":"gpt-6-astra", "stopReason":"stop", "content":[{"type":"text", "text":"{}"}],
    "usage":{"input":70, "output":20, "cacheRead":30, "cacheWrite":0, "totalTokens":120}}}))
print(json.dumps({"type":"agent_settled", "aborted":False}))
''')
        tools = {"node": sys.executable, "pi_cli": str(cli), "versions": {"pi": "pi fixture-1.4.0"}}
        with patch.object(pi_grader.tool_artifacts, "pi_toolchain", return_value=tools, create=True), \
                patch.object(pi_grader.pi_client, "operator_token", return_value="fixture-token"):
            result = runner.execute_model(self.root / "session", "fixture", settings=spec, grading=True,
                                          output_schema={}, usage_drain_seconds=0, agent_version="claude fixture")
        self.assertTrue(result["conditions_valid"])
        self.assertEqual(result["status"], "completed")
        self.assertEqual(result["model"], "gpt-6-astra")
        self.assertEqual(result["backend"], "pi")
        self.assertEqual(result["provider"], "openai-codex")
        self.assertEqual(result["agent_version"], "pi fixture-1.4.0")
        self.assertEqual(result["harness_attempts"], 1)
        self.assertEqual(result["usage_collection"]["drain_seconds"], 0)
        self.assertEqual(result["usage"]["total_tokens"]["value"], 120)


    def test_zero_drain_cancel_reaps_process(self):
        cancelled = threading.Event()
        folder = self.root / "cancel-session"
        fixture_pid = settings.CACHE / digest({"private_root": str(folder.resolve())})[:16] / "fixture-pid"
        def cancel_when_started():
            deadline = time.monotonic() + 5
            while not fixture_pid.exists() and time.monotonic() < deadline:
                time.sleep(.01)
            cancelled.set()
        watcher = threading.Thread(target=cancel_when_started)
        watcher.start()
        start = time.monotonic()
        result = runner.execute_codex(folder, "FIXTURE_WAIT", cancel_event=cancelled, usage_drain_seconds=0,
                                      codex_version="fixture")
        watcher.join(timeout=6)
        self.assertFalse(watcher.is_alive())
        self.assertLess(time.monotonic() - start, 10)
        self.assertEqual(result["status"], "interrupted")
        self.assertEqual(result["usage"]["observed"]["total_tokens"], 240)
        self.assertIsNone(result["usage"]["total_tokens"]["value"])
        with self.assertRaises(ProcessLookupError):
            os.kill(int(fixture_pid.read_text()), 0)


    def test_operator_cancel_excludes_late_answer_but_certifies_completed_usage(self):
        spec = settings.execution_settings()
        spec["preparation_only"] = False
        spec["solver_backend"] = "codex"
        folder = self.root / "drain-session"
        fixture_pid = settings.CACHE / digest({"private_root": str(folder.resolve())})[:16] / "fixture-pid"
        cancelled = threading.Event()
        def cancel_when_started():
            deadline = time.monotonic() + 5
            while not fixture_pid.exists() and time.monotonic() < deadline:
                time.sleep(.01)
            cancelled.set()
        watcher = threading.Thread(target=cancel_when_started)
        watcher.start()
        result = runner.execute_codex(folder, "FIXTURE_DRAIN", settings=spec, usage_drain_seconds=0,
                                      codex_version="fixture", cancel_event=cancelled)
        watcher.join(timeout=6)
        self.assertFalse(watcher.is_alive())
        self.assertEqual(result["status"], "interrupted")
        self.assertEqual(result["answer"], "")
        self.assertTrue(result["usage_collection"]["post_cutoff_answer_excluded"])
        self.assertEqual((folder / "post-cutoff-answer.txt").read_text(), "제한 이후의 답변")
        self.assertEqual(result["usage"]["total_tokens"]["value"], 240)
        self.assertIsNone(result["token_measurement"]["observed_cleanup_tokens"])
        self.assertEqual([step["signal"] for step in result["shutdown"]["steps"]], ["SIGINT"])
        self.assertTrue(result["conditions_valid"])
        self.assertFalse(result["answer_eligible"])


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


    def test_source_inventory_rejects_untracked_and_ignored_before_indexing(self):
        from . import execution, tool_indexes
        from .core import digest
        source = self.root / "grafana"
        source.mkdir()
        (source / "tracked.py").write_text("pass\n")
        (source / ".gitignore").write_text(".codemap/\nignored.txt\n")
        def git(*args):
            return subprocess.check_output(["git", *args], cwd=source, stderr=subprocess.DEVNULL, text=True)
        git("init", "-q")
        git("add", ".")
        git("-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture")
        inventory = tool_indexes.tracked_manifest(source)
        corpus = {"source": str(source), "files": inventory, "source_sha256": digest(inventory)}
        state = {"id": "codemap-search", "source": str(source)}
        for name in ("untracked.txt", "ignored.txt", ".codemap/config.toml"):
            path = source / name
            path.parent.mkdir(exist_ok=True)
            path.write_text("fixture")
            with self.assertRaisesRegex(ContractError, name):
                tool_indexes.tracked_manifest(source)
            with self.assertRaisesRegex(ContractError, name):
                execution.verify_source(state, corpus)
            with patch.object(tool_indexes.settings, "CACHE", self.root), \
                 patch.object(tool_indexes.routes, "PREPARATION", self.root / "preparation"), \
                 patch.object(tool_indexes.routes, "SOURCE_COMMIT", git("rev-parse", "HEAD").strip()), \
                 patch.object(tool_indexes.routes, "contract", return_value={"contract_hash": "fixture"}), \
                 patch.object(tool_indexes, "read_json", return_value={"contract_hash": "fixture"}):
                with self.assertRaisesRegex(ContractError, name):
                    tool_indexes.prepare_source(emit=lambda _: None)
            with patch.object(tool_indexes, "workspace_root", return_value=self.root / "copy"):
                with self.assertRaisesRegex(ContractError, name):
                    tool_indexes.workspace("codemap-search", corpus)
                self.assertFalse((self.root / "copy/source").exists())
            if name.startswith(".codemap/"):
                self.assertEqual(tool_indexes.tracked_manifest(source, allowed_metadata={".codemap"}), inventory)
            path.unlink()
        tool_indexes.verify_source_inventory(source, inventory)
        destination = self.root / "copy/source"
        runner.copy_source(source, destination)
        (destination / "ignored.txt").write_text("contaminated work copy")
        with patch.object(tool_indexes, "workspace_root", return_value=destination.parent):
            with self.assertRaisesRegex(ContractError, "ignored.txt"):
                tool_indexes.workspace("codemap-search", corpus)


    def test_pi_audit_includes_runtime_errors_and_fstrings(self):
        from . import pi_audit, route_transport
        messages = pi_audit.model_error_messages()
        for message in ("MCP 응답 형식 오류", "source 경로가 없습니다", "잘못된 read 범위", "도구 인자 타입 오류: {}"):
            self.assertIn(message, messages)
        original = pi_audit.inspect.getsource
        for target in (route_transport.MCPClient, route_transport.auxiliary_call, route_transport.source_path):
            with patch.object(pi_audit.inspect, "getsource", side_effect=lambda obj: 'def injected():\n    raise ValueError(f"Grafana {key}")\n' if obj is target else original(obj)):
                self.assertTrue(pi_audit.audit_text(pi_audit.model_error_messages()))


    def test_schedule_workers_grading_has_no_round_barrier(self):
        from . import execution
        from types import SimpleNamespace
        later = threading.Event()
        rows = [{"id": "a1", "arm": "a", "attempt": 1}, {"id": "b1", "arm": "b", "attempt": 1},
                {"id": "b2", "arm": "b", "attempt": 2}]
        def worker(row):
            if row["id"] == "a1":
                self.assertTrue(later.wait(2), "grader incorrectly waited for the previous round")
            elif row["id"] == "b2":
                later.set()
        coordinator = SimpleNamespace(drain=lambda: None, fatal=False, ledger={})
        execution.schedule_workers(rows, coordinator, threading.Event(), worker)
        self.assertNotIn("error", coordinator.ledger)


    def test_pi_codemode_reconciles_only_proven_nested_cancellations(self):
        from . import pi_client
        events = [{"type": "tool_execution_start", "toolCallId": "s", "toolName": "codemode", "args": {}},
                  {"type": "tool_execution_start", "toolCallId": "s/1", "parentToolCallId": "s", "toolName": "bash", "args": {"command": "sleep 2"}},
                  {"type": "tool_execution_end", "toolCallId": "s", "toolName": "codemode", "isError": True,
                   "result": {"details": {"calls": [{"id": "s/?", "name": "bash", "args": '{"command":"sleep 2"}', "status": "cancelled"}]}}},
                  {"type": "message_end", "message": {"role": "assistant", "stopReason": "stop", "content": [{"type": "text", "text": "Done"}],
                   "usage": {"input": 1, "output": 1, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 2}}}, {"type": "agent_settled"}]
        state = pi_client.EventState()
        for event in events:
            state.add(event)
        self.assertEqual(list(state.tools.values()), pi_client.tool_items(events))
        self.assertEqual(state.tools["s/1"]["status"], "cancelled")
        self.assertNotIn("result", state.tools["s/1"])  # No invented output/end timestamp.
        self.assertTrue(state.usage(terminal_complete=True, evidence="fixture")["complete"])
        items = list(state.tools.values())
        before = pi_client.cancelled_before_admission(items, [], native=True)
        self.assertEqual(len(before["calls"]), 1)
        self.assertEqual(before["calls"][0]["match"], "parent+name+args unique")
        self.assertFalse(before["calls"][0]["budget_consumed"])
        self.assertEqual(pi_client.cancelled_before_admission(items, [{"command": "sleep 2"}], native=True)["calls"], [])
        for field, value in (("status", "error"), ("args", '{"command":"different"}'), ("id", "other/?")):
            changed = copy.deepcopy(events)
            changed[2]["result"]["details"]["calls"][0][field] = value
            self.assertFalse(pi_client.usage(changed, terminal_complete=True, evidence="fixture")["complete"])
        duplicate = copy.deepcopy(events)
        duplicate.insert(2, {**events[1], "toolCallId": "s/2"})
        self.assertFalse(pi_client.usage(duplicate, terminal_complete=True, evidence="fixture")["complete"])
        duplicate[3]["result"]["details"]["calls"] *= 2
        self.assertTrue(pi_client.usage(duplicate, terminal_complete=True, evidence="fixture")["complete"])
        unmatched = pi_client.cancelled_before_admission(pi_client.tool_items(duplicate), [], native=True)
        self.assertEqual(unmatched["calls"], [])
        self.assertEqual(set(unmatched["ambiguous"]), {"s/1", "s/2"})
        product = copy.deepcopy(events)
        product[1]["toolName"] = product[2]["result"]["details"]["calls"][0]["name"] = "navigation_read"
        product[1]["args"] = {"file_path": "source.py"}
        product[2]["result"]["details"]["calls"][0]["args"] = '{"file_path": "source.py"}'
        items = pi_client.tool_items(product)
        self.assertEqual(len(pi_client.cancelled_before_admission(items, [], native=False, enabled_tools=["read"])["calls"]), 1)
        admitted = [{"method": "tools/call", "tool": "read", "arguments": {"file_path": "source.py"}}]
        self.assertEqual(pi_client.cancelled_before_admission(items, admitted, native=False, enabled_tools=["read"])["calls"], [])
        # Pinned codemode keeps only a 197-unit argument preview; long commands match by that prefix, shared prefixes stay unresolved.
        long = copy.deepcopy(events)
        long[1]["args"] = {"command": "rg -n " + "x" * 300}
        long[2]["result"]["details"]["calls"][0]["args"] = pi_client.script_args_preview(long[1]["args"])
        self.assertEqual(len(long[2]["result"]["details"]["calls"][0]["args"]), pi_client.ARGS_PREVIEW_CHARS)
        self.assertTrue(pi_client.usage(long, terminal_complete=True, evidence="fixture")["complete"])
        shared = copy.deepcopy(long)
        shared.insert(2, {**long[1], "toolCallId": "s/2", "args": {"command": long[1]["args"]["command"] + "y"}})
        self.assertFalse(pi_client.usage(shared, terminal_complete=True, evidence="fixture")["complete"])


    def test_pi_bash_killed_preserves_call_log_and_partial_timing(self):
        from . import route_metrics
        events = [{"type": "tool_execution_start", "toolCallId": "b", "toolName": "bash", "args": {"command": "sleep 3"}},
                  {"type": "tool_execution_end", "toolCallId": "b", "toolName": "bash", "isError": True,
                   "result": {"content": [{"type": "text", "text": "Command timed out after 1 seconds"}]}}, {"type": "agent_settled"}]
        for name, rows in (("events.jsonl", events), ("bash-calls.jsonl", [{"id": 1, "command": "sleep 3", "tool": "bash", "dispatched": True}]),
                           ("bash-results.jsonl", [])):
            (self.root / name).write_text("".join(json.dumps(row) + "\n" for row in rows))
        write_json(self.root / "runtime.json", {"record_collection_complete": True})
        write_json(self.root / "pi-policy.json", {"measurement_mode": "direct"})
        calls, complete, _, violations = route_metrics.normalize_pi_native_calls(self.root)
        self.assertTrue(complete)
        self.assertEqual(violations, [])
        self.assertEqual(calls[0]["status"], "killed")
        observed = route_metrics.pi_observations(self.root, calls, call_log_complete=complete)
        self.assertIsNone(observed["tool_seconds"])
        self.assertEqual(observed["observed_tool_seconds"], 0)
        self.assertEqual(observed["tool_seconds_missing_calls"], 1)
        write_json(self.root / "native-agent.json", {})
        (self.root / "bash-results.jsonl").write_text(json.dumps({"id": 1, "started_monotonic": 1, "finished_monotonic": 2}) + "\npartial")
        calls, complete, _, _ = route_metrics.normalize_pi_native_calls(self.root)
        observed = route_metrics.pi_observations(self.root, calls, call_log_complete=complete)
        self.assertTrue(complete)
        self.assertIsNone(observed["tool_seconds"])
        self.assertEqual(observed["observed_tool_seconds"], 1)
        self.assertEqual(observed["tool_seconds_missing_calls"], 0)


    def test_cleanup_private_counts_only_bound_claude_result_metadata(self):
        from . import pi_client
        folder, workspace, home = self.root / "slot", self.root / "source", self.root / "local-home"
        folder.mkdir(); workspace.mkdir()
        project = home / ".claude/projects" / pi_client.claude_project_slug(workspace)
        project.mkdir(parents=True)
        (project / "session.jsonl").write_text(json.dumps({"cwd": str(workspace.resolve())}) + "\n")
        results = project / "session/tool-results"
        results.mkdir(parents=True)
        output = results / "large.txt"
        output.write_bytes(b"x" * 85592)
        original_bytes, original_text = Path.read_bytes, Path.read_text
        def guarded(method):
            def read(path, *args, **kwargs):
                self.assertNotIn("tool-results", path.parts)
                return method(path, *args, **kwargs)
            return read
        with patch.object(pi_client, "private_root", return_value=self.root / "private"), \
             patch.object(Path, "read_bytes", guarded(original_bytes)), patch.object(Path, "read_text", guarded(original_text)):
            result = pi_client.cleanup_private(folder, workspace, "claude-bridge", None, home=home)
        self.assertEqual(result["claude_persisted_tool_results"], {"count": 1, "bytes": 85592})
        self.assertEqual(result["removed_project"], str(project))
        self.assertFalse(project.exists())



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
                stop = ExecutionStop(folder, process, 0); stop.request("interrupted", {"total_tokens": 504000})
                stop.request("timeout", {"total_tokens": 900000})
                clock.return_value = 12.99; stop.advance(); self.assertEqual(send.call_count, 1)
                clock.return_value = 13; stop.advance()
                clock.return_value = 15.99; stop.advance(); self.assertEqual(send.call_count, 2)
                clock.return_value = 16; stop.advance()
                self.assertEqual([s["signal"] for s in stop.steps], ["SIGINT", "SIGTERM", "SIGKILL"])
                self.assertEqual(stop.cutoff["observed_usage"]["total_tokens"], 504000)
                self.assertEqual(stop.cutoff["reason"], "interrupted")

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
            write_json(folder / "runtime.json", {**runtime, "termination": "interrupted"})
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
signal.signal(signal.SIGALRM, lambda signum, frame: sys.exit(124))
signal.alarm(15)
print(json.dumps({"type":"thread.started","thread_id":"fixture-thread"}), flush=True)
record("r1", 1)
while True: time.sleep(.02)
'''
        spec = settings.execution_settings()
        spec["preparation_only"] = False
        spec["solver_backend"] = "codex"
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            popen = subprocess.Popen
            def launch(args, **kwargs):
                return popen([sys.executable, "-c", script], **kwargs)
            def environment(path, **ignored):
                import os
                home = folder / "codex-home"; home.mkdir()
                return {**os.environ, "CODEX_HOME": str(home)}, home
            cancelled = threading.Event()
            def cancel_when_usage_recorded():
                deadline = time.monotonic() + 5
                rollout = folder / "codex-home/sessions/fixture-thread.jsonl"
                while not rollout.exists() and time.monotonic() < deadline:
                    time.sleep(.01)
                cancelled.set()
            watcher = threading.Thread(target=cancel_when_usage_recorded)
            watcher.start()
            with patch("benchmark.runner.subprocess.Popen", side_effect=launch), patch("benchmark.runner.isolated_environment", side_effect=environment), \
                 patch("benchmark.settings.CACHE", folder), patch("benchmark.codex_client.executable", return_value="local-fixture"), \
                 patch("benchmark.codex_client.capabilities", return_value={"version": "fixture"}):
                result = execute_codex(folder, "fixture", settings=spec, cancel_event=cancelled)
            watcher.join(timeout=6)
            self.assertFalse(watcher.is_alive())
            self.assertEqual(result["status"], "interrupted")
            self.assertIsNone(result["token_measurement"]["observed_tokens_at_cutoff"])
            self.assertIsNone(result["token_measurement"]["observed_cleanup_tokens"])
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
                write_json(folder / "run.json", {"status": "interrupted"})
            finally:
                if child.poll() is None: child.kill()
                child.wait()
