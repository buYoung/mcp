#!/usr/bin/env python3
"""Independent explicit script/frontmatter extraction with HTMLParser and TypeScript.

This is a declaration oracle for embedded script code, not a Vue/Astro/Svelte
template compiler. Expressions in markup and unsupported script languages are
outside its coverage. Script blocks are parsed independently.
"""

import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import subprocess
import sys
import tempfile


class Scripts(HTMLParser):
    def __init__(self, source, extension):
        super().__init__(convert_charrefs=False)
        self.source, self.extension = source, extension
        self.offsets, offset = [], 0
        for line in source.splitlines(keepends=True):
            self.offsets.append(offset)
            offset += len(line)
        self.stack, self.active, self.regions = [], None, []

    def position(self):
        line, column = self.getpos()
        return self.offsets[line - 1] + column

    def handle_starttag(self, tag, attributes):
        attrs = dict(attributes)
        if tag == "script" and (not self.stack or self.extension == "astro"):
            kind = (attrs.get("lang") or "js").lower()
            mime = (attrs.get("type") or "").lower()
            if kind in {"js", "javascript", "ts", "typescript"} and mime in {"", "module", "text/javascript", "application/javascript"} and "src" not in attrs:
                self.active = (self.position() + len(self.get_starttag_text()), "ts" if kind in {"ts", "typescript"} else "js")
        if tag not in {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"}:
            self.stack.append(tag)

    def handle_endtag(self, tag):
        if tag == "script" and self.active:
            start, kind = self.active
            self.regions.append((start, self.position(), kind))
            self.active = None
        if tag in self.stack:
            self.stack = self.stack[:len(self.stack) - 1 - self.stack[::-1].index(tag)]

    def handle_startendtag(self, tag, attributes):
        pass


def inspect(path):
    data = path.read_bytes()
    source = data.decode("utf-8-sig")
    extension = path.suffix[1:]
    regions, body = [], source
    if extension == "astro" and source.splitlines()[:1] == ["---"]:
        lines = source.splitlines(keepends=True)
        closing = next((index for index, line in enumerate(lines[1:], 1) if line.strip() == "---"), None)
        if closing is None:
            raise ValueError("Unterminated Astro frontmatter")
        start, end = len(lines[0]), sum(map(len, lines[:closing]))
        regions.append((start, end, "ts"))
        stop = end + len(lines[closing])
        body = "".join(character if character in "\r\n" else " " for character in source[:stop]) + source[stop:]
    parser = Scripts(body, extension)
    parser.feed(body)
    parser.close()
    if parser.active:
        raise ValueError("Unterminated script block")
    regions.extend(parser.regions)
    declarations, parts = [], []
    with tempfile.TemporaryDirectory(prefix="codemap-component-oracle-") as temporary:
        for index, (start, end, kind) in enumerate(regions):
            mask = "".join(character if character in "\r\n" else " " for character in source[:start]) + source[start:end]
            part = Path(temporary) / f"part{index}.{kind}"
            part.write_text(mask)
            result = subprocess.run(["node", str(Path(__file__).with_name("typescript_oracle.cjs")), str(part)], capture_output=True, text=True)
            if result.returncode:
                raise ValueError(result.stdout or result.stderr)
            oracle = json.loads(result.stdout)
            parts.append({"start_character": start, "end_character": end, "language": kind,
                "mask_sha256": oracle["file_sha256"], "compiler_version": oracle["version"]})
            declarations.extend(oracle["declarations"])
    return {"oracle": "Python HTMLParser + TypeScript compiler", "declarations": declarations,
        "file_sha256": hashlib.sha256(data).hexdigest(), "parts": parts,
        "scope": "Explicit script blocks and Astro frontmatter; component template expressions excluded"}


if __name__ == "__main__":
    print(json.dumps(inspect(Path(sys.argv[1])), ensure_ascii=False))
