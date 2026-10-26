"""Harness-owned filesystem/network boundaries; native product settings stay unchanged."""
from __future__ import annotations

import contextlib
import fnmatch
import hashlib
import io
import json
import os
import pickle
import re
import shutil
import sqlite3
import stat
import subprocess
import sys
import tempfile
import urllib.parse
import uuid
from pathlib import Path
from typing import Callable

from .core import ContractError, canonical, digest, file_digest, read_json, require

METADATA = {".git", ".codemap", ".codegraph", ".codebase-memory", ".zvec-grep", ".serena", ".codex", "graphify-out"}
SYSTEM_READ = ("/bin", "/sbin", "/usr", "/System/Library", "/System/Cryptexes", "/Library/Apple",
               "/Library/Preferences", "/private/etc", "/private/var/db", "/dev", "/opt/homebrew")


def private_environment(home: Path, slot_tmp: Path | None = None) -> dict:
    home = home.resolve()
    tmp = (slot_tmp or home / "tmp").resolve()
    require(tmp.is_relative_to(home) and tmp != home, "private tmp는 arm home 내부여야 합니다")
    paths = {"HOME": home, "TMPDIR": tmp, "XDG_CONFIG_HOME": home / "config",
             "XDG_CACHE_HOME": home / "cache", "XDG_DATA_HOME": home / "data",
             "XDG_STATE_HOME": home / "state", "XDG_RUNTIME_DIR": home / "runtime", "npm_config_cache": home / "npm-cache",
             "UV_CACHE_DIR": home / "uv-cache"}
    for path in paths.values():
        path.mkdir(parents=True, exist_ok=True, mode=0o700)
    return {**{key: str(path) for key, path in paths.items()}, "PYTHONDONTWRITEBYTECODE": "1",
            "npm_config_userconfig": str(home / "npmrc"), "npm_config_globalconfig": str(home / "npm-globalrc")}


def preflight(source: Path, home: Path) -> dict:
    """Path sandboxes cannot distinguish hard links; never admit them to writable roots."""
    counts = {"files": 0, "symlinks": 0}
    def unreadable(error):
        raise error
    for root in (source.resolve(), home.resolve()):
        require(root.is_dir(), "격리 root 누락")
        for directory, dirs, files in os.walk(root, followlinks=False, onerror=unreadable):
            for name in dirs + files:
                path = Path(directory) / name
                info = path.lstat()
                if stat.S_ISLNK(info.st_mode):
                    target = path.resolve()
                    require(target.is_relative_to(root) and target.exists(), f"격리 root 밖 symlink: {path}")
                    counts["symlinks"] += 1
                elif stat.S_ISREG(info.st_mode):
                    require(info.st_nlink == 1, f"hard link 금지: {path}")
                    counts["files"] += 1
                else:
                    require(stat.S_ISDIR(info.st_mode) or stat.S_ISSOCK(info.st_mode), f"특수 파일 금지: {path}")
    return counts


def runtime_roots(artifact: dict) -> list[Path]:
    roots = [Path(artifact["installation"]).resolve()]
    candidates = list(artifact.get("launch", []))
    for value in artifact.get("runtime", {}).values():
        if isinstance(value, dict) and value.get("path"):
            candidates.append(value["path"])
    for value in candidates:
        path = Path(value)
        if not path.is_absolute() or not path.exists():
            continue
        resolved = path.resolve()
        roots.append(resolved.parent)
        if "python-runtimes" in resolved.parts:
            i = resolved.parts.index("python-runtimes")
            roots.append(Path(*resolved.parts[:i + 2]))
        if "node" in resolved.parts and "versions" in resolved.parts:
            i = resolved.parts.index("versions")
            roots.append(Path(*resolved.parts[:i + 3]))
        python = path.with_name("python")
        if python.exists() and "python-runtimes" in python.resolve().parts:
            parts = python.resolve().parts
            roots.append(Path(*parts[:parts.index("python-runtimes") + 2]))
    return sorted(set(roots))


def _profile_text(read_roots, write_roots, *, port=None, unix_roots=(), cbm=False, preparation_network=False) -> str:
    lines = ["(version 1)", "(allow default)", "(deny file-read-data)", "(deny file-write*)", "(deny network*)",
             '(allow file-read-data (literal "/"))']
    for operation, paths in (("file-read-data", [*SYSTEM_READ, *read_roots]), ("file-write*", write_roots)):
        for path in sorted({str(Path(p).resolve()) for p in paths}):
            lines.append(f"(allow {operation} (subpath {json.dumps(path)}))")
    lines.append('(allow file-write* (literal "/dev/null") (literal "/dev/zero") (subpath "/dev/fd"))')
    if preparation_network:
        lines.append("(allow network*)")
    elif port is not None:
        require(type(port) is int and 1024 <= port <= 65535, "잘못된 예약 포트")
        lines.extend([f'(allow network-bind (local ip "localhost:{port}"))',
                      f'(allow network-inbound (local ip "localhost:{port}"))',
                      f'(allow network-outbound (remote ip "localhost:{port}"))'])
    for root in unix_roots:
        lines.append(f"(allow network* (subpath {json.dumps(str(Path(root).resolve()))}))")
    if cbm:
        lines.append('(allow file-read-data file-write* (regex #"^/private/tmp/cbm-[^/]+(/.*)?$"))')
        for path in sorted(cbm_temporary_paths()):
            lines.append(f"(deny file-read-data file-write* (subpath {json.dumps(str(path.resolve()))}))")
        for path in sorted({p for root in [*read_roots, *write_roots] for p in Path(root).resolve().parents}):
            lines.append(f"(allow file-read-data (literal {json.dumps(str(path))}))")
    return "\n".join(lines) + "\n"


