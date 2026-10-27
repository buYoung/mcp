"""Byte-referenced product delivery guards, separate from native CLI/script output defaults."""
from .core import require

DEFAULT_TEXT_BYTES = 1024 * 1024
PRODUCT_TEXT_BYTES = {"codemap-search": 100000, "serena": 1250000,
                      **{arm: DEFAULT_TEXT_BYTES for arm in ("codegraph", "zvec-grep", "graphify", "codebase-memory-mcp")}}


def output_limits(arm: str) -> dict:
    require(arm in PRODUCT_TEXT_BYTES or arm == "rg", "unknown delivery policy")
    if arm == "rg":
        return {"reference_text_bytes": None, "codex_tokens": None, "claude_tokens": None}
    size = PRODUCT_TEXT_BYTES[arm]
    tokens = (size + 3) // 4
    # Codex measures text UTF-8 bytes, not its enclosing wire JSON. codemap exports its own token value.
    # Claude's UTF-16 estimate must stay within the N/2 fast path, avoiding count_tokens entirely.
    return {"reference_text_bytes": size, "codex_tokens": 100000 if arm == "codemap-search" else tokens,
            "claude_tokens": 2 * tokens}
