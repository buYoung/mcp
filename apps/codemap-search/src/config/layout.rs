//! Canonical configuration paths and the value-preserving section migration.

use std::path::Path;
use toml_edit::{DocumentMut, Item, Key, Table};

use super::{assign_config_key, ConfigLayer};

mod comments;

pub(super) struct Setting {
    pub section: &'static str,
    pub key: &'static str,
    pub internal: &'static str,
    pub legacy_section: &'static str,
}

macro_rules! settings {
    ($(($section:literal, $key:literal, $internal:literal, $legacy:literal)),* $(,)?) => {
        pub(super) const SETTINGS: &[Setting] = &[
            $(Setting { section: $section, key: $key, internal: $internal, legacy_section: $legacy }),*
        ];
    };
}

settings![
    (
        "output",
        "is_redact_enabled",
        "is_redact_enabled",
        "tool_output"
    ),
    ("output", "max_bytes", "output_byte_cap", ""),
    (
        "output.client",
        "claude_max_result_chars",
        "claude_max_result_chars",
        ""
    ),
    (
        "output.client",
        "codex_output_token_limit",
        "codex_output_token_limit",
        ""
    ),
    (
        "output.overview",
        "is_stats_enabled",
        "is_overview_stats_enabled",
        "tool_output"
    ),
    (
        "output.overview",
        "max_bytes",
        "overview_output_byte_cap",
        ""
    ),
    (
        "output.search",
        "detail_file_limit",
        "result_threshold",
        "search"
    ),
    (
        "output.search",
        "overview_file_limit",
        "search_overview_file_limit",
        "search"
    ),
    (
        "output.search",
        "snippet_max_lines",
        "search_detail_snippet_max_lines",
        "search"
    ),
    (
        "output.search",
        "symbol_limit",
        "search_detail_symbol_limit",
        "search"
    ),
    (
        "output.search",
        "max_bytes",
        "search_detail_byte_cap",
        "search"
    ),
    (
        "output.search",
        "literal_max_chars",
        "search_literal_max_len",
        "search"
    ),
    (
        "output.search",
        "literal_limit",
        "search_literal_limit",
        "search"
    ),
    (
        "output.search",
        "anchor_snippet_limit",
        "search_anchor_snippet_limit",
        "search"
    ),
    (
        "output.read",
        "max_bytes",
        "read_output_byte_cap",
        "tool_output"
    ),
    (
        "output.grep",
        "max_columns",
        "grep_max_columns",
        "tool_output"
    ),
    ("output.grep", "max_bytes", "grep_output_byte_cap", ""),
    (
        "output.context",
        "is_enabled",
        "caller_context_default",
        "caller_context"
    ),
    (
        "output.context",
        "caller_limit",
        "caller_list_cap",
        "caller_context"
    ),
    (
        "output.context",
        "callee_limit",
        "callee_list_cap",
        "caller_context"
    ),
    (
        "output.context",
        "max_bytes",
        "annotation_sub_budget",
        "caller_context"
    ),
    (
        "output.context",
        "common_name_threshold",
        "common_name_threshold",
        "caller_context"
    ),
    (
        "output.context",
        "caller_omit_def_threshold",
        "caller_omit_def_threshold",
        "caller_context"
    ),
    ("index", "path", "index_path", "index"),
    ("index", "max_file_bytes", "max_file_size", "index"),
    (
        "index",
        "store_references",
        "navigation_store_references",
        "caller_context"
    ),
    ("index.refresh", "watch", "watch", "refresh"),
    (
        "index.refresh",
        "watch_debounce_ms",
        "watch_debounce_ms",
        "refresh"
    ),
    (
        "index.refresh",
        "index_staleness_ms",
        "index_staleness_ms",
        "refresh"
    ),
    (
        "index.refresh",
        "indexer_auto_restart",
        "indexer_auto_restart",
        "refresh"
    ),
    (
        "index.language_support",
        "is_document_support_enabled",
        "is_document_support_enabled",
        "language_support"
    ),
    (
        "index.language_support",
        "is_shell_support_enabled",
        "is_shell_support_enabled",
        "language_support"
    ),
    (
        "index.language_support",
        "is_infrastructure_support_enabled",
        "is_infrastructure_support_enabled",
        "language_support"
    ),
    (
        "index.language_support",
        "is_interface_support_enabled",
        "is_interface_support_enabled",
        "language_support"
    ),
    (
        "index.language_support",
        "is_build_support_enabled",
        "is_build_support_enabled",
        "language_support"
    ),
    (
        "output.navigation",
        "is_enabled",
        "navigation_context_default",
        "caller_context"
    ),
    (
        "output.navigation",
        "callsite_budget",
        "navigation_callsite_budget",
        "caller_context"
    ),
    (
        "output.navigation",
        "scan_limit",
        "scan_cap",
        "caller_context"
    ),
];