def product_profile(ident: str, state: dict, artifact: dict, *, slot_tmp: Path) -> dict:
    from .tool_indexes import CBM_UI_PORT, ZVEC_PORT, product_environment
    source, home = Path(state["source"]).resolve(), Path(state["home"]).resolve()
    require(ident == artifact["id"] and source != home and not source.is_relative_to(home)
            and not home.is_relative_to(source), "제품 격리 identity/root 불일치")
    env = {**product_environment(ident, home, source), **private_environment(home, slot_tmp)}
    # Serena's native cache must stay at the prepared path, not a host-user cache.
    if ident == "serena":
        env["XDG_CACHE_HOME"] = str(home / "cache")
    preflight(source, home)
    reads = [source, home, *runtime_roots(artifact)]
    writes = [source, home]
    unix = [home]
    port = ZVEC_PORT if ident == "zvec-grep" else CBM_UI_PORT if ident == "codebase-memory-mcp" else None
    if ident == "codebase-memory-mcp":
        runtime = Path(env["CBM_RUNTIME_DIR"])
        runtime.mkdir(parents=True, exist_ok=True, mode=0o700)
        reads.append(runtime)
        writes.append(runtime)
        unix.append(runtime)
    preparation_network = bool(state.get("preparation_network"))
    require(not preparation_network or ident == "serena", "승인되지 않은 제품 준비 network 예외")
    text = _profile_text(reads, writes, port=port, unix_roots=unix, cbm=ident == "codebase-memory-mcp",
                         preparation_network=preparation_network)
    # The profile is harness-owned, outside all product-writable roots.
    directory = source.parent / "isolation"
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    sha = hashlib.sha256(text.encode()).hexdigest()
    path = directory / (sha + ".sb")
    if path.exists():
        require(file_digest(path) == sha, "격리 profile 변경")
    else:
        path.write_text(text)
        path.chmod(0o600)
    return {"profile_path": str(path), "profile_sha256": sha, "env": env,
            "allowed": {"read": sorted({str(p) for p in reads}), "write": sorted({str(p) for p in writes}),
                        "system_read": list(SYSTEM_READ), "unix": [str(p) for p in unix],
                        "cbm_tmp": "/private/tmp/cbm-*" if ident == "codebase-memory-mcp" else None,
                        "preparation_network": preparation_network}, "port": port}


def wrap(launch: list[str], profile: dict) -> list[str]:
    require(sys.platform == "darwin" and Path("/usr/bin/sandbox-exec").is_file(), "macOS Seatbelt가 필요합니다")
    require(file_digest(Path(profile["profile_path"])) == profile["profile_sha256"], "격리 profile hash 불일치")
    require(launch and Path(launch[0]).name != "sandbox-exec", "중첩 Seatbelt 금지")
    return ["/usr/bin/sandbox-exec", "-f", profile["profile_path"], *launch]


def cbm_temporary_paths() -> set[Path]:
    return set(Path("/private/tmp").glob("cbm-*"))


def cleanup_cbm_temporary(before: set[Path]) -> list[str]:
    removed = []
    for path in sorted(cbm_temporary_paths() - before):
        require(path.lstat().st_uid == os.geteuid() and not path.is_symlink(), "CBM 임시 파일 소유권 불명확")
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()
        removed.append(str(path))
    return removed


# Pinned release inventory. Unknown tools/arguments require explicit policy review, not inference.
PINNED_ARGUMENTS = {
    'codemap-search': {
        'initial_instructions': set(''.split()),
        'overview': set('format path'.split()),
        'search': set('caller_context debug event_key extension_hint include_events include_seen language_hint query'.split()),
        'read': set('debug expand file_path include_events include_seen limit offset unresolved view'.split()),
        'find': set('entry_type include_ignored max_depth path pattern pattern_type'.split()),
        'grep': set('-A -B -C -i -n debug expand glob head_limit include_events include_ignored include_seen multiline offset output_mode path pattern type unresolved view'.split()),
        'analyze': set('compare days filter language limit offset order sort target tool view'.split()),
    },
    'codegraph': {
        'codegraph_explore': set('maxFiles projectPath query'.split()),
    },
    'zvec-grep': {
        'zvec_grep_search': set('autoUpdate embeddingConcurrency excludedFileTypes fileTypes follow freshness fts fuse globs hidden ignoreFiles insensitiveGlobs limit maxDepth maxFileSizeBytes modifiedAfter modifiedBefore noIgnore preferSymbol queries query root symbolTypes trace vector'.split()),
    },
    'graphify': {
        'query_graph': set('context_filter depth mode project_path question token_budget'.split()),
        'get_node': set('label node_id project_path'.split()),
        'get_neighbors': set('label node_id project_path relation_filter token_budget'.split()),
        'get_community': set('community_id project_path token_budget'.split()),
        'god_nodes': set('exclude_hubs_percentile project_path top_n'.split()),
        'graph_stats': set('project_path'.split()),
        'shortest_path': set('max_hops project_path source target undirected'.split()),
        'list_prs': set('base project_path repo'.split()),
        'get_pr_impact': set('pr_number project_path repo'.split()),
        'triage_prs': set('base project_path repo'.split()),
    },
    'codebase-memory-mcp': {
        'index_repository': set('async doc_candidates mode name persistence repo_path status target_projects'.split()),
        'search_graph': set('detail exclude_entry_points fields file_pattern format include_connected kind label limit max_degree max_output_tokens min_degree name_pattern namespace offset project qn_pattern query relationship semantic_limit semantic_offset semantic_query'.split()),
        'query_graph': set('cursor format graph max_output_tokens max_rows offset project query'.split()),
        'trace_path': set('cursor depth direction edge_types format function_name include_evidence include_tests limit max_output_tokens mode parameter_name project risk_labels'.split()),
        'get_code_snippet': set('format include_neighbors max_lines max_output_tokens member_limit member_offset project qualified_name source_mode start_line'.split()),
        'get_file_outline': set('file_path format labels limit offset project'.split()),
        'get_graph_schema': set('diagnostics format limit offset project'.split()),
        'compare_graphs': set('base_project limit scan_limit target_project'.split()),
        'get_architecture': set('aspects entry_node_id expected_generation format include_behavior_evidence path project target_node_id'.split()),
        'search_code': set('context debug directory_limit directory_offset file_pattern format limit match_limit max_output_tokens mode path_filter pattern project raw_content_offset raw_limit raw_offset regex result_limit result_offset source_max_lines'.split()),
        'list_projects': set('detail format include_details limit metadata_only offset'.split()),
        'delete_project': set('project'.split()),
        'index_status': set('diagnostics format project verbose'.split()),
        'check_index_coverage': set('diagnostics format path_limit path_offset paths project scope_limit scope_offset scopes'.split()),
        'detect_changes': set('base_branch changed_cursor changed_limit changed_offset config_path depth direction edge_types fixpoint format impact_cursor impact_offset limit max_output_tokens module_cursor module_limit module_offset project scope seed_cursor seed_filter seed_limit seed_offset seeds since via'.split()),
        'manage_adr': set('content format mode project section_limit section_offset section_updates'.split()),
        'ingest_traces': set('label project traces'.split()),
    },
    'serena': {
        'create_text_file': set('content relative_path'.split()),
        'replace_content': set('allow_multiple_occurrences mode needle relative_path repl'.split()),
        'replace_in_files': set('dry_run expected_count max_answer_chars mode needle occurrence_ids paths_exclude_glob paths_include_glob relative_path repl'.split()),
        'replace_symbol_body': set('body name_path relative_path'.split()),
        'insert_after_symbol': set('body name_path relative_path'.split()),
        'insert_before_symbol': set('body name_path relative_path'.split()),
        'read_file': set('end_line max_answer_chars relative_path start_line'.split()),
        'list_dir': set('max_answer_chars recursive relative_path skip_ignored_files'.split()),
        'find_file': set('file_mask relative_path skip_ignored_files'.split()),
        'search_for_pattern': set('context_lines_after context_lines_before max_answer_chars multiline paths_exclude_glob paths_include_glob relative_path restrict_search_to_code_files skip_ignored_files substring_pattern'.split()),
        'get_symbols_overview': set('depth max_answer_chars relative_path'.split()),
        'find_symbol': set('depth exclude_kinds include_body include_info include_kinds max_answer_chars max_matches name_path_pattern relative_path substring_matching'.split()),
        'find_referencing_symbols': set('exclude_kinds include_kinds max_answer_chars name_path relative_path'.split()),
        'find_implementations': set('exclude_kinds include_info include_kinds max_answer_chars name_path relative_path'.split()),
        'find_declaration': set('containing_symbol_name_path include_body include_info regex relative_path'.split()),
        'get_diagnostics_for_file': set('end_line max_answer_chars min_severity relative_path start_line'.split()),
        'rename_symbol': set('name_path new_name relative_path'.split()),
        'safe_delete_symbol': set('name_path_pattern relative_path'.split()),
        'write_memory': set('content max_chars memory_name'.split()),
        'read_memory': set('memory_name'.split()),
        'list_memories': set('topic'.split()),
        'delete_memory': set('memory_name'.split()),
        'rename_memory': set('new_name old_name'.split()),
        'edit_memory': set('allow_multiple_occurrences memory_name mode needle repl'.split()),
        'execute_shell_command': set('capture_stderr command cwd max_answer_chars'.split()),
        'activate_project': set('project session'.split()),
        'get_current_config': set(''.split()),
        'onboarding': set(''.split()),
        'initial_instructions': set(''.split()),
    },
}

