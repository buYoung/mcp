"""Shared helpers for the related-terms PoC scripts.

Everything here is deterministic and offline: path resolution, hashing, the
GitHub Explore front-matter subset parser, and the `rtn-2` normalization rules
that the Rust reader re-implements (see contracts/dataset.md).
"""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

EXPERIMENT_DIR = Path(__file__).resolve().parent
REPO_ROOT = EXPERIMENT_DIR.parents[3]
DATA_ROOT = REPO_ROOT / ".codemap" / "poc" / "related-terms"
RAW_DIR = DATA_ROOT / "sources" / "raw"
ARCHIVE_DIR = DATA_ROOT / "sources" / "archives"
DATASETS_DIR = DATA_ROOT / "datasets"
RESULTS_DIR = EXPERIMENT_DIR / "results"
SOURCES_LOCK_PATH = EXPERIMENT_DIR / "sources.lock.json"

SCHEMA_VERSION = "rt-dataset/1"
NORMALIZATION_VERSION = "rtn-2"
# Characters that only separate words inside an expression. Any other
# non-alphanumeric, non-whitespace character makes the token key lossy.
TOKEN_SEPARATORS = frozenset("_-./:")


def repo_relative(path: Path) -> str:
    return path.resolve().relative_to(REPO_ROOT).as_posix()


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: object) -> str:
    """One canonical JSON text: sorted keys, no insignificant whitespace, UTF-8."""
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def read_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


# --- rtn-2 normalization -----------------------------------------------------------------


def split_identifier(identifier: str) -> list[str]:
    """Port of `codemap_search::parser::split_identifier` (camel/snake/hyphen/digit/uncased).

    The Rust reader calls the product function directly; `rtpoc normalize-check`
    verifies that both produce the same keys for every dataset surface.
    """
    tokens: list[str] = []
    current: list[str] = []
    chars = list(identifier)
    length = len(chars)

    # Single-character islower/isupper read the Unicode Lowercase/Uppercase properties,
    # matching Rust `char::is_lowercase`/`is_uppercase`.
    def is_lower(c: str) -> bool:
        return c.islower()

    def is_upper(c: str) -> bool:
        return c.isupper()

    def is_uncased_alpha(c: str) -> bool:
        return c.isalpha() and not is_lower(c) and not is_upper(c)

    for i, c in enumerate(chars):
        if c in "_-":
            if current:
                tokens.append("".join(current).lower())
                current = []
            continue
        prev = chars[i - 1] if i > 0 else ""
        prev_is_lowercase = i > 0 and is_lower(prev)
        prev_is_digit = i > 0 and prev.isascii() and prev.isdigit()
        prev_is_uppercase = i > 0 and is_upper(prev)
        current_is_uppercase = is_upper(c)
        next_is_lowercase = i + 1 < length and is_lower(chars[i + 1])

        is_camel_boundary = (prev_is_lowercase or prev_is_digit) and current_is_uppercase
        is_acronym_boundary = prev_is_uppercase and current_is_uppercase and next_is_lowercase
        is_digit_boundary = prev_is_digit and c.isalpha()
        prev_is_uncased = i > 0 and is_uncased_alpha(prev)
        current_is_uncased = is_uncased_alpha(c)
        is_uncased_boundary = i > 0 and (
            (prev_is_uncased and not current_is_uncased and (c.isalpha() or (c.isascii() and c.isdigit())))
            or (
                not prev_is_uncased
                and (prev.isalpha() or (prev.isascii() and prev.isdigit()))
                and current_is_uncased
            )
        )
        if (is_camel_boundary or is_acronym_boundary or is_digit_boundary or is_uncased_boundary) and current:
            tokens.append("".join(current).lower())
            current = []
        current.append(c)
    if current:
        tokens.append("".join(current).lower())
    return tokens


def exact_key(surface: str) -> str:
    """Lowercase, collapse whitespace runs, trim. Punctuation is preserved."""
    return " ".join(surface.lower().split())


def token_key(surface: str) -> tuple[str, bool]:
    """Alphanumeric runs split by identifier boundaries, joined by one space.

    Returns `(key, is_lossy)`. A lossy key dropped a character that is neither a
    word separator nor alphanumeric (for example `c++`, `c#`, `one's`); lossy keys
    are never used for normalized matching. Non-ASCII text has no token key: Python
    `isalnum` and Rust `is_alphanumeric` disagree on Unicode combining marks, so only
    exact matching is defined for it.
    """
    if not surface.isascii():
        return "", True
    parts: list[str] = []
    current: list[str] = []
    is_lossy = False
    for c in surface:
        if c.isalnum():
            current.append(c)
            continue
        if not (c.isspace() or c in TOKEN_SEPARATORS):
            is_lossy = True
        if current:
            parts.append("".join(current))
            current = []
    if current:
        parts.append("".join(current))
    tokens = [token for part in parts for token in split_identifier(part)]
    return " ".join(tokens), is_lossy or not tokens


# --- GitHub Explore front matter ---------------------------------------------------------

_FRONT_MATTER = re.compile(r"^---\r?\n(.*?)\r?\n---(?:\r?\n|$)", re.S)
_FIELD = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)\s*:\s?(.*)$")


def _unquote_scalar(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] == '"':
        return json.loads(value)
    if len(value) >= 2 and value[0] == value[-1] == "'":
        return value[1:-1].replace("''", "'")
    # A plain scalar ends at a ` #` comment.
    comment = re.search(r"\s#", value)
    return (value[: comment.start()] if comment else value).strip()


def parse_front_matter(text: str) -> dict[str, str]:
    """Parse the flat `key: value` YAML subset used by `topics/*/index.md`.

    Supports comment lines, plain scalars with indented folded continuation lines, and
    single/double quoted scalars that may continue on unindented lines. A repeated key
    keeps its last value, as YAML loaders do. Anything else raises, so unsupported YAML
    never parses silently.
    """
    match = _FRONT_MATTER.match(text)
    if not match:
        raise ValueError("missing front matter")
    fields: dict[str, list[str]] = {}
    current: str | None = None

    def is_open_quote(pieces: list[str]) -> bool:
        joined = " ".join(pieces)
        return bool(joined) and joined[0] in "'\"" and (len(joined) < 2 or joined[-1] != joined[0])

    for line in match.group(1).splitlines():
        if current is not None and is_open_quote(fields[current]):
            fields[current].append(line.strip())
            continue
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line[0] in " \t":
            if current is None:
                raise ValueError(f"continuation without key: {line!r}")
            fields[current].append(line.strip())
            continue
        field = _FIELD.match(line)
        if not field:
            raise ValueError(f"unsupported front matter line: {line!r}")
        current = field.group(1)
        fields[current] = [field.group(2).strip()]
    parsed: dict[str, str] = {}
    for key, pieces in fields.items():
        raw = " ".join(piece for piece in pieces if piece)
        if raw.startswith(("|", ">", "[", "{", "&", "*", "!")):
            raise ValueError(f"unsupported YAML scalar for {key}: {raw[:20]!r}")
        parsed[key] = _unquote_scalar(raw)
    return parsed


def split_csv_field(value: str | None) -> list[str]:
    if not value:
        return []
    return [item.strip() for item in value.split(",") if item.strip()]
