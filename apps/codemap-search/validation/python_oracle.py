#!/usr/bin/env python3
"""Independent Python declaration/call positions; parses without executing source."""

import ast
import hashlib
import json
from pathlib import Path
import sys


def inspect(path):
    source = path.read_bytes()
    tree = ast.parse(source, filename=str(path))
    declarations, calls = [], []

    def visit(node, function_depth=0):
        is_function = isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
        if is_function and function_depth == 0:
            declarations.append({"name": node.name, "kind": "fn", "start": node.lineno,
                "name_line": node.lineno, "end": node.end_lineno})
        if isinstance(node, ast.Call):
            calls.append({"function": ast.unparse(node.func), "line": node.lineno, "end": node.end_lineno})
        for child in ast.iter_child_nodes(node):
            visit(child, function_depth + int(is_function or isinstance(node, ast.Lambda)))

    visit(tree)
    return {"oracle": "Python ast", "version": sys.version.split()[0], "file_sha256": hashlib.sha256(source).hexdigest(),
        "declarations": declarations, "calls": calls}


if __name__ == "__main__":
    print(json.dumps(inspect(Path(sys.argv[1])), ensure_ascii=False))
