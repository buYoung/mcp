"""Claude native Bash environment isolation and pre-exec ownership of detached shell groups."""
from __future__ import annotations

import os
import socket
import subprocess
import sys
import threading
from pathlib import Path
from types import SimpleNamespace

from .core import append_jsonl, read_json, require, write_json
from .pi_client import private_root


def socket_path(folder: Path) -> Path:
    return private_root(folder) / "s"


def wrapper(folder: Path, environment: dict) -> str:
    path = private_root(folder) / "bin" / "zsh"
    path.parent.mkdir(exist_ok=True)
    require(not path.exists() and not path.is_symlink(), "private shell already exists")
    address = socket_path(folder)
    require(len(os.fsencode(address)) < 104, "private shell socket path too long")
    path.write_text(f'''#!{sys.executable} -I
import os, socket, sys
if sys.argv[1:] == ["--version"]:
    os.execve("/bin/zsh", ["/bin/zsh", "-f", "--version"], {environment!r})
try:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(10)
        connection.connect({str(address)!r})
        if connection.recv(1) != b"1":
            raise RuntimeError("shell ownership not confirmed")
except Exception:
    print("Native shell launch rejected: ownership not confirmed.", file=sys.stderr)
    sys.exit(125)
os.execve("/bin/zsh", ["/bin/zsh", "-f", *sys.argv[1:]], {environment!r})
''')
    path.chmod(0o700)
    write_json(folder / "shell-monitor.json", {"socket": str(address), "wrapper": str(path), "environment_keys": sorted(environment)}, exclusive=True)
    return str(path)


class ShellMonitor:
    """Darwin LOCAL_PEERPID authenticates the wrapper before it releases a model command."""
    def __init__(self, folder: Path, registry=None):
        self.folder = folder
        self.path = Path(read_json(folder / "shell-monitor.json")["socket"])
        self.root_pid = None
        self.ready, self.closing = threading.Event(), threading.Event()
        self.groups, self.errors = {}, []
        self.registry = registry
        self.listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.listener.bind(str(self.path))
        self.path.chmod(0o600)
        self.listener.listen(8)
        self.listener.settimeout(.1)
        self.thread = threading.Thread(target=self._serve, name="native-shell-owner", daemon=True)
        self.thread.start()

    def started(self, process):
        self.root_pid = process.pid
        self.ready.set()

    def _descendant(self, pid: int) -> bool:
        seen = set()
        while pid > 1 and pid not in seen:
            if pid == self.root_pid:
                return True
            seen.add(pid)
            result = subprocess.run(["/bin/ps", "-p", str(pid), "-o", "ppid="], capture_output=True, text=True, timeout=5)
            if result.returncode != 0 or not result.stdout.strip().isdigit():
                return False
            pid = int(result.stdout.strip())
        return False

    def _serve(self):
        while not self.closing.is_set():
            try:
                connection, _ = self.listener.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            with connection:
                try:
                    require(self.ready.wait(10) and not self.closing.is_set(), "shell owner unavailable")
                    # Darwin sys/un.h: SOL_LOCAL=0, LOCAL_PEERPID=2. Never trust a caller-supplied PID.
                    pid = connection.getsockopt(0, 2)
                    require(self._descendant(pid), "unowned shell connection")
                    pgid = os.getpgid(pid)
                    require(pgid != os.getpgrp(), "unsafe native shell process group")
                    if pgid != self.root_pid and pgid not in self.groups:
                        require(pgid == pid, "native shell group leader not authenticated")
                        from .execution import ProcessRegistry
                        item = {"pid": pid, "pgid": pgid, "start_fingerprint": ProcessRegistry.fingerprint(pid)}
                        require(bool(item["start_fingerprint"]), "native shell process exited before admission")
                        if self.registry is not None:
                            self.registry.register(pid, pgid, "native-shell")
                        self.groups[pgid] = item
                        append_jsonl(self.folder / "native-groups.jsonl", item)
                    require(not (self.folder / "stop.json").exists(), "solver stopped")
                    connection.sendall(b"1")
                except Exception as exc:
                    self.errors.append(str(exc))

    def close(self, pump) -> dict:
        from .runner import cleanup_solver, signal_target
        from .execution import ProcessRegistry
        self.closing.set()
        self.ready.set()
        self.listener.close()
        self.thread.join(timeout=12)
        self.path.unlink(missing_ok=True)
        results = []
        for pgid, item in self.groups.items():
            if not signal_target(pgid, 0, group=True):
                results.append({**item, "complete": True})
                continue
            fingerprint = ProcessRegistry.fingerprint(pgid)
            if fingerprint and fingerprint != item["start_fingerprint"]:
                results.append({**item, "complete": False, "error": "native group identity changed; no signal sent"})
                continue
            # A leader may have exited while its authenticated group still has children.
            results.append({**item, **cleanup_solver(SimpleNamespace(pid=pgid), pump)})
        result = {"complete": not self.thread.is_alive() and all(row["complete"] for row in results),
                  "groups": results, "admission_errors": self.errors}
        write_json(self.folder / "native-shell-cleanup.json", result)
        return result
