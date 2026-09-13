#!/usr/bin/env python3
"""Validate pinned programming-language repositories without running their code.

The corpus is fully indexed. Twenty deterministic authored-file samples per repo
are compared with disk and independent parsers where available. Source probes
exercise negative call attribution and exclusions separately from real samples.
"""

from __future__ import annotations

import argparse
import base64
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

from public_validation import APP, DATA, Checks, McpClient, command, save_json, sha256

MANIFEST = DATA / "development-languages.json"
CTAGS = {"java": "Java", "csharp": "C#", "php": "PHP", "ruby": "Ruby", "lua": "Lua",
    "kotlin": "Kotlin", "powershell": "PowerShell", "c": "C", "cpp": "C++",
    "asm": "Asm", "sql": "SQL", "bash": "Sh", "zsh": "Zsh"}
CALLS_DISABLED = {"sql", "bash", "zsh"}
COMPONENTS = {"vue", "astro", "svelte"}
CALLABLE_KINDS = {"function", "method", "prototype", "procedure", "subroutine"}
# Only fixtures with a language-level constant/immutable binding. Uppercase
# Python, Lua, PowerShell and shell variables are not constant declarations.
CONSTANT_PROBES = {"typescript", "javascript", "java", "csharp", "groovy",
    "php", "ruby", "swift", "kotlin", "scala", "dart", "c", "cpp"}


