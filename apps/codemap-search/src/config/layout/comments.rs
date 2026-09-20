//! Relocate TOML comments without reading or rewriting string values as comments.

use toml_edit::{Decor, DocumentMut, Item, Key, Table};
use std::collections::BTreeMap;

use super::{table_at_mut, SECTION_ALIASES, SECTION_MOVES, SETTINGS};

type Relocations = Vec<(String, String)>;

fn canonical_section(section: &str) -> Option<String> {
    for &(legacy, target) in SECTION_ALIASES.iter().chain(SECTION_MOVES) {
        if section == legacy {
            return Some(target.into());
        }
        if let Some(suffix) = section.strip_prefix(&format!("{legacy}.")) {
            return Some(format!("{target}.{suffix}"));
        }
    }
    if let Some(suffix) = section.strip_prefix("exclude.test_")
        .or_else(|| section.strip_prefix("caller_context.test_"))
    {
        return Some(format!("output.context.exclude.test_{suffix}"));
    }
    let target = match section {
        "exclude" => "index.exclude",
        "search" => "output.search",
        "tool_output" => "output",
        "caller_context" => "output.context",
        "refresh" => "index.refresh",
        "language_support" => "index.language_support",
        _ if section.starts_with("output.") || section.starts_with("index.") => section,
        "output" | "index" | "analysis" => section,
        _ => return None,
    };
    Some(target.into())
}

fn destination(section: &str, key: &str) -> Option<(String, String)> {
    for &(target, keys) in super::super::exclude::SECTIONS {
        if keys.contains(&key) {
            return Some((target.into(), key.into()));
        }
    }
    if let Some(setting) = SETTINGS.iter().find(|setting| setting.internal == key) {
        return Some((setting.section.into(), setting.key.into()));
    }
    let target = canonical_section(section)?;
    if SETTINGS.iter().any(|setting| setting.section == target && setting.key == key)
        || SECTION_MOVES.iter().any(|(_, section)| target == *section)
        || target.starts_with("output.context.exclude.test_")
    {
        return Some((target, key.into()));
    }
    None
}

fn comment_body(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix('#').map(str::trim_start)
}

fn header(line: &str) -> Option<(&str, String)> {
    let body = comment_body(line)?;
    if body.starts_with("[[") {
        return None;
    }
    let section = body.strip_prefix('[')?.split_once(']')?.0;
    Some((section, canonical_section(section)?))
}

fn assignment(line: &str) -> Option<&str> {
    let (key, _) = comment_body(line)?.split_once('=')?;
    let key = key.trim();
    (!key.is_empty()
        && key.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
        .then_some(key)
}

fn deliver(
    pending: &mut String,
    section: &str,
    target: &str,
    kept: &mut String,
    moved: &mut Relocations,
) {
    if target == section {
        kept.push_str(pending);
    } else if !pending.trim().is_empty() {
        let text = pending.lines().map(|line| {
            header(line).map_or_else(|| line.to_string(), |(legacy, _)| {
                line.replacen(legacy, target, 1)
            })
        }).collect::<Vec<_>>().join("\n");
        moved.push((target.into(), text));
    }
    pending.clear();
}

/// Recognized inactive assignments carry their preceding explanation. Incomplete
/// examples and comments whose destination is unknown stay where they were.
fn split_examples(text: &str, section: &str, example_scope: &str, moved: &mut Relocations) -> String {
    let lines: Vec<_> = text.split_inclusive('\n').collect();
    let mut kept = String::new();
    let mut pending = String::new();
    let mut scope = example_scope.to_string();
    let mut has_pending_header = false;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if let Some((legacy, target)) = header(line) {
            pending.push_str(&line.replacen(legacy, &target, 1));
            scope = target;
            has_pending_header = true;
            index += 1;
            continue;
        }
        if let Some(key) = assignment(line) {
            if let Some((target, renamed)) = destination(&scope, key) {
                let mut end = index + 1;
                let mut example = comment_body(line).unwrap().to_string();
                while toml::from_str::<toml::Value>(&example).is_err() && end < lines.len() {
                    let Some(body) = comment_body(lines[end]) else { break };
                    example.push('\n');
                    example.push_str(body);
                    end += 1;
                }
                if toml::from_str::<toml::Value>(&example).is_ok() {
                    pending.push_str(&line.replacen(key, &renamed, 1));
                    for continuation in &lines[index + 1..end] {
                        pending.push_str(continuation);
                    }
                    deliver(&mut pending, section, &target, &mut kept, moved);
                    has_pending_header = false;
                    index = end;
                    continue;
                }
            }
        }
        pending.push_str(line);
        index += 1;
    }
    let target = if has_pending_header { scope.as_str() } else { section };
    deliver(&mut pending, section, target, &mut kept, moved);
    kept
}

