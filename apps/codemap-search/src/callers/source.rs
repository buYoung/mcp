//! Syntax checks shared by live caller and fallback callee scans.

use std::ops::Range;
use std::path::Path;
use tree_sitter::{Parser, Point, Tree};

use crate::parser::CallSite;

pub(super) struct SourceSyntax {
    tree: Tree,
}

impl SourceSyntax {
    pub(super) fn parse(file_path: &str, source: &[u8]) -> Option<Self> {
        let path = Path::new(file_path);
        let spec = crate::lang::spec_for_path(path)?;
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        let mut parser = Parser::new();
        parser.set_language(&spec.grammar(ext)).ok()?;
        Some(Self {
            tree: parser.parse(source, None)?,
        })
    }

    pub(super) fn is_code(&self, range: Range<usize>) -> bool {
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
            .tree
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
