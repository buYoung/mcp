"""Extract code-search commands from local agent session logs (internal experiment).

Reads Codex (`~/.codex/sessions`) and Claude Code (`~/.claude/projects`) session logs and writes one line per
search: `rg`/`grep`/`git grep` patterns, `find -name`/`fd`/`rg --files -g` file patterns, codemap-search
`search`/`grep`/`find` and Claude `Grep`/`Glob` calls. Each search keeps the searches and file reads that
followed it in the same session, which become the expected words and the related code of a test input.

The output holds raw project names, paths and identifiers, so it is written outside the repository
(default `~/.cache/related-terms-search-log/`). Only generalized test inputs are copied into the repository.

  python3 apps/codemap-search/experiments/related-terms-poc/extract_search_log.py
"""

from __future__ import annotations

import argparse
import glob
import hashlib
import json
import os
import re
import shlex

DEFAULT_OUTPUT = os.path.expanduser("~/.cache/related-terms-search-log")
FOLLOW_ACTIONS = 10
MARKERS = ('"tool_use"', '"function_call"', '"custom_tool_call"', '"session_meta"', '"turn_context"')

RG_VALUE_OPTIONS = {"-g", "--glob", "--iglob", "-t", "--type", "-T", "--type-not", "-A", "-B", "-C", "-m", "--max-count",
                    "-M", "--max-columns", "-j", "--threads", "--type-add", "--sort", "--sortr", "--color", "--colors",
                    "-E", "--encoding", "--ignore-file", "-d", "--max-depth", "--max-filesize", "-r", "--replace",
                    "--path-separator", "--pre", "--pre-glob", "--context-separator", "--field-match-separator", "-f", "--file",
                    "-e", "--regexp"}
GREP_VALUE_OPTIONS = {"-A", "-B", "-C", "-m", "--max-count", "-f", "--file", "--include", "--exclude", "--exclude-dir",
                      "--color", "--colour", "-d", "-D", "--label", "--context", "--after-context", "--before-context"}
FIND_NAME_OPTIONS = {"-name", "-iname", "-path", "-ipath", "-regex", "-iregex", "-wholename"}
FD_VALUE_OPTIONS = {"-e", "--extension", "-t", "--type", "-d", "--max-depth", "--min-depth", "-E", "--exclude", "-x",
                    "--exec", "-X", "--exec-batch", "-j", "--threads", "-S", "--size", "--changed-within", "--changed-before", "-c", "--color"}
READ_VALUE_OPTIONS = {"cat": set(), "less": set(), "bat": {"-r", "--line-range", "-l", "--language"}, "head": {"-n", "-c"},
                      "tail": {"-n", "-c"}, "nl": {"-b", "-v", "-w", "-s", "-i"}, "sed": {"-e", "--expression", "-f", "--file"}}
CODEMAP_SEARCH = {"search": ("query", "content"), "grep": ("pattern", "content"), "find": ("pattern", "filename")}
JS_CALL = re.compile(r"tools\.(\w+)\(\{")
JS_FIELD = re.compile(r"""["']?(\w+)["']?\s*:\s*("(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|`(?:[^`\\]|\\.)*`)""")


def js_string(literal):
    body = literal[1:-1]
    if literal[0] == '"':
        try:
            return json.loads(literal)
        except ValueError:
            pass
    return re.sub(r"\\(.)", lambda m: {"n": "\n", "t": "\t"}.get(m.group(1), m.group(1)), body)


def js_calls(code):
    """`tools.<name>({...})` calls in a Codex `exec` cell, with their string fields."""
    calls = []
    for match in JS_CALL.finditer(code):
        depth, end = 0, match.end() - 1
        for end in range(match.end() - 1, min(len(code), match.end() + 20000)):
            depth += {"{": 1, "}": -1}.get(code[end], 0)
            if depth == 0:
                break
        fields = {key: js_string(value) for key, value in JS_FIELD.findall(code[match.end():end])}
        calls.append((match.group(1), fields))
    return calls


