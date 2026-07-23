//! Structural extraction for indented Sass syntax.

use raffia::ast::{
    AtRulePrelude, ComplexSelectorChild, SimpleBlock, SimpleSelector, Statement, Stylesheet,
};
use raffia::{Parser, Span, Spanned, Syntax};

use super::{CodeRange, ExtractedFile, ExtractedSymbol, IndexAuxiliary, SymbolFlags};

pub(super) fn extract(
    source: &str,
    file_path: &str,
    collect_auxiliary: bool,
) -> Result<(ExtractedFile, IndexAuxiliary), String> {
    let mut symbols = Vec::new();
    if let Some((stylesheet, recovery_spans)) = parse_recoverable_prefix(source) {
        collect_statements(
            &stylesheet.statements,
            source,
            &recovery_spans,
            &mut symbols,
        );
    }
    symbols.sort_by_key(|symbol| {
        (
            symbol.range.start_line,
            symbol.range.start_col,
            symbol.name.clone(),
            symbol.kind.clone(),
        )
    });

    let mut auxiliary = IndexAuxiliary::default();
    if collect_auxiliary {
        auxiliary.format_text.push(source.to_string());
    }
    Ok((
        ExtractedFile {
            file_path: file_path.to_string(),
            total_lines: source.lines().count(),
            symbols,
            literals: Vec::new(),
            docstrings: Vec::new(),
            navigation: None,
        },
        auxiliary,
    ))
}

fn parse_recoverable_prefix(source: &str) -> Option<(Stylesheet<'_>, Vec<Span>)> {
    let mut candidate_end = source.len();
    loop {
        let candidate = &source[..candidate_end];
        let mut parser = Parser::new(candidate, Syntax::Sass);
        match parser.parse::<Stylesheet>() {
            Ok(stylesheet) => {
                let recovery_spans = parser
                    .recoverable_errors()
                    .iter()
                    .map(|error| error.span.clone())
                    .collect();
                return Some((stylesheet, recovery_spans));
            }
            Err(error) => {
                let error_start = error.span.start.min(candidate_end);
                let line_start = candidate[..error_start]
                    .rfind('\n')
                    .map_or(0, |index| index + 1);
                if line_start == 0 || line_start >= candidate_end {
                    return None;
                }
                candidate_end = line_start;
            }
        }
    }
}

fn collect_statements(
    statements: &[Statement<'_>],
    source: &str,
    recovery_spans: &[Span],
    symbols: &mut Vec<ExtractedSymbol>,
) {
    for statement in statements {
        if recovery_spans
            .iter()
            .any(|error| span_contains_offset(statement.span(), error.start))
        {
            continue;
        }
        match statement {
            Statement::AtRule(rule) => {
                if let Some(prelude) = &rule.prelude {
                    match prelude {
                        AtRulePrelude::Keyframes(name) => {
                            push_symbol(source, name.span(), "keyframes", symbols)
                        }
                        AtRulePrelude::Property(name) => {
                            push_symbol(source, name.span(), "custom_property", symbols)
                        }
                        AtRulePrelude::SassFunction(function) => {
                            push_symbol(source, &function.name.span, "function", symbols)
                        }
                        AtRulePrelude::SassMixin(mixin) => {
                            push_symbol(source, &mixin.name.span, "mixin", symbols)
                        }
                        _ => {}
                    }
                }
                if let Some(block) = &rule.block {
                    collect_block(block, source, recovery_spans, symbols);
                }
            }
            Statement::Declaration(declaration) => {
                let span = declaration.name.span();
                if source
                    .get(span.start..span.end)
                    .is_some_and(|name| name.starts_with("--"))
                {
                    push_symbol(source, span, "custom_property", symbols);
                }
            }
            Statement::QualifiedRule(rule) => {
                for selector in &rule.selector.selectors {
                    for child in &selector.children {
                        let ComplexSelectorChild::CompoundSelector(compound) = child else {
                            continue;
                        };
                        for simple in &compound.children {
                            if !matches!(simple, SimpleSelector::Nesting(_)) {
                                push_symbol(source, simple.span(), "selector", symbols);
                            }
                        }
                    }
                }
                collect_block(&rule.block, source, recovery_spans, symbols);
            }
            Statement::SassIfAtRule(rule) => {
                collect_block(&rule.if_clause.block, source, recovery_spans, symbols);
                for clause in &rule.else_if_clauses {
                    collect_block(&clause.block, source, recovery_spans, symbols);
                }
                if let Some(block) = &rule.else_clause {
                    collect_block(block, source, recovery_spans, symbols);
                }
            }
            Statement::SassVariableDeclaration(declaration) => {
                push_symbol(source, &declaration.name.span, "variable", symbols);
            }
            Statement::UnknownSassAtRule(rule) => {
                if let Some(block) = &rule.block {
                    collect_block(block, source, recovery_spans, symbols);
                }
            }
            _ => {}
        }
    }
}

fn collect_block(
    block: &SimpleBlock<'_>,
    source: &str,
    recovery_spans: &[Span],
    symbols: &mut Vec<ExtractedSymbol>,
) {
    collect_statements(&block.statements, source, recovery_spans, symbols);
}

fn span_contains_offset(span: &Span, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}

fn push_symbol(source: &str, span: &Span, kind: &str, symbols: &mut Vec<ExtractedSymbol>) {
    let Some(name) = source.get(span.start..span.end).map(str::trim) else {
        return;
    };
    if name.is_empty() {
        return;
    }
    let symbol = ExtractedSymbol {
        name: name.to_string(),
        kind: kind.to_string(),
        range: range(source, span),
        docstring: None,
        flags: SymbolFlags {
            has_todo: false,
            has_fixme: false,
            is_test: false,
            is_exported: false,
            is_deprecated: false,
        },
        owner: None,
    };
    if !symbols.contains(&symbol) {
        symbols.push(symbol);
    }
}

fn range(source: &str, span: &Span) -> CodeRange {
    let (start_line, start_col) = point(source, span.start);
    let (end_line, end_col) = point(source, span.end);
    CodeRange {
        start_line,
        start_col,
        end_line,
        end_col,
    }
}

fn point(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    (line, offset - line_start + 1)
}