def probe_source(language):
    """Small syntax-valid examples, not project builds or runtime tests."""
    if language in {"typescript", "javascript"}:
        argument = "receiver: any" if language == "typescript" else "receiver"
        return ("export const CM_VALIDATION_LIMIT = 7;\n"
            "export function cm_validation_target() { return CM_VALIDATION_LIMIT; }\n"
            "export function cm_validation_caller() { return cm_validation_target(); }\n"
            'export function cm_validation_string() { return "cm_validation_target()"; }\n'
            "export function cm_validation_comment() { /* cm_validation_target(); */ return 0; }\n"
            f"export function cm_validation_unknown({argument}) {{ return receiver.cm_validation_target(); }}\n")
    if language == "python":
        return ('CM_VALIDATION_LIMIT = 7\ndef cm_validation_target():\n    return CM_VALIDATION_LIMIT\n'
            'def cm_validation_caller():\n    return cm_validation_target()\n'
            'def cm_validation_string():\n    return "cm_validation_target()"\n'
            'def cm_validation_comment():\n    # cm_validation_target()\n    return 0\n'
            'def cm_validation_unknown(receiver):\n    return receiver.cm_validation_target()\n')
    if language in {"java", "csharp", "groovy"}:
        value_type = "def" if language == "groovy" else "int"
        receiver_type = "def" if language == "groovy" else ("dynamic" if language == "csharp" else "External")
        return ("class CmValidationProbe {\n"
            "    static final int CM_VALIDATION_LIMIT = 7;\n".replace("static final", "const" if language == "csharp" else "static final")
            + f"    static {value_type} cm_validation_target() {{ return CM_VALIDATION_LIMIT; }}\n"
            + f"    static {value_type} cm_validation_caller() {{ return cm_validation_target(); }}\n"
            + '    static String cm_validation_string() { return "cm_validation_target()"; }\n'.replace("String", "string" if language == "csharp" else "String")
            + f"    static {value_type} cm_validation_comment() {{ /* cm_validation_target(); */ return 0; }}\n"
            + f"    static {value_type} cm_validation_unknown({receiver_type} receiver) {{ return receiver.cm_validation_target(); }}\n"
            + "}\n")
    if language == "php":
        return ('<?php\nconst CM_VALIDATION_LIMIT = 7;\n'
            'function cm_validation_target() { return CM_VALIDATION_LIMIT; }\n'
            'function cm_validation_caller() { return cm_validation_target(); }\n'
            'function cm_validation_string() { return "cm_validation_target()"; }\n'
            'function cm_validation_comment() { /* cm_validation_target(); */ return 0; }\n'
            'function cm_validation_unknown($receiver) { return $receiver->cm_validation_target(); }\n')
    if language == "ruby":
        return ('CM_VALIDATION_LIMIT = 7\ndef cm_validation_target()\n  CM_VALIDATION_LIMIT\nend\n'
            'def cm_validation_caller()\n  cm_validation_target()\nend\n'
            'def cm_validation_string()\n  "cm_validation_target()"\nend\n'
            'def cm_validation_comment()\n  # cm_validation_target()\n  0\nend\n'
            'def cm_validation_unknown(receiver)\n  receiver.cm_validation_target()\nend\n')
    if language == "lua":
        return ('local CM_VALIDATION_LIMIT = 7\n'
            'function cm_validation_target() return CM_VALIDATION_LIMIT end\n'
            'function cm_validation_caller() return cm_validation_target() end\n'
            'function cm_validation_string() return "cm_validation_target()" end\n'
            'function cm_validation_comment()\n  -- cm_validation_target()\n  return 0\nend\n'
            'function cm_validation_unknown(receiver) return receiver.cm_validation_target() end\n')
    if language in {"swift", "kotlin", "scala", "dart"}:
        if language == "swift":
            constant, signature, argument = "let CM_VALIDATION_LIMIT = 7", "func NAME() -> Int", "func cm_validation_unknown(_ receiver: External) -> Int"
            text_signature = "func cm_validation_string() -> String"
        elif language == "kotlin":
            constant, signature, argument = "const val CM_VALIDATION_LIMIT = 7", "fun NAME(): Int", "fun cm_validation_unknown(receiver: External): Int"
            text_signature = "fun cm_validation_string(): String"
        elif language == "scala":
            constant, signature, argument = "val CM_VALIDATION_LIMIT = 7", "def NAME(): Int =", "def cm_validation_unknown(receiver: External): Int ="
            text_signature = "def cm_validation_string(): String ="
        else:
            constant, signature, argument = "const CM_VALIDATION_LIMIT = 7;", "int NAME()", "int cm_validation_unknown(dynamic receiver)"
            text_signature = "String cm_validation_string()"
        return (constant + "\n" + signature.replace("NAME", "cm_validation_target") + " { return CM_VALIDATION_LIMIT; }\n"
            + signature.replace("NAME", "cm_validation_caller") + " { return cm_validation_target(); }\n"
            + text_signature + ' { return "cm_validation_target()"; }\n'
            + signature.replace("NAME", "cm_validation_comment") + " { /* cm_validation_target(); */ return 0; }\n"
            + argument + " { return receiver.cm_validation_target(); }\n")
    if language == "powershell":
        return ('$CM_VALIDATION_LIMIT = 7\nfunction cm_validation_target { return $CM_VALIDATION_LIMIT }\n'
            'function cm_validation_caller { cm_validation_target }\n'
            'function cm_validation_string { return "cm_validation_target()" }\n'
            'function cm_validation_comment {\n  # cm_validation_target()\n  return 0\n}\n'
            'function cm_validation_unknown($receiver) { $receiver.cm_validation_target() }\n')
    if language in {"c", "cpp"}:
        return ('const int CM_VALIDATION_LIMIT = 7;\nint cm_validation_target(void) { return CM_VALIDATION_LIMIT; }\n'
            'int cm_validation_caller(void) { return cm_validation_target(); }\n'
            'const char *cm_validation_string(void) { return "cm_validation_target()"; }\n'
            'int cm_validation_comment(void) { /* cm_validation_target(); */ return 0; }\n'
            'int cm_validation_unknown(int (*cm_validation_target)(void)) { return cm_validation_target(); }\n')
    if language in {"bash", "zsh"}:
        return ('CM_VALIDATION_LIMIT=7\ncm_validation_target() { echo "$CM_VALIDATION_LIMIT"; }\n'
            'cm_validation_caller() { cm_validation_target; }\n'
            'cm_validation_string() { echo "cm_validation_target()"; }\n')
    if language == "asm":
        return ('cm_validation_target:\n    ret\ncm_validation_caller:\n    call cm_validation_target\n'
            '    ret\n; cm_validation_target()\n')
    if language == "sql":
        return ('CREATE TABLE cm_validation_target (id INTEGER PRIMARY KEY);\n'
            'CREATE VIEW cm_validation_caller AS SELECT id FROM cm_validation_target;\n'
            "SELECT 'cm_validation_target()';\n-- cm_validation_target()\n")
    if language in COMPONENTS:
        return ('<script>\nfunction cm_validation_target() { return 7; }\n'
            'function cm_validation_caller() { return cm_validation_target(); }\n</script>\n'
            '<div id="cm_validation_marker">cm_validation_target()</div>\n')
    raise ValueError(f"No probe for {language}")


