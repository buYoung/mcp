//! `grep` — literal/regex content search over file contents, powered by ripgrep's
//! `grep`/`grep-searcher`/`grep-regex` library crates. Mirrors Claude Code's Grep
//! (same parameter names `-i`/`-n`/`-A`/`-B`/`-C`, `output_mode`, `head_limit`,
//! `glob`, `type`) so an agent can swap its built-in. Respects `.gitignore` +
//! `.codemapignore` by default (decided divergence) with an `include_ignored`
//! override. Reads disk directly, so it sees comments and just-changed files the
//! BM25 index can miss — this realizes the spec's "rg 역할" alongside `search`.

mod expansion;

use super::live_symbols::{LiveAnchor, LiveOutput};
use super::{
    arg_bool, arg_required_str, arg_usize, build_glob_matcher, split_grep_globs, GlobMatcher,
};
use crate::workspace::{
    build_walker, current_dir, resolve_for_filesystem_tool, walk_root_is_visible, FilesystemTool,
};
use grep::regex::RegexMatcherBuilder;
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkContext, SinkMatch};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime};

const DEFAULT_HEAD_LIMIT: usize = 250;
/// Hard wall-clock cap for scanning non-test source files outside an extension glob/type filter.
const OUTSIDE_FILTER_SCAN_BUDGET: Duration = Duration::from_millis(400);

/// Stops searching a file on its very first match so out-of-glob summaries stay fast.
struct FirstMatchSink {
    matched: bool,
}

impl Sink for FirstMatchSink {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        _mat: &SinkMatch<'_>,
    ) -> Result<bool, std::io::Error> {
        self.matched = true;
        Ok(false)
    }
}

/// True when `glob` contains positive patterns that restrict by file extension across the search
/// root (e.g. `*.go`, `**/*.ts`, `*.{ts,tsx}`) rather than narrowing to a specific subdirectory.
fn is_extension_restricted_glob(patterns: &[String]) -> bool {
    let mut positive_count = 0usize;
    for raw in patterns {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('!') {
            continue;
        }
        positive_count += 1;
        let normalized = trimmed
            .strip_prefix("./")
            .unwrap_or(trimmed)
            .strip_prefix("**/")
            .unwrap_or(trimmed.strip_prefix("./").unwrap_or(trimmed));
        if normalized.contains('/') {
            return false;
        }
        let Some(ext) = normalized.strip_prefix("*.") else {
            return false;
        };
        if ext.is_empty() {
            return false;
        }
    }
    positive_count > 0
}

/// Programming/source files only (excluding docs/markup/data formats like `.md`, `.json`, `.yaml`)
/// so extension-filtered grep notes flag real code in sibling languages without doc/config noise.
fn is_code_source_file(path: &Path) -> bool {
    crate::lang::spec_for_path(path).is_some_and(|spec| {
        !matches!(
            spec.language_name(),
            "document" | "json" | "yaml" | "toml" | "xml" | "html" | "css" | "less"
        )
    })
}