const SECTION_MOVES: &[(&str, &str)] = &[
    ("redact", "output.redact"),
    ("macro_expansion", "output.macro_expansion"),
    ("event_navigation", "output.event_navigation"),
];

const SECTION_ALIASES: &[(&str, &str)] = &[
    ("analysis.navigation", "output.navigation"),
    ("analysis.macro_expansion", "output.macro_expansion"),
    ("analysis.event_navigation", "output.event_navigation"),
];

pub(super) fn is_canonical_key(section: &str, key: &str) -> bool {
    let full = format!("{section}.{key}");
    SETTINGS.iter().any(|setting| {
        (setting.section == section && setting.key == key)
            || setting.section == full
            || setting.section.starts_with(&format!("{full}."))
    }) || SECTION_MOVES.iter().any(|(_, target)| *target == full)
        || SECTION_ALIASES.iter().any(|(legacy, _)| *legacy == full)
        || super::exclude::SECTIONS
            .iter()
            .any(|(section, _)| *section == full || section.starts_with(&format!("{full}.")))
}

fn value_at<'a>(value: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    path.split('.').try_fold(value, |value, key| value.get(key))
}

/// The new spelling wins within a file; file precedence is still repo > global.
pub(super) fn normalize(layer: &mut ConfigLayer, value: &toml::Value, path: &Path) {
    if let Some(navigation) = value_at(value, "analysis.navigation") {
        normalize_table(layer, "output.navigation", navigation, path);
    }
    for section in ["output", "index", "analysis"] {
        if let Some(value) = value.get(section) {
            normalize_table(layer, section, value, path);
        }
    }
    for &(legacy, canonical) in SECTION_MOVES {
        let alias = SECTION_ALIASES
            .iter()
            .find(|(_, target)| *target == canonical)
            .and_then(|(alias, _)| value_at(value, alias).map(|value| (*alias, value)));
        let canonical_value = value_at(value, canonical).map(|value| (canonical, value));
        if alias.is_none() && canonical_value.is_none() {
            continue;
        }
        let mut merged = value
            .get(legacy)
            .and_then(toml::Value::as_table)
            .cloned()
            .unwrap_or_default();
        for (section, value) in [alias, canonical_value].into_iter().flatten() {
            if let Some(table) = value.as_table() {
                merged.extend(table.clone());
            } else {
                super::warn(&format!(
                    "config '{section}' must be a table: {} — ignored",
                    path.display()
                ));
            }
        }
        let merged = toml::Value::Table(merged);
        match legacy {
            "redact" => layer.redact = super::redact::normalize(&merged, path),
            "macro_expansion" => {
                layer.macro_expansion = super::macro_expansion::normalize(&merged, path)
            }
            "event_navigation" => {
                layer.event_navigation = super::event_navigation::normalize(&merged, path)
            }
            _ => unreachable!(),
        }
    }
}

fn normalize_table(layer: &mut ConfigLayer, section: &str, value: &toml::Value, path: &Path) {
    if super::exclude::SECTIONS
        .iter()
        .any(|(name, _)| *name == section)
    {
        super::exclude::normalize_section(layer, section, value, path);
        return;
    }
    let Some(table) = value.as_table() else {
        super::warn(&format!(
            "config '{section}' must be a table: {} — ignored",
            path.display()
        ));
        return;
    };
    for (key, value) in table {
        let full = format!("{section}.{key}");
        if let Some(setting) = SETTINGS
            .iter()
            .find(|setting| setting.section == section && setting.key == key)
        {
            assign_config_key(layer, setting.internal, value, &full, path);
        } else if SECTION_MOVES.iter().any(|(_, target)| *target == full)
            || SECTION_ALIASES.iter().any(|(legacy, _)| *legacy == full)
        {
            // These sections own their validation and per-key legacy fallback.
        } else if is_canonical_key(section, key) {
            normalize_table(layer, &full, value, path);
        } else if !super::section_accepts_key(section, key) {
            super::warn(&format!(
                "unknown config key '{full}': {} — ignored",
                path.display()
            ));
        }
    }
}