def segments(command):
    """Simple commands of a shell line, split at `|`, `&&`, `||`, `;` and newlines."""
    try:
        lexer = shlex.shlex(command.replace("\\\n", " "), posix=True, punctuation_chars=";&|()\n")
        lexer.whitespace = " \t\r"
        lexer.whitespace_split = True
        tokens = list(lexer)
    except ValueError:
        return []
    current, result = [], []
    for token in tokens:
        if token and set(token) <= set(";&|()\n"):
            if current:
                result.append(current)
            current = []
        else:
            current.append(token)
    if current:
        result.append(current)
    return result


def positionals(arguments, value_options):
    found, options, skip = [], {}, False
    for i, token in enumerate(arguments):
        if skip:
            skip = False
            continue
        if token == "--":
            found += arguments[i + 1:]
            break
        if token.startswith("--") and "=" in token:
            key, value = token.split("=", 1)
            options.setdefault(key, []).append(value)
        elif token in value_options:
            if i + 1 < len(arguments):
                options.setdefault(token, []).append(arguments[i + 1])
            skip = True
        elif token.startswith("-") and len(token) > 1:
            if token[:2] in value_options and len(token) > 2 and not token.startswith("--"):
                options.setdefault(token[:2], []).append(token[2:])
            else:
                options.setdefault(token, []).append("")
        else:
            found.append(token)
    return found, options


def shell_actions(command):
    """Searches and file reads in one shell command line."""
    actions = []
    for words in segments(command):
        while words and re.fullmatch(r"\w+=.*", words[0]):
            words = words[1:]
        if not words:
            continue
        name = os.path.basename(words[0])
        rest = words[1:]
        if name == "git" and rest[:1] == ["grep"]:
            name, rest = "git-grep", rest[1:]
        if name == "rg":
            found, options = positionals(rest, RG_VALUE_OPTIONS)
            if "--files" in options:
                globs = options.get("-g", []) + options.get("--glob", []) + options.get("--iglob", [])
                actions += [("search", "rg-files", g, "filename") for g in globs if not g.startswith("!")]
                continue
            patterns = options.get("-e", []) + options.get("--regexp", [])
            if not patterns and found:
                patterns = [found[0]]
            actions += [("search", "rg", p, "content") for p in patterns]
        elif name in ("grep", "egrep", "fgrep", "git-grep"):
            found, options = positionals(rest, GREP_VALUE_OPTIONS | {"-e"})
            patterns = options.get("-e", []) or found[:1]
            actions += [("search", "grep", p, "content") for p in patterns]
        elif name == "find":
            for i, token in enumerate(rest[:-1]):
                if token in FIND_NAME_OPTIONS:
                    actions.append(("search", "find", rest[i + 1], "filename"))
        elif name in ("fd", "fdfind"):
            found, options = positionals(rest, FD_VALUE_OPTIONS | {"-g", "--glob"})
            if found:
                actions.append(("search", "fd", found[0], "filename"))
        elif name in READ_VALUE_OPTIONS:
            found, options = positionals(rest, READ_VALUE_OPTIONS[name])
            if name == "sed" and not (options.get("-e") or options.get("--expression")):
                found = found[1:]
            actions += [("read", name, f, "") for f in found if ("/" in f or "." in f) and "$" not in f and "*" not in f]
    return actions


def tool_actions(name, fields):
    """Searches and reads of one structured tool call (codemap-search, Claude Code tools)."""
    short = re.sub(r"^mcp__codemap[-_]search__", "", name)
    if short != name or name in ("search", "grep", "find", "read"):
        if short in CODEMAP_SEARCH:
            key, kind = CODEMAP_SEARCH[short]
            return [("search", f"codemap-{short}", fields[key], kind)] if fields.get(key) else []
        if short == "read" and fields.get("file_path"):
            return [("read", "codemap-read", fields["file_path"], "")]
        return []
    if name == "Grep" and fields.get("pattern"):
        return [("search", "claude-grep", fields["pattern"], "content")]
    if name == "Glob" and fields.get("pattern"):
        return [("search", "claude-glob", fields["pattern"], "filename")]
    if name == "Read" and fields.get("file_path"):
        return [("read", "claude-read", fields["file_path"], "")]
    if name in ("Bash", "exec_command", "shell", "local_shell"):
        command = fields.get("command") or fields.get("cmd") or ""
        if isinstance(command, list):
            command = command[-1] if command[:2] in (["bash", "-lc"], ["zsh", "-lc"], ["sh", "-c"]) else shlex.join(command)
        return shell_actions(command) if isinstance(command, str) else []
    return []