fn summarize_outside_matches(
    candidates: &[PathBuf],
    matcher: &grep::regex::RegexMatcher,
    multiline: bool,
    ext_glob: Option<&str>,
    type_opt: Option<&str>,
) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let deadline = Instant::now() + OUTSIDE_FILTER_SCAN_BUDGET;
    let next = AtomicUsize::new(0);
    let timed_out = AtomicBool::new(false);
    let threads = crate::parallel::threads().min(candidates.len()).max(1);
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut searcher = SearcherBuilder::new()
                        .line_number(false)
                        .multi_line(multiline)
                        .binary_detection(BinaryDetection::quit(0))
                        .build();
                    let mut local: BTreeMap<String, usize> = BTreeMap::new();
                    let mut checked = 0usize;
                    loop {
                        if checked & 15 == 0 && Instant::now() >= deadline {
                            timed_out.store(true, Ordering::Relaxed);
                            break;
                        }
                        let idx = next.fetch_add(1, Ordering::Relaxed);
                        let Some(path) = candidates.get(idx) else {
                            break;
                        };
                        checked += 1;
                        let mut sink = FirstMatchSink { matched: false };
                        if searcher.search_path(matcher, path, &mut sink).is_ok() && sink.matched {
                            let key = path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|e| format!(".{}", e.to_ascii_lowercase()))
                                .unwrap_or_else(|| {
                                    path.file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("other")
                                        .to_string()
                                });
                            *local.entry(key).or_insert(0) += 1;
                        }
                    }
                    local
                })
            })
            .collect();
        for worker in workers {
            if let Ok(local) = worker.join() {
                for (ext, count) in local {
                    *totals.entry(ext).or_insert(0) += count;
                }
            }
        }
    });
    let total: usize = totals.values().sum();
    if total == 0 {
        return None;
    }
    let mut ranked: Vec<(String, usize)> = totals.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let shown: Vec<String> = ranked
        .iter()
        .take(5)
        .map(|(ext, count)| format!("{ext}: {count}"))
        .collect();
    let breakdown = if ranked.len() > 5 {
        format!("{}, +{} other(s)", shown.join(", "), ranked.len() - 5)
    } else {
        shown.join(", ")
    };
    let filter_desc = match (ext_glob, type_opt) {
        (Some(g), Some(t)) => format!("glob \"{g}\" / type \"{t}\""),
        (Some(g), None) => format!("glob \"{g}\""),
        (None, Some(t)) => format!("type \"{t}\""),
        (None, None) => "current filter".to_string(),
    };
    let plus = if timed_out.load(Ordering::Relaxed) {
        "+"
    } else {
        ""
    };
    Some(format!(
        "[Note: {total}{plus} source file(s) outside {filter_desc} also matched ({breakdown}) — widen or omit the filter to inspect them.]"
    ))
}

struct LineHit {
    line_number: u64,
    text: String,
    source_byte_len: usize,
    is_match: bool,
}

fn source_stamp(path: &std::path::Path) -> Option<(u64, SystemTime)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()?))
}

/// Inspect the matching buffer, or validate a full-source reread before rendering hits.
fn redact_hits(
    path: &std::path::Path,
    bytes: Option<&[u8]>,
    expected_stamp: Option<(u64, SystemTime)>,
    hits: &mut [LineHit],
) {
    if !crate::redact::is_enabled() || hits.is_empty() {
        return;
    }
    let owned;
    let bytes = match bytes {
        Some(bytes) => Some(bytes),
        None if expected_stamp.is_some() && expected_stamp == source_stamp(path) => {
            owned = std::fs::read(path).ok();
            owned.as_deref()
        }
        None => None,
    };
    let mut next = 0;
    if let Some(bytes) = bytes {
        let source = String::from_utf8_lossy(bytes);
        let scan = crate::redact::SourceScan::new(path, &source);
        let mut offset = 0;
        for (line, original) in source.split_inclusive('\n').enumerate() {
            if next == hits.len() {
                break;
            }
            let body = strip_eol(original);
            while next < hits.len() && hits[next].line_number == line as u64 + 1 {
                let hit = &mut hits[next];
                hit.text = if hit.text == body {
                    scan.render_range(&source, offset..offset + body.len())
                        .into_owned()
                } else {
                    crate::redact::hidden(&hit.text)
                };
                next += 1;
            }
            offset += original.len();
        }
    }
    if expected_stamp.is_some() && expected_stamp != source_stamp(path) {
        next = 0;
    }
    for hit in &mut hits[next..] {
        hit.text = crate::redact::hidden(&hit.text);
    }
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
                source_byte_len: line.len(),
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
            source_byte_len: line.len(),
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
    filter_file: Option<super::live_symbols::jev::CapturedFile>,
}

#[derive(Clone)]
struct ContentRow {
    text: String,
    path: String,
    line_number: u64,
    is_match: bool,
    is_source_complete: bool,
    filter_source: Option<String>,
}

fn content_anchors(page: &[ContentRow]) -> Vec<LiveAnchor> {
    let mut selected: Vec<usize> = page
        .iter()
        .enumerate()
        .filter_map(|(index, row)| row.is_match.then_some(index))
        .collect();
    if selected.is_empty() {
        selected.extend(page.iter().enumerate().map(|(index, _)| index));
    }
    selected.sort_unstable();
    selected
        .into_iter()
        .filter_map(|index| {
            let row = &page[index];
            let line_number = usize::try_from(row.line_number)
                .ok()
                .filter(|line_number| *line_number > 0)?;
            Some(LiveAnchor {
                file_path: row.path.clone(),
                start_line: Some(line_number),
                end_line: Some(line_number),
            })
        })
        .collect()
}

