"""Slot-owned Claude HOME and memory-only credentials for the bridge child-env API."""
from __future__ import annotations

import json
import os
import pwd
import shutil
import stat
import threading
from dataclasses import dataclass, field
from pathlib import Path

from .core import file_digest, read_json, require, write_json

TOKEN_FILE_ENV = "CODEMAP_BENCH_CLAUDE_TOKEN_FILE"
CHILD_ENV_FD = "CODEMAP_BENCH_CLAUDE_CHILD_ENV_FD"
BRIDGE_FACTORY = "createClaudeBridgeExtension"
CHILD_ENV_CONTRACT = "createClaudeBridgeExtension/getChildEnv-v1"


@dataclass(repr=False)
class Credential:
    token: str = field(repr=False)
    paths: tuple[str, ...] = field(default=(), repr=False)

    @property
    def redactions(self) -> tuple[str, ...]:
        return (self.token, *self.paths)


def read_token_file() -> Credential:
    """Never expose the path, OS exception or file content in an error or evidence."""
    value = os.environ.get(TOKEN_FILE_ENV)
    require(bool(value), "Claude 운영자 토큰 파일이 필요합니다 (CODEMAP_BENCH_CLAUDE_TOKEN_FILE)")
    descriptor = None
    try:
        path = Path(value).expanduser()
        before = path.lstat()
        require(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid()
                and stat.S_IMODE(before.st_mode) == 0o600 and before.st_nlink == 1,
                "Claude 토큰 파일은 운영자 소유 0600 regular file이어야 합니다 (link 금지)")
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
        opened = os.fstat(descriptor)
        require((opened.st_dev, opened.st_ino, opened.st_mode, opened.st_nlink, opened.st_uid)
                == (before.st_dev, before.st_ino, before.st_mode, before.st_nlink, before.st_uid),
                "Claude 토큰 파일 상태가 변경됐습니다")
        with os.fdopen(descriptor, "rb") as stream:
            descriptor = None
            raw = stream.read(65537)
        require(0 < len(raw) <= 65536, "Claude 토큰 파일이 비었거나 형식이 잘못됐습니다")
        token = raw.decode("utf-8").strip()
        require(bool(token) and not any(c.isspace() for c in token) and "\x00" not in token,
                "Claude 토큰 파일이 비었거나 형식이 잘못됐습니다")
        return Credential(token, tuple(dict.fromkeys((value, str(path.absolute()), str(path.resolve())))))
    except (OSError, UnicodeError):
        raise ValueError("Claude 토큰 파일을 안전하게 읽을 수 없습니다 (내용과 경로 비공개)") from None
    finally:
        if descriptor is not None:
            os.close(descriptor)


class ChildEnvChannel:
    """One inherited anonymous FD; the first trusted extension reads and closes it before tools load."""
    def __init__(self, credential: Credential, *, local_endpoint: str | None = None):
        self.credential = credential
        self.read_fd, self.write_fd = os.pipe()
        self.error = False
        payload = {"oauthToken": credential.token}
        if local_endpoint is not None:
            from .pi_client import local_endpoint_url
            payload["baseUrl"] = local_endpoint_url(local_endpoint)
        data = json.dumps(payload).encode()
        def send():
            try:
                view = memoryview(data)
                while view:
                    view = view[os.write(self.write_fd, view):]
            except OSError:
                self.error = True
            finally:
                try:
                    os.close(self.write_fd)
                except OSError:
                    pass
        self.writer = threading.Thread(target=send, daemon=True)
        self.writer.start()

    def close_parent_read(self) -> None:
        if self.read_fd is not None:
            os.close(self.read_fd)
            self.read_fd = None

    def close(self) -> None:
        self.close_parent_read()
        self.writer.join(timeout=2)
        require(not self.writer.is_alive(), "Claude credential pipe cleanup incomplete")


def bridge_extension_source(bridge: Path) -> str:
    return ('import * as bridge from ' + json.dumps(str(bridge)) + ';\n'
            'import {readFileSync, closeSync} from "node:fs";\n'
            f'const fdKey = {json.dumps(CHILD_ENV_FD)};\n' + '''
export default (pi) => {
  const fd = Number(process.env[fdKey]);
  delete process.env[fdKey];
  let credentials;
  try {
    if (typeof bridge.createClaudeBridgeExtension !== "function") throw new Error();
    if (!Number.isInteger(fd) || fd < 3) throw new Error();
    credentials = JSON.parse(readFileSync(fd, "utf8"));
    if (typeof credentials.oauthToken !== "string" || !credentials.oauthToken || /\\s/.test(credentials.oauthToken)) throw new Error();
    if (credentials.baseUrl !== undefined) {
      const url = new URL(credentials.baseUrl);
      if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || !url.port || url.username || url.password || url.search || url.hash) throw new Error();
    }
  } catch {
    throw new Error("Claude bridge child-env API or private credentials unavailable");
  } finally {
    if (Number.isInteger(fd) && fd >= 3) { try { closeSync(fd); } catch {} }
  }
  const getChildEnv = ({purpose}) => {
    if (purpose !== "provider" && purpose !== "summary") throw new Error("Unsupported Claude child purpose");
    return Object.freeze({CLAUDE_CODE_OAUTH_TOKEN: credentials.oauthToken,
      ...(credentials.baseUrl ? {ANTHROPIC_BASE_URL: credentials.baseUrl} : {})});
  };
  bridge.createClaudeBridgeExtension({getChildEnv})(pi);
};
''')


