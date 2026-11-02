//! Syntax checks shared by live caller and fallback callee scans.

use std::ops::Range;
use std::path::Path;
use tree_sitter::{Parser, Point, Tree};

use crate::parser::{CallSite, CodeRange};

pub(super) struct SourceSyntax {
    pub(super) tree: Tree,
    embedded: Option<Vec<(Range<usize>, SourceSyntax)>>,
}

impl SourceSyntax {
    pub(super) fn parse(file_path: &str, source: &[u8]) -> Option<Self> {
        let path = Path::new(file_path);
        let spec = crate::lang::spec_for_path(path)?;
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        let mut parser = Parser::new();
        parser.set_language(&spec.grammar(ext)).ok()?;
        let embedded = crate::lang::is_composite_extension(ext).then(|| {
            crate::parser::composite::extract_sources(
                std::str::from_utf8(source).unwrap_or(""),
                ext,
            )
            .into_iter()
            .filter(|part| matches!(part.grammar_ext, "js" | "jsx" | "ts" | "tsx"))
            .filter_map(|part| {
                Self::parse(
                    &format!("embedded.{}", part.grammar_ext),
                    part.source.as_bytes(),
                )
                .map(|syntax| (part.range, syntax))
            })
            .collect()
        });
        Some(Self {
            tree: crate::parser::parse_source(&mut parser, source).ok()?,
            embedded,
        })
    }

    pub(super) fn tree_for_range(&self, range: &CodeRange) -> Option<&Tree> {
        let Some(embedded) = &self.embedded else {
            return Some(&self.tree);
        };
        let start = Point::new(
            range.start_line.checked_sub(1)?,
            range.start_col.checked_sub(1)?,
        );
        let end = Point::new(
            range.end_line.checked_sub(1)?,
            range.end_col.checked_sub(1)?,
        );
        embedded.iter().find_map(|(span, syntax)| {
            let node = syntax
                .tree
                .root_node()
                .descendant_for_point_range(start, end)?;
            (span.start <= node.start_byte() && node.end_byte() <= span.end).then_some(&syntax.tree)
        })
    }

    pub(super) fn is_code(&self, range: Range<usize>) -> bool {
        if let Some(embedded) = &self.embedded {
            return embedded.iter().any(|(span, syntax)| {
                span.start <= range.start && range.end <= span.end && syntax.is_code(range.clone())
            });
        }
        let mut current = self
            .tree
            .root_node()
            .descendant_for_byte_range(range.start, range.end);
        while let Some(node) = current {
            let kind = node.kind();
            // Expressions inside interpolated strings are code. Check inner literals
            // and comments first as we walk outwards, so nested text stays excluded.
            if matches!(
                kind,
                "interpolation"
                    | "string_interpolation"
                    | "template_substitution"
                    | "interpolated_expression"
                    | "embedded_expression"
                    | "command_substitution"
                    | "simple_expansion"
                    | "expansion"
                    | "sub_expression"
            ) {
                return true;
            }
            if kind.contains("comment")
                || kind.contains("string")
                || kind.contains("regex")
                || matches!(
                    kind,
                    "char_literal"
                        | "character_literal"
                        | "heredoc_body"
                        | "literal"
                        | "jsx_text"
                        | "html_text"
                )
            {
                return false;
            }
            current = node.parent();
        }
        true
    }

    pub(super) fn is_member_access(&self, range: Range<usize>) -> bool {
        if let Some(embedded) = &self.embedded {
            return embedded.iter().any(|(span, syntax)| {
                span.start <= range.start
                    && range.end <= span.end
                    && syntax.is_member_access(range.clone())
            });
        }
        let mut current = self
            .tree
            .root_node()
            .descendant_for_byte_range(range.start, range.end);
        while let Some(node) = current {
            if matches!(
                node.kind(),
                "field_expression" | "member_expression" | "attribute" | "selector_expression"
            ) {
                return ["field", "property", "attribute"].iter().any(|field| {
                    node.child_by_field_name(field).is_some_and(|member| {
                        member.start_byte() <= range.start && range.end <= member.end_byte()
                    })
                });
            }
            current = node.parent();
        }
        false
    }

    /// In Rust/C-family syntax a scoped module call differs from an object member
    /// call, even though both can carry a navigation receiver string.
    pub(super) fn is_member_call(&self, call: &CallSite) -> Option<bool> {
        let start = Point::new(
            call.range.start_line.checked_sub(1)?,
            call.range.start_col.checked_sub(1)?,
        );
        let end = Point::new(
            call.range.end_line.checked_sub(1)?,
            call.range.end_col.checked_sub(1)?,
        );
        let node = self
            .tree_for_range(&call.range)?
            .root_node()
            .descendant_for_point_range(start, end)?;
        if node.kind() != "call_expression" {
            return None;
        }
        let mut function = node.child_by_field_name("function")?;
        while matches!(
            function.kind(),
            "generic_function" | "parenthesized_expression"
        ) {
            function = function
                .child_by_field_name("function")
                .or_else(|| function.named_child(0))?;
        }
        Some(matches!(
            function.kind(),
            "field_expression" | "member_expression" | "attribute" | "selector_expression"
        ))
    }
}

/// The lines of `source` that use each of `patterns` in code, sorted. A pattern is a name or a
/// qualified path such as `module.name` or `module::name`. It must stand alone rather than
/// inside a longer identifier, a plain name must not follow `.` as another value's member, and
/// the match must lie outside comments and strings; a shell `$NAME` or an assembly `$NAME`
/// operand uses `NAME`. Text the grammar cannot parse is matched by spelling alone.
pub(crate) fn code_use_lines(file_path: &str, source: &str, patterns: &[String]) -> Vec<Vec<usize>> {
    let language = crate::lang::spec_for_path(Path::new(file_path))
        .map(|spec| spec.language_name())
        .unwrap_or_default();
    let is_sigil = matches!(language, "bash" | "zsh" | "powershell" | "asm");
    // Shell words, function names among them, may contain hyphens.
    let is_shell = matches!(language, "bash" | "zsh");
    let is_name_char = |c: char| {
        c.is_alphanumeric() || c == '_' || (c == '$' && !is_sigil) || (c == '-' && is_shell)
    };
    let mut found: Vec<Vec<usize>> = vec![Vec::new(); patterns.len()];
    let mut syntax: Option<Option<SourceSyntax>> = None;
    let mut line_start = 0;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        for (pattern_index, pattern) in patterns.iter().enumerate() {
            if pattern.is_empty() {
                continue;
            }
            let is_path = pattern.contains('.') || pattern.contains("::");
            let mut from = 0;
            while let Some(offset) = line[from..].find(pattern.as_str()) {
                let at = from + offset;
                from = at + pattern.len();
                let before = line[..at].chars().next_back();
                let after = line[from..].chars().next();
                if before.is_some_and(|c| is_name_char(c) || (c == '.' && !is_path))
                    || after.is_some_and(is_name_char)
                {
                    continue;
                }
                let start = line_start + at;
                let is_code = syntax
                    .get_or_insert_with(|| SourceSyntax::parse(file_path, source.as_bytes()))
                    .as_ref()
                    .is_none_or(|syntax| syntax.is_code(start..start + pattern.len()));
                if is_code {
                    found[pattern_index].push(index + 1);
                    break;
                }
            }
        }
        line_start += line.len();
    }
    found
}
