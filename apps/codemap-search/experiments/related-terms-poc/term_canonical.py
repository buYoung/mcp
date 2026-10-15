"""Spelling differences that do not change which word a term is, shared by the dataset build and lookup input.

`$state`, `@Component`, `todo!`, `useState()`, `__init__` and `.json` are decorated forms of `state`,
`Component`, `todo`, `useState`, `init` and `json`; `AbortSignal`, `abort signal` and `abort_signal`
differ only by case and inner separators. The category dataset stores each word once under `word_key`
and lookup input is keyed the same way, so these spellings never appear as separate words.
Forms whose remainder would not be the same word keep their symbols: shell variables (`$?`, `$0`),
one-letter extensions (`.c`, `.h`), namespaced names (`std::vector`), `C#`, `C++`, `.NET`.
"""

import re

KEPT = {".net"}
CALL_SUFFIX = re.compile(r"(?<=[\w!])\(\)$")
MACRO_SUFFIX = re.compile(r"(?<=\w)!$")
DUNDER = re.compile(r"__([A-Za-z][A-Za-z0-9_]*?)__")
SIGIL_PREFIX = re.compile(r"^[$@](?=[A-Za-z])")
EXTENSION_PREFIX = re.compile(r"^\.(?=[A-Za-z0-9][\w.\-]+$)")
INNER_SEPARATORS = re.compile(r"(?<=[a-z0-9])[\s_\-]+(?=[a-z0-9])")


def canonical_token(token):
    if token.lower() in KEPT:
        return token
    bare = MACRO_SUFFIX.sub("", CALL_SUFFIX.sub("", token))
    dunder = DUNDER.fullmatch(bare)
    if dunder:
        bare = dunder.group(1)
    bare = EXTENSION_PREFIX.sub("", SIGIL_PREFIX.sub("", bare))
    return bare or token


def canonical_surface(text):
    """`$state` -> `state`, `@Injectable()` -> `Injectable`, `todo!` -> `todo`; whitespace collapsed."""
    return " ".join(canonical_token(token) for token in text.split())


def exact_key(text):
    """Lowercase, collapse whitespace runs, trim (same as `rt-related/1` `exact_key`)."""
    return " ".join(text.lower().split())


def word_key(text):
    """One key per word: `$state` == `state`, `AbortSignal` == `abort signal` == `abort_signal`; `--flag` keeps its dashes."""
    return INNER_SEPARATORS.sub("", exact_key(canonical_surface(text)))
