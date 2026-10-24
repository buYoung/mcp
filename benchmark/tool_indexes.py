"""Full pinned corpus and task-owned native indexes; never solves a benchmark question."""
from __future__ import annotations

import argparse
import errno
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
import tomllib
from pathlib import Path

from . import route_contract as routes, settings
from .tool_artifacts import clean_environment, load_artifacts, run_logged, tree_files
from .core import command, digest, file_digest, read_json, read_jsonl, require, write_json
from .runner import copy_source

CORPUS = routes.PREPARATION / "corpus.json"
BASELINES = routes.PREPARATION / "baselines"
ZVEC_PORT = 19700
CBM_UI_PORT = 19701


def workspace_root(ident: str) -> Path:
    require(ident in routes.ARM_IDS, "알려지지 않은 arm")
    root = settings.CACHE.resolve() / digest({"workspace_revision": 2, "arm": ident})[:16]
    if ident == "serena":
        require(not any((path / "node_modules").exists() for path in [root, *root.parents]),
                "Serena에 상위 node_modules와 격리된 CODEMAP_BENCH_CACHE가 필요합니다")
    return root


def tracked_manifest(source: Path, inventory: dict | None = None) -> dict:
    if inventory is None:
        records = subprocess.check_output(["git", "ls-tree", "-rz", "--full-tree", "HEAD"], cwd=source).split(b"\0")
        inventory = {}
        for row in records:
            if row:
                metadata, raw_path = row.split(b"\t", 1)
                mode, kind, sha = metadata.decode().split()
                require(kind == "blob", "Git submodule은 별도 source 계약이 필요합니다")
                inventory[raw_path.decode()] = {"mode": mode, "git_blob_sha1": sha}
    result = {}
    for name, expected in inventory.items():
        path = source / name
        require(path.exists() or path.is_symlink(), f"추적 source 누락: {name}")
        data = os.readlink(path).encode() if path.is_symlink() else path.read_bytes()
        blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        require(blob == expected["git_blob_sha1"], f"고정 source 변경: {name}")
        result[name] = {"mode": expected["mode"], "git_blob_sha1": blob,
                        "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
    return result


def prepare_source(emit=print) -> dict:
    for name in ("02-delivery.json", "03-artifacts.json"):
        record = read_json(routes.EVIDENCE / name)
        require(record["contract_hash"] == routes.contract()["contract_hash"], f"선행 계약 변경: {name}")
    source = settings.CACHE / "grafana"
    routes.PREPARATION.mkdir(parents=True, exist_ok=True)
    if not source.exists():
        emit("고정 Grafana 전체 소스 취득")
        stage = routes.PREPARATION / "grafana-checkout"
        stage.mkdir(exist_ok=True)
        if not (stage / ".git").exists():
            run_logged(["git", "init", "--quiet"], stage, routes.PREPARATION / "source-init.log")
            run_logged(["git", "remote", "add", "origin", "https://github.com/grafana/grafana.git"], stage,
                       routes.PREPARATION / "source-remote.log")
        run_logged(["git", "fetch", "--no-tags", "--depth=1", "origin", routes.SOURCE_COMMIT], stage,
                   routes.PREPARATION / "source-fetch.log", timeout=1800)
        run_logged(["git", "checkout", "--quiet", "--detach", routes.SOURCE_COMMIT], stage,
                   routes.PREPARATION / "source-checkout.log")
        stage.rename(source)
    require(command(["git", "rev-parse", "--show-toplevel"], source).strip() == str(source), "독립 Grafana Git 복제본 필요")
    require(command(["git", "rev-parse", "HEAD"], source).strip() == routes.SOURCE_COMMIT, "Grafana source commit 변경")
    inventory = tracked_manifest(source)
    evidence = read_json(routes.DATA / "evidence.json")
    for name, expected in evidence["source_manifest"].items():
        require(inventory[name]["sha256"] == expected["sha256"] and inventory[name]["git_blob_sha1"] == expected["git_blob_sha1"],
                f"고정 정답 source 메타데이터 불일치: {name}")
    for key, excerpt in evidence["evidence"].items():
        lines = (source / excerpt["path"]).read_text().splitlines()
        text = "\n".join(lines[excerpt["start_line"] - 1:excerpt["end_line"]])
        require(text == excerpt["text"], f"고정 excerpt 원문 불일치: {key}")
    result = {"source": str(source), "source_commit": routes.SOURCE_COMMIT, "files": inventory,
              "source_sha256": digest(inventory), "tracked_files": len(inventory),
              "verified_evidence_files": len(evidence["source_manifest"]), "verified_excerpts": len(evidence["evidence"])}
    write_json(CORPUS, result)
    emit(f"Grafana 추적 파일 {len(inventory)}개, 근거 파일 81개·구간 73개 확인")
    return result


def workspace(ident: str, corpus: dict) -> tuple[Path, Path]:
    base = workspace_root(ident)
    base.mkdir(parents=True, exist_ok=True)
    source = base / "source"
    if not source.exists():
        copy_source(Path(corpus["source"]), source)
    require(digest(tracked_manifest(source, corpus["files"])) == corpus["source_sha256"], "arm source 복사 불일치")
    home = base / "home"
    home.mkdir(exist_ok=True)
    from .isolation import preflight
    preflight(source, home)
    return source, home


def cbm_runtime_dir() -> Path:
    runtime = (settings.CACHE / "cbmrt").resolve()
    # Darwin sun_path includes NUL; CBM uses a 16-hex-key socket name.
    socket_path = runtime / f"cbm-daemon-{os.geteuid()}" / "cbm-0000000000000000.sock"
    require(len(os.fsencode(socket_path)) < 104,
            "CBM rendezvous 경로가 너무 깁니다: 더 짧은 CODEMAP_BENCH_CACHE가 필요합니다")
    return runtime


def product_environment(ident: str, home: Path, source: Path) -> dict:
    from .isolation import private_environment
    home, source = home.resolve(), source.resolve()
    env = private_environment(home)
    if ident == "codemap-search":
        env.update(CODEMAP_HOME=str(home))
    elif ident == "codegraph":
        env.update(CODEGRAPH_NO_DAEMON="1", DO_NOT_TRACK="1")
    elif ident == "zvec-grep":
        env.update(ZVEC_GREP_HOME=str(home), ZVEC_GREP_MODEL_CACHE=str(home / "model-cache"),
                   ZVEC_GREP_SERVER_URL=f"http://127.0.0.1:{ZVEC_PORT}/mcp")
    elif ident == "codebase-memory-mcp":
        env.update(CBM_CACHE_DIR=str(home), CBM_ALLOWED_ROOT=str(source), CBM_RUNTIME_DIR=str(cbm_runtime_dir()))
    elif ident == "serena":
        from .serena_support import environment
        env.update(environment(home, load_artifacts(verify=False)[ident]))
    return env


def prepare_index(ident: str, emit=print) -> dict:
    corpus = read_json(CORPUS)
    artifact = load_artifacts(verify=False)[ident]
    source, home = workspace(ident, corpus)
    base = source.parent
    record = base / "prepared-index.json"
    if record.exists():
        result = read_json(record)
        require(result["artifact_sha256"] == digest(artifact), "준비된 index의 실행 파일 계약 변경")
        require(result["source_sha256"] == corpus["source_sha256"], "준비된 index의 source 변경")
        require(result["env"] == product_environment(ident, home, source), "색인 환경 변경: 기존 색인을 재인증할 수 없습니다")
        from .isolation import audit_index_paths
        result["index_path_audit"] = audit_index_paths(ident, result, corpus)
        write_json(record, result)
        return result
    env_overrides = product_environment(ident, home, source)
    env = clean_environment(env_overrides)
    from .isolation import product_profile, wrap
    profile = product_profile(ident, {"source": str(source), "home": str(home),
                                      "preparation_network": ident == "serena"}, artifact, slot_tmp=home / "tmp")
    env = clean_environment(profile["env"])
    launch = wrap(artifact["launch"], profile) if ident != "rg" else artifact["launch"]
    commands = []
    emit(f"색인 준비: {ident}")
    if ident == "codemap-search":
        permissions = codemap_default_permissions(launch, source, base, env, commands)
        commands.append(run_logged([*launch, "index", "."], source, base / "index.log", env=env, timeout=3600))
        index_paths = [source / ".codemap"]
        mode = {"external_ai": False, "Jev": "native default disabled", "filesystem_permissions": permissions}
    elif ident == "codegraph":
        commands.append(run_logged([*launch, "init", "--yes", str(source)], source, base / "index.log", env=env, timeout=3600))
        commands.append(run_logged([*launch, "status", str(source), "--json"], source, base / "index-status.log", env=env))
        native = read_json(base / "index-status.log")
        index = native.get("index", {})
        require(native.get("initialized") is True and native.get("fileCount", 0) > 0
                and native.get("version") == artifact["source_pin"]["version"]
                and index.get("builtWithVersion") == artifact["source_pin"]["version"]
                and index.get("state") == "complete" and index.get("pendingRefs") == 0
                and index.get("reindexRecommended") is False
                and native.get("worktreeMismatch") is None
                and native.get("pendingChanges") == {"added": 0, "modified": 0, "removed": 0},
                "CodeGraph native index가 준비 완료 상태가 아닙니다")
        index_paths = [source / ".codegraph"]
        mode = {"daemon": "direct task-owned process", "watch": "native default", "telemetry": "DO_NOT_TRACK=1",
                "output": "native default", "native_readiness": native}
    elif ident == "zvec-grep":
        revision = routes.load_profile()["index_mode"]["zvec_model_revision"]
        model_cache = home / "model-cache"
        native_cache = model_cache / "model2vec/minishlab--potion-code-16M-v2" / revision
        (native_cache / "tokenizer").mkdir(parents=True, exist_ok=True)
        for item in artifact["model_assets"]:
            target = native_cache / ("tokenizer/tokenizer.json" if Path(item["path"]).name == "tokenizer.json" else "model.safetensors")
            shutil.copy2(item["path"], target)
            require(file_digest(target) == item["sha256"], "zvec model cache 복사 오류")
        commands.append(run_logged([*launch, "index", str(source), "--embedding", "local/potion-code-16m-v2",
                                    "--mode", "direct", "--model-cache", str(model_cache)], source, base / "index.log", env=env, timeout=3600))
        commands.append(run_logged([*launch, "status", str(source), "--mode", "direct", "--check-ready"],
                                   source, base / "index-status.log", env=env))
        index_paths = [source / ".zvec-grep", model_cache]
        mode = {"embedding": "local/potion-code-16m-v2", "model_revision": revision,
                "embedding_workers": "native default", "toolset": "native agent default"}
    elif ident == "graphify":
        output = home / "graphify-out"
        graph_path = output / "graphify-out/graph.json"  # --out names a project output root.
        extraction = [*launch, "extract", str(source), "--code-only", "--out", str(output)]
        if output.exists():
            previous = base / "index.log.json"
            completed = read_json(previous)
            require(graph_path.is_file() and completed.get("exit_code") == 0 and completed.get("command") == extraction,
                    "불완전하거나 출처가 다른 Graphify 출력을 재사용할 수 없습니다")
            commands.append(completed)
        else:
            commands.append(run_logged(extraction, source, base / "index.log", env=env, timeout=3600))
        require(graph_path.is_file(), "Graphify graph.json 누락")
        index_paths = [output]
        mode = {"extraction": "fresh code-only", "clustering": "native default", "external_ai": False,
                "graph_path": str(graph_path),
                "native_warnings": [line for line in (base / "index.log").read_text().splitlines() if "warning:" in line]}
    elif ident == "codebase-memory-mcp":
        cbm_runtime_dir().mkdir(parents=True, exist_ok=True)
        commands.append(run_logged([*launch, "config", "set", "ui_port", str(CBM_UI_PORT)],
                                   source, base / "ui-config.log", env=env))
        commands.append(run_logged([*launch, "cli", "--json", "index_repository", "--repo-path", str(source)],
                                   source, base / "index.log", env=env, timeout=3600))
        # run_logged merges stderr, where the CLI writes `level=... msg=...` diagnostics before its JSON result.
        output = [line for line in (base / "index.log").read_text().splitlines() if line.strip()]
        diagnostics = [line for line in output if line.startswith("level=")]
        results = [line for line in output if not line.startswith("level=")]
        require(len(results) == 1, "codebase-memory-mcp 색인 결과 JSON 형식 오류")
        native = json.loads(results[0])
        require(not native.get("isError") and native.get("structuredContent", {}).get("status") == "indexed",
                "codebase-memory-mcp native index가 준비 완료 상태가 아닙니다")
        commands.append(run_logged([*launch, "cli", "--json", "index_status", "--project",
                                    native["structuredContent"]["project"], "--format", "json"], source,
                                   base / "index-status.log", env=env))
        status_lines = [line for line in (base / "index-status.log").read_text().splitlines() if line.strip()]
        diagnostics.extend(line for line in status_lines if line.startswith("level="))
        status_results = [line for line in status_lines if not line.startswith("level=")]
        require(len(status_results) == 1, "codebase-memory-mcp 상태 결과 JSON 형식 오류")
        status = json.loads(status_results[0])
        native_status = status.get("structuredContent", {})
        require(not status.get("isError") and native_status.get("status") == "ready"
                and native_status.get("project") == native["structuredContent"]["project"]
                and native_status.get("root_path") == str(source),
                "codebase-memory-mcp native 상태가 조회 준비 완료가 아닙니다")
        index_paths = [home]
        if (source / ".codebase-memory").is_dir():
            index_paths.append(source / ".codebase-memory")
        mode = {"auto_index": "native default false", "auto_watch": "native default true",
                "watcher_enabled": "native default true", "cache": str(home), "ui_port": CBM_UI_PORT,
                "runtime_dir": str(cbm_runtime_dir()),
                "native_readiness": native["structuredContent"], "native_status": native_status,
                "native_warnings": sorted(set(diagnostics))}
    elif ident == "serena":
        from .serena_support import prepare_index as prepare_serena
        index_paths, mode, commands = prepare_serena(artifact, source, home, isolation_profile=profile)
    else:
        index_paths = []
        mode = {"index": "none", "output": "native CLI defaults"}
    require(digest(tracked_manifest(source, corpus["files"])) == corpus["source_sha256"], "색인 중 추적 source 변경")
    for path in index_paths:
        require(path.is_dir() and any(path.iterdir()), f"native index 누락: {path}")
    result = {"id": ident, "source": str(source), "home": str(home), "source_sha256": corpus["source_sha256"],
              "artifact_sha256": digest(artifact), "env": env_overrides, "modes": mode,
              "index_paths": [str(p) for p in index_paths], "commands": commands,
              "status": "indexed_probe_pending", "preparation_isolation": profile}
    from .isolation import audit_index_paths
    result["index_path_audit"] = audit_index_paths(ident, result, corpus)
    write_json(record, result)
    emit(f"색인 완료: {ident}")
    return result


CODEMAP_PERMISSION_TOOLS = ("find", "grep", "read")


def codemap_default_permissions(launch: list[str], source: Path, base: Path, env: dict, commands: list) -> dict:
    """Record the unmodified native scaffold; the execution environment owns isolation."""
    config = source / ".codemap" / "config.toml"
    require(not config.exists(), "codemap 저장소 설정이 이미 있습니다")
    # Only `mcp` startup writes the native repo scaffold; EOF on stdin ends the server.
    commands.append(run_logged([*launch, "mcp", "--no-call-log"], source, base / "scaffold.log", env=env,
                               timeout=300, stdin=subprocess.DEVNULL))
    require(config.is_file(), "codemap 저장소 설정 scaffold 누락")
    permissions = tomllib.loads(config.read_text())["filesystem_permissions"]
    require(all(permissions.get(tool) == "anywhere" for tool in CODEMAP_PERMISSION_TOOLS),
            "codemap 접근 범위 기본값 변경")
    return {**{tool: permissions[tool] for tool in CODEMAP_PERMISSION_TOOLS}, "native_default": "anywhere",
            "config_sha256": file_digest(config)}


def load_index(ident: str) -> dict:
    root = workspace_root(ident)
    path = root / "prepared-index.json"
    require(path.exists(), f"색인 완료 기록이 없습니다: {ident}")
    return read_json(path)


def freeze_baseline(ident: str, state: dict) -> dict:
    baseline = BASELINES / ident
    record = baseline / "baseline.json"
    if record.exists():
        value = read_json(record)
        require(value["artifact_sha256"] == state["artifact_sha256"], "baseline 실행 파일 변경")
        return value
    baseline.mkdir(parents=True, exist_ok=True)
    for part in ("source", "home"):
        target = baseline / part
        require(not target.exists(), f"미완료 baseline 복사: {target}")
        copy_source(Path(state[part]), target)
    # Track product state separately from the immutable tracked-source inventory.
    value = {"id": ident, "artifact_sha256": state["artifact_sha256"], "source_sha256": state["source_sha256"],
             "paths": {part: str(baseline / part) for part in ("source", "home")},
             "state_sha256": {part: digest(tree_files(baseline / part)) for part in ("source", "home")}}
    write_json(record, value)
    return value


def remove_disposable_copy(root: Path) -> None:
    """Remove only a caller-validated probe copy, including native read-only Go caches."""
    def retry_owned_unlink(operation, name, error):
        if operation is os.rmdir and isinstance(error, OSError) and error.errno == errno.ENOTEMPTY:
            # Finder can write .DS_Store into a directory on an external volume while it is removed.
            directory = Path(name)
            require(directory.is_relative_to(root), "작업 복사본 밖의 삭제 금지")
            for _ in range(5):
                leftovers = list(directory.iterdir())
                require(all(item.name == ".DS_Store" and item.is_file() for item in leftovers),
                        f"삭제 중 작업 복사본 변경: {directory}")
                for item in leftovers:
                    item.unlink(missing_ok=True)
                try:
                    os.rmdir(directory)
                    return
                except OSError as retry:
                    if retry.errno != errno.ENOTEMPTY:
                        raise
            raise error
        if not isinstance(error, PermissionError) or operation not in {os.unlink, os.rmdir}:
            raise error
        parent = Path(name).parent
        require(parent.is_relative_to(root), "작업 복사본 밖의 권한 변경 금지")
        parent.chmod(parent.stat().st_mode | 0o200)
        operation(name)
    shutil.rmtree(root, onexc=retry_owned_unlink)


def restore_baseline(ident: str, state: dict, baseline: dict) -> dict:
    """Restore at the same absolute paths so native root identities stay valid."""
    started = time.monotonic()
    for part in ("source", "home"):
        target = Path(state[part])
        require(target == workspace_root(ident) / part, "작업 소유 경로 외의 reset 금지")
        require(digest(tree_files(Path(baseline["paths"][part]))) == baseline["state_sha256"][part], "baseline state 변경")
        old = target.with_name(f"{part}.probe-{time.time_ns()}")
        target.rename(old)
        copy_source(Path(baseline["paths"][part]), target)
        require(digest(tree_files(target)) == baseline["state_sha256"][part], "native state reset 불일치")
        remove_disposable_copy(old)  # Only after restoration has been verified.
        # A previously interrupted reset can leave an older disposable copy.
        for stale in target.parent.glob(f"{part}.probe-*"):
            if stale.name.removeprefix(f"{part}.probe-").isdigit() and stale.is_dir() and not stale.is_symlink():
                remove_disposable_copy(stale)
    corpus = read_json(CORPUS)
    require(digest(tracked_manifest(Path(state["source"]), corpus["files"])) == corpus["source_sha256"], "reset source 불일치")
    from .isolation import preflight
    preflight(Path(state["source"]), Path(state["home"]))
    return {"status": "verified", "same_absolute_paths": True, "probe_state_removed": True,
            "elapsed_seconds": time.monotonic() - started,
            "application_state": "fresh pre-query baseline", "os_page_cache": "uncontrolled, not claimed cold"}


def probe_configuration(ident: str, state: dict, artifacts: dict) -> dict:
    from .native_baseline import configuration
    artifact = artifacts[ident]
    source, home = Path(state["source"]), Path(state["home"])
    env = product_environment(ident, home, source)
    require(state["env"] == env, "준비된 색인 환경과 현재 실행 환경 불일치")
    if ident == "rg":
        if state.get("agent_backend") == "claude_code":
            from .claude_client import configuration
        return configuration(source, Path(artifact["launch"][0]))
    launch = artifact["launch"]
    client_config = {"transport": "stdio", "launch": launch, "env": env}
    if ident == "codemap-search":
        client_config["launch"] = [*launch, "mcp"]
    elif ident == "codegraph":
        client_config["launch"] = [*launch, "serve", "--mcp"]
    elif ident == "graphify":
        client_config["launch"] = [launch[0], "-m", "graphify.serve", state["modes"]["graph_path"]]
    elif ident == "zvec-grep":
        client_config = {"transport": "http", "url": env["ZVEC_GREP_SERVER_URL"],
                         "server_launch": [*launch, "server", "run", "--listen", f"127.0.0.1:{ZVEC_PORT}"],
                         "env": state["env"]}
    elif ident == "codebase-memory-mcp":
        client_config["shutdown_command"] = [*launch, "daemon", "stop"]
    elif ident == "serena":
        # Serena 2.x defaults to arbitrary-Python REPL, outside the search-only contract.
        client_config["launch"] = [*launch, "start-mcp-server", "--project", str(source),
                                   "--agent-interface", "tools"]
        # Preserve Serena's native 240-second tool timeout within the common 300-second run budget.
        client_config["timeout_seconds"] = routes.load_profile()["limits"]["elapsed_seconds"]
    client_config["product_arm"] = ident
    return {"schema": "grafana-route-relay-v1", "arm": ident, "source": str(source),
            "policy_state": {key: state[key] for key in ("source", "home", "modes")},
            "client": client_config, "log": "<per-run directory>/relay.jsonl",
            "rg_binary": artifacts["rg"]["launch"][0], "auxiliaries": []}


def probe_inputs(state: dict, artifacts: dict) -> dict:
    """Bind evidence to the settings/code actually probed, before publishing a new handoff."""
    backend = state.get("agent_backend", "codex")
    return {"contract_hash": routes.contract()["contract_hash"], "artifact_sha256": digest(artifacts[state["id"]]),
            "agent_backend": backend, "agent_version": command(["claude" if backend == "claude_code" else "codex", "--version"]).strip(),
            "source_sha256": state["source_sha256"], "baseline": state["baseline"],
            "configuration": probe_configuration(state["id"], state, artifacts),
            "codex_version": command(["codex", "--version"]).strip(),
            "implementation_hashes": routes.implementation_hashes([
                "benchmark/tool_indexes.py", "benchmark/route_transport.py", "benchmark/native_baseline.py",
                "benchmark/tool_artifacts.py", "benchmark/serena_support.py", "benchmark/route_prepare.py", "benchmark/runner.py",
                "benchmark/responses.py", "benchmark/core.py", "benchmark/settings.py", "benchmark/usage.py", "benchmark/claude_client.py",
                "benchmark/readiness.py", "benchmark/warm_host.py", "benchmark/stdio_shim.py", "benchmark/isolation.py"])}


def verify_probe(state: dict, artifacts: dict) -> None:
    require(state.get("status") == "ready", "중립 조회가 완료되지 않았습니다")
    from .readiness import POLICY
    ready = state.get("prepared_index_ready", {})
    require(ready.get("ready") is True and ready.get("kind") == "prepared_index_ready"
            and state.get("warmup_policy") == POLICY and ready.get("cleanup", {}).get("complete") is True,
            "prepared semantic readiness 근거가 없습니다")
    for item in ready["evidence"]:
        require(file_digest(Path(item["path"])) == item["sha256"], "semantic readiness evidence changed")
    expected = probe_inputs(state, artifacts)
    require(state.get("probe_inputs") == expected and state.get("probe_identity") == digest(expected),
            f"현재 코드·설정과 중립 조회 근거 불일치: {state['id']}; probe 재실행 필요")
    require(state.get("probe_output_sha256") == probe_output_digest(state), "중립 조회 이후 노출 도구·결과 설정 변경")
    for name in ("native_inventory", "probe_log"):
        require(file_digest(Path(state[name])) == state[name + "_sha256"], f"중립 조회 근거 파일 변경: {state['id']}/{name}")
    if state["id"] == "rg":
        require(state["native_agent"] == expected["configuration"] and state["relay_settings"] is None, "기준군 설정 변경")
    else:
        actual = state["relay_settings"]
        require(all(actual[k] == v for k, v in expected["configuration"].items() if k != "auxiliaries"), "MCP 실행 설정 변경")
        require(actual["auxiliaries"] == (["read"] if state["id"] == "graphify" else []), "보조 도구 정책 변경")


def probe_output_digest(state: dict) -> str:
    return digest({key: state.get(key) for key in (
        "status", "modes", "relay_settings", "native_agent", "native_inventory_sha256", "probe_log_sha256", "tool_condition",
        "auxiliary_necessity", "probes", "cleanup", "reset", "prepared_index_ready", "warmup_policy", "isolation")})


def ready_path(ident: str, backend="codex") -> Path:
    require(backend in {"codex", "claude_code"}, "지원하지 않는 조회 실행기")
    return workspace_root(ident) / ("ready.json" if backend == "codex" else "ready-claude.json")


def index_handoff(backend="codex") -> str:
    require(backend in {"codex", "claude_code"}, "지원하지 않는 조회 실행기")
    return "04-indexes.json" if backend == "codex" else "04-indexes-claude.json"


def probe_is_current(ident: str, backend="codex") -> bool:
    path = ready_path(ident, backend)
    if not path.exists():
        return False
    try:
        verify_probe(read_json(path), load_artifacts(verify=False))
        return True
    except (OSError, ValueError, KeyError, TypeError):
        return False


def verify_native_probe(ident: str, name: str, result: dict, state: dict | None = None) -> None:
    from .readiness import assert_ready
    assert_ready(ident, name, result, state)


def probe_index(ident: str, emit=print, *, backend="codex") -> dict:
    import threading
    from . import readiness, isolation
    from .execution import ProcessRegistry
    from .warm_host import prepare_session
    artifacts = load_artifacts(verify=False)
    state = load_index(ident)
    state["agent_backend"] = backend
    if ident == "serena":
        from .serena_support import language_dependencies
        state["modes"]["language_server_dependencies"] = language_dependencies(Path(state["home"]))
    require(state["artifact_sha256"] == digest(artifacts[ident]), "index artifact identity 변경")
    baseline = freeze_baseline(ident, state)
    start = time.monotonic()
    restored = restore_baseline(ident, state, baseline)
    restore_seconds = time.monotonic() - start
    state["baseline"] = baseline
    inputs = probe_inputs(state, artifacts)
    probe = Path(state["source"]).parent / "probes" / str(time.time_ns())
    probe.mkdir(parents=True)
    emit(f"새 제품 프로세스 semantic readiness·경계 검증: {ident}")
    registry, session = ProcessRegistry(probe), None
    cleanup, observed, extra = {}, {}, {}
    try:
        if ident == "rg":
            from .native_baseline import probe as probe_native
            if backend == "claude_code":
                from .claude_client import probe as probe_native
            observed = probe_native(inputs["configuration"], probe, [routes.DATA / "dataset.json", routes.EVIDENCE / "01-contract.json"])
            state.update(observed)
        session = prepare_session(state, probe, {"backend": backend}, threading.Event(), registry)
        receipt = session.await_ready(time.monotonic() + 300)
        if ident != "rg":
            extra = session.verify_preparation()
            relay_settings = session.relay_settings
            observed = {"native_inventory": str(probe / "inventory.json"),
                "native_inventory_sha256": file_digest(probe / "inventory.json"), "relay_settings": relay_settings,
                "tool_condition": "assisted" if relay_settings["auxiliaries"] else "native_only",
                "auxiliary_necessity": relay_settings["auxiliary_necessity"], "probes": extra["observations"],
                "probe_log": str(probe / "warmup.jsonl")}
        audit = isolation.audit_index_paths(ident, state, read_json(CORPUS))
    except Exception as exc:
        write_json(probe / "preparation-failure.json", {"error": str(exc)})
        raise
    finally:
        if session:
            cleanup = session.close("preparation_complete")
        groups = registry.cleanup()
        write_json(probe / "preparation-cleanup.json", {"session": cleanup, "groups": groups})
        if cleanup.get("complete") is True and groups["complete"]:
            require(digest(tracked_manifest(Path(state["source"]), read_json(CORPUS)["files"])) == state["source_sha256"],
                    "probe 중 source 변경")
            # Restore after failed gates too, but never while a product/writer may still be alive.
            reset = restore_baseline(ident, state, baseline)
            write_json(probe / "reset.json", reset)
    require(cleanup.get("complete") is True and groups["complete"], f"작업 소유 서버 정리 실패: {ident}")
    if ident == "serena":
        from .serena_support import verify_indexing_log
        verify_indexing_log((probe / "native.stderr.log").read_text())
    receipt.update(kind="prepared_index_ready", boundary_canaries=extra.get("boundary_canaries", []), index_audit=audit,
                   cleanup=cleanup, restore=restored, reset=reset)
    receipt["timings"].update(restore_seconds=restore_seconds, cleanup_seconds=cleanup["cleanup_seconds"],
                              reset_seconds=reset["elapsed_seconds"])
    receipt["evidence"] = [{"path": str(p), "sha256": file_digest(p)} for p in sorted(probe.iterdir())
                            if p.is_file() and not p.is_symlink()]
    observed["probe_log_sha256"] = file_digest(Path(observed["probe_log"]))
    result = {**state, **observed, "status": "ready", "cleanup": cleanup, "baseline": baseline, "reset": reset,
              "prepared_index_ready": receipt, "warmup_policy": readiness.POLICY,
              "isolation": session.profile, "probe_inputs": inputs, "probe_identity": digest(inputs),
              "limitations": ["Native indexing exclusions and output limits are retained.", "OS page cache is uncontrolled.",
                              "Prepared index readiness is not a reusable live process; each slot must pass its own gate."]}
    result["probe_output_sha256"] = probe_output_digest(result)
    write_json(ready_path(ident, backend), result)
    emit(f"semantic gate·경계·정리·동일 절대 경로 복원 완료: {ident}")
    return result


def publish_indexes(backend="codex") -> dict:
    corpus = read_json(CORPUS)
    artifacts = load_artifacts(verify=False)
    states = {}
    for ident in routes.ARM_IDS:
        state = read_json(ready_path(ident, backend))
        require(state["agent_backend"] == backend, "조회 실행기 불일치")
        verify_probe(state, artifacts)
        require(state["status"] == "ready" and state["artifact_sha256"] == digest(artifacts[ident]), "native readiness identity 불일치")
        require(digest(tracked_manifest(Path(state["source"]), corpus["files"])) == corpus["source_sha256"], "source drift")
        for part in ("source", "home"):
            require(digest(tree_files(Path(state[part]))) == state["baseline"]["state_sha256"][part], "probe 이후 index state drift")
        states[ident] = state
    return routes.publish_handoff(index_handoff(backend), {"status": "verified", "agent_backend": backend,
        "input_hashes": routes.contract()["input_hashes"],
        "source_manifest": {"path": str(CORPUS), "sha256": file_digest(CORPUS), "source_sha256": corpus["source_sha256"],
                            "tracked_files": corpus["tracked_files"], "evidence_files": 81, "evidence_ranges": 73},
        "prepared_states": states, "solver_runs": 0, "grader_runs": 0},
        implementation=["benchmark/tool_indexes.py", "benchmark/native_baseline.py", "benchmark/route_transport.py", "benchmark/claude_client.py",
                        "benchmark/readiness.py", "benchmark/warm_host.py", "benchmark/stdio_shim.py", "benchmark/isolation.py"],
        dependencies=["02-delivery.json", "03-artifacts.json"])


def main():
    parser = argparse.ArgumentParser(description="동일한 고정 Grafana source와 제품별 native 색인 준비")
    parser.add_argument("arm", choices=["source", "publish", *routes.ARM_IDS])
    parser.add_argument("--probe", action="store_true")
    args = parser.parse_args()
    emit = lambda message: print(message, flush=True)
    if args.arm == "source":
        prepare_source(emit)
    elif args.arm == "publish":
        print(publish_indexes()["status"])
    elif args.probe:
        probe_index(args.arm, emit)
    else:
        prepare_index(args.arm, emit)


if __name__ == "__main__":
    main()
