//! Extraction of executable regions from component formats without changing their source
//! coordinates. Each returned buffer has exactly the original byte length and line breaks;
//! non-code bytes are spaces, so tree-sitter ranges continue to point at the outer file.

#[derive(Debug)]
pub(super) struct CompositeSource {
    pub(super) grammar_ext: &'static str,
    pub(super) source: String,
}

struct SourceMasks<'a> {
    original: &'a str,
    sources: Vec<CompositeSource>,
}

impl<'a> SourceMasks<'a> {
    fn new(original: &'a str) -> Self {
        Self {
            original,
            sources: Vec::new(),
        }
    }

    fn copy_code(&mut self, grammar_ext: &str, start: usize, end: usize) {
        if start >= end || end > self.original.len() {
            return;
        }
        // Each executable region gets an independent same-length mask. Combining adjacent
        // blocks makes a line comment or incomplete token in one block leak into the next.
        let mut mask: Vec<u8> = self
            .original
            .bytes()
            .map(|byte| {
                if matches!(byte, b'\n' | b'\r') {
                    byte
                } else {
                    b' '
                }
            })
            .collect();
        mask[start..end].copy_from_slice(&self.original.as_bytes()[start..end]);
        self.sources.push(CompositeSource {
            grammar_ext: if grammar_ext == "ts" { "ts" } else { "js" },
            // Code regions are delimited by tags or full lines, therefore their byte offsets
            // are UTF-8 boundaries. The remaining bytes are ASCII spaces.
            source: String::from_utf8(mask).expect("composite source mask must be UTF-8"),
        });
    }

    fn finish(self) -> Vec<CompositeSource> {
        self.sources
    }
}

/// Return embedded executable regions for supported component extensions. The scanner is
/// deliberately conservative: Vue/Svelte accept only top-level script blocks, Astro accepts
/// executable scripts anywhere in its document, and an unclosed block is never guessed.
pub(super) fn extract_sources(source: &str, extension: &str) -> Vec<CompositeSource> {
    let mut masks = SourceMasks::new(source);
    let body_start = if extension == "astro" {
        match extract_astro_frontmatter(source, &mut masks) {
            AstroFrontmatter::Absent => 0,
            AstroFrontmatter::Verified(body_start) => body_start,
            // A missing closing delimiter makes the rest of the document unverified
            // frontmatter, not markup in which a script-shaped string may be extracted.
            AstroFrontmatter::Unterminated => return masks.finish(),
        }
    } else {
        0
    };
    extract_scripts(source, body_start, extension == "astro", &mut masks);
    masks.finish()
}

enum AstroFrontmatter {
    Absent,
    Verified(usize),
    Unterminated,
}

fn extract_astro_frontmatter(source: &str, masks: &mut SourceMasks<'_>) -> AstroFrontmatter {
    let bytes = source.as_bytes();
    let start = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        3
    } else {
        0
    };
    if !source[start..].starts_with("---") || !is_line_end(bytes.get(start + 3).copied()) {
        return AstroFrontmatter::Absent;
    }
    let content_start = line_end_after(bytes, start).unwrap_or(bytes.len());
    let mut line_start = content_start;
    while line_start < bytes.len() {
        let line_end = bytes[line_start..]
            .iter()
            .position(|byte| matches!(*byte, b'\n' | b'\r'))
            .map(|offset| line_start + offset)
            .unwrap_or(bytes.len());
        if source[line_start..line_end].trim() == "---" {
            masks.copy_code("ts", content_start, line_start);
            return AstroFrontmatter::Verified(
                line_end_after(bytes, line_end).unwrap_or(bytes.len()),
            );
        }
        line_start = line_end_after(bytes, line_end).unwrap_or(bytes.len());
    }
    AstroFrontmatter::Unterminated
}

fn is_line_end(byte: Option<u8>) -> bool {
    matches!(byte, None | Some(b'\n' | b'\r'))
}

fn line_end_after(bytes: &[u8], index: usize) -> Option<usize> {
    match bytes.get(index) {
        Some(b'\n') => Some(index + 1),
        Some(b'\r') if bytes.get(index + 1) == Some(&b'\n') => Some(index + 2),
        Some(b'\r') => Some(index + 1),
        None => None,
        Some(_) => bytes[index..]
            .iter()
            .position(|byte| matches!(*byte, b'\n' | b'\r'))
            .and_then(|offset| line_end_after(bytes, index + offset)),
    }
}