def create_probe(root, language, extension):
    directory = root / "_codemap_language_probe"
    directory.mkdir(exist_ok=False)
    source = probe_source(language)
    name = f"probe.{extension}"
    (directory / name).write_text(source)
    for part in ("node_modules", "_validation_excluded", "ignored"):
        (directory / part).mkdir()
        (directory / part / name).write_text(source)
    (directory / ".gitignore").write_text("ignored/\n")
    return (directory / name).relative_to(root).as_posix()


def sample_files(root, qualification, slug, count):
    candidates = [item["name"].removeprefix("./") for item in qualification["reports"]
        if item["stats"]["code"] > 0]
    candidates = [path for path in candidates if (root / path).stat().st_size <= 1_048_576]
    ignored = subprocess.run(["git", "check-ignore", "--no-index", "-z", "--stdin"], cwd=root,
        input="\0".join(candidates) + "\0", text=True, capture_output=True)
    if ignored.returncode not in (0, 1):
        raise RuntimeError(ignored.stderr)
    candidates = set(candidates) - set(ignored.stdout.split("\0"))
    return sorted(candidates, key=lambda path: hashlib.sha256(f"codemap-languages-v1:{slug}:{path}".encode()).digest())[:count]


def independent_oracle(path, language, oracle_root=None):
    if language == "python":
        return json.loads(command([sys.executable, str(DATA / "python_oracle.py"), str(path)]))
    if language in {"typescript", "javascript"}:
        return json.loads(command(["node", str(DATA / "typescript_oracle.cjs"), str(path)]))
    if language == "swift" or language in COMPONENTS:
        oracle = "swift_oracle.py" if language == "swift" else "component_oracle.py"
        return json.loads(command([sys.executable, str(DATA / oracle), str(path)], stderr=subprocess.PIPE))
    if language in {"dart", "scala", "groovy"} and oracle_root:
        if language == "dart":
            return {**json.loads(command([str(oracle_root / "dart-oracle"), str(path)], stderr=subprocess.PIPE)), "file_sha256": sha256(path)}
        manifest = json.loads((oracle_root / "manifest.json").read_text())
        helper = "ScalaOracle" if language == "scala" else "GroovyOracle"
        lines = command(["java", "-cp", manifest["classpath"], helper, str(path)], stderr=subprocess.PIPE).splitlines()
        name, version = lines[0].split("\t")
        declarations = []
        for line in lines[1:]:
            encoded, start, name_line, end = line.split("\t")
            declarations.append({"name": base64.b64decode(encoded).decode(), "kind": "fn",
                "start": int(start), "name_line": int(name_line), "end": int(end)})
        return {"oracle": name, "version": version, "declarations": declarations, "file_sha256": sha256(path)}
    if language in CTAGS:
        raw = command(["ctags", "--options=NONE", f"--language-force={CTAGS[language]}", "--output-format=json",
            "--fields=+neK", "--extras=-p", "-o", "-", str(path.resolve())], stderr=subprocess.PIPE)
        tags = [json.loads(line) for line in raw.splitlines() if line]
        declarations = [{"name": item["name"], "name_line": item["line"], "start": item["line"],
            "end": item.get("end"), "kind": item["kind"], "scope": item.get("scope"), "scopeKind": item.get("scopeKind")}
            for item in tags if item.get("kind") in (CALLABLE_KINDS | ({"label"} if language == "asm" else set())) and item.get("scopeKind") not in {"function", "method"}]
        return {"oracle": "Universal Ctags", "declarations": declarations, "raw_tags": tags,
            "file_sha256": sha256(path), "scope": "Named callables detected by Ctags; not a compiler or complete semantic oracle"}
    return {"oracle": None, "declarations": [], "file_sha256": sha256(path),
        "limitation": "No independent declaration parser configured; disk/range consistency and explicit probe declarations are checked"}