def codex_session(path):
    cwd, actions = "", []
    with open(path, errors="ignore") as handle:
        for line in handle:
            if not any(m in line for m in MARKERS):
                continue
            try:
                row = json.loads(line)
            except ValueError:
                continue
            payload = row.get("payload") or {}
            kind = row.get("type")
            if kind in ("session_meta", "turn_context"):
                cwd = payload.get("cwd") or cwd
                continue
            if payload.get("type") == "custom_tool_call" and payload.get("name") == "exec":
                for name, fields in js_calls(payload.get("input") or ""):
                    base = fields.get("workdir") or cwd
                    actions += [(*a, base) for a in tool_actions(name, fields)]
            elif payload.get("type") in ("function_call", "local_shell_call"):
                try:
                    fields = json.loads(payload.get("arguments") or "{}")
                except ValueError:
                    continue
                if payload.get("type") == "local_shell_call":
                    fields, name = (payload.get("action") or {}), "local_shell"
                else:
                    name = payload.get("name") or ""
                    if (payload.get("namespace") or "").startswith("mcp__codemap"):
                        name = f"mcp__codemap_search__{name}"
                if isinstance(fields, dict):
                    base = fields.get("workdir") or cwd
                    actions += [(*a, base) for a in tool_actions(name, fields)]
    return actions


def claude_session(path):
    actions = []
    with open(path, errors="ignore") as handle:
        for line in handle:
            if '"tool_use"' not in line:
                continue
            try:
                row = json.loads(line)
            except ValueError:
                continue
            content = (row.get("message") or {}).get("content")
            if not isinstance(content, list):
                continue
            for block in content:
                if isinstance(block, dict) and block.get("type") == "tool_use" and isinstance(block.get("input"), dict):
                    actions += [(*a, row.get("cwd") or "") for a in tool_actions(block.get("name") or "", block["input"])]
    return actions


def absolute(path, cwd):
    path = os.path.expanduser(path)
    return os.path.normpath(path if os.path.isabs(path) or not cwd else os.path.join(cwd, path))


def search_rows(source, path, actions):
    session = hashlib.sha256(path.encode()).hexdigest()[:12]
    for i, (action, tool, query, kind, cwd) in enumerate(actions):
        if action != "search":
            continue
        follow = actions[i + 1:i + 1 + FOLLOW_ACTIONS]
        yield {
            "id": f"{session}-{i}", "source": source, "session": session, "cwd": cwd, "tool": tool, "kind": kind, "query": query,
            "follow": [{"action": a, "tool": t, "query": q} for a, t, q, _, _ in follow],
            "files": list(dict.fromkeys(absolute(q, c) for a, _, q, _, c in follow if a == "read")),
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--codex", default=os.path.expanduser("~/.codex/sessions"))
    parser.add_argument("--claude", default=os.path.expanduser("~/.claude/projects"))
    parser.add_argument("--output", default=DEFAULT_OUTPUT)
    arguments = parser.parse_args()
    os.makedirs(arguments.output, exist_ok=True)
    counts = {"codex": 0, "claude": 0}
    with open(os.path.join(arguments.output, "searches.jsonl"), "w") as out:
        sessions = [("codex", p, codex_session) for p in sorted(glob.glob(os.path.join(arguments.codex, "**/*.jsonl"), recursive=True))]
        sessions += [("claude", p, claude_session) for p in sorted(glob.glob(os.path.join(arguments.claude, "**/*.jsonl"), recursive=True))]
        for source, path, reader in sessions:
            for row in search_rows(source, path, reader(path)):
                out.write(json.dumps(row, ensure_ascii=False) + "\n")
                counts[source] += 1
    print(json.dumps({"output": arguments.output, "searches": counts, "sessions": len(sessions)}))


if __name__ == "__main__":
    main()
