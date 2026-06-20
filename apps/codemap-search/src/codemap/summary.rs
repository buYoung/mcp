#[derive(Debug, Clone)]
pub struct ExtractedSymbolSummary<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub is_exported: bool,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone)]
pub struct ExtractedFileSummary<'a> {
    pub file_path: String,
    pub total_lines: usize,
    pub symbol_count: usize,
    pub symbols: Vec<ExtractedSymbolSummary<'a>>,
}

/// One directory node in the root overview: its path plus the number of source
/// files and significant symbols anywhere beneath it (recursive aggregate). The
/// root view shows this skeleton instead of a per-file dump so a large repo stays
/// cheap to orient in — the agent reads the tree, spots the relevant subtree by
/// its counts, then drills with `overview <dir>`.
#[derive(Debug, Clone)]
pub struct DirectorySummary {
    pub path: String,
    pub file_count: usize,
    pub symbol_count: usize,
}

/// Kinds whose body opens a function scope. A symbol strictly contained within
/// one of these is function-local (a local variable, nested fn, or closure) and
/// is dropped from significant views. The tree-sitter layer normalizes both free
/// functions and methods to "fn"; "method"/"function" are listed for resilience
/// if that normalization ever changes.
const FUNCTION_SCOPE_KINDS: &[&str] = &["fn", "method", "function"];

/// A symbol is "significant" when it is exported, or when it is not nested inside
/// a function scope in the same file. Type members (class/struct/impl methods —
/// contained by a type symbol, not a function) stay significant; function-local
/// symbols are dropped. Operates on the flat per-file symbol list, since the
/// extractor records no parent links.
pub(crate) fn is_significant_symbol(
    symbol: &crate::parser::ExtractedSymbol,
    file_symbols: &[crate::parser::ExtractedSymbol],
) -> bool {
    if symbol.flags.is_exported {
        return true;
    }
    !file_symbols.iter().any(|parent| {
        FUNCTION_SCOPE_KINDS.contains(&parent.kind.as_str())
            && crate::parser::range_strictly_contains(&parent.range, &symbol.range)
    })
}

/// Build a per-file summary carrying only significant symbols, with their count.
/// Shared by the root and folder views so both apply the same filter.
pub(crate) fn summarize_file(file: &crate::parser::ExtractedFile) -> ExtractedFileSummary<'_> {
    let symbols: Vec<ExtractedSymbolSummary<'_>> = file
        .symbols
        .iter()
        .filter(|s| is_significant_symbol(s, &file.symbols))
        .map(|s| ExtractedSymbolSummary {
            name: &s.name,
            kind: &s.kind,
            is_exported: s.flags.is_exported,
            start_line: s.range.start_line,
            end_line: s.range.end_line,
        })
        .collect();
    ExtractedFileSummary {
        file_path: super::normalize_path(&file.file_path).into_owned(),
        total_lines: file.total_lines,
        symbol_count: symbols.len(),
        symbols,
    }
}

/// Aggregate every ancestor directory's recursive file + significant-symbol counts in
/// one pass over the file summaries: each prefix of a file's path is an ancestor, so a
/// file in `src/a/b.ts` credits both `src` and `src/a`. Shared by the root and folder
/// views so a directory's counts are identical regardless of where you view it from. The
/// BTreeMap keeps the result path-sorted.
pub(crate) fn build_directory_summaries(files: &[ExtractedFileSummary]) -> Vec<DirectorySummary> {
    let mut dir_counts: std::collections::BTreeMap<String, (usize, usize)> =
        std::collections::BTreeMap::new();
    for file in files {
        let parts: Vec<&str> = file.file_path.split('/').collect();
        for i in 1..parts.len() {
            let dir_path = parts[0..i].join("/");
            if dir_path.is_empty() {
                continue;
            }
            let entry = dir_counts.entry(dir_path).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += file.symbol_count;
        }
    }
    dir_counts
        .into_iter()
        .map(|(path, (file_count, symbol_count))| DirectorySummary {
            path,
            file_count,
            symbol_count,
        })
        .collect()
}
