"""Full pinned corpus and task-owned native indexes; never solves a benchmark question."""
from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

from . import route_contract as routes, settings
from .tool_artifacts import clean_environment, load_artifacts, run_logged, tree_files
from .core import command, digest, file_digest, read_json, read_jsonl, require, write_json
from .runner import copy_source

WORKSPACES = routes.PREPARATION / "workspaces"
CORPUS = routes.PREPARATION / "corpus.json"
BASELINES = routes.PREPARATION / "baselines"


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
    base = WORKSPACES / ident
    base.mkdir(parents=True, exist_ok=True)
    source = base / "source"
    if not source.exists():
        copy_source(Path(corpus["source"]), source)
    require(digest(tracked_manifest(source, corpus["files"])) == corpus["source_sha256"], "arm source 복사 불일치")
    home = base / "home"
    home.mkdir(exist_ok=True)
    return source, home


def product_environment(ident: str, home: Path, source: Path) -> dict:
    if ident == "codemap-search":
        return {"CODEMAP_HOME": str(home)}
    if ident == "codegraph":
        return {"CODEGRAPH_NO_DAEMON": "1", "DO_NOT_TRACK": "1"}
    if ident == "zvec-grep":
        return {"ZVEC_GREP_HOME": str(home), "ZVEC_GREP_SERVER_URL": "http://127.0.0.1:18999/mcp"}
    if ident == "codebase-memory-mcp":
        return {"CBM_CACHE_DIR": str(home), "CBM_ALLOWED_ROOT": str(source)}
    return {}


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
        return result
    env_overrides = product_environment(ident, home, source)
    env = clean_environment(env_overrides)
    launch = artifact["launch"]
    commands = []
    emit(f"색인 준비: {ident}")
    if ident == "codemap-search":
        commands.append(run_logged([*launch, "index", "."], source, base / "index.log", env=env, timeout=3600))
        index_paths = [source / ".codemap"]
        mode = {"external_ai": False, "Jev": "native default disabled"}
    elif ident == "codegraph":
        commands.append(run_logged([*launch, "init", "--yes", str(source)], source, base / "index.log", env=env, timeout=3600))
        commands.append(run_logged([*launch, "status", str(source)], source, base / "index-status.log", env=env))
        index_paths = [source / ".codegraph"]
        mode = {"daemon": "direct task-owned process", "watch": "native default", "telemetry": "DO_NOT_TRACK=1",
                "output": "native default"}
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
        commands.append(run_logged([*launch, "cli", "--json", "index_repository", "--repo-path", str(source)],
                                   source, base / "index.log", env=env, timeout=3600))
        commands.append(run_logged([*launch, "cli", "--json", "list_projects"], source,
                                   base / "index-status.log", env=env))
        native = read_json(base / "index.log")
        require(not native.get("isError") and native.get("structuredContent", {}).get("status") == "indexed",
                "codebase-memory-mcp native index가 준비 완료 상태가 아닙니다")
        index_paths = [home]
        if (source / ".codebase-memory").is_dir():
            index_paths.append(source / ".codebase-memory")
        mode = {"auto_index": "native default false", "auto_watch": "native default true",
                "watcher_enabled": "native default true", "cache": str(home),
                "native_readiness": native["structuredContent"]}
    else:
        index_paths = []
        mode = {"index": "none", "output": "native CLI defaults"}
    require(digest(tracked_manifest(source, corpus["files"])) == corpus["source_sha256"], "색인 중 추적 source 변경")
    for path in index_paths:
        require(path.is_dir() and any(path.iterdir()), f"native index 누락: {path}")
    result = {"id": ident, "source": str(source), "home": str(home), "source_sha256": corpus["source_sha256"],
              "artifact_sha256": digest(artifact), "env": env_overrides, "modes": mode,
              "index_paths": [str(p) for p in index_paths], "commands": commands,
              "status": "indexed_probe_pending"}
    write_json(record, result)
    emit(f"색인 완료: {ident}")
    return result


