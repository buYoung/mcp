//! `grep` — literal/regex content search over file contents, powered by ripgrep's
//! `grep`/`grep-searcher`/`grep-regex` library crates. Mirrors Claude Code's Grep
//! (same parameter names `-i`/`-n`/`-A`/`-B`/`-C`, `output_mode`, `head_limit`,
//! `glob`, `type`) so an agent can swap its built-in. Respects `.gitignore` +
//! `.codemapignore` by default (decided divergence) with an `include_ignored`
//! override. Reads disk directly, so it sees comments and just-changed files the
//! BM25 index can miss — this realizes the spec's "rg 역할" alongside `search`.

use super::{
    arg_bool, arg_required_str, arg_usize, build_glob_matcher, build_walker, current_dir,
    resolve_within_cwd, split_grep_globs, GlobMatcher,
};
use grep::regex::RegexMatcherBuilder;
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkContext, SinkMatch};
use serde_json::Value;
use std::time::SystemTime;

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
    /// Modification time, used only to sort `files_with_matches` output (newest first).
    mtime: SystemTime,
}

/// Cap a line at the column limit, replacing an over-long line with ripgrep's omission
/// marker (`--max-columns` parity). Matched lines and context lines get distinct markers.
/// `max_columns == 0` disables the cap. Width is measured in bytes, matching ripgrep.
fn cap_line(text: &str, max_columns: usize, is_match: bool) -> String {
    if max_columns > 0 && text.len() > max_columns {
        if is_match {
            "[Omitted long matching line]".to_string()
        } else {
            "[Omitted long context line]".to_string()
        }
    } else {
        text.to_string()
    }
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
    // Accept `include`/`file_pattern` as aliases for `glob`: agents were observed sending both,
    // and a wrong key was silently ignored → an unintended whole-repo search. Canonical wins;
    // earlier aliases win over later ones.
    let glob_opt = ["glob", "include", "file_pattern"]
        .iter()
        .find_map(|key| args.get(*key).and_then(|v| v.as_str()));
    let type_opt = args.get("type").and_then(|v| v.as_str());
    // Default to `content` so a grep returns `file:line:text` with line numbers up front:
    // agents overwhelmingly want the exact match location, and a file-list-only default
    // sent them re-querying in loops to recover line numbers. `files_with_matches` stays
    // available for cheap enumeration when only the set of matching files is needed.
    let output_mode = args
        .get("output_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("content");
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

    // `dot_matches_new_line(multiline)` makes `.` cross lines under multiline search —
    // the only place dotall lives in the grep API (equivalent to `rg -U --multiline-dotall`).
    // Without multiline, set the line terminator to `\n` so a `\n`-containing pattern errors
    // with rg's guidance (and the searcher keeps its fast line-by-line path) instead of
    // silently returning no matches — matching ripgrep's non-`-U` behavior.
    let mut matcher_builder = RegexMatcherBuilder::new();
    matcher_builder
        .case_insensitive(case_insensitive)
        .multi_line(multiline)
        .dot_matches_new_line(multiline);
    if !multiline {
        matcher_builder.line_terminator(Some(b'\n'));
    }
    let matcher = matcher_builder
        .build(pattern)
        .map_err(|e| (-32602, format!("Invalid regex pattern '{pattern}': {e}")))?;

    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .before_context(before)
        .after_context(after)
        .multi_line(multiline)
        .binary_detection(BinaryDetection::quit(0))
        .build();

    // Shared gitignore-style glob (child 01's helper): a slash-less glob matches the
    // basename at any depth, identical to `rg --glob`. The glob arg is split on whitespace,
    // then brace-free tokens on commas, yielding multiple patterns (Claude Code behavior).
    let glob_matcher: Option<GlobMatcher> = match glob_opt {
        Some(g) => Some(build_glob_matcher(&base, &split_grep_globs(g))?),
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
        // Default noise filter: skip minified web bundles (*.min.js/.css/.cjs/.mjs) — the
        // file-level analogue of the junk-dir excludes, same bypass semantics. `include_ignored`
        // reaches them; an explicit `glob` whitelist still has to match (a user globbing
        // `*.min.js` is not asking for the noise filter to hide their target).
        if !include_ignored && glob_matcher.is_none() {
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if super::is_minified_bundle(name) {
                    continue;
                }
            }
        }
        if let Some(ref gm) = glob_matcher {
            let rel = p.strip_prefix(&base).unwrap_or(p);
            // 명시적으로 지정된 단일 파일 `path`는 strip 결과가 빈 문자열이다. ripgrep은
            // 명시적으로 지정된 파일을 `-g`와 무관하게 항상 검색하고 글롭은 디렉터리 순회
            // 중에만 적용하므로, 이 경우 글롭 필터를 건너뛴다.
            if !rel.as_os_str().is_empty() && !gm.is_match(rel) {
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
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            files.push(FileResult {
                path: display,
                hits: sink.hits,
                occurrences: sink.occurrences,
                mtime,
            });
        }
    }

    let max_columns = crate::config::get().grep_max_columns;

    match output_mode {
        "count" => {
            // Per-file `path:count` rows (ripgrep's count output) + summary, paginated.
            // `content`/`count` keep ripgrep's native (walk) order — only
            // `files_with_matches` is mtime-sorted.
            if files.is_empty() {
                return Ok("Found 0 total occurrence(s) across 0 file(s).".to_string());
            }
            let rows: Vec<(String, usize)> =
                files.iter().map(|f| (f.path.clone(), f.occurrences)).collect();
            let (page, footer) = paginate(&rows, offset, head_limit);
            // Summary counts the post-head_limit rows (Claude Code sums the truncated slice).
            let total_occ: usize = page.iter().map(|(_, c)| c).sum();
            let mut out = String::new();
            for (p, c) in &page {
                out.push_str(&format!("{p}:{c}\n"));
            }
            out.push_str(&format!(
                "Found {total_occ} total occurrence(s) across {} file(s).",
                page.len()
            ));
            if let Some(f) = footer {
                out.push('\n');
                out.push_str(&f);
            }
            Ok(out)
        }
        "content" => {
            let mut lines: Vec<String> = Vec::new();
            for f in &files {
                for hit in &f.hits {
                    let sep = if hit.is_match { ':' } else { '-' };
                    let text = cap_line(&hit.text, max_columns, hit.is_match);
                    if show_line_numbers {
                        lines.push(format!("{}{sep}{}{sep}{}", f.path, hit.line_number, text));
                    } else {
                        lines.push(format!("{}{sep}{}", f.path, text));
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
            // Sort ONLY this mode by mtime descending, ties by filename ascending (Claude
            // Code parity). `content`/`count` above stay in ripgrep's native order.
            let mut sorted: Vec<&FileResult> = files.iter().collect();
            sorted.sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.path.cmp(&b.path)));
            let total = sorted.len();
            let paths: Vec<String> = sorted.iter().map(|f| f.path.clone()).collect();
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
