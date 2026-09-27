//! Credentials are kept out of `config.toml`. Auth files are optional and never
//! rewritten; diagnostics deliberately omit TOML parser details and user-supplied
//! key names because either can contain credentials.

use std::io::Write;
use std::path::Path;

use crate::config_locale::config_comment_language;
use crate::jev::SecretString;

pub(crate) const AUTH_FILE_NAME: &str = "auth.toml";

#[derive(Debug, Clone, Default)]
pub struct AuthConfig {
    /// Resolved from repo/global `auth.toml`; the environment fallback is request-local.
    pub jev_api_key: Option<SecretString>,
}

pub(super) fn load(repo_root: &Path, global_dir: &Path) -> AuthConfig {
    let repo_path = repo_root.join(super::CODEMAP_DIR_NAME).join(AUTH_FILE_NAME);
    AuthConfig {
        jev_api_key: read_jev_api_key(&repo_path)
            .or_else(|| read_jev_api_key(&global_dir.join(AUTH_FILE_NAME))),
    }
}

fn read_jev_api_key(path: &Path) -> Option<SecretString> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            super::warn(&format!(
                "auth read failed: {}: {error} — ignored",
                path.display()
            ));
            return None;
        }
    };
    let value: toml::Value = match toml::from_str(&contents) {
        Ok(value) => value,
        Err(_) => {
            // A TOML error's Display includes the offending source line.
            super::warn(&format!(
                "auth parse failed: {}: invalid TOML — ignored",
                path.display()
            ));
            return None;
        }
    };
    let table = value.as_table()?;
    if table.keys().any(|key| key != "jev") {
        super::warn(&format!(
            "unknown auth sections: {} — ignored",
            path.display()
        ));
    }
    let jev = table.get("jev")?;
    let Some(jev) = jev.as_table() else {
        super::warn(&format!(
            "auth 'jev' must be a table: {} — ignored",
            path.display()
        ));
        return None;
    };
    if jev.keys().any(|key| key != "api_key") {
        super::warn(&format!(
            "unknown auth keys in 'jev': {} — ignored",
            path.display()
        ));
    }
    match jev.get("api_key")?.as_str() {
        Some(value) if value.trim().is_empty() => None,
        Some(value) => Some(SecretString::new(value)),
        None => {
            super::warn(&format!(
                "auth 'jev.api_key' must be a string: {} — ignored",
                path.display()
            ));
            None
        }
    }
}

/// Create only an empty credential template, never copy keys from the environment.
/// `create_new` preserves existing files and symlinks without reading their contents.
pub(super) fn ensure_repo_auth(repo_root: &Path) {
    let dir = repo_root.join(super::CODEMAP_DIR_NAME);
    let path = dir.join(AUTH_FILE_NAME);
    if let Err(error) = std::fs::create_dir_all(&dir) {
        super::warn(&format!(
            "auth template skipped: create {}: {error}",
            dir.display()
        ));
        return;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return,
        Err(error) => {
            super::warn(&format!(
                "auth template skipped: write {}: {error}",
                path.display()
            ));
            return;
        }
    };
    let template = config_comment_language().select(
        include_str!("../auth_template.toml"),
        include_str!("../auth_template.ko.toml"),
    );
    if let Err(error) = file.write_all(template.as_bytes()) {
        super::warn(&format!(
            "auth template skipped: write {}: {error}",
            path.display()
        ));
        return;
    }
    super::warn(&format!("created empty auth template: {}", path.display()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_layers_fall_back_per_key_without_changing_jev_settings() {
        let repo = tempfile::tempdir().unwrap();
        let global = tempfile::tempdir().unwrap();
        let dir = repo.path().join(super::super::CODEMAP_DIR_NAME);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            global.path().join(AUTH_FILE_NAME),
            "[jev]\napi_key = 'global-test-key'\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("config.toml"),
            "[analysis.jev]\nsearch_filter_enabled = true\ntimeout_ms = 5000\n",
        )
        .unwrap();
        let path = dir.join(AUTH_FILE_NAME);
        for body in [
            "",
            "[jev]\n",
            "[jev]\napi_key = ''\n",
            "[jev]\napi_key = '   '\n",
            "[jev]\napi_key = 123\n",
            "jev = false\n",
            "[jev]\napi_key = 'unterminated-test-key\n",
        ] {
            std::fs::write(&path, body).unwrap();
            let resolved = super::super::load(repo.path(), global.path());
            assert_eq!(
                resolved.auth.jev_api_key.as_ref().unwrap().expose(),
                "global-test-key"
            );
            assert!(resolved.jev.search_filter_enabled);
            assert_eq!(resolved.jev.timeout_ms, 5000);
        }
        std::fs::write(
            &path,
            "[jev]\napi_key = 'repo-test-key'\nmodel = 'ignored'\ntimeout_ms = 99\n",
        )
        .unwrap();
        let resolved = super::super::load(repo.path(), global.path());
        assert_eq!(
            resolved.auth.jev_api_key.as_ref().unwrap().expose(),
            "repo-test-key"
        );
        assert_eq!(resolved.jev.timeout_ms, 5000);
        let debug = format!("{resolved:?}");
        assert!(!debug.contains("repo-test-key"));
        assert!(!debug.contains("global-test-key"));
        assert!(debug.contains("SecretString([REDACTED])"));
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(global.path().join(AUTH_FILE_NAME)).unwrap();
        assert!(load(repo.path(), global.path()).jev_api_key.is_none());
    }

    #[test]
    fn test_auth_scaffold_is_empty_private_and_preserves_existing_files() {
        let repo = tempfile::tempdir().unwrap();
        ensure_repo_auth(repo.path());
        let path = repo
            .path()
            .join(super::super::CODEMAP_DIR_NAME)
            .join(AUTH_FILE_NAME);
        let original = std::fs::read_to_string(&path).unwrap();
        assert!(read_jev_api_key(&path).is_none());
        let parsed: toml::Value = toml::from_str(&original).unwrap();
        assert!(parsed["jev"].as_table().unwrap().is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        ensure_repo_auth(repo.path());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        let existing = "# keep this note\n[jev]\napi_key = 'existing-test-key'\n";
        std::fs::write(&path, existing).unwrap();
        ensure_repo_auth(repo.path());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), existing);
    }

    #[cfg(unix)]
    #[test]
    fn test_auth_scaffold_preserves_symlinks() {
        let repo = tempfile::tempdir().unwrap();
        let dir = repo.path().join(super::super::CODEMAP_DIR_NAME);
        std::fs::create_dir_all(&dir).unwrap();
        let target = repo.path().join("private.toml");
        let existing = "[jev]\napi_key = 'symlink-test-key'\n";
        std::fs::write(&target, existing).unwrap();
        let path = dir.join(AUTH_FILE_NAME);
        std::os::unix::fs::symlink(&target, &path).unwrap();
        ensure_repo_auth(repo.path());
        assert!(std::fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), existing);
    }
}
