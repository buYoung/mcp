use std::ops::ControlFlow;
use std::time::{Duration, Instant};
use tree_sitter::{ParseOptions, Parser, Tree};

const PARSE_TIME_LIMIT: Duration = Duration::from_millis(5000);

/// Code grammar parsing, including live context, must yield on pathological input.
/// The progress hook interrupts tree-sitter itself; an abandoned worker would
/// otherwise keep consuming CPU and prevent the indexer from shutting down.
pub(crate) fn parse_source(parser: &mut Parser, source: &[u8]) -> Result<Tree, String> {
    parse_with_limit(parser, source, PARSE_TIME_LIMIT)
}

fn parse_with_limit(parser: &mut Parser, source: &[u8], limit: Duration) -> Result<Tree, String> {
    let started = Instant::now();
    let mut has_timed_out = false;
    let mut progress = |_: &tree_sitter::ParseState| {
        has_timed_out = started.elapsed() >= limit;
        if has_timed_out {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let tree = parser.parse_with_options(
        &mut |offset, _| source.get(offset..).unwrap_or_default(),
        None,
        Some(ParseOptions::new().progress_callback(&mut progress)),
    );
    if let Some(tree) = tree {
        return Ok(tree);
    }
    // A cancelled parser retains its continuation. Never resume it with a
    // different file, even when a caller reuses the parser after an error.
    parser.reset();
    if has_timed_out {
        Err(format!(
            "Tree-sitter parsing exceeded the {} ms per-file limit",
            limit.as_millis()
        ))
    } else {
        Err("Failed to parse file content".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_nonterminating_reduction_is_cancelled_and_parser_can_be_reused() {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_zsh::LANGUAGE.into())
            .unwrap();
        // https://github.com/georgeharker/tree-sitter-zsh/issues/37
        let error = parse_with_limit(&mut parser, b"c=${x//[^)]}\n", Duration::from_millis(20))
            .unwrap_err();
        assert!(error.contains("20 ms per-file limit"), "{error}");
        let tree = parse_source(&mut parser, b"healthy() { print ok; }\n").unwrap();
        assert!(!tree.root_node().has_error());
        assert_eq!(
            tree.root_node().named_child(0).unwrap().kind(),
            "function_definition"
        );
    }
}
