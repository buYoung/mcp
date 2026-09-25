//! Connection-local source delivery history, independent of relevance selection.
use regex::Regex;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::LazyLock;

const MAX_SOURCE_LINES: usize = 100_000;
const MAX_SOURCE_FILES: usize = 1_024;
const MAX_REVISION_BYTES: u64 = 8 * 1024 * 1024;
const REMOVED_DUPLICATE_NOTE: &str = "<--removed duplicated-->";
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^## [0-9]+\. (.+)$").unwrap());
static GROUP_ROW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([0-9]+)([:-])(.*)$").unwrap());
static GREP_ROW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)([:-])([0-9]+)([:-])(.*)$").unwrap());
type LineKey = (usize, blake3::Hash);
struct SourceFile {
    revision: blake3::Hash,
    lines: HashSet<LineKey>,
}
#[derive(Default)]
pub(super) struct SourceHistory {
    files: HashMap<PathBuf, SourceFile>,
    pending: HashMap<PathBuf, SourceFile>,
    line_count: usize,
}
#[derive(Default)]
pub(super) struct FoldedSource {
    pub spans: usize,
    pub lines: usize,
    pub saved_bytes: usize,
    /// Rendered source removed per file, excluding the replacement notice.
    pub removed_source_bytes: HashMap<String, usize>,
}
struct Row {
    path: String,
    key: LineKey,
    start: usize,
    end: usize,
}
fn display_path(path: &str) -> String {
    let p = std::path::Path::new(path);
    if let Ok(root) = std::env::current_dir() {
        if let Ok(relative) = p.strip_prefix(root) {
            return relative.to_string_lossy().into_owned();
        }
    }
    path.strip_prefix("./").unwrap_or(path).to_owned()
}
fn source_row(line: &str) -> Option<LineKey> {
    let (number, body) = line.trim_start().split_once('→')?;
    let number = number.parse::<usize>().ok().filter(|n| *n > 0)?;
    Some((number, blake3::hash(body.as_bytes())))
}
fn rows(tool: &str, arguments: &Value, text: &str) -> Vec<Row> {
    let view = crate::tools::get_arg(arguments, "view")
        .and_then(Value::as_str)
        .unwrap_or("full");
    if !matches!(view, "full" | "source" | "source_grouped")
        || (tool == "grep"
            && (crate::tools::get_arg(arguments, "-n").and_then(Value::as_bool) == Some(false)
                || crate::tools::get_arg(arguments, "output_mode")
                    .and_then(Value::as_str)
                    .is_some_and(|mode| mode != "content")))
        || (tool == "search" && crate::tools::get_arg(arguments, "event_key").is_some())
    {
        return Vec::new();
    }
    let mut path = if tool == "read" {
        ["file_path", "path", "file", "query"]
            .iter()
            .find_map(|key| crate::tools::get_arg(arguments, key).and_then(Value::as_str))
            .map(display_path)
    } else {
        None
    };
    let mut result = Vec::new();
    let mut start = 0;
    for raw in text.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        if let Some(heading) = HEADING.captures(line).filter(|_| tool != "read") {
            path = Some(display_path(heading.get(1).unwrap().as_str()));
        } else {
            let row = if tool == "grep" && view == "source" {
                GREP_ROW.captures(line).and_then(|m| {
                    Some((
                        display_path(&m[1]),
                        (m[3].parse().ok()?, blake3::hash(m[5].as_bytes())),
                    ))
                })
            } else if let Some(path) = &path {
                if let Some(key) = source_row(line) {
                    Some((path.clone(), key))
                } else if tool == "grep" && matches!(view, "full" | "source_grouped") {
                    GROUP_ROW.captures(line).and_then(|m| {
                        Some((
                            path.clone(),
                            (m[1].parse().ok()?, blake3::hash(m[3].as_bytes())),
                        ))
                    })
                } else {
                    None
                }
            } else {
                None
            };
            if let Some((path, key)) = row {
                if key.0 > 0 {
                    result.push(Row {
                        path,
                        key,
                        start,
                        end: start + raw.len(),
                    });
                }
            }
        }
        start += raw.len();
    }
    result
}
fn file_revision(path: &str) -> Option<(PathBuf, blake3::Hash)> {
    let path = crate::workspace::resolve_within_cwd(path).ok()?;
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_REVISION_BYTES {
        return None;
    }
    Some((path.clone(), blake3::hash(&std::fs::read(path).ok()?)))
}
impl SourceHistory {
    /// Delivery options must not affect retrieval or enter a Jev request body.
    pub fn tool_arguments<'a>(tool: &str, arguments: &'a Value) -> Cow<'a, Value> {
        if !matches!(tool, "search" | "read" | "grep")
            || crate::tools::get_arg(arguments, "include_seen").is_none()
        {
            return Cow::Borrowed(arguments);
        }
        let mut request = arguments.clone();
        if let Some(request) = request.as_object_mut() {
            request.retain(|key, _| {
                key.chars()
                    .filter(|c| *c != '_' && *c != '-')
                    .flat_map(char::to_lowercase)
                    .collect::<String>()
                    != "includeseen"
            });
        }
        Cow::Owned(request)
    }

    pub fn validate_arguments(tool: &str, arguments: &Value) -> Result<(), (i64, String)> {
        if matches!(tool, "search" | "read" | "grep")
            && crate::tools::get_arg(arguments, "include_seen")
                .is_some_and(|value| !value.is_boolean())
        {
            return Err((-32602, "Parameter 'include_seen' must be a boolean".into()));
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.files.clear();
        self.pending.clear();
        self.line_count = 0;
    }
    pub fn begin_request(&mut self) {
        self.discard_pending();
    }
    pub fn discard_pending(&mut self) {
        self.pending.clear();
    }
    /// Successful write/flush is not acknowledgement of client context retention.
    pub fn commit_written(&mut self) {
        for (path, source) in std::mem::take(&mut self.pending) {
            if self.line_count.saturating_add(source.lines.len()) > MAX_SOURCE_LINES
                || self.files.len() >= MAX_SOURCE_FILES
            {
                self.clear();
                return;
            }
            let entry = self.files.entry(path).or_insert_with(|| SourceFile {
                revision: source.revision,
                lines: HashSet::new(),
            });
            if entry.revision != source.revision {
                self.line_count -= entry.lines.len();
                entry.lines.clear();
                entry.revision = source.revision;
            }
            let previous = entry.lines.len();
            entry.lines.extend(source.lines);
            self.line_count += entry.lines.len() - previous;
        }
    }
    pub fn prepare(&mut self, tool: &str, arguments: &Value, response: &mut Value) -> FoldedSource {
        let mut folded = FoldedSource::default();
        if !matches!(tool, "search" | "read" | "grep")
            || response.get("isError").and_then(Value::as_bool) == Some(true)
        {
            return folded;
        }
        let Some(content) = response.get_mut("content").and_then(Value::as_array_mut) else {
            return folded;
        };
        if content.len() != 1 || content[0].get("type").and_then(Value::as_str) != Some("text") {
            return folded;
        }
        let Some(text) = content[0].get("text").and_then(Value::as_str) else {
            return folded;
        };
        let rows = rows(tool, arguments, text);
        let mut revisions = HashMap::new();
        let mut repeated = Vec::with_capacity(rows.len());
        for row in &rows {
            let revision = revisions
                .entry(row.path.clone())
                .or_insert_with(|| file_revision(&row.path));
            let was_written = revision.as_ref().is_some_and(|(path, revision)| {
                self.files.get(path).is_some_and(|source| {
                    source.revision == *revision && source.lines.contains(&row.key)
                })
            });
            let is_repeated_in_response = if let Some((path, revision)) = revision {
                !self
                    .pending
                    .entry(path.clone())
                    .or_insert_with(|| SourceFile {
                        revision: *revision,
                        lines: HashSet::new(),
                    })
                    .lines
                    .insert(row.key)
            } else {
                false
            };
            repeated.push(was_written || is_repeated_in_response);
        }
        let should_fold =
            crate::tools::get_arg(arguments, "include_seen").and_then(Value::as_bool) != Some(true);
        let mut edits = Vec::new();
        if should_fold {
            let mut start = 0;
            while start < rows.len() {
                if !repeated[start] {
                    start += 1;
                    continue;
                }
                let mut end = start + 1;
                while end < rows.len()
                    && repeated[end]
                    && rows[end].path == rows[end - 1].path
                    && rows[end].start == rows[end - 1].end
                    && rows[end - 1].key.0.checked_add(1) == Some(rows[end].key.0)
                {
                    end += 1;
                }
                let first = &rows[start];
                let last = &rows[end - 1];
                folded.spans += 1;
                folded.lines += end - start;
                *folded
                    .removed_source_bytes
                    .entry(first.path.clone())
                    .or_default() += last.end - first.start;
                edits.push(first.start..last.end);
                start = end;
            }
        }
        if !edits.is_empty() {
            let mut output = String::with_capacity(text.len());
            let mut cursor = 0;
            for range in edits {
                output.push_str(&text[cursor..range.start]);
                cursor = range.end;
            }
            output.push_str(&text[cursor..]);
            if !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            output.push_str(REMOVED_DUPLICATE_NOTE);
            folded.saved_bytes = text.len().saturating_sub(output.len());
            content[0]["text"] = Value::String(output);
        }
        folded
    }
}
