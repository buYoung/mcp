//! Optional byte-aligned parser views; selecting a view is not a build proof.
pub(crate) fn project(source: &str, language: &str) -> String {
    let mut output = Vec::new();
    let lines: Vec<_> = source.split_inclusive('\n').collect();
    let mut frames = Vec::new();
    let mut is_active = true;
    let pattern = regex::Regex::new(r"^\s*#\s*(if|ifdef|ifndef|elif|elseif|else|endif)\b").unwrap();
    for (index, line) in lines.iter().enumerate() {
        let directive = pattern.captures(line).map(|m| m[1].to_owned());
        if language != "swift" {
            if let Some(directive) = &directive {
                match directive.as_str() {
                    "if" | "ifdef" | "ifndef" => {
                        let guard = if directive == "ifndef" {
                            line.split_whitespace()
                                .last()
                                .filter(|name| {
                                    lines[index + 1..lines.len().min(index + 5)]
                                        .iter()
                                        .any(|line| line.contains(&format!("define {name}")))
                                })
                                .is_some()
                        } else {
                            false
                        };
                        frames.push((is_active, guard));
                        is_active = is_active && guard;
                    }
                    "else" | "elif" => {
                        if let Some((parent, first)) = frames.last() {
                            is_active = *parent && !*first && directive == "else";
                        }
                    }
                    "endif" => {
                        if let Some((parent, _)) = frames.pop() {
                            is_active = parent;
                        }
                    }
                    _ => {}
                }
            }
        }
        if directive.is_some() || !is_active {
            output.extend(
                line.bytes()
                    .map(|b| if matches!(b, b'\n' | b'\r') { b } else { b' ' }),
            );
        } else {
            output.extend(line.bytes());
        }
    }
    String::from_utf8(output).expect("ASCII byte masking preserves UTF-8")
}
