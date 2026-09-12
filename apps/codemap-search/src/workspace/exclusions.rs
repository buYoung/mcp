//! Directory rules shared by live walks, index refreshes and initial config generation.

use std::collections::BTreeSet;
use std::path::Path;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

pub(crate) const COMMON_DIRECTORIES: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    ".bzr",
    ".jj",
    ".sl",
    ".idea",
    ".vscode",
    ".vs",
    ".codemap",
    ".codemap-index",
];

#[derive(Clone, Debug)]
pub(crate) struct DirectoryExclusions {
    names: GlobSet,
    paths: GlobSet,
}

impl DirectoryExclusions {
    pub(crate) fn new(patterns: &[String]) -> Result<Self, String> {
        let mut names = GlobSetBuilder::new();
        let mut paths = GlobSetBuilder::new();
        for pattern in patterns {
            let normalized = normalize_pattern(pattern)?;
            let is_anchored = normalized.contains('/');
            let glob = GlobBuilder::new(normalized.strip_prefix("./").unwrap_or(&normalized))
                .literal_separator(true)
                .backslash_escape(false)
                .build()
                .map_err(|error| format!("invalid excluded directory '{pattern}': {error}"))?;
            if is_anchored {
                paths.add(glob);
            } else {
                names.add(glob);
            }
        }
        Ok(Self {
            names: names.build().map_err(|error| error.to_string())?,
            paths: paths.build().map_err(|error| error.to_string())?,
        })
    }

    /// Call only for directories (or ancestors of a file), never for a file basename.
    pub(crate) fn matches_directory(&self, path: &Path, workspace_root: &Path) -> bool {
        path.file_name()
            .is_some_and(|name| self.names.is_match(Path::new(name)))
            || path.strip_prefix(workspace_root).is_ok_and(|relative| {
                self.paths
                    .is_match(relative.to_string_lossy().replace('\\', "/"))
            })
    }
}

pub(crate) fn normalize_pattern(pattern: &str) -> Result<String, String> {
    let normalized = pattern.replace('\\', "/");
    if normalized.trim().is_empty()
        || normalized.starts_with('/')
        || normalized.as_bytes().get(1) == Some(&b':')
        || normalized.split('/').any(|component| component == "..")
        || normalized.trim_matches(['.', '/']).is_empty()
    {
        return Err(format!(
            "excluded directory must be a name or workspace-relative glob: {pattern:?}"
        ));
    }
    Ok(normalized)
}

/// Read-only, one-time discovery. Profiles produce recursive globs across the workspace.
pub(crate) fn recommended_directories(root: &Path) -> Vec<String> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut recommendations: BTreeSet<String> =
        COMMON_DIRECTORIES.iter().map(|s| s.to_string()).collect();
    let mut matcher =
        DirectoryExclusions::new(&recommendations.iter().cloned().collect::<Vec<_>>())
            .expect("common directory patterns are valid");
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        if directory != root && matcher.matches_directory(&directory, &root) {
            continue;
        }
        let mut walker = ignore::WalkBuilder::new(&directory);
        super::apply_ignore_settings(&mut walker, true);
        walker.max_depth(Some(1)).follow_links(false);
        let entries: Vec<_> = walker
            .build()
            .filter_map(|entry| match entry {
                Ok(entry) if entry.depth() > 0 => Some(entry),
                Ok(_) => None,
                Err(error) => {
                    tracing::warn!("project discovery skipped an unreadable path: {error}");
                    None
                }
            })
            .collect();
        let names: Vec<_> = entries
            .iter()
            .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
            .filter_map(|entry| entry.file_name().to_str())
            .collect();
        let previous_len = recommendations.len();
        for pattern in project_patterns(&names) {
            // Keep recommendations independent of the discovered project's location.
            recommendations.insert(format!(
                "**/{}",
                pattern.strip_prefix("**/").unwrap_or(pattern)
            ));
        }
        if recommendations.len() != previous_len {
            matcher =
                DirectoryExclusions::new(&recommendations.iter().cloned().collect::<Vec<_>>())
                    .expect("built-in recursive directory patterns are valid");
        }
        let mut children: Vec<_> = entries
            .into_iter()
            .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_dir()))
            .map(|entry| entry.into_path())
            .filter(|path| !matcher.matches_directory(path, &root))
            // Orphan dependency trees must not become projects during discovery either.
            .filter(|path| {
                !path.file_name().is_some_and(|name| {
                    ["node_modules", ".yarn", ".venv", "venv", "__pycache__"]
                        .iter()
                        .any(|candidate| name == *candidate)
                })
            })
            .collect();
        children.sort();
        pending.extend(children.into_iter().rev());
    }
    recommendations.into_iter().collect()
}