QUERY_TOOLS = {
    "codemap-search": set("initial_instructions overview search read find grep analyze".split()),
    "codegraph": {"codegraph_explore"}, "zvec-grep": {"zvec_grep_search"},
    "graphify": set("query_graph get_node get_neighbors get_community god_nodes graph_stats shortest_path".split()),
    "codebase-memory-mcp": set("search_graph query_graph trace_path get_code_snippet get_file_outline get_graph_schema compare_graphs get_architecture search_code list_projects index_status check_index_coverage detect_changes".split()),
    "serena": set("read_file list_dir find_file search_for_pattern get_symbols_overview find_symbol find_referencing_symbols find_implementations find_declaration get_diagnostics_for_file get_current_config initial_instructions".split()),
    "rg": {"rg", "read"},
}
PINNED_PROMPTS = {"codebase-memory-mcp": {"explore_codebase": {"project", "question"},
                                             "review_change_impact": {"project", "change", "base_branch"}}}
ALIASES = {("codemap-search", "read"): {"path", "file", "query"},
           ("codemap-search", "overview"): {"file_path", "file", "query"},
           ("codemap-search", "search"): {"workspace_scope", "scope"},
           ("codemap-search", "grep"): {"include", "file_pattern"}}


# Arguments a pinned release adds only for some corpora. codemap-search exposes workspace_scope on
# search in monorepos (Grafana); RelayPolicy validates it as a path through ALIASES.
CORPUS_DEPENDENT_ARGUMENTS = {("codemap-search", "search"): {"workspace_scope"},
                              ("codemap-search", "read"): {"ranges"}}


def validate_inventory(ident: str, inventory: list[dict]) -> None:
    require(ident in PINNED_ARGUMENTS or ident == "rg", "미검토 제품")
    if ident == "rg":
        return
    for tool in inventory:
        require(tool["name"] in PINNED_ARGUMENTS[ident], f"미검토 native 도구: {ident}/{tool['name']}")
        actual = set(tool["inputSchema"].get("properties", {}))
        expected = PINNED_ARGUMENTS[ident][tool["name"]]
        optional = CORPUS_DEPENDENT_ARGUMENTS.get((ident, tool["name"]), set())
        require(expected <= actual <= expected | optional, f"native 인자 inventory 변경: {ident}/{tool['name']}")


