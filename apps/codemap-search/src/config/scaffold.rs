//! Localized initial configuration and the one-time v6 directory-list migration.
//! TOML spans keep edits confined to the existing array, preserving user comments.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use serde::Deserialize;
use toml::Spanned;

use crate::workspace::exclusions::{recommended_directories, DirectoryExclusions};

#[derive(Default, Deserialize)]
struct DirectorySection {
    excluded_directories: Option<Spanned<Vec<Spanned<String>>>>,
}

#[derive(Deserialize)]
struct DirectorySettings {
    excluded_directories: Option<Spanned<Vec<Spanned<String>>>>,
    index: Option<DirectorySection>,
    exclude: Option<DirectorySection>,
}

impl DirectorySettings {
    fn array(&self) -> Option<&Spanned<Vec<Spanned<String>>>> {
        self.exclude
            .as_ref()
            .and_then(|exclude| exclude.excluded_directories.as_ref())
            .or_else(|| {
                self.index
                    .as_ref()
                    .and_then(|index| index.excluded_directories.as_ref())
            })
            .or(self.excluded_directories.as_ref())
    }
}

/// Replace the existing target atomically, without replacing a user's config symlink.
pub(super) fn write_migration(path: &Path, original: &str, updated: &str) -> std::io::Result<()> {
    let target = path.canonicalize()?;
    let temporary = target.with_file_name(format!(".codemap-config-{}.tmp", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.set_permissions(std::fs::metadata(&target)?.permissions())?;
        file.write_all(updated.as_bytes())?;
        file.sync_all()?;
        if std::fs::read_to_string(&target)? != original {
            return Err(std::io::Error::other(
                "config changed during migration; retry on next startup",
            ));
        }
        drop(file);
        std::fs::rename(&temporary, &target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub(super) fn fresh_config(template: &str, root: &Path) -> Result<String, String> {
    let settings: DirectorySettings =
        toml::from_str(template).map_err(|error| error.to_string())?;
    let array = settings
        .array()
        .ok_or("template has no excluded_directories array")?;
    let mut output = template.to_string();
    let values = recommended_directories(root);
    let body = values
        .iter()
        .map(|value| format!("    {},\n", toml::Value::String(value.clone())))
        .collect::<String>();
    output.replace_range(array.span(), &format!("[\n{body}]"));
    Ok(output)
}

pub(super) fn migrate_exclusions(
    contents: &str,
    root: &Path,
    inherited: &[String],
) -> Result<String, String> {
    // Reject malformed arrays rather than replacing values whose meaning is unknown.
    let settings: DirectorySettings =
        toml::from_str(contents).map_err(|error| error.to_string())?;
    let original = settings
        .array()
        .map(|array| {
            array
                .get_ref()
                .iter()
                .map(|value| value.get_ref().clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| inherited.to_vec());
    DirectoryExclusions::new(&original)?;
    let mut values = original.clone();
    let mut seen: BTreeSet<String> = original.iter().map(|s| s.replace('\\', "/")).collect();
    for value in crate::workspace::EXCLUDED_DIRS
        .iter()
        .map(|s| s.to_string())
        .chain(recommended_directories(root))
    {
        if seen.insert(value.clone()) {
            values.push(value);
        }
    }
    let mut output = contents.to_string();
    if let Some(array) = settings.array() {
        let additions = &values[original.len()..];
        if !additions.is_empty() {
            let close = array.span().end - 1;
            if contents.as_bytes().get(close) != Some(&b']') {
                return Err("cannot safely locate the end of excluded_directories".into());
            }
            let added = additions
                .iter()
                .map(|value| toml::Value::String(value.clone()).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let separator = if contents[array.span()].contains('\n') {
                "\n    "
            } else {
                " "
            };
            output.insert_str(close, &format!("{separator}{added} "));
            if let Some(last) = array.get_ref().last() {
                let tail = &contents[last.span().end..close];
                let has_comma = tail
                    .lines()
                    .any(|line| line.split('#').next().unwrap_or("").contains(','));
                if !has_comma {
                    output.insert(last.span().end, ',');
                }
            }
        }
    } else {
        let serialized = values
            .iter()
            .map(|value| toml::Value::String(value.clone()).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let assignment = format!("excluded_directories = [{serialized}]\n");
        if settings.exclude.is_some() || settings.index.is_some() {
            let section = if settings.exclude.is_some() {
                "exclude"
            } else {
                "index"
            };
            let header = regex::Regex::new(&format!(
                r#"(?m)^[\t ]*\[(?:{section}|"{section}"|'{section}')\][\t ]*(?:#[^\r\n]*)?\r?$"#,
            ))
            .unwrap();
            let header = header.find(contents).ok_or("cannot safely insert excluded_directories into this table; add the array explicitly and retry")?;
            let at =
                header.end() + usize::from(contents.as_bytes().get(header.end()) == Some(&b'\n'));
            output.insert_str(at, &format!("\n{assignment}"));
        } else {
            output.push_str(&format!("\n[exclude]\n{assignment}"));
        }
    }
    // A header-looking line inside a multiline string must never be treated as a table.
    let before: toml::Value = toml::from_str(contents).map_err(|error| error.to_string())?;
    let after: toml::Value = toml::from_str(&output).map_err(|error| error.to_string())?;
    let after_settings: DirectorySettings =
        toml::from_str(&output).map_err(|error| error.to_string())?;
    let actual: Vec<_> = after_settings
        .array()
        .ok_or("missing migrated array")?
        .get_ref()
        .iter()
        .map(|value| value.get_ref().clone())
        .collect();
    if actual != values || without_exclusions(before) != without_exclusions(after) {
        return Err("directory migration would change unrelated settings".into());
    }
    Ok(output)
}

fn without_exclusions(mut value: toml::Value) -> toml::Value {
    if let Some(table) = value.as_table_mut() {
        table.remove("excluded_directories");
        for section in ["index", "exclude"] {
            if let Some(settings) = table.get_mut(section).and_then(toml::Value::as_table_mut) {
                settings.remove("excluded_directories");
                if settings.is_empty() {
                    table.remove(section);
                }
            }
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_migration_preserves_concurrent_edits_and_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config.toml");
        std::fs::write(&path, "original").unwrap();
        assert!(write_migration(&path, "stale", "updated").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        write_migration(&path, "original", "updated").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "updated");
        #[cfg(unix)]
        {
            let link = root.path().join("linked.toml");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            write_migration(&link, "updated", "through link").unwrap();
            assert!(link.is_symlink());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "through link");
        }
    }

    #[test]
    fn test_migration_preserves_comments_strings_and_sections() {
        let root = tempfile::tempdir().unwrap();
        let source = "# personal\n[index]\nexcluded_directories = [\n  'custom', # keep this comma, comment\n  'a#b' # keep this too\n]\nmax_file_size = 2000\n[refresh]\nwatch = false\n";
        let updated = migrate_exclusions(source, root.path(), &[]).unwrap();
        assert!(updated.contains("'custom', # keep this comma, comment"));
        assert!(updated.contains("'a#b', # keep this too"));
        assert!(updated.contains("max_file_size = 2000\n[refresh]\nwatch = false"));
        assert!(updated.contains("\"node_modules\""));
        assert!(updated.contains("\".vscode\""));
        assert_eq!(
            updated,
            migrate_exclusions(&updated, root.path(), &[]).unwrap()
        );
    }

    #[test]
    fn test_legacy_inline_missing_and_empty_arrays() {
        let root = tempfile::tempdir().unwrap();
        for source in [
            "excluded_directories = []\n",
            "index = { excluded_directories = ['custom'] }\n",
            "[index]\nmax_file_size = 1000\n",
            "[search]\nresult_threshold = 2\n",
        ] {
            let updated = migrate_exclusions(source, root.path(), &["inherited".into()]).unwrap();
            assert!(updated.contains("\"node_modules\""), "{updated}");
        }
        for source in [
            "[index]\nexcluded_directories = 4",
            "[index]\nexcluded_directories = ['../bad']",
            "bad = [",
        ] {
            assert!(migrate_exclusions(source, root.path(), &[]).is_err());
        }
    }

    #[test]
    fn test_localized_templates_generate_equivalent_settings() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("package.json"), "{}").unwrap();
        let english = fresh_config(super::super::CONFIG_TEMPLATE, root.path()).unwrap();
        let korean = fresh_config(super::super::CONFIG_TEMPLATE_KO, root.path()).unwrap();
        assert_eq!(
            toml::from_str::<toml::Value>(&english).unwrap(),
            toml::from_str::<toml::Value>(&korean).unwrap()
        );
        assert!(english.contains("\"**/node_modules\""));
        assert!(!english.contains("\"**/target\""));
    }
}
