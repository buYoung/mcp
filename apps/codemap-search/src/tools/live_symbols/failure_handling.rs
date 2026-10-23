//! Failure trailer for `read view=source`: what the window throws, and how the outside call
//! sites of its functions handle a failure. A call inside a `try` body is listed with each
//! clause that can catch there, as written, and whether the clause rethrows, returns or is
//! empty; a Rust call says whether `?`, `unwrap` or `expect` handles its error, and a Go call
//! whether the `err` it assigns is checked. Shapes come from each language's syntax tree;
//! nothing is resolved, so a clause's exception type is shown, not matched.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use tree_sitter::{Node, Tree};

use crate::callers::CallerSite;

/// Bytes for the whole trailer, including its header.
const TRAILER_BYTE_CAP: usize = 1024;
/// Time for parsing the window and the caller files.
const TRAILER_TIME_BUDGET: Duration = Duration::from_millis(100);
/// Thrown expressions listed for the window, each with at most [`LINES_PER_THROW`] lines.
const THROWS_LIMIT: usize = 4;
const LINES_PER_THROW: usize = 4;
/// Call sites checked per function.
const SITES_PER_FUNCTION: usize = 6;
/// Characters of a clause header or thrown expression kept.
const SNIPPET_CHARS: usize = 60;
pub(crate) const HEADER: &str = "[Failure handling around this window (syntax only, approximate)]";
const PARTIAL_LINE: &str =
    "- failure scan stopped at its time limit; more call sites may handle failures";

/// A function of the window and its confirmed outside call sites.
pub(super) struct FunctionCallers {
    pub name: String,
    pub line: usize,
    pub sites: Vec<CallerSite>,
}

/// The trailer for lines `start..=end` of `path`, within `byte_cap` bytes and never above
/// [`TRAILER_BYTE_CAP`]; `None` when nothing in or around the window handles a failure.
pub(super) fn trailer(
    root: &Path,
    path: &str,
    (start, end): (usize, usize),
    functions: &[FunctionCallers],
    byte_cap: usize,
) -> Option<String> {
    let started = Instant::now();
    let byte_cap = byte_cap.min(TRAILER_BYTE_CAP);
    if byte_cap < HEADER.len() + 64 {
        return None;
    }
    let mut files = ParsedFiles::default();
    let mut lines = Vec::new();
    if let Some(window) = files.get(root, path) {
        let throws = window_throws(window, start, end);
        if !throws.is_empty() {
            lines.push(format!("- throws in window: {}", throws.join(", ")));
        }
    }
    let mut is_partial = false;
    for function in functions {
        let mut handled = Vec::new();
        for site in function.sites.iter().take(SITES_PER_FUNCTION) {
            if started.elapsed() >= TRAILER_TIME_BUDGET {
                is_partial = true;
                break;
            }
            let Some(file) = files.get(root, &site.path) else {
                continue;
            };
            // Lines of another file carry its path; bare `L` lines are the window's file.
            let other_path = (site.path != path).then_some(site.path.as_str());
            let Some(handling) = site_handling(file, site.line, &function.name, other_path) else {
                continue;
            };
            let location = if site.path == path {
                format!("L{}", site.line)
            } else {
                format!("{}:{}", site.path, site.line)
            };
            let caller = site.caller.as_deref().unwrap_or("top-level");
            handled.push(format!("{caller} ({location}) {handling}"));
        }
        if !handled.is_empty() {
            lines.push(format!(
                "- {} L{} callers: {}",
                function.name,
                function.line,
                handled.join("; ")
            ));
        }
    }
    if is_partial {
        lines.push(PARTIAL_LINE.to_string());
    }
    let mut text = String::from(HEADER);
    for line in lines {
        if text.len() + 1 + line.len() > byte_cap {
            break;
        }
        text.push('\n');
        text.push_str(&line);
    }
    (text.len() > HEADER.len()).then_some(text)
}

