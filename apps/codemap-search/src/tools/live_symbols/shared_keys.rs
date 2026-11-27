//! Key trailer for `read view=source`: other code using the same string keys or constant names
//! as the window. Code that never calls each other still meets through a shared key (a queue,
//! event, hook, cache or session entry), so changing one side can break the other. Matching is
//! by exact text and nothing is resolved; source rows above the trailer stay unchanged.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::structure::callable;
use super::LiveOutput;
use crate::index::EngineSupervisor;
use crate::parser::ExtractedFile;

/// Keys taken from one window, in order of appearance.
const WINDOW_KEY_LIMIT: usize = 40;
/// Places (an enclosing function, or a top-level line) listed per key before the rest are counted.
const PLACES_PER_KEY: usize = 4;
/// A key used in more files than this is too common to point at related code; it is skipped.
const COMMON_KEY_FILE_LIMIT: usize = 12;
/// Bytes for the whole trailer, including its header.
const TRAILER_BYTE_CAP: usize = 1536;
/// Time cap so trailer lookup stays bounded on very large snapshots.
const TRAILER_TIME_BUDGET: Duration = Duration::from_millis(100);
/// Per-key byte ceiling (without / with live snippets) so one noisy key cannot starve the rest.
const MAX_KEY_LINE_BYTES: usize = 320;
const MAX_KEY_LINE_BYTES_WITH_SNIPPET: usize = 460;
/// Live 1-line snippet caps for top rare / cross-language keys.
const MAX_SNIPPETS_PER_TRAILER: usize = 5;
const MAX_SNIPPET_CHARS: usize = 96;
pub(crate) const HEADER: &str =
    "[Same keys used outside this window (exact text match, approximate)]";

/// Inverted index from exact key text to `(file_index, line)` sites in a published snapshot.
/// Built lazily once per snapshot generation so shared-key lookups run in O(1) without scanning
/// the repository linearly or starving alphabetically later directories under load.
#[derive(Clone, Debug, Default)]
pub(crate) struct SharedKeyLookup {
    sites_by_key: HashMap<Box<str>, Vec<(usize, usize)>>,
}

impl SharedKeyLookup {
    pub(crate) fn build(files: &[Arc<ExtractedFile>]) -> Self {
        let mut sites_by_key: HashMap<Box<str>, Vec<(usize, usize)>> = HashMap::new();
        for (index, file) in files.iter().enumerate() {
            for literal in &file.literals {
                if is_key_literal(&literal.text) {
                    sites_by_key
                        .entry(literal.text.as_str().into())
                        .or_default()
                        .push((index, literal.line));
                }
            }
            if let Some(navigation) = &file.navigation {
                for site in &navigation.key_sites {
                    sites_by_key
                        .entry(site.name.as_str().into())
                        .or_default()
                        .push((index, site.line));
                }
            }
        }
        Self { sites_by_key }
    }

