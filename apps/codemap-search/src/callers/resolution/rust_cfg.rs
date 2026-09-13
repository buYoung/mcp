//! A bounded three-valued subset of Rust cfg, using explicitly supplied target facts.
use tree_sitter::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Condition {
    True,
    False,
    Unknown,
}

impl Condition {
    pub fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::True, Self::True) => Self::True,
            _ => Self::Unknown,
        }
    }
    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::False, Self::False) => Self::False,
            _ => Self::Unknown,
        }
    }
    fn not(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }
}

struct Predicate<'a> {
    rest: &'a str,
    target_os: Option<&'a str>,
}

impl<'a> Predicate<'a> {
    fn whitespace(&mut self) {
        loop {
            self.rest = self.rest.trim_start();
            if self.rest.starts_with("//") {
                self.rest = self.rest.split_once('\n').map_or("", |(_, tail)| tail);
            } else if self.rest.starts_with("/*") {
                // Nested block comments and unusual cfg token shapes remain unknown.
                if let Some((_, tail)) = self.rest.split_once("*/") {
                    self.rest = tail;
                } else {
                    return;
                }
            } else {
                return;
            }
        }
    }
    fn take(&mut self, token: &str) -> bool {
        self.whitespace();
        if let Some(tail) = self.rest.strip_prefix(token) {
            self.rest = tail;
            true
        } else {
            false
        }
    }
    fn name(&mut self) -> Option<&'a str> {
        self.whitespace();
        let end = self
            .rest
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
            .count();
        if end == 0 {
            return None;
        }
        let (name, tail) = self.rest.split_at(end);
        self.rest = tail;
        Some(name)
    }
    fn string(&mut self) -> Option<&'a str> {
        if !self.take("\"") {
            return None;
        }
        let (value, tail) = self.rest.split_once('"')?;
        // Escaped/raw strings are not guessed; ordinary target_os values are sufficient.
        if value.contains(['\\', '\n', '\r']) {
            return None;
        }
        self.rest = tail;
        Some(value)
    }
    fn expression(&mut self, depth: usize) -> Option<Condition> {
        if depth > 32 {
            return None;
        }
        let name = self.name()?;
        if self.take("=") {
            let value = self.string()?;
            return Some(if name == "target_os" {
                self.target_os.map_or(Condition::Unknown, |target| {
                    if target == value {
                        Condition::True
                    } else {
                        Condition::False
                    }
                })
            } else {
                Condition::Unknown
            });
        }
        if self.take("(") {
            let mut items = Vec::new();
            if !self.take(")") {
                loop {
                    items.push(self.expression(depth + 1)?);
                    if self.take(")") {
                        break;
                    }
                    if !self.take(",") {
                        return None;
                    }
                    if self.take(")") {
                        break;
                    }
                }
            }
            return Some(match name {
                "all" => items.into_iter().fold(Condition::True, Condition::and),
                "any" => items.into_iter().fold(Condition::False, Condition::or),
                "not" if items.len() == 1 => items[0].not(),
                _ => Condition::Unknown,
            });
        }
        Some(match name {
            "true" => Condition::True,
            "false" => Condition::False,
            _ => Condition::Unknown,
        })
    }
}

fn attribute(text: &str, target_os: Option<&str>) -> Condition {
    let Some(body) = text
        .trim()
        .strip_prefix('#')
        .map(|s| s.trim_start_matches('!').trim_start())
        .and_then(|s| s.strip_prefix('['))
        .and_then(|s| s.strip_suffix(']'))
    else {
        return Condition::Unknown;
    };
    let mut parser = Predicate {
        rest: body,
        target_os,
    };
    match parser.name() {
        Some("cfg_attr") => Condition::Unknown,
        Some("cfg") => {
            // TestCodeFilter owns test selection; this does not infer a Cargo test target.
            if parser.rest.trim() == "(test)" {
                return Condition::True;
            }
            if !parser.take("(") {
                return Condition::Unknown;
            }
            let Some(result) = parser.expression(0) else {
                return Condition::Unknown;
            };
            parser.take(",");
            if !parser.take(")") {
                return Condition::Unknown;
            }
            parser.whitespace();
            if parser.rest.is_empty() {
                result
            } else {
                Condition::Unknown
            }
        }
        _ => Condition::True,
    }
}

pub(super) fn condition(mut node: Node<'_>, source: &str, target_os: Option<&str>) -> Condition {
    let mut state = Condition::True;
    loop {
        let mut previous = node.prev_named_sibling();
        while let Some(item) = previous {
            if item.kind().contains("comment") {
                previous = item.prev_named_sibling();
                continue;
            }
            if item.kind() != "attribute_item" {
                break;
            }
            state = state.and(
                item.utf8_text(source.as_bytes())
                    .map_or(Condition::Unknown, |text| attribute(text, target_os)),
            );
            previous = item.prev_named_sibling();
        }
        if matches!(node.kind(), "source_file" | "declaration_list" | "block") {
            let mut cursor = node.walk();
            for item in node
                .named_children(&mut cursor)
                .filter(|child| child.kind() == "inner_attribute_item")
            {
                state = state.and(
                    item.utf8_text(source.as_bytes())
                        .map_or(Condition::Unknown, |text| attribute(text, target_os)),
                );
            }
        }
        let Some(parent) = node.parent() else { break };
        node = parent;
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_os_and_boolean_cfg_keep_unknown_facts_unknown() {
        for (input, os, expected) in [
            (
                "#[cfg(target_os = \"macos\")]",
                Some("macos"),
                Condition::True,
            ),
            ("#[cfg(target_os = \"macos\")]", None, Condition::Unknown),
            (
                "#[cfg(not(target_os = \"macos\"))]",
                Some("linux"),
                Condition::True,
            ),
            (
                "#[cfg(all(target_os=\"macos\", not(target_os=\"windows\"),))]",
                Some("macos"),
                Condition::True,
            ),
            (
                "#[cfg(any(target_os=\"macos\", feature=\"ui\"))]",
                Some("macos"),
                Condition::True,
            ),
            (
                "#[cfg(all(target_os=\"windows\", feature=\"ui\"))]",
                Some("macos"),
                Condition::False,
            ),
            ("#[cfg(feature=\"ui\")]", Some("macos"), Condition::Unknown),
            ("#[cfg(any())]", None, Condition::False),
            ("#[cfg(all())]", None, Condition::True),
            ("#[cfg(not(true, false))]", None, Condition::Unknown),
            (
                "#[cfg_attr(feature=\"ui\", path=\"other.rs\")]",
                Some("macos"),
                Condition::Unknown,
            ),
            (
                "#[cfg(target_os=r\"macos\")]",
                Some("macos"),
                Condition::Unknown,
            ),
        ] {
            assert_eq!(attribute(input, os), expected, "{input}");
        }
    }
}