class RelayPolicy:
    def __init__(self, ident: str, state: dict, inventory: list[dict], *, tracked=None):
        self.ident, self.state = ident, state
        self.source = Path(state["source"]).resolve()
        self.inventory = {t["name"]: t for t in inventory}
        if tracked is None:
            from .tool_indexes import CORPUS
            tracked = read_json(CORPUS)["files"]
        self.tracked = set(tracked)
        self.directories = {"."}
        for name in self.tracked:
            self.directories.update(str(p) for p in Path(name).parents)

    def path(self, value, *, file=False) -> Path:
        require(isinstance(value, str) and value and "\x00" not in value, "source 경로 문자열 필요")
        if value.startswith("file:"):
            uri = urllib.parse.urlsplit(value)
            require(uri.scheme == "file" and uri.netloc in {"", "localhost"} and not uri.query and not uri.fragment,
                    "외부/비정규 file URI 금지")
            value = urllib.parse.unquote(uri.path)
        require("://" not in value and "\x00" not in value, "잘못된 source URI")
        raw = Path(value)
        lexical = raw if raw.is_absolute() else self.source / raw
        require(not METADATA.intersection(lexical.parts), "제품/실행 metadata 접근 금지")
        resolved = lexical.resolve()
        require(resolved.is_relative_to(self.source), "고정 source 밖의 경로")
        relative = str(resolved.relative_to(self.source))
        require(not METADATA.intersection(resolved.relative_to(self.source).parts), "제품/실행 metadata 접근 금지")
        require(relative in self.tracked or not file and relative in self.directories, "추적 corpus에 없는 경로")
        if resolved.is_file():
            require(resolved.stat().st_nlink == 1, "hard link 금지")
        return resolved

    def glob(self, value) -> None:
        values = value if isinstance(value, list) else [value]
        require(all(isinstance(v, str) for v in values), "glob 문자열/배열 필요")
        for pattern in values:
            require("\x00" not in pattern and "://" not in pattern and not pattern.startswith("file:"), "glob URI/NUL 금지")
            for alternative in re.split(r"[{},]+", pattern):
                alternative = alternative.lstrip("!")
                parts = Path(alternative).parts
                require(".." not in parts and not METADATA.intersection(parts), "glob 경계 이탈")
                prefix = re.split(r"[*?\[]", alternative, 1)[0]
                if prefix:
                    literal = Path(prefix if prefix == alternative or prefix.endswith("/") else str(Path(prefix).parent))
                    lexical = literal if literal.is_absolute() else self.source / literal
                    require(lexical.is_relative_to(self.source), "고정 source 밖의 경로")
                    # A glob filters files below the validated search root and may be relative to it, so a literal
                    # part that names nothing in the source is not an access. Existing targets still validate, so a
                    # glob cannot name an escaping symlink, metadata or a hard link.
                    if lexical != self.source and (lexical.exists() or lexical.is_symlink()):
                        self.path(str(lexical))
                # Reject matching actual metadata even through wildcard spellings (e.g. .* / *).
                # Ordinary recursive globs remain valid; product ignore defaults fence implicit metadata.
                for part in parts:
                    if part not in {"*", "**"} and any(fnmatch.fnmatchcase(m, part) for m in METADATA):
                        raise ContractError("metadata를 지목하는 glob 금지")

    def validate(self, name: str, arguments: dict) -> None:
        require(name in self.inventory and name in QUERY_TOOLS[self.ident] | ({"read"} if self.ident == "graphify" else set()),
                "허용되지 않은 탐색 도구")
        require(isinstance(arguments, dict), "도구 arguments 객체 필요")
        properties = self.inventory[name]["inputSchema"].get("properties", {})
        allowed = set(properties) | ALIASES.get((self.ident, name), set())
        require(set(arguments) <= allowed, "알려지지 않은 도구 인자")
        for key, value in arguments.items():
            if self.ident == "codemap-search" and name == "read" and key == "ranges" and value is None:
                continue  # Native read treats null as absent; forward it unchanged.
            schema = properties.get(key, {})
            types = schema.get("type")
            types = types if isinstance(types, list) else [types]
            matches = {"string": isinstance(value, str), "integer": type(value) is int,
                       "number": type(value) in (float, int), "boolean": type(value) is bool,
                       "array": isinstance(value, list), "object": isinstance(value, dict), "null": value is None}
            require(None in types or any(matches.get(t, False) for t in types), f"도구 인자 타입 오류: {key}")
        if self.ident == "rg" or self.ident == "graphify" and name == "read":
            if "file_path" in arguments:
                self.path(arguments["file_path"], file=True)
            if "path" in arguments:
                self.path(arguments["path"])
            if "glob" in arguments:
                self.glob(arguments["glob"])
        elif self.ident == "codemap-search":
            if name in {"read", "overview"}:
                values = []
                for key in ("file_path", "path", "file", "query"):
                    if key in arguments:
                        value = arguments[key]
                        if name == "overview" and isinstance(value, str) and value in {"", "all", "전체"}:
                            value = "."
                        values.append(self.path(value, file=name == "read"))
                require(len(set(values)) <= 1, "서로 다른 path aliases")
                if name == "read" and arguments.get("ranges") is not None:
                    require("ranges" in properties and isinstance(arguments["ranges"], list), "미광고 ranges 인자")
                    require(not values and 1 <= len(arguments["ranges"]) <= 20, "read ranges 개수/단일 경로 충돌")
                    for item in arguments["ranges"]:
                        require(isinstance(item, dict) and set(item) <= {"file_path", "path", "file", "query", "offset", "limit"},
                                "read range 인자 오류")
                        paths = [self.path(item[key], file=True) for key in ("file_path", "path", "file", "query") if key in item]
                        require(bool(paths) and len(set(paths)) == 1, "read range path aliases 오류")
                        for key in ("offset", "limit"):
                            require(key not in item or type(item[key]) is int, "read range 위치 오류")
            elif name == "search":
                for key in ("workspace_scope", "scope"):
                    if key in arguments:
                        value = arguments[key]
                        require(isinstance(value, str), "scope 문자열 필요")
                        if value not in {"", "all", "전체"}:
                            self.path(value)
            elif name in {"find", "grep"}:
                if "path" in arguments:
                    self.path(arguments["path"])
                require(arguments.get("include_ignored") is not True, "metadata ignore bypass 금지")
                for key in ("glob", "include", "file_pattern"):
                    if key in arguments:
                        self.glob(arguments[key])
                if name == "find" and arguments.get("pattern_type", "glob") == "glob" and "pattern" in arguments:
                    self.glob(arguments["pattern"])
        elif self.ident == "codegraph":
            if "projectPath" in arguments:
                require(self.path(arguments["projectPath"]) == self.source, "다른 workspace selector")
        elif self.ident == "graphify":
            if "project_path" in arguments:
                value = arguments["project_path"]
                expected = Path(self.state["modes"]["graph_path"]).parent.parent.resolve()
                require(isinstance(value, str) and ":" not in value and
                        (self.source / value).resolve() == expected, "고정 graph selector 외 경로")
        elif self.ident == "zvec-grep":
            if "root" in arguments:
                require(self.path(arguments["root"]) == self.source, "다른 workspace root")
            for key in ("globs", "insensitiveGlobs"):
                if key in arguments:
                    self.glob(arguments[key])
            if "ignoreFiles" in arguments:
                values = arguments["ignoreFiles"]
                for value in values if isinstance(values, list) else [values]:
                    self.path(value, file=True)
            require(not any(arguments.get(k) is True for k in ("hidden", "noIgnore", "follow")), "metadata/링크 탐색 bypass 금지")
        elif self.ident == "serena":
            if arguments.get("relative_path", "") != "":
                self.path(arguments["relative_path"], file=name in {"read_file", "get_symbols_overview", "get_diagnostics_for_file"})
            for key in ("paths_include_glob", "paths_exclude_glob", "file_mask"):
                if key in arguments:
                    self.glob(arguments[key])
            require(arguments.get("skip_ignored_files") is not False, "metadata ignore bypass 금지")
        elif self.ident == "codebase-memory-mcp":
            project = self.state["modes"]["native_readiness"]["project"]
            for key in ("project", "base_project", "target_project"):
                if key in arguments:
                    require(arguments[key] == project, "고정 project selector 이탈")
            for key in ("file_path", "path", "path_filter", "config_path"):
                if key in arguments:
                    self.path(arguments[key], file=key in {"file_path", "config_path"})
            for key in ("paths", "scopes"):
                if key in arguments:
                    require(isinstance(arguments[key], list), "경로 배열 필요")
                    for value in arguments[key]:
                        self.path(value, file=key == "paths")
            if "file_pattern" in arguments:
                self.glob(arguments["file_pattern"])

    def prompt_inventory(self, prompts: list[dict]) -> dict:
        actual = {item["name"]: {arg["name"] for arg in item.get("arguments", [])} for item in prompts}
        require(all(name in PINNED_PROMPTS.get(self.ident, {}) and keys == PINNED_PROMPTS[self.ident][name]
                    for name, keys in actual.items()), "미검토 product prompt inventory")
        return actual

    def prompt(self, name: str, arguments: dict, advertised: dict) -> None:
        require(name in advertised and isinstance(arguments, dict) and set(arguments) <= advertised[name],
                "광고되지 않은 prompt/인자")
        require(all(isinstance(value, str) for value in arguments.values()), "prompt 인자는 문자열이어야 합니다")
        if self.ident == "codebase-memory-mcp" and "project" in arguments:
            require(arguments["project"] == self.state["modes"]["native_readiness"]["project"], "고정 prompt project 이탈")

    def uri(self, value: str, advertised: set[str]) -> None:
        require(isinstance(value, str), "resource URI 문자열 필요")
        if value.startswith("file:"):
            self.path(value, file=True)
            return
        parsed = urllib.parse.urlsplit(value)
        require(parsed.scheme not in {"", "http", "https", "ftp", "data"} and value in advertised,
                "광고되지 않은 product resource URI")
        decoded = urllib.parse.unquote(parsed.path)
        require(".." not in Path(decoded).parts and not METADATA.intersection(Path(decoded).parts), "resource URI 경계 이탈")