def load_index(ident: str) -> dict:
    root = WORKSPACES / ident
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


def restore_baseline(ident: str, state: dict, baseline: dict) -> dict:
    """Restore at the same absolute paths so native root identities stay valid."""
    for part in ("source", "home"):
        target = Path(state[part])
        require(target == WORKSPACES / ident / part, "작업 소유 경로 외의 reset 금지")
        require(digest(tree_files(Path(baseline["paths"][part]))) == baseline["state_sha256"][part], "baseline state 변경")
        old = target.with_name(f"{part}.probe-{time.time_ns()}")
        target.rename(old)
        copy_source(Path(baseline["paths"][part]), target)
        require(digest(tree_files(target)) == baseline["state_sha256"][part], "native state reset 불일치")
        shutil.rmtree(old)  # Only this preparation's disposable probe copy, after restoration is verified.
        # A previously interrupted reset can leave an older disposable copy.
        for stale in target.parent.glob(f"{part}.probe-*"):
            if stale.name.removeprefix(f"{part}.probe-").isdigit() and stale.is_dir() and not stale.is_symlink():
                shutil.rmtree(stale)
    corpus = read_json(CORPUS)
    require(digest(tracked_manifest(Path(state["source"]), corpus["files"])) == corpus["source_sha256"], "reset source 불일치")
    return {"status": "verified", "same_absolute_paths": True, "probe_state_removed": True,
            "application_state": "fresh pre-query baseline", "os_page_cache": "uncontrolled, not claimed cold"}


def probe_configuration(ident: str, state: dict, artifacts: dict) -> dict:
    from .native_baseline import configuration
    artifact = artifacts[ident]
    source, home = Path(state["source"]), Path(state["home"])
    env = product_environment(ident, home, source)
    require(state["env"] == env, "준비된 색인 환경과 현재 실행 환경 불일치")
    if ident == "rg":
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
        client_config = {"transport": "http", "url": "http://127.0.0.1:18999/mcp",
                         "server_launch": [*launch, "server", "run", "--listen", "127.0.0.1:18999"],
                         "env": state["env"]}
    elif ident == "codebase-memory-mcp":
        client_config["shutdown_command"] = [*launch, "daemon", "stop"]
    return {"schema": "grafana-route-relay-v1", "arm": ident, "source": str(source),
            "client": client_config, "log": "<per-run directory>/relay.jsonl",
            "rg_binary": artifacts["rg"]["launch"][0], "auxiliaries": []}


def probe_inputs(state: dict, artifacts: dict) -> dict:
    """Bind evidence to the settings/code actually probed, before publishing a new handoff."""
    return {"contract_hash": routes.contract()["contract_hash"], "artifact_sha256": digest(artifacts[state["id"]]),
            "source_sha256": state["source_sha256"], "baseline": state["baseline"],
            "configuration": probe_configuration(state["id"], state, artifacts),
            "codex_version": command(["codex", "--version"]).strip(),
            "implementation_hashes": routes.implementation_hashes([
                "benchmark/tool_indexes.py", "benchmark/route_transport.py", "benchmark/native_baseline.py",
                "benchmark/tool_artifacts.py", "benchmark/route_prepare.py", "benchmark/runner.py",
                "benchmark/responses.py", "benchmark/core.py", "benchmark/settings.py", "benchmark/usage.py"])}


def verify_probe(state: dict, artifacts: dict) -> None:
    require(state.get("status") == "ready", "중립 조회가 완료되지 않았습니다")
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
        "status", "relay_settings", "native_agent", "native_inventory_sha256", "probe_log_sha256", "tool_condition",
        "auxiliary_necessity", "probes", "cleanup", "reset")})


def probe_is_current(ident: str) -> bool:
    path = WORKSPACES / ident / "ready.json"
    if not path.exists():
        return False
    try:
        verify_probe(read_json(path), load_artifacts(verify=False))
        return True
    except (OSError, ValueError, KeyError, TypeError):
        return False