    pub(crate) fn sites(&self, key: &str) -> &[(usize, usize)] {
        self.sites_by_key.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// One key of the window and the places that use it outside the window.
struct KeyUse {
    text: String,
    is_literal: bool,
    sites: Vec<(usize, usize)>,
}

/// The trailer for a read window, within `byte_cap` bytes and never above [`TRAILER_BYTE_CAP`].
pub(super) fn trailer(
    engine: &EngineSupervisor,
    output: &LiveOutput,
    byte_cap: usize,
) -> Option<String> {
    let byte_cap = byte_cap.min(TRAILER_BYTE_CAP);
    if !output.should_list_outside_callers || byte_cap < HEADER.len() + 64 {
        return None;
    }
    let [anchor] = output.anchors.as_slice() else {
        return None;
    };
    let (start, end) = anchor.start_line.zip(anchor.end_line)?;
    if engine.is_warming() || engine.is_dead() || engine.last_error().is_some() {
        return None;
    }
    let root = crate::workspace::root_dir().ok()?;
    let path = anchor.file_path.as_str();
    let filter = crate::callers::test_code::TestCodeFilter::from_config(&root);
    if filter.is_file_excluded(path) {
        return None;
    }
    let snapshot = engine.published_snapshot();
    let files = snapshot.codemap();
    let file_index = files
        .binary_search_by(|file| file.file_path.as_str().cmp(path))
        .ok()?;
    let file = &files[file_index];
    // Stale or unindexed files would point at the wrong lines.
    if super::diagnostics::unavailable(Some(file), anchor, &root, snapshot.flows().digest(path))
        .is_some()
    {
        return None;
    }
    let source = std::fs::read_to_string(root.join(path)).ok()?;
    let lines: Vec<&str> = source.lines().collect();
    let mut keys = window_keys(file, &lines, start, end);
    if keys.is_empty() {
        return None;
    }
    let lookup = snapshot.shared_key_lookup();
    let started = Instant::now();
    let mut excluded_cache: HashMap<usize, bool> = HashMap::new();
    for key in &mut keys {
        if started.elapsed() >= TRAILER_TIME_BUDGET {
            break;
        }
        let mut distinct_files: Vec<usize> = Vec::with_capacity(COMMON_KEY_FILE_LIMIT + 1);
        for &(index, line) in lookup.sites(&key.text) {
            let is_excluded = *excluded_cache
                .entry(index)
                .or_insert_with(|| filter.is_file_excluded(&files[index].file_path));
            if is_excluded {
                continue;
            }
            if index == file_index && start <= line && line <= end {
                continue;
            }
            if !distinct_files.contains(&index) {
                distinct_files.push(index);
                if distinct_files.len() > COMMON_KEY_FILE_LIMIT {
                    key.sites.clear();
                    break;
                }
            }
            key.sites.push((index, line));
        }
    }
    trailer_text(&root, &files, path, keys, byte_cap)
}

/// String keys and constant names inside lines `start..=end`, first appearance first.
fn window_keys(file: &ExtractedFile, lines: &[&str], start: usize, end: usize) -> Vec<KeyUse> {
    let mut found: Vec<(usize, String, bool)> = file
        .literals
        .iter()
        .filter(|literal| {
            start <= literal.line
                && literal.line <= end
                && is_key_literal(&literal.text)
                && is_argument(lines, literal.line, &literal.text, true)
        })
        .map(|literal| (literal.line, literal.text.clone(), true))
        .collect();
    if let Some(navigation) = &file.navigation {
        found.extend(
            navigation
                .key_sites
                .iter()
                .filter(|site| {
                    start <= site.line
                        && site.line <= end
                        && is_argument(lines, site.line, &site.name, false)
                })
                .map(|site| (site.line, site.name.clone(), false)),
        );
    }
    found.sort_by_key(|(line, _, _)| *line);
    let mut keys: Vec<KeyUse> = Vec::new();
    for (_, text, is_literal) in found {
        if let Some(key) = keys.iter_mut().find(|key| key.text == text) {
            key.is_literal |= is_literal;
        } else if keys.len() < WINDOW_KEY_LIMIT {
            keys.push(KeyUse {
                text,
                is_literal,
                sites: Vec::new(),
            });
        }
    }
    keys
}

/// Whether `text` on 1-based `line` appears in a key-bearing position:
/// - for string literals (`is_literal == true`): call arguments (`(`, `,`), constant/value
///   assignments and equality checks (`=`, `:=`, `==`, `===`, `!=`, `=>`), map/struct/case
///   entries (`:`), or `case "..."` branches across languages;
/// - for identifier key sites (`is_literal == false`): call arguments (`(`, `,`) only.
fn is_argument(lines: &[&str], line: usize, text: &str, is_literal: bool) -> bool {
    let Some(row) = line.checked_sub(1).and_then(|index| lines.get(index)) else {
        return false;
    };
    let mut search_from = 0;
    while let Some(found) = row[search_from..].find(text) {
        let at = search_from + found;
        search_from = at + text.len();
        let before = if is_literal {
            match row[..at].strip_suffix(['\'', '"', '`']) {
                Some(rest) => rest,
                None => continue,
            }
        } else {
            let is_part_of_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
            if row[..at].ends_with(is_part_of_name)
                || row[at + text.len()..].starts_with(is_part_of_name)
            {
                continue;
            }
            strip_qualifiers(&row[..at])
        };
        let prev = previous_symbol(lines, line - 1, before);
        if prev.is_some_and(|c| c == '(' || c == ',') {
            return true;
        }
        if is_literal {
            if prev.is_some_and(|c| c == '=' || c == ':') {
                return true;
            }
            let trimmed = before.trim_end();
            if let Some(prefix) = trimmed.strip_suffix("case") {
                if prefix.is_empty()
                    || !prefix
                        .chars()
                        .last()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    return true;
                }
            }
        }
    }
    false
}

/// `before` without trailing `Name.`, `Name::` or `$name->` qualifier segments.
fn strip_qualifiers(mut before: &str) -> &str {
    loop {
        let Some(rest) = before
            .strip_suffix('.')
            .or_else(|| before.strip_suffix("::"))
            .or_else(|| before.strip_suffix("->"))
        else {
            return before;
        };
        let name_start = rest
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
            .map_or(0, |index| index + 1);
        if name_start == rest.len() {
            return before;
        }
        before = &rest[..name_start];
    }
}

/// The last non-blank character of `before`, or of one of the three earlier lines.
fn previous_symbol(lines: &[&str], index: usize, before: &str) -> Option<char> {
    if let Some(c) = before.trim_end().chars().last() {
        return Some(c);
    }
    lines[..index]
        .iter()
        .rev()
        .take(3)
        .find_map(|row| row.trim_end().chars().last())
}

/// A string that can name a shared entry: one token with an inner separator or mixed case.
/// Plain words, sentences, paths, templates and numbers are left out.
fn is_key_literal(text: &str) -> bool {
    let length = text.chars().count();
    (3..=80).contains(&length)
        && text.chars().any(char::is_alphabetic)
        && !text.chars().any(|c| {
            c.is_whitespace()
                || matches!(
                    c,
                    '/' | '\\'
                        | '%'
                        | '{'
                        | '}'
                        | '$'
                        | '<'
                        | '>'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | ','
                        | ';'
                        | '='
                        | '"'
                        | '\''
                        | '`'
                        | '#'
                        | '?'
                        | '&'
                        | '*'
                        | '+'
                        | '|'
                )
        })
        && !text.starts_with(['.', '@', '-', ':'])
        && (text.contains(['_', '-', ':', '.'])
            || (text.chars().any(char::is_uppercase) && text.chars().any(char::is_lowercase)))
}

/// Coarse language family for ranking cross-language key bridges ahead of same-language keys.
fn language_family(path: &str) -> &str {
    let path = Path::new(path);
    crate::callers::symbols::language_family(path)
        .or_else(|| path.extension().and_then(|ext| ext.to_str()))
        .unwrap_or("")
}

fn is_cross_language(left_path: &str, right_path: &str) -> bool {
    let left = language_family(left_path);
    let right = language_family(right_path);
    !left.is_empty() && !right.is_empty() && left != right
}

fn trailer_text(
    root: &Path,
    files: &[std::sync::Arc<ExtractedFile>],
    path: &str,
    keys: Vec<KeyUse>,
    byte_cap: usize,
) -> Option<String> {
    let reserve = omission_line(WINDOW_KEY_LIMIT).len();
    // Keys reaching other files only; prioritize rare cross-language contracts, then rare
    // same-language keys, ahead of broader accidental collisions.
    let mut ranked: Vec<(u8, usize, usize, KeyUse)> = keys
        .into_iter()
        .enumerate()
        .filter_map(|(order, key)| {
            let mut distinct_files: Vec<usize> = key.sites.iter().map(|(file, _)| *file).collect();
            distinct_files.sort_unstable();
            distinct_files.dedup();
            if distinct_files.len() > COMMON_KEY_FILE_LIMIT {
                return None;
            }
            let outside_count = distinct_files
                .iter()
                .filter(|&&file| files[file].file_path != path)
                .count();
            if outside_count == 0 {
                return None;
            }
            let has_cross_lang = distinct_files
                .iter()
                .any(|&file| is_cross_language(&files[file].file_path, path));
            let tier: u8 = match (has_cross_lang, outside_count) {
                (true, 1..=4) => 0,
                (false, 1..=3) => 1,
                (true, _) => 2,
                (false, _) => 3,
            };
            Some((tier, outside_count, order, key))
        })
        .collect();
    ranked.sort_by_key(|(tier, outside_count, order, _)| (*tier, *outside_count, *order));
    let mut text = String::from(HEADER);
    let mut omitted = 0;
    let mut snippets_left = MAX_SNIPPETS_PER_TRAILER;
    let mut file_cache: HashMap<String, Option<String>> = HashMap::new();
    for (tier, _, _, key) in &ranked {
        if omitted > 0 {
            omitted += 1;
            continue;
        }
        let snippet_budget = if snippets_left == 0 {
            0
        } else if *tier == 0 {
            snippets_left.min(2)
        } else if *tier == 1 {
            1
        } else {
            0
        };
        let (line, snippets_used) =
            key_line(root, files, path, key, snippet_budget, &mut file_cache);
        if text.len() + line.len() + reserve > byte_cap {
            // Retry without snippets before dropping a high-signal key at the budget boundary.
            let (compact_line, _) = if snippets_used > 0 {
                key_line(root, files, path, key, 0, &mut file_cache)
            } else {
                (line, 0)
            };
            if text.len() + compact_line.len() + reserve > byte_cap {
                omitted += 1;
                continue;
            }
            text.push_str(&compact_line);
            continue;
        }
        snippets_left = snippets_left.saturating_sub(snippets_used);
        text.push_str(&line);
    }
    if text.len() == HEADER.len() {
        return None;
    }
    if omitted > 0 {
        text.push_str(&omission_line(omitted));
    }
    Some(text)
}

fn omission_line(count: usize) -> String {
    format!("\n- {count} more shared key(s) (grep them)")
}

/// `- 'key': other/file.ts: fn L12 (`...`), L14, Owner.method L30; this file: L80, +2 more places`
fn key_line(
    root: &Path,
    files: &[std::sync::Arc<ExtractedFile>],
    path: &str,
    key: &KeyUse,
    snippet_budget: usize,
    file_cache: &mut HashMap<String, Option<String>>,
) -> (String, usize) {
    // (file index, place label) -> lines: cross-language other files first, then other files,
    // then this file, each ordered by path and first line.
    let mut places: Vec<((usize, String), Vec<usize>)> = Vec::new();
    let mut sites = key.sites.clone();
    sites.sort_by(|(a_file, a_line), (b_file, b_line)| {
        let (a_path, b_path) = (&files[*a_file].file_path, &files[*b_file].file_path);
        (
            a_path == path,
            !is_cross_language(a_path, path),
            a_path,
            a_line,
        )
            .cmp(&(
                b_path == path,
                !is_cross_language(b_path, path),
                b_path,
                b_line,
            ))
    });
    sites.dedup();
    for (file, line) in sites {
        let label = enclosing_label(&files[file], line);
        match places
            .iter_mut()
            .find(|(place, _)| place.0 == file && place.1 == label)
        {
            Some((_, lines)) => lines.push(line),
            None => places.push(((file, label), vec![line])),
        }
    }
    // Top-level lines of the window's file, or of a file where the key is also used inside a
    // function, are mostly imports and declarations; the functions are what can break.
    let files_with_functions: Vec<usize> = places
        .iter()
        .filter(|((_, label), _)| !label.is_empty())
        .map(|((file, _), _)| *file)
        .collect();
    places.retain(|((file, label), _)| {
        !label.is_empty()
            || (files[*file].file_path != path && !files_with_functions.contains(file))
    });
    let shown_key = if key.is_literal {
        format!("'{}'", key.text)
    } else {
        key.text.clone()
    };
    let line_byte_cap = if snippet_budget > 0 {
        MAX_KEY_LINE_BYTES_WITH_SNIPPET
    } else {
        MAX_KEY_LINE_BYTES
    };
    let mut line = format!("\n- {shown_key}: ");
    let mut current_file = None;
    let mut emitted_places = 0usize;
    let mut snippets_used = 0usize;
    let mut snippet_files: Vec<usize> = Vec::new();
    for ((file, label), lines) in places.iter().take(PLACES_PER_KEY) {
        let file_path = &files[*file].file_path;
        let allow_snippet = snippets_used < snippet_budget
            && file_path != path
            && !snippet_files.contains(file)
            && (snippets_used == 0 || is_cross_language(file_path, path));
        let snippet = if allow_snippet {
            lines.first().and_then(|&first_line| {
                live_snippet(root, file_path, first_line, &key.text, file_cache)
            })
        } else {
            None
        };
        let mut piece = String::new();
        if current_file != Some(*file) {
            if current_file.is_some() {
                piece.push_str("; ");
            }
            piece.push_str(if file_path == path {
                "this file"
            } else {
                file_path
            });
            piece.push_str(": ");
        } else {
            piece.push_str(", ");
        }
        let numbers = if let Some(ref code) = snippet {
            let mut parts = vec![format!("L{} ({code})", lines[0])];
            parts.extend(lines[1..].iter().map(|number| format!("L{number}")));
            parts.join(", ")
        } else {
            lines
                .iter()
                .map(|number| format!("L{number}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        if label.is_empty() {
            piece.push_str(&numbers);
        } else {
            piece.push_str(&format!("{label} {numbers}"));
        }
        if emitted_places > 0 && line.len() + piece.len() > line_byte_cap {
            break;
        }
        line.push_str(&piece);
        current_file = Some(*file);
        emitted_places += 1;
        if snippet.is_some() {
            snippets_used += 1;
            snippet_files.push(*file);
        }
    }
    if places.len() > emitted_places {
        line.push_str(&format!(", +{} more places", places.len() - emitted_places));
    }
    (line, snippets_used)
}

/// Read and live-verify a single source line from `rel_path`, returning a compact inline Markdown
/// code span only when the live line still contains `key_text`.
fn live_snippet(
    root: &Path,
    rel_path: &str,
    line_num: usize,
    key_text: &str,
    file_cache: &mut HashMap<String, Option<String>>,
) -> Option<String> {
    if line_num == 0 {
        return None;
    }
    let max_size = crate::config::get().max_file_size;
    let abs_path = root.join(rel_path);
    let content = file_cache
        .entry(rel_path.to_string())
        .or_insert_with(|| {
            let meta = std::fs::metadata(&abs_path).ok()?;
            if meta.len() > max_size {
                return None;
            }
            std::fs::read_to_string(&abs_path).ok()
        })
        .as_deref()?;
    let mut offset = 0usize;
    let mut target_range = None;
    for (idx, chunk) in content.split_inclusive('\n').enumerate() {
        if idx + 1 == line_num {
            let body = chunk
                .strip_suffix('\n')
                .unwrap_or(chunk)
                .strip_suffix('\r')
                .unwrap_or(chunk.strip_suffix('\n').unwrap_or(chunk));
            target_range = Some((offset, offset + body.len(), body));
            break;
        }
        offset += chunk.len();
    }
    let (start_byte, end_byte, raw_line) = target_range?;
    // Live verification: never quote a stale line if the file changed since indexing.
    if !raw_line.contains(key_text) {
        return None;
    }
    let rendered = if crate::redact::is_enabled() {
        crate::redact::SourceScan::new(&abs_path, content)
            .render_range(content, start_byte..end_byte)
            .into_owned()
    } else {
        raw_line.to_string()
    };
    let normalized = rendered.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    let clipped = clip_around_key(&normalized, key_text, MAX_SNIPPET_CHARS);
    Some(if clipped.contains('`') {
        format!("`` {clipped} ``")
    } else {
        format!("`{clipped}`")
    })
}

fn clip_around_key(text: &str, key_text: &str, max_chars: usize) -> String {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    let key_char_pos = text
        .find(key_text)
        .and_then(|byte_pos| chars.iter().position(|(b, _)| *b >= byte_pos))
        .unwrap_or(0);
    if key_char_pos + key_text.chars().count() <= max_chars.saturating_sub(1) {
        let end_byte = chars[max_chars.saturating_sub(1)].0;
        return format!("{}…", &text[..end_byte]);
    }
    let window = max_chars.saturating_sub(2);
    let start_char = key_char_pos
        .saturating_sub(window / 3)
        .min(chars.len().saturating_sub(window));
    let end_char = (start_char + window).min(chars.len());
    let start_byte = chars[start_char].0;
    let end_byte = if end_char < chars.len() {
        chars[end_char].0
    } else {
        text.len()
    };
    let prefix = if start_char > 0 { "…" } else { "" };
    let suffix = if end_char < chars.len() { "…" } else { "" };
    format!("{prefix}{}{suffix}", &text[start_byte..end_byte])
}

/// `Owner.name` of the innermost function containing `line`, or empty at top level.
pub(super) fn enclosing_label(file: &ExtractedFile, line: usize) -> String {
    file.symbols
        .iter()
        .filter(|symbol| {
            callable(symbol)
                && symbol.range.start_line <= line
                && line <= symbol.range.end_line_inclusive()
        })
        .min_by_key(|symbol| symbol.range.end_line_inclusive() - symbol.range.start_line)
        .map(|symbol| {
            let name = symbol.name.split_whitespace().collect::<Vec<_>>().join(" ");
            match &symbol.owner {
                Some(owner) => format!("{owner}.{name}"),
                None => name,
            }
        })
        .unwrap_or_default()
}