def hidden_references() -> list[Path]:
    from . import settings, route_contract as routes
    import pwd
    home = Path(pwd.getpwuid(os.getuid()).pw_dir)
    return [routes.DATA / "dataset.json", routes.DATA / "evidence.json", routes.DATA / "grading.json",
            settings.REPOSITORY, settings.REPOSITORY / ".git", routes.PREPARATION / "grading-contract",
            routes.PREPARATION / "plans", routes.PREPARATION / "campaigns", routes.EVIDENCE,
            home / ".pi", home / ".codex", home / ".claude", home / ".claude.json",
            home / ".claude/projects", home / ".claude/todos", home / ".claude/shell-snapshots",
            home / ".claude/history.jsonl", Path("/Users/buyong/tmp"),
            Path("/Volumes/Samsung_990_PRO/worktrees/buyong-mcp"), Path("/Volumes/Samsung_990_PRO/codemap-bench-work")]


@contextlib.contextmanager
def canary_files(source: Path, directory: Path, *, metadata_root: Path | None = None):
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    token = "ISOLATION_" + uuid.uuid4().hex
    root = Path(tempfile.mkdtemp(prefix="canaries-", dir=directory))
    paths = {}
    link = source / (".boundary-" + uuid.uuid4().hex)
    metadata = metadata_root or source / ".codemap"
    made_metadata = not metadata.exists()
    metadata.mkdir(exist_ok=True)
    meta_file = metadata / ("canary-" + uuid.uuid4().hex)
    try:
        paths = {name: root / name / "canary.txt" for name in
                 ("tmp", "pi", "codex", "claude", "grading-contract", "plans", "runs", "other-workspace", "worktree")}
        paths.update({"claude-projects": root / ".claude/projects/canary.txt",
                      "claude-todos": root / ".claude/todos/canary.txt",
                      "claude-shell-snapshots": root / ".claude/shell-snapshots/canary.txt",
                      "claude-history": root / ".claude/history.jsonl",
                      "claude-config-history": root / ".claude.json"})
        for path in paths.values():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(token)
        meta_file.write_text(token)
        link.symlink_to(paths["tmp"])
        yield token, {**paths, "symlink": link, "metadata": meta_file}
    finally:
        link.unlink(missing_ok=True)
        meta_file.unlink(missing_ok=True)
        if made_metadata:
            metadata.rmdir()
        shutil.rmtree(root)