def validate_sample(checks, path, language, output, has_live_checks, oracle_root=None):
    data = (checks.root / path).read_bytes()
    # read's documented contract strips a leading UTF-8 BOM before line 1.
    source = data.decode("utf-8", errors="replace").removeprefix("\ufeff")
    lines = source.splitlines()
    text, elapsed, error = checks.client.tool("overview", {"path": path})
    actual = [(name, kind, int(start), int(end)) for name, kind, start, end in
        re.findall(r"^- (.+?) \(([^)]+)\) \[L(\d+)-(\d+)\]", text, re.M)]
    invalid = [item for item in actual if not 1 <= item[2] <= item[3] <= len(lines)
        or item[0].strip('"\'`') not in "\n".join(lines[item[2]-1:item[3]])]
    checks.record(f"overview-source:{path}", "source_consistency", not error and not invalid
        and text.startswith(f"# Detailed Codemap: {path} "),
        {"file": path, "symbols": len(actual), "invalid": invalid, "protocol_error": error}, elapsed)
    try:
        oracle = independent_oracle(checks.root / path, language, oracle_root)
    except (subprocess.CalledProcessError, ValueError) as failure:
        oracle = {"oracle": "parse failed", "declarations": [], "error": str(failure), "file_sha256": sha256(checks.root / path)}
    save_json(output / "oracle" / (hashlib.sha256(path.encode()).hexdigest()[:16] + ".json"), {"file": path, **oracle})
    expected = oracle["declarations"]
    missing, end_mismatch = [], []
    for item in expected:
        matches = [row for row in actual if row[1] == "fn" and row[0] == item["name"] and row[2] <= item["name_line"] <= row[3]]
        if not matches:
            missing.append(item)
        elif item.get("end") is not None and not any(row[3] == item["end"] for row in matches):
            end_mismatch.append({"expected": item, "actual": matches})
    has_oracle = bool(oracle.get("oracle")) and "error" not in oracle
    has_comparable_symbols = bool(expected) or not any(row[1] == "fn" for row in actual)
    status = None if has_oracle and has_comparable_symbols and not missing and not end_mismatch else "unverified"
    checks.record(f"overview-oracle:{path}", "declaration_comparison", not error and not missing and not end_mismatch,
        {"file": path, "oracle": oracle.get("oracle"), "expected_count": len(expected), "missing": missing,
            "end_positions_available": sum(item.get("end") is not None for item in expected),
            "end_mismatch": end_mismatch, "note": oracle.get("limitation", oracle.get("error")),
            "coverage": "Independent named-callable recall and end-line check; extra product symbols checked against disk separately"}, status=status)
    if not has_live_checks or not lines:
        return
    first = expected[0]["name_line"] if expected else 1
    limit = min(8, len(lines) - first + 1)
    text, elapsed, error = checks.client.tool("read", {"file_path": path, "offset": first, "limit": limit})
    actual_lines = [(int(line), content) for line, content in re.findall(r"^\s*(\d+)→(.*)$", text.split("\n# results\n", 1)[-1], re.M)]
    expected_lines = list(enumerate(lines[first - 1:first - 1 + limit], first))
    checks.record(f"read:{path}", "live_content", not error and actual_lines == expected_lines,
        {"file": path, "offset": first, "limit": limit, "matches_source": actual_lines == expected_lines}, elapsed)
    token = expected[0]["name"] if expected else next(iter(re.findall(r"\b[A-Za-z_][A-Za-z_0-9]{5,}\b", source)), None)
    if not token:
        return
    occurrences = [index for index, line in enumerate(lines, 1) if token in line]
    text, elapsed, error = checks.client.tool("grep", {"path": path, "pattern": re.escape(token), "head_limit": 1000})
    positions = [int(value) for value in re.findall(r"^" + re.escape(path) + r":(\d+):", text.split("\n# results\n", 1)[-1], re.M)]
    checks.record(f"grep:{path}", "live_search", not error and positions == occurrences,
        {"file": path, "token": token, "expected": occurrences, "actual": positions, "protocol_error": error}, elapsed)
    checks.query(f"find:{path}", "find", {"path": str(Path(path).parent), "pattern": Path(path).name}, contains=[path], category="file_enumeration")
    if expected:
        checks.query(f"search:{path}", "search", {"query": f"{token} {Path(path).stem}", "workspace_scope": "all", "caller_context": False},
            contains=[path], category="search_discovery")


