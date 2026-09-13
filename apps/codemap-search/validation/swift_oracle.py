#!/usr/bin/env python3
"""Read declaration positions from the Swift compiler's parse-only AST dump."""

import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


def inspect(path):
    source = path.read_bytes()
    result = subprocess.run(["swiftc", "-frontend", "-dump-parse", str(path)], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr.strip())
    declarations, function_indents = [], []
    pattern = re.compile(r'^(\s*)\((func_decl|constructor_decl).*?range=\[.*?:(\d+):\d+ - line:(\d+):\d+\] "([^"]+)"')
    for line in result.stdout.splitlines():
        if not re.match(r"^\s*\(", line):
            continue
        indent = len(line) - len(line.lstrip())
        while function_indents and indent <= function_indents[-1]:
            function_indents.pop()
        match = pattern.search(line)
        if match:
            name = match[5].split("(", 1)[0]
            if not function_indents:
                declarations.append({"name": name, "native_name": match[5], "kind": "fn",
                    "start": int(match[3]), "name_line": int(match[3]), "end": int(match[4])})
            function_indents.append(indent)
    return {"oracle": "Swift compiler -frontend -dump-parse", "declarations": declarations,
        "file_sha256": hashlib.sha256(source).hexdigest(),
        "scope": "Named functions and initializers outside function bodies; parse-only, no type checking or macro expansion"}


if __name__ == "__main__":
    print(json.dumps(inspect(Path(sys.argv[1])), ensure_ascii=False))