fn table_at_mut<'a>(table: &'a mut Table, path: &str) -> Result<&'a mut Table, String> {
    let mut table = table;
    for part in path.split('.') {
        if !table.contains_key(part) {
            table.insert(part, Item::Table(Table::new()));
        }
        let key = table.key(part).cloned();
        let item = table.get_mut(part).unwrap();
        if let Item::Value(toml_edit::Value::InlineTable(inline)) = item {
            let mut converted = Table::new();
            *converted.decor_mut() = inline.decor().clone();
            if let Some(prefix) = key.as_ref().and_then(|key| key.leaf_decor().prefix()) {
                let inline_prefix = converted
                    .decor()
                    .prefix()
                    .and_then(|raw| raw.as_str())
                    .unwrap_or("");
                let prefix = format!("{}{inline_prefix}", prefix.as_str().unwrap_or(""));
                converted.decor_mut().set_prefix(prefix);
            }
            for (name, value) in inline.iter() {
                if let Some(key) = inline.key(name) {
                    converted.insert_formatted(key, Item::Value(value.clone()));
                }
            }
            *item = Item::Table(converted);
        }
        table = item
            .as_table_mut()
            .ok_or_else(|| format!("'{path}' must be a table to migrate safely"))?;
        table.set_dotted(false);
    }
    Ok(table)
}

fn legacy_entry(document: &DocumentMut, section: &str, key: &str) -> Option<(Key, Item)> {
    let table = document.get(section).and_then(Item::as_table_like);
    table
        .and_then(|table| Some((table.key(key)?.clone(), table.get(key)?.clone())))
        .or_else(|| {
            Some((
                document.as_table().key(key)?.clone(),
                document.get(key)?.clone(),
            ))
        })
}

fn section_at<'a>(document: &'a DocumentMut, path: &str) -> Option<&'a Item> {
    let mut parts = path.split('.');
    let mut item = document.get(parts.next()?)?;
    for part in parts {
        item = item.as_table_like()?.get(part)?;
    }
    Some(item)
}

fn move_section(
    document: &mut DocumentMut,
    original: &DocumentMut,
    legacy: &str,
    canonical: &str,
) -> Result<(), String> {
    let Some(source) = section_at(original, legacy) else {
        return Ok(());
    };
    let source = source
        .as_table_like()
        .ok_or_else(|| format!("'{legacy}' must be a table to migrate safely"))?;
    let target = table_at_mut(document.as_table_mut(), canonical)?;
    for (name, value) in source.iter() {
        if let Some(key) = source.key(name) {
            insert_setting(target, name, (key.clone(), value.clone()));
        }
    }
    let removed = if let Some((parent, key)) = legacy.rsplit_once('.') {
        table_at_mut(document.as_table_mut(), parent)?.remove_entry(key)
    } else {
        document.remove_entry(legacy)
    };
    if let Some((key, item)) = removed {
        comments::move_section(document, &key, &item, canonical)?;
    }
    Ok(())
}

fn insert_setting(table: &mut Table, key: &str, source: (Key, Item)) {
    if table.contains_key(key) {
        return;
    }
    let (original, value) = source;
    let mut renamed = Key::new(key);
    *renamed.leaf_decor_mut() = original.leaf_decor().clone();
    table.insert_formatted(&renamed, value);
}

