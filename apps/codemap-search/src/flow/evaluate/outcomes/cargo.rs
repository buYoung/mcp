use super::*;
use std::path::Path;

pub(super) struct Dependency {
    pub evidence: Vec<Location>,
    pub roots: Vec<String>,
}

impl Query<'_> {
    /// Build metadata is read live, not cached across index generations. It uses
    /// the same file/source-byte/time limits as source relationship analysis.
    pub(super) fn model_metadata(&mut self, path: &Path) -> Option<String> {
        let canonical_root = crate::workspace::canonicalize_path_lenient(self.root);
        let path = crate::workspace::canonicalize_path_lenient(path);
        if !path.starts_with(&canonical_root) {
            return None;
        }
        let key = crate::workspace::workspace_relative_key(&path, &canonical_root);
        let location = Location {
            path: key.clone(),
            range: CodeRange {
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 2,
            },
            name: "API model build evidence".into(),
        };
        if !self.tick(&location)
            || self.files.len() >= FILES_PER_QUERY && !self.files.contains_key(&key)
        {
            return None;
        }
        use std::io::Read;
        let remaining = (8 * 1024 * 1024usize)
            .saturating_sub(self.source_bytes)
            .min(crate::config::get().max_file_size as usize);
        let mut bytes = Vec::new();
        std::fs::File::open(&path)
            .ok()?
            .take(remaining as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        self.source_bytes += bytes.len();
        if bytes.len() > remaining {
            self.analysis_limit(&location, "API model source byte budget reached");
            return None;
        }
        if let Some(expected) = self.index.digest(&key) {
            if crate::implementations::digest(&bytes) != expected {
                self.diagnostic(
                    &location,
                    "model dependency changed since indexing; stale relationships withheld",
                );
                return None;
            }
        }
        if self.filter.is_file_excluded(&key) {
            return None;
        }
        self.files.insert(key, true);
        String::from_utf8(bytes).ok()
    }
    pub(super) fn serde_dependency(&mut self, path: &str, name: &str) -> Option<Dependency> {
        let file = self.root.join(path);
        let mut manifests = Vec::new();
        let mut locks = Vec::new();
        let mut directory = file.parent()?.to_path_buf();
        for _ in 0..24 {
            let manifest_path = directory.join("Cargo.toml");
            if manifest_path.is_file() {
                let source = self.model_metadata(&manifest_path)?;
                let manifest: toml::Value = toml::from_str(&source).ok()?;
                if manifest.get("patch").is_some() || manifest.get("replace").is_some() {
                    return None;
                }
                manifests.push((manifest_path, source, manifest));
            }
            let lock = directory.join("Cargo.lock");
            if lock.is_file() {
                locks.push(lock);
            }
            for config in [
                directory.join(".cargo/config.toml"),
                directory.join(".cargo/config"),
            ] {
                if config.is_file() {
                    let config: toml::Value =
                        toml::from_str(&self.model_metadata(&config)?).ok()?;
                    if config.get("source").is_some()
                        || config.get("paths").is_some()
                        || config.get("patch").is_some()
                    {
                        return None;
                    }
                }
            }
            if directory == self.root {
                break;
            }
            if !directory.pop() || !directory.starts_with(self.root) {
                break;
            }
        }
        let (manifest_path, source, package) = manifests
            .iter()
            .find(|(_, _, manifest)| manifest.get("package").is_some())?;
        let dependencies = package.get("dependencies")?.as_table()?;
        if ["std", "core", "alloc"]
            .iter()
            .any(|key| dependencies.contains_key(*key))
        {
            return None;
        }
        let named = dependencies
            .iter()
            .filter(|(key, _)| key.replace('-', "_") == name)
            .collect::<Vec<_>>();
        if named.len() != 1 {
            return None;
        }
        let (cargo_name, mut dependency) = named[0];
        if dependency.get("optional").and_then(toml::Value::as_bool) == Some(true) {
            return None;
        }
        if dependency.get("workspace").and_then(toml::Value::as_bool) == Some(true) {
            dependency = manifests.iter().find_map(|(_, _, manifest)| {
                manifest
                    .get("workspace")?
                    .get("dependencies")?
                    .get(cargo_name)
            })?;
        }
        let version = if let Some(version) = dependency.as_str() {
            if name != "serde_json" {
                return None;
            }
            version
        } else {
            let table = dependency.as_table()?;
            if ["path", "git", "registry", "registry-index"]
                .iter()
                .any(|key| table.contains_key(*key))
                || table.get("optional").and_then(toml::Value::as_bool) == Some(true)
            {
                return None;
            }
            if table
                .get("package")
                .and_then(toml::Value::as_str)
                .unwrap_or(name)
                != "serde_json"
            {
                return None;
            }
            table.get("version")?.as_str()?
        };
        // This model is for the serde_json 1.x API. Wider/unknown dependency
        // requirements and non-registry overrides need a separate proof.
        let version = version.trim().trim_start_matches(['^', '~', '=']);
        if version != "1" && !version.starts_with("1.")
            || version.contains([',', ' ', '>', '<', '|'])
        {
            return None;
        }
        let lock: toml::Value = toml::from_str(&self.model_metadata(locks.first()?)?).ok()?;
        let packages = lock
            .get("package")?
            .as_array()?
            .iter()
            .filter(|package| {
                package.get("name").and_then(toml::Value::as_str) == Some("serde_json")
            })
            .collect::<Vec<_>>();
        if packages.len() != 1 {
            return None;
        }
        let locked = packages[0];
        if !locked.get("version")?.as_str()?.starts_with("1.")
            || locked.get("source")?.as_str()?
                != "registry+https://github.com/rust-lang/crates.io-index"
            || locked
                .get("checksum")
                .and_then(toml::Value::as_str)
                .is_none()
        {
            return None;
        }
        if dependency.get("target").is_some()
            || package.get("target").is_some_and(|targets| {
                targets.as_table().is_some_and(|targets| {
                    targets.values().any(|target| {
                        target
                            .get("dependencies")
                            .and_then(toml::Value::as_table)
                            .is_some_and(|deps| {
                                deps.keys().any(|key| key.replace('-', "_") == name)
                            })
                    })
                })
            })
        {
            return None;
        }
        let base = manifest_path.parent()?;
        let mut roots = Vec::new();
        if let Some(path) = package
            .get("lib")
            .and_then(|lib| lib.get("path"))
            .and_then(toml::Value::as_str)
        {
            roots.push(base.join(path));
        } else if base.join("src/lib.rs").is_file() {
            roots.push(base.join("src/lib.rs"));
        }
        if let Some(bins) = package.get("bin").and_then(toml::Value::as_array) {
            roots.extend(
                bins.iter()
                    .filter_map(|bin| bin.get("path").and_then(toml::Value::as_str))
                    .map(|path| base.join(path)),
            );
        }
        if base.join("src/main.rs").is_file() {
            roots.push(base.join("src/main.rs"));
        }
        if roots.is_empty()
            || !roots.iter().any(|root| {
                root.parent()
                    .is_some_and(|directory| file.starts_with(directory))
            })
        {
            return None;
        }
        if file.starts_with(base.join("src/bin")) && package.get("bin").is_none() {
            return None;
        }
        let roots = roots
            .into_iter()
            .map(|root| crate::workspace::workspace_relative_key(&root, self.root))
            .collect();
        let line = source
            .lines()
            .position(|line| line.trim_start().starts_with(name))
            .map_or(1, |line| line + 1);
        Some(Dependency {
            roots,
            evidence: vec![Location {
                path: crate::workspace::workspace_relative_key(manifest_path, self.root),
                range: CodeRange {
                    start_line: line,
                    start_col: 1,
                    end_line: line,
                    end_col: 2,
                },
                name: format!(
                    "registry serde_json {} as {name}",
                    locked.get("version")?.as_str()?
                ),
            }],
        })
    }
}