fn append(document: &mut DocumentMut, section: &str, text: &str) -> Result<(), String> {
    if !text.contains('#') {
        return Ok(());
    }
    let table = table_at_mut(document.as_table_mut(), section)?;
    let existing = table.decor().suffix().and_then(|raw| raw.as_str()).unwrap_or("");
    let suffix = format!("{existing}\n{}\n", text.trim_matches(['\r', '\n']));
    table.decor_mut().set_suffix(suffix);
    Ok(())
}

pub(super) fn move_section(
    document: &mut DocumentMut,
    key: &Key,
    item: &Item,
    target: &str,
) -> Result<(), String> {
    let decor = match item {
        Item::Table(table) => Some(table.decor()),
        Item::Value(value) => Some(value.decor()),
        _ => None,
    };
    let mut moved = Vec::new();
    for raw in [
        key.leaf_decor().prefix(),
        key.leaf_decor().suffix(),
        decor.and_then(Decor::prefix),
        decor.and_then(Decor::suffix),
    ].into_iter().flatten().filter_map(|raw| raw.as_str()) {
        let remaining = split_examples(raw, target, target, &mut moved);
        append(document, target, &remaining)?;
    }
    for (section, text) in moved {
        append(document, &section, &text)?;
    }
    Ok(())
}

fn relocate_decor(decor: &mut Decor, section: &str, prefix_scope: &str, moved: &mut Relocations) {
    if let Some(raw) = decor.prefix().and_then(|raw| raw.as_str()) {
        let updated = split_examples(raw, section, prefix_scope, moved);
        decor.set_prefix(updated);
    }
    if let Some(raw) = decor.suffix().and_then(|raw| raw.as_str()) {
        let updated = split_examples(raw, section, section, moved);
        decor.set_suffix(updated);
    }
}

fn collect_order(table: &Table, section: &str, order: &mut Vec<(isize, String)>) {
    if let Some(position) = table.position() {
        order.push((position, section.into()));
    }
    for (name, item) in table.iter() {
        let path = if section.is_empty() { name.into() } else { format!("{section}.{name}") };
        match item {
            Item::Table(child) => collect_order(child, &path, order),
            Item::ArrayOfTables(children) => {
                for child in children.iter() {
                    if let Some(position) = child.position() {
                        order.push((position, format!("{path}[]")));
                    }
                }
            }
            _ => {}
        }
    }
}

fn walk(table: &mut Table, section: &str, previous: &BTreeMap<String, String>, moved: &mut Relocations) {
    // TOML attaches comments before a header to that header. An inactive setting
    // there still belongs to the preceding section, even if headers are reordered.
    let prefix_scope = previous.get(section).map(String::as_str).unwrap_or(section);
    relocate_decor(table.decor_mut(), section, prefix_scope, moved);
    for (mut key, item) in table.iter_mut() {
        relocate_decor(key.leaf_decor_mut(), section, section, moved);
        let path = if section.is_empty() {
            key.get().to_string()
        } else {
            format!("{section}.{}", key.get())
        };
        match item {
            Item::Table(child) => walk(child, &path, previous, moved),
            Item::Value(value) => relocate_decor(value.decor_mut(), section, section, moved),
            // Rule entries move with their array. A section path alone cannot
            // identify which entry owns an inactive example inside that array.
            Item::ArrayOfTables(_) | Item::None => {}
        }
    }
}

pub(super) fn relocate_examples(document: &mut DocumentMut) -> Result<(), String> {
    let mut moved = Vec::new();
    let mut order = Vec::new();
    collect_order(document.as_table(), "", &mut order);
    order.sort_by_key(|(position, _)| *position);
    let mut previous = BTreeMap::new();
    let mut last = String::new();
    for (_, section) in order {
        previous.insert(section.clone(), last);
        last = section;
    }
    walk(document.as_table_mut(), "", &previous, &mut moved);
    let trailing = split_examples(document.trailing().as_str().unwrap_or(""), "", &last, &mut moved);
    document.set_trailing(trailing);
    for (section, text) in moved {
        append(document, &section, &text)?;
    }
    Ok(())
}