def boundary_canaries(ident: str, call: Callable[[str, dict], dict], state: dict) -> list[dict]:
    source = Path(state["source"])
    rows = [{"kind": "existence_only", "path": str(path), "exists": path.exists()} for path in hidden_references()]
    inventory = state.get("boundary_inventory")
    if inventory is None:
        inventory = read_json(Path(state["native_inventory"]))["tools"] if state.get("native_inventory") else []
    enabled = state.get("relay_settings", {}).get("enabled_tools", QUERY_TOOLS[ident])
    metadata = {"codemap-search": source / ".codemap", "codegraph": source / ".codegraph",
                "zvec-grep": source / ".zvec-grep", "serena": source / ".serena",
                "codebase-memory-mcp": Path(state["home"]),
                "graphify": Path(state["modes"]["graph_path"]).parent if ident == "graphify" else source / "graphify-out"}
    with canary_files(source, source.parent / "boundary", metadata_root=metadata.get(ident)) as (token, targets):
        for name, key, seed, array in path_argument_cases(ident, inventory):
            if name not in enabled:
                continue
            for label, target in targets.items():
                for spelling in (str(target), target.resolve().as_uri()):
                    arguments = {**seed, key: [spelling] if array else spelling}
                    try:
                        response = call(name, arguments)
                    except (ValueError, OSError) as exc:
                        rows.append({"tool": name, "argument": key, "target": label, "uri": spelling.startswith("file:"),
                                     "rejected": True, "error": str(exc)})
                    else:
                        require(token not in canonical(response), f"격리 canary 유출: {ident}/{name}/{key}/{label}")
                        # An empty search is not evidence of an enforced input boundary.
                        require(response.get("isError") is True or "error" in response,
                                f"relay 경계 거절 누락: {ident}/{name}/{key}/{label}")
                        rows.append({"tool": name, "argument": key, "target": label, "rejected": True,
                                     "result_sha256": digest(response)})
    require(any(row.get("rejected") for row in rows), "canary 경로 inventory 없음")
    return rows


def path_argument_cases(ident: str, inventory: list[dict]) -> list[tuple]:
    cases = []
    names = {t["name"]: set(t["inputSchema"].get("properties", {})) for t in inventory}
    if not names:
        names = {name: keys for name, keys in PINNED_ARGUMENTS.get(ident, {}).items() if name in QUERY_TOOLS[ident]}
    for name, keys in names.items():
        if name not in QUERY_TOOLS[ident] | ({"read"} if ident == "graphify" else set()):
            continue
        paths = keys & {"path", "file_path", "relative_path", "root", "projectPath", "project_path", "config_path",
                        "glob", "include", "file_pattern", "path_filter", "paths", "scopes", "globs", "ignoreFiles",
                        "insensitiveGlobs", "paths_include_glob", "paths_exclude_glob", "file_mask",
                        "project", "base_project", "target_project"}
        paths |= ALIASES.get((ident, name), set())
        if ident == "codemap-search" and name == "find":
            paths.add("pattern")
        for key in sorted(paths):
            cases.append((name, key, {}, key in {"paths", "scopes"}))
            if key in {"globs", "ignoreFiles", "insensitiveGlobs"}:
                cases.append((name, key, {}, True))
    return cases

class _IndexUnpickler(pickle.Unpickler):
    def find_class(self, module, name):
        raise pickle.UnpicklingError("index pickle globals 금지")


_TANTIVY_PATHS = r'''
use tantivy::{Index, schema::{TantivyDocument, Value}};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let index = Index::open_in_dir(std::env::args().nth(1).ok_or("index path missing")?)?;
    let field = index.schema().get_field("file_path")?;
    let reader = index.reader()?;
    let searcher = reader.searcher();
    let mut paths = std::collections::BTreeSet::new();
    for segment in searcher.segment_readers() {
        let store = segment.get_store_reader(16)?;
        for id in segment.doc_ids_alive() {
            let doc: TantivyDocument = store.get(id)?;
            if let Some(value) = doc.get_first(field).and_then(|v| v.as_str()) { paths.insert(value.to_owned()); }
        }
    }
    for path in paths { println!("{path}"); }
    Ok(())
}
'''


