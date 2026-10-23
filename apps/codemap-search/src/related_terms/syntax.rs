//! Syntax roles apply only to fallback parts, never to a classified whole expression.
use super::normalize::input_parts;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

const CODE_ONLY: &str = "fn func fun def impl struct enum mod typedef typealias pub mut dyn elif nil nullptr constexpr consteval constinit decltype noexcept typename sizeof alignof alignas typeof instanceof readonly extern endif endforeach endwhile endfor endswitch enddeclare isset unset foreach lambda nonlocal del var let val const bool boolean int uint void usize isize u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 char wchar_t goto fallthrough chan crate";
const ENGLISH: &str = "class type interface trait function union namespace public private protected internal static final abstract sealed override virtual inline unsafe async await suspend export default return new case yield throw throws long short double float byte string true false null undefined and or not is as in with from";
const OPERATORS: &str = "and or not is as in with from";
const TRAILING: &str = "struct interface bool boolean int uint void usize isize u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 char string long short double float byte true false null nil undefined";
const EXTENSIONS: &str = "js mjs cjs jsx ts tsx mts cts py pyi pyx java kt kts scala groovy cs csx vb fs cpp cc cxx c h hpp hh hxx inl ipp rs php phtml go swift rb dart lua sh bash zsh bat cmd ps1 css scss sass less html htm xml xaml vue svelte json jsonl yaml yml toml ini cfg conf md mdx txt sql proto gradle sln csproj lock env svg wasm";
fn contains(list: &str, word: &str) -> bool {
    list.split_whitespace().any(|item| item == word)
}
fn keyword(word: &str) -> bool {
    contains(CODE_ONLY, word) || contains(ENGLISH, word)
}

static CHUNK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9_]+(?:(?:\.|-|::)[A-Za-z0-9_]+)*").expect("constant chunk regex")
});
static EXTENSION_GROUP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.\{([^{}]*)\}").expect("constant extension regex"));

pub(super) fn skipped_parts(query: &str) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut subjects = HashSet::new();
    let mut text = String::new();
    let mut chars = query.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                text.push(if next.is_ascii_alphabetic() {
                    ' '
                } else {
                    next
                });
            }
        } else if c == '[' {
            let mut class = String::new();
            let mut closed = false;
            while let Some(next) = chars.next() {
                if next == '\\' {
                    if let Some(escaped) = chars.next() {
                        class.push(escaped);
                    }
                } else if next == ']' {
                    closed = true;
                    break;
                } else {
                    class.push(next);
                }
            }
            if closed {
                seen.extend(
                    class
                        .split(|c: char| !c.is_ascii_alphabetic())
                        .filter(|s| !s.is_empty())
                        .map(str::to_lowercase),
                );
                text.push(' ');
            } else {
                text.push('[');
                text.push_str(&class);
            }
        } else {
            text.push(c);
        }
    }
    let text = EXTENSION_GROUP.replace_all(&text, |captures: &regex::Captures<'_>| {
        captures[1]
            .split(',')
            .map(|item| format!(".{}", item.trim()))
            .collect::<Vec<_>>()
            .join("|")
    });
    let alternatives: Vec<_> = text.split(['|', ',', '{', '}']).collect();
    let mut shows_code = query.contains([
        '\\', '[', ']', '(', ')', '^', '$', '*', '+', '?', '{', '}', '|',
    ]);
    for alternative in &alternatives {
        let chunks: Vec<_> = CHUNK.find_iter(alternative).collect();
        shows_code |= chunks
            .iter()
            .any(|m| input_parts(m.as_str()).len() > 1 || contains(CODE_ONLY, m.as_str()));
        shows_code |= chunks.windows(2).any(|pair| {
            pair.iter()
                .all(|m| keyword(m.as_str()) && !contains(OPERATORS, m.as_str()))
                && alternative[pair[0].end()..pair[1].start()]
                    .trim()
                    .is_empty()
        });
    }
    for alternative in alternatives {
        for found in CHUNK.find_iter(alternative) {
            let chunk = found.as_str();
            let word = chunk.to_lowercase();
            let parts = input_parts(chunk);
            if parts.len() > 1 {
                if let Some((head, extension)) = chunk
                    .rsplit_once('.')
                    .filter(|(_, ext)| contains(EXTENSIONS, &ext.to_lowercase()))
                {
                    subjects.extend(input_parts(head));
                    seen.insert(extension.to_lowercase());
                } else {
                    subjects.extend(parts);
                }
                continue;
            }
            seen.insert(word.clone());
            let before = &alternative[..found.start()];
            let after = &alternative[found.end()..];
            if before.ends_with('.')
                && contains(EXTENSIONS, &word)
                && after
                    .trim_matches([' ', '*', '\'', '"', ')', '$'])
                    .is_empty()
            {
                continue;
            }
            let previous: Vec<_> = CHUNK.find_iter(before).collect();
            let declaration = (!after.is_empty() && !after.starts_with('('))
                || previous.last().is_some_and(|m| keyword(m.as_str()))
                || (contains(TRAILING, chunk) && !previous.is_empty());
            let member = before.ends_with('.') || before.ends_with("::") || before.ends_with("->");
            if keyword(chunk)
                && (contains(CODE_ONLY, chunk) || shows_code)
                && !member
                && declaration
            {
                continue;
            }
            subjects.insert(word);
        }
    }
    seen.retain(|word| !subjects.contains(word));
    seen
}
