"""Register and pre-index the fixed corpus with Serena's native local LSP backend."""
from __future__ import annotations

import json
import os
import re
from pathlib import Path

from .core import file_digest, read_json, require
from .tool_artifacts import ARTIFACTS, clean_environment, run_logged


def environment(home: Path, artifact: dict) -> dict:
    runtime_paths = [str(Path(artifact["runtime"][name]["path"]).parent) for name in ("gopls", "go", "node", "npm")]
    return {"SERENA_HOME": str(home / "serena"), "GOPATH": str(home / "go"),
            "GOCACHE": str(home / "go-build"), "GOPLSCACHE": str(home / "gopls"),
            "XDG_CACHE_HOME": str(home / "cache"), "GOTELEMETRY": "off",
            "GIT_CEILING_DIRECTORIES": str(ARTIFACTS),
            "PATH": os.pathsep.join([*runtime_paths, os.environ.get("PATH", "")])}


def prepare_index(artifact: dict, source: Path, home: Path) -> tuple[list[Path], dict, list[dict]]:
    launch, base = artifact["launch"], source.parent
    env = clean_environment(environment(home, artifact))
    commands = []
    project = source / ".serena/project.yml"
    if not project.exists():
        commands.append(run_logged([*launch, "project", "create", str(source), "--name", "grafana",
                                    "--language", "go", "--language", "typescript"],
                                   source, base / "project-create.log", env=env))
    else:
        # An interrupted first index may leave a correctly registered project behind.
        previous = read_json(base / "project-create.log.json")
        require(previous["exit_code"] == 0 and file_digest(Path(previous["log"])) == previous["log_sha256"],
                "Serena 프로젝트 생성 근거가 없습니다")
        commands.append(previous)
    python = str(Path(launch[0]).with_name("python"))
    configure = """
import json, sys
from pathlib import Path
from ruamel.yaml import YAML
source, home = map(Path, sys.argv[1:])
yaml = YAML()
project = source / '.serena/project.yml'
config = home / 'serena/serena_config.yml'
for path, overrides in [(project, {'read_only': True, 'excluded_tools': ['activate_project', 'remove_project', 'switch_modes']}),
                        (config, {'base_modes': [], 'default_modes': ['one-shot', 'no-memories'],
                                  'web_dashboard': False, 'web_dashboard_open_on_launch': False, 'gui_log_window': False})]:
    data = yaml.load(path)
    data.update(overrides)
    yaml.dump(data, path)
project_data, config_data = yaml.load(project), yaml.load(config)
assert project_data['language_servers'] == ['go', 'typescript']
assert project_data['ls_workspace_folders'] == ['.']
assert config_data['projects'] == [str(source)]
print(json.dumps({'project': str(project), 'registered_projects': config_data['projects'],
                  'languages': project_data['language_servers'], 'read_only': project_data['read_only'],
                  'agent_interface_default': config_data['agent_interface'],
                  'default_max_tool_answer_chars': config_data['default_max_tool_answer_chars'],
                  'tool_timeout_seconds': config_data['tool_timeout'],
                  'symbol_info_budget_seconds': config_data['symbol_info_budget']}))
"""
    commands.append(run_logged([python, "-c", configure, str(source), str(home)], source,
                               base / "project-config.log", env=env))
    config = json.loads((base / "project-config.log").read_text())
    commands.append(run_logged([*launch, "project", "index", str(source)], source,
                               base / "index.log", env=env, timeout=7200))
    output = (base / "index.log").read_text()
    verify_indexing_log(output)
    summary = re.search(r"Indexed files per language: (.+)", output)
    require(summary is not None, "Serena 언어별 색인 완료 내역이 없습니다")
    counts = {name: int(count) for name, count in re.findall(r"(go|typescript)\s*[:=]\s*(\d+)", summary.group(1))}
    require(set(counts) == {"go", "typescript"} and all(counts.values()), "Serena Go·TypeScript 색인 누락")
    failures = re.search(r"Failed to index (\d+) files", output)
    failed_count = int(failures.group(1)) if failures else 0
    # Exit code zero alone is insufficient: Serena catches individual file failures.
    require(failed_count == 0, f"Serena 색인 실패 {failed_count}개: {source / '.serena/logs/indexing.txt'}")
    caches = sorted((source / ".serena/cache").rglob("raw_document_symbols.pkl"))
    require({path.parent.name for path in caches} == {"go", "typescript"}
            and all(path.stat().st_size > 0 for path in caches), "Serena 언어별 심볼 캐시 누락")
    return [source / ".serena", home], {
        "external_ai": False, "backend": "LSP", "project": "grafana", "workspace_folders": ["."],
        "configuration": config, "indexed_files_per_language": counts, "failed_files": failed_count,
        "native_failure_log": str(source / ".serena/logs/indexing.txt") if failed_count else None,
        "symbol_caches": [{"path": str(path), "sha256": file_digest(path)} for path in caches],
        "output": "native defaults", "context": "desktop-app (native default; full product read tools)",
        "agent_interface": "tools", "native_agent_interface_default": config["agent_interface_default"],
        "agent_interface_exception": "REPL permits arbitrary Python outside the search-only contract",
        "modes": ["one-shot", "no-memories"], "read_only": True,
        "excluded_operations": ["editing", "shell", "memory", "project activation/removal", "mode switching"],
        "runtime": artifact["runtime"],
        "language_server_dependencies": language_dependencies(home),
    }, commands


def language_dependencies(home: Path) -> list[dict]:
    dependencies = []
    for package in ("typescript", "typescript-language-server"):
        manifests = list((home / "serena/language_servers/static/TypeScriptLanguageServer").glob(f"*/node_modules/{package}/package.json"))
        require(len(manifests) == 1, f"Serena 언어 서버 의존성 경로가 모호합니다: {package}")
        manifest = manifests[0]
        dependencies.append({"package": package, "version": read_json(manifest)["version"],
                             "manifest": str(manifest), "sha256": file_digest(manifest)})
    return dependencies


def verify_indexing_log(text: str) -> None:
    require(not re.search(r"Timeout waiting for TypeScript server to become ready|"
                          r"indexing did not complete within", text, re.IGNORECASE),
            "Serena LSP 색인이 끝나기 전에 timeout으로 진행했습니다")


def verify_probe(name: str, result: dict) -> None:
    text = "\n".join(item.get("text", "") for item in result.get("content", []))
    require(text and not re.search(r"(?im)^(error|error executing tool|no active project)\b", text),
            f"Serena 중립 조회 실패: {name}: {text[:300]}")
    if name == "get_current_config":
        require(all(line in text for line in ("Active project: grafana", "Language backend: LSP",
                                             "Language server status: ready", "Agent interface: tools")),
                "Serena 활성 프로젝트/언어 서버 준비 상태 불일치")
    if name == "get_symbols_overview":
        symbols = json.loads(text)
        require(isinstance(symbols, (dict, list)) and bool(symbols), "Serena 심볼 조회가 비어 있습니다")
