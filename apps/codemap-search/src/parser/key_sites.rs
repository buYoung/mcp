//! Upper-snake-case constant names per line, for the read trailer that lists other code using
//! the same keys. A lexical scan of any language: comment and import lines are skipped and no
//! name is resolved, so equal names in unrelated scopes still count as one key.

use super::KeySite;

/// Sites kept per file; generated or table-like files stay bounded.
const MAX_SITES_PER_FILE: usize = 4096;

pub(crate) fn constants(source: &str) -> Vec<KeySite> {
    let mut sites = Vec::new();
    for (index, line) in source.lines().enumerate() {
        if is_skipped_line(line.trim_start()) {
            continue;
        }
        let mut seen_on_line: Vec<&str> = Vec::new();
        for token in line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if is_constant_name(token) && !seen_on_line.contains(&token) {
                seen_on_line.push(token);
                sites.push(KeySite {
                    name: token.to_string(),
                    line: index + 1,
                });
                if sites.len() == MAX_SITES_PER_FILE {
                    return sites;
                }
            }
        }
    }
    sites
}

fn is_skipped_line(line: &str) -> bool {
    [
        "//", "/*", "*", "--", "# ", "#!", "<!--", "import ", "from ", "use ", "#include",
        "export {",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
}

/// `SOME_NAME`: upper-case letters and digits in at least two `_`-separated parts.
fn is_constant_name(token: &str) -> bool {
    token.len() >= 4
        && token.starts_with(|c: char| c.is_ascii_uppercase())
        && !token.ends_with('_')
        && !token.contains("__")
        && token.contains('_')
        && token
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && token.chars().filter(|c| c.is_ascii_uppercase()).count() >= 2
}