fn move_exclusions(document: &mut DocumentMut, original: &DocumentMut) -> Result<(), String> {
    let Some(source) = original.get("exclude") else {
        return Ok(());
    };
    let source = source
        .as_table_like()
        .ok_or("'exclude' must be a table to migrate safely")?;
    for &(section, keys) in super::exclude::SECTIONS {
        let target = table_at_mut(document.as_table_mut(), section)?;
        for &name in keys {
            if let (Some(key), Some(value)) = (source.key(name), source.get(name)) {
                super::exclude::inherit_setting(target, key.clone(), value.clone());
            }
        }
    }
    if let Some(source) = document
        .get_mut("exclude")
        .and_then(Item::as_table_like_mut)
    {
        for &key in super::exclude::WORKSPACE_KEYS
            .iter()
            .chain(super::exclude::TEST_KEYS)
        {
            source.remove(key);
        }
    }
    Ok(())
}

fn remove_legacy(document: &mut DocumentMut, section: &str, key: &str) {
    document.remove(key);
    if let Some(table) = document.get_mut(section).and_then(Item::as_table_like_mut) {
        table.remove(key);
    }
}

fn append_example(table: &mut Table, comment: &str, assignment: &str) {
    let suffix = table
        .decor()
        .suffix()
        .and_then(|raw| raw.as_str())
        .unwrap_or("");
    let suffix = format!("{suffix}\n# {comment}\n# {assignment}\n");
    table.decor_mut().set_suffix(suffix);
}

/// Reuse the localized template's descriptions without replacing user comments or values.
fn restore_setting_comments(document: &mut DocumentMut) -> Result<(), String> {
    let template = super::config_template(super::config_comment_language());
    let mut section = "";
    let mut comments = Vec::new();
    for line in template.lines() {
        let line = line.trim();
        if let Some(header) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = header;
            comments.clear();
            continue;
        }
        let body = line.strip_prefix('#').map(str::trim_start).unwrap_or(line);
        let assignment = body.split_once('=').filter(|(key, _)| {
            let key = key.trim();
            !key.is_empty()
                && key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        });
        if let Some((name, _)) = assignment {
            let name = name.trim();
            let exists = section_at(document, section)
                .and_then(Item::as_table_like)
                .is_some_and(|table| table.contains_key(name));
            if exists && !comments.is_empty() {
                let table = table_at_mut(document.as_table_mut(), section)?;
                if let Some((mut key, value)) = table.remove_entry(name) {
                    let prefix = key
                        .leaf_decor()
                        .prefix()
                        .and_then(|raw| raw.as_str())
                        .unwrap_or("");
                    let description = comments.join("\n");
                    if !prefix.contains(&description) {
                        let prefix = format!("{prefix}\n{description}\n");
                        key.leaf_decor_mut().set_prefix(prefix);
                    }
                    table.insert_formatted(&key, value);
                }
            }
            comments.clear();
        } else if line.starts_with('#') {
            comments.push(line);
        }
    }
    Ok(())
}

fn order_tables(table: &mut Table, path: &str, next: &mut isize) {
    let order: &[&str] = match path {
        "" => &[
            "output",
            "index",
            "analysis",
            "filesystem_permissions",
            "update",
        ],
        "output" => &[
            "client",
            "overview",
            "search",
            "read",
            "grep",
            "context",
            "navigation",
            "macro_expansion",
            "event_navigation",
            "redact",
        ],
        "index" => &["exclude", "refresh", "language_support"],
        _ => &[],
    };
    let mut names: Vec<String> = order
        .iter()
        .filter(|name| table.contains_key(name))
        .map(|name| name.to_string())
        .collect();
    names.extend(
        table
            .iter()
            .filter(|(name, _)| !order.contains(name))
            .map(|(name, _)| name.to_string()),
    );
    for name in names {
        let child_path = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}.{name}")
        };
        match table.get_mut(&name) {
            Some(Item::Table(child)) => {
                child.set_position(Some(*next));
                *next += 1;
                order_tables(child, &child_path, next);
            }
            Some(Item::ArrayOfTables(children)) => {
                for child in children.iter_mut() {
                    child.set_position(Some(*next));
                    *next += 1;
                    order_tables(child, &child_path, next);
                }
            }
            _ => {}
        }
    }
    // Keep the existing key path while displaying shared output exclusions last.
    if path == "output" {
        if let Some(exclude) = table
            .get_mut("context")
            .and_then(Item::as_table_mut)
            .and_then(|context| context.get_mut("exclude"))
            .and_then(Item::as_table_mut)
        {
            exclude.set_position(Some(*next));
            *next += 1;
            order_tables(exclude, "output.context.exclude", next);
        }
    }
}