/// A parsed live file: its text, tree and language name.
struct ParsedFile {
    text: String,
    tree: Tree,
    language: &'static str,
}

#[derive(Default)]
struct ParsedFiles(HashMap<String, Option<ParsedFile>>);

impl ParsedFiles {
    fn get(&mut self, root: &Path, path: &str) -> Option<&ParsedFile> {
        self.0
            .entry(path.to_string())
            .or_insert_with(|| parse(&root.join(path)))
            .as_ref()
    }
}

fn parse(path: &Path) -> Option<ParsedFile> {
    let ext = path.extension()?.to_str()?;
    if crate::lang::is_composite_extension(ext) {
        return None;
    }
    let spec = crate::lang::spec_for_path(path)?;
    let text = std::fs::read_to_string(path).ok()?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&spec.grammar(ext)).ok()?;
    let tree = crate::parser::parse_source(&mut parser, text.as_bytes()).ok()?;
    Some(ParsedFile {
        text,
        tree,
        language: spec.language_name(),
    })
}

/// Thrown expressions starting on lines `start..=end`, grouped by what is thrown.
fn window_throws(file: &ParsedFile, start: usize, end: usize) -> Vec<String> {
    let mut thrown: Vec<(String, Vec<usize>)> = Vec::new();
    let mut stack = vec![file.tree.root_node()];
    while let Some(node) = stack.pop() {
        let first = node.start_position().row + 1;
        let last = node.end_position().row + 1;
        if last < start || end < first {
            continue;
        }
        if first >= start && is_throw(node, file) {
            let what = thrown_expression(node_text(node, file));
            match thrown.iter_mut().find(|(text, _)| *text == what) {
                Some((_, lines)) => lines.push(first),
                None => thrown.push((what, vec![first])),
            }
            continue;
        }
        let mut cursor = node.walk();
        let mut children: Vec<_> = node.named_children(&mut cursor).collect();
        children.reverse();
        stack.extend(children);
    }
    thrown
        .into_iter()
        .take(THROWS_LIMIT)
        .map(|(what, lines)| {
            let shown: Vec<_> = lines
                .iter()
                .take(LINES_PER_THROW)
                .map(|line| format!("L{line}"))
                .collect();
            let more = if lines.len() > LINES_PER_THROW {
                format!(", +{} more", lines.len() - LINES_PER_THROW)
            } else {
                String::new()
            };
            format!("{what} ({}{more})", shown.join(", "))
        })
        .collect()
}

/// How a failure of the call to `name` on `line` is handled where it is made, if visibly.
/// Lines of `file` are written with `other_path` when it is not the window's file.
fn site_handling(
    file: &ParsedFile,
    line: usize,
    name: &str,
    other_path: Option<&str>,
) -> Option<String> {
    let anchor = callee_token(file, line, name)?;
    let at = |first: usize, last: usize| match (other_path, first == last) {
        (Some(path), true) => format!("{path}:{first}"),
        (Some(path), false) => format!("{path}:{first}-{last}"),
        (None, true) => format!("L{first}"),
        (None, false) => format!("L{first}-{last}"),
    };
    match file.language {
        "rust" => rust_handling(anchor, file),
        "go" => go_handling(anchor, file, &at),
        "javascript" | "typescript" => {
            promise_handling(anchor, file, &at).or_else(|| enclosing_handlers(anchor, file, &at))
        }
        _ => enclosing_handlers(anchor, file, &at),
    }
}

/// The token spelling the callee `name` on `line`.
fn callee_token<'t>(file: &'t ParsedFile, line: usize, name: &str) -> Option<Node<'t>> {
    let row = line.checked_sub(1)?;
    let line_text = file.text.lines().nth(row)?;
    line_text.match_indices(name).find_map(|(column, _)| {
        let node = file.tree.root_node().descendant_for_point_range(
            tree_sitter::Point::new(row, column),
            tree_sitter::Point::new(row, column + name.len()),
        )?;
        (node_text(node, file) == name).then_some(node)
    })
}

