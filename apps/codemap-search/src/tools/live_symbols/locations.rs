//! Compact only known, generated location slots; never rewrite source or literal values.

fn compact_slot(text: &str, prefix: &str, path: &str) -> String {
    let needle = format!("{prefix}{path}:");
    let mut output = String::new();
    let mut rest = text;
    while let Some(start) = rest.find(&needle) {
        let after = start + needle.len();
        let digits = rest[after..].bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            output.push_str(&rest[..start]);
            output.push_str(prefix);
            output.push('L');
        } else {
            output.push_str(&rest[..after]);
        }
        rest = &rest[after..];
    }
    output.push_str(rest);
    output
}

/// Caller/callee text contains generated definition/callsite slots, with no source
/// snippets or constant previews. Match the complete path, including its delimiter.
pub(super) fn calls(text: &str, path: &str) -> String {
    compact_slot(&compact_slot(text, " — ", path), "(", path)
}

pub(super) fn reference(text: &str, path: &str) -> String {
    match text.split_once(" = ") {
        Some((location, value)) => format!("{} = {value}", compact_slot(location, " — ", path)),
        None => compact_slot(text, " — ", path),
    }
}