def probe_index(ident: str, emit=print) -> dict:
    from .route_transport import NativeRelay, auxiliary_call
    artifacts = load_artifacts(verify=False)
    state = load_index(ident)
    source = Path(state["source"])
    require(state["artifact_sha256"] == digest(artifacts[ident]), "index artifact identity 변경")
    baseline = freeze_baseline(ident, state)
    restore_baseline(ident, state, baseline)
    inputs = probe_inputs({**state, "baseline": baseline}, artifacts)
    probe = source.parent / "probes" / str(time.time_ns())
    probe.mkdir(parents=True)
    emit(f"현재 실행 설정으로 중립 조회 재검증: {ident}")
    if ident == "rg":
        from .native_baseline import probe as probe_native
        observed = probe_native(inputs["configuration"], probe, [routes.DATA / "dataset.json", routes.EVIDENCE / "01-contract.json"])
        result = {**state, **observed, "status": "ready", "baseline": baseline,
                  "probe_inputs": inputs, "probe_identity": digest(inputs),
                  "reset": restore_baseline(ident, state, baseline),
                  "limitations": ["Actual model tool inventory and delivery are unobserved; only local native permissions were probed."]}
        result["probe_output_sha256"] = probe_output_digest(result)
        write_json(source.parent / "ready.json", result)
        emit("기본 에이전트 준비: 검색 MCP 없음, 고정 rg 사용 가능, 원문 읽기 허용·정답 접근/쓰기 거부 확인")
        return result
    relay_settings = {**inputs["configuration"], "log": str(probe / "relay.jsonl")}
    relay = None
    did_initialize = False
    cleanup = {}
    try:
        relay = NativeRelay(relay_settings)
        initialized = relay.initialize({"protocolVersion": "2025-03-26", "capabilities": {},
                                        "clientInfo": {"name": "codex-mcp-client", "version": command(["codex", "--version"]).strip()}})
        did_initialize = True
        inventory = relay.list_tools()
        by_name = {t["name"]: t for t in inventory}
        mutations = {"index_repository", "delete_project", "manage_adr", "ingest_traces"}
        outside_corpus = {"list_prs", "get_pr_impact", "triage_prs"}
        # Native search may refresh its own index (zvec readOnlyHint=false).
        # Classify explicit administration operations, not that advisory annotation.
        query_tools = [t["name"] for t in inventory if t["name"] not in mutations | outside_corpus]
        probes = []
        if ident == "codemap-search":
            probes = [("initial_instructions", {}), ("read", {"file_path": "README.md"})]
        elif ident == "codegraph":
            probes = [("codegraph_explore", {"query": "configuration settings", "projectPath": str(source)})]
        elif ident == "zvec-grep":
            probes = [("zvec_grep_search", {"query": "configuration settings", "root": str(source)})]
        elif ident == "graphify":
            probes = [("query_graph", {"question": "configuration settings"})]
        elif ident == "codebase-memory-mcp":
            project = state["modes"]["native_readiness"]["project"]
            probes = [("search_code", {"pattern": "package setting", "project": project, "project_name": project})]
        observations = []
        for number, (name, arguments) in enumerate(probes, 1):
            require(name in query_tools, f"native query tool이 없습니다: {ident}/{name}")
            properties = by_name[name]["inputSchema"].get("properties", {})
            arguments = {k: v for k, v in arguments.items() if k in properties}
            result = relay.call(number, {"name": name, "arguments": arguments})
            require(not result.get("isError"), f"native capability probe 오류: {ident}/{name}: {str(result)[:500]}")
            observations.append({"tool": name, "arguments": arguments, "result_sha256": digest(result), "isError": False})
        auxiliaries, necessity = [], []
        if ident == "graphify":
            # Graph traversal returns source locations/graph metadata, with no source-body read operation.
            auxiliaries = ["read"]
            necessity = [{"operation": "inspect original source body at a discovered file/line location",
                          "reason": "Native graph queries/get_node return graph metadata and locations; the native surface has no raw-source reader.",
                          "native_inventory_sha256": digest(inventory), "probe_log": str(probe / "relay.jsonl"),
                          "added": "read", "quality_based": False}]
            read_result = auxiliary_call(source, "read", {"file_path": "README.md"})
            require(read_result["content"][0]["text"] == (source / "README.md").read_text(), "보조 원문 read 변형")
        relay_settings.update(query_tools=query_tools, auxiliaries=auxiliaries, auxiliary_necessity=necessity,
                              enabled_tools=[*query_tools, *auxiliaries],
                              excluded_tools=[{"name": name, "reason": "administration" if name in mutations else "external GitHub PR data outside the fixed corpus"}
                                              for name in by_name if name in mutations | outside_corpus])
        write_json(probe / "inventory.json", {"initialize": initialized, "tools": inventory})
        write_json(probe / "observations.json", observations)
    finally:
        if relay and relay.product:
            cleanup["frontend"] = relay.product.close()
        if ident == "zvec-grep" and "frontend" in cleanup:
            cleanup["daemon"] = cleanup["frontend"].get("http_server")
        if ident == "codebase-memory-mcp" and did_initialize:
            cleanup["daemon"] = cleanup["frontend"].get("daemon")
        if ident in {"codegraph", "codemap-search", "graphify", "rg"}:
            cleanup["daemon"] = {"mode": "task-owned foreground or no process", "shared_daemon": False}
    require(cleanup.get("frontend", {}).get("complete", True), f"작업 소유 서버 정리 실패: {ident}")
    reset = restore_baseline(ident, state, baseline)
    relay_settings["log"] = "<per-run directory>/relay.jsonl"
    result = {**state, "status": "ready", "native_inventory": str(probe / "inventory.json"),
              "native_inventory_sha256": file_digest(probe / "inventory.json"), "relay_settings": relay_settings,
              "tool_condition": "assisted" if auxiliaries else "native_only", "auxiliary_necessity": necessity,
              "probes": observations, "probe_log": str(probe / "relay.jsonl"), "probe_log_sha256": file_digest(probe / "relay.jsonl"),
              "cleanup": cleanup, "baseline": baseline, "reset": reset,
              "probe_inputs": inputs, "probe_identity": digest(inputs),
              "limitations": ["Native indexing exclusions and output limits are retained.", "OS page cache is not controlled."]}
    result["probe_output_sha256"] = probe_output_digest(result)
    write_json(source.parent / "ready.json", result)
    emit(f"연결·중립 조회·정리·초기 상태 복원 완료: {ident}")
    return result


