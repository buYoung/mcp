//! Refresh only recognized generated prose; retain user notes, assignments and string contents.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::config_locale::ConfigCommentLanguage;

use super::super::{
    config_template, config_value_ranges, CONFIG_TEMPLATE, CONFIG_TEMPLATE_KO, MIGRATIONS,
    VERSION_MARKER_PREFIX,
};

// Exact retired template/migration lines, not patterns that could match arbitrary user notes.
const LEGACY_COMMENTS: &str = include_str!("legacy_comments.txt");

fn assignment_name(line: &str) -> Option<&str> {
    let body = line.trim().strip_prefix('#').unwrap_or(line).trim();
    let (key, _) = body.split_once('=')?;
    let key = key.trim().trim_matches(['\'', '"']);
    (!key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
    .then_some(key)
}

fn section_name(line: &str) -> Option<&str> {
    let body = line.trim().strip_prefix('#').unwrap_or(line).trim();
    if body.starts_with("[[") {
        return None;
    }
    body.strip_prefix('[')?
        .split_once(']')
        .map(|(name, _)| name)
}

fn prose_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|line| {
        line.starts_with('#')
            && !line.starts_with(VERSION_MARKER_PREFIX)
            && assignment_name(line).is_none()
            && section_name(line).is_none()
    })
}

fn setting_descriptions(template: &str) -> BTreeMap<(&str, &str), String> {
    let mut descriptions = BTreeMap::new();
    let mut section = "";
    let mut comments = Vec::new();
    for line in template.lines() {
        if !line.starts_with('#') {
            if let Some(name) = section_name(line) {
                section = name;
                comments.clear();
                continue;
            }
        }
        if let Some(key) = assignment_name(line) {
            if !comments.is_empty() {
                descriptions.insert((section, key), comments.join("\n"));
            }
            comments.clear();
        } else if line.starts_with('#') {
            comments.push(line);
        }
    }
    descriptions
}

/// The language of a file's generated comments, so a sync keeps it. The OS locale decides only
/// when neither template's prose dominates, for example in a hand-written file.
pub(in crate::config) fn comment_language(contents: &str) -> ConfigCommentLanguage {
    let english: BTreeSet<&str> = prose_lines(CONFIG_TEMPLATE).collect();
    let korean: BTreeSet<&str> = prose_lines(CONFIG_TEMPLATE_KO).collect();
    let (mut english_lines, mut korean_lines) = (0usize, 0usize);
    for line in prose_lines(contents) {
        english_lines += usize::from(english.contains(line));
        korean_lines += usize::from(korean.contains(line));
    }
    match english_lines.cmp(&korean_lines) {
        Ordering::Greater => ConfigCommentLanguage::English,
        Ordering::Less => ConfigCommentLanguage::Korean,
        Ordering::Equal => crate::config_locale::config_comment_language(),
    }
}

pub(super) fn refresh(contents: &str) -> String {
    let template = config_template(comment_language(contents));
    let descriptions = setting_descriptions(template);
    let mut generated: BTreeSet<&str> = [CONFIG_TEMPLATE, CONFIG_TEMPLATE_KO, LEGACY_COMMENTS]
        .into_iter()
        .flat_map(prose_lines)
        .collect();
    for migration in MIGRATIONS {
        generated.extend(prose_lines(migration.english_block));
        generated.extend(prose_lines(migration.korean_block));
    }
    let ranges = config_value_ranges(contents);
    let mut section = "";
    let mut example_scope = "";
    let mut marker = "";
    let mut body = String::new();
    let mut inactive_assignment: Option<String> = None;
    let mut offset = 0;
    for line in contents.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        // Includes multiline strings and comments inside arrays: neither belongs to our prose.
        if ranges.iter().any(|range| range.contains(&start)) {
            body.push_str(line);
            continue;
        }
        let trimmed = line.trim();
        // A commented multiline value is still user data, not a sequence of new settings.
        if let Some(value) = inactive_assignment.as_mut() {
            if let Some(continuation) = line.trim_start().strip_prefix('#') {
                value.push_str(continuation);
                if toml::from_str::<toml::Value>(value).is_ok() {
                    inactive_assignment = None;
                }
                body.push_str(line);
                continue;
            }
            if trimmed.is_empty() {
                value.push('\n');
                body.push_str(line);
                continue;
            }
            inactive_assignment = None;
        }
        if trimmed.starts_with(VERSION_MARKER_PREFIX) && marker.is_empty() {
            marker = trimmed;
            continue;
        }
        if generated.contains(trimmed) {
            continue;
        }
        if trimmed.starts_with("[[") {
            section = "";
            example_scope = "";
        }
        if let Some(name) = section_name(trimmed) {
            if trimmed.starts_with('#') {
                example_scope = name;
                if name == section {
                    continue;
                }
            } else {
                section = name;
                example_scope = name;
            }
        }
        if let Some(key) = assignment_name(trimmed) {
            if let Some(value) = trimmed.strip_prefix('#') {
                if toml::from_str::<toml::Value>(value).is_err() {
                    inactive_assignment = Some(format!("{value}\n"));
                }
            }
            let scope = if trimmed.starts_with('#') {
                example_scope
            } else {
                section
            };
            if let Some(description) = descriptions.get(&(scope, key)) {
                body.push_str(description);
                body.push('\n');
            }
        }
        if trimmed.is_empty() && (body.is_empty() || body.ends_with("\n\n")) {
            continue;
        }
        body.push_str(line);
    }
    let header = template
        .lines()
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .collect::<Vec<_>>()
        .join("\n");
    let marker = if marker.is_empty() {
        String::new()
    } else {
        format!("{marker}\n")
    };
    format!(
        "{marker}{}\n\n{}\n",
        header.trim_end(),
        body.trim_matches(['\r', '\n'])
    )
}
