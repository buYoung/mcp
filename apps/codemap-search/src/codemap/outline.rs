use crate::declarations;
use crate::parser::{ExtractedFile, ExtractedSymbol};
use std::path::Path;

/// Producer-owned rows let Jev remove declarations without reparsing Markdown or
/// rereading changed files after the asynchronous judgment.
#[derive(Debug, Clone)]
pub(crate) struct DeclarationOutline {
    rows: Vec<(Option<ExtractedSymbol>, String)>,
}

impl DeclarationOutline {
    pub(crate) fn retain(&mut self, is_retained: impl Fn(&ExtractedSymbol) -> bool) {
        self.rows
            .retain(|(symbol, _)| symbol.as_ref().is_none_or(&is_retained));
    }
}

impl std::fmt::Display for DeclarationOutline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (_, text) in &self.rows {
            f.write_str(text)?;
        }
        Ok(())
    }
}

/// Enrich only the folder's immediate files. Never combine stale symbol spans
/// with changed source, nor run source extraction in call-target resolution.
pub(super) fn render(file: &ExtractedFile) -> Option<DeclarationOutline> {
    let path = Path::new(&file.file_path);
    let spec = crate::lang::spec_for_path(path)?;
    let source = std::fs::read_to_string(path).ok()?;
    let indexed = file.navigation.as_ref()?.implementations.as_ref()?;
    if indexed.source_digest != crate::implementations::digest(source.as_bytes()) {
        let mut rows = Vec::new();
        for symbol in super::summary::significant_symbols(&file.symbols) {
            rows.push((
                Some(symbol.clone()),
                format!("  - {} ({})\n", symbol.name, symbol.kind),
            ));
        }
        rows.push((
            None,
            "  - [Source changed since indexing; signatures unavailable.]\n".into(),
        ));
        return Some(DeclarationOutline { rows });
    }
    let extension = path.extension()?.to_str()?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&spec.grammar(extension)).ok()?;
    let tree = crate::parser::parse_source(&mut parser, source.as_bytes()).ok()?;
    let mut file = file.clone();
    file.symbols = super::summary::significant_symbols(&file.symbols)
        .cloned()
        .collect();
    if extension == "rs" {
        declarations::add_impl_containers(&mut file, &tree, &source);
    }
    let parents = declarations::parents(&file.symbols);
    let mut children = vec![Vec::new(); file.symbols.len() + 1];
    for (i, parent) in parents.iter().enumerate() {
        children[parent.unwrap_or(file.symbols.len())].push(i);
    }
    for siblings in &mut children {
        siblings.sort_by_key(|&i| {
            (
                file.symbols[i].range.start_line,
                file.symbols[i].range.start_col,
            )
        });
    }
    let mut pending: Vec<_> = children[file.symbols.len()]
        .iter()
        .rev()
        .map(|&i| (i, 1))
        .collect();
    let mut rows = Vec::new();
    while let Some((i, depth)) = pending.pop() {
        let symbol = &file.symbols[i];
        let signature = declarations::folder_signature(symbol, &tree, &source);
        let signature = if signature.chars().count() > 400 {
            format!(
                "{}… [signature shortened]",
                signature.chars().take(400).collect::<String>()
            )
        } else {
            signature
        };
        rows.push((
            Some(symbol.clone()),
            format!("{}- {signature}\n", "  ".repeat(depth)),
        ));
        pending.extend(children[i].iter().rev().map(|&child| (child, depth + 1)));
    }
    Some(DeclarationOutline { rows })
}