def validate_probe(checks, path, language):
    source = (checks.root / path).read_text().splitlines()
    if language in COMPONENTS:
        checks.query("probe:component-declaration", "overview", {"path": path}, contains=["cm_validation_marker"], category="probe_declaration")
    else:
        checks.query("probe:declarations", "overview", {"path": path}, contains=["cm_validation_target", "cm_validation_caller"], category="probe_declaration")
    read_args = {"file_path": path, "offset": 1, "limit": min(30, len(source))}
    if language in COMPONENTS:
        checks.query("probe:component-code-caller", "read", {"file_path": path, "offset": 2, "limit": 1},
            contains=[f"cm_validation_caller ({path}:3)"], excludes=[f"{path}:5 (top-level/unindexed)"],
            category="component_code_boundary", section=(path, 2))
        checks.query("probe:component-code-callee", "read", {"file_path": path, "offset": 3, "limit": 1},
            contains=[f"cm_validation_target — {path}:2"], category="callee_attribution", section=(path, 3))
    elif language in CALLS_DISABLED:
        checks.query("probe:unsupported-call-boundary", "read", read_args, excludes=["_calls (", "_callers (", "(precise)"],
            category="capability_boundary", section="symbols")
    elif language == "asm":
        checks.query("probe:asm-instruction-caller", "read", {"file_path": path, "offset": 1, "limit": 1},
            contains=[f"{path}:4 (top-level/unindexed)"], category="caller_attribution", section=(path, 1))
    else:
        def declaration_line(name):
            return next(i for i, line in enumerate(source, 1) if re.search(r"(?:function|def|fun|func|int|static|String|string|char).*\b" + name + r"\b", line)
                or line.startswith(f"function {name}"))
        target, caller, unknown = (declaration_line(name) for name in ("cm_validation_target", "cm_validation_caller", "cm_validation_unknown"))
        checks.query("probe:actual-caller", "read", {"file_path": path, "offset": target, "limit": 1}, contains=["cm_validation_caller ("],
            excludes=["cm_validation_string (", "cm_validation_comment (", "cm_validation_unknown ("], category="caller_attribution", section=(path, target))
        checks.query("probe:actual-callee", "read", {"file_path": path, "offset": caller, "limit": 1}, contains=[f"cm_validation_target — {path}:{target}"],
            category="callee_attribution", section=(path, caller))
        checks.query("probe:unknown-call", "read", {"file_path": path, "offset": unknown, "limit": 1}, contains=["cm_validation_target (unresolved)"],
            excludes=[f"cm_validation_target — {path}:{target}", "(precise)"], category="resolution_policy", section=(path, unknown))
        if language in CONSTANT_PROBES:
            constant_line = next(i for i, line in enumerate(source, 1) if "CM_VALIDATION_LIMIT" in line)
            checks.query("probe:constant-context", "read", {"file_path": path, "offset": target, "limit": 1},
                contains=[f"CM_VALIDATION_LIMIT — {path}:{constant_line} = 7"],
                category="constant_context", section=(path, target))
    for part in ("node_modules", "_validation_excluded", "ignored"):
        excluded = str(Path(path).parent / part / Path(path).name)
        checks.query(f"exclude:{part}", "grep", {"path": excluded, "pattern": "cm_validation_target", "output_mode": "files_with_matches"},
            excludes=[excluded], category="exclusion")
        checks.query(f"exclude:{part}:bypass", "grep", {"path": excluded, "pattern": "cm_validation_target", "output_mode": "files_with_matches", "include_ignored": True},
            contains=[excluded], category="exclusion")
    checks.query("exclude:search", "search", {"query": "cm_validation_target", "workspace_scope": "all", "caller_context": False},
        excludes=[str(Path(path).parent / part) + "/" for part in ("node_modules", "_validation_excluded", "ignored")], category="exclusion")