pub(super) fn migrate(contents: &str, path: &Path) -> Result<String, String> {
    let before: toml::Value = toml::from_str(contents).map_err(|error| error.to_string())?;
    let marker = contents
        .split_inclusive('\n')
        .next()
        .filter(|line| line.trim_start().starts_with(super::VERSION_MARKER_PREFIX));
    let body = marker.map_or(contents, |line| &contents[line.len()..]);
    let mut document: DocumentMut = body
        .parse()
        .map_err(|error: toml_edit::TomlError| error.to_string())?;
    comments::relocate_examples(&mut document)?;
    let original = document.clone();
    // v18 aliases win over the older flat spelling, but never replace canonical values.
    for &(legacy, canonical) in SECTION_ALIASES {
        move_section(&mut document, &original, legacy, canonical)?;
    }
    for setting in SETTINGS
        .iter()
        .filter(|setting| !setting.legacy_section.is_empty())
    {
        if let Some(entry) = legacy_entry(&original, setting.legacy_section, setting.internal) {
            let target = table_at_mut(document.as_table_mut(), setting.section)?;
            insert_setting(target, setting.key, entry);
        }
    }
    for setting in SETTINGS
        .iter()
        .filter(|setting| !setting.legacy_section.is_empty())
    {
        remove_legacy(&mut document, setting.legacy_section, setting.internal);
    }
    for &(legacy, canonical) in SECTION_MOVES {
        move_section(&mut document, &original, legacy, canonical)?;
    }
    move_exclusions(&mut document, &original)?;
    for (section, target) in [
        ("search", "output.search"),
        ("tool_output", "output"),
        ("caller_context", "output.context"),
        ("refresh", "index.refresh"),
        ("language_support", "index.language_support"),
        ("exclude", "index.exclude"),
    ] {
        if document
            .get(section)
            .and_then(Item::as_table_like)
            .is_some_and(|table| table.is_empty())
        {
            if let Some((key, item)) = document.remove_entry(section) {
                comments::move_section(&mut document, &key, &item, target)?;
            }
        }
    }
    for section in [
        "output",
        "output.client",
        "output.overview",
        "output.search",
        "output.read",
        "output.grep",
        "output.context",
        "output.navigation",
        "output.macro_expansion",
        "output.event_navigation",
        "index",
        "analysis",
    ] {
        table_at_mut(document.as_table_mut(), section)?.set_implicit(false);
    }
    let language = super::config_comment_language();
    for (section, key, assignment, english, korean) in [
        (
            "output",
            "max_bytes",
            "max_bytes = \"1mb\"",
            "Optional common MCP response ceiling; omitted keeps existing tool defaults.",
            "선택적인 MCP 공통 응답 한도. 생략하면 기존 도구 기본값을 유지합니다.",
        ),
        (
            "output.client",
            "claude_max_result_chars",
            "claude_max_result_chars = 200000",
            "Claude characters (maximum 500000); reconnect MCP after changing.",
            "Claude 문자 한도(최대 500000). 변경 후 MCP를 재연결하세요.",
        ),
        (
            "output.client",
            "codex_output_token_limit",
            "codex_output_token_limit = 50000",
            "Export with codemap-search codex-config; apply the fragment in Codex.",
            "codemap-search codex-config로 출력한 설정 조각을 Codex에 적용하세요.",
        ),
    ] {
        if !super::file_mentions_key(contents, key) {
            append_example(
                table_at_mut(document.as_table_mut(), section)?,
                language.select(english, korean),
                assignment,
            );
        }
    }
    restore_setting_comments(&mut document)?;
    order_tables(document.as_table_mut(), "", &mut 0);
    let updated = format!("{}{document}", marker.unwrap_or(""));
    let after: toml::Value = toml::from_str(&updated).map_err(|error| error.to_string())?;
    if super::normalize(before, path) != super::normalize(after, path) {
        return Err("section relocation would change configured values or inheritance; leaving the file untouched".into());
    }
    Ok(updated)
}