def audit_index_paths(ident: str, state: dict, corpus: dict) -> dict:
    """Read raw path fields, not search results. A missing/unknown index format fails closed."""
    from .tool_artifacts import load_artifacts
    source, home = Path(state["source"]).resolve(), Path(state["home"]).resolve()
    preflight(source, home)
    tracked = set(corpus["files"])
    directories = {str(parent) for path in tracked for parent in Path(path).parents}
    groups = {}

    def compare(label, paths, *, synthetic=None, allow_directories=False, unresolved_references=None):
        approved, matched, rejected, unresolved = set(), set(), set(), set()
        unique_paths = set()
        for value in paths:
            if value is None or value == "":
                continue
            require(isinstance(value, str), "raw index 경로 타입 변경")
            unique_paths.add(value)
        for value in unique_paths:
            path = Path(value)
            resolved = (path if path.is_absolute() else source / path).resolve()
            if not resolved.is_relative_to(source):
                rejected.add(value)
                continue
            relative = str(resolved.relative_to(source))
            if relative in tracked:
                matched.add(relative)
            elif allow_directories and relative in directories:
                approved.add(relative)
            elif synthetic and synthetic(value):
                approved.add(value)
            elif (unresolved_references is not None and value in unresolved_references
                  and not path.is_absolute() and ".." not in path.parts and ":" not in value and "\\" not in value):
                try:
                    resolved.lstat()
                except FileNotFoundError:
                    unresolved.add(value)
                else:
                    rejected.add(value)  # An existing untracked file/directory is never a reference exemption.
            else:
                rejected.add(value)
        groups[label] = {"tracked": len(matched), "approved_synthetic": len(approved),
                         "unapproved": sorted(rejected), "paths_sha256": digest(sorted(matched | approved | rejected | unresolved))}
        if unresolved_references is not None:
            groups[label].update(unresolved_reference=len(unresolved), unresolved_reference_values=sorted(unresolved))
        require(not rejected, f"미승인 index 경로: {ident}/{label}: {sorted(rejected)[:8]}")

    def sql(path, queries):
        require(path.is_file(), f"raw index 누락: {path}")
        with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True) as connection:
            for label, query, synthetic, dirs in queries:
                compare(label, [row[0] for row in connection.execute(query)], synthetic=synthetic, allow_directories=dirs)

    if ident == "codegraph":
        sql(source / ".codegraph/codegraph.db", [("files", "select path from files", None, False),
            ("nodes", "select distinct file_path from nodes", None, False)])
    elif ident == "codebase-memory-mcp":
        databases = [path for path in home.glob("*.db") if path.name not in {"_config.db", "activity.db"}]
        require(len(databases) == 1, "CBM 단일 project index 필요")
        def synthetic(value):
            return value in {"<python-builtins>", "{}", ".codebase-memory/.semantic-input/git-context-v1",
                             ".codebase-memory/.semantic-input/global-extension-config-v1",
                             ".codebase-memory/.semantic-input/project-extension-config-v1"} and not (source / value).exists()
        with sqlite3.connect(databases[0].as_uri() + "?mode=ro", uri=True) as connection:
            excluded_directories = {row[0] for row in connection.execute(
                "select rel_path from index_coverage where kind='not_indexed_dir' and detail='excluded subtree'")
                if (source / row[0]).is_dir()}
        sql(databases[0], [("files", "select rel_path from file_hashes", synthetic, False),
                           ("nodes", "select distinct file_path from nodes", synthetic, True),
                           ("coverage", "select rel_path from index_coverage", lambda x: x in excluded_directories, True)])
    elif ident == "graphify":
        graph = read_json(Path(state["modes"]["graph_path"]))
        links = graph.get("links", graph.get("edges", []))
        compare("links", [row.get("source_file") for row in links])
        # Graphify 0.9.95 _emit_rescued_import mints absent module/path stubs,
        # not file nodes. Their AST import edge still names the real importing source.
        import_targets = {row["target"] for row in links if isinstance(row.get("target"), str)
                          and row.get("relation") == "dynamic_import" and row.get("_origin") == "ast"
                          and row.get("confidence") == "EXTRACTED"}
        def import_reference(row):
            return (isinstance(row.get("id"), str) and row["id"] in import_targets
                    and row.get("_origin") == "ast" and row.get("confidence") == "EXTRACTED"
                    and row.get("file_type") == "code" and row.get("type") in (None, "module")
                    and row.get("external") is None and not row.get("source_location"))
        reference_paths, other_paths = set(), set()
        for row in graph["nodes"]:
            if isinstance(row.get("source_file"), str):
                (reference_paths if import_reference(row) else other_paths).add(row["source_file"])
        compare("nodes", [row.get("source_file") for row in graph["nodes"]],
                unresolved_references=reference_paths - other_paths)
    elif ident == "serena":
        paths = sorted((source / ".serena/cache").rglob("raw_document_symbols.pkl"))
        require(paths, "Serena raw index 누락")
        for path in paths:
            data = _IndexUnpickler(io.BytesIO(path.read_bytes())).load()
            compare(str(path.relative_to(source)), list(data["obj"]))
    elif ident in {"codemap-search", "zvec-grep"}:
        artifact = load_artifacts(verify=False)[ident]
        directory = source.parent / "index-audit"
        directory.mkdir(exist_ok=True)
        if ident == "codemap-search":
            deps = Path(artifact["launch"][0]).parent / "deps"
            libs = list(deps.glob("libtantivy-*.rlib"))
            require(len(libs) == 1, "고정 Tantivy build library가 필요합니다")
            helper, binary = directory / "paths.rs", directory / "paths"
            helper.write_text(_TANTIVY_PATHS)
            subprocess.run(["rustc", "--edition=2021", str(helper), "--extern", "tantivy=" + str(libs[0]),
                            "-L", "dependency=" + str(deps), "-L", "dependency=" + str(deps.parents[2] / "release/deps"),
                            "-o", str(binary)], check=True, capture_output=True, timeout=120)
            indexes = [p.parent for p in (source / ".codemap").rglob("meta.json") if "schema" in read_json(p)]
            require(indexes, "committed Tantivy index 누락")
            for index in indexes:
                result = subprocess.run([str(binary), str(index)], check=True, capture_output=True, text=True, timeout=120)
                compare(str(index.relative_to(source)), result.stdout.splitlines())
        else:
            module = Path(artifact["installation"]) / "dist/engine/storage/zvec.js"
            storage = source / ".zvec-grep"
            manifest = read_json(storage / "manifest.json")
            require(Path(manifest["path"]).resolve() == storage and
                    {Path(row["absolutePath"]).resolve() for row in manifest["rootPaths"]} == {source},
                    "zvec raw index root 이탈")
            # Zvec readOnly open/close still mutates native .proxima files. Audit a disposable
            # APFS clone; stored absolute roots remain unchanged and are checked above/below.
            script = f'''import {{createWorkspaceIndexStorage}} from {json.dumps(module.as_uri())};
const s=createWorkspaceIndexStorage({{storagePath:process.argv[1],readOnly:true}});
try {{ console.log(JSON.stringify(s.listFiles().map(f=>[f.relativePath,f.absolutePath]))); }} finally {{s.close();}}'''
            profile = product_profile(ident, state, artifact, slot_tmp=home / "tmp" / "audit")
            with tempfile.TemporaryDirectory(dir=home / "tmp" / "audit", prefix="raw-index-") as temporary:
                clone = Path(temporary) / "storage"
                subprocess.run(["cp", "-cR", str(storage), str(clone)], check=True, capture_output=True, timeout=120)
                result = subprocess.run(wrap([artifact["launch"][0], "--input-type=module", "-e", script, str(clone)], profile),
                                        cwd=source, env=profile["env"], check=True, capture_output=True, text=True, timeout=120)
                files = json.loads(result.stdout)
            compare("files", [row[0] for row in files])
            require(all(isinstance(row[1], str) for row in files), "zvec absolute path inventory 누락")
            compare("absolute_files", [row[1] for row in files])
    else:
        require(ident == "rg", "미검토 index 형식")
    return {"status": "verified", "arm": ident, "source_sha256": corpus["source_sha256"], "groups": groups,
            "policy": "tracked source, explicit synthetic rows and tracked ancestors only; external paths rejected"
                      + ("; Graphify absent AST dynamic-import stubs recorded as unresolved_reference" if ident == "graphify" else "")}