fn project_patterns(names: &[&str]) -> BTreeSet<&'static str> {
    let mut patterns = BTreeSet::new();
    let mut add = |markers: &[&str], directories: &[&'static str]| {
        if markers.iter().any(|marker| names.contains(marker)) {
            patterns.extend(directories.iter().copied());
        }
    };
    add(
        &["package.json"],
        &[
            "node_modules",
            ".yarn",
            "dist",
            "build",
            "coverage",
            ".next",
            ".nuxt",
            ".output",
            ".svelte-kit",
            ".astro",
            ".turbo",
            ".parcel-cache",
        ],
    );
    add(&["Cargo.toml"], &["target"]);
    add(&["go.mod", "go.work"], &["vendor"]);
    add(&["pom.xml"], &["target"]);
    add(
        &[
            "build.gradle",
            "build.gradle.kts",
            "settings.gradle",
            "settings.gradle.kts",
        ],
        &[".gradle", "build"],
    );
    add(
        &["build.sbt"],
        &["target", "project/target", ".bloop", ".metals"],
    );
    add(&["composer.json"], &["vendor"]);
    add(&["Gemfile"], &["vendor/bundle"]);
    add(&["Package.swift"], &[".build"]);
    add(&["Podfile"], &["Pods"]);
    add(&["Cartfile"], &["Carthage/Build"]);
    add(&["pubspec.yaml"], &[".dart_tool", "build"]);
    add(
        &["CMakeLists.txt"],
        &["build", "cmake-build-*", "CMakeFiles", "_deps"],
    );
    add(
        &[
            "MODULE.bazel",
            "WORKSPACE",
            "WORKSPACE.bazel",
            "BUILD",
            "BUILD.bazel",
        ],
        &["bazel-bin", "bazel-out", "bazel-testlogs"],
    );
    if names.iter().any(|name| {
        ["pyproject.toml", "setup.py", "setup.cfg", "Pipfile"].contains(name)
            || (name.starts_with("requirements") && name.ends_with(".txt"))
    }) {
        patterns.extend([
            ".venv",
            "venv",
            "**/__pycache__",
            ".pytest_cache",
            ".mypy_cache",
            ".ruff_cache",
            ".tox",
            ".nox",
            "build",
            "dist",
            "**/*.egg-info",
        ]);
    }
    if names.iter().any(|name| {
        [".csproj", ".sln", ".slnx"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
    }) {
        patterns.extend(["bin", "obj"]);
    }
    if names.iter().any(|name| name.ends_with(".gemspec")) {
        patterns.insert("vendor/bundle");
    }
    if names.iter().any(|name| name.ends_with(".tf")) {
        patterns.insert(".terraform");
    }
    patterns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directory_rules_are_scoped_and_cross_platform() {
        let root = Path::new("/repo");
        let matcher = DirectoryExclusions::new(&[
            ".idea".into(),
            "./build".into(),
            "apps\\web\\dist".into(),
            "apps/api/**/__pycache__".into(),
        ])
        .unwrap();
        for path in [
            ".idea",
            "apps/web/.idea",
            "build",
            "apps/web/dist",
            "apps/api/__pycache__",
            "apps/api/src/__pycache__",
        ] {
            assert!(matcher.matches_directory(&root.join(path), root), "{path}");
        }
        for path in [
            "apps/web/build",
            "apps/api/dist",
            "apps/web/__pycache__",
            "builder",
        ] {
            assert!(!matcher.matches_directory(&root.join(path), root), "{path}");
        }
        for pattern in [
            "",
            " ",
            "/tmp",
            "C:\\tmp",
            "../build",
            "apps/../build",
            ".",
            "[",
        ] {
            assert!(
                DirectoryExclusions::new(&[pattern.into()]).is_err(),
                "{pattern}"
            );
        }
    }

    #[test]
    fn test_project_discovery_uses_globs_and_ignores_generated_projects() {
        let root = tempfile::tempdir().unwrap();
        for path in [
            "apps/web/package.json",
            "apps/api/pyproject.toml",
            "crates/core/Cargo.toml",
            "apps/web/node_modules/lib/composer.json",
            "apps/web/build/Cargo.toml",
            "notes/readme.md",
        ] {
            let path = root.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        let result = recommended_directories(root.path());
        assert!(result.contains(&".vscode".into()));
        assert!(result.contains(&"**/node_modules".into()));
        assert!(result.contains(&"**/__pycache__".into()));
        assert!(result.contains(&"**/target".into()));
        assert!(!result.contains(&"build".into()));
        assert!(!result.contains(&"**/vendor".into()));
        assert!(result
            .iter()
            .all(|pattern| !pattern.contains('/') || pattern.starts_with("**/")));
        assert_eq!(result, recommended_directories(root.path()));
    }

    #[test]
    fn test_all_project_profiles() {
        for (marker, expected) in [
            ("package.json", "node_modules"),
            ("requirements-dev.txt", "**/__pycache__"),
            ("Cargo.toml", "target"),
            ("go.work", "vendor"),
            ("pom.xml", "target"),
            ("build.gradle.kts", ".gradle"),
            ("build.sbt", "project/target"),
            ("App.csproj", "obj"),
            ("composer.json", "vendor"),
            ("app.gemspec", "vendor/bundle"),
            ("Package.swift", ".build"),
            ("Podfile", "Pods"),
            ("Cartfile", "Carthage/Build"),
            ("pubspec.yaml", ".dart_tool"),
            ("CMakeLists.txt", "cmake-build-*"),
            ("MODULE.bazel", "bazel-out"),
            ("main.tf", ".terraform"),
        ] {
            assert!(project_patterns(&[marker]).contains(expected), "{marker}");
        }
        assert!(project_patterns(&["main.sql", "main.lua", "README.md", "script.ps1"]).is_empty());
    }
}
