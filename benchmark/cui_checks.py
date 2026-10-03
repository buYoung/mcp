"""Exercise actual Inquirer prompts in a PTY; the backend records requests only."""
import json
import os
import pty
import select
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

from . import execution, route_contract as routes, settings


class CUI:
    def __init__(self, root: Path, running_backend=False):
        self.log = root / "calls.jsonl"
        catalog = execution.catalog()
        plan = {"plan_id": "fixture", "profile_name": "검증", "preparation_only": True, "status": "prepared_not_executed",
                "targets": [{**item, "tool_condition": "native_agent_baseline" if item["id"] == "rg" else "native_only",
                             "actual_version": "fixture"} for item in catalog["route_targets"]],
                "questions": 3, "repeats": 1, "runs": 18, "spec": settings.execution_settings(),
                "readiness": str(root / "readiness.json")}
        script = root / "fixture.mjs"
        script.write_text(f"""
import {{ appendFileSync }} from 'node:fs';
import {{ interactive, requireTerminal }} from {json.dumps((settings.ROOT/'start.mjs').as_uri())};
import * as prompts from {json.dumps((settings.REPOSITORY/'node_modules/@inquirer/prompts/dist/index.js').as_uri())};
requireTerminal();
const catalog={json.dumps(catalog,ensure_ascii=False)};
const plan={json.dumps(plan,ensure_ascii=False)};
const call=async(action,request)=>{{
  appendFileSync({json.dumps(str(self.log))},JSON.stringify({{action,request}})+'\\n');
  return action==='catalog'?catalog:action==='prepare'?plan:{{status:'complete'}};
}};
try {{ await interactive({{prompts,call}}); }}
catch(error) {{ if(['ExitPromptError','AbortPromptError'].includes(error.name)) process.exitCode=130; else throw error; }}
""")
        env = {**os.environ, "CI": "true", "TERM": "xterm-256color"}
        if running_backend:
            stub = root / "python3"
            stub.write_text(f"#!{sys.executable}\n" + '''import json, os, signal, subprocess, sys, time
from pathlib import Path
json.load(sys.stdin)
child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])
Path(__file__).with_name('pids.json').write_text(json.dumps([os.getpid(), child.pid]))
def stop(signum, frame):
    child.wait(timeout=3)
    print(json.dumps({'event':'result','value':{'status':'interrupted'}}),flush=True)
    sys.exit(130)
signal.signal(signal.SIGINT, stop)
print(json.dumps({'event':'progress','message':'fixture-started'}),flush=True)
while True: time.sleep(.02)
''')
            stub.chmod(0o755)
            script.write_text(f"""
import {{ backend }} from {json.dumps((settings.ROOT/'start.mjs').as_uri())};
try {{ await backend('prepare',{{profile:'grafana-routes'}}); }}
catch(error) {{ process.exitCode=error.name==='BenchCancelled'?130:1; }}
""")
            env["PATH"] = str(root) + os.pathsep + env["PATH"]
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.execvpe("node", ["node", str(script)], env)
        self.buffer = b""
        self.status = None

    def wait(self, text):
        expected = text.encode()
        deadline = time.monotonic() + 8
        while expected not in self.buffer and time.monotonic() < deadline:
            ready, _, _ = select.select([self.fd], [], [], .05)
            if ready:
                try:
                    self.buffer += os.read(self.fd, 65536)
                except OSError:
                    break
        if expected not in self.buffer:
            raise AssertionError(f"Missing prompt {text}: {self.buffer.decode(errors='replace')[-3000:]}")
        self.buffer = b""

    def send(self, text):
        time.sleep(.03)
        os.write(self.fd, text.encode())

    def finish(self):
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            ready, _, _ = select.select([self.fd], [], [], .02)
            if ready:
                try:
                    self.buffer += os.read(self.fd, 65536)
                except OSError:
                    pass
            pid, status = os.waitpid(self.pid, os.WNOHANG)
            if pid:
                self.status = os.waitstatus_to_exitcode(status)
                return self.status
            time.sleep(.02)
        raise AssertionError("CUI did not terminate: " + self.buffer.decode(errors="replace")[-3000:])

    def calls(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def close(self):
        if self.status is None:
            try:
                os.kill(self.pid, signal.SIGKILL)
                os.waitpid(self.pid, 0)
            except ProcessLookupError:
                pass
        os.close(self.fd)


class CUIChecks(unittest.TestCase):
    def exercise(self, callback, **options):
        with tempfile.TemporaryDirectory() as temporary:
            cui = CUI(Path(temporary), **options)
            try:
                callback(cui)
            finally:
                cui.close()

    def test_default_enter_prepares_without_start_even_with_CI(self):
        def run(cui):
            cui.wait("평가 유형을 선택하세요.")
            cui.send("\r")
            self.assertEqual(cui.finish(), 0)
            self.assertEqual([c["action"] for c in cui.calls()], ["catalog", "prepare"])
        self.exercise(run)

    def test_fixed_profile_and_targets_are_forwarded(self):
        def run(cui):
            cui.wait("평가 유형을 선택하세요.")
            cui.send("\r")
            self.assertEqual(cui.finish(), 0)
            self.assertEqual(cui.calls()[1]["request"], {"profile": routes.PROFILE, "targets": list(routes.ARM_IDS)})
        self.exercise(run)

    def test_ctrl_c_before_preparation(self):
        def run(cui):
            cui.wait("평가 유형을 선택하세요.")
            cui.send("\x03")
            self.assertEqual(cui.finish(), 130)
            self.assertEqual([c["action"] for c in cui.calls()], ["catalog"])
        self.exercise(run)

    def test_non_tty_entry_refuses_without_backend(self):
        result = subprocess.run(["node", "benchmark/start.mjs"], cwd=settings.REPOSITORY, input="y\ny\n", text=True,
                                capture_output=True, env={**os.environ, "CI": "true"}, timeout=5)
        self.assertEqual(result.returncode, 1)
        self.assertIn("대화형 터미널", result.stderr)

    def test_ctrl_c_during_backend_reaps_owned_group(self):
        def run(cui):
            cui.wait("fixture-started")
            pids = json.loads(cui.log.with_name("pids.json").read_text())
            cui.send("\x03")
            self.assertEqual(cui.finish(), 130)
            for pid in pids:
                with self.assertRaises(ProcessLookupError):
                    os.kill(pid, 0)
        self.exercise(run, running_backend=True)


if __name__ == "__main__":
    unittest.main()
