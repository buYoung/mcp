use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::parser::CodeRange;

pub(super) struct SourceMap {
    pub text: String,
    pub inputs: BTreeSet<PathBuf>,
    lines: Vec<Option<usize>>,
}

impl SourceMap {
    pub(super) fn prepare_nasm_declarations(&mut self) -> Result<(), String> {
        let mut text = String::new();
        let mut lines = Vec::new();
        let mut emit = |name: &str, is_global: bool, origin: Option<usize>| -> Result<(), String> {
            if name.starts_with("..@") {
                return Ok(()); // NASM's generated macro-local names are not stable source names.
            }
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.'))
            {
                return Err(format!(
                    "NASM declaration name cannot be represented by the source grammar: {name}"
                ));
            }
            if is_global {
                text.push_str(".globl ");
            }
            text.push_str(name);
            text.push_str(if is_global { "\n" } else { ":\n" });
            lines.push(origin);
            Ok(())
        };
        for (index, line) in self.text.lines().enumerate() {
            let code = line.trim();
            if let Some(directive) = code
                .strip_prefix('[')
                .and_then(|code| code.strip_suffix(']'))
            {
                if let Some((kind, names)) = directive.split_once(char::is_whitespace) {
                    if kind.eq_ignore_ascii_case("global") {
                        for name in names.split(',') {
                            let name = name.trim().split([':', ' ', '\t']).next().unwrap_or("");
                            emit(name, true, self.lines[index])?;
                        }
                    }
                }
            } else if let Some((name, _)) = code.split_once(':') {
                let name = name.trim_end();
                if !name.chars().any(char::is_whitespace) {
                    emit(name, false, self.lines[index])?;
                }
            }
        }
        // NASM has already assembled and validated these listing rows. Its canonical
        // directives/operands exceed the generic ASM grammar; project only declarations.
        self.text = text;
        self.lines = lines;
        Ok(())
    }

    pub(super) fn prepare_assembly(&mut self, is_gas: bool, is_arm: bool) {
        let mut text = String::new();
        let mut lines = Vec::new();
        for (index, line) in self.text.lines().enumerate() {
            let mut starts = vec![0];
            let mut quote = None;
            let mut is_escaped = false;
            if is_gas {
                for (offset, ch) in line.char_indices() {
                    if is_escaped {
                        is_escaped = false;
                        continue;
                    }
                    if ch == '\\' {
                        is_escaped = true;
                        continue;
                    }
                    if let Some(delimiter) = quote {
                        if ch == delimiter {
                            quote = None;
                        }
                        continue;
                    }
                    if matches!(ch, '\'' | '"') {
                        quote = Some(ch);
                    } else if (!is_arm && ch == '#') || line[offset..].starts_with("//") {
                        break;
                    } else if ch == ';' {
                        starts.push(offset + 1);
                    }
                }
            }
            for (part, start) in starts.iter().enumerate() {
                let end = starts.get(part + 1).map_or(line.len(), |next| next - 1);
                let code = &line[*start..end];
                // Leading blank/comment-only slots provoke an empty instruction in
                // this grammar. Remove blank slots and rebuild the parallel line map.
                if code.trim().is_empty() {
                    continue;
                }
                text.push_str(code);
                text.push('\n');
                lines.push(self.lines[index]);
            }
        }
        self.text = text;
        self.lines = lines;
    }

    pub(super) fn active_lines(&self) -> Vec<[usize; 2]> {
        let mut lines: Vec<_> = self.lines.iter().flatten().copied().collect();
        lines.sort_unstable();
        lines.dedup();
        let mut ranges: Vec<[usize; 2]> = Vec::new();
        for line in lines {
            if let Some(last) = ranges
                .last_mut()
                .filter(|last| last[1].saturating_add(1) == line)
            {
                last[1] = line;
            } else {
                ranges.push([line, line]);
            }
        }
        ranges
    }
    pub(super) fn parse(output: &str, target: &Path, directory: &Path) -> Result<Self, String> {
        let target = crate::workspace::canonicalize_path_lenient(&directory.join(target));
        let mut result = Self {
            text: String::new(),
            inputs: BTreeSet::new(),
            lines: Vec::new(),
        };
        let mut current = None;
        let mut line_number = 0usize;
        let mut increment = 1usize;
        let mut has_target_marker = false;
        for line in output.lines() {
            if let Some((number, step, file)) = marker(line)? {
                line_number = number;
                increment = step;
                current = if file.starts_with('<') {
                    None
                } else {
                    let path = crate::workspace::canonicalize_path_lenient(&directory.join(file));
                    result.inputs.insert(path.clone());
                    Some(path)
                };
                result.lines.push(None);
                has_target_marker |= current.as_ref() == Some(&target);
            } else {
                let is_target = current.as_deref() == Some(target.as_path());
                // Header declarations and macro definition bodies do not become source
                // declarations in the including file. Keep line slots for source mapping.
                if is_target
                    && !line.trim().is_empty()
                    && !line.trim_start().starts_with('#')
                    && !line.trim_start().starts_with('%')
                {
                    result.text.push_str(line);
                    result.lines.push(Some(line_number));
                } else {
                    result.lines.push(None);
                }
                line_number = line_number.saturating_add(increment);
            }
            result.text.push('\n');
        }
        if !has_target_marker {
            return Err("preprocessor supplied no original-file line mapping".into());
        }
        Ok(result)
    }

