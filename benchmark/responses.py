"""Native response schemas and conservative source-location decoding."""
from __future__ import annotations

import re


def tool_schema(name, description, properties, required):
    return {"name": name, "description": description,
            "inputSchema": {"type": "object", "properties": properties, "required": required, "additionalProperties": False},
            "annotations": {"readOnlyHint": True, "destructiveHint": False}}



def content_text(result: dict) -> str:
    return "\n".join(c.get("text", "") for c in result.get("content", []) if c.get("type") == "text")



def text_result(text: str, is_error=False) -> dict:
    return {"content": [{"type": "text", "text": text}], "isError": is_error}



def source_lines(tool: str, arguments: dict, text: str, *, partial=False) -> list[dict] | None:
    lines = []
    current_path = arguments.get("file_path", arguments.get("path", arguments.get("file"))) if tool == "read" else None
    recognized = tool in {"find", "initial_instructions"}
    # A byte cap may cut a line exactly at an apparently valid source prefix.
    marker = "\n[host output truncated]"
    if marker in text:
        text = text.split(marker, 1)[0]
        partial = True
    if partial and not text.endswith("\n"):
        text = text.rsplit("\n", 1)[0] if "\n" in text else ""
    for line in text.splitlines():
        match = re.match(r"^(.+\.(?:tsx?|jsx?|go|rs|py|json|yaml|yml|toml|md|html|css|sql))(?::|-)(\d+)(?::|-)(.*)$", line)
        if match:
            lines.append({"path": match[1].removeprefix("./"), "line": int(match[2]), "text": match[3]})
            recognized = True
            continue
        match = re.match(r"^\s*(\d+)\s*[→│|](.*)$", line)
        if match and current_path:
            lines.append({"path": current_path.removeprefix("./"), "line": int(match[1]), "text": match[2]})
            recognized = True
    # Unknown search/overview formats remain unobservable; never infer evidence from a filename.
    return lines if recognized or text.strip() in {"", "No matches found.", "No matches found"} else None

