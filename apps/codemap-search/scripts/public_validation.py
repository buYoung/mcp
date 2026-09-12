#!/usr/bin/env python3
"""Pinned public-repository validation. Uses only the Python standard library."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import shutil
import subprocess
import sys
import threading
import time
from datetime import datetime, timezone


APP = Path(__file__).resolve().parents[1]
DATA = APP / "validation"
FIXTURES = APP / "tests/fixtures/public_validation"
EXCLUDED_PARTS = {"vendor", "vendored", "node_modules", "target", "third_party", "third-party", "dist", "build"}


def command(args, cwd=None, **kwargs):
    return subprocess.check_output(args, cwd=cwd, text=True, **kwargs).strip()


def save_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tracked_files(root):
    return sorted(p for p in command(["git", "ls-files", "-z"], root).split("\0") if p)


def qualify(root, spec):
    """Measure Git-tracked target-language source independently of codemap-search."""
    if command(["git", "rev-parse", "--is-shallow-repository"], root) != "false":
        raise RuntimeError("Qualification needs full history, not a shallow clone")
    if command(["git", "rev-parse", "HEAD"], root) != spec["sha"]:
        raise RuntimeError(f"Unexpected HEAD: {root}")
    excluded, paths = {}, []
    extension = ".rs" if spec["language"] == "Rust" else ".go"
    for path in tracked_files(root):
        if not path.endswith(extension):
            continue
        file = root / path
        if file.is_symlink():
            excluded[path] = "symlink"
            continue
        if set(Path(path).parts[:-1]) & EXCLUDED_PARTS:
            excluded[path] = "dependency/build directory"
            continue
        header = file.read_bytes()[:4096].decode("utf-8", errors="replace").lower()
        if path.endswith((".pb.go", ".gen.go", "_generated.go")) or (
            any(marker in header for marker in ("code generated", "automatically generated", "@generated"))
            and ("do not edit" in header or "@generated" in header)
        ):
            excluded[path] = "generated filename/header"
            continue
        paths.append(path)
    reports = []
    for offset in range(0, len(paths), 200):
        result = json.loads(command([
            "tokei", "--files", "--output", "json", "--types", spec["language"],
            "--no-ignore", "--hidden", *paths[offset:offset + 200],
        ], root))
        reports.extend(result.get(spec["language"], {}).get("reports", []))
    if len(reports) != len(paths):
        raise RuntimeError(f"tokei measured {len(reports)}/{len(paths)} files")
    total = sum(item["stats"]["code"] for item in reports)
    commits = int(command(["git", "rev-list", "--count", "HEAD"], root))
    return {
        "sha": spec["sha"], "url": spec["url"], "language": spec["language"],
        "commits": commits, "code_lines": total, "files": len(paths),
        "qualified": total >= 200_000 and commits >= 1000,
        "tokei_version": command(["tokei", "--version"]),
        "method": "tracked target-language files; comments/blanks, dependency/build directories, explicit generated headers/names and symlinks excluded; authored tests included",
        "excluded": excluded, "reports": reports,
    }


class McpClient:
    """Sequential newline-JSON MCP client with bounded waits and raw transcripts."""

    def __init__(self, binary, root, output, timeout=120):
        self.started = time.monotonic()
        output.mkdir(parents=True, exist_ok=True)
        self.log = (output / "mcp.jsonl").open("a")
        self.stderr = (output / "server.stderr.log").open("a")
        self.process = subprocess.Popen(
            [str(binary), "mcp"], cwd=root, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=self.stderr, text=True, bufsize=1,
        )
        self.responses = queue.Queue()
        self.timeout, self.request_id = timeout, 0
        threading.Thread(target=self._read, daemon=True).start()
        try:
            response, _ = self.request("initialize", {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": {"name": "public-repository-validation", "version": "1"},
            })
            if "error" in response:
                raise RuntimeError(f"MCP initialization rejected: {response['error']}")
            self.process.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
            self.process.stdin.flush()
            self.tool("initial_instructions", {})
        except BaseException:
            self.close()
            raise

    def _read(self):
        try:
            for line in self.process.stdout:
                self.responses.put(json.loads(line))
        except Exception as error:
            self.responses.put(error)
        finally:
            self.responses.put(EOFError("MCP stdout closed"))

    def request(self, method, params):
        self.request_id += 1
        request = {"jsonrpc": "2.0", "id": self.request_id, "method": method, "params": params}
        started = time.monotonic()
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        deadline = started + self.timeout
        while True:
            response = self.responses.get(timeout=max(0.001, deadline - time.monotonic()))
            if isinstance(response, Exception):
                raise response
            if response.get("id") == self.request_id:
                break
        elapsed = round((time.monotonic() - started) * 1000, 3)
        self.log.write(json.dumps({"request": request, "response": response, "elapsed_ms": elapsed}, ensure_ascii=False) + "\n")
        self.log.flush()
        return response, elapsed

    def tool(self, name, arguments):
        response, elapsed = self.request("tools/call", {"name": name, "arguments": arguments})
        if "error" in response:
            return json.dumps(response["error"], ensure_ascii=False), elapsed, True
        result = response.get("result", {})
        text = "\n".join(item.get("text", "") for item in result.get("content", []) if item.get("type") == "text")
        return text, elapsed, bool(result.get("isError"))

    def ready(self, path, timeout=300):
        started = time.monotonic()
        while time.monotonic() - started < timeout:
            text, _, error = self.tool("overview", {"path": path})
            if not error and text.startswith(f"# Detailed Codemap: {path} ") and "## Symbols" in text:
                return round(time.monotonic() - self.started, 3)
            time.sleep(0.5)
        raise TimeoutError(f"Index did not become ready for {path}: {text[:400]}")

    def close(self):
        if self.process.poll() is None:
            try:
                self.process.stdin.close()
            except BrokenPipeError:
                pass
            try:
                self.process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.process.terminate()
                try:
                    self.process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=5)
        self.log.close()
        self.stderr.close()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()


def write_config(root, profile):
    config = (DATA / "config.toml").read_text()
    if profile == "structural":
        config = config.replace("navigation_context_default = false", "navigation_context_default = true")
        config = config.replace("navigation_store_references = false", "navigation_store_references = true")
    path = root / ".codemap/config.toml"
    path.parent.mkdir(exist_ok=True)
    path.write_text(config)
    return sha256(path)


def load_specs(names):
    specs = json.loads((DATA / "repositories.json").read_text())["repositories"]
    if names:
        unknown = set(names) - {item["name"] for item in specs}
        if unknown:
            raise ValueError(f"Unknown repositories: {sorted(unknown)}")
        specs = [item for item in specs if item["name"] in names]
    return specs


def rust_oracle(path):
    # Match the documented significant-symbol view: exported definitions and
    # definitions outside function bodies. Preserve macros' actual kind in evidence;
    # codemap-search currently normalizes macro definitions to fn in its display.
    source = path.read_bytes()
    process = subprocess.run(["rust-analyzer", "parse", "--json"], input=source, capture_output=True, check=True)
    tree = json.loads(process.stdout)
    declarations, calls = [], []

    def visit(node, function_depth=0):
        children = node.get("children", [])
        if node["kind"] == "ERROR":
            raise RuntimeError(f"rust-analyzer parse error in {path}: {node['start']}")
        is_function = node["kind"] == "FN" and any(child["kind"] == "BLOCK_EXPR" for child in children)
        is_macro = node["kind"] == "MACRO_RULES"
        is_exported = any(child["kind"] == "VISIBILITY" for child in children)
        if (is_function or is_macro) and (function_depth == 0 or is_exported):
            name = next((child for child in children if child["kind"] == "NAME"), None)
            significant = [child for child in children if child["kind"] not in {"ATTR", "COMMENT", "WHITESPACE"}]
            if name and significant:
                declarations.append({
                    "name": source[name["start"][0]:name["end"][0]].decode(), "kind": "macro" if is_macro else "fn",
                    "start": significant[0]["start"][1] + 1,
                    "end": source[:node["end"][0]].rstrip(b"\n").count(b"\n") + 1,
                })
        if node["kind"] in {"CALL_EXPR", "METHOD_CALL_EXPR", "MACRO_CALL"}:
            calls.append({"text": source[node["start"][0]:node["end"][0]].decode(), "line": node["start"][1] + 1, "kind": node["kind"]})
        for child in children:
            if child["type"] == "Node":
                visit(child, function_depth + int(is_function))

    visit(tree)
    return {"declarations": declarations, "calls": calls}


def symbol_context(text, path, start):
    lines = text.split("\n# results\n", 1)[0].splitlines()
    for index, line in enumerate(lines):
        if re.search(r"\[[^\]]+\] — " + re.escape(path) + rf":{start}-\d+", line):
            indent = len(line) - len(line.lstrip())
            end = index + 1
            while end < len(lines):
                candidate = lines[end]
                if len(candidate) - len(candidate.lstrip()) <= indent and re.search(r"\[[^\]]+\] — ", candidate):
                    break
                end += 1
            return "\n".join(lines[index:end])
    return ""


class Checks:
    def __init__(self, client, root):
        self.client, self.root, self.results = client, root, []

    def record(self, name, category, passed, detail, elapsed_ms=None, status=None):
        self.results.append({"id": name, "category": category, "status": status or ("pass" if passed else "fail"), "detail": detail, "elapsed_ms": elapsed_ms})

    def query(self, name, tool, arguments, contains=(), excludes=(), category=None, section=None):
        text, elapsed, error = self.client.tool(tool, arguments)
        checked = text.split("\n# results\n", 1)[0] if section == "symbols" else text
        if isinstance(section, tuple):
            checked = symbol_context(text, *section)
        has_section = not isinstance(section, tuple) or bool(checked)
        missing = [value for value in contains if value not in checked]
        unexpected = [value for value in excludes if value in checked]
        detail = {"tool": tool, "arguments": arguments, "missing": missing, "unexpected": unexpected, "protocol_error": error, "context_found": has_section}
        status = None
        if missing and not unexpected and not error and has_section and category == "callees" and re.search(r"ambiguous|more not shown|cap", checked):
            status = "unverified"
            detail["reason"] = "Required target not displayed; output explicitly reports ambiguity or truncation"
        self.record(name, category or tool, not error and has_section and not missing and not unexpected, detail, elapsed, status)
        return text


def validate_file(checks, spec, path, oracle_binary, output, read_live=True):
    source = (checks.root / path).read_bytes()
    oracle = rust_oracle(checks.root / path) if spec["language"] == "Rust" else json.loads(command([str(oracle_binary), str(checks.root / path)]))
    save_json(output / "oracle" / (path.replace("/", "__") + ".json"), {"file_sha256": hashlib.sha256(source).hexdigest(), **oracle})
    text, elapsed, error = checks.client.tool("overview", {"path": path})
    expected = {(item["name"], item["start"], item["end"]) for item in oracle["declarations"]}
    actual = {(name, int(start), int(end)) for name, start, end in re.findall(r"^- (\S+) \(fn\) \[L(\d+)-(\d+)\]$", text, re.M)}
    checks.record(f"overview:{path}", "declaration_ranges", not error and expected == actual, {
        "file": path, "expected_count": len(expected), "actual_count": len(actual),
        "missing": sorted(expected - actual), "unexpected": sorted(actual - expected), "protocol_error": error,
        "oracle": "rust-analyzer parse --json" if spec["language"] == "Rust" else "go/parser",
    }, elapsed)
    # Check exact live contents separately from indexed annotations.
    if read_live and oracle["declarations"]:
        first = oracle["declarations"][0]
        start, limit = first["start"], min(8, first["end"] - first["start"] + 1)
        text, elapsed, error = checks.client.tool("read", {"file_path": path, "offset": start, "limit": limit})
        expected_lines = list(enumerate(source.decode().splitlines()[start - 1:start - 1 + limit], start))
        actual_lines = [(int(line), content) for line, content in re.findall(r"^\s*(\d+)→(.*)$", text.split("\n# results\n", 1)[-1], re.M)]
        checks.record(f"read:{path}", "live_content", not error and actual_lines == expected_lines, {"file": path, "offset": start, "limit": limit, "matches_source": actual_lines == expected_lines}, elapsed)
    return oracle


def sampled_files(root, spec, qualification, count=20):
    candidates = [item["name"].removeprefix("./") for item in qualification["reports"] if item["stats"]["code"] > 0]
    candidates = [path for path in candidates if path not in spec["files"] and (root / path).stat().st_size <= 1_048_576]
    ignored = subprocess.run(["git", "check-ignore", "--no-index", "-z", "--stdin"], input="\0".join(candidates) + "\0", cwd=root, capture_output=True, text=True)
    if ignored.returncode not in (0, 1):
        raise RuntimeError(ignored.stderr)
    excluded = set(ignored.stdout.split("\0"))
    ordered = sorted(set(candidates) - excluded, key=lambda path: hashlib.sha256(f"codemap-public-v1:{spec['name']}:{path}".encode()).digest())
    return ordered[:count]


def create_probes(root, language):
    directory = root / "_codemap_validation_probe"
    directory.mkdir()
    if language == "Rust":
        source = (
            "pub const CM_VALIDATION_LIMIT: usize = 7;\n"
            "pub fn cm_validation_target() -> usize { CM_VALIDATION_LIMIT }\n"
            "pub fn cm_validation_caller() -> usize { cm_validation_target() }\n"
            'pub fn cm_validation_string() -> &\'static str { "cm_validation_target()" }\n'
            "pub fn cm_validation_comment() -> usize {\n"
            "    // cm_validation_target()\n    0\n}\n"
            "#[test]\nfn cm_validation_test() { let _ = cm_validation_target(); }\n"
            "#[cm_validation_custom]\nfn cm_validation_custom_test() { let _ = cm_validation_target(); }\n"
            "mod constants;\nuse constants::CM_VALIDATION_IMPORTED;\n"
            "pub fn cm_validation_imported_constant() -> usize { CM_VALIDATION_IMPORTED }\n"
        )
        file, target, caller, string_name, comment, test_name, constant = "probe.rs", "cm_validation_target", "cm_validation_caller", "cm_validation_string", "cm_validation_comment", "cm_validation_test", "CM_VALIDATION_LIMIT"
        excluded_source = "pub fn cm_validation_excluded() {}\n"
        needle = "cm_validation_excluded"
        dynamic_before = "pub fn cm_validation_refresh_before() -> usize { 1 }\n"
        dynamic_after = "pub fn cm_validation_refresh_after() -> usize { 2 }\n"
        (directory / "constants.rs").write_text("pub const CM_VALIDATION_IMPORTED: usize = 11;\n")
    else:
        source = (
            "package validationprobe\nconst CM_VALIDATION_LIMIT = 7\n"
            "func CmValidationTarget() int { return CM_VALIDATION_LIMIT }\n"
            "func CmValidationCaller() int { return CmValidationTarget() }\n"
            'func CmValidationString() string { return "CmValidationTarget()" }\n'
            "func CmValidationComment() int {\n    // CmValidationTarget()\n    return 0\n}\n"
            "func CmValidationImportedConstant() int { return CM_VALIDATION_IMPORTED }\n"
        )
        file, target, caller, string_name, comment, test_name, constant = "probe.go", "CmValidationTarget", "CmValidationCaller", "CmValidationString", "CmValidationComment", "TestCmValidation", "CM_VALIDATION_LIMIT"
        (directory / "probe_test.go").write_text("package validationprobe\nimport \"testing\"\nfunc TestCmValidation(t *testing.T) { CmValidationTarget() }\n")
        (directory / "probe_custom.go").write_text("package validationprobe\nfunc CmValidationCustomTest() { CmValidationTarget() }\n")
        excluded_source = "package validationprobe\nfunc CmValidationExcluded() {}\n"
        needle = "CmValidationExcluded"
        dynamic_before = "package validationprobe\nfunc CmValidationRefreshBefore() int { return 1 }\n"
        dynamic_after = "package validationprobe\nfunc CmValidationRefreshAfter() int { return 2 }\n"
        (directory / "constants.go").write_text("package validationprobe\nconst CM_VALIDATION_IMPORTED = 11\n")
    (directory / file).write_text(source)
    for part in ("_validation_excluded", "node_modules", "ignored"):
        (directory / part).mkdir()
        (directory / part / file).write_text(excluded_source)
    ignore = root / ".codemapignore"
    original_ignore = ignore.read_text() if ignore.exists() else ""
    ignore.write_text(original_ignore + "\n_codemap_validation_probe/ignored/\n")
    (root / ".codemap" / file).write_text(excluded_source)
    lines = source.splitlines()
    line_of = lambda name: next(i for i, line in enumerate(lines, 1) if re.search(r"(?:fn|func) " + re.escape(name) + r"\(", line))
    return {
        "directory": directory.relative_to(root).as_posix(), "file": (directory / file).relative_to(root).as_posix(),
        "basename": file, "target": target, "target_line": line_of(target), "caller": caller, "caller_line": line_of(caller),
        "string": string_name, "comment": comment, "test": test_name, "constant": constant,
        "constant_line": 1 if language == "Rust" else 2, "needle": needle,
        "before": dynamic_before, "after": dynamic_after,
        "custom_test": "cm_validation_custom_test" if language == "Rust" else "CmValidationCustomTest",
        "imported_line": line_of("cm_validation_imported_constant" if language == "Rust" else "CmValidationImportedConstant"),
    }


def validate_probes(checks, probe, language):
    path, directory = probe["file"], probe["directory"]
    target_args = {"file_path": path, "offset": probe["target_line"], "limit": 1}
    checks.query("callers:syntax-and-tests", "read", target_args,
        contains=[f"{probe['caller']} ({path}:{probe['caller_line']})"],
        excludes=[f"{probe['string']} (", f"{probe['comment']} (", f"{probe['test']} ("], category="callers", section=(path, probe["target_line"]))
    checks.query("calls:definition-location", "read", {"file_path": path, "offset": probe["caller_line"], "limit": 1},
        contains=[f"{probe['target']} — {path}:{probe['target_line']}"], category="callees", section=(path, probe["caller_line"]))
    checks.query("references:same-file-constant", "read", target_args,
        contains=[f"{probe['constant']}", f"{path}:{probe['constant_line']}"], category="references")
    checks.query("references:cross-file-constant", "read", {"file_path": path, "offset": probe["imported_line"], "limit": 1},
        contains=[f"CM_VALIDATION_IMPORTED — {directory}/constants{Path(path).suffix}:{1 if language == 'Rust' else 2}"], category="reference_coverage", section=(path, probe["imported_line"]))
    pattern = re.escape(probe["target"]) + r"\("
    source = (checks.root / path).read_text().splitlines()
    matches = [(i, line) for i, line in enumerate(source, 1) if re.search(pattern, line)]
    for mode in ("content", "files_with_matches", "count"):
        arguments = {"path": path, "pattern": pattern, "output_mode": mode, "head_limit": 0}
        text, elapsed, error = checks.client.tool("grep", arguments)
        raw = text.split("\n# results\n", 1)[-1].strip()
        if mode == "content":
            actual = [(int(line), value) for line, value in re.findall(re.escape(path) + r":(\d+):(.*)", raw)]
            passed = actual == matches
        elif mode == "files_with_matches":
            passed = [line for line in raw.splitlines() if line.endswith(Path(path).suffix)] == [path] and "# symbols" not in text
        else:
            passed = f"{path}:{len(matches)}" in raw and "# symbols" not in text
        checks.record(f"grep:{mode}", "live_content", not error and passed, {"arguments": arguments, "expected_matches": len(matches), "protocol_error": error, "raw_output": raw if mode != "content" else None}, elapsed)
    pages = []
    for offset in range(len(matches)):
        text, _, error = checks.client.tool("grep", {"path": path, "pattern": pattern, "head_limit": 1, "offset": offset})
        pages.extend((int(line), value) for line, value in re.findall(re.escape(path) + r":(\d+):(.*)", text.split("\n# results\n", 1)[-1]))
        if error:
            break
    checks.record("grep:pagination", "live_content", pages == matches, {"pages": len(matches), "exact_ordered_matches": pages == matches})
    # Explicit file/directory traversal must obey the same exclusions as indexing.
    for part in ("_validation_excluded", "node_modules", "ignored"):
        excluded = f"{directory}/{part}/{probe['basename']}"
        checks.query(f"exclude:{part}:direct-file", "grep", {"path": excluded, "pattern": probe["needle"], "output_mode": "files_with_matches"}, excludes=[excluded], category="exclusion")
        checks.query(f"exclude:{part}:bypass", "grep", {"path": excluded, "pattern": probe["needle"], "output_mode": "files_with_matches", "include_ignored": True}, contains=[excluded], category="exclusion")
    checks.query("exclude:find", "find", {"path": directory, "pattern": "*"}, contains=[path], excludes=[f"{directory}/{part}/" for part in ("_validation_excluded", "node_modules", "ignored")], category="exclusion")
    checks.query("exclude:mandatory", "grep", {"path": f".codemap/{probe['basename']}", "pattern": probe["needle"], "include_ignored": True, "output_mode": "files_with_matches"}, excludes=[f".codemap/{probe['basename']}"], category="exclusion")
    checks.query("exclude:search", "search", {"query": probe["needle"], "workspace_scope": "all", "caller_context": False}, excludes=[f"{directory}/{part}/" for part in ("_validation_excluded", "node_modules", "ignored")], category="exclusion")


def validate_test_configuration(checks, probe, language):
    path = checks.root / ".codemap/config.toml"
    original = path.read_text()
    arguments = {"file_path": probe["file"], "offset": probe["target_line"], "limit": 1}
    included = original.replace("should_include_test_code = false", "should_include_test_code = true")
    custom = re.sub(r"^test_file_patterns = .*", 'test_file_patterns = ["probe_custom.go"]' if language == "Go" else "test_file_patterns = []", original, flags=re.M)
    if language == "Rust":
        custom = re.sub(r"^rust = .*", 'rust = ["cm_validation_custom"]', custom, flags=re.M)
    try:
        for name, config, present, absent in (
            ("include-tests", included, [probe["test"], probe["custom_test"]], []),
            ("replace-builtins-with-custom", custom, [probe["test"]], [probe["custom_test"]]),
            ("restore", original, [probe["custom_test"]], [probe["test"]]),
        ):
            path.write_text(config)
            good, elapsed, text = wait_condition(checks.client, "read", arguments,
                lambda text, error: not error and all(f"{value} (" in text for value in present) and all(f"{value} (" not in text for value in absent), timeout=90)
            checks.record(f"test-policy:{name}", "test_policy", good, {"expected_callers": present, "excluded_callers": absent, "last_output": text if not good else None}, elapsed)
    finally:
        path.write_text(original)


def validate_find(checks, spec):
    directory = Path(spec["files"][1]).parent.as_posix()
    extension = ".rs" if spec["language"] == "Rust" else ".go"
    paths = [path for path in tracked_files(checks.root) if path.startswith(directory + "/") and path.endswith(extension)]
    ignored = subprocess.run(["git", "check-ignore", "--no-index", "-z", "--stdin"], input="\0".join(paths) + "\0", cwd=checks.root, capture_output=True, text=True)
    if ignored.returncode not in (0, 1):
        raise RuntimeError(ignored.stderr)
    excluded = set(ignored.stdout.split("\0"))
    expected = set(paths) - excluded
    text, elapsed, error = checks.client.tool("find", {"path": directory, "pattern": "*" + extension})
    actual = {line for line in text.splitlines() if line.endswith(extension)}
    checks.record(f"find:{directory}", "file_enumeration", not error and expected == actual, {"expected_count": len(expected), "actual_count": len(actual), "missing": sorted(expected - actual), "unexpected": sorted(actual - expected)}, elapsed)


def wait_condition(client, tool, arguments, predicate, timeout=45):
    started = time.monotonic()
    while time.monotonic() - started < timeout:
        text, _, error = client.tool(tool, arguments)
        if predicate(text, error):
            return True, round((time.monotonic() - started) * 1000, 3), text
        time.sleep(0.5)
    return False, round((time.monotonic() - started) * 1000, 3), text


def validate_refresh(checks, probe):
    path = f"{probe['directory']}/refresh{Path(probe['basename']).suffix}"
    file = checks.root / path
    names = [re.search(r"(?:fn|func) (\w+)", probe[key])[1] for key in ("before", "after")]
    for phase, source, present, absent in (
        ("create", probe["before"], names[0], names[1]),
        ("modify", probe["after"], names[1], names[0]),
        ("delete", None, None, names[1]),
    ):
        if source is None:
            file.unlink()
        else:
            file.write_text(source)
        good, elapsed, text = wait_condition(checks.client, "overview", {"path": path},
            lambda text, error: (present in text and not error and absent not in text) if present else (absent not in text and not text.startswith("# Detailed Codemap:") and not re.search(r"Total Files\*\*: [1-9]", text)))
        checks.record(f"refresh:{phase}:overview", "refresh", good, {"path": path, "expected_symbol": present, "removed_symbol": absent, "last_output": text if not good else None}, elapsed)
        expected_name = present or absent
        good, elapsed, text = wait_condition(checks.client, "search", {"query": expected_name, "workspace_scope": "all", "caller_context": False},
            lambda text, error: not error and ((path in text and present in text) if present else path not in text))
        checks.record(f"refresh:{phase}:search", "refresh", good, {"path": path, "query": expected_name, "last_output": text if not good else None}, elapsed)
        if phase == "modify":
            good, elapsed, text = wait_condition(checks.client, "search", {"query": names[0], "workspace_scope": "all", "caller_context": False}, lambda text, error: not error and path not in text)
            checks.record("refresh:modify:old-search-term", "refresh", good, {"path": path, "removed_query": names[0], "last_output": text if not good else None}, elapsed)


def run_validation(args):
    run_id = args.run_id or datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S")
    if not re.fullmatch(r"[A-Za-z0-9_-]+", run_id):
        raise ValueError("run-id must contain only letters, digits, underscores and hyphens")
    output_root = args.cache / "runs" / run_id
    output_root.mkdir(parents=True, exist_ok=False)
    oracle_binary = output_root / "go_oracle"
    subprocess.run(["go", "build", "-o", str(oracle_binary), str(DATA / "go_oracle.go")], check=True)
    metadata = {
        "started_utc": datetime.now(timezone.utc).isoformat(), "run_id": run_id,
        "binary": str(args.binary), "binary_sha256": sha256(args.binary), "binary_version": command([str(args.binary), "--version"]),
        "runner_sha256": sha256(Path(__file__)), "manifest_sha256": sha256(DATA / "repositories.json"),
        "source_commit": command(["git", "rev-parse", "HEAD"], APP),
        "python": sys.version, "platform": sys.platform,
        "git": command(["git", "--version"]), "tokei": command(["tokei", "--version"]),
        "go": command(["go", "version"]), "rust_analyzer": command(["rust-analyzer", "--version"]),
        "profiles": args.profile or ["default", "structural"], "results": [],
    }
    shutil.copy(Path(__file__), output_root / "runner.py")
    shutil.copy(DATA / "repositories.json", output_root / "repositories.json")
    shutil.copy(DATA / "config.toml", output_root / "config.toml")
    save_json(output_root / "summary.json", metadata)
    for spec in load_specs(args.repo):
        qualification = json.loads((args.cache / "qualification" / f"{spec['name']}.json").read_text())
        if not qualification["qualified"] or qualification["sha"] != spec["sha"]:
            raise RuntimeError(f"Run prepare first: {spec['name']}")
        save_json(output_root / f"{spec['name']}-qualification.json", qualification)
        for profile in metadata["profiles"]:
            label = f"{spec['name']}-{profile}"
            output = output_root / label
            output.mkdir()
            root = output / "worktree"
            subprocess.run(["git", "worktree", "add", "--quiet", "--detach", str(root), spec["sha"]], cwd=args.cache / "repos" / spec["name"], check=True, env={**os.environ, "GIT_LFS_SKIP_SMUDGE": "1"})
            config_hash = write_config(root, profile)
            probe = create_probes(root, spec["language"])
            print(f"{label}: fresh index, {root}", flush=True)
            result = {"repository": spec["name"], "sha": spec["sha"], "profile": profile, "config_sha256": config_hash, "worktree": str(root), "checks": []}
            client = None
            try:
                client = McpClient(args.binary, root, output)
                result["cold_index_seconds"] = client.ready(spec["files"][0], timeout=args.index_timeout)
                checks = Checks(client, root)
                result["checks"] = checks.results
                print(f"{label}: index ready in {result['cold_index_seconds']}s", flush=True)
                for path in spec["files"]:
                    validate_file(checks, spec, path, oracle_binary, output)
                result["sample_files"] = sampled_files(root, spec, qualification)
                for path in result["sample_files"]:
                    validate_file(checks, spec, path, oracle_binary, output, read_live=False)
                for case in spec["cases"]:
                    section = tuple(case["section"]) if "section" in case else None
                    checks.query(case["id"], case["tool"], case["arguments"], contains=case.get("contains", []), excludes=case.get("excludes", []), category=case.get("category"), section=section)
                validate_find(checks, spec)
                validate_probes(checks, probe, spec["language"])
                validate_refresh(checks, probe)
                validate_test_configuration(checks, probe, spec["language"])
            except Exception as error:
                result["error"] = f"{type(error).__name__}: {error}"
                print(f"{label}: infrastructure error: {result['error']}", flush=True)
            finally:
                if client:
                    client.close()
                result["tracked_changes"] = command(["git", "status", "--porcelain", "--untracked-files=no"], root)
                result["counts"] = {status: sum(check["status"] == status for check in result["checks"]) for status in ("pass", "fail", "unverified")}
                save_json(output / "results.json", result)
                metadata["results"].append(result)
                save_json(output_root / "summary.json", metadata)
                print(f"{label}: {result['counts']}", flush=True)
    metadata["regressions"] = run_regressions(args, output_root)
    metadata["finished_utc"] = datetime.now(timezone.utc).isoformat()
    save_json(output_root / "summary.json", metadata)
    print(f"Results: {output_root / 'summary.json'}", flush=True)
    return int(any(result_has_issues(result) for result in metadata["results"] + metadata["regressions"]))


def result_has_issues(result):
    return bool(result.get("error") or result.get("tracked_changes") or any(check["status"] != "pass" for check in result["checks"]))


def run_regressions(args, output_root):
    results = []
    for language, filename, cases in (
        ("rust", "lib.rs", [("str-iterator", 9, ["Unrelated::split —", "Unrelated::collect —"]), ("macro", 13, ["Unrelated::matches —"])]),
        ("go", "probe.go", [("builtin", 14, ["Unrelated.len —"]), ("package", 18, ["Unrelated.Expand —"]), ("atomic-caller", 12, ["AtomicMethod ("])]),
    ):
        for profile in args.profile or ["default", "structural"]:
            output = output_root / f"regression-{language}-{profile}"
            root = output / "source"
            root.mkdir(parents=True)
            shutil.copy(FIXTURES / language / filename, root / filename)
            write_config(root, profile)
            with McpClient(args.binary, root, output) as client:
                client.ready(filename)
                checks = Checks(client, root)
                for name, line, excludes in cases:
                    checks.query(name, "read", {"file_path": filename, "offset": line, "limit": 1}, excludes=excludes, category="wrong_caller" if name == "atomic-caller" else "wrong_callee", section=(filename, line))
                if language == "go":
                    checks.query("implicit-constant", "read", {"file_path": filename, "offset": 31, "limit": 3}, contains=["ImplicitValue — probe.go:28"], category="references", section=(filename, 31))
                results.append({"language": language, "profile": profile, "checks": checks.results})
                save_json(output / "results.json", results[-1])
    return results


def prepare(args):
    for spec in load_specs(args.repo):
        root = args.cache / "repos" / spec["name"]
        if not root.exists():
            root.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run(["git", "clone", "--filter=blob:none", "--no-checkout", "--single-branch", "--no-tags", spec["url"], str(root)], check=True)
        if command(["git", "remote", "get-url", "origin"], root).removesuffix(".git") != spec["url"].removesuffix(".git"):
            raise RuntimeError(f"Unexpected clone origin: {root}")
        run = args.cache / "qualification" / spec["name"]
        if not run.exists():
            run.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run(["git", "worktree", "add", "--detach", str(run), spec["sha"]], cwd=root, check=True, env={**os.environ, "GIT_LFS_SKIP_SMUDGE": "1"})
        if command(["git", "status", "--porcelain", "--untracked-files=no"], run):
            raise RuntimeError(f"Qualification worktree has tracked changes: {run}")
        result = qualify(run, spec)
        save_json(args.cache / "qualification" / f"{spec['name']}.json", result)
        print(f"{spec['name']}: code={result['code_lines']}, commits={result['commits']}, qualified={result['qualified']}", flush=True)
        if not result["qualified"]:
            raise RuntimeError(f"Repository below corpus thresholds: {spec['name']}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path, default=Path.home() / ".cache/codemap-public-validation")
    parser.add_argument("--binary", type=Path, default=Path(shutil.which("codemap-search") or "codemap-search"))
    sub = parser.add_subparsers(dest="operation", required=True)
    prep = sub.add_parser("prepare", help="Clone pinned repositories and independently qualify their size/history")
    prep.add_argument("--repo", action="append")
    run_parser = sub.add_parser("run", help="Fresh-index checks and create/modify/delete refresh checks, with raw transcripts")
    run_parser.add_argument("--repo", action="append")
    run_parser.add_argument("--profile", choices=["default", "structural"], action="append")
    run_parser.add_argument("--run-id")
    run_parser.add_argument("--index-timeout", type=int, default=600)
    regression_parser = sub.add_parser("regressions", help="Run the small Rust/Go reproductions without public repository clones")
    regression_parser.add_argument("--profile", choices=["default", "structural"], action="append")
    regression_parser.add_argument("--run-id")
    query_parser = sub.add_parser("query", help="Call any navigation tool against an existing validation worktree")
    query_parser.add_argument("--root", type=Path, required=True)
    query_parser.add_argument("--ready-file")
    query_parser.add_argument("tool", choices=["overview", "search", "read", "grep", "find"])
    query_parser.add_argument("arguments", type=json.loads)
    args = parser.parse_args()
    args.cache = args.cache.expanduser().resolve()
    args.binary = args.binary.expanduser().resolve()
    if args.operation == "prepare":
        prepare(args)
    elif args.operation == "run":
        return run_validation(args)
    elif args.operation == "regressions":
        run_id = args.run_id or datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S")
        if not re.fullmatch(r"[A-Za-z0-9_-]+", run_id):
            raise ValueError("run-id must contain only letters, digits, underscores and hyphens")
        output = args.cache / "runs" / run_id
        output.mkdir(parents=True, exist_ok=False)
        results = run_regressions(args, output)
        save_json(output / "summary.json", {"binary_sha256": sha256(args.binary), "runner_sha256": sha256(Path(__file__)), "regressions": results})
        for result in results:
            print(result["language"], result["profile"], {check["id"]: check["status"] for check in result["checks"]})
        print(f"Results: {output / 'summary.json'}")
        return int(any(result_has_issues(result) for result in results))
    elif args.operation == "query":
        with McpClient(args.binary, args.root, args.cache / "queries") as client:
            if args.ready_file:
                client.ready(args.ready_file)
            text, _, error = client.tool(args.tool, args.arguments)
            print(text)
            return int(error)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (RuntimeError, TimeoutError, OSError, ValueError, EOFError, subprocess.SubprocessError, queue.Empty) as error:
        print(f"Validation error: {error}", file=sys.stderr)
        sys.exit(2)
