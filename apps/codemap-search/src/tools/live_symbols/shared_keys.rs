//! Key trailer for `read view=source`: other code using the same string keys or constant names
//! as the window. Code that never calls each other still meets through a shared key (a queue,
//! event, hook, cache or session entry), so changing one side can break the other. Matching is
//! by exact text and nothing is resolved; source rows above the trailer stay unchanged.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::structure::callable;
use super::LiveOutput;
use crate::index::EngineSupervisor;
use crate::parser::ExtractedFile;

/// Keys taken from one window, in order of appearance.
const WINDOW_KEY_LIMIT: usize = 24;
/// Places (an enclosing function, or a top-level line) listed per key before the rest are counted.
const PLACES_PER_KEY: usize = 8;
/// A key used in more files than this is too common to point at related code; it is skipped.
const COMMON_KEY_FILE_LIMIT: usize = 12;
/// Bytes for the whole trailer, including its header.
const TRAILER_BYTE_CAP: usize = 1536;
/// Time for scanning the index. Places found by then are listed with a note that the scan stopped.
const TRAILER_TIME_BUDGET: Duration = Duration::from_millis(100);
const PARTIAL_LINE: &str =
    "\n- key scan stopped at its time limit; more places may exist (grep the key)";
pub(crate) const HEADER: &str =
    "[Same keys used outside this window (exact text match, approximate)]";

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
    let started = Instant::now();
    let byte_cap = byte_cap.min(TRAILER_BYTE_CAP);
    if !output.should_list_outside_callers || byte_cap < HEADER.len() + PARTIAL_LINE.len() + 64 {
        return None;
    }
    let [anchor] = output.anchors.as_slice() else {
        return None;
    };
    let (start, end) = anchor.start_line.zip(anchor.end_line)?;
    if engine.is_warming() || engine.is_dead() || engine.last_error().is_some() {
        return None;
    }
    let root = std::env::current_dir().ok()?;
    let path = anchor.file_path.as_str();
    let filter = crate::callers::test_code::TestCodeFilter::from_config(&root);
    if filter.is_file_excluded(path) {
        return None;
    }
    let snapshot = engine.published_snapshot();
    let files = snapshot.codemap();
    let file = files.iter().find(|file| file.file_path == path)?;
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
    let wanted: HashMap<String, usize> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key.text.clone(), index))
        .collect();
    let deadline = started + TRAILER_TIME_BUDGET;
    let mut is_partial = false;
    for (index, other) in files.iter().enumerate() {
        if index % 64 == 0 && Instant::now() >= deadline {
            is_partial = true;
            break;
        }
        if filter.is_file_excluded(&other.file_path) {
            continue;
        }
        let is_window_file = other.file_path == path;
        let mut add = |text: &str, line: usize| {
            if let Some(&key) = wanted.get(text) {
                if !(is_window_file && start <= line && line <= end) {
                    keys[key].sites.push((index, line));
                }
            }
        };
        for literal in &other.literals {
            add(&literal.text, literal.line);
        }
        if let Some(navigation) = &other.navigation {
            for site in &navigation.key_sites {
                add(&site.name, site.line);
            }
        }
    }
    trailer_text(&files, path, keys, is_partial, byte_cap)
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

/// Whether `text` on 1-based `line` is a call argument: after skipping its quotes (strings) or a
/// qualifier such as `Type.`, `self::` or `$this->` (names), the previous non-blank character,
/// possibly at the end of an earlier line, is `(` or `,`.
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
        if previous_symbol(lines, line - 1, before).is_some_and(|c| c == '(' || c == ',') {
            return true;
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

fn trailer_text(
    files: &[ExtractedFile],
    path: &str,
    keys: Vec<KeyUse>,
    is_partial: bool,
    byte_cap: usize,
) -> Option<String> {
    let reserve =
        omission_line(WINDOW_KEY_LIMIT).len() + if is_partial { PARTIAL_LINE.len() } else { 0 };
    // Keys reaching other files only, in window order; read windows and callers cover one file.
    let listed: Vec<KeyUse> = keys
        .into_iter()
        .filter(|key| {
            let mut file_count: Vec<usize> = key.sites.iter().map(|(file, _)| *file).collect();
            file_count.sort_unstable();
            file_count.dedup();
            key.sites
                .iter()
                .any(|(file, _)| files[*file].file_path != path)
                && file_count.len() <= COMMON_KEY_FILE_LIMIT
        })
        .collect();
    let mut text = String::from(HEADER);
    let mut omitted = 0;
    for key in &listed {
        let line = key_line(files, path, key);
        if omitted > 0 || text.len() + line.len() + reserve > byte_cap {
            omitted += 1;
            continue;
        }
        text.push_str(&line);
    }
    if text.len() == HEADER.len() {
        return None;
    }
    if omitted > 0 {
        text.push_str(&omission_line(omitted));
    }
    if is_partial {
        text.push_str(PARTIAL_LINE);
    }
    Some(text)
}

fn omission_line(count: usize) -> String {
    format!("\n- {count} more shared key(s) (grep them)")
}

/// `- 'key': other/file.ts: fn L12, L14, Owner.method L30; this file: L80, +2 more places`
fn key_line(files: &[ExtractedFile], path: &str, key: &KeyUse) -> String {
    // (file index, place label) -> lines, other files first, then by path and first line.
    let mut places: Vec<((usize, String), Vec<usize>)> = Vec::new();
    let mut sites = key.sites.clone();
    sites.sort_by(|(a_file, a_line), (b_file, b_line)| {
        let (a_path, b_path) = (&files[*a_file].file_path, &files[*b_file].file_path);
        (a_path == path, a_path, a_line).cmp(&(b_path == path, b_path, b_line))
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
    let mut line = format!("\n- {shown_key}: ");
    let mut current_file = None;
    for ((file, label), lines) in places.iter().take(PLACES_PER_KEY) {
        if current_file != Some(*file) {
            if current_file.is_some() {
                line.push_str("; ");
            }
            let file_path = &files[*file].file_path;
            line.push_str(if file_path == path {
                "this file"
            } else {
                file_path
            });
            line.push_str(": ");
            current_file = Some(*file);
        } else {
            line.push_str(", ");
        }
        let numbers = lines
            .iter()
            .map(|number| format!("L{number}"))
            .collect::<Vec<_>>()
            .join(", ");
        if label.is_empty() {
            line.push_str(&numbers);
        } else {
            line.push_str(&format!("{label} {numbers}"));
        }
    }
    if places.len() > PLACES_PER_KEY {
        line.push_str(&format!(", +{} more places", places.len() - PLACES_PER_KEY));
    }
    line
}

/// `Owner.name` of the innermost function containing `line`, or empty at top level.
fn enclosing_label(file: &ExtractedFile, line: usize) -> String {
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