/// Cap a line at the column limit, replacing an over-long line with ripgrep's omission
/// marker (`--max-columns` parity). Matched lines and context lines get distinct markers.
/// `max_columns == 0` disables the cap. Width is measured in bytes, matching ripgrep.
fn cap_line(text: &str, source_byte_len: usize, max_columns: usize, is_match: bool) -> String {
    if max_columns > 0 && source_byte_len > max_columns {
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
    } else if end < total {
        Some(format!(
            "[Showing results {}-{} of {}; next_offset={}. Continue with offset={} and the same head_limit, or raise head_limit if you want a larger page.]",
            start + 1,
            end,
            total,
            end,
            end
        ))
    } else {
        Some(format!(
            "[Showing results {}-{} of {}; this is the last page.]",
            start + 1,
            end,
            total
        ))
    };
    (page, footer)
}

/// Slice `items` under a connection-scoped byte budget (always emitting at least one row so
/// `next_offset` strictly advances) while preserving grep's native `next_offset` footer.
fn paginate_bounded<T: Clone, F: FnMut(&T) -> usize>(
    items: &[T],
    offset: usize,
    head_limit: usize,
    byte_budget: usize,
    mut item_bytes: F,
) -> (Vec<T>, Option<String>) {
    let total = items.len();
    let start = offset.min(total);
    let limit = if head_limit == 0 { total } else { head_limit };
    let max_end = (start + limit).min(total);
    let mut end = start;
    let mut used = 0usize;
    let mut byte_capped = false;
    while end < max_end {
        let added = item_bytes(&items[end]) + usize::from(end > start);
        if end > start && used.saturating_add(added) > byte_budget {
            byte_capped = true;
            break;
        }
        used = used.saturating_add(added);
        end += 1;
    }
    let page = items[start..end].to_vec();
    let truncated = start > 0 || end < total;
    let footer = if !truncated {
        None
    } else if page.is_empty() {
        Some(format!("[No results at offset {offset}; {total} total.]"))
    } else if end < total {
        if byte_capped {
            Some(format!(
                "[Showing results {}-{} of {} (10 KiB client limit); next_offset={}. Continue with offset={} or narrow path/glob/pattern.]",
                start + 1,
                end,
                total,
                end,
                end
            ))
        } else {
            Some(format!(
                "[Showing results {}-{} of {}; next_offset={}. Continue with offset={} and the same head_limit, or raise head_limit if you want a larger page.]",
                start + 1,
                end,
                total,
                end,
                end
            ))
        }
    } else {
        Some(format!(
            "[Showing results {}-{} of {}; this is the last page.]",
            start + 1,
            end,
            total
        ))
    };
    (page, footer)
}

/// Appended to every empty result so the agent knows which narrowing filters were in
/// effect and what to do next. Observed failure mode without it: agents rerun the same
/// pattern through a shell grep, or repeat the call unchanged. Names filters only — never
/// echoes argument values — to keep the reply short.
fn empty_result_hint(
    path: &str,
    glob_opt: Option<&str>,
    type_opt: Option<&str>,
    case_insensitive: bool,
    multiline: bool,
    include_ignored: bool,
) -> String {
    let mut active_filters: Vec<&str> = Vec::new();
    if path != "." {
        active_filters.push("path");
    }
    if glob_opt.is_some() {
        active_filters.push("glob");
    }
    if type_opt.is_some() {
        active_filters.push("type");
    }
    if !case_insensitive {
        active_filters.push("case-sensitive");
    }
    if !multiline {
        active_filters.push("single-line");
    }
    if !include_ignored {
        active_filters.push("ignore files respected");
    }
    let active = if active_filters.is_empty() {
        "none".to_string()
    } else {
        active_filters.join(", ")
    };
    format!(
        "_Active filters: {active}. {} Adjust the relevant filter or pattern before retrying._",
        super::grep_regex_guidance()
    )
}