fn extract_scripts(
    source: &str,
    mut index: usize,
    allow_nested_scripts: bool,
    masks: &mut SourceMasks<'_>,
) {
    let bytes = source.as_bytes();
    let mut ancestors: Vec<String> = Vec::new();
    let mut astro_expression_depth = 0usize;
    while index < bytes.len() {
        if allow_nested_scripts {
            if bytes[index] == b'{' {
                astro_expression_depth += 1;
                index += 1;
                continue;
            }
            if astro_expression_depth > 0 {
                if bytes[index..].starts_with(b"//") {
                    index = bytes[index + 2..]
                        .iter()
                        .position(|byte| matches!(*byte, b'\n' | b'\r'))
                        .map(|offset| index + 2 + offset)
                        .unwrap_or(bytes.len());
                    continue;
                }
                if bytes[index..].starts_with(b"/*") {
                    index = bytes[index + 2..]
                        .windows(2)
                        .position(|window| window == b"*/")
                        .map(|offset| index + 4 + offset)
                        .unwrap_or(bytes.len());
                    continue;
                }
                if matches!(bytes[index], b'\'' | b'"' | b'`') {
                    index = quoted_value_end(bytes, index).unwrap_or(bytes.len());
                    continue;
                }
                if bytes[index] == b'}' {
                    astro_expression_depth -= 1;
                    index += 1;
                    continue;
                }
            }
        }
        if bytes[index..].starts_with(b"<!--") {
            index = bytes[index + 4..]
                .windows(3)
                .position(|window| window == b"-->")
                .map(|offset| index + 7 + offset)
                .unwrap_or(bytes.len());
            continue;
        }
        if bytes[index..].starts_with(b"{/*") {
            index = bytes[index + 3..]
                .windows(3)
                .position(|window| window == b"*/}")
                .map(|offset| index + 6 + offset)
                .unwrap_or(bytes.len());
            continue;
        }
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }
        let tag_start = index;
        let Some(tag) = parse_tag(source, tag_start) else {
            index += 1;
            continue;
        };
        index = tag.end;
        if tag.is_closing {
            if ancestors.last().is_some_and(|name| name == &tag.name) {
                ancestors.pop();
            }
            continue;
        }
        let is_script = if allow_nested_scripts {
            // Astro reserves capitalized tags for components. `<Script>` is a component, not
            // the native `<script>` element; lowercasing it would index component child text.
            tag.raw_name == "script"
        } else {
            tag.name == "script"
        };
        if is_script && (allow_nested_scripts || ancestors.is_empty()) {
            let Some(close_start) = find_script_close(source, index, allow_nested_scripts) else {
                // An unclosed tag has no verified boundary; leave it masked.
                return;
            };
            masks.copy_code(script_grammar(tag.attributes), index, close_start);
            let Some(close_tag) = parse_tag(source, close_start) else {
                return;
            };
            index = close_tag.end;
            continue;
        }
        if !tag.is_self_closing && !is_void_element(&tag.name) {
            ancestors.push(tag.name);
        }
    }
}

struct Tag<'a> {
    raw_name: &'a str,
    name: String,
    attributes: &'a str,
    end: usize,
    is_closing: bool,
    is_self_closing: bool,
}

fn parse_tag(source: &str, start: usize) -> Option<Tag<'_>> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'<') {
        return None;
    }
    let mut cursor = start + 1;
    let is_closing = bytes.get(cursor) == Some(&b'/');
    if is_closing {
        cursor += 1;
    }
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    let name_start = cursor;
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
    {
        cursor += 1;
    }
    if cursor == name_start {
        return None;
    }
    let raw_name = &source[name_start..cursor];
    let name = raw_name.to_ascii_lowercase();
    let attributes_start = cursor;
    let end = tag_end(source, cursor)?;
    let inside = &source[attributes_start..end - 1];
    Some(Tag {
        raw_name,
        name,
        attributes: inside,
        end,
        is_closing,
        is_self_closing: inside.trim_end().ends_with('/'),
    })
}

fn tag_end(source: &str, mut cursor: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut quote = None;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        match quote {
            Some(current) if byte == current => quote = None,
            Some(_) => {}
            None if matches!(byte, b'\'' | b'\"') => quote = Some(byte),
            None if byte == b'>' => return Some(cursor + 1),
            None => {}
        }
        cursor += 1;
    }
    None
}

fn find_script_close(source: &str, mut cursor: usize, is_case_sensitive: bool) -> Option<usize> {
    while cursor < source.len() {
        let relative = source[cursor..].find("</")?;
        let candidate = cursor + relative;
        let Some(tag) = parse_tag(source, candidate) else {
            // `</` is ordinary JavaScript text unless it begins a complete closing tag.
            cursor = candidate + 2;
            continue;
        };
        let is_script = if is_case_sensitive {
            tag.raw_name == "script"
        } else {
            tag.name == "script"
        };
        if tag.is_closing && is_script {
            return Some(candidate);
        }
        cursor = tag.end;
    }
    None
}