/// The catching clauses of the nearest `try` whose protected part contains `at`, within the
/// enclosing function.
fn enclosing_handlers(
    at: Node,
    file: &ParsedFile,
    location: &dyn Fn(usize, usize) -> String,
) -> Option<String> {
    let mut child = at;
    while let Some(parent) = child.parent() {
        if is_function(parent.kind()) {
            return None;
        }
        let handlers = handlers(parent, file);
        if !handlers.is_empty() && is_protected(parent, child) {
            let clauses: Vec<_> = handlers
                .iter()
                .map(|handler| describe_handler(handler, file, location))
                .collect();
            return Some(format!("in try → {}", clauses.join(", ")));
        }
        child = parent;
    }
    None
}

/// One clause that catches a failure: from its first node through its body.
struct Handler<'t> {
    start: Node<'t>,
    body: Option<Node<'t>>,
    end: Node<'t>,
}

/// The clauses of `node` that catch a failure: `catch`, `except` and `rescue` clauses, and
/// Dart's `on T`/`catch (e)` parts, which precede their block as siblings.
fn handlers<'t>(node: Node<'t>, file: &ParsedFile) -> Vec<Handler<'t>> {
    let mut found = Vec::new();
    let mut cursor = node.walk();
    if file.language == "dart" && node.kind() == "try_statement" {
        let body = node.child_by_field_name("body");
        let mut start = None;
        for child in node.children(&mut cursor) {
            match child.kind() {
                "on" | "catch_clause" => {
                    start.get_or_insert(child);
                }
                "block" if body.is_none_or(|body| body.id() != child.id()) => {
                    if let Some(start) = start.take() {
                        found.push(Handler {
                            start,
                            body: Some(child),
                            end: child,
                        });
                    }
                }
                _ => {}
            }
        }
        return found;
    }
    for child in node.named_children(&mut cursor) {
        let clauses: Vec<_> = if child.kind() == "catch_clauses" {
            let mut nested = child.walk();
            child
                .named_children(&mut nested)
                .filter(|clause| is_handler(clause.kind()))
                .collect()
        } else if is_handler(child.kind()) {
            vec![child]
        } else {
            Vec::new()
        };
        found.extend(clauses.into_iter().map(|clause| Handler {
            start: clause,
            body: clause_body(clause),
            end: clause,
        }));
    }
    found
}

/// Whether `child` of the `try` node `node` is what its handlers protect: its `body` (and
/// Java's resources) when the grammar names one, else anything but a handler or cleanup.
fn is_protected(node: Node, child: Node) -> bool {
    if let Some(body) = node.child_by_field_name("body") {
        return body.id() == child.id()
            || node
                .child_by_field_name("resources")
                .is_some_and(|resources| resources.id() == child.id());
    }
    !is_handler(child.kind())
        && !matches!(
            child.kind(),
            "catch_clauses"
                | "finally_clause"
                | "finally_block"
                | "ensure"
                | "seh_finally_clause"
                | "else_clause"
                | "else"
        )
}

