"""Parts of a code-search input that are code syntax, not what the agent is looking for (`rt-syntax-parts/1`).

Agents wrap the subject of a search in syntax: `pub fn build`, `class HeroDisplay`, `*_test.go`,
`pending[Mm]easure`. When an input resolves part by part, each of `fn`, `go` or `mm` would bring every sense of that
word. A part is syntax when every place it occurs in the input is one of:

- a file extension: the last dotted segment of a file name (`client.go`, `themes.css`, `*.{ts,tsx}`);
- a regex character class (`[Mm]`, `[AB]`);
- a reserved word of the ten measured languages (JavaScript, Python, TypeScript, Java, C#, C++, C, Rust, PHP, Go)
  written as its own token in declaration position: with more text after it in the same alternative (`fn build`,
  `struct `, `async [a-z]+\\(`), after another reserved word (`pub fn`); type names and literals may also follow a
  name (`key string`, `return true`).
  Reserved words that are ordinary English words count only when the input also shows code: regex syntax, a joined
  identifier (`DeviceUpdate`, `acquire_lock`, `client.go`), a code-only reserved word or two adjacent reserved words
  (`export default`).

A part that also occurs inside a joined identifier (`class_declaration`, `UseRoutes`), after member access
(`.where(`), or alone in an alternative (`list|locationApi`) is a subject and stays.
"""

import re

# Reserved words that are not English words: syntax wherever they stand as their own token in declaration position.
CODE_ONLY_KEYWORDS = frozenset("""
fn func fun def impl struct enum mod typedef typealias pub mut dyn elif nil nullptr constexpr consteval constinit decltype
noexcept typename sizeof alignof alignas typeof instanceof readonly extern endif endforeach endwhile endfor endswitch
enddeclare isset unset foreach lambda nonlocal del var let val const bool boolean int uint void usize isize u8 u16 u32
u64 u128 i8 i16 i32 i64 i128 f32 f64 char wchar_t goto fallthrough chan crate
""".split())
# Reserved words that are also English words: syntax only when the input shows code.
ENGLISH_KEYWORDS = frozenset("""
class type interface trait function union namespace public private protected internal static final abstract sealed
override virtual inline unsafe async await suspend export default return new case yield throw throws long short double float
byte string true false null undefined and or not is as in with from
""".split())
# Reserved operator words, also the glue of English phrases (`new or existing`): never a sign that the input is code.
OPERATOR_KEYWORDS = frozenset("and or not is as in with from".split())
# Of the reserved words above, those that may follow the name they belong to (`key string`, `X struct`, `return true`).
TRAILING_KEYWORDS = frozenset("""
struct interface bool boolean int uint void usize isize u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 char string long
short double float byte true false null nil undefined
""".split())
EXTENSIONS = frozenset("""
js mjs cjs jsx ts tsx mts cts py pyi pyx java kt kts scala groovy cs csx vb fs cpp cc cxx c h hpp hh hxx inl ipp rs php
phtml go swift rb dart lua sh bash zsh bat cmd ps1 css scss sass less html htm xml xaml vue svelte json jsonl yaml yml
toml ini cfg conf md mdx txt sql proto gradle sln csproj lock env svg wasm
""".split())

CHARACTER_CLASS = re.compile(r"(?<!\\)\[(?:\\.|[^\]\\])*\]")
EXTENSION_GROUP = re.compile(r"\.\{([^{}]*)\}")
ESCAPE_CLASS = re.compile(r"\\[A-Za-z]")
ESCAPED_SYMBOL = re.compile(r"\\([^A-Za-z0-9])")
ALTERNATIVE = re.compile(r"[|,{}]")
CHUNK = re.compile(r"[A-Za-z0-9_]+(?:(?:\.|-|::)[A-Za-z0-9_]+)*")
SUBPART = re.compile(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|[0-9]+")
REGEX_SYNTAX = re.compile(r"[\\\[\]()^$*+?{}|]")


def subparts(chunk):
    """`DeviceUpdate` -> device, update; `client.go` -> client, go; `get_mut` -> get, mut."""
    return [p.lower() for piece in re.split(r"\.|-|::|_", chunk) for p in SUBPART.findall(piece)]


def occurrences(query):
    """(word, role) for every word in the input; role is extension, class, keyword or subject."""
    found = [(run.lower(), "class") for match in CHARACTER_CLASS.finditer(query) for run in re.findall(r"[A-Za-z]+", match.group())]
    text = ESCAPED_SYMBOL.sub(r"\1", ESCAPE_CLASS.sub(" ", CHARACTER_CLASS.sub(" ", query)))
    # `*.{ts,tsx}` lists extensions: spell each one as `.ts` so it keeps its dot in its own alternative.
    text = EXTENSION_GROUP.sub(lambda m: "|".join("." + item.strip() for item in m.group(1).split(",")), text)
    chunks = [(match, alternative) for alternative in ALTERNATIVE.split(text) for match in CHUNK.finditer(alternative)]
    declaring = (CODE_ONLY_KEYWORDS | ENGLISH_KEYWORDS) - OPERATOR_KEYWORDS
    shows_code = bool(REGEX_SYNTAX.search(query)) or any(len(subparts(m.group())) > 1 or m.group() in CODE_ONLY_KEYWORDS
                                                         for m, _ in chunks) \
        or any(a.group() in declaring and b.group() in declaring and a_alt is b_alt and not a_alt[a.end():b.start()].strip()
               for (a, a_alt), (b, b_alt) in zip(chunks, chunks[1:]))
    for match, alternative in chunks:
        chunk, start, end = match.group(), match.start(), match.end()
        parts = subparts(chunk)
        if len(parts) > 1:
            last = chunk.rsplit(".", 1)
            is_extension = len(last) == 2 and last[1].lower() in EXTENSIONS
            found += [(p, "subject") for p in (subparts(last[0]) if is_extension else parts)]
            if is_extension:
                found.append((last[1].lower(), "extension"))
            continue
        word = chunk.lower()
        before, after = alternative[:start], alternative[end:]
        if before.endswith(".") and word in EXTENSIONS and not after.strip(" *'\")$"):
            found.append((word, "extension"))
        elif chunk in CODE_ONLY_KEYWORDS | ENGLISH_KEYWORDS and (chunk in CODE_ONLY_KEYWORDS or shows_code) \
                and not re.search(r"(\.|::|->)$", before) and is_declaration_position(chunk, before, after):
            found.append((word, "keyword"))
        else:
            found.append((word, "subject"))
    return found


def is_declaration_position(keyword, before, after):
    """More text follows in the same alternative, another reserved word precedes it (`pub fn`), or a type name or
    literal follows a name (`key string`)."""
    if after and not after.startswith("("):
        return True
    previous = CHUNK.findall(before)
    if previous and previous[-1] in CODE_ONLY_KEYWORDS | ENGLISH_KEYWORDS:
        return True
    return keyword in TRAILING_KEYWORDS and bool(previous)


def is_syntax_part(span, query):
    """Whether the resolved part `span` of `query` occurs only as syntax; multi-word spans are never syntax."""
    if " " in span.strip():
        return False
    roles = {role for word, role in occurrences(query) if word == span.lower()}
    return bool(roles) and "subject" not in roles
