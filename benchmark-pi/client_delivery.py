"""Client delivery guards; product results and pi's native read-tool defaults are unchanged."""
DEFAULT_TEXT_BYTES = 1048576
PRODUCT_TEXT_BYTES = {"codemap-search": 100000,
                      **{arm: DEFAULT_TEXT_BYTES for arm in ("codegraph", "zvec-grep", "graphify", "codebase-memory-mcp")}}
CLAUDE_RESULT_META = "anthropic/maxResultSizeChars"
CLAUDE_PRODUCT_CHARS = 500000


def client_tool_metadata(tool: dict) -> dict:
    """Preserve every product key; only fill the absent Claude result-size annotation."""
    return {**tool, "_meta": {CLAUDE_RESULT_META: CLAUDE_PRODUCT_CHARS, **tool.get("_meta", {})}}