/// A clause as written up to its body, with what the body does on a failure.
fn describe_handler(
    handler: &Handler,
    file: &ParsedFile,
    location: &dyn Fn(usize, usize) -> String,
) -> String {
    let start = handler.start;
    let text = file
        .text
        .get(start.start_byte()..handler.end.end_byte())
        .unwrap_or("");
    let mut header = match handler.body {
        Some(body) => file
            .text
            .get(start.start_byte()..body.start_byte())
            .unwrap_or(text)
            .to_string(),
        None => text.lines().next().unwrap_or(text).to_string(),
    };
    // A Scala handler names its exception types in the case patterns of its body.
    if let Some(body) = handler.body.filter(|body| body.kind() == "case_block") {
        let mut cursor = body.walk();
        let patterns: Vec<_> = body
            .named_children(&mut cursor)
            .filter_map(|case| case.child_by_field_name("pattern"))
            .map(|pattern| format!("case {}", collapse(node_text(pattern, file))))
            .collect();
        if !patterns.is_empty() {
            header = format!("{} {{ {} }}", header.trim(), patterns.join("; "));
        }
    }
    let header = collapse(header.trim_end().trim_end_matches(['{', ':']));
    let first = start.start_position().row + 1;
    let last = handler.end.end_position().row + 1;
    let scope = handler.body.unwrap_or(start);
    let mut flags = Vec::new();
    if contains(scope, &|node| is_throw(node, file)) {
        flags.push("rethrows");
    }
    if contains(scope, &|node| is_return(node, file)) {
        flags.push("returns");
    }
    if handler.body.is_some_and(is_empty_body) {
        flags.push("empty");
    }
    let lines = location(first, last);
    if flags.is_empty() {
        format!("{} {lines}", truncate(&header))
    } else {
        format!("{} {lines} {}", truncate(&header), flags.join(" "))
    }
}

/// The statements a clause runs: its `body` field, else its last block-shaped child.
fn clause_body(clause: Node) -> Option<Node> {
    clause.child_by_field_name("body").or_else(|| {
        let mut cursor = clause.walk();
        let body = clause
            .named_children(&mut cursor)
            .filter(|child| {
                matches!(
                    child.kind(),
                    "block"
                        | "statement_block"
                        | "compound_statement"
                        | "statements"
                        | "then"
                        | "case_block"
                )
            })
            .last();
        body
    })
}

/// Whether a body holds no statement other than comments or `pass`.
fn is_empty_body(body: Node) -> bool {
    let mut cursor = body.walk();
    let is_empty = body.named_children(&mut cursor).all(|statement| {
        matches!(
            statement.kind(),
            "comment" | "line_comment" | "block_comment" | "pass_statement"
        )
    });
    is_empty
}

/// A promise rejection handled on the call itself: `f().catch(...)`.
fn promise_handling(
    at: Node,
    file: &ParsedFile,
    location: &dyn Fn(usize, usize) -> String,
) -> Option<String> {
    let call = ancestor_of_kind(at, "call_expression")?;
    let member = call
        .parent()
        .filter(|parent| parent.kind() == "member_expression")?;
    let property = node_text(member.child_by_field_name("property")?, file);
    let handler = member
        .parent()
        .filter(|parent| parent.kind() == "call_expression")?;
    (property == "catch").then(|| {
        let line = handler.start_position().row + 1;
        format!("rejection handled by .catch() at {}", location(line, line))
    })
}

fn rust_handling(at: Node, file: &ParsedFile) -> Option<String> {
    let call = ancestor_of_kind(at, "call_expression")?;
    let parent = call.parent()?;
    match parent.kind() {
        "try_expression" => Some("error propagated with ?".into()),
        "field_expression" => {
            let method = node_text(parent.child_by_field_name("field")?, file);
            matches!(method, "unwrap" | "expect").then(|| format!("panics on error (.{method}())"))
        }
        _ => None,
    }
}

fn go_handling(
    at: Node,
    file: &ParsedFile,
    location: &dyn Fn(usize, usize) -> String,
) -> Option<String> {
    let call = ancestor_of_kind(at, "call_expression")?;
    let mut statement = call.parent()?;
    if statement.kind() == "expression_list" {
        statement = statement.parent()?;
    }
    if !matches!(
        statement.kind(),
        "short_var_declaration" | "assignment_statement"
    ) {
        return None;
    }
    let left = statement.child_by_field_name("left")?;
    let mut cursor = left.walk();
    let error = left
        .named_children(&mut cursor)
        .filter(|name| name.kind() == "identifier")
        .map(|name| node_text(name, file))
        .find(|name| *name == "err" || name.ends_with("Err"))?;
    let is_checked =
        |condition: Node| collapse(node_text(condition, file)).contains(&format!("{error} != nil"));
    // `if v, err := f(); err != nil {`
    if let Some(statement_if) = statement
        .parent()
        .filter(|parent| parent.kind() == "if_statement")
    {
        if statement_if
            .child_by_field_name("condition")
            .is_some_and(is_checked)
        {
            let line = statement_if.start_position().row + 1;
            return Some(format!("{error} checked at {}", location(line, line)));
        }
    }
    let mut next = statement.next_named_sibling();
    while let Some(sibling) = next {
        if sibling.kind() == "if_statement"
            && sibling
                .child_by_field_name("condition")
                .is_some_and(is_checked)
        {
            let line = sibling.start_position().row + 1;
            return Some(format!("{error} checked at {}", location(line, line)));
        }
        if node_text(sibling, file).contains(error) {
            break;
        }
        next = sibling.next_named_sibling();
    }
    Some(format!("{error} assigned; no `if {error} != nil` follows"))
}