def home_path(folder: Path) -> Path:
    from .pi_client import private_root
    return private_root(folder) / "login"


def create_home(folder: Path) -> Path:
    home = home_path(folder)
    require(not home.exists() and not home.is_symlink(), "Claude 슬롯 HOME은 새 빈 디렉터리여야 합니다")
    require(home.resolve() != Path(pwd.getpwuid(os.getuid()).pw_dir).resolve(), "Claude 실제 HOME 사용 금지")
    home.mkdir(mode=0o700)
    info = home.stat()
    write_json(folder / "claude-home.json", {"path": str(home), "device": info.st_dev, "inode": info.st_ino,
                                           "owner": info.st_uid}, exclusive=True)
    return home


def cleanup_home(folder: Path, workspace: Path, redactions) -> dict:
    from .pi_client import redact
    receipt = read_json(folder / "claude-home.json")
    home = home_path(folder)
    require(str(home) == receipt["path"] and not home.is_symlink(), "Claude private HOME binding changed")
    info = home.stat()
    require((info.st_dev, info.st_ino, info.st_uid) == (receipt["device"], receipt["inode"], os.geteuid()),
            "Claude private HOME ownership changed")
    files, total, leaked = 0, 0, False
    categories = {}
    persisted = {"count": 0, "bytes": 0}
    for path in home.rglob("*"):
        require(not path.is_symlink(), "Claude private HOME symlink rejected")
        if path.is_file():
            item = path.stat()
            require(item.st_nlink == 1, "Claude private HOME hard link rejected")
            files += 1
            total += item.st_size
            category = str(path.relative_to(home)).split("/")[:2]
            key = "/".join(category)
            categories[key] = categories.get(key, 0) + 1
            if "tool-results" in path.relative_to(home).parts:
                persisted["count"] += 1
                persisted["bytes"] += item.st_size
            if redactions:
                raw = path.read_bytes()
                leaked |= redact(raw, redactions) != raw
    shutil.rmtree(home)
    return {"complete": not leaked, "private_home_removed": not home.exists(),
            "claude_persisted_tool_results": persisted, "claude_json_modified_by_harness": False,
            "private_home_files": files, "private_home_bytes": total, "private_home_categories": categories,
            "credential_leak_detected": leaked, "real_home_used": False}


def verify_bridge_api(tools: dict, root: Path) -> dict:
    """Import the candidate entry without querying/starting Claude; old bridge fails here."""
    import subprocess
    from .pi_client import PRIVATE_FLAGS
    from .tool_artifacts import clean_environment
    root.mkdir(parents=True, exist_ok=False)
    for name in ("home", "agent", "tmp"):
        (root / name).mkdir(mode=0o700)
    result_path = root / "api.json"
    extension = root / "probe.mjs"
    extension.write_text('import * as bridge from ' + json.dumps(tools["bridge_extension"]) + ';\n'
        'import {writeFileSync} from "node:fs";\nexport default () => writeFileSync(' + json.dumps(str(result_path)) +
        ', JSON.stringify({available: typeof bridge.createClaudeBridgeExtension === "function"}));\n')
    env = clean_environment({"HOME": str(root / "home"), "TMPDIR": str(root / "tmp"),
        "PI_CODING_AGENT_DIR": str(root / "agent"), "PI_SKIP_VERSION_CHECK": "1", "PI_TELEMETRY": "0"})
    result = subprocess.run([tools["node"], tools["pi_cli"], *PRIVATE_FLAGS, "--no-mcp", "--no-tools", "-e", str(extension), "--list-models"],
                            cwd=root, env=env, capture_output=True, timeout=60)
    available = result.returncode == 0 and result_path.is_file() and read_json(result_path).get("available") is True
    write_json(root / "result.json", {"available": available, "entry_sha256": file_digest(Path(tools["bridge_extension"])),
                                      "exit_code": result.returncode, "claude_children": 0})
    require(available, "Claude bridge에 createClaudeBridgeExtension({ getChildEnv }) API가 필요합니다; Claude child 실행 금지")
    return {"contract": CHILD_ENV_CONTRACT, "evidence": str(root / "result.json"), "sha256": file_digest(root / "result.json")}