    pub(super) fn nasm_listing(
        listing: &str,
        preprocessed: &str,
        target: &Path,
        directory: &Path,
    ) -> Result<Self, String> {
        let quoted_target =
            serde_json::to_string(&target.display().to_string()).map_err(|e| e.to_string())?;
        let mut normalized = format!("# 1 {quoted_target}\n");
        let mut root_line = None;
        let mut repeat_lines = Vec::new();
        let mut processed = 0;
        for row in listing.lines() {
            // NASM listing.c places source at LIST_INDENT=40. Unknown layouts
            // cannot be used as evidence of a physical source location.
            let Some(prefix) = row.get(..40) else {
                continue;
            };
            let Some(source) = row.get(40..) else {
                continue;
            };
            let Some(number) = prefix
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<usize>().ok())
            else {
                continue;
            };
            let depth = prefix
                .split_once('<')
                .and_then(|(_, rest)| rest.split_once('>'))
                .and_then(|(value, _)| value.parse::<usize>().ok())
                .unwrap_or(0);
            let text = source.trim_start();
            if depth == 0 {
                let directive = text
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if directive == "%rep" {
                    repeat_lines.push(number);
                }
                root_line = if directive == "%include" {
                    None
                } else if directive == "%endrep" {
                    repeat_lines.pop()
                } else {
                    Some(number)
                };
            }
            let Some(code) = text.strip_prefix(";;;").map(str::trim_start) else {
                continue;
            };
            if code.starts_with("[macro]") {
                continue;
            }
            if let Some(line) = root_line {
                normalized.push_str(&format!("# {line} {quoted_target}\n{code}\n"));
                processed += 1;
            }
        }
        if processed == 0
            && preprocessed
                .lines()
                .any(|line| !line.trim().is_empty() && !line.starts_with("%line"))
        {
            return Err("NASM listing did not expose verifiable expanded source".into());
        }
        let mut map = Self::parse(&normalized, target, directory)?;
        for line in preprocessed.lines() {
            if let Some((_, _, file)) = marker(line)? {
                if !file.starts_with('<') {
                    map.inputs
                        .insert(crate::workspace::canonicalize_path_lenient(
                            &directory.join(file),
                        ));
                }
            }
        }
        Ok(map)
    }

    pub(super) fn original_range(
        &self,
        range: &CodeRange,
        source_lines: &[&str],
    ) -> Option<CodeRange> {
        let first = *self.lines.get(range.start_line.checked_sub(1)?)?.as_ref()?;
        let last = *self
            .lines
            .get(range.end_line_inclusive().checked_sub(1)?)?
            .as_ref()?;
        if first == 0 || last < first || last > source_lines.len() {
            return None;
        }
        // Preprocessor output columns describe expanded tokens, not spelling columns.
        // Only original line positions are asserted; enclose the whole source lines.
        Some(CodeRange {
            start_line: first,
            start_col: 1,
            end_line: last,
            end_col: source_lines[last - 1].len() + 1,
        })
    }
}

fn marker(line: &str) -> Result<Option<(usize, usize, String)>, String> {
    if let Some(rest) = line.strip_prefix("%line ") {
        let (position, file) = rest
            .split_once(char::is_whitespace)
            .ok_or("invalid NASM line marker")?;
        let (number, step) = position.split_once('+').unwrap_or((position, "1"));
        return Ok(Some((
            number.parse().map_err(|_| "invalid NASM line number")?,
            step.parse().map_err(|_| "invalid NASM line increment")?,
            file.trim().to_string(),
        )));
    }
    let Some(rest) = line.strip_prefix('#').map(str::trim_start) else {
        return Ok(None);
    };
    let Some((number, rest)) = rest.split_once(char::is_whitespace) else {
        return Ok(None);
    };
    let Ok(number) = number.parse() else {
        return Ok(None);
    };
    let rest = rest.trim_start();
    if !rest.starts_with('"') {
        return Err("invalid compiler line marker".into());
    }
    let mut escaped = false;
    let end = rest
        .char_indices()
        .skip(1)
        .find_map(|(index, ch)| {
            if escaped {
                escaped = false;
                None
            } else if ch == '\\' {
                escaped = true;
                None
            } else {
                (ch == '"').then_some(index)
            }
        })
        .ok_or("unfinished compiler line marker")?;
    let file = serde_json::from_str(&rest[..=end])
        .map_err(|_| "unsupported filename escaping in line marker")?;
    Ok(Some((number, 1, file)))
}

pub(super) fn has_explicit_line_directive(source: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim_start();
        line.strip_prefix('#').is_some_and(|rest| {
            let rest = rest.trim_start();
            rest.starts_with("line ")
                || rest.starts_with("line\t")
                || rest.starts_with(|ch: char| ch.is_ascii_digit())
        }) || line.starts_with("%line ")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_markers_keep_headers_out_and_map_macro_lines() {
        let target = Path::new("/tmp/source.c");
        let map = SourceMap::parse(
            "# 1 \"/tmp/header.h\" 1\nint hidden();\n# 4 \"/tmp/source.c\" 2\nint expanded() {}\n",
            target,
            Path::new("/tmp"),
        )
        .unwrap();
        assert!(!map.text.contains("hidden"));
        let range = CodeRange {
            start_line: 4,
            start_col: 1,
            end_line: 4,
            end_col: 18,
        };
        assert_eq!(
            map.original_range(&range, &["", "", "", "DECLARE(expanded)"])
                .unwrap()
                .start_line,
            4
        );
        let map = SourceMap::parse(
            "%line 3+0 /tmp/source.c\nfirst:\nsecond:\n",
            target,
            Path::new("/tmp"),
        )
        .unwrap();
        assert_eq!(map.lines, [None, Some(3), Some(3)]);
    }
}