fn ancestor_of_kind<'t>(mut node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    loop {
        if node.kind() == kind {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn is_handler(kind: &str) -> bool {
    matches!(
        kind,
        "catch_clause" | "except_clause" | "rescue" | "catch_block" | "seh_except_clause"
    )
}

fn is_return(node: Node, file: &ParsedFile) -> bool {
    match node.kind() {
        "return_statement" | "return_expression" | "return" => true,
        // Swift
        "control_transfer_statement" => node_text(node, file).starts_with("return"),
        _ => false,
    }
}

fn is_throw(node: Node, file: &ParsedFile) -> bool {
    match node.kind() {
        "throw_statement" | "throw_expression" | "raise_statement" | "rethrow_statement" => true,
        // Swift
        "control_transfer_statement" => node_text(node, file).starts_with("throw"),
        // Ruby: `raise X` and `fail X` are method calls.
        "call" | "identifier" if file.language == "ruby" => {
            let callee = node
                .child_by_field_name("method")
                .map_or_else(|| node_text(node, file), |method| node_text(method, file));
            matches!(callee, "raise" | "fail")
                && node.child_by_field_name("receiver").is_none()
                && !node.parent().is_some_and(|parent| parent.kind() == "call")
        }
        _ => false,
    }
}

/// What a throw statement throws: the created type, the called factory or the thrown value.
fn thrown_expression(statement: &str) -> String {
    let text = collapse(statement);
    let mut rest = text.as_str();
    for keyword in ["throw", "raise", "rethrow", "fail"] {
        if let Some(stripped) = rest.strip_prefix(keyword) {
            if stripped.is_empty() || stripped.starts_with([' ', ';', '(']) {
                rest = stripped.trim_start();
                break;
            }
        }
    }
    rest = rest.strip_prefix("new ").unwrap_or(rest).trim_start();
    let head: String = rest
        .chars()
        .take_while(|c| !matches!(c, '(' | ',' | ';' | '{' | ' '))
        .collect();
    if head.is_empty() {
        "rethrow".into()
    } else {
        truncate(&head)
    }
}

fn is_function(kind: &str) -> bool {
    matches!(
        kind,
        "function_definition"
            | "function_declaration"
            | "function_item"
            | "method_declaration"
            | "method_definition"
            | "method"
            | "singleton_method"
            | "constructor_declaration"
            | "secondary_constructor"
            | "local_function_statement"
            | "init_declaration"
            | "function_signature"
            | "func_literal"
    )
}

/// Whether a node below `node` passes `test`.
fn contains(node: Node, test: &dyn Fn(Node) -> bool) -> bool {
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        if current.id() != node.id() && test(current) {
            return true;
        }
        let mut cursor = current.walk();
        stack.extend(current.named_children(&mut cursor));
    }
    false
}

fn node_text<'t>(node: Node, file: &'t ParsedFile) -> &'t str {
    file.text.get(node.byte_range()).unwrap_or("")
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= SNIPPET_CHARS {
        text.to_string()
    } else {
        let kept: String = text.chars().take(SNIPPET_CHARS).collect();
        format!("{kept}…")
    }
}
