//! `grep` — literal/regex content search over file contents, powered by ripgrep's
//! `grep`/`grep-searcher`/`grep-regex` library crates. Mirrors Claude Code's Grep
//! (same parameter names `-i`/`-n`/`-A`/`-B`/`-C`, `output_mode`, `head_limit`,
//! `glob`, `type`) so an agent can swap its built-in. Respects `.gitignore` +
//! `.codemapignore` by default (decided divergence) with an `include_ignored`
//! override. Reads disk directly, so it sees comments and just-changed files the
//! BM25 index can miss — this realizes the spec's "rg 역할" alongside `search`.

use super::{arg_bool, arg_required_str, arg_usize, build_walker, current_dir, resolve_within_cwd};
use grep::regex::RegexMatcherBuilder;
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkContext, SinkMatch};
use serde_json::Value;

const DEFAULT_HEAD_LIMIT: usize = 250;

struct LineHit {
    line_number: u64,
    text: String,
    is_match: bool,
}

/// Collects matched + context lines for a single file. `occurrences` counts match
/// regions (ripgrep "count" semantics), not physical lines.
struct CollectSink {
    hits: Vec<LineHit>,
    occurrences: usize,
}

fn strip_eol(s: &str) -> &str {
    let s = s.strip_suffix('\n').unwrap_or(s);
    s.strip_suffix('\r').unwrap_or(s)
}

impl Sink for CollectSink {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, std::io::Error> {
        let start = mat.line_number().unwrap_or(0);
        let cow = String::from_utf8_lossy(mat.bytes());
        let text: &str = cow.as_ref();
        let text = text.strip_suffix('\n').unwrap_or(text);
        // A match region may span several physical lines under multiline search.
        for (i, line) in text.split('\n').enumerate() {
            let line = line.strip_suffix('\r').unwrap_or(line);
            self.hits.push(LineHit {
                line_number: start + i as u64,
                text: line.to_string(),
                is_match: true,
            });
        }
        self.occurrences += 1;
        Ok(true)
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        ctx: &SinkContext<'_>,
    ) -> Result<bool, std::io::Error> {
        let cow = String::from_utf8_lossy(ctx.bytes());
        let line = strip_eol(cow.as_ref());
        self.hits.push(LineHit {
            line_number: ctx.line_number().unwrap_or(0),
            text: line.to_string(),
            is_match: false,
        });
        Ok(true)
    }
}

struct FileResult {
    path: String,
    hits: Vec<LineHit>,
    occurrences: usize,
}

/// Slice `items` to one page and produce a pagination footer when truncated.
fn paginate<T: Clone>(items: &[T], offset: usize, head_limit: usize) -> (Vec<T>, Option<String>) {
    let total = items.len();
    let start = offset.min(total);
    let limit = if head_limit == 0 { total } else { head_limit };
    let end = (start + limit).min(total);
    let page = items[start..end].to_vec();
    let truncated = start > 0 || end < total;
    let footer = if !truncated {
        None
    } else if page.is_empty() {
        Some(format!("[No results at offset {offset}; {total} total.]"))
    } else {
        Some(format!(
            "[Showing results {}-{} of {}; pass a larger head_limit or offset to page further.]",
            start + 1,
            end,
            total
        ))
    };
    (page, footer)
}

pub fn grep(args: &Value) -> Result<String, (i64, String)> {
    let pattern = arg_required_str(args, "pattern")?;
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let glob_opt = args.get("glob").and_then(|v| v.as_str());
    let type_opt = args.get("type").and_then(|v| v.as_str());
    let output_mode = args
        .get("output_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("files_with_matches");
    let case_insensitive = arg_bool(args, "-i", false);
    let multiline = arg_bool(args, "multiline", false);
    let show_line_numbers = arg_bool(args, "-n", true);
    let context_both = arg_usize(args, "-C", 0);
    let (before, after) = if context_both > 0 {
        (context_both, context_both)
    } else {
        (arg_usize(args, "-B", 0), arg_usize(args, "-A", 0))
    };
    let head_limit = arg_usize(args, "head_limit", DEFAULT_HEAD_LIMIT);
    let offset = arg_usize(args, "offset", 0);
    let include_ignored = arg_bool(args, "include_ignored", false);

    let base = resolve_within_cwd(path)?;
    if !base.exists() {
        return Err((-32602, format!("Search path does not exist: {path}")));
    }
    let cwd = current_dir()?;
    let cwd_canonical = cwd.canonicalize().unwrap_or(cwd);

    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(case_insensitive)
        .multi_line(multiline)
        .build(pattern)
        .map_err(|e| (-32602, format!("Invalid regex pattern '{pattern}': {e}")))?;

    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .before_context(before)
        .after_context(after)
        .multi_line(multiline)
        .binary_detection(BinaryDetection::quit(0))
        .build();

    let glob_matcher = match glob_opt {
        Some(g) => Some(
            globset::GlobBuilder::new(g)
                .literal_separator(true)
                .build()
                .map_err(|e| (-32602, format!("Invalid glob '{g}': {e}")))?
                .compile_matcher(),
        ),
        None => None,
    };

    let mut walker = build_walker(&base, include_ignored);
    if let Some(t) = type_opt {
        let mut tb = ignore::types::TypesBuilder::new();
        tb.add_defaults();
        tb.select(t);
        let types = tb
            .build()
            .map_err(|e| (-32602, format!("Unknown type filter '{t}': {e}")))?;
        walker.types(types);
    }

    let mut files: Vec<FileResult> = Vec::new();
    for result in walker.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let p = entry.path();
        if let Some(ref gm) = glob_matcher {
            let rel = p.strip_prefix(&base).unwrap_or(p);
            if !gm.is_match(rel) {
                continue;
            }
        }
        let mut sink = CollectSink {
            hits: Vec::new(),
            occurrences: 0,
        };
        if searcher.search_path(&matcher, p, &mut sink).is_err() {
            continue;
        }
        if sink.occurrences > 0 {
            let display = p
                .strip_prefix(&cwd_canonical)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/");
            files.push(FileResult {
                path: display,
                hits: sink.hits,
                occurrences: sink.occurrences,
            });
        }
    }

    match output_mode {
        "count" => {
            let total_occ: usize = files.iter().map(|f| f.occurrences).sum();
            if total_occ == 0 {
                return Ok("No matches found".to_string());
            }
            Ok(format!(
                "Found {total_occ} total occurrence(s) across {} file(s).",
                files.len()
            ))
        }
        "content" => {
            let mut lines: Vec<String> = Vec::new();
            for f in &files {
                for hit in &f.hits {
                    let sep = if hit.is_match { ':' } else { '-' };
                    if show_line_numbers {
                        lines.push(format!(
                            "{}{sep}{}{sep}{}",
                            f.path, hit.line_number, hit.text
                        ));
                    } else {
                        lines.push(format!("{}{sep}{}", f.path, hit.text));
                    }
                }
            }
            if lines.is_empty() {
                return Ok("No matches found".to_string());
            }
            let (page, footer) = paginate(&lines, offset, head_limit);
            let mut out = page.join("\n");
            if let Some(f) = footer {
                out.push('\n');
                out.push_str(&f);
            }
            Ok(out)
        }
        // default: files_with_matches
        _ => {
            if files.is_empty() {
                return Ok("No matches found".to_string());
            }
            let total = files.len();
            let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
            let (page, footer) = paginate(&paths, offset, head_limit);
            let mut out = format!("Found {total} file(s)\n");
            out.push_str(&page.join("\n"));
            if let Some(f) = footer {
                out.push('\n');
                out.push_str(&f);
            }
            Ok(out)
        }
    }
}
