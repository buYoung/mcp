//! Callable bounds are resolved from the exact live buffer used for the result.
use crate::parser::{CodeExtractor, TreeSitterExtractor};
use std::path::Path;

#[derive(Clone, Debug)]
pub(crate) struct CallableBounds {
    pub symbol_index: usize,
    pub start: usize,
    pub end: usize,
}

pub(crate) fn input_byte_cap() -> usize {
    crate::config::get().max_file_size.min(4 * 1024 * 1024) as usize
}

pub(crate) fn bounds(path: &Path, source: &str) -> Result<Vec<CallableBounds>, String> {
    capture(path, source, false).map(|capture| capture.bounds)
}

pub(crate) struct CapturedSource {
    pub file: crate::parser::ExtractedFile,
    pub bounds: Vec<CallableBounds>,
    pub reference_gaps: Vec<crate::parser::CodeRange>,
    pub self_member_uses: Vec<super::references::SelfMemberUse>,
    pub declaration_headers: Vec<Option<(usize, usize)>>,
}

/// Parse the exact source buffer once for both declaration identity and safe body bounds.
pub(crate) fn capture(
    path: &Path,
    source: &str,
    should_capture_context: bool,
) -> Result<CapturedSource, String> {
    if source.len() > input_byte_cap() {
        return Err(format!(
            "live parsing input exceeds {} bytes",
            input_byte_cap()
        ));
    }
    let spec = crate::lang::spec_for_path(path).ok_or("no source parser for this file")?;
    let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
    if crate::lang::is_composite_extension(ext) {
        return Err("callable expansion is unavailable for composite source files".into());
    }
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&spec.grammar(ext))
        .map_err(|e| e.to_string())?;
    let tree = crate::parser::parse_source(&mut parser, source.as_bytes())?;
    let mut file = TreeSitterExtractor::new().extract(source, &path.to_string_lossy())?;
    let mut reference_gaps = Vec::new();
    let mut self_member_uses = Vec::new();
    if should_capture_context {
        let dependencies =
            super::references::capture_dependencies(&tree, source, spec.language_name());
        reference_gaps = dependencies.gaps;
        self_member_uses = dependencies.self_member_uses;
        if let Some(navigation) = &mut file.navigation {
            // These observations belong to this immutable request buffer, not the index.
            let mut seen: std::collections::HashSet<_> = navigation
                .references
                .iter()
                .map(|r| (r.name.clone(), r.range.start_line, r.range.start_col))
                .collect();
            navigation.references.extend(
                dependencies.references.into_iter().filter(|r| {
                    seen.insert((r.name.clone(), r.range.start_line, r.range.start_col))
                }),
            );
        }
    }
    // Keep source ranges rather than inferred signatures. These identify parameter
    // types and member contracts without copying every sibling implementation.
    let declaration_headers = if should_capture_context {
        file.symbols
            .iter()
            .map(|symbol| {
                let node = super::structure::symbol_node(&tree, symbol, source)?;
                if node.has_error() || node.is_missing() {
                    return None;
                }
                let end = node
                    .child_by_field_name("body")
                    .map_or(symbol.range.end_line_inclusive(), |body| {
                        body.start_position().row + 1
                    });
                Some((symbol.range.start_line, end))
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut result = Vec::new();
    for (symbol_index, symbol) in file
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, s)| super::structure::callable(s))
    {
        let Some(node) = super::structure::symbol_node(&tree, symbol, source) else {
            continue;
        };
        if node.has_error() || node.is_missing() || node.child_by_field_name("body").is_none() {
            continue;
        }
        // The navigation extractor records body bindings but some language queries do
        // not record parameters. Live context must distinguish a parameter's value
        // operations from calls through an unavailable global/service receiver.
        if let (true, Some(parameters), Some(navigation)) = (
            should_capture_context,
            node.child_by_field_name("parameters"),
            file.navigation.as_mut(),
        ) {
            let mut cursor = parameters.walk();
            for parameter in parameters.named_children(&mut cursor) {
                for binding in crate::parser::local_bindings_from_node(parameter, source.as_bytes())
                {
                    if !navigation
                        .local_bindings
                        .iter()
                        .any(|existing| existing == &binding)
                    {
                        navigation.local_bindings.push(binding);
                    }
                }
            }
        }
        let mut ancestor = node.parent();
        let mut has_error_ancestor = false;
        while let Some(parent) = ancestor {
            has_error_ancestor |= parent.is_error() || parent.is_missing();
            ancestor = parent.parent();
        }
        if has_error_ancestor {
            continue;
        }
        let mut start = symbol.range.start_line;
        let mut outer = node;
        if let Some(parent) = node
            .parent()
            .filter(|p| matches!(p.kind(), "decorated_definition" | "export_statement"))
        {
            start = parent.start_position().row + 1;
            outer = parent;
        }
        while let Some(previous) = outer.prev_named_sibling().filter(|p| {
            matches!(
                p.kind(),
                "attribute_item" | "attribute" | "annotation" | "marker_annotation"
            ) && source[p.end_byte()..outer.start_byte()].trim().is_empty()
        }) {
            start = previous.start_position().row + 1;
            outer = previous;
        }
        // Line-oriented output cannot isolate a callable sharing its boundary
        // line with an unrelated declaration or containing statement.
        let line_start = source[..outer.start_byte()]
            .rfind('\n')
            .map_or(0, |i| i + 1);
        let tail = source[node.end_byte()..]
            .split('\n')
            .next()
            .unwrap_or("")
            .trim();
        if !source[line_start..outer.start_byte()].trim().is_empty()
            || !(tail.is_empty() || tail == ";" || tail.starts_with("//") || tail.starts_with('#'))
        {
            continue;
        }
        result.push(CallableBounds {
            symbol_index,
            start,
            end: symbol.range.end_line_inclusive(),
        });
    }
    result.sort_by_key(|r| (r.start, r.end));
    Ok(CapturedSource {
        file,
        bounds: result,
        reference_gaps,
        self_member_uses,
        declaration_headers,
    })
}

pub(crate) fn containing(
    ranges: &[CallableBounds],
    line: usize,
) -> Result<&CallableBounds, String> {
    let mut matches: Vec<_> = ranges
        .iter()
        .filter(|r| r.start <= line && line <= r.end)
        .collect();
    matches.sort_by_key(|r| r.end - r.start);
    let first = matches
        .first()
        .ok_or("no supported named callable body contains this line")?;
    if matches
        .get(1)
        .is_some_and(|other| other.end - other.start == first.end - first.start)
    {
        return Err(
            "multiple callable boundaries share the anchor line; use a narrower source location"
                .into(),
        );
    }
    Ok(first)
}
