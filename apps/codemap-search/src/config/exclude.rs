//! Canonical test-exclusion settings and the v8 section relocation.

use super::{normalize_config_section, ConfigLayer};
use std::path::Path;
use toml_edit::{DocumentMut, Item, Key, Table, TableLike};

pub(super) const TEST_KEYS: &[&str] = &[
    "should_include_test_code",
    "test_file_patterns",
    "test_attributes",
    "test_decorators",
    "test_calls",
];

/// Apply valid canonical values after the legacy aliases, per language for rule maps.
pub(super) fn normalize_section(layer: &mut ConfigLayer, value: &toml::Value, path: &Path) {
    let mut canonical = ConfigLayer::default();
    normalize_config_section(&mut canonical, "exclude", value, path);
    layer.should_include_test_code = canonical
        .should_include_test_code
        .or(layer.should_include_test_code);
    layer.test_code_rules.file_patterns = canonical
        .test_code_rules
        .file_patterns
        .or(layer.test_code_rules.file_patterns.take());
    layer
        .test_code_rules
        .attributes
        .extend(canonical.test_code_rules.attributes);
    layer
        .test_code_rules
        .decorators
        .extend(canonical.test_code_rules.decorators);
    layer
        .test_code_rules
        .calls
        .extend(canonical.test_code_rules.calls);
}

fn take_settings(source: &mut dyn TableLike, target: &mut Table) {
    for &name in TEST_KEYS {
        let Some(key) = source.key(name).cloned() else {
            continue;
        };
        if let Some(mut value) = source.remove(name) {
            if let Some(table) = value.as_table_mut() {
                table.set_position(None);
                table.set_dotted(false);
            }
            target.insert_formatted(&key, value);
        }
    }
}

fn inherit_setting(target: &mut Table, key: Key, fallback: Item) {
    let Some(existing) = target.get_mut(key.get()) else {
        target.insert_formatted(&key, fallback);
        return;
    };
    // Lists replace lists; only language tables inherit missing language entries.
    if matches!(
        key.get(),
        "test_attributes" | "test_decorators" | "test_calls"
    ) {
        if let (Some(existing), Some(fallback)) =
            (existing.as_table_like_mut(), fallback.as_table_like())
        {
            for (name, value) in fallback.iter() {
                if let Some(key) = fallback.key(name) {
                    existing.entry_format(key).or_insert(value.clone());
                }
            }
        }
    }
}

fn section_table(key: &Key, value: Item) -> Result<Table, String> {
    match value {
        Item::Table(table) => Ok(table),
        Item::Value(toml_edit::Value::InlineTable(inline)) => {
            // InlineTable::into_table formats entries and drops the wrapper decoration.
            // Transfer the original keys/values and attach its comments to the new header.
            let mut table = Table::new();
            *table.decor_mut() = inline.decor().clone();
            if let Some(prefix) = key.leaf_decor().prefix() {
                table.decor_mut().set_prefix(prefix.clone());
            }
            for (name, value) in inline.iter() {
                if let Some(key) = inline.key(name) {
                    table.insert_formatted(key, Item::Value(value.clone()));
                }
            }
            Ok(table)
        }
        _ => Err("exclude must be a table to migrate safely".into()),
    }
}

fn without_test_settings(mut value: toml::Value) -> toml::Value {
    if let Some(root) = value.as_table_mut() {
        for &key in TEST_KEYS {
            root.remove(key);
        }
        for section in ["caller_context", "exclude"] {
            if let Some(table) = root.get_mut(section).and_then(toml::Value::as_table_mut) {
                for &key in TEST_KEYS {
                    table.remove(key);
                }
                if table.is_empty() {
                    root.remove(section);
                }
            }
        }
    }
    value
}

pub(super) fn migrate(contents: &str, original: &str, path: &Path) -> Result<String, String> {
    let before: toml::Value = toml::from_str(original).map_err(|error| error.to_string())?;
    // The version marker belongs to the document, not the first setting's decoration.
    let marker = contents
        .split_inclusive('\n')
        .next()
        .filter(|line| line.trim_start().starts_with(super::VERSION_MARKER_PREFIX));
    let mut body = marker
        .map_or(contents, |line| &contents[line.len()..])
        .to_string();
    let mut generated_comments = Vec::new();
    for migration in super::MIGRATIONS
        .iter()
        .filter(|migration| migration.version == 7)
    {
        for block in [migration.english_block, migration.korean_block] {
            if body.contains(block) {
                body = body.replacen(block, "", 1);
                generated_comments.push(block);
            }
        }
    }
    let mut document: DocumentMut = body
        .parse()
        .map_err(|error: toml_edit::TomlError| error.to_string())?;
    let mut target = match document.remove_entry("exclude") {
        Some((key, value)) => section_table(&key, value)?,
        None => Table::new(),
    };
    let mut legacy = Table::new();
    take_settings(document.as_table_mut(), &mut legacy);
    if let Some(caller) = document
        .get_mut("caller_context")
        .and_then(Item::as_table_like_mut)
    {
        take_settings(caller, &mut legacy);
    }
    for &name in TEST_KEYS {
        if let Some((key, value)) = legacy.remove_entry(name) {
            inherit_setting(&mut target, key, value);
        }
    }
    target.set_position(None);
    target.set_dotted(false);
    target.set_implicit(false);
    if !generated_comments.is_empty() {
        let suffix = target
            .decor()
            .suffix()
            .and_then(|suffix| suffix.as_str())
            .unwrap_or("")
            .to_owned();
        target
            .decor_mut()
            .set_suffix(format!("{suffix}\n\n{}", generated_comments.join("\n\n")));
    }
    document.insert("exclude", Item::Table(target));
    let header = marker
        .map(|line| format!("{}\n", line.trim_end_matches(['\r', '\n'])))
        .unwrap_or_default();
    let updated = format!("{header}{document}");
    let after: toml::Value = toml::from_str(&updated).map_err(|error| error.to_string())?;
    // Compare presence as well as values: an explicit default must not become an
    // omitted key that would inherit a different global value after migration.
    let before_config = super::normalize(before.clone(), path);
    let after_config = super::normalize(after.clone(), path);
    if before_config.should_include_test_code != after_config.should_include_test_code
        || before_config.test_code_rules != after_config.test_code_rules
        || without_test_settings(before) != without_test_settings(after)
    {
        return Err("section relocation would change effective or unrelated settings".into());
    }
    Ok(updated)
}