pub fn grep(args: &Value) -> Result<String, (i64, String)> {
    grep_with_metadata(args).map(|output| output.text)
}

pub(crate) fn grep_with_metadata(args: &Value) -> Result<LiveOutput, (i64, String)> {
    grep_impl(args, false)
}

pub(crate) fn grep_for_filter(args: &Value) -> Result<LiveOutput, (i64, String)> {
    grep_impl(args, true)
}

fn grep_impl(args: &Value, should_capture_bodies: bool) -> Result<LiveOutput, (i64, String)> {
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
    let options = super::live_options::LiveOptions::parse_grep(args)?;
    let case_insensitive = arg_bool(args, "-i", false);
    let multiline = arg_bool(args, "multiline", false);
    let show_line_numbers = arg_bool(args, "-n", true);
    let context_both = arg_usize(args, "-C", 0);
    let (before, after) = if options.should_expand_callable {
        (0, 0)
    } else if context_both > 0 {
        (context_both, context_both)
    } else {
        (arg_usize(args, "-B", 0), arg_usize(args, "-A", 0))
    };
    let head_limit = arg_usize(args, "head_limit", DEFAULT_HEAD_LIMIT);
    let offset = arg_usize(args, "offset", 0);
    let include_ignored = arg_bool(args, "include_ignored", false);

    let base = resolve_for_filesystem_tool(path, FilesystemTool::Grep)?;
    if !base.exists() {
        let suggestions = super::path_suggestions::note(&base);
        return Err((
            -32602,
            format!("Search path does not exist: {path}{suggestions}"),
        ));
    }
    let cwd = current_dir()?;
    // Paths in another root's index show absolute.
    let display_base = crate::config::get()
        .workspace_root
        .is_none()
        .then(|| cwd.canonicalize().unwrap_or(cwd));

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
    let glob_patterns = glob_opt.map(split_grep_globs);
    let is_ext_glob = glob_patterns
        .as_deref()
        .is_some_and(is_extension_restricted_glob);
    let glob_matcher: Option<GlobMatcher> = match glob_patterns.as_deref() {
        Some(patterns) => Some(build_glob_matcher(&base, patterns)?),
        None => None,
    };

    let walker = build_walker(&base, include_ignored);
    let type_matcher: Option<ignore::types::Types> = match type_opt {
        Some(t) => {
            let mut tb = ignore::types::TypesBuilder::new();
            tb.add_defaults();
            tb.select(t);
            Some(
                tb.build()
                    .map_err(|e| (-32602, format!("Unknown type filter '{t}': {e}")))?,
            )
        }
        None => None,
    };
    let should_scan_outside =
        base.is_dir() && output_mode != "count" && (type_opt.is_some() || is_ext_glob);
    let root_for_filter = crate::workspace::root_dir().unwrap_or_else(|_| base.clone());
    let outside_test_filter = should_scan_outside
        .then(|| crate::callers::test_code::TestCodeFilter::from_config(&root_for_filter));
    let mut outside_candidates: Vec<PathBuf> = Vec::new();

    let mut files: Vec<FileResult> = Vec::new();
    // ignore::Walk yields an explicitly named root file without running filter_entry.
    // Enforce the shared ancestor policy before it can reach search_path, including
    // mandatory internal/index exclusions when include_ignored is enabled.
    let entries = walk_root_is_visible(&base, !include_ignored)
        .then(|| walker.build())
        .into_iter()
        .flatten();
    let entries: Box<dyn Iterator<Item = _>> = if options.should_expand_callable {
        let mut entries: Vec<_> = entries.collect();
        entries.sort_by(|a, b| {
            a.as_ref()
                .ok()
                .map(|e| e.path())
                .cmp(&b.as_ref().ok().map(|e| e.path()))
        });
        Box::new(entries.into_iter())
    } else {
        Box::new(entries)
    };
    let configured_max_columns = crate::config::get().grep_max_columns;
    let max_columns = if configured_max_columns > 0 {
        configured_max_columns
    } else if crate::config::session_delivery_byte_cap().is_some() {
        600
    } else {
        0
    };
    let mut expanded = expansion::ExpansionPage::new(offset, head_limit, options.view);
    let mut content_row_count = 0usize;
    for result in entries {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let p = entry.path();
        if !include_ignored && crate::workspace::is_default_live_tool_excluded_file(p) {
            continue;
        }
        let rel = p.strip_prefix(&base).unwrap_or(p);
        let mut excluded_by_ext_filter = false;
        if let Some(ref gm) = glob_matcher {
            // 명시적으로 지정된 단일 파일 `path`는 strip 결과가 빈 문자열이다. ripgrep은
            // 명시적으로 지정된 파일을 `-g`와 무관하게 항상 검색하고 글롭은 디렉터리 순회
            // 중에만 적용하므로, 이 경우 글롭 필터를 건너뛴다.
            if !rel.as_os_str().is_empty() && !gm.is_match(rel) {
                if should_scan_outside && is_ext_glob && gm.is_unmatched_by_whitelist(rel) {
                    excluded_by_ext_filter = true;
                } else {
                    continue;
                }
            }
        }
        if let Some(ref tm) = type_matcher {
            if !rel.as_os_str().is_empty() && !tm.matched(p, false).is_whitelist() {
                if should_scan_outside {
                    excluded_by_ext_filter = true;
                } else {
                    continue;
                }
            }
        }
        if excluded_by_ext_filter {
            if is_code_source_file(p) {
                let filter_rel = p
                    .strip_prefix(&root_for_filter)
                    .unwrap_or(rel)
                    .to_string_lossy()
                    .replace('\\', "/");
                let is_test = outside_test_filter
                    .as_ref()
                    .is_some_and(|filter| filter.is_file_excluded(&filter_rel));
                if !is_test {
                    outside_candidates.push(p.to_path_buf());
                }
            }
            continue;
        }
        let mut sink = CollectSink {
            hits: Vec::new(),
            occurrences: 0,
        };
        let bytes = if options.should_expand_callable || should_capture_bodies {
            use std::io::Read;
            std::fs::File::open(p).ok().and_then(|file| {
                let cap = super::live_symbols::callable::input_byte_cap();
                let mut bytes = Vec::new();
                file.take(cap as u64 + 1).read_to_end(&mut bytes).ok()?;
                (bytes.len() <= cap).then_some(bytes)
            })
        } else {
            None
        };
        let expected_stamp =
            (crate::redact::is_enabled() && output_mode == "content" && bytes.is_none())
                .then(|| source_stamp(p))
                .flatten();
        let searched = if let Some(bytes) = bytes.as_ref() {
            // Matching, syntax bounds and rendering share this immutable live buffer.
            searcher.search_slice(&matcher, bytes, &mut sink)
        } else {
            searcher.search_path(&matcher, p, &mut sink)
        };
        if searched.is_err() {
            continue;
        }
        if sink.occurrences > 0 {
            if output_mode == "content" {
                redact_hits(p, bytes.as_deref(), expected_stamp, &mut sink.hits);
            }
            let display = display_base
                .as_ref()
                .and_then(|base| p.strip_prefix(base).ok())
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/");
            if options.should_expand_callable {
                expanded.add_file(
                    &display,
                    bytes.as_deref(),
                    &sink.hits,
                    show_line_numbers,
                    max_columns,
                    should_capture_bodies,
                );
                continue;
            }
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let is_selected = content_row_count.saturating_add(sink.hits.len()) > offset
                && (head_limit == 0 || content_row_count < offset.saturating_add(head_limit));
            content_row_count = content_row_count.saturating_add(sink.hits.len());
            let filter_file = if should_capture_bodies && is_selected {
                bytes
                    .as_deref()
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
                    .and_then(|source| {
                        super::live_symbols::jev::CapturedFile::new(
                            &display,
                            source.strip_prefix('\u{feff}').unwrap_or(source),
                        )
                    })
            } else {
                None
            };
            files.push(FileResult {
                path: display,
                hits: sink.hits,
                occurrences: sink.occurrences,
                mtime,
                filter_file,
            });
        }
    }

    let no_match_hint = || {
        empty_result_hint(
            path,
            glob_opt,
            type_opt,
            case_insensitive,
            multiline,
            include_ignored,
        )
    };

    let outside_note = if should_scan_outside {
        summarize_outside_matches(
            &outside_candidates,
            &matcher,
            multiline,
            glob_opt.filter(|_| is_ext_glob),
            type_opt,
        )
    } else {
        None
    };

    if options.should_expand_callable {
        let mut output = expanded.finish();
        if output.text.is_empty() && output.notices.is_empty() {
            output.text = match outside_note {
                Some(note) => format!("No matches found\n{note}\n{}", no_match_hint()),
                None => format!("No matches found\n{}", no_match_hint()),
            };
        } else if let Some(note) = outside_note {
            let existing_notices: usize = output.notices.iter().map(|n| n.len() + 1).sum();
            let cap = crate::config::get().grep_output_byte_cap;
            if output.text.len() + existing_notices + note.len() < cap {
                output.notices.push(note);
            }
        }
        return Ok(output);
    }
    match output_mode {
        "count" => {
            // Per-file `path:count` rows (ripgrep's count output) + summary, paginated.
            // `content`/`count` keep ripgrep's native (walk) order — only
            // `files_with_matches` is mtime-sorted.
            if files.is_empty() {
                // Keep the summary line first: `test_grep_count_mode` pins "total occurrence".
                return Ok(LiveOutput {
                    text: format!(
                        "Found 0 total occurrence(s) across 0 file(s).\n{}",
                        no_match_hint()
                    ),
                    anchors: Vec::new(),
                    notices: Vec::new(),
                    ..LiveOutput::default()
                });
            }
            let rows: Vec<(String, usize)> = files
                .iter()
                .map(|f| (f.path.clone(), f.occurrences))
                .collect();
            let (page, footer) = match crate::config::session_delivery_byte_cap() {
                Some(session_cap) => paginate_bounded(
                    &rows,
                    offset,
                    head_limit,
                    session_cap.saturating_sub(512),
                    |(p, _)| p.len() + 12,
                ),
                None => paginate(&rows, offset, head_limit),
            };
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
            let anchors = page
                .iter()
                .map(|(path, _)| LiveAnchor {
                    file_path: path.clone(),
                    start_line: None,
                    end_line: None,
                })
                .collect();
            Ok(LiveOutput {
                text: out,
                anchors,
                notices: Vec::new(),
                ..LiveOutput::default()
            })
        }
        "content" => {
            let mut lines: Vec<ContentRow> = Vec::new();
            for f in &files {
                for hit in &f.hits {
                    let sep = if hit.is_match { ':' } else { '-' };
                    let text = cap_line(&hit.text, hit.source_byte_len, max_columns, hit.is_match);
                    let rendered = if show_line_numbers {
                        format!("{}{sep}{}{sep}{}", f.path, hit.line_number, text)
                    } else {
                        format!("{}{sep}{}", f.path, text)
                    };
                    lines.push(ContentRow {
                        text: rendered,
                        path: f.path.clone(),
                        line_number: hit.line_number,
                        is_match: hit.is_match,
                        is_source_complete: max_columns == 0 || hit.source_byte_len <= max_columns,
                        filter_source: should_capture_bodies.then(|| hit.text.clone()),
                    });
                }
            }
            if lines.is_empty() {
                let text = match outside_note {
                    Some(note) => format!("No matches found\n{note}\n{}", no_match_hint()),
                    None => format!("No matches found\n{}", no_match_hint()),
                };
                return Ok(LiveOutput {
                    text,
                    anchors: Vec::new(),
                    notices: Vec::new(),
                    ..LiveOutput::default()
                });
            }
            let outside_reserve = outside_note.as_ref().map_or(0, |n| n.len() + 2);
            let (page, footer) = match crate::config::session_delivery_byte_cap() {
                Some(session_cap) => match options.view {
                    super::live_options::LiveView::SourceGrouped
                    | super::live_options::LiveView::Full => {
                        let reserve = if options.view == super::live_options::LiveView::Full {
                            2_000 + outside_reserve
                        } else {
                            320 + outside_reserve
                        };
                        let mut last_path: Option<String> = None;
                        let mut file_count = 0usize;
                        paginate_bounded(
                            &lines,
                            offset,
                            head_limit,
                            session_cap.saturating_sub(reserve),
                            |row| {
                                let suffix_len = row.text.len().saturating_sub(
                                    row.path.len() + usize::from(show_line_numbers),
                                );
                                let heading_len = if last_path.as_deref() != Some(row.path.as_str())
                                {
                                    last_path = Some(row.path.clone());
                                    file_count += 1;
                                    let digits =
                                        file_count.checked_ilog10().map_or(1, |d| d as usize + 1);
                                    row.path.len() + 12 + digits
                                } else {
                                    0
                                };
                                heading_len + suffix_len
                            },
                        )
                    }
                    _ => paginate_bounded(
                        &lines,
                        offset,
                        head_limit,
                        session_cap.saturating_sub(320 + outside_reserve),
                        |row| row.text.len(),
                    ),
                },
                None => paginate(&lines, offset, head_limit),
            };
            let mut output = LiveOutput::default();
            output.jev.judges_enclosing_bodies = true;
            output.jev.files = files
                .iter_mut()
                .filter_map(|file| file.filter_file.take())
                .collect();
            for row in &page {
                if !output.text.is_empty() {
                    output.text.push('\n');
                }
                let start = output.text.len();
                output.text.push_str(&row.text);
                output
                    .path_prefixes
                    .push(start..start + row.path.len() + usize::from(show_line_numbers));
                output.record_file(&row.path, start, output.text.len());
                if let Some(content) = &row.filter_source {
                    if let Ok(line) = usize::try_from(row.line_number) {
                        output.jev.rows.push(super::live_symbols::jev::SourceRow {
                            path: row.path.clone(),
                            line,
                            range: start..output.text.len(),
                            is_complete: row.is_source_complete,
                            content: content.clone(),
                        });
                    }
                }
                if row.is_source_complete {
                    if let Ok(line) = usize::try_from(row.line_number) {
                        output.record_source(&row.path, line, line);
                    }
                }
            }
            if let Some(f) = &footer {
                output.text.push('\n');
                output.text.push_str(f);
            }
            output.anchors = content_anchors(&page);
            output.footer = footer;
            if let Some(note) = outside_note {
                output.notices.push(note);
            }
            Ok(output)
        }
        // default: files_with_matches
        _ => {
            if files.is_empty() {
                let text = match outside_note {
                    Some(note) => format!("No matches found\n{note}\n{}", no_match_hint()),
                    None => format!("No matches found\n{}", no_match_hint()),
                };
                return Ok(LiveOutput {
                    text,
                    anchors: Vec::new(),
                    notices: Vec::new(),
                    ..LiveOutput::default()
                });
            }
            // Sort ONLY this mode by mtime descending, ties by filename ascending (Claude
            // Code parity). `content`/`count` above stay in ripgrep's native order.
            let mut sorted: Vec<&FileResult> = files.iter().collect();
            sorted.sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.path.cmp(&b.path)));
            let total = sorted.len();
            let paths: Vec<String> = sorted.iter().map(|f| f.path.clone()).collect();
            let outside_reserve = outside_note.as_ref().map_or(0, |n| n.len() + 2);
            let (page, footer) = match crate::config::session_delivery_byte_cap() {
                Some(session_cap) => paginate_bounded(
                    &paths,
                    offset,
                    head_limit,
                    session_cap.saturating_sub(512 + outside_reserve),
                    String::len,
                ),
                None => paginate(&paths, offset, head_limit),
            };
            let mut out = format!("Found {total} file(s)\n");
            out.push_str(&page.join("\n"));
            if let Some(f) = footer {
                out.push('\n');
                out.push_str(&f);
            }
            if let Some(note) = outside_note {
                out.push('\n');
                out.push_str(&note);
            }
            let anchors = page
                .iter()
                .map(|path| LiveAnchor {
                    file_path: path.clone(),
                    start_line: None,
                    end_line: None,
                })
                .collect();
            Ok(LiveOutput {
                text: out,
                anchors,
                notices: Vec::new(),
                ..LiveOutput::default()
            })
        }
    }
}