def publish_indexes() -> dict:
    corpus = read_json(CORPUS)
    artifacts = load_artifacts(verify=False)
    states = {}
    for ident in routes.ARM_IDS:
        state = read_json(WORKSPACES / ident / "ready.json")
        verify_probe(state, artifacts)
        require(state["status"] == "ready" and state["artifact_sha256"] == digest(artifacts[ident]), "native readiness identity 불일치")
        require(digest(tracked_manifest(Path(state["source"]), corpus["files"])) == corpus["source_sha256"], "source drift")
        for part in ("source", "home"):
            require(digest(tree_files(Path(state[part]))) == state["baseline"]["state_sha256"][part], "probe 이후 index state drift")
        states[ident] = state
    return routes.publish_handoff("04-indexes.json", {"status": "verified", "input_hashes": routes.contract()["input_hashes"],
        "source_manifest": {"path": str(CORPUS), "sha256": file_digest(CORPUS), "source_sha256": corpus["source_sha256"],
                            "tracked_files": corpus["tracked_files"], "evidence_files": 81, "evidence_ranges": 73},
        "prepared_states": states, "solver_runs": 0, "grader_runs": 0},
        implementation=["benchmark/tool_indexes.py", "benchmark/native_baseline.py", "benchmark/route_transport.py"],
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