fn quoted_value_end(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = *bytes.get(start)?;
    let mut cursor = start + 1;
    let mut is_escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if is_escaped {
            is_escaped = false;
        } else if byte == b'\\' {
            is_escaped = true;
        } else if byte == quote {
            return Some(cursor + 1);
        }
        cursor += 1;
    }
    None
}

fn script_grammar(attributes: &str) -> &'static str {
    let mut cursor = 0;
    let bytes = attributes.as_bytes();
    while cursor < bytes.len() {
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let name_start = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
        {
            cursor += 1;
        }
        if name_start == cursor {
            cursor += 1;
            continue;
        }
        let name = attributes[name_start..cursor].to_ascii_lowercase();
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            continue;
        }
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let value_start = cursor;
        let (value_start, value_end) = match bytes.get(cursor) {
            Some(b'\"' | b'\'') => {
                let quote = bytes[cursor];
                cursor += 1;
                let start = cursor;
                while bytes.get(cursor).is_some_and(|byte| *byte != quote) {
                    cursor += 1;
                }
                let end = cursor;
                cursor += usize::from(bytes.get(cursor) == Some(&quote));
                (start, end)
            }
            _ => {
                while bytes
                    .get(cursor)
                    .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'/')
                {
                    cursor += 1;
                }
                (value_start, cursor)
            }
        };
        if name == "lang" {
            let value = attributes[value_start..value_end]
                .trim()
                .to_ascii_lowercase();
            return if matches!(value.as_str(), "ts" | "typescript") {
                "ts"
            } else {
                "js"
            };
        }
    }
    "js"
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::extract_sources;

    #[test]
    fn masks_only_top_level_component_code_without_changing_coordinates() {
        let source = "<template>\n<script>const fake = 'noise';</script>\n</template>\n<script lang=\"ts\">\nconst real = 'kept';\n</script>\n<style>.noise { color: red; }</style>\n";
        let sources = extract_sources(source, "vue");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].grammar_ext, "ts");
        assert_eq!(sources[0].source.len(), source.len());
        assert_eq!(
            sources[0].source.lines().nth(4),
            Some("const real = 'kept';")
        );
        assert!(!sources[0].source.contains("const fake"));
        assert!(!sources[0].source.contains("color: red"));
    }

    #[test]
    fn isolates_blocks_and_preserves_crlf_multibyte_mask_coordinates() {
        let source = "<script>// comment</script><script>function later_symbol() {}</script>\r\n<div>한글</div><script lang=\"ts\">function typed_symbol() {}</script>\r\n";
        let sources = extract_sources(source, "svelte");
        assert_eq!(sources.len(), 3);
        assert_eq!(sources[0].grammar_ext, "js");
        assert_eq!(sources[1].grammar_ext, "js");
        assert_eq!(sources[2].grammar_ext, "ts");
        for masked in sources {
            assert_eq!(masked.source.len(), source.len());
            for (index, byte) in source.bytes().enumerate() {
                if matches!(byte, b'\r' | b'\n') {
                    assert_eq!(masked.source.as_bytes()[index], byte);
                }
            }
        }
    }

    #[test]
    fn accepts_real_scripts_after_markup_quotes_and_nested_astro_scripts() {
        let vue =
            "<template><p>Don't stop</p></template><script>function retained_vue() {}</script>";
        assert!(extract_sources(vue, "vue")[0]
            .source
            .contains("retained_vue"));

        let astro = "<html><body><script>function nested_astro() {}</script></body></html>";
        assert!(extract_sources(astro, "astro")[0]
            .source
            .contains("nested_astro"));
    }

    #[test]
    fn leaves_unterminated_astro_frontmatter_unverified() {
        let source = "---\nconst value = 1;\n<script>function unverified() {}</script>\n";
        assert!(extract_sources(source, "astro").is_empty());
    }

    #[test]
    fn continues_after_non_tag_script_body_close_prefix() {
        let source = "<script>const marker = \"</\"; function retained_symbol() {}</script>";
        assert!(extract_sources(source, "vue")[0]
            .source
            .contains("retained_symbol"));
    }

    #[test]
    fn excludes_astro_expression_strings_and_capitalized_components() {
        let source = "<div>{\"<script>function expression_only_symbol() {}</script>\"}</div>\n<Script>function component_child_text() {}</Script>\n{enabled && <script>function expression_markup_symbol() {}</script>}\n<script>function real_astro_symbol() {}</script>\n";
        let sources = extract_sources(source, "astro");
        assert_eq!(sources.len(), 2);
        let extracted = sources
            .iter()
            .map(|source| source.source.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(extracted.contains("real_astro_symbol"));
        assert!(extracted.contains("expression_markup_symbol"));
        assert!(!extracted.contains("expression_only_symbol"));
        assert!(!extracted.contains("component_child_text"));
    }
}