def run_repository(args, language, slug, manifest, output):
    label = language["language"] + "-" + slug.replace("/", "__")
    destination = output / label
    destination.mkdir()
    qualification_path = args.cache / "language-qualification" / slug.replace("/", "__") / (language["language"] + ".json")
    qualification = json.loads(qualification_path.read_text())
    if qualification["sha"] != manifest["revisions"][slug] or qualification["commits"] < manifest["minimum_commits"] or qualification["unmeasured_files"]:
        raise RuntimeError(f"Invalid qualification: {label}")
    if not qualification["qualified"] and not manifest["size_exception_policy"]["approved"]:
        raise RuntimeError(f"Unapproved size exception: {label}")
    clone = args.cache / "language-repos" / slug.replace("/", "__")
    root = destination / "worktree"
    if args.previous_run:
        previous = args.cache / "runs" / args.previous_run / label
        previous_summary = json.loads((previous.parent / "summary.json").read_text())
        for profile in args.profile or ("default", "structural"):
            if not (previous / profile / "results.json").is_file() and not previous_summary.get("finished_utc"):
                raise RuntimeError(f"Previous profile has not completed: {previous / profile}")
        if (previous / "worktree").is_dir():
            root = previous / "worktree"
            if command(["git", "rev-parse", "HEAD"], root) != qualification["sha"]:
                raise RuntimeError(f"Previous worktree revision changed: {root}")
    is_reused = root.is_dir()
    if not is_reused:
        subprocess.run(["git", "worktree", "add", "--detach", "--quiet", str(root), qualification["sha"]], cwd=clone, check=True, capture_output=True)
    if command(["git", "status", "--porcelain", "--untracked-files=no"], root):
        raise RuntimeError(f"Checkout modified tracked source: {label}")
    shutil.copy(qualification_path, destination / "qualification.json")
    files = sample_files(root, qualification, slug, args.samples)
    if not files:
        raise RuntimeError(f"No eligible sample files available: {label}")
    if (root / "_codemap_language_probe").is_dir():
        probe = f"_codemap_language_probe/probe.{language['extensions'][0]}"
        if (root / probe).read_text() != probe_source(language["language"]):
            raise RuntimeError(f"Previous probe changed: {root / probe}")
    else:
        probe = create_probe(root, language["language"], language["extensions"][0])
    records = []
    for profile in args.profile or ("default", "structural"):
        profile_dir = destination / profile
        profile_dir.mkdir()
        config = (DATA / "config.toml").read_text().replace('index_path = ".codemap/index"', f'index_path = ".codemap/index-{profile}"')
        config = config.replace("is_shell_support_enabled = false", "is_shell_support_enabled = true")
        if profile == "structural":
            config = config.replace("navigation_context_default = false", "navigation_context_default = true").replace("navigation_store_references = false", "navigation_store_references = true")
        config_path = root / ".codemap/config.toml"
        config_path.parent.mkdir(exist_ok=True)
        config_path.write_text(config)
        shutil.copy(config_path, profile_dir / "config.toml")
        ready, checks = None, None
        try:
            with McpClient(args.binary, root, profile_dir, timeout=180) as client:
                checks = Checks(client, root)
                ready = client.ready(probe, timeout=1200)
                for index, path in enumerate(files):
                    validate_sample(checks, path, language["language"], profile_dir, index < 3, args.cache / "oracles")
                validate_probe(checks, probe, language["language"])
        except Exception as error:
            # Keep completed checks even when a later request/initial index fails.
            # A partial file must never authorize reuse of a still-running profile.
            save_json(profile_dir / "partial-results.json", {
                "language": language["language"], "repository": slug, "profile": profile,
                "sha": qualification["sha"], "config_sha256": sha256(config_path),
                "worktree": str(root), "ready_seconds": ready, "sample_files": files,
                "checks": checks.results if checks else [],
                "execution_error": f"{type(error).__name__}: {error}"})
            raise
        record = {"language": language["language"], "repository": slug, "sha": qualification["sha"],
            "code_lines": qualification["code_lines"], "commits": qualification["commits"], "size_exception": not qualification["qualified"],
            "profile": profile, "worktree": str(root), "worktree_reused": is_reused, "config_sha256": sha256(config_path), "ready_seconds": ready,
            "sample_files": files, "sample_shortfall": max(0, args.samples - len(files)), "checks": checks.results, "tracked_changes": command(["git", "status", "--porcelain", "--untracked-files=no"], root)}
        record["counts"] = {status: sum(check["status"] == status for check in checks.results) for status in ("pass", "fail", "unverified")}
        save_json(profile_dir / "results.json", record)
        records.append(record)
        print(label, profile, record["counts"], flush=True)
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", required=True, type=Path)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--previous-run", help="Reuse completed worktrees/indexes; keep new transcripts and re-run checks")
    parser.add_argument("--language", action="append")
    parser.add_argument("--repository", action="append")
    parser.add_argument("--profile", action="append", choices=("default", "structural"))
    parser.add_argument("--samples", type=int, default=20)
    parser.add_argument("--jobs", type=int, default=2, choices=range(1, 5))
    args = parser.parse_args()
    if any(not re.fullmatch(r"[A-Za-z0-9_-]+", name) for name in (args.run_id, args.previous_run or "none")) or args.samples < 1:
        parser.error("Invalid run ID or sample count")
    args.cache, args.binary = args.cache.expanduser().resolve(), args.binary.expanduser().resolve()
    manifest = json.loads(MANIFEST.read_text())
    selected = [(language, slug) for language in manifest["languages"] if language["language"] not in {"rust", "go"}
        and (not args.language or language["language"] in args.language)
        for slug in language.get("selected_candidates", language["candidates"])
        if not args.repository or slug in args.repository]
    if not selected:
        parser.error("No selected repositories; Rust/Go use public_validation.py")
    output = args.cache / "runs" / args.run_id
    output.mkdir(parents=True, exist_ok=False)
    oracle_manifest = args.cache / "oracles/manifest.json"
    if any(language["language"] in {"dart", "scala", "groovy"} for language, _ in selected):
        native = json.loads(oracle_manifest.read_text())
        for key, path in (("dart_source_sha256", DATA / "dart_oracle/main.dart"), ("scala_source_sha256", DATA / "ScalaOracle.scala"),
            ("groovy_source_sha256", DATA / "GroovyOracle.java"), ("dart_binary_sha256", args.cache / "oracles/dart-oracle")):
            if sha256(path) != native[key]:
                raise RuntimeError(f"Prepared oracle changed; run prepare_validation_oracles.py: {path}")
        for artifact in native["artifacts"]:
            if sha256(Path(artifact["path"])) != artifact["sha256"]:
                raise RuntimeError(f"Oracle artifact changed: {artifact['path']}")
        shutil.copy(oracle_manifest, output / "native-oracles.json")
    shutil.copy(MANIFEST, output / MANIFEST.name)
    shutil.copy(args.binary, output / "tested-codemap-search")
    args.binary = output / "tested-codemap-search"
    scripts = [Path(__file__), APP / "scripts/public_validation.py", DATA / "python_oracle.py", DATA / "typescript_oracle.cjs", DATA / "swift_oracle.py", DATA / "component_oracle.py"]
    for script in scripts:
        shutil.copy(script, output / script.name)
    shutil.copytree(DATA / "dart_oracle", output / "dart_oracle", ignore=shutil.ignore_patterns(".dart_tool"))
    for path in (DATA / "GroovyOracle.java", DATA / "ScalaOracle.scala"):
        shutil.copy(path, output / path.name)
    summary = {"run_id": args.run_id, "started_utc": datetime.now(timezone.utc).isoformat(),
        "binary_sha256": sha256(args.binary), "manifest_sha256": sha256(MANIFEST), "samples_per_repository": args.samples,
        "source_commit": command(["git", "rev-parse", "HEAD"], APP), "runner_sha256": sha256(Path(__file__)), "previous_run": args.previous_run,
        "source_sha256": {path.relative_to(APP).as_posix(): sha256(path) for directory in (APP / "src", APP / "queries") for path in sorted(directory.rglob("*")) if path.is_file()},
        "versions": {"python": sys.version, **{name: command([binary, "--version"]) if shutil.which(binary) else "unavailable"
            for name, binary in (("ctags", "ctags"), ("node", "node"), ("swift", "swiftc"))}},
        "results": [], "errors": []}
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        futures = {pool.submit(run_repository, args, language, slug, manifest, output): (language["language"], slug) for language, slug in selected}
        for future in as_completed(futures):
            try:
                summary["results"].extend(future.result())
            except Exception as error:
                record = {"language": futures[future][0], "repository": futures[future][1], "error": f"{type(error).__name__}: {error}"}
                summary["errors"].append(record)
                print(json.dumps(record), flush=True)
            save_json(output / "summary.json", summary)
    summary["finished_utc"] = datetime.now(timezone.utc).isoformat()
    save_json(output / "summary.json", summary)
    print(output / "summary.json", flush=True)
    return int(bool(summary["errors"]) or any(row["counts"]["fail"] or row["counts"]["unverified"] for row in summary["results"]))


if __name__ == "__main__":
    raise SystemExit(main())