# Claude Code network configuration: https://code.claude.com/docs/en/network-config
# Nonessential telemetry/update traffic is disabled. No wildcard or arbitrary HTTPS egress.
CLAUDE_PROXY_HOSTS = frozenset({"api.anthropic.com", "claude.ai", "platform.claude.com"})
CLAUDE_PROXY_PORT = 19550


def claude_baseline_profile(value: dict, folder: Path) -> dict:
    cli = Path(shutil.which("claude") or "claude").resolve()
    home = (folder / "cli-home").resolve()
    env = private_environment(home, home / "tmp")
    env.update(CLAUDE_CONFIG_DIR=str(home / "claude"), CLAUDE_CODE_TMPDIR=env["TMPDIR"],
               HTTP_PROXY=f"http://127.0.0.1:{CLAUDE_PROXY_PORT}", HTTPS_PROXY=f"http://127.0.0.1:{CLAUDE_PROXY_PORT}",
               NO_PROXY="")
    reads = [Path(value["source"]), Path(value["rg_binary"]).parent, cli.parent, home,
             folder / "claude-settings.json", folder / "mcp-config.json"]
    # Native CLI installs may be a node script rather than the standalone binary.
    node = shutil.which("node")
    if node:
        reads.extend(runtime_roots({"installation": str(cli.parent), "launch": [node]}))
    text = _profile_text(reads, [home], port=CLAUDE_PROXY_PORT, unix_roots=[home])
    text += "".join(f"(deny file-read-data (subpath {json.dumps(str(Path(value['source']) / name))}))\n"
                    for name in sorted(METADATA))
    path = folder / "claude-outer.sb"
    path.write_text(text)
    path.chmod(0o600)
    return {"profile_path": str(path.resolve()), "profile_sha256": file_digest(path), "env": env,
            "allowed": {"read": [str(p.resolve()) for p in reads], "write": [str(home)],
                        "proxy_hosts": sorted(CLAUDE_PROXY_HOSTS), "inner_sandbox": False}, "port": CLAUDE_PROXY_PORT}


def claude_baseline_command(command: list[str], profile: dict, folder: Path) -> list[str]:
    from .core import write_json
    config = folder / "claude-boundary.json"
    write_json(config, {"profile": profile, "proxy_log": str((folder / "claude-proxy.jsonl").resolve())})
    bootstrap = (f"import sys; sys.path.insert(0, {str(Path(__file__).resolve().parent.parent)!r}); "
                 "from pathlib import Path; from benchmark.isolation import _claude_launch; "
                 "sys.exit(_claude_launch(Path(sys.argv[1]), sys.argv[2:]))")
    return [sys.executable, "-c", bootstrap, str(config.resolve()), *command]


def _serve_proxy(port: int, log: Path) -> None:
    import select
    import socket
    import socketserver
    import threading
    from .core import append_jsonl
    lock = threading.Lock()

    class Handler(socketserver.StreamRequestHandler):
        def handle(self):
            self.connection.settimeout(20)
            line = self.rfile.readline(8193)
            parts = line.decode("ascii", errors="replace").strip().split()
            host = ""
            permitted = False
            if len(parts) == 3 and parts[0] == "CONNECT" and parts[2] in {"HTTP/1.0", "HTTP/1.1"}:
                host, _, target_port = parts[1].rpartition(":")
                permitted = host in CLAUDE_PROXY_HOSTS and target_port == "443"
            # No request body, forwarding headers or alternate HTTP proxy methods.
            total = len(line)
            while True:
                header = self.rfile.readline(8193)
                total += len(header)
                if not header or header in {b"\r\n", b"\n"} or total > 16384:
                    break
            permitted = permitted and total <= 16384
            with lock:
                append_jsonl(log, {"host": host, "permitted": permitted, "method": parts[0] if parts else None})
            if not permitted:
                self.wfile.write(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
                return
            try:
                with socket.create_connection((host, 443), timeout=20) as upstream:
                    self.wfile.write(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    self.wfile.flush()
                    while True:
                        readable, _, _ = select.select([self.connection, upstream], [], [], 300)
                        if not readable:
                            return
                        for stream in readable:
                            data = stream.recv(65536)
                            if not data:
                                return
                            (upstream if stream is self.connection else self.connection).sendall(data)
            except OSError:
                return

    class Server(socketserver.ThreadingTCPServer):
        # Sequential CLI probes reuse this leased port after CONNECT leaves TIME_WAIT.
        # SO_REUSEADDR does not permit taking over an active listener (no SO_REUSEPORT).
        allow_reuse_address = True
        daemon_threads = True
    with Server(("127.0.0.1", port), Handler) as server:
        print("ready", flush=True)
        server.serve_forever()


def _claude_launch(config_path: Path, command: list[str]) -> int:
    import signal
    import selectors
    config = read_json(config_path)
    profile = config["profile"]
    # The runner owns this process group. Proxy and sandboxed CLI inherit it; no detached child.
    proxy = subprocess.Popen([sys.executable, "-m", "benchmark.isolation", "proxy", str(profile["port"]),
                              config["proxy_log"]], stdout=subprocess.PIPE, cwd=Path(__file__).resolve().parent.parent)
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(proxy.stdout, selectors.EVENT_READ)
            if not selector.select(10) or proxy.stdout.readline() != b"ready\n":
                print("Claude allowlist proxy 시작 실패", file=sys.stderr)
                return 125  # Distinguish a boundary launcher failure from a denied command.
        env = {**os.environ, **profile["env"]}
        child = subprocess.Popen(wrap(command, profile), env=env)
        def stop(signum, frame):
            child.send_signal(signum)
            proxy.terminate()
        signal.signal(signal.SIGTERM, stop)
        signal.signal(signal.SIGINT, stop)
        return child.wait()
    finally:
        proxy.terminate()
        try:
            proxy.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proxy.kill()
            proxy.wait()
        proxy.stdout.close()


if __name__ == "__main__":
    if sys.argv[1] == "proxy":
        _serve_proxy(int(sys.argv[2]), Path(sys.argv[3]))
    elif sys.argv[1] == "claude-launch":
        sys.exit(_claude_launch(Path(sys.argv[2]), sys.argv[3:]))
    else:
        raise SystemExit("unknown isolation operation")
